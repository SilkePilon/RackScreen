//! Radarr, Sonarr and Lidarr queues over the kube tunnel: the active download
//! and the items that finished, for the media role's card.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use kube::Client;
use rackscreen_core::event::{Download, Event};
use serde_json::Value;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArrKind {
    Radarr,
    Sonarr,
    Lidarr,
}

impl ArrKind {
    pub fn name(self) -> &'static str {
        match self {
            ArrKind::Radarr => "radarr",
            ArrKind::Sonarr => "sonarr",
            ArrKind::Lidarr => "lidarr",
        }
    }
    /// The queue page, with the joins that carry the human titles.
    pub fn queue_path(self) -> &'static str {
        match self {
            ArrKind::Radarr => "/api/v3/queue?pageSize=5&includeMovie=true",
            ArrKind::Sonarr => "/api/v3/queue?pageSize=5&includeSeries=true&includeEpisode=true",
            ArrKind::Lidarr => "/api/v1/queue?pageSize=5&includeArtist=true&includeAlbum=true",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArrApp {
    pub kind: ArrKind,
    pub service: String,
    pub port: u16,
    pub api_key: String,
}

#[derive(Clone, Debug)]
pub struct ArrConfig {
    pub namespace: String,
    /// Only apps with an API key.
    pub apps: Vec<ArrApp>,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    /// `app/id`, unique across the three apps.
    pub key: String,
    pub title: String,
    pub size: f64,
    pub sizeleft: f64,
    /// seconds; negative means unknown
    pub timeleft_secs: i64,
    pub status: String,
}

impl QueueItem {
    /// Finished downloading (imported or waiting to be).
    pub fn done(&self) -> bool {
        self.status == "completed" || (self.size > 0.0 && self.sizeleft <= 0.0)
    }
    pub fn progress(&self) -> f32 {
        if self.size <= 0.0 {
            return 0.0;
        }
        (1.0 - self.sizeleft / self.size).clamp(0.0, 1.0) as f32
    }
}

/// `hh:mm:ss`, or `d.hh:mm:ss`; anything else is unknown (`-1`).
pub fn parse_timeleft(s: &str) -> i64 {
    let (days, rest) = match s.split_once('.') {
        Some((d, r)) => (d.parse::<i64>().unwrap_or(-1), r),
        None => (0, s),
    };
    let parts: Vec<i64> = rest.split(':').filter_map(|p| p.parse().ok()).collect();
    if days < 0 || parts.len() != 3 {
        return -1;
    }
    days * 86_400 + parts[0] * 3600 + parts[1] * 60 + parts[2]
}

fn title_of(kind: ArrKind, r: &Value) -> String {
    let s = |path: &str| r.pointer(path).and_then(Value::as_str).map(str::to_string);
    let joined = match kind {
        ArrKind::Radarr => s("/movie/title"),
        ArrKind::Sonarr => match (
            s("/series/title"),
            r.pointer("/episode/seasonNumber").and_then(Value::as_u64),
            r.pointer("/episode/episodeNumber").and_then(Value::as_u64),
        ) {
            (Some(series), Some(sn), Some(ep)) => Some(format!("{series} · S{sn}E{ep}")),
            (Some(series), _, _) => Some(series),
            _ => None,
        },
        ArrKind::Lidarr => match (s("/artist/artistName"), s("/album/title")) {
            (Some(a), Some(b)) => Some(format!("{a} · {b}")),
            (Some(a), None) => Some(a),
            _ => None,
        },
    };
    joined
        .or_else(|| s("/title"))
        .unwrap_or_else(|| "download".into())
}

pub fn parse_queue(kind: ArrKind, json: &str) -> Vec<QueueItem> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    v.get("records")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| {
                    let id = r.get("id")?.as_u64()?;
                    Some(QueueItem {
                        key: format!("{}/{id}", kind.name()),
                        title: title_of(kind, r),
                        size: r.get("size").and_then(Value::as_f64).unwrap_or(0.0),
                        sizeleft: r.get("sizeleft").and_then(Value::as_f64).unwrap_or(0.0),
                        timeleft_secs: r
                            .get("timeleft")
                            .and_then(Value::as_str)
                            .map(parse_timeleft)
                            .unwrap_or(-1),
                        status: r
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The unfinished item with the least time left; an unknown ETA sorts last.
pub fn pick_download(items: &[QueueItem]) -> Option<Download> {
    items
        .iter()
        .filter(|i| !i.done())
        .min_by_key(|i| {
            if i.timeleft_secs < 0 {
                i64::MAX
            } else {
                i.timeleft_secs
            }
        })
        .map(|i| Download {
            title: i.title.clone(),
            progress: i.progress(),
            eta_secs: i.timeleft_secs,
        })
}

/// Emits `MediaAdded` for items last seen completed that are gone now. The
/// first poll only primes.
#[derive(Debug, Default)]
pub struct QueueTracker {
    seen: HashMap<String, (String, bool)>,
    primed: bool,
}

impl QueueTracker {
    pub fn diff(&mut self, items: &[QueueItem]) -> Vec<Event> {
        let now: HashMap<String, (String, bool)> = items
            .iter()
            .map(|i| (i.key.clone(), (i.title.clone(), i.done())))
            .collect();
        let mut out = Vec::new();
        if self.primed {
            let mut gone: Vec<(&String, &(String, bool))> = self
                .seen
                .iter()
                .filter(|(k, (_, done))| *done && !now.contains_key(*k))
                .collect();
            gone.sort_by(|a, b| a.0.cmp(b.0));
            for (_, (title, _)) in gone {
                out.push(Event::MediaAdded {
                    title: title.clone(),
                });
            }
        }
        self.seen = now;
        self.primed = true;
        out
    }
}

struct AppLink {
    app: ArrApp,
    tunnel: Option<Tunnel>,
}

impl AppLink {
    async fn queue(&mut self, client: &Client, ns: &str) -> Result<Vec<QueueItem>> {
        if self.tunnel.is_none() {
            let (pod, svc) = find_pod_for_service(
                client,
                ns,
                &self.app.service,
                self.app.port,
                self.app.kind.name(),
            )
            .await?;
            tracing::info!(
                "{}: forwarding to {ns}/{pod} (service {svc})",
                self.app.kind.name()
            );
            self.tunnel = Some(Tunnel::open(client, ns, &pod, self.app.port).await?);
        }
        let t = self.tunnel.as_mut().expect("opened above");
        let r = t
            .get(
                self.app.kind.queue_path(),
                &[("x-api-key", self.app.api_key.as_str())],
            )
            .await;
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                self.tunnel = None;
                return Err(e);
            }
        };
        anyhow::ensure!(
            r.status == 200,
            "{} queue HTTP {}",
            self.app.kind.name(),
            r.status
        );
        Ok(parse_queue(self.app.kind, &r.body))
    }
}

pub async fn run_arr(client: Client, cfg: ArrConfig, ctx: SourceCtx) {
    let mut links: Vec<AppLink> = cfg
        .apps
        .into_iter()
        .map(|app| AppLink { app, tunnel: None })
        .collect();
    let mut tracker = QueueTracker::default();
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let mut items: Vec<QueueItem> = Vec::new();
        let mut any_ok = false;
        for link in &mut links {
            match link.queue(&client, &cfg.namespace).await {
                Ok(q) => {
                    any_ok = true;
                    items.extend(q);
                }
                Err(e) => tracing::warn!("{}: {e:#}", link.app.kind.name()),
            }
        }
        if any_ok {
            ctx.emit_all(tracker.diff(&items));
            ctx.emit(Event::Download(pick_download(&items)));
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(2))) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADARR: &str = include_str!("../tests/fixtures/arr-radarr-queue.json");
    const SONARR: &str = include_str!("../tests/fixtures/arr-sonarr-queue.json");
    const LIDARR: &str = include_str!("../tests/fixtures/arr-lidarr-queue.json");

    #[test]
    fn timeleft_parses_hms_and_days() {
        assert_eq!(parse_timeleft("00:19:12"), 19 * 60 + 12);
        assert_eq!(parse_timeleft("1.02:03:04"), 86_400 + 2 * 3600 + 3 * 60 + 4);
        assert_eq!(parse_timeleft("00:00:00"), 0);
        assert_eq!(parse_timeleft(""), -1);
        assert_eq!(parse_timeleft("soon"), -1);
    }

    #[test]
    fn queues_parse_per_app_with_their_titles() {
        let r = parse_queue(ArrKind::Radarr, RADARR);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].key, "radarr/101");
        assert_eq!(r[0].title, "Dune: Part Two");
        assert_eq!(
            (r[0].size, r[0].sizeleft, r[0].timeleft_secs),
            (12e9, 7.8e9, 19 * 60 + 12)
        );
        assert_eq!(r[0].status, "downloading");
        assert_eq!(r[1].title, "Blade Runner 2049");
        assert!(r[1].done(), "completed, nothing left");
        let s = parse_queue(ArrKind::Sonarr, SONARR);
        assert_eq!(s[0].title, "The Expanse · S2E5");
        assert_eq!(s[0].timeleft_secs, 86_400 + 2 * 3600 + 3 * 60 + 4);
        let l = parse_queue(ArrKind::Lidarr, LIDARR);
        assert_eq!(l[0].title, "Daft Punk · Discovery");
        assert_eq!(l[0].key, "lidarr/9");
        assert!(parse_queue(ArrKind::Radarr, "garbage").is_empty());
        let bare = parse_queue(
            ArrKind::Radarr,
            r#"{"records":[{"id":5,"title":"raw.release","size":10,"sizeleft":5}]}"#,
        );
        assert_eq!(
            bare[0].title, "raw.release",
            "falls back to the release title"
        );
        assert_eq!(bare[0].timeleft_secs, -1);
    }

    #[test]
    fn the_download_with_the_least_time_left_wins() {
        let mut items = parse_queue(ArrKind::Radarr, RADARR);
        items.extend(parse_queue(ArrKind::Sonarr, SONARR));
        items.extend(parse_queue(ArrKind::Lidarr, LIDARR));
        let d = pick_download(&items).unwrap();
        assert_eq!(d.title, "Daft Punk · Discovery");
        assert!((d.progress - 0.5).abs() < 1e-6);
        assert_eq!(d.eta_secs, 270);
        assert!(
            pick_download(&items[1..2]).is_none(),
            "a completed item is not a download"
        );
        let unknown = vec![QueueItem {
            key: "radarr/1".into(),
            title: "x".into(),
            size: 10.0,
            sizeleft: 2.0,
            timeleft_secs: -1,
            status: "downloading".into(),
        }];
        assert_eq!(
            pick_download(&unknown).unwrap().eta_secs,
            -1,
            "unknown ETA still shows"
        );
        let mut both = unknown.clone();
        both.extend(parse_queue(ArrKind::Lidarr, LIDARR));
        assert_eq!(
            pick_download(&both).unwrap().title,
            "Daft Punk · Discovery",
            "a known ETA beats an unknown one"
        );
    }

    #[test]
    fn tracker_reports_items_that_left_the_queue_completed() {
        let mut t = QueueTracker::default();
        let r = parse_queue(ArrKind::Radarr, RADARR);
        assert!(t.diff(&r).is_empty(), "first poll primes");
        assert!(t.diff(&r).is_empty());
        let only_first = vec![r[0].clone()];
        assert_eq!(
            t.diff(&only_first),
            vec![Event::MediaAdded {
                title: "Blade Runner 2049".into()
            }]
        );
        assert!(
            t.diff(&[]).is_empty(),
            "a downloading item that vanished was removed, not added"
        );
        let mut done = r[0].clone();
        done.sizeleft = 0.0;
        done.status = "completed".into();
        assert!(t.diff(&[done]).is_empty());
        assert_eq!(
            t.diff(&[]),
            vec![Event::MediaAdded {
                title: "Dune: Part Two".into()
            }]
        );
    }
}

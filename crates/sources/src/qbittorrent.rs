//! qBittorrent Web API poller over a port-forward tunnel.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use kube::Client;
use rackscreen_core::event::{Event, LinkTarget, Torrent};
use serde::Deserialize;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct QbitConfig {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RawTorrent {
    pub hash: String,
    pub name: String,
    pub state: String,
    /// 0.0 ..= 1.0
    pub progress: f64,
    #[serde(default)]
    pub eta: i64,
    #[serde(default)]
    pub dlspeed: i64,
}

const ACTIVE_STATES: [&str; 3] = ["downloading", "forcedDL", "metaDL"];
const MIN_SPEED_BPS: i64 = 500;

pub fn parse_torrents(json: &str) -> Vec<RawTorrent> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn active_torrents(raw: &[RawTorrent]) -> Vec<Torrent> {
    let mut out: Vec<Torrent> = raw
        .iter()
        .filter(|t| ACTIVE_STATES.contains(&t.state.as_str()) && t.dlspeed >= MIN_SPEED_BPS)
        .map(|t| Torrent {
            name: t.name.chars().take(30).collect(),
            progress: (t.progress * 100.0) as f32,
            eta_secs: t.eta,
            speed_bps: t.dlspeed,
        })
        .collect();
    out.sort_by(|a, b| {
        b.progress
            .partial_cmp(&a.progress)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(3);
    out
}

/// Detects newly added and finished torrents between polls.
#[derive(Default)]
pub struct TorrentTracker {
    seen: HashMap<String, (String, f64)>,
    primed: bool,
}

impl TorrentTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&mut self, raw: &[RawTorrent]) -> Vec<Event> {
        let mut out = Vec::new();
        let mut now: HashMap<String, (String, f64)> = HashMap::new();
        for t in raw {
            now.insert(t.hash.clone(), (t.name.clone(), t.progress));
            if self.primed && !self.seen.contains_key(&t.hash) {
                out.push(Event::TorrentAdded {
                    name: t.name.clone(),
                });
            }
            if self.primed
                && t.progress >= 1.0
                && self.seen.get(&t.hash).is_some_and(|(_, p)| *p < 1.0)
            {
                out.push(Event::TorrentDone {
                    name: t.name.clone(),
                });
            }
        }
        if self.primed {
            for (hash, (name, progress)) in &self.seen {
                // `*progress < 1.0` keeps us from re-announcing a torrent that already
                // emitted TorrentDone when it was observed at 100% in a previous poll.
                if !now.contains_key(hash) && *progress > 0.99 && *progress < 1.0 {
                    out.push(Event::TorrentDone { name: name.clone() });
                }
            }
        }
        self.seen = now;
        self.primed = true;
        out
    }
}

struct Session {
    tunnel: Tunnel,
    cookie: Option<String>,
    port: u16,
}

impl Session {
    fn origin(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    async fn login(&mut self, user: &str, pass: &str) -> Result<()> {
        let origin = self.origin();
        let body = format!(
            "username={}&password={}",
            urlencoding::encode(user),
            urlencoding::encode(pass)
        );
        let r = self
            .tunnel
            .post_form(
                "/api/v2/auth/login",
                &body,
                &[("referer", origin.as_str()), ("origin", origin.as_str())],
            )
            .await?;
        anyhow::ensure!(
            r.status == 200 && (r.body.trim() == "Ok." || r.body.trim().is_empty()),
            "login HTTP {} {:?}",
            r.status,
            r.body.trim()
        );
        self.cookie = r
            .headers
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| {
                c.split(';')
                    .next()
                    .filter(|p| p.starts_with("SID="))
                    .map(str::to_string)
            });
        Ok(())
    }

    async fn torrents(&mut self) -> Result<Vec<RawTorrent>> {
        let origin = self.origin();
        let cookie = self.cookie.clone().unwrap_or_default();
        let r = self
            .tunnel
            .get(
                "/api/v2/torrents/info?filter=downloading",
                &[("referer", origin.as_str()), ("cookie", cookie.as_str())],
            )
            .await?;
        anyhow::ensure!(r.status == 200, "torrents/info HTTP {}", r.status);
        Ok(parse_torrents(&r.body))
    }
}

pub async fn run_qbittorrent(client: Client, cfg: QbitConfig, ctx: SourceCtx) {
    let mut tracker = TorrentTracker::new();
    let mut backoff = 5u64;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let session = async {
            let (pod, svc) =
                find_pod_for_service(&client, &cfg.namespace, &cfg.service, cfg.port, "qbit")
                    .await?;
            tracing::info!(
                "qbittorrent: forwarding to {}/{pod} (service {svc})",
                cfg.namespace
            );
            let tunnel = Tunnel::open(&client, &cfg.namespace, &pod, cfg.port).await?;
            let mut s = Session {
                tunnel,
                cookie: None,
                port: cfg.port,
            };
            s.login(&cfg.user, &cfg.pass).await.context("login")?;
            Ok::<_, anyhow::Error>(s)
        }
        .await;
        let mut s = match session {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("qbittorrent: {e:#}; retry in {backoff}s");
                ctx.emit(Event::Link {
                    target: LinkTarget::QBittorrent,
                    up: false,
                });
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(60);
                continue;
            }
        };
        backoff = 5;
        let mut failures = 0;
        loop {
            match s.torrents().await {
                Ok(raw) => {
                    failures = 0;
                    ctx.emit(Event::Link {
                        target: LinkTarget::QBittorrent,
                        up: true,
                    });
                    ctx.emit_all(tracker.diff(&raw));
                    ctx.emit(Event::Torrents(active_torrents(&raw)));
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("qbittorrent poll: {e:#}");
                    if failures >= 2 {
                        ctx.emit(Event::Link {
                            target: LinkTarget::QBittorrent,
                            up: false,
                        });
                        break;
                    }
                }
            }
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(1))) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(hash: &str, state: &str, progress: f64, speed: i64) -> RawTorrent {
        RawTorrent {
            hash: hash.into(),
            name: format!("t-{hash}"),
            state: state.into(),
            progress,
            eta: 100,
            dlspeed: speed,
        }
    }

    #[test]
    fn parses_api_json() {
        let json = r#"[{"hash":"abc","name":"ubuntu.iso","state":"downloading","progress":0.42,"eta":900,"dlspeed":123456}]"#;
        let v = parse_torrents(json);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "ubuntu.iso");
        assert!(parse_torrents("garbage").is_empty());
    }

    #[test]
    fn active_filters_sorts_truncates() {
        let list = vec![
            raw("a", "downloading", 0.1, 1000),
            raw("b", "stalledDL", 0.5, 1000),
            raw("c", "downloading", 0.9, 100),
            raw("d", "forcedDL", 0.7, 5000),
            raw("e", "metaDL", 0.0, 800),
            raw("f", "downloading", 0.3, 800),
        ];
        let act = active_torrents(&list);
        assert_eq!(act.len(), 3);
        assert_eq!(act[0].name, "t-d");
        assert_eq!(act[0].progress, 70.0);
        assert_eq!(act[2].name, "t-a");
    }

    #[test]
    fn tracker_added_and_done() {
        let mut t = TorrentTracker::new();
        assert!(
            t.diff(&[raw("a", "downloading", 0.5, 1000)]).is_empty(),
            "first poll primes silently"
        );
        let evs = t.diff(&[
            raw("a", "downloading", 0.6, 1000),
            raw("b", "downloading", 0.0, 1000),
        ]);
        assert_eq!(evs, vec![Event::TorrentAdded { name: "t-b".into() }]);
        let evs = t.diff(&[
            raw("a", "downloading", 1.0, 0),
            raw("b", "downloading", 0.1, 1000),
        ]);
        assert_eq!(evs, vec![Event::TorrentDone { name: "t-a".into() }]);
        let evs = t.diff(&[raw("b", "downloading", 0.995, 1000)]);
        assert!(evs.is_empty());
        let evs = t.diff(&[]);
        assert_eq!(evs, vec![Event::TorrentDone { name: "t-b".into() }]);
    }
}

//! Jellyfin now-playing sessions from a server outside the cluster.

use std::time::Duration;

use rackscreen_core::event::{Event, JfKind, JfSession, JfStatus};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct JellyfinConfig {
    pub url: String,
    pub api_key: String,
    pub poll_secs: u64,
}

/// 100 ns ticks to seconds.
fn ticks_secs(v: Option<&Value>) -> f64 {
    v.and_then(Value::as_f64).unwrap_or(0.0) / 10_000_000.0
}

/// Sessions with a `NowPlayingItem`, in the server's order.
pub fn parse_sessions(json: &str) -> Vec<JfSession> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let item = s.get("NowPlayingItem")?;
            let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
            let u32_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
            let kind = match item.get("Type").and_then(Value::as_str) {
                Some("Movie") => JfKind::Movie,
                Some("Episode") => JfKind::Episode,
                _ => JfKind::Other,
            };
            let play = s.get("PlayState");
            Some(JfSession {
                title: str_of(item, "Name").unwrap_or_else(|| "Unknown".into()),
                kind,
                series: str_of(item, "SeriesName"),
                season: u32_of(item, "ParentIndexNumber"),
                episode: u32_of(item, "IndexNumber"),
                year: u32_of(item, "ProductionYear"),
                runtime_secs: ticks_secs(item.get("RunTimeTicks")),
                position_secs: ticks_secs(play.and_then(|p| p.get("PositionTicks"))),
                paused: play
                    .and_then(|p| p.get("IsPaused"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                transcoding: s.get("TranscodingInfo").is_some_and(|t| !t.is_null()),
                user: str_of(s, "UserName").unwrap_or_default(),
                device: str_of(s, "DeviceName").unwrap_or_default(),
                last_activity: str_of(s, "LastActivityDate").unwrap_or_default(),
            })
        })
        .collect()
}

/// What an HTTP outcome means for the tile.
pub fn status_of(status: u16, reached: bool) -> JfStatus {
    match (reached, status) {
        (false, _) => JfStatus::Offline,
        (true, 401 | 403) => JfStatus::Unauthorized,
        (true, 200..=299) => JfStatus::Ok,
        _ => JfStatus::Offline,
    }
}

async fn poll(cfg: &JellyfinConfig) -> Event {
    let url = format!(
        "{}/Sessions?activeWithinSeconds=60",
        cfg.url.trim_end_matches('/')
    );
    let resp = client()
        .get(&url)
        .header(
            "Authorization",
            format!("MediaBrowser Token=\"{}\"", cfg.api_key),
        )
        .send()
        .await;
    match resp {
        Err(e) => {
            tracing::debug!("jellyfin: {e}");
            Event::Jellyfin {
                status: JfStatus::Offline,
                sessions: Vec::new(),
            }
        }
        Ok(r) => {
            let status = status_of(r.status().as_u16(), true);
            let sessions = if status == JfStatus::Ok {
                parse_sessions(&r.text().await.unwrap_or_default())
            } else {
                Vec::new()
            };
            Event::Jellyfin { status, sessions }
        }
    }
}

pub async fn run_jellyfin(cfg: JellyfinConfig, ctx: SourceCtx) {
    let mut last: Option<JfStatus> = None;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let ev = poll(&cfg).await;
        if let Event::Jellyfin { status, .. } = &ev {
            if last != Some(*status) {
                match status {
                    JfStatus::Ok => tracing::info!("jellyfin: sessions ok"),
                    JfStatus::Offline => tracing::warn!("jellyfin: unreachable at {}", cfg.url),
                    JfStatus::Unauthorized => {
                        tracing::warn!("jellyfin: api key rejected; check the key in Configure")
                    }
                }
                last = Some(*status);
            }
        }
        ctx.emit(ev);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(2))) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSIONS: &str = include_str!("../tests/fixtures/jellyfin-sessions.json");
    const EMPTY: &str = include_str!("../tests/fixtures/jellyfin-sessions-empty.json");

    #[test]
    fn sessions_parse_movies_episodes_pause_and_transcoding() {
        let s = parse_sessions(SESSIONS);
        assert_eq!(s.len(), 3, "the session without an item is skipped");
        let movie = &s[0];
        assert_eq!(
            (movie.title.as_str(), movie.kind, movie.year),
            ("Blade Runner 2049", JfKind::Movie, Some(2017))
        );
        assert!(
            (movie.runtime_secs - 9840.0).abs() < 1e-6,
            "ticks are 100 ns"
        );
        assert!((movie.position_secs - 3720.0).abs() < 1e-6);
        assert!(!movie.paused && !movie.transcoding);
        assert_eq!(
            (movie.user.as_str(), movie.device.as_str()),
            ("silke", "Living room TV")
        );
        assert_eq!(movie.last_activity, "2026-09-07T12:00:05.000Z");
        let ep = &s[1];
        assert_eq!((ep.title.as_str(), ep.kind), ("Home", JfKind::Episode));
        assert_eq!(
            (ep.series.as_deref(), ep.season, ep.episode),
            (Some("The Expanse"), Some(2), Some(5))
        );
        assert!(ep.transcoding && !ep.paused);
        assert!(s[2].paused);
        assert!(parse_sessions(EMPTY).is_empty());
        assert!(parse_sessions("nope").is_empty());
        assert_eq!(status_of(200, true), JfStatus::Ok);
        assert_eq!(status_of(401, true), JfStatus::Unauthorized);
        assert_eq!(status_of(403, true), JfStatus::Unauthorized);
        assert_eq!(status_of(502, true), JfStatus::Offline);
        assert_eq!(status_of(200, false), JfStatus::Offline);
    }
}

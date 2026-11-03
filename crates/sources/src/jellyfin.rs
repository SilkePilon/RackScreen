//! Jellyfin now-playing sessions from a server outside the cluster.

use std::time::Duration;

use rackscreen_core::event::{CoverRef, Event, JfKind, JfSession, JfStatus, MusicSource, Track};
use serde_json::Value;

use crate::cover::CoverCache;
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

fn is_audio(item: &Value) -> bool {
    item.get("Type").and_then(Value::as_str) == Some("Audio")
}

/// Video sessions with a `NowPlayingItem`, in the server's order; audio
/// goes to the music screen (`parse_audio`).
pub fn parse_sessions(json: &str) -> Vec<JfSession> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let item = s.get("NowPlayingItem").filter(|i| !is_audio(i))?;
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

/// One audio session, for the music screen.
#[derive(Clone, Debug, PartialEq)]
pub struct JfAudio {
    pub title: String,
    pub artist: String,
    /// `user · device`.
    pub device: String,
    pub duration_secs: f64,
    pub position_secs: f64,
    pub paused: bool,
    /// The item whose primary image is the cover: `AlbumId`, else `Id`.
    pub cover_item: Option<String>,
    pub last_activity: String,
}

impl JfAudio {
    pub fn into_track(self, cover: Option<CoverRef>) -> Track {
        Track {
            source: MusicSource::Jellyfin,
            title: self.title,
            artist: self.artist,
            device: self.device,
            duration_secs: self.duration_secs,
            position_secs: self.position_secs,
            playing: !self.paused,
            since: 0,
            cover,
        }
    }
}

/// Audio sessions, in the server's order.
pub fn parse_audio(json: &str) -> Vec<JfAudio> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let item = s.get("NowPlayingItem").filter(|i| is_audio(i))?;
            let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
            let artist = item
                .get("Artists")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| str_of(item, "AlbumArtist"))
                .unwrap_or_default();
            let play = s.get("PlayState");
            Some(JfAudio {
                title: str_of(item, "Name").unwrap_or_else(|| "Unknown".into()),
                artist,
                device: format!(
                    "{} · {}",
                    str_of(s, "UserName").unwrap_or_default(),
                    str_of(s, "DeviceName").unwrap_or_default()
                ),
                duration_secs: ticks_secs(item.get("RunTimeTicks")),
                position_secs: ticks_secs(play.and_then(|p| p.get("PositionTicks"))),
                paused: play
                    .and_then(|p| p.get("IsPaused"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                cover_item: str_of(item, "AlbumId").or_else(|| str_of(item, "Id")),
                last_activity: str_of(s, "LastActivityDate").unwrap_or_default(),
            })
        })
        .collect()
}

/// The audio session to show: a playing one (the most recently active of
/// those), else the most recently active paused one.
pub fn pick_audio(sessions: &[JfAudio]) -> Option<&JfAudio> {
    sessions
        .iter()
        .max_by(|a, b| (!a.paused, &a.last_activity).cmp(&(!b.paused, &b.last_activity)))
}

/// The primary image of `item`, at most 168 px wide, as a JPEG (the server
/// may store WebP, which the cover decoder does not read).
pub fn cover_url(base: &str, item: &str) -> String {
    format!(
        "{}/Items/{item}/Images/Primary?maxWidth=168&format=jpg",
        base.trim_end_matches('/')
    )
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

fn auth(cfg: &JellyfinConfig) -> String {
    format!("MediaBrowser Token=\"{}\"", cfg.api_key)
}

/// One `/Sessions` poll: the video tile's event and the audio session.
async fn poll(cfg: &JellyfinConfig) -> (Event, Option<JfAudio>) {
    let url = format!(
        "{}/Sessions?activeWithinSeconds=60",
        cfg.url.trim_end_matches('/')
    );
    let resp = client()
        .get(&url)
        .header("Authorization", auth(cfg))
        .send()
        .await;
    match resp {
        Err(e) => {
            tracing::debug!("jellyfin: {e}");
            (
                Event::Jellyfin {
                    status: JfStatus::Offline,
                    sessions: Vec::new(),
                },
                None,
            )
        }
        Ok(r) => {
            let status = status_of(r.status().as_u16(), true);
            let body = if status == JfStatus::Ok {
                r.text().await.unwrap_or_default()
            } else {
                String::new()
            };
            let audio = pick_audio(&parse_audio(&body)).cloned();
            (
                Event::Jellyfin {
                    status,
                    sessions: parse_sessions(&body),
                },
                audio,
            )
        }
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn run_jellyfin(cfg: JellyfinConfig, ctx: SourceCtx) {
    let mut last: Option<JfStatus> = None;
    let mut covers = CoverCache::default();
    let mut prev_track: Option<Track> = None;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let (ev, audio) = poll(&cfg).await;
        let track = match audio {
            Some(a) => {
                let cover = match &a.cover_item {
                    Some(item) => {
                        let req = client()
                            .get(cover_url(&cfg.url, item))
                            .header("Authorization", auth(&cfg));
                        covers.fetch(&format!("jf-{item}"), req).await
                    }
                    None => None,
                };
                let mut t = a.into_track(cover);
                rackscreen_core::music::stamp_since(prev_track.as_ref(), &mut t, unix_now());
                Some(t)
            }
            None => None,
        };
        prev_track = track.clone();
        ctx.emit(Event::Music {
            source: MusicSource::Jellyfin,
            track,
        });
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

    const AUDIO: &str = include_str!("../tests/fixtures/jellyfin-sessions-audio.json");

    #[test]
    fn audio_sessions_go_to_music_not_to_the_video_tile() {
        let video = parse_sessions(AUDIO);
        assert_eq!(video.len(), 1, "audio is not a video session");
        assert_eq!(video[0].title, "Blade Runner 2049");
        let all = parse_audio(AUDIO);
        assert_eq!(all.len(), 2);
        let a = pick_audio(&all).unwrap();
        assert_eq!(
            (a.title.as_str(), a.artist.as_str(), a.device.as_str()),
            ("Teardrop", "Massive Attack", "silke · Desk"),
            "the playing one; Artists[0]"
        );
        assert!((a.duration_secs - 331.0).abs() < 1e-6);
        assert!((a.position_secs - 134.0).abs() < 1e-6);
        assert!(!a.paused);
        assert_eq!(a.cover_item.as_deref(), Some("album-9"), "AlbumId first");
        let old = &all[0];
        assert_eq!(old.artist, "Massive Attack", "AlbumArtist without Artists");
        assert_eq!(
            old.cover_item.as_deref(),
            Some("track-old"),
            "else the item Id"
        );
        assert!(old.paused);
        assert_eq!(
            pick_audio(&all[..1]).unwrap().title,
            "Angel",
            "only paused: the most recent"
        );
        assert!(parse_audio(SESSIONS).is_empty());
        assert_eq!(
            cover_url("http://jf:8096/", "album-9"),
            "http://jf:8096/Items/album-9/Images/Primary?maxWidth=168&format=jpg"
        );
        let t = a.clone().into_track(None);
        assert_eq!(t.source, rackscreen_core::event::MusicSource::Jellyfin);
        assert!(t.playing && t.title == "Teardrop");
    }
}

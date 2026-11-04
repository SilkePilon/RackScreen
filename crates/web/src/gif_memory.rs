//! `/api/gifs/memory`: how much of the Pi's RAM the decoded GIFs take and
//! how many more fit. Every frame is kept decoded and packed (240×240
//! RGB565, LZ4), so a GIF costs at most `frames × PACKED_FRAME_MAX`, at most
//! `MAX_FRAMES` frames; most pack to a quarter of that.
//!
//! The budget: memory the GIFs could have is what is available now plus
//! what they already hold, less a reserve for the OS and the service.
//! GIFs a screen shows that are still loading are counted at full size.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_app::hub::GifStatus;
use rackscreen_core::gif::{MAX_FRAMES, PACKED_FRAME_MAX};
use serde::Serialize;

use crate::{system, ApiResult, AppState};

const MIB: f64 = 1024.0 * 1024.0;

/// Headroom left for the OS, page cache and the rest of the service.
pub const RESERVE_MB: f64 = 150.0;

/// The most one GIF can take, in MB.
pub fn max_gif_mb() -> f64 {
    (MAX_FRAMES * PACKED_FRAME_MAX) as f64 / MIB
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Budget {
    /// What all GIFs together may take.
    pub budget_mb: f64,
    /// Of that, not yet taken by decoded or loading GIFs.
    pub left_mb: f64,
    /// Full-length GIFs that fit in `left_mb`.
    pub fits_more: u32,
}

/// `budget = max(0, available + decoded - reserve)`,
/// `left = max(0, budget - decoded - pending)`,
/// `fits_more = floor(left / max_gif)`.
pub fn budget(available_mb: f64, decoded_mb: f64, pending_mb: f64) -> Budget {
    let budget_mb = (available_mb + decoded_mb - RESERVE_MB).max(0.0);
    let left_mb = (budget_mb - decoded_mb - pending_mb).max(0.0);
    Budget {
        budget_mb: round1(budget_mb),
        left_mb: round1(left_mb),
        fits_more: (left_mb / max_gif_mb()).floor() as u32,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GifRow {
    pub name: String,
    pub used: bool,
    /// `off`, `loading`, `ready` or `error`.
    pub state: &'static str,
    pub frames: Option<usize>,
    pub mb: Option<f64>,
    pub error: Option<String>,
}

/// The saved config's GIFs with what the render loop holds of them. The
/// loop runs the config it was started with, so a GIF it holds counts even
/// when the saved config no longer shows it; a newly used one it does not
/// know yet is `loading`.
pub fn rows(gifs: &[String], used: &HashSet<String>, live: &[GifStatus]) -> Vec<GifRow> {
    gifs.iter()
        .map(|name| {
            let is_used = used.contains(name);
            let l = live.iter().find(|g| &g.name == name);
            let (state, frames, bytes, error) = match l {
                Some(l) if l.state == "ready" => ("ready", l.frames, l.bytes, None),
                _ if !is_used => ("off", None, None, None),
                Some(l) if l.used => (l.state, None, None, l.error.clone()),
                _ => ("loading", None, None, None),
            };
            GifRow {
                name: name.clone(),
                used: is_used,
                state,
                frames,
                mb: bytes.map(|b| round1(b as f64 / MIB)),
                error,
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct HostMemory {
    pub total_mb: Option<u64>,
    pub available_mb: Option<u64>,
    pub process_rss_mb: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GifMemory {
    pub memory: HostMemory,
    pub frame_bytes: usize,
    pub max_frames: usize,
    pub max_gif_mb: f64,
    pub reserve_mb: f64,
    pub gifs: Vec<GifRow>,
    /// Decoded GIFs in memory now (the render loop's, all of them).
    pub decoded_mb: f64,
    /// GIFs a screen shows that are still loading, at full size.
    pub pending_mb: f64,
    pub budget_mb: Option<f64>,
    pub left_mb: Option<f64>,
    pub fits_more: Option<u32>,
}

pub fn gif_memory(state: &AppState) -> GifMemory {
    let cfg = Config::load_or_default(&state.hub.config_path).ok();
    let names: Vec<String> = cfg
        .as_ref()
        .map(|c| c.gifs.iter().map(|g| g.name.clone()).collect())
        .unwrap_or_default();
    let used: HashSet<String> = cfg
        .as_ref()
        .map(|c| c.used_gifs().into_iter().map(|(_, n, _)| n).collect())
        .unwrap_or_default();
    let live = state.hub.status().gifs;
    let gifs = rows(&names, &used, &live);
    let decoded_mb = live.iter().filter_map(|g| g.bytes).sum::<usize>() as f64 / MIB;
    let pending_mb = gifs.iter().filter(|g| g.state == "loading").count() as f64 * max_gif_mb();
    let mem = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| system::parse_meminfo(&t));
    let b = mem.map(|m| budget(m.available_mb as f64, decoded_mb, pending_mb));
    GifMemory {
        memory: HostMemory {
            total_mb: mem.map(|m| m.total_mb),
            available_mb: mem.map(|m| m.available_mb),
            process_rss_mb: system::process_rss_mb().map(round1),
        },
        frame_bytes: PACKED_FRAME_MAX,
        max_frames: MAX_FRAMES,
        max_gif_mb: round1(max_gif_mb()),
        reserve_mb: RESERVE_MB,
        gifs,
        decoded_mb: round1(decoded_mb),
        pending_mb: round1(pending_mb),
        budget_mb: b.map(|b| b.budget_mb),
        left_mb: b.map(|b| b.left_mb),
        fits_more: b.map(|b| b.fits_more),
    }
}

pub async fn memory(State(state): State<Arc<AppState>>) -> ApiResult<GifMemory> {
    Ok(Json(gif_memory(&state)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::gif::FRAME_BYTES;

    #[test]
    fn a_full_gif_is_at_most_about_17_mb() {
        let m = max_gif_mb();
        assert!((m - 16.54).abs() < 0.01, "{m}");
    }

    #[test]
    fn budget_is_available_plus_decoded_less_the_reserve() {
        // a Pi 3B+ with 600 MB available and two full GIFs decoded
        let decoded = 2.0 * max_gif_mb();
        let b = budget(600.0, decoded, 0.0);
        assert_eq!(b.budget_mb, round1(600.0 + decoded - RESERVE_MB));
        assert_eq!(b.left_mb, 450.0, "available less the reserve");
        assert_eq!(b.fits_more, 27, "450 / 16.55");
        // a GIF still loading takes its full size off what is left
        let p = budget(600.0, decoded, max_gif_mb());
        assert_eq!(p.fits_more, 26);
        assert_eq!(p.budget_mb, b.budget_mb, "the budget itself is the same");
    }

    #[test]
    fn budget_never_goes_negative() {
        let b = budget(100.0, 0.0, 0.0);
        assert_eq!((b.budget_mb, b.left_mb, b.fits_more), (0.0, 0.0, 0));
        let over = budget(160.0, 70.0, 66.0);
        assert_eq!(over.budget_mb, 80.0);
        assert_eq!((over.left_mb, over.fits_more), (0.0, 0));
        assert_eq!(budget(150.0 + 16.5, 0.0, 0.0).fits_more, 0, "just short");
    }

    fn live(name: &str, used: bool, state: &'static str, frames: Option<usize>) -> GifStatus {
        GifStatus {
            name: name.into(),
            used,
            state,
            frames,
            bytes: frames.map(|f| f * FRAME_BYTES),
            error: (state == "error").then(|| "HTTP 404".to_string()),
        }
    }

    #[test]
    fn rows_join_the_saved_config_with_the_render_loop() {
        let names: Vec<String> = ["cat", "dog", "new", "spare", "bad", "old"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let used: HashSet<String> = ["cat", "dog", "new", "bad"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let l = vec![
            live("cat", true, "ready", Some(100)),
            live("dog", true, "loading", None),
            live("spare", false, "off", None),
            live("bad", true, "error", None),
            live("old", true, "ready", Some(10)),
        ];
        let r = rows(&names, &used, &l);
        let by = |n: &str| r.iter().find(|g| g.name == n).unwrap();
        assert_eq!(by("cat").state, "ready");
        assert_eq!(by("cat").frames, Some(100));
        assert_eq!(by("cat").mb, Some(round1(100.0 * FRAME_BYTES as f64 / MIB)));
        assert_eq!(by("dog").state, "loading");
        assert_eq!(by("new").state, "loading", "saved but not started yet");
        assert_eq!(by("spare").state, "off");
        assert!(!by("spare").used);
        assert_eq!(by("bad").state, "error");
        assert_eq!(by("bad").error.as_deref(), Some("HTTP 404"));
        assert_eq!(
            by("old").state,
            "ready",
            "taken off its screen but still held until the restart"
        );
        assert!(!by("old").used);
    }
}

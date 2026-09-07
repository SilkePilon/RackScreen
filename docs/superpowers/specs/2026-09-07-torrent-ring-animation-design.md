# Torrent ring in/out animation

Date: 2026-09-07. Status: approved by Silke. Small change to the HEALTH torrent mode (`crates/core/src/scene.rs::torrent_scene`, `crates/core/src/model.rs`).

## Behaviour

- The model keeps one animated entry per torrent, keyed by name: `TorrentAnim { progress: Smooth (800 ms, ease-out, like all other values), radius: Smooth (400 ms), leaving: bool, last: Torrent }`.
- New torrent: entry created with `progress` starting at 0 and set to the reported progress, so its ring sweeps clockwise from empty to the real value. Subsequent `Event::Torrents` updates retarget the smooth; no jumps.
- Torrent missing from a `Torrents` update (gone or done): entry marked `leaving`, `progress` retargeted to 0 (ring unwinds counter-clockwise from its last value). While any entry is leaving, the remaining entries keep their current radius slot. Once the leaving entry's smoothed progress is below 0.5 %, it is removed and the remaining entries are re-sorted by target progress (most complete outermost) with `radius` smoothed to the new slot radius over 400 ms.
- Slots: the standard radii `102, 86, 70`; at most 3 entries drawn (top 3 by target progress, plus leaving entries that still occupy a slot; a fourth active torrent waits for a slot).
- Torrent mode: `Model::torrent_mode()` stays true while the cluster is healthy, the qBittorrent link is up, and there is at least one non-leaving entry OR a leaving entry still unwinding. Entering torrent mode therefore shows the new ring sweeping in from 0; leaving shows the last ring unwinding fully (about 0.8 s) before the Health scene returns.
- The scene reads the animated entries (`Model::torrent_rings(now) -> Vec<(radius, progress, accent index)>`) instead of raw torrents. Badge (speed / ETA) and the download icon are unchanged and use the real torrent list; while only leaving entries remain, the badge shows the last known speed/ETA of those entries.
- Fake source keys `t` (toggle torrents) and `6` (torrent done) exercise it; no source changes needed.

## Tests

- New entry: `torrent_rings` reports progress 0 at the arrival instant and the target after 1 s.
- Removal: entry unwinds (progress decreasing over time) and is dropped once under 0.5 %; `torrent_mode()` stays true until then, then false.
- Re-sort: after a removal the remaining ring's radius eases from its old slot to the new one (intermediate value between at 0.2 s, equal at 0.5 s).
- Golden `torrent_unwind` mid-animation.
- Existing goldens unchanged (`torrent` golden is rendered at 5 s after the event, when smoothing has settled; verify byte-identical, otherwise render it at `now = 5.0` with the event applied at 0.0 as before).

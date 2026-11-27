# PortWatch Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `portwatch` role that shows the leading GTA VI PC-port effort from portwatch-vi.xyz: score %, milestone route ring, name pill (verified check, private lock), fps and GPU tiles, and the site's update age.

**Architecture:** `rackscreen-core` gets a dependency-free port of the site's scoring (`portwatch.rs`) and a new `RingKind::Route`; `rackscreen-sources` parses `tracker.json` with serde, scores it with the core function and emits one `Event::PortWatch(PortWatchSnapshot)`; the model keeps the last snapshot; a self-drawing role scene (`p2/roles/portwatch.rs`, like `iss_scene`) composes ring, hero, pill, tiles and footer. Brand logos are SVG icons in `assets/icons`, drawn through the existing usvg icon cache.

**Tech Stack:** Rust workspace (core, sources, render, app, web), serde_json, reqwest, chrono, tiny-skia/usvg, golden PNG tests, React web UI (only a mock row).

Spec: `docs/superpowers/specs/2026-11-27-portwatch-design.md`. Mockups: `docs/superpowers/specs/assets/2026-11-27-portwatch-mockup.html` (row C) and `…-complete-icon.html` (option F).

## Global Constraints

- Data URL: `https://portwatch-vi.xyz/data/tracker.json`; no key; no personal data in requests; shared client from `crates/sources/src/http.rs`.
- Poll: `poll_secs` default 900, minimum 300; `If-None-Match` with the last `ETag`; failures retry after 1, 5, 15, then 60 min (`rackscreen_sources::gif::retry_delay`).
- Scoring must match `scoring.js`: groundwork weights 0.20/0.28/0.32/0.20 → first 10 %; ladder ticks 10/20/30/40/50; tracks story 0.35, render 0.25, perf fps/30 0.20, stability 0.20; stability labels unknown 0, constant 0, often 0.25, sometimes 0.55, rarely 0.8, stable 1; `manualScore` overrides; round half up to one decimal; `nuked` (any truthy value) skipped; ties by `stats.stars` desc.
- Fixture check: `crates/sources/testdata/portwatch-tracker.json` (committed) → leader `Belobug` 91.3, top-3 90.3.
- Milestone colours: Groundwork `TEXT2` (spec said `TEXT3`; its 30 % pill fill would be 0x29, under the floor), Exe runs `VIOLET`, First frame `INDIGO`, Main menu `BLUE`, In the world `TEAL`, Playable `GREEN`, Complete `AMBER`. Marker icons: `construction`, `terminal`, `image`, `list`, `public`, `directions_run`, `check_circle`.
- Brand colours: AMD `#ED1C24`, NVIDIA `#76B900`, Intel `#0071C5`. Logos from Simple Icons `simple-icons@13.21.0` (CC0-1.0).
- No gradients. No project avatar, no stability on screen.
- Every drawable must pass the golden contrast floor (`contrast_floor_on_every_role`): non-black colour × alpha ≥ 0x2A for shapes/icons, ≥ 0x78 for text. Therefore the route ring dims at **0.35** (the `Bands` 0.3 would put `TEXT3` at 0x29).
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Test commands: `cargo test -p <crate>`; goldens: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden <name>` writes new PNGs, then rerun without it.

## File Structure

| File | Responsibility |
|---|---|
| `crates/core/src/portwatch.rs` (new) | Milestone, Gpu, Effort, scoring, ranking, snapshot, text helpers |
| `crates/core/src/lib.rs` | `pub mod portwatch;` |
| `crates/core/src/event.rs` | `Event::PortWatch(PortWatchSnapshot)` |
| `crates/core/src/model.rs` | keep last snapshot, `role_available` |
| `crates/core/src/p2/ring.rs` | `RingKind::Route`, `RouteMarker`, `RingSpec::route` |
| `crates/core/src/p2/roles/portwatch.rs` (new) | `portwatch_scene` |
| `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_model.rs`, `crates/core/src/theme.rs` | role wiring |
| `crates/sources/src/portwatch.rs` (new) | JSON → `Effort`s, fetch with ETag, poll loop |
| `crates/sources/src/lib.rs` | `pub mod portwatch;` |
| `crates/sources/src/fake.rs` | demo snapshot in `initial()` |
| `crates/sources/testdata/portwatch-tracker.json` (committed with this plan) | trimmed real snapshot |
| `assets/icons/ms-*.svg`, `assets/icons/brand-*.svg`, `assets/icons/BRANDS-LICENSE.md` | icons |
| `crates/render/src/assets.rs` | register icons |
| `scripts/fetch-assets.sh` | fetch the new icons |
| `crates/app/src/config.rs`, `crates/app/src/run.rs`, `config.example.yaml` | config + spawn |
| `crates/web/src/roles.rs`, `web/src/mock/data.ts`, `scripts/role-previews.sh`, `web/public/roles/portwatch.webp` | web catalog |
| `crates/render/tests/golden.rs`, `crates/render/tests/goldens/p2_portwatch*.png` | goldens |

---

### Task 1: Core scoring (`rackscreen_core::portwatch`)

**Files:**
- Create: `crates/core/src/portwatch.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod portwatch;` in alphabetical order, after `pub mod p2;`)

**Interfaces:**
- Produces:
  - `pub enum Milestone { Groundwork, ExeRuns, FirstFrame, MainMenu, InTheWorld, Playable, Complete }` (`Clone, Copy, Debug, PartialEq, Eq`), with `const ALL: [Milestone; 7]`, `fn at(self) -> f32`, `fn label(self) -> &'static str`, `fn color(self) -> Color`, `fn icon(self) -> &'static str`.
  - `pub enum Gpu { Amd, Nvidia, Intel }` with `fn parse(&str) -> Option<Gpu>`, `fn icon(self) -> &'static str`, `fn color(self) -> Color`.
  - `pub fn parse_gpus<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<Gpu>`
  - `pub fn stability_label(label: &str) -> f64`
  - `pub struct Effort { name, repo: Option<String>, verified: bool, gpus: Vec<Gpu>, groundwork: [f64; 4], ladder: [f64; 5], story, render, fps, stability: f64, manual_score: Option<f64>, nuked: bool, stars: u64 }` (`Clone, Debug, Default, PartialEq`), `fn private(&self) -> bool`.
  - `pub struct Scored { pub score: f32, pub milestone: Milestone }`, `pub fn score(e: &Effort) -> Scored`
  - `pub struct Ranked { pub index: usize, pub scored: Scored }`, `pub fn ranked(efforts: &[Effort]) -> Vec<Ranked>`, `pub fn top3_average(ranked: &[Ranked]) -> f32`
  - `pub struct PortWatchSnapshot { pub score: f32, pub milestone: Milestone, pub name: String, pub private: bool, pub verified: bool, pub fps: Option<f32>, pub gpus: Vec<Gpu>, pub updated_unix: i64 }` (`Clone, Debug, PartialEq`)
  - `pub fn snapshot(efforts: &[Effort], updated_unix: i64) -> Option<PortWatchSnapshot>`
  - `pub fn hero_text(score: f32) -> String`, `pub fn fps_text(fps: Option<f32>) -> String`, `pub fn fmt_age(secs: i64) -> String`

- [ ] **Step 1: Write the module with tests first (tests at the bottom, implementation stubs `todo!()`)**

Create `crates/core/src/portwatch.rs`:

```rust
//! PortWatch VI (portwatch-vi.xyz): the site's scoring, ported from its
//! `assets/js/scoring.js`, on plain data. The source parses `tracker.json`
//! into `Effort`s; the screen draws a `PortWatchSnapshot`.

use crate::theme::p2::{AMBER, BLUE, GREEN, INDIGO, TEAL, TEXT2, VIOLET};
use crate::theme::Color;

/// How far an effort got. Each ladder step needs the one before it; past
/// Playable four quality tracks fill 50 to 100 %.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    Groundwork,
    ExeRuns,
    FirstFrame,
    MainMenu,
    InTheWorld,
    Playable,
    Complete,
}

impl Milestone {
    pub const ALL: [Milestone; 7] = [
        Milestone::Groundwork,
        Milestone::ExeRuns,
        Milestone::FirstFrame,
        Milestone::MainMenu,
        Milestone::InTheWorld,
        Milestone::Playable,
        Milestone::Complete,
    ];

    /// The percentage the milestone starts at.
    pub fn at(self) -> f32 {
        match self {
            Milestone::Groundwork => 0.0,
            Milestone::ExeRuns => 10.0,
            Milestone::FirstFrame => 20.0,
            Milestone::MainMenu => 30.0,
            Milestone::InTheWorld => 40.0,
            Milestone::Playable => 50.0,
            Milestone::Complete => 100.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Milestone::Groundwork => "Groundwork",
            Milestone::ExeRuns => "Exe runs",
            Milestone::FirstFrame => "First frame",
            Milestone::MainMenu => "Main menu",
            Milestone::InTheWorld => "In the world",
            Milestone::Playable => "Playable",
            Milestone::Complete => "Complete",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Milestone::Groundwork => TEXT2,
            Milestone::ExeRuns => VIOLET,
            Milestone::FirstFrame => INDIGO,
            Milestone::MainMenu => BLUE,
            Milestone::InTheWorld => TEAL,
            Milestone::Playable => GREEN,
            Milestone::Complete => AMBER,
        }
    }

    /// The marker icon on the route ring (asset name).
    pub fn icon(self) -> &'static str {
        match self {
            Milestone::Groundwork => "ms-construction",
            Milestone::ExeRuns => "ms-terminal",
            Milestone::FirstFrame => "ms-image",
            Milestone::MainMenu => "ms-list",
            Milestone::InTheWorld => "ms-public",
            Milestone::Playable => "ms-directions_run",
            Milestone::Complete => "ms-check_circle",
        }
    }
}

/// The ladder after groundwork, in order.
const LADDER: [Milestone; 5] = [
    Milestone::ExeRuns,
    Milestone::FirstFrame,
    Milestone::MainMenu,
    Milestone::InTheWorld,
    Milestone::Playable,
];

/// A GPU vendor an effort runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gpu {
    Amd,
    Nvidia,
    Intel,
}

impl Gpu {
    pub fn parse(s: &str) -> Option<Gpu> {
        match s.trim().to_ascii_lowercase().as_str() {
            "amd" => Some(Gpu::Amd),
            "nvidia" => Some(Gpu::Nvidia),
            "intel" => Some(Gpu::Intel),
            _ => None,
        }
    }
    /// The brand logo (asset name, Simple Icons, CC0).
    pub fn icon(self) -> &'static str {
        match self {
            Gpu::Amd => "brand-amd",
            Gpu::Nvidia => "brand-nvidia",
            Gpu::Intel => "brand-intel",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Gpu::Amd => Color::hex(0xED1C24),
            Gpu::Nvidia => Color::hex(0x76B900),
            Gpu::Intel => Color::hex(0x0071C5),
        }
    }
}

/// Vendor names to GPUs: unknown ones dropped, order kept, no repeats.
pub fn parse_gpus<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<Gpu> {
    let mut out = Vec::new();
    for g in names.into_iter().filter_map(Gpu::parse) {
        if !out.contains(&g) {
            out.push(g);
        }
    }
    out
}

/// A stability label as 0..=1; anything unknown counts as 0.
pub fn stability_label(label: &str) -> f64 {
    match label {
        "often" => 0.25,
        "sometimes" => 0.55,
        "rarely" => 0.8,
        "stable" => 1.0,
        _ => 0.0,
    }
}

/// One effort, with the JSON's loose values already turned into numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effort {
    pub name: String,
    /// `owner/repo`; `None` (or no `/`) is a private build.
    pub repo: Option<String>,
    /// PortWatch checked it itself (`verification: "portwatch"`).
    pub verified: bool,
    pub gpus: Vec<Gpu>,
    /// loader, shaders, graphics, proven.
    pub groundwork: [f64; 4],
    /// loads, boots, menu, world, playable.
    pub ladder: [f64; 5],
    pub story: f64,
    pub render: f64,
    /// Average fps on the named PC; 0 when unknown.
    pub fps: f64,
    /// 0..=1 (a label already mapped by `stability_label`).
    pub stability: f64,
    pub manual_score: Option<f64>,
    /// Taken offline: kept in the data, not ranked.
    pub nuked: bool,
    pub stars: u64,
}

impl Effort {
    pub fn private(&self) -> bool {
        !self.repo.as_deref().is_some_and(|r| r.contains('/'))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scored {
    pub score: f32,
    pub milestone: Milestone,
}

fn c01(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// `Math.round(x * 10) / 10` for the scores (all ≥ 0).
fn round1(x: f64) -> f64 {
    (x * 10.0 + 0.5).floor() / 10.0
}

pub fn score(e: &Effort) -> Scored {
    todo!()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ranked {
    /// Index into the efforts slice.
    pub index: usize,
    pub scored: Scored,
}

/// Efforts that are not nuked, best first; equal scores rank more stars first.
pub fn ranked(efforts: &[Effort]) -> Vec<Ranked> {
    todo!()
}

/// The mean of the first three scores, one decimal; 0 without any.
pub fn top3_average(ranked: &[Ranked]) -> f32 {
    todo!()
}

/// What the screen shows: the leader and the site's update time.
#[derive(Clone, Debug, PartialEq)]
pub struct PortWatchSnapshot {
    pub score: f32,
    pub milestone: Milestone,
    pub name: String,
    pub private: bool,
    pub verified: bool,
    /// `None` when the leader has no fps figure (0 or missing).
    pub fps: Option<f32>,
    pub gpus: Vec<Gpu>,
    pub updated_unix: i64,
}

/// The leader as a snapshot; `None` when no effort is ranked.
pub fn snapshot(efforts: &[Effort], updated_unix: i64) -> Option<PortWatchSnapshot> {
    todo!()
}

/// The hero: one decimal, `.0` dropped (`91.3`, `100`, `34`).
pub fn hero_text(score: f32) -> String {
    todo!()
}

/// The fps tile: `22`, `4.5`, `—` without a figure.
pub fn fps_text(fps: Option<f32>) -> String {
    todo!()
}

/// The footer: `just now`, `5m ago`, `2h ago`, `3d ago`.
pub fn fmt_age(secs: i64) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playable(story: f64, render: f64, fps: f64, stability: f64) -> Effort {
        Effort {
            name: "x".into(),
            repo: Some("o/r".into()),
            ladder: [1.0; 5],
            story,
            render,
            fps,
            stability,
            ..Effort::default()
        }
    }

    #[test]
    fn groundwork_only_fills_the_first_ten_percent() {
        let e = Effort {
            groundwork: [0.85, 0.35, 0.7, 0.8],
            ..Effort::default()
        };
        // 10 × (0.2·0.85 + 0.28·0.35 + 0.32·0.7 + 0.2·0.8) = 6.52
        assert_eq!(
            score(&e),
            Scored {
                score: 6.5,
                milestone: Milestone::Groundwork
            }
        );
    }

    #[test]
    fn a_partial_step_stops_the_ladder() {
        let e = Effort {
            ladder: [1.0, 1.0, 0.5, 1.0, 1.0],
            ..Effort::default()
        };
        assert_eq!(
            score(&e),
            Scored {
                score: 25.0,
                milestone: Milestone::FirstFrame
            }
        );
        let e = Effort {
            ladder: [1.0, 1.0, 1.0, 0.5, 0.0],
            ..Effort::default()
        };
        assert_eq!(
            score(&e),
            Scored {
                score: 35.0,
                milestone: Milestone::MainMenu
            }
        );
    }

    #[test]
    fn a_partial_step_never_drops_below_the_groundwork() {
        // groundwork 10 × 1 = 10, then loads 0.5 → 0 + 0.5 × (10 − 10) = 10
        let e = Effort {
            groundwork: [1.0; 4],
            ladder: [0.5, 0.0, 0.0, 0.0, 0.0],
            ..Effort::default()
        };
        assert_eq!(score(&e).score, 10.0);
        assert_eq!(score(&e).milestone, Milestone::Groundwork);
    }

    #[test]
    fn playable_adds_the_four_tracks() {
        // Belobug on 2026-11-27: 50 + 50 × (0.35·0.8 + 0.25·0.8 + 0.2·22/30 + 0.2·1)
        let s = score(&playable(0.8, 0.8, 22.0, 1.0));
        assert_eq!(s.score, 91.3);
        assert_eq!(s.milestone, Milestone::Playable);
        // fps above 30 counts as 30
        assert_eq!(score(&playable(1.0, 1.0, 120.0, 1.0)).milestone, Milestone::Complete);
        assert_eq!(score(&playable(1.0, 1.0, 120.0, 1.0)).score, 100.0);
        assert_eq!(score(&playable(0.0, 0.0, 0.0, 0.0)).score, 50.0);
    }

    #[test]
    fn tracks_do_not_count_before_playable() {
        let mut e = playable(1.0, 1.0, 30.0, 1.0);
        e.ladder[4] = 0.0;
        assert_eq!(score(&e).score, 40.0);
        assert_eq!(score(&e).milestone, Milestone::InTheWorld);
    }

    #[test]
    fn manual_score_overrides() {
        let mut e = playable(0.0, 0.0, 0.0, 0.0);
        e.manual_score = Some(140.0);
        assert_eq!(score(&e).score, 100.0);
        e.manual_score = Some(f64::NAN);
        assert_eq!(score(&e).score, 50.0);
    }

    #[test]
    fn stability_labels() {
        assert_eq!(stability_label("rarely"), 0.8);
        assert_eq!(stability_label("stable"), 1.0);
        assert_eq!(stability_label("constant"), 0.0);
        assert_eq!(stability_label("unknown"), 0.0);
        assert_eq!(stability_label("whatever"), 0.0);
    }

    #[test]
    fn ranking_skips_nuked_and_breaks_ties_on_stars() {
        let mut a = playable(0.0, 0.0, 0.0, 0.0); // 50
        a.name = "a".into();
        a.stars = 5;
        let mut b = a.clone();
        b.name = "b".into();
        b.stars = 50;
        let mut c = playable(1.0, 1.0, 30.0, 1.0); // 100, nuked
        c.nuked = true;
        let r = ranked(&[a, b, c]);
        assert_eq!(r.iter().map(|x| x.index).collect::<Vec<_>>(), vec![1, 0]);
    }

    #[test]
    fn top3_is_the_rounded_mean() {
        let efforts = [
            playable(0.8, 0.8, 22.0, 1.0),    // 91.3
            playable(0.8, 0.75, 20.0, 1.0),   // 90.0
            playable(0.5, 0.7, 25.0, 1.0),    // something lower
        ];
        let r = ranked(&efforts);
        let sum: f64 = r.iter().take(3).map(|x| x.scored.score as f64).sum();
        let want = ((sum / 3.0 * 10.0 + 0.5).floor() / 10.0) as f32;
        assert_eq!(top3_average(&r), want);
        assert_eq!(top3_average(&[]), 0.0);
    }

    #[test]
    fn private_is_a_missing_repo() {
        let mut e = Effort::default();
        assert!(e.private());
        e.repo = Some("noslash".into());
        assert!(e.private());
        e.repo = Some("Belobug/VIctory-Belobug-AMD".into());
        assert!(!e.private());
    }

    #[test]
    fn gpus_parse_in_order_without_repeats() {
        assert_eq!(
            parse_gpus(["NVIDIA", "amd", "Voodoo", "AMD", "Intel"]),
            vec![Gpu::Nvidia, Gpu::Amd, Gpu::Intel]
        );
        assert!(parse_gpus([]).is_empty());
    }

    #[test]
    fn snapshot_is_the_leader() {
        let mut lead = playable(0.8, 0.8, 22.0, 1.0);
        lead.name = "Belobug".into();
        lead.verified = true;
        lead.gpus = vec![Gpu::Amd];
        let mut early = Effort {
            name: "XCL".into(),
            ladder: [1.0, 1.0, 1.0, 0.5, 0.0],
            ..Effort::default()
        };
        early.repo = Some("xcl-dev/xcl".into());
        let s = snapshot(&[early, lead], 1_795_000_000).unwrap();
        assert_eq!(
            s,
            PortWatchSnapshot {
                score: 91.3,
                milestone: Milestone::Playable,
                name: "Belobug".into(),
                private: false,
                verified: true,
                fps: Some(22.0),
                gpus: vec![Gpu::Amd],
                updated_unix: 1_795_000_000,
            }
        );
        assert_eq!(snapshot(&[], 0), None);
    }

    #[test]
    fn zero_fps_is_no_figure() {
        let mut e = playable(0.0, 0.0, 0.0, 0.0);
        e.fps = 0.0;
        assert_eq!(snapshot(&[e], 0).unwrap().fps, None);
    }

    #[test]
    fn texts() {
        assert_eq!(hero_text(91.3), "91.3");
        assert_eq!(hero_text(100.0), "100");
        assert_eq!(hero_text(34.0), "34");
        assert_eq!(hero_text(6.5), "6.5");
        assert_eq!(fps_text(Some(22.0)), "22");
        assert_eq!(fps_text(Some(4.5)), "4.5");
        assert_eq!(fps_text(Some(12.4)), "12");
        assert_eq!(fps_text(None), "—");
        assert_eq!(fmt_age(-5), "just now");
        assert_eq!(fmt_age(59), "just now");
        assert_eq!(fmt_age(5 * 60 + 10), "5m ago");
        assert_eq!(fmt_age(2 * 3600 + 59 * 60), "2h ago");
        assert_eq!(fmt_age(3 * 86_400 + 5), "3d ago");
    }

    #[test]
    fn milestones_start_where_the_ring_bands_do() {
        let ats: Vec<f32> = Milestone::ALL.iter().map(|m| m.at()).collect();
        assert_eq!(ats, vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 100.0]);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p rackscreen-core portwatch`
Expected: FAIL (panics `not yet implemented`).

- [ ] **Step 3: Implement the stubs**

Replace each `todo!()` body:

```rust
pub fn score(e: &Effort) -> Scored {
    const GROUNDWORK: [f64; 4] = [0.20, 0.28, 0.32, 0.20];
    let gw: f64 = e
        .groundwork
        .iter()
        .zip(GROUNDWORK)
        .map(|(v, w)| w * c01(*v))
        .sum();
    let mut score = 10.0 * gw;
    let mut reached = Milestone::Groundwork;
    let mut prev = score;
    for (i, step) in LADDER.iter().enumerate() {
        let v = c01(e.ladder[i]);
        let tick = step.at() as f64;
        if v >= 1.0 {
            score = tick;
            prev = tick;
            reached = *step;
            continue;
        }
        if v > 0.0 {
            score = score.max(prev + v * (tick - prev));
        }
        break;
    }
    // the tracks only count once a build is Playable
    let quality = 0.35 * c01(e.story)
        + 0.25 * c01(e.render)
        + 0.20 * c01(e.fps / 30.0)
        + 0.20 * c01(e.stability);
    if reached == Milestone::Playable {
        score = 50.0 + quality * 50.0;
        if quality >= 1.0 {
            reached = Milestone::Complete;
        }
    }
    if let Some(m) = e.manual_score.filter(|m| m.is_finite()) {
        score = m.clamp(0.0, 100.0);
    }
    Scored {
        score: round1(score) as f32,
        milestone: reached,
    }
}

pub fn ranked(efforts: &[Effort]) -> Vec<Ranked> {
    let mut out: Vec<Ranked> = efforts
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.nuked)
        .map(|(index, e)| Ranked {
            index,
            scored: score(e),
        })
        .collect();
    out.sort_by(|a, b| {
        b.scored
            .score
            .total_cmp(&a.scored.score)
            .then(efforts[b.index].stars.cmp(&efforts[a.index].stars))
    });
    out
}

pub fn top3_average(ranked: &[Ranked]) -> f32 {
    let top = &ranked[..ranked.len().min(3)];
    if top.is_empty() {
        return 0.0;
    }
    let sum: f64 = top.iter().map(|r| r.scored.score as f64).sum();
    round1(sum / top.len() as f64) as f32
}

pub fn snapshot(efforts: &[Effort], updated_unix: i64) -> Option<PortWatchSnapshot> {
    let lead = *ranked(efforts).first()?;
    let e = &efforts[lead.index];
    Some(PortWatchSnapshot {
        score: lead.scored.score,
        milestone: lead.scored.milestone,
        name: e.name.trim().to_string(),
        private: e.private(),
        verified: e.verified,
        fps: (e.fps.is_finite() && e.fps > 0.0).then_some(e.fps as f32),
        gpus: e.gpus.clone(),
        updated_unix,
    })
}

pub fn hero_text(score: f32) -> String {
    let s = format!("{score:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

pub fn fps_text(fps: Option<f32>) -> String {
    match fps {
        None => "—".into(),
        Some(f) if f >= 10.0 => format!("{f:.0}"),
        Some(f) => hero_text(f),
    }
}

pub fn fmt_age(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        "just now".into()
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86_400 {
        format!("{}h ago", s / 3600)
    } else {
        format!("{}d ago", s / 86_400)
    }
}
```

Note on the `top3_is_the_rounded_mean` test: it recomputes the mean from the ranked scores, so it holds whatever the third score is.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p rackscreen-core portwatch`
Expected: all `portwatch::tests::*` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/portwatch.rs crates/core/src/lib.rs
git commit -m "feat(core): PortWatch scoring port

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Source: parse `tracker.json`, fetch with ETag, poll

**Files:**
- Create: `crates/sources/src/portwatch.rs`
- Modify: `crates/sources/src/lib.rs` (add `pub mod portwatch;` in alphabetical order)
- Test data (already committed): `crates/sources/testdata/portwatch-tracker.json`

**Interfaces:**
- Consumes: `rackscreen_core::portwatch::{Effort, parse_gpus, stability_label, snapshot, ranked, top3_average, PortWatchSnapshot}` (Task 1); `rackscreen_core::event::Event::PortWatch` is added in Task 3, so this task emits nothing yet and the poll loop is added in Task 3.
- Produces:
  - `pub const URL: &str`
  - `pub fn parse_efforts(body: &[u8]) -> anyhow::Result<(Vec<Effort>, i64)>` (efforts, `updated` as unix seconds)
  - `pub fn parse(body: &[u8]) -> anyhow::Result<PortWatchSnapshot>` (errors when no effort ranks)
  - `pub struct PortWatchConfig { pub poll_secs: u64 }`
  - `pub enum Fetched { NotModified, Body { body: Vec<u8>, etag: Option<String> } }`, `pub async fn fetch(etag: Option<&str>) -> anyhow::Result<Fetched>`

- [ ] **Step 1: Write the failing tests**

Create `crates/sources/src/portwatch.rs` with the parsing code stubbed and these tests:

```rust
//! PortWatch VI tracker: `tracker.json` from portwatch-vi.xyz, scored with
//! the site's formula (`rackscreen_core::portwatch`) into the leader snapshot.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use rackscreen_core::portwatch::{
    parse_gpus, snapshot, stability_label, Effort, PortWatchSnapshot,
};
use serde::Deserialize;
use serde_json::Value;

use crate::http::{client, read_capped};

pub const URL: &str = "https://portwatch-vi.xyz/data/tracker.json";
/// The file is ~320 KB; refuse anything absurd.
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct PortWatchConfig {
    pub poll_secs: u64,
}

#[derive(Deserialize)]
struct Tracker {
    updated: String,
    #[serde(default)]
    efforts: Vec<RawEffort>,
}

/// Only the fields the screen needs; every one may be missing or odd.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawEffort {
    name: Value,
    repo: Value,
    verification: Value,
    gpu: Value,
    gta: Value,
    groundwork: Value,
    #[serde(rename = "manualScore")]
    manual_score: Value,
    nuked: Value,
    stats: Value,
}

/// A JS-style number in 0..=1 terms: `true` 1, a number or numeric string
/// as is (clamped later), anything else 0.
fn num(v: Option<&Value>) -> f64 {
    todo!()
}

/// JavaScript truthiness (`nuked` is an object with a date and a note).
fn truthy(v: &Value) -> bool {
    todo!()
}

fn effort(raw: &RawEffort) -> Effort {
    todo!()
}

pub fn parse_efforts(body: &[u8]) -> Result<(Vec<Effort>, i64)> {
    todo!()
}

pub fn parse(body: &[u8]) -> Result<PortWatchSnapshot> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::portwatch::{ranked, top3_average, Gpu, Milestone};

    const FIXTURE: &[u8] = include_bytes!("../testdata/portwatch-tracker.json");

    #[test]
    fn the_fixture_leader_matches_the_site() {
        let (efforts, updated) = parse_efforts(FIXTURE).unwrap();
        assert_eq!(efforts.len(), 45);
        // 2026-11-27T16:18:23.809Z
        assert_eq!(updated, 1_795_796_303);
        let r = ranked(&efforts);
        assert_eq!(efforts[r[0].index].name, "Belobug");
        assert_eq!(r[0].scored.score, 91.3);
        assert_eq!(efforts[r[1].index].name, "VIctory-NVIDIA");
        assert_eq!(r[1].scored.score, 90.0);
        // the site's history entry for the day: top3 90.3
        assert_eq!(top3_average(&r), 90.3);
    }

    #[test]
    fn the_fixture_snapshot() {
        let s = parse(FIXTURE).unwrap();
        assert_eq!(s.name, "Belobug");
        assert_eq!(s.score, 91.3);
        assert_eq!(s.milestone, Milestone::Playable);
        assert!(!s.private);
        assert!(s.verified);
        assert_eq!(s.fps, Some(22.0));
        assert_eq!(s.gpus, vec![Gpu::Amd]);
    }

    #[test]
    fn known_efforts_score_like_the_site() {
        let (efforts, _) = parse_efforts(FIXTURE).unwrap();
        let by = |name: &str| {
            let e = efforts.iter().find(|e| e.name == name).unwrap();
            (rackscreen_core::portwatch::score(e), e)
        };
        let (s, e) = by("Doomsday");
        assert_eq!(s.score, 82.1);
        assert!(e.private() && e.verified);
        assert_eq!(e.gpus, vec![Gpu::Amd, Gpu::Nvidia]);
        let (s, _) = by("XCL");
        assert_eq!((s.score, s.milestone), (35.0, Milestone::MainMenu));
        let (s, _) = by("ScarlettX");
        assert_eq!((s.score, s.milestone), (5.2, Milestone::Groundwork));
    }

    #[test]
    fn nuked_objects_are_skipped() {
        let (efforts, _) = parse_efforts(FIXTURE).unwrap();
        let nuked: Vec<&str> = efforts
            .iter()
            .filter(|e| e.nuked)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(nuked.len(), 4, "{nuked:?}");
        assert!(nuked.contains(&"ZETTROX"));
    }

    #[test]
    fn loose_values() {
        let body = br#"{"updated":"2026-11-27T00:00:00Z","efforts":[
          {"name":"A","repo":"a/b","verification":"portwatch","gpu":["NVIDIA",3,"amd"],
           "gta":{"loads":true,"boots":"1","menu":false,"stability":"rarely","fps":"9"},
           "stats":{"stars":7}},
          {"name":"B","repo":null,"gta":{"loads":1,"boots":1,"menu":1,"world":1,"playable":1,
           "story":0.5,"render":0.5,"fps":15,"stability":0.6},"manualScore":12.5,
           "nuked":false},
          {"gta":null,"groundwork":null,"stats":null}
        ]}"#;
        let (e, _) = parse_efforts(body).unwrap();
        assert_eq!(e[0].ladder, [1.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(e[0].stability, 0.8);
        assert_eq!(e[0].fps, 9.0);
        assert_eq!(e[0].gpus, vec![Gpu::Nvidia, Gpu::Amd]);
        assert_eq!(e[0].stars, 7);
        assert!(e[0].verified);
        assert_eq!(e[1].stability, 0.6);
        assert_eq!(e[1].manual_score, Some(12.5));
        assert!(!e[1].nuked && e[1].private());
        assert_eq!(e[2], Effort::default());
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(parse(b"not json").is_err());
        assert!(parse(br#"{"updated":"2026-11-27T00:00:00Z","efforts":[]}"#).is_err());
        assert!(parse(br#"{"updated":"yesterday","efforts":[{"name":"A"}]}"#).is_err());
    }

    /// Hits the real site: `cargo test -p rackscreen-sources portwatch_live -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn portwatch_live() {
        match fetch(None).await.unwrap() {
            Fetched::Body { body, etag } => {
                let s = parse(&body).unwrap();
                eprintln!("leader {} {} % (etag {etag:?})", s.name, s.score);
                assert!(s.score > 0.0);
            }
            Fetched::NotModified => panic!("no etag was sent"),
        }
    }
}
```

Also add the fetch types (used by the live test) above the tests:

```rust
pub enum Fetched {
    NotModified,
    Body { body: Vec<u8>, etag: Option<String> },
}

/// One GET, conditional on `etag`.
pub async fn fetch(etag: Option<&str>) -> Result<Fetched> {
    todo!()
}
```

Register the module in `crates/sources/src/lib.rs`: `pub mod portwatch;`.

Check the `updated` constant before relying on it: `date -u -d 2026-11-27T16:18:23Z +%s` prints `1795796303`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p rackscreen-sources portwatch`
Expected: FAIL (`not yet implemented`).

- [ ] **Step 3: Implement**

```rust
fn num(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Bool(true)) => 1.0,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn effort(raw: &RawEffort) -> Effort {
    let gta = raw.gta.as_object();
    let g = |k: &str| num(gta.and_then(|m| m.get(k)));
    let gw = raw.groundwork.as_object();
    let w = |k: &str| num(gw.and_then(|m| m.get(k)));
    // a number is the owner's rating; a string is a label (a numeric
    // string is a label too, as on the site)
    let stability = match gta.and_then(|m| m.get("stability")) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => stability_label(s),
        _ => 0.0,
    };
    let gpus = raw
        .gpu
        .as_array()
        .map(|a| parse_gpus(a.iter().filter_map(Value::as_str)))
        .unwrap_or_default();
    Effort {
        name: raw.name.as_str().unwrap_or("").trim().to_string(),
        repo: raw.repo.as_str().map(str::to_string),
        verified: raw.verification.as_str() == Some("portwatch"),
        gpus,
        groundwork: [w("loader"), w("shaders"), w("graphics"), w("proven")],
        ladder: [g("loads"), g("boots"), g("menu"), g("world"), g("playable")],
        story: g("story"),
        render: g("render"),
        fps: g("fps"),
        stability,
        manual_score: raw.manual_score.as_f64(),
        nuked: truthy(&raw.nuked),
        stars: raw
            .stats
            .get("stars")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }
}

pub fn parse_efforts(body: &[u8]) -> Result<(Vec<Effort>, i64)> {
    let t: Tracker = serde_json::from_slice(body).context("tracker.json")?;
    let updated = chrono::DateTime::parse_from_rfc3339(&t.updated)
        .with_context(|| format!("updated {:?}", t.updated))?
        .timestamp();
    Ok((t.efforts.iter().map(effort).collect(), updated))
}

pub fn parse(body: &[u8]) -> Result<PortWatchSnapshot> {
    let (efforts, updated) = parse_efforts(body)?;
    match snapshot(&efforts, updated) {
        Some(s) => Ok(s),
        None => bail!("tracker.json has no ranked efforts"),
    }
}

pub async fn fetch(etag: Option<&str>) -> Result<Fetched> {
    use reqwest::header::{ETAG, IF_NONE_MATCH};
    let mut req = client().get(URL);
    if let Some(tag) = etag {
        req = req.header(IF_NONE_MATCH, tag);
    }
    let resp = req.send().await.context("request")?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if !status.is_success() {
        bail!("HTTP {}", status.as_u16());
    }
    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = read_capped(resp, MAX_BYTES, "tracker.json").await?;
    Ok(Fetched::Body { body, etag })
}
```

`raw.stats.get("stars")` works on `Value` (returns `None` for non-objects). `Duration` is used in Task 3; if clippy flags it as unused now, add the import in Task 3 instead.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p rackscreen-sources portwatch`
Expected: PASS (the live test is ignored).
Run once by hand: `cargo test -p rackscreen-sources portwatch_live -- --ignored --nocapture`
Expected: PASS and prints the current leader.

- [ ] **Step 5: Commit**

```bash
git add crates/sources/src/portwatch.rs crates/sources/src/lib.rs crates/sources/testdata/portwatch-tracker.json
git commit -m "feat(sources): parse and fetch the PortWatch tracker

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Event, model state and the poll loop

**Files:**
- Modify: `crates/core/src/event.rs` (new variant at the end of `pub enum Event`, after `Sen(SenEvent)`)
- Modify: `crates/core/src/model.rs` (field next to `sen: SenState` ~line 508, init ~line 596, accessor near `pub fn sen`, `apply` arm near `Event::Sen(ev)` ~line 1461)
- Modify: `crates/sources/src/portwatch.rs` (add `run_portwatch`)

**Interfaces:**
- Consumes: `PortWatchSnapshot` (Task 1), `fetch`/`parse`/`Fetched`/`PortWatchConfig` (Task 2).
- Produces: `Event::PortWatch(crate::portwatch::PortWatchSnapshot)`; `Model::portwatch(&self) -> Option<&PortWatchSnapshot>`; `pub async fn run_portwatch(cfg: PortWatchConfig, ctx: SourceCtx)`.

- [ ] **Step 1: Write the failing model test**

In `crates/core/src/model.rs`'s test module add:

```rust
#[test]
fn portwatch_keeps_the_last_snapshot() {
    use crate::portwatch::{Milestone, PortWatchSnapshot};
    let mut m = Model::new(Thresholds::default());
    assert!(m.portwatch().is_none());
    let snap = PortWatchSnapshot {
        score: 91.3,
        milestone: Milestone::Playable,
        name: "Belobug".into(),
        private: false,
        verified: true,
        fps: Some(22.0),
        gpus: vec![],
        updated_unix: 1_795_796_303,
    };
    m.apply(Event::PortWatch(snap.clone()), 0.0);
    assert_eq!(m.portwatch(), Some(&snap));
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rackscreen-core portwatch_keeps_the_last_snapshot`
Expected: FAIL to compile (`no variant PortWatch`).

- [ ] **Step 3: Implement**

`crates/core/src/event.rs`, last variant of `Event`:

```rust
    /// The PortWatch VI leader, scored; sent when tracker.json changed.
    PortWatch(crate::portwatch::PortWatchSnapshot),
```

`crates/core/src/model.rs`:
- struct field after `sen: SenState,`: `portwatch: Option<crate::portwatch::PortWatchSnapshot>,`
- in `Model::new` after `sen: SenState::default(),`: `portwatch: None,`
- accessor after `pub fn sen(&self)`:

```rust
    pub fn portwatch(&self) -> Option<&crate::portwatch::PortWatchSnapshot> {
        self.portwatch.as_ref()
    }
```

- `apply` arm next to `Event::Sen(ev) => …`:

```rust
            Event::PortWatch(snap) => self.portwatch = Some(snap),
```

Fix any other exhaustive `match` on `Event` the compiler reports (none expected besides `apply`).

Append the loop to `crates/sources/src/portwatch.rs` (above the tests):

```rust
use rackscreen_core::event::Event;

use crate::SourceCtx;

/// Poll the tracker: a changed file is parsed and sent; an unchanged one
/// (304) costs one request; failures keep the last snapshot on screen and
/// retry after 1, 5, 15, then 60 minutes.
pub async fn run_portwatch(cfg: PortWatchConfig, ctx: SourceCtx) {
    let mut etag: Option<String> = None;
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let got = match fetch(etag.as_deref()).await {
            Ok(Fetched::NotModified) => Ok(None),
            Ok(Fetched::Body { body, etag: tag }) => parse(&body).map(|s| Some((s, tag))),
            Err(e) => Err(e),
        };
        let wait = match got {
            Ok(None) => {
                failures = 0;
                Duration::from_secs(cfg.poll_secs)
            }
            Ok(Some((snap, tag))) => {
                failures = 0;
                etag = tag;
                tracing::info!(
                    "portwatch: leader {} {} % ({})",
                    snap.name,
                    snap.score,
                    snap.milestone.label()
                );
                ctx.emit(Event::PortWatch(snap));
                Duration::from_secs(cfg.poll_secs)
            }
            Err(e) => {
                failures += 1;
                if failures == 1 {
                    tracing::warn!("portwatch: {e:#}");
                } else {
                    tracing::debug!("portwatch: {e:#} (failure {failures})");
                }
                crate::gif::retry_delay(failures)
            }
        };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-core portwatch && cargo test -p rackscreen-sources portwatch && cargo clippy -p rackscreen-sources -p rackscreen-core --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/event.rs crates/core/src/model.rs crates/sources/src/portwatch.rs
git commit -m "feat: PortWatch event, model state and poller

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Icons and the route ring

**Files:**
- Create: `assets/icons/ms-{construction,terminal,image,list,public,directions_run,check_circle,verified,lock,schedule,sports_esports}.svg`, `assets/icons/brand-{amd,nvidia,intel}.svg`, `assets/icons/BRANDS-LICENSE.md`
- Modify: `scripts/fetch-assets.sh`, `crates/render/src/assets.rs`, `crates/core/src/p2/ring.rs`

**Interfaces:**
- Produces: icon names `ms-construction`, `ms-terminal`, `ms-image`, `ms-list`, `ms-public`, `ms-directions_run`, `ms-check_circle`, `ms-verified`, `ms-lock`, `ms-schedule`, `ms-sports_esports`, `brand-amd`, `brand-nvidia`, `brand-intel`; in `p2::ring`: `pub struct RouteMarker { pub band: usize, pub icon: &'static str }`, `RingKind::Route { frac: f32, bands: Vec<Color>, markers: Vec<RouteMarker> }`, `RingSpec::route(frac, bands, markers) -> RingSpec`, consts `ROUTE_R = 104.0`, `ROUTE_DIM_ALPHA = 0.35`, `MARKER_R = 8.0`, `MARKER_CUT_R = 9.5`, `MARKER_STROKE = 1.6`, `MARKER_ICON_PX = 10.5`.

- [ ] **Step 1: Fetch the icons**

In `scripts/fetch-assets.sh`, append to `MS_ICONS` (inside the quotes, after `videocam`):

```
  construction terminal image list public directions_run check_circle verified lock
  schedule sports_esports
```

and after the Material loop add:

```bash
# GPU vendor logos (Simple Icons, CC0-1.0) for the portwatch screen.
SIMPLE="https://cdn.jsdelivr.net/npm/simple-icons@13.21.0/icons"
for i in amd nvidia intel; do
  curl -fsSL "$SIMPLE/$i.svg" -o "assets/icons/brand-$i.svg"
  grep -q 'viewBox="0 0 24 24"' "assets/icons/brand-$i.svg" || { echo "bad svg: brand-$i"; exit 1; }
done
```

Fetch only the new files (the full script refetches everything; that is fine too):

```bash
MATERIAL="https://unpkg.com/@material-symbols/svg-700@0.23.0/rounded"
for i in construction terminal image list public directions_run check_circle verified lock schedule sports_esports; do
  curl -fsSL "$MATERIAL/$i.svg" | sed 's/width="48" height="48"/width="24" height="24"/' > "assets/icons/ms-$i.svg"
  grep -q '<svg[^>]*width="24" height="24"' "assets/icons/ms-$i.svg" || echo "bad svg: ms-$i"
done
for i in amd nvidia intel; do
  curl -fsSL "https://cdn.jsdelivr.net/npm/simple-icons@13.21.0/icons/$i.svg" -o "assets/icons/brand-$i.svg"
done
grep -L 'viewBox="0 0 24 24"' assets/icons/brand-*.svg   # expect no output
```

Create `assets/icons/BRANDS-LICENSE.md`:

```markdown
# brand-*.svg

`brand-amd.svg`, `brand-nvidia.svg` and `brand-intel.svg` are the AMD, NVIDIA
and Intel logos from Simple Icons 13.21.0 (https://simpleicons.org), released
under CC0-1.0. The marks belong to their owners; RackScreen uses them only to
show which GPU vendor a PortWatch effort runs on.
```

- [ ] **Step 2: Register them and run the asset test**

In `crates/render/src/assets.rs`, append inside `icons!( … )` after `"ms-toggle_on",`:

```rust
    "ms-construction",
    "ms-terminal",
    "ms-image",
    "ms-list",
    "ms-public",
    "ms-directions_run",
    "ms-check_circle",
    "ms-verified",
    "ms-lock",
    "ms-schedule",
    "ms-sports_esports",
    "brand-amd",
    "brand-nvidia",
    "brand-intel",
```

Run: `cargo test -p rackscreen-render every_icon_parses_as_svg`
Expected: PASS (Simple Icons have no `width`, usvg takes 24 from the viewBox; if the test reports another width, add `width="24" height="24"` to the `<svg>` tag of that file).

- [ ] **Step 3: Write the failing ring tests**

In `crates/core/src/p2/ring.rs` test module:

```rust
    fn route(frac: f32) -> RingSpec {
        use crate::theme::p2::{GREEN, TEXT3};
        let mut bands = vec![TEXT3; 5];
        bands.extend([GREEN; 5]);
        RingSpec::route(
            frac,
            bands,
            vec![
                RouteMarker { band: 0, icon: "ms-construction" },
                RouteMarker { band: 5, icon: "ms-directions_run" },
            ],
        )
    }

    fn arcs(d: &[Drawable]) -> Vec<(f32, f32)> {
        d.iter()
            .filter_map(|x| match x {
                Drawable::Arc { sweep_deg, alpha, r, .. } if (*r - ROUTE_R).abs() < 0.01 => {
                    Some((*sweep_deg, *alpha))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn route_lights_bands_up_to_frac() {
        let d = route(0.25).overlay();
        let a = arcs(&d);
        // two full bands, the third dimmed with its first half lit, seven dimmed
        let lit = a.iter().filter(|(_, al)| *al == 1.0).count();
        let dim = a.iter().filter(|(_, al)| *al == ROUTE_DIM_ALPHA).count();
        assert_eq!((lit, dim), (3, 8));
        assert_eq!(route(0.25).r, ROUTE_R);
    }

    #[test]
    fn route_markers_show_reached_current_and_ahead() {
        use crate::theme::p2::{GREEN, INK, TEXT3};
        let icon = |d: &[Drawable], name: &str| {
            d.iter()
                .find_map(|x| match x {
                    Drawable::Icon { name: n, color, alpha, .. } if *n == name => {
                        Some((*color, *alpha))
                    }
                    _ => None,
                })
                .unwrap()
        };
        // band 0 reached, band 5 ahead
        let d = route(0.3).overlay();
        assert_eq!(icon(&d, "ms-construction"), (INK, 0.9));
        assert_eq!(icon(&d, "ms-directions_run"), (GREEN, 0.55));
        // band 5 current
        let d = route(0.55).overlay();
        assert_eq!(icon(&d, "ms-directions_run"), (GREEN, 1.0));
        let _ = TEXT3;
        // every marker cuts the ring with a black disc first
        let cuts = d
            .iter()
            .filter(|x| matches!(x, Drawable::Ellipse { rx, color, .. }
                if *rx == MARKER_CUT_R && *color == crate::theme::BLACK))
            .count();
        assert_eq!(cuts, 2);
    }
```

Run: `cargo test -p rackscreen-core route_`
Expected: FAIL to compile.

- [ ] **Step 4: Implement `RingKind::Route`**

In `crates/core/src/p2/ring.rs`:

```rust
/// A milestone marker on a `Route` ring, centred on band `band`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteMarker {
    pub band: usize,
    pub icon: &'static str,
}
```

New variant in `RingKind`:

```rust
    /// Equal bands lit up to `frac` (behind it full, the band holding it lit
    /// to `frac` over a dimmed band, ahead dimmed), with a round marker per
    /// milestone: reached filled, current outlined and bright, ahead faint.
    Route {
        frac: f32,
        bands: Vec<Color>,
        markers: Vec<RouteMarker>,
    },
```

Constants next to the others:

```rust
/// The route ring sits 2 px inside the usual radius so its markers fit.
pub const ROUTE_R: f32 = 104.0;
/// Dimmed route bands; 0.3 would put `TEXT3` under the panel's floor.
pub const ROUTE_DIM_ALPHA: f32 = 0.35;
pub const MARKER_R: f32 = 8.0;
pub const MARKER_CUT_R: f32 = 9.5;
pub const MARKER_STROKE: f32 = 1.6;
pub const MARKER_ICON_PX: f32 = 10.5;
```

Constructor in `impl RingSpec`:

```rust
    pub fn route(frac: f32, bands: Vec<Color>, markers: Vec<RouteMarker>) -> RingSpec {
        RingSpec {
            r: ROUTE_R,
            ..RingSpec::new(RingKind::Route {
                frac,
                bands,
                markers,
            })
        }
    }
```

Arm in `overlay()`:

```rust
            RingKind::Route {
                frac,
                bands,
                markers,
            } => {
                if bands.is_empty() {
                    return out;
                }
                let n = bands.len() as f32;
                let gap = self.gap_frac(BANDS_GAP_K);
                let fr = frac.clamp(0.0, 1.0);
                let lit_of = |i: usize| ((fr * n) - i as f32).clamp(0.0, 1.0);
                for (i, c) in bands.iter().enumerate() {
                    let (a, b) = (i as f32 / n + gap / 2.0, (i as f32 + 1.0) / n - gap / 2.0);
                    let lit = lit_of(i);
                    if lit >= 1.0 {
                        out.extend(self.arc(a, b, *c, 1.0));
                    } else {
                        out.extend(self.arc(a, b, *c, ROUTE_DIM_ALPHA));
                        if lit > 0.0 {
                            out.extend(self.arc(a, a + (b - a) * lit, *c, 1.0));
                        }
                    }
                }
                for m in markers {
                    let Some(c) = bands.get(m.band).copied() else {
                        continue;
                    };
                    let (cx, cy) = self.point((m.band as f32 + 0.5) / n);
                    let lit = lit_of(m.band);
                    let (fill, stroke_a, icon_c, icon_a) = if lit >= 1.0 {
                        (Some(c), 1.0, INK, 0.9)
                    } else if lit > 0.0 {
                        (None, 1.0, c, 1.0)
                    } else {
                        (None, 0.45, c, 0.55)
                    };
                    out.push(Drawable::Ellipse {
                        cx,
                        cy,
                        rx: MARKER_CUT_R,
                        ry: MARKER_CUT_R,
                        color: BLACK,
                        alpha: 1.0,
                    });
                    if let Some(f) = fill {
                        out.push(Drawable::Ellipse {
                            cx,
                            cy,
                            rx: MARKER_R,
                            ry: MARKER_R,
                            color: f,
                            alpha: 1.0,
                        });
                    }
                    out.push(Drawable::Arc {
                        cx,
                        cy,
                        r: MARKER_R,
                        width: MARKER_STROKE,
                        start_deg: 0.0,
                        sweep_deg: 360.0,
                        color: c,
                        alpha: stroke_a,
                    });
                    out.push(Drawable::Icon {
                        name: m.icon,
                        cx,
                        cy,
                        size: MARKER_ICON_PX,
                        color: icon_c,
                        alpha: icon_a,
                        scale: 1.0,
                        dy: 0.0,
                    });
                }
            }
```

Add imports at the top of `ring.rs`: `use crate::theme::p2::INK;` and `use crate::theme::BLACK;` (merge with the existing `use crate::theme::p2::TRACK;`).

The `route_lights_bands_up_to_frac` count: at 0.25 bands 0 and 1 are lit (2 arcs at 1.0), band 2 has a dim arc plus a lit half (1 + 1), bands 3–9 dim (7) → lit 3, dim 8; the marker stroke arcs have `r = MARKER_R`, so the helper filters them out by radius.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rackscreen-core ring && cargo test -p rackscreen-render every_icon_parses_as_svg`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add assets/icons scripts/fetch-assets.sh crates/render/src/assets.rs crates/core/src/p2/ring.rs
git commit -m "feat: route ring with milestone markers; PortWatch icons and GPU logos

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The `portwatch` role and its screen

**Files:**
- Create: `crates/core/src/p2/roles/portwatch.rs`
- Modify: `crates/core/src/theme.rs` (enum, `ALL`, `title`, `icon`, `name`, test `thirty_one_roles_with_unique_names`), `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene.rs` (`role_scene`), `crates/core/src/scene_model.rs` (`needs_data`), `crates/core/src/model.rs` (`role_available`), `crates/web/src/roles.rs` (`group`, `description`), `web/src/mock/data.ts`, `scripts/role-previews.sh`, `crates/render/tests/golden.rs`
- Create: goldens `crates/render/tests/goldens/p2_portwatch.png`, `p2_portwatch_private.png`, `p2_portwatch_early.png`, `p2_portwatch_complete.png`, preview `web/public/roles/portwatch.webp`

**Interfaces:**
- Consumes: Task 1 (`Milestone`, `Gpu`, `hero_text`, `fps_text`, `fmt_age`), Task 3 (`Model::portwatch`), Task 4 (`RingSpec::route`, `RouteMarker`, icon names).
- Produces: `Role::PortWatch` (name `portwatch`, title `PORTWATCH`, icon `ms-sports_esports`); `pub fn portwatch_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene`; `pub fn route_ring(score: f32) -> RingSpec`.

- [ ] **Step 1: Add the role (compile-driven)**

`crates/core/src/theme.rs`:
- enum: add `PortWatch,` after `Sen,` with doc `/// The leading GTA VI PC-port effort from PortWatch VI.`
- `ALL: [Role; 32]`, append `Role::PortWatch,` after `Role::Sen,`
- `title`: `Role::PortWatch => "PORTWATCH",`
- `icon`: `Role::PortWatch => "ms-sports_esports",`
- `name`: `Role::PortWatch => "portwatch",`
- test rename and update:

```rust
    #[test]
    fn thirty_two_roles_with_unique_names() {
        assert_eq!(Role::ALL.len(), 32);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 32);
        assert_eq!(Role::parse("portwatch"), Some(Role::PortWatch));
        assert!(!Role::PortWatch.is_cluster() && !Role::PortWatch.is_sky());
        // (keep the remaining existing assertions of the old test unchanged)
```

`crates/core/src/p2/roles/mod.rs`: `pub mod portwatch;` and add `Role::PortWatch` to the `return None` arm (`Role::PowerMix | Role::Music | Role::Iss | Role::Gif(_) | Role::Sen | Role::PortWatch => return None,`), updating the doc comment ("…music, iss and portwatch, which draw themselves").

`crates/core/src/scene.rs` `role_scene`: `Role::PortWatch => crate::p2::roles::portwatch::portwatch_scene(model, now, ui),`

`crates/core/src/scene_model.rs` `needs_data`: `Role::PortWatch => self.portwatch().is_none(),`

`crates/core/src/model.rs` `role_available`:

```rust
            // skipped in the cycle until the first tracker.json arrived
            Role::PortWatch => self.portwatch().is_some(),
```

`crates/web/src/roles.rs`: in `group`: `Role::PortWatch => "Gaming",` (before `_ => "Cluster"`); in `description`:

```rust
        Role::PortWatch => {
            "How close the GTA VI PC port is: the leading effort's score from PortWatch VI, its milestone, fps and GPU."
        }
```

`web/src/mock/data.ts`, after the `printer` row:

```ts
  ["portwatch", "PORTWATCH", "Gaming", "How close the GTA VI PC port is, from PortWatch VI."],
```

`scripts/role-previews.sh`: append `portwatch` to `roles=(… printer sen portwatch)`.

- [ ] **Step 2: Write the failing scene tests**

Create `crates/core/src/p2/roles/portwatch.rs` with stubs and tests:

```rust
//! The PortWatch screen: the leader's score on a route ring of milestone
//! markers, a pill with the leader (verified check, private lock), fps and
//! GPU tiles, and how long ago the site updated.

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::font::{measure, FontId};
use crate::p2::layout::{PILL_H, PILL_MAX_TEXT_W, PILL_PAD, PILL_PX, PILL_FILL_ALPHA, PILL_R};
use crate::p2::ring::{RingKind, RingSpec, RouteMarker};
use crate::p2::text::{
    hero_at, hero_fit_scale, marquee_run, middle_baseline, run_centred, top_icon, unit_superscript,
};
use crate::p2::ui::ScreenUi;
use crate::portwatch::{fmt_age, fps_text, hero_text, Gpu, Milestone, PortWatchSnapshot};
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{CARD, TEXT, TEXT2, TEXT3};
use crate::theme::Role;

/// Layout (px on the 240 panel), from the approved mockup.
const HERO_BASE: f32 = 97.0;
const HERO_MAX_SCALE: f32 = 0.94;
const PILL_CY_PW: f32 = 124.0;
const TILE_Y: f32 = 143.0;
const TILE_W: f32 = 52.0;
const TILE_H: f32 = 44.0;
const TILE_R: f32 = 12.0;
const TILE_GAP: f32 = 4.0;
const TILE_TOP_CY: f32 = 161.0;
const TILE_LABEL_BASE: f32 = 178.0;
const TILE_VALUE_PX: f32 = 18.0;
const TILE_LABEL_PX: f32 = 8.5;
const FOOT_CY: f32 = 197.0;
const FOOT_PX: f32 = 9.0;
const FOOT_ICON_PX: f32 = 10.0;
const ICON_GAP: f32 = 3.0;
const VERIFIED_PX: f32 = 12.0;
const LOCK_PX: f32 = 11.0;
/// The widest the logos in the gpu tile may get.
const LOGO_MAX_W: f32 = 42.0;

/// The ten bands' colours: five ladder steps, four Playable, Complete.
fn band_colors() -> Vec<crate::theme::Color> {
    todo!()
}

/// The route ring for `score` (0..=100).
pub fn route_ring(score: f32) -> RingSpec {
    todo!()
}

pub fn portwatch_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::theme::p2::{GREEN, TEXT2 as T2};

    fn model(s: PortWatchSnapshot, unix: i64) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.apply(Event::PortWatch(s), 0.0);
        m
    }

    fn snap() -> PortWatchSnapshot {
        PortWatchSnapshot {
            score: 91.3,
            milestone: Milestone::Playable,
            name: "Belobug".into(),
            private: false,
            verified: true,
            fps: Some(22.0),
            gpus: vec![Gpu::Amd],
            updated_unix: 1_000_000,
        }
    }

    fn icons(s: &Scene) -> Vec<&'static str> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .collect()
    }

    fn texts(s: &Scene) -> String {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn ring_has_ten_bands_and_seven_markers() {
        let r = route_ring(91.3);
        match &r.kind {
            RingKind::Route { frac, bands, markers } => {
                assert!((frac - 0.913).abs() < 1e-6);
                assert_eq!(bands.len(), 10);
                assert_eq!(bands[0], T2);
                assert_eq!(bands[7], GREEN);
                assert_eq!(
                    markers.iter().map(|m| m.band).collect::<Vec<_>>(),
                    vec![0, 1, 2, 3, 4, 5, 9]
                );
                assert_eq!(markers[6].icon, "ms-check_circle");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn public_verified_leader() {
        let m = model(snap(), 1_000_000 + 2 * 3600 + 30);
        let s = portwatch_scene(&m, 120.0, &mut ScreenUi::new());
        let ic = icons(&s);
        assert!(ic.contains(&"ms-sports_esports"));
        assert!(ic.contains(&"ms-verified"));
        assert!(!ic.contains(&"ms-lock"));
        assert!(ic.contains(&"brand-amd"));
        assert!(ic.contains(&"ms-schedule"));
        let t = texts(&s);
        assert!(t.contains("Belobug"), "{t}");
        assert!(t.contains("22") && t.contains("fps") && t.contains("gpu"), "{t}");
        assert!(t.contains("2h ago"), "{t}");
        assert!(t.contains('%'), "{t}");
    }

    #[test]
    fn private_unverified_without_fps_or_gpu() {
        let mut p = snap();
        p.private = true;
        p.verified = false;
        p.fps = None;
        p.gpus = vec![];
        let m = model(p, 1_000_000);
        let s = portwatch_scene(&m, 120.0, &mut ScreenUi::new());
        let ic = icons(&s);
        assert!(ic.contains(&"ms-lock"));
        assert!(!ic.contains(&"ms-verified"));
        assert!(!ic.iter().any(|n| n.starts_with("brand-")));
        // fps and gpu both show a dash
        assert_eq!(texts(&s).matches('—').count(), 2, "{}", texts(&s));
    }

    #[test]
    fn two_gpus_fit_one_tile() {
        let mut p = snap();
        p.gpus = vec![Gpu::Amd, Gpu::Nvidia, Gpu::Intel];
        let m = model(p, 1_000_000);
        let s = portwatch_scene(&m, 120.0, &mut ScreenUi::new());
        let logos: Vec<(f32, f32)> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, cx, size, .. } if name.starts_with("brand-") => {
                    Some((*cx, *size))
                }
                _ => None,
            })
            .collect();
        assert_eq!(logos.len(), 2, "first two only");
        let left = logos[0].0 - logos[0].1 / 2.0;
        let right = logos[1].0 + logos[1].1 / 2.0;
        assert!(right - left <= LOGO_MAX_W + 0.01, "{left}..{right}");
        // inside the gpu tile (the right one)
        assert!(left >= CX + TILE_GAP / 2.0 && right <= CX + TILE_GAP / 2.0 + TILE_W);
    }

    #[test]
    fn the_hero_rolls_the_score() {
        let m = model(snap(), 1_000_000);
        let s = portwatch_scene(&m, 120.0, &mut ScreenUi::new());
        let t = texts(&s);
        for ch in ["9", "1", ".", "3"] {
            assert!(t.contains(ch), "{t}");
        }
    }
}
```

(`Model::set_unix_now(i64)` is at `crates/core/src/model.rs:819`.)

Run: `cargo test -p rackscreen-core p2::roles::portwatch`
Expected: FAIL (`not yet implemented`).

- [ ] **Step 3: Implement the scene**

```rust
fn band_colors() -> Vec<crate::theme::Color> {
    let mut c: Vec<_> = Milestone::ALL[..5].iter().map(|m| m.color()).collect();
    c.extend([Milestone::Playable.color(); 4]);
    c.push(Milestone::Complete.color());
    c
}

pub fn route_ring(score: f32) -> RingSpec {
    let bands = band_colors();
    let markers = Milestone::ALL
        .iter()
        .map(|m| RouteMarker {
            // Complete marks the last band, the rest the band they start
            band: if *m == Milestone::Complete {
                9
            } else {
                (m.at() / 10.0) as usize
            },
            icon: m.icon(),
        })
        .collect();
    RingSpec::route((score / 100.0).clamp(0.0, 1.0), bands, markers)
}

/// Pill: [verified] name [lock], one group centred; a long name scrolls.
fn pill(s: &PortWatchSnapshot, ui: &mut ScreenUi, now: Secs) -> Vec<Drawable> {
    let color = s.milestone.color();
    let lead = if s.verified { VERIFIED_PX + ICON_GAP } else { 0.0 };
    let tail = if s.private { ICON_GAP + LOCK_PX } else { 0.0 };
    let name_w = measure(FontId::SemiBold, PILL_PX, &s.name, 0.0);
    let slot = name_w.min(PILL_MAX_TEXT_W - lead - tail);
    let content = lead + slot + tail;
    let w = content + PILL_PAD;
    let cy = PILL_CY_PW;
    let icon_cy = cy + 0.5;
    let x0 = CX - content / 2.0;
    let mut out = vec![Drawable::RoundRect {
        x: CX - w / 2.0,
        y: cy - PILL_H / 2.0,
        w,
        h: PILL_H,
        r: PILL_R,
        color,
        alpha: PILL_FILL_ALPHA,
    }];
    let icon = |name: &'static str, cx: f32, size: f32| Drawable::Icon {
        name,
        cx,
        cy: icon_cy,
        size,
        color,
        alpha: 1.0,
        scale: 1.0,
        dy: 0.0,
    };
    if s.verified {
        out.push(icon("ms-verified", x0 + VERIFIED_PX / 2.0, VERIFIED_PX));
    }
    let baseline = middle_baseline(FontId::SemiBold, PILL_PX, cy + 0.5);
    out.extend(marquee_run(
        ui,
        ("pill", 0),
        &s.name,
        x0 + lead + slot / 2.0,
        baseline,
        slot,
        FontId::SemiBold,
        PILL_PX,
        color,
        cy - PILL_H / 2.0,
        PILL_H,
        now,
    ));
    if s.private {
        out.push(icon("ms-lock", x0 + lead + slot + ICON_GAP + LOCK_PX / 2.0, LOCK_PX));
    }
    out
}

/// One logo's size (its width: the Simple Icons fill the 24-unit box
/// across) alone in the tile, and beside another.
fn logo_size(g: Gpu, alone: bool) -> f32 {
    match (g, alone) {
        (Gpu::Amd, true) => 34.0,
        (Gpu::Nvidia, true) => 22.0,
        (Gpu::Intel, true) => 30.0,
        (Gpu::Amd, false) => 24.0,
        (Gpu::Nvidia, false) => 15.0,
        (Gpu::Intel, false) => 20.0,
    }
}

fn tiles(s: &PortWatchSnapshot) -> Vec<Drawable> {
    let mut out = Vec::new();
    let left = CX - TILE_GAP / 2.0 - TILE_W;
    let right = CX + TILE_GAP / 2.0;
    for x in [left, right] {
        out.push(Drawable::RoundRect {
            x,
            y: TILE_Y,
            w: TILE_W,
            h: TILE_H,
            r: TILE_R,
            color: CARD,
            alpha: 1.0,
        });
    }
    let value_base = middle_baseline(FontId::SemiBold, TILE_VALUE_PX, TILE_TOP_CY);
    let value = |cx: f32, text: &str, color| {
        run_centred(cx, value_base, text, FontId::SemiBold, TILE_VALUE_PX, 0.0, color, 1.0)
    };
    let label = |cx: f32, text: &str| {
        run_centred(cx, TILE_LABEL_BASE, text, FontId::Medium, TILE_LABEL_PX, 0.0, TEXT2, 1.0)
    };
    let (lc, rc) = (left + TILE_W / 2.0, right + TILE_W / 2.0);
    out.push(value(lc, &fps_text(s.fps), if s.fps.is_some() { TEXT } else { TEXT3 }));
    out.push(label(lc, "fps"));
    let gpus = &s.gpus[..s.gpus.len().min(2)];
    if gpus.is_empty() {
        out.push(value(rc, "—", TEXT3));
    } else {
        let alone = gpus.len() == 1;
        let sizes: Vec<f32> = gpus.iter().map(|g| logo_size(*g, alone)).collect();
        let raw = sizes.iter().sum::<f32>() + ICON_GAP * (sizes.len() as f32 - 1.0);
        let k = (LOGO_MAX_W / raw).min(1.0);
        let mut x = rc - raw * k / 2.0;
        for (g, size) in gpus.iter().zip(&sizes) {
            let w = size * k;
            out.push(Drawable::Icon {
                name: g.icon(),
                cx: x + w / 2.0,
                cy: TILE_TOP_CY,
                size: w,
                color: g.color(),
                alpha: 1.0,
                scale: 1.0,
                dy: 0.0,
            });
            x += w + ICON_GAP * k;
        }
    }
    out.push(label(rc, "gpu"));
    out
}

fn footer(text: &str) -> Vec<Drawable> {
    let tw = measure(FontId::Medium, FOOT_PX, text, 0.0);
    let total = FOOT_ICON_PX + ICON_GAP + tw;
    let x0 = CX - total / 2.0;
    vec![
        Drawable::Icon {
            name: "ms-schedule",
            cx: x0 + FOOT_ICON_PX / 2.0,
            cy: FOOT_CY,
            size: FOOT_ICON_PX,
            color: TEXT3,
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        },
        Drawable::Run {
            x: x0 + FOOT_ICON_PX + ICON_GAP,
            baseline: middle_baseline(FontId::Medium, FOOT_PX, FOOT_CY),
            text: text.to_string(),
            font: FontId::Medium,
            px: FOOT_PX,
            letter_spacing: 0.0,
            color: TEXT3,
            alpha: 1.0,
            clip: None,
        },
    ]
}

pub fn portwatch_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    let mut scene = Scene::new();
    let Some(s) = model.portwatch() else {
        // `scene_for_role` shows the waiting screen before data; keep the
        // dispatcher total anyway
        scene.push(top_icon(Role::PortWatch.icon()));
        return scene;
    };
    let spec = route_ring(s.score);
    scene.items.extend(spec.track());
    scene.push(top_icon(Role::PortWatch.icon()));
    scene.base_len = scene.items.len();
    let spec = match spec.kind {
        RingKind::Route { frac, bands, markers } => {
            let frac = ui.glide(("ring", 0), frac, now);
            RingSpec::route(frac, bands, markers)
        }
        _ => spec,
    };
    scene.items.extend(spec.overlay());
    let text = hero_text(s.score);
    let scale = hero_fit_scale(&text, Some("%"), HERO_MAX_SCALE, HERO_BASE);
    let unit_w = crate::p2::layout::UNIT_GAP
        + measure(
            FontId::Bold,
            crate::p2::layout::HERO_PX * scale * crate::p2::layout::UNIT_SCALE,
            "%",
            0.0,
        );
    let h = hero_at(ui, &text, scale, CX - unit_w / 2.0, HERO_BASE, TEXT, now);
    scene.items.extend(h.drawables.iter().cloned());
    scene.push(unit_superscript(&h, "%"));
    scene.items.extend(pill(s, ui, now));
    scene.items.extend(tiles(s));
    scene
        .items
        .extend(footer(&fmt_age(model.unix_now() - s.updated_unix)));
    scene
}
```

Check while implementing:
- `marquee_run`'s exact parameter order in `crates/core/src/p2/text.rs:294` (ui, key, text, cx, baseline, slot_w, font, px, color, clip_y, clip_h, now) and that its key type is `UiKey = (&'static str, u32)` (as used by `pill`).
- `Scene::push` and `Scene::new` exist (used in `iss_scene`).
- `RingSpec::point` uses `self.r`, so markers follow `ROUTE_R`.
- `hero_fit_scale(text, unit, max_scale, baseline)` signature at `text.rs:171`.

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: PASS, including `thirty_two_roles_with_unique_names`, `p2::roles::portwatch::tests::*`, `role_index_roundtrip`. Fix any other test in core that counts roles (search: `grep -rn "31" crates/core/src --include=*.rs | grep -i role`).

- [ ] **Step 5: Goldens**

In `crates/render/tests/golden.rs`:

`full_model()`: before `m.tick(0.0); m` add

```rust
    m.apply(
        Event::PortWatch(PortWatchSnapshot {
            score: 91.3,
            milestone: Milestone::Playable,
            name: "Belobug".into(),
            private: false,
            verified: true,
            fps: Some(22.0),
            gpus: vec![Gpu::Amd],
            updated_unix: UNIX - 2 * 3600,
        }),
        0.0,
    );
```

with `use rackscreen_core::portwatch::{Gpu, Milestone, PortWatchSnapshot};` at the top. Then a new test after `printer_and_home_assistant`:

```rust
/// PortWatch: a private leader, an early race and a complete port.
#[test]
fn portwatch_states() {
    let states = [
        (
            "private",
            PortWatchSnapshot {
                score: 78.7,
                milestone: Milestone::Playable,
                name: "Doomsday".into(),
                private: true,
                verified: true,
                fps: Some(4.0),
                gpus: vec![Gpu::Amd, Gpu::Nvidia],
                updated_unix: UNIX - 14 * 60,
            },
        ),
        (
            "early",
            PortWatchSnapshot {
                score: 34.0,
                milestone: Milestone::MainMenu,
                name: "VIctory-NVIDIA".into(),
                private: false,
                verified: false,
                fps: None,
                gpus: vec![Gpu::Nvidia, Gpu::Intel],
                updated_unix: UNIX - 3 * 3600,
            },
        ),
        (
            "complete",
            PortWatchSnapshot {
                score: 100.0,
                milestone: Milestone::Complete,
                name: "Belobug".into(),
                private: false,
                verified: true,
                fps: Some(31.0),
                gpus: vec![Gpu::Amd],
                updated_unix: UNIX - 5 * 60,
            },
        ),
    ];
    for (name, snap) in states {
        let mut m = full_model();
        m.apply(Event::PortWatch(snap), 0.0);
        m.tick(0.0);
        check(
            &format!("p2_portwatch_{name}"),
            &render(&m, Role::PortWatch, 120.0),
        );
    }
}
```

Generate and inspect:

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden portwatch
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden every_role_normal
cargo test -p rackscreen-render --test golden
git status --short crates/render/tests/goldens
```

Expected: only `p2_portwatch*.png` are new; no other golden changes. Open `p2_portwatch.png`, `p2_portwatch_private.png`, `p2_portwatch_early.png`, `p2_portwatch_complete.png` (Read tool) and compare with the mockup (row C of `docs/superpowers/specs/assets/2026-11-27-portwatch-mockup.html`): markers on the ring at the band centres, the check marker gold at 100 %, pill group centred, tiles side by side under the pill, footer clock and text on one line, no overlap with the ring. `contrast_floor_on_every_role` and `base_cache_hit_matches_a_fresh_render` must pass.

Make the web preview: `scripts/role-previews.sh` (needs ImageMagick `magick`), then `git status --short web/public/roles` shows only `portwatch.webp` new (others unchanged byte-for-byte; if `magick` re-encodes the others differently, `git checkout web/public/roles` and run just: `magick crates/render/tests/goldens/p2_portwatch.png -resize 160x160 -quality 82 -define webp:method=6 web/public/roles/portwatch.webp`).

- [ ] **Step 6: Workspace tests and lint**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && (cd web && npm run build)`
Expected: PASS. Fix compile errors in exhaustive role matches the compiler points to (e.g. `crates/web/src/roles.rs::description`).

- [ ] **Step 7: Commit**

```bash
git add crates/core crates/web/src/roles.rs web/src/mock/data.ts scripts/role-previews.sh crates/render/tests web/public/roles/portwatch.webp
git commit -m "feat: portwatch role: route ring, leader pill, fps and GPU tiles

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Config, spawning and the demo source

**Files:**
- Modify: `crates/app/src/config.rs`, `crates/app/src/run.rs`, `config.example.yaml`, `crates/sources/src/fake.rs`

**Interfaces:**
- Consumes: `run_portwatch`, `PortWatchConfig` (Tasks 2–3), `Role::PortWatch` (Task 5), `Config::uses_role`.
- Produces: `Config.portwatch: PortWatchCfg { poll_secs: u64 }` (default 900); `PORTWATCH_MIN_POLL_SECS = 300`.

- [ ] **Step 1: Failing config tests**

In `crates/app/src/config.rs` tests:

```rust
    #[test]
    fn portwatch_poll_defaults_and_floor() {
        let cfg = Config::default();
        assert_eq!(cfg.portwatch.poll_secs, 900);
        assert_eq!(cfg.portwatch.effective_poll_secs(), 900);
        let mut low = cfg.clone();
        low.portwatch.poll_secs = 10;
        assert_eq!(low.portwatch.effective_poll_secs(), 300);
        // the default is not written back to the file
        let yaml = serde_yaml_ng::to_string(&cfg).unwrap();
        assert!(!yaml.contains("portwatch"), "{yaml}");
    }
```

(The app crate's YAML library is `serde_yaml_ng`.)

Run: `cargo test -p rackscreen-app portwatch_poll`
Expected: FAIL to compile.

- [ ] **Step 2: Implement config**

Field in `Config`, after `sen`:

```rust
    #[serde(default, skip_serializing_if = "PortWatchCfg::is_default")]
    pub portwatch: PortWatchCfg,
```

Struct next to `SenCfg`:

```rust
/// PortWatch VI (portwatch-vi.xyz): how often tracker.json is checked.
/// The site updates about every 3 hours; an unchanged file costs one 304.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PortWatchCfg {
    pub poll_secs: u64,
}

pub const PORTWATCH_MIN_POLL_SECS: u64 = 300;

impl Default for PortWatchCfg {
    fn default() -> Self {
        Self { poll_secs: 900 }
    }
}

impl PortWatchCfg {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn effective_poll_secs(&self) -> u64 {
        self.poll_secs.max(PORTWATCH_MIN_POLL_SECS)
    }
}
```

`config.example.yaml`, after the commented `sen:` block:

```yaml
# portwatch:
#   poll_secs: 900         # how often portwatch-vi.xyz/data/tracker.json is
#                          # checked (min 300); used by the portwatch role
```

- [ ] **Step 3: Spawn when used**

In `crates/app/src/run.rs`, inside `spawn_external_sources` at its start (before the jellyfin block):

```rust
    if cfg.uses_role(Role::PortWatch) {
        runtime.spawn(rackscreen_sources::portwatch::run_portwatch(
            rackscreen_sources::portwatch::PortWatchConfig {
                poll_secs: cfg.portwatch.effective_poll_secs(),
            },
            ctx.clone(),
        ));
    }
```

(`Role` is already imported in `run.rs`; `spawn_external_sources` only runs for real sources, so the fake source feeds the demo below.)

- [ ] **Step 4: Demo snapshot in the fake source**

In `crates/sources/src/fake.rs`, add to `initial()`'s list after `self.home_assistant(),`:

```rust
            self.portwatch(),
```

and the builder next to `fn printer`:

```rust
    /// Today's PortWatch leader, updated two hours before the clock.
    fn portwatch(&self) -> Event {
        use rackscreen_core::portwatch::{Gpu, Milestone, PortWatchSnapshot};
        Event::PortWatch(PortWatchSnapshot {
            score: 91.3,
            milestone: Milestone::Playable,
            name: "Belobug".into(),
            private: false,
            verified: true,
            fps: Some(22.0),
            gpus: vec![Gpu::Amd],
            updated_unix: self.unix_start - 2 * 3600,
        })
    }
```

Extend `initial_has_every_new_role_fed`:

```rust
        assert!(evs.iter().any(|e| matches!(e, Event::PortWatch(_))));
```

- [ ] **Step 5: Run**

Run: `cargo test -p rackscreen-app && cargo test -p rackscreen-sources && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

Manual check in the simulator (fake source), if available locally: run the app with a screen whose roles include `portwatch` (see README "simulator" section for the command; e.g. `cargo run -- run --fake --sim` with a config that lists `portwatch`) and look at the window.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/config.rs crates/app/src/run.rs config.example.yaml crates/sources/src/fake.rs
git commit -m "feat: portwatch config, poller spawn and demo data

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Version and final check

**Files:**
- Modify: `Cargo.toml` (`version = "0.21.0"`), `Cargo.lock` (via cargo)

- [ ] **Step 1: Bump**

Set `[workspace.package] version = "0.21.0"` in `Cargo.toml`, then `cargo build --workspace` to refresh `Cargo.lock`.

- [ ] **Step 2: Full verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd web && npm run build)
cargo test -p rackscreen-sources portwatch_live -- --ignored --nocapture
```

Expected: all PASS; the live test prints the current leader.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: version 0.21.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

Release and Pi deploy are Silke's call (push + tag `v0.21.0`, release workflow, then `sudo rackscreen update` on the Pi, and add `portwatch` to a screen in the web UI).

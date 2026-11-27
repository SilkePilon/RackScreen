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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ranked {
    /// Index into the efforts slice.
    pub index: usize,
    pub scored: Scored,
}

/// Efforts that are not nuked, best first; equal scores rank more stars first.
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

/// The mean of the first three scores, one decimal; 0 without any.
pub fn top3_average(ranked: &[Ranked]) -> f32 {
    let top = &ranked[..ranked.len().min(3)];
    if top.is_empty() {
        return 0.0;
    }
    let sum: f64 = top.iter().map(|r| r.scored.score as f64).sum();
    round1(sum / top.len() as f64) as f32
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

/// The hero: one decimal, `.0` dropped (`91.3`, `100`, `34`).
pub fn hero_text(score: f32) -> String {
    let s = format!("{score:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// The fps tile: `22`, `4.5`, `—` without a figure.
pub fn fps_text(fps: Option<f32>) -> String {
    match fps {
        None => "—".into(),
        Some(f) if f >= 10.0 => format!("{f:.0}"),
        Some(f) => hero_text(f),
    }
}

/// The footer: `just now`, `5m ago`, `2h ago`, `3d ago`.
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
        assert_eq!(
            score(&playable(1.0, 1.0, 120.0, 1.0)).milestone,
            Milestone::Complete
        );
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
            playable(0.8, 0.8, 22.0, 1.0),  // 91.3
            playable(0.8, 0.75, 20.0, 1.0), // 90.0
            playable(0.5, 0.7, 25.0, 1.0),  // something lower
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

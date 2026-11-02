//! The power-mix ring: twenty 5 % segments coloured by source, the leading
//! source's run centred at 12 o'clock, and a capsule (icon + share) on the
//! ring for every source with at least two segments.

use crate::anim::Secs;
use crate::electricity::Source;
use crate::model::Model;
use crate::p2::font::{measure, FontId};
use crate::p2::layout::EDGE_R;
use crate::p2::ring::{RingKind, RingSpec};
use crate::p2::text::middle_baseline;
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{AMBER, BLUE, CYAN, INK, TRACK, VIOLET};
use crate::theme::{Color, BLACK};

pub const SEGMENTS: usize = 20;
pub const MIX_R: f32 = 104.0;
pub const MIX_W: f32 = 9.0;
pub const MIX_GAP_K: f32 = 1.5;
pub const CAPSULE_H: f32 = 22.0;
pub const CAPSULE_PX: f32 = 10.0;
pub const ICON_PX: f32 = 14.0;
/// Rotation to a new leader.
pub const ROTATE_SECS: Secs = 1.0;
/// Stagger of the segment colour fades.
pub const FADE_STAGGER: Secs = 0.02;

pub fn source_color(s: Source) -> Color {
    match s {
        Source::Wind => CYAN,
        Source::Solar => AMBER,
        Source::Gas => Color::hex(0xF2A65A),
        Source::Nuclear => VIOLET,
        Source::Coal => Color::hex(0x9A8F85),
        Source::Biomass => Color::hex(0x7CCF8E),
        Source::Hydro => BLUE,
        _ => Color::hex(0x5A5A60),
    }
}

/// Percent of total production per source, in `Source::ALL` order, zero shares dropped.
pub fn shares_pct(mix_mw: &[(Source, f32)]) -> Vec<(Source, f32)> {
    let total: f32 = mix_mw.iter().map(|(_, mw)| mw.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut out: Vec<(Source, f32)> = mix_mw
        .iter()
        .map(|(s, mw)| (*s, mw.max(0.0) / total * 100.0))
        .filter(|(_, p)| *p > 0.0)
        .collect();
    out.sort_by_key(|(s, _)| s.index());
    out
}

/// Largest-remainder apportionment of the 20 segments, input order kept.
pub fn segment_counts(shares: &[(Source, f32)]) -> Vec<(Source, usize)> {
    let total: f32 = shares.iter().map(|(_, p)| p.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let raw: Vec<f32> = shares
        .iter()
        .map(|(_, p)| p.max(0.0) / total * SEGMENTS as f32)
        .collect();
    let mut counts: Vec<usize> = raw.iter().map(|r| r.floor() as usize).collect();
    let mut left = SEGMENTS - counts.iter().sum::<usize>();
    let mut by_rem: Vec<(usize, f32)> = raw
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r - r.floor()))
        .collect();
    by_rem.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (i, _) in by_rem {
        if left == 0 {
            break;
        }
        counts[i] += 1;
        left -= 1;
    }
    shares
        .iter()
        .zip(counts)
        .map(|((s, _), c)| (*s, c))
        .collect()
}

/// Index of the leader: most segments, then the larger share, then `Source::ALL` order.
pub fn leader(counts: &[(Source, usize)], shares: &[(Source, f32)]) -> Option<usize> {
    let share = |i: usize| shares.get(i).map(|s| s.1).unwrap_or(0.0);
    (0..counts.len()).max_by(|&a, &b| {
        counts[a]
            .1
            .cmp(&counts[b].1)
            .then_with(|| share(a).total_cmp(&share(b)))
            .then_with(|| counts[b].0.index().cmp(&counts[a].0.index()))
    })
}

/// Start angle of segment 0 (clockwise from 12 o'clock) that centres the
/// leader's run on 12 o'clock.
pub fn rotation_for(counts: &[(Source, usize)], lead: usize) -> f32 {
    let before: usize = counts.iter().take(lead).map(|c| c.1).sum();
    let mid = before as f32 + counts.get(lead).map(|c| c.1).unwrap_or(0) as f32 / 2.0;
    -mid / SEGMENTS as f32 * 360.0
}

/// Centre of a capsule `w × h` on the ring at `angle_deg`, moved inward along
/// its angle until the whole capsule (3 px outline included) sits inside the edge.
pub fn capsule_centre(angle_deg: f32, w: f32, h: f32) -> (f32, f32) {
    let a = angle_deg.to_radians();
    let mut rr = MIX_R;
    let mut p = (CX + rr * a.sin(), CY - rr * a.cos());
    while rr > 60.0 {
        p = (CX + rr * a.sin(), CY - rr * a.cos());
        let fx = (p.0 - CX).abs() + w / 2.0 + 3.0;
        let fy = (p.1 - CY).abs() + h / 2.0 + 3.0;
        if fx.hypot((p.1 - CY).abs()) < EDGE_R && (p.0 - CX).abs().hypot(fy) < EDGE_R {
            break;
        }
        rr -= 1.0;
    }
    p
}

pub fn power_mix_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    let mut scene = Scene::new();
    // shares glide per source so the counts move one segment at a time
    let shares: Vec<(Source, f32)> = shares_pct(&model.electricity().mix)
        .into_iter()
        .map(|(s, p)| (s, ui.glide(("share", s.index() as u32), p, now)))
        .collect();
    let counts = segment_counts(&shares);
    let rot = match leader(&counts, &shares) {
        Some(lead) => ui.glide_angle(("rot", 0), rotation_for(&counts, lead), now, ROTATE_SECS),
        None => 0.0,
    };
    let spec = RingSpec {
        kind: RingKind::Empty,
        r: MIX_R,
        width: MIX_W,
        start_deg: rot,
        sweep_deg: 360.0,
    };
    let gap = spec.gap_frac(MIX_GAP_K);
    let mut owner: Vec<Color> = Vec::with_capacity(SEGMENTS);
    for (s, c) in &counts {
        for _ in 0..*c {
            owner.push(source_color(*s));
        }
    }
    owner.resize(SEGMENTS, TRACK);
    for (j, color) in owner.iter().enumerate() {
        let f0 = j as f32 / SEGMENTS as f32 + gap / 2.0;
        let f1 = (j + 1) as f32 / SEGMENTS as f32 - gap / 2.0;
        for (c, alpha) in ui.fade(("seg", j as u32), *color, now, j as Secs * FADE_STAGGER) {
            scene.items.extend(spec.arc(f0, f1, c, alpha));
        }
    }
    let mut acc = 0usize;
    for (i, (s, c)) in counts.iter().enumerate() {
        let mid = rot + (acc as f32 + *c as f32 / 2.0) / SEGMENTS as f32 * 360.0;
        acc += c;
        if *c < 2 {
            continue;
        }
        let text = format!("{:.0}%", shares[i].1);
        let tw = measure(FontId::Bold, CAPSULE_PX, &text, 0.0);
        let cw = 16.0 + 3.0 + tw + 14.0;
        let (x, y) = capsule_centre(mid, cw, CAPSULE_H);
        let color = source_color(*s);
        scene.push(Drawable::RoundRect {
            x: x - cw / 2.0 - 3.0,
            y: y - CAPSULE_H / 2.0 - 3.0,
            w: cw + 6.0,
            h: CAPSULE_H + 6.0,
            r: (CAPSULE_H + 6.0) / 2.0,
            color: BLACK,
            alpha: 1.0,
        });
        scene.push(Drawable::RoundRect {
            x: x - cw / 2.0,
            y: y - CAPSULE_H / 2.0,
            w: cw,
            h: CAPSULE_H,
            r: CAPSULE_H / 2.0,
            color,
            alpha: 1.0,
        });
        scene.push(Drawable::Icon {
            name: s.icon(),
            cx: x - cw / 2.0 + 6.0 + ICON_PX / 2.0,
            cy: y,
            size: ICON_PX,
            color: INK,
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        });
        scene.push(Drawable::Run {
            x: x - cw / 2.0 + 23.0,
            baseline: middle_baseline(FontId::Bold, CAPSULE_PX, y + 0.5),
            text,
            font: FontId::Bold,
            px: CAPSULE_PX,
            letter_spacing: 0.0,
            color: INK,
            alpha: 1.0,
            clip: None,
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electricity::Source;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ui::ScreenUi;
    use crate::scene::Drawable;

    fn shares() -> Vec<(Source, f32)> {
        vec![
            (Source::Wind, 38.0),
            (Source::Solar, 24.0),
            (Source::Gas, 14.0),
            (Source::Nuclear, 12.0),
            (Source::Coal, 7.0),
            (Source::Biomass, 5.0),
        ]
    }

    #[test]
    fn largest_remainder_fills_twenty_segments_and_the_leader_is_centred() {
        let counts = segment_counts(&shares());
        assert_eq!(counts.iter().map(|c| c.1).sum::<usize>(), SEGMENTS);
        assert_eq!(
            counts[0],
            (Source::Wind, 8),
            "38 % -> 7.6 -> 8 with its remainder"
        );
        assert_eq!(counts[1], (Source::Solar, 5));
        assert_eq!(counts[2], (Source::Gas, 3));
        assert_eq!(counts[5], (Source::Biomass, 1));
        assert_eq!(leader(&counts, &shares()), Some(0));
        let rot = rotation_for(&counts, 0);
        assert!(
            (rot - (-(8.0 / 2.0) / 20.0 * 360.0)).abs() < 1e-4,
            "wind's run centred on 12 o'clock: {rot}"
        );
        let tie = vec![
            (Source::Wind, 25.0),
            (Source::Solar, 25.0),
            (Source::Gas, 50.0),
        ];
        let c = segment_counts(&tie);
        assert_eq!(leader(&c, &tie), Some(2));
        assert!(segment_counts(&[]).is_empty());
        assert_eq!(
            segment_counts(&[(Source::Gas, 100.0)]),
            vec![(Source::Gas, 20)]
        );
    }

    #[test]
    fn capsules_move_inward_until_they_fit() {
        let (x, y) = capsule_centre(90.0, 60.0, 22.0);
        assert!(
            (y - 120.0).abs() < 1e-3,
            "3 o'clock stays on the horizontal"
        );
        assert!(
            x < 120.0 + MIX_R && x > 160.0,
            "shifted inward from the ring: {x}"
        );
        let (_, y0) = capsule_centre(0.0, 60.0, 22.0);
        assert!(y0 > 120.0 - MIX_R, "12 o'clock moves down: {y0}");
        let (x45, y45) = capsule_centre(45.0, 40.0, 22.0);
        let d = ((x45 - 120.0).powi(2) + (y45 - 120.0).powi(2)).sqrt();
        assert!(d + 22.0 / 2.0 + 3.0 <= 116.0 + 1e-3);
    }

    #[test]
    fn scene_has_twenty_segments_and_a_capsule_per_big_source() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: shares().into_iter().map(|(s, p)| (s, p * 100.0)).collect(),
                renewable_pct: 67.0,
                fossil_free_pct: 79.0,
                carbon_gco2: 150.0,
                updated_at: String::new(),
            },
            0.0,
        );
        let mut ui = ScreenUi::new();
        let s = power_mix_scene(&m, 1.0, &mut ui);
        let arcs = s
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Arc { .. }))
            .count();
        assert_eq!(arcs, SEGMENTS, "one arc per segment, no fades yet");
        let icons: Vec<&str> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .collect();
        assert_eq!(
            icons,
            vec!["em-nuclear", "em-wind", "em-solar", "em-gas"],
            "Source::ALL order; coal (1) and biomass (1) get no capsule"
        );
        let runs: Vec<String> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(runs, vec!["12%", "38%", "24%", "14%"]);
        assert!(
            !s.items
                .iter()
                .any(|d| matches!(d, Drawable::CurvedText { .. })),
            "no title"
        );
        // a leader change turns the ring and fades the segments
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: vec![(Source::Solar, 7000.0), (Source::Wind, 3000.0)],
                renewable_pct: 100.0,
                fossil_free_pct: 100.0,
                carbon_gco2: 20.0,
                updated_at: String::new(),
            },
            2.0,
        );
        power_mix_scene(&m, 2.0, &mut ui);
        let s2 = power_mix_scene(&m, 2.6, &mut ui);
        let arcs2 = s2
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Arc { .. }))
            .count();
        assert!(
            arcs2 > SEGMENTS,
            "segments that changed colour draw two layers while fading"
        );
        let start = |s: &crate::scene::Scene| match &s.items[1] {
            Drawable::Arc { start_deg, .. } => *start_deg,
            _ => panic!("arc"),
        };
        assert_ne!(start(&s), start(&s2), "the ring is turning");
        let empty = power_mix_scene(
            &Model::new(Thresholds::default()),
            0.0,
            &mut ScreenUi::new(),
        );
        assert_eq!(empty.items.len(), 1 + SEGMENTS, "track segments only");
    }
}

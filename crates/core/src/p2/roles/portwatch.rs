//! The PortWatch screen: the leader's score on a route ring of milestone
//! markers, a pill with the leader (verified check, private lock), fps and
//! GPU tiles, and how long ago the site updated.

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::font::{measure, FontId};
use crate::p2::layout::{PILL_FILL_ALPHA, PILL_H, PILL_MAX_TEXT_W, PILL_PAD, PILL_PX, PILL_R};
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
    let mut c: Vec<_> = Milestone::ALL[..5].iter().map(|m| m.color()).collect();
    c.extend([Milestone::Playable.color(); 4]);
    c.push(Milestone::Complete.color());
    c
}

/// The route ring for `score` (0..=100).
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
    let lead = if s.verified {
        VERIFIED_PX + ICON_GAP
    } else {
        0.0
    };
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
        out.push(icon(
            "ms-lock",
            x0 + lead + slot + ICON_GAP + LOCK_PX / 2.0,
            LOCK_PX,
        ));
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
        run_centred(
            cx,
            value_base,
            text,
            FontId::SemiBold,
            TILE_VALUE_PX,
            0.0,
            color,
            1.0,
        )
    };
    let label = |cx: f32, text: &str| {
        run_centred(
            cx,
            TILE_LABEL_BASE,
            text,
            FontId::Medium,
            TILE_LABEL_PX,
            0.0,
            TEXT2,
            1.0,
        )
    };
    let (lc, rc) = (left + TILE_W / 2.0, right + TILE_W / 2.0);
    out.push(value(
        lc,
        &fps_text(s.fps),
        if s.fps.is_some() { TEXT } else { TEXT3 },
    ));
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
        RingKind::Route {
            frac,
            bands,
            markers,
        } => {
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
            RingKind::Route {
                frac,
                bands,
                markers,
            } => {
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
        assert!(
            t.contains("22") && t.contains("fps") && t.contains("gpu"),
            "{t}"
        );
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

//! A P2 screen as plain data, and the composer that turns it into a `Scene`.

use crate::anim::Secs;
use crate::p2::chart::{card_drawables, Card};
use crate::p2::font::{measure, FontId};
use crate::p2::layout::*;
use crate::p2::ring::{RingKind, RingSpec};
use crate::p2::text::{
    hero_at, hero_fit_scale, pill, ring_ends, top_icon, unit_below, unit_superscript,
};
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{CARD, OK, TEXT};
use crate::theme::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct P2Screen {
    /// A short upper-case name; not drawn (the icon replaced the curved
    /// title), kept so builders and tests can tell states apart.
    pub title: String,
    /// Asset name of the icon in the ring gap; empty draws none. The role
    /// dispatcher fills it from `Role::icon`.
    pub icon: &'static str,
    pub hero: String,
    pub hero_scale: f32,
    pub hero_color: Color,
    pub unit: Option<String>,
    pub unit_below: bool,
    pub ring: RingSpec,
    /// Scale labels at the two ends of the ring (weather: day low and high).
    pub ends: Option<(String, String)>,
    pub pill: Option<(String, Color)>,
    pub card: Option<Card>,
    /// The health role's live indicator.
    pub live: bool,
}

impl P2Screen {
    pub fn new(title: impl Into<String>, hero: impl Into<String>) -> P2Screen {
        P2Screen {
            title: title.into(),
            icon: "",
            hero: hero.into(),
            hero_scale: 1.0,
            hero_color: TEXT,
            unit: None,
            unit_below: false,
            ring: RingSpec::empty(),
            ends: None,
            pill: None,
            card: None,
            live: false,
        }
    }
}

/// The ring with its animated fractions glided (0.65 s) by the screen's state.
fn glided_ring(spec: &RingSpec, ui: &mut ScreenUi, now: Secs) -> RingSpec {
    let kind = match &spec.kind {
        RingKind::Fill { frac, color } => RingKind::Fill {
            frac: ui.glide(("ring", 0), *frac, now),
            color: *color,
        },
        RingKind::Parts(parts) => RingKind::Parts(
            parts
                .iter()
                .enumerate()
                .map(|(i, (f, c))| (ui.glide(("part", i as u32), *f, now), *c))
                .collect(),
        ),
        RingKind::Bands { colors, frac } => RingKind::Bands {
            colors: colors.clone(),
            frac: ui.glide(("band", 0), *frac, now),
        },
        other => other.clone(),
    };
    RingSpec {
        kind,
        ..spec.clone()
    }
}

pub fn compose(s: &P2Screen, ui: &mut ScreenUi, now: Secs) -> Scene {
    let mut scene = Scene::new();
    let ring = glided_ring(&s.ring, ui, now);
    scene.items.extend(ring.track());
    if !s.icon.is_empty() {
        scene.push(top_icon(s.icon));
    }
    let pill_cy = if s.unit_below {
        PILL_CY_UNIT_BELOW
    } else {
        PILL_CY
    };
    if s.card.is_some() {
        scene.push(Drawable::RoundRect {
            x: CX - CARD_W / 2.0,
            y: card_top(pill_cy),
            w: CARD_W,
            h: CARD_H,
            r: CARD_R,
            color: CARD,
            alpha: 1.0,
        });
    }
    // The base is a prefix of the original paint order (fill, icon, ends,
    // hero, unit, pill, card background): a drawable joins it only when
    // nothing drawn before it overlaps it. The icon sits in the ring gap,
    // clear of the fill and its caps (`icon_sits_in_the_ring_gap_clear_of_the_caps`);
    // the card background sits inside the ring and below the pill, clear of
    // the fill, icon, ends, hero and unit. Moving both up is pixel-neutral.
    scene.base_len = scene.items.len();
    scene.items.extend(ring.overlay());
    if let Some((a, b)) = &s.ends {
        scene.items.extend(ring_ends(a, b));
    }
    let baseline = if s.unit_below {
        HERO_BASELINE_UNIT_BELOW
    } else {
        HERO_BASELINE
    };
    let unit_sup = s.unit.as_deref().filter(|_| !s.unit_below);
    let fit = hero_fit_scale(&s.hero, unit_sup, s.hero_scale, baseline);
    let scale = ui.glide_with(("hero-scale", 0), fit, now, HERO_FIT_GLIDE_SECS);
    let unit_w = unit_sup.map_or(0.0, |u| {
        UNIT_GAP + measure(FontId::Bold, HERO_PX * scale * UNIT_SCALE, u, 0.0)
    });
    let h = hero_at(
        ui,
        &s.hero,
        scale,
        CX - unit_w / 2.0,
        baseline,
        s.hero_color,
        now,
    );
    scene.items.extend(h.drawables.iter().cloned());
    if let Some(u) = &s.unit {
        scene.push(if s.unit_below {
            unit_below(u)
        } else {
            unit_superscript(&h, u)
        });
    }
    if let Some((text, color)) = &s.pill {
        scene.items.extend(pill(ui, text, *color, pill_cy, now));
    }
    if let Some(card) = &s.card {
        scene
            .items
            .extend(card_drawables(card, ui, chart_rect(pill_cy), now));
    }
    if s.live {
        let p = ((now % 2.0) / 2.0) as f32;
        scene.push(Drawable::Arc {
            cx: CX,
            cy: LIVE_CY,
            r: 2.0 + 5.0 * p,
            width: 1.0,
            start_deg: 0.0,
            sweep_deg: 360.0,
            color: OK,
            alpha: 0.6 * (1.0 - p),
        });
        scene.push(Drawable::Ellipse {
            cx: CX,
            cy: LIVE_CY,
            rx: 2.0,
            ry: 2.0,
            color: OK,
            alpha: 1.0,
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::chart::Card;
    use crate::p2::ring::RingSpec;
    use crate::p2::ui::ScreenUi;
    use crate::theme::layout::CY;
    use crate::theme::p2::{BLUE, CARD, OK, TEXT, TEXT2, TRACK};

    fn sample() -> P2Screen {
        let mut s = P2Screen::new("cpu", "42");
        s.icon = "ms-memory";
        s.unit = Some("%".into());
        s.ring = RingSpec::fill(0.42, BLUE);
        s.pill = Some(("7 nodes · avg".into(), TEXT2));
        s.card = Some(Card::Spark {
            series: (0..31).map(|i| 40.0 + (i % 5) as f32).collect(),
            prev: Some(39.0),
            scroll: 1.0,
            min: 0.0,
            max: 100.0,
            now_idx: None,
            color: BLUE,
            caption: "last 60 s".into(),
        });
        s
    }

    #[test]
    fn compose_orders_track_icon_card_then_fill_hero_unit_pill() {
        let mut ui = ScreenUi::new();
        let scene = compose(&sample(), &mut ui, 0.0);
        let items = &scene.items;
        assert!(matches!(items[0], Drawable::Clear(_)));
        assert!(
            matches!(items[1], Drawable::Arc { color, .. } if color == TRACK),
            "track first"
        );
        assert!(
            matches!(&items[2], Drawable::Icon { name, cx, cy, size, color, alpha, .. }
                if *name == "ms-memory" && *cx == CX && *cy == ICON_CY && *size == ICON_PX && *color == TEXT && *alpha == 0.70),
            "icon in the base, right after the track"
        );
        assert!(
            matches!(items[3], Drawable::RoundRect { color, .. } if color == CARD),
            "card background in the base"
        );
        assert_eq!(scene.base_len, 4);
        assert!(
            matches!(items[4], Drawable::Arc { sweep_deg, .. } if (sweep_deg - RING_SWEEP_DEG * 0.42).abs() < 1e-3),
            "fill right after the base"
        );
        let runs: Vec<&String> = items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(runs[0], "4");
        assert_eq!(runs[1], "2");
        assert_eq!(runs[2], "%");
        assert_eq!(runs[3], "7 nodes · avg");
        assert!(items.iter().any(|d| matches!(d, Drawable::RoundRect { y, w, h, color, .. } if *color == CARD && *w == CARD_W && *h == CARD_H && (*y - card_top(PILL_CY)).abs() < 1e-3)));
        assert!(items
            .iter()
            .any(|d| matches!(d, Drawable::Polyline { width, .. } if *width == 1.8)));
        assert_eq!(scene.fade, 1.0);
    }

    #[test]
    fn icon_sits_in_the_ring_gap_clear_of_the_caps() {
        assert_eq!((RING_START_DEG, RING_SWEEP_DEG), (16.0, 328.0));
        // the ring ends 16° either side of 12 o'clock; its caps are RING_W/2 round
        let half_gap = (360.0 - RING_SWEEP_DEG) / 2.0;
        let a = half_gap.to_radians();
        let cap = (CX + RING_R * a.sin(), CY - RING_R * a.cos());
        let dx = (cap.0 - CX).abs() - ICON_PX / 2.0;
        assert!(dx > RING_W / 2.0, "icon box edge {dx} px from a cap centre");
        assert_eq!((ICON_CY, ICON_PX), (CY - RING_R, 22.0));
    }

    #[test]
    fn a_screen_without_an_icon_draws_none() {
        let mut ui = ScreenUi::new();
        let scene = compose(&P2Screen::new("ups", "—"), &mut ui, 0.0);
        assert!(!scene
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { .. })));
    }

    #[test]
    fn ring_fill_glides_and_unit_below_moves_everything_down() {
        let mut ui = ScreenUi::new();
        let mut s = sample();
        compose(&s, &mut ui, 0.0);
        s.ring = RingSpec::fill(0.9, BLUE);
        compose(&s, &mut ui, 0.3);
        let mid = compose(&s, &mut ui, 0.6);
        let sweep = mid
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Arc {
                    sweep_deg, color, ..
                } if *color == BLUE => Some(*sweep_deg),
                _ => None,
            })
            .next()
            .unwrap();
        assert!(
            sweep > RING_SWEEP_DEG * 0.42 && sweep < RING_SWEEP_DEG * 0.9,
            "gliding: {sweep}"
        );
        let mut below = sample();
        below.unit = Some("Mb/s".into());
        below.unit_below = true;
        below.live = true;
        let scene = compose(&below, &mut ui, 1.0);
        assert!(scene.items.iter().any(|d| matches!(d, Drawable::Run { text, baseline, .. } if text == "Mb/s" && (*baseline - (HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY)).abs() < 1e-3)));
        assert!(scene.items.iter().any(|d| matches!(d, Drawable::RoundRect { y, color, .. } if *color == CARD && (*y - card_top(PILL_CY_UNIT_BELOW)).abs() < 1e-3)));
        assert!(scene.items.iter().any(
            |d| matches!(d, Drawable::Ellipse { cy, color, .. } if *color == OK && *cy == LIVE_CY)
        ));
        let empty = P2Screen::new("ups", "—");
        let scene = compose(&empty, &mut ui, 1.0);
        assert_eq!(
            scene
                .items
                .iter()
                .filter(|d| matches!(d, Drawable::Arc { .. }))
                .count(),
            1,
            "track only"
        );
        assert!(
            !scene
                .items
                .iter()
                .any(|d| matches!(d, Drawable::RoundRect { .. })),
            "no pill, no card"
        );
    }
}

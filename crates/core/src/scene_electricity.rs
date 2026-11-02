//! Electricity roles: power mix, price, carbon intensity, renewable / carbon-free.

use crate::anim::{breathe, Secs};
use crate::electricity::{price_level, PriceLevel};
use crate::model::Model;
use crate::scene::{
    badge, badge_w, icon_at, ring, ring_states, seg_count, Drawable, Scene, SegState,
};
use crate::theme::layout::*;
use crate::theme::{carbon_color, Color, BLUE, GREEN, GREY, WHITE};

pub const GAUGE_SEGS: usize = 45;
pub const GAUGE_START_DEG: f32 = 225.0;
pub const GAUGE_PITCH_DEG: f32 = 6.0;
const SEGS_PER_BAND: usize = GAUGE_SEGS / 5;
/// Angle the needle sweeps from `t = 0` (segment 0) to `t = 1` (the last segment).
const GAUGE_SWEEP_DEG: f32 = (GAUGE_SEGS - 1) as f32 * GAUGE_PITCH_DEG;
const NEEDLE_R0: f32 = 88.0;
const NEEDLE_R1: f32 = 116.0;
const NEEDLE_W: f32 = 4.0;
const DIM_BAND_ALPHA: f32 = 0.45;
const PRICE_TEXT_CY: f32 = 100.0;
const PRICE_TEXT_PX: f32 = 40.0;
const PRICE_CAPTION_CY: f32 = 130.0;
const PRICE_CAPTION_PX: f32 = 11.0;
const LEVEL_BADGE_W: f32 = 84.0;
const LEVEL_BADGE_PX: f32 = 13.0;

/// Angle of the needle for position `t`.
pub fn needle_angle(t: f32) -> f32 {
    GAUGE_START_DEG + GAUGE_SWEEP_DEG * t.clamp(0.0, 1.0)
}

/// The segment the needle points at. A gap segment belongs to the band
/// before it and is never lit, so a needle over a gap breathes the segment
/// just before it instead.
fn needle_segment(t: f32) -> usize {
    let seg = ((t * (GAUGE_SEGS - 1) as f32).round() as usize).min(GAUGE_SEGS - 1);
    if seg / SEGS_PER_BAND < 4 && seg % SEGS_PER_BAND == SEGS_PER_BAND - 1 {
        seg - 1
    } else {
        seg
    }
}

pub fn price_scene(model: &Model, now: Secs) -> Scene {
    let p = model.prices();
    let cur = model.current_price();
    let level = cur.map(|v| price_level(v, p.avg).0);
    let t = model.smooth_price_needle(now);
    let needle_seg = needle_segment(t);
    let mut states = Vec::with_capacity(GAUGE_SEGS);
    for i in 0..GAUGE_SEGS {
        let band = i / SEGS_PER_BAND;
        let gap = band < 4 && i % SEGS_PER_BAND == SEGS_PER_BAND - 1;
        if gap {
            states.push(SegState::Off);
            continue;
        }
        let color = PriceLevel::ALL[band].color();
        let alpha = match level {
            Some(lvl) if lvl.index() == band && i == needle_seg => breathe(now, 2.4),
            Some(lvl) if lvl.index() == band => 1.0,
            _ => DIM_BAND_ALPHA,
        };
        states.push(SegState::On(color, alpha));
    }
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: GAUGE_SEGS,
        states,
        pitch_deg: GAUGE_PITCH_DEG,
        start_deg: GAUGE_START_DEG,
    });
    if level.is_some() {
        s.push(Drawable::Tick {
            cx: CX,
            cy: CY,
            angle_deg: needle_angle(t),
            r0: NEEDLE_R0,
            r1: NEEDLE_R1,
            width: NEEDLE_W,
            color: WHITE,
            alpha: 1.0,
        });
    }
    let (text, color) = match cur {
        Some(v) if v < 0.0 => (format!("{v:.3}"), BLUE),
        Some(v) => (format!("{v:.3}"), WHITE),
        None => ("--".to_string(), GREY),
    };
    s.push(Drawable::Text {
        cx: CX,
        cy: PRICE_TEXT_CY,
        text,
        px: PRICE_TEXT_PX,
        color,
        alpha: 1.0,
    });
    s.push(Drawable::Text {
        cx: CX,
        cy: PRICE_CAPTION_CY,
        text: "€/kWh".to_string(),
        px: PRICE_CAPTION_PX,
        color: GREY,
        alpha: 1.0,
    });
    let (word, stroke) = match level {
        Some(lvl) => (lvl.word().to_string(), lvl.color()),
        None => ("no data".to_string(), GREY),
    };
    let mut b = badge_w(BADGE_CY, stroke, word, LEVEL_BADGE_W);
    if let Drawable::Badge { text_px, .. } = &mut b {
        *text_px = LEVEL_BADGE_PX;
    }
    s.push(b);
    s
}

pub fn carbon_scene(model: &Model, now: Secs) -> Scene {
    let g = model.smooth_carbon(now);
    let color = carbon_color(g);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states((g / 800.0 * 100.0).clamp(0.0, 100.0), color, SEG_N, now),
    ));
    s.push(icon_at("cloud", ICON_CY, ICON_SIZE, color, 1.0));
    s.push(badge(BADGE_CY, color, format!("{g:.0} g")));
    s
}

pub fn renewable_scene(model: &Model, now: Secs) -> Scene {
    let ren = model.smooth_renewable(now);
    let ff = model.smooth_fossil_free(now);
    let inner_color = Color::hex(0x69D6F8);
    let mut s = Scene::new();
    s.push(ring(RING_R, ring_states(ren, GREEN, SEG_N, now)));
    s.push(ring(
        84.0,
        ring_states(ff, inner_color, seg_count(84.0), now),
    ));
    s.push(icon_at("leaf", 92.0, 60.0, WHITE, 1.0));
    let odd = ((now / 5.0).floor() as i64).rem_euclid(2) == 1;
    let (text, stroke, dot) = if odd {
        (format!("{ff:.0}%"), inner_color, inner_color)
    } else {
        (format!("{ren:.0}%"), GREEN, GREEN)
    };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s.push(Drawable::Dots {
        cx: CX - 26.0,
        cy: BADGE_CY,
        spacing: 0.0,
        r: 3.0,
        colors: vec![dot],
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electricity::Source;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn mix_model(list: &[(Source, f32)]) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: list.to_vec(),
                renewable_pct: 61.0,
                fossil_free_pct: 73.0,
                carbon_gco2: 214.0,
                updated_at: "2026-09-07T12:00:00Z".into(),
            },
            0.0,
        );
        m
    }

    /// A model at slot 56 (14:00) with a flat day at 0.15 €/kWh except slot
    /// 56, and a three-day average of 0.20.
    fn price_model(at_now: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let mut eur = vec![0.15f32; 96];
        eur[56] = at_now;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.20,
                currency: "EUR".into(),
            },
            0.0,
        );
        m
    }

    fn gauge_states(s: &Scene) -> Vec<SegState> {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(states.clone()),
                _ => None,
            })
            .expect("ring")
    }

    fn needle(s: &Scene) -> Option<f32> {
        s.items.iter().find_map(|d| match d {
            Drawable::Tick { angle_deg, .. } => Some(*angle_deg),
            _ => None,
        })
    }

    fn texts(s: &Scene) -> Vec<(String, Color)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Text { text, color, .. } => Some((text.clone(), *color)),
                _ => None,
            })
            .collect()
    }

    fn badge_text(s: &Scene) -> String {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("badge")
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

    #[test]
    fn price_gauge_has_five_bands_with_gaps_and_lights_the_needle_band() {
        // 0.221 / 0.20 = 1.105: NORMAL, band 2
        let m = price_model(0.221);
        let s = price_scene(&m, 5.0);
        let states = gauge_states(&s);
        assert_eq!(states.len(), GAUGE_SEGS);
        for band in 0..4 {
            assert_eq!(states[band * 9 + 8], SegState::Off, "gap after band {band}");
        }
        assert!(
            matches!(states[44], SegState::On(..)),
            "the last band has no gap"
        );
        assert!(matches!(states[0], SegState::On(c, a) if c == GREEN && (a - 0.45).abs() < 1e-6));
        assert!(matches!(states[18], SegState::On(c, a) if c == crate::theme::AMBER && a == 1.0));
        assert!(
            matches!(states[27], SegState::On(c, a) if c == PriceLevel::Pricey.color() && (a - 0.45).abs() < 1e-6)
        );
        assert!(
            matches!(states[44], SegState::On(c, a) if c == crate::theme::RED && (a - 0.45).abs() < 1e-6)
        );
        // t = 0.4 + (0.205 / 0.25) / 5 = 0.564; needle segment round(0.564 * 44) = 25 breathes
        assert!(
            matches!(states[25], SegState::On(_, a) if a < 1.0),
            "needle segment breathes"
        );
        let angle = needle(&s).expect("needle");
        assert!((angle - (225.0 + 264.0 * 0.564)).abs() < 0.5, "{angle}");
        assert_eq!(badge_text(&s), "NORMAL");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Badge { stroke, w, .. } if *stroke == PriceLevel::Normal.color() && *w == 84.0)));
        let t = texts(&s);
        assert_eq!(t[0], ("0.221".to_string(), WHITE));
        assert_eq!(t[1].0, "€/kWh");
        assert!(icons(&s).is_empty(), "no euro icon any more");
    }

    #[test]
    fn price_gauge_words_follow_the_bands() {
        // needle segments for these prices are 6, 13, 21, 30 and 38: never the
        // first segment of a band, so that one is lit at exactly 1.0
        for (price, word, band) in [
            (0.10, "V.CHEAP", 0usize),
            (0.15, "CHEAP", 1),
            (0.20, "NORMAL", 2),
            (0.25, "PRICEY", 3),
            (0.30, "V.PRICEY", 4),
        ] {
            let s = price_scene(&price_model(price), 5.0);
            assert_eq!(badge_text(&s), word);
            let states = gauge_states(&s);
            assert!(
                matches!(states[band * 9], SegState::On(c, a) if c == PriceLevel::ALL[band].color() && a == 1.0),
                "band {band} lit for {price}: {:?}",
                states[band * 9]
            );
            let other = if band == 0 { 9 } else { 0 };
            assert!(
                matches!(states[other], SegState::On(_, a) if (a - 0.45).abs() < 1e-6),
                "other bands dim for {price}"
            );
        }
    }

    #[test]
    fn needle_over_a_gap_breathes_the_segment_before_it() {
        // 0.115 / 0.20 = 0.575: band 0, t = (0.275 / 0.3) / 5 = 0.1833, round(0.1833 * 44) = 8, the gap
        let s = price_scene(&price_model(0.115), 5.0);
        let states = gauge_states(&s);
        assert_eq!(states[8], SegState::Off, "the gap stays a gap");
        assert!(
            matches!(states[7], SegState::On(_, a) if a < 1.0),
            "segment 7 breathes instead"
        );
        assert!(matches!(states[6], SegState::On(_, a) if a == 1.0));
    }

    #[test]
    fn negative_price_is_blue_and_very_cheap() {
        let s = price_scene(&price_model(-0.006), 5.0);
        assert_eq!(texts(&s)[0], ("-0.006".to_string(), BLUE));
        assert_eq!(badge_text(&s), "V.CHEAP");
        assert!(
            (needle(&s).unwrap() - 225.0).abs() < 0.5,
            "pinned at the cheap end"
        );
    }

    #[test]
    fn price_gauge_without_data_hides_the_needle() {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let s = price_scene(&m, 5.0);
        assert!(needle(&s).is_none());
        assert_eq!(texts(&s)[0], ("--".to_string(), GREY));
        assert_eq!(badge_text(&s), "no data");
        assert!(gauge_states(&s).iter().all(|st| {
            matches!(st, SegState::Off)
                || matches!(st, SegState::On(_, a) if (*a - 0.45).abs() < 1e-6)
        }));
        // a day with a hole at the current slot reads the same way
        let mut eur = vec![0.15f32; 96];
        eur[56] = f32::NAN;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.20,
                currency: "EUR".into(),
            },
            0.0,
        );
        let s = price_scene(&m, 5.0);
        assert!(needle(&s).is_none());
        assert_eq!(badge_text(&s), "no data");
    }

    #[test]
    fn carbon_scene_colours_ring_icon_and_badge() {
        let m = mix_model(&[(Source::Gas, 100.0)]);
        let s = carbon_scene(&m, 5.0);
        assert_eq!(badge_text(&s), "214 g");
        assert_eq!(icons(&s), vec!["cloud"]);
        // 214 of 800 g
        assert_eq!(s.lit_count(), 16);
        let want = carbon_color(214.0);
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "cloud", color, .. } if *color == want)));
    }

    #[test]
    fn renewable_shows_two_rings_and_alternates_the_badge() {
        let m = mix_model(&[(Source::Wind, 61.0), (Source::Gas, 39.0)]);
        let s = renewable_scene(&m, 2.5);
        let radii: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii, vec![RING_R, 84.0]);
        assert_eq!(badge_text(&s), "61%");
        assert_eq!(badge_text(&renewable_scene(&m, 7.5)), "73%");
        assert_eq!(icons(&s), vec!["leaf"]);
        let dots = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Dots { colors, .. } => Some(colors.clone()),
                _ => None,
            })
            .expect("dots");
        assert_eq!(dots, vec![GREEN]);
        assert_eq!(s.lit_count(), 37, "61% of 60 segments");
    }
}

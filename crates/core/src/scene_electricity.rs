//! Electricity roles: power mix, price, carbon intensity, renewable / carbon-free.

use crate::anim::{breathe, Secs};
use crate::electricity::{partition, MIX_ICON_MIN_SEGS};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{carbon_color, price_color, Color, BLUE, GREEN, GREY, OFF, WHITE};

const MIX_ICON_R: f32 = 60.0;
const MIX_ICON_SIZE: f32 = 26.0;
const MIX_TICK_R0: f32 = 80.0;
const MIX_TICK_R1: f32 = 88.0;

pub fn power_mix_scene(model: &Model, now: Secs) -> Scene {
    let shares = model.smooth_shares(now);
    let sections = partition(&shares, SEG_N);
    // Non-zero only while a new leader is being turned up to 12 o'clock; the
    // ring, its ticks and its icons all move together.
    let start_deg = model.smooth_mix_start(now);
    let mut states = vec![SegState::Off; SEG_N];
    for (k, sec) in sections.iter().enumerate() {
        let end = (sec.start + sec.len).min(SEG_N);
        for (i, st) in states.iter_mut().enumerate().take(end).skip(sec.start) {
            let last_of_leader = k == 0 && i + 1 == sec.start + sec.len;
            let alpha = if last_of_leader {
                breathe(now, 2.4)
            } else {
                1.0
            };
            *st = SegState::On(sec.source.color(), alpha);
        }
    }
    let mut s = Scene::new();
    let mut r = ring(RING_R, states);
    if let Drawable::Ring { start_deg: sd, .. } = &mut r {
        *sd = start_deg;
    }
    s.push(r);
    for sec in &sections {
        if sec.len < MIX_ICON_MIN_SEGS {
            continue;
        }
        let angle =
            (sec.start as f32 + (sec.len as f32 - 1.0) / 2.0) * (360.0 / SEG_N as f32) + start_deg;
        let a = angle.to_radians();
        let (sin, cos) = (a.sin(), a.cos());
        s.push(Drawable::Tick {
            cx: CX,
            cy: CY,
            angle_deg: angle,
            r0: MIX_TICK_R0,
            r1: MIX_TICK_R1,
            width: 3.0,
            color: OFF,
            alpha: 1.0,
        });
        s.push(Drawable::Icon {
            name: sec.source.icon(),
            cx: CX + MIX_ICON_R * sin,
            cy: CY - MIX_ICON_R * cos,
            size: MIX_ICON_SIZE,
            color: sec.source.color(),
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        });
    }
    s
}

/// Two segments per hour of the day.
pub const PRICE_SEGS: usize = 48;

fn price_badge_text(ct: f32) -> String {
    if ct >= 100.0 {
        format!("{:.2} €", ct / 100.0)
    } else {
        format!("{ct:.1} ct")
    }
}

pub fn price_scene(model: &Model, now: Secs, local_hour: u32) -> Scene {
    let p = model.prices();
    let valid: Vec<f32> = p.ct.iter().copied().filter(|v| v.is_finite()).collect();
    let (min, max) = valid
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let span = (max - min).max(0.01);
    let mut states = vec![SegState::Off; PRICE_SEGS];
    for h in 0..24usize {
        let Some(v) = p.ct.get(h).copied().filter(|v| v.is_finite()) else {
            continue;
        };
        let color = if v < 0.0 {
            BLUE
        } else {
            price_color((v - min) / span)
        };
        let alpha = if (h as u32) < local_hour {
            0.35
        } else if h as u32 == local_hour {
            breathe(now, 2.4)
        } else {
            1.0
        };
        states[h * 2] = SegState::On(color, alpha);
        states[h * 2 + 1] = SegState::On(color, alpha);
    }
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: PRICE_SEGS,
        states,
        pitch_deg: 360.0 / PRICE_SEGS as f32,
        start_deg: 0.0,
    });
    s.push(icon_at("euro", ICON_CY, ICON_SIZE, WHITE, 1.0));
    let cur =
        p.ct.get(local_hour as usize)
            .copied()
            .filter(|v| v.is_finite());
    let stroke = match cur {
        Some(v) if v < 0.0 => BLUE,
        Some(v) => price_color((v - min) / span),
        None => GREY,
    };
    let window_odd = ((now / 5.0).floor() as i64).rem_euclid(2) == 1;
    let text = if window_odd && !valid.is_empty() {
        format!("min {min:.1}")
    } else {
        cur.map(price_badge_text).unwrap_or_else(|| "--".into())
    };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
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

    fn price_model(ct: Vec<f32>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                ct_per_kwh: ct,
                currency: "EUR".into(),
            },
            0.0,
        );
        m
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
    fn power_mix_draws_a_section_per_source_and_icons_for_the_big_ones() {
        let m = mix_model(&[
            (Source::Solar, 48.0),
            (Source::Wind, 24.0),
            (Source::Gas, 13.0),
            (Source::Coal, 7.0),
            (Source::Nuclear, 4.0),
            (Source::Biomass, 2.0),
            (Source::Hydro, 2.0),
        ]);
        let shares = m.smooth_shares(5.0);
        assert_eq!(shares.len(), 7);
        let sections = partition(&shares, SEG_N);
        assert_eq!(sections.len(), 7);
        let s = power_mix_scene(&m, 5.0);
        let big = sections
            .iter()
            .filter(|sec| sec.len >= MIX_ICON_MIN_SEGS)
            .count();
        let names = icons(&s);
        assert_eq!(names.len(), big, "one icon per section with room for one");
        assert!(big < 7, "the smallest sources are too thin for an icon");
        assert_eq!(names[0], "em-solar", "the leader comes first");
        let ticks: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Tick { angle_deg, .. } => Some(*angle_deg),
                _ => None,
            })
            .collect();
        assert_eq!(ticks.len(), names.len(), "a tick under every icon");
        let solar_span = sections[0].len as f32 * 6.0;
        assert!(
            ticks[0] > 0.0 && ticks[0] < solar_span,
            "first tick sits inside the solar section, got {}",
            ticks[0]
        );
        // no centre icon and no badge: the ring is the whole story
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Badge { .. })));
        assert!(!names.contains(&"zap"));
        let lit = s.lit_count();
        assert_eq!(lit + 7, SEG_N, "one dark gap per section");
    }

    fn ring_start(s: &Scene) -> f32 {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { start_deg, .. } => Some(*start_deg),
                _ => None,
            })
            .expect("ring")
    }

    #[test]
    fn a_new_leader_turns_the_ring_up_to_twelve_oclock() {
        let mut m = mix_model(&[(Source::Solar, 70.0), (Source::Wind, 30.0)]);
        // let the first mix settle: solar leads from 12 o'clock, ring at rest
        let mut t = 0.0;
        while t < 1.0 {
            m.tick(t);
            t += 1.0 / 30.0;
        }
        assert_eq!(ring_start(&power_mix_scene(&m, 1.0)), 0.0, "settled ring");
        assert_eq!(icons(&power_mix_scene(&m, 1.0))[0], "em-solar");
        // wind takes the lead: `partition` moves it to segment 0 at once, so the
        // ring turns back by where wind was and eases the turn out
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: vec![(Source::Wind, 70.0), (Source::Solar, 30.0)],
                renewable_pct: 61.0,
                fossil_free_pct: 73.0,
                carbon_gco2: 214.0,
                updated_at: "2026-09-07T12:00:01Z".into(),
            },
            1.0,
        );
        let mut swapped: Option<(Secs, f32)> = None;
        while t < 2.0 {
            m.tick(t);
            let sd = ring_start(&power_mix_scene(&m, t));
            if sd != 0.0 && swapped.is_none() {
                swapped = Some((t, sd));
            }
            t += 1.0 / 30.0;
        }
        let (t0, sd) = swapped.expect("the ring turns on a leader change");
        assert!(sd.abs() > 30.0, "turned by {sd} degrees at {t0}");
        let icons_at_swap = icons(&power_mix_scene(&m, t0));
        assert_eq!(icons_at_swap[0], "em-wind", "wind leads the sections now");
        // still turning half way through the smooth, back at rest after it
        let mid = ring_start(&power_mix_scene(&m, t0 + 0.4));
        assert!(mid != 0.0 && mid.abs() < sd.abs(), "eases out, got {mid}");
        assert_eq!(ring_start(&power_mix_scene(&m, t0 + 1.0)), 0.0, "settled");
    }

    #[test]
    fn power_mix_without_data_is_an_empty_ring() {
        let m = Model::new(Thresholds::default());
        let s = power_mix_scene(&m, 5.0);
        assert_eq!(s.lit_count(), 0);
        assert!(icons(&s).is_empty());
    }

    fn day() -> Vec<f32> {
        let mut ct = vec![0.0f32; 24];
        for (h, v) in ct.iter_mut().enumerate() {
            *v = 6.2 + h as f32;
        }
        ct[14] = 22.1;
        ct
    }

    #[test]
    fn price_ring_has_two_segments_per_hour_and_dims_the_past() {
        let m = price_model(day());
        let s = price_scene(&m, 2.5, 14);
        let (n, states) = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { n, states, .. } => Some((*n, states.clone())),
                _ => None,
            })
            .expect("ring");
        assert_eq!(n, PRICE_SEGS);
        assert!(matches!(states[0], SegState::On(_, a) if (a - 0.35).abs() < 1e-6));
        assert!(matches!(states[27], SegState::On(_, a) if (a - 0.35).abs() < 1e-6));
        assert!(
            matches!(states[28], SegState::On(_, a) if a < 1.0),
            "now breathes"
        );
        assert!(matches!(states[29], SegState::On(_, a) if a < 1.0));
        assert!(matches!(states[30], SegState::On(_, a) if a == 1.0));
        // hour 0 is the cheapest, hour 23 the dearest
        assert!(matches!(states[0], SegState::On(c, _) if c == GREEN));
        assert!(matches!(states[47], SegState::On(c, _) if c == crate::theme::RED));
        assert_eq!(icons(&s), vec!["euro"]);
        assert_eq!(badge_text(&s), "22.1 ct");
    }

    #[test]
    fn price_badge_alternates_with_the_daily_minimum() {
        let m = price_model(day());
        assert_eq!(badge_text(&price_scene(&m, 7.5, 14)), "min 6.2");
        assert_eq!(badge_text(&price_scene(&m, 12.5, 14)), "22.1 ct");
        let mut ct = day();
        ct[14] = 102.0;
        assert_eq!(
            badge_text(&price_scene(&price_model(ct), 2.5, 14)),
            "1.02 €"
        );
        let empty = price_model(vec![]);
        assert_eq!(badge_text(&price_scene(&empty, 2.5, 14)), "--");
        assert_eq!(badge_text(&price_scene(&empty, 7.5, 14)), "--");
    }

    #[test]
    fn missing_hours_are_dark_and_the_badge_says_nothing() {
        // a partial day: ENTSO-E published 0..12, the rest is still missing
        let mut ct = day();
        for v in ct.iter_mut().skip(12) {
            *v = f32::NAN;
        }
        let m = price_model(ct);
        let s = price_scene(&m, 2.5, 14);
        let states = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(states.clone()),
                _ => None,
            })
            .expect("ring");
        assert!(matches!(states[22], SegState::On(..)), "hour 11 is priced");
        assert!(
            states[24..48].iter().all(|st| *st == SegState::Off),
            "the missing hours stay dark"
        );
        assert_eq!(s.lit_count(), 24, "two segments for each of the 12 hours");
        // the current hour is one of the missing ones
        assert_eq!(badge_text(&s), "--");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Badge { stroke, .. } if *stroke == GREY)));
        // the min window still works off the hours that did arrive
        assert_eq!(badge_text(&price_scene(&m, 7.5, 14)), "min 6.2");
        // and a priced current hour is unaffected
        assert_eq!(badge_text(&price_scene(&m, 2.5, 3)), "9.2 ct");
    }

    #[test]
    fn negative_prices_are_blue() {
        let mut ct = day();
        ct[3] = -1.5;
        let m = price_model(ct);
        let s = price_scene(&m, 2.5, 3);
        let states = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(states.clone()),
                _ => None,
            })
            .expect("ring");
        assert!(matches!(states[6], SegState::On(c, _) if c == BLUE));
        assert_eq!(badge_text(&s), "-1.5 ct");
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

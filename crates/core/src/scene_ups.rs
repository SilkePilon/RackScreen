//! UPS role: battery charge outside, load inside, runtime in the badge.

use crate::anim::{breathe, Secs};
use crate::format::fmt_runtime;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREEN, RED, WHITE};

/// Charge ring colour: green above 50 %, amber above 20 %, red at or below or on low battery.
pub fn charge_color(charge_pct: f32, low_battery: bool) -> Color {
    if low_battery || charge_pct <= 20.0 {
        RED
    } else if charge_pct <= 50.0 {
        AMBER
    } else {
        GREEN
    }
}

pub fn ups_scene(model: &Model, now: Secs) -> Scene {
    let u = model.ups();
    let charge = model.smooth_ups_charge(now);
    let load = model.smooth_ups_load(now);
    let color = charge_color(charge, u.low_battery);
    let mut states = ring_states(charge, color, SEG_N, now);
    if u.low_battery {
        let a = breathe(now, 1.2);
        for st in states.iter_mut() {
            if let SegState::On(c, _) = *st {
                *st = SegState::On(c, a);
            }
        }
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(ring(84.0, ring_states(load, AMBER, seg_count(84.0), now)));
    let (icon, icon_color, alpha) = if u.on_battery {
        ("battery-warning", RED, breathe(now, 1.6))
    } else {
        ("battery-charging", WHITE, 1.0)
    };
    s.push(icon_at(icon, 92.0, 60.0, icon_color, alpha));
    let stroke = if u.on_battery { RED } else { GREEN };
    s.push(badge(BADGE_CY, stroke, fmt_runtime(u.runtime_secs)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    fn model(on_battery: bool, low: bool, charge: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Ups {
                on_battery,
                low_battery: low,
                charge_pct: charge,
                load_pct: 6.0,
                runtime_secs: 42 * 60,
            },
            0.0,
        );
        m
    }

    fn badge_text(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .expect("badge")
    }

    fn icon(s: &Scene) -> &'static str {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .expect("icon")
    }

    #[test]
    fn charge_colour_thresholds() {
        assert_eq!(charge_color(100.0, false), GREEN);
        assert_eq!(charge_color(50.0, false), AMBER);
        assert_eq!(charge_color(20.0, false), RED);
        assert_eq!(charge_color(90.0, true), RED);
    }

    #[test]
    fn online_is_a_full_green_ring_with_load_inside() {
        let s = ups_scene(&model(false, false, 100.0), 5.0);
        assert_eq!(s.lit_count(), 60);
        let radii: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii, vec![RING_R, 84.0]);
        assert_eq!(icon(&s), "battery-charging");
        assert_eq!(badge_text(&s), ("42 min".into(), GREEN));
    }

    #[test]
    fn on_battery_swaps_icon_and_reddens_the_badge() {
        let s = ups_scene(&model(true, false, 80.0), 5.0);
        assert_eq!(icon(&s), "battery-warning");
        assert_eq!(badge_text(&s).1, RED);
        let low = ups_scene(&model(true, true, 15.0), 0.3);
        // low battery: every lit segment breathes together
        let alphas: Vec<f32> = low
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(
                    states
                        .iter()
                        .filter_map(|s| match s {
                            SegState::On(_, a) => Some(*a),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap();
        assert!(alphas
            .iter()
            .all(|a| (a - alphas[0]).abs() < 1e-6 && *a < 1.0));
    }
}

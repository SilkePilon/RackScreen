//! Thermal role: hottest node on the ring, temperature-coloured node dots in a wide badge.

use crate::anim::Secs;
use crate::model::Model;
use crate::scene::{badge_text_alternate, icon_at, node_dots_badge, ring, ring_states, Scene};
use crate::theme::layout::*;
use crate::theme::{temp_color, WHITE};

pub fn thermal_scene(model: &Model, now: Secs) -> Scene {
    let st = model.state();
    let hot = model.smooth_hot_temp(now);
    let color = temp_color(hot);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states(hot.clamp(0.0, 100.0), color, SEG_N, now),
    ));
    s.push(icon_at("thermometer", ICON_CY, ICON_SIZE, WHITE, 1.0));
    let colors: Vec<_> = st.temps.iter().map(|(_, c)| temp_color(*c)).collect();
    let hottest = st
        .temps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
        .map(|(i, _)| i);
    let avg = if st.temps.is_empty() {
        0.0
    } else {
        st.temps.iter().map(|(_, c)| c).sum::<f32>() / st.temps.len() as f32
    };
    for d in node_dots_badge(BADGE_CY, color, colors, hottest, now) {
        s.push(d);
    }
    // the badge holds dots; the temperature text goes in a second small badge below
    s.push(badge_text_alternate(
        MARKER_CY + 4.0,
        color,
        format!("{hot:.0}°"),
        format!("{avg:.0}°"),
        now,
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;
    use crate::theme::{AMBER, RED};

    fn model(list: &[(&str, f32)]) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeTemps(list.iter().map(|(n, c)| (n.to_string(), *c)).collect()),
            0.0,
        );
        m
    }

    fn badges(s: &Scene) -> Vec<(String, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Badge { text, w, .. } => Some((text.clone(), *w)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hottest_node_drives_ring_and_text() {
        let m = model(&[("n1", 41.0), ("n2", 66.0), ("n3", 47.0), ("n4", 39.0)]);
        let s = thermal_scene(&m, 2.5);
        // 66 % of 60 segments
        assert_eq!(s.lit_count(), 40);
        assert!(s.items.iter().any(|d| matches!(
            d,
            Drawable::Icon {
                name: "thermometer",
                ..
            }
        )));
        let b = badges(&s);
        assert_eq!(b.len(), 2, "dot badge plus the temperature badge");
        assert_eq!(b[1].0, "66°", "hottest in the even window");
        assert_eq!(b[1].1, 48.0);
        let s2 = thermal_scene(&m, 5.5);
        assert_eq!(badges(&s2)[1].0, "48°", "average in the odd window");
    }

    #[test]
    fn dots_are_temperature_coloured_and_the_hottest_breathes() {
        let m = model(&[("n1", 20.0), ("n2", 80.0)]);
        let s = thermal_scene(&m, 5.0);
        let dots = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Dots { colors, .. } => Some(colors.clone()),
                _ => None,
            })
            .expect("dots");
        assert_eq!(dots.len(), 2);
        assert_eq!((dots[0].r, dots[0].g, dots[0].b), (0x4f, 0x8d, 0xff));
        assert_eq!((dots[1].r, dots[1].g, dots[1].b), (RED.r, RED.g, RED.b));
        assert!(dots[1].a < 255, "the hottest dot breathes");
    }

    #[test]
    fn no_temps_is_an_empty_cold_scene() {
        let m = Model::new(Thresholds::default());
        let s = thermal_scene(&m, 2.5);
        assert_eq!(s.lit_count(), 0);
        assert_eq!(badges(&s)[1].0, "0°");
        assert_ne!(temp_color(0.0), AMBER);
    }
}

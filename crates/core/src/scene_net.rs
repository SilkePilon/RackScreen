//! Network role: download outside, upload inside, a pulse crawling with the flow.

use crate::anim::Secs;
use crate::format::fmt_mbit;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, BLUE, VIOLET, WHITE};

/// Lit segments for `fill`, with a three-segment bright pulse at `phase` laps
/// travelling clockwise (or the other way), the rest of the lit part dimmed.
pub fn flow_states(
    fill: f32,
    phase: f32,
    n: usize,
    color: Color,
    clockwise: bool,
) -> Vec<SegState> {
    let lit = ((fill.clamp(0.0, 1.0) * n as f32).round() as usize).clamp(1, n);
    let head = ((phase.rem_euclid(1.0) * lit as f32).floor() as usize).min(lit - 1);
    (0..n)
        .map(|i| {
            if i >= lit {
                return SegState::Off;
            }
            let pos = if clockwise { i } else { lit - 1 - i };
            let behind = (pos + lit - head) % lit;
            let alpha = match behind {
                0 => 1.0,
                1 => 0.85,
                2 => 0.7,
                _ => 0.45,
            };
            SegState::On(color, alpha)
        })
        .collect()
}

pub fn net_scene(model: &Model, now: Secs) -> Scene {
    let n = model.net();
    let (rx, tx) = (model.smooth_net_rx(now), model.smooth_net_tx(now));
    let (prx, ptx) = model.net_phases();
    let mut s = Scene::new();
    s.push(ring(RING_R, flow_states(rx, prx, SEG_N, BLUE, true)));
    s.push(ring(
        84.0,
        flow_states(tx, ptx, seg_count(84.0), VIOLET, false),
    ));
    s.push(icon_at("arrow-down-up", 92.0, 60.0, WHITE, 1.0));
    s.push(badge(BADGE_CY, BLUE, fmt_mbit(n.rx_bps + n.tx_bps)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    #[test]
    fn pulse_sits_at_the_head_and_trails_behind() {
        let v = flow_states(0.5, 0.0, 60, BLUE, true);
        assert_eq!(
            v.iter().filter(|s| matches!(s, SegState::On(..))).count(),
            30
        );
        assert!(matches!(v[0], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[1], SegState::On(_, a) if a == 0.85));
        assert!(matches!(v[2], SegState::On(_, a) if a == 0.7));
        assert!(matches!(v[3], SegState::On(_, a) if a == 0.45));
        assert!(matches!(v[29], SegState::On(_, a) if a == 0.45));
        assert_eq!(v[30], SegState::Off);
        // a third of a lap in: head at segment 10
        let v = flow_states(0.5, 1.0 / 3.0, 60, BLUE, true);
        assert!(matches!(v[10], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[9], SegState::On(_, a) if a == 0.45));
        // counter-clockwise: the head starts at the last lit segment
        let v = flow_states(0.5, 0.0, 60, VIOLET, false);
        assert!(matches!(v[29], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[28], SegState::On(_, a) if a == 0.85));
        // the floor still lights something
        assert_eq!(
            flow_states(0.0, 0.0, 60, BLUE, true)
                .iter()
                .filter(|s| matches!(s, SegState::On(..)))
                .count(),
            1
        );
    }

    #[test]
    fn scene_has_two_rings_and_a_total_badge() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Network {
                rx_bps: 40e6,
                tx_bps: 1.3e6,
            },
            0.0,
        );
        let s = net_scene(&m, 5.0);
        let radii: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii, vec![RING_R, 84.0]);
        let text = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "41 Mb");
        assert!(s.items.iter().any(|d| matches!(
            d,
            Drawable::Icon {
                name: "arrow-down-up",
                ..
            }
        )));
    }
}

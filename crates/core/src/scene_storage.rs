//! Storage role: Longhorn volume health on the outer ring, used capacity inside.

use crate::anim::{breathe, Secs};
use crate::event::Robustness;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, BLUE, GREEN, GREY, RED, VIOLET, WHITE};

fn robustness_color(r: Robustness) -> Color {
    match r {
        Robustness::Healthy => GREEN,
        Robustness::Degraded => AMBER,
        Robustness::Faulted => RED,
        Robustness::Unknown => GREY,
    }
}

/// Ring states for the volumes: equal sections with an unlit gap when there is room.
pub fn volume_states(volumes: &[(String, Robustness)], now: Secs) -> Vec<SegState> {
    let n = SEG_N;
    if volumes.is_empty() {
        return vec![SegState::Off; n];
    }
    if volumes.iter().all(|(_, r)| *r == Robustness::Healthy) {
        return ring_states(100.0, VIOLET, n, now);
    }
    let count = volumes.len();
    let per = (n / count).max(1); // segments per volume
    let gap = if per >= 3 { 1 } else { 0 };
    let mut out = vec![SegState::Off; n];
    for (vi, (_, r)) in volumes.iter().enumerate().take(n) {
        let start = vi * per;
        let end = if vi == count - 1 {
            n
        } else {
            (start + per).min(n)
        };
        let lit_end = end.saturating_sub(gap);
        for st in out.iter_mut().take(lit_end).skip(start) {
            let a = if *r == Robustness::Healthy {
                1.0
            } else {
                breathe(now, 1.2)
            };
            *st = SegState::On(robustness_color(*r), a);
        }
    }
    out
}

pub fn storage_scene(model: &Model, now: Secs) -> Scene {
    let st = model.state();
    let healthy = st
        .volumes
        .iter()
        .filter(|(_, r)| *r == Robustness::Healthy)
        .count();
    let used_pct = model.smooth_storage_pct(now);
    let all_ok = healthy == st.volumes.len();
    let mut s = Scene::new();
    s.push(ring(RING_R, volume_states(&st.volumes, now)));
    s.push(ring(
        80.0,
        ring_states(used_pct, BLUE, crate::scene::seg_count(80.0), now),
    ));
    s.push(icon_at("database", 92.0, 60.0, WHITE, 1.0));
    let window = ((now / 4.0).floor() as i64).rem_euclid(3);
    let gb = st.storage_used as f32 / 1_073_741_824.0;
    let (text, stroke) = match window {
        0 => (
            format!("{healthy}/{}", st.volumes.len()),
            if all_ok { VIOLET } else { AMBER },
        ),
        1 => (
            if gb >= 1000.0 {
                format!("{:.1} TB", gb / 1024.0)
            } else {
                format!("{gb:.0} GB")
            },
            BLUE,
        ),
        _ => (format!("{used_pct:.0}%"), BLUE),
    };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 4.0).floor() * 4.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    if !all_ok {
        s.push(Drawable::Dots {
            cx: CX,
            cy: MARKER_CY,
            spacing: 0.0,
            r: 3.0,
            colors: vec![AMBER.with_alpha(breathe(now, 2.4))],
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn vols(n: usize, degraded: &[usize]) -> Vec<(String, Robustness)> {
        (0..n)
            .map(|i| {
                let r = if degraded.contains(&i) {
                    Robustness::Degraded
                } else {
                    Robustness::Healthy
                };
                (format!("v{i}"), r)
            })
            .collect()
    }

    fn lit(states: &[SegState]) -> usize {
        states
            .iter()
            .filter(|s| matches!(s, SegState::On(..)))
            .count()
    }

    #[test]
    fn all_healthy_is_a_full_violet_ring() {
        let v = volume_states(&vols(21, &[]), 0.0);
        assert_eq!(v.len(), 60);
        assert_eq!(lit(&v), 60);
        assert!(matches!(v[0], SegState::On(c, _) if c == VIOLET));
        assert_eq!(volume_states(&[], 0.0), vec![SegState::Off; 60]);
    }

    #[test]
    fn degraded_volume_gets_its_own_amber_section() {
        let v = volume_states(&vols(20, &[3]), 0.0);
        // 60 / 20 = 3 segments per volume, one of them a gap
        assert!(matches!(v[0], SegState::On(c, _) if c == GREEN));
        assert_eq!(v[2], SegState::Off, "gap after each volume");
        assert!(matches!(v[9], SegState::On(c, a) if c == AMBER && a < 1.0));
        assert!(matches!(v[10], SegState::On(c, _) if c == AMBER));
        assert_eq!(v[11], SegState::Off);
        assert!(matches!(v[12], SegState::On(c, _) if c == GREEN));
    }

    #[test]
    fn more_volumes_than_segments_does_not_panic() {
        let v = volume_states(&vols(70, &[0]), 0.0);
        assert_eq!(v.len(), 60);
        // per == 1, no gap: the first 60 volumes take one segment each
        assert!(matches!(v[0], SegState::On(c, _) if c == AMBER));
        assert!(matches!(v[59], SegState::On(c, _) if c == GREEN));
    }

    fn storage_model(volumes: Vec<(String, Robustness)>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Storage {
                volumes,
                used_bytes: 49 * (1 << 30),
                capacity_bytes: 128 * (1 << 30),
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

    #[test]
    fn badge_cycles_volumes_bytes_and_percent() {
        let m = storage_model(vols(21, &[]));
        assert_eq!(badge_text(&storage_scene(&m, 0.5)), "21/21");
        assert_eq!(badge_text(&storage_scene(&m, 4.5)), "49 GB");
        assert_eq!(badge_text(&storage_scene(&m, 8.5)), "38%");
        assert_eq!(badge_text(&storage_scene(&m, 12.5)), "21/21");
    }

    #[test]
    fn degraded_storage_shows_the_marker_dot() {
        let ok = storage_scene(&storage_model(vols(4, &[])), 0.5);
        assert!(!ok.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
        let bad = storage_scene(&storage_model(vols(4, &[1])), 0.5);
        assert_eq!(badge_text(&bad), "3/4");
        assert!(bad.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
        // two rings: volumes outside, used capacity inside
        let rings: Vec<_> = bad
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(rings, vec![RING_R, 80.0]);
    }
}

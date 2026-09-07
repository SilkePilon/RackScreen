//! GitHub activity role: today against the month's best outside, the last week inside.

use crate::anim::{breathe, Secs};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, GH_GREENS, WHITE};

/// Calendar green for a day's count relative to the best day; `None` for zero.
pub fn level_color(count: u32, best: u32) -> Option<Color> {
    if count == 0 {
        return None;
    }
    let q = count as f32 / best.max(1) as f32;
    Some(if q <= 0.25 {
        GH_GREENS[0]
    } else if q <= 0.5 {
        GH_GREENS[1]
    } else if q <= 0.75 {
        GH_GREENS[2]
    } else {
        GH_GREENS[3]
    })
}

/// Seven sections for the last seven days (oldest first from 12 o'clock), one
/// unlit gap between them, today's section breathing.
pub fn week_states(days: &[(String, u32)], best: u32, n: usize, now: Secs) -> Vec<SegState> {
    let start = days.len().saturating_sub(7);
    let week = &days[start..];
    let per = (n / 7).max(1);
    let mut out = vec![SegState::Off; n];
    for (k, (_, count)) in week.iter().enumerate() {
        let Some(color) = level_color(*count, best) else {
            continue;
        };
        let first = k * per;
        let last = (first + per).min(n).saturating_sub(1); // the gap
        let alpha = if k + 1 == week.len() {
            breathe(now, 2.4)
        } else {
            1.0
        };
        for st in out.iter_mut().take(last).skip(first) {
            *st = SegState::On(color, alpha);
        }
    }
    out
}

pub fn github_scene(model: &Model, now: Secs) -> Scene {
    let g = model.github();
    let best = g.best();
    let today = model.smooth_gh_today(now);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states(
            (today / best as f32 * 100.0).clamp(0.0, 100.0),
            GH_GREENS[3],
            SEG_N,
            now,
        ),
    ));
    s.push(ring(84.0, week_states(&g.days, best, seg_count(84.0), now)));
    s.push(icon_at("github", 92.0, 60.0, WHITE, 1.0));
    s.push(badge(BADGE_CY, GH_GREENS[3], format!("{}", g.today())));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    fn days(counts: &[u32]) -> Vec<(String, u32)> {
        counts
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("d{i}"), *c))
            .collect()
    }

    #[test]
    fn levels_are_quartiles_of_the_best_day() {
        assert_eq!(level_color(0, 40), None);
        assert_eq!(level_color(10, 40), Some(GH_GREENS[0]));
        assert_eq!(level_color(20, 40), Some(GH_GREENS[1]));
        assert_eq!(level_color(30, 40), Some(GH_GREENS[2]));
        assert_eq!(level_color(40, 40), Some(GH_GREENS[3]));
        assert_eq!(level_color(3, 0), Some(GH_GREENS[3]), "best is never zero");
    }

    #[test]
    fn week_ring_has_seven_sections_with_gaps_and_today_breathing() {
        let d = days(&[9, 9, 9, 12, 30, 0, 8, 25, 43, 28]);
        let v = week_states(&d, 43, 49, 0.6);
        // 49 / 7 = 7 per day: 6 lit + 1 gap
        assert!(matches!(v[0], SegState::On(c, _) if c == GH_GREENS[1])); // 12 of 43
        assert!(matches!(v[5], SegState::On(..)));
        assert_eq!(v[6], SegState::Off, "gap");
        assert!(matches!(v[7], SegState::On(c, _) if c == GH_GREENS[2])); // 30 of 43
        assert!(
            v[14..21].iter().all(|s| *s == SegState::Off),
            "a zero day is dark"
        );
        assert!(matches!(v[42], SegState::On(c, a) if c == GH_GREENS[2] && a < 1.0)); // today, 28 of 43
        assert!(matches!(v[35], SegState::On(c, a) if c == GH_GREENS[3] && a == 1.0)); // 43 of 43

        // fewer than seven days still works
        let v = week_states(&days(&[1, 2]), 2, 49, 0.0);
        assert!(matches!(v[0], SegState::On(..)) && matches!(v[7], SegState::On(..)));
        assert!(v[14..].iter().all(|s| *s == SegState::Off));
    }

    #[test]
    fn scene_fills_outer_ring_by_today_over_best() {
        let mut m = Model::new(Thresholds::default());
        let mut counts = vec![5u32; 30];
        counts[28] = 43;
        counts[29] = 28;
        m.apply(
            Event::GithubActivity {
                days: days(&counts),
            },
            0.0,
        );
        let s = github_scene(&m, 5.0);
        // 28 / 43 of 60 segments = 39
        assert_eq!(s.lit_count(), 39);
        let text = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "28");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "github", .. })));
    }
}

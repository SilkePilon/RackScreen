//! The GitHub contributions role.

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{GREEN, TEXT2};

/// Monday = 0 .. Sunday = 6 for a local timestamp.
pub fn weekday_index(local_secs: i64) -> usize {
    (local_secs.div_euclid(86_400) + 3).rem_euclid(7) as usize
}

/// Consecutive days with a contribution, counted back from today.
pub fn streak(days: &[(String, u32)]) -> u32 {
    days.iter().rev().take_while(|d| d.1 > 0).count() as u32
}

pub fn github(model: &Model, _now: Secs) -> P2Screen {
    let g = model.github();
    let best = g.best();
    let today = g.today();
    let mut s = P2Screen::new("CONTRIBUTIONS", today.to_string());
    s.ring = RingSpec::fill(today as f32 / best as f32, GREEN);
    s.pill = Some((
        format!("best day {best} · {} streak", streak(&g.days)),
        TEXT2,
    ));
    let wd = weekday_index(model.local_secs());
    let mut values = [0.0f32; 7];
    for k in 0..=wd {
        if let Some(d) = g.days.len().checked_sub(1 + k).and_then(|i| g.days.get(i)) {
            values[wd - k] = d.1 as f32;
        }
    }
    s.card = Some(Card::Week {
        values,
        today: wd,
        color: GREEN,
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{GREEN, TEXT2};

    #[test]
    fn weekday_and_streak() {
        assert_eq!(weekday_index(0), 3, "1970-01-01 was a Thursday");
        assert_eq!(weekday_index(4 * 86_400), 0, "Monday");
        assert_eq!(
            weekday_index(1_788_782_400 + 7200),
            0,
            "2026-09-07 is a Monday"
        );
        let days = |v: &[u32]| {
            v.iter()
                .enumerate()
                .map(|(i, c)| (format!("d{i}"), *c))
                .collect::<Vec<_>>()
        };
        assert_eq!(streak(&days(&[0, 3, 1, 2])), 3);
        assert_eq!(streak(&days(&[1, 0])), 0);
    }

    #[test]
    fn github_fills_by_best_day_and_lays_the_week_out() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400 + 2 * 86_400); // Wednesday 2026-09-09 12:00Z
        m.set_utc_offset_secs(7200);
        let days = (0..30)
            .map(|i| (format!("d{i}"), [5u32, 12, 30, 0, 8, 25, 43, 28][i % 8]))
            .collect();
        m.apply(Event::GithubActivity { days }, 0.0);
        let s = github(&m, 0.0);
        // today is index 29 -> 29 % 8 = 5 -> 25; best is 43
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("CONTRIBUTIONS", "25"));
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 25.0 / 43.0).abs() < 1e-5 && color == GREEN)
        );
        assert_eq!(s.pill, Some(("best day 43 · 2 streak".into(), TEXT2)));
        match &s.card {
            Some(Card::Week {
                values,
                today,
                color,
            }) => {
                assert_eq!((*today, *color), (2, GREEN));
                assert_eq!(values[2], 25.0, "today");
                assert_eq!(values[1], 8.0, "yesterday: index 28 -> 4 -> 8");
                assert_eq!(values[0], 0.0, "Monday: index 27 -> 3 -> 0");
                assert_eq!(&values[3..], &[0.0; 4], "the rest of the week is empty");
            }
            _ => panic!("week"),
        }
    }
}

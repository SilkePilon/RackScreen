//! The GitHub contributions role.

use crate::anim::Secs;
use crate::event::CiStatus;
use crate::format::fmt_mins_secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CRIT, GREEN, OK, TEXT, TEXT2, TEXT3, TRACK};
use crate::theme::Color;

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

/// Ring segment colour for one run.
pub fn ci_color(s: CiStatus) -> Color {
    match s {
        CiStatus::Success => OK,
        CiStatus::Failure => CRIT,
        CiStatus::InProgress => BLUE,
        CiStatus::Other => TRACK,
    }
}

pub fn ci(model: &Model, _now: Secs) -> P2Screen {
    let runs = &model.ci().runs;
    let total = runs.len();
    let passed = runs
        .iter()
        .filter(|r| r.status == CiStatus::Success)
        .count();
    let failed = runs
        .iter()
        .filter(|r| r.status == CiStatus::Failure)
        .count();
    let mut s = P2Screen::new("CI", format!("{passed}/{total}"));
    s.hero_scale = 0.82;
    let mut colors: Vec<Color> = runs.iter().rev().map(|r| ci_color(r.status)).collect();
    while colors.len() < 12 {
        colors.insert(0, TRACK);
    }
    s.ring = RingSpec::discrete(colors);
    s.pill = Some(match runs.first() {
        Some(r) => {
            let repo = r.repo.rsplit('/').next().unwrap_or(&r.repo);
            match r.status {
                CiStatus::InProgress => (format!("{repo} · building"), BLUE),
                CiStatus::Success => (format!("{repo} · passed"), OK),
                CiStatus::Failure => (format!("{repo} · failed"), CRIT),
                CiStatus::Other => (format!("{repo} · cancelled"), TEXT2),
            }
        }
        None => ("no runs".into(), TEXT2),
    });
    let durations: Vec<i64> = runs.iter().filter_map(|r| r.duration_secs).collect();
    let avg = if durations.is_empty() {
        "—".to_string()
    } else {
        fmt_mins_secs(durations.iter().sum::<i64>() / durations.len() as i64)
    };
    s.card = Some(Card::Stats(vec![
        (avg, "avg run".into(), TEXT),
        (
            failed.to_string(),
            "failed".into(),
            if failed > 0 { CRIT } else { TEXT3 },
        ),
    ]));
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
    use crate::event::{CiRun, CiStatus};
    use crate::theme::p2::{BLUE, CRIT, OK, TEXT, TEXT3, TRACK};

    fn run(id: u64, status: CiStatus, duration: Option<i64>) -> CiRun {
        CiRun {
            id,
            repo: "silkepilon/RackScreen".into(),
            status,
            created_at: 1000 + id as i64,
            updated_at: 1000 + id as i64 + duration.unwrap_or(0),
            duration_secs: duration,
        }
    }

    #[test]
    fn ci_counts_passes_and_names_the_newest_run() {
        let mut m = Model::new(Thresholds::default());
        let mut runs: Vec<CiRun> = (1..=12)
            .map(|i| {
                run(
                    i,
                    if i == 5 {
                        CiStatus::Failure
                    } else {
                        CiStatus::Success
                    },
                    Some(100 + i as i64),
                )
            })
            .collect();
        runs.reverse();
        runs[0].status = CiStatus::InProgress;
        runs[0].duration_secs = None;
        m.apply(Event::CiRuns(runs.clone()), 0.0);
        let s = ci(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.hero_scale),
            ("CI", "10/12", 0.82)
        );
        match &s.ring.kind {
            RingKind::Discrete(c) => {
                assert_eq!(c.len(), 12);
                assert_eq!(c[11], BLUE, "newest last");
                assert_eq!(c[4], CRIT, "run 5 failed");
                assert_eq!(c[0], OK);
            }
            _ => panic!("discrete"),
        }
        assert_eq!(s.pill, Some(("RackScreen · building".into(), BLUE)));
        match &s.card {
            Some(Card::Stats(items)) => {
                // completed runs 1..=11 took 101..=111 s: mean 106
                assert_eq!(
                    items[0],
                    ("1m 46s".to_string(), "avg run".to_string(), TEXT)
                );
                assert_eq!(items[1], ("1".to_string(), "failed".to_string(), CRIT));
            }
            _ => panic!("stats"),
        }
        runs[0].status = CiStatus::Failure;
        m.apply(Event::CiRuns(runs.clone()), 1.0);
        let s = ci(&m, 1.0);
        assert_eq!(s.hero, "10/12");
        assert_eq!(s.pill, Some(("RackScreen · failed".into(), CRIT)));
        runs[0].status = CiStatus::Success;
        m.apply(Event::CiRuns(runs[..3].to_vec()), 2.0);
        let s = ci(&m, 2.0);
        assert_eq!(s.hero, "3/3");
        assert_eq!(s.pill, Some(("RackScreen · passed".into(), OK)));
        assert!(
            matches!(&s.ring.kind, RingKind::Discrete(c) if c.len() == 12 && c[0] == TRACK && c[8] == TRACK && c[9] == OK),
            "padded with track at the old end"
        );
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[1] == ("0".to_string(), "failed".to_string(), TEXT3))
        );
        m.apply(Event::CiRuns(vec![]), 3.0);
        let s = ci(&m, 3.0);
        assert_eq!(
            (s.hero.as_str(), s.pill.clone().unwrap().0.as_str()),
            ("0/0", "no runs")
        );
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0].0 == "—"));
        assert_eq!(ci_color(CiStatus::Other), TRACK);
    }
}

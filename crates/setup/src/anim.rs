//! Small time-based animations for the TUI. Times are seconds on a monotonic clock.

use rackscreen_core::anim::{Easing, Secs, Tween};

/// A value sliding from `from` to `to` with ease-out.
#[derive(Clone, Copy, Debug)]
pub struct Slide {
    tween: Tween,
}

impl Slide {
    pub fn fixed(v: f32) -> Slide {
        Slide {
            tween: Tween::new(v, v, 0.0, 0.0, Easing::OutCubic),
        }
    }
    pub fn to(&self, target: f32, now: Secs, secs: Secs) -> Slide {
        let from = self.value(now);
        Slide {
            tween: Tween::new(from, target, now, secs, Easing::OutCubic),
        }
    }
    pub fn value(&self, now: Secs) -> f32 {
        self.tween.value(now)
    }
    pub fn target(&self) -> f32 {
        self.tween.to
    }
    pub fn done(&self, now: Secs) -> bool {
        self.tween.done(now)
    }
}

const SETTLE_SECS: Secs = 0.25;
const SETTLE_STAGGER: Secs = 0.04;
const SETTLE_MAX_ROWS: usize = 12;

/// Rows of a freshly opened pane fade from faint to their final colour, each row a
/// little after the one above. `amount` is 0.0 (faint) to 1.0 (final).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settle {
    start: Secs,
}

impl Settle {
    pub fn new(now: Secs) -> Settle {
        Settle { start: now }
    }
    /// A settle that is already over (initial state, tests).
    pub fn finished() -> Settle {
        Settle {
            start: f64::NEG_INFINITY,
        }
    }
    pub fn amount(&self, row: usize, now: Secs) -> f32 {
        let delay = row.min(SETTLE_MAX_ROWS) as Secs * SETTLE_STAGGER;
        let t = ((now - self.start - delay) / SETTLE_SECS) as f32;
        Easing::OutCubic.apply(t)
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.start >= SETTLE_SECS + SETTLE_MAX_ROWS as Secs * SETTLE_STAGGER
    }
}

/// A 1.2 s sine between 0 and 1, for the unsaved dot.
pub fn pulse(now: Secs) -> f32 {
    (0.5 + 0.5 * (now * std::f64::consts::TAU / 1.2).sin()) as f32
}

pub fn spinner_frame<'a>(frames: &'a [&'a str], now: Secs) -> &'a str {
    let i = ((now * 12.0) as usize) % frames.len().max(1);
    frames[i]
}

/// Header glyph cycling on a 2.4 s loop, echoing the panels' breathing ring.
pub fn ring_glyph<'a>(frames: &'a [&'a str], now: Secs) -> &'a str {
    let i = ((now / 2.4 * frames.len() as f64) as usize) % frames.len().max(1);
    frames[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_eases_and_finishes() {
        let s = Slide::fixed(0.0).to(10.0, 1.0, 0.15);
        assert_eq!(s.value(1.0), 0.0);
        assert!(s.value(1.05) > 5.0, "ease-out is front loaded");
        assert!((s.value(2.0) - 10.0).abs() < 1e-5);
        // 1.15 - 1.0 lands a hair under 0.15 in f64, so check just past the end
        assert!(s.done(1.151));
    }

    #[test]
    fn spinner_and_ring_cycle() {
        let f = ["a", "b", "c", "d"];
        assert_eq!(ring_glyph(&f, 0.0), "a");
        assert_eq!(ring_glyph(&f, 0.6), "b");
        assert_eq!(ring_glyph(&f, 2.4), "a");
        assert_ne!(spinner_frame(&f, 0.0), spinner_frame(&f, 0.1));
    }

    #[test]
    fn settle_staggers_rows_and_finishes() {
        let s = Settle::new(10.0);
        assert_eq!(s.amount(0, 10.0), 0.0);
        assert_eq!(s.amount(5, 10.0), 0.0, "later rows have not started");
        assert!(
            (s.amount(0, 10.25) - 1.0).abs() < 1e-5,
            "row 0 done after 0.25 s"
        );
        assert!(s.amount(1, 10.25) < 1.0, "row 1 started 40 ms later");
        assert!(s.amount(0, 10.1) > s.amount(1, 10.1));
        // the stagger is capped so long lists finish together
        assert_eq!(
            s.amount(12, 10.0 + 0.48 + 0.25),
            s.amount(40, 10.0 + 0.48 + 0.25)
        );
        assert!(!s.done(10.5));
        assert!(s.done(10.0 + 0.25 + 0.48 + 0.001));
        // before it started nothing is visible, and a fixed settle is always done
        assert!(Settle::finished().done(0.0));
        assert_eq!(Settle::finished().amount(3, 0.0), 1.0);
    }

    #[test]
    fn pulse_stays_in_unit_range_with_a_1_2_s_period() {
        for i in 0..100 {
            let v = pulse(i as f64 * 0.037);
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
        assert!((pulse(0.0) - pulse(1.2)).abs() < 1e-4);
        assert!((pulse(0.3) - 1.0).abs() < 1e-4, "peak a quarter period in");
    }
}

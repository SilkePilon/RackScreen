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
}

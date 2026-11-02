//! Text wider than its slot holds still, then scrolls left and wraps.

use crate::anim::Secs;

pub const HOLD_SECS: Secs = 1.5;
pub const SPEED_PX_PER_SEC: f32 = 30.0;
pub const GAP_PX: f32 = 36.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Marquee {
    text: String,
    started: Secs,
}

impl Marquee {
    pub fn new(text: &str, now: Secs) -> Marquee {
        Marquee {
            text: text.to_string(),
            started: now,
        }
    }

    /// Changing the text restarts the hold; the same text keeps its phase.
    pub fn set(&mut self, text: &str, now: Secs) {
        if text != self.text {
            self.text = text.to_string();
            self.started = now;
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Pixels the text has scrolled left, or `None` when it fits the slot.
    pub fn offset(&self, text_w: f32, slot_w: f32, now: Secs) -> Option<f32> {
        if text_w <= slot_w {
            return None;
        }
        let period = HOLD_SECS + ((text_w + GAP_PX) / SPEED_PX_PER_SEC) as Secs;
        let p = (now - self.started).max(0.0) % period;
        Some(if p < HOLD_SECS {
            0.0
        } else {
            (p - HOLD_SECS) as f32 * SPEED_PX_PER_SEC
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_text_never_moves_and_wide_text_holds_then_scrolls() {
        let m = Marquee::new("ubuntu-24.04.iso", 10.0);
        assert_eq!(m.offset(100.0, 132.0, 11.0), None, "fits");
        assert_eq!(m.offset(200.0, 132.0, 10.0), Some(0.0));
        assert_eq!(
            m.offset(200.0, 132.0, 11.4),
            Some(0.0),
            "still holding at 1.4 s"
        );
        let at = m.offset(200.0, 132.0, 12.5).unwrap();
        assert!(
            (at - 30.0).abs() < 1e-4,
            "1 s past the hold at 30 px/s: {at}"
        );
        // period = 1.5 + (200 + 36) / 30 = 9.3666 s; just before the wrap the
        // text has scrolled its width plus the gap
        let end = m.offset(200.0, 132.0, 10.0 + 9.36).unwrap();
        assert!((end - (236.0 - 0.2)).abs() < 0.01, "{end}");
        assert_eq!(
            m.offset(200.0, 132.0, 10.0 + 9.3667),
            Some(0.0),
            "wrapped and holding"
        );
    }

    #[test]
    fn new_text_restarts_the_hold() {
        let mut m = Marquee::new("a", 0.0);
        assert!(m.offset(200.0, 132.0, 3.0).unwrap() > 0.0);
        m.set("a", 3.0);
        assert!(
            m.offset(200.0, 132.0, 3.0).unwrap() > 0.0,
            "same text keeps scrolling"
        );
        m.set("b", 3.0);
        assert_eq!(m.text(), "b");
        assert_eq!(m.offset(200.0, 132.0, 3.5), Some(0.0));
    }
}

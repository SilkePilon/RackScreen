//! Fixed-size sample buffers behind the sparklines: a stepped ring buffer that
//! remembers the sample that just scrolled off, and 24 hourly slots.

use std::collections::VecDeque;

use crate::anim::{Easing, Secs};

/// Samples at a nominal `step`, the newest last. Holds one more than `cap`
/// so the sparkline can slide the dropped point off the left edge.
#[derive(Clone, Debug, PartialEq)]
pub struct History {
    cap: usize,
    step: Secs,
    values: VecDeque<f32>,
    pushed_at: Option<Secs>,
}

impl History {
    pub fn new(cap: usize, step: Secs) -> History {
        History {
            cap: cap.max(1),
            step: step.max(0.1),
            values: VecDeque::with_capacity(cap + 1),
            pushed_at: None,
        }
    }

    /// Append a sample; non-finite values are skipped.
    pub fn push(&mut self, v: f32, now: Secs) {
        if !v.is_finite() {
            return;
        }
        self.values.push_back(v);
        while self.values.len() > self.cap + 1 {
            self.values.pop_front();
        }
        self.pushed_at = Some(now);
    }

    /// Replace the contents with the newest `cap + 1` finite values of
    /// `values` (oldest first). The line does not scroll after a prefill.
    pub fn prefill(&mut self, values: &[f32], _now: Secs) {
        let keep: Vec<f32> = values.iter().copied().filter(|v| v.is_finite()).collect();
        let start = keep.len().saturating_sub(self.cap + 1);
        self.values = keep[start..].iter().copied().collect();
        self.pushed_at = None;
    }

    /// The last `cap` samples, oldest first.
    pub fn series(&self) -> Vec<f32> {
        let start = self.values.len().saturating_sub(self.cap);
        self.values.iter().skip(start).copied().collect()
    }

    /// The sample just before `series`, once the buffer is full.
    pub fn prev(&self) -> Option<f32> {
        if self.values.len() > self.cap {
            self.values.front().copied()
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.values.len().min(self.cap)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn step(&self) -> Secs {
        self.step
    }

    pub fn mean(&self) -> Option<f32> {
        let s = self.series();
        if s.is_empty() {
            None
        } else {
            Some(s.iter().sum::<f32>() / s.len() as f32)
        }
    }

    /// Sparkline scroll phase: 0 right after a push (the line sits one step to
    /// the right), 1 once it has slid into place over the first 30 % of the step.
    pub fn scroll(&self, now: Secs) -> f32 {
        match self.pushed_at {
            Some(t) if self.prev().is_some() => {
                Easing::InOutCubic.apply(((now - t) / (0.3 * self.step)).clamp(0.0, 1.0) as f32)
            }
            _ => 1.0,
        }
    }
}

pub const HOURLY_CAP: i64 = 24;

/// One value per unix hour for the last 24 hours; a repeated hour replaces,
/// an older hour than the newest is ignored, gaps are simply absent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hourly {
    slots: VecDeque<(i64, f32)>,
}

impl Hourly {
    pub fn new() -> Hourly {
        Hourly::default()
    }

    pub fn push(&mut self, unix_hour: i64, v: f32) {
        if !v.is_finite() {
            return;
        }
        match self.slots.back_mut() {
            Some(b) if b.0 == unix_hour => b.1 = v,
            Some(b) if b.0 > unix_hour => return,
            _ => self.slots.push_back((unix_hour, v)),
        }
        while self
            .slots
            .front()
            .is_some_and(|f| unix_hour - f.0 >= HOURLY_CAP)
        {
            self.slots.pop_front();
        }
    }

    pub fn values(&self) -> Vec<f32> {
        self.slots.iter().map(|s| s.1).collect()
    }

    /// Values of hours at or after `unix_hour_from` (today's, for instance).
    pub fn values_since(&self, unix_hour_from: i64) -> Vec<f32> {
        self.slots
            .iter()
            .filter(|s| s.0 >= unix_hour_from)
            .map(|s| s.1)
            .collect()
    }

    pub fn mean(&self) -> Option<f32> {
        if self.slots.is_empty() {
            None
        } else {
            Some(self.slots.iter().map(|s| s.1).sum::<f32>() / self.slots.len() as f32)
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The 24 hours ending at `unix_hour_now`, oldest first; an hour without a
    /// sample is `None`.
    pub fn last_24(&self, unix_hour_now: i64) -> Vec<Option<f32>> {
        (0..HOURLY_CAP)
            .map(|i| {
                let h = unix_hour_now - (HOURLY_CAP - 1) + i;
                self.slots.iter().find(|s| s.0 == h).map(|s| s.1)
            })
            .collect()
    }

    /// Replace everything with `(unix hour, value)` pairs, oldest first.
    pub fn replace(&mut self, hourly: &[(i64, f32)]) {
        self.slots.clear();
        for (h, v) in hourly {
            self.push(*h, *v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_cap_plus_the_dropped_sample_and_scrolls_after_a_push() {
        let mut h = History::new(3, 2.0);
        assert!(h.is_empty());
        assert_eq!(h.scroll(0.0), 1.0, "nothing to scroll");
        h.push(1.0, 0.0);
        h.push(2.0, 2.0);
        h.push(3.0, 4.0);
        assert_eq!(h.series(), vec![1.0, 2.0, 3.0]);
        assert_eq!(h.prev(), None, "not full yet");
        assert_eq!(h.scroll(4.0), 1.0);
        h.push(f32::NAN, 5.0);
        assert_eq!(h.len(), 3, "NaN is skipped");
        h.push(4.0, 6.0);
        assert_eq!(h.series(), vec![2.0, 3.0, 4.0]);
        assert_eq!(h.prev(), Some(1.0));
        assert_eq!(h.scroll(6.0), 0.0, "the line starts one step to the right");
        let mid = h.scroll(6.3);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(h.scroll(6.6), 1.0, "settled after 30 % of the 2 s step");
        h.push(5.0, 8.0);
        assert_eq!((h.series(), h.prev()), (vec![3.0, 4.0, 5.0], Some(2.0)));
        assert!((h.mean().unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn prefill_takes_the_newest_and_does_not_scroll() {
        let mut h = History::new(3, 2.0);
        h.prefill(&[9.0, f32::NAN, 1.0, 2.0, 3.0, 4.0], 10.0);
        assert_eq!(h.series(), vec![2.0, 3.0, 4.0]);
        assert_eq!(h.prev(), Some(1.0));
        assert_eq!(h.scroll(10.0), 1.0, "a prefilled line stands still");
        h.push(5.0, 12.0);
        assert_eq!(h.scroll(12.0), 0.0);
    }

    #[test]
    fn hourly_slots_replace_the_same_hour_and_drop_old_ones() {
        let mut h = Hourly::new();
        assert!(h.is_empty() && h.mean().is_none());
        h.push(100, 10.0);
        h.push(100, 11.0);
        assert_eq!(h.values(), vec![11.0]);
        h.push(101, 12.0);
        h.push(99, 5.0);
        assert_eq!(h.values(), vec![11.0, 12.0], "an older hour is ignored");
        for i in 2..30 {
            h.push(100 + i, i as f32);
        }
        assert_eq!(h.len(), 24);
        assert_eq!(
            h.values()[0],
            6.0,
            "hour 106 is the oldest kept at hour 129"
        );
        assert_eq!(h.values_since(128), vec![28.0, 29.0]);
        assert!((h.mean().unwrap() - 17.5).abs() < 1e-5);
    }
    #[test]
    fn last_24_lines_the_slots_up_with_gaps() {
        let mut h = Hourly::new();
        h.push(100, 1.0);
        h.push(101, 2.0);
        h.push(103, 4.0);
        let v = h.last_24(103);
        assert_eq!(v.len(), 24);
        assert_eq!(v[23], Some(4.0));
        assert_eq!(v[22], None, "hour 102 was never pushed");
        assert_eq!(v[21], Some(2.0));
        assert_eq!(v[20], Some(1.0));
        assert!(v[..20].iter().all(|x| x.is_none()));
        assert_eq!(
            h.last_24(104)[23],
            None,
            "a newer current hour shifts everything left"
        );
        assert_eq!(h.last_24(104)[22], Some(4.0));
        assert!(Hourly::new().last_24(5).iter().all(|x| x.is_none()));
    }
}

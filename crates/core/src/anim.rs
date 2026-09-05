//! Time-based animation primitives. All times are seconds on a monotonic clock.

pub type Secs = f64;

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    OutCubic,
    InOutCubic,
    Spring,
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = clamp01(t);
        match self {
            Easing::Linear => t,
            Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Spring => {
                // easeOutBack: overshoots to ~1.10 around t=0.6 then settles at 1.0
                let c1 = 1.70158_f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: Secs,
    pub duration: Secs,
    pub easing: Easing,
}

impl Tween {
    pub fn new(from: f32, to: f32, start: Secs, duration: Secs, easing: Easing) -> Self {
        Self { from, to, start, duration, easing }
    }
    pub fn progress(&self, now: Secs) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        clamp01(((now - self.start) / self.duration) as f32)
    }
    pub fn value(&self, now: Secs) -> f32 {
        let e = self.easing.apply(self.progress(now));
        self.from + (self.to - self.from) * e
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.start >= self.duration
    }
}

/// A value that eases toward a retargetable goal without ever jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Smooth {
    current: f32,
    target: f32,
    tween: Option<Tween>,
    duration: Secs,
}

impl Smooth {
    pub fn new(initial: f32, duration: Secs) -> Self {
        Self { current: initial, target: initial, tween: None, duration }
    }
    pub fn set(&mut self, target: f32, now: Secs) {
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }
        let from = self.value(now);
        self.current = from;
        self.target = target;
        self.tween = Some(Tween::new(from, target, now, self.duration, Easing::OutCubic));
    }
    pub fn value(&self, now: Secs) -> f32 {
        match self.tween {
            Some(t) => t.value(now),
            None => self.current,
        }
    }
    pub fn target(&self) -> f32 {
        self.target
    }
}

/// Sine breathe between 0.35 and 1.0.
pub fn breathe(now: Secs, period: Secs) -> f32 {
    let p = pulse(now, period);
    0.35 + 0.65 * p
}

/// Sine pulse 0..1 with the given period, starting at 0.
pub fn pulse(now: Secs, period: Secs) -> f32 {
    let phase = (now / period) * std::f64::consts::TAU;
    (0.5 - 0.5 * phase.cos()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_midpoint() {
        assert_eq!(Easing::Linear.apply(0.5), 0.5);
    }

    #[test]
    fn out_cubic_known_value() {
        assert!((Easing::OutCubic.apply(0.5) - 0.875).abs() < 1e-6);
    }

    #[test]
    fn spring_overshoots_then_settles() {
        assert!(Easing::Spring.apply(0.6) > 1.0);
        assert!((Easing::Spring.apply(1.0) - 1.0).abs() < 1e-6);
        assert!((Easing::Spring.apply(0.0)).abs() < 1e-6);
    }

    #[test]
    fn tween_clamps_and_reports_done() {
        let t = Tween::new(0.0, 10.0, 1.0, 2.0, Easing::Linear);
        assert_eq!(t.value(0.0), 0.0);
        assert_eq!(t.value(2.0), 5.0);
        assert_eq!(t.value(5.0), 10.0);
        assert!(!t.done(2.9));
        assert!(t.done(3.0));
    }

    #[test]
    fn smooth_retarget_is_continuous() {
        let mut s = Smooth::new(0.0, 1.0);
        s.set(100.0, 0.0);
        let mid = s.value(0.5);
        assert!(mid > 50.0 && mid < 100.0);
        s.set(0.0, 0.5);
        assert!((s.value(0.5) - mid).abs() < 1e-4);
        assert!((s.value(2.0)).abs() < 1e-4);
    }

    #[test]
    fn smooth_ignores_same_target() {
        let mut s = Smooth::new(5.0, 1.0);
        s.set(5.0, 0.0);
        assert_eq!(s.value(0.5), 5.0);
    }

    #[test]
    fn breathe_range() {
        for i in 0..100 {
            let v = breathe(i as f64 * 0.05, 2.4);
            assert!((0.35..=1.0).contains(&v));
        }
        assert!((breathe(0.0, 2.4) - 0.35).abs() < 1e-6);
        assert!((breathe(1.2, 2.4) - 1.0).abs() < 1e-6);
    }
}

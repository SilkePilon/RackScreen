//! The face's expression: five knobs per mood, tweened, plus the idle life
//! (blinks, gaze, breathing and mood wobble) that keeps it alive.

use crate::anim::{Easing, Secs};
use crate::mood::Mood;

pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}
pub fn ease(k: f32) -> f32 {
    Easing::InOutCubic.apply(k)
}
/// xorshift64, the same generator `Model` uses for its screen order.
pub fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}
/// Uniform in `0..1`.
pub fn unit(state: &mut u64) -> f32 {
    (xorshift(state) >> 11) as f32 / (1u64 << 53) as f32
}

/// Lid openness, upper-lid slant (+ inner corners down = angry, − = sad),
/// lower-lid rise (0.7 is the `^ ^` shape), eye size and vertical lift in
/// pixels (negative is up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expr {
    pub open: f32,
    pub tilt: f32,
    pub lower: f32,
    pub scale: f32,
    pub lift: f32,
}

const fn expr(open: f32, tilt: f32, lower: f32, scale: f32, lift: f32) -> Expr {
    Expr {
        open,
        tilt,
        lower,
        scale,
        lift,
    }
}

impl Expr {
    pub fn of(m: Mood) -> Expr {
        match m {
            Mood::Scared => expr(1.30, -0.40, 0.00, 0.85, -2.0),
            Mood::Sad => expr(0.65, -1.00, 0.00, 1.00, 8.0),
            Mood::Angry => expr(0.60, 1.00, 0.15, 1.00, 0.0),
            Mood::Hot => expr(0.70, 0.30, 0.20, 1.00, 3.0),
            Mood::Worried => expr(0.85, -0.60, 0.10, 1.00, 2.0),
            Mood::Sleepy => expr(0.25, 0.00, 0.00, 1.00, 6.0),
            Mood::Happy => expr(1.00, 0.00, 0.70, 1.00, -4.0),
            Mood::Content => expr(1.00, 0.00, 0.00, 1.00, 0.0),
        }
    }
    pub fn lerp(a: Expr, b: Expr, k: f32) -> Expr {
        expr(
            lerp(a.open, b.open, k),
            lerp(a.tilt, b.tilt, k),
            lerp(a.lower, b.lower, k),
            lerp(a.scale, b.scale, k),
            lerp(a.lift, b.lift, k),
        )
    }
    pub fn close_to(&self, o: &Expr, eps: f32) -> bool {
        (self.open - o.open).abs() <= eps
            && (self.tilt - o.tilt).abs() <= eps
            && (self.lower - o.lower).abs() <= eps
            && (self.scale - o.scale).abs() <= eps
            && (self.lift - o.lift).abs() <= eps
    }
}

pub const EXPR_TWEEN_SECS: Secs = 0.42;

/// An expression easing toward a retargetable goal, never jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExprTween {
    from: Expr,
    to: Expr,
    start: Secs,
}

impl ExprTween {
    pub fn new(still: Expr) -> Self {
        Self {
            from: still,
            to: still,
            start: -1.0e9,
        }
    }
    pub fn retarget(&mut self, to: Expr, now: Secs) {
        if to == self.to {
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.start = now;
    }
    pub fn value(&self, now: Secs) -> Expr {
        let k = ((now - self.start) / EXPR_TWEEN_SECS).clamp(0.0, 1.0) as f32;
        Expr::lerp(self.from, self.to, ease(k))
    }
    pub fn target(&self) -> Expr {
        self.to
    }
}

/// What the idle layer contributes this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdleOut {
    /// 0 open .. 1 shut, already scaled by the mood's blink depth.
    pub blink: f32,
    /// Where the eyes look, each axis in −1..1.
    pub gaze: (f32, f32),
    /// Breathing bob plus the mood wobble, in pixels.
    pub wobble: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Idle {
    next_blink: Secs,
    blink_start: Option<Secs>,
    gaze_from: (f32, f32),
    gaze_to: (f32, f32),
    gaze_start: Secs,
    gaze_dur: Secs,
    next_gaze: Secs,
}

impl Default for Idle {
    fn default() -> Self {
        Self::new()
    }
}

impl Idle {
    pub fn new() -> Self {
        Self {
            next_blink: 1.5,
            blink_start: None,
            gaze_from: (0.0, 0.0),
            gaze_to: (0.0, 0.0),
            gaze_start: 0.0,
            gaze_dur: 0.26,
            next_gaze: 0.8,
        }
    }

    pub fn tick(&mut self, mood: Mood, now: Secs, rng: &mut u64) -> IdleOut {
        // blink: 150 ms every 1.5..4 s, about every 5 s when sleepy
        if self.blink_start.is_none() && now >= self.next_blink {
            self.blink_start = Some(now);
        }
        let mut blink = 0.0;
        if let Some(start) = self.blink_start {
            let p = ((now - start) / 0.15) as f32;
            if p < 1.0 {
                blink = (p * std::f32::consts::PI).sin();
            } else {
                self.blink_start = None;
                self.next_blink = now
                    + if mood == Mood::Sleepy {
                        4.0 + 2.0 * unit(rng) as Secs
                    } else {
                        1.5 + 2.5 * unit(rng) as Secs
                    };
            }
        }
        if matches!(mood, Mood::Scared | Mood::Angry) {
            blink *= 0.6;
        }
        // gaze: retarget every 0.7..2 s, 0.35..0.85 s when jittery; sad and sleepy look down
        let fast = matches!(mood, Mood::Worried | Mood::Scared);
        if now >= self.next_gaze {
            self.gaze_from = self.gaze_value(now);
            self.gaze_to = match mood {
                Mood::Sad | Mood::Sleepy => ((unit(rng) - 0.5) * 0.6, 0.7),
                _ => ((unit(rng) - 0.5) * 1.4, (unit(rng) - 0.5) * 0.9),
            };
            self.gaze_start = now;
            self.gaze_dur = if fast { 0.12 } else { 0.26 };
            self.next_gaze = now
                + if fast {
                    0.35 + 0.5 * unit(rng) as Secs
                } else {
                    0.7 + 1.3 * unit(rng) as Secs
                };
        }
        let gaze = self.gaze_value(now);
        // f64 sines: an f32 clock would stutter after a few days of uptime
        let s = |w: f64| (now * w).sin() as f32;
        let mut wobble = (0.0, 1.5 * s(0.9));
        match mood {
            Mood::Angry => {
                if s(1.3) > 0.2 {
                    wobble.0 = s(46.0) * 2.2;
                }
            }
            Mood::Scared => wobble.0 = s(70.0) * 1.4,
            Mood::Sleepy => wobble.1 += s(1.4) * 4.0 + 3.0,
            Mood::Hot => wobble.1 += s(2.2) * 1.2,
            _ => {}
        }
        IdleOut {
            blink,
            gaze,
            wobble,
        }
    }

    fn gaze_value(&self, now: Secs) -> (f32, f32) {
        let k = ease(((now - self.gaze_start) / self.gaze_dur).clamp(0.0, 1.0) as f32);
        (
            lerp(self.gaze_from.0, self.gaze_to.0, k),
            lerp(self.gaze_from.1, self.gaze_to.1, k),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mood_table_matches_the_spec() {
        let check = |m: Mood, open: f32, tilt: f32, lower: f32, scale: f32, lift: f32| {
            let e = Expr::of(m);
            assert_eq!(
                (e.open, e.tilt, e.lower, e.scale, e.lift),
                (open, tilt, lower, scale, lift),
                "{m:?}"
            );
        };
        check(Mood::Scared, 1.30, -0.40, 0.0, 0.85, -2.0);
        check(Mood::Sad, 0.65, -1.00, 0.0, 1.0, 8.0);
        check(Mood::Angry, 0.60, 1.00, 0.15, 1.0, 0.0);
        check(Mood::Hot, 0.70, 0.30, 0.20, 1.0, 3.0);
        check(Mood::Worried, 0.85, -0.60, 0.10, 1.0, 2.0);
        check(Mood::Sleepy, 0.25, 0.0, 0.0, 1.0, 6.0);
        check(Mood::Happy, 1.00, 0.0, 0.70, 1.0, -4.0);
        check(Mood::Content, 1.00, 0.0, 0.0, 1.0, 0.0);
    }

    #[test]
    fn expr_tween_retargets_without_a_jump() {
        let mut t = ExprTween::new(Expr::of(Mood::Content));
        assert_eq!(t.value(5.0), Expr::of(Mood::Content));
        t.retarget(Expr::of(Mood::Sad), 10.0);
        assert!(t.value(10.0).close_to(&Expr::of(Mood::Content), 1e-4));
        let mid = t.value(10.21);
        assert!(mid.lift > 0.5 && mid.lift < 7.5, "half way: {}", mid.lift);
        assert!(t.value(10.42).close_to(&Expr::of(Mood::Sad), 1e-4));
        t.retarget(Expr::of(Mood::Content), 10.21);
        assert!(t.value(10.21).close_to(&mid, 1e-4));
    }

    #[test]
    fn idle_blinks_wanders_and_is_deterministic() {
        let mut rng = 99u64;
        let mut idle = Idle::new();
        let mut blinked = false;
        let mut gazes = std::collections::HashSet::new();
        let mut t = 0.0;
        while t < 30.0 {
            let o = idle.tick(Mood::Content, t, &mut rng);
            if o.blink > 0.9 {
                blinked = true;
            }
            assert!((0.0..=1.0).contains(&o.blink));
            assert!(o.gaze.0.abs() <= 1.0 && o.gaze.1.abs() <= 1.0);
            gazes.insert(((o.gaze.0 * 100.0) as i32, (o.gaze.1 * 100.0) as i32));
            assert!(o.wobble.1.abs() <= 1.5 + 1e-6, "content only breathes");
            t += 1.0 / 30.0;
        }
        assert!(blinked, "no full blink in 30 s");
        assert!(gazes.len() > 20, "gaze never wandered");
        let mut a = (Idle::new(), 7u64);
        let mut b = (Idle::new(), 7u64);
        for i in 0..900 {
            let t = i as f64 / 30.0;
            assert_eq!(
                a.0.tick(Mood::Worried, t, &mut a.1),
                b.0.tick(Mood::Worried, t, &mut b.1)
            );
        }
    }

    #[test]
    fn moods_shape_the_idle_life() {
        let mut rng = 3u64;
        let mut idle = Idle::new();
        let mut max_blink: f32 = 0.0;
        let mut t = 0.0;
        while t < 20.0 {
            max_blink = max_blink.max(idle.tick(Mood::Scared, t, &mut rng).blink);
            t += 1.0 / 30.0;
        }
        assert!(
            max_blink > 0.5 && max_blink <= 0.6 + 1e-3,
            "scared blinks 60 %: {max_blink}"
        );
        let mut idle = Idle::new();
        let mut down = true;
        let mut t = 0.0;
        while t < 20.0 {
            let g = idle.tick(Mood::Sad, t, &mut rng).gaze;
            if t > 1.0 && g.1 < 0.3 {
                down = false;
            }
            t += 1.0 / 30.0;
        }
        assert!(down, "sad looks down");
        let mut idle = Idle::new();
        let mut blinks = 0;
        let mut was = false;
        let mut t = 0.0;
        while t < 60.0 {
            let b = idle.tick(Mood::Sleepy, t, &mut rng).blink > 0.5;
            if b && !was {
                blinks += 1;
            }
            was = b;
            t += 1.0 / 30.0;
        }
        assert!(
            (8..=16).contains(&blinks),
            "sleepy blinks about every 5 s: {blinks}"
        );
        let mut idle = Idle::new();
        let w = idle.tick(Mood::Scared, 1.0, &mut rng).wobble;
        assert!(w.0.abs() > 0.0, "scared trembles sideways");
        let s = idle.tick(Mood::Sleepy, 1.0, &mut rng).wobble;
        assert!(s.1 > 0.0, "sleepy nods down");
    }
}

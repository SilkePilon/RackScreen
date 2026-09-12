//! The face's expression: six knobs per mood, tweened, plus the idle behaviour
//! (blinks, gaze, head tilt, mood wobble) that keeps it alive between acts.

use crate::anim::{Easing, Secs};
use crate::mood::Mood;

pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}
/// 0 at `p <= a`, 1 at `p >= b`, linear between.
pub fn seg(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a)).clamp(0.0, 1.0)
}
pub fn inseg(p: f32, a: f32, b: f32) -> bool {
    p >= a && p < b
}
/// Rises 0..1..0 over `[a, b]`.
pub fn bump(p: f32, a: f32, b: f32) -> f32 {
    (seg(p, a, b) * std::f32::consts::PI).sin()
}
pub fn ease(k: f32) -> f32 {
    Easing::InOutCubic.apply(k)
}
pub fn out_back(k: f32) -> f32 {
    Easing::Spring.apply(k)
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

/// One expression: lid openness, upper-lid slant (+ inner corners down =
/// angry, − = sad), lower-lid squint (0.7 is the `^ ^` shape), eye size,
/// separation and vertical lift in pixels (negative is up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expr {
    pub open: f32,
    pub tilt: f32,
    pub lower: f32,
    pub scale: f32,
    pub sep: f32,
    pub lift: f32,
}

const fn expr(open: f32, tilt: f32, lower: f32, scale: f32, sep: f32, lift: f32) -> Expr {
    Expr {
        open,
        tilt,
        lower,
        scale,
        sep,
        lift,
    }
}

impl Expr {
    pub const CONTENT: Expr = expr(1.00, 0.00, 0.00, 1.00, 1.00, 0.0);
    pub const HAPPY: Expr = expr(1.00, 0.00, 0.70, 1.00, 1.00, -4.0);
    pub const EXCITED: Expr = expr(1.20, -0.10, 0.10, 1.10, 1.02, -6.0);
    pub const WORRIED: Expr = expr(0.85, -0.60, 0.10, 1.00, 1.00, 2.0);
    pub const SAD: Expr = expr(0.65, -1.00, 0.00, 1.00, 1.00, 8.0);
    pub const ANGRY: Expr = expr(0.60, 1.00, 0.15, 1.00, 0.96, 0.0);
    pub const HOT: Expr = expr(0.70, 0.30, 0.20, 1.00, 1.00, 3.0);
    pub const SCARED: Expr = expr(1.30, -0.40, 0.00, 0.85, 1.00, -2.0);
    pub const SLEEPY: Expr = expr(0.25, 0.00, 0.00, 1.00, 1.00, 6.0);
    pub const BORED: Expr = expr(0.50, 0.00, 0.00, 1.00, 1.00, 2.0);
    // transient poses used inside acts only
    pub const RELIEF: Expr = expr(0.12, 0.00, 0.40, 1.00, 1.00, 0.0);
    pub const GRIMACE: Expr = expr(0.55, 0.50, 0.35, 1.00, 0.97, 0.0);
    pub const WOW: Expr = expr(1.35, 0.00, 0.00, 1.05, 1.00, -3.0);
    pub const MEH: Expr = expr(0.50, 0.00, 0.00, 1.00, 1.00, 2.0);
    pub const FOCUS: Expr = expr(0.75, 0.20, 0.10, 1.00, 0.94, 0.0);

    pub fn of(m: Mood) -> Expr {
        match m {
            Mood::Content => Expr::CONTENT,
            Mood::Happy => Expr::HAPPY,
            Mood::Excited => Expr::EXCITED,
            Mood::Worried => Expr::WORRIED,
            Mood::Sad => Expr::SAD,
            Mood::Angry => Expr::ANGRY,
            Mood::Hot => Expr::HOT,
            Mood::Scared => Expr::SCARED,
            Mood::Sleepy => Expr::SLEEPY,
            Mood::Bored => Expr::BORED,
        }
    }
    pub fn lerp(a: Expr, b: Expr, k: f32) -> Expr {
        expr(
            lerp(a.open, b.open, k),
            lerp(a.tilt, b.tilt, k),
            lerp(a.lower, b.lower, k),
            lerp(a.scale, b.scale, k),
            lerp(a.sep, b.sep, k),
            lerp(a.lift, b.lift, k),
        )
    }
    pub fn close_to(&self, o: &Expr, eps: f32) -> bool {
        (self.open - o.open).abs() <= eps
            && (self.tilt - o.tilt).abs() <= eps
            && (self.lower - o.lower).abs() <= eps
            && (self.scale - o.scale).abs() <= eps
            && (self.sep - o.sep).abs() <= eps
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
    /// Head tilt in radians.
    pub tilt: f32,
    /// Mood wobble offset in pixels.
    pub wobble: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Idle {
    next_blink: Secs,
    blink_start: Option<Secs>,
    double: bool,
    gaze_from: (f32, f32),
    gaze_to: (f32, f32),
    gaze_start: Secs,
    gaze_dur: Secs,
    next_gaze: Secs,
    tilt_start: Option<(Secs, f32)>,
    next_tilt: Secs,
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
            double: false,
            gaze_from: (0.0, 0.0),
            gaze_to: (0.0, 0.0),
            gaze_start: 0.0,
            gaze_dur: 0.26,
            next_gaze: 0.8,
            tilt_start: None,
            next_tilt: 10.0,
        }
    }

    pub fn tick(&mut self, mood: Mood, now: Secs, rng: &mut u64) -> IdleOut {
        // blink: every 1.5..4 s, 150 ms, 20 % double
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
                if !self.double && unit(rng) < 0.2 {
                    self.double = true;
                    self.next_blink = now + 0.12;
                } else {
                    self.double = false;
                    self.next_blink = now + 1.5 + 2.5 * unit(rng) as Secs;
                }
            }
        }
        if matches!(mood, Mood::Scared | Mood::Angry) {
            blink *= 0.6;
        }
        // gaze: retarget every 0.7..2 s (0.35..0.85 s when jittery)
        let fast = matches!(mood, Mood::Worried | Mood::Excited | Mood::Scared);
        if now >= self.next_gaze {
            self.gaze_from = self.gaze_value(now);
            self.gaze_to = match mood {
                Mood::Bored => (if unit(rng) < 0.5 { -1.0 } else { 1.0 }, 0.15),
                Mood::Sleepy => ((unit(rng) - 0.5) * 0.3, 0.6),
                Mood::Sad => ((unit(rng) - 0.5) * 0.6, 0.5),
                _ => ((unit(rng) - 0.5) * 1.6, (unit(rng) - 0.5) * 1.0),
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
        // head tilt: ±6° for 1.5 s every 10..25 s
        if self.tilt_start.is_none() && now >= self.next_tilt {
            let dir = if unit(rng) < 0.5 { -1.0 } else { 1.0 };
            self.tilt_start = Some((now, dir));
        }
        let mut tilt = 0.0;
        if let Some((start, dir)) = self.tilt_start {
            let p = ((now - start) / 1.5) as f32;
            if p < 1.0 {
                tilt = dir * 6.0_f32.to_radians() * bump(p, 0.0, 1.0);
            } else {
                self.tilt_start = None;
                self.next_tilt = now + 10.0 + 15.0 * unit(rng) as Secs;
            }
        }
        let t = now as f32;
        let wobble = match mood {
            Mood::Excited => (0.0, -(t * 9.0).sin().abs() * 7.0),
            Mood::Angry => {
                let burst = if (t * 1.3).sin() > 0.2 { 1.0 } else { 0.0 };
                ((t * 46.0).sin() * 2.2 * burst, 0.0)
            }
            Mood::Scared => ((t * 70.0).sin() * 1.4, 0.0),
            Mood::Sleepy => (0.0, (t * 1.4).sin() * 4.0 + 3.0),
            Mood::Sad => (0.0, 4.0),
            Mood::Bored => (0.0, 2.0),
            _ => (0.0, 0.0),
        };
        IdleOut {
            blink,
            gaze,
            tilt,
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
    fn helpers() {
        assert_eq!(seg(0.5, 0.0, 1.0), 0.5);
        assert_eq!(seg(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(seg(2.0, 0.0, 1.0), 1.0);
        assert!(inseg(0.2, 0.2, 0.5) && !inseg(0.5, 0.2, 0.5));
        assert!((bump(0.5, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(bump(0.0, 0.0, 1.0).abs() < 1e-6);
        let mut s = 7u64;
        let a = unit(&mut s);
        assert!((0.0..1.0).contains(&a));
        assert_ne!(a, unit(&mut s));
    }

    #[test]
    fn mood_table_matches_the_spec() {
        assert_eq!(Expr::of(Mood::Content), Expr::CONTENT);
        assert_eq!(Expr::HAPPY.lower, 0.70);
        assert_eq!(Expr::HAPPY.lift, -4.0);
        assert_eq!(Expr::SAD.tilt, -1.0);
        assert_eq!(Expr::SAD.lift, 8.0);
        assert_eq!(Expr::ANGRY.tilt, 1.0);
        assert_eq!(Expr::SCARED.scale, 0.85);
        assert_eq!(Expr::SLEEPY.open, 0.25);
        assert_eq!(Expr::of(Mood::Bored).open, 0.50);
        let mid = Expr::lerp(Expr::CONTENT, Expr::SAD, 0.5);
        assert!((mid.lift - 4.0).abs() < 1e-6);
        assert!((mid.tilt + 0.5).abs() < 1e-6);
    }

    #[test]
    fn expr_tween_retargets_without_a_jump() {
        let mut t = ExprTween::new(Expr::CONTENT);
        assert_eq!(t.value(5.0), Expr::CONTENT);
        t.retarget(Expr::SAD, 10.0);
        let before = t.value(10.0);
        assert!(before.close_to(&Expr::CONTENT, 1e-4));
        let mid = t.value(10.21);
        assert!(mid.lift > 0.5 && mid.lift < 7.5, "half way: {}", mid.lift);
        assert!(t.value(10.42).close_to(&Expr::SAD, 1e-4));
        // retarget mid-flight starts from the current value
        t.retarget(Expr::CONTENT, 10.21);
        assert!(t.value(10.21).close_to(&mid, 1e-4));
        assert_eq!(t.target(), Expr::CONTENT);
    }

    #[test]
    fn idle_blinks_and_wanders_deterministically() {
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
            t += 1.0 / 30.0;
        }
        assert!(blinked, "no full blink in 30 s");
        assert!(gazes.len() > 20, "gaze never wandered");
        // same seed, same story
        let mut rng2 = 99u64;
        let mut idle2 = Idle::new();
        let mut rng3 = 99u64;
        let mut idle3 = Idle::new();
        for i in 0..900 {
            let t = i as f64 / 30.0;
            assert_eq!(
                idle2.tick(Mood::Content, t, &mut rng2),
                idle3.tick(Mood::Content, t, &mut rng3)
            );
        }
    }

    #[test]
    fn scared_and_angry_blink_shallow_and_bored_looks_sideways() {
        let mut rng = 3u64;
        let mut idle = Idle::new();
        let mut max_blink: f32 = 0.0;
        let mut t = 0.0;
        while t < 20.0 {
            max_blink = max_blink.max(idle.tick(Mood::Scared, t, &mut rng).blink);
            t += 1.0 / 30.0;
        }
        assert!(max_blink > 0.5 && max_blink <= 0.6 + 1e-3, "{max_blink}");
        let mut idle = Idle::new();
        let mut t = 0.0;
        let mut sideways = false;
        while t < 20.0 {
            let g = idle.tick(Mood::Bored, t, &mut rng).gaze;
            if g.0.abs() > 0.95 {
                sideways = true;
            }
            t += 1.0 / 30.0;
        }
        assert!(sideways);
    }

    #[test]
    fn wobble_is_zero_for_content_and_sags_for_sad() {
        let mut rng = 1u64;
        let mut idle = Idle::new();
        let c = idle.tick(Mood::Content, 1.0, &mut rng).wobble;
        assert_eq!(c, (0.0, 0.0));
        let s = idle.tick(Mood::Sad, 1.0, &mut rng).wobble;
        assert_eq!(s, (0.0, 4.0));
    }
}

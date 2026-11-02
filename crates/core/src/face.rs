//! The face: a mood tween plus the idle life, producing one `FaceFrame` per tick.

use crate::anim::Secs;
use crate::face_expr::{Expr, ExprTween, Idle, IdleOut};
use crate::mood::Mood;

/// What `scene_face` draws: the tweened expression and the idle life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceFrame {
    pub e: Expr,
    pub blink: f32,
    pub gaze: (f32, f32),
    pub wobble: (f32, f32),
    pub mood: Mood,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    tween: ExprTween,
    idle: Idle,
    rng: u64,
    mood: Mood,
    last: IdleOut,
}

impl Default for Face {
    fn default() -> Self {
        Self::new()
    }
}

impl Face {
    pub fn new() -> Face {
        Face {
            tween: ExprTween::new(Expr::of(Mood::Content)),
            idle: Idle::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            mood: Mood::Content,
            last: IdleOut {
                blink: 0.0,
                gaze: (0.0, 0.0),
                wobble: (0.0, 0.0),
            },
        }
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.rng = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
    }

    pub fn mood(&self) -> Mood {
        self.mood
    }

    /// Advance once per frame with the mood the rack state gives.
    pub fn tick(&mut self, mood: Mood, now: Secs) {
        if mood != self.mood {
            self.mood = mood;
            self.tween.retarget(Expr::of(mood), now);
        }
        self.last = self.idle.tick(mood, now, &mut self.rng);
    }

    /// The frame at `now`: the expression is re-evaluated, the idle life is
    /// the last tick's, so two reads at the same instant agree.
    pub fn frame(&self, now: Secs) -> FaceFrame {
        FaceFrame {
            e: self.tween.value(now),
            blink: self.last.blink,
            gaze: self.last.gaze,
            wobble: self.last.wobble,
            mood: self.mood,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mood_changes_tween_and_frames_are_pure_in_now() {
        let mut f = Face::new();
        f.set_seed(42);
        f.tick(Mood::Content, 0.0);
        f.tick(Mood::Sad, 1.0);
        let at = f.frame(1.21);
        assert!(
            at.e.lift > 0.5 && at.e.lift < 7.5,
            "half way to sad: {}",
            at.e.lift
        );
        assert_eq!(at.mood, Mood::Sad);
        assert_eq!(f.frame(1.21), at, "the same instant gives the same frame");
        f.tick(Mood::Sad, 2.0);
        assert!((f.frame(2.0).e.lift - 8.0).abs() < 1e-4);
        f.tick(Mood::Content, 3.0);
        assert!((f.frame(3.42).e.lift).abs() < 1e-4, "back after 0.42 s");
    }
}

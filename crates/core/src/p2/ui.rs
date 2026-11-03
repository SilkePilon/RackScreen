//! Per-screen widget state: glides, rolls, marquees and colour fades, keyed by
//! a static name plus an index. The run loop owns one per physical screen and
//! it resets whenever the screen's role changes.

use std::collections::HashMap;

use crate::anim::{Easing, Secs, Smooth};
use crate::p2::marquee::Marquee;
use crate::p2::roll::Roll;
use crate::theme::{Color, Role};

pub type UiKey = (&'static str, u32);

/// Data steps glide over this long, ease-out cubic (`Smooth`'s easing).
pub const GLIDE_SECS: Secs = 0.65;
/// A segment's colour cross-fades over this long.
pub const FADE_SECS: Secs = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorFade {
    cur: Color,
    prev: Option<Color>,
    start: Secs,
}

impl ColorFade {
    /// The layers to draw, bottom first: `(colour, alpha)`.
    pub fn layers(&self, now: Secs) -> Vec<(Color, f32)> {
        match self.prev {
            Some(p) if now < self.start => vec![(p, 1.0)],
            Some(p) if now < self.start + FADE_SECS => {
                let e = Easing::OutCubic.apply(((now - self.start) / FADE_SECS) as f32);
                vec![(p, 1.0 - e), (self.cur, e)]
            }
            _ => vec![(self.cur, 1.0)],
        }
    }
}

#[derive(Debug, Default)]
pub struct ScreenUi {
    role: Option<Role>,
    glides: HashMap<UiKey, Smooth>,
    rolls: HashMap<UiKey, Roll>,
    marquees: HashMap<UiKey, Marquee>,
    fades: HashMap<UiKey, ColorFade>,
    /// When each key was first asked for (`since`).
    starts: HashMap<UiKey, Secs>,
    /// The outgoing role's state while a transition runs.
    out: Option<Box<ScreenUi>>,
}

impl ScreenUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// The role this state belongs to; a different role starts from scratch so
    /// nothing rolls or glides across a role change.
    pub fn for_role(&mut self, role: Role) {
        if self.role != Some(role) {
            let out = self.out.take();
            *self = Self::default();
            self.role = Some(role);
            self.out = out;
        }
    }

    /// State for the outgoing half of a transition away from `from`. The first call
    /// moves this screen's state (which belongs to `from`) into the slot, so
    /// the incoming role starts fresh here and the old one keeps animating.
    pub fn outgoing(&mut self, from: Role) -> &mut ScreenUi {
        if self.out.as_ref().and_then(|o| o.role) != Some(from) {
            let mut prev = std::mem::take(self);
            prev.out = None;
            if prev.role != Some(from) {
                prev = ScreenUi::default();
            }
            self.out = Some(Box::new(prev));
        }
        self.out.as_mut().expect("set above")
    }

    /// The transition is over: drop the outgoing state.
    pub fn end_transition(&mut self) {
        self.out = None;
    }

    pub fn role(&self) -> Option<Role> {
        self.role
    }

    /// The first `now` this key was asked for under the current role: a
    /// clock that starts when something first shows.
    pub fn since(&mut self, key: UiKey, now: Secs) -> Secs {
        *self.starts.entry(key).or_insert(now)
    }

    pub fn glide(&mut self, key: UiKey, target: f32, now: Secs) -> f32 {
        self.glide_with(key, target, now, GLIDE_SECS)
    }

    /// First sight of a key shows `target` at once; later changes ease over `duration`.
    pub fn glide_with(&mut self, key: UiKey, target: f32, now: Secs, duration: Secs) -> f32 {
        let s = self
            .glides
            .entry(key)
            .or_insert_with(|| Smooth::new(target, duration));
        s.set(target, now);
        s.value(now)
    }

    /// Like `glide_with` for an angle in degrees: always the short way round.
    /// The returned value is unwrapped (may leave 0..360); callers take `rem_euclid(360)`.
    pub fn glide_angle(&mut self, key: UiKey, target_deg: f32, now: Secs, duration: Secs) -> f32 {
        match self.glides.get_mut(&key) {
            None => {
                self.glides.insert(key, Smooth::new(target_deg, duration));
                target_deg
            }
            Some(s) => {
                let mut delta = (target_deg - s.target()).rem_euclid(360.0);
                if delta > 180.0 {
                    delta -= 360.0;
                }
                s.set(s.target() + delta, now);
                s.value(now)
            }
        }
    }

    pub fn roll(&mut self, key: UiKey, text: &str, now: Secs) -> &Roll {
        let r = self
            .rolls
            .entry(key)
            .or_insert_with(|| Roll::new(text, now));
        r.set(text, now);
        r
    }

    pub fn marquee(&mut self, key: UiKey, text: &str, now: Secs) -> &Marquee {
        let m = self
            .marquees
            .entry(key)
            .or_insert_with(|| Marquee::new(text, now));
        m.set(text, now);
        m
    }

    /// Cross-fade a keyed colour; a change starts `delay` after `now` (the
    /// per-segment stagger). Returns the layers to draw, bottom first.
    pub fn fade(&mut self, key: UiKey, color: Color, now: Secs, delay: Secs) -> Vec<(Color, f32)> {
        let f = self.fades.entry(key).or_insert(ColorFade {
            cur: color,
            prev: None,
            start: now - FADE_SECS,
        });
        if f.cur != color {
            f.prev = Some(f.cur);
            f.cur = color;
            f.start = now + delay;
        }
        f.layers(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{BLACK, WHITE};

    #[test]
    fn glides_start_at_their_target_and_ease_to_new_ones() {
        let mut ui = ScreenUi::new();
        assert_eq!(
            ui.glide(("ring", 0), 0.4, 0.0),
            0.4,
            "no glide from zero on first sight"
        );
        assert_eq!(ui.glide(("ring", 0), 0.4, 0.1), 0.4);
        let mid = ui.glide(("ring", 0), 0.8, 1.0);
        assert_eq!(mid, 0.4, "retarget starts where it is");
        let later = ui.glide(("ring", 0), 0.8, 1.3);
        assert!(later > 0.4 && later < 0.8, "{later}");
        assert!(
            (ui.glide(("ring", 0), 0.8, 1.7) - 0.8).abs() < 1e-5,
            "done after 0.65 s"
        );
        assert_eq!(
            ui.glide(("ring", 1), 0.1, 1.7),
            0.1,
            "another key is independent"
        );
    }

    #[test]
    fn angle_glides_take_the_short_way_round() {
        let mut ui = ScreenUi::new();
        assert_eq!(ui.glide_angle(("rot", 0), 350.0, 0.0, 1.0), 350.0);
        let v = ui.glide_angle(("rot", 0), 10.0, 1.0, 1.0);
        assert_eq!(v, 350.0);
        let v = ui.glide_angle(("rot", 0), 10.0, 1.5, 1.0);
        assert!(v > 350.0 && v < 370.0, "passes through 360: {v}");
        let v = ui.glide_angle(("rot", 0), 10.0, 2.5, 1.0);
        assert!((v.rem_euclid(360.0) - 10.0).abs() < 1e-3, "{v}");
    }

    #[test]
    fn rolls_marquees_and_fades_are_keyed_and_reset_on_role_change() {
        let mut ui = ScreenUi::new();
        assert_eq!(ui.roll(("hero", 0), "42", 0.0).text(), "42");
        ui.roll(("hero", 0), "43", 1.0);
        assert!(ui.roll(("hero", 0), "43", 1.1).rolling(1.1));
        assert_eq!(ui.marquee(("pill", 0), "abc", 0.0).text(), "abc");
        let first = ui.fade(("seg", 3), WHITE, 0.0, 0.0);
        assert_eq!(first, vec![(WHITE, 1.0)]);
        let layers = ui.fade(("seg", 3), BLACK, 1.0, 0.06);
        assert_eq!(
            layers,
            vec![(WHITE, 1.0)],
            "before its staggered start the old colour shows"
        );
        let layers = ui.fade(("seg", 3), BLACK, 1.3, 0.06);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].0, WHITE);
        assert_eq!(layers[1].0, BLACK);
        assert!((layers[0].1 + layers[1].1 - 1.0).abs() < 1e-5);
        assert_eq!(ui.fade(("seg", 3), BLACK, 2.0, 0.06), vec![(BLACK, 1.0)]);
        ui.for_role(Role::Cpu);
        assert_eq!(ui.role(), Some(Role::Cpu));
        ui.glide(("ring", 0), 0.5, 0.0);
        ui.for_role(Role::Cpu);
        assert_eq!(
            ui.glide(("ring", 0), 0.9, 0.0),
            0.5,
            "same role keeps state"
        );
        ui.for_role(Role::Mem);
        assert_eq!(
            ui.glide(("ring", 0), 0.9, 0.0),
            0.9,
            "new role starts fresh"
        );
    }
    #[test]
    fn outgoing_keeps_the_old_roles_state_while_the_new_one_starts_fresh() {
        let mut ui = ScreenUi::new();
        ui.for_role(Role::Cpu);
        ui.glide(("x", 0), 10.0, 0.0);
        ui.glide(("x", 0), 50.0, 1.0);
        let mid = ui.outgoing(Role::Cpu).glide(("x", 0), 50.0, 1.2);
        assert!(
            mid > 10.0 && mid < 50.0,
            "the cpu glide carries on in the outgoing slot"
        );
        assert_eq!(ui.role(), None);
        ui.for_role(Role::Mem);
        assert_eq!(
            ui.outgoing(Role::Cpu).role(),
            Some(Role::Cpu),
            "for_role leaves the slot alone"
        );
        ui.end_transition();
        assert_eq!(
            ui.outgoing(Role::Cpu).role(),
            None,
            "after the transition the slot starts empty"
        );
    }

    #[test]
    fn since_keeps_the_first_sight_until_the_role_changes() {
        let mut ui = ScreenUi::new();
        ui.for_role(Role::Gif(0));
        assert_eq!(ui.since(("gif", 0), 3.0), 3.0);
        assert_eq!(ui.since(("gif", 0), 7.0), 3.0);
        ui.for_role(Role::Gif(1));
        assert_eq!(ui.since(("gif", 0), 9.0), 9.0, "a new role starts over");
    }
}

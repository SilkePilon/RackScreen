//! Per-physical-screen role list, cycling and the slide transition.

use crate::anim::{Easing, Secs};
use crate::scene::Placement;
use crate::theme::Role;

pub const TRANSITION_SECS: Secs = 0.75;
/// Quiet time between two serialised transitions, so the SPI bus is idle for a
/// moment before the next screen starts pushing full frames again.
pub const TRANSITION_GAP_SECS: Secs = 0.5;
pub const MIN_CYCLE_SECS: Secs = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: Role,
    pub to: Role,
    /// 0..1 over the whole transition; < 0.5 is the outgoing half.
    pub t: f32,
}

#[derive(Clone, Debug)]
pub struct ScreenState {
    roles: Vec<Role>,
    cycle_secs: Secs,
    current: usize,
    since: Secs,
    transition_started: Option<Secs>,
}

impl ScreenState {
    pub fn new(roles: Vec<Role>, cycle_secs: Secs) -> ScreenState {
        ScreenState::new_with_offset(roles, cycle_secs, 0.0)
    }

    /// Like `new`, but the first dwell lasts `offset` seconds longer, so screens
    /// with equal dwells do not iris on the same frame. The offset is clamped to
    /// one dwell, so no screen waits more than two dwells for its first change.
    pub fn new_with_offset(roles: Vec<Role>, cycle_secs: Secs, offset: Secs) -> ScreenState {
        let roles = if roles.is_empty() {
            vec![Role::Cpu]
        } else {
            roles
        };
        let cycle_secs = cycle_secs.max(MIN_CYCLE_SECS);
        ScreenState {
            roles,
            cycle_secs,
            current: 0,
            since: offset.clamp(0.0, cycle_secs),
            transition_started: None,
        }
    }

    pub fn set_roles(&mut self, roles: Vec<Role>, cycle_secs: Secs) {
        *self = ScreenState::new(roles, cycle_secs);
    }

    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// The role being shown (during a transition: the outgoing one until t >= 0.5).
    pub fn current(&self) -> Role {
        self.roles[self.current]
    }

    pub fn next_role(&self) -> Role {
        self.roles[(self.current + 1) % self.roles.len()]
    }

    /// Advance timers and start the next transition once the dwell is over.
    pub fn tick(&mut self, now: Secs) {
        self.advance(now);
        if self.ready_to_transition(now) {
            self.start_transition(now);
        }
    }

    /// Finish a running transition once its time is up; never start one. The
    /// caller that serialises transitions drives screens with this plus
    /// `ready_to_transition`/`start_transition`.
    pub fn advance(&mut self, now: Secs) {
        if self.roles.len() < 2 {
            return;
        }
        if let Some(t0) = self.transition_started {
            if now - t0 >= TRANSITION_SECS {
                self.current = (self.current + 1) % self.roles.len();
                self.since = now;
                self.transition_started = None;
            }
        }
    }

    /// The dwell has elapsed and nothing is in the way, so this screen would
    /// iris now if it were allowed to.
    pub fn ready_to_transition(&self, now: Secs) -> bool {
        self.roles.len() >= 2
            && self.transition_started.is_none()
            && now - self.since >= self.cycle_secs
    }

    /// Begin the iris. Ignored for a static screen or one already transitioning.
    pub fn start_transition(&mut self, now: Secs) {
        if self.roles.len() >= 2 && self.transition_started.is_none() {
            self.transition_started = Some(now);
        }
    }

    pub fn transitioning(&self) -> bool {
        self.transition_started.is_some()
    }

    #[cfg(test)]
    pub(crate) fn set_since(&mut self, since: Secs) {
        self.since = since;
    }

    pub fn transition(&self, now: Secs) -> Option<Transition> {
        let t0 = self.transition_started?;
        let t = ((now - t0) / TRANSITION_SECS).clamp(0.0, 1.0) as f32;
        Some(Transition {
            from: self.current(),
            to: self.next_role(),
            t,
        })
    }
}

/// Placement of the outgoing and incoming scene at `t ∈ 0..1`: the old one
/// moves up and out, the new one comes in from below, both a touch smaller
/// mid-move.
pub fn slide(t: f32) -> (Placement, Placement) {
    let t = t.clamp(0.0, 1.0);
    let e = Easing::InOutCubic.apply(t);
    let s = 1.0 - 0.08 * (std::f32::consts::PI * t).sin();
    (
        Placement {
            dy: -240.0 * e,
            scale: s,
            alpha: 1.0 - 0.6 * e,
        },
        Placement {
            dy: 240.0 * (1.0 - e),
            scale: s,
            alpha: 0.4 + 0.6 * e,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_screen_never_transitions() {
        let mut s = ScreenState::new(vec![Role::Cpu], 3.0);
        s.tick(100.0);
        assert!(s.transition(100.0).is_none());
        assert_eq!(s.current(), Role::Cpu);
    }

    #[test]
    fn empty_roles_fall_back_to_cpu() {
        let s = ScreenState::new(vec![], 3.0);
        assert_eq!(s.roles(), &[Role::Cpu]);
    }

    #[test]
    fn cycles_after_dwell() {
        let mut s = ScreenState::new(vec![Role::Cpu, Role::Thermal], 5.0);
        s.tick(4.9);
        assert!(s.transition(4.9).is_none());
        s.tick(5.1);
        let tr = s.transition(5.2).unwrap();
        assert_eq!((tr.from, tr.to), (Role::Cpu, Role::Thermal));
        assert!(tr.t > 0.0 && tr.t < 0.5);
        s.tick(5.9);
        assert!(s.transition(5.9).is_none());
        assert_eq!(s.current(), Role::Thermal);
        s.tick(11.0);
        assert_eq!(s.transition(11.0).unwrap().to, Role::Cpu, "wraps around");
    }

    /// Tick two screens at 30 Hz and return the tick index at which each one's
    /// first transition started.
    fn first_transition_ticks(a: &mut ScreenState, b: &mut ScreenState) -> (usize, usize) {
        let (mut ta, mut tb) = (None, None);
        for i in 0..600 {
            let now = i as f64 / 30.0;
            a.tick(now);
            b.tick(now);
            if ta.is_none() && a.transition(now).is_some() {
                ta = Some(i);
            }
            if tb.is_none() && b.transition(now).is_some() {
                tb = Some(i);
            }
            if ta.is_some() && tb.is_some() {
                break;
            }
        }
        (ta.expect("a transitions"), tb.expect("b transitions"))
    }

    #[test]
    fn offset_screens_do_not_iris_in_lockstep() {
        let roles = vec![Role::Cpu, Role::Mem];
        let mut a = ScreenState::new_with_offset(roles.clone(), 5.0, 0.0);
        let mut b = ScreenState::new_with_offset(roles.clone(), 5.0, 2.5);
        let (ta, tb) = first_transition_ticks(&mut a, &mut b);
        assert_ne!(ta, tb, "different offsets, different ticks");
        assert_eq!(tb - ta, 75, "2.5 s later at 30 Hz");
        // equal offsets (the old behaviour) do start on the same tick
        let mut c = ScreenState::new(roles.clone(), 5.0);
        let mut d = ScreenState::new(roles.clone(), 5.0);
        let (tc, td) = first_transition_ticks(&mut c, &mut d);
        assert_eq!(tc, td);
        // an offset longer than the dwell is clamped, so the wait stays bounded
        let e = ScreenState::new_with_offset(roles, 3.0, 5.0);
        assert!(
            (e.since - 3.0).abs() < 1e-9,
            "clamped to one dwell, got {}",
            e.since
        );
    }

    #[test]
    fn cycle_floor() {
        let s = ScreenState::new(vec![Role::Cpu, Role::Mem], 1.0);
        assert_eq!(s.cycle_secs, MIN_CYCLE_SECS);
    }
    #[test]
    fn slide_moves_old_up_and_new_in_from_below() {
        let (o, i) = slide(0.0);
        assert_eq!((o.dy, o.scale, o.alpha), (0.0, 1.0, 1.0));
        assert!((i.dy - 240.0).abs() < 1e-3 && (i.alpha - 0.4).abs() < 1e-6);
        let (o, i) = slide(0.5);
        assert!((o.dy + 120.0).abs() < 1e-3 && (i.dy - 120.0).abs() < 1e-3);
        assert!((o.scale - 0.92).abs() < 1e-4 && (i.scale - 0.92).abs() < 1e-4);
        assert!((o.alpha - 0.7).abs() < 1e-4 && (i.alpha - 0.7).abs() < 1e-4);
        let (o, i) = slide(1.0);
        assert!((o.dy + 240.0).abs() < 1e-3);
        assert_eq!((i.dy, i.scale, i.alpha), (0.0, 1.0, 1.0));
    }
}

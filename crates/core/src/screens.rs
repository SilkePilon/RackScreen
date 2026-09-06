//! Per-physical-screen role list, cycling and the iris transition.

use crate::anim::{Easing, Secs};
use crate::theme::Role;

pub const TRANSITION_SECS: Secs = 0.5;
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
        let roles = if roles.is_empty() {
            vec![Role::Cpu]
        } else {
            roles
        };
        ScreenState {
            roles,
            cycle_secs: cycle_secs.max(MIN_CYCLE_SECS),
            current: 0,
            since: 0.0,
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

    /// Advance timers. `busy` = a splash or sweep is active on this screen; the
    /// transition is postponed while busy.
    pub fn tick(&mut self, now: Secs, busy: bool) {
        if self.roles.len() < 2 {
            return;
        }
        match self.transition_started {
            Some(t0) => {
                if now - t0 >= TRANSITION_SECS {
                    self.current = (self.current + 1) % self.roles.len();
                    self.since = now;
                    self.transition_started = None;
                }
            }
            None => {
                if !busy && now - self.since >= self.cycle_secs {
                    self.transition_started = Some(now);
                }
            }
        }
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

/// Zoom (scale about the centre) and ring reveal fraction for a transition instant.
/// First half: outgoing scene shrinks with ease-in and its ring unlights backwards.
/// Second half: incoming scene grows with ease-out and its ring lights clockwise.
pub fn transition_transform(t: f32) -> (f32, f32) {
    if t < 0.5 {
        let p = t / 0.5;
        let e = 1.0 - Easing::OutCubic.apply(1.0 - p); // ease-in
        (1.0 - e, 1.0 - e)
    } else {
        let p = (t - 0.5) / 0.5;
        let e = Easing::OutCubic.apply(p);
        (e, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_screen_never_transitions() {
        let mut s = ScreenState::new(vec![Role::Cpu], 3.0);
        s.tick(100.0, false);
        assert!(s.transition(100.0).is_none());
        assert_eq!(s.current(), Role::Cpu);
    }

    #[test]
    fn empty_roles_fall_back_to_cpu() {
        let s = ScreenState::new(vec![], 3.0);
        assert_eq!(s.roles(), &[Role::Cpu]);
    }

    #[test]
    fn cycles_after_dwell_and_waits_while_busy() {
        let mut s = ScreenState::new(vec![Role::Cpu, Role::Thermal], 5.0);
        s.tick(4.9, false);
        assert!(s.transition(4.9).is_none());
        s.tick(5.0, true);
        assert!(s.transition(5.0).is_none(), "busy postpones");
        s.tick(5.1, false);
        let tr = s.transition(5.2).unwrap();
        assert_eq!((tr.from, tr.to), (Role::Cpu, Role::Thermal));
        assert!(tr.t > 0.0 && tr.t < 0.5);
        s.tick(5.7, false);
        assert!(s.transition(5.7).is_none());
        assert_eq!(s.current(), Role::Thermal);
        s.tick(10.8, false);
        assert_eq!(s.transition(10.8).unwrap().to, Role::Cpu, "wraps around");
    }

    #[test]
    fn cycle_floor_and_transform_shape() {
        let s = ScreenState::new(vec![Role::Cpu, Role::Mem], 1.0);
        assert_eq!(s.cycle_secs, MIN_CYCLE_SECS);
        assert_eq!(transition_transform(0.0), (1.0, 1.0));
        let (z, r) = transition_transform(0.49);
        assert!(z < 0.1 && r < 0.1);
        assert_eq!(transition_transform(0.5), (0.0, 0.0));
        assert_eq!(transition_transform(1.0), (1.0, 1.0));
    }
}

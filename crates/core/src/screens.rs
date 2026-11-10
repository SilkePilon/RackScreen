//! Per-physical-screen role list, cycling and the crossfade transition.

use crate::anim::{Easing, Secs};
use crate::theme::Role;
use crate::transition::{Kind, Spec};

/// How long a transition takes unless the config says otherwise.
pub const TRANSITION_SECS: Secs = 0.4;
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
    pub kind: Kind,
}

/// One step of a screen's cycle: a role and how long it stays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub role: Role,
    /// The dwell; a `ScreenState` raises it to `MIN_CYCLE_SECS`.
    pub secs: Secs,
}

impl Entry {
    pub fn new(role: Role, secs: Secs) -> Entry {
        Entry { role, secs }
    }
}

#[derive(Clone, Debug)]
pub struct ScreenState {
    entries: Vec<Entry>,
    current: usize,
    /// The entry the running transition goes to, picked when it started.
    target: usize,
    since: Secs,
    transition_started: Option<Secs>,
    /// The running transition's style and length, or the next one's.
    spec: Spec,
}

impl ScreenState {
    pub fn new(entries: Vec<Entry>) -> ScreenState {
        ScreenState::new_with_offset(entries, 0.0)
    }

    /// Like `new`, but the first dwell lasts `offset` seconds longer, so screens
    /// with equal dwells do not transition on the same frame. The offset is clamped to
    /// the first dwell, so no screen waits more than two dwells for its first change.
    pub fn new_with_offset(entries: Vec<Entry>, offset: Secs) -> ScreenState {
        let mut entries = if entries.is_empty() {
            vec![Entry::new(Role::Cpu, MIN_CYCLE_SECS)]
        } else {
            entries
        };
        for e in &mut entries {
            e.secs = e.secs.max(MIN_CYCLE_SECS);
        }
        let first = entries[0].secs;
        ScreenState {
            entries,
            current: 0,
            target: 0,
            since: offset.clamp(0.0, first),
            transition_started: None,
            spec: Spec::default(),
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The current role; during a transition the outgoing one until the
    /// transition ends (the panel already shows the incoming one from t = 0.5).
    pub fn current(&self) -> Role {
        self.entries[self.current].role
    }

    /// The entry a transition starting now would go to: the first one after
    /// the current entry whose role `can_show`. `None` on a static screen or
    /// when no other entry can show.
    pub fn next_entry(&self, can_show: impl Fn(Role) -> bool) -> Option<usize> {
        let n = self.entries.len();
        (1..n)
            .map(|k| (self.current + k) % n)
            .find(|&i| can_show(self.entries[i].role))
    }

    /// The current role cannot show but another entry can, so the screen
    /// should leave it now (`request_leave`).
    pub fn stranded(&self, can_show: impl Fn(Role) -> bool) -> bool {
        !can_show(self.current()) && self.next_entry(can_show).is_some()
    }

    /// Advance timers and start the next transition once the dwell is over.
    pub fn tick(&mut self, now: Secs, can_show: impl Fn(Role) -> bool) {
        self.advance(now);
        if self.ready_to_transition(now) {
            self.start_transition_to(now, self.spec, self.next_entry(can_show));
        }
    }

    /// Finish a running transition once its time is up; never start one. The
    /// caller that serialises transitions drives screens with this plus
    /// `ready_to_transition`/`start_transition_to`.
    pub fn advance(&mut self, now: Secs) {
        if let Some(t0) = self.transition_started {
            if now - t0 >= self.spec.secs {
                self.current = self.target;
                self.since = now;
                self.transition_started = None;
            }
        }
    }

    /// The dwell has elapsed and nothing is in the way, so this screen would
    /// transition now if it were allowed to.
    pub fn ready_to_transition(&self, now: Secs) -> bool {
        self.entries.len() >= 2
            && self.transition_started.is_none()
            && now - self.since >= self.cycle_secs()
    }

    /// Begin the transition to entry `to` (from `next_entry`) in style
    /// `spec`, and say whether it started. With nowhere to go the current role
    /// stays and its dwell restarts. Ignored for a static screen or one
    /// already transitioning.
    pub fn start_transition_to(&mut self, now: Secs, spec: Spec, to: Option<usize>) -> bool {
        if self.entries.len() < 2 || self.transition_started.is_some() {
            return false;
        }
        match to.filter(|&i| i < self.entries.len()) {
            Some(i) => {
                self.target = i;
                self.transition_started = Some(now);
                self.spec = spec;
                true
            }
            None => {
                self.since = now;
                false
            }
        }
    }

    /// The running transition's spec (or the last one's).
    pub fn spec(&self) -> Spec {
        self.spec
    }

    /// Make the dwell run out `delay` seconds from `now`, so the screen
    /// transitions then (or takes the next free slot).
    pub fn make_due(&mut self, now: Secs, delay: Secs) {
        if self.entries.len() >= 2 && self.transition_started.is_none() {
            self.since = now + delay - self.cycle_secs();
        }
    }

    /// The current role asks to go now (it has nothing to show any more);
    /// the change still waits for a free transition slot.
    pub fn request_leave(&mut self, now: Secs) {
        self.make_due(now, 0.0);
    }

    /// Push the dwell back by `secs` (a show is playing for that long), and
    /// leave at least `MIN_CYCLE_SECS` of it for after, so no role change
    /// fires right as the roles fade back in.
    pub fn extend_dwell(&mut self, now: Secs, secs: Secs) {
        self.since = self.since.max(now - self.cycle_secs() + MIN_CYCLE_SECS) + secs;
    }

    /// How far the current dwell has run, 0..=1; a static screen stays at 0.
    pub fn progress(&self, now: Secs) -> f64 {
        if self.entries.len() < 2 {
            return 0.0;
        }
        ((now - self.since) / self.cycle_secs()).clamp(0.0, 1.0)
    }

    /// The current entry's dwell.
    pub fn cycle_secs(&self) -> Secs {
        self.entries[self.current].secs
    }

    /// The role the next transition goes to if it started now, and the
    /// seconds until the dwell ends; during a transition, its incoming role
    /// and zero. `None` when there is nothing to change to. Lets a role warm
    /// up before its slot.
    pub fn upcoming(&self, now: Secs, can_show: impl Fn(Role) -> bool) -> Option<(Role, Secs)> {
        if self.transition_started.is_some() {
            return Some((self.entries[self.target].role, 0.0));
        }
        let to = self.next_entry(can_show)?;
        let left = (self.since + self.cycle_secs() - now).max(0.0);
        Some((self.entries[to].role, left))
    }

    /// Show `role` now (it must be in the list) and start its dwell afresh;
    /// a running transition is dropped. Returns whether the role was found.
    pub fn jump_to(&mut self, role: Role, now: Secs) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.role == role) else {
            return false;
        };
        self.current = i;
        self.since = now;
        self.transition_started = None;
        true
    }

    /// When the running transition began, if one is running.
    pub fn transition_started(&self) -> Option<Secs> {
        self.transition_started
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
        let t = ((now - t0) / self.spec.secs).clamp(0.0, 1.0) as f32;
        Some(Transition {
            from: self.current(),
            to: self.entries[self.target].role,
            t,
            kind: self.spec.kind,
        })
    }
}

/// Which role a transition frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Out,
    In,
}

/// Crossfade via black at `t ∈ 0..1`: the outgoing role fades to black over
/// the first half, the incoming one fades in over the second. Brightness is
/// quantised to 1/64 so equal frames are skipped by the render workers.
pub fn fade(t: f32) -> (Half, f32) {
    let t = t.clamp(0.0, 1.0);
    let q = |b: f32| (b * 64.0).round() / 64.0;
    if t < 0.5 {
        (Half::Out, q(1.0 - Easing::InOutCubic.apply(2.0 * t)))
    } else {
        (Half::In, q(Easing::InOutCubic.apply(2.0 * t - 1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every role in `roles` with the same dwell.
    fn each(roles: &[Role], secs: Secs) -> Vec<Entry> {
        roles.iter().map(|&r| Entry::new(r, secs)).collect()
    }

    fn all(_: Role) -> bool {
        true
    }

    #[test]
    fn static_screen_never_transitions() {
        let mut s = ScreenState::new(each(&[Role::Cpu], 3.0));
        s.tick(100.0, all);
        assert!(s.transition(100.0).is_none());
        assert_eq!(s.current(), Role::Cpu);
    }

    #[test]
    fn empty_roles_fall_back_to_cpu() {
        let s = ScreenState::new(each(&[], 3.0));
        assert_eq!(s.entries(), &[Entry::new(Role::Cpu, MIN_CYCLE_SECS)]);
    }

    #[test]
    fn cycles_after_dwell() {
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Thermal], 5.0));
        s.tick(4.9, all);
        assert!(s.transition(4.9).is_none());
        s.tick(5.1, all);
        let tr = s.transition(5.2).unwrap();
        assert_eq!((tr.from, tr.to), (Role::Cpu, Role::Thermal));
        assert!(tr.t > 0.0 && tr.t < 0.5);
        s.tick(5.9, all);
        assert!(s.transition(5.9).is_none());
        assert_eq!(s.current(), Role::Thermal);
        s.tick(11.0, all);
        assert_eq!(s.transition(11.0).unwrap().to, Role::Cpu, "wraps around");
    }

    /// Tick two screens at 30 Hz and return the tick index at which each one's
    /// first transition started.
    fn first_transition_ticks(a: &mut ScreenState, b: &mut ScreenState) -> (usize, usize) {
        let (mut ta, mut tb) = (None, None);
        for i in 0..600 {
            let now = i as f64 / 30.0;
            a.tick(now, all);
            b.tick(now, all);
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
    fn offset_screens_do_not_transition_in_lockstep() {
        let roles = vec![Role::Cpu, Role::Mem];
        let mut a = ScreenState::new_with_offset(each(&roles, 5.0), 0.0);
        let mut b = ScreenState::new_with_offset(each(&roles, 5.0), 2.5);
        let (ta, tb) = first_transition_ticks(&mut a, &mut b);
        assert_ne!(ta, tb, "different offsets, different ticks");
        assert_eq!(tb - ta, 75, "2.5 s later at 30 Hz");
        // equal offsets (the old behaviour) do start on the same tick
        let mut c = ScreenState::new(each(&roles, 5.0));
        let mut d = ScreenState::new(each(&roles, 5.0));
        let (tc, td) = first_transition_ticks(&mut c, &mut d);
        assert_eq!(tc, td);
        // an offset longer than the dwell is clamped, so the wait stays bounded
        let e = ScreenState::new_with_offset(each(&roles, 3.0), 5.0);
        assert!(
            (e.since - 3.0).abs() < 1e-9,
            "clamped to one dwell, got {}",
            e.since
        );
    }

    #[test]
    fn cycle_floor() {
        let s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 1.0));
        assert_eq!(s.cycle_secs(), MIN_CYCLE_SECS);
    }
    #[test]
    fn fade_goes_out_to_black_then_in() {
        assert_eq!(fade(0.0), (Half::Out, 1.0));
        assert_eq!(fade(0.5), (Half::In, 0.0));
        assert_eq!(fade(1.0), (Half::In, 1.0));
        let (h, b) = fade(0.25);
        assert_eq!(h, Half::Out);
        assert!(b > 0.0 && b < 1.0);
        // monotonic within each half, quantised to 1/64
        let mut last = 2.0;
        for i in 0..50 {
            let (h, b) = fade(i as f32 / 100.0);
            assert_eq!(h, Half::Out);
            assert!(b <= last);
            assert_eq!((b * 64.0).fract(), 0.0);
            last = b;
        }
        let mut last = -1.0;
        for i in 50..=100 {
            let (h, b) = fade(i as f32 / 100.0);
            assert_eq!(h, Half::In);
            assert!(b >= last);
            last = b;
        }
    }

    #[test]
    fn extend_dwell_pushes_the_next_change_past_the_show() {
        let roles = vec![Role::Cpu, Role::Mem];
        // 5 s of a 15 s dwell left: still 5 s left once a 10 s show is over
        let mut s = ScreenState::new(each(&roles, 15.0));
        s.extend_dwell(10.0, 10.0);
        assert!(!s.ready_to_transition(24.9));
        assert!(s.ready_to_transition(25.0));
        // overdue (waiting for the slot): at least the minimum dwell after the show
        let mut s = ScreenState::new(each(&roles, 15.0));
        s.extend_dwell(40.0, 10.0);
        assert!(!s.ready_to_transition(50.0 + MIN_CYCLE_SECS - 0.01));
        assert!(s.ready_to_transition(50.0 + MIN_CYCLE_SECS));
    }

    #[test]
    fn transition_lasts_four_tenths() {
        assert_eq!(TRANSITION_SECS, 0.4);
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 5.0));
        s.tick(5.0, all);
        assert_eq!(s.transition(5.0).unwrap().kind, Kind::default());
        s.tick(5.39, all);
        assert!(s.transitioning());
        s.tick(5.4, all);
        assert!(!s.transitioning());
    }

    #[test]
    fn a_spec_sets_the_style_and_the_length() {
        use crate::transition::Style;
        let spec = Spec {
            kind: Kind::of(Style::Slide),
            secs: 1.0,
        };
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 5.0));
        s.start_transition_to(5.0, spec, s.next_entry(all));
        let tr = s.transition(5.5).unwrap();
        assert_eq!(tr.kind, Kind::of(Style::Slide));
        assert!((tr.t - 0.5).abs() < 1e-6);
        s.advance(5.99);
        assert!(s.transitioning(), "a second long");
        s.advance(6.0);
        assert!(!s.transitioning());
        assert_eq!(s.current(), Role::Mem);
        assert_eq!(s.spec(), spec, "kept for the next one");
        // the spec of a running transition is not replaced
        s.start_transition_to(6.0, spec, s.next_entry(all));
        s.start_transition_to(6.0, Spec::default(), s.next_entry(all));
        assert_eq!(s.transition(6.5).unwrap().kind, Kind::of(Style::Slide));
    }

    #[test]
    fn make_due_ends_the_dwell_early() {
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 30.0));
        assert!(!s.ready_to_transition(1.0));
        s.make_due(1.0, 0.0);
        assert!(s.ready_to_transition(1.0));
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 30.0));
        s.make_due(1.0, 0.5);
        assert!(!s.ready_to_transition(1.4));
        assert!(s.ready_to_transition(1.5));
        // a static screen and a transitioning one are left alone
        let mut one = ScreenState::new(each(&[Role::Cpu], 30.0));
        one.make_due(1.0, 0.0);
        assert!(!one.ready_to_transition(1.0));
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 30.0));
        s.start_transition_to(1.0, s.spec(), s.next_entry(all));
        s.make_due(1.1, 0.0);
        assert!(s.transitioning());
        s.advance(1.5);
        assert!(!s.ready_to_transition(1.6), "the new dwell runs in full");
    }

    #[test]
    fn progress_and_jump() {
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem, Role::Pods], 10.0));
        assert_eq!(s.progress(0.0), 0.0);
        assert!((s.progress(5.0) - 0.5).abs() < 1e-9);
        assert_eq!(s.progress(50.0), 1.0);
        assert_eq!(s.cycle_secs(), 10.0);
        assert!(s.jump_to(Role::Pods, 20.0));
        assert_eq!(s.current(), Role::Pods);
        assert_eq!(s.progress(20.0), 0.0);
        assert!(!s.transitioning());
        assert!(!s.jump_to(Role::Weather, 21.0), "not in the list");
        assert_eq!(s.current(), Role::Pods);
        // a jump drops a running transition
        s.start_transition_to(30.0, s.spec(), s.next_entry(all));
        assert!(s.transitioning());
        assert!(s.jump_to(Role::Cpu, 30.1));
        assert!(!s.transitioning());
        assert_eq!(s.current(), Role::Cpu);
        let single = ScreenState::new(each(&[Role::Cpu], 10.0));
        assert_eq!(single.progress(100.0), 0.0, "a static screen never cycles");
    }

    #[test]
    fn each_entry_keeps_its_own_dwell() {
        let mut s = ScreenState::new(vec![
            Entry::new(Role::Cpu, 5.0),
            Entry::new(Role::Iss, 30.0),
            Entry::new(Role::Mem, 1.0),
        ]);
        assert_eq!(s.entries()[2].secs, MIN_CYCLE_SECS, "floored per entry");
        s.tick(5.0, all);
        s.tick(5.4, all);
        assert_eq!((s.current(), s.cycle_secs()), (Role::Iss, 30.0));
        assert!((s.progress(20.4) - 0.5).abs() < 1e-9);
        s.tick(35.3, all);
        assert!(!s.transitioning(), "30 s, not the first entry's 5");
        s.tick(35.4, all);
        assert_eq!(s.transition(35.4).unwrap().to, Role::Mem);
        s.tick(35.9, all);
        assert_eq!(s.cycle_secs(), MIN_CYCLE_SECS);
        s.make_due(40.0, 1.0);
        assert!(s.ready_to_transition(41.0));
        assert!(!s.ready_to_transition(40.9));
    }

    #[test]
    fn a_role_that_cannot_show_is_skipped() {
        let roles = [Role::Cpu, Role::Iss, Role::Mem];
        let no_iss = |r: Role| r != Role::Iss;
        let mut s = ScreenState::new(each(&roles, 5.0));
        assert_eq!(s.next_entry(no_iss), Some(2));
        s.tick(5.0, no_iss);
        // the target is fixed once the transition runs, whatever can show now
        assert_eq!(s.transition(5.1).unwrap().to, Role::Mem);
        s.tick(5.2, all);
        assert_eq!(s.transition(5.2).unwrap().to, Role::Mem);
        s.tick(5.4, all);
        assert_eq!(s.current(), Role::Mem);
        // wraps past the end
        s.tick(10.4, no_iss);
        assert_eq!(s.transition(10.4).unwrap().to, Role::Cpu);
    }

    #[test]
    fn with_nothing_else_to_show_the_current_role_stays() {
        let only_cpu = |r: Role| r == Role::Cpu;
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Iss, Role::Mem], 5.0));
        assert_eq!(s.next_entry(only_cpu), None);
        s.tick(5.0, only_cpu);
        assert!(!s.transitioning());
        assert_eq!(s.current(), Role::Cpu);
        assert!(!s.ready_to_transition(9.9), "the dwell restarted");
        assert!(s.ready_to_transition(10.0));
        assert!(!s.start_transition_to(10.0, s.spec(), None));
        // the current role itself unavailable too: nowhere to go, so it stays
        let nothing = |_: Role| false;
        assert!(!s.stranded(nothing));
        assert_eq!(s.upcoming(10.0, nothing), None);
    }

    #[test]
    fn request_leave_goes_at_once_when_stranded() {
        let no_cpu = |r: Role| r != Role::Cpu;
        let mut s = ScreenState::new(each(&[Role::Cpu, Role::Mem], 30.0));
        assert!(s.stranded(no_cpu));
        assert!(!s.stranded(all));
        s.request_leave(2.0);
        assert!(s.ready_to_transition(2.0));
        s.tick(2.0, no_cpu);
        assert_eq!(s.transition(2.0).unwrap().to, Role::Mem);
        // a static screen has nowhere to go
        let one = ScreenState::new(each(&[Role::Cpu], 30.0));
        assert!(!one.stranded(no_cpu));
    }

    #[test]
    fn upcoming_names_the_next_role_and_the_wait() {
        let roles = [Role::Cpu, Role::Iss, Role::Mem];
        let mut s = ScreenState::new(each(&roles, 10.0));
        assert_eq!(s.upcoming(2.0, all), Some((Role::Iss, 8.0)));
        assert_eq!(s.upcoming(2.0, |r| r != Role::Iss), Some((Role::Mem, 8.0)));
        assert_eq!(s.upcoming(12.0, all), Some((Role::Iss, 0.0)), "overdue");
        s.tick(10.0, all);
        assert_eq!(s.upcoming(10.1, all), Some((Role::Iss, 0.0)), "on its way");
        let one = ScreenState::new(each(&[Role::Cpu], 10.0));
        assert_eq!(one.upcoming(0.0, all), None);
    }
}

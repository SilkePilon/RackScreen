//! Splash (screen-local) and sweep (rack-wide) animation bookkeeping.

use std::collections::VecDeque;

use crate::anim::Secs;
use crate::model::FxRequest;
use crate::theme::{Color, Role, AMBER, BLUE, GREEN, RED};

pub const SPLASH_SECS: Secs = 2.5;
pub const SPLASH_COLLAPSE_SECS: Secs = 1.0;
pub const SWEEP_WIPE_SECS: Secs = 0.25;
pub const SWEEP_STAGGER_SECS: Secs = 0.12;
pub const SWEEP_HOLD_SECS: Secs = 2.5;
pub const BOOT_HOLD_SECS: Secs = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplashKind {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode,
    TorrentAdded,
}

impl SplashKind {
    pub fn icon(self) -> &'static str {
        match self {
            SplashKind::PodStarted => "package-plus",
            SplashKind::PodCrashed => "package-x",
            SplashKind::PodGone => "package-minus",
            SplashKind::HotNode => "flame",
            SplashKind::TorrentAdded => "download",
        }
    }
    pub fn color(self) -> Color {
        match self {
            SplashKind::PodStarted => BLUE,
            SplashKind::PodCrashed => RED,
            SplashKind::PodGone => BLUE.with_alpha(0.6),
            SplashKind::HotNode => AMBER,
            SplashKind::TorrentAdded => BLUE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Splash {
    pub kind: SplashKind,
    pub role: Role,
    pub started: Secs,
    pub count: u32,
}

impl Splash {
    pub fn elapsed(&self, now: Secs) -> f32 {
        (now - self.started).max(0.0) as f32
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.started >= SPLASH_SECS
    }
}

#[derive(Debug, Default)]
pub struct SplashQueue {
    active: Option<Splash>,
    pending: Option<Splash>,
    last_tick: Option<Secs>,
}

impl SplashQueue {
    pub fn push(&mut self, kind: SplashKind, role: Role, now: Secs) {
        if let Some(a) = &mut self.active {
            if a.kind == kind && now - a.started < SPLASH_COLLAPSE_SECS {
                a.count += 1;
                return;
            }
        }
        if let Some(p) = &mut self.pending {
            if p.kind == kind {
                p.count += 1;
                return;
            }
        }
        let s = Splash {
            kind,
            role,
            started: now,
            count: 1,
        };
        if self.active.is_none() {
            self.active = Some(s);
        } else {
            self.pending = Some(s);
        }
    }

    /// Advance. While `frozen` (a sweep is running) the active splash does not age.
    pub fn tick(&mut self, now: Secs, frozen: bool) {
        let dt = self.last_tick.map_or(0.0, |l| (now - l).max(0.0));
        self.last_tick = Some(now);
        if frozen {
            if let Some(a) = &mut self.active {
                a.started += dt;
            }
            return;
        }
        if self.active.is_some_and(|a| a.done(now)) {
            self.active = self.pending.take().map(|mut p| {
                p.started = now;
                p
            });
        }
    }

    pub fn active(&self) -> Option<&Splash> {
        self.active.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SweepKind {
    TorrentDone,
    NodeNotReady,
    NodeReady,
    AlertFiring,
    AlertResolved,
    LinkUp,
    Boot,
}

impl SweepKind {
    pub fn direction(self) -> Direction {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring => Direction::Up,
            _ => Direction::Down,
        }
    }
    pub fn color(self, role: Role) -> Color {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring => RED,
            SweepKind::Boot => role.accent(),
            _ => GREEN,
        }
    }
    pub fn icon(self, role: Role) -> &'static str {
        match self {
            SweepKind::TorrentDone => "circle-check",
            SweepKind::NodeNotReady => "server-off",
            SweepKind::NodeReady => "server",
            SweepKind::AlertFiring => "triangle-alert",
            SweepKind::AlertResolved => "shield-check",
            SweepKind::LinkUp => "plug",
            SweepKind::Boot => role.icon(),
        }
    }
    pub fn hold(self) -> Secs {
        match self {
            SweepKind::Boot => BOOT_HOLD_SECS,
            _ => SWEEP_HOLD_SECS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SweepPhase {
    Idle,
    WipeIn(f32),
    Hold(f32),
    WipeOut(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    pub kind: SweepKind,
    pub started: Secs,
}

impl Sweep {
    /// Position of a physical screen in the wave: 0 is the origin screen.
    /// Down starts at the top screen (index 0), Up at the bottom (index screens-1).
    pub fn order_index(direction: Direction, screen: usize, screens: usize) -> usize {
        match direction {
            Direction::Down => screen,
            Direction::Up => screens.saturating_sub(1).saturating_sub(screen),
        }
    }

    fn hold_end(&self, screens: usize) -> Secs {
        let last = screens.saturating_sub(1) as f64;
        self.started + last * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS + self.kind.hold()
    }

    pub fn phase(&self, screen: usize, screens: usize, now: Secs) -> SweepPhase {
        let idx = Sweep::order_index(self.kind.direction(), screen, screens) as f64;
        let in_start = self.started + idx * SWEEP_STAGGER_SECS;
        let in_end = in_start + SWEEP_WIPE_SECS;
        let out_start = self.hold_end(screens) + idx * SWEEP_STAGGER_SECS;
        let out_end = out_start + SWEEP_WIPE_SECS;
        if now < in_start || now >= out_end {
            SweepPhase::Idle
        } else if now < in_end {
            SweepPhase::WipeIn(((now - in_start) / SWEEP_WIPE_SECS) as f32)
        } else if now < out_start {
            SweepPhase::Hold(((now - in_end) / (out_start - in_end)) as f32)
        } else {
            SweepPhase::WipeOut(((now - out_start) / SWEEP_WIPE_SECS) as f32)
        }
    }

    pub fn done(&self, screens: usize, now: Secs) -> bool {
        let last = screens.saturating_sub(1) as f64;
        now >= self.hold_end(screens) + last * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS
    }
}

#[derive(Debug, Default)]
pub struct SweepQueue {
    active: Option<Sweep>,
    queue: VecDeque<SweepKind>,
}

impl SweepQueue {
    pub fn push(&mut self, kind: SweepKind) {
        if !self.queue.contains(&kind) {
            self.queue.push_back(kind);
        }
    }
    pub fn tick(&mut self, now: Secs, screens: usize) {
        if self.active.is_some_and(|s| s.done(screens, now)) {
            self.active = None;
        }
        if self.active.is_none() {
            if let Some(kind) = self.queue.pop_front() {
                self.active = Some(Sweep { kind, started: now });
            }
        }
    }
    pub fn active(&self) -> Option<&Sweep> {
        self.active.as_ref()
    }
}

#[derive(Debug)]
pub struct Fx {
    /// One queue per role, indexed by `Role::index()`.
    pub splashes: Vec<SplashQueue>,
    pub sweeps: SweepQueue,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            splashes: (0..Role::ALL.len())
                .map(|_| SplashQueue::default())
                .collect(),
            sweeps: SweepQueue::default(),
        }
    }
}

impl Fx {
    pub fn apply(&mut self, req: FxRequest, now: Secs) {
        let mut splash = |kind: SplashKind, role: Role| {
            self.splashes[role.index()].push(kind, role, now);
        };
        match req {
            FxRequest::PodStarted => splash(SplashKind::PodStarted, Role::Pods),
            FxRequest::PodCrashed => splash(SplashKind::PodCrashed, Role::Pods),
            FxRequest::PodGone => splash(SplashKind::PodGone, Role::Pods),
            FxRequest::HotNode(role) => splash(SplashKind::HotNode, role),
            FxRequest::TorrentAdded => splash(SplashKind::TorrentAdded, Role::Health),
            FxRequest::TorrentDone => self.sweeps.push(SweepKind::TorrentDone),
            FxRequest::NodeNotReady => self.sweeps.push(SweepKind::NodeNotReady),
            FxRequest::NodeReady => self.sweeps.push(SweepKind::NodeReady),
            FxRequest::AlertFiring => self.sweeps.push(SweepKind::AlertFiring),
            FxRequest::AlertResolved => self.sweeps.push(SweepKind::AlertResolved),
            FxRequest::LinkUp => self.sweeps.push(SweepKind::LinkUp),
            FxRequest::Boot => self.sweeps.push(SweepKind::Boot),
        }
    }

    pub fn tick(&mut self, now: Secs, screens: usize) {
        self.sweeps.tick(now, screens);
        let frozen = self.sweeps.active().is_some();
        for q in &mut self.splashes {
            q.tick(now, frozen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_collapses_same_kind_within_a_second() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.push(SplashKind::PodStarted, Role::Pods, 0.5);
        q.push(SplashKind::PodStarted, Role::Pods, 0.9);
        let a = q.active().unwrap();
        assert_eq!(a.count, 3);
        assert_eq!(a.started, 0.0);
    }

    #[test]
    fn splash_queues_one_pending_and_promotes_when_done() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.push(SplashKind::PodCrashed, Role::Pods, 1.5);
        q.push(SplashKind::PodCrashed, Role::Pods, 1.6);
        assert_eq!(q.active().unwrap().kind, SplashKind::PodStarted);
        q.tick(2.0, false);
        assert_eq!(q.active().unwrap().kind, SplashKind::PodStarted);
        q.tick(2.6, false);
        let a = q.active().unwrap();
        assert_eq!(a.kind, SplashKind::PodCrashed);
        assert_eq!(a.count, 2);
        assert_eq!(a.started, 2.6);
        q.tick(5.2, false);
        assert!(q.active().is_none());
    }

    #[test]
    fn frozen_splash_does_not_age() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.tick(0.0, false);
        q.tick(1.0, true);
        q.tick(2.0, true);
        q.tick(3.0, false);
        let a = q.active().expect("still active after being frozen 2 s");
        assert!((a.elapsed(3.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sweep_phases_follow_direction() {
        let s = Sweep {
            kind: SweepKind::NodeNotReady,
            started: 0.0,
        };
        // Up: origin is the bottom screen (index 3), top screen (index 0) is last.
        assert_eq!(Sweep::order_index(Direction::Up, 3, 4), 0);
        assert_eq!(Sweep::order_index(Direction::Up, 0, 4), 3);
        assert_eq!(Sweep::order_index(Direction::Down, 0, 4), 0);
        assert_eq!(Sweep::order_index(Direction::Down, 3, 4), 3);
        assert!(matches!(s.phase(3, 4, 0.1), SweepPhase::WipeIn(_)));
        assert_eq!(s.phase(0, 4, 0.1), SweepPhase::Idle);
        assert!(matches!(s.phase(0, 4, 0.5), SweepPhase::WipeIn(_)));
        assert!(matches!(s.phase(3, 4, 1.5), SweepPhase::Hold(_)));
        assert!(matches!(s.phase(0, 4, 1.5), SweepPhase::Hold(_)));
        let hold_end = 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS + SWEEP_HOLD_SECS;
        assert!(matches!(
            s.phase(3, 4, hold_end + 0.1),
            SweepPhase::WipeOut(_)
        ));
        assert!(matches!(s.phase(0, 4, hold_end + 0.1), SweepPhase::Hold(_)));
        assert!(!s.done(4, hold_end + 0.5));
        assert!(s.done(4, hold_end + 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS));
        assert_eq!(s.phase(0, 4, 10.0), SweepPhase::Idle);
    }

    #[test]
    fn single_screen_sweep_has_no_stagger() {
        let s = Sweep {
            kind: SweepKind::TorrentDone,
            started: 0.0,
        };
        let total = SWEEP_WIPE_SECS + SWEEP_HOLD_SECS + SWEEP_WIPE_SECS;
        assert_eq!(Sweep::order_index(Direction::Up, 0, 1), 0);
        assert!(matches!(s.phase(0, 1, 0.1), SweepPhase::WipeIn(_)));
        assert!(matches!(
            s.phase(0, 1, SWEEP_WIPE_SECS + 0.1),
            SweepPhase::Hold(_)
        ));
        assert!(matches!(s.phase(0, 1, total - 0.1), SweepPhase::WipeOut(_)));
        assert!(!s.done(1, total - 1e-6));
        assert!(s.done(1, total));
        assert_eq!(s.phase(0, 1, total), SweepPhase::Idle);
    }

    #[test]
    fn sweep_queue_serialises_and_dedupes() {
        let mut q = SweepQueue::default();
        q.push(SweepKind::AlertFiring);
        q.push(SweepKind::AlertFiring);
        q.push(SweepKind::TorrentDone);
        q.tick(0.0, 4);
        assert_eq!(q.active().unwrap().kind, SweepKind::AlertFiring);
        q.tick(1.0, 4);
        assert_eq!(q.active().unwrap().kind, SweepKind::AlertFiring);
        q.tick(4.0, 4);
        assert_eq!(q.active().unwrap().kind, SweepKind::TorrentDone);
        q.tick(8.0, 4);
        assert!(q.active().is_none());
    }

    #[test]
    fn fx_routes_requests() {
        let mut fx = Fx::default();
        fx.apply(FxRequest::HotNode(Role::Mem), 0.0);
        fx.apply(FxRequest::NodeNotReady, 0.0);
        fx.tick(0.0, 4);
        assert_eq!(
            fx.splashes[Role::Mem.index()].active().unwrap().kind,
            SplashKind::HotNode
        );
        assert_eq!(fx.sweeps.active().unwrap().kind, SweepKind::NodeNotReady);
    }
}

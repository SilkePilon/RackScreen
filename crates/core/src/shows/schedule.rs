//! When a show plays: the every-N-minutes schedule, play-now requests and the
//! transitions around a run. Pure; the model feeds it time and permission.

use super::ShowDef;
use crate::anim::Secs;
use crate::transition::Spec;

/// The transition that was around a show before styles existed: a 0.4 s
/// fade via black (0.2 s out, 0.2 s in), which `Spec::default` still is.
pub const SHOW_FADE_SECS: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowPhase {
    /// The roles give way to the show, panel by panel.
    In,
    Playing,
    /// The show gives way to the roles.
    Out,
}

/// What one panel shows during a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanelView {
    /// Its role, untouched (waiting for its turn, or done).
    Role,
    /// Between the two: `t` of the transition, from the show to the role
    /// when `from_show`, else from the role to the show.
    Mix { t: f32, from_show: bool },
    /// The show.
    Show,
}

#[derive(Clone, Copy, Debug)]
pub struct ShowRun {
    pub def: &'static ShowDef,
    pub started: Secs,
    /// The transition in and out, and its per-panel stagger.
    pub spec: Spec,
    pub panels: u32,
}

impl ShowRun {
    /// Delay before panel `pos` starts its transition.
    fn delay(&self, pos: u32) -> Secs {
        self.spec
            .kind
            .panel_delay(pos, self.panels, self.spec.stagger())
    }

    /// Length of the in (and the out) phase: one transition plus the
    /// stagger of the last panel.
    pub fn phase_secs(&self) -> Secs {
        self.spec.secs + self.spec.stagger() * self.panels.saturating_sub(1) as Secs
    }

    /// When the show's clock starts: half-way through the last panel's
    /// transition in, which is where the old fade via black showed it.
    fn play_start(&self) -> Secs {
        self.phase_secs() - self.spec.secs / 2.0
    }

    fn out_start(&self) -> Secs {
        self.play_start() + self.def.secs
    }

    /// The whole sequence: the transitions in and out plus the show itself.
    pub fn total(&self) -> Secs {
        self.out_start() + self.phase_secs()
    }

    /// Seconds into the run, with a hair of slack so `started + 0.2` lands
    /// in the next phase despite float rounding.
    fn elapsed(&self, now: Secs) -> Secs {
        (now - self.started).max(0.0) + 1e-9
    }

    /// The phase at `now`; `None` once the run is over.
    pub fn phase(&self, now: Secs) -> Option<ShowPhase> {
        let e = self.elapsed(now);
        Some(if e < self.phase_secs() {
            ShowPhase::In
        } else if e < self.out_start() {
            ShowPhase::Playing
        } else if e < self.total() {
            ShowPhase::Out
        } else {
            return None;
        })
    }

    /// What the panel at left-to-right position `pos` shows at `now`;
    /// `None` once the run is over.
    pub fn view(&self, now: Secs, pos: u32) -> Option<PanelView> {
        let e = self.elapsed(now);
        let phase = self.phase(now)?;
        let secs = self.spec.secs;
        let delay = self.delay(pos);
        Some(match phase {
            ShowPhase::Playing => PanelView::Show,
            ShowPhase::In => {
                let t = (e - delay) / secs;
                if t < 0.0 {
                    PanelView::Role
                } else if t >= 1.0 {
                    PanelView::Show
                } else {
                    PanelView::Mix {
                        t: t as f32,
                        from_show: false,
                    }
                }
            }
            ShowPhase::Out => {
                let t = (e - self.out_start() - delay) / secs;
                if t < 0.0 {
                    PanelView::Show
                } else if t >= 1.0 {
                    PanelView::Role
                } else {
                    PanelView::Mix {
                        t: t as f32,
                        from_show: true,
                    }
                }
            }
        })
    }

    /// The show's local time at `now`: 0 until it starts playing, held on
    /// its last frame while it goes out.
    pub fn local_t(&self, now: Secs) -> f64 {
        let last = (self.def.secs - 1.0 / super::FPS).max(0.0);
        (now - self.started - self.play_start()).clamp(0.0, last)
    }
}

/// A play-now request.
#[derive(Clone, Copy, Debug)]
pub enum Request {
    Id(&'static ShowDef),
    Random,
}

pub struct ShowScheduler {
    enabled: bool,
    every: Secs,
    list: Vec<&'static ShowDef>,
    last_id: Option<&'static str>,
    last_end: Secs,
    current: Option<ShowRun>,
}

impl ShowScheduler {
    pub fn new(enabled: bool, every_mins: u32, list: Vec<&'static ShowDef>) -> Self {
        Self {
            enabled,
            every: every_mins as Secs * 60.0,
            list,
            last_id: None,
            last_end: 0.0,
            current: None,
        }
    }

    /// Clear a finished run, then start one when due (or `request` is set)
    /// and `allowed`, with transition `spec` over `panels` panels; returns
    /// true when one started. A request that is not allowed is dropped here
    /// (the caller decides whether to keep it).
    pub fn tick(
        &mut self,
        now: Secs,
        allowed: bool,
        request: Option<Request>,
        spec: Spec,
        panels: u32,
        rng: &mut impl FnMut() -> u64,
    ) -> bool {
        if let Some(run) = self.current {
            if run.phase(now).is_some() {
                return false;
            }
            self.last_end = run.started + run.total();
            self.current = None;
        }
        if !allowed {
            return false;
        }
        let due = self.enabled && !self.list.is_empty() && now - self.last_end >= self.every;
        let def = match request {
            Some(Request::Id(d)) => d,
            Some(Request::Random) => self.pick(rng),
            None if due => self.pick(rng),
            None => return false,
        };
        self.last_id = Some(def.id);
        self.current = Some(ShowRun {
            def,
            started: now,
            spec,
            panels: panels.max(1),
        });
        true
    }

    /// A random show from the list (every show when it is empty), never the
    /// last one again when there is a choice.
    fn pick(&self, rng: &mut impl FnMut() -> u64) -> &'static ShowDef {
        let pool: Vec<&'static ShowDef> = if self.list.is_empty() {
            super::SHOWS.iter().collect()
        } else {
            self.list.clone()
        };
        let fresh: Vec<&'static ShowDef> = pool
            .iter()
            .copied()
            .filter(|d| Some(d.id) != self.last_id)
            .collect();
        let from = if fresh.is_empty() { &pool } else { &fresh };
        from[(rng() % from.len() as u64) as usize]
    }

    /// Count the next scheduled show from `now`, as if one just ended.
    pub fn restart(&mut self, now: Secs) {
        self.last_end = now;
    }

    pub fn current(&self) -> Option<&ShowRun> {
        self.current.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::{Direction, Kind, Style};

    fn nop(_: &mut super::super::world::World, _: u32, _: f64) {}
    static A: ShowDef = ShowDef {
        id: "a",
        name: "A",
        secs: 5.0,
        draw: nop,
    };
    static B: ShowDef = ShowDef {
        id: "b",
        name: "B",
        secs: 7.0,
        draw: nop,
    };
    static C: ShowDef = ShowDef {
        id: "c",
        name: "C",
        secs: 3.0,
        draw: nop,
    };

    /// xorshift64, like the model's.
    fn rng(seed: u64) -> impl FnMut() -> u64 {
        let mut x = seed;
        move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        }
    }

    fn run(def: &'static ShowDef, started: Secs, spec: Spec, panels: u32) -> ShowRun {
        ShowRun {
            def,
            started,
            spec,
            panels,
        }
    }

    fn mix(v: PanelView) -> (f32, bool) {
        match v {
            PanelView::Mix { t, from_show } => (t, from_show),
            other => panic!("not a mix: {other:?}"),
        }
    }

    #[test]
    fn default_fade_keeps_the_old_timing() {
        // 0.2 s roles out, the show, 0.2 s show out, 0.2 s roles in
        let run = run(&A, 0.0, Spec::default(), 4);
        assert!((run.total() - 5.6).abs() < 1e-9);
        assert!((run.phase_secs() - 0.4).abs() < 1e-9);
        assert_eq!(run.phase(0.0), Some(ShowPhase::In));
        assert_eq!(run.phase(0.39), Some(ShowPhase::In));
        assert_eq!(run.phase(0.4), Some(ShowPhase::Playing));
        assert_eq!(run.phase(5.0), Some(ShowPhase::Playing));
        assert_eq!(run.phase(5.2), Some(ShowPhase::Out));
        assert_eq!(run.phase(5.59), Some(ShowPhase::Out));
        assert_eq!(run.phase(5.6), None);
        assert_eq!(run.phase(99.0), None);
        // every panel together: the same view on every position
        for pos in 0..4 {
            let (t, from_show) = mix(run.view(0.1, pos).unwrap());
            assert!((t - 0.25).abs() < 1e-6, "pos {pos}: {t}");
            assert!(!from_show);
            assert_eq!(run.view(1.0, pos), Some(PanelView::Show));
            let (t, from_show) = mix(run.view(5.3, pos).unwrap());
            assert!((t - 0.25).abs() < 1e-6);
            assert!(from_show);
            let (t, _) = mix(run.view(5.5, pos).unwrap());
            assert!((t - 0.75).abs() < 1e-6);
            assert_eq!(run.view(5.6, pos), None);
        }
        // the show's clock starts as it fades in, at 0.2 s
        assert_eq!(run.local_t(0.0), 0.0);
        assert_eq!(run.local_t(0.2), 0.0);
        assert!((run.local_t(1.2) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_directional_style_sweeps_panel_by_panel() {
        let spec = Spec {
            kind: Kind::new(Style::Wipe, Direction::Right).unwrap(),
            secs: 0.4,
        };
        let run = run(&A, 0.0, spec, 4);
        // 0.1 s between panels: the in phase is 0.4 + 3 × 0.1
        assert!((run.phase_secs() - 0.7).abs() < 1e-9);
        assert!((run.total() - (0.7 - 0.2 + 5.0 + 0.7)).abs() < 1e-9);
        assert_eq!(run.view(0.05, 0).map(mix).map(|m| m.1), Some(false));
        assert_eq!(run.view(0.05, 1), Some(PanelView::Role), "not yet");
        assert_eq!(run.view(0.05, 3), Some(PanelView::Role));
        assert_eq!(run.view(0.45, 0), Some(PanelView::Show), "done");
        let (t, _) = mix(run.view(0.45, 3).unwrap());
        assert!((t - 0.375).abs() < 1e-6, "{t}");
        assert_eq!(run.view(0.7, 3), Some(PanelView::Show));
        // leftward: the rightmost panel goes first
        let left = ShowRun {
            spec: Spec {
                kind: Kind::new(Style::Wipe, Direction::Left).unwrap(),
                ..spec
            },
            ..run
        };
        assert_eq!(left.view(0.05, 0), Some(PanelView::Role));
        assert!(matches!(left.view(0.05, 3), Some(PanelView::Mix { .. })));
        // out: the same order, from the show to the roles
        let out0 = run.out_start();
        assert!((out0 - (0.5 + 5.0)).abs() < 1e-9);
        assert_eq!(run.view(out0 - 0.01, 0), Some(PanelView::Show));
        let (t, from_show) = mix(run.view(out0 + 0.1, 0).unwrap());
        assert!((t - 0.25).abs() < 1e-6);
        assert!(from_show);
        assert_eq!(run.view(out0 + 0.1, 3), Some(PanelView::Show), "waits");
        assert_eq!(run.view(out0 + 0.4, 0), Some(PanelView::Role));
        let (t, from_show) = mix(run.view(out0 + 0.69, 3).unwrap());
        assert!(t > 0.9 && from_show, "the last panel finishes last: {t}");
        assert_eq!(run.view(out0 + 0.7, 0), None, "over");
        // the clock starts half-way through the last panel's transition
        assert_eq!(run.local_t(0.5), 0.0);
        assert!((run.local_t(1.5) - 1.0).abs() < 1e-9);
        // one panel: no stagger at all
        let one = ShowRun { panels: 1, ..run };
        assert!((one.phase_secs() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn local_time_starts_with_play_and_holds_the_last_frame() {
        let run = run(&A, 10.0, Spec::default(), 4);
        assert_eq!(run.local_t(10.0), 0.0);
        assert!((run.local_t(11.2) - 1.0).abs() < 1e-9);
        let end = run.local_t(15.3);
        assert!(end < 5.0 && end > 4.9, "held inside the show, got {end}");
        assert_eq!(run.local_t(15.3), run.local_t(15.39));
    }

    #[test]
    fn due_after_every_mins_and_only_when_allowed() {
        let mut s = ShowScheduler::new(true, 2, vec![&A]);
        let mut r = rng(7);
        assert!(!s.tick(0.0, true, None, Spec::default(), 4, &mut r));
        assert!(!s.tick(119.9, true, None, Spec::default(), 4, &mut r));
        assert!(
            !s.tick(120.0, false, None, Spec::default(), 4, &mut r),
            "not while not allowed"
        );
        assert!(s.current().is_none());
        assert!(
            s.tick(130.0, true, None, Spec::default(), 4, &mut r),
            "as soon as allowed again"
        );
        let run = *s.current().unwrap();
        assert_eq!((run.def.id, run.started), ("a", 130.0));
        assert!(
            !s.tick(131.0, true, None, Spec::default(), 4, &mut r),
            "never two at once"
        );
        // the next one is due every_mins after this one ended
        let end = 130.0 + run.total();
        assert!(!s.tick(end + 0.01, true, None, Spec::default(), 4, &mut r));
        assert!(s.current().is_none(), "finished run cleared");
        assert!(!s.tick(end + 119.0, true, None, Spec::default(), 4, &mut r));
        assert!(s.tick(end + 120.5, true, None, Spec::default(), 4, &mut r));
    }

    #[test]
    fn disabled_or_empty_never_schedules() {
        let mut r = rng(3);
        let mut off = ShowScheduler::new(false, 2, vec![&A]);
        let mut empty = ShowScheduler::new(true, 2, vec![]);
        for i in 0..100 {
            let now = i as f64 * 60.0;
            assert!(!off.tick(now, true, None, Spec::default(), 4, &mut r));
            assert!(!empty.tick(now, true, None, Spec::default(), 4, &mut r));
        }
    }

    #[test]
    fn never_the_same_show_twice_in_a_row() {
        let mut s = ShowScheduler::new(true, 2, vec![&A, &B, &C]);
        let mut r = rng(11);
        let mut now = 0.0;
        let mut seen = Vec::new();
        while seen.len() < 60 {
            if s.tick(now, true, None, Spec::default(), 4, &mut r) {
                seen.push(s.current().unwrap().def.id);
            }
            now += 1.0;
        }
        for w in seen.windows(2) {
            assert_ne!(w[0], w[1], "{seen:?}");
        }
        for id in ["a", "b", "c"] {
            assert!(seen.contains(&id), "{id} never picked: {seen:?}");
        }
    }

    #[test]
    fn a_request_starts_at_once_but_not_when_not_allowed() {
        let mut s = ShowScheduler::new(true, 15, vec![&A, &B]);
        let mut r = rng(5);
        assert!(!s.tick(
            1.0,
            false,
            Some(Request::Id(&B)),
            Spec::default(),
            4,
            &mut r
        ));
        assert!(
            !s.tick(2.0, true, None, Spec::default(), 4, &mut r),
            "a refused request is not kept"
        );
        assert!(s.tick(3.0, true, Some(Request::Id(&B)), Spec::default(), 4, &mut r));
        assert_eq!(s.current().unwrap().def.id, "b");
        assert!(
            !s.tick(4.0, true, Some(Request::Id(&A)), Spec::default(), 4, &mut r),
            "not over a run"
        );
        // random picks from the list, still without an immediate repeat
        let end = 3.0 + s.current().unwrap().total();
        assert!(s.tick(
            end + 1.0,
            true,
            Some(Request::Random),
            Spec::default(),
            4,
            &mut r
        ));
        assert_eq!(s.current().unwrap().def.id, "a");
        // a request works even with the schedule off; random with an empty
        // list falls back to every show
        let mut off = ShowScheduler::new(false, 15, vec![]);
        assert!(off.tick(1.0, true, Some(Request::Random), Spec::default(), 4, &mut r));
        assert!(super::super::by_id(off.current().unwrap().def.id).is_some());
    }

    #[test]
    fn restart_counts_every_mins_from_now() {
        let mut s = ShowScheduler::new(true, 2, vec![&A]);
        let mut r = rng(4);
        s.restart(1000.0);
        assert!(
            !s.tick(1000.0, true, None, Spec::default(), 4, &mut r),
            "overdue, but restarted"
        );
        assert!(!s.tick(1119.9, true, None, Spec::default(), 4, &mut r));
        assert!(s.tick(1120.0, true, None, Spec::default(), 4, &mut r));
    }

    #[test]
    fn a_request_resets_the_schedule() {
        let mut s = ShowScheduler::new(true, 2, vec![&A, &B]);
        let mut r = rng(9);
        assert!(s.tick(
            100.0,
            true,
            Some(Request::Id(&A)),
            Spec::default(),
            4,
            &mut r
        ));
        let end = 100.0 + s.current().unwrap().total();
        assert!(
            !s.tick(end + 60.0, true, None, Spec::default(), 4, &mut r),
            "due 2 min after the request's end"
        );
        assert!(s.tick(end + 120.0, true, None, Spec::default(), 4, &mut r));
    }
}

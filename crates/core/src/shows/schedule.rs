//! When a show plays: the every-N-minutes schedule, play-now requests and the
//! fade sequence around a run. Pure; the model feeds it time and permission.

use super::ShowDef;
use crate::anim::Secs;

/// Each of the three fades around a show: roles out, show out, roles in.
pub const SHOW_FADE_SECS: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowPhase {
    RolesOut,
    Playing,
    ShowOut,
    RolesIn,
}

#[derive(Clone, Copy, Debug)]
pub struct ShowRun {
    pub def: &'static ShowDef,
    pub started: Secs,
}

impl ShowRun {
    /// The whole sequence: three fades plus the show itself.
    pub fn total(&self) -> Secs {
        3.0 * SHOW_FADE_SECS + self.def.secs
    }

    /// The phase at `now` and its brightness 0..1; `None` once the run is over.
    pub fn phase(&self, now: Secs) -> Option<(ShowPhase, f32)> {
        // A hair of slack, so `started + 0.2` lands in the next phase despite
        // float rounding.
        let e = (now - self.started).max(0.0) + 1e-9;
        let f = SHOW_FADE_SECS;
        let play_end = f + self.def.secs;
        // Both out phases follow the crossfade's out half, roles-in its in half.
        let out = |u: f64| crate::screens::fade(0.5 * u as f32).1;
        Some(if e < f {
            (ShowPhase::RolesOut, out(e / f))
        } else if e < play_end {
            (ShowPhase::Playing, 1.0)
        } else if e < play_end + f {
            (ShowPhase::ShowOut, out((e - play_end) / f))
        } else if e < play_end + 2.0 * f {
            let u = (e - play_end - f) / f;
            (
                ShowPhase::RolesIn,
                crate::screens::fade(0.5 + 0.5 * u as f32).1,
            )
        } else {
            return None;
        })
    }

    /// The show's local time at `now`: 0 when it starts playing, held on its
    /// last frame while it fades out.
    pub fn local_t(&self, now: Secs) -> f64 {
        let last = (self.def.secs - 1.0 / super::FPS).max(0.0);
        (now - self.started - SHOW_FADE_SECS).clamp(0.0, last)
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
    /// and `allowed`; returns true when one started. A request that is not
    /// allowed is dropped here (the caller decides whether to keep it).
    pub fn tick(
        &mut self,
        now: Secs,
        allowed: bool,
        request: Option<Request>,
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
        self.current = Some(ShowRun { def, started: now });
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
    use crate::screens::fade;

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

    #[test]
    fn phase_boundaries() {
        let run = ShowRun {
            def: &A,
            started: 0.0,
        };
        assert!((run.total() - 5.6).abs() < 1e-9);
        assert_eq!(run.phase(0.0), Some((ShowPhase::RolesOut, 1.0)));
        let (p, b) = run.phase(0.1).unwrap();
        assert_eq!(
            (p, b),
            (ShowPhase::RolesOut, fade(0.25).1),
            "the crossfade's out half"
        );
        assert_eq!(run.phase(0.2), Some((ShowPhase::Playing, 1.0)));
        assert_eq!(run.phase(5.0), Some((ShowPhase::Playing, 1.0)));
        assert_eq!(run.phase(5.2), Some((ShowPhase::ShowOut, 1.0)));
        assert_eq!(run.phase(5.3).unwrap().1, fade(0.25).1);
        assert_eq!(run.phase(5.4), Some((ShowPhase::RolesIn, 0.0)));
        assert_eq!(run.phase(5.5).unwrap(), (ShowPhase::RolesIn, fade(0.75).1));
        assert_eq!(run.phase(5.6), None);
        assert_eq!(run.phase(99.0), None);
        // brightness never rises during an out phase, never falls during roles-in
        let mut last = (ShowPhase::RolesOut, 2.0f32);
        for i in 0..560 {
            let (p, b) = run.phase(i as f64 / 100.0).unwrap();
            if p == last.0 {
                match p {
                    ShowPhase::RolesIn => assert!(b >= last.1),
                    _ => assert!(b <= last.1),
                }
            }
            last = (p, b);
        }
    }

    #[test]
    fn local_time_starts_with_play_and_holds_the_last_frame() {
        let run = ShowRun {
            def: &A,
            started: 10.0,
        };
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
        assert!(!s.tick(0.0, true, None, &mut r));
        assert!(!s.tick(119.9, true, None, &mut r));
        assert!(!s.tick(120.0, false, None, &mut r), "not while not allowed");
        assert!(s.current().is_none());
        assert!(
            s.tick(130.0, true, None, &mut r),
            "as soon as allowed again"
        );
        let run = *s.current().unwrap();
        assert_eq!((run.def.id, run.started), ("a", 130.0));
        assert!(!s.tick(131.0, true, None, &mut r), "never two at once");
        // the next one is due every_mins after this one ended
        let end = 130.0 + run.total();
        assert!(!s.tick(end + 0.01, true, None, &mut r));
        assert!(s.current().is_none(), "finished run cleared");
        assert!(!s.tick(end + 119.0, true, None, &mut r));
        assert!(s.tick(end + 120.5, true, None, &mut r));
    }

    #[test]
    fn disabled_or_empty_never_schedules() {
        let mut r = rng(3);
        let mut off = ShowScheduler::new(false, 2, vec![&A]);
        let mut empty = ShowScheduler::new(true, 2, vec![]);
        for i in 0..100 {
            let now = i as f64 * 60.0;
            assert!(!off.tick(now, true, None, &mut r));
            assert!(!empty.tick(now, true, None, &mut r));
        }
    }

    #[test]
    fn never_the_same_show_twice_in_a_row() {
        let mut s = ShowScheduler::new(true, 2, vec![&A, &B, &C]);
        let mut r = rng(11);
        let mut now = 0.0;
        let mut seen = Vec::new();
        while seen.len() < 60 {
            if s.tick(now, true, None, &mut r) {
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
        assert!(!s.tick(1.0, false, Some(Request::Id(&B)), &mut r));
        assert!(
            !s.tick(2.0, true, None, &mut r),
            "a refused request is not kept"
        );
        assert!(s.tick(3.0, true, Some(Request::Id(&B)), &mut r));
        assert_eq!(s.current().unwrap().def.id, "b");
        assert!(
            !s.tick(4.0, true, Some(Request::Id(&A)), &mut r),
            "not over a run"
        );
        // random picks from the list, still without an immediate repeat
        let end = 3.0 + s.current().unwrap().total();
        assert!(s.tick(end + 1.0, true, Some(Request::Random), &mut r));
        assert_eq!(s.current().unwrap().def.id, "a");
        // a request works even with the schedule off; random with an empty
        // list falls back to every show
        let mut off = ShowScheduler::new(false, 15, vec![]);
        assert!(off.tick(1.0, true, Some(Request::Random), &mut r));
        assert!(super::super::by_id(off.current().unwrap().def.id).is_some());
    }

    #[test]
    fn restart_counts_every_mins_from_now() {
        let mut s = ShowScheduler::new(true, 2, vec![&A]);
        let mut r = rng(4);
        s.restart(1000.0);
        assert!(
            !s.tick(1000.0, true, None, &mut r),
            "overdue, but restarted"
        );
        assert!(!s.tick(1119.9, true, None, &mut r));
        assert!(s.tick(1120.0, true, None, &mut r));
    }

    #[test]
    fn a_request_resets_the_schedule() {
        let mut s = ShowScheduler::new(true, 2, vec![&A, &B]);
        let mut r = rng(9);
        assert!(s.tick(100.0, true, Some(Request::Id(&A)), &mut r));
        let end = 100.0 + s.current().unwrap().total();
        assert!(
            !s.tick(end + 60.0, true, None, &mut r),
            "due 2 min after the request's end"
        );
        assert!(s.tick(end + 120.0, true, None, &mut r));
    }
}

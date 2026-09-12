//! Plays the acts: one at a time from a short queue, habits when nothing is
//! happening, edge triggers from cluster state, and the rest expression that
//! tweens toward the mood in between.

use std::collections::VecDeque;

use crate::anim::Secs;
use crate::electricity::PriceLevel;
use crate::face_acts::{
    run_act, weather_cat, ActKind, EyeOv, EyePair, Placed, Post, WeatherCat, IDLE_HABITS,
};
use crate::face_expr::{unit, Expr, ExprTween, Idle};
use crate::model::FxRequest;
use crate::mood::{Mood, MoodEngine, MoodInputs, MoodTuning};
use crate::theme::Role;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceTuning {
    pub bored_after: Secs,
    pub reaction: Secs,
    pub idle_habits: bool,
    pub weather_habits: bool,
}

impl Default for FaceTuning {
    fn default() -> Self {
        Self {
            bored_after: 120.0 * 60.0,
            reaction: 60.0,
            idle_habits: true,
            weather_habits: true,
        }
    }
}

/// Everything the player reads from the model each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceInputs {
    pub mood: MoodInputs,
    pub ups_have: bool,
    pub ups_charge_pct: f32,
    pub storage_have: bool,
    pub storage_pct: f32,
    pub gh_have: bool,
    pub gh_today: u32,
    pub price_level: Option<PriceLevel>,
    pub weather_have: bool,
    pub weather_code: u16,
    pub weather_temp_c: f32,
    pub weather_gust_kmh: f32,
    pub weather_is_day: bool,
    pub sky_have: bool,
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub moon_illumination: f32,
    pub unix_now: i64,
}

/// What the rasteriser draws this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFrame {
    pub e: Expr,
    pub blink: f32,
    pub off: (f32, f32),
    pub rot: f32,
    pub gaze: (f32, f32),
    pub eyes: [EyeOv; 2],
    pub sprite: Option<EyePair>,
    pub placed: Vec<Placed>,
    pub post: Post,
}

/// Queue depth; a fourth waiting act is dropped.
pub const QUEUE_CAP: usize = 3;
/// Pod start and gone acts: at most one per this many seconds.
pub const POD_RATE_SECS: Secs = 20.0;
/// Crashes within this window make the face dizzy.
pub const DIZZY_WINDOW_SECS: Secs = 600.0;
pub const DIZZY_CRASHES: usize = 3;
/// Edge thresholds with their re-arm levels.
pub const UPS_LOW_PCT: f32 = 20.0;
pub const UPS_LOW_REARM_PCT: f32 = 30.0;
pub const STORAGE_FULL_PCT: f32 = 90.0;
pub const STORAGE_FULL_REARM_PCT: f32 = 85.0;
pub const GH_MILESTONE: u32 = 10;
pub const FULL_MOON: f32 = 0.97;
/// A sunrise or sunset counts while `unix_now` is within this many seconds after it.
pub const SUN_EDGE_SECS: i64 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    kind: ActKind,
    started: Secs,
    dur: Secs,
}

impl Playing {
    fn progress(&self, now: Secs) -> f32 {
        (((now - self.started) / self.dur).clamp(0.0, 1.0)) as f32
    }
    fn done(&self, now: Secs) -> bool {
        now - self.started >= self.dur
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacePlayer {
    tuning: FaceTuning,
    mood: MoodEngine,
    idle: Idle,
    rng: u64,
    queue: VecDeque<ActKind>,
    current: Option<Playing>,
    /// Expression at rest, tweening toward the mood.
    rest: ExprTween,
    last_mood: Mood,
    last_pod_start: Option<Secs>,
    last_pod_gone: Option<Secs>,
    crashes: Vec<Secs>,
    // habits
    next_idle: Secs,
    last_idle: Option<ActKind>,
    next_weather: Secs,
    weather_seen: Option<WeatherCat>,
    // edges
    ups_low_armed: bool,
    storage_full_armed: bool,
    gh_milestone_armed: bool,
    price_prev: Option<PriceLevel>,
    sunrise_done: Option<i64>,
    sunset_done: Option<i64>,
    last_tick: Secs,
}

impl FacePlayer {
    pub fn new(now: Secs) -> Self {
        let mut rng = 0x9E37_79B9_7F4A_7C15;
        let first_idle = now + 45.0 + 75.0 * unit(&mut rng) as Secs;
        Self {
            tuning: FaceTuning::default(),
            mood: MoodEngine::new(now),
            idle: Idle::new(),
            rng,
            queue: VecDeque::new(),
            current: None,
            rest: ExprTween::new(Expr::CONTENT),
            last_mood: Mood::Content,
            last_pod_start: None,
            last_pod_gone: None,
            crashes: Vec::new(),
            next_idle: first_idle,
            last_idle: None,
            next_weather: now,
            weather_seen: None,
            ups_low_armed: true,
            storage_full_armed: true,
            gh_milestone_armed: true,
            price_prev: None,
            sunrise_done: None,
            sunset_done: None,
            last_tick: now,
        }
    }

    pub fn set_tuning(&mut self, t: FaceTuning) {
        self.tuning = t;
        self.mood.set_tuning(MoodTuning {
            bored_after: t.bored_after,
            reaction: t.reaction,
        });
    }
    pub fn tuning(&self) -> FaceTuning {
        self.tuning
    }
    pub fn set_seed(&mut self, seed: u64) {
        self.rng = seed | 1;
    }
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.mood.set_bedtime_near(near);
    }
    pub fn mood(&self) -> Mood {
        self.last_mood
    }
    pub fn current_act(&self) -> Option<ActKind> {
        self.current.map(|p| p.kind)
    }
    pub fn queued(&self) -> Vec<ActKind> {
        self.queue.iter().copied().collect()
    }

    /// The same request stream the splashes and sweeps get.
    pub fn on_fx(&mut self, req: &FxRequest, now: Secs) {
        let kind = match req {
            FxRequest::PodStarted => ActKind::OhHi,
            FxRequest::PodCrashed => ActKind::Ouch,
            FxRequest::PodGone => ActKind::Bye,
            FxRequest::HotNode(Role::Mem) => ActKind::Stuffed,
            FxRequest::HotNode(_) => ActKind::WorkingHard,
            FxRequest::HotTemp => ActKind::TooHot,
            FxRequest::VolumeDegraded => ActKind::Hmm,
            FxRequest::VolumeHealthy => ActKind::DisksFine,
            FxRequest::TorrentAdded => ActKind::Incoming,
            FxRequest::TorrentDone => ActKind::GotIt,
            FxRequest::NodeNotReady => ActKind::LostOne,
            FxRequest::NodeReady => ActKind::ItsBack,
            FxRequest::AlertFiring => ActKind::Alarm,
            FxRequest::AlertResolved => ActKind::AllClear,
            FxRequest::LinkUp => ActKind::FoundYou,
            FxRequest::Boot => ActKind::WakeUp,
            FxRequest::GithubPush => ActKind::Catch,
            FxRequest::GithubStar => ActKind::StarryEyes,
            FxRequest::GithubMerge => ActKind::Merge,
            FxRequest::GithubRelease => ActKind::Party,
            FxRequest::GithubRunFailed => ActKind::EyeRoll,
            FxRequest::GithubRunPassed => ActKind::Ding,
            FxRequest::Thunder => ActKind::Lightning,
            FxRequest::RainSoon => ActKind::UhOhRain,
            FxRequest::AirWorse => ActKind::Cough,
            FxRequest::AppSynced => ActKind::Launch,
            FxRequest::AppDegraded => ActKind::Grump,
            FxRequest::AppHealthy => ActKind::Wink,
            FxRequest::UpsOnBattery => ActKind::LightsFlicker,
            FxRequest::UpsOnline => ActKind::Phew,
            FxRequest::IssPass => ActKind::LookUp,
        };
        if kind == ActKind::Ouch {
            self.mood.crashed(now);
            self.crashes.retain(|t| now - t <= DIZZY_WINDOW_SECS);
            self.crashes.push(now);
        }
        self.request(kind, now);
        if kind == ActKind::Ouch && self.crashes.len() >= DIZZY_CRASHES {
            self.crashes.clear();
            self.request(ActKind::Dizzy, now);
        }
    }

    pub fn on_link_down(&mut self, now: Secs) {
        self.request(ActKind::Hello, now);
    }

    /// Queue an event act (not a habit), honouring rate limits, the cap,
    /// severity, and boot.
    fn request(&mut self, kind: ActKind, now: Secs) {
        if kind == ActKind::WakeUp {
            self.queue.clear();
            self.current = Some(Playing {
                kind,
                started: now,
                dur: kind.def().dur as Secs,
            });
            self.mood.touch(now);
            return;
        }
        if kind.is_rate_limited() {
            let last = if kind == ActKind::OhHi {
                &mut self.last_pod_start
            } else {
                &mut self.last_pod_gone
            };
            if last.is_some_and(|t| now - t < POD_RATE_SECS) {
                return;
            }
            *last = Some(now);
        }
        self.mood.touch(now);
        if self.queue.contains(&kind)
            || self.current.is_some_and(|p| p.kind == kind && !p.done(now))
        {
            return;
        }
        // an interruptible habit gives way: jump to its out phase
        if let Some(cur) = self.current {
            if cur.kind.is_habit() {
                let out_start = cur.dur - crate::face_acts::IN_S as Secs;
                if now - cur.started < out_start {
                    self.current = Some(Playing {
                        started: now - out_start,
                        ..cur
                    });
                }
            }
        }
        if kind.is_severe() {
            self.queue.push_front(kind);
            self.queue.truncate(QUEUE_CAP);
        } else if self.queue.len() < QUEUE_CAP {
            self.queue.push_back(kind);
        }
    }

    /// Advance: finish or start acts, run edge triggers and habits, and move
    /// the rest expression toward the mood.
    pub fn tick(&mut self, i: &FaceInputs, now: Secs) {
        self.last_tick = now;
        self.edges(i, now);
        if let Some(cur) = self.current {
            if cur.done(now) {
                let def = cur.kind.def();
                let end = Expr::of(def.mood);
                self.rest = ExprTween::new(end);
                if !cur.kind.is_habit() {
                    self.mood.react(def.mood, now);
                }
                self.current = None;
            }
        }
        if self.current.is_none() {
            if let Some(kind) = self.queue.pop_front() {
                self.start(kind, now);
            } else {
                self.habits(i, now);
            }
        }
        let _ = self.idle.tick(self.last_mood, now, &mut self.rng);
        self.last_mood = self.mood.current(&i.mood, now);
        self.rest.retarget(Expr::of(self.last_mood), now);
    }

    fn start(&mut self, kind: ActKind, now: Secs) {
        self.current = Some(Playing {
            kind,
            started: now,
            dur: kind.def().dur as Secs,
        });
    }

    fn edges(&mut self, i: &FaceInputs, now: Secs) {
        if i.ups_have {
            if self.ups_low_armed && i.ups_charge_pct < UPS_LOW_PCT {
                self.ups_low_armed = false;
                self.request(ActKind::OnFumes, now);
            } else if !self.ups_low_armed && i.ups_charge_pct > UPS_LOW_REARM_PCT {
                self.ups_low_armed = true;
            }
        }
        if i.storage_have {
            if self.storage_full_armed && i.storage_pct >= STORAGE_FULL_PCT {
                self.storage_full_armed = false;
                self.request(ActKind::SoFull, now);
            } else if !self.storage_full_armed && i.storage_pct < STORAGE_FULL_REARM_PCT {
                self.storage_full_armed = true;
            }
        }
        if i.gh_have {
            if self.gh_milestone_armed && i.gh_today >= GH_MILESTONE {
                self.gh_milestone_armed = false;
                self.request(ActKind::LevelUp, now);
            } else if !self.gh_milestone_armed && i.gh_today < GH_MILESTONE {
                self.gh_milestone_armed = true;
            }
        }
        if let Some(level) = i.price_level {
            let cheap = |l: PriceLevel| matches!(l, PriceLevel::VeryCheap | PriceLevel::Cheap);
            let pricey = |l: PriceLevel| matches!(l, PriceLevel::Pricey | PriceLevel::VeryPricey);
            if let Some(prev) = self.price_prev {
                if prev != level {
                    if cheap(level) && !cheap(prev) {
                        self.request(ActKind::KaChing, now);
                    } else if pricey(level) && !pricey(prev) {
                        self.request(ActKind::Expensive, now);
                    }
                }
            }
            self.price_prev = Some(level);
        }
        if i.sky_have {
            let passing =
                |edge: Option<i64>| edge.filter(|e| (*e..*e + SUN_EDGE_SECS).contains(&i.unix_now));
            if let Some(e) = passing(i.sunrise) {
                if self.sunrise_done != Some(e) {
                    self.sunrise_done = Some(e);
                    self.request(ActKind::Morning, now);
                }
            }
            if let Some(e) = passing(i.sunset) {
                if self.sunset_done != Some(e) {
                    self.sunset_done = Some(e);
                    let kind = if i.moon_illumination >= FULL_MOON {
                        ActKind::Awoo
                    } else {
                        ActKind::Evening
                    };
                    self.request(kind, now);
                }
            }
        }
    }

    /// Nothing is playing and nothing is queued: maybe a weather or idle habit.
    fn habits(&mut self, i: &FaceInputs, now: Secs) {
        if self.tuning.weather_habits && i.weather_have {
            let cat = weather_cat(
                i.weather_code,
                i.weather_temp_c,
                i.weather_gust_kmh,
                i.weather_is_day,
            );
            let changed = cat != self.weather_seen;
            if changed {
                self.weather_seen = cat;
            }
            if let Some(cat) = cat {
                if changed || now >= self.next_weather {
                    self.next_weather = now + 240.0 + 240.0 * unit(&mut self.rng) as Secs;
                    self.start(cat.act(), now);
                    return;
                }
            }
        }
        if self.tuning.idle_habits
            && matches!(self.last_mood, Mood::Content | Mood::Bored)
            && now >= self.next_idle
        {
            self.next_idle = now + 45.0 + 75.0 * unit(&mut self.rng) as Secs;
            let bored = self.last_mood == Mood::Bored;
            let pool: Vec<(ActKind, u32)> = IDLE_HABITS
                .iter()
                .copied()
                .filter(|(k, _)| bored || *k != ActKind::DozingOff)
                .collect();
            let total: u32 = pool.iter().map(|(_, w)| w).sum();
            let mut pick = (unit(&mut self.rng) * total as f32) as u32;
            let mut kind = pool[0].0;
            for (k, w) in &pool {
                if pick < *w {
                    kind = *k;
                    break;
                }
                pick -= w;
            }
            if matches!(kind, ActKind::Sneeze | ActKind::Hic) && self.last_idle == Some(kind) {
                kind = ActKind::Humming;
            }
            self.last_idle = Some(kind);
            self.start(kind, now);
        }
    }

    /// The face this instant. Idle blink, gaze and tilt are computed here from
    /// the seeded generator, so equal seeds and clocks give equal frames.
    pub fn frame(&self, now: Secs) -> FaceFrame {
        let mut idle = self.idle.clone();
        let mut rng = self.rng;
        let io = idle.tick(self.last_mood, now, &mut rng);
        match self.current {
            Some(cur) => {
                let def = cur.kind.def();
                let held = self.rest.value(now);
                let f = run_act(&def, cur.progress(now), held, io.gaze);
                FaceFrame {
                    e: f.e,
                    blink: io.blink,
                    off: f.off,
                    rot: f.rot + io.tilt,
                    gaze: f.gaze,
                    eyes: f.eyes,
                    sprite: f.sprite,
                    placed: f.placed,
                    post: f.post,
                }
            }
            None => FaceFrame {
                e: self.rest.value(now),
                blink: io.blink,
                off: io.wobble,
                rot: io.tilt,
                gaze: io.gaze,
                eyes: [EyeOv::default(); 2],
                sprite: None,
                placed: Vec::new(),
                post: Post::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> FaceInputs {
        FaceInputs::default()
    }

    /// Advance at 30 fps from `from` to `to`, ticking with `inputs`.
    fn run(p: &mut FacePlayer, i: &FaceInputs, from: Secs, to: Secs) {
        let mut t = from;
        while t < to {
            p.tick(i, t);
            t += 1.0 / 30.0;
        }
    }

    fn player() -> FacePlayer {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            idle_habits: false,
            weather_habits: false,
            ..FaceTuning::default()
        });
        p
    }

    #[test]
    fn fx_requests_map_to_acts_and_play_one_at_a_time() {
        let mut p = player();
        p.on_fx(&FxRequest::PodCrashed, 1.0);
        p.on_fx(&FxRequest::GithubStar, 1.0);
        p.tick(&quiet(), 1.0);
        assert_eq!(p.current_act(), Some(ActKind::Ouch));
        assert_eq!(p.queued(), vec![ActKind::StarryEyes]);
        run(&mut p, &quiet(), 1.0, 3.9);
        assert_eq!(
            p.current_act(),
            Some(ActKind::StarryEyes),
            "next act starts when ouch (2.8 s) ends"
        );
        run(&mut p, &quiet(), 3.9, 7.0);
        assert_eq!(p.current_act(), None);
        assert_eq!(p.mood(), Mood::Excited, "the last act's mood holds");
        run(&mut p, &quiet(), 7.0, 68.0);
        assert_eq!(
            p.mood(),
            Mood::Worried,
            "reaction expired after 60 s; the crash still worries"
        );
        run(&mut p, &quiet(), 68.0, 320.0);
        assert_eq!(p.mood(), Mood::Content, "crash worry gone after 300 s");
    }

    #[test]
    fn severe_acts_jump_the_queue_and_the_queue_caps_at_three_without_duplicates() {
        let mut p = player();
        p.on_fx(&FxRequest::GithubPush, 0.0);
        p.on_fx(&FxRequest::GithubPush, 0.0); // one request per commit: collapses
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::Catch));
        assert!(
            p.queued().is_empty(),
            "the second push collapsed into the first"
        );
        p.on_fx(&FxRequest::AppSynced, 0.0);
        p.on_fx(&FxRequest::TorrentDone, 0.0);
        p.on_fx(&FxRequest::GithubMerge, 0.0);
        p.on_fx(&FxRequest::AppHealthy, 0.0); // fourth in the queue: dropped
        assert_eq!(
            p.queued(),
            vec![ActKind::Launch, ActKind::GotIt, ActKind::Merge]
        );
        p.on_fx(&FxRequest::UpsOnBattery, 0.1);
        assert_eq!(p.queued()[0], ActKind::LightsFlicker, "severe goes first");
        assert_eq!(p.queued().len(), 3, "and the cap still holds");
    }

    #[test]
    fn pod_starts_are_rate_limited() {
        let mut p = player();
        p.on_fx(&FxRequest::PodStarted, 0.0);
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::OhHi));
        p.on_fx(&FxRequest::PodStarted, 5.0);
        assert!(p.queued().is_empty(), "5 s later: dropped");
        p.on_fx(&FxRequest::PodStarted, 21.0);
        assert_eq!(p.queued(), vec![ActKind::OhHi], "21 s later: allowed again");
    }

    #[test]
    fn boot_plays_first_and_is_not_interrupted() {
        let mut p = player();
        p.on_fx(&FxRequest::GithubStar, 0.0);
        p.on_fx(&FxRequest::Boot, 0.0);
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::WakeUp));
        p.on_fx(&FxRequest::PodCrashed, 0.5);
        run(&mut p, &quiet(), 0.5, 3.0);
        assert_eq!(
            p.current_act(),
            Some(ActKind::WakeUp),
            "still waking up at 3 s"
        );
        run(&mut p, &quiet(), 3.0, 3.7);
        assert_eq!(p.current_act(), Some(ActKind::Ouch), "then the severe one");
    }

    #[test]
    fn three_crashes_in_ten_minutes_make_it_dizzy() {
        let mut p = player();
        for t in [0.0, 100.0, 200.0] {
            p.on_fx(&FxRequest::PodCrashed, t);
            run(&mut p, &quiet(), t, t + 4.0);
        }
        p.tick(&quiet(), 204.0);
        assert_eq!(p.current_act(), Some(ActKind::Dizzy));
    }

    #[test]
    fn link_down_and_up() {
        let mut p = player();
        p.on_link_down(1.0);
        p.tick(&quiet(), 1.0);
        assert_eq!(p.current_act(), Some(ActKind::Hello));
        run(&mut p, &quiet(), 1.0, 5.0);
        p.on_fx(&FxRequest::LinkUp, 5.0);
        p.tick(&quiet(), 5.0);
        assert_eq!(p.current_act(), Some(ActKind::FoundYou));
    }

    #[test]
    fn edge_triggers_fire_once_and_rearm() {
        let mut p = player();
        let mut i = quiet();
        i.ups_have = true;
        i.ups_charge_pct = 50.0;
        run(&mut p, &i, 0.0, 1.0);
        i.ups_charge_pct = 19.0;
        p.tick(&i, 1.0);
        assert_eq!(p.current_act(), Some(ActKind::OnFumes));
        run(&mut p, &i, 1.0, 5.0);
        i.ups_charge_pct = 25.0;
        run(&mut p, &i, 5.0, 6.0);
        i.ups_charge_pct = 15.0;
        run(&mut p, &i, 6.0, 7.0);
        assert_eq!(p.current_act(), None, "not re-armed until above 30");
        i.ups_charge_pct = 35.0;
        run(&mut p, &i, 7.0, 8.0);
        i.ups_charge_pct = 10.0;
        p.tick(&i, 8.0);
        assert_eq!(p.current_act(), Some(ActKind::OnFumes));

        // storage
        let mut p = player();
        let mut i = quiet();
        i.storage_have = true;
        i.storage_pct = 91.0;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), Some(ActKind::SoFull));

        // contributions
        let mut p = player();
        let mut i = quiet();
        i.gh_have = true;
        i.gh_today = 9;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.gh_today = 10;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::LevelUp));

        // prices: first observation only arms, then a band change plays
        let mut p = player();
        let mut i = quiet();
        i.price_level = Some(PriceLevel::Normal);
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.price_level = Some(PriceLevel::Cheap);
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::KaChing));
        run(&mut p, &i, 0.1, 4.0);
        i.price_level = Some(PriceLevel::VeryPricey);
        p.tick(&i, 4.0);
        assert_eq!(p.current_act(), Some(ActKind::Expensive));

        // sunrise, sunset, full moon
        let mut p = player();
        let mut i = quiet();
        i.sky_have = true;
        i.sunrise = Some(1000);
        i.sunset = Some(2000);
        i.unix_now = 999;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.unix_now = 1010;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::Morning));
        run(&mut p, &i, 0.1, 4.0);
        i.unix_now = 1020;
        p.tick(&i, 4.0);
        assert_eq!(p.current_act(), None, "not twice for the same sunrise");
        i.unix_now = 2005;
        i.moon_illumination = 0.5;
        p.tick(&i, 4.1);
        assert_eq!(p.current_act(), Some(ActKind::Evening));
        let mut p = player();
        i.unix_now = 1999;
        p.tick(&i, 0.0);
        i.unix_now = 2005;
        i.moon_illumination = 0.98;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::Awoo));
        // a Pi booting at noon does not replay sunrise
        let mut p = player();
        i.unix_now = 5000;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
    }

    #[test]
    fn weather_habit_plays_on_change_and_then_every_few_minutes() {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            idle_habits: false,
            ..FaceTuning::default()
        });
        let mut i = quiet();
        i.weather_have = true;
        i.weather_code = 61;
        i.weather_temp_c = 12.0;
        i.weather_is_day = true;
        p.tick(&i, 0.0);
        assert_eq!(
            p.current_act(),
            Some(ActKind::Raining),
            "first sight of rain"
        );
        run(&mut p, &i, 0.0, 5.0);
        assert_eq!(p.current_act(), None);
        let mut replayed_at = None;
        let mut t = 5.0;
        while t < 600.0 {
            p.tick(&i, t);
            if p.current_act() == Some(ActKind::Raining) {
                replayed_at = Some(t);
                break;
            }
            t += 1.0 / 30.0;
        }
        let at = replayed_at.expect("replayed within ten minutes");
        assert!((240.0..=480.0).contains(&at), "replay at {at}");
        // a real event interrupts the habit: it jumps to its out phase
        p.on_fx(&FxRequest::PodCrashed, at + 0.5);
        run(&mut p, &i, at + 0.5, at + 0.95);
        assert_eq!(
            p.current_act(),
            Some(ActKind::Ouch),
            "habit cut short within 0.3 s"
        );
    }

    #[test]
    fn idle_habits_fire_when_content_and_never_sneeze_twice() {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            weather_habits: false,
            ..FaceTuning::default()
        });
        let mut seen = Vec::new();
        let mut last = None;
        let mut t = 0.0;
        while t < 1800.0 {
            p.tick(&quiet(), t);
            let cur = p.current_act();
            if let Some(k) = cur {
                if cur != last {
                    seen.push(k);
                }
            }
            last = cur;
            t += 1.0 / 30.0;
        }
        assert!(
            seen.len() >= 10 && seen.len() <= 40,
            "{} habits in 30 min",
            seen.len()
        );
        assert!(seen.iter().all(|k| k.is_idle_habit()));
        assert!(!seen.contains(&ActKind::DozingOff), "not bored yet");
        for w in seen.windows(2) {
            assert!(
                !(matches!(w[0], ActKind::Sneeze | ActKind::Hic) && w[0] == w[1]),
                "{w:?} twice"
            );
        }
        // bored: dozing off joins in
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            bored_after: 10.0,
            weather_habits: false,
            ..FaceTuning::default()
        });
        let mut dozed = false;
        let mut t = 0.0;
        while t < 1800.0 {
            p.tick(&quiet(), t);
            if p.current_act() == Some(ActKind::DozingOff) {
                dozed = true;
                break;
            }
            t += 1.0 / 30.0;
        }
        assert!(dozed);
    }

    #[test]
    fn frames_are_continuous_and_deterministic() {
        let mut p = player();
        p.on_fx(&FxRequest::NodeNotReady, 1.0);
        let mut prev = p.frame(0.0);
        let mut t = 0.0;
        while t < 8.0 {
            p.tick(&quiet(), t);
            let f = p.frame(t);
            let d = (f.e.open - prev.e.open).abs() + (f.e.lift - prev.e.lift).abs() / 10.0;
            assert!(d < 0.25, "jump at {t}: {d}");
            assert!((f.off.1 - prev.off.1).abs() < 12.0, "offset jump at {t}");
            prev = f;
            t += 1.0 / 30.0;
        }
        assert!(
            p.frame(8.0).e.close_to(&Expr::SAD, 0.05),
            "held sad after lost-one"
        );
        let mut a = player();
        let mut b = player();
        for k in 0..300 {
            let t = k as f64 / 30.0;
            a.tick(&quiet(), t);
            b.tick(&quiet(), t);
            assert_eq!(a.frame(t), b.frame(t));
        }
    }
}

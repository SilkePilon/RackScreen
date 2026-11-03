//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

use rackscreen_core::p2::ui::ScreenUi;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Timelike;
use rackscreen_core::anim::Secs;
use rackscreen_core::event::Event;
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::night::is_night;
use rackscreen_core::p2::status::night_scene;
use rackscreen_core::shows::schedule::{Request, ShowScheduler};
use rackscreen_core::shows::world::Layout;
use rackscreen_core::theme::Role;
use rackscreen_display::{DisplayCmd, Mailbox};
use rackscreen_render::frame::Orient;

use crate::night::{NightFrame, NightPhase};
use crate::worker::{Job, ScreenWorker};

pub struct ScreenSlot {
    /// Physical position, top to bottom; the model is asked for `scene(index, now)`.
    pub index: usize,
    /// Role this panel was opened as (calibrate pattern, logging); the model owns
    /// which role is actually shown.
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    /// Rolls, glides and marquees of whatever role this panel is showing.
    pub ui: ScreenUi,
    /// Brightness applied while packing (by the screen's render worker).
    pub brightness: f32,
}

impl ScreenSlot {
    pub fn new(
        index: usize,
        role: Role,
        orient: Orient,
        mailbox: Mailbox,
        brightness: f32,
    ) -> Self {
        Self {
            index,
            role,
            orient,
            mailbox,
            ui: ScreenUi::new(),
            brightness,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NightWindow {
    pub enabled: bool,
    pub start_min: u32,
    pub end_min: u32,
}

/// The next frame deadline: one tick after the last, or `now` when the loop
/// fell more than a tick behind it (no burst of catch-up frames).
pub fn advance(deadline: Instant, tick: Duration, now: Instant) -> Instant {
    let next = deadline + tick;
    if next + tick < now {
        now
    } else {
        next
    }
}

/// Local wall clock as `(minutes since midnight, unix seconds, seconds east of UTC)`;
/// one clock read per tick.
fn clock() -> (u32, i64, i32) {
    let t = chrono::Local::now();
    (
        t.hour() * 60 + t.minute(),
        t.timestamp(),
        t.offset().local_minus_utc(),
    )
}

/// Sends `Quit` to every display thread and raises `stop` when dropped, so an
/// early return (renderer setup failure) or a panic in the render loop never
/// leaves display threads blocked in `Mailbox::take` or main waiting for a
/// signal that will not come.
pub struct ShutdownGuard {
    mailboxes: Vec<Mailbox>,
    stop: Arc<AtomicBool>,
}

impl ShutdownGuard {
    pub fn new(mailboxes: Vec<Mailbox>, stop: Arc<AtomicBool>) -> Self {
        Self { mailboxes, stop }
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for mb in &self.mailboxes {
            mb.put(DisplayCmd::Quit);
        }
    }
}

pub struct RenderLoop {
    pub rx: Receiver<Event>,
    pub screens: Vec<ScreenSlot>,
    /// Role list per physical screen, same order as `screens`.
    pub screens_roles: Vec<Vec<Role>>,
    /// Dwell per screen before cycling to its next role.
    pub cycle_secs: Vec<Secs>,
    pub thresholds: Thresholds,
    pub night: NightWindow,
    pub fps: u32,
    pub stop: Arc<AtomicBool>,
    /// Whether an Electricity Maps token is configured; without one the
    /// electricity screens show "no token" instead of "waiting for data".
    pub token_present: bool,
    /// `location.lat`/`lon` are set; without them the sky screens show a pin.
    pub location_present: bool,
    /// A GitHub token is configured; without one `gh-activity` shows a key.
    pub github_token_present: bool,
    /// An arr API key is configured: the media card never shows the torrent.
    pub arr_configured: bool,
    /// `jellyfin.url` is set; without it the role says "Check config".
    pub jellyfin_configured: bool,
    /// Only one screen may transition at a time: four at once stall the SPI bus.
    pub one_at_a_time: bool,
    /// The four-screen show schedule, the world layout and the screen order.
    pub shows: ShowScheduler,
    pub show_layout: Layout,
    pub show_order: Vec<usize>,
    /// The play-now request file; `None` when there is no home directory.
    pub play_path: Option<PathBuf>,
}

/// How often the play-now request file is checked.
const PLAY_POLL: Duration = Duration::from_secs(1);

/// Consume a play-now request, if one is waiting: refused at night (it is
/// dropped, not kept for the morning) and with fewer than two screens.
fn poll_play(path: &Path, model: &mut Model, phase: NightPhase) {
    let req = match crate::play::take(path) {
        None => return,
        Some(Err(e)) => {
            tracing::warn!("play request ignored: {e:#}");
            return;
        }
        Some(Ok(r)) => r,
    };
    let name = match req {
        Request::Id(d) => d.id,
        Request::Random => "random",
    };
    if phase != NightPhase::Day {
        tracing::info!("play request {name} dropped: night mode");
    } else if model.show_run().is_some() {
        tracing::info!("play request {name} dropped: a show is already playing");
    } else if model.request_show(req) {
        tracing::info!("play request: {name}");
    } else {
        tracing::info!("play request {name} dropped: shows need at least two screens");
    }
}

impl RenderLoop {
    pub fn run(mut self) -> Result<()> {
        let _guard = ShutdownGuard::new(
            self.screens.iter().map(|s| s.mailbox.clone()).collect(),
            self.stop.clone(),
        );
        // Declared after `_guard`, so the render threads are joined before the
        // guard sends `Quit` to the display threads.
        let workers: Vec<ScreenWorker> = self
            .screens
            .iter()
            .map(|s| {
                ScreenWorker::spawn(s.index, s.orient.clone(), s.mailbox.clone(), s.brightness)
            })
            .collect::<Result<_>>()?;
        let mut force_full = true;
        let start = Instant::now();
        let mut model = Model::new(self.thresholds);
        model.set_screens(self.screens_roles.clone(), self.cycle_secs.clone());
        model.set_one_at_a_time(self.one_at_a_time);
        // A fresh seed per boot, so the screens do not cycle in the same order
        // after every restart.
        model.set_rng_seed(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(1)
                | 1,
        );
        model.set_token_present(self.token_present);
        model.set_location_present(self.location_present);
        model.set_github_token_present(self.github_token_present);
        model.set_arr_configured(self.arr_configured);
        model.set_jellyfin_configured(self.jellyfin_configured);
        let shows = std::mem::replace(&mut self.shows, ShowScheduler::new(false, 15, Vec::new()));
        model.set_shows(
            shows,
            self.show_layout,
            std::mem::take(&mut self.show_order),
        );
        let mut next_play_poll = Instant::now();
        let mut show_started: Option<Secs> = None;
        let tick = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let mut phase = NightPhase::Day;
        let mut stats = crate::stats::FrameStats::new(self.screens.len(), 60.0, 0.0);
        let mut deadline = Instant::now();

        while !self.stop.load(Ordering::Relaxed) {
            let now = start.elapsed().as_secs_f64();
            // the clock first: events bucketed by local day and hour read it
            let (minutes, unix, offset) = clock();
            model.set_unix_now(unix);
            model.set_utc_offset_secs(offset);
            while let Ok(ev) = self.rx.try_recv() {
                model.apply(ev, now);
            }

            let night = match model.night_override() {
                Some(v) => v,
                None => {
                    self.night.enabled
                        && is_night(minutes, self.night.start_min, self.night.end_min)
                }
            };

            let prev = phase;
            phase = phase.step(night, now);
            if phase != prev {
                tracing::info!("night mode: {prev:?} -> {phase:?}");
            }
            if let Some(path) = &self.play_path {
                if Instant::now() >= next_play_poll {
                    next_play_poll = Instant::now() + PLAY_POLL;
                    poll_play(path, &mut model, phase);
                }
            }
            // Shows start only in `Day`; the phase is known before the tick,
            // so none starts on the tick the night fade begins.
            model.set_night_day(phase == NightPhase::Day);
            model.tick(now);
            let run = model.show_run().copied();
            if run.map(|r| r.started) != show_started {
                if let Some(r) = run {
                    tracing::info!("show: {} ({:.1} s)", r.def.id, r.total());
                }
                show_started = run.map(|r| r.started);
            }
            // The moon is up and still. The same scene goes to every worker each
            // idle tick: it is skipped for free, but a panel whose last push
            // failed (`needs_full`) gets a full frame again within 500 ms.
            if prev == NightPhase::Icon && phase == NightPhase::Icon {
                for w in &workers {
                    w.send(Job {
                        scene: night_scene(1.0),
                        fade: None,
                        force_full: false,
                    })?;
                }
                for w in &workers {
                    w.wait()?;
                }
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            if prev == NightPhase::Icon && phase != NightPhase::Icon {
                // the hours under the moon belong to no stats period
                for s in &self.screens {
                    let _ = s.mailbox.take_push_stats();
                }
                stats.reset(now);
            }
            if prev.shows_icon() && phase == NightPhase::Day {
                for s in &self.screens {
                    // pushes from before the night belong to no period
                    let _ = s.mailbox.take_push_stats();
                }
                force_full = true;
                // the night is not part of the stats period
                stats.reset(now);
                // the first frame after the night is not late
                deadline = Instant::now();
                model.tick(now);
            }
            let frame = phase.frame(now);
            let fade = match frame {
                NightFrame::Fade(f) => Some(f),
                NightFrame::Day | NightFrame::Icon(_) => None,
            };

            for (i, s) in self.screens.iter_mut().enumerate() {
                let t = Instant::now();
                let scene = match frame {
                    NightFrame::Icon(a) => night_scene(a),
                    NightFrame::Day | NightFrame::Fade(_) => model.scene(s.index, now, &mut s.ui),
                };
                stats
                    .screen(i)
                    .build
                    .push(t.elapsed().as_secs_f32() * 1000.0);
                workers[i].send(Job {
                    scene,
                    fade,
                    force_full,
                })?;
            }
            force_full = false;
            for (i, w) in workers.iter().enumerate() {
                let d = w.wait()?;
                let st = stats.screen(i);
                if d.skipped {
                    st.skipped += 1;
                } else {
                    st.render.push(d.render_ms);
                    st.flush.push(d.flush_ms);
                }
            }
            if stats.due(now) {
                for (i, s) in self.screens.iter().enumerate() {
                    tracing::info!(
                        "{}",
                        stats.line(i, s.mailbox.take_push_stats(), stats.elapsed(now))
                    );
                }
                stats.reset(now);
            }

            deadline = advance(deadline, tick, Instant::now());
            if let Some(d) = deadline.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
        }
        // `_guard` sends Quit to every mailbox and sets `stop` on drop.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_keeps_cadence_and_resyncs_when_far_behind() {
        let t0 = Instant::now();
        let tick = Duration::from_millis(16);
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(5)), t0 + tick);
        // one tick late: keep the cadence, the next sleep is short
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(20)), t0 + tick);
        // more than a tick behind the next deadline: start over from now
        let late = t0 + Duration::from_millis(50);
        assert_eq!(advance(t0, tick, late), late);
    }

    #[test]
    fn shutdown_guard_quits_mailboxes_and_sets_stop() {
        let a = Mailbox::new();
        let b = Mailbox::new();
        let stop = Arc::new(AtomicBool::new(false));
        let guard = ShutdownGuard::new(vec![a.clone(), b.clone()], stop.clone());
        assert!(a.try_take().is_none());
        drop(guard);
        assert!(stop.load(Ordering::Relaxed));
        assert!(matches!(a.take(), DisplayCmd::Quit));
        assert!(matches!(b.take(), DisplayCmd::Quit));
    }
}

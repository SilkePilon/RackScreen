//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

use rackscreen_core::p2::ui::ScreenUi;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Timelike;
use rackscreen_core::anim::Secs;
use rackscreen_core::event::Event;
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::night::{bedtime_near, is_night};
use rackscreen_core::theme::Role;
use rackscreen_display::{DisplayCmd, Mailbox};
use rackscreen_render::frame::Orient;
use tiny_skia::Pixmap;

use crate::worker::{Job, ScreenWorker};

const NIGHT_FADE_SECS: Secs = 1.0;

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

/// Sleepy around the night window; never when night is disabled or forced.
pub fn face_bedtime(night: &NightWindow, minutes: u32, override_: Option<bool>) -> bool {
    night.enabled && override_.is_none() && bedtime_near(minutes, night.start_min, night.end_min)
}

#[cfg(test)]
mod bedtime_tests {
    use super::*;

    #[test]
    fn bedtime_needs_night_enabled_and_no_override() {
        let on = NightWindow {
            enabled: true,
            start_min: 1380,
            end_min: 420,
        };
        let off = NightWindow {
            enabled: false,
            ..on
        };
        assert!(face_bedtime(&on, 1360, None));
        assert!(!face_bedtime(&off, 1360, None));
        assert!(!face_bedtime(&on, 1360, Some(false)));
        assert!(!face_bedtime(&on, 720, None));
    }
}

/// Multiply every channel by `k` (0 = black, 1 = unchanged).
pub fn darken(px: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    for p in px.data_mut().as_chunks_mut::<4>().0 {
        p[0] = (p[0] as f32 * k) as u8;
        p[1] = (p[1] as f32 * k) as u8;
        p[2] = (p[2] as f32 * k) as u8;
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
    /// Only one screen may iris at a time: four at once stall the SPI bus.
    pub one_at_a_time: bool,
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
        let tick = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let mut asleep = false;
        let mut fade_started: Option<Secs> = None;
        let mut stats = crate::stats::FrameStats::new(self.screens.len(), 60.0, 0.0);
        let mut deadline = Instant::now();

        while !self.stop.load(Ordering::Relaxed) {
            let now = start.elapsed().as_secs_f64();
            // the clock first: events bucketed by local day and hour read it
            let (minutes, unix, offset) = clock();
            model.set_local_slot(minutes / 15);
            model.set_unix_now(unix);
            model.set_utc_offset_secs(offset);
            while let Ok(ev) = self.rx.try_recv() {
                model.apply(ev, now);
            }
            model.set_bedtime_near(face_bedtime(&self.night, minutes, model.night_override()));
            model.tick(now);

            let night = match model.night_override() {
                Some(v) => v,
                None => {
                    self.night.enabled
                        && is_night(minutes, self.night.start_min, self.night.end_min)
                }
            };

            if night && asleep {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            if !night && asleep {
                tracing::info!("night mode over, waking displays");
                for s in &self.screens {
                    s.mailbox.put(DisplayCmd::Wake);
                    // pushes from before the night belong to no period
                    let _ = s.mailbox.take_push_stats();
                }
                asleep = false;
                force_full = true;
                // the night is not part of the stats period
                stats.reset(now);
                // the first frame after waking is not late
                deadline = Instant::now();
                model.tick(now);
            }
            let fade = if night {
                let f0 = *fade_started.get_or_insert(now);
                Some((((now - f0) / NIGHT_FADE_SECS) as f32).min(1.0))
            } else {
                fade_started = None;
                None
            };

            for (i, s) in self.screens.iter_mut().enumerate() {
                let t = Instant::now();
                let scene = model.scene(s.index, now, &mut s.ui);
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

            if fade == Some(1.0) {
                tracing::info!("night mode, sleeping displays");
                for s in &self.screens {
                    s.mailbox.put(DisplayCmd::Sleep);
                }
                asleep = true;
                continue;
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
    fn darken_scales_channels() {
        let mut p = Pixmap::new(2, 1).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        darken(&mut p, 0.5);
        assert_eq!(p.data()[0], 127);
        darken(&mut p, 0.0);
        assert_eq!(p.data()[0], 0);
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

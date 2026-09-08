//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

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
use rackscreen_core::theme::Role;
use rackscreen_display::{DisplayCmd, Mailbox};
use rackscreen_render::frame::{dirty_rect, new_pixmap, Orient, Rect};
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

const NIGHT_FADE_SECS: Secs = 1.0;

pub struct ScreenSlot {
    /// Physical position, top to bottom; the model is asked for `scene(index, now)`.
    pub index: usize,
    /// Role this panel was opened as (calibrate pattern, logging); the model owns
    /// which role is actually shown.
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    cur: Pixmap,
    oriented: Pixmap,
    prev: Pixmap,
    first: bool,
}

impl ScreenSlot {
    pub fn new(index: usize, role: Role, orient: Orient, mailbox: Mailbox) -> Self {
        Self {
            index,
            role,
            orient,
            mailbox,
            cur: new_pixmap(),
            oriented: new_pixmap(),
            prev: new_pixmap(),
            first: true,
        }
    }

    /// Orient the freshly rendered frame, diff, push. Returns the dirty rect pushed.
    fn flush(&mut self) -> Option<Rect> {
        self.orient.apply(&self.cur, &mut self.oriented);
        let rect = if self.first {
            Some(Rect::full())
        } else {
            dirty_rect(&self.prev, &self.oriented)
        };
        self.first = false;
        if let Some(r) = rect {
            self.mailbox
                .put(DisplayCmd::Frame(self.oriented.clone(), r));
        }
        std::mem::swap(&mut self.prev, &mut self.oriented);
        rect
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NightWindow {
    pub enabled: bool,
    pub start_min: u32,
    pub end_min: u32,
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

/// Local wall clock as `(hour, minutes since midnight, unix seconds, seconds east of UTC)`;
/// one clock read per tick.
fn clock() -> (u32, u32, i64, i32) {
    let t = chrono::Local::now();
    (
        t.hour(),
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
    /// Only one screen may iris at a time: four at once stall the SPI bus.
    pub one_at_a_time: bool,
}

impl RenderLoop {
    pub fn run(mut self) -> Result<()> {
        let _guard = ShutdownGuard::new(
            self.screens.iter().map(|s| s.mailbox.clone()).collect(),
            self.stop.clone(),
        );
        let mut renderer = Renderer::new()?;
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
        model.apply(Event::Boot, 0.0);
        let tick = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let mut asleep = false;
        let mut fade_started: Option<Secs> = None;

        while !self.stop.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            let now = start.elapsed().as_secs_f64();
            while let Ok(ev) = self.rx.try_recv() {
                model.apply(ev, now);
            }
            let (hour, minutes, unix, offset) = clock();
            model.set_local_hour(hour);
            model.set_unix_now(unix);
            model.set_utc_offset_secs(offset);
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
                }
                asleep = false;
                for s in &mut self.screens {
                    s.first = true;
                }
                model.apply(Event::Boot, now);
                model.tick(now);
            }
            let fade = if night {
                let f0 = *fade_started.get_or_insert(now);
                Some((((now - f0) / NIGHT_FADE_SECS) as f32).min(1.0))
            } else {
                fade_started = None;
                None
            };

            for s in &mut self.screens {
                let scene = model.scene(s.index, now);
                renderer.render(&scene, &mut s.cur);
                if let Some(f) = fade {
                    darken(&mut s.cur, 1.0 - f);
                }
                s.flush();
            }

            if fade == Some(1.0) {
                tracing::info!("night mode, sleeping displays");
                for s in &self.screens {
                    s.mailbox.put(DisplayCmd::Sleep);
                }
                asleep = true;
                continue;
            }

            let spent = t0.elapsed();
            if spent < tick {
                std::thread::sleep(tick - spent);
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

    #[test]
    fn first_flush_is_full_then_dirty_only() {
        let mb = Mailbox::new();
        let mut slot = ScreenSlot::new(0, Role::Cpu, Orient::identity(), mb.clone());
        assert_eq!(slot.flush(), Some(Rect::full()));
        assert!(matches!(mb.take(), DisplayCmd::Frame(_, r) if r == Rect::full()));
        assert_eq!(slot.flush(), None, "identical frame pushes nothing");
        slot.cur.data_mut()[(10 * 240 + 10) * 4] = 200;
        assert_eq!(
            slot.flush(),
            Some(Rect {
                x: 10,
                y: 10,
                w: 1,
                h: 1
            })
        );
    }
}

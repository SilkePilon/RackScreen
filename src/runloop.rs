//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
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
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    cur: Pixmap,
    oriented: Pixmap,
    prev: Pixmap,
    first: bool,
}

impl ScreenSlot {
    pub fn new(role: Role, orient: Orient, mailbox: Mailbox) -> Self {
        Self { role, orient, mailbox, cur: new_pixmap(), oriented: new_pixmap(), prev: new_pixmap(), first: true }
    }

    /// Orient the freshly rendered frame, diff, push. Returns the dirty rect pushed.
    fn flush(&mut self) -> Option<Rect> {
        self.orient.apply(&self.cur, &mut self.oriented);
        let rect = if self.first { Some(Rect::full()) } else { dirty_rect(&self.prev, &self.oriented) };
        self.first = false;
        if let Some(r) = rect {
            self.mailbox.put(DisplayCmd::Frame(self.oriented.clone(), r));
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
    for p in px.data_mut().chunks_exact_mut(4) {
        p[0] = (p[0] as f32 * k) as u8;
        p[1] = (p[1] as f32 * k) as u8;
        p[2] = (p[2] as f32 * k) as u8;
    }
}

fn local_minutes() -> u32 {
    use chrono::Timelike;
    let t = chrono::Local::now();
    t.hour() * 60 + t.minute()
}

pub struct RenderLoop {
    pub rx: Receiver<Event>,
    pub screens: Vec<ScreenSlot>,
    pub thresholds: Thresholds,
    pub night: NightWindow,
    pub fps: u32,
    pub stop: Arc<AtomicBool>,
}

impl RenderLoop {
    pub fn run(mut self) -> Result<()> {
        let mut renderer = Renderer::new()?;
        let start = Instant::now();
        let mut model = Model::new(self.thresholds);
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
            model.tick(now);

            let night = match model.night_override() {
                Some(v) => v,
                None => self.night.enabled && is_night(local_minutes(), self.night.start_min, self.night.end_min),
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
                let scene = model.scene(s.role, now);
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
        for s in &self.screens {
            s.mailbox.put(DisplayCmd::Quit);
        }
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
    fn first_flush_is_full_then_dirty_only() {
        let mb = Mailbox::new();
        let mut slot = ScreenSlot::new(Role::Cpu, Orient::identity(), mb.clone());
        assert_eq!(slot.flush(), Some(Rect::full()));
        assert!(matches!(mb.take(), DisplayCmd::Frame(_, r) if r == Rect::full()));
        assert_eq!(slot.flush(), None, "identical frame pushes nothing");
        slot.cur.data_mut()[(10 * 240 + 10) * 4] = 200;
        assert_eq!(slot.flush(), Some(Rect { x: 10, y: 10, w: 1, h: 1 }));
    }
}

//! One render thread per screen: skip unchanged scenes, render, darken for
//! the night fade, diff, pack and hand the frame to the display thread.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{anyhow, Result};
use rackscreen_core::scene::Scene;
use rackscreen_display::Mailbox;
use rackscreen_render::frame::{
    brightness_lut, darken, dirty_rects, new_pixmap, pack_oriented, Orient, Rect,
};
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

pub struct Job {
    pub scene: Scene,
    /// Night fade progress, `Some(1.0)` = black.
    pub fade: Option<f32>,
    /// Push the whole frame (at start and after the night).
    pub force_full: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Done {
    pub render_ms: f32,
    pub flush_ms: f32,
    pub skipped: bool,
}

/// Frame buffers and the last scene of one screen.
pub struct ScreenFrames {
    orient: Orient,
    mailbox: Mailbox,
    /// Brightness applied while packing.
    lut: [u8; 256],
    /// The frame being rendered, unrotated.
    cur: Pixmap,
    /// The last pushed frame, unrotated.
    prev: Pixmap,
    first: bool,
    /// Scene and fade of the last rendered frame.
    last: Option<(Scene, Option<f32>)>,
}

impl ScreenFrames {
    pub fn new(orient: Orient, mailbox: Mailbox, brightness: f32) -> Self {
        Self {
            orient,
            mailbox,
            lut: brightness_lut(brightness),
            cur: new_pixmap(),
            prev: new_pixmap(),
            first: true,
            last: None,
        }
    }

    /// Render and push one frame, or skip it when scene and fade match the
    /// last one (the panel already shows it).
    pub fn run(&mut self, r: &mut Renderer, job: Job) -> Done {
        // `|` not `||`: the needs-full flag is taken every frame
        if job.force_full | self.mailbox.take_needs_full() {
            self.first = true;
        }
        let unchanged =
            !self.first && matches!(&self.last, Some((s, f)) if *f == job.fade && *s == job.scene);
        if unchanged {
            return Done {
                skipped: true,
                ..Done::default()
            };
        }
        let t = Instant::now();
        r.render(&job.scene, &mut self.cur);
        if let Some(f) = job.fade {
            darken(&mut self.cur, 1.0 - f);
        }
        let render_ms = t.elapsed().as_secs_f32() * 1000.0;
        let t = Instant::now();
        self.flush();
        let flush_ms = t.elapsed().as_secs_f32() * 1000.0;
        self.last = Some((job.scene, job.fade));
        Done {
            render_ms,
            flush_ms,
            skipped: false,
        }
    }

    /// Diff the fresh frame against the last pushed one (both unrotated), pack
    /// the changed panel rects, push. Returns the panel rects pushed.
    pub fn flush(&mut self) -> Option<Vec<Rect>> {
        let src = if self.first {
            vec![Rect::full()]
        } else {
            dirty_rects(&self.prev, &self.cur)
        };
        self.first = false;
        if src.is_empty() {
            return None;
        }
        let panel: Vec<Rect> = src.iter().map(|r| self.orient.map_rect(*r)).collect();
        let (cur, orient, lut) = (&self.cur, &self.orient, &self.lut);
        self.mailbox
            .put_frame(&panel, |rs| pack_oriented(cur, orient, rs, lut));
        std::mem::swap(&mut self.prev, &mut self.cur);
        Some(panel)
    }
}

/// A persistent render thread for one screen. The renderer is built on the
/// thread (its caches need not be `Send`).
pub struct ScreenWorker {
    jobs: Option<SyncSender<Job>>,
    done: Receiver<Done>,
    handle: Option<JoinHandle<()>>,
}

impl ScreenWorker {
    pub fn spawn(index: usize, orient: Orient, mailbox: Mailbox, brightness: f32) -> Result<Self> {
        let (jobs, job_rx) = sync_channel::<Job>(1);
        let (done_tx, done) = sync_channel::<Done>(1);
        let (ready_tx, ready_rx) = sync_channel::<Result<()>>(1);
        let handle = std::thread::Builder::new()
            .name(format!("render-{index}"))
            .spawn(move || {
                let mut renderer = match Renderer::new() {
                    Ok(r) => {
                        let _ = ready_tx.send(Ok(()));
                        r
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let mut frames = ScreenFrames::new(orient, mailbox, brightness);
                for job in job_rx {
                    if done_tx.send(frames.run(&mut renderer, job)).is_err() {
                        break;
                    }
                }
            })?;
        ready_rx
            .recv()
            .map_err(|_| anyhow!("render worker {index} died at start"))??;
        Ok(Self {
            jobs: Some(jobs),
            done,
            handle: Some(handle),
        })
    }

    pub fn send(&self, job: Job) -> Result<()> {
        self.jobs
            .as_ref()
            .expect("jobs sender lives until drop")
            .send(job)
            .map_err(|_| anyhow!("render worker stopped"))
    }

    pub fn wait(&self) -> Result<Done> {
        self.done
            .recv()
            .map_err(|_| anyhow!("render worker stopped"))
    }
}

impl Drop for ScreenWorker {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::scene::{Drawable, Scene};
    use rackscreen_core::theme::Color;
    use rackscreen_display::DisplayCmd;
    use rackscreen_render::frame::Rect;

    fn scene(x: f32) -> Scene {
        let mut s = Scene::new();
        s.push(Drawable::RoundRect {
            x,
            y: 100.0,
            w: 10.0,
            h: 10.0,
            r: 2.0,
            color: Color::hex(0xFFFFFF),
            alpha: 1.0,
        });
        s
    }

    fn job(s: Scene) -> Job {
        Job {
            scene: s,
            fade: None,
            force_full: false,
        }
    }

    #[test]
    fn same_scene_twice_is_skipped() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        assert!(!f.run(&mut r, job(scene(50.0))).skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == vec![Rect::full()]));
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
        assert!(mb.try_take().is_none());
        assert!(!f.run(&mut r, job(scene(60.0))).skipped);
        assert!(
            matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects.len() == 1 && fr.rects[0] != Rect::full())
        );
    }

    #[test]
    fn force_full_and_fade_change_are_not_skipped() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        f.run(&mut r, job(scene(50.0)));
        let _ = mb.take();
        let d = f.run(
            &mut r,
            Job {
                scene: scene(50.0),
                fade: None,
                force_full: true,
            },
        );
        assert!(!d.skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == vec![Rect::full()]));
        let d = f.run(
            &mut r,
            Job {
                scene: scene(50.0),
                fade: Some(0.5),
                force_full: false,
            },
        );
        assert!(!d.skipped);
    }

    #[test]
    fn failed_push_forces_the_next_frame_full() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        f.run(&mut r, job(scene(50.0)));
        let _ = mb.take();
        // the display thread failed to push that frame
        mb.set_needs_full();
        let d = f.run(&mut r, job(scene(50.0)));
        assert!(!d.skipped, "the panel may be stale: not skipped");
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == vec![Rect::full()]));
        assert!(!mb.take_needs_full(), "taken once");
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
    }

    #[test]
    fn worker_thread_renders_and_reports() {
        let mb = Mailbox::new();
        let w = ScreenWorker::spawn(0, Orient::identity(), mb.clone(), 1.0).unwrap();
        w.send(job(scene(50.0))).unwrap();
        let d = w.wait().unwrap();
        assert!(!d.skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(_)));
    }

    #[test]
    fn flush_maps_rects_to_panel_space() {
        let mb = Mailbox::new();
        let mut frames = ScreenFrames::new(Orient::new(90, false), mb.clone(), 1.0);
        assert_eq!(frames.flush(), Some(vec![Rect::full()]));
        assert!(matches!(mb.take(), DisplayCmd::Frame(f) if f.rects == vec![Rect::full()]));
        assert_eq!(frames.flush(), None, "identical frame pushes nothing");
        // source pixel (10, 20) lands at panel (219, 10) for a 90° turn
        frames.cur.data_mut()[(20 * 240 + 10) * 4] = 200;
        assert_eq!(
            frames.flush(),
            Some(vec![Rect {
                x: 219,
                y: 10,
                w: 1,
                h: 1
            }])
        );
    }
}

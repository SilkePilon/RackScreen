//! One render thread per screen: skip unchanged scenes, render, darken for
//! the night fade, diff, pack and hand the frame to the display thread.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{anyhow, Result};
use rackscreen_core::scene::{Blend, Scene};
use rackscreen_core::transition::composite;
use rackscreen_display::Mailbox;
use rackscreen_render::frame::{
    brightness_lut, copy_rect, darken, dirty_rects, new_pixmap, pack_oriented, video_rects, Orient,
    Rect,
};
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

use crate::hub::FrameTap;

pub struct Job {
    pub scene: Scene,
    /// The incoming scene of a two-frame transition: both are rendered and
    /// composited in the transition's style.
    pub blend: Option<Blend>,
    /// Night fade progress, `Some(1.0)` = black.
    pub fade: Option<f32>,
    /// Push the whole frame (at start and after the night).
    pub force_full: bool,
}

impl Job {
    pub fn single(scene: Scene, fade: Option<f32>, force_full: bool) -> Job {
        Job {
            scene,
            blend: None,
            fade,
            force_full,
        }
    }
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
    /// The two sides of a transition, kept once allocated; only touched
    /// while a two-frame style plays.
    sides: Option<(Pixmap, Pixmap)>,
    first: bool,
    /// Scene, blend and fade of the last rendered frame.
    last: Option<(Scene, Option<Blend>, Option<f32>)>,
    /// The web UI's live view, fed only while someone watches.
    tap: Option<(usize, Arc<FrameTap>)>,
    /// A frame was rendered since the tap last got a copy.
    tap_dirty: bool,
}

impl ScreenFrames {
    pub fn new(orient: Orient, mailbox: Mailbox, brightness: f32) -> Self {
        Self {
            orient,
            mailbox,
            lut: brightness_lut(brightness),
            cur: new_pixmap(),
            prev: new_pixmap(),
            sides: None,
            first: true,
            last: None,
            tap: None,
            tap_dirty: false,
        }
    }

    /// Feed the live view of screen `index` through `tap`.
    pub fn with_tap(mut self, index: usize, tap: Arc<FrameTap>) -> Self {
        self.tap = Some((index, tap));
        self
    }

    /// Copy the latest frame (`prev` after a flush) into the tap when it
    /// wants one: there are watchers and the throttle allows it. Called on
    /// every job, so a change that fell in a throttled slot still reaches
    /// the browser once the slot frees up.
    fn feed_tap(&mut self) {
        let Some((index, tap)) = &self.tap else {
            return;
        };
        if !self.tap_dirty {
            return;
        }
        let now = Instant::now();
        if tap.wants(*index, now) {
            tap.publish(*index, self.prev.data(), now);
            self.tap_dirty = false;
        }
    }

    /// Render and push one frame, or skip it when scene and fade match the
    /// last one (the panel already shows it).
    pub fn run(&mut self, r: &mut Renderer, job: Job) -> Done {
        // `|` not `||`: the needs-full flag is taken every frame
        if job.force_full | self.mailbox.take_needs_full() {
            self.first = true;
        }
        let unchanged = !self.first
            && matches!(&self.last, Some((s, b, f)) if *f == job.fade && *s == job.scene && *b == job.blend);
        if unchanged {
            self.feed_tap();
            return Done {
                skipped: true,
                ..Done::default()
            };
        }
        let t = Instant::now();
        match &job.blend {
            None => r.render(&job.scene, &mut self.cur),
            Some(b) => {
                let (from, to) = self
                    .sides
                    .get_or_insert_with(|| (new_pixmap(), new_pixmap()));
                r.render(&job.scene, from);
                r.render(&b.to, to);
                composite(b.kind, from.data(), to.data(), self.cur.data_mut(), b.t);
            }
        }
        if let Some(f) = job.fade {
            darken(&mut self.cur, 1.0 - f);
        }
        let render_ms = t.elapsed().as_secs_f32() * 1000.0;
        let video = job.blend.is_none()
            && job.fade.is_none()
            && job
                .scene
                .items
                .iter()
                .any(|d| matches!(d, rackscreen_core::scene::Drawable::Video(_)));
        let t = Instant::now();
        self.flush_as(video);
        let flush_ms = t.elapsed().as_secs_f32() * 1000.0;
        self.last = Some((job.scene, job.blend, job.fade));
        self.tap_dirty = true;
        self.feed_tap();
        Done {
            render_ms,
            flush_ms,
            skipped: false,
        }
    }

    /// Diff the fresh frame against the last pushed one (both unrotated), pack
    /// the changed panel rects, push. Returns the panel rects pushed.
    pub fn flush(&mut self) -> Option<Vec<Rect>> {
        self.flush_as(false)
    }

    /// `video`: the frame is a moving picture; push only what changed by
    /// more than one RGB565 step from what the panel shows, and keep `prev`
    /// equal to the panel (only the pushed parts are copied into it).
    pub fn flush_as(&mut self, video: bool) -> Option<Vec<Rect>> {
        let first = self.first;
        let src = if first {
            vec![Rect::full()]
        } else if video {
            video_rects(&self.prev, &self.cur)
        } else {
            dirty_rects(&self.prev, &self.cur)
        };
        self.first = false;
        if src.is_empty() {
            return None;
        }
        let panel: Vec<Rect> = src.iter().map(|r| self.orient.map_rect(*r)).collect();
        let (cur, orient, lut) = (&self.cur, &self.orient, &self.lut);
        let mut sent: Vec<Rect> = Vec::new();
        self.mailbox.put_frame(&panel, |rs| {
            sent.extend_from_slice(rs);
            pack_oriented(cur, orient, rs, lut)
        });
        if video && !first {
            // what the panel got: the pushed rects, plus any of an unsent
            // frame they were merged with, all taken from `cur`
            let back = self.orient.inverse();
            for r in sent {
                copy_rect(&self.cur, &mut self.prev, back.map_rect(r));
            }
        } else {
            std::mem::swap(&mut self.prev, &mut self.cur);
        }
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
    pub fn spawn(
        index: usize,
        orient: Orient,
        mailbox: Mailbox,
        brightness: f32,
        tap: Option<Arc<FrameTap>>,
    ) -> Result<Self> {
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
                if let Some(tap) = tap {
                    frames = frames.with_tap(index, tap);
                }
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

    /// A full frame as the mailbox sends it: cut to the round panel.
    fn full_round() -> Vec<Rect> {
        rackscreen_render::frame::round_rects(&[Rect::full()], 240)
    }

    fn job(s: Scene) -> Job {
        Job::single(s, None, false)
    }

    #[test]
    fn video_pushes_only_changes_beyond_a_step_from_what_the_panel_shows() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::new(270, false), mb.clone(), 1.0);
        let fill = |p: &mut Pixmap, v: u8| {
            for c in p.data_mut().as_chunks_mut::<4>().0 {
                *c = [v, v, v, 255];
            }
        };
        fill(&mut f.cur, 100);
        assert!(f.flush_as(true).is_some(), "the first frame is full");
        let _ = mb.take();
        // within a step of the panel: nothing to push
        fill(&mut f.cur, 106);
        assert!(f.flush_as(true).is_none());
        assert!(mb.try_take().is_none());
        // only 6 from the last frame, but 12 from what the panel shows
        fill(&mut f.cur, 112);
        assert!(f.flush_as(true).is_some());
        assert!(matches!(mb.take(), DisplayCmd::Frame(_)));
        let mid = (120 * 240 + 120) * 4;
        assert_eq!(f.prev.data()[mid], 112, "the panel copy follows the push");
    }

    #[test]
    fn same_scene_twice_is_skipped() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        assert!(!f.run(&mut r, job(scene(50.0))).skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == full_round()));
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
        let d = f.run(&mut r, Job::single(scene(50.0), None, true));
        assert!(!d.skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == full_round()));
        let d = f.run(&mut r, Job::single(scene(50.0), Some(0.5), false));
        assert!(!d.skipped);
    }

    #[test]
    fn a_blend_renders_both_scenes_and_composites() {
        use rackscreen_core::scene::Blend;
        use rackscreen_core::transition::{Direction, Kind, Style};
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        let blend = |t: f32| {
            Some(Blend {
                to: scene(200.0),
                kind: Kind::new(Style::Wipe, Direction::Right).unwrap(),
                t,
            })
        };
        let job = |t: f32| Job {
            scene: scene(20.0),
            blend: blend(t),
            fade: None,
            force_full: false,
        };
        assert!(!f.run(&mut r, job(0.5)).skipped);
        let _ = mb.take();
        // half way through a rightward wipe: the old rect (x 20) is covered
        // by the new frame's black, the new rect (x 200) not yet revealed
        fn px(f: &ScreenFrames, x: u32, y: u32) -> u8 {
            f.prev.data()[((y * 240 + x) * 4) as usize]
        }
        assert_eq!(px(&f, 25, 105), 0, "old rect gone");
        assert_eq!(px(&f, 205, 105), 0, "new rect not yet there");
        // the same blend again is skipped; a different t is not
        assert!(f.run(&mut r, job(0.5)).skipped);
        assert!(!f.run(&mut r, job(0.95)).skipped);
        let _ = mb.take();
        assert_eq!(px(&f, 205, 105), 255, "the new rect, revealed");
        assert_eq!(px(&f, 25, 105), 0);
        // the night fade darkens the composite too
        let d = f.run(
            &mut r,
            Job {
                fade: Some(0.5),
                ..job(0.95)
            },
        );
        assert!(!d.skipped);
        let _ = mb.take();
        let v = px(&f, 205, 105);
        assert!(v < 200 && v > 0, "{v}");
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
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == full_round()));
        assert!(!mb.take_needs_full(), "taken once");
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
    }

    #[test]
    fn worker_thread_renders_and_reports() {
        let mb = Mailbox::new();
        let w = ScreenWorker::spawn(0, Orient::identity(), mb.clone(), 1.0, None).unwrap();
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
        assert!(matches!(mb.take(), DisplayCmd::Frame(f) if f.rects == full_round()));
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

    #[test]
    fn tap_gets_frames_only_while_watched() {
        let mb = Mailbox::new();
        let tap = Arc::new(FrameTap::new(1));
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0).with_tap(0, tap.clone());
        let mut r = Renderer::new().unwrap();
        f.run(&mut r, job(scene(50.0)));
        assert_eq!(tap.seq(0), 0, "nobody watches: no copy");
        let guard = tap.watch();
        // the frame is unchanged, but the browser never saw it: copied now
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
        assert_eq!(tap.seq(0), 1);
        let frame = tap.latest(0).unwrap();
        assert_eq!(frame.rgba.len(), 240 * 240 * 4);
        assert!(frame.rgba.iter().any(|b| *b != 0), "has the white rect");
        // the same frame again: nothing new to copy
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
        assert_eq!(tap.seq(0), 1);
        // a change inside the throttle window waits for the next job
        assert!(!f.run(&mut r, job(scene(60.0))).skipped);
        assert_eq!(tap.seq(0), 1, "throttled");
        std::thread::sleep(std::time::Duration::from_millis(110));
        assert!(f.run(&mut r, job(scene(60.0))).skipped);
        assert_eq!(tap.seq(0), 2, "copied once the slot is free");
        drop(guard);
        std::thread::sleep(std::time::Duration::from_millis(110));
        assert!(!f.run(&mut r, job(scene(70.0))).skipped);
        assert_eq!(tap.seq(0), 2, "no watchers again");
    }
}

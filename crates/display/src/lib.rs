//! Display backends and the per-screen mailbox between render loop and display threads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use rackscreen_render::frame::merge_rects;

pub use rackscreen_render::frame::Rect;

#[cfg(feature = "pi")]
pub mod gc9a01;
#[cfg(feature = "sim")]
pub mod sim;

/// Big-endian RGB565 for one or more panel rectangles; the bytes of each
/// rect follow the previous one.
#[derive(Clone, Debug, PartialEq)]
pub struct PackedFrame {
    pub rects: Vec<Rect>,
    pub bytes: Vec<u8>,
}

impl PackedFrame {
    pub fn slices(&self) -> impl Iterator<Item = (Rect, &[u8])> {
        let mut off = 0;
        self.rects.iter().map(move |r| {
            let n = r.area() as usize * 2;
            let s = &self.bytes[off..off + n];
            off += n;
            (*r, s)
        })
    }
}

/// RGB565 byte count for `rects`.
fn packed_len(rects: &[Rect]) -> usize {
    rects.iter().map(|r| r.area() as usize * 2).sum()
}

pub trait Display: Send {
    fn push(&mut self, frame: &PackedFrame) -> Result<()>;
    fn sleep(&mut self) -> Result<()>;
    fn wake(&mut self) -> Result<()>;
}

#[derive(Debug)]
pub enum DisplayCmd {
    Frame(PackedFrame),
    Quit,
}

/// What is waiting for the display thread. Frames accumulate (the rects of an
/// untaken frame join the next one, which is packed from the newest pixels) so
/// a slow panel never misses a region that changed in a frame it did not get
/// to push. A pending control
/// command is delivered before the pending frame and never causes a frame to
/// be dropped.
#[derive(Default)]
struct Slot {
    frame: Option<PackedFrame>,
    control: Option<DisplayCmd>,
}

/// Push timings recorded by the display thread; read and reset by the stats logger.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PushStats {
    pub frames: u32,
    pub total_ms: f64,
    pub max_ms: f64,
    pub bytes: u64,
}

/// Per-screen mailbox between the render loop and one display thread.
#[derive(Clone)]
pub struct Mailbox {
    inner: Arc<(Mutex<Slot>, Condvar)>,
    stats: Arc<Mutex<PushStats>>,
    /// Set by the display thread when a push failed: the panel may not show
    /// the frame the render side diffs against, so the next one goes out whole.
    needs_full: Arc<AtomicBool>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(Slot::default()), Condvar::new())),
            stats: Arc::new(Mutex::new(PushStats::default())),
            needs_full: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Ask the render side to push its next frame whole.
    pub fn set_needs_full(&self) {
        self.needs_full.store(true, Ordering::Relaxed);
    }

    /// Whether a full frame was asked for since the last call; clears it.
    pub fn take_needs_full(&self) -> bool {
        self.needs_full.swap(false, Ordering::Relaxed)
    }

    pub fn record_push(&self, ms: f64, bytes: u64) {
        let mut s = self.stats.lock().unwrap();
        s.frames += 1;
        s.total_ms += ms;
        s.max_ms = s.max_ms.max(ms);
        s.bytes += bytes;
    }

    pub fn take_push_stats(&self) -> PushStats {
        std::mem::take(&mut *self.stats.lock().unwrap())
    }

    /// Queue a control command. A `Frame` given here replaces the pending
    /// frame as is; use `put_frame` for rendered frames.
    pub fn put(&self, cmd: DisplayCmd) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        match cmd {
            DisplayCmd::Frame(f) => {
                debug_assert_eq!(f.bytes.len(), packed_len(&f.rects), "frame bytes vs rects");
                slot.frame = Some(f)
            }
            control => slot.control = Some(control),
        }
        cv.notify_one();
    }

    /// Queue a rendered frame. `rects` changed since the previous frame. When
    /// the display thread has not taken the previous frame yet, its rects join
    /// this one so no region is lost, and `pack` is called once for the merged
    /// set: it must read the newest pixels. Runs under the mailbox lock.
    pub fn put_frame(&self, rects: &[Rect], pack: impl FnOnce(&[Rect]) -> Vec<u8>) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        let mut all = rects.to_vec();
        if let Some(old) = slot.frame.take() {
            all.extend(old.rects);
        }
        let full = Rect::full();
        let rects = merge_rects(all, full.w, full.h);
        let bytes = pack(&rects);
        debug_assert_eq!(bytes.len(), packed_len(&rects), "packed bytes vs rects");
        slot.frame = Some(PackedFrame { rects, bytes });
        cv.notify_one();
    }

    pub fn take(&self) -> DisplayCmd {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        loop {
            if let Some(cmd) = Self::pop(&mut slot) {
                return cmd;
            }
            slot = cv.wait(slot).unwrap();
        }
    }

    pub fn try_take(&self) -> Option<DisplayCmd> {
        Self::pop(&mut self.inner.0.lock().unwrap())
    }

    fn pop(slot: &mut Slot) -> Option<DisplayCmd> {
        if let Some(c) = slot.control.take() {
            return Some(c);
        }
        slot.frame.take().map(DisplayCmd::Frame)
    }
}

pub fn spawn_display_thread(
    name: String,
    mut display: Box<dyn Display>,
    mailbox: Mailbox,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("display-{name}"))
        .spawn(move || loop {
            let result = match mailbox.take() {
                DisplayCmd::Frame(f) => {
                    let t0 = std::time::Instant::now();
                    let r = display.push(&f);
                    if r.is_ok() {
                        mailbox
                            .record_push(t0.elapsed().as_secs_f64() * 1000.0, f.bytes.len() as u64);
                    } else {
                        // these rects never reached the panel
                        mailbox.set_needs_full();
                    }
                    r
                }
                DisplayCmd::Quit => {
                    let _ = display.sleep();
                    return;
                }
            };
            if let Err(e) = result {
                tracing::warn!(screen = %name, "display error: {e:#}");
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        })
        .expect("spawn display thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_for(rects: &[Rect], tag: u8) -> Vec<u8> {
        vec![tag; rects.iter().map(|r| r.area() as usize * 2).sum()]
    }

    #[test]
    fn put_frame_packs_the_given_rects() {
        let mb = Mailbox::new();
        let r = Rect {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        };
        mb.put_frame(&[r], |rs| bytes_for(rs, 7));
        match mb.take() {
            DisplayCmd::Frame(f) => {
                assert_eq!(f.rects, vec![r]);
                assert_eq!(f.bytes.len(), 24);
            }
            _ => panic!(),
        }
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn untaken_frame_rects_join_the_next_pack() {
        let mb = Mailbox::new();
        let a = Rect {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        };
        let b = Rect {
            x: 100,
            y: 100,
            w: 2,
            h: 2,
        };
        mb.put_frame(&[a], |rs| bytes_for(rs, 1));
        let mut seen = Vec::new();
        mb.put_frame(&[b], |rs| {
            seen = rs.to_vec();
            bytes_for(rs, 2)
        });
        assert_eq!(
            seen,
            vec![b, a],
            "pending rects are repacked from the newest pixels"
        );
        match mb.take() {
            DisplayCmd::Frame(f) => {
                assert_eq!(f.rects, vec![b, a]);
                assert!(f.bytes.iter().all(|&x| x == 2));
            }
            _ => panic!(),
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "packed bytes vs rects")]
    fn put_frame_rejects_a_short_pack() {
        Mailbox::new().put_frame(&[Rect::full()], |_| vec![0; 10]);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "frame bytes vs rects")]
    fn put_rejects_a_short_frame() {
        Mailbox::new().put(DisplayCmd::Frame(PackedFrame {
            rects: vec![Rect::full()],
            bytes: vec![0; 10],
        }));
    }

    #[test]
    fn needs_full_is_taken_once() {
        let mb = Mailbox::new();
        assert!(!mb.take_needs_full());
        mb.clone().set_needs_full();
        assert!(mb.take_needs_full());
        assert!(!mb.take_needs_full());
    }

    #[test]
    fn slices_split_bytes_per_rect() {
        let rects = vec![
            Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            Rect {
                x: 5,
                y: 5,
                w: 2,
                h: 1,
            },
        ];
        let f = PackedFrame {
            rects: rects.clone(),
            bytes: vec![1, 1, 2, 2, 2, 2],
        };
        let s: Vec<(Rect, Vec<u8>)> = f.slices().map(|(r, b)| (r, b.to_vec())).collect();
        assert_eq!(
            s,
            vec![(rects[0], vec![1, 1]), (rects[1], vec![2, 2, 2, 2])]
        );
    }

    #[test]
    fn frame_is_delivered_after_pending_control() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Quit);
        mb.put_frame(&[Rect::full()], |rs| bytes_for(rs, 1));
        assert!(matches!(mb.take(), DisplayCmd::Quit));
        assert!(matches!(mb.take(), DisplayCmd::Frame(..)));
        assert!(mb.try_take().is_none());
        mb.put_frame(&[Rect::full()], |rs| bytes_for(rs, 1));
        mb.put(DisplayCmd::Quit);
        assert!(matches!(mb.take(), DisplayCmd::Quit));
        assert!(matches!(mb.take(), DisplayCmd::Frame(..)));
    }

    #[test]
    fn push_stats_accumulate_and_reset() {
        let mb = Mailbox::new();
        mb.record_push(2.0, 100);
        mb.record_push(4.0, 300);
        let s = mb.take_push_stats();
        assert_eq!(
            (s.frames, s.total_ms, s.max_ms, s.bytes),
            (2, 6.0, 4.0, 400)
        );
        assert_eq!(mb.take_push_stats(), PushStats::default());
    }

    #[test]
    fn take_blocks_until_put() {
        let mb = Mailbox::new();
        let mb2 = mb.clone();
        let h = std::thread::spawn(move || mb2.take());
        std::thread::sleep(std::time::Duration::from_millis(50));
        mb.put(DisplayCmd::Quit);
        assert!(matches!(h.join().unwrap(), DisplayCmd::Quit));
    }
}

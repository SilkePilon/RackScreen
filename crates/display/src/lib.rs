//! Display backends and the per-screen mailbox between render loop and display threads.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use tiny_skia::Pixmap;

pub use rackscreen_render::frame::Rect;

#[cfg(feature = "pi")]
pub mod gc9a01;
#[cfg(feature = "sim")]
pub mod sim;

pub trait Display: Send {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> Result<()>;
    fn sleep(&mut self) -> Result<()>;
    fn wake(&mut self) -> Result<()>;
}

#[derive(Debug)]
pub enum DisplayCmd {
    Frame(Pixmap, Rect),
    Sleep,
    Wake,
    Quit,
}

/// What is waiting for the display thread. Frames accumulate (dirty rects are
/// unioned, the newest full pixmap is kept) so a slow panel never misses a
/// region that changed in a frame it did not get to push. A pending control
/// command is delivered before the pending frame and never causes a frame to
/// be dropped.
#[derive(Default)]
struct Slot {
    frame: Option<(Pixmap, Rect)>,
    control: Option<DisplayCmd>,
}

/// Per-screen mailbox between the render loop and one display thread.
#[derive(Clone)]
pub struct Mailbox {
    inner: Arc<(Mutex<Slot>, Condvar)>,
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
        }
    }

    pub fn put(&self, cmd: DisplayCmd) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        match cmd {
            DisplayCmd::Frame(px, rect) => {
                let rect = match slot.frame.take() {
                    Some((_, old)) => old.union(rect),
                    None => rect,
                };
                slot.frame = Some((px, rect));
            }
            control => slot.control = Some(control),
        }
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
        slot.frame.take().map(|(px, r)| DisplayCmd::Frame(px, r))
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
                DisplayCmd::Frame(px, rect) => display.push(&px, rect),
                DisplayCmd::Sleep => display.sleep(),
                DisplayCmd::Wake => display.wake(),
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

    fn frame_at(tag: u8, x: u32, y: u32) -> DisplayCmd {
        let mut p = Pixmap::new(2, 2).unwrap();
        p.data_mut()[0] = tag;
        DisplayCmd::Frame(p, Rect { x, y, w: 1, h: 1 })
    }

    fn frame(tag: u8) -> DisplayCmd {
        let mut p = Pixmap::new(2, 2).unwrap();
        p.data_mut()[0] = tag;
        DisplayCmd::Frame(
            p,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
        )
    }

    #[test]
    fn newer_frame_replaces_older() {
        let mb = Mailbox::new();
        mb.put(frame(1));
        mb.put(frame(2));
        match mb.take() {
            DisplayCmd::Frame(p, _) => assert_eq!(p.data()[0], 2),
            _ => panic!(),
        }
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn unconsumed_frames_accumulate_dirty_rect() {
        let mb = Mailbox::new();
        mb.put(frame_at(1, 10, 10));
        mb.put(frame_at(2, 100, 100));
        match mb.take() {
            DisplayCmd::Frame(p, r) => {
                assert_eq!(p.data()[0], 2, "newest pixmap is kept");
                assert_eq!(
                    r,
                    Rect {
                        x: 10,
                        y: 10,
                        w: 91,
                        h: 91
                    },
                    "dirty rect is the union of both frames"
                );
            }
            _ => panic!(),
        }
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn frame_is_delivered_after_pending_control() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Sleep);
        mb.put(frame(1));
        assert!(matches!(mb.take(), DisplayCmd::Sleep));
        assert!(matches!(mb.take(), DisplayCmd::Frame(p, _) if p.data()[0] == 1));
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn control_is_not_replaced_by_frame() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Sleep);
        mb.put(frame(1));
        assert!(matches!(mb.take(), DisplayCmd::Sleep));
        assert!(matches!(mb.take(), DisplayCmd::Frame(..)));
        mb.put(frame(1));
        mb.put(DisplayCmd::Quit);
        assert!(matches!(mb.take(), DisplayCmd::Quit));
        assert!(matches!(mb.take(), DisplayCmd::Frame(..)));
    }

    #[test]
    fn take_blocks_until_put() {
        let mb = Mailbox::new();
        let mb2 = mb.clone();
        let h = std::thread::spawn(move || mb2.take());
        std::thread::sleep(std::time::Duration::from_millis(50));
        mb.put(DisplayCmd::Wake);
        assert!(matches!(h.join().unwrap(), DisplayCmd::Wake));
    }
}

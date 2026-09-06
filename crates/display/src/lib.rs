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

impl DisplayCmd {
    fn is_control(&self) -> bool {
        !matches!(self, DisplayCmd::Frame(..))
    }
}

/// Single-slot mailbox. A newer frame replaces an unconsumed older frame;
/// control commands are never replaced by frames.
#[derive(Clone)]
pub struct Mailbox {
    inner: Arc<(Mutex<Option<DisplayCmd>>, Condvar)>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }

    pub fn put(&self, cmd: DisplayCmd) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        match (&*slot, &cmd) {
            (Some(existing), DisplayCmd::Frame(..)) if existing.is_control() => return,
            _ => *slot = Some(cmd),
        }
        cv.notify_one();
    }

    pub fn take(&self) -> DisplayCmd {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        loop {
            if let Some(cmd) = slot.take() {
                return cmd;
            }
            slot = cv.wait(slot).unwrap();
        }
    }

    pub fn try_take(&self) -> Option<DisplayCmd> {
        self.inner.0.lock().unwrap().take()
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
    fn control_is_not_replaced_by_frame() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Sleep);
        mb.put(frame(1));
        assert!(matches!(mb.take(), DisplayCmd::Sleep));
        mb.put(frame(1));
        mb.put(DisplayCmd::Quit);
        assert!(matches!(mb.take(), DisplayCmd::Quit));
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

//! Desktop simulator: one minifb window showing every panel as a round screen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use tiny_skia::Pixmap;

use rackscreen_render::frame::Rect;

use crate::Display;

const PANEL: usize = 240;
const PAD: usize = 20;
const CELL: usize = PANEL + PAD;
const BG: u32 = 0x0a0a0a;
const BEZEL_A: u32 = 0x141414;
const BEZEL_B: u32 = 0x202020;

pub struct SimHub {
    window: Window,
    buf: Arc<Mutex<Vec<u32>>>,
    w: usize,
    h: usize,
    key_tx: Sender<char>,
}

pub struct SimPanel {
    buf: Arc<Mutex<Vec<u32>>>,
    stride: usize,
    ox: usize,
    oy: usize,
    asleep: bool,
}

fn layout(n: usize, grid: bool) -> (usize, usize, Vec<(usize, usize)>) {
    if grid {
        let cols = 2;
        let rows = n.div_ceil(cols);
        let origins = (0..n)
            .map(|i| (PAD + (i % cols) * CELL, PAD + (i / cols) * CELL))
            .collect();
        (PAD + cols * CELL, PAD + rows * CELL, origins)
    } else {
        let origins = (0..n).map(|i| (PAD, PAD + i * CELL)).collect();
        (PAD + CELL, PAD + n * CELL, origins)
    }
}

/// The simulator's key map: everything the fake source understands.
fn key_char(key: Key) -> Option<char> {
    Some(match key {
        Key::Key1 => '1',
        Key::Key2 => '2',
        Key::Key3 => '3',
        Key::Key4 => '4',
        Key::Key5 => '5',
        Key::Key6 => '6',
        Key::Key7 => '7',
        Key::Key8 => '8',
        Key::Key9 => '9',
        Key::Key0 => '0',
        Key::T => 't',
        Key::N => 'n',
        Key::B => 'b',
        Key::H => 'h',
        Key::P => 'p',
        _ => return None,
    })
}

fn draw_bezel(buf: &mut [u32], stride: usize, ox: usize, oy: usize) {
    let c = PANEL as f32 / 2.0 - 0.5;
    for y in 0..PANEL {
        for x in 0..PANEL {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let v = if d <= 120.0 {
                0
            } else if d <= 125.0 {
                BEZEL_A
            } else if d <= 127.0 {
                BEZEL_B
            } else {
                continue;
            };
            buf[(oy + y) * stride + ox + x] = v;
        }
    }
}

impl SimHub {
    pub fn new(n: usize, grid: bool, key_tx: Sender<char>) -> Result<(SimHub, Vec<SimPanel>)> {
        let (w, h, origins) = layout(n, grid);
        let mut init = vec![BG; w * h];
        for &(ox, oy) in &origins {
            draw_bezel(&mut init, w, ox, oy);
        }
        let buf = Arc::new(Mutex::new(init));
        let window =
            Window::new("RackScreen sim", w, h, WindowOptions::default()).context("open window")?;
        let panels = origins
            .iter()
            .map(|&(ox, oy)| SimPanel {
                buf: buf.clone(),
                stride: w,
                ox,
                oy,
                asleep: false,
            })
            .collect();
        Ok((
            SimHub {
                window,
                buf,
                w,
                h,
                key_tx,
            },
            panels,
        ))
    }

    /// Blocks on the window loop until the window closes, Escape is pressed, or `stop` is set.
    pub fn run(mut self, stop: Arc<AtomicBool>) {
        self.window.set_target_fps(60);
        let mut frames = 0u32;
        let mut last = Instant::now();
        while self.window.is_open()
            && !self.window.is_key_down(Key::Escape)
            && !stop.load(Ordering::Relaxed)
        {
            for key in self.window.get_keys_pressed(KeyRepeat::No) {
                if let Some(ch) = key_char(key) {
                    let _ = self.key_tx.send(ch);
                }
            }
            let snapshot = self.buf.lock().unwrap().clone();
            if self
                .window
                .update_with_buffer(&snapshot, self.w, self.h)
                .is_err()
            {
                break;
            }
            frames += 1;
            if last.elapsed() >= Duration::from_secs(1) {
                self.window.set_title(&format!("RackScreen sim  {frames} fps  [1-8 events, 9/0 volume, h hot, p price outage, t torrent, n night, b boot, Esc quit]"));
                frames = 0;
                last = Instant::now();
            }
        }
        stop.store(true, Ordering::Relaxed);
    }
}

impl SimPanel {
    /// For tests: a panel drawing into a caller-provided buffer.
    pub fn with_buffer(buf: Arc<Mutex<Vec<u32>>>, stride: usize, ox: usize, oy: usize) -> Self {
        Self {
            buf,
            stride,
            ox,
            oy,
            asleep: false,
        }
    }

    fn blit(&self, frame: &Pixmap, dirty: Rect) {
        let c = PANEL as f32 / 2.0 - 0.5;
        let data = frame.data();
        let mut buf = self.buf.lock().unwrap();
        for y in dirty.y..dirty.y + dirty.h {
            for x in dirty.x..dirty.x + dirty.w {
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                if d > 120.0 {
                    continue;
                }
                let i = ((y * PANEL as u32 + x) * 4) as usize;
                let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
                buf[(self.oy + y as usize) * self.stride + self.ox + x as usize] = v;
            }
        }
    }
}

impl Display for SimPanel {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> anyhow::Result<()> {
        if self.asleep {
            return Ok(());
        }
        self.blit(frame, dirty);
        Ok(())
    }

    fn sleep(&mut self) -> anyhow::Result<()> {
        let mut black = Pixmap::new(PANEL as u32, PANEL as u32).unwrap();
        black.fill(tiny_skia::Color::BLACK);
        self.blit(&black, Rect::full());
        self.asleep = true;
        Ok(())
    }

    fn wake(&mut self) -> anyhow::Result<()> {
        self.asleep = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let (w, h, o) = layout(4, false);
        assert_eq!((w, h), (280, 1060));
        assert_eq!(o[3], (20, 800));
        let (w, h, o) = layout(4, true);
        assert_eq!((w, h), (540, 540));
        assert_eq!(o[3], (280, 280));
    }

    #[test]
    fn keys_forward_every_fake_command() {
        for (key, ch) in [
            (Key::Key9, '9'),
            (Key::Key0, '0'),
            (Key::H, 'h'),
            (Key::P, 'p'),
            (Key::Key1, '1'),
            (Key::T, 't'),
            (Key::N, 'n'),
            (Key::B, 'b'),
        ] {
            assert_eq!(key_char(key), Some(ch));
        }
        assert_eq!(key_char(Key::Escape), None);
    }

    #[test]
    fn panel_masks_to_circle_and_sleeps() {
        let stride = 280;
        let buf = Arc::new(Mutex::new(vec![BG; stride * 280]));
        {
            let mut b = buf.lock().unwrap();
            draw_bezel(&mut b, stride, 20, 20);
        }
        let mut panel = SimPanel::with_buffer(buf.clone(), stride, 20, 20);
        let mut white = Pixmap::new(240, 240).unwrap();
        white.fill(tiny_skia::Color::WHITE);
        panel.push(&white, Rect::full()).unwrap();
        {
            let b = buf.lock().unwrap();
            assert_eq!(
                b[(20 + 120) * stride + 20 + 120],
                0xffffff,
                "centre is white"
            );
            assert_eq!(
                b[(20) * stride + 20],
                BG,
                "corner outside circle keeps bezel background"
            );
        }
        panel.sleep().unwrap();
        panel.push(&white, Rect::full()).unwrap();
        assert_eq!(
            buf.lock().unwrap()[(20 + 120) * stride + 20 + 120],
            0,
            "asleep panel ignores frames"
        );
        panel.wake().unwrap();
        panel.push(&white, Rect::full()).unwrap();
        assert_eq!(
            buf.lock().unwrap()[(20 + 120) * stride + 20 + 120],
            0xffffff
        );
    }
}

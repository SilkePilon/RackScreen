//! GC9A01 240x240 round LCD over SPI, using rppal. Init sequence ported from the Python driver.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use rppal::gpio::{Gpio, OutputPin};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use tiny_skia::Pixmap;

use rackscreen_render::frame::{pack_rgb565, Rect};

use crate::Display;

#[derive(Clone, Copy, Debug)]
pub struct Pins {
    pub bus: u8,
    pub cs: u8,
    pub dc: u8,
    pub rst: u8,
    pub hz: u32,
}

pub enum Step {
    Cmd(u8, &'static [u8]),
    Delay(u64),
}

use Step::{Cmd, Delay};

pub const INIT: &[Step] = &[
    Cmd(0xEF, &[]),
    Cmd(0xEB, &[0x14]),
    Cmd(0xFE, &[]),
    Cmd(0xEF, &[]),
    Cmd(0xEB, &[0x14]),
    Cmd(0x84, &[0x40]),
    Cmd(0x85, &[0xFF]),
    Cmd(0x86, &[0xFF]),
    Cmd(0x87, &[0xFF]),
    Cmd(0x88, &[0x0A]),
    Cmd(0x89, &[0x21]),
    Cmd(0x8A, &[0x00]),
    Cmd(0x8B, &[0x80]),
    Cmd(0x8C, &[0x01]),
    Cmd(0x8D, &[0x01]),
    Cmd(0x8E, &[0xFF]),
    Cmd(0x8F, &[0xFF]),
    Cmd(0xB6, &[0x00, 0x20]),
    Cmd(0x36, &[0x08]),
    Cmd(0x3A, &[0x05]),
    Cmd(0x90, &[0x08, 0x08, 0x08, 0x08]),
    Cmd(0xBD, &[0x06]),
    Cmd(0xBC, &[0x00]),
    Cmd(0xFF, &[0x60, 0x01, 0x04]),
    Cmd(0xC3, &[0x13]),
    Cmd(0xC4, &[0x13]),
    Cmd(0xC9, &[0x22]),
    Cmd(0xBE, &[0x11]),
    Cmd(0xE1, &[0x10, 0x0E]),
    Cmd(0xDF, &[0x21, 0x0C, 0x02]),
    Cmd(0xF0, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]),
    Cmd(0xF1, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]),
    Cmd(0xF2, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]),
    Cmd(0xF3, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]),
    Cmd(0xED, &[0x1B, 0x0B]),
    Cmd(0xAE, &[0x77]),
    Cmd(0xCD, &[0x63]),
    Cmd(
        0x70,
        &[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03],
    ),
    Cmd(0xE8, &[0x34]),
    Cmd(
        0x62,
        &[
            0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70,
        ],
    ),
    Cmd(
        0x63,
        &[
            0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70,
        ],
    ),
    Cmd(0x64, &[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07]),
    Cmd(
        0x66,
        &[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00],
    ),
    Cmd(
        0x67,
        &[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98],
    ),
    Cmd(0x74, &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00]),
    Cmd(0x98, &[0x3E, 0x07]),
    Cmd(0x35, &[]),
    Cmd(0x21, &[]),
    Cmd(0x11, &[]),
    Delay(120),
    Cmd(0x29, &[]),
    Delay(20),
];

/// Column/row address bytes for an inclusive window.
pub fn window_bytes(x0: u16, y0: u16, x1: u16, y1: u16) -> ([u8; 4], [u8; 4]) {
    (
        [
            (x0 >> 8) as u8,
            (x0 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
        ],
        [
            (y0 >> 8) as u8,
            (y0 & 0xFF) as u8,
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
        ],
    )
}

/// Where the kernel reports the live spidev transfer buffer size.
pub const SPIDEV_BUFSIZ_PATH: &str = "/sys/module/spidev/parameters/bufsiz";
/// The kernel default when the sysfs value is missing or unreadable.
pub const DEFAULT_BUFSIZ: usize = 4096;

/// The chunk size to really use: the requested one, clamped to the kernel's live
/// `spidev.bufsiz` (writes larger than that fail with EMSGSIZE). A cmdline change only
/// takes effect after a reboot, so the config value may run ahead of the kernel.
pub fn effective_chunk(requested: usize, sysfs_value: Option<&str>) -> usize {
    let bufsiz = sysfs_value
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_BUFSIZ);
    requested.min(bufsiz)
}

pub struct Gc9a01 {
    spi: Spi,
    dc: OutputPin,
    rst: OutputPin,
    chunk: usize,
    brightness: f32,
}

impl Gc9a01 {
    pub fn open(pins: Pins, chunk: usize, brightness: f32) -> Result<Self> {
        let sysfs = std::fs::read_to_string(SPIDEV_BUFSIZ_PATH).ok();
        let effective = effective_chunk(chunk, sysfs.as_deref());
        if effective < chunk {
            tracing::info!(
                "spi_chunk {chunk} clamped to the kernel's spidev.bufsiz {effective} \
                 (reboot after enabling SPI to use the larger value)"
            );
        }
        let chunk = effective;
        let bus = match pins.bus {
            0 => Bus::Spi0,
            1 => Bus::Spi1,
            b => anyhow::bail!("unsupported spi bus {b}"),
        };
        let ss = match pins.cs {
            0 => SlaveSelect::Ss0,
            1 => SlaveSelect::Ss1,
            2 => SlaveSelect::Ss2,
            c => anyhow::bail!("unsupported chip select {c}"),
        };
        let spi = Spi::new(bus, ss, pins.hz, Mode::Mode0).context("open spi")?;
        let gpio = Gpio::new().context("open gpio")?;
        let dc = gpio.get(pins.dc).context("dc pin")?.into_output_low();
        let rst = gpio.get(pins.rst).context("rst pin")?.into_output_high();
        let mut d = Self {
            spi,
            dc,
            rst,
            chunk: chunk.max(64),
            brightness,
        };
        d.reset();
        d.init()?;
        d.clear()?;
        Ok(d)
    }

    fn reset(&mut self) {
        self.rst.set_high();
        sleep(Duration::from_millis(10));
        self.rst.set_low();
        sleep(Duration::from_millis(10));
        self.rst.set_high();
        sleep(Duration::from_millis(120));
    }

    fn cmd(&mut self, c: u8, data: &[u8]) -> Result<()> {
        self.dc.set_low();
        self.spi.write(&[c]).context("spi cmd")?;
        if !data.is_empty() {
            self.dc.set_high();
            self.spi.write(data).context("spi cmd data")?;
        }
        Ok(())
    }

    fn data(&mut self, bytes: &[u8]) -> Result<()> {
        self.dc.set_high();
        for part in bytes.chunks(self.chunk) {
            self.spi.write(part).context("spi data")?;
        }
        Ok(())
    }

    fn init(&mut self) -> Result<()> {
        for step in INIT {
            match step {
                Cmd(c, d) => self.cmd(*c, d)?,
                Delay(ms) => sleep(Duration::from_millis(*ms)),
            }
        }
        Ok(())
    }

    fn set_window(&mut self, r: Rect) -> Result<()> {
        let (col, row) = window_bytes(
            r.x as u16,
            r.y as u16,
            (r.x + r.w - 1) as u16,
            (r.y + r.h - 1) as u16,
        );
        self.cmd(0x2A, &col)?;
        self.cmd(0x2B, &row)?;
        self.cmd(0x2C, &[])
    }

    fn clear(&mut self) -> Result<()> {
        self.set_window(Rect::full())?;
        self.data(&vec![0u8; 240 * 240 * 2])
    }
}

impl Display for Gc9a01 {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> Result<()> {
        if dirty.is_empty() {
            return Ok(());
        }
        let bytes = pack_rgb565(frame, dirty, self.brightness);
        self.set_window(dirty)?;
        self.data(&bytes)
    }

    fn sleep(&mut self) -> Result<()> {
        self.clear()?;
        self.cmd(0x28, &[])?;
        sleep(Duration::from_millis(20));
        self.cmd(0x10, &[])?;
        sleep(Duration::from_millis(120));
        Ok(())
    }

    fn wake(&mut self) -> Result<()> {
        self.cmd(0x11, &[])?;
        sleep(Duration::from_millis(120));
        self.cmd(0x29, &[])?;
        sleep(Duration::from_millis(20));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_bytes_inclusive_big_endian() {
        let (c, r) = window_bytes(0, 0, 239, 239);
        assert_eq!(c, [0, 0, 0, 0xEF]);
        assert_eq!(r, [0, 0, 0, 0xEF]);
        let (c, _) = window_bytes(10, 5, 25, 9);
        assert_eq!(c, [0, 10, 0, 25]);
    }

    #[test]
    fn effective_chunk_clamps_to_live_bufsiz() {
        assert_eq!(effective_chunk(65536, Some("4096\n")), 4096);
        assert_eq!(effective_chunk(4096, Some("65536")), 4096);
        assert_eq!(effective_chunk(65536, None), 4096);
        assert_eq!(effective_chunk(65536, Some("65536")), 65536);
        assert_eq!(effective_chunk(65536, Some("garbage")), 4096);
    }

    #[test]
    fn init_sequence_ends_with_display_on() {
        let cmds: Vec<u8> = INIT
            .iter()
            .filter_map(|s| if let Cmd(c, _) = s { Some(*c) } else { None })
            .collect();
        assert_eq!(cmds.first(), Some(&0xEF));
        assert_eq!(cmds.last(), Some(&0x29));
        assert!(cmds.contains(&0x11));
        assert!(matches!(INIT[INIT.len() - 1], Delay(20)));
    }
}

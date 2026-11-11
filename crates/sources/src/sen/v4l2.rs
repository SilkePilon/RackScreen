//! The hardware H.264 decoder of the Raspberry Pi (and of any board with a
//! V4L2 stateful memory-to-memory decoder that hands out single-plane
//! I420): the picture layout and the bookkeeping here, the device itself in
//! `device`.
//!
//! Units go in with a timestamp that counts them; pictures come out some
//! time later carrying the timestamp of their unit, which tells both the
//! presentation time and whether the picture is from before a break.

#![cfg_attr(
    not(all(
        target_os = "linux",
        target_env = "gnu",
        target_pointer_width = "64",
        target_endian = "little"
    )),
    allow(dead_code)
)]

use rackscreen_core::video::Planes;

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    target_endian = "little"
))]
mod device;

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    target_endian = "little"
))]
pub use device::Decoder;

/// Where there is no V4L2: a decoder that never opens.
#[cfg(not(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    target_endian = "little"
)))]
pub struct Decoder(std::convert::Infallible);

#[cfg(not(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "64",
    target_endian = "little"
)))]
impl Decoder {
    pub fn open() -> anyhow::Result<Decoder> {
        anyhow::bail!("no V4L2 on this platform")
    }
    pub fn name(&self) -> &str {
        match self.0 {}
    }
    pub fn reset(&mut self) {}
    pub fn decode(
        &mut self,
        _data: &[u8],
        _pts: Option<u64>,
        _on_picture: &mut dyn FnMut(Option<u64>, &Planes),
    ) -> anyhow::Result<bool> {
        match self.0 {}
    }
}

/// Buffers for the bitstream, one unit each. The decoder takes a unit as
/// soon as it is queued, so few are in use at a time.
pub const OUTPUT_BUFFERS: u32 = 3;
/// The size asked for a bitstream buffer; the driver may give more (the
/// Pi's gives 512 KB). A 360p keyframe is some 40 KB.
pub const OUTPUT_BYTES: u32 = 256 * 1024;
/// Buffers for decoded pictures (350 KB each at 360p): one being filled,
/// one being read, two to spare. The decoder keeps its reference pictures
/// in its own memory.
pub const CAPTURE_BUFFERS: u32 = 4;
/// Units queued in a row without any picture coming out before the decoder
/// counts as stuck: three seconds of the stream.
pub const STARVED_UNITS: u32 = 90;

/// The visible part of a decoded picture inside its buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// Where the three planes of a single-plane I420 (`YU12`) buffer lie: the
/// luma rows `stride` apart over the coded (padded) height, then the two
/// chroma planes at half the stride and half the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: usize,
    pub height: usize,
    pub y_stride: usize,
    pub c_stride: usize,
    y: (usize, usize),
    u: (usize, usize),
    v: (usize, usize),
    /// Bytes a buffer has to hold.
    pub need: usize,
}

impl Layout {
    /// From what the driver reports: the coded size, the bytes per luma
    /// line and the visible rectangle (the whole picture without one).
    /// `None` when the numbers do not describe a picture.
    pub fn i420(
        coded_width: u32,
        coded_height: u32,
        bytesperline: u32,
        visible: Option<Rect>,
    ) -> Option<Layout> {
        let stride = bytesperline.max(coded_width) as usize;
        let rows = coded_height as usize;
        if stride == 0 || rows == 0 || stride > 8192 || rows > 8192 {
            return None;
        }
        if !stride.is_multiple_of(2) || !rows.is_multiple_of(2) {
            return None;
        }
        let vis = visible.unwrap_or(Rect {
            left: 0,
            top: 0,
            width: coded_width,
            height: coded_height,
        });
        let (left, top) = (vis.left as usize, vis.top as usize);
        let (width, height) = (vis.width as usize, vis.height as usize);
        if width < 16 || height < 16 || left + width > stride || top + height > rows {
            return None;
        }
        // chroma has no half positions
        if !left.is_multiple_of(2) || !top.is_multiple_of(2) {
            return None;
        }
        let c_stride = stride / 2;
        let y_size = stride * rows;
        let c_size = c_stride * (rows / 2);
        let c_off = (top / 2) * c_stride + left / 2;
        Some(Layout {
            width,
            height,
            y_stride: stride,
            c_stride,
            y: (top * stride + left, y_size),
            u: (y_size + c_off, y_size + c_size),
            v: (y_size + c_size + c_off, y_size + 2 * c_size),
            need: y_size + 2 * c_size,
        })
    }

    /// The planes of the picture in `buf`, read in place.
    pub fn planes<'a>(&self, buf: &'a [u8]) -> Option<Planes<'a>> {
        if buf.len() < self.need {
            return None;
        }
        let p = Planes {
            y: &buf[self.y.0..self.y.1],
            u: &buf[self.u.0..self.u.1],
            v: &buf[self.v.0..self.v.1],
            width: self.width,
            height: self.height,
            y_stride: self.y_stride,
            c_stride: self.c_stride,
        };
        p.valid().then_some(p)
    }
}

/// What a picture's timestamp says about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stamp {
    /// From a unit queued before the last break: not to be shown.
    Stale,
    /// A picture to show, with its presentation time if it is known.
    Fresh(Option<u64>),
}

/// Units remembered for their presentation time; more than a decoder
/// holds back.
const STAMPS: usize = 32;
/// The first unit's number. Timestamps below it are not ours (a driver
/// that does not copy them hands out zero).
const FIRST_UNIT: u64 = 100;
/// A unit's timestamp is its number in hundredths of a second.
const STAMP_STEP_US: i64 = 10_000;

/// Numbers the units that go into the decoder and reads the numbers back
/// from the pictures that come out.
pub struct Stamps {
    next: u64,
    /// Pictures of units before this one are stale.
    valid_from: u64,
    ring: [(u64, Option<u64>); STAMPS],
}

impl Default for Stamps {
    fn default() -> Self {
        Stamps {
            next: FIRST_UNIT,
            valid_from: FIRST_UNIT,
            ring: [(0, None); STAMPS],
        }
    }
}

impl Stamps {
    /// The timestamp (seconds, microseconds) for the next unit.
    pub fn push(&mut self, pts: Option<u64>) -> (i64, i64) {
        let n = self.next;
        self.next += 1;
        self.ring[(n % STAMPS as u64) as usize] = (n, pts);
        ((n / 100) as i64, (n % 100) as i64 * STAMP_STEP_US)
    }

    /// The units queued so far are from before a break.
    pub fn reset(&mut self) {
        self.valid_from = self.next;
    }

    /// What the picture with this timestamp is.
    pub fn take(&self, sec: i64, usec: i64) -> Stamp {
        let n = match (u64::try_from(sec), u64::try_from(usec)) {
            (Ok(s), Ok(us)) if us < 1_000_000 => s
                .saturating_mul(100)
                .saturating_add((us + STAMP_STEP_US as u64 / 2) / STAMP_STEP_US as u64),
            _ => return Stamp::Fresh(None),
        };
        if n < FIRST_UNIT || n >= self.next {
            // not a timestamp of ours: nothing is known about the picture
            return Stamp::Fresh(None);
        }
        if n < self.valid_from {
            return Stamp::Stale;
        }
        let (unit, pts) = self.ring[(n % STAMPS as u64) as usize];
        Stamp::Fresh(if unit == n { pts } else { None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_360p_picture_lies_in_a_buffer_padded_to_368_rows() {
        let vis = Rect {
            left: 0,
            top: 0,
            width: 640,
            height: 360,
        };
        let l = Layout::i420(640, 368, 640, Some(vis)).unwrap();
        assert_eq!((l.width, l.height), (640, 360));
        assert_eq!((l.y_stride, l.c_stride), (640, 320));
        assert_eq!(l.need, 640 * 368 * 3 / 2);
        // a buffer where every byte says which plane and row it is in
        let mut buf = vec![0u8; l.need];
        for row in 0..368 {
            buf[row * 640..][..640].fill(row as u8);
        }
        for row in 0..184 {
            buf[640 * 368 + row * 320..][..320].fill(100 + (row % 100) as u8);
            buf[640 * 368 + 320 * 184 + row * 320..][..320].fill(200 + (row % 50) as u8);
        }
        let p = l.planes(&buf).unwrap();
        assert!(p.valid());
        assert_eq!((p.width, p.height), (640, 360));
        assert_eq!(p.y[359 * p.y_stride + 639], 359u16 as u8);
        assert_eq!(p.u[0], 100);
        assert_eq!(p.u[179 * p.c_stride + 319], 179);
        assert_eq!(p.v[0], 200);
        assert_eq!(p.v[179 * p.c_stride + 319], 200 + 29);
        // a buffer that is too short is not read
        assert!(l.planes(&buf[..l.need - 1]).is_none());
    }

    #[test]
    fn a_wider_stride_and_an_offset_rectangle_are_followed() {
        let vis = Rect {
            left: 16,
            top: 8,
            width: 320,
            height: 180,
        };
        let l = Layout::i420(352, 192, 384, Some(vis)).unwrap();
        assert_eq!((l.y_stride, l.c_stride), (384, 192));
        let mut buf = vec![0u8; l.need];
        let (y_size, c_size) = (384 * 192, 192 * 96);
        buf[8 * 384 + 16] = 1;
        buf[y_size + 4 * 192 + 8] = 2;
        buf[y_size + c_size + 4 * 192 + 8] = 3;
        let p = l.planes(&buf).unwrap();
        assert_eq!((p.y[0], p.u[0], p.v[0]), (1, 2, 3));
        assert!(p.valid());
        // without a rectangle the coded size is the picture
        let l = Layout::i420(352, 192, 0, None).unwrap();
        assert_eq!((l.width, l.height, l.y_stride), (352, 192, 352));
    }

    #[test]
    fn numbers_that_are_no_picture_give_no_layout() {
        let rect = |left, top, width, height| {
            Some(Rect {
                left,
                top,
                width,
                height,
            })
        };
        assert!(Layout::i420(0, 0, 0, None).is_none());
        assert!(Layout::i420(640, 0, 640, None).is_none());
        assert!(Layout::i420(641, 368, 641, None).is_none(), "odd stride");
        assert!(Layout::i420(640, 361, 640, None).is_none(), "odd rows");
        assert!(Layout::i420(8, 8, 8, None).is_none(), "too small");
        assert!(Layout::i420(100_000, 368, 0, None).is_none());
        assert!(Layout::i420(640, 368, 640, rect(0, 0, 648, 360)).is_none());
        assert!(Layout::i420(640, 368, 640, rect(0, 16, 640, 360)).is_none());
        assert!(Layout::i420(640, 368, 640, rect(1, 0, 320, 180)).is_none());
        assert!(Layout::i420(640, 368, 640, rect(0, 0, 0, 0)).is_none());
        assert!(Layout::i420(640, 368, 640, rect(u32::MAX, u32::MAX, 640, 360)).is_none());
    }

    #[test]
    fn every_layout_that_is_given_out_holds_valid_planes() {
        for (w, h, bpl) in [
            (640, 368, 640),
            (640, 360, 672),
            (1920, 1088, 1920),
            (16, 16, 32),
        ] {
            for (left, top, dw, dh) in [(0, 0, 0, 0), (0, 0, 1, 3), (2, 4, 6, 8)] {
                let vis = Rect {
                    left,
                    top,
                    width: w - left - dw.min((w - left).saturating_sub(16)),
                    height: h - top - dh.min((h - top).saturating_sub(16)),
                };
                if let Some(l) = Layout::i420(w, h, bpl, Some(vis)) {
                    let buf = vec![0u8; l.need];
                    assert!(l.planes(&buf).is_some(), "{w}x{h} {vis:?}");
                }
            }
        }
    }

    #[test]
    fn timestamps_carry_the_presentation_time_through() {
        let mut s = Stamps::default();
        let a = s.push(Some(900_000));
        let b = s.push(None);
        let c = s.push(Some(906_000));
        assert_ne!(a, (0, 0), "zero is what a driver without timestamps gives");
        assert!(a < b && b < c);
        assert!(a.1 < 1_000_000);
        assert_eq!(s.take(a.0, a.1), Stamp::Fresh(Some(900_000)));
        assert_eq!(s.take(b.0, b.1), Stamp::Fresh(None));
        assert_eq!(s.take(c.0, c.1), Stamp::Fresh(Some(906_000)));
        // a driver may round the microseconds
        assert_eq!(s.take(c.0, c.1 + 999), Stamp::Fresh(Some(906_000)));
        assert_eq!(s.take(c.0, c.1 - 999), Stamp::Fresh(Some(906_000)));
    }

    #[test]
    fn pictures_from_before_a_break_are_stale() {
        let mut s = Stamps::default();
        let old = s.push(Some(1));
        s.reset();
        let new = s.push(Some(2));
        assert_eq!(s.take(old.0, old.1), Stamp::Stale);
        assert_eq!(s.take(new.0, new.1), Stamp::Fresh(Some(2)));
        // long after: still stale, and the times of units long gone are
        // not mixed up with newer ones in the same slot
        let mut last = new;
        for i in 0..1000 {
            last = s.push(Some(10 + i));
        }
        assert_eq!(s.take(old.0, old.1), Stamp::Stale);
        assert_eq!(s.take(new.0, new.1), Stamp::Fresh(None));
        assert_eq!(s.take(last.0, last.1), Stamp::Fresh(Some(1009)));
        // the seconds roll over into the next hundred units
        assert!(last.0 >= 10 && last.1 < 1_000_000);
    }

    #[test]
    fn timestamps_that_are_not_ours_say_nothing() {
        let mut s = Stamps::default();
        let a = s.push(Some(5));
        s.reset();
        // zero, negative, out of range, and a unit not queued yet
        assert_eq!(s.take(0, 0), Stamp::Fresh(None));
        assert_eq!(s.take(-1, 0), Stamp::Fresh(None));
        assert_eq!(s.take(0, -5), Stamp::Fresh(None));
        assert_eq!(s.take(1, 2_000_000), Stamp::Fresh(None));
        assert_eq!(s.take(a.0 + 50, 0), Stamp::Fresh(None));
        assert_eq!(s.take(i64::MAX, 999_999), Stamp::Fresh(None));
    }
}

//! Frame buffers, per-screen orientation, dirty rectangles and RGB565 packing.

use rackscreen_core::theme::layout::SIZE;
use tiny_skia::Pixmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn full() -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: SIZE,
            h: SIZE,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
    pub fn union(self, o: Rect) -> Rect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w).max(o.x + o.w);
        let y1 = (self.y + self.h).max(o.y + o.h);
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }
    pub fn intersects(&self, o: Rect) -> bool {
        !self.is_empty()
            && !o.is_empty()
            && self.x < o.x + o.w
            && o.x < self.x + self.w
            && self.y < o.y + o.h
            && o.y < self.y + self.h
    }
}

pub fn new_pixmap() -> Pixmap {
    let mut p = Pixmap::new(SIZE, SIZE).expect("pixmap");
    p.fill(tiny_skia::Color::BLACK);
    p
}

/// Maps destination pixels to source pixels for a rotation (clockwise, multiples of 90)
/// followed by an optional horizontal flip.
#[derive(Clone)]
pub struct Orient {
    size: u32,
    rotate: u32,
    hflip: bool,
    map: Vec<u32>,
}

impl Orient {
    pub fn identity() -> Orient {
        Orient::new_sized(SIZE, 0, false)
    }

    pub fn new(rotate_cw: u32, hflip: bool) -> Orient {
        Orient::new_sized(SIZE, rotate_cw, hflip)
    }

    pub fn new_sized(size: u32, rotate_cw: u32, hflip: bool) -> Orient {
        let n = size;
        let mut map = Vec::with_capacity((n * n) as usize);
        for y in 0..n {
            for x in 0..n {
                let (rx, ry) = if hflip { (n - 1 - x, y) } else { (x, y) };
                let (sx, sy) = match rotate_cw % 360 {
                    0 => (rx, ry),
                    90 => (ry, n - 1 - rx),
                    180 => (n - 1 - rx, n - 1 - ry),
                    270 => (n - 1 - ry, rx),
                    other => panic!("rotate must be a multiple of 90, got {other}"),
                };
                map.push(sy * n + sx);
            }
        }
        Orient {
            size: n,
            rotate: rotate_cw % 360,
            hflip,
            map,
        }
    }

    /// Where source pixel (sx, sy) lands on the panel.
    pub fn src_to_dst(&self, sx: u32, sy: u32) -> (u32, u32) {
        let n = self.size;
        let (rx, ry) = match self.rotate {
            0 => (sx, sy),
            90 => (n - 1 - sy, sx),
            180 => (n - 1 - sx, n - 1 - sy),
            270 => (sy, n - 1 - sx),
            _ => unreachable!("checked in new_sized"),
        };
        if self.hflip {
            (n - 1 - rx, ry)
        } else {
            (rx, ry)
        }
    }

    /// A source-space rectangle in panel coordinates.
    pub fn map_rect(&self, r: Rect) -> Rect {
        if r.is_empty() {
            return r;
        }
        let (ax, ay) = self.src_to_dst(r.x, r.y);
        let (bx, by) = self.src_to_dst(r.x + r.w - 1, r.y + r.h - 1);
        Rect {
            x: ax.min(bx),
            y: ay.min(by),
            w: ax.abs_diff(bx) + 1,
            h: ay.abs_diff(by) + 1,
        }
    }

    /// The orientation that maps panel pixels back to source pixels. A flip
    /// with a rotation is a reflection, its own inverse; a plain rotation is
    /// undone by the opposite one.
    pub fn inverse(&self) -> Orient {
        if self.hflip {
            self.clone()
        } else {
            Orient::new_sized(self.size, (360 - self.rotate) % 360, false)
        }
    }

    pub fn is_identity(&self) -> bool {
        self.map.iter().enumerate().all(|(i, &m)| i as u32 == m)
    }

    pub fn apply(&self, src: &Pixmap, dst: &mut Pixmap) {
        debug_assert_eq!(src.width(), self.size);
        let s = src.data();
        let d = dst.data_mut();
        for (i, &m) in self.map.iter().enumerate() {
            let (di, si) = (i * 4, m as usize * 4);
            d[di..di + 4].copy_from_slice(&s[si..si + 4]);
        }
    }
}

/// Smallest rectangle covering every pixel that differs. None when identical.
pub fn dirty_rect(prev: &Pixmap, next: &Pixmap) -> Option<Rect> {
    let w = prev.width() as usize;
    let h = prev.height() as usize;
    let a = prev.data();
    let b = next.data();
    let mut y0 = usize::MAX;
    let mut y1 = 0usize;
    let mut x0 = usize::MAX;
    let mut x1 = 0usize;
    for y in 0..h {
        let ra = &a[y * w * 4..(y + 1) * w * 4];
        let rb = &b[y * w * 4..(y + 1) * w * 4];
        if ra == rb {
            continue;
        }
        y0 = y0.min(y);
        y1 = y;
        let differs = |x: usize| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4];
        let first = (0..w).find(|&x| differs(x)).unwrap();
        let last = (0..w).rev().find(|&x| differs(x)).unwrap();
        x0 = x0.min(first);
        x1 = x1.max(last);
    }
    if y0 == usize::MAX {
        return None;
    }
    Some(Rect {
        x: x0 as u32,
        y: y0 as u32,
        w: (x1 - x0 + 1) as u32,
        h: (y1 - y0 + 1) as u32,
    })
}

/// Pack the given rectangle as big-endian RGB565 with a brightness multiplier.
pub fn pack_rgb565(px: &Pixmap, rect: Rect, brightness: f32) -> Vec<u8> {
    let w = px.width() as usize;
    let data = px.data();
    let mut out = Vec::with_capacity((rect.w * rect.h * 2) as usize);
    let k = brightness.clamp(0.0, 1.0);
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            let i = (y as usize * w + x as usize) * 4;
            let r = (data[i] as f32 * k) as u16;
            let g = (data[i + 1] as f32 * k) as u16;
            let b = (data[i + 2] as f32 * k) as u16;
            let v = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
            out.push((v >> 8) as u8);
            out.push((v & 0xFF) as u8);
        }
    }
    out
}

pub const MAX_RECTS: usize = 4;
/// Changed rows within this many rows of each other (8 rows apart still
/// counts) end up in one rect.
const BAND_GAP_ROWS: u32 = 8;
/// Above this share of the frame, one full window write is cheaper.
const FULL_FRACTION: f64 = 0.70;

fn merge_overlaps(rects: &mut Vec<Rect>) {
    'again: loop {
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if rects[i].intersects(rects[j]) {
                    rects[i] = rects[i].union(rects[j]);
                    rects.remove(j);
                    continue 'again;
                }
            }
        }
        return;
    }
}

/// Overlaps merged, at most `MAX_RECTS` (closest pairs merged first: least
/// added area), one full rect when they cover more than 70 % of `w`×`h`.
pub fn merge_rects(rects: Vec<Rect>, w: u32, h: u32) -> Vec<Rect> {
    merge_rects_max(rects, w, h, MAX_RECTS, FULL_FRACTION)
}

/// `merge_rects` with its limits given.
fn merge_rects_max(
    mut rects: Vec<Rect>,
    w: u32,
    h: u32,
    max_rects: usize,
    full_fraction: f64,
) -> Vec<Rect> {
    rects.retain(|r| !r.is_empty());
    merge_overlaps(&mut rects);
    while rects.len() > max_rects {
        let mut best = (0, 1, u64::MAX);
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let extra = rects[i]
                    .union(rects[j])
                    .area()
                    .saturating_sub(rects[i].area() + rects[j].area());
                if extra < best.2 {
                    best = (i, j, extra);
                }
            }
        }
        rects[best.0] = rects[best.0].union(rects[best.1]);
        rects.remove(best.1);
        merge_overlaps(&mut rects);
    }
    let total: u64 = rects.iter().map(Rect::area).sum();
    if total as f64 > full_fraction * (w as u64 * h as u64) as f64 {
        return vec![Rect { x: 0, y: 0, w, h }];
    }
    rects
}

/// Video frames: changed rows within this many rows of each other share a
/// rect. Finer than `BAND_GAP_ROWS`: a moving picture changes in many
/// scattered rows, and each window costs only ~470 pixels of wire time.
const VIDEO_GAP_ROWS: u32 = 2;
const VIDEO_MAX_RECTS: usize = 16;
const VIDEO_FULL_FRACTION: f64 = 0.85;
/// A video pixel counts as changed when one RGB565 channel moved by more
/// than one step from what the panel shows (the green channel's step is
/// half the size, so two of its steps): an error of at most one step, which
/// the next real change clears.
const VIDEO_TOLERANCE_STEPS: u8 = 1;

/// The panel's 565 steps of an 8-bit channel value: red and blue keep 5
/// bits, green 6 (counted in 5-bit steps by halving).
fn steps_differ(a: &[u8], b: &[u8]) -> bool {
    let d = |x: u8, y: u8, shift: u32| (x >> shift).abs_diff(y >> shift);
    d(a[0], b[0], 3) > VIDEO_TOLERANCE_STEPS
        || d(a[1], b[1], 2) > 2 * VIDEO_TOLERANCE_STEPS
        || d(a[2], b[2], 3) > VIDEO_TOLERANCE_STEPS
}

/// Changed regions of a video frame against what the panel shows (`shown`,
/// unrotated): only pixels off by more than `VIDEO_TOLERANCE_STEPS`, grouped
/// in fine row bands. About 40 % less to push than `dirty_rects` on the SEN
/// stream, where compression noise touches nearly every pixel a little.
pub fn video_rects(shown: &Pixmap, next: &Pixmap) -> Vec<Rect> {
    let w = shown.width() as usize;
    let h = shown.height() as usize;
    let (a, b) = (shown.data(), next.data());
    let mut bands: Vec<Rect> = Vec::new();
    for y in 0..h {
        let ra = &a[y * w * 4..(y + 1) * w * 4];
        let rb = &b[y * w * 4..(y + 1) * w * 4];
        let differs = |x: usize| steps_differ(&ra[x * 4..x * 4 + 3], &rb[x * 4..x * 4 + 3]);
        // only what the round panel shows: outside it nothing is ever sent
        let (lo, hi) = round_span(w as u32, y as u32, y as u32 + 1);
        let (lo, hi) = (lo as usize, hi as usize);
        let Some(x0) = (lo..hi).find(|&x| differs(x)) else {
            continue;
        };
        let x1 = (x0..hi).rev().find(|&x| differs(x)).unwrap_or(x0);
        let row = Rect {
            x: x0 as u32,
            y: y as u32,
            w: (x1 - x0 + 1) as u32,
            h: 1,
        };
        match bands.last_mut() {
            Some(last) if (y as u32) <= last.y + last.h - 1 + VIDEO_GAP_ROWS => {
                *last = last.union(row)
            }
            _ => bands.push(row),
        }
    }
    merge_rects_max(
        bands,
        w as u32,
        h as u32,
        VIDEO_MAX_RECTS,
        VIDEO_FULL_FRACTION,
    )
}

/// Copy `r` (source space) of `from` into `to`.
pub fn copy_rect(from: &Pixmap, to: &mut Pixmap, r: Rect) {
    let w = from.width() as usize;
    let (x0, x1) = (r.x as usize, (r.x + r.w) as usize);
    let src = from.data();
    let dst = to.data_mut();
    for y in r.y as usize..(r.y + r.h) as usize {
        let (i0, i1) = ((y * w + x0) * 4, (y * w + x1) * 4);
        dst[i0..i1].copy_from_slice(&src[i0..i1]);
    }
}

/// Rows per band when a rect is cut to the round panel.
const ROUND_BAND_ROWS: u32 = 30;
/// What one more panel window costs, in pixels of wire time: five small SPI
/// writes, about 0.15 ms against 2 bytes per pixel at 50 MHz.
const WINDOW_COST_PX: u64 = 470;
/// Columns kept beyond the exact circle on each side.
const ROUND_MARGIN: f32 = 1.0;

/// Columns `[x0, x1)` of a `size`-wide round panel that are visible on any of
/// rows `[y0, y1)`: those of the row nearest the centre.
fn round_span(size: u32, y0: u32, y1: u32) -> (u32, u32) {
    let c = size as f32 / 2.0;
    // the row whose centre is nearest the panel centre
    let y = (c - 0.5).clamp(y0 as f32, (y1 - 1) as f32);
    let d = (y + 0.5 - c).abs();
    let hw = (c * c - d * d).max(0.0).sqrt() + ROUND_MARGIN;
    let x0 = (c - hw).floor().max(0.0) as u32;
    let x1 = ((c + hw).ceil() as u32).min(size);
    (x0, x1)
}

/// `r` limited to the columns visible on its rows, or `None` when it lies
/// wholly outside the circle.
fn round_clip(size: u32, r: Rect) -> Option<Rect> {
    let (x0, x1) = round_span(size, r.y, r.y + r.h);
    let (a, b) = (r.x.max(x0), (r.x + r.w).min(x1));
    (a < b).then(|| Rect {
        x: a,
        w: b - a,
        ..r
    })
}

/// The panels are round: what lies outside the circle inscribed in the
/// `size`×`size` frame is never seen, about a fifth of a full frame. Each rect
/// is clipped to the columns visible on its rows, and cut into row bands
/// (each clipped on its own) when the pixels that saves outweigh the extra
/// windows. Rects wholly outside the circle are dropped. The circle is
/// symmetric, so this works the same in panel and source space.
pub fn round_rects(rects: &[Rect], size: u32) -> Vec<Rect> {
    let mut out = Vec::new();
    for &r in rects {
        if r.is_empty() {
            continue;
        }
        let one = round_clip(size, r);
        let mut bands = Vec::new();
        let mut y = r.y;
        while y < r.y + r.h {
            let end = ((y / ROUND_BAND_ROWS + 1) * ROUND_BAND_ROWS).min(r.y + r.h);
            if let Some(b) = round_clip(size, Rect { y, h: end - y, ..r }) {
                bands.push(b);
            }
            y = end;
        }
        let one_cost = one.map_or(0, |o| o.area());
        let bands_cost = bands.iter().map(Rect::area).sum::<u64>()
            + bands.len().saturating_sub(1) as u64 * WINDOW_COST_PX;
        if bands.len() > 1 && bands_cost < one_cost {
            out.extend(bands);
        } else {
            out.extend(one);
        }
    }
    out
}

/// Changed regions between two frames, grouped into row bands.
pub fn dirty_rects(prev: &Pixmap, next: &Pixmap) -> Vec<Rect> {
    let w = prev.width() as usize;
    let h = prev.height() as usize;
    let (a, b) = (prev.data(), next.data());
    let mut bands: Vec<Rect> = Vec::new();
    for y in 0..h {
        let ra = &a[y * w * 4..(y + 1) * w * 4];
        let rb = &b[y * w * 4..(y + 1) * w * 4];
        if ra == rb {
            continue;
        }
        let differs = |x: usize| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4];
        let x0 = (0..w).find(|&x| differs(x)).unwrap() as u32;
        let x1 = (0..w).rev().find(|&x| differs(x)).unwrap() as u32;
        let row = Rect {
            x: x0,
            y: y as u32,
            w: x1 - x0 + 1,
            h: 1,
        };
        match bands.last_mut() {
            Some(last) if (y as u32) <= last.y + last.h - 1 + BAND_GAP_ROWS => {
                *last = last.union(row)
            }
            _ => bands.push(row),
        }
    }
    merge_rects(bands, w as u32, h as u32)
}

/// `out[v] = v × brightness`, truncated like `pack_rgb565`.
pub fn brightness_lut(brightness: f32) -> [u8; 256] {
    let k = brightness.clamp(0.0, 1.0);
    std::array::from_fn(|v| (v as f32 * k) as u8)
}

/// Multiply every colour channel by `k` (0 = black, 1 = unchanged); alpha stays.
pub fn darken(px: &mut Pixmap, k: f32) {
    if k >= 1.0 {
        return;
    }
    let lut = brightness_lut(k);
    for p in px.data_mut().as_chunks_mut::<4>().0 {
        p[0] = lut[p[0] as usize];
        p[1] = lut[p[1] as usize];
        p[2] = lut[p[2] as usize];
    }
}

/// Big-endian RGB565 of panel-space `rects`, read from the unrotated `src`
/// through `orient`; the bytes of each rect follow the previous one.
pub fn pack_oriented(src: &Pixmap, orient: &Orient, rects: &[Rect], lut: &[u8; 256]) -> Vec<u8> {
    let n = orient.size as usize;
    let data = src.data();
    let cap: usize = rects.iter().map(|r| r.area() as usize * 2).sum();
    let mut out = Vec::with_capacity(cap);
    for r in rects {
        for y in r.y..r.y + r.h {
            let row =
                &orient.map[y as usize * n + r.x as usize..y as usize * n + (r.x + r.w) as usize];
            for &m in row {
                let i = m as usize * 4;
                let red = lut[data[i] as usize] as u16;
                let green = lut[data[i + 1] as usize] as u16;
                let blue = lut[data[i + 2] as usize] as u16;
                let v = ((red & 0xF8) << 8) | ((green & 0xFC) << 3) | (blue >> 3);
                out.extend_from_slice(&v.to_be_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(p: &mut Pixmap, x: u32, y: u32, rgb: (u8, u8, u8)) {
        let w = p.width() as usize;
        let i = (y as usize * w + x as usize) * 4;
        let d = p.data_mut();
        d[i] = rgb.0;
        d[i + 1] = rgb.1;
        d[i + 2] = rgb.2;
        d[i + 3] = 255;
    }

    #[test]
    fn the_inverse_orientation_undoes_every_orientation() {
        for rotate in [0, 90, 180, 270] {
            for hflip in [false, true] {
                let o = Orient::new_sized(240, rotate, hflip);
                let back = o.inverse();
                for p in [(0, 0), (5, 17), (239, 3), (120, 200)] {
                    let (x, y) = o.src_to_dst(p.0, p.1);
                    assert_eq!(back.src_to_dst(x, y), p, "{rotate} {hflip}");
                }
            }
        }
    }

    #[test]
    fn video_rects_skip_changes_within_one_step() {
        let mut a = new_pixmap();
        a.fill(tiny_skia::Color::from_rgba8(100, 100, 100, 255));
        let mut b = a.clone();
        // one 565 step or less: not worth a push
        for x in 0..240 {
            px(&mut b, x, 50, (106, 103, 104));
        }
        assert!(video_rects(&a, &b).is_empty());
        // more than a step, on two rows far apart: two thin rects
        px(&mut b, 30, 40, (120, 100, 100));
        px(&mut b, 200, 180, (100, 100, 80));
        let rs = video_rects(&a, &b);
        assert_eq!(rs.len(), 2, "{rs:?}");
        assert!(rs.iter().all(|r| r.h == 1 && r.w == 1));
    }

    fn get(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let w = p.width() as usize;
        let i = (y as usize * w + x as usize) * 4;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    fn marked(size: u32) -> Pixmap {
        let mut p = Pixmap::new(size, size).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        px(&mut p, 0, 0, (255, 0, 0)); // top-left red
        px(&mut p, size - 1, 0, (0, 255, 0)); // top-right green
        p
    }

    #[test]
    fn rotate_90_cw_moves_top_left_to_top_right() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 90, false).apply(&src, &mut dst);
        assert_eq!(get(&dst, 2, 0), (255, 0, 0));
        assert_eq!(get(&dst, 2, 2), (0, 255, 0));
    }

    #[test]
    fn rotate_270_cw_moves_top_left_to_bottom_left() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 270, false).apply(&src, &mut dst);
        assert_eq!(get(&dst, 0, 2), (255, 0, 0));
        assert_eq!(get(&dst, 0, 0), (0, 255, 0));
    }

    #[test]
    fn hflip_after_rotation() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 0, true).apply(&src, &mut dst);
        assert_eq!(get(&dst, 2, 0), (255, 0, 0));
        Orient::new_sized(3, 180, true).apply(&src, &mut dst);
        assert_eq!(get(&dst, 0, 2), (255, 0, 0));
    }

    #[test]
    fn identity_detected() {
        assert!(Orient::new_sized(4, 0, false).is_identity());
        assert!(!Orient::new_sized(4, 90, false).is_identity());
    }

    #[test]
    fn dirty_rect_bounds_changes() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        assert_eq!(dirty_rect(&a, &b), None);
        px(&mut b, 10, 20, (1, 2, 3));
        px(&mut b, 15, 25, (4, 5, 6));
        assert_eq!(
            dirty_rect(&a, &b),
            Some(Rect {
                x: 10,
                y: 20,
                w: 6,
                h: 6
            })
        );
    }

    #[test]
    fn pack_known_colours() {
        let mut p = Pixmap::new(2, 2).unwrap();
        px(&mut p, 0, 0, (255, 0, 0));
        px(&mut p, 1, 0, (0, 255, 0));
        px(&mut p, 0, 1, (0, 0, 255));
        px(&mut p, 1, 1, (255, 255, 255));
        let out = pack_rgb565(
            &p,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
            1.0,
        );
        assert_eq!(out, vec![0xF8, 0x00, 0x07, 0xE0, 0x00, 0x1F, 0xFF, 0xFF]);
        let half = pack_rgb565(
            &p,
            Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            0.5,
        );
        assert_eq!(half, vec![0x78, 0x00]);
        let sub = pack_rgb565(
            &p,
            Rect {
                x: 1,
                y: 1,
                w: 1,
                h: 1,
            },
            1.0,
        );
        assert_eq!(sub, vec![0xFF, 0xFF]);
    }

    #[test]
    fn rect_union() {
        let a = Rect {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        };
        let b = Rect {
            x: 5,
            y: 5,
            w: 1,
            h: 1,
        };
        assert_eq!(
            a.union(b),
            Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 6
            }
        );
    }

    #[test]
    fn darken_scales_channels_and_keeps_alpha() {
        let mut p = Pixmap::new(2, 1).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        darken(&mut p, 0.5);
        assert_eq!(&p.data()[..4], &[127, 127, 127, 255]);
        darken(&mut p, 0.0);
        assert_eq!(&p.data()[..4], &[0, 0, 0, 255]);
        let mut q = noise(8, 3);
        let before = q.data().to_vec();
        darken(&mut q, 1.0);
        assert_eq!(q.data(), &before[..], "k = 1 is a no-op");
    }

    fn noise(size: u32, seed: u32) -> Pixmap {
        let mut p = Pixmap::new(size, size).unwrap();
        let mut s = seed;
        for b in p.data_mut().as_chunks_mut::<4>().0 {
            for c in &mut b[..3] {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                *c = (s >> 24) as u8;
            }
            b[3] = 255;
        }
        p
    }

    const ORIENTS: [(u32, bool); 8] = [
        (0, false),
        (90, false),
        (180, false),
        (270, false),
        (0, true),
        (90, true),
        (180, true),
        (270, true),
    ];

    #[test]
    fn src_to_dst_inverts_the_map() {
        for (rot, flip) in ORIENTS {
            let o = Orient::new_sized(5, rot, flip);
            for dy in 0..5 {
                for dx in 0..5 {
                    let src = o.map[(dy * 5 + dx) as usize];
                    assert_eq!(o.src_to_dst(src % 5, src / 5), (dx, dy), "{rot} {flip}");
                }
            }
        }
    }

    #[test]
    fn map_rect_covers_the_same_pixels() {
        for (rot, flip) in ORIENTS {
            let o = Orient::new_sized(6, rot, flip);
            let r = Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 2,
            };
            let m = o.map_rect(r);
            assert_eq!(m.area(), r.area());
            for sy in r.y..r.y + r.h {
                for sx in r.x..r.x + r.w {
                    let (dx, dy) = o.src_to_dst(sx, sy);
                    assert!(dx >= m.x && dx < m.x + m.w && dy >= m.y && dy < m.y + m.h);
                }
            }
        }
    }

    #[test]
    fn pack_oriented_equals_orient_then_pack() {
        let src = noise(240, 7);
        for (rot, flip) in ORIENTS {
            let o = Orient::new(rot, flip);
            let mut oriented = Pixmap::new(240, 240).unwrap();
            o.apply(&src, &mut oriented);
            for k in [1.0f32, 0.6] {
                let panel = o.map_rect(Rect {
                    x: 10,
                    y: 30,
                    w: 50,
                    h: 20,
                });
                let old = pack_rgb565(&oriented, panel, k);
                let new = pack_oriented(&src, &o, &[panel], &brightness_lut(k));
                assert_eq!(old, new, "{rot} {flip} {k}");
            }
        }
    }

    #[test]
    fn dirty_rects_split_far_apart_changes() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        px(&mut b, 100, 5, (255, 255, 255));
        px(&mut b, 120, 200, (255, 255, 255));
        assert_eq!(
            dirty_rects(&a, &b),
            vec![
                Rect {
                    x: 100,
                    y: 5,
                    w: 1,
                    h: 1
                },
                Rect {
                    x: 120,
                    y: 200,
                    w: 1,
                    h: 1
                }
            ]
        );
        assert!(dirty_rects(&a, &a.clone()).is_empty());
    }

    fn visible(x: u32, y: u32) -> bool {
        let (dx, dy) = (x as f32 + 0.5 - 120.0, y as f32 + 0.5 - 120.0);
        dx * dx + dy * dy <= 120.0 * 120.0
    }

    fn covers(rects: &[Rect], r: Rect) {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let n = rects
                    .iter()
                    .filter(|q| x >= q.x && x < q.x + q.w && y >= q.y && y < q.y + q.h)
                    .count();
                assert!(n <= 1, "({x}, {y}) in {n} rects");
                if visible(x, y) {
                    assert_eq!(n, 1, "visible ({x}, {y}) not pushed");
                }
                assert!(rects.iter().all(|q| q.x >= r.x
                    && q.x + q.w <= r.x + r.w
                    && q.y >= r.y
                    && q.y + q.h <= r.y + r.h));
            }
        }
    }

    #[test]
    fn round_rects_cut_a_full_frame_to_the_circle() {
        let out = round_rects(&[Rect::full()], 240);
        covers(&out, Rect::full());
        assert_eq!(out.len(), 8);
        let area: u64 = out.iter().map(Rect::area).sum();
        assert!(area < 51_000, "{area}");
        assert!(area > 45_239, "the circle itself is π·120²");
    }

    #[test]
    fn round_rects_drop_corners_and_keep_small_rects_whole() {
        let corner = Rect {
            x: 0,
            y: 0,
            w: 20,
            h: 20,
        };
        assert!(round_rects(&[corner], 240).is_empty());
        let mid = Rect {
            x: 100,
            y: 100,
            w: 40,
            h: 40,
        };
        assert_eq!(round_rects(&[mid], 240), vec![mid]);
        // a wide strip near the top loses its invisible ends, as one rect
        let top = Rect {
            x: 0,
            y: 10,
            w: 240,
            h: 6,
        };
        let out = round_rects(&[top], 240);
        assert_eq!(out.len(), 1);
        assert!(out[0].w <= 130, "{:?}", out[0]);
        covers(&out, top);
        // an uneven rect straddling bands
        let odd = Rect {
            x: 3,
            y: 41,
            w: 230,
            h: 170,
        };
        let out = round_rects(&[odd, corner], 240);
        covers(&out, odd);
        assert!(out.iter().map(Rect::area).sum::<u64>() <= odd.area());
    }

    #[test]
    fn dirty_rects_merge_close_rows() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        px(&mut b, 50, 10, (255, 0, 0));
        px(&mut b, 60, 18, (255, 0, 0)); // 8 rows below: same band
        assert_eq!(
            dirty_rects(&a, &b),
            vec![Rect {
                x: 50,
                y: 10,
                w: 11,
                h: 9
            }]
        );
    }

    #[test]
    fn merge_rects_caps_at_four_and_falls_back_to_full() {
        let row = |y| Rect {
            x: 0,
            y,
            w: 10,
            h: 1,
        };
        let out = merge_rects((0..6).map(|i| row(i * 40)).collect(), 240, 240);
        assert_eq!(out.len(), MAX_RECTS);
        let total: u64 = out.iter().map(Rect::area).sum();
        assert!(total >= 60);
        let big = vec![
            Rect {
                x: 0,
                y: 0,
                w: 240,
                h: 100,
            },
            Rect {
                x: 0,
                y: 140,
                w: 240,
                h: 100,
            },
        ];
        assert_eq!(merge_rects(big, 240, 240), vec![Rect::full()]);
        let overlap = vec![
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            Rect {
                x: 5,
                y: 5,
                w: 10,
                h: 10,
            },
        ];
        assert_eq!(
            merge_rects(overlap, 240, 240),
            vec![Rect {
                x: 0,
                y: 0,
                w: 15,
                h: 15
            }]
        );
    }
}

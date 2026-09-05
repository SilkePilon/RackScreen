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
        Rect { x: 0, y: 0, w: SIZE, h: SIZE }
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
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

pub fn new_pixmap() -> Pixmap {
    let mut p = Pixmap::new(SIZE, SIZE).expect("pixmap");
    p.fill(tiny_skia::Color::BLACK);
    p
}

/// Maps destination pixels to source pixels for a rotation (clockwise, multiples of 90)
/// followed by an optional horizontal flip.
pub struct Orient {
    size: u32,
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
        Orient { size: n, map }
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
    Some(Rect { x: x0 as u32, y: y0 as u32, w: (x1 - x0 + 1) as u32, h: (y1 - y0 + 1) as u32 })
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
        assert_eq!(dirty_rect(&a, &b), Some(Rect { x: 10, y: 20, w: 6, h: 6 }));
    }

    #[test]
    fn pack_known_colours() {
        let mut p = Pixmap::new(2, 2).unwrap();
        px(&mut p, 0, 0, (255, 0, 0));
        px(&mut p, 1, 0, (0, 255, 0));
        px(&mut p, 0, 1, (0, 0, 255));
        px(&mut p, 1, 1, (255, 255, 255));
        let out = pack_rgb565(&p, Rect { x: 0, y: 0, w: 2, h: 2 }, 1.0);
        assert_eq!(out, vec![0xF8, 0x00, 0x07, 0xE0, 0x00, 0x1F, 0xFF, 0xFF]);
        let half = pack_rgb565(&p, Rect { x: 0, y: 0, w: 1, h: 1 }, 0.5);
        assert_eq!(half, vec![0x78, 0x00]);
        let sub = pack_rgb565(&p, Rect { x: 1, y: 1, w: 1, h: 1 }, 1.0);
        assert_eq!(sub, vec![0xFF, 0xFF]);
    }

    #[test]
    fn rect_union() {
        let a = Rect { x: 0, y: 0, w: 2, h: 2 };
        let b = Rect { x: 5, y: 5, w: 1, h: 1 };
        assert_eq!(a.union(b), Rect { x: 0, y: 0, w: 6, h: 6 });
    }
}

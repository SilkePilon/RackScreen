//! The pixel world shows draw into, and the mockup's drawing helpers.
//!
//! A world is `WORLD_H` px high and spans every panel plus the bezel gaps
//! between them; panel `i` shows the `PANEL_W`-wide slice at `Layout::x0(i)`,
//! scaled ×`SCALE`. The primitives are ports of the JavaScript mockup
//! helpers (`P`, `spr`, `dith`, `disc`, `ring`, `bands`), pixel for pixel.

pub const WORLD_H: u32 = 60;
pub const PANEL_W: u32 = 60;
pub const SCALE: u32 = 4;
/// Width of the mockups' world: four panels with 8 px gaps.
pub const MOCKUP_W: f64 = 264.0;

/// Bezel gap in world px: one world px is about 0.54 mm on a GC9A01.
pub fn gap_px(gap_mm: f32) -> u32 {
    (gap_mm / 0.54).round().max(0.0) as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub panels: u32,
    pub gap: u32,
}

impl Layout {
    /// World width: every panel plus the gaps between them.
    pub fn width(&self) -> u32 {
        self.panels * PANEL_W + self.panels.saturating_sub(1) * self.gap
    }
    /// Left edge of panel `i` in the world.
    pub fn x0(&self, i: u32) -> u32 {
        i * (PANEL_W + self.gap)
    }
}

/// The mockup hash: `fract(sin(i·127.1 + 311.7)·43758.5453)`.
pub fn rnd(i: f64) -> f64 {
    let x = (i * 127.1 + 311.7).sin() * 43758.5453;
    x - x.floor()
}

/// JavaScript `Math.round`: halves round towards +∞.
pub fn js_round(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// RGBA8, row-major, `width × WORLD_H`. Drawing clips to the raster.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldFrame {
    pub width: u32,
    pub rgba: Vec<u8>,
}

impl WorldFrame {
    /// A fully transparent frame (every byte 0).
    pub fn new(width: u32) -> Self {
        Self {
            width,
            rgba: vec![0; (width * WORLD_H * 4) as usize],
        }
    }
    /// The RGBA bytes at `(x, y)`.
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

pub struct World<'a> {
    pub frame: &'a mut WorldFrame,
    pub layout: Layout,
}

impl World<'_> {
    pub fn w(&self) -> i32 {
        self.frame.width as i32
    }
    pub fn h(&self) -> i32 {
        WORLD_H as i32
    }
    pub fn px0(&self, i: u32) -> i32 {
        self.layout.x0(i) as i32
    }
    /// Left edge of panel `i`, clamped to the last panel, for mockup
    /// elements pinned to a screen that a narrower layout lacks.
    pub fn panel(&self, i: u32) -> i32 {
        self.px0(i.min(self.layout.panels.saturating_sub(1)))
    }
    /// A mockup count of things spread over its 264 px world, scaled to this
    /// world's width so every screen keeps the mockup's density.
    pub fn spread(&self, n: u32) -> u32 {
        js_round(n as f64 * self.w() as f64 / MOCKUP_W).max(0) as u32
    }

    /// Canvas `fillRect` at integer coordinates, clipped to the raster.
    fn fill(&mut self, c: u32, x: i32, y: i32, w: i32, h: i32) {
        // Canvas normalises negative sizes.
        let (x, w) = if w < 0 { (x + w, -w) } else { (x, w) };
        let (y, h) = if h < 0 { (y + h, -h) } else { (y, h) };
        let (x1, y1) = ((x + w).min(self.w()), (y + h).min(self.h()));
        let (x, y) = (x.max(0), y.max(0));
        let px = [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
        for yy in y..y1 {
            let row = (yy as usize) * self.frame.width as usize;
            for xx in x..x1 {
                let i = (row + xx as usize) * 4;
                self.frame.rgba[i..i + 4].copy_from_slice(&px);
            }
        }
    }

    /// The mockup's `P`: a `w × h` rect of `c` at rounded `(x, y)`.
    pub fn p(&mut self, c: u32, x: f64, y: f64, w: i32, h: i32) {
        self.fill(c, js_round(x), js_round(y), w, h);
    }

    /// A string sprite: each char is a palette key; `.` and space are clear.
    pub fn spr(&mut self, rows: &[&str], pal: &[(char, u32)], x: f64, y: f64, flip: bool) {
        let (x, y) = (js_round(x), js_round(y));
        for (r, row) in rows.iter().enumerate() {
            let chars: Vec<char> = row.chars().collect();
            for c in 0..chars.len() {
                let ch = chars[if flip { chars.len() - 1 - c } else { c }];
                if ch == '.' || ch == ' ' {
                    continue;
                }
                if let Some(&(_, col)) = pal.iter().find(|(k, _)| *k == ch) {
                    self.fill(col, x + c as i32, y + r as i32, 1, 1);
                }
            }
        }
    }

    /// Checker dither: pixels where `(i + j + ph)` is even.
    pub fn dith(&mut self, c: u32, x: i32, y: i32, w: i32, h: i32, ph: i32) {
        for j in 0..h {
            for i in 0..w {
                if (i + j + ph) % 2 == 0 {
                    self.fill(c, x + i, y + j, 1, 1);
                }
            }
        }
    }

    /// A filled pixel disc of radius `r`.
    pub fn disc(&mut self, c: u32, cx: f64, cy: f64, r: i32) {
        let rf = r as f64;
        for y in -r..=r {
            let yf = y as f64;
            let w = (rf * rf - yf * yf + rf * 0.6).sqrt().floor() as i32;
            self.fill(c, js_round(cx - w as f64), js_round(cy + yf), 2 * w + 1, 1);
        }
    }

    /// A 1 px ring of radius `r`.
    pub fn ring(&mut self, c: u32, cx: f64, cy: f64, r: f64) {
        let n = ((r * 6.3).floor() as i32).max(8);
        for k in 0..n {
            let a = k as f64 / n as f64 * std::f64::consts::PI * 2.0;
            self.fill(
                c,
                js_round(cx + a.cos() * r),
                js_round(cy + a.sin() * r),
                1,
                1,
            );
        }
    }

    /// Full-width colour bands from the top, `(colour, height)`, with a
    /// one-row dither seam on each side of every boundary.
    pub fn bands(&mut self, list: &[(u32, i32)]) {
        let w = self.w();
        let mut y = 0;
        for &(c, h) in list {
            self.fill(c, 0, y, w, h);
            y += h;
        }
        let mut yy = 0;
        for pair in list.windows(2) {
            yy += pair[0].1;
            self.dith(pair[1].0, 0, yy - 1, w, 1, 0);
            self.dith(pair[0].0, 0, yy, w, 1, 1);
        }
    }

    /// Fill the whole world with `c`.
    pub fn clear(&mut self, c: u32) {
        let (w, h) = (self.w(), self.h());
        self.fill(c, 0, 0, w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: Layout = Layout { panels: 2, gap: 0 };

    fn rgb(f: &WorldFrame, x: u32, y: u32) -> Option<u32> {
        let [r, g, b, a] = f.at(x, y);
        (a == 255).then_some(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    #[test]
    fn gap_from_millimetres() {
        assert_eq!(gap_px(25.0), 46);
        assert_eq!(gap_px(0.0), 0);
        assert_eq!(gap_px(-3.0), 0);
    }

    #[test]
    fn layout_width_and_panel_edges() {
        let l = Layout { panels: 4, gap: 46 };
        assert_eq!(l.width(), 378);
        assert_eq!(l.x0(0), 0);
        assert_eq!(l.x0(3), 318);
        assert_eq!(Layout { panels: 3, gap: 46 }.width(), 272);
        assert_eq!(Layout { panels: 2, gap: 46 }.width(), 166);
        assert_eq!(Layout { panels: 2, gap: 46 }.x0(1), 106);
    }

    #[test]
    fn spread_keeps_the_mockup_count_at_its_width() {
        let mut f = WorldFrame::new(264);
        let w = World {
            frame: &mut f,
            layout: L,
        };
        assert_eq!(w.spread(11), 11);
        let mut f = WorldFrame::new(378);
        let w = World {
            frame: &mut f,
            layout: Layout { panels: 4, gap: 46 },
        };
        assert_eq!((w.spread(11), w.spread(90)), (16, 129));
        assert_eq!((w.panel(2), w.panel(9)), (212, 318));
    }

    #[test]
    fn rnd_matches_javascript() {
        assert!(
            (rnd(1.0) - 0.10468242550996365).abs() < 1e-9,
            "{}",
            rnd(1.0)
        );
        assert!((0.0..1.0).contains(&rnd(-7.0)));
    }

    #[test]
    fn js_round_rounds_halves_up() {
        assert_eq!(js_round(2.5), 3);
        assert_eq!(js_round(-2.5), -2);
        assert_eq!(js_round(-2.6), -3);
    }

    #[test]
    fn p_rounds_and_clips() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        w.p(0xff0000, -1.0, -1.0, 3, 3);
        w.p(0x00ff00, 118.6, 58.5, 5, 5);
        w.p(0x0000ff, 4.5, 10.4, 1, 1);
        assert_eq!(rgb(&f, 0, 0), Some(0xff0000));
        assert_eq!(rgb(&f, 1, 1), Some(0xff0000));
        assert_eq!(rgb(&f, 2, 2), None);
        assert_eq!(rgb(&f, 119, 59), Some(0x00ff00));
        assert_eq!(rgb(&f, 118, 59), None);
        assert_eq!(rgb(&f, 5, 10), Some(0x0000ff));
    }

    #[test]
    fn spr_draws_keys_and_skips_dots() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        let pal = [('a', 0x111111), ('b', 0x222222)];
        w.spr(&["ab", ".z", " a"], &pal, 3.0, 4.0, false);
        w.spr(&["ab"], &pal, 10.0, 4.0, true);
        assert_eq!(rgb(&f, 3, 4), Some(0x111111));
        assert_eq!(rgb(&f, 4, 4), Some(0x222222));
        assert_eq!(rgb(&f, 3, 5), None, "dot is clear");
        assert_eq!(rgb(&f, 4, 5), None, "unknown key is clear");
        assert_eq!(rgb(&f, 3, 6), None, "space is clear");
        assert_eq!(rgb(&f, 4, 6), Some(0x111111));
        assert_eq!(rgb(&f, 10, 4), Some(0x222222), "flipped");
        assert_eq!(rgb(&f, 11, 4), Some(0x111111), "flipped");
    }

    #[test]
    fn dith_sets_alternate_pixels() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        w.dith(0xabcdef, 2, 3, 4, 2, 0);
        w.dith(0x123456, 2, 10, 2, 1, 1);
        for j in 0..2 {
            for i in 0..4 {
                let want = ((i + j) % 2 == 0).then_some(0xabcdef);
                assert_eq!(rgb(&f, 2 + i, 3 + j), want, "({i},{j})");
            }
        }
        assert_eq!(rgb(&f, 2, 10), None);
        assert_eq!(rgb(&f, 3, 10), Some(0x123456));
    }

    #[test]
    fn disc_rows_follow_the_mockup() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        w.disc(0xffffff, 20.0, 20.0, 2);
        // r=2: w(y) = floor(sqrt(4 - y² + 1.2)) → y=±2: 1, y=±1: 2, y=0: 2.
        let row = |f: &WorldFrame, y: u32| {
            (10..30)
                .filter(|&x| rgb(f, x, y).is_some())
                .collect::<Vec<_>>()
        };
        assert_eq!(row(&f, 18), vec![19, 20, 21]);
        assert_eq!(row(&f, 19), vec![18, 19, 20, 21, 22]);
        assert_eq!(row(&f, 20), vec![18, 19, 20, 21, 22]);
        assert_eq!(row(&f, 22), vec![19, 20, 21]);
        assert!(row(&f, 17).is_empty() && row(&f, 23).is_empty());
    }

    #[test]
    fn ring_draws_at_radius() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        w.ring(0xffffff, 30.0, 30.0, 5.0);
        assert_eq!(rgb(&f, 35, 30), Some(0xffffff), "angle 0");
        assert_eq!(rgb(&f, 30, 30), None, "hollow");
    }

    #[test]
    fn bands_fill_rows_with_dither_seams() {
        let mut f = WorldFrame::new(L.width());
        let mut w = World {
            frame: &mut f,
            layout: L,
        };
        w.bands(&[(0x111111, 3), (0x222222, 2), (0x333333, 55)]);
        assert_eq!(rgb(&f, 5, 0), Some(0x111111));
        // Row 2 (before boundary 3): next colour on even x.
        assert_eq!(rgb(&f, 0, 2), Some(0x222222));
        assert_eq!(rgb(&f, 1, 2), Some(0x111111));
        // Row 3 (boundary): previous colour on odd x.
        assert_eq!(rgb(&f, 0, 3), Some(0x222222));
        assert_eq!(rgb(&f, 1, 3), Some(0x111111));
        // Row 4 before boundary 5, row 5 the boundary.
        assert_eq!(rgb(&f, 0, 4), Some(0x333333));
        assert_eq!(rgb(&f, 1, 4), Some(0x222222));
        assert_eq!(rgb(&f, 0, 5), Some(0x333333));
        assert_eq!(rgb(&f, 1, 5), Some(0x222222));
        assert_eq!(rgb(&f, 1, 59), Some(0x333333));
    }

    #[test]
    fn clear_paints_everything_opaque() {
        let mut f = WorldFrame::new(L.width());
        World {
            frame: &mut f,
            layout: L,
        }
        .clear(0x010203);
        assert!(f.rgba.chunks(4).all(|p| p == [1, 2, 3, 255]));
    }
}

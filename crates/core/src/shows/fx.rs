//! Extra drawing helpers for the movie-trope shows: ordered-dither
//! gradients, soft light, lines, screen shake and flashes, and a "stage"
//! coordinate that hurries characters through the bezel gaps.

use super::world::{js_round, World, PANEL_W};

/// 4×4 Bayer thresholds, `0..16`.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// How much faster than on a screen a stage walker crosses a bezel gap.
const GAP_RUSH: f64 = 4.0;

/// One opaque pixel at integer `(x, y)`, clipped.
pub fn put(g: &mut World, c: u32, x: i32, y: i32) {
    if x < 0 || y < 0 || x >= g.w() || y >= g.h() {
        return;
    }
    let i = ((y as u32 * g.frame.width + x as u32) * 4) as usize;
    g.frame.rgba[i..i + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
}

/// The colour at `(x, y)` (clamped into the raster).
pub fn get(g: &World, x: i32, y: i32) -> u32 {
    let x = x.clamp(0, g.w() - 1) as u32;
    let y = y.clamp(0, g.h() - 1) as u32;
    let [r, gg, b, _] = g.frame.at(x, y);
    ((r as u32) << 16) | ((gg as u32) << 8) | b as u32
}

/// `a` mixed towards `b` by `k` (`0..=1`).
pub fn mix(a: u32, b: u32, k: f64) -> u32 {
    let k = k.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as f64, ((b >> s) & 0xff) as f64);
        ((x + (y - x) * k).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// Blend `c` over the pixel at `(x, y)` with strength `k`.
pub fn blend(g: &mut World, c: u32, x: i32, y: i32, k: f64) {
    if x < 0 || y < 0 || x >= g.w() || y >= g.h() || k <= 0.0 {
        return;
    }
    let under = get(g, x, y);
    put(g, mix(under, c, k), x, y);
}

/// Whether the ordered dither turns pixel `(x, y)` on at coverage `k`.
pub fn on(x: i32, y: i32, k: f64) -> bool {
    (BAYER[(y & 3) as usize][(x & 3) as usize] as f64 + 0.5) / 16.0 < k
}

/// A vertical gradient through `stops` from row `y0` to `y1` (exclusive),
/// ordered-dithered between neighbouring colours, across `x0..x1`.
pub fn vgrad(g: &mut World, stops: &[u32], x0: i32, x1: i32, y0: i32, y1: i32) {
    let n = stops.len().max(1) - 1;
    for y in y0..y1 {
        let u = if y1 - y0 > 1 {
            (y - y0) as f64 / (y1 - y0 - 1) as f64 * n as f64
        } else {
            0.0
        };
        let i = (u.floor() as usize).min(n.saturating_sub(1));
        let k = u - i as f64;
        let (a, b) = (stops[i], stops[(i + 1).min(n)]);
        for x in x0.max(0)..x1.min(g.w()) {
            put(g, if on(x, y, k) { b } else { a }, x, y);
        }
    }
}

/// Smooth value noise in `0..1`, one random value per integer lattice
/// point, for clouds and mist.
pub fn vnoise(x: f64, y: f64, seed: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |a: f64, b: f64| super::world::rnd(a * 57.0 + b * 131.0 + seed);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

/// A soft round light: `c` blended in with strength falling off to the rim.
pub fn glow(g: &mut World, c: u32, cx: f64, cy: f64, r: f64, k: f64) {
    let ri = r.ceil() as i32 + 1;
    let (ix, iy) = (js_round(cx), js_round(cy));
    for y in iy - ri..=iy + ri {
        for x in ix - ri..=ix + ri {
            let d = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt() / r;
            if d < 1.0 {
                blend(g, c, x, y, k * (1.0 - d) * (1.0 - d));
            }
        }
    }
}

/// A 1 px line.
pub fn line(g: &mut World, c: u32, x0: f64, y0: f64, x1: f64, y1: f64) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.0) as i32;
    for i in 0..=n {
        let u = i as f64 / n as f64;
        put(
            g,
            c,
            js_round(x0 + (x1 - x0) * u),
            js_round(y0 + (y1 - y0) * u),
        );
    }
}

/// Blend `c` over the whole world with strength `k`.
pub fn tint(g: &mut World, c: u32, k: f64) {
    if k <= 0.0 {
        return;
    }
    for y in 0..g.h() {
        for x in 0..g.w() {
            blend(g, c, x, y, k);
        }
    }
}

/// Shift the whole picture by `(dx, dy)`, repeating the edge rows and
/// columns into the space it leaves.
pub fn shake(g: &mut World, dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }
    let (w, h) = (g.w(), g.h());
    let src = g.frame.rgba.clone();
    for y in 0..h {
        let sy = (y - dy).clamp(0, h - 1);
        for x in 0..w {
            let sx = (x - dx).clamp(0, w - 1);
            let (d, s) = (((y * w + x) * 4) as usize, ((sy * w + sx) * 4) as usize);
            g.frame.rgba[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
}

/// World x of stage position `v`: walkers move at their own pace on a
/// screen and dash through the bezels, so the chase stays on the glass.
pub fn stage_x(g: &World, v: f64) -> f64 {
    let pw = PANEL_W as f64;
    let gap = g.layout.gap as f64;
    let mut rest = v;
    let last = g.layout.panels.saturating_sub(1);
    for i in 0..=last {
        let x0 = g.px0(i) as f64;
        if rest < pw || i == last {
            return x0 + rest;
        }
        rest -= pw;
        if rest < gap / GAP_RUSH {
            return x0 + pw + rest * GAP_RUSH;
        }
        rest -= gap / GAP_RUSH;
    }
    v
}

/// Stage position of offset `off` on screen `i`.
pub fn stage_at(g: &World, i: u32, off: f64) -> f64 {
    let i = i.min(g.layout.panels.saturating_sub(1)) as f64;
    i * (PANEL_W as f64 + g.layout.gap as f64 / GAP_RUSH) + off
}

#[cfg(test)]
mod tests {
    use super::super::world::{Layout, WorldFrame};
    use super::*;

    #[test]
    fn stage_hurries_through_gaps() {
        let l = Layout { panels: 4, gap: 46 };
        let mut f = WorldFrame::new(l.width());
        let g = World {
            frame: &mut f,
            layout: l,
        };
        assert_eq!(stage_x(&g, 10.0), 10.0);
        assert_eq!(stage_x(&g, 60.0), 60.0);
        assert_eq!(stage_x(&g, 61.0), 64.0);
        assert_eq!(stage_at(&g, 1, 0.0), 71.5);
        assert_eq!(stage_x(&g, stage_at(&g, 1, 5.0)), 111.0);
        assert_eq!(stage_x(&g, stage_at(&g, 3, 30.0)), 348.0);
    }

    #[test]
    fn mix_and_dither() {
        assert_eq!(mix(0x000000, 0xffffff, 0.5), 0x808080);
        assert!(!on(0, 0, 0.0) && on(0, 0, 0.05) && on(3, 3, 1.0));
    }
}

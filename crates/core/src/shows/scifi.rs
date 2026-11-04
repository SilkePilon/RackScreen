//! Drawing helpers the sci-fi shows share: lines, ordered dither, colour
//! mixing and see-through tints.

use super::world::{js_round, World};

/// The 4×4 Bayer threshold at `(x, y)`, in `0..1`.
pub fn bayer(x: i32, y: i32) -> f64 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f64 + 0.5) / 16.0
}

/// `a` → `b` by `u` (clamped), per channel.
pub fn mix(a: u32, b: u32, u: f64) -> u32 {
    let u = u.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 255) as f64, ((b >> s) & 255) as f64);
        ((x + (y - x) * u).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// The colour at `(x, y)`, or `None` off the raster.
pub fn get(g: &World, x: i32, y: i32) -> Option<u32> {
    if x < 0 || y < 0 || x >= g.w() || y >= g.h() {
        return None;
    }
    let [r, gg, b, _] = g.frame.at(x as u32, y as u32);
    Some(((r as u32) << 16) | ((gg as u32) << 8) | b as u32)
}

/// One pixel of `c` over whatever is at `(x, y)`, `a` opaque.
pub fn tint(g: &mut World, x: i32, y: i32, c: u32, a: f64) {
    if let Some(old) = get(g, x, y) {
        g.p(mix(old, c, a), x as f64, y as f64, 1, 1);
    }
}

/// A 1 px line from `(x0, y0)` to `(x1, y1)`, its colour picked per step by
/// `col(u)` with `u` running 0 at the start to 1 at the end.
pub fn line(g: &mut World, x0: f64, y0: f64, x1: f64, y1: f64, col: impl Fn(f64) -> u32) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as i32).max(1);
    for k in 0..=n {
        let u = k as f64 / n as f64;
        g.p(col(u), x0 + (x1 - x0) * u, y0 + (y1 - y0) * u, 1, 1);
    }
}

/// A soft round glow: `c` over the scene, fading from `a` at the centre to
/// nothing at radius `r`.
pub fn glow(g: &mut World, cx: f64, cy: f64, r: f64, c: u32, a: f64) {
    let ri = r.ceil() as i32;
    let (x0, y0) = (js_round(cx), js_round(cy));
    for y in -ri..=ri {
        for x in -ri..=ri {
            let d = ((x * x + y * y) as f64).sqrt() / r;
            if d < 1.0 {
                tint(g, x0 + x, y0 + y, c, a * (1.0 - d) * (1.0 - d));
            }
        }
    }
}

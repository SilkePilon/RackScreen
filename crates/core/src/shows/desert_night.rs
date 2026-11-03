//! Desert night: a tumbleweed bounces and rolls past cactus silhouettes
//! under a big moon; a shooting star streaks over the middle screens.
//! Port of the round-2 mockup `desertNight`.

use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 9.0;

const CACTUS: u32 = 0x120c1c;

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let wf = w as f64;
    g.bands(&[
        (0x1a1430, 16),
        (0x2a1f4a, 16),
        (0x3e2a5a, 10),
        (0x2a1e3a, 18),
    ]);
    for i in 0..g.spread(45) {
        let c = if (f + i * 3) % 29 < 2 {
            0xffffff
        } else {
            0x9a8fd0
        };
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * wf).floor(), (rnd(fi + 5.0) * 34.0).floor());
        g.p(c, x, y, 1, 1);
    }
    // The moon on panel 0 + 34 (mockup x 34), with two craters.
    let mx = (g.px0(0) + 34) as f64;
    g.disc(0xf4ead0, mx, 14.0, 7);
    g.p(0xd8cba8, mx - 3.0, 12.0, 2, 2);
    g.p(0xd8cba8, mx + 2.0, 16.0, 2, 1);
    // The shooting star runs from panel 1 + 32 to panel 2 + 54 (mockup x 100
    // to 190), so it streaks over the same two middle screens.
    if t > 4.8 && t < 5.6 {
        let u = (t - 4.8) / 0.8;
        let (a, b) = ((g.panel(1) + 32) as f64, (g.panel(2) + 54) as f64);
        let (sx, sy) = (a + u * (b - a), 6.0 + u * 14.0);
        for k in 0..8 {
            let c = if k < 2 { 0xffffff } else { 0x8f86c8 };
            g.p(c, sx - k as f64 * 2.0, sy - k as f64 * 0.6, 1, 1);
        }
    }
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(5.0 + 3.0 * (xf * 0.05).sin() + 2.0 * (xf * 0.13).sin());
        g.p(0x22182e, xf, (46 - h) as f64, 1, h + 14);
    }
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(2.0 + 2.0 * (xf * 0.08 + 1.0).sin());
        g.p(0x1a1226, xf, (52 - h) as f64, 1, h + 8);
    }
    // Cacti on panel 0 + 58, 2 + 6 and 3 + 16 (mockup x 58, 142, 220); the
    // first is cut by the bezel as in the mockup. A screen the layout lacks
    // has no cactus rather than a second one crowding the last screen.
    for (i, off) in [(0, 58), (2, 6), (3, 16)] {
        if i >= g.layout.panels {
            continue;
        }
        let cx = (g.px0(i) + off) as f64;
        g.p(CACTUS, cx, 30.0, 3, 22);
        g.p(CACTUS, cx - 4.0, 36.0, 4, 2);
        g.p(CACTUS, cx - 4.0, 32.0, 2, 4);
        g.p(CACTUS, cx + 3.0, 39.0, 4, 2);
        g.p(CACTUS, cx + 5.0, 35.0, 2, 4);
    }
    let p = t / SECS;
    let tx = -10.0 + p * (wf + 20.0);
    let hop = (p * PI * 9.0).sin().abs();
    let ty = 44 - js_round(hop * 10.0);
    let rot = (f / 2) as f64;
    for k in 0..26 {
        let a = k as f64 * 2.4 + rot * 0.5;
        let r = (1 + k % 5) as f64;
        let c = if k % 3 != 0 { 0x8a6a4a } else { 0x5a4030 };
        g.p(
            c,
            tx + js_round(a.cos() * r) as f64,
            (ty + js_round(a.sin() * r)) as f64,
            1,
            1,
        );
    }
    g.p(CACTUS, tx - 3.0, 51.0, 7, 1);
}

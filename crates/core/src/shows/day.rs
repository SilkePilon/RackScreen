//! Day in a minute: the sun rises on the left, arcs over every screen and
//! sets on the right while the sky steps through dawn, day, dusk and night
//! (stepped colours, no gradients); then stars and a moon.
//! Port of the round-1 mockup `daycycle`.

use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// Sky, horizon and hills for dawn, day, day, dusk and night.
const STEPS: [[u32; 3]; 5] = [
    [0xf6a35c, 0xf9c98a, 0x5a3a3a],
    [0x6ec3ef, 0xa6dcf5, 0x2e5a3e],
    [0x6ec3ef, 0xa6dcf5, 0x2e5a3e],
    [0xe2685a, 0xf2a65a, 0x3a2a3e],
    [0x141a3a, 0x22305a, 0x141826],
];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let wf = w as f64;
    let p = t / SECS;
    let st = STEPS[((p * 5.0).floor() as usize).min(4)];
    g.bands(&[(st[0], 30), (st[1], 14), (st[2], 16)]);
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(5.0 + 3.0 * (xf * 0.07).sin() + 2.0 * (xf * 0.17 + 2.0).sin());
        g.p(st[2], xf, (44 - h) as f64, 1, h + 1);
    }
    if p < 0.8 {
        let sx = p / 0.8 * wf;
        let sy = 44.0 - (PI * p / 0.8).sin() * 36.0;
        g.disc(0xfff1a8, sx, sy, 4);
        let ro = ((f >> 1) % 2) as f64;
        for k in 0..8 {
            let a = k as f64 / 8.0 * PI * 2.0 + ro * 0.39;
            g.p(0xffe27a, sx + a.cos() * 7.0, sy + a.sin() * 7.0, 1, 1);
        }
    } else {
        for i in 0..g.spread(30) {
            let c = if (f + i) % 19 < 2 { 0xffffff } else { 0x8f9ad8 };
            let fi = i as f64;
            let (x, y) = ((rnd(fi) * wf).floor(), (rnd(fi + 5.0) * 30.0).floor());
            g.p(c, x, y, 1, 1);
        }
        let mx = (p - 0.8) / 0.2 * wf;
        g.disc(0xe6e2cf, mx, 12.0, 4);
        g.disc(0x141a3a, mx + 2.0, 11.0, 3);
    }
}

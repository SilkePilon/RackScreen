//! Fireworks: rockets launch one screen at a time over a city skyline,
//! bursting in four colours, then a finale on every screen at once.
//! Port of the round-1 mockup `fireworks`.

use super::world::{ease, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 8.0;

/// Burst palettes: `[mid, bright, fading]`.
const FWC: [[u32; 3]; 4] = [
    [0xff6b5a, 0xffb547, 0x7a2a2a],
    [0x73eff7, 0xffffff, 0x2f6b8f],
    [0xa7f070, 0xffe27a, 0x3a6a30],
    [0xc58bff, 0xffb3f0, 0x4a2a6a],
];

/// `(panel, launch time, palette)`: one screen at a time, then the finale.
const LAUNCHES: [(u32, f64, usize); 8] = [
    (0, 0.2, 0),
    (1, 1.4, 1),
    (2, 2.6, 2),
    (3, 3.8, 3),
    (0, 5.2, 1),
    (1, 5.2, 2),
    (2, 5.2, 3),
    (3, 5.2, 0),
];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, h) = (g.w(), g.h());
    let wf = w as f64;
    g.p(0x0a0a1a, 0.0, 0.0, w, h);
    for i in 0..g.spread(25) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * wf).floor(), (rnd(fi + 1.0) * 40.0).floor());
        g.p(0x2a2a4a, x, y, 1, 1);
    }
    // Skyline: blocks of random width and height until the world is full.
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64;
        let bw = 6 + (rnd(fi + 3.0) * 9.0).floor() as i32;
        let bh = 6 + (rnd(fi + 33.0) * 10.0).floor() as i32;
        g.p(0x13132a, x as f64, (60 - bh) as f64, bw, bh);
        let mut wy = 60 - bh + 2;
        while wy < 58 {
            if rnd((x + wy) as f64) > 0.6 {
                g.p(0x3a3a20, (x + 2) as f64, wy as f64, 1, 1);
            }
            wy += 3;
        }
        x += bw;
        i += 1;
    }
    for (pi, t0, ci) in LAUNCHES {
        // A screen the layout lacks has no rocket; the finale still covers
        // every screen there is.
        if pi >= g.layout.panels {
            continue;
        }
        let lt = t - t0;
        if lt < 0.0 {
            continue;
        }
        let cx = (g.px0(pi) + 30) as f64;
        let cy = (16 + (pi % 2) * 4) as f64;
        let col = FWC[ci];
        if lt < 0.7 {
            let y = 58.0 - (58.0 - cy) * ease(lt / 0.7);
            g.p(0xffe9a8, cx, y, 1, 2);
            g.p(0x7a5a3a, cx, y + 2.0, 1, 2);
            continue;
        }
        let e = lt - 0.7;
        if e > 1.8 {
            continue;
        }
        if e < 0.1 {
            g.disc(0xffffff, cx, cy, 3);
            continue;
        }
        let (pf, cf) = (pi as f64, ci as f64);
        for k in 0..26u32 {
            let kf = k as f64;
            let a = kf / 26.0 * PI * 2.0 + rnd(kf + cf) * 0.2;
            let sp = 14.0 + rnd(kf * 3.0 + pf) * 8.0;
            let x = cx + a.cos() * sp * e.min(1.0) * 0.9;
            let y = cy + a.sin() * sp * e.min(1.0) * 0.9 + e * e * 6.0;
            let c = if e < 0.6 {
                col[1]
            } else if e < 1.2 {
                col[0]
            } else {
                col[2]
            };
            if e > 1.2 && (f + k).is_multiple_of(3) {
                continue;
            }
            g.p(c, x, y, 1, 1);
            if e < 0.9 {
                g.p(col[0], x - a.cos(), y - a.sin(), 1, 1);
            }
        }
    }
}

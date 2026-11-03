//! Space cruise: a small ship flies past a ringed planet over three layers
//! of stars, an asteroid tumbles by, the engine flickers.
//! Port of the round-1 mockup `space`.

use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 8.0;

const SHIP: &[&str] = &[
    "..kk..........",
    ".kddkk........",
    "kddddddkkk....",
    "kdddcccddddkk..",
    "kddddddddddddk.",
    "kdddcccddddkk..",
    "kddddddkkk....",
    ".kddkk........",
    "..kk..........",
];
const SHIPP: &[(char, u32)] = &[('k', 0x3a3f52), ('d', 0xd8dde8), ('c', 0x5ad1ff)];

const ROCK: &[&str] = &[".mm.", "mmmm", "mmm.", ".m.."];
const ROCKP: &[(char, u32)] = &[('m', 0x6a6070)];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, h) = (g.w(), g.h());
    let (wf, hf) = (w as f64, h as f64);
    g.p(0x05040d, 0.0, 0.0, w, h);
    for (n, sp, c) in [
        (50, 4.0, 0x3a3a5a),
        (30, 10.0, 0x8a8ab0),
        (14, 22.0, 0xffffff),
    ] {
        for i in 0..g.spread(n) {
            let fi = i as f64;
            let x = ((rnd(fi + sp) * wf - t * sp) % wf + wf) % wf;
            g.p(c, x, (rnd(fi + sp * 3.0) * hf).floor(), 1, 1);
        }
    }
    // The planet on panel 2 + 42 (mockup x 178).
    let pp = g.layout.panels.saturating_sub(1).min(2);
    let (pcx, pcy) = (g.px0(pp), 28);
    for y in -16i32..=16 {
        for x in -16i32..=16 {
            if x * x + y * y > 196 {
                continue;
            }
            let band = ((y + 16) / 5) % 2;
            let mut c = if band == 1 { 0xd9784a } else { 0xb95a3a };
            if x > 6 && (x + y) % 2 == 0 {
                c = 0x5a2a20;
            }
            if x > 10 {
                c = 0x3a1a14;
            }
            g.p(c, (pcx + 42 + x) as f64, (pcy + y) as f64, 1, 1);
        }
    }
    let (pcx, pcyf) = ((pcx + 42) as f64, pcy as f64);
    for a in 0..120 {
        let th = a as f64 / 120.0 * PI * 2.0;
        let (rx, ry) = (th.cos() * 24.0, th.sin() * 5.0);
        if th.sin() < 0.0 && rx.abs() < 14.0 {
            continue;
        }
        g.p(0xe8c48a, pcx + rx, pcyf + ry - rx * 0.15, 1, 1);
    }
    // The asteroid drifts from panel 3 + 16 (mockup x 220) to panel 2 + 36
    // (mockup 220 - 8·6 = 172), the screen after the planet's to the planet's.
    let (a0, a1) = ((g.px0(pp + 1) + 16) as f64, (g.px0(pp) + 36) as f64);
    let ax = a0 + (t / SECS) * (a1 - a0);
    let ay = (12 + js_round(t.sin() * 2.0)) as f64;
    g.spr(ROCK, ROCKP, ax, ay, (f >> 2) % 2 == 1);
    let sx = -16.0 + (t / SECS) * (wf + 32.0);
    let sy = (26 + js_round((t * 2.0).sin() * 2.0)) as f64;
    let fl = (f >> 1) % 2 == 1;
    g.p(
        if fl { 0xffb347 } else { 0xff6a3d },
        sx - 3.0,
        sy + 3.0,
        3,
        3,
    );
    g.p(0xffe27a, sx - 1.0, sy + 4.0, 1, 1);
    if fl {
        g.p(0xff6a3d, sx - 5.0, sy + 4.0, 2, 1);
    }
    g.spr(SHIP, SHIPP, sx, sy, false);
}

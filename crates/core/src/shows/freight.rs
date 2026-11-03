//! Dusk freight: a long teal train with lit windows rolls through every
//! screen at sunset, smoke trailing, telegraph wires sagging between poles.
//! Port of the round-1 mockup `freight`.

use super::world::{js_round, World};

/// Show length; the train crosses the whole world in it.
pub const SECS: f64 = 9.0;

const LOCO: &[&str] = &[
    "....cc..............kk......",
    "....cc.............kmmk.....",
    "..kkkkkkkkkkkkkkkkkkmmkk....",
    ".kbbbbbbbbbbbbbbbbbbbbbbbk..",
    ".kbyybbyybbyybbbbbbbbbbbbbk.",
    ".kbyybbyybbyybbbbbbbbbbbbbbk",
    ".kbbbbbbbbbbbbbbbbbbbbbbbbbk",
    ".kddddddddddddddddddddddddddk",
    "..kwwk..kwwk..kwwk..kwwk....",
    "...kk....kk....kk....kk.....",
];

const CARR: &[&str] = &[
    "kkkkkkkkkkkkkkkkkkkkkkk",
    "kbbbbbbbbbbbbbbbbbbbbbk",
    "kbyybbyybbyybbyybbyybbk",
    "kbyybbyybbyybbyybbyybbk",
    "kbbbbbbbbbbbbbbbbbbbbbk",
    "kdddddddddddddddddddddk",
    ".kwwk.............kwwk.",
    "..kk...............kk..",
];

const TRP: &[(char, u32)] = &[
    ('k', 0x120a16),
    ('b', 0x2f6f6a),
    ('d', 0x1d4744),
    ('y', 0xffd98a),
    ('w', 0x0e0a12),
    ('c', 0x3a2a30),
    ('m', 0x2a1e24),
];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let (fi, wf) = (f as i32, w as f64);
    g.bands(&[
        (0x2b1d4a, 14),
        (0x6b2f5e, 10),
        (0xc4505a, 8),
        (0xf08c5a, 8),
        (0x3a1f45, 20),
    ]);
    // Sun on panel 1 (mockup x 120 = 68 + 52), its lower edge cut by a line.
    let p1 = g.px0(1) as f64;
    g.disc(0xffd27a, p1 + 52.0, 40.0, 9);
    g.p(0xf08c5a, p1 + 32.0, 40.0, 40, 1);
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(6.0 + 3.0 * (xf * 0.05).sin() + 2.0 * (xf * 0.13).sin());
        g.p(0x3a1f45, xf, (40 - h + 6) as f64, 1, h);
    }
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(4.0 + 2.0 * (xf * 0.09 + 1.0).sin() + (xf * 0.21).sin());
        g.p(0x24142e, xf, (46 - h) as f64, 1, h + 14);
    }
    // Track: rail and scrolling sleepers.
    g.p(0x1a0f1f, 0.0, 51.0, w, 1);
    let mut x = (-(fi * 3)) % 6;
    while x < w {
        g.p(0x3b2433, x as f64, 52.0, 3, 1);
        x += 6;
    }
    // Train: six carriages and the loco, crossing the whole world.
    let len = 28.0 + 6.0 * 24.0;
    let x0 = -len - 10.0 + (t / SECS) * (wf + len + 30.0);
    for k in 0..6 {
        g.spr(CARR, TRP, x0 + k as f64 * 24.0, 43.0, false);
    }
    g.spr(LOCO, TRP, x0 + 6.0 * 24.0, 41.0, false);
    for k in 0..7 {
        let age = (fi + k * 5) % 35;
        let a = age as f64;
        let (sx, sy) = (x0 + 6.0 * 24.0 + 5.0 - a * 1.6, 40.0 - a * 0.8);
        let sz = if age < 10 {
            2
        } else if age < 22 {
            3
        } else {
            2
        };
        let c = if age < 14 {
            0xd9c3c9
        } else if age < 26 {
            0x9e7f93
        } else {
            0x6b4f66
        };
        g.p(c, sx, sy, sz, sz);
    }
    // A telegraph pole on every panel, wires sagging between them.
    let n = g.layout.panels;
    for i in 0..n {
        let px = (g.px0(i) + 8) as f64;
        g.p(0x140b18, px, 18.0, 1, 34);
        g.p(0x140b18, px - 3.0, 20.0, 7, 1);
    }
    for i in 0..n.saturating_sub(1) {
        let (a, b) = (g.px0(i) + 8, g.px0(i + 1) + 8);
        for x in a..=b {
            let u = (x - a) as f64 / (b - a) as f64;
            g.p(
                0x140b18,
                x as f64,
                (21 + js_round(4.0 * u * (1.0 - u) * 3.0)) as f64,
                1,
                1,
            );
        }
    }
}

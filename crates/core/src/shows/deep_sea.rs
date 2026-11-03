//! Deep sea: kelp sways, light rays drift, bubbles rise, a school of orange
//! fish swims across while a big shadow passes slowly behind.
//! Port of the round-1 mockup `deep`.

use super::world::{js_round, rnd, World};

/// Show length; the fish cross the whole world in it.
pub const SECS: f64 = 10.0;

const FISH: &[&str] = &["..oo.", ".oooe", "ooooo", ".ooo."];
const FISHP: &[(char, u32)] = &[('o', 0xffb347), ('e', 0x3a1a0a)];

const SHADOW: &[&str] = &[
    "....ssssssss........",
    "..ssssssssssss....ss",
    ".ssssssssssssssss.ss",
    "ssssssssssssssssssss",
    ".ssssssssssssssss.ss",
    "..ssssssssssss....ss",
    "....ssssssss........",
];
const SHADOWP: &[(char, u32)] = &[('s', 0x0b2244)];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let (fi, wf) = (f as i32, w as f64);
    g.bands(&[
        (0x123a6b, 14),
        (0x0d2a52, 18),
        (0x0a1a3a, 22),
        (0x2b2742, 6),
    ]);
    // Light rays drifting right, wrapping over the whole world.
    for k in 0..g.spread(8) as i32 {
        let x0 = ((k * 37 + fi / 6) % (w + 40)) - 20;
        for y in 0..40 {
            if (y + k) % 3 != 0 {
                g.p(0x1b4c80, x0 as f64 + y as f64 * 0.6, y as f64, 1, 1);
            }
        }
    }
    // Kelp, one stalk every 24 px.
    for k in 0..g.spread(11) {
        let kf = k as f64;
        let kx = 8 + k as i32 * 24 + (rnd(kf) * 8.0).floor() as i32;
        for s in 0..9 {
            let sf = s as f64;
            let yy = 54 - s * 3;
            let ox = js_round((t * 1.6 + sf * 0.5 + kf).sin() * 1.4 * (sf / 3.0));
            let c = if s % 2 == 1 { 0x2fa36f } else { 0x1f7a5a };
            g.p(c, (kx + ox) as f64, yy as f64, 2, 3);
        }
    }
    // The big shadow, at 0.6 of the fish's pace.
    let bx = -60.0 + (t / SECS) * (wf + 120.0) * 0.6;
    g.spr(SHADOW, SHADOWP, bx, 22.0, false);
    for n in 0..9 {
        let nf = n as f64;
        let fx = -30.0 + (t / SECS) * (wf + 60.0) + ((n % 3) * 7) as f64 - ((n / 3) * 5) as f64;
        let fy = (30 + (n / 3) * 4 + js_round((t * 3.0 + nf).sin() * 1.5)) as f64;
        g.spr(FISH, FISHP, fx, fy, false);
    }
    // Bubbles, one column every 19 px.
    for b in 0..g.spread(14) {
        let bf = b as f64;
        let bx2 = 10 + b as i32 * 19;
        let cyc = (t * 0.6 + rnd(bf)) % 1.0;
        let by = 56.0 - cyc * 56.0;
        let sz = if cyc > 0.7 { 1 } else { 2 };
        let x = bx2 + js_round((cyc * 12.0 + bf).sin());
        g.p(0x8fd3ff, x as f64, by, sz, sz);
    }
}

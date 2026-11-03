//! Mountains, alpine dawn: three snowy ranges in layers, mist drifting in
//! the valley, pines in front; the sun rises behind the peaks while an
//! eagle glides across. Port of the round-2 mockup `alpine`.

use super::world::{ease, js_round, rnd, World};

pub const SECS: f64 = 10.0;

const EAGLE: [&[&str]; 2] = [
    &["k.......k", ".kk...kk.", "...kkk..."],
    &[".........", "kkkkkkkkk", "...kkk..."],
];
const EAGLEP: &[(char, u32)] = &[('k', 0x14121a)];

/// `(seed, base, amp, rock, snow)`, back to front.
const RANGES: [(f64, f64, f64, u32, Option<u32>); 3] = [
    (1.3, 42.0, 22.0, 0x4a5a85, Some(0xdde4f2)),
    (2.7, 48.0, 18.0, 0x344470, Some(0xc8d2ea)),
    (4.1, 56.0, 12.0, 0x1f2a4a, None),
];

/// A ridge line's height at world x: two folded sines.
fn ridge(seed: f64, base: f64, amp: f64, x: f64) -> f64 {
    base - (x * 0.031 + seed).sin().abs() * amp - (x * 0.083 + seed * 2.0).sin().abs() * amp * 0.45
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, h) = (g.w(), g.h());
    let wf = w as f64;
    g.bands(&[
        (0x1c2a4a, 10),
        (0x3a4f7a, 10),
        (0x8a7aa0, 8),
        (0xe8a07a, 10),
        (0xf6c48a, 22),
    ]);
    // The sun rises on panel 1 + 28 (mockup x 96), behind the ranges.
    let sy = 40.0 - ease(t / SECS) * 22.0;
    g.disc(0xfff1c8, (g.panel(1) + 28) as f64, sy, 6);
    for (seed, base, amp, c, snow) in RANGES {
        for x in 0..w {
            let xf = x as f64;
            let top = js_round(ridge(seed, base, amp, xf));
            g.p(c, xf, top as f64, 1, h - top);
            if let Some(s) = snow {
                let line = base - amp * 0.8;
                if (top as f64) < line {
                    let depth = 2 + ((line - top as f64) / 2.0).floor() as i32;
                    g.p(s, xf, top as f64, 1, depth);
                }
            }
        }
        // Mist drifts in the valley in front of the middle range.
        if seed > 2.0 && seed < 3.0 {
            let off = (t * 4.0).floor() as i32;
            for y in 0..3 {
                g.dith(0x9aa8cc, 0, 40 + y, w, 1, (y + off) % 2);
            }
        }
    }
    // Pines along the front ridge, one every 12 px.
    for k in 0..g.spread(22) {
        let x = k as i32 * 12 + (rnd(k as f64) * 6.0).floor() as i32;
        let y = js_round(ridge(4.1, 56.0, 12.0, x as f64));
        for r in 0..6 {
            g.p(0x14203a, (x - r / 2) as f64, (y - 6 + r) as f64, 1 + r, 1);
        }
    }
    let p = t / SECS;
    let ex = -10.0 + p * (wf + 20.0);
    let ey = (14 + js_round((p * 9.0).sin() * 5.0)) as f64;
    g.spr(EAGLE[((f >> 3) % 2) as usize], EAGLEP, ex, ey, false);
}

//! Desert caravan: three camels walk in line over the dunes, roped
//! together, leaving footprints; distant mesas shimmer in the heat.
//! Port of the round-2 mockup `caravan`.

use super::world::{js_round, World};

pub const SECS: f64 = 10.0;

const CAMEL: [&[&str]; 2] = [
    &[
        "......kk........",
        "....kkbbk.......",
        "....kbbbbk..kk..",
        "...kbbbbbbkkbbk.",
        "..kbbbbbbbbbbbk.",
        "kkbbbbbbbbbbbk..",
        "kbbbbbbbbbbbk...",
        ".kbbbbbbbbbbk...",
        "..kb.kb..kb.kb..",
        "..kb.kb..kb.kb..",
        "..k...k..k...k..",
    ],
    &[
        "......kk........",
        "....kkbbk.......",
        "....kbbbbk..kk..",
        "...kbbbbbbkkbbk.",
        "..kbbbbbbbbbbbk.",
        "kkbbbbbbbbbbbk..",
        "kbbbbbbbbbbbk...",
        ".kbbbbbbbbbbk...",
        "...kb.kbkb.kb...",
        "...kb.kbkb.kb...",
        "...k...kk...k...",
    ],
];
const CAMP: &[(char, u32)] = &[('k', 0x5a3a1e), ('b', 0xc8904a)];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let wf = w as f64;
    g.bands(&[
        (0x8fc8e8, 14),
        (0xb8dcef, 10),
        (0xf2e2b0, 10),
        (0xe0b070, 26),
    ]);
    // The sun on panel 3 + 6 (mockup x 210).
    g.disc(0xfffbe0, (g.panel(3) + 6) as f64, 10.0, 5);
    // Distant mesas, one every 52 px, each row jittered a pixel for heat
    // shimmer (the mockup draws rows narrowing downwards from 14 px).
    for y in 32..36 {
        let off = js_round((f as f64 * 0.7 + y as f64).sin());
        for k in 0..g.spread(5) as i32 {
            let mx = 20 + k * 52 + off;
            g.p(0xc89a6a, mx as f64, y as f64, 14 - (y - 32) * 2, 1);
        }
    }
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(5.0 + 3.0 * (xf * 0.04).sin() + 2.0 * (xf * 0.11 + 1.0).sin());
        g.p(0xd8a460, xf, (38 - h + 4) as f64, 1, h + 20);
    }
    for x in 0..w {
        let xf = x as f64;
        let h = js_round(3.0 + 2.0 * (xf * 0.06 + 2.0).sin());
        g.p(0xc8904a, xf, (50 - h) as f64, 1, h + 10);
    }
    for x in 0..w {
        if (x * 7) % 11 < 3 {
            g.dith(0xa8703a, x, 50, 1, 3, x);
        }
    }
    let lead = -20.0 + (t / SECS) * (wf + 70.0);
    for n in 0..3u32 {
        let cx = lead - n as f64 * 20.0;
        let ph = ((f >> 2) + n) % 2;
        g.spr(CAMEL[ph as usize], CAMP, cx, (39 + ph) as f64, false);
        if n > 0 {
            g.p(0x5a3a1e, cx + 14.0, 44.0, 6, 1);
        }
    }
    // Footprints trail the last camel; a fixed trail length, not a density.
    for k in 0..30 {
        let fx = lead - 60.0 - k as f64 * 4.0;
        if fx < 0.0 {
            break;
        }
        if k % 2 == 0 {
            g.p(
                0xa8703a,
                fx,
                (52 + if k % 4 != 0 { 0 } else { 1 }) as f64,
                1,
                1,
            );
        }
    }
}

//! Temple run: in a torch-lit temple corridor an explorer lifts a golden
//! idol off its pedestal, the ceiling opens and a giant boulder chases them
//! across every screen. They dive out of the doorway into the jungle, the
//! boulder plugs it with a thud, and their hat sails back onto their head.

use super::fx::{self, glow, line, put, shake, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

/// Top row of the floor.
const FLOOR: i32 = 47;
const R: i32 = 10;
/// The doorway's jamb, as an offset on the last screen.
const DOOR: f64 = 27.0;

const T_GRAB: f64 = 1.3;
const T_DROP: f64 = 2.0;
const T_LAND: f64 = 2.45;
const T_RUN: f64 = 2.6;
const T_DIVE: f64 = 7.9;
const T_THUD: f64 = 8.45;
const T_HAT: f64 = 9.35;

const PAL: &[(char, u32)] = &[
    ('h', 0xa8703a),
    ('H', 0x3a2414),
    ('s', 0xe8a878),
    ('k', 0x1e140e),
    ('j', 0x8a6236),
    ('J', 0x5e4024),
    ('w', 0xe6d6a8),
    ('p', 0x6a5a44),
    ('b', 0x2e2018),
    ('y', 0xf2c040),
    ('Y', 0xfff0a0),
    ('o', 0xb07a20),
];

const HAT: &[&str] = &["..hhh..", ".hHHHh.", "hhhhhhh"];

const STAND: &[&str] = &[
    "..sss..", "..ssk..", "..jwj..", ".jjwjj.", ".jJjJj.", ".s.Jj.s", "...pp..", "..p.p..",
    "..p.p..", ".bb.bb.",
];

const HOLD: &[&str] = &[
    "s.......s",
    "s.......s",
    "s.......s",
    "s..sss..s",
    "s..ssk..s",
    "jjjjwjjjj",
    "...jwj...",
    "...JjJ...",
    "....Jj...",
    "....pp...",
    "...p.p...",
    "...p.p...",
    "..bb.bb..",
];

const RUN: [&[&str]; 3] = [
    &[
        "...sss.", "...ssk.", "..jwj..", ".jjwjjs", "s.jJjJ.", "..Jjj..", "..ppp..", ".pp.pp.",
        "pp...pp", "b.....bb",
    ],
    &[
        "...sss.", "...ssk.", "..jwj..", "..jwjjs", "..jJj..", "..sjj..", "..ppp..", "...pp..",
        "..pp...", "..bbb..",
    ],
    &[
        "...sss.", "...ssk.", "..jwj..", ".jjwjj.", "s.jJj.s", "..Jjj..", "..ppp..", "..p.pp.",
        ".pp..p.", ".b...bb",
    ],
];

const DIVE: &[&str] = &[
    "...........ss.",
    "bpppJjjjjjsssk",
    "bpppjjjwwjss..",
    "......jj......",
];

const SIT: &[&str] = &[
    "s....s....",
    "s...sss...",
    "s...ssk...",
    "jj.jwjj...",
    "..jjwj.s..",
    "...jJj....",
    "...pppppbb",
    "...pppppbb",
];

const IDOL: &[&str] = &[".yy.", "yYYy", ".ko.", "yyyy", ".oo."];

const STONE: [u32; 3] = [0x4a3b30, 0x52412f, 0x453528];
const MORTAR: u32 = 0x261c15;

fn sparkle(g: &mut World, x: f64, y: f64, f: u32) {
    let n = (f % 6) as i32;
    if n < 3 {
        let r = 1 + n.min(1);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            for k in 1..=r {
                put(g, 0xfff6c0, js_round(x) + dx * k, js_round(y) + dy * k);
            }
        }
        put(g, 0xffffff, js_round(x), js_round(y));
    }
}

fn torch(g: &mut World, x: i32, f: u32) {
    let fl = rnd((f / 2) as f64 + x as f64) * 2.0;
    glow(g, 0xffa040, x as f64 + 1.0, 21.0, 17.0 + fl, 0.42);
    g.p(0x2a1a10, x as f64, 24.0, 3, 6);
    g.p(0x6a4a2a, x as f64 - 1.0, 23.0, 5, 1);
    g.p(0x1a100a, x as f64 + 1.0, 30.0, 1, 2);
    let ph = (f / 2) % 3;
    let flame: &[&str] = match ph {
        0 => &[".r..", ".ro.", "royr", ".yy."],
        1 => &["..r.", ".or.", "royo", ".yy."],
        _ => &[".r..", "..o.", "ryor", ".yy."],
    };
    g.spr(
        flame,
        &[('r', 0xd8401c), ('o', 0xff8a2a), ('y', 0xffe27a)],
        x as f64 - 0.0,
        19.0,
        false,
    );
}

/// Stone block wall with bevels, darker towards the ceiling.
fn wall(g: &mut World) {
    let w = g.w();
    for y in 7..FLOOR {
        let row = (y - 7) / 6;
        let off = if row % 2 == 0 { 0 } else { 5 };
        for x in 0..w {
            let bx = (x + off).div_euclid(10);
            let (lx, ly) = ((x + off).rem_euclid(10), (y - 7) % 6);
            let id = (bx * 31 + row * 7) as f64;
            let mut c = STONE[(rnd(id) * 3.0) as usize % 3];
            if lx == 0 || ly == 5 {
                c = MORTAR;
            } else if ly == 0 {
                c = fx::mix(c, 0x7a6450, 0.45);
            } else if lx == 9 || ly == 4 {
                c = fx::mix(c, MORTAR, 0.35);
            }
            // Weathered chips.
            if rnd((x * 13 + y * 7) as f64) > 0.93 {
                c = fx::mix(c, MORTAR, 0.5);
            }
            let dark = ((20 - y).max(0) as f64 / 13.0).min(1.0) * 0.5;
            put(g, fx::mix(c, 0x0e0806, dark), x, y);
        }
    }
    // Ceiling beam and its shadow.
    vgrad(g, &[0x120c08, 0x2a2018, 0x3a2c20], 0, w, 0, 7);
    for x in 0..w {
        if (x / 3) % 2 == 0 {
            put(g, 0x4a3828, x, 6);
        }
        if rnd(x as f64 * 0.7) > 0.8 {
            put(g, 0x1a120c, x, 7);
        }
    }
    // A frieze of carved glyphs along the top of the wall.
    const GLYPHS: [&str; 3] = [
        "#.#..###..#.#..#...",
        "###..#.#..###.###...",
        "#.#..###..#.#..#....",
    ];
    for x in 0..w {
        put(g, 0x2e2219, x, 9);
        put(g, 0x2e2219, x, 14);
        for (r, row) in GLYPHS.iter().enumerate() {
            let k = x.rem_euclid(19) as usize;
            if row.as_bytes().get(k) == Some(&b'#') {
                put(g, 0x2a1e16, x, 11 + r as i32);
                put(g, 0x6e5840, x, 10 + r as i32);
            }
        }
    }
}

fn floor(g: &mut World) {
    let w = g.w();
    vgrad(
        g,
        &[0x5e4a38, 0x3e3026, 0x2a2019, 0x1a120d],
        0,
        w,
        FLOOR,
        60,
    );
    for x in 0..w {
        put(g, 0x7a624a, x, FLOOR);
        if x.rem_euclid(14) == 0 {
            g.p(0x231a13, x as f64, (FLOOR + 1) as f64, 1, 4);
        }
        if (x + 7).rem_euclid(14) == 0 {
            g.p(0x231a13, x as f64, (FLOOR + 6) as f64, 1, 6);
        }
        if rnd(x as f64 * 3.3) > 0.85 {
            put(g, 0x6a5440, x, FLOOR + 2 + (x % 3));
        }
    }
    for y in [FLOOR + 5, FLOOR + 11] {
        g.p(0x2a2019, 0.0, y as f64, w, 1);
    }
}

fn vines(g: &mut World, t: f64) {
    for i in 0..g.spread(22) {
        let fi = i as f64;
        let x = (rnd(fi * 3.1 + 2.0) * g.w() as f64).floor();
        let len = 4 + (rnd(fi + 9.0) * 14.0) as i32;
        for y in 6..6 + len {
            let sw = ((t * 1.7 + fi).sin() * (y - 6) as f64 / 12.0).round();
            let c = if (y + i as i32) % 4 == 0 {
                0x5a8a34
            } else {
                0x3a5e24
            };
            g.p(c, x + sw, y as f64, 1, 1);
            if (y + i as i32) % 3 == 0 {
                let side = if y % 2 == 0 { 1.0 } else { -1.0 };
                g.p(0x6a9a3a, x + sw + side, y as f64, 1, 1);
            }
        }
    }
}

fn carvings(g: &mut World) {
    let lit = 0x76604a;
    let dark = 0x2a1e16;
    // A great carved mask on screen 1.
    let mx = (g.panel(1) + 34) as f64;
    g.p(dark, mx - 1.0, 16.0, 16, 18);
    g.p(0x5a4836, mx, 17.0, 14, 16);
    g.p(lit, mx, 17.0, 14, 1);
    g.p(dark, mx + 2.0, 21.0, 3, 2);
    g.p(dark, mx + 9.0, 21.0, 3, 2);
    g.p(0xd88a2a, mx + 3.0, 21.0, 1, 1);
    g.p(0xd88a2a, mx + 10.0, 21.0, 1, 1);
    g.p(lit, mx + 6.0, 23.0, 2, 4);
    g.p(dark, mx + 3.0, 28.0, 8, 2);
    for k in 0..4 {
        g.p(lit, mx + 3.0 + k as f64 * 2.0, 28.0, 1, 1);
    }
    for k in 0..7 {
        g.p(lit, mx + k as f64 * 2.0, 33.0, 1, 1);
    }
    // A zigzag serpent on screen 2.
    let sx = g.panel(2) + 6;
    for k in 0..26 {
        let y = 22 + ((k / 3) % 2) * 3 + if (k / 3) % 2 == 0 { k % 3 } else { 2 - k % 3 };
        put(g, dark, sx + k, y + 1);
        put(g, lit, sx + k, y);
    }
    g.p(lit, (sx + 26) as f64, 21.0, 3, 3);
    put(g, 0xd84020, sx + 27, 22);
    // A sun disc over the pedestal on screen 0.
    let px = (g.px0(0) + 36) as f64;
    g.ring(lit, px, 22.0, 4.0);
    g.ring(dark, px, 22.0, 2.0);
    for k in 0..8 {
        let a = k as f64 / 8.0 * PI * 2.0;
        put(
            g,
            lit,
            js_round(px + a.cos() * 6.0),
            js_round(22.0 + a.sin() * 6.0),
        );
    }
    // Bones and a skull on screen 2, cobweb on screen 1.
    let bx = (g.panel(2) + 40) as f64;
    g.spr(
        &[".www.", "wkwkw", ".www.", "..w.w"],
        &[('w', 0xd8ccb0), ('k', 0x2a2018)],
        bx,
        (FLOOR - 4) as f64,
        false,
    );
    g.p(0xc8bca0, bx - 7.0, (FLOOR + 1) as f64, 5, 1);
    g.p(0xc8bca0, bx - 8.0, FLOOR as f64, 1, 3);
    let cx = g.panel(1) + 4;
    for k in 0..7 {
        put(g, 0x6a6058, cx + k, 7 + k);
        put(g, 0x6a6058, cx + k, 7);
        put(g, 0x6a6058, cx, 7 + k);
    }
    line(g, 0x4a423c, cx as f64 + 4.0, 7.0, cx as f64, 11.0);
}

fn pedestal(g: &mut World, x: f64) {
    g.p(0x2a1e16, x - 1.0, 37.0, 10, 2);
    g.p(0x6e5a44, x - 1.0, 36.0, 10, 1);
    g.p(0x4e3e30, x + 1.0, 39.0, 6, 8);
    g.p(0x6a5640, x + 1.0, 39.0, 1, 8);
    g.p(0x2a1e16, x + 6.0, 39.0, 1, 8);
    g.p(0x3a2c22, x, (FLOOR - 1) as f64, 8, 1);
}

fn boulder(g: &mut World, x: f64, y: f64, rot: f64) {
    // Ground shadow.
    for k in -R..=R {
        if k % 2 == 0 {
            put(g, 0x140c08, js_round(x) + k, FLOOR);
        }
    }
    g.disc(0x3a3028, x, y, R);
    g.disc(0x6a5a48, x - 0.5, y - 0.5, R - 1);
    g.disc(0x82705a, x - 2.0, y - 2.0, R - 4);
    g.disc(0x9a8670, x - 3.0, y - 4.0, 2);
    for k in 0..7 {
        let a = rot + k as f64 * 0.9 + rnd(k as f64) * 0.5;
        let rr = 3.0 + (k % 3) as f64 * 2.5;
        let (cx, cy) = (x + a.cos() * rr, y + a.sin() * rr);
        let c = if k % 3 == 0 { 0x4a6a30 } else { 0x4a3c30 };
        g.p(c, cx, cy, 2, 1);
        g.p(0x3a3028, cx, cy + 1.0, 1, 1);
    }
    // Rim shade on the lower right.
    for k in 0..24 {
        let a = -0.3 + k as f64 / 24.0 * 2.2;
        put(
            g,
            0x2e261e,
            js_round(x + a.cos() * (R as f64 - 0.5)),
            js_round(y + a.sin() * (R as f64 - 0.5)),
        );
    }
}

fn dust(g: &mut World, x: f64, t: f64, n: u32, seed: f64) {
    for k in 0..n {
        let kf = k as f64 + seed;
        let age = (t * 1.6 + rnd(kf)) % 1.0;
        let px = x - age * (8.0 + rnd(kf + 1.0) * 14.0);
        let py = FLOOR as f64 - 2.0 - age * (4.0 + rnd(kf + 2.0) * 10.0);
        let r = (1.0 + age * 3.0) as i32;
        let c = if age < 0.5 { 0x9a8670 } else { 0x6a5a48 };
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r && (dx + dy + k as i32) % 2 == 0 {
                    put(g, c, js_round(px) + dx, js_round(py) + dy);
                }
            }
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let p0 = g.px0(0) as f64;
    let p3 = g.panel(3) as f64;
    wall(g);
    carvings(g);

    // Daylight and jungle beyond the doorway on the last screen.
    let door = p3 + DOOR;
    let dx0 = js_round(door) + 3;
    vgrad(g, &[0x8fd0f0, 0xc8ecf0, 0xf0f8d8], dx0, w, 0, FLOOR);
    for x in dx0..w {
        let xf = x as f64;
        let h = js_round(16.0 + 4.0 * (xf * 0.31).sin() + 3.0 * (xf * 0.77).sin());
        g.p(0x8cc0a8, xf, (FLOOR - h) as f64, 1, h);
        let h2 = js_round(9.0 + 3.0 * (xf * 0.5 + 1.0).sin() + 2.0 * (xf * 1.3).sin());
        g.p(0x4e8a3e, xf, (FLOOR - h2) as f64, 1, h2);
        if (x + h2) % 3 == 0 {
            put(g, 0x6aa84a, x, FLOOR - h2);
        }
    }
    // A palm trunk and hanging leaves sway in front.
    let tx = door + 16.0;
    for y in 8..FLOOR {
        let c = if y % 3 == 0 { 0x5a4028 } else { 0x7a5a36 };
        g.p(c, tx + ((y as f64) * 0.08).round(), y as f64, 2, 1);
    }
    for k in 0..5 {
        let a = -PI * 0.9 + k as f64 * 0.45 + (t * 1.5).sin() * 0.06;
        for r in 0..12 {
            let rf = r as f64;
            let droop = rf * rf * 0.04;
            put(
                g,
                if r % 2 == 0 { 0x2e6a24 } else { 0x3e8a2e },
                js_round(tx + 1.0 + a.cos() * rf),
                js_round(8.0 + a.sin() * rf * 0.6 + droop),
            );
        }
    }
    for i in 0..6 {
        let fi = i as f64;
        let lx = door + 3.0 + fi * 5.0 + ((t * 1.3 + fi).sin() * 1.2).round();
        g.disc(0x2e5e24, lx, 1.0 + rnd(fi) * 3.0, 3 + (i % 2));
        g.disc(0x4e8a3e, lx - 1.0, rnd(fi) * 3.0, 1);
    }
    // The doorway's jamb and lintel.
    g.p(0x3a2c22, door, 18.0, 4, FLOOR - 18);
    g.p(0x6a5640, door, 18.0, 1, FLOOR - 18);
    g.p(0x2a2019, door - 4.0, 14.0, 11, 4);
    g.p(0x6e5a44, door - 4.0, 14.0, 11, 1);
    g.p(0x3a2c22, door + 3.0, 0.0, 4, 14);
    put(g, 0xd88a2a, js_round(door), 16);

    floor(g);
    g.p(0x5a8a34, dx0 as f64, FLOOR as f64, w - dx0, 1);
    vgrad(g, &[0x6a9a3a, 0x4a7a2e, 0x3a5e24], dx0, w, FLOOR + 1, 60);
    for x in dx0..w {
        if rnd(x as f64) > 0.6 {
            put(g, 0x8aba4a, x, FLOOR - 1);
        }
    }
    // Sunlight spilling in through the doorway.
    for y in 18..60 {
        for x in js_round(door) - 22..js_round(door) {
            let k = (x as f64 - door + 22.0) / 22.0;
            fx::blend(g, 0xfff0c0, x, y, k * k * 0.3);
        }
    }

    vines(g, t);

    // A shaft of light falls on the idol's pedestal.
    let ped = p0 + 32.0;
    for y in 0..37 {
        for x in -4i32..=4 {
            let xx = js_round(ped + 4.0 + x as f64 + y as f64 * 0.1);
            let k = 1.0 - x.abs() as f64 / 5.0;
            fx::blend(g, 0xfff0b0, xx, y, k * 0.22);
        }
    }
    let n = g.layout.panels;
    for (i, off) in [(0u32, 50), (1, 12), (2, 48), (3, 14)] {
        if i < n {
            torch(g, g.px0(i) + off, f);
        }
    }

    // Dust motes drift in the light.
    for k in 0..g.spread(40) {
        let kf = k as f64;
        let x = (rnd(kf) * w as f64 + (t * 0.8 + kf).sin() * 3.0).floor();
        let y = ((rnd(kf + 3.0) * 40.0 + t * (1.0 + rnd(kf + 5.0) * 2.0)) % 40.0 + 8.0).floor();
        fx::blend(g, 0xffe0a0, x as i32, y as i32, 0.5);
    }

    // The ceiling trap door on screen 0 opens.
    let hole = p0 + 9.0;
    if t > T_DROP - 0.15 {
        g.p(0x000000, hole - 2.0, 0.0, 15, 7);
        let slide = ((t - T_DROP + 0.15) * 40.0).min(13.0);
        g.p(0x3a2c20, hole - 2.0 + slide, 0.0, 15, 7);
        g.p(0x4a3828, hole - 2.0 + slide, 6.0, 15, 1);
    }

    // Rumble: dust trickles from the ceiling while the boulder rolls.
    let rolling = t > T_DROP - 0.3 && t < T_THUD + 0.5;
    if rolling {
        for k in 0..g.spread(26) {
            let kf = k as f64;
            let x = (rnd(kf + 40.0) * w as f64).floor();
            let y = ((t * 30.0 + rnd(kf + 41.0) * 60.0) % 45.0) + 6.0;
            g.p(0x8a7660, x, y, 1, 2);
        }
    }

    pedestal(g, ped);
    let ex_stage0 = 24.0;
    let run_end = stage_at(g, 3, 6.0);
    let run_u = ((t - T_RUN) / (T_DIVE - T_RUN)).clamp(0.0, 1.0);
    let run_v = ex_stage0 + (run_end - ex_stage0) * (run_u * 0.85 + ease(run_u) * 0.15);

    // The boulder: drops through the trap door, then chases.
    let bx_land = p0 + 14.0;
    let (bx, by, rot);
    if t < T_DROP {
        bx = bx_land;
        by = -20.0;
        rot = 0.0;
    } else if t < T_LAND {
        let u = (t - T_DROP) / (T_LAND - T_DROP);
        bx = bx_land;
        by = -12.0 + (FLOOR - R + 12) as f64 * u * u;
        rot = 0.0;
    } else {
        let b = ((t - T_LAND) * 9.0).sin().abs() * (2.0 - (t - T_LAND) * 6.0).max(0.0);
        let chase = ((t - T_RUN + 0.1) / (T_DIVE - T_RUN)).clamp(0.0, 1.0);
        let v0 = bx_land - p0;
        let lag = 18.0 - 8.0 * chase;
        let target = run_v - lag;
        let v = if t < T_RUN {
            v0
        } else if t < T_THUD {
            let u = ((t - T_DIVE) / (T_THUD - T_DIVE)).clamp(0.0, 1.0);
            let end = stage_at(g, 3, DOOR - 9.0);
            (target.max(v0)) + (end - target.max(v0)) * u * u * (t > T_DIVE) as i32 as f64
        } else {
            stage_at(g, 3, DOOR - 9.0)
        };
        bx = stage_x(g, v.max(v0));
        by = (FLOOR - R) as f64 - b.round();
        rot = (v.max(v0) - v0) / R as f64;
    }
    if (T_RUN..T_THUD + 0.4).contains(&t) {
        dust(g, bx - 6.0, t, 7, 0.0);
    }

    // The explorer.
    let face_back = t > T_DROP - 0.1 && t < T_RUN;
    let held = t > T_GRAB;
    if t < T_GRAB {
        let walk = ease(t / (T_GRAB - 0.2));
        let ex = p0 + 14.0 + walk * 10.0;
        let fr = if t < T_GRAB - 0.2 {
            RUN[((f / 2) % 3) as usize]
        } else {
            STAND
        };
        g.spr(fr, PAL, ex, (FLOOR - fr.len() as i32) as f64, false);
        g.spr(
            HAT,
            PAL,
            ex + 1.0,
            (FLOOR - fr.len() as i32 - 3) as f64,
            false,
        );
    } else if t < T_RUN {
        let ex = p0 + 24.0;
        g.spr(HOLD, PAL, ex, (FLOOR - 13) as f64, face_back);
        g.spr(HAT, PAL, ex + 1.0, (FLOOR - 13) as f64, false);
    } else if t < T_DIVE {
        let ex = stage_x(g, run_v);
        let fr = RUN[((f / 2) % 3) as usize];
        let bob = ((f / 2) % 3 == 1) as i32 as f64;
        let ey = (FLOOR - fr.len() as i32) as f64 - bob;
        g.spr(fr, PAL, ex, ey, false);
        g.spr(HAT, PAL, ex + 1.0, ey - 3.0, false);
        g.spr(IDOL, PAL, ex + 6.0, ey + 3.0, false);
    } else if t < 8.7 {
        let u = ((t - T_DIVE) / 0.6).clamp(0.0, 1.0);
        let ex = stage_x(g, run_end) + u * 28.0;
        let ey = FLOOR as f64 - 9.0 - (u * PI).sin() * 7.0 + u * 5.0;
        g.spr(DIVE, PAL, ex - 4.0, ey, false);
        g.spr(IDOL, PAL, ex + 10.0, ey - 3.0, false);
    } else if t < 9.0 {
        let ex = stage_x(g, run_end) + 28.0;
        g.spr(DIVE, PAL, ex, (FLOOR - 4) as f64, false);
        g.spr(IDOL, PAL, ex + 14.0, (FLOOR - 5) as f64, false);
    } else {
        let ex = stage_x(g, run_end) + 28.0;
        g.spr(SIT, PAL, ex, (FLOOR - 8) as f64, false);
        let up = ease((t - 9.0) / 0.3) * 3.0;
        g.spr(IDOL, PAL, ex - 1.5, (FLOOR - 15) as f64 - up, false);
        if t >= T_HAT {
            g.spr(HAT, PAL, ex + 3.0, (FLOOR - 9) as f64, false);
            if t < T_HAT + 0.5 {
                for k in 0..3 {
                    put(g, 0xffffff, js_round(ex) + 1 + k * 4, FLOOR - 13 - (k % 2));
                }
            }
        }
        if t > 9.2 {
            sparkle(g, ex, (FLOOR - 16) as f64 - up, f);
        }
    }
    if !held {
        g.spr(IDOL, PAL, ped + 2.0, 31.0, false);
        sparkle(g, ped + 3.0, 31.0, f);
    } else if t < T_RUN {
        g.spr(IDOL, PAL, p0 + 26.5, (FLOOR - 18) as f64, false);
        sparkle(g, p0 + 28.0, (FLOOR - 18) as f64, f + 3);
    }
    if t > T_DROP - 0.2 && t < T_RUN {
        // "!" over the explorer's head.
        g.p(0xffffff, p0 + 34.0, (FLOOR - 26) as f64, 1, 3);
        g.p(0xffffff, p0 + 34.0, (FLOOR - 22) as f64, 1, 1);
    }

    boulder(g, bx, by, rot);

    // The hat sails off in the dive and boomerangs back.
    if (T_DIVE..T_HAT).contains(&t) {
        let u = ease((t - T_DIVE) / (T_HAT - T_DIVE));
        let a = (stage_x(g, run_end) + 2.0, FLOOR as f64 - 16.0);
        let c = (p3 + 4.0, -18.0);
        let b = (stage_x(g, run_end) + 31.0, (FLOOR - 9) as f64);
        let hx = (1.0 - u).powi(2) * a.0 + 2.0 * u * (1.0 - u) * c.0 + u * u * b.0;
        let hy = (1.0 - u).powi(2) * a.1 + 2.0 * u * (1.0 - u) * c.1 + u * u * b.1;
        let spin = (f / 2) % 4;
        let hat: &[&str] = match spin {
            0 => HAT,
            1 => &["..h", ".hh", "hHh", "hHh", "hh.", "h.."],
            2 => &["hhhhhhh", ".hHHHh.", "..hhh.."],
            _ => &["h..", "hh.", "hHh", "hHh", ".hh", "..h"],
        };
        g.spr(hat, PAL, hx, hy, false);
    }

    // Thud: the boulder plugs the doorway.
    if t > T_THUD && t < T_THUD + 0.9 {
        let u = (t - T_THUD) / 0.9;
        for k in 0..14 {
            let kf = k as f64;
            let a = PI + rnd(kf) * PI;
            let r = u * (8.0 + rnd(kf + 1.0) * 12.0);
            let c = if k % 2 == 0 { 0x9a8670 } else { 0x6a5a48 };
            g.p(c, bx + a.cos() * r, by + 8.0 + a.sin() * r * 0.6, 2, 2);
        }
        for k in 0..8 {
            let kf = k as f64;
            let x = p3 + 20.0 + rnd(kf + 7.0) * 30.0;
            g.p(0x8a7660, x, 6.0 + u * 40.0 + rnd(kf) * 6.0, 1, 2);
        }
    }

    // Screen shake on the landing and the thud.
    let sh = if (T_LAND..T_LAND + 0.3).contains(&t) || (T_THUD..T_THUD + 0.35).contains(&t) {
        2
    } else if rolling && t > T_RUN {
        (f % 2) as i32
    } else {
        0
    };
    if sh > 0 {
        let s = if f.is_multiple_of(2) { sh } else { -sh };
        shake(g, s / 2, s);
    }
}

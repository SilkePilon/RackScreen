//! The heist: a dark museum vault, a jewel under a spotlight, red laser
//! beams everywhere. A cat burglar drops in through the skylight on a
//! rope, contorts through the beams and swaps the jewel for a rock. All
//! clean, until the museum cat strolls in and walks straight through the
//! last laser: alarms flash red and the burglar is yanked out.

use super::fx::{self, blend, glow, line, put, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

const FLOOR: i32 = 46;

const T_DROP: f64 = 1.0;
const T_FLAT: f64 = 3.2;
const T_GRAB: f64 = 3.7;
const T_SWAP: f64 = 4.4;
const T_UP: f64 = 5.6;
const T_CAT: f64 = 4.8;
const T_TRIP: f64 = 7.1;
const T_YANK: f64 = 7.3;

const BUR: &[(char, u32)] = &[
    ('k', 0x101018),
    ('h', 0x3a3a52),
    ('e', 0xe8b088),
    ('H', 0x8a8a9a),
    ('g', 0x6ae0ff),
];

const STRAIGHT: &[&str] = &[
    "..kkk..", "..eek..", "..kkk..", "k.khk.k", ".kkhkk.", "..kHk..", "..khk..", "..k.k..",
    "..k.k..", "..k.k..", ".kk.kk.",
];
const SPLIT: &[&str] = &[
    "....kkk....",
    "....eek....",
    "k...kkk...k",
    ".k.kkhkk.k.",
    "..k.khk.k..",
    "....kHk....",
    "..kkkhkkk..",
    ".kk.....kk.",
    "kk.......kk",
];
const TUCK: &[&str] = &[
    "k.kkk.k", ".keekk.", "..kkk..", "..khk..", "..kHk..", ".kkhkk.", "kk...kk", ".kk.kk.",
];
/// Face down, head to the right, arms reaching for the pedestal.
const FLAT: [&[&str]; 2] = [
    &[
        "kk..........",
        ".kkkkkHkkkk.",
        "kk...hkkkeek",
        ".........kk.",
        "........k..k",
    ],
    &[
        "kk..........",
        ".kkkkkHkkkk.",
        "kk...hkkkeek",
        "........kk.k",
        "........k..k",
    ],
];

const CAT: [&[&str]; 2] = [
    &[
        "o.o......",
        "ooo.....o",
        "eoo....o.",
        "ooooooo..",
        ".oSoSoo..",
        ".o.o.o.o.",
    ],
    &[
        "o.o......",
        "ooo......",
        "eoo.....o",
        "ooooooooo",
        ".oSoSoo..",
        "o..oo..o.",
    ],
];
const CAT_SIT: [&[&str]; 2] = [
    &[
        "o.o...", "ooo...", "eoo...", ".ooo..", ".oSoo.", ".oooo.", "oo.ooo",
    ],
    &[
        "o.o...", "ooo...", "eoo...", "pooo..", ".oSoo.", ".oooo.", "oo.ooo",
    ],
];
const CATP: &[(char, u32)] = &[
    ('o', 0xe8903a),
    ('S', 0xa8582a),
    ('e', 0x5af06a),
    ('p', 0xffa8a8),
];

const GUARD_SLEEP: &[&str] = &[
    "..ccc...", ".ccccc..", "..sss...", "..sss...", ".uuuuu..", "uuuuuuu.", "uuuuuuuu", ".uuuubbb",
];
const GUARD_UP: &[&str] = &[
    "..ccc..", ".ccccc.", "..sks..", "..sss..", "s.uuu.s", "suuyuus", "..uuu..", "..uuu..",
    "..u.u..", "..u.u..", ".bb.bb.",
];
const GUARDP: &[(char, u32)] = &[
    ('c', 0x1e2a50),
    ('s', 0xe0a878),
    ('k', 0x101018),
    ('u', 0x34508a),
    ('y', 0xf2c040),
    ('b', 0x101018),
];

fn beam(g: &mut World, a: (f64, f64), b: (f64, f64), f: u32, k: f64) {
    let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32;
    let fl = 0.85 + 0.15 * rnd((f / 2) as f64 + a.0);
    for i in 0..=n {
        let u = i as f64 / n as f64;
        let (x, y) = (
            js_round(a.0 + (b.0 - a.0) * u),
            js_round(a.1 + (b.1 - a.1) * u),
        );
        blend(g, 0xff2030, x, y - 1, 0.3 * k * fl);
        blend(g, 0xff2030, x, y + 1, 0.3 * k * fl);
        blend(g, 0xff4050, x, y, k * fl);
    }
}

fn emitter(g: &mut World, x: f64, y: f64) {
    g.p(0x3a3a4a, x - 1.0, y - 1.0, 3, 3);
    put(g, 0xff4050, js_round(x), js_round(y));
}

fn frame(g: &mut World, x: f64, y: f64, w: i32, h: i32) {
    g.p(0x6a4a1a, x - 1.0, y - 1.0, w + 2, h + 2);
    g.p(0xb08a3a, x - 1.0, y - 1.0, w + 2, 1);
    g.p(0xb08a3a, x - 1.0, y - 1.0, 1, h + 2);
}

fn room(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(g, &[0x07080f, 0x10142a, 0x1a1e3a, 0x22284a], 0, w, 0, FLOOR);
    // Ceiling cornice and wall panels with pilasters.
    g.p(0x2a2e48, 0.0, 5.0, w, 1);
    g.p(0x05060c, 0.0, 6.0, w, 1);
    for x in 0..w {
        if x.rem_euclid(24) == 0 {
            g.p(0x2a2e4c, x as f64, 7.0, 3, FLOOR - 7);
            g.p(0x3a3e60, x as f64, 7.0, 1, FLOOR - 7);
        }
        if x.rem_euclid(24) == 8 {
            g.p(0x161a30, x as f64, 12.0, 1, 18);
            g.p(0x161a30, x as f64 + 11.0, 12.0, 1, 18);
        }
    }
    // Wainscot rail.
    g.p(0x2e3456, 0.0, 34.0, w, 1);
    g.p(0x0c0e1c, 0.0, 35.0, w, 1);
    vgrad(g, &[0x161a30, 0x10132a], 0, w, 36, FLOOR);
    // Marble floor, checkered, with a gloss line.
    for y in FLOOR..60 {
        for x in 0..w {
            let tile = ((x.div_euclid(6)) + (y - FLOOR) / 3) % 2 == 0;
            let mut c = if tile { 0x1e2238 } else { 0x2a2f4c };
            if rnd((x * 3 + y * 29) as f64) > 0.93 {
                c = fx::mix(c, 0x5a6088, 0.4);
            }
            put(g, c, x, y);
        }
    }
    g.p(0x4a5078, 0.0, FLOOR as f64, w, 1);
    let _ = (f, t);
}

/// Panel 0: the vault door and a sweeping camera.
fn vault(g: &mut World, f: u32, t: f64) {
    let p0 = g.px0(0) as f64;
    let (cx, cy) = (p0 + 30.0, 24.0);
    g.disc(0x0a0c16, cx, cy, 16);
    g.disc(0x5a6070, cx, cy, 15);
    g.disc(0x7a8292, cx - 1.0, cy - 1.0, 13);
    g.ring(0x3a404e, cx, cy, 11.0);
    for k in 0..12 {
        let a = k as f64 / 12.0 * PI * 2.0;
        g.p(0xc8ccd8, cx + a.cos() * 13.5, cy + a.sin() * 13.5, 1, 1);
    }
    // The wheel.
    g.disc(0x3a404e, cx, cy, 3);
    for k in 0..3 {
        let a = k as f64 / 3.0 * PI + 0.3;
        line(
            g,
            0xb8c0cc,
            cx - a.cos() * 7.0,
            cy - a.sin() * 7.0,
            cx + a.cos() * 7.0,
            cy + a.sin() * 7.0,
        );
    }
    g.disc(0xd8dce8, cx, cy, 1);
    // Camera on the wall, sweeping.
    let (kx, ky) = (p0 + 50.0, 10.0);
    g.p(0x3a3a4a, kx, ky - 3.0, 1, 3);
    let a = PI * 0.6 + (t * 1.2).sin() * 0.5;
    for k in 0..4 {
        g.p(
            0xb8bcc8,
            kx + a.cos() * k as f64,
            ky + a.sin() * k as f64,
            2,
            2,
        );
    }
    if (f / 6).is_multiple_of(2) {
        put(g, 0xff2020, js_round(kx), js_round(ky));
    }
    // Its faint view cone.
    for r in 4..26 {
        for s in -3..=3 {
            let b = a + s as f64 * 0.06;
            blend(
                g,
                0x9aa8ff,
                js_round(kx + b.cos() * r as f64),
                js_round(ky + b.sin() * r as f64),
                0.07,
            );
        }
    }
}

/// Panel 1: paintings, velvet ropes and sweeping beams.
fn gallery(g: &mut World, f: u32, t: f64, alarm: bool) {
    let p1 = g.panel(1) as f64;
    frame(g, p1 + 8.0, 13.0, 14, 10);
    vgrad(
        g,
        &[0x6aa8d8, 0xf0c070],
        js_round(p1 + 8.0),
        js_round(p1 + 22.0),
        13,
        19,
    );
    g.p(0x3a6a3a, p1 + 8.0, 19.0, 14, 4);
    g.p(0x4a8a4a, p1 + 12.0, 18.0, 6, 1);
    g.disc(0xfff0b0, p1 + 18.0, 15.0, 1);
    frame(g, p1 + 36.0, 11.0, 10, 13);
    g.p(0x2a1e1a, p1 + 36.0, 11.0, 10, 13);
    g.disc(0xd8a070, p1 + 41.0, 16.0, 2);
    g.p(0x5a2a1a, p1 + 38.0, 19.0, 6, 5);
    g.p(0x3a1a10, p1 + 39.0, 13.0, 4, 2);
    put(g, 0x5a2a1a, js_round(p1 + 41.0), 17);
    // Little picture lamps.
    for x in [p1 + 13.0, p1 + 39.0] {
        g.p(0xb08a3a, x, 9.0, 4, 1);
        glow(g, 0xfff0c0, x + 2.0, 13.0, 7.0, 0.25);
    }
    // Stanchions and a velvet rope.
    for x in [p1 + 6.0, p1 + 28.0, p1 + 50.0] {
        g.p(0xc8a040, x, 36.0, 1, 10);
        g.p(0xc8a040, x - 1.0, (FLOOR - 1) as f64, 3, 1);
        g.p(0xffe080, x, 35.0, 1, 1);
    }
    for (a, b) in [(p1 + 6.0, p1 + 28.0), (p1 + 28.0, p1 + 50.0)] {
        for x in js_round(a)..=js_round(b) {
            let u = (x as f64 - a) / (b - a);
            put(g, 0xa01830, x, 37 + js_round(u * (1.0 - u) * 12.0));
        }
    }
    // Two beams sweeping back and forth.
    if !alarm {
        for k in 0..2 {
            let x = p1 + 30.0 + (t * 1.4 + k as f64 * PI).sin() * 22.0;
            beam(g, (x, 7.0), (x + 6.0, FLOOR as f64), f, 0.8);
        }
    }
}

/// Panel 3: the guard's desk and the low beam by the door.
fn guard_post(g: &mut World, f: u32, t: f64) {
    let p3 = g.panel(3) as f64;
    // An arched doorway the cat comes through.
    g.p(0x05060c, p3 + 44.0, 18.0, 14, FLOOR - 18);
    g.disc(0x05060c, p3 + 51.0, 19.0, 7);
    g.p(0x2a2e48, p3 + 43.0, 18.0, 1, FLOOR - 18);
    // Desk with security monitors.
    g.p(0x3a2a1e, p3 + 4.0, 36.0, 20, 2);
    g.p(0x2a1e14, p3 + 6.0, 38.0, 2, 8);
    g.p(0x2a1e14, p3 + 20.0, 38.0, 2, 8);
    for k in 0..2 {
        let mx = p3 + 6.0 + k as f64 * 9.0;
        g.p(0x2a2e3a, mx, 27.0, 8, 7);
        let c = if (f / 4 + k).is_multiple_of(7) {
            0x8ad8ff
        } else {
            0x3a8aa8
        };
        g.p(c, mx + 1.0, 28.0, 6, 5);
        g.p(
            0x2a6a88,
            mx + 2.0,
            30.0 + (k as f64 + t * 2.0).sin().round(),
            3,
            1,
        );
        g.p(0x2a2e3a, mx + 3.0, 34.0, 2, 2);
        glow(g, 0x6ad8ff, mx + 4.0, 31.0, 8.0, 0.2);
    }
    // A mug.
    g.p(0xe8e8f0, p3 + 21.0, 33.0, 2, 3);
    put(g, 0xe8e8f0, js_round(p3 + 23.0), 34);
    // Emitter posts for the last, low beam.
    for x in [p3 + 28.0, p3 + 40.0] {
        g.p(0x3a3a4a, x, 38.0, 2, 8);
        put(
            g,
            0xff4050,
            js_round(x + if x < p3 + 30.0 { 1.0 } else { 0.0 }),
            41,
        );
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    room(g, f, t);
    let alarm = t >= T_TRIP;
    vault(g, f, t);
    gallery(g, f, t, alarm);
    guard_post(g, f, t);

    // Panel 2: skylight, moonbeam, spotlight and the pedestal.
    let p2 = g.panel(2) as f64;
    let cx = p2 + 30.0;
    let open = ((t - 0.4) / 0.5).clamp(0.0, 1.0);
    g.p(0x2a2e48, cx - 8.0, 0.0, 16, 6);
    g.p(0x3a5a8a, cx - 6.0, 0.0, 12, 5);
    g.p(0x1a2a4a, cx - 6.0 + open * 6.0, 0.0, 6, 5);
    g.p(0x1a2a4a, cx - open * 6.0, 0.0, 6, 5);
    for y in 5..FLOOR {
        let half = 5.0 + (y - 5) as f64 * 0.15;
        for x in js_round(cx - half)..=js_round(cx + half) {
            blend(g, 0x8aa8ff, x, y, 0.06 + 0.06 * open);
        }
    }
    // Spotlight cone onto the jewel.
    let (lx, ly) = (p2 + 44.0, 8.0);
    g.p(0x3a3e58, lx - 1.0, ly - 2.0, 4, 3);
    for k in 0..30 {
        let u = k as f64 / 30.0;
        let y = ly + 1.0 + (32.0 - ly) * u;
        let x = lx + (cx - lx) * u;
        let half = 1.0 + u * 5.0;
        for dx in -js_round(half)..=js_round(half) {
            blend(g, 0xfff4d8, js_round(x) + dx, js_round(y), 0.12);
        }
    }
    glow(g, 0xfff4d8, cx, 32.0, 10.0, 0.35);
    // Pedestal with a velvet cushion.
    g.p(0x0c0e1a, cx - 5.0, 35.0, 11, 11);
    g.p(0xd8d4c8, cx - 5.0, 35.0, 11, 1);
    g.p(0xa8a498, cx - 4.0, 36.0, 9, 10);
    g.p(0xc8c4b8, cx - 4.0, 36.0, 2, 10);
    g.p(0x7a7468, cx + 3.0, 36.0, 2, 10);
    g.p(0xd8d4c8, cx - 6.0, (FLOOR - 1) as f64, 13, 1);
    g.p(0x8a1a2a, cx - 3.0, 33.0, 7, 2);
    // Its reflection on the marble.
    for y in FLOOR + 1..FLOOR + 6 {
        for x in js_round(cx - 4.0)..js_round(cx + 5.0) {
            if (x + y) % 2 == 0 {
                blend(g, 0xa8a498, x, y, 0.25);
            }
        }
    }

    // The jewel, then the rock it is swapped for.
    let swapped = t >= T_SWAP;
    let jewel = |g: &mut World, x: f64, y: f64| {
        g.spr(
            &[".bcb.", "bcwcb", ".bcb.", "..b.."],
            &[('b', 0x2a8ad8), ('c', 0x6ae0ff), ('w', 0xffffff)],
            x - 2.0,
            y,
            false,
        );
    };
    if t < T_GRAB {
        jewel(g, cx, 29.0);
        if (f / 2).is_multiple_of(5) {
            put(g, 0xffffff, js_round(cx + 2.0), 28);
            put(g, 0xffffff, js_round(cx + 3.0), 27);
        }
    } else if swapped {
        g.spr(
            &[".gg.", "gGgg", "gggg"],
            &[('g', 0x6a6a6a), ('G', 0x9a9a9a)],
            cx - 2.0,
            30.0,
            false,
        );
    }

    // Lasers criss-cross above the pedestal.
    let lk = if alarm { 0.0 } else { 1.0 };
    if lk > 0.0 {
        for (a, b) in [
            ((p2 + 4.0, 12.0), (p2 + 56.0, 30.0)),
            ((p2 + 4.0, 30.0), (p2 + 56.0, 12.0)),
            ((p2 + 4.0, 20.0), (p2 + 56.0, 22.0)),
        ] {
            emitter(g, a.0, a.1);
            emitter(g, b.0, b.1);
            beam(g, a, b, f, lk);
        }
    }

    // The burglar.
    let rope_top = 0.0;
    let (bx, by, rows): (f64, f64, &[&str]);
    let mut holding = false;
    if t < T_DROP {
        bx = cx;
        by = -20.0;
        rows = STRAIGHT;
    } else if t < T_FLAT {
        let u = (t - T_DROP) / (T_FLAT - T_DROP);
        by = -12.0 + ease(u) * 30.0;
        bx = cx;
        rows = if (0.3..0.5).contains(&u) {
            SPLIT
        } else if (0.55..0.75).contains(&u) {
            TUCK
        } else {
            STRAIGHT
        };
    } else if t < T_UP {
        bx = cx;
        by = 24.0;
        let reach = (T_GRAB - 0.2..T_SWAP + 0.2).contains(&t);
        rows = FLAT[reach as usize];
        holding = t > T_GRAB;
    } else if t < T_YANK {
        let u = (t - T_UP) / (T_YANK - T_UP);
        bx = cx;
        by = 18.0 - u * 6.0;
        rows = STRAIGHT;
        holding = true;
    } else {
        let u = ((t - T_YANK) / 0.35).min(1.0);
        bx = cx;
        by = 12.0 - u * u * 40.0;
        rows = SPLIT;
        holding = true;
    }
    let w = rows[0].len() as f64;
    let harness = if rows.len() == 5 { 1.0 } else { 5.0 };
    let hook_y = by + harness;
    line(g, 0xb8b0a0, cx, rope_top, cx, hook_y - 1.0);
    let sx = bx - (w / 2.0).floor();
    // A moonlit rim so the dark suit reads against the dark room.
    let rim: Vec<(char, u32)> = BUR.iter().map(|&(c, _)| (c, 0x7a8ac8)).collect();
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        g.spr(rows, &rim, sx + dx, by + dy, false);
    }
    g.spr(rows, BUR, sx, by, false);
    if holding && rows.len() == 5 {
        jewel(g, bx + 4.0, by + 5.0);
    } else if holding {
        jewel(g, bx + 4.0, by + 3.0);
    }
    if rows.len() == 5 && t > T_SWAP && t < T_SWAP + 0.4 {
        g.spr(
            &[".gg.", "gGgg"],
            &[('g', 0x6a6a6a), ('G', 0x9a9a9a)],
            bx + 1.0,
            by + 5.0,
            false,
        );
    }
    // A thumbs up after the swap.
    if (T_SWAP + 0.4..T_UP).contains(&t) {
        g.p(0x101018, bx - 6.0, by - 3.0, 2, 2);
        g.p(0x101018, bx - 6.0, by - 5.0, 1, 2);
    }
    // Speed lines as the burglar is yanked out.
    if (T_YANK..T_YANK + 0.5).contains(&t) {
        for k in 0..5 {
            let x = cx - 6.0 + k as f64 * 3.0;
            line(
                g,
                0xd8d8f0,
                x,
                by + 12.0,
                x,
                by + 20.0 + (k % 2) as f64 * 4.0,
            );
        }
    }

    // The cat strolls in from the door and through the low beam.
    let p3 = g.panel(3) as f64;
    let beam_l = (p3 + 29.0, 41.0);
    let beam_r = (p3 + 40.0, 41.0);
    let walk_end = 7.4;
    let cat_x = if t < walk_end {
        p3 + 52.0 - (t - T_CAT).max(0.0) * 8.0
    } else {
        p3 + 52.0 - (walk_end - T_CAT) * 8.0
    };
    if t > T_CAT {
        if t < walk_end {
            g.spr(
                CAT[((f / 2) % 2) as usize],
                CATP,
                cat_x,
                (FLOOR - 6) as f64,
                false,
            );
        } else {
            g.spr(
                CAT_SIT[((f / 4) % 2) as usize],
                CATP,
                cat_x + 1.0,
                (FLOOR - 7) as f64,
                false,
            );
        }
    }
    let cut = if t > T_CAT && cat_x < beam_r.0 - 1.0 {
        cat_x + 8.0
    } else {
        beam_l.0
    };
    if t < T_TRIP + 0.1 || (f / 2).is_multiple_of(2) {
        beam(g, (cut.max(beam_l.0), beam_l.1), beam_r, f, 1.0);
    }

    // The guard dozes until the alarm, then leaps up.
    if !alarm {
        g.spr(
            GUARD_SLEEP,
            GUARDP,
            p3 + 12.0,
            28.0 + ((t * 2.0).sin() * 0.5).round(),
            false,
        );
        for k in 0..3 {
            let age = (t * 0.6 + k as f64 / 3.0) % 1.0;
            let (zx, zy) = (p3 + 20.0 + age * 8.0, 24.0 - age * 14.0);
            let c = fx::mix(0xc8d0ff, 0x22284a, age);
            g.p(c, zx, zy, 4, 1);
            put(g, c, js_round(zx + 2.0), js_round(zy + 1.0));
            put(g, c, js_round(zx + 1.0), js_round(zy + 2.0));
            g.p(c, zx, zy + 3.0, 4, 1);
        }
    } else {
        let hop = if t < T_TRIP + 0.4 {
            ((t - T_TRIP) / 0.4 * PI).sin() * 4.0
        } else {
            0.0
        };
        g.spr(
            GUARD_UP,
            GUARDP,
            p3 + 13.0,
            (FLOOR - 11) as f64 - hop,
            false,
        );
        if t < T_TRIP + 1.5 {
            g.p(0xffffff, p3 + 16.0, (FLOOR - 18) as f64 - hop, 1, 3);
            put(
                g,
                0xffffff,
                js_round(p3 + 16.0),
                js_round((FLOOR - 14) as f64 - hop),
            );
        }
    }

    // Alarm: beacons spin and red floods every screen, from the last to
    // the first.
    if alarm {
        let n = g.layout.panels;
        for i in 0..n {
            let delay = (n - 1 - i) as f64 * 0.12;
            if t < T_TRIP + delay {
                continue;
            }
            let x0 = g.px0(i);
            let bx = x0 as f64 + 30.0;
            let pulse = ((t - T_TRIP - delay) * 2.0 * PI * 1.6).sin() * 0.5 + 0.5;
            for y in 0..60 {
                for x in x0 - 2..x0 + 62 {
                    blend(g, 0xff1020, x, y, 0.12 + 0.3 * pulse);
                }
            }
            g.p(0x3a3a4a, bx - 3.0, 6.0, 7, 1);
            g.p(0xff3030, bx - 2.0, 3.0, 5, 3);
            g.p(0xffd0d0, bx - 1.0, 3.0, 2, 1);
            let a = (t - T_TRIP) * 9.0 + i as f64;
            for r in 2..28 {
                for s in -2..=2 {
                    let b = a + s as f64 * 0.04;
                    let k = 0.35 * (1.0 - r as f64 / 28.0);
                    blend(
                        g,
                        0xff6050,
                        js_round(bx + b.cos() * r as f64),
                        js_round(4.0 + b.sin().abs() * r as f64 * 0.8),
                        k,
                    );
                }
            }
        }
    }
}

//! Birds at sunset: a marsh at dusk, the sky running violet to gold over
//! still water that mirrors it, reeds and cattails swaying, lily pads, a
//! heron fishing in the shallows and an old rowing boat with an angler,
//! the sun sinking into the water on the last screen. A V of geese honks
//! across and off the right edge, with one little goose far behind,
//! flapping like mad, dipping so low it skims the water and startles the
//! heron. Worn out, it lands on a mooring post and pants. Then the flock
//! comes back for it: the V swings round, the straggler takes off and
//! slots in at the tip, and leads them all home across the setting sun.

use super::fx::{blend, get, glow, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.5;

/// The waterline.
const HZ: i32 = 38;

/// Where the heron stands in the shallows.
const HERON_FEET: f64 = 50.0;
const T_POST: f64 = 6.6;
const T_BACK: f64 = 7.4;
const T_TAKEOFF: f64 = 8.5;
const T_JOIN: f64 = 9.0;

const GOOSEP: &[(char, u32)] = &[
    ('k', 0x2a1a30),
    ('l', 0x4a2e48),
    ('r', 0xf0a060),
    ('b', 0x1a1020),
    ('h', 0xf0905a),
];
/// A goose in flight facing right: wings up, level, down.
const GOOSE: [&[&str]; 3] = [
    &[
        "...kr....",
        "...kkr...",
        "....kkr..",
        "kkkkkkkkb",
        "..llkk...",
        ".........",
    ],
    &[
        ".........",
        ".........",
        "..kkkkr..",
        "kkkkkkkkb",
        "..llkk...",
        ".........",
    ],
    &[
        ".........",
        ".........",
        "....kk...",
        "kkkkkkkkb",
        "..llkkr..",
        "...kkr...",
    ],
];
/// The straggler sitting on the post, facing right; tired, then perked up.
/// Both are eight rows tall so they sit on the same spot.
const SIT: [&[&str]; 2] = [
    &[
        ".........",
        ".........",
        "......kb.",
        ".....khk.",
        ".....kk..",
        "..hhhkk..",
        ".kkkkkkk.",
        "kkllllkk.",
    ],
    &[
        ".....kbb.",
        ".....kh..",
        ".....k...",
        ".....k...",
        "..hhhkk..",
        ".kkkkkkk.",
        "kkllllkk.",
        ".kkkkkk..",
    ],
];

const HERONP: &[(char, u32)] = &[
    ('k', 0x3a3050),
    ('l', 0x7a6a90),
    ('b', 0xe0a050),
    ('e', 0xf0e0a0),
];
/// The heron facing left: still, then startled with its neck shot up.
const HERON: [&[&str]; 2] = [
    &[
        "...........",
        "...........",
        "..kkkk.....",
        "bbkek......",
        "...kk......",
        "....k......",
        "....kk.....",
        ".....k.....",
        "....kkk....",
        "...kklllk..",
        "..kklllllk.",
        "..kkllllllk",
        "...kkkkkkk.",
        ".....k.....",
        ".....k.....",
        ".....k.....",
        "....k.k....",
    ],
    &[
        "..kkkk.....",
        "bbkek......",
        "...kk......",
        "....k......",
        "....k......",
        "....k......",
        "....kk.....",
        "...kkkk....",
        "..kklllkk..",
        ".kklllllkk.",
        "..kkllllllk",
        "...kklllkk.",
        "....kkkkk..",
        ".....k.....",
        ".....k.....",
        ".....k.....",
        "....k.k....",
    ],
];

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}
fn last(g: &World) -> u32 {
    g.layout.panels - 1
}
/// The sun's centre: sinking into the water on the last screen.
fn sun(g: &World, t: f64) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 38.0, 21.0 + (t / SECS) * 17.0)
}
/// Top of the mooring post the straggler rests on.
fn post(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 21.0, 46.0)
}
fn heron_x(g: &World) -> f64 {
    g.panel(if g.layout.panels >= 4 { 1 } else { 0 }) as f64 + 36.0
}

/// V slots behind the tip: alternate sides, each a step further back.
fn slot(i: usize) -> (f64, f64) {
    if i == 0 {
        return (0.0, 0.0);
    }
    let rank = i.div_ceil(2) as f64;
    let side = if i % 2 == 1 { -1.0 } else { 1.0 };
    (-rank * 7.0, side * rank * 2.8)
}

/// The flock's tip: (world x, y, facing right). Out to the right first, then
/// back for the straggler.
fn flock(g: &World, t: f64) -> (f64, f64, bool) {
    let (px, py) = post(g);
    let ps = stage_at(g, last(g), px - g.panel(last(g)) as f64);
    if t < T_BACK {
        let s = -10.0 + t * 48.0;
        (stage_x(g, s), 19.0 + (t * 0.8).sin() * 2.0, true)
    } else if t < T_JOIN {
        // Swinging round and coming back in low.
        let u = (t - T_BACK) / (T_JOIN - T_BACK);
        let s = ps + 70.0 - ease(u) * 70.0;
        (stage_x(g, s), 14.0 + (py - 14.0 - 19.0) * ease(u), false)
    } else {
        let s = ps - (t - T_JOIN) * 26.0 - (t - T_JOIN).powi(2) * 6.0;
        (stage_x(g, s), py - 19.0 - (t - T_JOIN) * 3.0, false)
    }
}

/// The straggler's flight: (x, y, facing right, flap speed).
fn straggler(g: &World, t: f64) -> (f64, f64, bool, f64) {
    let (px, py) = post(g);
    let ps = stage_at(g, last(g), px - g.panel(last(g)) as f64);
    if t < T_POST {
        let s0 = -40.0;
        let u = t / T_POST;
        let s = s0 + (ps - 3.0 - s0) * (u * 0.85 + ease(u) * 0.15);
        // Sinking lower and lower, a dip to the water by the heron.
        let hs = stage_at(g, if g.layout.panels >= 4 { 1 } else { 0 }, 30.0);
        let dip = (-((s - hs) / 14.0).powi(2)).exp() * 14.0;
        let y = 22.0 + u * 6.0 + (t * 5.0).sin() * 1.5 + dip;
        let y = if t > T_POST - 0.6 {
            y + (py - 6.0 - y) * smooth(T_POST - 0.6, T_POST, t)
        } else {
            y
        };
        (stage_x(g, s), y.min(HZ as f64 - 3.0), true, 2.0)
    } else if t < T_TAKEOFF {
        (px - 4.0, py - 8.0, true, 0.0)
    } else {
        // Up off the post, turning to lead the V.
        let (fx, fy, _) = flock(g, t);
        let u = smooth(T_TAKEOFF, T_JOIN, t);
        let x = px + (fx - px) * u;
        let y = py - 4.0 + (fy - py + 4.0) * u;
        (x, y, false, 1.0)
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    let dusk = t / SECS;
    let stops = [
        mix(0x3a2a6a, 0x221a4a, dusk),
        mix(0x8a3a7a, 0x5a2a6a, dusk),
        mix(0xe0585a, 0xc04a5a, dusk),
        mix(0xf8964a, 0xe8703a, dusk),
        mix(0xffd27a, 0xf8a050, dusk),
    ];
    vgrad(g, &stops, 0, w, 0, HZ);
    // The first stars as it darkens.
    for i in 0..g.spread(20) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * w as f64) as i32, (rnd(fi + 1.0) * 12.0) as i32);
        blend(g, 0xf0e8ff, x, y, smooth(5.0, 11.0, t) * rnd(fi + 2.0));
    }
    let (sx, sy) = sun(g, t);
    glow(g, 0xffd890, sx, sy, 30.0, 0.5);
    glow(g, 0xfff0c0, sx, sy, 12.0, 0.6);
    g.disc(0xffc070, sx, sy, 7);
    g.disc(0xffe0a0, sx, sy, 6);
    g.disc(0xfff4d0, sx - 1.0, sy - 1.0, 3);
    // Long thin clouds lit from below.
    for k in 0..g.spread(9) {
        let kf = k as f64;
        let span = w as f64 + 60.0;
        let x = (rnd(kf + 10.0) * span + t * 1.5) % span - 30.0;
        let y = 6.0 + rnd(kf + 11.0) * 20.0;
        let len = 14 + (rnd(kf + 12.0) * 22.0) as i32;
        let near_sun = (1.0 - ((x - sx).abs() / 60.0)).max(0.0);
        let top = mix(0x6a3a7a, 0x8a4a7a, near_sun);
        let lit = mix(0xf08a6a, 0xffd890, near_sun);
        g.p(top, x, y, len, 1);
        g.p(lit, x + 2.0, y + 1.0, len - 3, 1);
        g.p(top, x + 5.0, y - 1.0, len / 2, 1);
    }
}

/// Far shore: low wooded hills in hazy violet.
fn shore(g: &mut World) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let h = 1.5 + 1.2 * (xf * 0.06).sin() + 1.0 * (xf * 0.17 + 1.3).sin() + rnd(xf) * 0.8;
        let top = HZ - js_round(h.max(0.5));
        for y in top..HZ {
            // Hazy: lighter towards the waterline.
            let k = (y - top) as f64 / 4.0;
            put(
                g,
                if on(x, y, k * 0.5) {
                    0x7a3a6a
                } else {
                    0x5a2c5e
                },
                x,
                y,
            );
        }
    }
}

/// Water: the sky mirrored, rippled and darkened, with the sun's glitter.
fn water(g: &mut World, t: f64, f: u32) {
    let w = g.w();
    for y in HZ..g.h() {
        let depth = (y - HZ) as f64 / (g.h() - HZ) as f64;
        let src = 2 * HZ - 1 - y;
        for x in 0..w {
            let wob = js_round(
                ((y as f64) * 0.9 + t * 3.0 + x as f64 * 0.05).sin() * (0.5 + depth * 1.5),
            );
            let c = get(g, x + wob, src.max(0));
            put(g, mix(c, 0x1a1238, 0.25 + depth * 0.35), x, y);
        }
    }
    // Glints of the sun on the water: short streaks that flicker.
    let (sx, sy) = sun(g, t);
    for y in HZ + 1..g.h() {
        let d = (y - HZ) as f64;
        let hw = 3.0 + d * 0.45;
        for j in 0..(2 + (d / 5.0) as i32) {
            let seed = (y * 13 + j * 7) as f64 + (f / 2) as f64 * 0.71;
            let x = sx + (rnd(seed) - 0.5) * 2.0 * hw;
            let len = 1 + (rnd(seed + 1.0) * 3.0) as i32;
            for i in 0..len {
                blend(g, 0xffe8a8, js_round(x) + i, y, 0.85 - d * 0.02);
            }
        }
    }
    // Bright waterline under the sinking sun.
    let hw = (49.0 - (sy - HZ as f64).powi(2)).max(0.0).sqrt();
    if hw > 0.0 {
        g.p(0xfff0c0, sx - hw, HZ as f64, (2.0 * hw) as i32 + 1, 1);
    }
    // Long ripple lines.
    for k in 0..g.spread(24) {
        let kf = k as f64;
        let y = HZ + 2 + (rnd(kf + 40.0) * 20.0) as i32;
        let x = (rnd(kf + 41.0) * w as f64 + t * 2.0) % w as f64;
        let len = 3 + (rnd(kf + 42.0) * 6.0) as i32;
        for i in 0..len {
            blend(g, 0xf8b880, js_round(x) + i, y, 0.25);
        }
    }
}

/// A cattail clump: stems sway, brown heads, sword leaves.
fn reeds(g: &mut World, x: f64, n: usize, t: f64, seed: f64) {
    for k in 0..n {
        let kf = k as f64 + seed;
        let bx = x + k as f64 * 2.3 + rnd(kf) * 2.0;
        let h = 12.0 + rnd(kf + 1.0) * 12.0;
        let sway = ((t * 1.3 + kf).sin()) * 1.6;
        for i in 0..(h as i32) {
            let u = i as f64 / h;
            let xx = bx + sway * u * u;
            put(g, 0x1e1428, js_round(xx), 60 - i);
        }
        let tip = (bx + sway, 60.0 - h);
        if rnd(kf + 2.0) > 0.4 {
            g.p(0x3a1e1a, tip.0 - 1.0, tip.1, 2, 4);
            put(g, 0x5a3424, js_round(tip.0) - 1, js_round(tip.1));
            put(g, 0x1e1428, js_round(tip.0), js_round(tip.1) - 1);
        } else {
            // A blade leaf curling over.
            for i in 0..5 {
                put(
                    g,
                    0x2a1a30,
                    js_round(tip.0 + i as f64 * 0.6),
                    js_round(tip.1 + i as f64 * 0.5 - 1.0),
                );
            }
        }
    }
}

fn lily_pads(g: &mut World, x: f64, y: f64) {
    for (dx, dy, r) in [(0.0, 0.0, 3.0), (7.0, 2.0, 2.0), (-6.0, 3.0, 2.0)] {
        let (cx, cy) = (x + dx, y + dy);
        for i in -(r as i32 * 2)..=(r as i32 * 2) {
            let hh = ((1.0 - (i as f64 / (r * 2.0)).powi(2)).max(0.0)).sqrt() * r * 0.5;
            for j in -(hh.round() as i32)..=(hh.round() as i32) {
                put(
                    g,
                    if j < 0 { 0x3a5a3a } else { 0x2a3a2e },
                    js_round(cx) + i,
                    js_round(cy) + j,
                );
            }
        }
        put(g, 0x1a2a20, js_round(cx) + 1, js_round(cy));
    }
    // A pink water lily on the big pad.
    put(g, 0xf0a0c0, js_round(x) - 1, js_round(y) - 1);
    put(g, 0xf8d0e0, js_round(x), js_round(y) - 2);
    put(g, 0xf0a0c0, js_round(x) + 1, js_round(y) - 1);
}

fn heron(g: &mut World, t: f64) {
    let x = heron_x(g);
    let td = t_dip(g);
    let startled = (td - 0.1..td + 1.3).contains(&t);
    let pose = usize::from(startled);
    g.spr(HERON[pose], HERONP, x, HERON_FEET - 17.0, false);
    // Its reflection, broken by ripples.
    for (r, row) in HERON[pose].iter().enumerate().rev().take(8) {
        for (c, ch) in row.chars().enumerate() {
            if ch != '.' && (r + c) % 2 == 0 {
                blend(
                    g,
                    0x2a2038,
                    js_round(x) + c as i32,
                    HERON_FEET as i32 + (16 - r as i32),
                    0.5,
                );
            }
        }
    }
    if startled {
        let k = js_round(x) + 7;
        g.p(0xfff0e0, k as f64, HERON_FEET - 22.0, 1, 3);
        put(g, 0xfff0e0, k, HERON_FEET as i32 - 18);
    }
}

/// When the straggler skims past the heron (the lowest point of its dip).
fn t_dip(g: &World) -> f64 {
    let mut best = (f64::MAX, 3.0);
    for i in 0..48 {
        let tt = 1.5 + i as f64 / 12.0;
        let (_, y, _, _) = straggler(g, tt);
        if -y < best.0 {
            best = (-y, tt);
        }
    }
    best.1
}

/// The old rowing boat with an angler and a lantern.
fn boat(g: &mut World, t: f64, f: u32) {
    if g.layout.panels < 4 {
        return;
    }
    let x = g.panel(2) as f64 + 14.0;
    let y = HZ as f64 + 8.0 + (t * 1.5).sin() * 0.5;
    for r in 0..3 {
        g.p(0x2a1a28, x + r as f64, y + r as f64, 22 - 2 * r, 1);
    }
    g.p(0x4a2a30, x - 1.0, y - 1.0, 24, 1);
    // The angler: hat, body, rod and line.
    g.p(0x1e1428, x + 12.0, y - 6.0, 3, 5);
    g.p(0x1e1428, x + 11.0, y - 7.0, 5, 1);
    g.p(0x1e1428, x + 12.0, y - 9.0, 3, 2);
    let tip = (x + 26.0, y - 14.0 + (t * 2.0).sin() * 0.5);
    super::fx::line(g, 0x2a1a30, x + 14.0, y - 4.0, tip.0, tip.1);
    for i in 0..((y + 3.0 - tip.1) as i32) {
        blend(g, 0xd8a8a0, js_round(tip.0 + 1.0), js_round(tip.1) + i, 0.3);
    }
    // Bobber with a ripple.
    put(g, 0xf04a4a, js_round(tip.0 + 1.0), js_round(y) + 3);
    if (f / 6).is_multiple_of(3) {
        for i in -2..=2 {
            blend(g, 0xf8c890, js_round(tip.0 + 1.0) + i, js_round(y) + 4, 0.4);
        }
    }
    // A lantern on the bow.
    g.p(0x1e1428, x + 2.0, y - 5.0, 1, 4);
    put(g, 0xffd070, js_round(x) + 2, js_round(y) - 6);
    glow(
        g,
        0xffc060,
        x + 2.0,
        y - 6.0,
        6.0,
        0.5 * smooth(4.0, 8.0, t) + 0.15,
    );
}

fn post_draw(g: &mut World) {
    let (px, py) = post(g);
    g.p(0x2a1a24, px - 1.0, py, 3, 60 - py as i32);
    g.p(0x4a2e30, px - 1.0, py, 3, 1);
    // Rope around it.
    g.p(0x6a4a40, px - 1.0, py + 4.0, 3, 1);
}

/// A goose centred on `(x, y)`; the water mirrors it later.
fn goose(g: &mut World, x: f64, y: f64, right: bool, phase: usize) {
    g.spr(GOOSE[phase % 3], GOOSEP, x - 4.0, y - 3.0, !right);
}

fn geese(g: &mut World, t: f64, f: u32) {
    let (tx, ty, right) = flock(g, t);
    let joined = t >= T_JOIN;
    // Seven geese; after the swing round the old leader drops into slot 1.
    for i in 0..7usize {
        let s = if t >= T_BACK { i + 1 } else { i };
        let (dx, dy) = slot(s);
        let dx = if right { dx } else { -dx };
        let ph = ((f as usize / 2) + i * 2) % 3;
        let bob = ((t * 3.0 + i as f64).sin() * 0.6).round();
        goose(g, tx + dx, ty + dy + bob, right, ph);
    }
    // The straggler.
    let (sx, sy, sright, flap) = straggler(g, t);
    if (T_POST..T_TAKEOFF).contains(&t) {
        let perked = t > T_BACK + 0.4;
        g.spr(SIT[usize::from(perked)], GOOSEP, sx, sy, false);
        if !perked {
            // Panting puffs.
            let u = (t * 2.0) % 1.0;
            put(
                g,
                0xf8e8f0,
                js_round(sx + 8.0 + u * 3.0),
                js_round(sy + 1.0 - u * 2.0),
            );
        } else if t < T_TAKEOFF {
            // A happy honk at the returning flock.
            for k in 0..3 {
                let a = -0.6 + k as f64 * 0.6;
                put(
                    g,
                    0xfff0e0,
                    js_round(sx + 9.0 + a.cos() * 3.0),
                    js_round(sy + 1.0 + a.sin() * 3.0),
                );
            }
        }
    } else {
        let ph = if flap > 1.5 {
            (f as usize) % 3
        } else {
            (f as usize / 2) % 3
        };
        goose(g, sx, sy, sright, ph);
        if t < T_POST && (f / 4).is_multiple_of(3) {
            // Sweat drop.
            put(g, 0xa8d8ff, js_round(sx) - 3, js_round(sy) - 4);
        }
        if joined {
            // Proud sparkle on the new leader.
            let k = ((t - T_JOIN) * 4.0) as i32 % 4;
            if k < 2 {
                put(g, 0xfff4c0, js_round(sx) - 4, js_round(sy) - 6);
                put(g, 0xfff4c0, js_round(sx) - 5, js_round(sy) - 5);
                put(g, 0xfff4c0, js_round(sx) - 3, js_round(sy) - 5);
                put(g, 0xfff4c0, js_round(sx) - 4, js_round(sy) - 4);
            }
        }
    }
    // The splash when it skims the water by the heron.
    let td = t_dip(g);
    let (dx, dy, _, _) = straggler(g, td);
    if (td - 0.2..td + 0.5).contains(&t) && dy > HZ as f64 - 6.0 {
        let u = (t - td + 0.2) / 0.7;
        for k in 0..6 {
            let a = PI + k as f64 / 5.0 * PI;
            let r = u * 6.0;
            put(
                g,
                0xfff0d0,
                js_round(dx + a.cos() * r),
                js_round(HZ as f64 + 1.0 + a.sin() * r * 0.7 + u * u * 6.0),
            );
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    shore(g);
    geese(g, t, f);
    water(g, t, f);
    lily_pads(g, g.panel(0) as f64 + 34.0, 50.0);
    heron(g, t);
    boat(g, t, f);
    post_draw(g);
    geese_low(g, t, f);
    reeds(g, g.panel(0) as f64 + 2.0, 6, t, 0.0);
    if g.layout.panels >= 4 {
        reeds(g, g.panel(1) as f64 + 4.0, 3, t, 20.0);
    }
    reeds(g, g.panel(last(g)) as f64 + 46.0, 5, t, 40.0);
}

/// The straggler on the post is in front of the water, so redraw it there.
fn geese_low(g: &mut World, t: f64, f: u32) {
    if (T_POST - 0.6..T_TAKEOFF + 0.3).contains(&t) {
        let (sx, sy, sright, _) = straggler(g, t);
        if (T_POST..T_TAKEOFF).contains(&t) {
            let perked = t > T_BACK + 0.4;
            g.spr(SIT[usize::from(perked)], GOOSEP, sx, sy, false);
        } else {
            goose(g, sx, sy, sright, (f as usize / 2) % 3);
        }
    }
}

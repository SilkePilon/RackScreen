//! Mountains, alpine dawn: stars fade as the sky runs from night blue to
//! gold, and three ranges of peaks catch the first light, pink alpenglow
//! turning to gold on their sunlit faces. Mist lies in the valleys, pines
//! fill the lower slopes, a little red cable car creeps up its wire and a
//! chalet's chimney smokes at the foot of the big mountain. A climber with
//! a huge pack sets off from the chalet and slogs up the ridge across every
//! screen, while a mountain goat watches him puff past and then bounds up
//! the cliffs in three easy leaps. Just as the sun breaks over the summit
//! the climber gets there, plants his flag and throws his arms up in the
//! sunburst, until the goat strolls over and eats the flag.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_SET_OFF: f64 = 0.6;
const T_SUMMIT: f64 = 8.6;
const T_FLAG: f64 = 8.9;
const T_GOAT_GO: f64 = 3.9;
const T_GOAT_UP: f64 = 6.0;
const T_CHEW: f64 = 10.2;

const CLIMBP: &[(char, u32)] = &[
    ('k', 0x1a1420),
    ('r', 0xe04a3a),
    ('d', 0xa02a2a),
    ('s', 0xf0b890),
    ('b', 0x3a5a9a),
    ('p', 0xe8a030),
    ('o', 0xa86a20),
    ('h', 0x2a8a6a),
];
/// The climber facing right: two strides, then arms up at the summit.
const CLIMBER: [&[&str]; 3] = [
    &[
        "..hh....", ".hhhh...", "..ss....", "ppsk....", "pprrr...", "porrrs..", "ppdrr...",
        "..bb....", "..b.b...", ".kk.kk..",
    ],
    &[
        "..hh....", ".hhhh...", "..ss....", "ppsk....", "pprrr...", "porrr...", "ppdrrs..",
        "..bb....", "..bb....", "..kkk...",
    ],
    &[
        "..hh.s..", ".hhhhr..", "s.ss.r..", "rpsk.r..", "rprrrr..", "porrr...", "ppdrr...",
        "..bb....", "..b.b...", ".kk.kk..",
    ],
];

const GOATP: &[(char, u32)] = &[
    ('w', 0xf4f0e8),
    ('g', 0xc8c4c8),
    ('k', 0x2a2430),
    ('h', 0x4a4048),
    ('r', 0xe04a3a),
];
/// The goat facing left: standing, leaping, chewing.
const GOAT: [&[&str]; 3] = [
    &[
        ".h.......",
        "hh.......",
        "kww......",
        "wwwwwwww.",
        ".wwwwwwgw",
        ".wgwwwwg.",
        ".k.k..k.k",
    ],
    &[
        "..h......",
        ".hh......",
        ".kww.....",
        "..wwwwwwww",
        "..wwwwwwg.",
        ".k.wgwwk..",
        "k.......k.",
    ],
    &[
        ".h.......",
        "hh.......",
        "kww......",
        "rwwwwwww.",
        "rwwwwwwgw",
        ".wgwwwwg.",
        ".k.k..k.k",
    ],
];

/// A sprite with a 1 px dark outline so it reads on snow and sky.
fn outlined(g: &mut World, rows: &[&str], pal: &[(char, u32)], x: f64, y: f64, flip: bool) {
    let dark: Vec<(char, u32)> = pal.iter().map(|&(k, _)| (k, 0x1a1420)).collect();
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        g.spr(rows, &dark, x + dx, y + dy, flip);
    }
    g.spr(rows, pal, x, y, flip);
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}

/// The summit of the big mountain the climber goes up.
fn summit(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 28.0, 21.0)
}

/// How far the dawn has come, `0..1`.
fn dawn(t: f64) -> f64 {
    (t / SECS).clamp(0.0, 1.0)
}

/// The colour of the first light on the snow: pink, then gold, then white.
fn sunlight(t: f64) -> u32 {
    let k = dawn(t);
    if k < 0.45 {
        mix(0xf08aa8, 0xffb070, k / 0.45)
    } else {
        mix(0xffb070, 0xfff0d0, (k - 0.45) / 0.55)
    }
}

fn sun(g: &World, t: f64) -> (f64, f64) {
    let (sx, _) = summit(g);
    (sx + 14.0, 46.0 - ease(t / SECS * 1.1) * 36.0)
}

/// The front ridge, rising from the chalet to the summit and dropping
/// away behind it.
fn front(g: &World, x: f64) -> f64 {
    let (sx, sy) = summit(g);
    if x > sx {
        return sy + (x - sx) * 1.3 + ((x - sx) * 0.7).sin().abs() * 1.5;
    }
    let u = (x / sx).clamp(0.0, 1.0);
    let base = 52.0 - (52.0 - sy) * (u * 0.7 + u.powi(10) * 0.3);
    base - (x * 0.21).sin().abs() * 1.4 + (x * 0.07).sin() * 1.2 * (1.0 - u)
}

/// A distant range's ridge line at world x: sharp peaks from folded sines.
fn range(seed: f64, base: f64, amp: f64, x: f64) -> f64 {
    let a = (1.0 - (x * 0.034 + seed).sin().abs()).powf(1.4);
    let b = (1.0 - (x * 0.083 + seed * 2.3).sin().abs()).powf(1.6);
    base - amp * (a * 0.75 + b * 0.35) - (x * 0.31 + seed).sin().abs() * 0.8
}

/// Colour of a mountain pixel: lit faces dithered towards the dawn colour,
/// snow down to a ragged snowline with streaks in the gullies.
#[allow(clippy::too_many_arguments)]
fn rock_px(
    x: i32,
    y: i32,
    top: f64,
    lit_k: f64,
    snowline: f64,
    rock: u32,
    light: u32,
    shade: u32,
) -> u32 {
    let xf = x as f64;
    let depth = y as f64 - top;
    let ragged = snowline + (xf * 0.37).sin() * 2.5 + (xf * 0.91).sin() * 1.5;
    let streak = (xf * 0.55 + (y as f64) * 0.18).sin() > 0.55 && (y as f64) < ragged + 7.0;
    let snow = (y as f64) < ragged || streak;
    let k = lit_k * (1.0 - depth / 26.0).max(0.0);
    let (hi, lo) = if snow {
        (mix(light, 0xffffff, 0.3), mix(shade, light, 0.35 * lit_k))
    } else {
        (mix(rock, light, 0.22), mix(rock, light, 0.08 * lit_k))
    };
    // Three dithered steps between shade and light.
    let level = k * 3.0;
    let step = level.floor() + if on(x, y, level.fract()) { 1.0 } else { 0.0 };
    let c = mix(lo, hi, step / 3.0);
    if !snow && on(x + 2, y, 0.12) {
        mix(c, 0x000010, 0.25)
    } else {
        c
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    let k = dawn(t);
    let stops = [
        mix(0x141a3e, 0x3a70b8, k),
        mix(0x2a2a5e, 0x6aa0d8, k),
        mix(0x6a4a7a, 0xa8c8e8, k),
        mix(0xc87a8a, 0xf8d8b0, k),
        mix(0xf0a080, 0xffe8b0, k),
    ];
    vgrad(g, &stops, 0, w, 0, 46);
    for i in 0..g.spread(40) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * w as f64) as i32, (rnd(fi + 1.0) * 22.0) as i32);
        blend(g, 0xf0f0ff, x, y, (1.0 - k * 2.2).max(0.0) * rnd(fi + 2.0));
    }
    let (sx, sy) = sun(g, t);
    glow(g, 0xffe0a0, sx, sy, 40.0, 0.35 + k * 0.2);
    glow(g, 0xfff0c8, sx, sy, 14.0, 0.6);
    g.disc(0xffd890, sx, sy, 6);
    g.disc(0xfff4d8, sx, sy, 5);
    // Thin clouds catching the light.
    for k2 in 0..g.spread(7) {
        let kf = k2 as f64;
        let span = w as f64 + 40.0;
        let x = (rnd(kf + 9.0) * span + t * 1.0) % span - 20.0;
        let y = 5.0 + rnd(kf + 10.0) * 14.0;
        let len = 10 + (rnd(kf + 11.0) * 18.0) as i32;
        let lit = sunlight(t);
        g.p(mix(0x5a4a7a, 0xb8c0e0, k), x, y, len, 1);
        g.p(mix(lit, 0xffffff, k * 0.5), x + 2.0, y + 1.0, len - 4, 1);
    }
}

/// A snowy range, faces towards the sun lit, the rest in blue shadow;
/// `haze` pulls it towards the sky.
#[allow(clippy::too_many_arguments)]
fn mountains(g: &mut World, t: f64, seed: f64, base: f64, amp: f64, rock: u32, haze: f64) {
    let w = g.w();
    let lit = sunlight(t);
    let shade = mix(0x7a80b8, lit, 0.12);
    let hz = mix(0x7a7aa8, 0xb8cce8, dawn(t));
    let snowline = base - amp * 0.55;
    for x in 0..w {
        let xf = x as f64;
        let top = range(seed, base, amp, xf);
        let d = range(seed, base, amp, xf + 2.0) - range(seed, base, amp, xf - 2.0);
        let lit_k = (d * 0.45).clamp(0.0, 1.0) * (0.4 + dawn(t) * 0.6);
        for y in js_round(top)..60 {
            let c = rock_px(x, y, top, lit_k, snowline, rock, lit, shade);
            let h = haze + ((y as f64 - top) / 30.0).min(1.0) * 0.35;
            put(g, mix(c, hz, h), x, y);
        }
        if lit_k > 0.2 {
            blend(g, 0xfff4e0, x, js_round(top), 0.5);
        }
    }
}

fn mist(g: &mut World, t: f64, y0: i32, rows: i32, k: f64) {
    let w = g.w();
    let c = mix(0xb0b0d8, 0xf4f4fa, dawn(t));
    for y in y0..y0 + rows {
        let mid = 1.0 - ((y - y0) as f64 / rows as f64 - 0.5).abs() * 2.0;
        for x in 0..w {
            let xf = x as f64;
            let banks = ((xf * 0.045 + t * 0.4).sin()
                + (xf * 0.017 - t * 0.25 + y as f64 * 0.3).sin())
                * 0.5
                + 0.35;
            let a = k * mid * banks.clamp(0.0, 1.0);
            if a > 0.0 {
                blend(g, c, x, y, a * 0.85);
            }
        }
    }
}

fn pine(g: &mut World, x: f64, y: f64, h: i32, lit: u32, k: f64) {
    let dark = 0x14283a;
    let mid = mix(0x1e3e44, lit, k * 0.25);
    for r in 0..h {
        let half = (r + 1) / 2 + (r % 3 == 2) as i32;
        let yy = js_round(y) - h + r;
        for i in -half..=half {
            let c = if i > 0 { mid } else { dark };
            put(g, c, js_round(x) + i, yy);
        }
    }
    put(g, 0x2a1a14, js_round(x), js_round(y));
}

fn forest(g: &mut World, t: f64) {
    let lit = sunlight(t);
    // A band of pines on the far valley floor.
    for k in 0..g.spread(70) {
        let kf = k as f64;
        let x = rnd(kf + 50.0) * g.w() as f64;
        let y = 43.0 + rnd(kf + 51.0) * 4.0;
        pine(g, x, y, 5 + (rnd(kf + 52.0) * 4.0) as i32, lit, dawn(t));
    }
}

fn cable_car(g: &mut World, t: f64) {
    if g.layout.panels < 3 {
        return;
    }
    let (ax, ay) = (g.panel(1) as f64 + 8.0, 38.0);
    let (bx, by) = (g.panel(2) as f64 + 50.0, 14.0);
    // Pylons and the wire.
    for (px, py) in [(ax, ay), (bx, by)] {
        g.p(0x3a3a4a, px - 1.0, py, 3, 1);
        g.p(0x2a2a3a, px, py, 1, 8);
    }
    line(g, 0x2a2a3a, ax, ay, bx, by);
    let u = ((t / SECS) * 0.9 + 0.05).clamp(0.0, 1.0);
    let (cx, cy) = (ax + (bx - ax) * u, ay + (by - ay) * u);
    let sway = (t * 2.0).sin() * 0.5;
    g.p(0x2a2a3a, cx, cy, 1, 3);
    g.p(0xc83a3a, cx - 3.0 + sway, cy + 3.0, 7, 5);
    g.p(0xe85a4a, cx - 3.0 + sway, cy + 3.0, 7, 1);
    g.p(0xd8ecff, cx - 2.0 + sway, cy + 4.0, 2, 2);
    g.p(0xd8ecff, cx + 1.0 + sway, cy + 4.0, 2, 2);
    g.p(0x8a2a2a, cx - 3.0 + sway, cy + 7.0, 7, 1);
}

/// The big mountain in front: snow near the top, rock, then alpine meadow
/// and pines lower down.
fn big_mountain(g: &mut World, t: f64) {
    let w = g.w();
    let lit = sunlight(t);
    let (sx, sy) = summit(g);
    for x in 0..w {
        let xf = x as f64;
        let top = front(g, xf);
        let d = front(g, xf + 2.0) - front(g, xf - 2.0);
        let lit_k = (d * 0.35).clamp(0.0, 1.0) * (0.4 + dawn(t) * 0.6);
        for y in js_round(top)..60 {
            let c = if (y as f64) > 38.0 + (xf * 0.13).sin() * 2.0 {
                // Meadow, dew catching the light on sunny slopes.
                let m = if on(x, y, 0.3) { 0x2e5040 } else { 0x24443a };
                if on(x, y, lit_k * 0.3) {
                    mix(m, lit, 0.15)
                } else {
                    m
                }
            } else {
                rock_px(x, y, top, lit_k, sy + 11.0, 0x3e3450, lit, 0x8a90c8)
            };
            put(g, c, x, y);
        }
        blend(g, lit, x, js_round(top), 0.25 + lit_k * 0.5);
    }
    // Pines on the lower slopes.
    for k in 0..g.spread(26) {
        let kf = k as f64;
        let x = rnd(kf + 80.0) * (sx - 40.0);
        let y = front(g, x) + 6.0 + rnd(kf + 81.0) * 8.0;
        if y < 58.0 {
            pine(g, x, y, 6 + (rnd(kf + 82.0) * 4.0) as i32, lit, dawn(t));
        }
    }
}

fn chalet(g: &mut World, t: f64) {
    let x = g.panel(0) as f64 + 8.0;
    let y = front(g, x + 7.0).max(front(g, x)).round() + 1.0;
    // Walls, roof heavy with snow, windows, a door.
    g.p(0x6a3a22, x, y - 8.0, 15, 8);
    for r in 0..4 {
        g.p(0x4a2a1a, x, y - 7.0 + r as f64 * 2.0, 15, 1);
    }
    for r in 0..5 {
        g.p(
            0x3a2018,
            x - 2.0 + r as f64,
            y - 9.0 - r as f64,
            19 - 2 * r,
            1,
        );
        g.p(
            0xe8ecf8,
            x - 2.0 + r as f64,
            y - 10.0 - r as f64,
            19 - 2 * r,
            1,
        );
    }
    let lamp = if t < 3.0 { 0xffc860 } else { 0xb8904a };
    g.p(lamp, x + 2.0, y - 6.0, 3, 2);
    g.p(lamp, x + 10.0, y - 6.0, 3, 2);
    glow(g, 0xffc060, x + 3.0, y - 5.0, 5.0, 0.4 * (1.0 - dawn(t)));
    g.p(0x2a1810, x + 6.0, y - 4.0, 3, 4);
    // Chimney smoke.
    g.p(0x3a2a2a, x + 11.0, y - 15.0, 2, 4);
    for k in 0..9 {
        let kf = k as f64;
        let rise = (t * 2.5 + kf * 1.4) % 12.0;
        let sx = x + 12.0 + rise * 0.9 + (rise * 0.8 + kf).sin() * 1.2;
        let sy = y - 16.0 - rise * 1.3;
        let a = 0.55 * (1.0 - rise / 12.0);
        let r = 1 + (rise / 5.0) as i32;
        for j in -r..=r {
            for i in -r..=r {
                if i * i + j * j <= r * r {
                    blend(g, 0xd8d8e8, js_round(sx) + i, js_round(sy) + j, a);
                }
            }
        }
    }
}

/// The climber's stage position along the ridge.
fn climb_s(g: &World, t: f64) -> f64 {
    let s0 = stage_at(g, 0, 22.0);
    let s1 = stage_at(g, last(g), summit(g).0 - g.panel(last(g)) as f64 - 10.0);
    let u = ((t - T_SET_OFF) / (T_SUMMIT - T_SET_OFF)).clamp(0.0, 1.0);
    // A slog: slower towards the top.
    s0 + (s1 - s0) * (u * 1.25 - u * u * 0.25)
}

fn climber(g: &mut World, t: f64, f: u32) {
    let x = stage_x(g, climb_s(g, t));
    let y = front(g, x + 3.0).min(front(g, x + 4.0));
    let at_top = t >= T_SUMMIT;
    let pose = if (T_FLAG + 0.2..T_CHEW + 0.4).contains(&t) {
        2
    } else if at_top || t < T_SET_OFF {
        0
    } else {
        ((f / 3) % 2) as usize
    };
    outlined(g, CLIMBER[pose], CLIMBP, x, y - 10.0, false);
    if !at_top && t > T_SET_OFF {
        // Puffs of breath in the cold.
        let u = (t * 1.5) % 1.0;
        if u < 0.6 {
            blend(
                g,
                0xf0f4ff,
                js_round(x + 6.0 + u * 3.0),
                js_round(y - 8.0 - u * 2.0),
                0.7,
            );
        }
        // Walking pole.
        let swing = if (f / 3).is_multiple_of(2) { 1.0 } else { -1.0 };
        line(g, 0x6a6a7a, x + 5.0, y - 5.0, x + 6.0 + swing, y);
    }
    if t > T_CHEW + 0.8 {
        // He has noticed.
        let (ex, ey) = (js_round(x) + 3, js_round(y) - 18);
        g.p(0x1a1420, ex as f64 - 1.0, ey as f64 - 1.0, 3, 7);
        g.p(0xffe040, ex as f64, ey as f64, 1, 3);
        put(g, 0xffe040, ex, ey + 4);
    }
}

/// Where the goat is: `(x, feet y, pose, facing left)`.
fn goat(g: &World, t: f64) -> (f64, f64, usize, bool) {
    let first = if g.layout.panels >= 4 { 1 } else { 0 };
    let perch = stage_at(g, first, 46.0);
    let (sx, sy) = summit(g);
    let top_s = stage_at(g, last(g), sx - g.panel(last(g)) as f64 + 8.0);
    if t < T_GOAT_GO {
        let x = stage_x(g, perch);
        return (x, front(g, x + 4.0) - 2.0, 0, true);
    }
    if t < T_GOAT_UP {
        // Three big bounds up the cliffs.
        let u = (t - T_GOAT_GO) / (T_GOAT_UP - T_GOAT_GO) * 3.0;
        let hop = u.floor().min(2.0);
        let k = u - hop;
        let s = perch + (top_s - perch) * ((hop + ease(k)) / 3.0);
        let x = stage_x(g, s);
        let ground = front(g, x + 4.0) - 2.0;
        return (x, ground - (k * PI).sin() * 12.0, 1, false);
    }
    // Waits just behind the summit, then wanders over to the flag.
    let wait = (sx + 8.0, front(g, sx + 12.0) - 2.0);
    if t < T_CHEW - 0.8 {
        return (wait.0, wait.1, 0, true);
    }
    let u = ((t - (T_CHEW - 0.8)) / 0.8).clamp(0.0, 1.0);
    let x = wait.0 + (sx + 3.0 - wait.0) * ease(u);
    let y = wait.1 + (sy - 1.0 - wait.1) * ease(u);
    (x, y, if t >= T_CHEW { 2 } else { 0 }, true)
}

fn goat_draw(g: &mut World, t: f64, f: u32) {
    let (x, y, pose, left) = goat(g, t);
    let pose = if pose == 2 && (f / 3).is_multiple_of(2) {
        0
    } else {
        pose
    };
    outlined(g, GOAT[pose], GOATP, x, y - 7.0, !left);
    if t > T_GOAT_GO - 1.2 && t < T_GOAT_GO {
        // Watching the climber puff past, chewing.
        if (f / 4).is_multiple_of(2) {
            put(g, 0x2a2430, js_round(x), js_round(y) - 4);
        }
    }
}

fn flag(g: &mut World, t: f64, f: u32) {
    if t < T_FLAG {
        return;
    }
    let (sx, sy) = summit(g);
    let (px, py) = (sx + 1.0, sy - 1.0);
    g.p(0x8a8a9a, px, py - 10.0, 1, 11);
    // The cloth, eaten from the free end once the goat starts.
    let eaten = ((t - T_CHEW) / 1.2).clamp(0.0, 1.0);
    let len = js_round(6.0 * (1.0 - eaten));
    for i in 0..len {
        let wave = ((t * 6.0 - i as f64 * 0.9).sin() * 0.8).round() as i32;
        let c = if (i / 2) % 2 == 0 { 0xe04a3a } else { 0xfff0e0 };
        g.p(c, px + 1.0 + i as f64, py - 10.0 + wave as f64, 1, 4);
    }
    // Sun rays burst from behind him at the summit.
    let k =
        ((t - T_FLAG) / 0.4).clamp(0.0, 1.0) * (1.0 - ((t - T_FLAG - 2.0) / 2.0).clamp(0.0, 1.0));
    if k > 0.0 {
        let (cx, cy) = sun(g, t);
        for r in 0..12 {
            let a = r as f64 / 12.0 * PI * 2.0 + t * 0.15;
            for d in 8..24 {
                if (d + r + (f as i32 / 2)) % 3 != 0 {
                    blend(
                        g,
                        0xfff4c8,
                        js_round(cx + a.cos() * d as f64),
                        js_round(cy + a.sin() * d as f64),
                        0.35 * k * (1.0 - d as f64 / 24.0),
                    );
                }
            }
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    mountains(g, t, 1.3, 36.0, 20.0, 0x5a5a8a, 0.45);
    mist(g, t, 31, 6, 0.8 - dawn(t) * 0.4);
    mountains(g, t, 2.9, 44.0, 14.0, 0x3a3c66, 0.2);
    cable_car(g, t);
    mist(g, t, 37, 8, 1.1 - dawn(t) * 0.5);
    forest(g, t);
    big_mountain(g, t);
    chalet(g, t);
    goat_draw(g, t, f);
    flag(g, t, f);
    climber(g, t, f);
    mist(g, t, 54, 6, 0.5);
}

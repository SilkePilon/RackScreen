//! Rooftop cat: a city night from the roofs. A big moon hangs over a
//! skyline of towers whose windows wink on and off, a radio mast blinks,
//! chimneys smoke and a neon fish flickers on a wall. A ginger tabby on a
//! chimney-top roof spots a moth, wiggles, pounces and misses, then gives
//! chase roof to roof, leaping the gaps between the buildings (which are
//! the gaps between the screens). One leap to a taller building falls
//! short: the cat hangs from the ledge by its front paws, hind legs
//! kicking, while the moth flutters in its face, then scrambles up and
//! runs on. On the last roof the moth heads for the moon; the cat makes
//! its big leap, silhouetted against the moon, lands on the water tower's
//! hatch, which swings open and drops it into the tank. Splash. A soggy
//! head pops out of the hatch, ears flat, and the moth settles on it.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};

pub const SECS: f64 = 12.0;

const T_CROUCH: f64 = 1.3;
const T_POUNCE: f64 = 1.9;
const T_RUN: f64 = 2.3;
/// The short leap ends hanging from the ledge.
const T_HANG: f64 = 4.7;
const T_CLIMB: f64 = 6.3;
const T_RUN2: f64 = 6.8;
const T_EDGE: f64 = 8.2;
const T_LEAP: f64 = 8.7;
const T_LAND: f64 = 9.4;
const T_SPLASH: f64 = 9.6;
const T_HEAD: f64 = 10.4;
const T_REST: f64 = 10.9;

const OUT: u32 = 0x1a0e18;
const CATP: &[(char, u32)] = &[
    ('k', OUT),
    ('o', 0xe8903a),
    ('d', 0xa8541e),
    ('l', 0xf8dcae),
    ('e', 0xb8f040),
    ('n', 0xf08aa0),
];

#[derive(Clone, Copy, PartialEq)]
enum Pose {
    Sit,
    Crouch,
    Run(usize),
    Leap,
    Hang(usize),
    Gone,
}

/// The cat facing right; the feet are on the bottom row.
const SIT: &[&str] = &[
    ".......k...k",
    ".......kk.kk",
    ".......kooook",
    ".......koeoek",
    "..k....kolnlk",
    ".k.k..kkkkkk.",
    ".k...kodooook",
    "..k.kodoooolk",
    "..kkooddooolk",
    "...koooooolk.",
    "...kkkkkkkkk.",
];
const CROUCH: &[&str] = &[
    "..........k.k.",
    "k........kokok",
    ".k.kkkkkkkooek",
    "..kodooddooolnk",
    "..koooooooolk.",
    "..kkkkkkkkkkk.",
];
const RUN: [&[&str]; 2] = [
    &[
        "...........k.k",
        "k.........kokok",
        ".kkkkkkkkkkooek",
        "..kodoodoooolnk",
        "..kooooooookk..",
        ".kkk......kk...",
        "kk.........kk..",
    ],
    &[
        "...........k.k",
        "..........kokok",
        "kkkkkkkkkkkooek",
        "..kodoodoooolnk",
        "...koooooooko..",
        "....kk..kkk....",
        "...k.k..k..k...",
    ],
];
const LEAP: &[&str] = &[
    "............k.k.",
    "kk.........kokok",
    "..kkkkkkkkkkooek",
    "...kodoodoooolnk",
    "..kkooooooookkkk",
    "kkk.kkkkkkkk....",
];
const HANG: [&[&str]; 2] = [
    &[
        "kl..lk.", "ko..ok.", "kokkok.", "kooook.", "kodook.", ".kook..", "kodook.", "koooook",
        "kodook.k", "kk..kk.k", "k....k.k", ".......k",
    ],
    &[
        "kl..lk.", "ko..ok.", "kokkok.", "kooook.", "kodook.", ".kook..", "kodook.", "koooook",
        "kodookk", ".kk.k.k", ".k..kk.", "......k",
    ],
];
/// The soggy head in the hatch, ears flat.
const DUNK: &[&str] = &[
    "kk.....kk",
    ".kkoookk.",
    ".koeoeok.",
    ".kolnlok.",
    "..kkkkk..",
];

fn smooth(a: f64, b: f64, t: f64) -> f64 {
    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}
/// The taller building the short leap falls short of.
fn tall(g: &World) -> u32 {
    last(g) - 1
}

/// Roof height of the building on panel `i`.
fn roof(g: &World, i: u32) -> f64 {
    if i == 0 {
        40.0
    } else if i == last(g) {
        41.0
    } else if i == tall(g) {
        31.0
    } else {
        37.0
    }
}

/// Left edge of the building on panel `i`, from the panel's edge.
fn left(g: &World, i: u32) -> f64 {
    if i == tall(g) {
        12.0
    } else {
        -(g.layout.gap as f64) / 2.0
    }
}

/// The roof under stage position `s` (the nearer building in a gap).
fn roof_at(g: &World, s: f64) -> f64 {
    let step = stage_at(g, 1, 0.0);
    let i = ((s + (step - 60.0) / 2.0) / step)
        .floor()
        .clamp(0.0, last(g) as f64) as u32;
    roof(g, i)
}

/// The water tower on the last roof: (centre x, tank top y).
fn tower(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 39.0, 20.0)
}

fn moon(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 22.0, 13.0)
}

/// The cat: (x, feet y, pose).
fn cat(g: &World, t: f64, f: u32) -> (f64, f64, Pose) {
    let s0 = stage_at(g, 0, 18.0);
    let run = |a: f64, b: f64, sa: f64, sb: f64| -> (f64, f64, Pose) {
        let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
        let s = sa + (sb - sa) * u;
        let x = stage_x(g, s);
        // Over a gap: leap from roof to roof.
        let step = stage_at(g, 1, 0.0);
        let k = (s / step).floor();
        let (g0, g1) = (k * step + 55.0, (k + 1.0) * step + 3.0);
        if (g0..g1).contains(&s) && k < last(g) as f64 {
            let v = (s - g0) / (g1 - g0);
            let (y0, y1) = (roof(g, k as u32), roof(g, k as u32 + 1));
            return (
                x,
                y0 + (y1 - y0) * v - 10.0 * 4.0 * v * (1.0 - v),
                Pose::Leap,
            );
        }
        (x, roof_at(g, s), Pose::Run((f / 2 % 2) as usize))
    };
    if t < T_CROUCH {
        return (stage_x(g, s0), roof(g, 0), Pose::Sit);
    }
    if t < T_POUNCE {
        // The bottom wiggle before the pounce.
        let wig = ((t * 14.0).sin() * 0.6).round();
        return (stage_x(g, s0) + wig, roof(g, 0), Pose::Crouch);
    }
    if t < T_RUN {
        let u = (t - T_POUNCE) / (T_RUN - T_POUNCE);
        let x = stage_x(g, s0) + 8.0 * u;
        return (x, roof(g, 0) - 14.0 * 4.0 * u * (1.0 - u) * 0.6, Pose::Leap);
    }
    let ledge = stage_at(g, tall(g), left(g, tall(g)) - 2.0);
    let take_off = stage_at(g, tall(g) - 1, 54.0);
    let t_jump = T_HANG - 0.45;
    if t < t_jump {
        return run(T_RUN, t_jump, s0 + 8.0, take_off);
    }
    if t < T_HANG {
        // Too short: up, and only the front paws make it.
        let u = (t - t_jump) / (T_HANG - t_jump);
        let s = take_off + (ledge - take_off) * u;
        let (y0, y1) = (roof(g, tall(g) - 1), roof(g, tall(g)) + 11.0);
        return (
            stage_x(g, s),
            y0 + (y1 - y0) * u - 8.0 * 4.0 * u * (1.0 - u),
            Pose::Leap,
        );
    }
    let lx = stage_x(g, ledge);
    if t < T_CLIMB {
        let kick = ((t * 6.0) as usize) % 2;
        let sway = (t * 3.0).sin() * 0.6;
        return (lx + sway, roof(g, tall(g)) + 11.0, Pose::Hang(kick));
    }
    if t < T_RUN2 {
        let u = ease((t - T_CLIMB) / (T_RUN2 - T_CLIMB));
        let y = roof(g, tall(g)) + 11.0 * (1.0 - u);
        return (
            lx + 2.0 + 4.0 * u,
            y,
            if u < 0.5 { Pose::Hang(0) } else { Pose::Crouch },
        );
    }
    let edge = stage_at(g, last(g), 8.0);
    if t < T_EDGE {
        return run(T_RUN2, T_EDGE, ledge + 6.0, edge);
    }
    let ex = stage_x(g, edge);
    if t < T_LEAP {
        return (ex, roof(g, last(g)), Pose::Crouch);
    }
    let (tx, ty) = tower(g);
    let top = ty - 7.0;
    if t < T_LAND {
        let u = (t - T_LEAP) / (T_LAND - T_LEAP);
        let x = ex + (tx - 7.0 - ex) * u;
        let y = roof(g, last(g)) + (top - roof(g, last(g))) * u - 22.0 * 4.0 * u * (1.0 - u) * 0.75;
        return (x, y, Pose::Leap);
    }
    if t < T_SPLASH {
        // Down the hatch.
        let u = (t - T_LAND) / (T_SPLASH - T_LAND);
        return (tx - 7.0, top + 9.0 * u * u, Pose::Crouch);
    }
    (tx, top, Pose::Gone)
}

/// The moth: (x, y).
fn moth(g: &World, t: f64, f: u32) -> (f64, f64) {
    let jx = (t * 5.3).sin() * 2.0 + (t * 11.0).sin();
    let jy = (t * 7.1).cos() * 2.0 + (t * 13.0).sin() * 0.8;
    let (cx, cy, _) = cat(g, t, f);
    let (mx, my) = moon(g);
    let (tx, ty) = tower(g);
    if t < T_POUNCE {
        return (g.panel(0) as f64 + 30.0 + jx * 2.0, 26.0 + jy);
    }
    if t < T_HANG + 0.1 || (T_RUN2..T_EDGE).contains(&t) {
        // Always just out of reach ahead.
        return (cx + 18.0 + jx, cy - 12.0 + jy);
    }
    if t < T_RUN2 {
        let lx = g.panel(tall(g)) as f64 + left(g, tall(g));
        return (lx + 9.0 + jx, roof(g, tall(g)) + 2.0 + jy);
    }
    if t < T_LAND {
        // Off to the moon, dodging up as the cat flies past.
        let u = smooth(T_EDGE, T_LEAP + 0.2, t);
        let dodge = smooth(T_LEAP + 0.3, T_LAND, t) * 6.0;
        return (
            cx + 18.0 + (mx + 4.0 - cx - 18.0) * u + jx,
            cy - 12.0 + (my - cy + 12.0) * u - dodge + jy,
        );
    }
    if t < T_REST {
        return (tx + 6.0 + jx * 1.5, ty - 14.0 + jy);
    }
    // Settled on the soggy head.
    let u = smooth(T_REST, T_REST + 0.4, t);
    let (hx, hy) = (tx, ty - 12.0);
    let (ax, ay) = (tx + 6.0 + jx * 1.5, ty - 14.0 + jy);
    (ax + (hx - ax) * u, ay + (hy - ay) * u)
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x0a0820, 0x141034, 0x231a48, 0x3a2456, 0x5a2e5a, 0x6a3460],
        0,
        w,
        0,
        60,
    );
    for i in 0..g.spread(50) {
        let fi = i as f64;
        let (x, y) = (rnd(fi) * w as f64, rnd(fi + 1.0) * 26.0);
        let tw = 0.5 + 0.5 * (t * (1.0 + rnd(fi + 2.0) * 3.0) + fi).sin();
        blend(g, 0xe8e4ff, x as i32, y as i32, 0.3 + 0.5 * tw);
    }
    let (mx, my) = moon(g);
    glow(g, 0xd8d0ff, mx, my, 28.0, 0.3);
    g.disc(0xf4f0dc, mx, my, 8);
    for (dx, dy, r) in [
        (-3.0, -2.0, 2),
        (2.0, 3.0, 1),
        (3.0, -4.0, 1),
        (-1.0, 4.0, 1),
    ] {
        g.disc(0xd8d2bc, mx + dx, my + dy, r);
    }
    for y in -8..=8 {
        for x in -8..=8 {
            let d = ((x * x + y * y) as f64).sqrt();
            if (6.5..8.6).contains(&d) && x + y > 2 {
                blend(g, 0xb8b0a0, js_round(mx) + x, js_round(my) + y, 0.4);
            }
        }
    }
}

/// Window `id` of a building: lit or not, changing now and then.
fn window_on(id: f64, t: f64) -> bool {
    let epoch = ((t + rnd(id) * 7.0) / (2.5 + rnd(id + 1.0) * 3.0)).floor();
    rnd(id * 3.7 + epoch * 17.0) > 0.55
}

/// Two layers of far skyline with their own little windows.
fn skyline(g: &mut World, t: f64, f: u32) {
    let w = g.w();
    for (layer, (col, lit, y0, y1)) in [
        (0x221c46, 0x8a6a7a, 18.0, 34.0),
        (0x17122e, 0xc89060, 24.0, 40.0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut x = -(rnd(layer as f64 * 3.0) * 10.0);
        let mut k = 0;
        while x < w as f64 {
            let id = (layer * 400 + k) as f64;
            let bw = 8.0 + rnd(id + 1.0) * 14.0;
            let top = y0 + rnd(id + 2.0) * (y1 - y0);
            g.p(col, x, top, bw as i32 + 1, (60.0 - top) as i32);
            // Roof furniture: a far water tower or a spire now and then.
            let pick = rnd(id + 3.0);
            if pick > 0.85 {
                g.p(col, x + bw / 2.0 - 2.0, top - 4.0, 4, 3);
                g.p(col, x + bw / 2.0 - 1.0, top - 5.0, 2, 1);
                put(g, col, js_round(x + bw / 2.0 - 2.0), js_round(top) - 1);
                put(g, col, js_round(x + bw / 2.0 + 1.0), js_round(top) - 1);
            } else if pick > 0.75 {
                line(g, col, x + bw / 2.0, top, x + bw / 2.0, top - 7.0);
                if (f / 6 + k as u32).is_multiple_of(3) {
                    glow(g, 0xff4040, x + bw / 2.0, top - 7.0, 3.0, 0.7);
                    put(g, 0xff6a5a, js_round(x + bw / 2.0), js_round(top) - 7);
                }
            }
            for wy in ((top + 2.0) as i32..58).step_by(3) {
                for wx in ((x + 2.0) as i32..(x + bw - 1.0) as i32).step_by(2 + layer) {
                    let wid = id * 100.0 + (wx * 31 + wy) as f64;
                    if window_on(wid, t) {
                        put(g, mix(lit, col, rnd(wid + 5.0) * 0.5), wx, wy);
                    }
                }
            }
            x += bw + rnd(id + 4.0) * 3.0;
            k += 1;
        }
    }
    // A haze of city glow low down.
    for y in 34..60 {
        let k = (y - 34) as f64 / 26.0 * 0.25;
        for x in 0..w {
            if on(x, y, k) {
                blend(g, 0x8a4a6a, x, y, 0.35);
            }
        }
    }
}

/// The near buildings, one per screen, with their roof furniture.
fn buildings(g: &mut World, t: f64, f: u32) {
    let n = g.layout.panels;
    let walls = [0x3a2430, 0x2a2a40, 0x3a2a26, 0x26303a];
    for i in 0..n {
        let p = g.px0(i) as f64;
        let (x0, x1) = (p + left(g, i), p + 60.0 + g.layout.gap as f64 / 2.0);
        let ry = roof(g, i);
        let wall = walls[i as usize % 4];
        for x in js_round(x0)..js_round(x1) {
            for y in js_round(ry)..60 {
                // Brick courses, dithered darker towards the street.
                let brick = (y % 3 == 0)
                    || ((x + if (y / 3) % 2 == 0 { 0 } else { 2 }) % 4 == 0 && y % 3 == 1);
                let k = (y as f64 - ry) / 40.0;
                let c = if brick { mix(wall, OUT, 0.35) } else { wall };
                put(g, if on(x, y, k) { mix(c, OUT, 0.3) } else { c }, x, y);
            }
            // The parapet and its moonlit coping.
            put(g, mix(wall, 0xb8b0d8, 0.45), x, js_round(ry));
            put(g, mix(wall, OUT, 0.5), x, js_round(ry) + 1);
        }
        // The ledge the cat grabs juts out a little.
        put(g, mix(wall, 0xb8b0d8, 0.45), js_round(x0) - 1, js_round(ry));
        // Windows: lit ones warm, some with a curtain, some flickering TV-blue.
        let mut k = 0;
        for wy in ((ry + 5.0) as i32..58).step_by(7) {
            for wx in ((x0 + 4.0) as i32..(x1 - 4.0) as i32).step_by(9) {
                let id = (i * 1000 + k) as f64;
                k += 1;
                g.p(OUT, wx as f64 - 1.0, wy as f64 - 1.0, 6, 6);
                let lit = window_on(id, t) || (i == last(g) && k == 2 && t > T_SPLASH + 0.5);
                let c = if !lit {
                    0x1a1a30
                } else if rnd(id + 9.0) > 0.8 {
                    mix(0x5a8ae8, 0x8ab8ff, rnd(id + (f / 2) as f64))
                } else {
                    0xf8c870
                };
                g.p(c, wx as f64, wy as f64, 4, 4);
                if lit {
                    glow(g, c, wx as f64 + 2.0, wy as f64 + 2.0, 5.0, 0.25);
                    g.p(mix(c, OUT, 0.4), wx as f64, wy as f64, 1, 4);
                }
                g.p(
                    mix(wall, 0x8a8aa0, 0.3),
                    wx as f64 - 1.0,
                    wy as f64 + 4.0,
                    6,
                    1,
                );
            }
        }
    }
    // The first roof: a chimney stack and a TV aerial.
    let p0 = g.panel(0) as f64;
    let r0 = roof(g, 0);
    chimney(g, p0 + 6.0, r0, t);
    aerial(g, p0 + 46.0, r0);
    // Middle roofs: a skylight and a vent fan.
    for i in 1..tall(g) {
        let p = g.panel(i) as f64;
        let r = roof(g, i);
        g.p(0x5a6a8a, p + 30.0, r - 3.0, 9, 3);
        g.p(0x8aa0c8, p + 31.0, r - 2.0, 7, 1);
        vent(g, p + 46.0, r, f);
        chimney(g, p + 14.0, r, t + 3.0);
    }
    // The tall building: a satellite dish and a pipe.
    let pt = g.panel(tall(g)) as f64;
    let rt = roof(g, tall(g));
    g.p(0x4a4a5a, pt + 44.0, rt - 6.0, 2, 6);
    g.p(0x6a6a7a, pt + 44.0, rt - 6.0, 4, 1);
    for (dx, dy) in [
        (0, -5),
        (1, -6),
        (2, -7),
        (0, -6),
        (1, -7),
        (2, -8),
        (3, -8),
    ] {
        put(g, 0x9a9aaa, js_round(pt) + 26 + dx, js_round(rt) + dy);
    }
    g.p(0x6a6a7a, pt + 27.0, rt - 4.0, 1, 4);
    // The last roof: a radio aerial with a blinking light.
    let pl = g.panel(last(g)) as f64;
    let rl = roof(g, last(g));
    line(g, 0x4a4a5a, pl + 54.0, rl, pl + 54.0, rl - 14.0);
    if f % 18 < 6 {
        glow(g, 0xff3030, pl + 54.0, rl - 14.0, 4.0, 0.8);
        put(g, 0xff8070, js_round(pl + 54.0), js_round(rl - 14.0));
    }
}

fn chimney(g: &mut World, x: f64, ry: f64, t: f64) {
    g.p(0x5a3028, x, ry - 7.0, 6, 7);
    g.p(0x7a4434, x, ry - 7.0, 6, 1);
    g.p(0x3a2020, x + 4.0, ry - 6.0, 2, 6);
    g.p(0x2a1818, x + 1.0, ry - 9.0, 2, 2);
    g.p(0x2a1818, x + 3.0, ry - 9.0, 2, 2);
    for k in 0..5 {
        let u = (t * 0.35 + k as f64 / 5.0) % 1.0;
        let sx = x + 2.0 + u * 7.0 + (u * 7.0 + k as f64).sin();
        let sy = ry - 10.0 - u * 14.0;
        blend(g, 0x8a7a9a, js_round(sx), js_round(sy), 0.45 * (1.0 - u));
        blend(g, 0x8a7a9a, js_round(sx) + 1, js_round(sy), 0.3 * (1.0 - u));
    }
}

fn aerial(g: &mut World, x: f64, ry: f64) {
    line(g, 0x5a5a6a, x, ry, x, ry - 13.0);
    for (dy, w) in [(-12.0, 4.0), (-9.0, 6.0), (-6.0, 8.0)] {
        line(g, 0x5a5a6a, x - w / 2.0, ry + dy, x + w / 2.0, ry + dy);
    }
}

fn vent(g: &mut World, x: f64, ry: f64, f: u32) {
    g.p(0x4a4a5a, x, ry - 6.0, 7, 6);
    g.p(0x6a6a7a, x, ry - 6.0, 7, 1);
    let blades = [[(-1, 0), (1, 0)], [(0, -1), (0, 1)]];
    for (dx, dy) in blades[(f / 2 % 2) as usize] {
        put(g, 0x9a9aaa, js_round(x) + 3 + dx, js_round(ry) - 3 + dy);
    }
    put(g, 0x2a2a3a, js_round(x) + 3, js_round(ry) - 3);
}

/// A neon fish on the wall below the second roof, buzzing on and off.
fn neon(g: &mut World, t: f64, f: u32) {
    let p = g.panel(1) as f64;
    let (x, y) = (p + 34.0, roof(g, 1) + 9.0);
    g.p(0x120a14, x - 2.0, y - 2.0, 20, 11);
    let flick = rnd((f / 2) as f64 * 1.3) < 0.18 || (t % 3.7) < 0.15;
    let body = 0x5af0ff;
    let tail = if flick { 0x2a3a4a } else { 0xff5ab4 };
    glow(g, body, x + 7.0, y + 3.0, 14.0, 0.25);
    let pts = [
        (0.0, 3.0),
        (4.0, 0.0),
        (9.0, 0.0),
        (12.0, 3.0),
        (9.0, 6.0),
        (4.0, 6.0),
        (0.0, 3.0),
    ];
    for w in pts.windows(2) {
        line(g, body, x + w[0].0, y + w[0].1, x + w[1].0, y + w[1].1);
    }
    for (a, b) in [
        ((12.0, 3.0), (16.0, 0.0)),
        ((16.0, 0.0), (16.0, 6.0)),
        ((16.0, 6.0), (12.0, 3.0)),
    ] {
        line(g, tail, x + a.0, y + a.1, x + b.0, y + b.1);
    }
    if !flick {
        glow(g, 0xff5ab4, x + 15.0, y + 3.0, 6.0, 0.35);
    }
    put(g, 0xffffff, js_round(x) + 3, js_round(y) + 2);
}

/// The water tower: legs, a wooden tank with hoops, a conical roof and the
/// hatch that drops the cat in.
fn water_tower(g: &mut World, t: f64) {
    let (cx, ty) = tower(g);
    let rl = roof(g, last(g));
    let tank_h = 11.0;
    for dx in [-6.0, -2.0, 2.0, 5.0] {
        line(g, 0x2a2026, cx + dx, ty + tank_h, cx + dx * 1.2, rl);
    }
    line(g, 0x2a2026, cx - 6.0, ty + tank_h + 4.0, cx + 6.0, rl - 1.0);
    line(g, 0x2a2026, cx + 6.0, ty + tank_h + 4.0, cx - 6.0, rl - 1.0);
    for y in 0..tank_h as i32 {
        for x in -7..=7 {
            let c = if x % 3 == 0 { 0x5a3824 } else { 0x7a4c30 };
            let c = if x > 3 {
                mix(c, OUT, 0.35)
            } else if x < -4 {
                mix(c, 0xd8c8e0, 0.2)
            } else {
                c
            };
            put(g, c, js_round(cx) + x, js_round(ty) + y);
        }
    }
    for hy in [2, 6, 9] {
        g.p(0x3a3a4a, cx - 7.0, ty + hy as f64, 15, 1);
    }
    // Water spilling from the seams after the splash.
    let spill =
        smooth(T_SPLASH, T_SPLASH + 0.2, t) * (1.0 - smooth(T_SPLASH + 1.2, T_SPLASH + 2.2, t));
    if spill > 0.0 {
        for (sx, side) in [(-7.0, -1.0), (7.0, 1.0)] {
            for k in 0..8 {
                let u = k as f64 / 8.0;
                let x = cx + sx + side * u * 4.0;
                let y = ty + 3.0 + u * u * 12.0 + (t * 20.0 + k as f64).sin() * 0.3;
                blend(g, 0x8ac8ff, js_round(x), js_round(y), 0.8 * spill);
            }
        }
    }
    // The roof: a cone with the hatch near the top.
    for r in 0..7 {
        let hw = 8 - r;
        for x in -hw..=hw {
            let c = if x > hw / 2 {
                0x2a2a36
            } else if x < -hw / 2 {
                0x6a6a7a
            } else {
                0x4a4a5a
            };
            put(g, c, js_round(cx) + x, js_round(ty) - 1 - r);
        }
    }
    put(g, 0x8a8a9a, js_round(cx), js_round(ty) - 8);
    let open = t >= T_LAND + 0.1;
    if open {
        g.p(OUT, cx - 2.0, ty - 5.0, 4, 2);
        // The hatch lid swung back.
        line(g, 0x6a6a7a, cx + 2.0, ty - 5.0, cx + 4.0, ty - 9.0);
    } else {
        g.p(0x5a5a6a, cx - 2.0, ty - 5.0, 4, 1);
    }
}

fn splash(g: &mut World, t: f64) {
    let u = (t - T_SPLASH) / 0.9;
    if !(0.0..1.0).contains(&u) {
        return;
    }
    let (cx, ty) = tower(g);
    for k in 0..22 {
        let kf = k as f64;
        let a = -std::f64::consts::PI / 2.0 + (rnd(kf + 40.0) - 0.5) * 2.2;
        let sp = 10.0 + rnd(kf + 41.0) * 12.0;
        let x = cx + a.cos() * sp * u;
        let y = ty - 5.0 + a.sin() * sp * u + 20.0 * u * u;
        let c = if k % 3 == 0 { 0xe8f8ff } else { 0x8ac8ff };
        blend(g, c, js_round(x), js_round(y), 1.0 - u * 0.7);
    }
    if u < 0.3 {
        glow(g, 0xc8e8ff, cx, ty - 6.0, 6.0, 0.6 * (1.0 - u / 0.3));
    }
}

fn draw_cat(g: &mut World, t: f64, f: u32) {
    let (x, y, pose) = cat(g, t, f);
    let (rows, dx): (&[&str], f64) = match pose {
        Pose::Sit => (SIT, -6.0),
        Pose::Crouch => (CROUCH, -7.0),
        Pose::Run(i) => (RUN[i], -7.0),
        Pose::Leap => (LEAP, -8.0),
        // Hanging: the paws are at the top, on the ledge.
        Pose::Hang(i) => (HANG[i], -2.0),
        Pose::Gone => (&[][..], 0.0),
    };
    if !rows.is_empty() {
        let top = if matches!(pose, Pose::Hang(_)) {
            y - 11.0
        } else {
            y - rows.len() as f64
        };
        g.spr(rows, CATP, x + dx, top, false);
        // Moonlight along its back.
        if matches!(pose, Pose::Run(_) | Pose::Leap) {
            for i in 2..9 {
                blend(g, 0xc8c0f0, js_round(x + dx) + i, js_round(top) + 2, 0.25);
            }
        }
    }
    if (T_HANG..T_CLIMB).contains(&t) {
        // Bits of the crumbling ledge, and a bead of sweat.
        let u = (t - T_HANG) / 0.8;
        for k in 0..4 {
            let kf = k as f64;
            let dy = u * u * 40.0 * (0.7 + rnd(kf + 80.0) * 0.5);
            if dy < 30.0 {
                put(
                    g,
                    0x6a4a4a,
                    js_round(x + kf * 1.5 - 1.0),
                    js_round(y - 11.0 + dy),
                );
            }
        }
        let bead = (t * 1.5) % 1.0;
        blend(
            g,
            0x8ac8ff,
            js_round(x + 6.0),
            js_round(y - 9.0 + bead * 2.0),
            1.0 - bead,
        );
    }
    if t >= T_HEAD {
        let (cx, ty) = tower(g);
        let rise = smooth(T_HEAD, T_HEAD + 0.3, t);
        let hy = ty - 9.0 + 3.0 * (1.0 - rise);
        g.spr(DUNK, CATP, cx - 4.0, hy, false);
        // Redraw the hatch rim in front, then the drips.
        g.p(0x4a4a5a, cx - 3.0, ty - 4.0, 6, 1);
        for k in 0..3 {
            let u = (t * 1.4 + k as f64 / 3.0) % 1.0;
            let dxk = [-3.0, 0.0, 3.0][k];
            blend(
                g,
                0x8ac8ff,
                js_round(cx + dxk),
                js_round(hy + 5.0 + u * 8.0),
                0.9 * (1.0 - u),
            );
        }
        // A puff of annoyance.
        if (T_HEAD + 0.3..T_REST).contains(&t) {
            g.p(0xffe040, cx + 6.0, hy - 5.0, 1, 3);
            put(g, 0xffe040, js_round(cx) + 6, js_round(hy) - 1);
        }
    }
}

fn draw_moth(g: &mut World, t: f64, f: u32) {
    let (x, y) = moth(g, t, f);
    let settled = t > T_REST + 0.4;
    glow(g, 0xfff0c0, x, y, 3.0, 0.35);
    let up = !settled && f.is_multiple_of(2);
    let (xi, yi) = (js_round(x), js_round(y));
    put(g, 0x8a7a6a, xi, yi);
    put(g, 0x8a7a6a, xi, yi + 1);
    let wing = 0xf0e8d0;
    if up || (settled && f % 24 < 2) {
        put(g, wing, xi - 1, yi - 1);
        put(g, wing, xi + 1, yi - 1);
    } else {
        put(g, wing, xi - 1, yi);
        put(g, wing, xi + 1, yi);
        put(g, wing, xi - 2, yi + 1);
        put(g, wing, xi + 2, yi + 1);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    skyline(g, t, f);
    buildings(g, t, f);
    neon(g, t, f);
    water_tower(g, t);
    draw_cat(g, t, f);
    splash(g, t);
    draw_moth(g, t, f);
}

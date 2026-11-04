//! Close encounter: a quiet moonlit farm, a saucer with blinking lights
//! drifts in over the barn, beams a wiggling cow up and zips off; a moment
//! later the cow floats back down under a striped parachute, now wearing a
//! little antenna.

use super::scifi::{bayer, glow, mix, tint};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_ARRIVE: f64 = 0.8;
const T_HOVER: f64 = 3.6;
const T_BEAM: f64 = 3.9;
const T_LIFT: f64 = 4.2;
const T_IN: f64 = 6.6;
const T_ZIP: f64 = 7.5;
const T_GONE: f64 = 8.1;
const T_CHUTE: f64 = 8.4;
const T_LAND: f64 = 10.6;

const SAUCER: &[&str] = &[
    "........ggggg........",
    "......gGGgggggg......",
    "......gGgaggagg......",
    "....mmmmmmmmmmmmm....",
    "..mmMMMMMMMMMMMMMmm..",
    "mmmmmmmmmmmmmmmmmmmmm",
    ".ddddddddddddddddddd.",
    "....ddddDDDDDdddd....",
];
const SAUCERP: &[(char, u32)] = &[
    ('g', 0x6ab8c8),
    ('G', 0xd8f4ff),
    ('a', 0x7aff6a),
    ('m', 0x9aa4b8),
    ('M', 0xd0d8e8),
    ('d', 0x4a5266),
    ('D', 0x8affc8),
];

/// A cow facing left: body only, the legs are drawn on their own.
const COW: [&[&str]; 2] = [
    &[
        "h.h.........",
        "wwwbbwwwwbwt",
        "wewbbbwwwwwt",
        "wwwwwwwbbww.",
        "ppwwwwwbbww.",
        "pp.wwwwwwww.",
    ],
    // Grazing: the head down in the grass.
    &[
        "...bbwwwwbwt",
        "h.hbbbwwwwwt",
        "wwwwwwwbbww.",
        "wewwwwwbbww.",
        "wwwwwwwwwww.",
        "pp..........",
    ],
];
const COWP: &[(char, u32)] = &[
    ('w', 0xd8dce8),
    ('b', 0x1a1a24),
    ('e', 0x101018),
    ('p', 0xd890a0),
    ('h', 0xe8e0c0),
    ('t', 0x4a4a58),
];
const LEG: u32 = 0x4a4a58;

const CHUTE: &[&str] = &["...rwrwr...", ".rrwwrrwwr.", "rrrwwrrwwrr", "k.k..k..k.k"];
const CHUTEP: &[(char, u32)] = &[('r', 0xe8484a), ('w', 0xf0ece0), ('k', 0x2a2a34)];

/// Where the saucer's top-left corner is at `t` (`None` once it is gone).
fn saucer(g: &World, t: f64) -> Option<(f64, f64)> {
    let hover = ((g.panel(2) + 20) as f64, 10.0);
    if t < T_ARRIVE || t >= T_GONE {
        return None;
    }
    let bob = (t * 3.0).sin() * 1.2;
    if t < T_HOVER {
        let u = ease((t - T_ARRIVE) / (T_HOVER - T_ARRIVE));
        let x0 = g.px0(0) as f64 - 24.0;
        let y = 16.0 - 6.0 * u + (u * PI * 3.0).sin() * 3.0;
        Some((x0 + (hover.0 - x0) * u, y + bob))
    } else if t < T_ZIP {
        Some((hover.0, hover.1 + bob))
    } else {
        let u = (t - T_ZIP) / (T_GONE - T_ZIP);
        let u = u * u;
        Some((hover.0 + u * 160.0, hover.1 + bob + (u * PI).sin() * 3.0))
    }
}

fn sky(g: &mut World, f: u32) {
    let w = g.w();
    for y in 0..44 {
        for x in 0..w {
            let u = y as f64 / 40.0 * 4.0;
            let k = (u + bayer(x, y) - 0.5).floor().clamp(0.0, 4.0);
            g.p(mix(0x070a20, 0x2a3462, k / 4.0), x as f64, y as f64, 1, 1);
        }
    }
    let wf = w as f64;
    for i in 0..g.spread(60) {
        let fi = i as f64;
        let (x, y) = (
            (rnd(fi * 1.3) * wf).floor(),
            (rnd(fi * 2.1 + 4.0) * 30.0).floor(),
        );
        let c = if (f + i * 5) % 41 < 2 {
            0xffffff
        } else if i % 3 == 0 {
            0x8a94c8
        } else {
            0x4a5488
        };
        g.p(c, x, y, 1, 1);
    }
}

fn moon(g: &mut World) {
    let (mx, my) = ((g.panel(1) + 26) as f64, 11.0);
    glow(g, mx, my, 16.0, 0x8a9ad0, 0.35);
    g.disc(0xf0ead8, mx, my, 6);
    g.p(0xd0c8b0, mx - 3.0, my - 2.0, 2, 2);
    g.p(0xd0c8b0, mx + 1.0, my + 2.0, 3, 2);
    g.p(0xd0c8b0, mx + 2.0, my - 3.0, 1, 1);
}

/// A thin cloud bank drifting across the moon.
fn clouds(g: &mut World, t: f64) {
    let w = g.w();
    for (k, (y, len, sp)) in [(17, 34, 1.4), (8, 26, 0.9), (22, 40, 1.8)]
        .into_iter()
        .enumerate()
    {
        let kf = k as f64;
        let x0 = (rnd(kf + 20.0) * w as f64 + t * sp) % (w as f64 + 60.0) - 40.0;
        for i in 0..len {
            let th = (i as f64 / len as f64 * PI).sin();
            let hh = js_round(th * 2.5);
            for j in 0..hh {
                if bayer(js_round(x0) + i, y + j) < 0.5 {
                    tint(g, js_round(x0) + i, y - j, 0x5a6698, 0.6);
                }
            }
            if bayer(js_round(x0) + i, y + 1) < 0.25 {
                tint(g, js_round(x0) + i, y + 1, 0x3a4470, 0.5);
            }
        }
    }
}

fn hills(g: &mut World) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let h = 6.0 + 3.0 * (xf * 0.035 + 0.6).sin() + 1.5 * (xf * 0.11).sin();
        let top = js_round(38.0 - h);
        g.p(0x1a2444, xf, top as f64, 1, 60 - top);
        g.p(0x3a4a78, xf, top as f64, 1, 1);
    }
    for x in 0..w {
        let xf = x as f64;
        let h = 3.0 + 2.0 * (xf * 0.05 + 2.0).sin() + (xf * 0.17).sin();
        let top = js_round(42.0 - h);
        g.p(0x142034, xf, top as f64, 1, 60 - top);
        if (xf * 0.05 + 2.0).cos() > 0.0 {
            g.p(0x2a3a5a, xf, top as f64, 1, 1);
        }
    }
    // The field: dithered furrows down to the bottom.
    for y in 42..60 {
        for x in 0..w {
            let furrow = (y * 3 + x / 9) % 4 == 0;
            let c = if furrow {
                0x182c22
            } else if bayer(x, y) < 0.3 {
                0x10221a
            } else {
                0x142a1e
            };
            g.p(c, x as f64, y as f64, 1, 1);
        }
    }
}

fn farmhouse(g: &mut World, f: u32, t: f64) {
    let x = (g.px0(0) + 12) as f64;
    // Chimney smoke curling up.
    for k in 0..6 {
        let age = ((f + k * 6) % 36) as f64;
        let sx = x + 18.0 + age * 0.3 + (age * 0.3).sin();
        let sy = 22.0 - age * 0.4;
        let c = mix(0x8a8aa0, 0x2a3060, age / 36.0);
        g.p(c, sx, sy, 2, 1);
    }
    g.p(0x3a2a2a, x + 18.0, 23.0, 3, 5);
    // Roof, walls, door, porch light.
    for r in 0..9 {
        g.p(
            0x2a2234,
            x - 1.0 + r as f64,
            25.0 + r as f64 - 0.0,
            24 - 2 * r,
            1,
        );
    }
    for r in 0..9 {
        let rr = 8 - r;
        g.p(0x3a3046, x - 1.0 + rr as f64, (33 - rr) as f64, 1, 1);
    }
    g.p(0x6a6a84, x, 33.0, 22, 12);
    for yy in (34..45).step_by(2) {
        g.p(0x5a5a74, x, yy as f64, 22, 1);
    }
    g.p(0x2a2234, x - 1.0, 32.0, 24, 1);
    // Windows: warm light, one flickering blue with a TV; all of them
    // stutter while the saucer passes overhead.
    let stutter = (1.0..2.6).contains(&t) && f.is_multiple_of(3);
    for (i, wx) in [2.0, 8.0, 16.0].into_iter().enumerate() {
        let mut c = if i == 1 {
            if (f / 2).is_multiple_of(5) {
                0x8ab4ff
            } else {
                0x6a8ad8
            }
        } else {
            0xffd27a
        };
        if stutter {
            c = 0x2a2a3a;
        }
        g.p(c, x + wx, 36.0, 4, 4);
        g.p(0x3a2a2a, x + wx + 1.5, 36.0, 1, 4);
        g.p(0x3a2a2a, x + wx, 37.5, 4, 1);
        if !stutter {
            for k in 0..4 {
                tint(g, js_round(x + wx) + k, 46, c, 0.25);
            }
        }
    }
    g.p(0x3a2a2a, x + 12.0, 39.0, 3, 6);
    g.p(0xffe8a0, x + 12.0, 37.0, 1, 1);
    glow(g, x + 12.0, 37.0, 4.0, 0xffd27a, 0.35);
    // A tree beside the house.
    let tx = x + 32.0;
    g.p(0x1a1410, tx, 32.0, 2, 13);
    for (dx, dy, r) in [(1.0, 28.0, 6), (-3.0, 31.0, 4), (5.0, 31.0, 4)] {
        g.disc(0x12241c, tx + dx, dy, r);
    }
    for k in 0..10 {
        let a = rnd(k as f64 + 3.0) * PI;
        g.p(
            0x24402e,
            tx + 1.0 + (a.cos() * 5.0).round(),
            28.0 - (a.sin() * 5.0).round(),
            1,
            1,
        );
    }
}

fn barn(g: &mut World, f: u32) {
    let x = (g.panel(1) + 10) as f64;
    // Silo first, behind the barn's side.
    let sx = x + 30.0;
    g.p(0x5a6274, sx, 22.0, 9, 24);
    g.p(0x7a8294, sx + 1.0, 22.0, 3, 24);
    for yy in (24..46).step_by(4) {
        g.p(0x464c5c, sx, yy as f64, 9, 1);
    }
    g.disc(0x8a90a0, sx + 4.0, 22.0, 4);
    g.p(0x5a6274, sx + 5.0, 19.0, 4, 3);
    g.p(0x5a6274, sx, 22.0, 9, 1);
    // Gambrel roof and walls.
    let roof: [(f64, i32); 8] = [
        (8.0, 12),
        (6.0, 16),
        (4.0, 20),
        (2.0, 24),
        (1.0, 26),
        (0.0, 28),
        (-1.0, 30),
        (-2.0, 32),
    ];
    for (r, (dx, wid)) in roof.into_iter().enumerate() {
        g.p(0x3a1a1e, x + dx, 22.0 + r as f64, wid, 1);
    }
    g.p(0x5e2422, x, 30.0, 28, 16);
    for xx in (1..28).step_by(3) {
        g.p(0x4a1c1c, x + xx as f64, 30.0, 1, 16);
    }
    // Door with the white X, and the lit hay loft.
    let (dx, dy) = (x + 8.0, 35.0);
    g.p(0x3a1414, dx, dy, 12, 11);
    g.p(0xc8c0b0, dx, dy, 12, 1);
    g.p(0xc8c0b0, dx, dy, 1, 11);
    g.p(0xc8c0b0, dx + 11.0, dy, 1, 11);
    g.p(0xc8c0b0, dx + 6.0, dy, 1, 11);
    for k in 0..10 {
        g.p(
            0xa8a090,
            dx + 1.0 + k as f64 * 0.5,
            dy + 1.0 + k as f64,
            1,
            1,
        );
        g.p(
            0xa8a090,
            dx + 10.0 - k as f64 * 0.5,
            dy + 1.0 + k as f64,
            1,
            1,
        );
    }
    let loft = if (f / 7).is_multiple_of(9) {
        0xd8a050
    } else {
        0xffc860
    };
    g.p(0x2a1010, x + 11.0, 25.0, 6, 5);
    g.p(loft, x + 12.0, 26.0, 4, 4);
    g.p(0xd89040, x + 12.0, 29.0, 4, 1);
    glow(g, x + 14.0, 28.0, 6.0, 0xffc860, 0.25);
    // Weathervane: a little rooster on a rod.
    g.p(0x2a2a34, x + 13.0, 17.0, 1, 5);
    g.p(0x2a2a34, x + 11.0, 17.0, 5, 1);
    g.p(0x2a2a34, x + 12.0, 15.0, 3, 2);
    g.p(0x2a2a34, x + 14.0, 14.0, 1, 1);
}

fn windmill(g: &mut World, t: f64) {
    let (cx, top) = ((g.panel(3) + 38) as f64, 18.0);
    // Lattice tower.
    for k in 0..28 {
        let u = k as f64 / 28.0;
        let half = 1.0 + u * 5.0;
        g.p(0x4a5470, cx - half, top + k as f64, 1, 1);
        g.p(0x6a7490, cx + half, top + k as f64, 1, 1);
        if k % 5 == 0 {
            g.p(
                0x4a5470,
                cx - half,
                top + k as f64,
                (half * 2.0) as i32 + 1,
                1,
            );
        }
        if k % 5 == 2 {
            g.p(0x3a4460, cx - half * 0.5, top + k as f64, 1, 1);
            g.p(0x3a4460, cx + half * 0.5, top + k as f64, 1, 1);
        }
    }
    // Tail vane and blades: they spin up in the gust when the saucer zips by.
    g.p(0x3a4058, cx, top - 1.0, 9, 1);
    g.p(0x4a5470, cx + 7.0, top - 4.0, 3, 5);
    // Integrate the spin by hand so the blades never jump.
    let base = t * 0.9;
    let extra = if t > T_ZIP {
        let s = t - T_ZIP;
        2.5 * (s.min(0.4) + (1.0 - (-(s - 0.4).max(0.0) * 0.6).exp()) / 0.6) * 4.0
    } else {
        0.0
    };
    let rot = base + extra;
    for b in 0..12 {
        let a = rot + b as f64 * PI / 6.0;
        for r in 2..8 {
            let rf = r as f64;
            let c = if r > 5 { 0xb8c0d8 } else { 0x7a84a0 };
            g.p(c, cx + a.cos() * rf, top + a.sin() * rf, 1, 1);
        }
    }
    g.disc(0x5a6480, cx, top, 1);
}

/// Round hay bales and a scarecrow who turns to watch the saucer.
fn props(g: &mut World, t: f64, sau: Option<(f64, f64)>) {
    let x = (g.panel(2) + 4) as f64;
    for (dx, dy) in [(0.0, 44.0), (8.0, 46.0)] {
        g.disc(0x8a7438, x + dx + 3.0, dy + 3.0, 3);
        g.p(0x6a5428, x + dx + 1.0, dy + 3.0, 5, 1);
        g.p(0xb89a50, x + dx + 2.0, dy + 1.0, 2, 1);
        g.p(0x4a3a20, x + dx + 3.0, dy + 3.0, 1, 1);
    }
    let sx = (g.panel(2) + 50) as f64;
    g.p(0x4a3a28, sx, 33.0, 1, 15);
    g.p(0x4a3a28, sx - 4.0, 36.0, 9, 1);
    g.p(0x5a4a7a, sx - 2.0, 35.0, 5, 6);
    g.p(0xc8a040, sx - 5.0, 36.0, 1, 2);
    g.p(0xc8a040, sx + 5.0, 36.0, 1, 2);
    g.p(0xd8b860, sx - 1.0, 31.0, 3, 3);
    g.p(0x2a2020, sx - 2.0, 30.0, 5, 1);
    g.p(0x2a2020, sx - 1.0, 28.0, 3, 2);
    // Its eyes follow the saucer.
    let look: f64 = match sau {
        Some((ux, _)) if ux + 10.0 < sx - 6.0 => -1.0,
        Some(_) => 1.0,
        None if t > T_GONE => 1.0,
        None => 0.0,
    };
    g.p(0x2a2020, sx + look.min(0.0), 32.0, 1, 1);
    g.p(0x2a2020, sx + 1.0 + look.max(0.0), 32.0, 1, 1);
}

/// The fence in the middle distance: posts and two sagging rails.
fn fence(g: &mut World) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let sag = ((x % 12) as f64 / 12.0 * PI).sin();
        g.p(0x4a3a30, xf, js_round(41.0 + sag) as f64, 1, 1);
        g.p(0x3a2c26, xf, js_round(45.0 + sag) as f64, 1, 1);
    }
    let mut x = 0;
    while x < w {
        g.p(0x5a4636, x as f64, 39.0, 2, 9);
        g.p(0x7a6450, x as f64, 39.0, 1, 1);
        x += 12;
    }
}

fn fireflies(g: &mut World, f: u32, t: f64) {
    let wf = g.w() as f64;
    for i in 0..g.spread(14) {
        let fi = i as f64;
        if (f + i * 7) % 24 > 15 {
            continue;
        }
        let x = rnd(fi * 4.1 + 2.0) * wf + (t * 0.8 + fi).sin() * 4.0;
        let y = 44.0 + rnd(fi * 1.9) * 10.0 + (t * 1.3 + fi).cos() * 2.0;
        g.p(0xd8ff7a, x, y, 1, 1);
        tint(g, js_round(x) + 1, js_round(y), 0xd8ff7a, 0.3);
    }
}

/// Corn rows along the bottom of the last two screens.
fn corn(g: &mut World, f: u32) {
    let x0 = g.panel(2) + 40;
    let x1 = g.w();
    let mut x = x0;
    while x < x1 {
        let sway = js_round(((f as f64) * 0.25 + x as f64 * 0.3).sin() * 0.6) as f64;
        let h = 6 + (rnd(x as f64) * 4.0) as i32;
        let xf = x as f64;
        g.p(0x1e3a22, xf, (58 - h) as f64, 1, h + 2);
        g.p(0x2e5230, xf + sway, (58 - h) as f64, 1, 2);
        g.p(0x2e5230, xf - 1.0, (60 - h) as f64, 1, 1);
        g.p(0x2e5230, xf + 1.0, (62 - h) as f64, 1, 1);
        if x % 3 == 0 {
            g.p(0xb8a040, xf + 1.0, (59 - h) as f64, 1, 2);
        }
        x += 3;
    }
}

/// A cow at `(x, y)` (top-left of the body), legs dangling or standing.
fn cow(g: &mut World, x: f64, y: f64, head_down: bool, kick: Option<u32>, flip: bool) {
    let sprite = COW[head_down as usize];
    g.spr(sprite, COWP, x, y, flip);
    let legs = [3.0, 5.0, 8.0, 10.0];
    for (i, lx) in legs.into_iter().enumerate() {
        let lx = if flip { 11.0 - lx } else { lx };
        let (dx, len) = match kick {
            Some(k) => {
                let ph = (k + i as u32) % 2;
                (if ph == 0 { -1.0 } else { 1.0 }, 2 + ph as i32)
            }
            None => (0.0, 3),
        };
        g.p(LEG, x + lx + dx * 0.5, y + 6.0, 1, len);
        if kick.is_none() {
            g.p(0x1a1a24, x + lx, y + 8.0, 1, 1);
        }
    }
    // An udder, pink.
    g.p(0xd890a0, x + if flip { 4.0 } else { 7.0 }, y + 6.0, 1, 1);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, f);
    moon(g);
    clouds(g, t);
    hills(g);
    farmhouse(g, f, t);
    barn(g, f);
    windmill(g, t);
    fence(g);
    fireflies(g, f, t);

    let home = ((g.panel(2) + 24) as f64, 42.0);
    let sau = saucer(g, t);
    props(g, t, sau);
    // The beam: a widening cone of pale green, rings sliding down it.
    let beam_on = (T_BEAM..T_IN + 0.4).contains(&t) && !(t < T_BEAM + 0.2 && f.is_multiple_of(2));
    if let (true, Some((sx, sy))) = (beam_on, sau) {
        let cx = sx + 10.0;
        let top = sy + 8.0;
        let bottom = if t > T_IN {
            51.0 - (t - T_IN) / 0.4 * (51.0 - top)
        } else {
            51.0
        };
        for y in js_round(top)..js_round(bottom) {
            let u = (y as f64 - top) / (51.0 - top);
            let half = 3.0 + u * 10.0;
            let ring = ((y as f64 - t * 16.0).rem_euclid(6.0)) < 1.0;
            for x in js_round(cx - half)..=js_round(cx + half) {
                let edge = (x as f64 - cx).abs() > half - 1.0;
                let a = if edge {
                    0.55
                } else if ring {
                    0.45
                } else {
                    0.22
                };
                tint(g, x, y, 0xb8ffd0, a);
            }
        }
        if t <= T_IN {
            for x in -14..=14 {
                for y in -1..=1 {
                    if x * x / 49 + y * y <= 1 {
                        tint(g, js_round(cx) + x, 51 + y, 0xd8ffe0, 0.45);
                    }
                }
            }
        }
    }

    // The witness cow on the last screen, the other one on the third.
    let witness = (t / 2.3).floor() as i32 % 2 == 0;
    cow(g, (g.panel(3) + 8) as f64, 44.0, witness, None, false);
    if t < T_LIFT {
        let grazing = t < T_ARRIVE + 1.4 && (t / 1.7).floor() as i32 % 2 == 0;
        cow(g, home.0, home.1, grazing, None, false);
        if (T_BEAM..T_LIFT).contains(&t) {
            // A startled "!".
            g.p(0xffffff, home.0 + 1.0, home.1 - 7.0, 1, 3);
            g.p(0xffffff, home.0 + 1.0, home.1 - 3.0, 1, 1);
        }
    } else if t < T_IN {
        let u = ease((t - T_LIFT) / (T_IN - T_LIFT));
        let wob = js_round((t * 9.0).sin()) as f64;
        let y = home.1 + (14.0 - home.1) * u;
        cow(g, home.0 + wob, y, false, Some(f / 2), false);
    }
    if let Some((sx, sy)) = sau {
        // Zip trail.
        if t > T_ZIP {
            let (hx, hy) = ((g.panel(2) + 20) as f64, 10.0);
            let n = 30;
            for k in 0..n {
                let u = k as f64 / n as f64;
                let (x, y) = (hx + (sx - hx) * u + 10.0, hy + (sy - hy) * u + 5.0);
                g.p(mix(0x2a3a5a, 0x8affc8, u), x, y, 1, 1);
                if u > 0.6 {
                    g.p(mix(0x2a3a5a, 0xffffff, u), x, y - 1.0, 1, 1);
                }
            }
        }
        glow(g, sx + 10.0, sy + 6.0, 12.0, 0x8affc8, 0.25);
        g.spr(SAUCER, SAUCERP, sx, sy, false);
        // Rim lights chase round; faster once the cow is aboard.
        let speed = if t > T_IN { 1 } else { 3 };
        let cols = [0xff5a5a, 0xffe25a, 0x5affa0, 0x5ac8ff];
        for k in 0..7u32 {
            let c = cols[((k + f / speed) % 4) as usize];
            g.p(c, sx + 1.0 + k as f64 * 3.0, sy + 5.0, 1, 1);
        }
        // The pilot blinks.
        if f % 30 < 2 {
            g.p(0x6ab8c8, sx + 9.0, sy + 2.0, 1, 1);
            g.p(0x6ab8c8, sx + 12.0, sy + 2.0, 1, 1);
        }
    }
    // Back down under a parachute, swinging, then standing in a heap of
    // silk with a new antenna.
    if t >= T_CHUTE {
        let u = ((t - T_CHUTE) / (T_LAND - T_CHUTE)).min(1.0);
        let sway = (t * 2.2).sin() * 4.0 * (1.0 - u);
        let y = -26.0 + (home.1 + 26.0) * (1.0 - (1.0 - u) * (1.0 - u * 0.4));
        let x = home.0 + sway;
        let landed = t >= T_LAND;
        if !landed {
            let (cx, cy) = (x - 0.0, y - 10.0);
            g.spr(CHUTE, CHUTEP, cx, cy, false);
            for (ax, bx) in [(0.0, 2.0), (5.0, 5.0), (10.0, 9.0)] {
                g.p(0x8a8a9a, cx + ax, cy + 4.0, 1, 1);
                let n = 5;
                for k in 1..n {
                    let v = k as f64 / n as f64;
                    g.p(0x6a6a7a, cx + ax + (bx - ax) * v, cy + 4.0 + 6.0 * v, 1, 1);
                }
            }
            cow(g, x, y, false, Some(f / 4), false);
        } else {
            // The canopy settles beside it.
            let s = ((t - T_LAND) / 0.5).min(1.0);
            let hh = js_round(4.0 * (1.0 - s)).max(1);
            for k in 0..11 {
                let c = if (k / 2) % 2 == 0 { 0xe8484a } else { 0xf0ece0 };
                let col_h = (hh as f64 * ((k as f64 + 0.5) / 11.0 * PI).sin()).ceil() as i32;
                g.p(
                    c,
                    home.0 + 12.0 + k as f64,
                    home.1 + 8.0 - col_h as f64,
                    1,
                    col_h + 1,
                );
            }
            cow(g, home.0, home.1, false, None, false);
        }
        // The antenna, its tip blinking green.
        let (ax, ay) = (x.min(home.0 + 50.0), if landed { home.1 } else { y });
        let ax = if landed { home.0 } else { ax };
        g.p(0x3a3a48, ax + 1.0, ay - 3.0, 1, 3);
        g.p(
            if f % 6 < 3 { 0x7aff6a } else { 0x2a6a2a },
            ax + 1.0,
            ay - 4.0,
            1,
            1,
        );
    }
    corn(g, f);
}

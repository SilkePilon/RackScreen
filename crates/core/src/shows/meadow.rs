//! Bee's day: a bright summer meadow. Fluffy clouds sail over green hills
//! with hedgerows, a far windmill turns, and the grass sways in layers,
//! thick with poppies, daisies, cornflowers, clover, knapweed and
//! buttercups; butterflies flit about and a stream winds through the
//! grass. A chubby bee buzzes in and works its way along the screens,
//! flower to flower, getting yellower and rounder at every stop, its
//! pollen baskets swelling. At the stream a frog on a lily pad shoots out
//! its tongue; the bee zips up just in time and the frog gets a mouthful
//! of dandelion clock instead, and sits there with a fluffy white
//! moustache while the seeds drift away. The heavy bee lumbers on to its
//! hive hanging from an old oak, and is far too round for the door: it
//! sticks, bottom wiggling, until a slim friend headbutts it in with a
//! puff of pollen, slips in after it, and a drop of honey falls.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, vgrad, vnoise};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

mod cast;
use cast::{draw_bee, draw_butterflies, draw_frog, draw_helper};

pub const SECS: f64 = 12.0;

/// The frog's tongue shoots out.
const T_STRIKE: f64 = 6.3;
/// The bee sets off for home.
const T_LEAVE: f64 = 6.8;
/// It reaches the hive door.
const T_DOOR: f64 = 8.4;
/// Wedged in the door.
const T_STUCK: f64 = 8.75;
/// The friend's headbutt.
const T_BUMP: f64 = 9.9;
/// In it goes.
const T_POP: f64 = 10.1;

const OUT: u32 = 0x2a1e18;
/// Where the meadow meets the far hills.
const HORIZON: f64 = 34.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Poppy,
    Daisy,
    Cornflower,
    Clover,
    Knapweed,
    Buttercup,
}

/// A flower the bee visits: stage position, root, stem, kind and the
/// time it sits there.
struct Visit {
    s: f64,
    root: f64,
    stem: f64,
    kind: Kind,
    t0: f64,
    t1: f64,
}

fn smooth(a: f64, b: f64, t: f64) -> f64 {
    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}

/// The screen with the stream and the frog.
fn brook(g: &World) -> u32 {
    last(g) - 1
}

fn visits(g: &World) -> Vec<Visit> {
    let kinds = [Kind::Poppy, Kind::Daisy, Kind::Cornflower, Kind::Clover];
    let mut v = Vec::new();
    for i in 0..brook(g) {
        for (k, lx) in [18.0, 43.0].into_iter().enumerate() {
            let n = v.len();
            v.push(Visit {
                s: stage_at(g, i, lx),
                root: 52.0 + (n % 2) as f64,
                stem: [12.0, 15.0, 11.0, 14.0][n % 4],
                kind: kinds[(i as usize * 2 + k) % 4],
                t0: 0.0,
                t1: 0.0,
            });
        }
    }
    let slot = 4.6 / v.len() as f64;
    for (k, w) in v.iter_mut().enumerate() {
        w.t0 = 0.9 + k as f64 * slot;
        w.t1 = w.t0 + slot * 0.5;
    }
    v.push(Visit {
        s: stage_at(g, brook(g), 17.0),
        root: 47.0,
        stem: 10.0,
        kind: Kind::Buttercup,
        t0: 5.75,
        t1: T_STRIKE,
    });
    v
}

/// A flower head's centre: (x, y) with the breeze.
fn head_at(g: &World, s: f64, root: f64, stem: f64, t: f64) -> (f64, f64) {
    let x = stage_x(g, s);
    (x + sway(x, t) * stem * 0.12, root - stem)
}

fn sway(x: f64, t: f64) -> f64 {
    (t * 2.1 + x * 0.09).sin() + 0.4 * (t * 3.3 + x * 0.21).sin()
}

/// The hive door on the last screen.
fn door(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 18.0, 30.0)
}

/// The dandelion clock beside the buttercup, and the frog's mouth.
fn clock(g: &World) -> (f64, f64) {
    (g.panel(brook(g)) as f64 + 21.0, 38.0)
}

fn mouth(g: &World) -> (f64, f64) {
    (g.panel(brook(g)) as f64 + 35.0, 46.0)
}

/// How far the tongue is out, `0..1`.
fn tongue(t: f64) -> f64 {
    if t < T_STRIKE {
        0.0
    } else if t < T_STRIKE + 0.1 {
        (t - T_STRIKE) / 0.1
    } else if t < T_STRIKE + 0.25 {
        1.0
    } else {
        1.0 - smooth(T_STRIKE + 0.25, T_STRIKE + 0.45, t)
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(g, &[0x3e94e4, 0x62acec, 0x92c8f2, 0xc4e2f8], 0, w, 0, 36);
    let sx = g.panel(1) as f64 + 46.0;
    glow(g, 0xfff4c0, sx, 7.0, 36.0, 0.45);
    glow(g, 0xffffff, sx, 7.0, 8.0, 0.8);
    g.disc(0xfffbe0, sx, 7.0, 4);
    // Fat cumulus clouds drifting by, lit from above.
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let span = w as f64 + 60.0;
        let cx = (rnd(kf + 1.0) * span + t * (1.5 + rnd(kf + 2.0))) % span - 30.0;
        let cy = 8.0 + rnd(kf + 3.0) * 12.0;
        let puffs = 4 + (rnd(kf + 4.0) * 3.0) as i32;
        for pass in 0..2 {
            for i in 0..puffs {
                let fi = i as f64;
                let px = cx + (fi - puffs as f64 / 2.0) * 4.5;
                let r = 3 + (rnd(kf * 9.0 + fi) * 3.0) as i32 - (i == 0 || i == puffs - 1) as i32;
                let py = cy - r as f64 * 0.5;
                if pass == 0 {
                    g.disc(0xd4e4f4, px, py + 1.0, r);
                } else {
                    g.disc(0xffffff, px - 0.5, py - 0.5, r - 1);
                }
            }
        }
    }
}

/// Far rolling hills with hedgerows, trees and a windmill.
fn hills(g: &mut World, t: f64) {
    let w = g.w();
    let far = |x: f64| 26.0 - 3.0 * (x * 0.024 + 0.5).sin() - 1.5 * (x * 0.071).sin();
    let near = |x: f64| 30.0 - 2.5 * (x * 0.033 + 2.0).sin() - (x * 0.09).sin();
    for x in 0..w {
        for y in js_round(far(x as f64))..40 {
            put(g, if on(x, y, 0.3) { 0xa4cca0 } else { 0xb4d6ac }, x, y);
        }
    }
    let tree = |g: &mut World, x: f64, yb: f64, r: i32, c: u32| {
        g.p(0x5a4a3a, x, yb - 1.0, 1, 2);
        g.disc(c, x, yb - 1.0 - r as f64, r);
        put(
            g,
            mix(c, 0xffffff, 0.3),
            js_round(x) - 1,
            js_round(yb) - 2 - r,
        );
    };
    for k in 0..g.spread(12) {
        let x = rnd(k as f64 + 10.0) * w as f64;
        tree(g, x, far(x) + 1.0, 1, 0x6a9a68);
    }
    // The windmill on the far ridge, sails turning.
    let mx = g.panel(1) as f64 + 14.0;
    let my = far(mx) + 1.0;
    g.p(0x8a7a7a, mx - 1.0, my - 7.0, 3, 7);
    g.p(0x6a5a5a, mx + 1.0, my - 7.0, 1, 7);
    g.p(0x7a4a3a, mx - 1.0, my - 8.0, 3, 1);
    for i in 0..4 {
        let a = t * 1.2 + i as f64 * PI / 2.0;
        line(
            g,
            0xf0ece0,
            mx,
            my - 8.0,
            mx + a.cos() * 5.0,
            my - 8.0 + a.sin() * 5.0,
        );
    }
    // Nearer hills: a patchwork of fields with hedges running down them.
    let tones = [0x92c46c, 0xc4d47a, 0x80b860, 0xa8cc6a];
    for x in 0..w {
        let xf = x as f64;
        let top = near(xf);
        for y in js_round(top)..40 {
            let d = y as f64 - top;
            let fx = xf - d * 1.4 + 5.0 * (xf * 0.04).sin();
            let id = (fx / 31.0).floor();
            let c = tones[(rnd(id + 40.0) * 4.0) as usize];
            let c = if fx.rem_euclid(31.0) < 1.0 {
                0x5a8a48
            } else if on(x, y, (d / 8.0).min(1.0) * 0.4) {
                mix(c, 0x4a7a3a, 0.25)
            } else {
                c
            };
            put(g, c, x, y);
        }
        put(g, 0xb0d888, x, js_round(top));
    }
    for k in 0..g.spread(18) {
        let kf = k as f64;
        let x = rnd(kf + 20.0) * w as f64;
        tree(g, x, near(x) + 1.0 + rnd(kf + 21.0) * 4.0, 2, 0x4a7a3c);
    }
}

/// The meadow floor and its back layer of grass.
fn meadow(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x8ccc5c, 0x78c050, 0x62ac44, 0x4e983a],
        0,
        w,
        HORIZON as i32 + 4,
        60,
    );
    // The meadow's far edge, a soft line of long grass against the hills.
    for x in 0..w {
        let edge = HORIZON + 4.0 + (x as f64 * 0.13).sin() * 0.8;
        for y in js_round(edge - 2.0)..js_round(edge) + 1 {
            put(g, if on(x, y, 0.5) { 0x9ad46a } else { 0x86c45a }, x, y);
        }
    }
    // Patches of light and shade rolling across with the clouds.
    for y in HORIZON as i32 + 3..60 {
        for x in 0..w {
            let n = vnoise(x as f64 * 0.04 - t * 0.15, y as f64 * 0.12, 3.0);
            if n > 0.62 && on(x, y, (n - 0.62) * 3.0) {
                blend(g, 0x3a7a2a, x, y, 0.35);
            }
        }
    }
    grass(g, t, g.spread(900), 38.0, 58.0, 2.0, 6.0, 0);
}

/// Blades of grass rooted between `y0` and `y1`, swaying in the breeze;
/// nearer (lower) ones are taller and darker.
#[allow(clippy::too_many_arguments)]
fn grass(g: &mut World, t: f64, n: u32, y0: f64, y1: f64, h0: f64, h1: f64, seed: u32) {
    let w = g.w() as f64;
    let lights = [0xa8dc70, 0x8ccc5c, 0x6ab448, 0x4e9a3a, 0x3a8030];
    for k in 0..n {
        let kf = (k + seed * 5000) as f64;
        let x = rnd(kf + 0.3) * w;
        let d = rnd(kf + 0.7);
        let y = y0 + (y1 - y0) * d;
        if in_stream(g, x, y) {
            continue;
        }
        let h = h0 + (h1 - h0) * (d * 0.6 + rnd(kf + 0.9) * 0.4);
        let tip = x + sway(x, t) * h * 0.22 + (rnd(kf + 1.1) - 0.5) * 2.0;
        let c = lights[((1.0 - d) * 2.5 + rnd(kf + 1.3) * 2.5) as usize % 5];
        line(g, c, x, y, tip, y - h);
    }
}

/// The stream on the frog's screen: its centre line and half width.
fn stream(g: &World, x: f64) -> Option<(f64, f64)> {
    let p = g.panel(brook(g)) as f64;
    let lx = x - p;
    if !(-4.0..64.0).contains(&lx) {
        return None;
    }
    Some((
        49.5 + lx * 0.07 + (lx * 0.11).sin() * 1.5,
        3.0 + (lx * 0.05).sin(),
    ))
}

fn in_stream(g: &World, x: f64, y: f64) -> bool {
    stream(g, x).is_some_and(|(c, hw)| (y - c).abs() < hw + 1.5)
}

fn brook_water(g: &mut World, t: f64, f: u32) {
    let p = g.panel(brook(g));
    for x in p - 4..p + 64 {
        let Some((c, hw)) = stream(g, x as f64) else {
            continue;
        };
        for y in js_round(c - hw - 1.0)..=js_round(c + hw + 1.0) {
            let d = (y as f64 - c) / hw;
            let col = if d.abs() > 1.0 {
                if d < 0.0 {
                    0x6a8a3a
                } else {
                    0x8a7a4a
                }
            } else {
                let deep = 1.0 - d.abs();
                let ripple = ((x as f64 * 0.5 - t * 4.0 + y as f64 * 1.3).sin() * 0.5 + 0.5) * 0.3;
                if on(x, y, deep * 0.6 + ripple) {
                    0x3a88c8
                } else {
                    0x6ab4e4
                }
            };
            put(g, col, x, y);
        }
        // Sparkles riding downstream.
        let sp = rnd(x as f64 * 0.37 + ((t * 3.0 + rnd(x as f64)) as i32) as f64);
        if sp > 0.9 && !f.is_multiple_of(3) {
            put(g, 0xffffff, x, js_round(c - hw * 0.4));
        }
    }
    // Reeds and bulrushes on the far bank.
    let pf = p as f64;
    for (k, lx) in [6.0, 9.0, 11.0, 50.0, 53.0, 57.0].into_iter().enumerate() {
        let Some((c, hw)) = stream(g, pf + lx) else {
            continue;
        };
        let base = c - hw;
        let h = 9.0 + (k % 3) as f64 * 3.0;
        let tip = pf + lx + sway(pf + lx, t) * 1.2;
        line(g, 0x4a8a3a, pf + lx, base, tip, base - h);
        if k % 2 == 0 {
            g.p(0x6a3a22, tip - 0.5, base - h + 1.0, 2, 3);
        }
    }
}

/// The old oak on the last screen, with the hive hanging from a branch.
fn oak(g: &mut World, t: f64) {
    let p = g.panel(last(g)) as f64;
    // The trunk, roots and the hive's branch.
    for y in 14..60 {
        let flare = if y > 52 { (y - 52) as f64 * 0.8 } else { 0.0 };
        let (a, b) = (p + 42.0 - flare, p + 50.0 + flare * 0.6);
        for x in js_round(a)..js_round(b) {
            let u = (x as f64 - a) / (b - a);
            let bark = (x * 7 + y / 3 * 5) % 9 == 0;
            let c = if bark || u > 0.75 {
                0x4a3222
            } else if u < 0.25 {
                0x8a6a48
            } else {
                0x6a4e34
            };
            put(g, c, x, y);
        }
    }
    for i in 0..26 {
        let fi = i as f64;
        let x = p + 43.0 - fi;
        let y = 21.0 - fi * 0.12 + (fi * 0.3).sin() * 0.5;
        let th = if i < 10 { 3 } else { 2 };
        g.p(0x5a402a, x, y, 1, th);
        put(g, 0x8a6a48, js_round(x), js_round(y));
    }
    // Leafy canopy, the outer leaves stirring.
    for k in 0..34 {
        let kf = k as f64;
        let x = p + 4.0 + rnd(kf + 300.0) * 60.0;
        let y = -2.0 + rnd(kf + 301.0) * 18.0;
        let r = 4 + (rnd(kf + 302.0) * 4.0) as i32;
        let st = (t * 1.5 + kf).sin() * 0.6;
        g.disc(0x2e6a2e, x + st, y + 1.0, r);
        g.disc(0x3e8a38, x + st - 1.0, y, r - 1);
        g.disc(0x5aa848, x + st - 2.0, y - 1.0, (r - 3).max(1));
    }
    // Sun flecks through the leaves.
    for k in 0..20 {
        let kf = k as f64;
        let x = p + 6.0 + rnd(kf + 340.0) * 56.0;
        let y = rnd(kf + 341.0) * 16.0;
        if ((t * 0.8 + kf) % 2.0) < 1.4 {
            put(g, 0x9ad46a, js_round(x), js_round(y));
        }
    }
}

/// The straw hive: a dome of coiled bands with a dark round door.
fn hive(g: &mut World, t: f64) {
    let (dx, dy) = door(g);
    let (cx, top) = (dx + 7.0, 21.0);
    line(g, 0x5a402a, cx, top - 1.0, cx, top + 1.0);
    for r in 0..14 {
        let v = r as f64 / 13.0;
        let hw = 3.0 + 6.0 * (v * PI * 0.62).sin();
        let y = js_round(top) + 1 + r;
        let band = r % 3 == 2;
        for x in js_round(cx - hw)..=js_round(cx + hw) {
            let u = (x as f64 - cx) / hw;
            let mut c = if band { 0xb07a2a } else { 0xe0b050 };
            if u > 0.45 {
                c = mix(c, 0x6a4018, 0.4);
            } else if u < -0.5 && !band {
                c = 0xf0cc70;
            }
            if (x + r) % 4 == 0 && !band {
                c = mix(c, 0x8a5a20, 0.3);
            }
            put(g, c, x, y);
        }
        put(g, OUT, js_round(cx - hw) - 1, y);
        put(g, OUT, js_round(cx + hw) + 1, y);
    }
    g.p(OUT, cx - 6.0, top + 15.0, 13, 1);
    // The door.
    g.disc(0x2a1608, dx + 1.0, dy, 2);
    put(g, 0x1a0c04, js_round(dx) + 1, js_round(dy));
    // A honey drip forming under the hive after the bee is home.
    if t > T_POP + 0.4 {
        let seep = smooth(T_POP + 0.4, T_POP + 1.0, t);
        let (hx, hy) = (cx + 1.0, top + 16.0);
        g.p(
            0xe8a020,
            hx - 3.0 * seep,
            hy - 1.0,
            js_round(6.0 * seep) + 1,
            1,
        );
        let u = ((t - T_POP - 0.6).max(0.0) % 1.3) / 1.3;
        if u < 0.65 {
            let len = js_round(u / 0.65 * 3.0);
            g.p(0xd88a18, hx, hy, 2, len + 1);
            put(g, 0xffe080, js_round(hx), js_round(hy));
        } else {
            let fy = hy + 3.0 + (u - 0.65).powi(2) * 200.0;
            if fy < 58.0 {
                g.p(0xd88a18, hx, fy, 2, 3);
                g.p(0xe8a020, hx, fy - 1.0, 1, 1);
                put(g, 0xffe080, js_round(hx), js_round(fy) + 1);
            }
        }
    }
}

/// One flower: a stem from `root`, leaves, and a head at `(hx, hy)`.
fn flower(g: &mut World, kind: Kind, x: f64, root: f64, hx: f64, hy: f64, big: bool) {
    line(g, 0x3a8030, x, root, hx, hy + 1.0);
    let mid = (root + hy) / 2.0;
    put(g, 0x4e9a3a, js_round(x) - 1, js_round(mid));
    put(g, 0x4e9a3a, js_round(x) + 1, js_round(mid) + 2);
    let (xi, yi) = (js_round(hx), js_round(hy));
    let big = big as i32;
    match kind {
        Kind::Poppy => {
            g.p(
                0xa01818,
                hx - 2.0 - big as f64,
                hy - 1.0,
                5 + 2 * big,
                2 + big,
            );
            g.p(
                0xe83a28,
                hx - 2.0 - big as f64,
                hy - 2.0 - big as f64,
                5 + 2 * big,
                2 + big,
            );
            put(g, 0xff7a5a, xi - 1 - big, yi - 2 - big);
            put(g, 0x2a1a1a, xi, yi - 1);
        }
        Kind::Daisy => {
            for (dx, dy) in [
                (-2, 0),
                (2, 0),
                (0, -2),
                (0, 2),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                put(g, 0xffffff, xi + dx * (1 + big), yi + dy);
                if big > 0 {
                    put(g, 0xffffff, xi + dx, yi + dy);
                }
            }
            put(g, 0xf8c828, xi, yi);
            if big > 0 {
                put(g, 0xe0a018, xi + 1, yi);
            }
        }
        Kind::Cornflower => {
            for (dx, dy) in [
                (-2, -1),
                (2, -1),
                (0, -2),
                (-1, 0),
                (1, 0),
                (-1, -2),
                (1, -2),
            ] {
                put(g, 0x4a78e8, xi + dx, yi + dy);
            }
            put(g, 0x2a48b0, xi, yi - 1);
            put(g, 0x8ab0ff, xi - 1, yi - 2);
            if big > 0 {
                put(g, 0x4a78e8, xi - 3, yi - 2);
                put(g, 0x4a78e8, xi + 3, yi - 2);
            }
        }
        Kind::Clover => {
            g.disc(0xc85a98, hx, hy - 1.0, 1 + big);
            put(g, 0xf09ac8, xi - 1, yi - 2);
        }
        Kind::Knapweed => {
            g.p(0x5a7a3a, hx - 1.0, hy - 1.0, 3, 2);
            g.p(0xa848c8, hx - 1.0, hy - 3.0, 3, 2);
            put(g, 0xc878e8, xi, yi - 4);
        }
        Kind::Buttercup => {
            g.p(0xf8d020, hx - 1.0 - big as f64, hy - 1.0, 3 + 2 * big, 2);
            g.p(0xffe860, hx - 1.0, hy - 2.0, 3, 1);
            put(g, 0xc89a10, xi + 1 + big, yi);
        }
    }
}

const KINDS: [Kind; 6] = [
    Kind::Poppy,
    Kind::Daisy,
    Kind::Cornflower,
    Kind::Clover,
    Kind::Knapweed,
    Kind::Buttercup,
];

/// The wildflowers: a scattered field, then the ones the bee visits.
fn flowers(g: &mut World, t: f64) {
    let w = g.w() as f64;
    for k in 0..g.spread(150) {
        let kf = k as f64;
        let x = rnd(kf + 500.0) * w;
        let d = rnd(kf + 501.0);
        let root = 40.0 + d * 18.0;
        if in_stream(g, x, root) {
            continue;
        }
        let kind = KINDS[(rnd(kf + 502.0) * 6.0) as usize];
        let stem = 2.0 + d * 7.0;
        if d < 0.25 {
            // Far ones are just dabs of colour.
            let c = [0xe83a28, 0xffffff, 0x4a78e8, 0xc85a98, 0xa848c8, 0xf8d020][kind as usize];
            put(g, c, js_round(x), js_round(root - stem));
            continue;
        }
        let hx = x + sway(x, t) * stem * 0.12;
        flower(g, kind, x, root, hx, root - stem, false);
    }
    for v in visits(g) {
        let (hx, hy) = head_at(g, v.s, v.root, v.stem, t);
        let x = stage_x(g, v.s);
        flower(g, v.kind, x, v.root, hx, hy, true);
    }
    // The dandelion clock by the stream, until the frog gets it.
    let (cx, cy) = clock(g);
    let root = cy + 8.0;
    line(g, 0x5a9a3a, cx, root, cx, cy + 1.0);
    if t < T_STRIKE + 0.1 {
        for a in 0..10 {
            let a = a as f64 * PI / 5.0;
            put(
                g,
                0xffffff,
                js_round(cx + a.cos() * 2.5),
                js_round(cy + a.sin() * 2.5),
            );
        }
        g.disc(0xe8ecf0, cx, cy, 1);
    }
    put(g, 0xa0a080, js_round(cx), js_round(cy));
}

/// Dandelion seeds the frog scattered, and the odd pollen mote.
fn floaters(g: &mut World, t: f64) {
    let (cx, cy) = clock(g);
    let u = t - T_STRIKE - 0.1;
    if u > 0.0 {
        for k in 0..18 {
            let kf = k as f64;
            let a = rnd(kf + 600.0) * PI * 2.0;
            let sp = 4.0 + rnd(kf + 601.0) * 8.0;
            let burst = 1.0 - (-u * 3.0).exp();
            let x = cx
                + a.cos() * sp * burst
                + u * (2.0 + rnd(kf + 602.0) * 3.0)
                + (u * 2.0 + kf).sin();
            let y = cy + a.sin() * sp * 0.6 * burst - u * (1.5 + rnd(kf + 603.0) * 2.0);
            put(g, 0xffffff, js_round(x), js_round(y));
            blend(g, 0xd8d8c8, js_round(x), js_round(y) + 1, 0.7);
        }
    }
    let w = g.w() as f64;
    for k in 0..g.spread(30) {
        let kf = k as f64;
        let x = (rnd(kf + 700.0) * w + t * (2.0 + rnd(kf + 701.0) * 2.0)) % w;
        let y = 20.0 + rnd(kf + 702.0) * 30.0 + (t * 1.3 + kf).sin() * 2.0;
        blend(g, 0xfff8c0, js_round(x), js_round(y), 0.55);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    hills(g, t);
    meadow(g, t);
    brook_water(g, t, f);
    oak(g, t);
    hive(g, t);
    flowers(g, t);
    draw_frog(g, t, f);
    draw_butterflies(g, t, f);
    draw_helper(g, t, f);
    draw_bee(g, t, f);
    grass(g, t, g.spread(260), 55.0, 61.0, 4.0, 9.0, 1);
    floaters(g, t);
}

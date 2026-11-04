//! Desert night: a big full moon with seas and craters, the Milky Way
//! arching over the dunes, saguaros and prickly pears lit on their moon
//! side. A coyote on a rock tries to howl at the moon, but a tumbleweed
//! comes bouncing across every screen and sails right over its head, so it
//! ducks. The weed snags on a saguaro; the coyote, glaring, tries again
//! and gets its howl, with a shooting star for applause. Then a gust of
//! wind pulls the weed loose and bowls it back the other way, and it lands
//! on the coyote's head. The coyote howls on regardless in its new hat.

use super::fx::{blend, glow, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_DUCK: f64 = 3.2;
const T_UP: f64 = 4.4;
const T_SNAG: f64 = 5.1;
const T_HOWL2: f64 = 6.0;
const T_STAR: f64 = 6.7;
const T_GUST: f64 = 8.0;
const T_HAT: f64 = 9.1;

const OUT: u32 = 0x120c1c;

const COYP: &[(char, u32)] = &[
    ('k', OUT),
    ('b', 0x8a7a6a),
    ('l', 0xc8b8a0),
    ('e', 0xf0e070),
    ('d', 0x5a4c44),
];
/// The coyote facing left: sitting, howling, ducking.
const COYOTE: [&[&str]; 3] = [
    &[
        "...kk.k......",
        "...kbkbk.....",
        "..kbbbbbk....",
        "kkkbebbbk....",
        "kbbbbbbdk....",
        ".kkllbbk.....",
        "...kllbbk....",
        "...kllbbbk...",
        "...kllbbbdk..",
        "...kllbbbbdk.",
        "...kllbbbbbk.",
        "..kkllkbbbbkk",
        "..kk.kkkkkkdk",
    ],
    &[
        "..k..........",
        ".kbk.........",
        ".kbbk.k......",
        "..kbbkbk.....",
        "...kbebbk....",
        "...kbbbdk....",
        "...kllbbk....",
        "...kllbbbk...",
        "...kllbbbdk..",
        "...kllbbbbdk.",
        "...kllbbbbbk.",
        "..kkllkbbbbkk",
        "..kk.kkkkkkdk",
    ],
    &[
        ".............",
        ".............",
        ".............",
        ".............",
        "....k.k......",
        "...kbkbk.....",
        "kkkbebbbk....",
        "kbbbbbbbbbk..",
        ".kkllbbbbbdk.",
        "...kllbbbbdk.",
        "...kllbbbbbk.",
        "..kkllkbbbbkk",
        "..kk.kkkkkkdk",
    ],
];

fn coyote_panel(g: &World) -> u32 {
    if g.layout.panels >= 4 {
        2
    } else {
        1
    }
}
fn last(g: &World) -> u32 {
    g.layout.panels - 1
}
/// The coyote's rock: (left x, top y).
fn rock(g: &World) -> (f64, f64) {
    (g.panel(coyote_panel(g)) as f64 + 24.0, 47.0)
}
/// Top-left of the coyote sprite.
fn coyote_at(g: &World) -> (f64, f64) {
    let (rx, ry) = rock(g);
    (rx + 2.0, ry - 13.0)
}
/// The big saguaro the weed snags on: (trunk x, base y).
fn saguaro_at(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 36.0, 50.0)
}

/// Which pose the coyote is in, and whether it is howling.
fn pose(t: f64) -> usize {
    if (T_DUCK..T_UP).contains(&t) {
        2
    } else if (1.4..T_DUCK).contains(&t) || (T_HOWL2..T_GUST + 0.2).contains(&t) || t > T_HAT + 0.5
    {
        1
    } else {
        0
    }
}

/// Where the top of the coyote's head is in pose `p`.
fn head_top(g: &World, p: usize) -> (f64, f64) {
    let (cx, cy) = coyote_at(g);
    match p {
        1 => (cx + 3.0, cy + 1.0),
        2 => (cx + 4.0, cy + 4.0),
        _ => (cx + 5.0, cy),
    }
}

fn ground(x: f64) -> f64 {
    50.0 - 2.0 * (x * 0.031 + 0.5).sin() - (x * 0.083).sin()
}

/// Landings of the tumbleweed: (time, stage position or fixed world spot,
/// hop height).
fn hops(g: &World) -> Vec<(f64, f64, Option<f64>, f64)> {
    let cp = coyote_panel(g);
    let (sx, sy) = saguaro_at(g);
    let snag_s = stage_at(g, last(g), sx - g.panel(last(g)) as f64 - 4.0);
    let mut v = vec![(0.0, stage_at(g, 0, -10.0), None, 0.0)];
    let s_near = stage_at(g, cp, 8.0);
    let n = 7;
    for i in 1..=n {
        let u = i as f64 / n as f64;
        v.push((
            u * T_DUCK,
            v[0].1 + (s_near - v[0].1) * u,
            None,
            9.0 - u * 3.0,
        ));
    }
    // The big one, right over the coyote's head.
    v.push((T_DUCK + 0.9, stage_at(g, cp, 50.0), None, 20.0));
    v.push((T_DUCK + 1.4, stage_at(g, last(g), 4.0), None, 7.0));
    // Into the cactus.
    v.push((T_SNAG, snag_s, Some(sy - 16.0), 10.0));
    // The gust bowls it back, onto the coyote's head.
    let (hx, hy) = head_top(g, 0);
    v.push((T_GUST, snag_s, Some(sy - 16.0), 0.0));
    v.push((T_GUST + 0.55, stage_at(g, last(g), 6.0), None, 8.0));
    v.push((
        T_HAT,
        stage_at(g, cp, hx - g.panel(cp) as f64 + 1.0),
        Some(hy - 2.0),
        12.0,
    ));
    v
}

/// The tumbleweed: (x, y, spin angle, airborne height).
fn weed(g: &World, t: f64) -> (f64, f64, f64, f64) {
    let hs = hops(g);
    let land_y = |s: f64, fixed: Option<f64>| fixed.unwrap_or_else(|| ground(stage_x(g, s)) - 4.0);
    for w in hs.windows(2) {
        let (t0, s0, f0, _) = w[0];
        let (t1, s1, f1, h) = w[1];
        if t < t1 {
            let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            let s = s0 + (s1 - s0) * u;
            let x = stage_x(g, s);
            let base = land_y(s0, f0) + (land_y(s1, f1) - land_y(s0, f0)) * u;
            let lift = 4.0 * h * u * (1.0 - u);
            let spin = s / 4.0;
            return (x, base - lift, spin, ground(x) - 4.0 - (base - lift));
        }
    }
    // Sitting on the coyote's head, which bobs as it howls.
    let p = pose(t);
    let (hx, hy) = head_top(g, p);
    (hx + 1.0, hy - 2.0, hs[hs.len() - 1].1 / 4.0, 0.0)
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x07061a, 0x100c2a, 0x1c1438, 0x2a1c44, 0x3a2650],
        0,
        w,
        0,
        44,
    );
    // The Milky Way, a hazy band arching across with a dark dust lane.
    for x in 0..w {
        let xf = x as f64;
        let c = 30.0 - xf / w as f64 * 26.0 + (xf * 0.02).sin() * 3.0;
        for y in 0..42 {
            let d = (y as f64 - c) / 7.0;
            let band = (-d * d).exp() * (0.6 + 0.4 * (xf * 0.11 + y as f64 * 0.3).sin().abs());
            let lane =
                (-((y as f64 - c - 1.0) / 1.6).powi(2)).exp() * (0.5 + 0.5 * (xf * 0.05).sin());
            let k = (band - lane * 0.7).max(0.0);
            if k > 0.05 {
                blend(g, 0x8a7ac8, x, y, k * 0.42);
                if rnd((x * 13 + y * 71) as f64) < k * 0.09 {
                    blend(g, 0xe8e0ff, x, y, 0.25 + rnd((x + y * 7) as f64) * 0.45);
                }
            }
        }
    }
    for i in 0..g.spread(90) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * w as f64) as i32, (rnd(fi + 1.0) * 40.0) as i32);
        let tw = 0.5 + 0.5 * (t * (1.5 + rnd(fi + 3.0) * 3.0) + fi).sin();
        let col = [0xffffff, 0xd8e0ff, 0xffe8c8, 0xc8d0ff][i as usize % 4];
        blend(
            g,
            col,
            x,
            y,
            (0.35 + 0.65 * tw) * (0.4 + rnd(fi + 2.0) * 0.6),
        );
        if rnd(fi + 4.0) > 0.93 {
            // A bright one with a little cross.
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                blend(g, col, x + dx, y + dy, 0.35 * tw);
            }
        }
    }
}

fn moon(g: &mut World) {
    let (cx, cy) = (g.panel(0) as f64 + 24.0, 15.0);
    glow(g, 0xb8b0e0, cx, cy, 30.0, 0.35);
    glow(g, 0xf0e8d0, cx, cy, 15.0, 0.4);
    let r = 9.5;
    for y in -10..=10 {
        for x in -10..=10 {
            let (xf, yf) = (x as f64, y as f64);
            let d = (xf * xf + yf * yf).sqrt() / r;
            if d > 1.0 {
                continue;
            }
            // Seas: soft blobs of grey.
            let sea =
                ((xf * 0.45 + 1.0).sin() + (yf * 0.5 - 0.5).cos() + (xf * 0.2 + yf * 0.3).sin())
                    / 3.0;
            let mut c = if sea > 0.35 { 0xcac2a8 } else { 0xf4ecd4 };
            if sea > 0.25 && sea <= 0.35 && on(x, y, 0.5) {
                c = 0xdcd4b8;
            }
            // Darker towards the limb.
            c = mix(c, 0xa8a090, (d - 0.75).max(0.0) * 1.6);
            put(g, c, js_round(cx) + x, js_round(cy) + y);
        }
    }
    // Craters with a bright rim on the lower right.
    for (dx, dy, cr) in [
        (-4.0, 3.0, 1.5),
        (3.0, -4.0, 1.0),
        (5.0, 4.0, 1.2),
        (-2.0, -5.0, 0.8),
    ] {
        let (x, y) = (js_round(cx + dx), js_round(cy + dy));
        put(g, 0xb0a890, x, y);
        if cr > 1.0 {
            put(g, 0xb0a890, x - 1, y);
            put(g, 0xb0a890, x, y - 1);
        }
        put(g, 0xffffff, x + 1, y + 1);
    }
    // A ray crater.
    put(g, 0xffffff, js_round(cx) + 1, js_round(cy) + 6);
    for i in 1..4 {
        blend(g, 0xffffff, js_round(cx) + 1 - i, js_round(cy) + 6 + i, 0.4);
        blend(
            g,
            0xffffff,
            js_round(cx) + 1 + i,
            js_round(cy) + 6 + i / 2,
            0.4,
        );
    }
}

fn land(g: &mut World, t: f64) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        // Far mesas with a moonlit top edge.
        let m = ((xf * 0.017 + 2.0).sin() * 1.3 + (xf * 0.06).sin() * 0.4).max(0.0);
        let top = if m > 0.5 {
            38 - ((m - 0.5) * 10.0).min(7.0) as i32
        } else {
            41
        };
        for y in top..60 {
            put(g, 0x241a3a, x, y);
        }
        put(g, 0x4a3a62, x, top);
        // Rolling dunes, crests catching the moon.
        let d = 44.0 - 2.5 * (xf * 0.045 + 1.0).sin() - 1.2 * (xf * 0.11).sin();
        for y in js_round(d)..60 {
            let k = (y as f64 - d) / 5.0;
            put(
                g,
                if on(x, y, k.min(1.0)) {
                    0x221a34
                } else {
                    0x342a50
                },
                x,
                y,
            );
        }
        put(g, 0x5a4a7a, x, js_round(d));
        let gy = ground(xf);
        for y in js_round(gy)..60 {
            put(g, if on(x, y, 0.2) { 0x1c1530 } else { 0x261e3c }, x, y);
        }
        put(g, 0x3e3460, x, js_round(gy));
    }
    // Sand ripples and pebbles, moonlit.
    for k in 0..g.spread(50) {
        let kf = k as f64;
        let x = (rnd(kf + 20.0) * w as f64) as i32;
        let y = ground(x as f64) as i32 + 2 + (rnd(kf + 21.0) * 8.0) as i32;
        let len = 2 + (rnd(kf + 22.0) * 4.0) as i32;
        for i in 0..len {
            put(g, 0x342a50, x + i, y);
        }
        if k % 4 == 0 {
            put(g, 0x5a5070, x, y - 1);
        }
    }
    // Gust: sand blowing left.
    let gust =
        ((t - T_GUST) / 0.3).clamp(0.0, 1.0) * (1.0 - ((t - T_GUST - 1.2) / 0.5).clamp(0.0, 1.0));
    if gust > 0.0 {
        for k in 0..g.spread(30) {
            let kf = k as f64;
            let x = w as f64 - ((rnd(kf + 30.0) * w as f64 + (t - T_GUST) * 140.0) % w as f64);
            let y = 36.0 + rnd(kf + 31.0) * 20.0;
            for i in 0..6 {
                blend(
                    g,
                    0x8a80b0,
                    js_round(x) + i,
                    js_round(y),
                    gust * 0.5 * (1.0 - i as f64 / 6.0),
                );
            }
        }
    }
}

/// A saguaro: ribbed trunk lit on the moon side, two arms.
fn saguaro(g: &mut World, x: f64, base: f64, h: f64, arms: [(f64, f64, bool); 2]) {
    let body = 0x1e3a3a;
    let dark = 0x14282a;
    let lit = 0x5a8a78;
    let trunk = |g: &mut World, x0: f64, y0: f64, y1: f64, wdt: i32| {
        for y in js_round(y0)..js_round(y1) {
            for i in 0..wdt {
                let c = if i == 0 {
                    lit
                } else if i == wdt - 1 || i % 2 == 0 {
                    dark
                } else {
                    body
                };
                put(g, c, js_round(x0) + i, y);
            }
        }
        // Rounded top.
        g.p(dark, x0 + 1.0, y0 - 1.0, wdt - 2, 1);
        put(g, lit, js_round(x0) + 1, js_round(y0) - 1);
    };
    trunk(g, x, base - h, base, 5);
    for (ay, ah, left) in arms {
        let ax: f64 = if left { -4.0 } else { 6.0 };
        // The elbow, then the arm up.
        g.p(body, x + if left { -3.0 } else { 4.0 }, base - ay, 4, 2);
        g.p(lit, x + if left { -3.0 } else { 4.0 }, base - ay, 4, 1);
        trunk(g, x + ax, base - ay - ah, base - ay + 1.0, 3);
    }
    // Spines catching the moon.
    for k in 0..(h as i32 / 3) {
        put(g, 0xa8c0b0, js_round(x) - 1, js_round(base - h) + k * 3 + 1);
    }
}

fn prickly_pear(g: &mut World, x: f64, y: f64) {
    for (dx, dy, r) in [
        (0.0, 0.0, 3),
        (-4.0, -3.0, 2),
        (3.0, -5.0, 2),
        (-1.0, -7.0, 2),
    ] {
        g.disc(0x1e3a3a, x + dx, y + dy, r);
        put(g, 0x5a8a78, js_round(x + dx) - r + 1, js_round(y + dy) - 1);
        put(g, 0xe85a7a, js_round(x + dx), js_round(y + dy) - r);
    }
}

fn plants(g: &mut World) {
    let (sx, sy) = saguaro_at(g);
    saguaro(g, sx, sy, 26.0, [(10.0, 8.0, true), (15.0, 7.0, false)]);
    saguaro(
        g,
        g.panel(0) as f64 + 44.0,
        52.0,
        18.0,
        [(8.0, 6.0, true), (11.0, 5.0, false)],
    );
    if g.layout.panels >= 4 {
        prickly_pear(g, g.panel(1) as f64 + 20.0, 52.0);
        saguaro(
            g,
            g.panel(1) as f64 + 44.0,
            48.0,
            12.0,
            [(6.0, 4.0, false), (5.0, 3.0, true)],
        );
    }
    // Barrel cactus by the coyote's rock.
    let (rx, ry) = rock(g);
    g.disc(0x1e3a3a, rx - 6.0, ry + 4.0, 2);
    put(g, 0xf0c050, js_round(rx) - 6, js_round(ry) + 1);
}

/// An elf owl peeking out of its hole in the big saguaro; its eyes go wide
/// when the weed lands next door.
fn owl(g: &mut World, t: f64, f: u32) {
    let (sx, sy) = saguaro_at(g);
    let (x, y) = (sx + 1.0, sy - 20.0);
    g.p(0x0a0810, x, y, 3, 4);
    g.p(0x6a5a48, x, y + 1.0, 3, 3);
    let wide = (T_SNAG..T_SNAG + 1.5).contains(&t) || (T_GUST..T_GUST + 0.8).contains(&t);
    let blink = (f % 40) < 2 && !wide;
    let eye = if blink { 0x4a3a30 } else { 0xf0d040 };
    put(g, eye, js_round(x), js_round(y) + 1);
    put(g, eye, js_round(x) + 2, js_round(y) + 1);
    if wide {
        put(g, 0xffffff, js_round(x) + 1, js_round(y) - 1);
        put(g, eye, js_round(x), js_round(y));
        put(g, eye, js_round(x) + 2, js_round(y));
    }
    put(g, 0xd8a040, js_round(x) + 1, js_round(y) + 2);
}

fn coyote(g: &mut World, t: f64) {
    let (rx, ry) = rock(g);
    // The rock, lit on its moon side.
    for r in 0..6 {
        let hw = 9 + r;
        for i in 0..2 * hw {
            let c = if i < 3 {
                0x5a5070
            } else if on(i, r, 0.3) {
                0x2a2240
            } else {
                0x382e50
            };
            put(g, c, js_round(rx) + 7 - hw + i, js_round(ry) + r);
        }
    }
    let p = pose(t);
    let (cx, cy) = coyote_at(g);
    g.spr(COYOTE[p], COYP, cx, cy, false);
    // Moonlit rim on its back.
    for r in 6..11 {
        blend(g, 0xd8d0f0, js_round(cx) + 3, js_round(cy) + r, 0.35);
    }
    if p == 1 {
        // The howl: arcs rising from the snout.
        let (hx, hy) = (cx, cy - 1.0);
        for k in 0..3 {
            let u = (t * 1.6 + k as f64 / 3.0) % 1.0;
            let r = 3.0 + u * 9.0;
            for a in 0..7 {
                let ang = -PI * 0.75 + (a as f64 - 3.0) * 0.17;
                blend(
                    g,
                    0xe8e0ff,
                    js_round(hx + ang.cos() * r),
                    js_round(hy + ang.sin() * r),
                    0.7 * (1.0 - u),
                );
            }
        }
    }
    if (T_DUCK + 0.2..T_UP).contains(&t) || (T_GUST + 0.3..T_HAT).contains(&t) {
        let (bx, by) = (js_round(cx) + 6, js_round(cy) - 6);
        g.p(OUT, (bx - 1) as f64, (by - 1) as f64, 3, 7);
        g.p(0xffe040, bx as f64, by as f64, 1, 3);
        put(g, 0xffe040, bx, by + 4);
    }
}

fn tumbleweed(g: &mut World, t: f64) {
    let (x, y, spin, air) = weed(g, t);
    let snagged = (T_SNAG..T_GUST).contains(&t);
    let spin = if snagged {
        spin + (t * 9.0).sin() * 0.15
    } else {
        spin
    };
    // A shadow on the sand, smaller the higher it flies.
    if air > -1.0 && t < T_HAT {
        let sw = (5.0 - air * 0.2).max(1.0) as i32;
        for i in -sw..=sw {
            blend(g, 0x0a0614, js_round(x) + i, js_round(ground(x)) + 1, 0.5);
        }
    }
    // A tangle of twigs: loops at different tilts.
    for k in 0..5 {
        let tilt = spin + k as f64 * 0.63;
        let (rx, ry) = (3.8 - (k % 2) as f64 * 0.7, 1.8 + (k % 3) as f64 * 0.8);
        for j in 0..22 {
            let a = j as f64 / 22.0 * PI * 2.0;
            let (ex, ey) = (a.cos() * rx, a.sin() * ry);
            let (px, py) = (
                ex * tilt.cos() - ey * tilt.sin(),
                ex * tilt.sin() + ey * tilt.cos(),
            );
            let lit = px + py < -1.5;
            let c = if lit {
                0xc8a878
            } else if (j + k) % 3 == 0 {
                0x5a4028
            } else {
                0x8a6a44
            };
            put(g, c, js_round(x + px), js_round(y + py));
        }
    }
    for k in 0..10 {
        let a = k as f64 * 2.4 + spin;
        let r = 1.0 + (k % 3) as f64;
        put(
            g,
            0x6a4c30,
            js_round(x + a.cos() * r),
            js_round(y + a.sin() * r),
        );
    }
    // Little twigs sticking out.
    for k in 0..4 {
        let a = spin * 1.0 + k as f64 * PI / 2.0 + 0.4;
        put(
            g,
            0x8a6a44,
            js_round(x + a.cos() * 4.6),
            js_round(y + a.sin() * 4.2),
        );
    }
}

fn shooting_star(g: &mut World, t: f64) {
    let u = (t - T_STAR) / 0.7;
    if !(0.0..1.0).contains(&u) {
        return;
    }
    let p0 = g.panel(coyote_panel(g)) as f64;
    let (x0, y0, x1, y1) = (p0 + 8.0, 5.0, p0 + 48.0, 17.0);
    let (x, y) = (x0 + (x1 - x0) * u, y0 + (y1 - y0) * u);
    for k in 0..14 {
        let kf = k as f64;
        let c = mix(0xffffff, 0x6a5aa8, kf / 14.0);
        blend(
            g,
            c,
            js_round(x - kf * 1.6),
            js_round(y - kf * 0.5),
            (1.0 - kf / 14.0) * (1.0 - u * 0.6),
        );
    }
    glow(g, 0xe8e0ff, x, y, 3.0, 0.8);
    put(g, 0xffffff, js_round(x), js_round(y));
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    moon(g);
    shooting_star(g, t);
    land(g, t);
    plants(g);
    owl(g, t, f);
    coyote(g, t);
    tumbleweed(g, t);
}

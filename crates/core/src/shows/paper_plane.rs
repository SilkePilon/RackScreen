//! Paper plane: on a sunny morning a kid at an attic window launches a
//! paper plane. The breeze carries it over the rooftops, where a cat on the
//! ridge swipes at it and misses; a pigeon takes off after it across the
//! park. Just as the pigeon lunges, the plane loops the loop and the
//! pigeon flies straight into a tree. The plane glides down the street and
//! slides neatly into the slot of a post box, whose flag pops up: air mail
//! delivered. The ruffled pigeon lands on the box to peer into the slot.

use super::fx::{blend, glow, mix, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.0;

const T_THROW: f64 = 0.6;
const T_SWIPE: f64 = 2.3;
const T_CHASE: f64 = 3.0;
const T_LOOP: f64 = 5.2;
const T_LOOPED: f64 = 6.4;
const T_LUNGE: f64 = 5.35;
const T_CRASH: f64 = 5.8;
const T_FREE: f64 = 8.6;
const T_SLOT: f64 = 8.6;
const T_IN: f64 = 9.2;
const T_PERCH: f64 = 9.8;

const LOOP_R: f64 = 9.0;

const SKY: [u32; 3] = [0x4aa0e6, 0x7cc4f2, 0xbfe6fa];

/// A pigeon facing right; wings up and down.
const PIGEON: [&[&str]; 2] = [
    &["..ww...", ".wwwgo.", "twwwgge", ".bbbb..", "..o.o.."],
    &[".......", ".bbbgo.", "twwwgge", ".wwww..", "..o.o.."],
];
const PIGEONP: &[(char, u32)] = &[
    ('w', 0x9aa4b8),
    ('b', 0x6a7488),
    ('g', 0x5aa88a),
    ('o', 0xe08a3a),
    ('e', 0x2a2a2a),
    ('t', 0x4a5468),
];

/// The cat on the ridge, sitting, facing left.
const CAT: &[&str] = &[
    "k.k....", "kkk....", "kekk...", "kkkk..t", ".wkkk.t", ".wkkkkt", ".wkkkk.", ".w.w...",
];
const CATP: &[(char, u32)] = &[
    ('k', 0x2a2430),
    ('e', 0xd8e050),
    ('w', 0xf0ece8),
    ('t', 0x2a2430),
];

fn roofs(g: &World) -> bool {
    g.layout.panels >= 4
}

/// Screens for the park and the post box (the last two).
fn park(g: &World) -> u32 {
    g.layout.panels.saturating_sub(2)
}
fn mail(g: &World) -> u32 {
    g.layout.panels.saturating_sub(1)
}

fn loop_c(g: &World) -> (f64, f64) {
    ((g.panel(park(g)) + 24) as f64, 20.0)
}

/// The post box's left edge and its slot row.
fn post_box(g: &World) -> (f64, f64) {
    ((g.panel(mail(g)) + 26) as f64, 36.0)
}

/// Keyframes of the glide as (time, stage position, height).
fn keys(g: &World) -> Vec<(f64, f64, f64)> {
    let (lx, ly) = loop_c(g);
    let lp = park(g);
    let (bx, by) = post_box(g);
    let mut k = vec![
        (T_THROW - 0.2, stage_at(g, 0, 42.0), 26.0),
        (T_THROW, stage_at(g, 0, 44.0), 26.0),
        (1.4, stage_at(g, 0, 56.0), 20.0),
    ];
    if roofs(g) {
        k.push((T_SWIPE, stage_at(g, 1, 20.0), 27.0));
        k.push((3.4, stage_at(g, 1, 50.0), 14.0));
    } else {
        k.push((2.6, stage_at(g, 0, 60.0) + 6.0, 16.0));
    }
    k.push((4.4, stage_at(g, lp, 4.0), 22.0));
    k.push((
        T_LOOP,
        stage_at(g, lp, lx - g.panel(lp) as f64),
        ly + LOOP_R,
    ));
    k.push((
        T_LOOP + 0.2,
        stage_at(g, lp, lx - g.panel(lp) as f64 + 3.0),
        ly + LOOP_R,
    ));
    // (The loop itself is drawn on its own; these bracket it.)
    k.push((
        T_LOOPED,
        stage_at(g, lp, lx - g.panel(lp) as f64),
        ly + LOOP_R,
    ));
    k.push((7.2, stage_at(g, lp, 58.0), 24.0));
    k.push((8.0, stage_at(g, mail(g), 10.0), 30.0));
    k.push((
        T_SLOT,
        stage_at(g, mail(g), bx - g.panel(mail(g)) as f64),
        by,
    ));
    k.push((
        T_IN,
        stage_at(g, mail(g), bx - g.panel(mail(g)) as f64 + 9.0),
        by,
    ));
    k.push((
        SECS + 1.0,
        stage_at(g, mail(g), bx - g.panel(mail(g)) as f64 + 9.0),
        by,
    ));
    k
}

/// Catmull-Rom through the keyframes: stage position and height at `t`.
fn glide(k: &[(f64, f64, f64)], t: f64) -> (f64, f64) {
    let n = k.len();
    let mut i = 0;
    while i + 2 < n && k[i + 1].0 <= t {
        i += 1;
    }
    let (t1, t2) = (k[i].0, k[i + 1].0);
    let u = ((t - t1) / (t2 - t1)).clamp(0.0, 1.0);
    let p0 = k[i.saturating_sub(1)];
    let p1 = k[i];
    let p2 = k[i + 1];
    let p3 = k[(i + 2).min(n - 1)];
    let cr = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * (2.0 * b
            + (-a + c) * u
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u
            + (-a + 3.0 * b - 3.0 * c + d) * u * u * u)
    };
    (cr(p0.1, p1.1, p2.1, p3.1), cr(p0.2, p1.2, p2.2, p3.2))
}

/// The plane's world position at `t`.
fn plane_at(g: &World, k: &[(f64, f64, f64)], t: f64) -> (f64, f64) {
    if (T_LOOP..T_LOOPED).contains(&t) {
        let (cx, cy) = loop_c(g);
        let u = (t - T_LOOP) / (T_LOOPED - T_LOOP);
        let phi = ease(u) * 2.0 * PI;
        return (cx + phi.sin() * LOOP_R * 1.15, cy + phi.cos() * LOOP_R);
    }
    let (s, y) = glide(k, t);
    (stage_x(g, s), y)
}

/// The paper plane, nose along `heading`, rasterised from two triangles.
fn plane(g: &mut World, x: f64, y: f64, heading: f64, flutter: f64) {
    let (c, s) = (heading.cos(), heading.sin());
    let tri = |px: f64, py: f64, a: (f64, f64), b: (f64, f64), d: (f64, f64)| {
        let e = |p: (f64, f64), q: (f64, f64)| (q.0 - p.0) * (py - p.1) - (q.1 - p.1) * (px - p.0);
        let (e1, e2, e3) = (e(a, b), e(b, d), e(d, a));
        (e1 >= 0.0 && e2 >= 0.0 && e3 >= 0.0) || (e1 <= 0.0 && e2 <= 0.0 && e3 <= 0.0)
    };
    let wing = 3.4 + flutter;
    const R: i32 = 8;
    const N: usize = (2 * R + 1) as usize;
    // Rasterise into a little mask first so the plane can get an outline.
    let mut mask = [[0u32; N]; N];
    for dy in -R..=R {
        for dx in -R..=R {
            let (wx, wy) = (js_round(x) + dx, js_round(y) + dy);
            let (rx, ry) = (wx as f64 - x, wy as f64 - y);
            // Into the plane's own frame.
            let (lx, ly) = (rx * c + ry * s, -rx * s + ry * c);
            let nose = (6.5, 0.0);
            let cell = &mut mask[(dy + R) as usize][(dx + R) as usize];
            if tri(lx, ly, nose, (-5.0, -wing), (-5.0, 0.4)) {
                *cell = if ly > -0.9 { 0xd0deec } else { 0xffffff };
            } else if tri(lx, ly, nose, (-5.0, 0.4), (-3.0, 2.4)) {
                *cell = 0x9ab4cc;
            }
        }
    }
    for dy in -R..=R {
        for dx in -R..=R {
            let (i, j) = ((dy + R) as usize, (dx + R) as usize);
            let (wx, wy) = (js_round(x) + dx, js_round(y) + dy);
            if mask[i][j] != 0 {
                put(g, mask[i][j], wx, wy);
            } else {
                let near = [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)]
                    .iter()
                    .any(|&(a, b)| {
                        let (ii, jj) = (i as i32 + a, j as i32 + b);
                        (0..N as i32).contains(&ii)
                            && (0..N as i32).contains(&jj)
                            && mask[ii as usize][jj as usize] != 0
                    });
                if near {
                    blend(g, 0x3a5a7a, wx, wy, 0.7);
                }
            }
        }
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(g, &SKY, 0, w, 0, 46);
    // The sun high over the last screen.
    let (sx, sy) = ((g.panel(mail(g)) + 44) as f64, 9.0);
    glow(g, 0xfff4c0, sx, sy, 18.0, 0.55);
    g.disc(0xfff2b0, sx, sy, 4);
    g.disc(0xfffbe8, sx, sy, 3);
    // Puffy clouds in two layers.
    for (n, sp, y0) in [(5u32, 1.0, 5.0), (4, 2.2, 12.0)] {
        for k in 0..g.spread(n) {
            let kf = k as f64 + sp * 10.0;
            let span = w as f64 + 40.0;
            let x = (rnd(kf) * span + t * sp) % span - 20.0;
            let y = y0 + rnd(kf + 1.0) * 5.0;
            let s = 0.8 + rnd(kf + 2.0) * 0.6;
            g.disc(0xc8e0f4, x + 1.0, y + 2.0, js_round(3.0 * s));
            g.disc(0xffffff, x - 3.0 * s, y + 1.0, js_round(2.0 * s));
            g.disc(0xffffff, x, y, js_round(3.0 * s));
            g.disc(0xffffff, x + 4.0 * s, y + 1.0, js_round(2.2 * s));
            g.p(0xd8eaf8, x - 5.0 * s, y + 3.0, js_round(11.0 * s), 1);
        }
    }
    // A hot-air balloon drifting very far off.
    let (bx, by) = (
        (g.panel(park(g)) + 8) as f64 + t * 0.8,
        9.0 + (t * 0.7).sin(),
    );
    for r in 0..5 {
        let hw = [1.0, 2.0, 2.0, 2.0, 1.0][r];
        for i in -2..=2 {
            if (i as f64).abs() <= hw {
                let c = if (i + 2) % 2 == 0 { 0xe85a4a } else { 0xf8e070 };
                put(g, c, js_round(bx) + i, js_round(by) + r as i32);
            }
        }
    }
    put(g, 0x8a6a4a, js_round(bx), js_round(by) + 6);
}

fn skyline(g: &mut World) {
    let w = g.w();
    // Hazy far town: blocks, a spire and trees in pale blue.
    let mut x = 0;
    let mut k = 0.0;
    while x < w {
        let bw = 5 + (rnd(k + 300.0) * 9.0) as i32;
        let bh = 4 + (rnd(k + 301.0) * 8.0) as i32;
        let top = 44 - bh;
        let c = if rnd(k + 302.0) > 0.5 {
            0x9ac4e0
        } else {
            0xa8cce4
        };
        g.p(c, x as f64, top as f64, bw, 46 - top);
        for wx in (x + 1..x + bw - 1).step_by(2) {
            if rnd(wx as f64 + k) > 0.5 {
                put(g, 0xc8e0f0, wx, top + 2);
            }
        }
        if rnd(k + 303.0) > 0.85 {
            for i in 0..6 {
                g.p(
                    c,
                    x as f64 + bw as f64 / 2.0 - (i / 3) as f64,
                    (top - 6 + i) as f64,
                    1 + 2 * (i / 3),
                    1,
                );
            }
        }
        x += bw + (rnd(k + 304.0) * 3.0) as i32;
        k += 1.0;
    }
    for k in 0..g.spread(18) {
        let kf = k as f64;
        let tx = rnd(kf + 310.0) * w as f64;
        g.disc(0x8ab8c8, tx, 43.0, 2 + (rnd(kf + 311.0) * 2.0) as i32);
    }
    // Grass from here down; each screen paints its own ground over it.
    vgrad(g, &[0x7ac85a, 0x5aa848, 0x4a9040], 0, w, 45, 60);
}

/// The kid's house: pastel wall, a dormer window with flowers.
fn house(g: &mut World, t: f64, f: u32) {
    let x0 = g.panel(0) as f64;
    let wall = (x0 + 18.0, 20.0);
    g.p(0xf0d8b0, wall.0, wall.1, 50, 40);
    for r in 0..8 {
        let y = wall.1 + 4.0 + r as f64 * 5.0;
        g.p(0xe0c498, wall.0, y, 50, 1);
    }
    // Roof eave above, tiles in rows.
    for r in 0..8 {
        let y = wall.1 - 8.0 + r as f64;
        g.p(
            if r % 2 == 0 { 0xc8503a } else { 0xb04030 },
            wall.0 - 6.0 + r as f64 * 0.5,
            y,
            60,
            1,
        );
    }
    g.p(0x8a3028, wall.0 - 2.0, wall.1, 54, 1);
    // A downpipe and a lower window.
    g.p(0xa8a8b0, wall.0 + 3.0, wall.1 + 1.0, 1, 39);
    g.p(0x6a8ab0, x0 + 26.0, 44.0, 8, 9);
    g.p(0xffffff, x0 + 26.0, 44.0, 8, 1);
    g.p(0xa8c8e8, x0 + 27.0, 45.0, 2, 3);
    g.p(0xffffff, x0 + 29.0, 44.0, 1, 9);
    // The open window: frame, dark room, the kid, a flower box.
    let (wx, wy) = (x0 + 34.0, 24.0);
    g.p(0xffffff, wx - 1.0, wy - 1.0, 14, 13);
    g.p(0x4a3a5a, wx, wy, 12, 11);
    g.p(0x5a4a6a, wx, wy, 12, 2);
    // Shutter swung open.
    g.p(0x4a9ad0, wx + 13.0, wy - 1.0, 4, 13);
    for r in 0..5 {
        g.p(0x3a7ab0, wx + 13.0, wy + 1.0 + r as f64 * 2.0, 4, 1);
    }
    kid(g, wx + 4.0, wy + 2.0, t, f);
    g.p(0x9a5a3a, wx - 1.0, wy + 11.0, 14, 3);
    g.p(0xb06a40, wx - 1.0, wy + 11.0, 14, 1);
    for i in 0..6 {
        let c = [0xf04a6a, 0xf8d040, 0xf8f8f8][i % 3];
        g.p(0x4a9a3a, wx + i as f64 * 2.0, wy + 10.0, 1, 1);
        put(g, c, js_round(wx + i as f64 * 2.0), js_round(wy + 9.0));
    }
}

fn kid(g: &mut World, x: f64, y: f64, t: f64, f: u32) {
    let skin = 0xf0c8a0;
    // Shoulders and shirt.
    g.p(0xf0b040, x - 1.0, y + 6.0, 7, 3);
    // Head with messy hair.
    g.p(skin, x, y + 1.0, 5, 5);
    g.p(0x6a3a1a, x - 1.0, y, 6, 2);
    g.p(0x6a3a1a, x - 1.0, y + 2.0, 1, 2);
    put(g, 0x6a3a1a, js_round(x + 5.0), js_round(y));
    put(g, 0x2a1a1a, js_round(x + 3.0), js_round(y + 3.0));
    put(g, 0xe88a8a, js_round(x + 4.0), js_round(y + 4.0));
    // The throwing arm: back, flung forward, then a happy wave.
    let (ax, ay) = (x + 5.0, y + 6.0);
    let a: f64 = if t < T_THROW - 0.35 {
        -2.4
    } else if t < T_THROW {
        -2.4 + ease((t - T_THROW + 0.35) / 0.35) * 2.2
    } else if t < T_THROW + 0.4 {
        -0.2
    } else {
        -1.3 + if (f / 3).is_multiple_of(2) { 0.4 } else { -0.2 }
    };
    for i in 1..5 {
        let fi = i as f64;
        put(
            g,
            if i == 4 { skin } else { 0xf0b040 },
            js_round(ax + a.cos() * fi),
            js_round(ay + a.sin() * fi),
        );
    }
    if (T_THROW + 0.4..9.0).contains(&t) {
        // Open mouth: cheering.
        put(g, 0x8a2a2a, js_round(x + 4.0), js_round(y + 4.0));
    }
}

/// Rooftops on the second screen: tiles, chimneys, a washing line.
fn rooftops(g: &mut World, t: f64) {
    if !roofs(g) {
        return;
    }
    let x0 = g.panel(1) as f64 - 8.0;
    for (i, (dx, top, c)) in [
        (0.0, 38.0, 0xc8603a),
        (26.0, 34.0, 0x6a7a9a),
        (52.0, 40.0, 0xb84a3a),
    ]
    .into_iter()
    .enumerate()
    {
        let bx = x0 + dx;
        let wall = [0xf0e0c0, 0xe8c8a8, 0xd8e0e8][i];
        g.p(wall, bx + 2.0, top + 6.0, 24, 60);
        for r in 0..7 {
            let rf = r as f64;
            let col = if r % 2 == 0 {
                c
            } else {
                mix(c, 0x000000, 0.18)
            };
            g.p(col, bx + 6.0 - rf, top + rf, 16 + r * 2, 1);
        }
        g.p(mix(c, 0xffffff, 0.35), bx + 6.0, top, 16, 1);
        for wx in [6.0, 16.0] {
            g.p(0x5a7aa0, bx + wx, top + 10.0, 4, 5);
            g.p(0xffffff, bx + wx, top + 10.0, 4, 1);
        }
        // Chimney with a curl of smoke.
        let cx = bx + 18.0;
        g.p(0x9a5a4a, cx, top - 6.0, 3, 6);
        g.p(0x7a4a3a, cx - 1.0, top - 7.0, 5, 1);
        for s in 0..3 {
            let a = ((t * 0.8 + s as f64 * 0.33 + i as f64 * 0.2) % 1.0) * 1.0;
            let r = js_round(1.0 + a * 1.5);
            g.disc(
                mix(0xe8eef4, SKY[1], a),
                cx + 1.0 + a * 6.0,
                top - 9.0 - a * 8.0,
                r,
            );
        }
    }
    // A TV aerial on the first chimney.
    let ax = x0 + 18.0;
    g.p(0x5a5a6a, ax + 1.0, 23.0, 1, 8);
    g.p(0x5a5a6a, ax - 2.0, 24.0, 7, 1);
    g.p(0x5a5a6a, ax - 1.0, 26.0, 5, 1);
}

/// The cat on the ridge: watches, swipes at the plane, misses.
fn cat(g: &mut World, t: f64, f: u32) {
    if !roofs(g) {
        return;
    }
    let u = ((t - T_SWIPE + 0.25) / 0.5).clamp(0.0, 1.0);
    let hop = js_round((u * PI).sin() * 3.0) as f64;
    let (cx, cy) = (g.panel(1) as f64 + 20.0, 30.0 - hop);
    g.spr(CAT, CATP, cx, cy, false);
    // Tail swish.
    let sw = js_round((t * 3.0).sin());
    put(g, 0x2a2430, js_round(cx + 7.0) + sw, js_round(cy + 2.0));
    // The paw lashes up as the plane passes overhead.
    if u > 0.0 && u < 1.0 {
        let reach = (u * PI).sin();
        let a = -1.4 - reach * 0.6;
        for i in 1..(2 + js_round(reach * 5.0)) {
            let fi = i as f64;
            put(
                g,
                0x2a2430,
                js_round(cx + 1.0 + a.cos() * fi),
                js_round(cy + 4.0 + a.sin() * fi),
            );
        }
        let fi = 1.0 + reach * 5.0;
        put(
            g,
            0xf0ece8,
            js_round(cx + 1.0 + a.cos() * fi),
            js_round(cy + 4.0 + a.sin() * fi),
        );
        // Startled ears flat.
        g.p(0x2a2430, cx - 1.0, cy, 4, 1);
    } else if t > T_SWIPE + 0.25 && t < T_SWIPE + 1.6 && (f / 4).is_multiple_of(2) {
        // Head turned to stare after it.
        put(g, 0xd8e050, js_round(cx + 2.0), js_round(cy + 2.0));
    }
}

/// The park: path, two big trees, a bench and a lamp post.
fn park_scene(g: &mut World, t: f64, rustle: f64) {
    let x0 = g.panel(park(g)) as f64;
    // A sandy path winding through.
    for y in 50..60 {
        let yf = y as f64;
        let cx = x0 + 30.0 + (yf * 0.4).sin() * 4.0;
        let hw = 3.0 + (yf - 50.0) * 0.6;
        g.p(0xe8d0a0, cx - hw, yf, js_round(hw * 2.0), 1);
    }
    // Flowers in the grass.
    for k in 0..12 {
        let kf = k as f64;
        let fx = x0 + rnd(kf + 400.0) * 60.0;
        let fy = 49.0 + rnd(kf + 401.0) * 10.0;
        put(
            g,
            [0xf8f8f8, 0xf8e040, 0xf05a8a][k % 3],
            js_round(fx),
            js_round(fy),
        );
    }
    // Bench.
    let bx = x0 + 6.0;
    g.p(0x8a5a3a, bx, 44.0, 12, 1);
    g.p(0xa86a40, bx, 46.0, 12, 2);
    g.p(0x3a3a4a, bx + 1.0, 48.0, 1, 3);
    g.p(0x3a3a4a, bx + 10.0, 48.0, 1, 3);
    // Lamp post.
    let lx = x0 + 52.0;
    g.p(0x3a4a5a, lx, 26.0, 1, 24);
    g.p(0x3a4a5a, lx - 1.0, 49.0, 3, 1);
    g.p(0x3a4a5a, lx - 2.0, 24.0, 5, 2);
    g.p(0xf8f0c8, lx - 1.0, 26.0, 3, 1);
    // Trees: trunk, layered round canopy; the right one rustles when hit.
    for (i, (dx, r)) in [(-4.0, 9.0), (44.0, 10.0)].into_iter().enumerate() {
        let tx = x0 + dx;
        let shake = if i == 1 {
            (t * 40.0).sin() * rustle * 1.2
        } else {
            0.0
        };
        g.p(0x7a5a3a, tx - 1.0, 34.0, 3, 16);
        g.p(0x5a4028, tx + 1.0, 34.0, 1, 16);
        let cy = 26.0;
        let ri = r as i32;
        g.disc(0x3a8a3a, tx + shake, cy + 2.0, ri);
        g.disc(0x4aa044, tx - 2.0 + shake, cy, ri - 2);
        g.disc(0x6ac05a, tx - 3.0 + shake, cy - 2.0, ri - 5);
        for k in 0..10 {
            let kf = k as f64 + i as f64 * 20.0;
            let a = rnd(kf + 410.0) * PI * 2.0;
            let rr = rnd(kf + 411.0) * (r - 2.0);
            put(
                g,
                0x8ad86a,
                js_round(tx + shake + a.cos() * rr - 1.0),
                js_round(cy + a.sin() * rr - 1.0),
            );
        }
    }
}

fn tree_hit(g: &World) -> (f64, f64) {
    ((g.panel(park(g)) + 44) as f64, 25.0)
}

/// The street with the post box.
fn street(g: &mut World) {
    let x0 = g.panel(mail(g)) as f64;
    // A garden hedge and a front gate behind the pavement.
    g.p(0x4a9a44, x0 - 4.0, 40.0, 70, 10);
    for k in 0..30 {
        let kf = k as f64;
        g.disc(0x5aae4a, x0 - 4.0 + kf * 2.4, 40.0 + (kf * 1.3).sin(), 2);
    }
    for k in 0..30 {
        put(
            g,
            0x7ac85a,
            js_round(x0 + rnd(k as f64 + 420.0) * 64.0 - 2.0),
            41 + (rnd(k as f64 + 421.0) * 7.0) as i32,
        );
    }
    // Pavement and kerb.
    g.p(0xc8c8d0, x0 - 6.0, 50.0, 72, 7);
    for k in 0..9 {
        g.p(0xa8a8b4, x0 - 6.0 + k as f64 * 9.0, 50.0, 1, 7);
    }
    g.p(0xe0e0e8, x0 - 6.0, 50.0, 72, 1);
    g.p(0x8a8a96, x0 - 6.0, 57.0, 72, 3);
}

fn post_box_draw(g: &mut World, t: f64, f: u32) {
    let (bx, sy) = post_box(g);
    let red = 0xd83a32;
    let dark = 0xa82a26;
    // Body: a pillar with a domed cap and a plinth.
    g.p(red, bx, 32.0, 10, 18);
    g.p(0xf06a5a, bx + 1.0, 32.0, 1, 18);
    g.p(dark, bx + 8.0, 32.0, 2, 18);
    g.p(dark, bx - 1.0, 31.0, 12, 1);
    for r in 0..3 {
        g.p(red, bx + r as f64, 28.0 + r as f64, 10 - r * 2, 1);
    }
    g.p(red, bx, 30.0, 10, 1);
    g.p(0xf06a5a, bx + 2.0, 28.0, 3, 1);
    g.p(0x3a3a4a, bx - 1.0, 49.0, 12, 2);
    // The slot, with a flap that flips as the mail goes in.
    g.p(0x1a1a24, bx + 1.0, sy, 8, 1);
    let flip = t > T_SLOT && t < T_IN + 0.15;
    g.p(
        if flip { 0x1a1a24 } else { 0xb8b8c0 },
        bx + 1.0,
        sy - 1.0,
        8,
        1,
    );
    // A little plaque.
    g.p(0xf8e8b0, bx + 2.0, 40.0, 6, 3);
    g.p(0x8a6a3a, bx + 3.0, 41.0, 4, 1);
    // The flag pops up once the mail is in.
    let up = ease((t - T_IN - 0.1) / 0.35);
    let a = PI / 2.0 - up * PI / 2.0 - (up * PI).sin() * 0.3;
    let (px, py) = (bx + 10.0, 38.0);
    g.p(0x6a6a74, px, py, 1, 1);
    // The flag arm: lying down along the side, then pointing up.
    for i in 0..6 {
        let fi = i as f64;
        let (x, y) = (px + 1.0 + a.sin() * fi, py - a.cos() * fi);
        put(g, 0x6a6a74, js_round(x), js_round(y));
    }
    let (fx, fy) = (px + 1.0 + a.sin() * 5.0, py - a.cos() * 5.0);
    g.p(0xf8d040, fx - 1.0, fy - 1.0, 3, 2);
    if t > T_IN && t < T_IN + 1.6 {
        // Ding! a sparkle, and a heart floating up out of the slot.
        let u = (t - T_IN) / 1.6;
        let (hx, hy) = (bx + 4.0, sy - 5.0 - u * 12.0);
        let c = mix(0xf04a6a, SKY[1], u * u);
        g.p(c, hx, hy, 2, 1);
        g.p(c, hx + 3.0, hy, 2, 1);
        g.p(c, hx, hy + 1.0, 5, 1);
        g.p(c, hx + 1.0, hy + 2.0, 3, 1);
        put(g, c, js_round(hx + 2.0), js_round(hy + 3.0));
        if (f / 2).is_multiple_of(2) && u < 0.5 {
            let (sx, sy2) = (fx + 3.0, fy - 2.0);
            put(g, 0xffffff, js_round(sx), js_round(sy2));
            put(g, 0xfff4a0, js_round(sx) - 1, js_round(sy2));
            put(g, 0xfff4a0, js_round(sx) + 1, js_round(sy2));
            put(g, 0xfff4a0, js_round(sx), js_round(sy2) - 1);
            put(g, 0xfff4a0, js_round(sx), js_round(sy2) + 1);
        }
    }
}

/// The pigeon: perched, chasing, lunging, stuck in the tree, then landing
/// on the post box. Returns its position and pose.
fn pigeon_at(g: &World, k: &[(f64, f64, f64)], t: f64) -> (f64, f64, bool, Pose) {
    let perch = if roofs(g) {
        (g.panel(1) as f64 + 33.0, 22.0)
    } else {
        ((g.panel(park(g)) + 50) as f64, 21.0)
    };
    let hit = tree_hit(g);
    let (mx, _) = post_box(g);
    let top = (mx + 2.0, 21.0);
    if t < T_CHASE {
        return (perch.0, perch.1, false, Pose::Sit);
    }
    if t < T_LUNGE {
        let (px, py) = plane_at(g, k, t - 0.55);
        let u = ease((t - T_CHASE) / 0.8);
        let (tx, ty) = (px - 6.0, py + 2.0);
        return (
            perch.0 + (tx - perch.0) * u,
            perch.1 + (ty - perch.1) * u,
            tx < perch.0 && u < 0.5,
            Pose::Fly,
        );
    }
    if t < T_CRASH {
        let (sx, sy, _, _) = pigeon_at(g, k, T_LUNGE - 0.001);
        let u = ((t - T_LUNGE) / (T_CRASH - T_LUNGE)).powi(2);
        return (
            sx + (hit.0 - sx) * u,
            sy + (hit.1 - sy) * u,
            false,
            Pose::Dive,
        );
    }
    if t < T_FREE {
        return (hit.0, hit.1, false, Pose::Stuck);
    }
    if t < T_PERCH {
        let u = ease((t - T_FREE) / (T_PERCH - T_FREE));
        // A wobbly flight over to the post box.
        let x = hit.0 + (top.0 - hit.0) * u;
        let y = hit.1 + (top.1 - hit.1) * u - (u * PI).sin() * 8.0 + (t * 9.0).sin() * 1.5;
        return (x, y, false, Pose::Fly);
    }
    (top.0, top.1, false, Pose::Peer)
}

#[derive(Clone, Copy, PartialEq)]
enum Pose {
    Sit,
    Fly,
    Dive,
    Stuck,
    Peer,
}

fn pigeon(g: &mut World, k: &[(f64, f64, f64)], t: f64, f: u32) {
    let (x, y, left, pose) = pigeon_at(g, k, t);
    match pose {
        Pose::Sit | Pose::Peer => {
            let mut x2 = x;
            if pose == Pose::Peer {
                // Leaning over to peer into the slot; a leaf on its head.
                let bob = ((t - T_PERCH) * 2.0).sin().max(0.0);
                x2 -= bob;
                g.spr(PIGEON[1], PIGEONP, x2, y + bob, true);
                g.p(0x5ab04a, x2 + 2.0, y - 1.0 + bob, 2, 1);
                put(g, 0x3a8a3a, js_round(x2 + 3.0), js_round(y - 2.0 + bob));
            } else {
                g.spr(PIGEON[1], PIGEONP, x2, y, false);
            }
        }
        Pose::Fly | Pose::Dive => {
            let fr = if pose == Pose::Dive {
                1
            } else {
                ((f / 2) % 2) as usize
            };
            g.spr(PIGEON[fr], PIGEONP, x - 3.0, y - 2.0, left);
            if pose == Pose::Fly && t > T_FREE && t < T_PERCH {
                // Dizzy stars.
                let a = t * 8.0;
                put(
                    g,
                    0xfff060,
                    js_round(x + a.cos() * 4.0),
                    js_round(y - 4.0 + a.sin() * 1.5),
                );
            }
        }
        Pose::Stuck => {
            // Only tail feathers and feet sticking out of the leaves.
            let wig = if (f / 2).is_multiple_of(2) { 0.0 } else { 1.0 };
            g.p(0x4a5468, x - 9.0, y + wig, 3, 2);
            g.p(0x6a7488, x - 10.0, y + 1.0 + wig, 2, 1);
            put(g, 0xe08a3a, js_round(x - 7.0), js_round(y + 3.0));
            put(g, 0xe08a3a, js_round(x - 6.0), js_round(y + 3.0));
        }
    }
}

/// Feathers and leaves bursting from the tree on impact.
fn burst(g: &mut World, t: f64) {
    let e = t - T_CRASH;
    if !(0.0..2.4).contains(&e) {
        return;
    }
    let (hx, hy) = tree_hit(g);
    for k in 0..14 {
        let kf = k as f64;
        let a = rnd(kf + 500.0) * PI * 2.0;
        let sp = 8.0 + rnd(kf + 501.0) * 10.0;
        let x = hx - 8.0 + a.cos() * sp * e.min(0.5) * 2.0 + (e * 3.0 + kf).sin() * e * 2.0;
        let y = hy + a.sin() * sp * e.min(0.5) + e * e * 6.0;
        let c = if k % 3 == 0 {
            0xf0f0f8
        } else if k % 3 == 1 {
            0x6ac05a
        } else {
            0x4aa044
        };
        if y < 60.0 {
            put(g, c, js_round(x), js_round(y));
        }
    }
}

/// Two butterflies dancing over the hedge by the post box.
fn butterflies(g: &mut World, t: f64, f: u32) {
    let x0 = g.panel(mail(g)) as f64;
    for (k, c) in [(0, 0xf8d040), (1, 0xf0f0ff)] {
        let kf = k as f64;
        let x = x0 + 12.0 + kf * 30.0 + (t * 0.9 + kf * 2.0).sin() * 8.0;
        let y = 36.0 + (t * 1.7 + kf).sin() * 4.0 + (t * 4.3 + kf).cos();
        let open = (f / 2 + k).is_multiple_of(2);
        put(g, 0x3a2a2a, js_round(x), js_round(y));
        if open {
            g.p(c, x - 2.0, y - 1.0, 2, 2);
            g.p(c, x + 1.0, y - 1.0, 2, 2);
        } else {
            g.p(c, x - 1.0, y - 2.0, 1, 2);
            g.p(c, x + 1.0, y - 2.0, 1, 2);
        }
    }
}

/// Wind: white curls drifting with the breeze.
fn wind(g: &mut World, t: f64) {
    let w = g.w() as f64;
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let span = w + 60.0;
        let x0 = (rnd(kf + 600.0) * span + t * 26.0) % span - 30.0;
        let y0 = 8.0 + rnd(kf + 601.0) * 30.0;
        let life = ((t * 0.4 + rnd(kf + 602.0)) % 1.0 * PI).sin();
        for i in 0..14 {
            let fi = i as f64;
            let (x, y) = if i < 9 {
                (x0 + fi * 1.6, y0 + (fi * 0.5 + t * 2.0).sin() * 0.6)
            } else {
                let a = (fi - 9.0) * 0.9;
                (x0 + 14.4 + a.sin() * 2.5, y0 - 2.5 + a.cos() * 2.5)
            };
            blend(g, 0xffffff, js_round(x), js_round(y), 0.55 * life);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let k = keys(g);
    sky(g, t);
    skyline(g);
    wind(g, t);
    let rustle = if t > T_CRASH {
        (1.0 - (t - T_CRASH) / 0.8).max(0.0)
    } else {
        0.0
    };
    park_scene(g, t, rustle);
    street(g);
    butterflies(g, t, f);
    rooftops(g, t);
    cat(g, t, f);
    house(g, t, f);
    // A dotted trail behind the plane.
    if t > T_THROW && t < T_SLOT {
        for s in 1..6 {
            let q = t - s as f64 * 0.07;
            if q > T_THROW {
                let (tx, ty) = plane_at(g, &k, q);
                blend(
                    g,
                    0xffffff,
                    js_round(tx),
                    js_round(ty),
                    0.5 - s as f64 * 0.08,
                );
            }
        }
    }
    // The plane: held by the kid, then flying, then into the slot.
    let (px, py) = if t < T_THROW - 0.2 {
        let x0 = g.panel(0) as f64;
        let back = if t < T_THROW - 0.35 { -2.0 } else { 0.0 };
        (x0 + 40.0 + back, 26.0 - back)
    } else {
        plane_at(g, &k, t)
    };
    let (nx, ny) = plane_at(g, &k, (t + 0.05).max(T_THROW - 0.15));
    let heading = if t < T_THROW - 0.2 {
        -0.3
    } else if t >= T_SLOT {
        0.0
    } else {
        (ny - py).atan2(nx - px)
    };
    let flutter = if t < T_SLOT {
        (t * 9.0).sin() * 0.6
    } else {
        0.0
    };
    if t < T_IN + 0.05 {
        plane(g, px, py, heading, flutter);
    }
    pigeon(g, &k, t, f);
    post_box_draw(g, t, f);
    burst(g, t);
}

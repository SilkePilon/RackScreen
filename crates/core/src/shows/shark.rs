//! Beach day: umbrellas, a sand castle, gulls and sailboats on a sunny
//! shore. A dark fin cuts through the water towards the swimmers, who
//! flee up the sand screaming, screen by screen, until the fin surfaces:
//! a grinning kid in a duck ring with a toy fin on their head. Then a gull
//! steals the fin.

use super::fx::{blend, glow, line, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

const HORIZON: i32 = 22;
/// Where the sea meets the sand, give or take the wash.
const SHORE: i32 = 40;

const T_FIN: f64 = 0.4;
const T_STOP: f64 = 6.4;
const T_RISE: f64 = 6.7;
const T_HA: f64 = 7.4;
const T_GULL: f64 = 8.4;

const FIN: &[&str] = &["...k", "..kd", ".kdd", "kddd", "kddd"];
const FINP: &[(char, u32)] = &[('k', 0x2a3440), ('d', 0x4a5868)];

/// Swimmers, as (screen, offset, suit colour, hair colour).
const SWIMMERS: [(u32, f64, u32, u32); 4] = [
    (1, 20.0, 0xe04a4a, 0x3a2414),
    (1, 40.0, 0x3a7ae0, 0xe0c060),
    (2, 16.0, 0x2ab070, 0x1a1010),
    (2, 38.0, 0xe07ab0, 0x8a4a1a),
];

const SKIN: u32 = 0xe8b088;

fn sky(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(g, &[0x4aa0e6, 0x7cc4f2, 0xbfe6fa], 0, w, 0, HORIZON);
    let sx = (g.px0(0) + 16) as f64;
    glow(g, 0xfff8c0, sx, 7.0, 14.0, 0.6);
    g.disc(0xfffbe0, sx, 7.0, 4);
    // Fluffy clouds, two layers drifting at different speeds.
    for (n, sp, c, shade, y0) in [
        (5u32, 1.2, 0xe8f4fc, 0xb8d8f0, 4.0),
        (4, 2.4, 0xffffff, 0xc8e0f4, 10.0),
    ] {
        for k in 0..g.spread(n) {
            let kf = k as f64 + sp;
            let x = (rnd(kf) * w as f64 + t * sp) % (w as f64 + 30.0) - 15.0;
            let y = y0 + rnd(kf + 1.0) * 4.0;
            g.disc(shade, x, y + 1.0, 3);
            g.disc(c, x - 3.0, y + 1.0, 2);
            g.disc(c, x, y, 3);
            g.disc(c, x + 4.0, y + 1.0, 2);
            g.p(shade, x - 4.0, y + 3.0, 10, 1);
        }
    }
    // Gulls.
    for k in 0..g.spread(4) {
        let kf = k as f64;
        let x = (rnd(kf + 9.0) * w as f64 + t * 6.0) % (w as f64 + 10.0) - 5.0;
        let y = 5.0 + rnd(kf + 3.0) * 10.0 + (t * 2.0 + kf).sin() * 1.5;
        gull(g, x, y, (f / 3 + k).is_multiple_of(2));
    }
}

fn gull(g: &mut World, x: f64, y: f64, up: bool) {
    let c = 0x3a4450;
    put(g, 0xffffff, js_round(x), js_round(y));
    if up {
        put(g, c, js_round(x - 1.0), js_round(y - 1.0));
        put(g, c, js_round(x + 1.0), js_round(y - 1.0));
        put(g, c, js_round(x - 2.0), js_round(y - 1.0));
        put(g, c, js_round(x + 2.0), js_round(y - 1.0));
    } else {
        put(g, c, js_round(x - 1.0), js_round(y));
        put(g, c, js_round(x + 1.0), js_round(y));
        put(g, c, js_round(x - 2.0), js_round(y + 1.0));
        put(g, c, js_round(x + 2.0), js_round(y + 1.0));
    }
}

fn sea(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x1e6aa8, 0x2a86c0, 0x3aa4d0, 0x5ac0d8],
        0,
        w,
        HORIZON,
        SHORE + 2,
    );
    g.p(0x184e88, 0.0, HORIZON as f64, w, 1);
    // Far boats: a sailboat on screen 1, a little steamer on screen 3.
    let bx = (g.panel(1) + 10) as f64 + t * 0.8;
    g.p(0x8a4a2a, bx, (HORIZON - 1) as f64, 7, 2);
    for k in 0..6 {
        g.p(0xffffff, bx + 3.0, (HORIZON - 2 - k) as f64, 1 + k / 2, 1);
    }
    g.p(0x5a3a2a, bx + 2.0, (HORIZON - 8) as f64, 1, 7);
    let sx = (g.panel(3) + 30) as f64 - t * 0.5;
    g.p(0x3a3a4a, sx, (HORIZON - 2) as f64, 10, 2);
    g.p(0xe8e8f0, sx + 2.0, (HORIZON - 4) as f64, 5, 2);
    g.p(0xd04040, sx + 4.0, (HORIZON - 6) as f64, 2, 2);
    let puff = (t * 1.5) % 1.0;
    g.disc(
        0xe8f0f8,
        sx + 5.0 - puff * 4.0,
        HORIZON as f64 - 8.0 - puff * 3.0,
        1,
    );
    // Glints on the water.
    for k in 0..g.spread(60) {
        let kf = k as f64;
        let y = HORIZON + 1 + (rnd(kf) * (SHORE - HORIZON - 3) as f64) as i32;
        let x = (rnd(kf + 1.0) * w as f64 + (t * 0.7 + kf).sin() * 2.0).floor();
        if (f / 2 + k) % 6 < 4 {
            let len = 1 + (y - HORIZON) / 6;
            g.p(
                if (f / 2 + k).is_multiple_of(6) {
                    0xffffff
                } else {
                    0x8ad8ec
                },
                x,
                y as f64,
                len,
                1,
            );
        }
    }
}

fn beach(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0xe8c880, 0xf0d898, 0xf4e0a8, 0xe0c080],
        0,
        w,
        SHORE,
        60,
    );
    // Wet sand and the wash running up and back.
    let wash = (t * 0.9).sin() * 1.5;
    for x in 0..w {
        let xf = x as f64;
        let edge = SHORE as f64 + 1.0 + wash + (xf * 0.15 + t).sin() * 0.8;
        let ei = js_round(edge);
        for y in SHORE - 1..ei {
            put(g, 0x7ad0dc, x, y);
        }
        put(g, 0xffffff, x, ei);
        put(g, 0xc8a868, x, ei + 1);
        if (x + ei) % 2 == 0 {
            put(g, 0xd0b070, x, ei + 2);
        }
        // Foam crests rolling in.
        for k in 0..2 {
            let ph = (t * 0.5 + k as f64 * 0.5) % 1.0;
            let y = HORIZON as f64 + 8.0 + ph * (SHORE - HORIZON - 9) as f64;
            let yy = js_round(y + (xf * 0.2 + k as f64).sin());
            if ((xf * 0.11 + k as f64 * 3.0 + t * 0.6).sin()) > -0.2 {
                put(g, 0xe8f8ff, x, yy);
            }
        }
        // Sand speckle.
        if rnd(xf * 1.3) > 0.7 {
            put(g, 0xd0a868, x, 45 + (x * 7) % 13);
        }
    }
    let _ = f;
}

fn umbrella(g: &mut World, x: f64, a: u32, b: u32) {
    g.p(0x6a4a2a, x, 30.0, 1, 22);
    for k in 0..7 {
        let half = 1 + k * 2;
        for dx in -half..=half {
            let c = if ((dx + 20) / 3) % 2 == 0 { a } else { b };
            put(g, c, js_round(x) + dx, 28 + k);
        }
    }
    // Shade on the sand.
    for dx in -10..=10 {
        for dy in 0..3 {
            if (dx + dy) % 2 == 0 {
                blend(g, 0x8a6a3a, js_round(x) + dx + 4, 52 + dy, 0.35);
            }
        }
    }
}

fn props(g: &mut World, t: f64) {
    let n = g.layout.panels;
    // Umbrellas and towels.
    let p0 = g.px0(0) as f64;
    umbrella(g, p0 + 38.0, 0xe84a4a, 0xffffff);
    g.p(0x4a8ae0, p0 + 26.0, 50.0, 12, 3);
    g.p(0xffffff, p0 + 26.0, 51.0, 12, 1);
    // A cooler and a bucket.
    g.p(0x2a7ad8, p0 + 46.0, 46.0, 6, 4);
    g.p(0xffffff, p0 + 46.0, 46.0, 6, 1);
    if n > 1 {
        let p1 = g.px0(1) as f64;
        // A striped beach ball rocking in the breeze.
        let bx = p1 + 46.0 + (t * 1.2).sin();
        g.disc(0xffffff, bx, 48.0, 2);
        g.p(0xe84a4a, bx - 2.0, 47.0, 2, 3);
        g.p(0x3a7ae0, bx + 1.0, 47.0, 2, 3);
        g.p(0xe0c040, p1 + 8.0, 50.0, 3, 3);
        g.p(0xc04040, p1 + 9.0, 48.0, 1, 2);
    }
    if n > 2 {
        // A sand castle with a fluttering flag.
        let cx = g.px0(2) as f64 + 44.0;
        g.p(0xc8a060, cx - 6.0, 46.0, 13, 6);
        g.p(0xd8b070, cx - 6.0, 46.0, 13, 1);
        g.p(0xc8a060, cx - 2.0, 41.0, 5, 5);
        g.p(0xd8b070, cx - 2.0, 41.0, 5, 1);
        for k in 0..3 {
            g.p(0xc8a060, cx - 6.0 + k as f64 * 6.0, 44.0, 2, 2);
        }
        g.p(0x8a6a3a, cx - 1.0, 49.0, 2, 3);
        g.p(0x5a3a2a, cx, 36.0, 1, 5);
        let fl = (t * 6.0).sin() > 0.0;
        g.p(0xe84a4a, cx + 1.0, 36.0, 3, 1);
        g.p(0xe84a4a, cx + 1.0, 37.0, if fl { 2 } else { 3 }, 1);
        // A spade stuck in the sand.
        g.p(0x3a7ae0, cx + 10.0, 44.0, 1, 5);
        g.p(0x3a7ae0, cx + 9.0, 49.0, 3, 2);
    }
    if n > 3 {
        let p3 = g.px0(3) as f64;
        umbrella(g, p3 + 50.0, 0x3a7ae0, 0xffe060);
        g.p(0xe07ab0, p3 + 36.0, 52.0, 10, 3);
        g.p(0xffffff, p3 + 36.0, 53.0, 10, 1);
    }
}

/// The swimmer `i` at time `t` given when they panic.
fn swimmer(g: &mut World, who: (usize, f64, u32, u32), panic: f64, t: f64, f: u32) {
    let (i, x0, suit, hair) = who;
    let bob = ((t * 3.0 + i as f64).sin() * 0.7).round();
    if panic < 0.0 {
        // Bobbing happily, an arm waving now and then.
        let y = 33.0 + bob;
        g.p(SKIN, x0, y, 3, 3);
        g.p(hair, x0, y, 3, 1);
        g.p(0x2a1a10, x0 + 1.0, y + 1.0, 1, 1);
        if ((t * 1.5 + i as f64) as i32) % 3 == 0 {
            g.p(SKIN, x0 + 3.0, y - 2.0, 1, 3);
        }
        g.p(0xe8f8ff, x0 - 1.0, y + 3.0, 5, 1);
        return;
    }
    let dir = if i.is_multiple_of(2) { -1.0 } else { 1.0 };
    let u = (panic / 1.4).min(1.0);
    let x = x0 + dir * ease(u) * 5.0;
    let y = 33.0 + ease(u) * 14.0;
    if panic < 0.5 {
        // Scream and splash.
        let jump = (panic / 0.5 * PI).sin() * 3.0;
        g.p(SKIN, x0, 33.0 - jump, 3, 3);
        g.p(hair, x0, 33.0 - jump, 3, 1);
        g.p(0x2a1a10, x0 + 1.0, 35.0 - jump, 1, 1);
        g.p(SKIN, x0 - 1.0, 30.0 - jump, 1, 3);
        g.p(SKIN, x0 + 3.0, 30.0 - jump, 1, 3);
        for k in 0..6 {
            let a = PI + k as f64 / 5.0 * PI;
            put(
                g,
                0xffffff,
                js_round(x0 + 1.0 + a.cos() * (3.0 + panic * 6.0)),
                js_round(36.0 + a.sin() * (2.0 + panic * 4.0)),
            );
        }
        g.p(0xffffff, x0 + 1.0, 22.0, 1, 3);
        put(g, 0xffffff, js_round(x0 + 1.0), 26);
        return;
    }
    // Running up the beach, arms flailing.
    let legs = (f / 2).is_multiple_of(2);
    let (xi, yi) = (x, y - 8.0);
    g.p(hair, xi, yi, 3, 1);
    g.p(SKIN, xi, yi + 1.0, 3, 2);
    g.p(0x2a1a10, xi + 1.0, yi + 2.0, 1, 1);
    g.p(SKIN, xi, yi + 3.0, 3, 2);
    g.p(suit, xi, yi + 5.0, 3, 2);
    let arm = if legs { -2.0 } else { -1.0 };
    g.p(SKIN, xi - 1.0, yi + 3.0 + arm, 1, 2);
    g.p(SKIN, xi + 3.0, yi + 2.0 - arm * 0.5, 1, 2);
    if legs {
        g.p(SKIN, xi - 1.0, yi + 7.0, 1, 2);
        g.p(SKIN, xi + 3.0, yi + 7.0, 1, 2);
    } else {
        g.p(SKIN, xi, yi + 7.0, 1, 2);
        g.p(SKIN, xi + 2.0, yi + 7.0, 1, 2);
    }
    // Water drips off as they leave the sea.
    if panic < 1.2 {
        put(
            g,
            0x8ad8ec,
            js_round(xi - dir),
            js_round(yi + 6.0 + (panic * 20.0) % 3.0),
        );
    }
    if panic < 2.4 && (f / 2).is_multiple_of(2) {
        g.p(0xffffff, xi + 1.0, yi - 5.0, 1, 3);
        put(g, 0xffffff, js_round(xi + 1.0), js_round(yi - 1.0));
    }
}

/// 3×5 pixel letters for "HA".
fn ha(g: &mut World, x: f64, y: f64, c: u32) {
    let rows = ["#.#.###", "#.#.#.#", "###.###", "#.#.#.#", "#.#.#.#"];
    for (r, row) in rows.iter().enumerate() {
        for (k, ch) in row.chars().enumerate() {
            if ch == '#' {
                put(g, c, js_round(x) + k as i32, js_round(y) + r as i32);
            }
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, f, t);
    sea(g, f, t);

    // The fin glides in from screen 0 to just off the last screen's shore.
    let v0 = stage_at(g, 0, 4.0);
    let v1 = stage_at(g, 3, 20.0);
    let fin_v = |t: f64| v0 + (v1 - v0) * ((t - T_FIN) / (T_STOP - T_FIN)).clamp(0.0, 1.0);
    let fx_now = stage_x(g, fin_v(t));
    let fin_y = 31.0 + (t * 2.0).sin() * 0.5;

    beach(g, f, t);
    props(g, t);

    // Swimmers panic as the fin comes within reach.
    for (i, &(p, off, suit, hair)) in SWIMMERS.iter().enumerate() {
        if p >= g.layout.panels {
            continue;
        }
        let s = stage_at(g, p, off);
        let t_panic = T_FIN + (s - 22.0 - v0) / (v1 - v0) * (T_STOP - T_FIN);
        let x = g.px0(p) as f64 + off;
        swimmer(g, (i, x, suit, hair), t - t_panic, t, f);
    }

    if t < T_RISE {
        // Its wake: a V of foam trailing behind.
        for k in 1..12 {
            let kf = k as f64;
            let x = stage_x(g, fin_v(t) - kf * 1.5);
            if t > T_FIN && fin_v(t) - kf * 1.5 > v0 {
                put(g, 0xe8f8ff, js_round(x), js_round(fin_y + 4.0 + kf * 0.25));
                if k % 2 == 0 {
                    put(g, 0xb8e8f4, js_round(x), js_round(fin_y + 4.0 - kf * 0.15));
                }
            }
        }
        let sink = if t > T_STOP { (t - T_STOP) * 6.0 } else { 0.0 };
        g.spr(FIN, FINP, fx_now - 2.0, fin_y - 1.0 + sink, false);
        g.p(0xe8f8ff, fx_now - 3.0, fin_y + 4.0, 7, 1);
        // Ominous dark water under it.
        for dx in -4..=4 {
            blend(
                g,
                0x0a2a4a,
                js_round(fx_now) + dx,
                js_round(fin_y + 5.0),
                0.4,
            );
        }
    } else {
        // The reveal: a kid in a duck ring with a toy fin strapped on.
        let rise = ease((t - T_RISE) / 0.5);
        let (kx, ky) = (fx_now, 34.0 - rise * 4.0 + ((t * 3.0).sin() * 0.6).round());
        // Splash ring as they pop up.
        if t < T_RISE + 0.6 {
            let u = (t - T_RISE) / 0.6;
            for k in 0..10 {
                let a = PI + k as f64 / 9.0 * PI;
                put(
                    g,
                    0xffffff,
                    js_round(kx + a.cos() * (4.0 + u * 8.0)),
                    js_round(36.0 + a.sin() * (2.0 + u * 6.0) + u * u * 4.0),
                );
            }
        }
        // Kid: head with snorkel mask, arms up.
        let arms_up = t < T_GULL + 0.6;
        g.p(0x5a3a1a, kx - 1.0, ky - 4.0, 4, 1);
        g.p(SKIN, kx - 1.0, ky - 3.0, 4, 4);
        g.p(0x4ad0f0, kx - 1.0, ky - 2.0, 4, 1);
        g.p(0x2a3440, kx + 2.0, ky - 2.0, 1, 1);
        g.p(0xffe040, kx + 3.0, ky - 6.0, 1, 4);
        if t < T_GULL + 0.6 && (T_HA..T_GULL).contains(&t) {
            g.p(0x8a2a2a, kx, ky, 2, 1);
        }
        g.p(SKIN, kx - 1.0, ky + 1.0, 4, 3);
        if arms_up {
            g.p(SKIN, kx - 3.0, ky - 4.0, 1, 4);
            g.p(SKIN, kx + 4.0, ky - 4.0, 1, 4);
        } else {
            g.p(SKIN, kx - 2.0, ky + 1.0, 1, 3);
            g.p(SKIN, kx + 3.0, ky + 1.0, 1, 3);
        }
        // The duck ring.
        g.p(0xf0c020, kx - 5.0, ky + 3.0, 13, 3);
        g.p(0xffe060, kx - 5.0, ky + 3.0, 13, 1);
        g.p(0xf0c020, kx + 6.0, ky - 2.0, 3, 5);
        g.p(0xffe060, kx + 6.0, ky - 2.0, 3, 1);
        g.p(0xff8a20, kx + 9.0, ky - 1.0, 2, 1);
        put(g, 0x1a1a1a, js_round(kx + 7.0), js_round(ky - 1.0));
        g.p(0xe8f8ff, kx - 6.0, ky + 6.0, 15, 1);
        // The toy fin on their head, until a gull takes it.
        let gu = ((t - T_GULL) / 1.4).clamp(0.0, 1.0);
        if t < T_GULL + 0.5 {
            g.spr(FIN, FINP, kx - 1.0, ky - 9.0, false);
            g.p(0x2a3440, kx - 1.0, ky - 4.0, 4, 1);
        } else {
            let u = (t - T_GULL - 0.5) / 1.0;
            let (fx2, fy2) = (kx - 1.0 - u * 30.0, ky - 9.0 - u * 26.0 + u * u * 4.0);
            g.spr(FIN, FINP, fx2, fy2 + 2.0, false);
        }
        if t > T_GULL {
            // The gull swoops in from the right and makes off with it.
            let (gx, gy) = if gu < 0.36 {
                let u = gu / 0.36;
                (kx + 30.0 - u * 30.0, ky - 30.0 + u * 20.0)
            } else {
                let u = (t - T_GULL - 0.5) / 1.0;
                (kx - u * 30.0, ky - 10.0 - u * 26.0 + u * u * 4.0)
            };
            let up = (f / 2).is_multiple_of(2);
            for s in [-1.0, 1.0] {
                line(
                    g,
                    0x3a4450,
                    gx,
                    gy,
                    gx + s * 4.0,
                    gy + if up { -3.0 } else { 1.0 },
                );
            }
            g.p(0xffffff, gx - 1.0, gy, 3, 2);
            put(g, 0xff9a2a, js_round(gx - 2.0), js_round(gy));
        }
        // HA HA!
        if (T_HA..T_GULL + 0.2).contains(&t) {
            let j = ((f / 3) % 2) as f64;
            ha(g, kx - 9.0, ky - 15.0 - j, 0xffffff);
            ha(g, kx + 3.0, ky - 13.0 + j, 0xffffff);
        }
        // Then a single drop of sweat: "aw".
        if t > T_GULL + 1.2 {
            put(g, 0x8ad8ff, js_round(kx + 4.0), js_round(ky - 4.0));
            put(g, 0x8ad8ff, js_round(kx + 4.0), js_round(ky - 3.0));
        }
    }
}

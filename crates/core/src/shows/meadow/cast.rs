//! The cast of the bee's day: the bee itself, its slim friend, the frog
//! and the butterflies.

use super::super::fx::{blend, line, mix, on, put, stage_at, stage_x};
use super::super::world::{ease, js_round, rnd, World};
use super::*;
use std::f64::consts::PI;

const STRIPE: u32 = 0xf8c020;
const BAND: u32 = 0x3a2818;
const POLLEN: u32 = 0xffd23a;
const BASKET: u32 = 0xf0a020;

/// The frog facing left; its mouth corner is the first pixel of row 5.
const FROG: &[&str] = &[
    "...kk..kk.....",
    "..kwek.kwek...",
    "..kgggkgggk...",
    ".kgggggggggk..",
    "kgggggggggggk.",
    "kkkkkggggggGGk",
    ".kllllgggGGGGk",
    "..kkllkGGGGkk.",
    ".kgkk.kggkgk..",
];
const FROGP: &[(char, u32)] = &[
    ('k', OUT),
    ('w', 0xffffff),
    ('e', OUT),
    ('g', 0x5ab040),
    ('G', 0x3a8030),
    ('l', 0xd8e8a0),
];

struct Bee {
    x: f64,
    y: f64,
    /// Pollen load, `0..1`: yellower and rounder.
    p: f64,
    left: bool,
    /// World x beyond which the bee is inside the hive.
    clip: f64,
    /// Wedged in the door, legs kicking.
    stuck: bool,
}

fn rx(p: f64) -> f64 {
    3.0 + 1.8 * p
}

fn ry(p: f64) -> f64 {
    2.2 + 1.5 * p
}

fn pollen(vs: &[Visit], t: f64) -> f64 {
    vs.iter().map(|v| smooth(v.t0, v.t1, t)).sum::<f64>() / vs.len() as f64
}

/// Where the bee sits on visit `v`: (stage, y).
fn seat(g: &World, v: &Visit, t: f64, p: f64) -> (f64, f64) {
    let (hx, hy) = head_at(g, v.s, v.root, v.stem, t);
    (v.s + hx - stage_x(g, v.s), hy - 1.5 - ry(p))
}

/// The stuck bee's centre, front half through the door.
fn wedged(g: &World) -> f64 {
    door(g).0 + 0.5
}

/// The bee at time `t`, before the clip and facing.
fn bee_at(g: &World, t: f64) -> (f64, f64, f64, f64, bool) {
    let vs = visits(g);
    let p = pollen(&vs, t);
    let wob = (t * 7.0).sin() * 0.8;
    let none = f64::INFINITY;
    let v0 = &vs[0];
    if t < v0.t0 {
        let (s1, y1) = seat(g, v0, t, p);
        let u = ease(t / v0.t0);
        let s = -14.0 + (s1 + 14.0) * u;
        return (stage_x(g, s), 22.0 + (y1 - 22.0) * u + wob, p, none, false);
    }
    let n = vs.len();
    for k in 0..n {
        let v = &vs[k];
        if t < v.t1 {
            let (s, y) = seat(g, v, t, p);
            return (stage_x(g, s), y + (t * 8.0).sin() * 0.4, p, none, false);
        }
        if k + 1 < n && t < vs[k + 1].t0 {
            let (sa, ya) = seat(g, v, t, p);
            let (sb, yb) = seat(g, &vs[k + 1], t, p);
            let u = ease((t - v.t1) / (vs[k + 1].t0 - v.t1));
            let y = ya + (yb - ya) * u - 8.0 * (u * PI).sin() + wob * (u * PI).sin();
            return (stage_x(g, sa + (sb - sa) * u), y, p, none, false);
        }
    }
    // The frog strikes: straight up out of reach.
    let (s0, y0) = seat(g, &vs[n - 1], T_STRIKE, p);
    let jitter = (t * 40.0).sin() * 0.6 * (1.0 - smooth(T_STRIKE, T_LEAVE, t));
    let up = (s0, y0 - 13.0 * smooth(T_STRIKE, T_STRIKE + 0.12, t));
    if t < T_LEAVE {
        return (stage_x(g, up.0) + jitter, up.1 + wob, p, none, false);
    }
    let (dx, dy) = door(g);
    let sd = stage_at(g, last(g), dx - g.panel(last(g)) as f64 - rx(p) - 2.0);
    if t < T_DOOR {
        // Heavy now: it sags between wingbeats.
        let u = ease((t - T_LEAVE) / (T_DOOR - T_LEAVE));
        let y = up.1 + (dy - up.1) * u + 6.0 * (u * PI).sin() + (t * 5.0).sin() * 1.5;
        return (stage_x(g, up.0 + (sd - up.0) * u), y, p, none, false);
    }
    let x0 = stage_x(g, sd);
    let clip = dx - 0.5;
    if t < T_STUCK {
        let u = smooth(T_DOOR, T_STUCK, t);
        return (x0 + (wedged(g) - x0) * u, dy, p, clip, false);
    }
    if t < T_BUMP {
        // Bottom wiggling, trying to squeeze in.
        let w = (t * 22.0).sin();
        return (
            wedged(g) + w.round() * 0.6,
            dy + (t * 11.0).cos().round() * 0.6,
            p,
            clip,
            true,
        );
    }
    let u = smooth(T_BUMP, T_POP, t);
    (wedged(g) + rx(p) * 2.6 * u, dy, p, clip, false)
}

fn bee(g: &World, t: f64) -> Bee {
    let (x, y, p, clip, stuck) = bee_at(g, t);
    let ahead = bee_at(g, t + 0.08).0;
    Bee {
        x,
        y,
        p,
        left: ahead < x - 0.3,
        clip,
        stuck,
    }
}

/// A bee: striped body (rounder with `p`), wings, legs and pollen.
#[allow(clippy::too_many_arguments)]
fn bee_body(g: &mut World, x: f64, y: f64, p: f64, left: bool, f: u32, clip: f64, kick: bool) {
    let (rx, ry) = (rx(p), ry(p));
    let dir = if left { -1 } else { 1 };
    let (xi, yi) = (js_round(x), js_round(y));
    let dot = |g: &mut World, c: u32, dx: i32, dy: i32| {
        let px = xi + dx * dir;
        if (px as f64) <= clip {
            put(g, c, px, yi + dy);
        }
    };
    // Legs, and the pollen baskets swelling on them.
    let ly = ry.ceil() as i32;
    for (k, lx) in [-2, 0, 2].into_iter().enumerate() {
        let kk = if kick && (f + k as u32).is_multiple_of(2) {
            1
        } else {
            0
        };
        dot(g, OUT, lx - kk, ly);
        dot(g, OUT, lx - 1 - kk, ly + kk);
    }
    let b = (p * 2.2) as i32;
    for dy in 0..b {
        for dx in -b..=b / 2 {
            dot(
                g,
                if dy == 0 && dx < 0 { 0xffe080 } else { BASKET },
                dx - 1,
                ly + dy,
            );
        }
    }
    let (wx, wy) = (rx.ceil() as i32 + 1, ry.ceil() as i32 + 1);
    for dy in -wy..=wy {
        for dx in -wx..=wx {
            let (u, v) = (dx as f64 / rx, dy as f64 / ry);
            let e = u * u + v * v;
            if e > 1.5 {
                continue;
            }
            let px = xi + dx * dir;
            let mut c = if e > 1.0 {
                OUT
            } else if dx as f64 > rx - 2.2 {
                if dx as f64 > rx - 1.5 && dy == -1 {
                    0xffffff
                } else {
                    BAND
                }
            } else if ((rx - 2.2 - dx as f64) / 1.7).floor() as i32 % 2 == 0 {
                STRIPE
            } else {
                BAND
            };
            if c != OUT && c != 0xffffff {
                if on(px, yi + dy, p * 0.6) && c == BAND && dx as f64 <= rx - 2.2 {
                    c = mix(BAND, POLLEN, 0.8);
                } else if c == STRIPE && on(px + 1, yi + dy, p * 0.7) {
                    c = POLLEN;
                }
                if v > 0.45 {
                    c = mix(c, OUT, 0.3);
                } else if v < -0.5 {
                    c = mix(c, 0xffffff, 0.3);
                }
            }
            dot(g, c, dx, dy);
        }
    }
    dot(g, OUT, -(rx.ceil() as i32) - 1, 0);
    // Wings beating: up, then swept back. Frantic when stuck.
    let wing = 0xe4f2ff;
    let pts: &[(i32, i32)] = if f.is_multiple_of(2) || kick {
        &[
            (0, -1),
            (-1, -1),
            (-1, -2),
            (-2, -2),
            (-1, -3),
            (-2, -3),
            (1, -1),
            (0, -2),
        ]
    } else {
        &[
            (-1, -1),
            (-2, -1),
            (-3, -1),
            (-2, 0),
            (-3, 0),
            (-4, 0),
            (0, -1),
        ]
    };
    let top = -(ry.ceil() as i32);
    for &(dx, dy) in pts {
        let px = xi + dx * dir;
        if (px as f64) <= clip {
            blend(g, wing, px, yi + top + dy, 0.8);
        }
    }
}

pub(super) fn draw_bee(g: &mut World, t: f64, f: u32) {
    if t >= T_POP {
        // A puff of pollen out of the door as it goes in.
        let u = (t - T_POP) / 0.8;
        if u < 1.0 {
            let (dx, dy) = door(g);
            for k in 0..14 {
                let kf = k as f64;
                let a = PI * (0.5 + rnd(kf + 800.0));
                let sp = 3.0 + rnd(kf + 801.0) * 7.0;
                let x = dx + a.cos() * sp * u.sqrt();
                let y = dy + a.sin() * sp * u.sqrt() * 0.7 + 6.0 * u * u;
                blend(g, POLLEN, js_round(x), js_round(y), 1.0 - u);
            }
        }
        // Home: a little heart floats up from the hive.
        let u = (t - 11.2) / 0.8;
        if (0.0..1.0).contains(&u) {
            let (dx, dy) = door(g);
            let (hx, hy) = (dx - 5.0 + (u * 9.0).sin(), dy - 6.0 - u * 9.0);
            let rows = ["pp.pp", "ppppp", ".ppp.", "..p.."];
            g.spr(&rows, &[('p', 0xf06890)], hx - 2.0, hy, false);
            put(g, 0xffc0d0, js_round(hx) - 1, js_round(hy));
        }
        return;
    }
    let b = bee(g, t);
    bee_body(g, b.x, b.y, b.p, b.left, f, b.clip, b.stuck);
    if b.stuck {
        // Sweat drops and a strained buzz.
        let (rx, ry) = (rx(b.p), ry(b.p));
        let u = (t * 2.0) % 1.0;
        blend(
            g,
            0x9ad8ff,
            js_round(b.x - rx * 0.4),
            js_round(b.y - ry - 2.0 + u * 3.0),
            1.0 - u,
        );
        for k in 0..3 {
            let a = t * 10.0 + k as f64 * 2.1;
            blend(
                g,
                0xffffff,
                js_round(b.x - rx - 2.0 + a.cos() * 1.5),
                js_round(b.y + a.sin() * 3.0),
                0.5,
            );
        }
    }
}

/// The slim friend who headbutts the stuck bee in, then slips in itself.
pub(super) fn draw_helper(g: &mut World, t: f64, f: u32) {
    let (dx, dy) = door(g);
    let p = pollen(&visits(g), T_DOOR);
    let target = wedged(g) - rx(p) - rx(0.0) - 1.0;
    let start = g.panel(last(g)) as f64 - 8.0;
    let (x, y, clip) = if t < 9.2 {
        return;
    } else if t < T_BUMP {
        let u = ((t - 9.2) / (T_BUMP - 9.2)).powf(1.6);
        (
            start + (target - start) * u,
            24.0 + (dy - 24.0) * u,
            f64::INFINITY,
        )
    } else if t < 10.7 {
        let u = smooth(T_BUMP, T_BUMP + 0.3, t);
        let back = 4.0 * (u * PI).sin();
        (target - back, dy + (t * 6.0).sin() * 0.8, f64::INFINITY)
    } else if t < 11.3 {
        let u = smooth(10.7, 11.3, t);
        (target + (dx + 6.0 - target) * u, dy, dx + 1.0)
    } else {
        return;
    };
    bee_body(g, x, y, 0.0, false, f, clip, false);
    if (T_BUMP..T_BUMP + 0.25).contains(&t) {
        for (ox, oy) in [(2, -4), (4, -3), (3, -6)] {
            put(g, 0xffffff, js_round(x) + ox, js_round(y) + oy);
        }
    }
}

pub(super) fn draw_frog(g: &mut World, t: f64, f: u32) {
    let (mx, my) = mouth(g);
    // The lily pad, rocking on the ripples.
    let bob = ((t * 2.0).sin() * 0.5).round();
    let (px, py) = (mx + 7.0, my + 4.0 + bob);
    for y in -2..=2 {
        for x in -9..=9 {
            let e = (x as f64 / 9.0).powi(2) + (y as f64 / 2.2).powi(2);
            if e < 1.0 && !(y < 0 && (-1..=1).contains(&(x + 5))) {
                let c = if y > 0 { 0x2a7a3a } else { 0x46a050 };
                put(g, c, js_round(px) + x, js_round(py) + y);
            }
        }
    }
    let (fx, fy) = (mx, my - 5.0 + bob);
    g.spr(FROG, FROGP, fx, fy, false);
    let (xi, yi) = (js_round(fx), js_round(fy));
    // Blinks, and the throat pulsing.
    if f % 40 < 2 || (T_STRIKE + 0.5..T_STRIKE + 0.7).contains(&t) {
        for ex in [3, 8] {
            g.p(0x5ab040, (xi + ex) as f64, (yi + 1) as f64, 2, 1);
        }
    }
    if (f / 4).is_multiple_of(3) && t < T_STRIKE {
        g.p(0xe8f0b8, (xi + 2) as f64, (yi + 7) as f64, 3, 1);
    }
    // The tongue.
    let ext = tongue(t);
    if ext > 0.0 {
        let (cx, cy) = clock(g);
        let (tx, ty) = (mx + (cx - mx) * ext, my + (cy - my) * ext);
        line(g, 0xe86080, mx, my, tx, ty);
        line(g, 0xc84060, mx, my + 1.0, tx, ty + 1.0);
        g.disc(0xff8aa0, tx, ty, 1);
    }
    // A fluffy moustache of dandelion seeds after.
    if t > T_STRIKE + 0.4 {
        for (ox, oy) in [(-1, 0), (0, 1), (1, 0), (2, 1), (-1, 2), (3, 0), (1, 2)] {
            put(g, 0xffffff, xi + ox, yi + 5 + oy);
        }
        blend(g, 0xd8d8c8, xi - 2, yi + 4, 0.8);
    }
}

pub(super) fn draw_butterflies(g: &mut World, t: f64, f: u32) {
    let spots = [
        (0u32, 34.0, 26.0, 0xf8f8f0, 0x2a2a2a),
        (1, 28.0, 32.0, 0xf08830, 0x3a1a10),
        (last(g), 16.0, 44.0, 0x5a9af0, 0x1a3a7a),
    ];
    for (k, &(panel, lx, cy, wing, edge)) in spots.iter().enumerate() {
        let kf = k as f64 * 2.3;
        let x =
            g.panel(panel) as f64 + lx + 12.0 * (t * 0.6 + kf).sin() + 3.0 * (t * 1.7 + kf).sin();
        let y = cy + 5.0 * (t * 0.9 + kf).sin() + 1.5 * (t * 8.0 + kf).sin();
        let (xi, yi) = (js_round(x), js_round(y));
        let open = (f / 2 + k as u32).is_multiple_of(2);
        if open {
            for (dx, dy) in [
                (-2, -1),
                (-1, -1),
                (-2, 0),
                (-1, 0),
                (1, -1),
                (2, -1),
                (1, 0),
                (2, 0),
            ] {
                put(g, wing, xi + dx, yi + dy);
            }
            for (dx, dy) in [(-3, -1), (3, -1), (-1, 1), (1, 1)] {
                put(g, edge, xi + dx, yi + dy);
            }
        } else {
            for (dx, dy) in [(-1, -2), (0, -2), (-1, -3), (0, -3), (1, -2)] {
                put(g, wing, xi + dx, yi + dy);
            }
            put(g, edge, xi, yi - 4);
        }
        put(g, OUT, xi, yi - 1);
        put(g, OUT, xi, yi);
        if rnd((f / 6 + k as u32) as f64) > 0.5 {
            blend(g, 0xffffff, xi + 1, yi - 3, 0.3);
        }
    }
}

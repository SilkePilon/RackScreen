//! Monster in the harbour: a harbour city at dusk, cranes and a container
//! ship, lit windows shimmering on the water. Bubbles, then a giant lizard
//! rises, wades across, roars until the screens shake. Tiny jets buzz its
//! head; it sneezes one clean out of the sky (the pilot pops a parachute).

use super::fx::{self, blend, glow, line, put, shake, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.0;

/// The harbour's waterline.
const WATER: i32 = 42;

const T_RISE: f64 = 1.6;
const T_UP: f64 = 3.4;
const T_WADE: f64 = 3.6;
const T_STAND: f64 = 5.6;
const T_ROAR: f64 = 5.8;
const T_HUSH: f64 = 7.0;
const T_JETS: f64 = 6.6;
const T_AH: f64 = 8.3;
const T_CHOO: f64 = 9.1;

const OUT: u32 = 0x0c1a16;
const SKIN: u32 = 0x2e6e4e;
const SKIN_L: u32 = 0x4a9060;
const BELLY: u32 = 0xa8c070;

/// A filled ellipse.
fn oval(g: &mut World, c: u32, cx: f64, cy: f64, rx: f64, ry: f64) {
    let y0 = (cy - ry).floor() as i32;
    let y1 = (cy + ry).ceil() as i32;
    for y in y0..=y1 {
        let d = (y as f64 + 0.5 - cy) / ry;
        if d.abs() > 1.0 {
            continue;
        }
        let hw = rx * (1.0 - d * d).sqrt();
        g.p(
            c,
            (cx - hw).round(),
            y as f64,
            (2.0 * hw).round().max(1.0) as i32,
            1,
        );
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x2a1f4a, 0x5a2a5e, 0xb44a5a, 0xf08a5a, 0xffc878],
        0,
        w,
        0,
        WATER,
    );
    // The low sun over the last screen.
    let sx = (g.panel(3) + 24) as f64;
    glow(g, 0xffe0a0, sx, 33.0, 16.0, 0.5);
    g.disc(0xfff0c0, sx, 33.0, 5);
    // Long dusk clouds.
    for k in 0..g.spread(7) {
        let kf = k as f64;
        let cx = (rnd(kf) * w as f64 + t * 1.5) % (w as f64 + 40.0) - 20.0;
        let cy = 6.0 + rnd(kf + 2.0) * 16.0;
        let len = 14 + (rnd(kf + 4.0) * 20.0) as i32;
        g.p(0x7a3a6a, cx, cy, len, 1);
        g.p(0xd86a6a, cx + 3.0, cy - 1.0, len - 6, 1);
    }
}

fn city(g: &mut World, f: u32, t: f64, roar: f64) {
    let w = g.w();
    // Far towers in haze.
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64;
        let bw = 4 + (rnd(fi + 1.0) * 7.0) as i32;
        let bh = 6 + (rnd(fi + 2.0) * 14.0) as i32;
        g.p(0x8a4a6a, x as f64, (WATER - 2 - bh) as f64, bw, bh + 2);
        if rnd(fi + 9.0) > 0.7 {
            g.p(0x8a4a6a, (x + bw / 2) as f64, (WATER - 5 - bh) as f64, 1, 3);
        }
        x += bw;
        i += 1;
    }
    // Near city blocks with lit windows, flickering when it roars.
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64 + 40.0;
        let bw = 6 + (rnd(fi + 1.0) * 8.0) as i32;
        let bh = 4 + (rnd(fi + 2.0) * 9.0) as i32;
        let c = if i % 2 == 0 { 0x3a2448 } else { 0x2e1e40 };
        g.p(c, x as f64, (WATER - bh) as f64, bw, bh);
        for wy in (WATER - bh + 1..WATER - 1).step_by(2) {
            for wx in (x + 1..x + bw - 1).step_by(2) {
                let r = rnd((wx * 5 + wy * 11) as f64);
                let lit = r > 0.5 && !(roar > 0.0 && rnd(r + (f / 2) as f64) > 0.6);
                if lit {
                    put(g, if r > 0.85 { 0xfff0b0 } else { 0xf0b860 }, wx, wy);
                }
            }
        }
        x += bw;
        i += 1;
    }
    // Container cranes on the quay of screens 0 and 3.
    for (i, off) in [(0u32, 8), (3, 40)] {
        if i >= g.layout.panels {
            continue;
        }
        let cx = (g.px0(i) + off) as f64;
        let c = 0x1a1028;
        g.p(c, cx, 18.0, 1, WATER - 18);
        g.p(c, cx + 8.0, 18.0, 1, WATER - 18);
        line(g, c, cx, 26.0, cx + 8.0, 34.0);
        line(g, c, cx + 8.0, 26.0, cx, 34.0);
        g.p(c, cx - 4.0, 18.0, 22, 2);
        line(g, c, cx + 4.0, 12.0, cx - 4.0, 18.0);
        line(g, c, cx + 4.0, 12.0, cx + 17.0, 18.0);
        g.p(0xff4040, cx + 4.0, 11.0, 1, 1);
        // The hook swings.
        let hx = cx + 13.0 + (t * 1.2 + i as f64).sin() * 0.8;
        line(g, c, cx + 13.0, 20.0, hx, 27.0);
        g.p(c, hx - 1.0, 27.0, 3, 1);
        if (f / 6).is_multiple_of(2) {
            put(g, 0xff4040, js_round(cx + 4.0), 11);
        } else {
            put(g, 0x5a1a20, js_round(cx + 4.0), 11);
        }
    }
    // A container ship at anchor on screen 0.
    let sx = (g.px0(0) + 24) as f64;
    g.p(0x1a1028, sx, (WATER - 3) as f64, 30, 4);
    g.p(0x1a1028, sx + 26.0, (WATER - 6) as f64, 4, 3);
    g.p(0x1a1028, sx + 2.0, (WATER - 9) as f64, 5, 6);
    put(g, 0xfff0b0, js_round(sx + 3.0), WATER - 8);
    put(g, 0xfff0b0, js_round(sx + 5.0), WATER - 8);
    let boxes = [0xc04a3a, 0x3a7ab0, 0xd8a03a, 0x4a9a5a, 0xb05a9a];
    for k in 0..6 {
        for r in 0..2 {
            if k == 5 && r == 1 {
                continue;
            }
            g.p(
                boxes[(k * 2 + r * 3) % 5],
                sx + 8.0 + k as f64 * 3.0,
                (WATER - 5 - r as i32 * 2) as f64,
                3,
                2,
            );
        }
    }
    g.p(0x5a1a1a, sx, (WATER - 1) as f64, 30, 1);
}

fn water(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x6a3a6a, 0x3a2a5a, 0x22204a, 0x14142e],
        0,
        w,
        WATER,
        60,
    );
    put(g, 0xffc878, 0, WATER);
    g.p(0xd88a6a, 0.0, WATER as f64, w, 1);
    // Ripples catch the sky.
    for k in 0..g.spread(70) {
        let kf = k as f64;
        let y = WATER + 2 + (rnd(kf) * 17.0) as i32;
        let x = (rnd(kf + 1.0) * w as f64 + (t * 0.8 + kf).sin() * 3.0).floor();
        let len = 2 + (rnd(kf + 2.0) * 4.0) as i32;
        let c = if y < WATER + 7 { 0x9a5a7a } else { 0x4a3a6a };
        g.p(c, x, y as f64, len, 1);
    }
    // The sun's broken reflection.
    let sx = (g.panel(3) + 24) as f64;
    for y in WATER + 1..60 {
        let wob = ((y as f64 * 1.3 + t * 5.0).sin() * 2.0).round();
        let half = 2 + (y - WATER) / 4;
        if (y + (f / 2) as i32) % 3 != 0 {
            g.p(0xffd08a, sx - half as f64 + wob, y as f64, half * 2, 1);
        }
    }
    // Lamps and windows on the quay mirrored as wobbling dashes.
    for k in 0..g.spread(30) {
        let kf = k as f64 + 100.0;
        let x = (rnd(kf) * w as f64).floor();
        let y = WATER + 2 + (rnd(kf + 1.0) * 8.0) as i32;
        let wob = ((t * 3.0 + kf).sin() * 1.5).round();
        g.p(0xd8a050, x + wob, y as f64, 2, 1);
    }
}

#[derive(Clone, Copy)]
struct Pose {
    jaw: f64,
    tilt: f64,
    glow: f64,
    squint: bool,
    arm_up: f64,
}

/// The monster, standing at `mx` with its feet `sink` px below the water.
fn monster(g: &mut World, mx: f64, base: f64, p: Pose, f: u32) {
    let (bx, by) = (mx, base - 9.0);
    let (hx, hy) = (mx + 7.0, base - 22.0 + p.tilt);
    // Tail sweeping behind into the water.
    for k in 0..12 {
        let kf = k as f64;
        let r = 4.0 - kf * 0.28;
        oval(
            g,
            OUT,
            bx - 7.0 - kf * 1.4,
            by + 6.0 + kf * 0.4,
            r + 1.0,
            r + 1.0,
        );
    }
    for k in 0..12 {
        let kf = k as f64;
        let r = 4.0 - kf * 0.28;
        oval(g, SKIN, bx - 7.0 - kf * 1.4, by + 6.0 + kf * 0.4, r, r);
    }
    // Body and neck, outlined.
    oval(g, OUT, bx, by, 9.0, 12.0);
    oval(g, OUT, hx - 3.0, (by + hy) / 2.0, 5.0, 8.0);
    oval(g, SKIN, bx, by, 8.0, 11.0);
    oval(g, SKIN, hx - 3.0, (by + hy) / 2.0, 4.0, 7.0);
    oval(g, SKIN_L, bx - 2.0, by - 4.0, 4.0, 6.0);
    oval(g, BELLY, bx + 4.0, by + 1.0, 3.5, 9.0);
    for k in 0..6 {
        g.p(0x7a9a50, bx + 2.0, by - 6.0 + k as f64 * 3.0, 4, 1);
    }
    // Dorsal spines from the head down the back, glowing when it roars.
    let spine = fx::mix(0x8ab8a0, 0x9af8ff, p.glow);
    for k in 0..7 {
        let u = k as f64 / 6.0;
        let sx = hx - 7.0 - u * 9.0;
        let sy = hy + 1.0 + u * 16.0;
        let hgt = 4.0 - (u - 0.4).abs() * 3.0;
        for r in 0..hgt.round() as i32 {
            g.p(OUT, sx - r as f64 - 1.0, sy - r as f64 + 1.0, 3, 1);
            g.p(spine, sx - r as f64, sy - r as f64, 2, 1);
        }
        if p.glow > 0.0 {
            glow(g, 0x9af8ff, sx - 1.0, sy - 1.0, 4.0, p.glow * 0.5);
        }
    }
    // Arms: little ones, of course.
    let ay = by - 3.0 - p.arm_up * 6.0;
    g.p(OUT, bx + 6.0, by - 4.0, 2, 4);
    line(g, OUT, bx + 7.0, by - 3.0, bx + 11.0, ay);
    line(g, SKIN_L, bx + 7.0, by - 4.0, bx + 10.0, ay - 1.0);
    g.p(0xf0f0e0, bx + 11.0, ay - 1.0, 1, 1);
    // Head: skull, snout and a jaw that drops.
    let jaw = p.jaw;
    g.p(OUT, hx - 5.0, hy - 4.0, 12, 7);
    g.p(OUT, hx + 6.0, hy - 2.0, 4, 5);
    g.p(SKIN, hx - 4.0, hy - 3.0, 10, 5);
    g.p(SKIN, hx + 6.0, hy - 1.0, 3, 3);
    g.p(SKIN_L, hx - 3.0, hy - 3.0, 7, 1);
    // Brow ridge and nostril.
    g.p(OUT, hx + 8.0, hy - 1.0, 1, 1);
    // Mouth: open shows teeth and a red throat.
    let jy = hy + 2.0 + jaw;
    if jaw > 0.5 {
        g.p(0x5a1018, hx - 1.0, hy + 2.0, 10, jaw.round() as i32);
        for k in 0..5 {
            put(
                g,
                0xffffff,
                js_round(hx + k as f64 * 2.0),
                js_round(hy + 2.0),
            );
            put(
                g,
                0xffffff,
                js_round(hx + 1.0 + k as f64 * 2.0),
                js_round(jy - 1.0),
            );
        }
    }
    g.p(OUT, hx - 3.0, jy, 12, 3);
    g.p(SKIN, hx - 2.0, jy, 10, 2);
    g.p(BELLY, hx - 1.0, jy + 1.0, 8, 1);
    // Eye.
    if p.squint {
        g.p(OUT, hx + 1.0, hy - 2.0, 3, 1);
    } else {
        g.p(0xffe040, hx + 1.0, hy - 2.0, 2, 2);
        put(g, OUT, js_round(hx + 2.0), js_round(hy - 2.0));
        if (f / 3).is_multiple_of(17) {
            g.p(SKIN, hx + 1.0, hy - 2.0, 2, 2);
        }
    }
}

const JET: &[&str] = &["k....", "kk...", "gggggn", ".kk..", "k...."];
const JETP: &[(char, u32)] = &[('k', 0x3a3e4a), ('g', 0xe8eef8), ('n', 0xff6a4a)];

fn jet(g: &mut World, x: f64, y: f64, left: bool, trail: f64) {
    let dir = if left { 1.0 } else { -1.0 };
    for k in 0..(trail as i32) {
        blend(
            g,
            0xf0e8f0,
            js_round(x + 2.5 + dir * (3.0 + k as f64)),
            js_round(y + 2.0),
            0.6 * (1.0 - k as f64 / trail),
        );
    }
    g.spr(JET, JETP, x, y, left);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let roar = if (T_ROAR..T_HUSH).contains(&t) {
        ((t - T_ROAR) / 0.3).min(1.0) * ((T_HUSH - t) / 0.3).min(1.0)
    } else {
        0.0
    };
    sky(g, t);
    city(g, f, t, roar);

    // Where the monster is: it surfaces on screen 1 and wades to screen 2.
    let v_rise = stage_at(g, 1, 26.0);
    let v_stand = stage_at(g, 2, 22.0);
    let walk = ((t - T_WADE) / (T_STAND - T_WADE)).clamp(0.0, 1.0);
    let v = v_rise + (v_stand - v_rise) * walk;
    let mx = stage_x(g, v);
    let step = if walk > 0.0 && walk < 1.0 {
        (walk * PI * 4.0).sin().abs()
    } else {
        0.0
    };
    let rise = ease((t - T_RISE) / (T_UP - T_RISE));
    let base = WATER as f64 + 6.0 + (1.0 - rise) * 32.0 - step * 1.5;

    // Pose for the roar and the sneeze.
    let ah = ((t - T_AH) / (T_CHOO - T_AH)).clamp(0.0, 1.0);
    let after = t - T_CHOO;
    let mut pose = Pose {
        jaw: roar * 5.0,
        tilt: 0.0,
        glow: roar,
        squint: false,
        arm_up: 0.0,
    };
    if (T_AH..T_CHOO).contains(&t) {
        pose.tilt = -ease(ah) * 3.0;
        pose.jaw = ah * 2.0;
        pose.squint = ah > 0.4;
    } else if (0.0..0.35).contains(&after) {
        pose.tilt = 2.0;
        pose.squint = true;
    } else if after >= 0.35 {
        pose.arm_up = ((after - 0.6) / 0.3).clamp(0.0, 1.0) * ((1.6 - after) / 0.3).clamp(0.0, 1.0);
        pose.squint = after > 1.0;
    }
    if t > T_RISE - 0.2 {
        monster(g, mx, base, pose, f);
    }

    water(g, f, t);
    // Bubbles warn of it, sweeping in from screen 0.
    if t < T_RISE + 0.4 {
        let bx = stage_x(
            g,
            stage_at(g, 0, 20.0) + (v_rise - 20.0) * (t / T_RISE).min(1.0),
        );
        for k in 0..8 {
            let kf = k as f64;
            let age = (t * 2.0 + rnd(kf)) % 1.0;
            let x = bx - 8.0 + rnd(kf + 1.0) * 16.0;
            g.ring(0xc8b8e0, x, WATER as f64 + 3.0 + (1.0 - age) * 6.0, 1.0);
        }
    }
    // Water around its legs: a wake ring and streaming water while rising.
    if t > T_RISE - 0.2 {
        let w0 = WATER as f64;
        for k in 0..20 {
            let kf = k as f64;
            let x = mx - 12.0 + kf * 1.3;
            let h = 1.0 + ((t * 8.0 + kf).sin() + 1.0) * 0.8;
            g.p(0xe8e0f8, x, w0 - h + 1.0, 1, h as i32);
        }
        g.p(0x8a7aa8, mx - 14.0, w0 + 1.0, 30, 1);
        if rise < 1.0 {
            for k in 0..14 {
                let kf = k as f64;
                let age = (t * 2.5 + rnd(kf)) % 1.0;
                let x = mx - 8.0 + rnd(kf + 3.0) * 18.0;
                let y = base - 32.0 + age * 30.0;
                if y < w0 {
                    put(g, 0xc8e0ff, js_round(x), js_round(y));
                }
            }
        }
        if step > 0.9 || (T_RISE + 1.4..T_UP).contains(&t) {
            for k in 0..10 {
                let a = PI + k as f64 / 9.0 * PI;
                let r = 10.0 + rnd(k as f64 + (f / 2) as f64) * 6.0;
                g.p(0xe8f0ff, mx + 2.0 + a.cos() * r, w0 + a.sin() * 4.0, 1, 2);
            }
        }
    }

    // Roar rings from the mouth.
    let (hx, hy) = (mx + 7.0, base - 22.0 + pose.tilt);
    if roar > 0.0 {
        for k in 0..3 {
            let r = ((t - T_ROAR) * 30.0 + k as f64 * 7.0) % 21.0 + 3.0;
            for a in 0..12 {
                let th = -0.6 + a as f64 / 11.0 * 1.2;
                put(
                    g,
                    0xfff0d8,
                    js_round(hx + 9.0 + th.cos() * r),
                    js_round(hy + 3.0 + th.sin() * r),
                );
            }
        }
    }

    // Jets: they arrive from the last screen and buzz the head.
    let n_jets = 3;
    let mut blasted = None;
    for j in 0..n_jets {
        let jt = t - T_JETS - j as f64 * 0.35;
        if jt < 0.0 {
            continue;
        }
        let p3 = g.panel(3) as f64;
        let ph = j as f64 * 2.1;
        let a = jt * 2.4 + ph;
        let orbit = (hx + 2.0 + a.cos() * 16.0, hy - 6.0 + a.sin() * 6.0);
        let enter = ease(jt / 1.2);
        let start = (p3 + 50.0, 10.0 + j as f64 * 5.0);
        let (mut x, mut y) = (
            start.0 + (orbit.0 - start.0) * enter,
            start.1 + (orbit.1 - start.1) * enter,
        );
        let mut left = enter < 1.0 || a.sin() < 0.0;
        if j == 1 && t >= T_CHOO {
            // The one in front of its nose is sneezed away.
            let u = t - T_CHOO;
            let a0 = (T_CHOO - T_JETS - 0.35) * 2.4 + ph;
            let o = (hx + 2.0 + a0.cos() * 16.0, hy - 6.0 + a0.sin() * 6.0);
            let p = (o.0 + u * 48.0, o.1 - u * 26.0 + u * u * 18.0);
            blasted = Some((p, u));
            continue;
        }
        if t >= T_CHOO {
            // The others scatter.
            let u = t - T_CHOO;
            x += u * 30.0 * if j == 0 { -1.0 } else { 1.0 };
            y -= u * 10.0;
            left = j == 0;
        }
        jet(g, x, y, left, 12.0);
    }
    if let Some(((x, y), u)) = blasted {
        if u < 1.1 {
            let spin = (f / 2) % 4;
            let rows: &[&str] = match spin {
                0 => JET,
                1 => &["..k..", ".kgk.", "kgggk", ".kgn.", "..g.."],
                2 => &["....k", "...kk", "ngggg", "..kk.", "....k"],
                _ => &["..g..", ".ngk.", "kgggk", ".kgk.", "..k.."],
            };
            for k in 0..6 {
                let kf = k as f64;
                g.p(
                    if k % 2 == 0 { 0x6a6a7a } else { 0x9a9aaa },
                    x - kf * 4.0 * (1.0 + u),
                    y + kf * 2.0 * (1.0 - u) + kf * 0.5,
                    2,
                    2,
                );
            }
            g.spr(rows, JETP, x, y, false);
        } else {
            // A parachute opens and drifts down.
            let p0 = (x, y);
            let du = u - 1.1;
            let (px, py) = (p0.0 - 6.0 + du * 3.0, (p0.1 - 6.0 + du * 6.0).min(30.0));
            let px = px + (du * 3.0).sin() * 1.5;
            oval(g, 0xf0f0f0, px, py, 4.0, 2.0);
            g.p(0xe04040, px - 1.0, py - 2.0, 2, 2);
            line(g, 0x8a8a9a, px - 3.0, py + 1.0, px, py + 5.0);
            line(g, 0x8a8a9a, px + 3.0, py + 1.0, px, py + 5.0);
            g.p(0x3a4a2a, px - 1.0, py + 5.0, 2, 3);
            put(g, 0xe8a878, js_round(px), js_round(py + 5.0));
        }
    }

    // Ah... CHOO: a spray cloud from the snout.
    if (0.0..0.8).contains(&after) {
        let u = after / 0.8;
        for k in 0..30 {
            let kf = k as f64;
            let a = (rnd(kf) - 0.5) * 1.2 - 0.25;
            let r = u * (8.0 + rnd(kf + 1.0) * 22.0);
            let c = if k % 3 == 0 { 0xc8f0a0 } else { 0xf4f8ff };
            if rnd(kf + 2.0) > u * 0.8 {
                g.p(
                    c,
                    hx + 10.0 + a.cos() * r,
                    hy + a.sin() * r + u * u * 6.0,
                    2,
                    1,
                );
            }
        }
        glow(g, 0xffffff, hx + 14.0, hy, 10.0 * (1.0 - u), 0.6);
    }
    // The "ah... ah..." beads over its head.
    if (T_AH..T_CHOO).contains(&t) {
        for k in 0..((ah * 3.0) as i32 + 1).min(3) {
            put(
                g,
                0xffffff,
                js_round(hx + 10.0 + k as f64 * 2.0),
                js_round(hy - 8.0),
            );
        }
    }

    // Stomps and the roar shake the world.
    let sh = if roar > 0.2 {
        if f.is_multiple_of(2) {
            2
        } else {
            -2
        }
    } else if step > 0.92 {
        1
    } else if (0.0..0.2).contains(&after) {
        if f.is_multiple_of(2) {
            1
        } else {
            -1
        }
    } else {
        0
    };
    if sh != 0 {
        shake(g, sh / 2, sh);
    }
}

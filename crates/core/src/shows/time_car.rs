//! Time machine: on a quiet night street a boxy silver car hums, then
//! floors it across the screens, throwing sparks while blue lightning
//! wraps round it. A flash, and it is gone, two burning tyre tracks left
//! on the road. A moment later the sky crackles and it drops back in,
//! frosted over, and a hubcap rolls off on its own.

use super::fx::{self, blend, glow, line, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

const T_GO: f64 = 1.2;
const T_GONE: f64 = 4.6;
const T_BACK: f64 = 7.0;
const T_LAND: f64 = 7.35;

/// Road surface rows.
const ROAD: i32 = 38;
const KERB: i32 = 53;
/// Row the tyres touch.
const TYRE: i32 = 49;

const CAR: [&[&str]; 2] = [
    &[
        ".......kkkkkkkk.......",
        "......kgggkgggsk......",
        "....kkgggkgggssskkk...",
        ".kkkssssssssssssssskk.",
        "kcssmmmmmmmmmmmmmsssyk",
        "krsssssssssssssssssssk",
        "kkddkkddddddddddkkddkk",
        "..kwhk..........kwhk..",
        "...kk............kk...",
    ],
    &[
        ".......kkkkkkkk.......",
        "......kgggkgggsk......",
        "....kkgggkgggssskkk...",
        ".kkkssssssssssssssskk.",
        "kcssmmmmmmmmmmmmmsssyk",
        "krsssssssssssssssssssk",
        "kkddkkddddddddddkkddkk",
        "..khwk..........khwk..",
        "...kk............kk...",
    ],
];

fn car_pal(coil: u32) -> [(char, u32); 10] {
    [
        ('k', 0x0e0e18),
        ('s', 0xc4ccd6),
        ('m', 0x8a929e),
        ('d', 0x4a505c),
        ('g', 0x4a6a96),
        ('w', 0x9aa2ae),
        ('h', 0x3a3e48),
        ('c', coil),
        ('r', 0xff3a3a),
        ('y', 0xfff2b0),
    ]
}

/// A jagged electric arc from `a` to `b`.
fn bolt(g: &mut World, a: (f64, f64), b: (f64, f64), seed: f64, core: u32) {
    let n = 5;
    let mut p = a;
    for i in 1..=n {
        let u = i as f64 / n as f64;
        let j = if i == n {
            0.0
        } else {
            (rnd(seed + i as f64) - 0.5) * 6.0
        };
        let q = (a.0 + (b.0 - a.0) * u + j * 0.4, a.1 + (b.1 - a.1) * u + j);
        line(g, 0x5aa8ff, p.0, p.1 + 1.0, q.0, q.1 + 1.0);
        line(g, core, p.0, p.1, q.0, q.1);
        p = q;
    }
}

fn sky(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    vgrad(g, &[0x070a1e, 0x10153a, 0x1e2152, 0x3a2c6a], 0, w, 0, 33);
    for i in 0..g.spread(40) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * w as f64).floor(), (rnd(fi + 5.0) * 20.0).floor());
        let tw = (f + i * 5) % 31 < 2;
        put(g, if tw { 0xffffff } else { 0x8a8ac8 }, x as i32, y as i32);
    }
    // The moon over the first screen, with a faint halo.
    let mx = (g.px0(0) + 44) as f64;
    glow(g, 0xb0b8ff, mx, 9.0, 11.0, 0.25);
    g.disc(0xf0ecd8, mx, 9.0, 4);
    g.p(0xd8d0b8, mx - 1.0, 8.0, 2, 1);
    g.p(0xd8d0b8, mx + 1.0, 11.0, 1, 1);
    // Two thin clouds drift.
    for k in 0..g.spread(3) {
        let kf = k as f64;
        let cx = (kf * 131.0 + t * 2.0) % (w as f64 + 30.0) - 15.0;
        g.p(0x2a2a5a, cx, 14.0 + kf * 3.0, 18, 1);
        g.p(0x2a2a5a, cx + 4.0, 13.0 + kf * 3.0, 9, 1);
    }
}

fn town(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    // Far skyline.
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64;
        let bw = 5 + (rnd(fi + 3.0) * 8.0) as i32;
        let bh = 8 + (rnd(fi + 33.0) * 10.0) as i32;
        g.p(0x1c1a40, x as f64, (32 - bh) as f64, bw, bh);
        for wy in (32 - bh + 2..30).step_by(2) {
            for wx in (x + 1..x + bw - 1).step_by(2) {
                if rnd((wx * 7 + wy * 13) as f64) > 0.82 {
                    put(g, 0x8a7a4a, wx, wy);
                }
            }
        }
        x += bw;
        i += 1;
    }
    // A clock tower on screen 1, its hands spinning as the car breaks
    // through.
    let cx = (g.panel(1) + 30) as f64;
    g.p(0x24204a, cx - 5.0, 9.0, 11, 26);
    g.p(0x2e2a5a, cx - 6.0, 8.0, 13, 2);
    g.p(0x24204a, cx - 3.0, 4.0, 7, 4);
    g.p(0x24204a, cx - 1.0, 2.0, 3, 2);
    put(g, 0x24204a, js_round(cx), 1);
    glow(g, 0xfff0b0, cx, 14.0, 6.0, 0.35);
    g.disc(0xf4e8b8, cx, 14.0, 3);
    let spin = if t > 3.6 && t < 5.4 {
        (t - 3.6).powi(2) * 20.0
    } else if t >= 5.4 {
        3.24 * 20.0
    } else {
        0.0
    };
    line(
        g,
        0x2a1e10,
        cx,
        14.0,
        cx + (spin - PI / 2.0).cos() * 2.4,
        14.0 + (spin - PI / 2.0).sin() * 2.4,
    );
    line(
        g,
        0x2a1e10,
        cx,
        14.0,
        cx + (spin / 12.0 + 0.5).cos() * 1.6,
        14.0 + (spin / 12.0 + 0.5).sin() * 1.6,
    );
    for wy in [20, 25, 30] {
        g.p(0x8a7a4a, cx - 2.0, wy as f64, 1, 2);
        g.p(0x8a7a4a, cx + 2.0, wy as f64, 1, 2);
    }
    // Near buildings with lit windows.
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64 + 50.0;
        let bw = 9 + (rnd(fi + 3.0) * 10.0) as i32;
        let bh = 6 + (rnd(fi + 33.0) * 8.0) as i32;
        let c = if i % 2 == 0 { 0x15122c } else { 0x1a1634 };
        g.p(c, x as f64, (34 - bh) as f64, bw - 1, bh);
        g.p(0x2a2650, x as f64, (34 - bh) as f64, bw - 1, 1);
        for wy in (34 - bh + 2..33).step_by(3) {
            for wx in (x + 1..x + bw - 2).step_by(3) {
                let r = rnd((wx * 3 + wy * 17) as f64);
                if r > 0.55 {
                    let c = if r > 0.9 { 0xffe9a8 } else { 0xd8a84a };
                    g.p(c, wx as f64, wy as f64, 2, 1);
                }
            }
        }
        x += bw;
        i += 1;
    }
    // A blinking diner sign on screen 2.
    let sx = (g.panel(2) + 8) as f64;
    let on = !(f / 4).is_multiple_of(5);
    g.p(0x1a1634, sx - 1.0, 18.0, 18, 9);
    let pink = if on { 0xff5ab8 } else { 0x5a2a4a };
    let teal = if on { 0x5af0e0 } else { 0x2a4a50 };
    if on {
        glow(g, 0xff5ab8, sx + 8.0, 22.0, 12.0, 0.25);
    }
    for k in 0..4 {
        let kx = sx + 1.0 + k as f64 * 4.0;
        g.p(pink, kx, 20.0, 3, 1);
        g.p(pink, kx, 20.0, 1, 3);
        g.p(pink, kx + 2.0, 20.0, 1, 3);
    }
    g.p(teal, sx, 25.0, 16, 1);
    // A star on top.
    put(g, teal, js_round(sx + 8.0), 16);
    put(g, teal, js_round(sx + 7.0), 17);
    put(g, teal, js_round(sx + 9.0), 17);
}

fn street(g: &mut World, f: u32) {
    let w = g.w();
    vgrad(g, &[0x2a2840, 0x23213a], 0, w, 33, ROAD);
    g.p(0x4a4868, 0.0, (ROAD - 1) as f64, w, 1);
    for x in (0..w).step_by(7) {
        put(g, 0x1a182c, x, 34);
        put(g, 0x1a182c, x, 35);
    }
    vgrad(g, &[0x181828, 0x1e1e30, 0x16161f], 0, w, ROAD, KERB);
    for x in 0..w {
        if rnd(x as f64 * 1.7) > 0.8 {
            put(g, 0x262636, x, ROAD + 2 + x % 11);
        }
        if (x / 6) % 2 == 0 {
            put(g, 0xc8b858, x, 44);
        }
    }
    g.p(0x4a4868, 0.0, KERB as f64, w, 1);
    vgrad(g, &[0x262440, 0x14122a], 0, w, KERB + 1, 60);
    // Streetlights: pole, lamp and a cone of light on the road.
    for i in 0..g.layout.panels {
        for off in [12, 48] {
            let x = g.px0(i) + off;
            for y in 18..TYRE + 4 {
                let half = (y - 18) as f64 * 0.42 + 1.0;
                for dx in -js_round(half)..=js_round(half) {
                    let k = 0.22 * (1.0 - (dx as f64 / (half + 1.0)).abs());
                    blend(g, 0xffe6a0, x + dx, y, k);
                }
            }
            g.p(0x0e0e18, x as f64, 16.0, 1, 21);
            g.p(0x0e0e18, (x - 2) as f64, 16.0, 4, 1);
            g.p(0xfff0c0, (x - 2) as f64, 17.0, 3, 1);
            glow(g, 0xffe6a0, x as f64 - 0.5, 17.5, 5.0, 0.6);
            let _ = f;
        }
    }
}

/// Burning tyre tracks from `x0` back to `x1`, `age` seconds old.
fn tracks(g: &mut World, x0: f64, x1: f64, age: f64, f: u32) {
    let fade = (1.0 - (age - 2.0) / 1.6).clamp(0.0, 1.0);
    for (row, ph) in [(TYRE as f64 - 2.0, 0.0), (TYRE as f64, 3.0)] {
        let mut x = x0;
        while x > x1 {
            let xi = js_round(x);
            put(g, 0x2a1408, xi, row as i32 + 1);
            if fade > 0.0 {
                let r = rnd(x + ph + (f / 2) as f64 * 0.37);
                let len = (fade * (1.0 + r * 4.0) * (1.0 - (x0 - x) / (x0 - x1) * 0.5)) as i32;
                for k in 0..len {
                    let c = if k == 0 {
                        0xfff0a0
                    } else if k < 2 {
                        0xffa030
                    } else {
                        0xd8401c
                    };
                    put(g, c, xi, row as i32 - k);
                }
                blend(g, 0xff8a2a, xi, row as i32 - len - 1, 0.4 * fade);
            } else if rnd(x + (f / 3) as f64) > 0.85 {
                let s = (age - 3.6) * 8.0;
                g.p(0x4a4858, x, row - 2.0 - s % 6.0, 1, 1);
            }
            x -= 1.0;
        }
    }
    if fade > 0.0 {
        glow(g, 0xff8a2a, x0 - 8.0, TYRE as f64 - 3.0, 14.0 * fade, 0.3);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, f, t);
    town(g, f, t);
    street(g, f);

    let v0 = 14.0;
    let v_gone = stage_at(g, 2, 32.0);
    let acc = 2.0 * (v_gone - v0) / (T_GONE - T_GO).powi(2);
    let tt = (t - T_GO).max(0.0);
    let v = v0 + 0.5 * acc * tt * tt;
    let speed = acc * tt;
    let x_gone = stage_x(g, v_gone);
    let coil_on = t > 0.4;
    let coil = if !coil_on {
        0x2a3a5a
    } else if (f / 2).is_multiple_of(2) {
        0x8ae0ff
    } else {
        0x4aa0ff
    };
    let pal = car_pal(coil);

    if t > T_GONE {
        tracks(g, x_gone + 16.0, x_gone - 42.0, t - T_GONE, f);
    }

    if t < T_GONE {
        let x = stage_x(g, v);
        let y = (TYRE - 8) as f64;
        // Headlight beam ahead of the car.
        for k in 0..22 {
            let spread = k as f64 * 0.25;
            for dy in -js_round(spread)..=js_round(spread) {
                blend(
                    g,
                    0xfff2c0,
                    js_round(x + 22.0 + k as f64),
                    js_round(y + 4.0 + spread * 0.6) + dy,
                    0.35 * (1.0 - k as f64 / 22.0),
                );
            }
        }
        // Speed streaks behind.
        if speed > 20.0 {
            for k in 0..6 {
                let len = (speed * 0.25 * (0.5 + rnd(k as f64 + (f / 2) as f64))).min(40.0);
                let yy = y + 1.0 + k as f64 * 1.3;
                line(g, 0x6a7a9a, x - 2.0, yy, x - 2.0 - len, yy);
            }
        }
        // Exhaust puffs while idling.
        if t < T_GO + 0.6 {
            for k in 0..4 {
                let age = (t * 2.0 + k as f64 * 0.25) % 1.0;
                let c = if age < 0.5 { 0x6a6a80 } else { 0x3a3a50 };
                g.p(
                    c,
                    x - 2.0 - age * 6.0,
                    y + 5.0 - age * 4.0,
                    1 + (age * 2.0) as i32,
                    1 + (age * 2.0) as i32,
                );
            }
        }
        let rumble = if speed > 0.0 { (f % 2) as f64 } else { 0.0 };
        g.spr(CAR[((f / 2) % 2) as usize], &pal, x, y - rumble, false);
        // Sparks from under the car.
        if t > 2.0 {
            for k in 0..(4.0 + speed * 0.12) as i32 {
                let kf = k as f64 + (f as f64) * 0.71;
                let sx = x + 4.0 - rnd(kf) * (6.0 + speed * 0.25);
                let sy = TYRE as f64 - rnd(kf + 1.0) * 4.0;
                put(
                    g,
                    if k % 3 == 0 { 0xfff0a0 } else { 0xffa030 },
                    js_round(sx),
                    js_round(sy),
                );
            }
        }
        // Blue lightning grows around the car.
        let charge = ((t - 2.6) / (T_GONE - 2.6)).clamp(0.0, 1.0);
        if charge > 0.0 {
            let (cx, cy) = (x + 11.0, y + 4.0);
            glow(
                g,
                0x4aa0ff,
                cx,
                cy,
                10.0 + charge * 14.0,
                0.25 + charge * 0.4,
            );
            let n = (1.0 + charge * 6.0) as i32;
            for k in 0..n {
                let s = k as f64 * 3.1 + (f as f64) * 1.7;
                let a = rnd(s) * PI * 2.0;
                let r = 8.0 + charge * 10.0 * rnd(s + 1.0);
                bolt(
                    g,
                    (cx + a.cos() * 3.0, cy + a.sin() * 2.0),
                    (cx + a.cos() * r * 1.4, cy + a.sin() * r * 0.8),
                    s,
                    if k % 2 == 0 { 0xffffff } else { 0xbfe6ff },
                );
            }
        }
    }

    // The return: the sky crackles over the last screen, then the car drops.
    let p3 = g.panel(3) as f64;
    let land_x = p3 + 19.0;
    if t > T_BACK - 0.5 && t < T_LAND + 0.2 {
        let u = ((t - T_BACK + 0.5) / 0.7).clamp(0.0, 1.0);
        let (cx, cy) = (land_x + 11.0, 10.0);
        glow(g, 0x4aa0ff, cx, cy, 6.0 + u * 14.0, 0.3 + u * 0.4);
        for k in 0..(2.0 + u * 5.0) as i32 {
            let s = k as f64 * 5.3 + f as f64 * 0.9;
            let a = rnd(s) * PI * 2.0;
            bolt(
                g,
                (cx, cy),
                (cx + a.cos() * 14.0, cy + a.sin() * 7.0),
                s,
                0xffffff,
            );
        }
    }
    if t >= T_BACK {
        let y_rest = (TYRE - 8) as f64;
        let y = if t < T_LAND {
            let u = (t - T_BACK) / (T_LAND - T_BACK);
            -12.0 + (y_rest + 12.0) * u * u
        } else {
            let s = (t - T_LAND) * 14.0;
            y_rest + (s.sin() * 2.5 * (-(t - T_LAND) * 4.0).exp()).round()
        };
        // Steam off the frosted body.
        for k in 0..6 {
            let kf = k as f64;
            let age = (t * 0.8 + rnd(kf)) % 1.0;
            let sx = land_x + 3.0 + rnd(kf + 2.0) * 16.0 + (age * 6.0 + kf).sin();
            blend(
                g,
                0xc8d0e8,
                js_round(sx),
                js_round(y - age * 10.0),
                0.7 * (1.0 - age),
            );
        }
        let lights_on = t > 8.0 && !(8.6..8.8).contains(&t) && !(9.0..9.2).contains(&t);
        let mut pal = car_pal(0x4aa0ff);
        if !lights_on {
            pal[9] = ('y', 0x5a5a40);
        }
        g.spr(CAR[0], &pal, land_x, y, false);
        // Frost on the roof and bonnet.
        for k in 0..18 {
            let kx = land_x + 2.0 + k as f64;
            if rnd(k as f64 + 0.5) > 0.4 {
                let ky = if (6.0..14.0).contains(&(k as f64)) {
                    y
                } else {
                    y + 3.0
                };
                put(g, 0xf4f8ff, js_round(kx), js_round(ky));
            }
        }
        if lights_on {
            for k in 0..16 {
                let spread = k as f64 * 0.25;
                for dy in -js_round(spread)..=js_round(spread) {
                    blend(
                        g,
                        0xfff2c0,
                        js_round(land_x + 22.0 + k as f64),
                        js_round(y + 4.0 + spread * 0.6) + dy,
                        0.35 * (1.0 - k as f64 / 16.0),
                    );
                }
            }
        }
        // A hubcap pops off and rolls away, wobbling to a stop.
        if t > T_LAND + 0.3 {
            let u = ((t - T_LAND - 0.3) / 1.8).clamp(0.0, 1.0);
            let hx = land_x + 18.0 + ease(u) * 18.0;
            let hy = TYRE as f64 - 3.0;
            if u < 1.0 {
                let ph = (f / 2) % 3;
                let w = [3, 2, 1][ph as usize];
                g.p(0xdfe6ee, hx - (w / 2) as f64, hy, w.max(1), 3);
                put(g, 0x8a929e, js_round(hx), js_round(hy + 1.0));
            } else {
                let wob = ((t - T_LAND - 2.1) * 10.0).sin() * (-(t - T_LAND - 2.1) * 3.0).exp();
                g.p(0xdfe6ee, hx - 1.0, hy + 2.0, 4, 1);
                put(g, 0x8a929e, js_round(hx + 1.0 + wob), js_round(hy + 1.0));
            }
        } else {
            put(g, 0xdfe6ee, js_round(land_x + 18.0), js_round(y + 7.0));
        }
    }

    // The flash as it vanishes.
    if (T_GONE..T_GONE + 0.6).contains(&t) {
        let u = (t - T_GONE) / 0.6;
        fx::tint(
            g,
            if u < 0.2 { 0xffffff } else { 0x9ad0ff },
            (1.0 - u) * 0.95,
        );
    }
    if (T_LAND..T_LAND + 0.25).contains(&t) {
        fx::shake(g, 0, if f.is_multiple_of(2) { 2 } else { -1 });
    }
}

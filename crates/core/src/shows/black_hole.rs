//! Event horizon: a black hole with a swirling accretion disk, its far side
//! lensed into a halo over the shadow, fed by a stream torn off a companion
//! star; stars drift inward and bend round it. A tiny probe arrives from
//! the left, is caught, spirals in, stretches into a red streak and
//! vanishes in a flash that fires jets out of the poles.

use super::scifi::{bayer, glow, line, mix, tint};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

const T_CAPTURE: f64 = 3.4;
const T_FALL: f64 = 7.2;
const SPACE: u32 = 0x03020a;
const SHADOW_R: f64 = 9.0;
/// Disk inner and outer radius.
const R_IN: f64 = 11.0;
const R_OUT: f64 = 80.0;
/// How flat the disk looks: its vertical squash.
const FLAT: f64 = 0.17;

/// The hole sits in the middle of the third screen (the last on a narrower
/// layout).
fn centre(g: &World) -> (f64, f64) {
    ((g.panel(2) + 30) as f64, 30.0)
}

fn disk_colour(r: f64) -> u32 {
    let u = ((r - R_IN) / (R_OUT - R_IN)).clamp(0.0, 1.0);
    let stops = [0xfff6dc, 0xffd27a, 0xff9a3a, 0xd8502a, 0x8a2a2a, 0x3a1020];
    let k = u * (stops.len() - 1) as f64;
    let i = (k.floor() as usize).min(stops.len() - 2);
    mix(stops[i], stops[i + 1], k - i as f64)
}

/// The angle a disk ring at radius `r` has turned through by `t`.
fn spin(r: f64, t: f64) -> f64 {
    t * 5.0 * (R_IN / r).powf(1.5)
}

/// The flash, 0 before the probe falls in.
fn flash(t: f64) -> f64 {
    let e = t - T_FALL;
    if e < 0.0 {
        0.0
    } else {
        (1.0 - e / 1.6).max(0.0)
    }
}

fn stars(g: &mut World, f: u32, t: f64) {
    let (cx, cy) = centre(g);
    let wf = g.w() as f64;
    let (lo, hi) = (6.0f64.ln(), 420.0f64.ln());
    for i in 0..g.spread(110) {
        let fi = i as f64;
        let (x0, y0) = (rnd(fi * 1.9 + 3.0) * wf, rnd(fi * 2.7 + 1.0) * 60.0);
        let (dx, dy) = (x0 - cx, (y0 - cy) * 2.0);
        let d0 = (dx * dx + dy * dy).sqrt().max(6.0);
        // Falling in on a slow log spiral, wrapping back out when swallowed.
        let ld = lo + (d0.ln() - t * 0.07 - lo).rem_euclid(hi - lo);
        let d = ld.exp();
        let th = dy.atan2(dx) + t * 0.6 * (20.0 / d).powf(1.5).min(3.0);
        // Lensing pushes the image of a star out round the Einstein ring.
        let dl = d + 130.0 / d;
        let (x, y) = (cx + th.cos() * dl, cy + th.sin() * dl * 0.5);
        let near = (1.0 - d / 60.0).clamp(0.0, 1.0);
        let base = if i % 5 == 0 {
            0xffffff
        } else if i % 2 == 0 {
            0x9a9ac8
        } else {
            0x4a4a6a
        };
        let tw = (f + i * 13) % 47 < 2;
        let c = mix(if tw { 0xffffff } else { base }, 0xff8a5a, near * 0.8);
        g.p(c, x, y, 1, 1);
    }
}

/// Faint blue gas over the first two screens, slowly drawn toward the hole.
fn haze(g: &mut World, t: f64) {
    let (cx, _) = centre(g);
    let x1 = js_round(cx - 40.0).max(0);
    for y in 0..60 {
        for x in 0..x1 {
            let (xf, yf) = (x as f64 + t * 2.0, y as f64);
            let n = (xf * 0.05 + yf * 0.12).sin()
                + 0.7 * (xf * 0.13 - yf * 0.07 + 1.0).sin()
                + 0.5 * (xf * 0.021 + 2.0).sin();
            let v = (n - 0.6) * 0.5;
            if v > bayer(x, y) {
                let c = if v > 0.5 + bayer(x + 2, y) * 0.5 {
                    0x1c2050
                } else {
                    0x10142e
                };
                g.p(c, x as f64, yf, 1, 1);
            }
        }
    }
}

/// A small spiral galaxy far off on the first screen.
fn galaxy(g: &mut World, t: f64) {
    let (gx, gy) = ((g.px0(0) + 20) as f64, 18.0);
    glow(g, gx, gy, 6.0, 0x8a8ac8, 0.35);
    for arm in 0..2 {
        for k in 0..26 {
            let kf = k as f64;
            let a = arm as f64 * PI + kf * 0.28 + t * 0.05;
            let r = kf * 0.35;
            let c = mix(0xd8d8ff, 0x3a3a6a, kf / 26.0);
            g.p(c, gx + a.cos() * r * 1.6, gy + a.sin() * r * 0.7, 1, 1);
        }
    }
    g.p(0xffffff, gx, gy, 1, 1);
}

/// A companion star on the last screen, a stream of its gas spiralling off
/// towards the disk.
fn companion(g: &mut World, f: u32, t: f64) {
    let (cx, cy) = centre(g);
    let (sx, sy) = ((g.panel(3) + 34) as f64, 20.0);
    glow(g, sx, sy, 14.0, 0xff9a5a, 0.45);
    g.disc(0xff8a3a, sx, sy, 7);
    g.disc(0xffb060, sx - 1.0, sy - 1.0, 5);
    g.disc(0xffd890, sx - 2.0, sy - 2.0, 2);
    // Its surface is pulled into a teardrop toward the hole.
    g.p(0xff8a3a, sx - 9.0, sy + 1.0, 3, 2);
    g.p(0xffb060, sx - 8.0, sy + 1.0, 1, 1);
    // A Bézier from the star's tip, bowing down, into the disk's rim.
    let (p0, p1, p2) = (
        (sx - 9.0, sy + 1.0),
        (sx - 26.0, cy + 8.0),
        (cx + 58.0, cy + 1.0),
    );
    let n = 90;
    for k in 0..n {
        let u = k as f64 / n as f64;
        let v = 1.0 - u;
        let x = v * v * p0.0 + 2.0 * v * u * p1.0 + u * u * p2.0;
        let y = v * v * p0.1 + 2.0 * v * u * p1.1 + u * u * p2.1;
        let flow = ((u * 16.0 - t * 5.0).sin() * 0.5 + 0.5) * (1.0 - u * 0.4);
        let c = mix(0xa8382a, 0xffc070, flow);
        let wd = if u < 0.5 { 2 } else { 1 };
        for d in 0..wd {
            tint(g, js_round(x), js_round(y) + d, c, 0.9 - d as f64 * 0.3);
        }
        if (k as u32 + f).is_multiple_of(5) {
            tint(g, js_round(x), js_round(y) - 1, 0xffd890, 0.4);
        }
    }
}

/// One half of the disk: the far half (`front = false`) or the near half.
fn disk(g: &mut World, t: f64, front: bool, boost: f64) {
    let (cx, cy) = centre(g);
    let mut r = R_IN;
    while r < R_OUT {
        let n = (r * 2.0) as i32;
        let base = disk_colour(r);
        let rot = spin(r, t);
        for k in 0..n {
            let kf = k as f64;
            // Clumpy: some arcs are gaps.
            if rnd(r * 13.0 + kf) < 0.25 {
                continue;
            }
            let th = kf / n as f64 * PI * 2.0 + rot;
            if (th.sin() > 0.0) != front {
                continue;
            }
            // Brighter where the disk turns toward us (the left).
            let beam = (-th.cos() * 0.35 + 0.15 + boost).max(0.0);
            let c = mix(base, 0xffffff, beam * (1.0 - (r - R_IN) / R_OUT));
            let x = cx + r * th.cos();
            let y = cy + r * th.sin() * FLAT + r * th.cos() * 0.03;
            let a = (1.0 - ((r - R_IN) / (R_OUT - R_IN)).powi(2) * 0.7).min(1.0);
            tint(g, js_round(x), js_round(y), c, a);
            if r < 30.0 {
                // Arcs, not dots, where the gas is fastest.
                let x2 = cx + r * (th - 0.08).cos();
                let y2 = cy + r * (th - 0.08).sin() * FLAT;
                tint(g, js_round(x2), js_round(y2), c, a * 0.7);
            }
        }
        r += if r < 24.0 { 0.7 } else { 1.2 };
    }
}

/// The shadow, the thin photon ring, and the far side of the disk lensed
/// into a halo above the shadow and a fainter one below.
fn hole(g: &mut World, t: f64, boost: f64) {
    let (cx, cy) = centre(g);
    let mut r = R_IN;
    while r < 26.0 {
        let rho = SHADOW_R + 1.5 + (r - R_IN) * 0.22;
        let base = disk_colour(r);
        let rot = spin(r, t);
        let n = (rho * 7.0) as i32;
        for k in 0..n {
            let kf = k as f64;
            if rnd(r * 7.0 + kf * 1.3) < 0.2 {
                continue;
            }
            let th = kf / n as f64 * PI * 2.0 + rot;
            let beam = (-th.cos() * 0.3 + 0.2 + boost).max(0.0);
            let c = mix(base, 0xffffff, beam);
            let (x, y) = (cx + rho * th.cos(), cy - rho * th.sin().abs());
            tint(g, js_round(x), js_round(y), c, 0.8);
            // The secondary image, thin, under the shadow.
            if r < 16.0 {
                let y2 = cy + (SHADOW_R + 1.0) * th.sin().abs();
                tint(
                    g,
                    js_round(cx + (SHADOW_R + 1.0) * th.cos()),
                    js_round(y2),
                    c,
                    0.5,
                );
            }
        }
        r += 1.0;
    }
    g.disc(0x000000, cx, cy, SHADOW_R as i32);
    g.ring(0xffe8c0, cx, cy, SHADOW_R + 0.5);
}

/// The probe: its position, whether it is behind the hole, and how far it
/// has fallen (0 cruising .. 1 at the horizon).
fn probe(g: &World, f: u32, t: f64) -> Option<(f64, f64, bool, f64, f64)> {
    let (cx, cy) = centre(g);
    let r0 = 28.0;
    if t < T_CAPTURE {
        let u = t / T_CAPTURE;
        let x0 = g.px0(0) as f64 - 6.0;
        let x = x0 + (cx - r0 - x0) * u;
        let y = 22.0 + (cy - 22.0) * u * u;
        return Some((x, y, false, 0.0, PI));
    }
    if t >= T_FALL {
        return None;
    }
    let u = (t - T_CAPTURE) / (T_FALL - T_CAPTURE);
    let r = |u: f64| R_IN - 3.0 + (r0 - R_IN + 3.0) * (1.0 - u).powf(1.4);
    // Orbital angle, summed frame by frame (it speeds up as it falls).
    let k0 = (T_CAPTURE * 12.0).ceil() as u32;
    let mut th = PI;
    for k in k0..f {
        let uu = (k as f64 / 12.0 - T_CAPTURE) / (T_FALL - T_CAPTURE);
        th += 1.6 * (28.0 / r(uu)).powf(1.2).min(6.0) / 12.0;
    }
    let rr = r(u);
    let x = cx + rr * th.cos();
    let y = cy + rr * th.sin() * 0.3;
    Some((
        x,
        y,
        th.sin() < 0.0,
        ((26.0 - rr) / 18.0).clamp(0.0, 1.0),
        th,
    ))
}

fn draw_probe(g: &mut World, f: u32, t: f64) {
    let Some((x, y, _, fall, th)) = probe(g, f, t) else {
        return;
    };
    let (cx, cy) = centre(g);
    if fall <= 0.1 {
        glow(g, x, y, 4.0, 0x8ad8ff, 0.5);
        g.p(0x2a4a8a, x - 2.0, y, 2, 1);
        g.p(0x2a4a8a, x + 2.0, y, 2, 1);
        g.p(0xc8c8d8, x, y - 1.0, 2, 3);
        g.p(0xe8e8f0, x, y - 1.0, 1, 1);
        if f % 8 < 3 {
            g.p(0xff4a4a, x + 1.0, y + 1.0, 1, 1);
        }
        // A thin exhaust trail.
        if t < T_CAPTURE {
            line(g, x - 9.0, y, x - 3.0, y, |u| mix(SPACE, 0x5a8ad8, u));
        }
        return;
    }
    // Spaghettified: a smear back along its orbit, stretched toward the
    // hole and reddening as it goes.
    let r = ((x - cx).powi(2) + ((y - cy) / 0.3).powi(2)).sqrt();
    let len = 0.15 + fall * 1.4;
    let n = 24;
    for k in 0..n {
        let v = k as f64 / n as f64;
        let a = th - v * len;
        let px = cx + r * a.cos();
        let py = cy + r * a.sin() * 0.3;
        let c = mix(mix(0xffffff, 0x8ad8ff, fall), 0x3a5aa8, v);
        tint(g, js_round(px), js_round(py), c, 1.0 - v * 0.6);
        if v < 0.3 {
            tint(g, js_round(px), js_round(py) - 1, 0x8ad8ff, 0.5 - v);
        }
    }
    let pull = fall * 5.0;
    let (ux, uy) = ((cx - x) / r.max(1.0), (cy - y) / r.max(1.0));
    line(g, x, y, x + ux * pull, y + uy * pull, |u| {
        mix(0xffffff, 0x5ac8ff, u)
    });
    glow(g, x, y, 3.0 + fall * 2.0, 0xbfe8ff, 0.6);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    g.clear(SPACE);
    // A faint dust haze around the hole.
    let (cx, cy) = centre(g);
    for y in 0..60 {
        for x in 0..w {
            let d = (((x as f64 - cx) / 3.0).powi(2) + (y as f64 - cy).powi(2)).sqrt();
            if (1.0 - d / 40.0) * 0.5 > bayer(x, y) {
                g.p(0x120814, x as f64, y as f64, 1, 1);
            }
        }
    }
    haze(g, t);
    galaxy(g, t);
    stars(g, f, t);
    companion(g, f, t);
    let fl = flash(t);
    let boost = fl * 0.6;
    disk(g, t, false, boost);
    let behind = probe(g, f, t).is_some_and(|p| p.2 && p.3 > 0.0);
    if behind {
        draw_probe(g, f, t);
    }
    hole(g, t, boost);
    disk(g, t, true, boost);
    if !behind {
        draw_probe(g, f, t);
    }
    glow(g, cx, cy, 20.0, 0xff9a5a, 0.18 + boost * 0.4);

    // The flash: a white bloom, a shock ring, and jets out of the poles.
    if fl > 0.0 {
        let e = t - T_FALL;
        if e < 0.25 {
            let a = 0.9 * (1.0 - e / 0.25).sqrt();
            for y in 0..60 {
                for x in 0..w {
                    tint(g, x, y, 0xfff4e0, a);
                }
            }
        }
        glow(g, cx, cy, 8.0 + fl * 14.0, 0xffffff, fl);
        let rr = 10.0 + (1.0 - fl) * 70.0;
        let rc = mix(SPACE, 0xffe0b0, fl);
        let n = (rr * 5.0) as i32;
        for k in 0..n {
            let a = k as f64 / n as f64 * PI * 2.0;
            tint(
                g,
                js_round(cx + rr * a.cos()),
                js_round(cy + rr * a.sin() * 0.45),
                rc,
                fl,
            );
        }
        let jet = (e / 0.4).min(1.0) * 32.0;
        for side in [-1.0, 1.0] {
            for d in 0..js_round(jet) {
                let df = d as f64;
                let wob = (df * 0.5 - t * 20.0).sin() * 0.6;
                let a = fl * (1.0 - df / 34.0);
                let y = cy + side * (SHADOW_R + df);
                let x = js_round(cx + wob);
                tint(g, x, js_round(y), 0xffffff, a);
                tint(g, x - 1, js_round(y), 0xa8d0ff, a * 0.8);
                tint(g, x + 1, js_round(y), 0xa8d0ff, a * 0.8);
                tint(g, x - 2, js_round(y), 0x4a7aff, a * 0.4);
                tint(g, x + 2, js_round(y), 0x4a7aff, a * 0.4);
            }
        }
    }
}

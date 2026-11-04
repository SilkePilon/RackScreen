//! Jump to hyperspace: a small fighter cruises past a nebula and a night-side
//! planet, spools up its engines, the stars stretch into streaks that
//! radiate from a vanishing point on the last screen, the ship shoots into
//! it with a white flash, and the streaks collapse back into stars.

use super::scifi::{bayer, glow, line, mix, tint};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 10.0;

/// Cruise until the charge, hold while the stars stretch, jump, flash.
const T_CHARGE: f64 = 3.0;
const T_JUMP: f64 = 5.3;
const T_FLASH: f64 = 5.8;
const T_CALM: f64 = 7.8;

const SHIP: &[&str] = &[
    "rk..............",
    "kwk.............",
    ".kwk............",
    "..kwkkkkk.......",
    "eekdddddddkk....",
    "eekdoddccCddkk..",
    "eekddddcCCdddddk",
    "eekDoDDDDDDDkk..",
    "eekDDDDDDDkk....",
    "..kWkkkkk.......",
    ".kWk............",
    "kWk.............",
    "rk..............",
];
const SHIPP: &[(char, u32)] = &[
    ('k', 0x1e2232),
    ('d', 0xd6dce6),
    ('D', 0x8c95a8),
    ('w', 0xb8c0cc),
    ('W', 0x6c7588),
    ('o', 0xff7a3a),
    ('c', 0x7ae0ff),
    ('C', 0x2a7fb0),
    ('e', 0x4a5062),
    ('r', 0xff4a5a),
];

const SPACE: u32 = 0x05040d;

/// Warp speed, in log-radius per second, at time `t`.
fn warp_speed(t: f64) -> f64 {
    if t < T_CHARGE {
        0.0
    } else if t < T_FLASH {
        5.0 * ease((t - T_CHARGE) / (T_JUMP - T_CHARGE)).powi(2)
    } else {
        5.0 * (1.0 - ease((t - T_FLASH) / (T_CALM - T_FLASH)))
    }
}

/// The stars' drift and the planet's: the left edge of the world in
/// "space" px, summed frame by frame so a speed change never jumps.
fn warp_dist(f: u32) -> f64 {
    (0..f).map(|k| warp_speed(k as f64 / 12.0) / 12.0).sum()
}

/// Where the ship's nose is at `t`, and whether it is drawn.
fn ship_x(g: &World, t: f64) -> f64 {
    let hold = (g.panel(1) + 34) as f64;
    let vx = vanish(g).0;
    if t < T_CHARGE {
        let u = t / T_CHARGE;
        -20.0 + (hold + 20.0) * (1.0 - (1.0 - u) * (1.0 - u))
    } else if t < T_JUMP {
        hold - 3.0 * ease((t - T_CHARGE) / (T_JUMP - T_CHARGE))
    } else {
        let u = ((t - T_JUMP) / (T_FLASH - T_JUMP)).min(1.0);
        hold - 3.0 + (vx - 6.0 - hold) * u * u
    }
}

/// The vanishing point: the centre of the last screen.
fn vanish(g: &World) -> (f64, f64) {
    ((g.px0(g.layout.panels - 1) + 30) as f64, 30.0)
}

fn nebula(g: &mut World, t: f64) {
    let w = g.w();
    let nx = (g.panel(2) as f64) - t * 1.2;
    let pal = [0x0e0820, 0x1c0e34, 0x34164c, 0x55225e, 0x8a3e7e];
    for y in 0..60 {
        for x in 0..w {
            let (xf, yf) = (x as f64 + t * 1.2, y as f64);
            let n = 0.55
                + 0.22 * (xf * 0.045 + yf * 0.09 + 1.0).sin()
                + 0.16 * (xf * 0.11 - yf * 0.15).sin()
                + 0.12 * (xf * 0.023 + yf * 0.23 + 2.0).sin();
            let ex = (x as f64 - nx) / 150.0;
            let ey = (yf - 24.0) / 26.0;
            let v = n * (-(ex * ex) - ey * ey).exp() * 5.2;
            let lv = (v + bayer(x, y) - 0.5).floor().clamp(0.0, 5.0) as usize;
            if lv > 0 {
                let mut c = pal[lv - 1];
                // A teal vein through the brightest gas.
                if lv >= 3 && (xf * 0.09 - yf * 0.2).sin() > 0.85 {
                    c = 0x1e4a6a;
                }
                // A dark dust lane across the middle.
                if lv >= 2 && ((xf * 0.03 + 0.5).sin() * 6.0 + 26.0 - yf).abs() < 1.5 {
                    c = pal[lv - 2];
                }
                g.p(c, x as f64, yf, 1, 1);
            }
        }
    }
}

/// Young stars glowing pink-white inside the gas.
fn knots(g: &mut World, f: u32, t: f64) {
    for (i, (p, dx, y)) in [(1, 40, 18), (2, 10, 30), (2, 44, 14), (3, 12, 40)]
        .into_iter()
        .enumerate()
    {
        let x = (g.panel(p) + dx) as f64 - t * 1.2;
        let pulse = 0.35 + 0.1 * ((f as f64 * 0.3 + i as f64 * 2.0).sin());
        glow(g, x, y as f64, 5.0, 0xff9ad8, pulse);
        g.p(0xffe0f4, x, y as f64, 1, 1);
    }
}

fn planet(g: &mut World, t: f64) {
    // A night-side world along the bottom of the first two screens.
    let (cx, cy, r) = (g.px0(0) as f64 + 70.0 - t * 1.5, 170.0, 128.0);
    let w = g.w();
    for x in 0..w {
        let dx = x as f64 - cx;
        if dx.abs() >= r + 3.0 {
            continue;
        }
        let top_atm = cy - ((r + 3.0) * (r + 3.0) - dx * dx).sqrt();
        for y in js_round(top_atm).max(0)..60 {
            let d = ((dx * dx) + (y as f64 - cy).powi(2)).sqrt();
            let c = if d > r + 1.5 {
                if bayer(x, y) < 0.5 {
                    0x123a5a
                } else {
                    continue;
                }
            } else if d > r {
                0x5ad1ff
            } else if d > r - 1.5 {
                0x2a7aa8
            } else {
                // Lit limb on the right, the night side on the left.
                let lit = (dx / r + 0.15).clamp(0.0, 1.0);
                let band = ((y as f64 * 0.7 + (dx * 0.05).sin() * 3.0) as i32) % 3;
                let base = if band == 0 { 0x0c1e2c } else { 0x0a1824 };
                if lit > bayer(x, y) * 1.2 + 0.3 {
                    0x1a3e52
                } else {
                    base
                }
            };
            g.p(c, x as f64, y as f64, 1, 1);
        }
    }
    // City lights on the night side.
    for i in 0..g.spread(30) {
        let fi = i as f64;
        let x = cx - r * 0.95 + rnd(fi * 1.7 + 40.0) * r * 1.3;
        let dx = x - cx;
        if dx.abs() >= r - 3.0 {
            continue;
        }
        let top = cy - (r * r - dx * dx).sqrt();
        let y = top + 2.0 + rnd(fi * 2.3 + 9.0) * 14.0;
        let c = if rnd(fi + 77.0) > 0.7 {
            0xffe6a0
        } else {
            0xc88a3a
        };
        g.p(c, x, y, 1, 1);
    }
}

fn moon(g: &mut World, t: f64) {
    let (mx, my) = ((g.panel(3) + 40) as f64 - t * 0.6, 15.0);
    g.disc(0x3a3a4a, mx, my, 4);
    g.disc(0x8a8a9a, mx + 1.0, my - 1.0, 3);
    g.p(0x6a6a7a, mx, my - 1.0, 1, 1);
    g.p(0x6a6a7a, mx + 2.0, my + 1.0, 1, 1);
    g.p(0xb8b8c8, mx + 2.0, my - 3.0, 1, 1);
}

/// A beacon buoy drifting on the third screen, its light blinking.
fn beacon(g: &mut World, f: u32, t: f64) {
    let (bx, by) = ((g.panel(2) + 14) as f64 - t * 0.8, 46.0 + (t * 1.3).sin());
    g.p(0x3a3f52, bx, by, 3, 3);
    g.p(0x6a7088, bx, by, 2, 1);
    g.p(0x3a3f52, bx + 1.0, by - 3.0, 1, 3);
    g.p(0x2a2f40, bx - 2.0, by + 1.0, 2, 1);
    g.p(0x2a2f40, bx + 3.0, by + 1.0, 2, 1);
    if f % 12 < 3 {
        g.p(0xff5a4a, bx + 1.0, by - 4.0, 1, 1);
        tint(g, js_round(bx), js_round(by - 4.0), 0xff5a4a, 0.4);
        tint(g, js_round(bx + 2.0), js_round(by - 4.0), 0xff5a4a, 0.4);
        tint(g, js_round(bx + 1.0), js_round(by - 5.0), 0xff5a4a, 0.4);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let wf = w as f64;
    g.clear(SPACE);
    nebula(g, t);
    knots(g, f, t);
    moon(g, t);
    planet(g, t);
    beacon(g, f, t);

    let (vx, vy) = vanish(g);
    let ws = warp_speed(t);
    let wd = warp_dist(f);
    let warping = (T_CHARGE..T_FLASH).contains(&t);
    // Three layers of stars; while warping they fly out from the vanishing
    // point, their streaks pointing back at it.
    for (layer, (n, sp, c)) in [
        (54u32, 3.0, 0x4a4a6a),
        (34, 8.0, 0x9a9ac0),
        (18, 18.0, 0xffffff),
    ]
    .into_iter()
    .enumerate()
    {
        let lf = layer as f64;
        for i in 0..g.spread(n) {
            let fi = i as f64 + lf * 100.0;
            let x = ((rnd(fi + sp) * wf - t * sp) % wf + wf) % wf;
            let y = (rnd(fi + sp * 3.0) * 60.0).floor();
            let (dx, dy) = (x - vx, y - vy);
            let r0 = (dx * dx + dy * dy).sqrt().max(0.5);
            let (ux, uy) = (dx / r0, dy / r0);
            let depth = 0.6 + lf * 0.35;
            // While warping a star flies straight out at its own pace,
            // wrapping back to the vanishing point when it leaves the world.
            let pace = depth * (0.7 + rnd(fi * 5.3) * 0.6);
            let streak = ws * pace * (2.0 + r0.min(300.0) * 0.05);
            let (r, tail) = if warping {
                let (lo, hi) = (2.0, 400.0);
                (
                    lo + (r0 - lo + wd * 40.0 * pace).rem_euclid(hi - lo),
                    streak,
                )
            } else if t >= T_FLASH {
                (r0, streak)
            } else {
                (r0, 0.0)
            };
            let (hx, hy) = (vx + ux * r, vy + uy * r);
            if tail >= 1.0 {
                let (tx, ty) = (hx - ux * tail, hy - uy * tail);
                let far = mix(0x2a3a7a, c, 0.3);
                line(g, tx, ty, hx, hy, |u| {
                    if u > 0.85 {
                        0xffffff
                    } else if u > 0.5 {
                        mix(0xa8d8ff, c, 0.3)
                    } else if u > 0.2 {
                        0x5a7ad0
                    } else {
                        far
                    }
                });
            } else {
                let tw = (f + i * 7).is_multiple_of(37) && layer == 2;
                g.p(if tw { 0xbfe6ff } else { c }, hx, hy, 1, 1);
            }
        }
    }

    // A tunnel of streaks bursting out of the vanishing point itself.
    if ws > 0.2 {
        let (lo, hi) = (2.0f64.ln(), 70.0f64.ln());
        for k in 0..60 {
            let kf = k as f64;
            let th = rnd(kf * 3.1 + 500.0) * PI * 2.0;
            let lr = lo + (rnd(kf * 1.3 + 900.0) * (hi - lo) + wd * 1.3).rem_euclid(hi - lo);
            let r = lr.exp();
            let tail = r * (1.0 - (-ws * 0.2).exp());
            let (ux, uy) = (th.cos(), th.sin() * 0.85);
            let c = if k % 3 == 0 { 0xffffff } else { 0x8ab8ff };
            line(
                g,
                vx + ux * (r - tail),
                vy + uy * (r - tail),
                vx + ux * r,
                vy + uy * r,
                |u| mix(0x1a2a6a, c, u),
            );
        }
        glow(g, vx, vy, 3.0 + ws, 0xbfe6ff, (ws / 5.0) * 0.8);
    }

    // The ship: cruise in, hold and shudder while charging, then shoot into
    // the vanishing point.
    if t < T_FLASH {
        let x = ship_x(g, t);
        let charge = ((t - T_CHARGE) / (T_JUMP - T_CHARGE)).clamp(0.0, 1.0);
        let shudder = if charge > 0.3 && f.is_multiple_of(2) {
            1.0
        } else {
            0.0
        };
        let y = 24.0 + js_round((t * 2.0).sin() * 1.5 * (1.0 - charge)) as f64 - shudder;
        let sx = x - 16.0;
        if t >= T_JUMP {
            // The trail: a white-hot bar back to where the jump began.
            let hold = (g.panel(1) + 18) as f64;
            for (dy, c) in [(5.0, 0x5ad1ff), (6.0, 0xffffff), (7.0, 0x5ad1ff)] {
                line(g, hold, y + dy, sx + 8.0, y + dy, |u| mix(0x1a3a7a, c, u));
            }
            for (dy, c) in [(4.0, 0x2a5aa0), (8.0, 0x2a5aa0)] {
                line(g, sx - 30.0, y + dy, sx + 6.0, y + dy, |u| mix(SPACE, c, u));
            }
        }
        // Engine plume: orange flicker at cruise, blue-white when charged.
        let fl = (f >> 1) % 2 == 1;
        let hot = charge > 0.0 || t >= T_JUMP;
        let plume = if hot {
            3.0 + charge * 7.0 + if fl { 1.0 } else { 0.0 }
        } else if fl {
            4.0
        } else {
            3.0
        };
        for k in 0..js_round(plume) {
            let u = k as f64 / plume;
            let c = if hot {
                mix(0xffffff, 0x3a7ad8, u)
            } else {
                mix(0xffe27a, 0xff4a2a, u)
            };
            let hh = if u < 0.4 { 3 } else { 1 };
            g.p(c, sx - 1.0 - k as f64, y + 6.0 - (hh / 2) as f64, 1, hh);
        }
        if hot {
            glow(
                g,
                sx,
                y + 6.0,
                4.0 + charge * 6.0,
                0x7ad0ff,
                0.5 + 0.3 * charge,
            );
            // Charge rings ripple back off the engines.
            let rp = (t * 3.0).fract();
            if charge > 0.2 && t < T_JUMP {
                g.ring(
                    mix(0x7ad0ff, SPACE, rp),
                    sx - rp * 8.0,
                    y + 6.0,
                    2.0 + rp * 6.0,
                );
            }
        }
        g.spr(SHIP, SHIPP, sx, y, false);
        // Wing-tip lights blink at cruise.
        if f % 8 >= 4 {
            g.p(0x5a1a22, sx, y, 1, 1);
            g.p(0x5a1a22, sx, y + 12.0, 1, 1);
        }
        if t >= T_JUMP {
            glow(g, sx + 8.0, y + 6.0, 6.0, 0xffffff, 0.6);
        }
    }

    // The flash where the ship vanished, and its fading ring.
    let ft = t - T_FLASH;
    if (0.0..1.6).contains(&ft) {
        if ft < 0.35 {
            let a = (1.0 - ft / 0.35).powi(2);
            for y in 0..60 {
                for x in 0..w {
                    tint(g, x, y, 0xffffff, a);
                }
            }
        }
        let r = 2.0 + ease(ft / 1.6) * 26.0;
        glow(
            g,
            vx,
            vy,
            (14.0 - ft * 6.0).max(2.0),
            0xffffff,
            (1.0 - ft / 1.6).powi(2),
        );
        let rc = mix(0xbfe6ff, SPACE, ft / 1.6);
        g.ring(rc, vx, vy, r);
        g.ring(mix(0x3a6ad0, SPACE, ft / 1.6), vx, vy, r * 0.7);
        // Four spikes of the lens flare.
        let sl = 12.0 * (1.0 - ft / 1.6);
        for a in 0..4 {
            let th = a as f64 * PI / 2.0;
            line(
                g,
                vx,
                vy,
                vx + th.cos() * sl,
                vy + th.sin() * sl * 0.6,
                |u| mix(0xffffff, 0x5a7ad0, u),
            );
        }
    }
    // Long after, a last twinkle where the ship went.
    let tt = t - 8.6;
    if (0.0..0.9).contains(&tt) && !f.is_multiple_of(3) {
        let s = js_round(2.0 * (tt / 0.9 * PI).sin());
        g.p(0xffffff, vx, vy, 1, 1);
        g.p(0xbfe6ff, vx - s as f64, vy, 2 * s + 1, 1);
        g.p(0xbfe6ff, vx, vy - s as f64, 1, 2 * s + 1);
    }
}

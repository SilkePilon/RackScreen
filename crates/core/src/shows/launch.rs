//! Lift-off: a rocket on a coastal pad at dawn, floodlit beside its red
//! gantry. Countdown lamps light one by one, the engines ignite in
//! billowing clouds of smoke, and the rocket climbs and pitches over
//! across the screens; the sky darkens to space, the first stage falls
//! away, the fairing opens and a tiny satellite unfolds its panels.

use super::world::{js_round, rnd, World};

pub const SECS: f64 = 12.0;

const IGNITE: f64 = 3.2;
const LIFT: f64 = 3.8;
const PITCH: f64 = 5.2;
const STAGE: f64 = 8.3;
const FAIRING: f64 = 10.0;
const RELEASE: f64 = 10.3;

/// Sky colours top to bottom, from the dawn coast (0) to space (3).
const SKY: [[u32; 5]; 4] = [
    [0x2a3a7a, 0x4a5a9a, 0x8a7aa8, 0xd89a8a, 0xf6c490],
    [0x1e2e6e, 0x30508e, 0x5272ae, 0x84a2cc, 0xb0c8e2],
    [0x0a1240, 0x12245e, 0x1c3c7c, 0x2c5c9a, 0x4a7ab8],
    [0x02030c, 0x040614, 0x060a1e, 0x0a1230, 0x10204a],
];

fn bayer(x: i32, y: i32) -> f64 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[(y & 3) as usize][(x & 3) as usize] as f64 + 0.5) / 16.0
}

/// Draw a rocket section along its axis from `(bx, by)` at angle `ang`
/// (0 = straight up, positive leans right), from `a0` to `a1` px along it.
fn rocket(g: &mut World, bx: f64, by: f64, ang: f64, a0: f64, a1: f64) {
    let (ux, uy) = (ang.sin(), -ang.cos());
    let (vx, vy) = (ang.cos(), ang.sin());
    let mut a = a0;
    while a <= a1 {
        // Half width: fins at the base, nose cone at the top.
        let hw: f64 = if a < 3.0 {
            2.5 - a * 0.3
        } else if a > 18.0 {
            (1.5 - (a - 18.0) * 0.35).max(0.0)
        } else {
            1.5
        };
        let mut b = -hw;
        while b <= hw {
            let c = if a < 3.0 && b.abs() > 1.4 {
                0xa82a2a
            } else if (12.0..13.0).contains(&a)
                || (4.0..5.0).contains(&a)
                || (8.0..8.5).contains(&a)
            {
                0x2a2a34
            } else if b > 0.5 {
                0xb8bcc8
            } else {
                0xf4f4f4
            };
            g.p(c, bx + ux * a + vx * b, by + uy * a + vy * b, 1, 1);
            b += 0.5;
        }
        a += 0.5;
    }
}

/// A flickering exhaust flame of length `len` behind `(bx, by)`.
fn flame(g: &mut World, bx: f64, by: f64, ang: f64, len: f64, f: u32, blue: bool) {
    let (ux, uy) = (-ang.sin(), ang.cos());
    let (vx, vy) = (ang.cos(), ang.sin());
    let len = len * if f.is_multiple_of(2) { 1.0 } else { 0.8 };
    let mut a = 0.0;
    while a <= len {
        let u = a / len;
        let hw = (1.6 * (1.0 - u)).max(0.3);
        let c = if blue {
            if u < 0.4 {
                0xe8f4ff
            } else {
                0x6ab8ff
            }
        } else if u < 0.25 {
            0xfffbe0
        } else if u < 0.55 {
            0xffd060
        } else {
            0xff7a2a
        };
        let mut b = -hw;
        while b <= hw {
            g.p(c, bx + ux * a + vx * b, by + uy * a + vy * b, 1, 1);
            b += 0.5;
        }
        a += 0.5;
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, wf) = (g.w(), g.w() as f64);
    let fi = f as i32;
    let pad = (g.panel(0) + 32) as f64;
    let end = (g.panel(2) + 22) as f64;
    let orbit = (g.panel(3) + 26) as f64;
    // Rocket base and angle over time.
    // The gravity turn: from straight up to level flight over panel 2.
    let turn = |u: f64| -> (f64, f64, f64) {
        let u = u.clamp(1e-3, 1.0);
        let x = pad + (end - pad) * u.powf(1.6);
        let y = 30.0 - 16.0 * u.powf(0.7);
        let dx = (end - pad) * 1.6 * u.powf(0.6);
        let dy = -16.0 * 0.7 * u.powf(-0.3);
        (x, y, dx.atan2(-dy))
    };
    let path = |tau: f64| -> (f64, f64, f64) {
        if tau < LIFT {
            (pad, 44.0, 0.0)
        } else if tau < PITCH {
            let u = (tau - LIFT) / (PITCH - LIFT);
            (pad, 44.0 - 14.0 * u * u, 0.0)
        } else {
            turn((tau - PITCH) / (STAGE - PITCH))
        }
    };

    // Sky: dawn over the coast on the left, space on the right.
    let (sa, sb) = ((g.panel(1) + 20) as f64, g.panel(3) as f64);
    for x in 0..w {
        let s = ((x as f64 - sa) / (sb - sa)).clamp(0.0, 1.0) * 3.0;
        for y in 0..60 {
            let lv = ((s + (bayer(x, y) - 0.5) * 0.5).round() as usize).min(3);
            let band = (((y as f64 / 9.0) + bayer(x + 1, y) * 0.9 - 0.45).floor() as usize).min(4);
            g.p(SKY[lv][band], x as f64, y as f64, 1, 1);
        }
    }
    // Stars where the sky is dark enough.
    for i in 0..g.spread(70) {
        let fv = i as f64;
        let x = rnd(fv) * wf;
        let s = ((x - sa) / (sb - sa)).clamp(0.0, 1.0);
        if rnd(fv + 2.0) > s * 1.3 - 0.2 {
            continue;
        }
        let tw = (f + i * 5) % 31 < 2;
        g.p(
            if tw { 0xffffff } else { 0x9aa4d0 },
            x,
            rnd(fv + 1.0) * 46.0,
            1,
            1,
        );
    }
    // The low sun on panel 1, glittering on the sea.
    let sun = (g.panel(1) + 40) as f64;
    g.disc(0xffe0a0, sun, 43.0, 5);
    g.disc(0xfff4d0, sun, 43.0, 3);
    // Wisps of cloud low over the coast.
    for k in 0..g.spread(5) {
        let kf = k as f64;
        let x = (rnd(kf + 5.0) * sb + t * 1.5) % sb - 10.0;
        let y = 28.0 + rnd(kf + 6.0) * 10.0;
        g.p(0xf0b8a0, x, y, 12, 1);
        g.p(0xd89aa0, x + 3.0, y + 1.0, 10, 1);
    }

    // Sea from the coast to panel 2; a beach, palms and the launch site.
    let coast = (g.panel(1) + 14) as f64;
    for x in js_round(coast)..js_round(sb) {
        let xf = x as f64;
        g.p(0x2a4a8a, xf, 44.0, 1, 16);
        if (x * 3 + fi / 2) % 7 == 0 {
            g.p(0x6a8ac0, xf, (46 + x % 11) as f64, 2, 1);
        }
        if (xf - sun).abs() < 6.0 && (x + fi / 2) % 3 == 0 {
            g.p(0xffd890, xf, (45 + (x * 7) % 9) as f64, 2, 1);
        }
    }
    g.p(0x8aa8d8, coast, 44.0, js_round(sb - coast), 1);
    // Ground of the launch site.
    for x in 0..js_round(coast) + 6 {
        let xf = x as f64;
        let top = 46
            + if x as f64 > coast - 6.0 {
                (x as f64 - coast + 6.0) as i32 / 3
            } else {
                0
            };
        g.p(0x3a4a3a, xf, top as f64, 1, 60 - top);
        if (x * 5) % 9 < 2 {
            g.p(0x2a3a2e, xf, (top + 3) as f64, 1, 1);
        }
    }
    g.p(0xd8c8a0, coast - 2.0, 47.0, 10, 2);
    // Pad: concrete deck, flame trench.
    g.p(0x8a8a90, pad - 10.0, 45.0, 21, 2);
    g.p(0x5a5a64, pad - 10.0, 47.0, 21, 2);
    g.p(0x1a1a20, pad - 3.0, 47.0, 7, 2);
    // Palms and a water tower around the site.
    for (px, ht, lean) in [
        (g.panel(0) as f64 + 6.0, 11, 1.0),
        (coast - 12.0, 9, -1.0),
        (coast - 3.0, 12, 1.0),
    ] {
        for j in 0..ht {
            g.p(
                0x3a2a20,
                px + lean * (j as f64 / ht as f64).powi(2) * 2.0,
                (46 - j) as f64,
                1,
                1,
            );
        }
        let (tx, ty) = (px + lean * 2.0, (46 - ht) as f64);
        let sway = js_round((t * 1.8 + px).sin() * 0.6) as f64;
        for (dx, dy) in [
            (-3.0, 1.0),
            (-2.0, 0.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (3.0, 1.0),
            (-4.0, 2.0),
            (4.0, 2.0),
        ] {
            g.p(0x1e3a2a, tx + dx + sway * dy, ty + dy, 1, 1);
        }
    }
    let wt = g.panel(0) as f64 + 14.0;
    g.p(0x6a6a74, wt, 38.0, 1, 8);
    g.p(0x6a6a74, wt + 4.0, 38.0, 1, 8);
    g.p(0xd8d8e0, wt - 1.0, 33.0, 7, 5);
    g.p(0xa8a8b4, wt + 3.0, 33.0, 3, 5);
    // Spectators on the beach of panel 1, camera flashes at lift-off.
    for k in 0..6 {
        let x = coast - 10.0 + k as f64 * 2.0;
        g.p(0x1a1a28, x, 46.0, 1, 3);
        g.p(0xe0b090, x, 45.0, 1, 1);
        if t > LIFT && (f + k * 3).is_multiple_of(11) {
            g.p(0xffffff, x + 1.0, 44.0, 1, 1);
        }
    }
    // Floodlight towers with beams on the rocket until lift-off.
    for (lx, dir) in [(pad - 22.0, 1.0), (pad + 20.0, -1.0)] {
        g.p(0x4a4a54, lx, 22.0, 1, 24);
        g.p(0xfff4c0, lx - 1.0, 21.0, 3, 2);
        if t < LIFT + 1.0 {
            for k in 0..14 {
                let kf = k as f64;
                let spread = kf * 0.45;
                let x = lx + dir * (kf + 1.0);
                for j in -js_round(spread)..=js_round(spread) {
                    let y = 22.0 + kf * 0.6 + j as f64;
                    let fade = 0.55 - kf * 0.025 - (j as f64).abs() * 0.06;
                    if bayer(x as i32, y as i32) < fade {
                        g.p(0xf8ecc0, x, y, 1, 1);
                    }
                }
            }
        }
    }
    // The gantry: a red lattice tower and an access arm that swings away.
    let gx = pad - 9.0;
    g.p(0xb84a2a, gx, 16.0, 1, 30);
    g.p(0xb84a2a, gx + 4.0, 16.0, 1, 30);
    for k in 0..10 {
        let y = 17.0 + k as f64 * 3.0;
        g.p(0x7a2a1a, gx + 1.0, y, 3, 1);
        g.p(0x7a2a1a, gx + 1.0 + (k % 2) as f64 * 2.0, y + 1.0, 1, 1);
    }
    g.p(0x8a3a1a, gx - 1.0, 15.0, 7, 1);
    if (f / 4).is_multiple_of(2) {
        g.p(0xff3a3a, gx + 2.0, 14.0, 1, 1);
    }
    let arm = (1.0 - ((t - 2.2) / 0.6).clamp(0.0, 1.0)) * 4.0;
    if arm > 0.0 {
        g.p(0xd8d8e0, gx + 5.0, 26.0, js_round(arm), 1);
    }
    // Countdown lamps on a board: red one by one, then all green.
    let board = g.panel(0) as f64 + 18.0;
    g.p(0x1a1a24, board, 39.0, 13, 4);
    for k in 0..5 {
        let lit = t > 0.4 + k as f64 * 0.55;
        let c = if t >= IGNITE {
            0x5aff7a
        } else if lit {
            0xff4a3a
        } else {
            0x3a2a2a
        };
        g.p(c, board + 1.0 + k as f64 * 2.5, 40.0, 2, 2);
    }

    // The contrail along the climb, widening and fading with age.
    if t > LIFT {
        let mut tau = LIFT;
        while tau < t.min(STAGE) {
            let (x, y, _) = path(tau);
            let age = t - tau;
            let r = js_round((1.0 + age * 0.5).min(2.5));
            let c = if age < 1.0 {
                0xf8f8fc
            } else if age < 3.0 {
                0xd8dce8
            } else {
                0xb0b6cc
            };
            let dens = (1.2 - (age - 1.5).max(0.0) * 0.3).max(0.3);
            let (cx, cy) = (js_round(x + age * 1.2), js_round(y + 1.0 + age * 0.6));
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r + r && bayer(cx + dx, cy + dy) < dens {
                        g.p(c, (cx + dx) as f64, (cy + dy) as f64, 1, 1);
                    }
                }
            }
            tau += 0.03;
        }
    }
    // Venting before ignition, then billowing smoke over the pad.
    if t < IGNITE {
        for k in 0..3 {
            let age = ((t * 1.5 + k as f64 * 0.33) % 1.0) * 1.0;
            g.p(
                0xf0f4ff,
                pad + 2.0 + age * 6.0,
                32.0 - age * 3.0,
                1 + js_round(age),
                1,
            );
        }
    }
    if t > IGNITE {
        for k in 0..18 {
            let kf = k as f64;
            let born = IGNITE + kf * 0.12;
            let age = t - born;
            if age < 0.0 {
                continue;
            }
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let reach = 26.0 * (1.0 - (-age * 0.9).exp()) * (0.6 + rnd(kf) * 0.5);
            let x = pad + side * reach;
            let y = 44.0 - age * 1.6 - rnd(kf + 1.0) * 3.0;
            let r = js_round((2.0 + age * 1.8).min(6.0));
            let c = if age < 1.0 { 0xffe8c0 } else { 0xe8e8ee };
            g.disc(0xa8aab8, x, y + 1.0, r);
            g.disc(c, x, y, r - 1);
        }
    }

    // The rocket: the full stack until staging, then the upper stage.
    if t < STAGE {
        let (x, y, ang) = path(t);
        if t > IGNITE {
            let len = if t < LIFT { 4.0 } else { 8.0 };
            flame(g, x, y, ang, len, f, false);
        }
        rocket(g, x, y, ang, 0.0, 22.0);
    } else {
        // The spent first stage tumbles down towards the clouds.
        let e = t - STAGE;
        let (x, y, ang) = path(STAGE);
        let (sx, sy) = (x - e * 6.0, y + e * e * 4.0);
        if sy < 62.0 {
            rocket(g, sx, sy, ang + e * 1.6, 0.0, 12.0);
            if e < 1.2 {
                for k in 0..4 {
                    let a = k as f64 * 1.7 + e * 4.0;
                    g.p(
                        0xd8d8e0,
                        sx + a.cos() * e * 6.0,
                        sy + a.sin() * e * 4.0,
                        1,
                        1,
                    );
                }
            }
        }
        // The upper stage coasts on to orbit with a pale blue flame.
        let u = ((t - STAGE) / (FAIRING - STAGE)).min(1.0);
        let ue = 1.0 - (1.0 - u) * (1.0 - u);
        let (ox, oy) = (x + (orbit - 24.0 - x) * ue, y + (22.0 - y) * ue);
        let base = (ox, oy);
        if t < FAIRING && t > STAGE + 0.25 {
            flame(
                g,
                base.0 + 13.0,
                base.1,
                std::f64::consts::FRAC_PI_2,
                5.0,
                f,
                true,
            );
        }
        let sat = t > FAIRING;
        if t < RELEASE {
            rocket(
                g,
                base.0,
                base.1,
                std::f64::consts::FRAC_PI_2,
                13.0,
                if sat { 18.0 } else { 22.0 },
            );
        }
        // Fairing halves spring away; the satellite drifts free.
        if sat {
            let e = t - FAIRING;
            let nx = base.0 + 18.0;
            for side in [-1.0, 1.0] {
                let px = nx + e * 4.0;
                let py = base.1 + side * (1.5 + e * 5.0);
                g.p(0xf4f4f4, px, py, 4, 1);
                g.p(0xb8bcc8, px + 1.0, py + side, 2, 1);
            }
            let e2 = (t - RELEASE).max(0.0);
            let (sx, sy) = (nx + 1.0 + e2.min(1.0) * 6.0, base.1 - 1.0);
            if t >= RELEASE {
                // The stage backs away.
                rocket(
                    g,
                    base.0 - e2 * 5.0,
                    base.1 + e2 * 2.0,
                    std::f64::consts::FRAC_PI_2 + e2 * 0.2,
                    13.0,
                    18.0,
                );
            }
            // Solar wings unfold, then a blinking light and signal rings.
            let open = ((t - RELEASE - 0.3) / 0.9).clamp(0.0, 1.0);
            let wing = js_round(open * 8.0);
            g.p(0xe0b040, sx, sy, 3, 3);
            g.p(0xb08a2a, sx + 2.0, sy, 1, 3);
            g.p(0xd8d8e0, sx + 1.0, sy - 2.0, 1, 2);
            for side in [-1, 1] {
                for k in 0..wing {
                    let x = if side < 0 {
                        sx - 2.0 - k as f64
                    } else {
                        sx + 4.0 + k as f64
                    };
                    g.p(
                        if k % 3 == 2 { 0x8aa0e0 } else { 0x2a4ab0 },
                        x,
                        sy - 1.0,
                        1,
                        5,
                    );
                }
                if wing > 0 {
                    g.p(
                        0x8a8a94,
                        if side < 0 { sx - 1.0 } else { sx + 3.0 },
                        sy + 1.0,
                        1,
                        1,
                    );
                }
            }
            if open >= 1.0 {
                if (f / 3).is_multiple_of(2) {
                    g.p(0xff4a4a, sx + 1.0, sy - 3.0, 1, 1);
                }
                let e3 = t - RELEASE - 1.2;
                for k in 0..2 {
                    let r = ((e3 * 6.0 + k as f64 * 4.0) % 8.0) + 2.0;
                    for a in -2..=2 {
                        let th = std::f64::consts::FRAC_PI_2 + a as f64 * 0.25;
                        g.p(
                            0x8ae0ff,
                            sx + 1.0 + th.cos() * r,
                            sy + 3.0 + th.sin() * r,
                            1,
                            1,
                        );
                    }
                }
            }
        }
    }

    // Far below on panel 2: a sea of cloud tops; panel 3: the Earth's limb.
    let p2 = g.panel(2) as f64;
    for x in js_round(p2) - 10..js_round(sb) {
        let xf = x as f64;
        let top = 50.0 + (xf * 0.3).sin() * 1.5 + (xf * 0.11).sin() * 2.0;
        let top = js_round(top);
        g.p(0xe8ecf8, xf, top as f64, 1, 60 - top);
        g.p(0xb8c4e0, xf, (top + 3) as f64, 1, 60 - top - 3);
    }
    let cx = (g.panel(3) + 30) as f64;
    for x in js_round(sb) - 4..w {
        let d = (x as f64 - cx) / 60.0;
        let top = js_round(47.0 + d * d * 30.0);
        g.p(0x6ac8ff, x as f64, (top - 1) as f64, 1, 1);
        g.p(0x1a4a9a, x as f64, top as f64, 1, 60 - top);
        for y in top..60 {
            if ((x as f64 * 0.3 + y as f64 * 0.8 + t * 0.4).sin()
                + (x as f64 * 0.11 - y as f64).sin())
                > 1.2
            {
                g.p(0xe8f0ff, x as f64, y as f64, 1, 1);
            }
        }
    }
    // A small moon high on the last screen.
    g.disc(0xd8d8e0, cx + 14.0, 9.0, 3);
    g.p(0xa8a8b8, cx + 13.0, 8.0, 2, 1);
}

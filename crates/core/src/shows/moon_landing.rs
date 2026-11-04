//! Lunar landing: over a cratered grey moonscape with the Earth rising on
//! the last screen, a lander descends on its thruster, kicks up dust and
//! touches down; a tiny astronaut climbs out, bounces over in low gravity,
//! plants a flag and waves, while an old rover parked nearby takes a photo.

use super::scifi::{bayer, glow, line, mix, tint};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_FINAL: f64 = 4.6;
const T_DOWN: f64 = 6.2;
const T_HATCH: f64 = 7.0;
const T_LADDER: f64 = 7.4;
const T_HOP: f64 = 8.2;
const T_FLAG: f64 = 9.6;
const T_WAVE: f64 = 10.3;

/// Ground line at the landing site.
const GROUND: f64 = 47.0;

const LANDER: &[&str] = &[
    ".......d.d.......",
    ".......ddd.......",
    ".....kkkkkkk.....",
    "....kssssssssk...",
    "...ksswwssssssk..",
    "...kssswssssshk..",
    "...kkkkkkkkkkkk..",
    "..gGgGgGgGgGgGgk.",
    "..GgGgGgGgGgGgGk.",
    "..gGgGgGgGgGgGgk.",
    "..kkkkkbbbkkkkkk.",
    ".......bbb.......",
];
const LANDERP: &[(char, u32)] = &[
    ('d', 0xc8c8d0),
    ('k', 0x2a2a30),
    ('s', 0xb8bcc8),
    ('S', 0x8a8e9a),
    ('w', 0x1a2a4a),
    ('h', 0x6a6e7a),
    ('g', 0xd8a838),
    ('G', 0xa87a20),
    ('b', 0x4a4a52),
];
/// Height of the lander from its top to the soles of its footpads.
const LANDER_H: f64 = 15.0;

const ASTRO: [&[&str]; 3] = [
    // Standing.
    &[".ww.", "wvvw", "bwww", "bww.", ".ww.", ".w.w"],
    // Mid-hop, legs tucked.
    &[".ww.", "wvvw", "bwww", "bwww", ".ww.", "w..w"],
    // Waving: the far arm up.
    &[".ww.w", "wvvww", "bwww.", "bww..", ".ww..", ".w.w."],
];
const ASTROP: &[(char, u32)] = &[('w', 0xf0f0f4), ('v', 0xf0b030), ('b', 0xa8a8b4)];

const ROVER: &[&str] = &[
    ".........cc.",
    ".........ck.",
    ".........k..",
    ".ppppppp.k..",
    ".PPPPPPP.k..",
    "kmmmmmmmmmk.",
    "kMMMMMMMMMk.",
    ".k..k..k..k.",
    "ooo.ooo.ooo.",
    ".o...o...o..",
];
const ROVERP: &[(char, u32)] = &[
    ('c', 0x3a3a44),
    ('k', 0x4a4a54),
    ('p', 0x3a5a9a),
    ('P', 0x24345a),
    ('m', 0xa8783a),
    ('M', 0x7a5428),
    ('o', 0x2a2a30),
];

fn sky(g: &mut World, f: u32) {
    let (w, wf) = (g.w(), g.w() as f64);
    g.clear(0x020208);
    // A faint band of the galaxy, sloping across.
    for x in 0..w {
        let cy = 6.0 + x as f64 * 0.05 + (x as f64 * 0.04).sin() * 3.0;
        for dy in -4..=4 {
            let y = js_round(cy) + dy;
            let a = 1.0 - (dy as f64).abs() / 5.0;
            if a * 0.6 > bayer(x, y) {
                g.p(0x14142a, x as f64, y as f64, 1, 1);
            }
        }
    }
    for i in 0..g.spread(70) {
        let fi = i as f64;
        let (x, y) = (
            (rnd(fi * 1.7) * wf).floor(),
            (rnd(fi * 3.3 + 1.0) * 36.0).floor(),
        );
        let c = if (f + i * 11) % 53 < 2 {
            0xffffff
        } else if i % 4 == 0 {
            0xc8c8e0
        } else {
            0x5a5a78
        };
        g.p(c, x, y, 1, 1);
    }
}

fn earth(g: &mut World, t: f64) {
    let (cx, cy) = (
        (g.px0(g.layout.panels - 1) + 30) as f64,
        26.0 - t / SECS * 10.0,
    );
    glow(g, cx, cy, 15.0, 0x3a6ad8, 0.35);
    let r = 10i32;
    for y in -r..=r {
        for x in -r..=r {
            let d2 = x * x + y * y;
            if d2 > r * r + 3 {
                continue;
            }
            let (xf, yf) = (x as f64, y as f64);
            // Continents from a few waves, clouds sliding over them.
            let land = (xf * 0.6 + yf * 0.3 + 1.0).sin() + (yf * 0.8 - xf * 0.2).cos() * 0.8;
            let cloud =
                ((xf + t * 0.8) * 0.8 + (yf * 1.3).sin() * 2.2).sin() * (yf * 0.9 + xf * 0.3).cos();
            let mut c = if land > 0.9 { 0x3a8a4a } else { 0x2a5ad8 };
            if land > 1.4 {
                c = 0xa8905a;
            }
            if cloud > 0.55 || y.abs() > 8 {
                c = 0xf0f4ff;
            }
            // Night on the left, the terminator dithered.
            let lit = (xf + 2.0) / r as f64;
            if lit < -0.2 || (lit < 0.2 && bayer(x, y) > (lit + 0.2) / 0.4) {
                c = mix(c, 0x05081a, 0.8);
            }
            if d2 > r * r - 4 && lit > 0.0 {
                c = mix(c, 0x9ad0ff, 0.5);
            }
            g.p(c, cx + xf, cy + yf, 1, 1);
        }
    }
}

/// An orbiter crossing the sky, blinking.
fn orbiter(g: &mut World, f: u32, t: f64) {
    let x = -10.0 + t / SECS * (g.w() as f64 + 20.0);
    let y = 9.0 + (x * 0.01).sin() * 2.0;
    g.p(0xb8b8c8, x, y, 2, 1);
    if f % 12 < 2 {
        g.p(0xffffff, x + 1.0, y, 1, 1);
        tint(g, js_round(x) + 1, js_round(y) - 1, 0xffffff, 0.5);
    }
}

/// Two ranges of jagged peaks; the faces turned to the sun on the right
/// are lit, with a darker gully down each.
fn mountains(g: &mut World) {
    let w = g.w();
    for (seed, base, every, tall, lit, shade, rim) in [
        (3.0, 36.0, 26, 13.0, 0x6a6a7a, 0x34343e, 0x9a9aa8),
        (9.0, 41.0, 19, 6.0, 0x8a8a94, 0x4e4e58, 0xb8b8c0),
    ] {
        let n = w / every + 2;
        for x in 0..w {
            let xf = x as f64;
            let (mut h, mut right, mut dist) = (0.0f64, false, 0.0);
            for k in -1..n {
                let kf = k as f64;
                let px = kf * every as f64 + rnd(kf + seed) * every as f64 * 0.8;
                let ph = tall * (0.45 + rnd(kf * 1.7 + seed) * 0.55);
                let sl = 0.5 + rnd(kf * 2.3 + seed) * 0.6;
                let v = ph - (xf - px).abs() * sl;
                if v > h {
                    (h, right, dist) = (v, xf > px, (xf - px).abs());
                }
            }
            let top = js_round(base - h);
            let mut c = if right { lit } else { shade };
            if right && (dist - h * 0.6).abs() < 0.6 && dist > 1.0 {
                c = shade;
            }
            g.p(c, xf, top as f64, 1, 44 - top);
            g.p(if right { rim } else { c }, xf, top as f64, 1, 1);
        }
    }
}

/// The plain: grey regolith, lighter towards the viewer, craters with a
/// sunlit rim on the right and shade on the left.
fn ground(g: &mut World) {
    let w = g.w();
    for y in 40..60 {
        let u = (y - 40) as f64 / 20.0;
        for x in 0..w {
            let k = (u * 3.0 + bayer(x, y) - 0.5).floor().clamp(0.0, 3.0);
            let c = [0x5a5a62, 0x6a6a72, 0x7a7a82, 0x86868e][k as usize];
            g.p(c, x as f64, y as f64, 1, 1);
        }
    }
    let wf = w as f64;
    for i in 0..g.spread(26) {
        let fi = i as f64;
        let cx = rnd(fi * 2.9 + 7.0) * wf;
        let cy = 42.0 + rnd(fi * 1.3 + 2.0) * 16.0;
        let r = 1.5 + rnd(fi * 4.1) * (1.0 + (cy - 40.0) * 0.15);
        crater(g, cx, cy, r);
    }
    // Pebbles.
    for i in 0..g.spread(90) {
        let fi = i as f64;
        let x = rnd(fi * 5.7 + 1.0) * wf;
        let y = 41.0 + rnd(fi * 2.2 + 8.0) * 19.0;
        let c = if i % 3 == 0 { 0xa8a8b0 } else { 0x4a4a52 };
        g.p(c, x, y, 1, 1);
    }
    // Two big ones: on the first screen and the second.
    crater(g, (g.panel(0) + 22) as f64, 50.0, 8.0);
    crater(g, (g.panel(1) + 12) as f64, 44.0, 4.5);
    // Boulders with shadows to the left.
    for (p, dx, y, s) in [
        (0, 46, 42, 3),
        (1, 50, 52, 4),
        (2, 54, 40, 2),
        (3, 10, 50, 4),
        (3, 44, 44, 2),
    ] {
        let x = (g.panel(p) + dx) as f64;
        g.p(
            0x34343c,
            x - s as f64 - 1.0,
            y as f64 + s as f64 - 1.0,
            s + 2,
            1,
        );
        g.disc(0x6a6a74, x, y as f64, s / 2 + 1);
        g.p(0xb0b0b8, x + 1.0, y as f64 - 1.0, 1, 1);
        g.p(0x4a4a54, x - 1.0, y as f64 + 1.0, 1, 1);
    }
}

fn crater(g: &mut World, cx: f64, cy: f64, r: f64) {
    let ri = r.ceil() as i32 + 1;
    for y in -ri..=ri {
        for x in -ri..=ri {
            let (xf, yf) = (x as f64, y as f64 * 2.2);
            let d = (xf * xf + yf * yf).sqrt();
            let (px, py) = (js_round(cx) + x, js_round(cy) + y);
            // The sun is on the right: the inner right wall is in shadow,
            // the inner left wall and the outer right rim catch the light.
            let ds = ((xf + r * 0.45).powi(2) + yf * yf).sqrt();
            let c = if d < r - 0.7 {
                if ds > r * 0.95 {
                    0x2e2e36
                } else if xf < -r * 0.45 {
                    0xa8a8b0
                } else {
                    0x5e5e66
                }
            } else if d < r + 0.6 {
                if xf > 0.0 {
                    0xc0c0c8
                } else {
                    0x56565e
                }
            } else {
                continue;
            };
            g.p(c, px as f64, py as f64, 1, 1);
        }
    }
}

/// The lander's top-left at `t`, and its thrust (0..1).
fn lander(g: &World, t: f64) -> (f64, f64, f64) {
    let (x0, y0) = ((g.panel(0) + 12) as f64, 6.0);
    let (x1, y1) = ((g.panel(2) + 8) as f64, 16.0);
    let land = GROUND - LANDER_H;
    if t < T_FINAL {
        // Steady across the first screens, easing to a stop over the site.
        let (u, a) = (t / T_FINAL, 0.6);
        let m = 2.0 / (1.0 + a);
        let ex = if u < a {
            m * u
        } else {
            m * a + m * (u - a) - m * (u - a).powi(2) / (2.0 * (1.0 - a))
        };
        (x0 + (x1 - x0) * ex, y0 + (y1 - y0) * u, 0.6)
    } else if t < T_DOWN {
        let u = (t - T_FINAL) / (T_DOWN - T_FINAL);
        let y = y1 + (land - y1) * (1.0 - (1.0 - u).powf(1.6));
        (x1, y, 1.0 - u * 0.4)
    } else {
        // A one-pixel settle on the legs after touchdown.
        let bounce = if (T_DOWN..T_DOWN + 0.25).contains(&t) {
            1.0
        } else {
            0.0
        };
        (x1, land + bounce, 0.0)
    }
}

fn draw_lander(g: &mut World, f: u32, t: f64) {
    let (x, y, thrust) = lander(g, t);
    let land = GROUND - LANDER_H;
    // Its shadow, cast to the left, sharpening as it nears the ground.
    let alt = (land - y).max(0.0);
    if alt < 30.0 {
        let sw = js_round(14.0 + alt * 0.4);
        let sx = x + 2.0 - alt * 0.6;
        for k in 0..sw {
            if alt < 8.0 || (k + f as i32) % 2 == 0 {
                tint(g, js_round(sx) + k, GROUND as i32, 0x202028, 0.7);
            }
        }
    }
    // Legs and footpads.
    let legs = [(3.0, -1.0), (13.0, 17.0)];
    for (hx, fx) in legs {
        line(g, x + hx, y + 9.0, x + fx, y + LANDER_H - 1.0, |_| 0x8a8a92);
        g.p(0xc8c8d0, x + fx - 1.0, y + LANDER_H - 1.0, 3, 1);
    }
    line(g, x + 8.0, y + 10.0, x + 8.0, y + LANDER_H - 1.0, |_| {
        0x6a6a72
    });
    // Ladder down the right leg.
    for k in 0..3 {
        g.p(0x5a5a62, x + 15.0, y + 10.0 + k as f64 * 2.0, 2, 1);
    }
    // Thruster flame and its glow.
    if thrust > 0.0 {
        let fl = 2.0 + thrust * 4.0 + if (f >> 1) % 2 == 1 { 1.0 } else { 0.0 };
        for k in 0..js_round(fl) {
            let u = k as f64 / fl;
            let c = mix(0xffffff, 0xff8a3a, u);
            let wd = if u < 0.5 { 3 } else { 1 };
            g.p(c, x + 9.0 - (wd / 2) as f64, y + 12.0 + k as f64, wd, 1);
        }
        glow(g, x + 9.0, y + 13.0, 6.0, 0xffb060, 0.4 * thrust);
        // Attitude puffs from the sides now and then.
        if f % 17 < 2 {
            g.p(0xe0e0e8, x + 1.0, y + 4.0, 1, 1);
            g.p(0x9a9aa8, x, y + 4.0, 1, 1);
        }
        if (f + 8) % 19 < 2 {
            g.p(0xe0e0e8, x + 15.0, y + 3.0, 1, 1);
            g.p(0x9a9aa8, x + 16.0, y + 3.0, 1, 1);
        }
    }
    g.spr(LANDER, LANDERP, x, y, false);
    // The hatch opens: a warm doorway.
    if t > T_HATCH {
        let open = ((t - T_HATCH) / 0.3).min(1.0);
        g.p(0x2a2a30, x + 11.0, y + 3.0, 3, 3);
        if open > 0.5 {
            g.p(0xffe0a0, x + 12.0, y + 4.0, 1, 2);
            glow(g, x + 13.0, y + 5.0, 3.0, 0xffe0a0, 0.3);
        }
    }
}

/// Dust thrown sideways by the exhaust near the ground, settling after.
fn dust(g: &mut World, t: f64) {
    let (x, y, _) = lander(g, t);
    let land = GROUND - LANDER_H;
    let alt = (land - y).max(0.0);
    let start = T_FINAL + 0.6;
    if t < start {
        return;
    }
    let cx = x + 9.0;
    let power = if t < T_DOWN {
        (1.0 - alt / 16.0).clamp(0.0, 1.0)
    } else {
        (1.0 - (t - T_DOWN) / 1.2).max(0.0)
    };
    for k in 0..60 {
        let kf = k as f64;
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let sp = 8.0 + rnd(kf * 1.7) * 18.0;
        let age = (t - start + rnd(kf * 3.1) * 0.8).rem_euclid(0.8);
        let dx = side * age * sp;
        let dy = -age * (2.0 + rnd(kf) * 4.0) + age * age * 4.0;
        let a = power * (1.0 - age / 0.8);
        if a <= 0.05 {
            continue;
        }
        let c = if k % 3 == 0 { 0xb8b0a0 } else { 0x9a948a };
        let (px, py) = (js_round(cx + dx), js_round(GROUND - 1.0 + dy));
        tint(g, px, py, c, a);
        tint(g, px + side as i32, py, c, a * 0.7);
        tint(g, px, py - 1, c, a * 0.5);
    }
}

/// Where the astronaut stands (feet at the ground) and which pose.
fn astronaut(g: &World, t: f64) -> Option<(f64, f64, usize)> {
    let (lx, ly, _) = lander(g, t);
    if t < T_LADDER {
        return None;
    }
    let base = lx + 18.0;
    let flag_x = (g.panel(2) + 41) as f64;
    if t < T_HOP {
        // Out of the hatch and down the ladder.
        let u = ((t - T_LADDER) / (T_HOP - T_LADDER)).min(1.0);
        let y = ly + 4.0 + (GROUND - 6.0 - ly - 4.0) * ease(u);
        Some((lx + 15.0 + u * 3.0, y, 0))
    } else if t < T_FLAG {
        // Three slow bounces.
        let u = (t - T_HOP) / (T_FLAG - T_HOP) * 3.0;
        let k = u.floor();
        let v = u - k;
        let x = base + (flag_x - 4.0 - base) * ((k + ease(v)) / 3.0);
        let y = GROUND - 6.0 - (v * PI).sin() * 6.0;
        Some((x, y, if v > 0.1 && v < 0.9 { 1 } else { 0 }))
    } else if t < T_WAVE {
        Some((flag_x - 4.0, GROUND - 6.0, 0))
    } else {
        Some((flag_x - 4.0, GROUND - 6.0, 2))
    }
}

fn flag(g: &mut World, f: u32, t: f64) {
    if t < T_FLAG {
        return;
    }
    let x = (g.panel(2) + 41) as f64;
    let u = ((t - T_FLAG) / 0.5).min(1.0);
    let h = js_round(u * 11.0);
    g.p(0xd0d0d8, x, GROUND - h as f64, 1, h);
    let un = ((t - T_FLAG - 0.4) / 0.4).clamp(0.0, 1.0);
    let fw = js_round(un * 7.0);
    if fw > 0 {
        let top = GROUND - 11.0;
        g.p(0xd0d0d8, x, top, fw + 1, 1);
        for i in 0..fw {
            let ripple = js_round((i as f64 * 0.9 - f as f64 * 0.5).sin() * 0.6) as f64;
            let c = if i < 3 { 0x2a8ad8 } else { 0xf0f0f0 };
            g.p(c, x + 1.0 + i as f64, top + 1.0 + ripple, 1, 4);
            g.p(0xf0c030, x + 1.0 + i as f64, top + 3.0 + ripple, 1, 1);
        }
    }
}

/// An old rover parked on the second screen; it turns its camera to the
/// landing and, at the end, snaps a photo.
fn rover(g: &mut World, f: u32, t: f64) {
    let (x, y) = ((g.panel(1) + 30) as f64, 40.0);
    g.p(0x2a2a32, x - 4.0, y + 9.0, 14, 1);
    g.spr(ROVER, ROVERP, x, y, false);
    // The camera head turns to the landing once it is down.
    let look = if t > T_DOWN { 1.0 } else { -1.0 };
    g.p(0x1a1a20, x + 9.5 + look * 0.5, y, 1, 1);
    // Its status light.
    if f % 24 < 3 {
        g.p(0x5aff7a, x + 2.0, y + 5.0, 1, 1);
    }
    let snap = t - 10.9;
    if (0.0..0.15).contains(&snap) || (0.3..0.45).contains(&snap) {
        glow(g, x + 10.0, y, 8.0, 0xffffff, 0.9);
        g.p(0xffffff, x + 10.0, y, 1, 1);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, f);
    orbiter(g, f, t);
    earth(g, t);
    mountains(g);
    ground(g);
    rover(g, f, t);
    flag(g, f, t);
    draw_lander(g, f, t);
    dust(g, t);
    // Bootprints where each bounce came down.
    if t > T_HOP {
        let (lx, _, _) = lander(g, t);
        let base = lx + 18.0;
        let flag_x = (g.panel(2) + 41) as f64;
        let done = (((t - T_HOP) / (T_FLAG - T_HOP)) * 3.0).floor().min(3.0) as i32;
        for k in 0..=done {
            let x = base + (flag_x - 4.0 - base) * (k as f64 / 3.0);
            g.p(0x3e3e46, x, GROUND, 1, 1);
            g.p(0x3e3e46, x + 2.0, GROUND, 1, 1);
        }
    }
    if let Some((x, y, pose)) = astronaut(g, t) {
        let sprite = if pose == 2 && (f / 3) % 2 == 1 {
            ASTRO[0]
        } else {
            ASTRO[pose]
        };
        if pose == 2 && (f / 3) % 2 == 1 {
            // Arm half down on alternate beats of the wave.
            g.p(0xf0f0f4, x + 4.0, y + 1.0, 1, 1);
        }
        g.spr(sprite, ASTROP, x, y, false);
        // Bootprint puffs where each bounce lands.
        if (T_HOP..T_FLAG + 0.2).contains(&t) && pose == 0 {
            tint(g, js_round(x) - 1, GROUND as i32 - 1, 0xb8b0a0, 0.6);
            tint(g, js_round(x) + 4, GROUND as i32 - 1, 0xb8b0a0, 0.6);
        }
    }
}

//! Deep sea: sunbeams slant into a bright reef of kelp, coral, sponges and
//! a scuttling crab. A school of silver fish swims in and splits around a
//! sea turtle paddling the other way, then follows the reef wall down into
//! the dark, past glowing jellyfish, to a little light bobbing in the deep.
//! They gather round it, curious, until a huge anglerfish opens its jaws
//! behind the lure; the school bursts apart and the jaws snap on nothing.
//! While the angler sulks, an octopus reaches out of its den, plucks the
//! lure off its stalk and pulls it home as a reading lamp.

use super::fx::{blend, glow, mix, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// The school reaches the lure, the jaws open, snap, then the octopus
/// reaches, grabs and has pulled the lure home.
const T_ARRIVE: f64 = 6.4;
const T_OPEN: f64 = 7.3;
const T_SNAP: f64 = 7.9;
const T_REACH: f64 = 8.9;
const T_GRAB: f64 = 9.7;
const T_HOME: f64 = 10.6;

const N_FISH: usize = 20;

/// A small silver fish facing right, two tail beats.
const FISH: [&[&str]; 2] = [
    &["y.bbb.", "ybsske", "y.www."],
    &[".ybbb.", "yysske", ".ywww."],
];

const LURE: u32 = 0x8affe0;

/// How far into the deep world x is: 0 on the reef, 1 at the bottom.
fn depth(g: &World, x: f64) -> f64 {
    let a = cliff(g) - 20.0;
    let b = (g.panel(3) - 10) as f64;
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// Where the reef drops off into the deep.
fn cliff(g: &World) -> f64 {
    (g.panel(3) - 92) as f64
}

/// Height of the sea floor at x (60 or more where it is out of sight).
fn floor_y(g: &World, x: f64) -> f64 {
    let c = cliff(g);
    let deep0 = (g.panel(3) - 26) as f64;
    let ripple = 1.6 * (x * 0.08).sin() + (x * 0.23).sin() * 0.8;
    if x < c {
        49.0 + ripple
    } else if x < c + 12.0 {
        49.0 + ripple + ((x - c) / 12.0).powi(2) * 14.0
    } else if x < deep0 {
        64.0
    } else {
        let u = ((x - deep0) / 20.0).min(1.0);
        64.0 - ease(u) * 14.0 + ripple * 0.6
    }
}

fn oval(g: &mut World, c: u32, cx: f64, cy: f64, rx: f64, ry: f64) {
    let ri = ry.ceil() as i32;
    for dy in -ri..=ri {
        let v = dy as f64 / ry;
        if v.abs() > 1.0 {
            continue;
        }
        let hw = rx * (1.0 - v * v).sqrt();
        g.p(c, cx - hw, cy + dy as f64, js_round(hw * 2.0).max(1), 1);
    }
}

fn water(g: &mut World, t: f64) {
    let w = g.w();
    let shallow = [0x48b8d0, 0x2a90c0, 0x1e70a6, 0x16548a, 0x103e6e];
    let deep = [0x0c2440, 0x0a1a34, 0x08142a, 0x060f22, 0x050b1a];
    for x in 0..w {
        let d = depth(g, x as f64);
        let stops: Vec<u32> = shallow
            .iter()
            .zip(deep.iter())
            .map(|(&a, &b)| mix(a, b, d))
            .collect();
        vgrad(g, &stops, x, x + 1, 0, 60);
    }
    // The bright, rippling underside of the surface.
    for x in 0..w {
        let xf = x as f64;
        let k = 1.0 - depth(g, xf);
        if k <= 0.0 {
            continue;
        }
        let y = js_round(1.0 + (xf * 0.3 + t * 2.4).sin() * 0.8 + (xf * 0.11 - t).sin() * 0.6);
        blend(g, 0xd8f8ff, x, y, 0.7 * k);
        blend(g, 0xa8e8f8, x, y + 1, 0.35 * k);
    }
    // Sunbeams slanting down, swaying a little.
    for k in 0..g.spread(8) {
        let kf = k as f64;
        let bx = rnd(kf + 1.0) * (cliff(g) + 40.0) - 20.0 + (t * 0.5 + kf).sin() * 3.0;
        let wid = 2.0 + rnd(kf + 2.0) * 3.0;
        let pulse = 0.6 + 0.4 * (t * 0.9 + kf * 1.7).sin();
        for y in 0..56 {
            let yf = y as f64;
            let x0 = bx + yf * 0.42;
            let fade = (1.0 - yf / 56.0) * pulse * (1.0 - depth(g, x0));
            for i in 0..js_round(wid) {
                blend(g, 0xc8f4ff, js_round(x0) + i, y, 0.16 * fade);
            }
        }
    }
    // Marine snow drifting down in the dark.
    for k in 0..g.spread(50) {
        let kf = k as f64;
        let x = rnd(kf + 10.0) * w as f64 + (t * 0.6 + kf).sin() * 1.5;
        let y = (rnd(kf + 11.0) * 60.0 + t * (1.5 + rnd(kf + 12.0) * 1.5)) % 60.0;
        let d = depth(g, x);
        if d > 0.2 {
            blend(g, 0x9ab0d0, js_round(x), js_round(y), 0.45 * d);
        }
    }
}

/// Distant reef rocks, hazy in the blue behind everything.
fn far_reef(g: &mut World) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let d = depth(g, xf);
        if d > 0.9 {
            continue;
        }
        let h = 6.0 + 3.0 * (xf * 0.06).sin() + 2.0 * (xf * 0.17 + 1.0).sin().max(0.0) * 2.0;
        let top = js_round(48.0 - h);
        let c = mix(0x1e6e9e, 0x0c2440, d);
        g.p(c, xf, top as f64, 1, 50 - top);
        put(g, mix(0x2e84b0, 0x0c2440, d), x, top);
    }
}

fn kelp(g: &mut World, x: f64, top: f64, t: f64, seed: f64) {
    let base = floor_y(g, x);
    let n = (base - top) as i32;
    for s in 0..n {
        let sf = s as f64;
        let h = sf / n as f64;
        let sway = (t * 1.1 + seed - h * 2.2).sin() * 3.5 * h * h;
        let y = base - sf;
        let c = if s % 6 < 3 { 0x4a8a3a } else { 0x3e7a32 };
        g.p(c, x + sway, y, 2, 1);
        put(g, 0x2a5a26, js_round(x + sway) + 1, y as i32);
        // Blades alternate sides, with a gas bladder at the root.
        if s % 5 == 2 && s > 3 {
            let side = if (s / 5) % 2 == 0 { 1.0 } else { -1.0 };
            let lw = (t * 1.6 + seed + sf).sin() * 0.8;
            for j in 1..5 {
                let jf = j as f64;
                let lx = x + sway + if side > 0.0 { 1.0 + jf } else { -jf };
                g.p(0x6aa848, lx, y - jf * 0.5 + lw * jf * 0.3, 1, 2);
            }
            g.p(
                0x8ab84a,
                x + sway + if side > 0.0 { 2.0 } else { -1.0 },
                y,
                1,
                1,
            );
        }
    }
}

fn reef(g: &mut World, t: f64, f: u32) {
    let w = g.w();
    let c = cliff(g);
    // Floor: sand with ripples and dancing caustics on the reef, dark silt
    // in the deep.
    for x in 0..w {
        let xf = x as f64;
        let top = js_round(floor_y(g, xf));
        if top >= 60 {
            continue;
        }
        let d = depth(g, xf);
        let sand = mix(0xd8c08a, 0x2a2a3a, d);
        let dark = mix(0xb89a6a, 0x1c1c2c, d);
        g.p(sand, xf, top as f64, 1, 60 - top);
        put(g, mix(0xf0e0b0, 0x3a3a4e, d), x, top);
        for y in top + 1..60 {
            if (x * 3 + y * 7) % 11 == 0 {
                put(g, dark, x, y);
            }
            if d < 0.5 {
                let cz = ((xf * 0.35 + t * 1.3).sin()
                    + (y as f64 * 0.9 - t * 0.9 + xf * 0.12).sin())
                .abs();
                if cz < 0.18 {
                    blend(g, 0xfff8e0, x, y, 0.5 * (1.0 - d * 2.0));
                }
            }
        }
    }
    // Seagrass tufts and pebbles along the sand.
    for k in 0..g.spread(30) {
        let kf = k as f64;
        let x = rnd(kf + 200.0) * (c - 4.0);
        let y = floor_y(g, x);
        if rnd(kf + 201.0) < 0.3 {
            g.p(0x9a8a7a, x, y + 2.0, 2, 1);
            put(g, 0xe8dcc0, js_round(x), js_round(y) + 2);
            continue;
        }
        for b in 0..3 {
            let bf = b as f64;
            let h = 2 + ((rnd(kf + 202.0 + bf) * 4.0) as i32);
            for j in 0..h {
                let jf = j as f64;
                let sway = (t * 1.5 + kf + bf).sin() * jf * 0.3;
                put(
                    g,
                    if j == h - 1 { 0x8ac85a } else { 0x4a9a4a },
                    js_round(x + bf - 1.0 + sway),
                    js_round(y - jf),
                );
            }
        }
    }
    // The reef wall falling away into the dark.
    let cx = js_round(c);
    for x in cx - 6..cx + 14 {
        let xf = x as f64;
        let top = js_round(floor_y(g, xf)).min(60);
        let d = depth(g, xf);
        for y in top.max(44)..60 {
            let k = ((y - 44) as f64 / 16.0).min(1.0);
            put(g, mix(mix(0x6a6a7a, 0x2a2a3a, d), 0x10101c, k * 0.6), x, y);
        }
    }
    // Rocks.
    for (i, (dx, r)) in [(c - 34.0, 5), (c - 8.0, 6), (c - 120.0, 4)]
        .into_iter()
        .enumerate()
    {
        let y = floor_y(g, dx) + 1.0;
        g.disc(0x4a5a72, dx, y, r);
        g.disc(0x6a7a92, dx - 1.0, y - 1.0, r - 2);
        g.p(0x2a3448, dx - r as f64, y + 1.0, r * 2 + 1, 1);
        if i == 0 {
            // A sea star resting on top.
            let (sx, sy) = (dx, y - r as f64);
            g.p(0xf07a3a, sx - 2.0, sy, 5, 1);
            g.p(0xf07a3a, sx, sy - 2.0, 1, 5);
            g.p(0xf8a060, sx, sy, 1, 1);
        }
    }
    let p0 = g.panel(0) as f64;
    // Kelp forest on the first screen.
    for (k, (dx, top)) in [(4.0, 6.0), (13.0, 14.0), (49.0, 4.0), (56.0, 18.0)]
        .into_iter()
        .enumerate()
    {
        kelp(g, p0 + dx, top, t, k as f64 * 1.7);
    }
    // Brain coral.
    let (bx, by) = (p0 + 26.0, floor_y(g, p0 + 26.0) - 1.0);
    g.disc(0xc8607a, bx, by, 5);
    g.disc(0xe08a9a, bx - 1.0, by - 1.0, 3);
    for k in 0..5 {
        let kf = k as f64;
        g.p(
            0x9a4060,
            bx - 4.0 + kf * 2.0,
            by - 2.0 + (kf * 1.7).sin() * 2.0,
            1,
            2,
        );
    }
    g.p(0x2a3448, bx - 5.0, by + 3.0, 11, 1);
    // Branching staghorn coral, orange.
    let sx = p0 + 38.0;
    let sy = floor_y(g, sx);
    for (a, l) in [(-1.9f64, 9.0), (-1.4, 11.0), (-1.0, 8.0), (-2.4, 6.0)] {
        for i in 0..l as i32 {
            let fi = i as f64;
            let x = sx + a.cos() * fi;
            let y = sy + a.sin() * fi;
            g.p(0xf08a4a, x, y, 2, 1);
            if i == l as i32 - 1 {
                g.p(0xffc080, x, y, 2, 1);
            }
        }
    }
    second_screen(g, t);
    // The crab sidles to and fro on the first screen.
    let crx = p0 + 32.0 + (t * 0.8).sin() * 9.0;
    let cry = floor_y(g, crx) - 3.0;
    let step = (f / 2).is_multiple_of(2);
    g.p(0xd84a2a, crx - 2.0, cry, 5, 2);
    g.p(0xf07a4a, crx - 1.0, cry, 3, 1);
    put(g, 0x101010, js_round(crx) - 1, js_round(cry) - 1);
    put(g, 0x101010, js_round(crx) + 1, js_round(cry) - 1);
    g.p(0xd84a2a, crx - 4.0, cry - 1.0, 2, 1);
    g.p(0xd84a2a, crx + 3.0, cry - 1.0, 2, 1);
    for lx in [-2.0, 0.0, 2.0] {
        g.p(
            0xa83a20,
            crx + lx + if step { 0.0 } else { 1.0 } - 0.5,
            cry + 2.0,
            1,
            1,
        );
    }
}

/// Sea fan, tube sponges, an anemone and a giant clam on the second
/// screen (whatever of it is still reef in a narrow layout).
fn second_screen(g: &mut World, t: f64) {
    let p1 = g.panel(1) as f64;
    let lim = cliff(g) - 6.0;
    // Purple sea fan and tube sponges.
    let fx = p1 + 10.0;
    if fx < lim {
        let fy = floor_y(g, fx);
        for k in 0..9 {
            let a = -PI + 0.35 + k as f64 * 0.3;
            let sway = (t * 1.2 + k as f64 * 0.2).sin() * 0.06;
            for i in 2..12 {
                if (i + k) % 2 == 0 || i > 9 {
                    let fi = i as f64;
                    put(
                        g,
                        if i > 9 { 0xd07ae0 } else { 0xa04ab0 },
                        js_round(fx + (a + sway).cos() * fi),
                        js_round(fy + (a + sway).sin() * fi),
                    );
                }
            }
        }
        g.p(0x6a2a7a, fx, fy - 2.0, 1, 3);
    }
    let tx = p1 + 50.0;
    for (i, h) in [8, 12, 6].into_iter().enumerate() {
        let x = tx + i as f64 * 3.0;
        if x > lim {
            continue;
        }
        let y = floor_y(g, x) - h as f64;
        g.p(0x8a4ab0, x, y, 3, h);
        g.p(0xb070d0, x, y, 1, h);
        g.p(0x3a1a50, x + 1.0, y, 1, 1);
    }
    // An anemone waving its tentacles.
    let (ax, ay) = (p1 + 30.0, floor_y(g, p1 + 30.0) - 1.0);
    if ax < lim {
        g.p(0xc84a5a, ax - 2.0, ay - 2.0, 5, 3);
        for k in 0..7 {
            let kf = k as f64;
            let base = ax - 3.0 + kf;
            for j in 1..5 {
                let jf = j as f64;
                let wv = (t * 2.2 + kf * 0.7 - jf * 0.6).sin() * jf * 0.35;
                put(
                    g,
                    if j == 4 { 0xffb0c0 } else { 0xf07a8a },
                    js_round(base + wv + (kf - 3.0) * jf * 0.2),
                    js_round(ay - 2.0 - jf),
                );
            }
        }
    }
    // A giant clam that yawns out a burst of bubbles now and then.
    let (qx, qy) = (p1 + 42.0, floor_y(g, p1 + 42.0) - 2.0);
    if qx > lim {
        return;
    }
    let cyc = (t / 3.0).fract();
    let open = if cyc < 0.3 {
        (cyc / 0.3 * PI).sin()
    } else {
        0.0
    };
    g.p(0x6a5a8a, qx - 4.0, qy, 9, 2);
    g.p(0x9a8ab8, qx - 4.0, qy, 9, 1);
    let lid = js_round(open * 3.0);
    g.p(0x2a1a3a, qx - 3.0, qy - lid as f64, 7, lid);
    g.p(0x7a6aa0, qx - 4.0, qy - 1.0 - lid as f64, 9, 1);
    g.p(0xa89ac8, qx - 3.0, qy - 2.0 - lid as f64, 7, 1);
    if lid > 0 {
        g.p(0xf8f0ff, qx, qy - lid as f64 + 1.0, 1, 1);
    }
}

fn bubbles(g: &mut World, t: f64) {
    let p1 = g.panel(1) as f64;
    // From the clam: a burst each time it yawns.
    let (qx, qy) = (p1 + 42.0, floor_y(g, p1 + 42.0) - 4.0);
    for k in 0..if qx < cliff(g) - 6.0 { 8 } else { 0 } {
        let kf = k as f64;
        let a = (t - (t / 3.0).floor() * 3.0) - kf * 0.08;
        let a = if a < 0.0 { a + 3.0 } else { a };
        let y = qy - a * 16.0;
        if y < 1.0 {
            continue;
        }
        let x = qx + (a * 4.0 + kf).sin() * 1.5 + rnd(kf) * 2.0 - 1.0;
        bubble(g, x, y, if k % 3 == 0 { 2 } else { 1 });
    }
    // Strings of bubbles from the reef.
    for k in 0..g.spread(10) {
        let kf = k as f64;
        let bx = rnd(kf + 30.0) * (cliff(g) - 10.0);
        let cyc = (t * 0.25 + rnd(kf + 31.0)) % 1.0;
        let y = floor_y(g, bx) - 2.0 - cyc * 50.0;
        let x = bx + (cyc * 14.0 + kf).sin() * 1.5;
        bubble(g, x, y, if cyc > 0.6 { 2 } else { 1 });
    }
}

fn bubble(g: &mut World, x: f64, y: f64, r: i32) {
    if r <= 1 {
        blend(g, 0xd8f8ff, js_round(x), js_round(y), 0.8);
        return;
    }
    g.ring(0xb8ecff, x, y, 1.5);
    put(g, 0xffffff, js_round(x) - 1, js_round(y) - 1);
}

fn jellyfish(g: &mut World, t: f64) {
    let c = cliff(g);
    for (k, (dx, y0, col)) in [(12.0, 34.0, 0xff8ad8), (34.0, 22.0, 0x8ad8ff)]
        .into_iter()
        .enumerate()
    {
        let kf = k as f64;
        let ph = t * 1.4 + kf * 2.0;
        let pump = ph.sin().max(0.0);
        let x = c + dx + (t * 0.3 + kf).sin() * 2.0;
        let y = y0 - t * 0.8 + (ph - 1.0).cos() * 1.2;
        glow(g, col, x, y, 9.0, 0.35);
        // The bell squashes as it pumps.
        let (rx, ry) = (4.0 + pump, 3.0 - pump * 0.8);
        oval(g, mix(col, 0x2a1a4a, 0.55), x, y, rx, ry);
        oval(
            g,
            mix(col, 0xffffff, 0.3),
            x - 1.0,
            y - 1.0,
            rx - 2.0,
            (ry - 1.5).max(0.6),
        );
        g.p(col, x - rx, y + 1.0, js_round(rx * 2.0), 1);
        for j in 0..4 {
            let jf = j as f64;
            for s in 1..8 {
                let sf = s as f64;
                let wx = x - 3.0 + jf * 2.0 + (t * 3.0 + sf * 0.8 + jf).sin() * sf * 0.2;
                blend(
                    g,
                    col,
                    js_round(wx),
                    js_round(y + 1.0 + sf),
                    0.8 * (1.0 - sf / 9.0),
                );
            }
        }
    }
}

/// A manta ray gliding through the blue in the distance.
fn manta(g: &mut World, t: f64) {
    let x = cliff(g) - 30.0 + t * 7.0;
    let y = 12.0 + (t * 0.5).sin() * 2.0;
    let flap = (t * 2.2).sin();
    let c = mix(0x16548a, 0x0a1a34, depth(g, x) * 0.6 + 0.2);
    for i in -9..=9 {
        let fi = i as f64;
        let a = fi.abs() / 9.0;
        let wy = y + flap * a * a * 3.0;
        let h = js_round(3.0 * (1.0 - a)) + 1;
        g.p(c, x + fi, wy - (h / 2) as f64, 1, h);
    }
    g.p(c, x - 1.0, y + 1.0, 3, 1);
    for i in 0..7 {
        put(
            g,
            c,
            js_round(x - 1.0 - i as f64 * 0.2),
            js_round(y + 2.0 + i as f64),
        );
    }
}

fn turtle(g: &mut World, x: f64, y: f64, t: f64) {
    // Flippers behind and in front of the shell, sweeping slowly.
    let flap = (t * 3.6).sin();
    let flipper = |g: &mut World, sx: f64, sy: f64, a: f64, len: i32, c: u32| {
        for i in 0..len {
            let fi = i as f64;
            let th = if i < len / 2 { 2 } else { 1 };
            g.p(c, sx + a.cos() * fi, sy + a.sin() * fi, th, th);
        }
    };
    flipper(g, x + 6.0, y + 2.0, 0.9 + flap * 0.5, 6, 0x4a6a3a);
    flipper(g, x - 3.0, y + 1.0, 2.2 + flap * 0.7, 9, 0x5a7a44);
    // Shell: a dome of scutes with a pale rim.
    for r in 0..8 {
        let v = (7 - r) as f64 / 7.0;
        let hw = 9.0 * (1.0 - v * v * 0.9).sqrt();
        let c = if r == 0 { 0x2a4a2a } else { 0x5a7a3a };
        g.p(c, x - hw, y - 7.0 + r as f64, js_round(hw * 2.0), 1);
    }
    for (dx, dy) in [
        (-4.0, -4.0),
        (0.0, -5.0),
        (4.0, -4.0),
        (-2.0, -2.0),
        (2.0, -2.0),
    ] {
        g.p(0x7a9a4a, x + dx - 1.0, y + dy, 3, 1);
        g.p(0x3a5a2a, x + dx - 1.0, y + dy + 1.0, 3, 1);
    }
    g.p(0x9ab85a, x - 5.0, y - 6.0, 4, 1);
    g.p(0xd8c890, x - 9.0, y + 1.0, 18, 1);
    // Head, with a beak and a calm eye.
    g.disc(0x7a9a5a, x - 11.0, y - 1.0, 2);
    g.p(0x9ab86a, x - 13.0, y - 2.0, 2, 1);
    put(g, 0x101810, js_round(x - 12.0), js_round(y - 2.0));
    g.p(0x5a7a44, x - 10.0, y + 1.0, 2, 1);
    // Front flipper over the shell edge.
    flipper(g, x - 6.0, y + 1.0, 2.6 - flap * 0.6, 8, 0x6a8a4a);
}

/// The anglerfish's lure: where the bulb hangs at `t`.
fn bulb(g: &World, t: f64) -> (f64, f64) {
    let (ax, ay) = angler_at(g);
    (
        ax - 24.0 + (t * 1.3).sin() * 1.5,
        ay - 10.0 + (t * 2.1).sin() * 1.2,
    )
}

fn angler_at(g: &World) -> (f64, f64) {
    ((g.panel(3) + 46) as f64, 33.0)
}

/// Octopus den opening.
fn den(g: &World) -> (f64, f64) {
    let x = (g.panel(3) + 21) as f64;
    (x, floor_y(g, x) - 2.0)
}

/// The tentacle tip and whether it holds the bulb.
fn tentacle(g: &World, t: f64) -> Option<(f64, f64)> {
    let (ox, oy) = den(g);
    let (bx, by) = bulb(g, T_GRAB);
    if t < T_REACH || t > T_HOME {
        return None;
    }
    if t < T_GRAB {
        let u = ease((t - T_REACH) / (T_GRAB - T_REACH));
        let (tx, ty) = bulb(g, t);
        Some((ox + (tx - ox) * u, oy - 2.0 + (ty - oy + 2.0) * u))
    } else {
        let u = ease((t - T_GRAB - 0.2) / (T_HOME - T_GRAB - 0.2));
        Some((bx + (ox - bx) * u, by + (oy - by) * u))
    }
}

/// Where the lure light is (it changes hands), if anywhere visible.
fn light(g: &World, t: f64) -> (f64, f64) {
    if t < T_GRAB {
        bulb(g, t)
    } else if t < T_HOME {
        tentacle(g, t).unwrap_or(den(g))
    } else {
        den(g)
    }
}

fn angler(g: &mut World, t: f64, f: u32) {
    let (ax, ay) = angler_at(g);
    // Mostly hidden in the dark; lit up by the flare as the jaws open.
    let reveal = if t < T_OPEN - 0.3 {
        0.07
    } else if t < T_SNAP + 0.6 {
        ease((t - T_OPEN + 0.3) / 0.3)
    } else if t < T_HOME {
        0.55
    } else {
        0.08
    };
    let open = if t < T_OPEN {
        0.0
    } else if t < T_SNAP {
        ease((t - T_OPEN) / 0.25)
    } else {
        (1.0 - (t - T_SNAP) / 0.12).max(0.0)
    };
    let bg = 0x060f22;
    let body = mix(bg, 0x3a3456, reveal);
    let rim = mix(bg, 0x6a6a90, reveal);
    // Tail and fins first.
    for i in 0..8 {
        let fi = i as f64;
        let sp = (t * 2.0).sin() * fi * 0.15;
        g.p(
            body,
            ax + 12.0 + fi,
            ay - 3.0 - fi * 0.6 + sp,
            1,
            js_round(6.0 + fi * 1.2),
        );
    }
    for i in 0..5 {
        let fi = i as f64;
        g.p(
            rim,
            ax - 2.0 + fi * 3.0,
            ay - 12.0 + fi * 0.6 - (fi % 2.0),
            1,
            3,
        );
    }
    oval(g, body, ax, ay, 14.0, 11.0);
    // Mottling and the lure-lit rim along the top.
    for k in 0..18 {
        let kf = k as f64;
        let (mx, my) = (
            ax - 10.0 + rnd(kf + 70.0) * 20.0,
            ay - 8.0 + rnd(kf + 71.0) * 15.0,
        );
        put(g, mix(bg, 0x4a4468, reveal), js_round(mx), js_round(my));
    }
    for i in 0..20 {
        let a = PI + 0.4 + i as f64 * 0.11;
        put(
            g,
            rim,
            js_round(ax + a.cos() * 13.0),
            js_round(ay + a.sin() * 10.5),
        );
    }
    // The mouth: a wide underbite full of needle teeth.
    let (mx0, my) = (ax - 15.0, ay + 1.0);
    let drop = open * 8.0;
    let teeth = mix(bg, 0xe8f0e0, reveal);
    if open > 0.0 {
        for i in 0..13 {
            let fi = i as f64;
            let h = js_round(drop * (1.0 - fi / 16.0));
            g.p(0x2a0818, mx0 + fi, my, 1, h.max(1));
        }
    }
    for i in 0..13 {
        let fi = i as f64;
        g.p(rim, mx0 + fi, my - 1.0, 1, 1);
        let jaw = my + drop * (1.0 - fi / 16.0) + 1.0;
        g.p(body, mx0 - 1.0 + fi, jaw, 1, 3);
        if i % 2 == 0 {
            g.p(teeth, mx0 + fi, my, 1, 2);
            g.p(teeth, mx0 + fi + 1.0, jaw - 2.0, 1, 2);
        }
    }
    // The eye: a pale glint even in the dark; grumpy after the miss.
    let (ex, ey) = (ax - 7.0, ay - 5.0);
    let blink = (f % 40) < 2 || (t > T_HOME + 0.5 && (t - T_HOME - 0.5) % 0.6 < 0.1);
    if !blink {
        if reveal > 0.3 {
            g.p(mix(bg, 0xc8e8f0, reveal), ex - 1.0, ey - 1.0, 3, 3);
            g.p(0x101018, ex, ey, 1, 1);
            put(g, 0xffffff, js_round(ex) - 1, js_round(ey) - 1);
        } else {
            // Just a pale glint in the dark.
            put(g, 0x9ab8c8, js_round(ex), js_round(ey));
        }
    }
    // Puzzled in the dark once the lure is gone.
    if t > T_HOME + 0.3 {
        let q = 0x8a9ab8;
        let (qx, qy) = (ax - 9.0, ay - 22.0 + ((t * 3.0).sin() * 0.6));
        g.p(q, qx, qy, 3, 1);
        g.p(q, qx + 3.0, qy + 1.0, 1, 2);
        g.p(q, qx + 1.0, qy + 3.0, 2, 1);
        g.p(q, qx + 1.0, qy + 4.0, 1, 1);
        g.p(q, qx + 1.0, qy + 6.0, 1, 1);
    }
    if t > T_SNAP + 0.2 {
        g.p(mix(bg, 0x8a86b0, reveal.max(0.5)), ex - 2.0, ey - 3.0, 4, 1);
        put(
            g,
            mix(bg, 0x8a86b0, reveal.max(0.5)),
            js_round(ex) + 2,
            js_round(ey) - 2,
        );
    }
    // The stalk arching from the brow to the bulb, or to a bare tip.
    let (bx, by) = bulb(g, t.min(T_GRAB));
    let (sx, sy) = (ax - 6.0, ay - 10.0);
    let (cx, cy) = (ax - 14.0, ay - 22.0);
    let stalk = mix(bg, 0x7a7aa0, reveal.max(0.3));
    for i in 0..=24 {
        let u = i as f64 / 24.0;
        let x = (1.0 - u).powi(2) * sx + 2.0 * (1.0 - u) * u * cx + u * u * bx;
        let y = (1.0 - u).powi(2) * sy + 2.0 * (1.0 - u) * u * cy + u * u * by;
        // After the theft the stalk droops.
        let droop = if t > T_GRAB {
            u * u * 4.0 * ((t - T_GRAB) / 0.5).min(1.0)
        } else {
            0.0
        };
        put(g, stalk, js_round(x), js_round(y + droop));
    }
}

fn octopus(g: &mut World, t: f64, f: u32) {
    let (ox, oy) = den(g);
    let (lx, ly) = light(g, t);
    let lit = |x: f64, y: f64| -> f64 {
        let d = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
        (1.0 - d / 26.0).clamp(0.0, 1.0)
    };
    let home = t > T_HOME;
    // The den: a heap of rocks with a dark doorway.
    let k = lit(ox, oy - 4.0).max(if home { 0.8 } else { 0.0 });
    g.disc(mix(0x10141e, 0x4a5068, k), ox, oy + 2.0, 9);
    g.disc(mix(0x141a28, 0x5a6280, k), ox - 2.0, oy, 6);
    g.disc(mix(0x10141e, 0x4a5068, k), ox + 6.0, oy - 2.0, 4);
    oval(g, 0x04060c, ox, oy + 2.0, 4.0, 3.0);
    if home {
        // The stolen lure as a cosy lamp by the doorway, the octopus
        // leaning out beside it, beaming.
        let tw = 0.9 + 0.1 * (t * 6.0).sin();
        let rise = ease((t - T_HOME) / 0.5);
        glow(g, LURE, ox + 4.0, oy, 16.0, 0.5 * tw);
        let hy = oy + 2.0 - rise * 3.0;
        oval(g, 0xc8502e, ox - 1.0, hy, 4.0, 3.5);
        oval(g, 0xf08a5a, ox - 2.0, hy - 1.0, 2.0, 1.5);
        g.p(0xf8b080, ox - 3.0, hy - 3.0, 2, 1);
        // Happy eyes: little arches.
        for ex in [-3.0, 0.0] {
            g.p(0x101010, ox + ex, hy + 1.0, 2, 1);
            put(g, 0x101010, js_round(ox + ex) - 1, js_round(hy) + 2);
            put(g, 0x101010, js_round(ox + ex) + 2, js_round(hy) + 2);
        }
        put(g, 0xff8aa0, js_round(ox) - 4, js_round(hy) + 3);
        put(g, 0xff8aa0, js_round(ox) + 2, js_round(hy) + 3);
        // An arm curled round the lamp.
        let (lx, ly) = (ox + 5.0, oy - 1.0 - rise * 2.0);
        g.p(0xc8502e, ox + 2.0, hy + 3.0, 3, 1);
        g.p(0xc8502e, lx - 1.0, ly + 1.0, 3, 1);
        g.p(0xc8502e, lx + 1.0, ly - 1.0, 1, 2);
        g.p(0xf0fff8, lx, ly - 1.0, 2, 2);
        put(g, 0xffffff, js_round(lx), js_round(ly) - 1);
        glow(g, 0xe8fff8, lx, ly, 4.0, 0.8);
    } else if t > T_SNAP + 0.5 || (t > 1.0 && (f / 30) % 3 == 1) {
        // Two eyes peeking out of the doorway.
        let e = mix(0x5a5060, 0xf0e8d0, k.max(0.3));
        g.p(e, ox - 2.0, oy + 1.0, 1, 1);
        g.p(e, ox + 1.0, oy + 1.0, 1, 1);
    }
    // The tentacle: curls out to the bulb and drags it home.
    if let Some((tx, ty)) = tentacle(g, t) {
        let n = 22;
        for i in 0..=n {
            let u = i as f64 / n as f64;
            let wig = (u * PI * 2.0 + t * 6.0).sin() * 3.0 * u * (1.0 - u) * 4.0 * 0.5;
            let x = ox + (tx - ox) * u + wig;
            let y = oy - 1.0 + (ty - oy + 1.0) * u - (u * PI).sin() * 6.0;
            let c = mix(0x5a2418, 0xe8683a, lit(x, y).max(0.4));
            let th = if u < 0.5 { 2 } else { 1 };
            g.p(c, x, y, th, th);
            if i % 2 == 1 && u < 0.85 {
                put(
                    g,
                    mix(0x8a4a3a, 0xf8c8a8, lit(x, y)),
                    js_round(x) - 1,
                    js_round(y),
                );
            }
        }
        // Curled around the bulb.
        if t >= T_GRAB {
            g.p(0xd8603a, tx - 1.0, ty + 1.0, 3, 1);
            g.p(0xd8603a, tx + 1.0, ty - 1.0, 1, 2);
        }
    }
}

fn lure_light(g: &mut World, t: f64) {
    if t > T_HOME {
        return;
    }
    let (x, y) = light(g, t);
    let flare = if (T_OPEN - 0.3..T_SNAP).contains(&t) {
        1.4
    } else {
        1.0
    };
    let pulse = 0.85 + 0.15 * (t * 4.0).sin();
    glow(g, LURE, x, y, 13.0 * flare, 0.5 * pulse);
    glow(g, 0xe8fff8, x, y, 4.0, 0.8);
    g.p(0xf0fff8, x, y, 2, 2);
    put(g, 0xffffff, js_round(x), js_round(y));
}

/// The school: each fish's position and whether it faces left.
fn school(g: &World, t: f64, i: usize) -> (f64, f64, bool) {
    let fi = i as f64;
    let ox = (rnd(fi + 100.0) - 0.5) * 26.0;
    let oy = (rnd(fi + 101.0) - 0.5) * 13.0;
    let wig = ((t * 2.3 + fi).sin() * 1.2, (t * 1.9 + fi * 1.3).cos() * 0.8);
    let s0 = -22.0;
    let s1 = stage_at(g, 3, 6.0);
    let path = |tt: f64| -> (f64, f64) {
        let p = (tt / T_ARRIVE).clamp(0.0, 1.0);
        let s = s0 + (s1 - s0) * (1.0 - (1.0 - p).powf(1.6));
        // Down along the reef wall into the deep.
        let cy = 22.0 + 6.0 * (tt * 0.9).sin() + 4.0 * p;
        (s, cy)
    };
    let (s, cy) = path(t);
    let mut x = stage_x(g, s + ox) + wig.0;
    let mut y = cy + oy + wig.1;
    // Split around the turtle.
    let (tx, ty) = turtle_at(g, t);
    let dx = x - tx;
    if dx.abs() < 20.0 {
        let side = if oy + (rnd(fi + 102.0) - 0.5) * 2.0 >= ty - cy {
            1.0
        } else {
            -1.0
        };
        let push = (1.0 - dx.abs() / 20.0).powf(0.7) * 11.0;
        y += side * push;
    }
    // Gather round the lure, circling it.
    let (bx, by) = bulb(g, t);
    let gather = ease((t - (T_ARRIVE - 1.0)) / 1.4);
    let r = 5.0 + rnd(fi + 103.0) * 9.0;
    let a = rnd(fi + 104.0) * PI * 2.0 + t * (0.6 + rnd(fi + 105.0) * 0.5);
    let (gx, gy) = (bx + a.cos() * r * 1.3, by + a.sin() * r * 0.8);
    let mut left = false;
    if gather > 0.0 {
        x += (gx - x) * gather;
        y += (gy - y) * gather;
        left = gather > 0.5 && (-a.sin()) < 0.0;
    }
    // Burst apart as the jaws open.
    let flee = T_OPEN + 0.1 + rnd(fi + 106.0) * 0.15;
    if t > flee {
        let (px, py, _) = school(g, flee - 0.001, i);
        let (dx, dy) = (px - bx, py - by);
        let d = (dx * dx + dy * dy).sqrt().max(0.5);
        let sp = 50.0 + rnd(fi + 107.0) * 40.0;
        let e = t - flee;
        let vx = dx / d * sp - 30.0;
        let vy = dy / d * sp * 0.7;
        x = px + vx * e;
        y = py + vy * e;
        left = vx < 0.0;
    }
    (x, y, left)
}

fn turtle_at(g: &World, t: f64) -> (f64, f64) {
    // It meets the school in the middle of the second screen.
    let s1 = stage_at(g, 3, 6.0);
    let frac = ((stage_at(g, 1, 30.0) + 22.0) / (s1 + 22.0)).clamp(0.0, 1.0);
    let meet = T_ARRIVE * (1.0 - (1.0 - frac).powf(1.0 / 1.6));
    (
        (g.panel(1) + 32) as f64 + (meet - t) * 9.0,
        25.0 + (t * 0.8).sin() * 2.0,
    )
}

fn fish(g: &mut World, t: f64, f: u32) {
    let (lx, ly) = light(g, t);
    for i in 0..N_FISH {
        let (x, y, left) = school(g, t, i);
        if x < -8.0 || x > g.w() as f64 + 8.0 || !(-4.0..64.0).contains(&y) {
            continue;
        }
        let d = depth(g, x);
        let near = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
        let lit = if t < T_HOME {
            (1.0 - near / 22.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let shade = |c: u32| mix(mix(c, 0x0a1830, d * 0.8), mix(c, LURE, 0.35), lit);
        let pal = [
            ('b', shade(0x4a7ab8)),
            ('s', shade(0xc8e0f0)),
            ('w', shade(0xe8f4ff)),
            ('y', shade(0xf0c040)),
            ('k', shade(0xd8e8f8)),
            ('e', 0x101018),
        ];
        let beat = ((f / 2) as usize + i) % 2;
        g.spr(FISH[beat], &pal, x - 3.0, y - 1.0, left);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    water(g, t);
    far_reef(g);
    manta(g, t);
    jellyfish(g, t);
    reef(g, t, f);
    bubbles(g, t);
    let (tx, ty) = turtle_at(g, t);
    if tx > -20.0 && tx < g.w() as f64 + 20.0 {
        turtle(g, tx, ty, t);
    }
    angler(g, t, f);
    octopus(g, t, f);
    fish(g, t, f);
    lure_light(g, t);
}

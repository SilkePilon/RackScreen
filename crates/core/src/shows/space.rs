//! Space hitchhiker: a cosy little liner with lit portholes and a glowing twin
//! engine cruises past a ringed gas giant, through a tumbling asteroid belt
//! and a dithered pink-and-teal nebula to an icy blue planet. On a drifting
//! rock in the belt a tiny one-eyed alien sticks out a thumb; the liner
//! brakes and the hitchhiker hops on the roof. In the nebula a comet comes
//! screaming in, the liner ducks and it skims overhead, and the alien
//! snaps a chip of ice off its head and licks it like an ice lolly. At the
//! icy planet the alien jumps off onto its own pocket-sized planetoid with
//! a dome house and a mailbox; the window lights and blinks a thank-you,
//! the liner blinks its portholes back and boosts away.

use super::fx::{blend, glow, mix, on, put, stage_at, stage_x};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_THUMB: f64 = 2.4;
const T_HOP_ON: f64 = 4.0;
const T_ABOARD: f64 = 4.45;
const T_COMET: f64 = 6.0;
const T_MISS: f64 = 6.75;
const T_LICK: f64 = 7.1;
const T_HOP_OFF: f64 = 9.6;
const T_HOME: f64 = 10.1;
const T_THANKS: f64 = 10.5;
const T_REPLY: f64 = 10.95;
const T_BOOST: f64 = 11.2;

/// The liner, facing right; its engines are on the left.
const SHIP: &[&str] = &[
    ".....................kkkk.........",
    "...................kkbbcbk........",
    ".....kk...........kwbbbbbbkk......",
    ".....kfk......kkkkwwwwwwwwwwkkk...",
    ".....kffk..kkkwwwwwwwwwwwwwwwwwkk.",
    "eeee.kfffkkwwwwwwwwwwwwwwwwwwwwwwk",
    "EEEEkkwwwwwowwowwowwowwowwowwwwwhk",
    "eeeekwwwwwwwwwwwwwwwwwwwwwwwwwwwwk",
    "EEEEkttttttttttttttttttttttttttttk",
    "eeeekWWWWWWWWWWWWWWWWWWWWWWWWWWkk.",
    "....kgggggggggggggggggggggggggkk..",
    ".....kgGgggGgggGgggGgggGgggggk....",
    "......kkkkkkkkkkkkkkkkkkkkkkk.....",
];
const SHIPP: &[(char, u32)] = &[
    ('k', 0x23263a),
    ('w', 0xeeeae2),
    ('W', 0xc8c4c0),
    ('h', 0xfff4c0),
    ('G', 0x6a7088),
    ('E', 0x6a6e84),
    ('b', 0x5ac8e8),
    ('c', 0xd8f4ff),
    ('o', 0xf8d070),
    ('t', 0x2aa0a0),
    ('g', 0x8a90a8),
    ('f', 0xd84a4a),
    ('e', 0x4a4e64),
];
/// Porthole spots in the hull (sprite coords).
const PORTS: [i32; 6] = [11, 14, 17, 20, 23, 26];

const ALIENP: &[(char, u32)] = &[
    ('p', 0xf06ab0),
    ('g', 0x6ad04a),
    ('d', 0x3a9a3a),
    ('w', 0xffffff),
    ('k', 0x1a1a2a),
    ('i', 0xa8f0ff),
];
const HEART: &[&str] = &["h.h", "hhh", ".h."];

/// The alien: standing, thumbing a lift (arm out right), licking ice.
const ALIEN: [&[&str]; 3] = [
    &["..p..", "..g..", ".ggg.", "gwkwg", ".ggg.", ".d.d."],
    &["..p...", "..g..g", ".ggg.g", "gwkwgg", ".ggg..", ".d.d.."],
    &["..p..i", "..g.gi", ".ggg.g", "gwkwg.", ".ggg..", ".d.d.."],
];

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}
/// Screen of the asteroid belt and of the nebula.
fn belt_screen(g: &World) -> u32 {
    if g.layout.panels >= 4 {
        1
    } else {
        0
    }
}
fn nebula_screen(g: &World) -> u32 {
    g.layout.panels.saturating_sub(2)
}

/// The hitchhiker's rock and the home planetoid.
fn rock_at(g: &World, t: f64) -> (f64, f64) {
    let x = g.panel(belt_screen(g)) as f64 + 34.0 + (t * 0.6).sin() * 2.0;
    (x, 36.0 + (t * 0.9).cos() * 1.5)
}
fn home(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 42.0, 21.0)
}

/// Keyframes of the cruise: (time, stage position of the ship's nose, y).
fn keys(g: &World) -> Vec<(f64, f64, f64)> {
    let (rx, _) = rock_at(g, T_HOP_ON);
    let pick = stage_at(
        g,
        belt_screen(g),
        rx - g.panel(belt_screen(g)) as f64 + 20.0,
    );
    let (hx, _) = home(g);
    let drop = stage_at(g, last(g), hx - g.panel(last(g)) as f64 + 2.0);
    let neb = stage_at(g, nebula_screen(g), 0.0);
    vec![
        (-1.0, stage_at(g, 0, -40.0), 26.0),
        (0.0, stage_at(g, 0, -10.0), 30.0),
        (1.8, stage_at(g, 0, 52.0), 36.0),
        (T_HOP_ON - 0.4, pick - 3.0, 22.0),
        (T_ABOARD, pick, 22.0),
        (T_COMET, neb + 26.0, 27.0),
        (T_MISS, neb + 44.0, 33.0),
        (7.6, neb + 58.0, 28.0),
        (T_HOP_OFF, drop - 6.0, 31.0),
        (T_BOOST, drop, 32.0),
        (12.0, drop + 60.0, 30.0),
        (13.0, drop + 160.0, 30.0),
    ]
}

/// Catmull-Rom through the keys: stage position and y at `t`.
fn cruise(k: &[(f64, f64, f64)], t: f64) -> (f64, f64) {
    let n = k.len();
    let mut i = 0;
    while i + 2 < n && k[i + 1].0 <= t {
        i += 1;
    }
    let u = ((t - k[i].0) / (k[i + 1].0 - k[i].0)).clamp(0.0, 1.0);
    let (p0, p1, p2, p3) = (
        k[i.saturating_sub(1)],
        k[i],
        k[i + 1],
        k[(i + 2).min(n - 1)],
    );
    let cr = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * (2.0 * b
            + (-a + c) * u
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u
            + (-a + 3.0 * b - 3.0 * c + d) * u * u * u)
    };
    (cr(p0.1, p1.1, p2.1, p3.1), cr(p0.2, p1.2, p2.2, p3.2))
}

/// The ship's top-left corner in the world, and its speed (px/s).
fn ship_at(g: &World, t: f64) -> (f64, f64, f64) {
    let k = keys(g);
    let (s, y) = cruise(&k, t);
    let (s2, _) = cruise(&k, t + 0.08);
    let bob = (t * 2.2).sin() * 0.8;
    (stage_x(g, s) - 34.0, y + bob, (s2 - s) / 0.08)
}

fn stars(g: &mut World, t: f64, f: u32) {
    let (w, h) = (g.w(), g.h());
    // Deep space: a faint dithered blue at the bottom.
    for y in 0..h {
        for x in 0..w {
            let k = (y as f64 / h as f64) * 0.35;
            put(g, if on(x, y, k) { 0x0c0e24 } else { 0x05060f }, x, y);
        }
    }
    for (n, sp, c) in [
        (60u32, 1.0, 0x3a3e60),
        (34, 3.0, 0x8a90c0),
        (16, 7.0, 0xffffff),
    ] {
        for i in 0..g.spread(n) {
            let fi = i as f64 + sp * 100.0;
            let x = ((rnd(fi) * w as f64 - t * sp) % w as f64 + w as f64) % w as f64;
            let y = (rnd(fi + 1.0) * h as f64).floor();
            let tw = c == 0xffffff && (f + i * 5) % 31 < 2;
            put(g, if tw { 0xfff4c0 } else { c }, js_round(x), y as i32);
            if tw {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    blend(g, 0xfff4c0, js_round(x) + dx, y as i32 + dy, 0.5);
                }
            }
        }
    }
}

/// A banded gas giant with a ring, lit from the left, on the first screen.
fn gas_giant(g: &mut World, t: f64) {
    let (cx, cy, r) = (g.panel(0) as f64 + 18.0, 14.0, 20.0);
    let bands = [0xe8b878, 0xd89a5a, 0xf0d0a0, 0xc8784a, 0xe8b070, 0xb86a40];
    let ring = |g: &mut World, front: bool| {
        for a in 0..360 {
            let th = a as f64 / 360.0 * PI * 2.0;
            if (th.sin() > 0.0) != front {
                continue;
            }
            for (k, rr) in [31.0, 33.0, 34.0, 36.0].iter().enumerate() {
                let (x, y) = (
                    cx + th.cos() * rr,
                    cy + th.sin() * rr * 0.28 - th.cos() * rr * 0.12,
                );
                let c = [0xc8a878, 0xe8d0a0, 0xb89868, 0xd8c090][k];
                let (xi, yi) = (js_round(x), js_round(y));
                // The planet's shadow falls across the back of the ring.
                let shade = !front && (x - cx).abs() < r * 0.6;
                put(g, if shade { mix(c, 0x05060f, 0.6) } else { c }, xi, yi);
            }
        }
    };
    ring(g, false);
    let ri = r as i32;
    for y in -ri..=ri {
        for x in -ri..=ri {
            let d2 = (x * x + y * y) as f64;
            if d2 > r * r {
                continue;
            }
            let lat = (y as f64 + (x as f64 * 0.12)) / r;
            let swirl = (x as f64 * 0.25 + t * 0.6 + lat * 9.0).sin() * 0.6;
            let bi = (((lat + 1.0) * 5.5 + swirl) as usize).min(bands.len() - 1);
            let mut c = bands[bi];
            // Lit from the upper left, a dithered terminator.
            let lx = (x as f64 + 6.0) / r;
            let ly = (y as f64 + 5.0) / r;
            let dark = ((lx * lx + ly * ly).sqrt() - 0.55).max(0.0) * 1.3;
            if on(js_round(cx) + x, js_round(cy) + y, dark.min(0.95)) {
                c = mix(c, 0x1a0e18, 0.7);
            } else if d2.sqrt() > r - 1.5 {
                c = mix(c, 0xfff0d0, 0.2);
            }
            put(g, c, js_round(cx) + x, js_round(cy) + y);
        }
    }
    // The great storm spot drifting round.
    let sx = cx - 6.0 + ((t * 0.6) % 3.0) * 3.0;
    g.p(0xb04a30, sx, cy + 6.0, 4, 2);
    put(g, 0xe0805a, js_round(sx) + 1, js_round(cy) + 6);
    ring(g, true);
    // A small moon in its own orbit.
    let a = t * 0.5 + 1.0;
    let (mx, my) = (cx + 30.0 + a.cos() * 4.0, cy + 26.0 + a.sin() * 2.0);
    g.disc(0x9a94a8, mx, my, 2);
    put(g, 0xd8d4e4, js_round(mx) - 1, js_round(my) - 1);
}

/// Rocks of the belt: (x, y, size, seed) at time `t`.
fn belt(g: &mut World, t: f64, front: bool) {
    let x0 = g.panel(belt_screen(g)) as f64;
    let n = if front { 6 } else { 18 };
    for k in 0..n {
        let kf = k as f64 + if front { 50.0 } else { 0.0 };
        let span = 110.0;
        let sp = if front { 3.0 } else { 1.2 };
        let x = x0 - 25.0 + (rnd(kf + 1.0) * span - t * sp).rem_euclid(span);
        let y = if front {
            44.0 + rnd(kf + 2.0) * 14.0
        } else {
            4.0 + rnd(kf + 2.0) * 50.0
        };
        let r = if front {
            3 + (rnd(kf + 3.0) * 3.0) as i32
        } else {
            1 + (rnd(kf + 3.0) * 3.0) as i32
        };
        asteroid(g, x, y, r, kf, t, if front { 1.0 } else { 0.6 });
    }
}

/// A lumpy rock lit from the left; `k` dims it with distance.
fn asteroid(g: &mut World, x: f64, y: f64, r: i32, seed: f64, t: f64, k: f64) {
    let spin = t * (0.4 + rnd(seed + 9.0));
    for dy in -r..=r {
        for dx in -r..=r {
            let a = (dy as f64).atan2(dx as f64);
            let lump = 1.0 + 0.25 * (a * 3.0 + spin + seed).sin();
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if d > r as f64 * lump + 0.3 {
                continue;
            }
            let lit = -(dx as f64 + dy as f64) / (2.0 * r as f64 + 0.1);
            let mut c = if lit > 0.25 {
                0xb8a898
            } else if lit > -0.3 {
                0x8a7a70
            } else {
                0x4a4048
            };
            if r > 2 && rnd(seed + (dx * 7 + dy) as f64) > 0.85 {
                c = mix(c, 0x2a2430, 0.5);
            }
            put(g, mix(0x05060f, c, k), js_round(x) + dx, js_round(y) + dy);
        }
    }
}

/// A dithered pink and teal nebula over the third screen, fading out.
fn vnoise(x: f64, y: f64, seed: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |a: f64, b: f64| rnd(a * 57.0 + b * 131.0 + seed);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

fn nebula(g: &mut World, t: f64) {
    let c = g.panel(nebula_screen(g)) as f64 + 30.0;
    let fbm = |x: f64, y: f64, s: f64| {
        0.65 * vnoise(x / 18.0, y / 12.0, s) + 0.35 * vnoise(x / 6.0, y / 4.0, s + 3.0)
    };
    let ramp_p = [0x2a1040, 0x6a1e6a, 0xb83e8a, 0xf08ac0, 0xffd8f0];
    let ramp_t = [0x0a2030, 0x14505a, 0x2a9aa0, 0x8ae8e0];
    for y in 0..g.h() {
        for x in (c as i32 - 64)..(c as i32 + 64) {
            let (fx, fy) = (x as f64, y as f64);
            let d = ((fx - c) / 60.0).powi(2) + ((fy - 28.0) / 36.0).powi(2);
            if d > 1.0 {
                continue;
            }
            let fall = (1.0 - d).powf(1.2);
            let p = (fbm(fx - t * 0.6, fy, 1.0) * 1.5 - 0.56) * fall;
            let q = (fbm(fx + 40.0 + t * 0.4, fy + 9.0, 7.0) * 1.5 - 0.66) * fall;
            for (v, ramp, k) in [(q, &ramp_t[..], 0.6), (p, &ramp_p[..], 0.75)] {
                if v <= 0.0 {
                    continue;
                }
                let u = (v * (ramp.len() - 1) as f64 * 2.2).min(ramp.len() as f64 - 1.01);
                let i = u.floor() as usize;
                let col = if on(x, y, u - i as f64) {
                    ramp[i + 1]
                } else {
                    ramp[i]
                };
                blend(g, col, x, y, k * (v * 4.0).min(1.0));
            }
        }
    }
    // A young star glowing in its heart.
    glow(g, 0xffd8f0, c - 6.0, 18.0, 10.0, 0.5);
    put(g, 0xffffff, js_round(c - 6.0), 18);
}

/// The icy planet rising at the bottom of the last screen, and the home
/// planetoid with its house.
fn icy_world(g: &mut World, t: f64, lit: bool) {
    let x0 = g.panel(last(g)) as f64;
    let (cx, cy, r) = (x0 + 46.0, 76.0, 34.0);
    let ri = r as i32;
    for y in -ri..=ri {
        for x in -ri..=ri {
            let d = ((x * x + y * y) as f64).sqrt();
            let (wx, wy) = (js_round(cx) + x, js_round(cy) + y);
            if d > r || wy >= g.h() || wy < 0 {
                continue;
            }
            let band = ((y as f64 * 0.4 + (x as f64 * 0.1 + t * 0.3).sin() * 2.0) as i32) % 3;
            let mut c = [0x9ad8f0, 0xbce8f8, 0x7ac0e0][band.unsigned_abs() as usize];
            let lx = (x as f64 + 12.0) / r;
            let ly = (y as f64 + 14.0) / r;
            if on(
                wx,
                wy,
                (((lx * lx + ly * ly).sqrt() - 0.6) * 1.4).clamp(0.0, 0.9),
            ) {
                c = mix(c, 0x0a1430, 0.65);
            }
            if d > r - 1.2 {
                c = 0xe8f8ff;
            }
            put(g, c, wx, wy);
        }
    }
    // Atmosphere halo.
    for a in 0..200 {
        let th = a as f64 / 200.0 * PI * 2.0;
        blend(
            g,
            0x8ad8ff,
            js_round(cx + th.cos() * (r + 1.5)),
            js_round(cy + th.sin() * (r + 1.5)),
            0.35,
        );
    }
    // The pocket planetoid: a rock with a dome house and a mailbox.
    let (hx, hy) = home(g);
    g.disc(0x4a4058, hx, hy + 2.0, 8);
    g.disc(0x7a6a80, hx - 1.0, hy + 1.0, 7);
    g.disc(0x9a8aa0, hx - 2.0, hy, 4);
    put(g, 0x5a4a68, js_round(hx) + 2, js_round(hy) + 3);
    put(g, 0x5a4a68, js_round(hx) - 3, js_round(hy) + 4);
    // The house on top.
    g.disc(0xd8e0f0, hx + 1.0, hy - 7.0, 3);
    g.p(0xd8e0f0, hx - 2.0, hy - 7.0, 7, 2);
    g.p(0xa8b0c8, hx - 2.0, hy - 6.0, 7, 1);
    g.p(
        if lit { 0xf8d070 } else { 0x3a4060 },
        hx + 2.0,
        hy - 8.0,
        2,
        2,
    );
    if lit {
        glow(g, 0xf8d070, hx + 3.0, hy - 7.0, 6.0, 0.45);
    }
    g.p(0x6a5a78, hx - 1.0, hy - 7.0, 1, 2);
    // Antenna with a blinking tip.
    g.p(0xa8b0c8, hx + 1.0, hy - 13.0, 1, 3);
    put(
        g,
        if (t * 3.0).sin() > 0.0 {
            0xf06ab0
        } else {
            0x6a3a5a
        },
        js_round(hx) + 1,
        js_round(hy) - 14,
    );
    // Mailbox on a post.
    g.p(0x8a8a9a, hx - 6.0, hy - 6.0, 1, 3);
    g.p(0xd84a4a, hx - 7.0, hy - 8.0, 3, 2);
}

/// The comet: head position at `t`, coming in from the top right.
fn comet_at(g: &World, t: f64) -> Option<(f64, f64)> {
    if !(T_COMET..T_MISS + 1.2).contains(&t) {
        return None;
    }
    let (sx, sy, _) = ship_at(g, T_MISS);
    let u = (t - T_MISS) / (T_MISS - T_COMET);
    // Passes just above the ship's roof at T_MISS.
    Some((sx + 18.0 - u * 50.0, sy - 5.0 + u * 22.0 * 0.6))
}

fn comet(g: &mut World, t: f64, f: u32) {
    let Some((x, y)) = comet_at(g, t) else {
        return;
    };
    // Dithered tail streaming back up-right.
    for i in 0..40 {
        let fi = i as f64;
        let (tx, ty) = (x + fi * 1.4, y - fi * 0.55);
        let k = 1.0 - fi / 40.0;
        for j in -3i32..=3 {
            let spread = j as f64 * (0.3 + fi * 0.04);
            let (px, py) = (js_round(tx - spread * 0.4), js_round(ty + spread));
            if on(px, py + f as i32, k * (1.0 - (j as f64).abs() / 4.0)) {
                blend(
                    g,
                    if j.abs() < 2 { 0xc8f0ff } else { 0x6ab0e8 },
                    px,
                    py,
                    0.7 * k,
                );
            }
        }
    }
    glow(g, 0xd8f4ff, x, y, 9.0, 0.7);
    g.disc(0xe8faff, x, y, 2);
    put(g, 0xffffff, js_round(x), js_round(y));
}

fn ship(g: &mut World, t: f64, f: u32) {
    let (x, y, v) = ship_at(g, t);
    let boost = smooth(T_BOOST, T_BOOST + 0.3, t);
    // Engine exhaust: a dithered plume that grows with speed and the boost.
    let len = 4.0 + v.abs().min(60.0) * 0.12 + boost * 14.0;
    for i in 0..(len as i32) {
        let k = 1.0 - i as f64 / len;
        for row in [5, 8] {
            let (ex, ey) = (js_round(x) - 1 - i, js_round(y) + row);
            let flick = rnd((f as i32 * 13 + i) as f64) * 0.3;
            for dy in 0..2 {
                if on(ex, ey + dy, k - flick + 0.1) {
                    let c = if i < 2 {
                        0xfff4d0
                    } else if k > 0.5 {
                        0x8ae8ff
                    } else {
                        0x3a7ad8
                    };
                    blend(g, c, ex, ey + dy, 0.85 * k + 0.15);
                }
            }
        }
    }
    let pulse = 0.5 + 0.2 * ((f as f64) * 1.3).sin() + boost * 0.3;
    glow(g, 0x6ad8ff, x - 1.0, y + 7.0, 7.0 + boost * 4.0, pulse);
    g.spr(SHIP, SHIPP, x, y, false);
    // Running light on the fin and a headlight.
    if (f / 6).is_multiple_of(2) {
        put(g, 0xff5a5a, js_round(x) + 6, js_round(y) + 1);
        glow(g, 0xff5a5a, x + 6.0, y + 1.0, 3.0, 0.4);
    }
    glow(g, 0xfff4c0, x + 34.0, y + 6.0, 6.0, 0.45);
    // Portholes blink back their thanks.
    let reply = (T_REPLY..T_REPLY + 0.6).contains(&t) && ((t - T_REPLY) * 6.0) as i32 % 2 == 0;
    for p in PORTS {
        let c = if reply { 0xffffff } else { 0xf8d070 };
        put(g, c, js_round(x) + p, js_round(y) + 6);
        if reply {
            glow(g, 0xfff0b0, x + p as f64, y + 6.0, 4.0, 0.5);
        }
    }
    // The captain in the cockpit.
    put(g, 0x2a3a5a, js_round(x) + 22, js_round(y) + 3);
    put(g, 0xf0c8a0, js_round(x) + 22, js_round(y) + 2);
}

fn alien(g: &mut World, t: f64, f: u32) {
    let (rx, ry) = rock_at(g, t);
    let (sx, sy, _) = ship_at(g, t);
    let roof = (sx + 12.0, sy - 2.0);
    if t < T_HOP_ON {
        let pose = if t > T_THUMB { 1 } else { 0 };
        let bob = if pose == 1 && (f / 4).is_multiple_of(2) {
            -1.0
        } else {
            0.0
        };
        g.spr(ALIEN[pose], ALIENP, rx - 2.0, ry - 9.0 + bob, false);
    } else if t < T_ABOARD {
        let u = (t - T_HOP_ON) / (T_ABOARD - T_HOP_ON);
        let x = rx - 2.0 + (roof.0 - rx + 2.0) * u;
        let y = ry - 9.0 + (roof.1 - ry + 9.0) * u - (u * PI).sin() * 8.0;
        g.spr(ALIEN[0], ALIENP, x, y, false);
    } else if t < T_HOP_OFF {
        // Riding on the roof; ducks as the comet skims over, then licks ice.
        let duck = (T_MISS - 0.25..T_MISS + 0.15).contains(&t);
        let licking = t > T_LICK;
        let pose = if licking { 2 } else { 0 };
        if duck {
            g.spr(&ALIEN[0][2..], ALIENP, roof.0, roof.1 + 2.0, false);
        } else {
            g.spr(ALIEN[pose], ALIENP, roof.0, roof.1, false);
        }
        if licking && (f / 3).is_multiple_of(4) {
            put(g, 0xffffff, js_round(roof.0) + 6, js_round(roof.1) - 1);
        }
        if (T_MISS..T_MISS + 0.4).contains(&t) {
            // The ice chip flicks off the comet into its hand.
            let u = (t - T_MISS) / 0.4;
            put(
                g,
                0xa8f0ff,
                js_round(roof.0 + 5.0 + 6.0 * (1.0 - u)),
                js_round(roof.1 - 5.0 * (1.0 - u)),
            );
        }
    } else if t < T_HOME {
        let (hx, hy) = home(g);
        let u = (t - T_HOP_OFF) / (T_HOME - T_HOP_OFF);
        let x = roof.0 + (hx - 6.0 - roof.0) * u;
        let y = roof.1 + (hy - 14.0 - roof.1) * u - (u * PI).sin() * 6.0;
        g.spr(ALIEN[0], ALIENP, x, y, false);
    } else {
        let (hx, hy) = home(g);
        // Home: waves from the doorstep, goes in, and a heart floats up.
        if t < T_THANKS {
            let wave = if (f / 3).is_multiple_of(2) { 1 } else { 0 };
            g.spr(ALIEN[wave], ALIENP, hx - 7.0, hy - 14.0, true);
        } else {
            let u = t - T_THANKS;
            let y = hy - 12.0 - u * 5.0;
            let x = hx + 1.0 + (u * 5.0).sin() * 1.5;
            g.spr(HEART, &[('h', 0xf06ab0)], x, y, false);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    stars(g, t, f);
    nebula(g, t);
    gas_giant(g, t);
    belt(g, t, false);
    // The house window: lit once the alien is in, blinking its thanks.
    let lit = t > T_THANKS && !((T_THANKS + 0.15..T_THANKS + 0.3).contains(&t));
    icy_world(g, t, lit);
    let (rx, ry) = rock_at(g, t);
    asteroid(g, rx, ry, 5, 7.0, t * 0.2, 1.0);
    comet(g, t, f);
    ship(g, t, f);
    alien(g, t, f);
    belt(g, t, true);
    if (T_MISS - 0.1..T_MISS + 0.2).contains(&t) {
        // Spray of ice where the comet grazes the roof.
        let (sx, sy, _) = ship_at(g, t);
        for k in 0..10 {
            let a = rnd(k as f64 + 70.0) * PI - PI;
            let r = (t - T_MISS + 0.1) * 40.0 * rnd(k as f64 + 71.0);
            put(
                g,
                0xd8f4ff,
                js_round(sx + 18.0 + a.cos() * r),
                js_round(sy - 3.0 + a.sin() * r * 0.5),
            );
        }
    }
}

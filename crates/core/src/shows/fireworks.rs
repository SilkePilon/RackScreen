//! Fireworks over the harbour: a city skyline at night across the water,
//! a lit ferris wheel turning, low clouds and a crowd along the quay rail
//! with a kid on a parent's shoulders and a phone held high. Shells go up
//! one screen at a time: a peony, a golden willow, a ring, a crossette that
//! splits, a glitter strobe. Every burst lights the clouds, mirrors in the
//! water and leaves smoke drifting off on the breeze. One rocket from the
//! barge is a dud: it sputters, falls back onto the deck and smoulders, and
//! a little dog on the quay tilts its head at it. The grand finale fills
//! every screen and the dog dives under a bench. In the quiet after, the
//! dud fizzes back to life, shoots up late and bursts into a big pink
//! heart; the crowd cheers loudest of all and the dog peeks out, wagging.

use super::fx::{blend, get, glow, mix, on, put, vgrad};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// The waterline.
const HZ: i32 = 35;
/// The quay rail the crowd leans on.
const RAIL: i32 = 49;
/// How long a rocket climbs before it bursts.
const CLIMB: f64 = 0.7;

const T_DUD: f64 = 4.1;
const T_DUD_FIZZ: f64 = 4.8;
const T_DUD_LAND: f64 = 5.5;
const T_FINALE: f64 = 6.0;
const T_HUSH: f64 = 8.6;
const T_RELIGHT: f64 = 9.3;
const T_HEART: f64 = 10.2;
/// When the late rocket leaves the barge.
const T_LATE: f64 = T_HEART - CLIMB;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Peony,
    Willow,
    Ring,
    Crossette,
    Strobe,
    Palm,
    Heart,
}

/// Star colours: `[white-hot, main, fading]`.
const RED: [u32; 3] = [0xfff0e0, 0xff5a4a, 0x7a2030];
const GOLD: [u32; 3] = [0xfff8d0, 0xffc050, 0x8a5020];
const CYAN: [u32; 3] = [0xf0ffff, 0x6ae8f8, 0x205a8a];
const GREEN: [u32; 3] = [0xf8fff0, 0x9af06a, 0x2a6a3a];
const VIOLET: [u32; 3] = [0xfff0ff, 0xc08aff, 0x4a2a7a];
const PINK: [u32; 3] = [0xfff0f8, 0xff7ab8, 0x8a2a5a];
const WHITE: [u32; 3] = [0xffffff, 0xe8f0ff, 0x6a7090];

/// `(launch time, panel, x on the panel, burst y, kind, colours)`.
type ShellDef = (f64, u32, f64, f64, Kind, [u32; 3]);

const SHELLS: &[ShellDef] = &[
    (0.3, 0, 30.0, 18.0, Kind::Peony, RED),
    (1.3, 1, 28.0, 15.0, Kind::Willow, GOLD),
    (2.3, 2, 32.0, 18.0, Kind::Ring, CYAN),
    (3.2, 3, 30.0, 16.0, Kind::Crossette, GREEN),
    (4.6, 1, 32.0, 19.0, Kind::Strobe, WHITE),
    (5.2, 0, 26.0, 16.0, Kind::Peony, VIOLET),
    (5.5, 3, 34.0, 20.0, Kind::Palm, GOLD),
    // The finale: every screen, twice over.
    (T_FINALE, 0, 22.0, 14.0, Kind::Peony, CYAN),
    (T_FINALE, 2, 38.0, 14.0, Kind::Peony, RED),
    (T_FINALE + 0.2, 1, 36.0, 20.0, Kind::Ring, PINK),
    (T_FINALE + 0.3, 3, 24.0, 15.0, Kind::Willow, GOLD),
    (T_FINALE + 0.6, 0, 40.0, 21.0, Kind::Crossette, VIOLET),
    (T_FINALE + 0.7, 2, 20.0, 20.0, Kind::Strobe, WHITE),
    (T_FINALE + 0.9, 1, 22.0, 13.0, Kind::Peony, GREEN),
    (T_FINALE + 1.0, 3, 38.0, 21.0, Kind::Peony, PINK),
    (T_FINALE + 1.3, 0, 30.0, 16.0, Kind::Willow, GOLD),
    (T_FINALE + 1.3, 1, 30.0, 17.0, Kind::Willow, GOLD),
    (T_FINALE + 1.3, 2, 30.0, 16.0, Kind::Willow, GOLD),
    (T_FINALE + 1.3, 3, 30.0, 17.0, Kind::Willow, GOLD),
    (T_FINALE + 1.6, 0, 30.0, 22.0, Kind::Strobe, WHITE),
    (T_FINALE + 1.6, 3, 30.0, 22.0, Kind::Strobe, WHITE),
    (T_FINALE + 1.7, 1, 30.0, 14.0, Kind::Ring, RED),
    (T_FINALE + 1.7, 2, 30.0, 14.0, Kind::Ring, CYAN),
];

/// A shell in the air or bursting: where, what, and how long since the burst.
#[derive(Clone, Copy)]
struct Shell {
    x: f64,
    y: f64,
    kind: Kind,
    col: [u32; 3],
    /// Seconds since launch.
    age: f64,
    seed: f64,
}

impl Shell {
    fn life(&self) -> f64 {
        match self.kind {
            Kind::Willow => 2.8,
            Kind::Heart => 2.4,
            Kind::Palm => 2.2,
            _ => 1.8,
        }
    }
    /// Seconds since the burst (negative while climbing).
    fn e(&self) -> f64 {
        self.age - CLIMB
    }
    /// How bright the burst's flash is now.
    fn flash(&self) -> f64 {
        let e = self.e();
        if e < 0.0 {
            0.0
        } else {
            (-e * 2.2).exp()
        }
    }
}

fn barge(g: &World) -> (f64, f64) {
    (g.panel(2) as f64 + 14.0, HZ as f64 + 6.0)
}

fn shells(g: &World, t: f64) -> Vec<Shell> {
    let mut v = Vec::new();
    for (i, &(t0, p, dx, y, kind, col)) in SHELLS.iter().enumerate() {
        // The finale skips a screen the layout lacks; the solo shells move
        // onto the last screen instead.
        if t0 >= T_FINALE && p >= g.layout.panels {
            continue;
        }
        let age = t - t0;
        let s = Shell {
            x: g.panel(p) as f64 + dx,
            y,
            kind,
            col,
            age,
            seed: i as f64 * 17.0,
        };
        if age >= 0.0 && s.e() < s.life() {
            v.push(s);
        }
    }
    // The late one.
    let (bx, _) = barge(g);
    let s = Shell {
        x: bx + 12.0,
        y: 17.0,
        kind: Kind::Heart,
        col: PINK,
        age: t - (T_HEART - CLIMB),
        seed: 999.0,
    };
    if s.age >= 0.0 && s.e() < s.life() {
        v.push(s);
    }
    v
}

/// The strongest burst light at `(x, y)`: colour and strength.
fn light(bursts: &[Shell], x: f64, y: f64) -> (u32, f64) {
    let mut best = (0, 0.0);
    for s in bursts {
        let d = ((x - s.x).powi(2) + (y - s.y).powi(2)).sqrt();
        let k = s.flash() * (1.0 - d / 44.0).max(0.0);
        if k > best.1 {
            best = (s.col[1], k);
        }
    }
    best
}

fn sky(g: &mut World, t: f64, bursts: &[Shell]) {
    let w = g.w();
    vgrad(
        g,
        &[0x05061a, 0x0a0e2a, 0x141638, 0x261c46, 0x3a2448],
        0,
        w,
        0,
        HZ,
    );
    for i in 0..g.spread(40) {
        let fi = i as f64;
        let (x, y) = ((rnd(fi) * w as f64) as i32, (rnd(fi + 1.0) * 26.0) as i32);
        let tw = 0.35 + 0.4 * ((t * 2.0 + fi).sin() * 0.5 + 0.5);
        blend(g, 0xd8e0ff, x, y, tw * rnd(fi + 2.0));
    }
    // Low clouds, lit from below by whatever bursts near them.
    for k in 0..g.spread(10) {
        let kf = k as f64;
        let span = w as f64 + 60.0;
        let cx = (rnd(kf + 20.0) * span + t * 1.2) % span - 30.0;
        let cy = 4.0 + rnd(kf + 21.0) * 12.0;
        let len = 16 + (rnd(kf + 22.0) * 26.0) as i32;
        for row in 0..3 {
            let inset = [4, 0, 2][row];
            for i in inset..len - inset {
                let x = js_round(cx) + i;
                let y = js_round(cy) + row as i32;
                let (c, k) = light(bursts, x as f64, y as f64);
                let base = [0x1a1c3a, 0x20223e, 0x161834][row];
                let lit = (k * [0.5, 0.8, 1.0][row]).min(0.9);
                let edge = i == inset || i == len - inset - 1;
                if !edge || on(x, y, 0.5) {
                    put(g, mix(base, c, lit), x, y);
                }
            }
        }
    }
}

/// Smoke each burst leaves, drifting right on the breeze.
fn smoke(g: &mut World, t: f64, bursts: &[Shell]) {
    for &(t0, p, dx, y, _, _) in SHELLS {
        if p >= g.layout.panels && t0 >= T_FINALE {
            continue;
        }
        let e = t - t0 - CLIMB;
        if !(0.2..6.0).contains(&e) {
            continue;
        }
        let x = g.panel(p) as f64 + dx;
        smoke_puff(g, x, y, e, t0, bursts);
    }
    let (bx, _) = barge(g);
    let e = t - T_HEART;
    if e > 0.2 {
        smoke_puff(g, bx + 12.0, 17.0, e, 77.0, bursts);
    }
}

fn smoke_puff(g: &mut World, x: f64, y: f64, e: f64, seed: f64, bursts: &[Shell]) {
    let fade = (1.0 - e / 6.0).max(0.0) * (e / 0.6).min(1.0);
    for k in 0..5 {
        let kf = k as f64 + seed;
        let px = x + (rnd(kf) - 0.5) * 16.0 + e * 2.2;
        let py = y + (rnd(kf + 1.0) - 0.4) * 10.0 - e * 0.6;
        let r = 2.5 + e * 1.1 + rnd(kf + 2.0) * 2.0;
        let ri = r.ceil() as i32;
        for j in -ri..=ri {
            for i in -ri..=ri {
                let d = ((i * i + j * j) as f64).sqrt() / r;
                if d < 1.0 {
                    let (xx, yy) = (js_round(px) + i, js_round(py) + j);
                    let (c, l) = light(bursts, xx as f64, yy as f64);
                    let col = mix(0x4a4a66, c, (l * 1.2).min(0.8));
                    blend(g, col, xx, yy, 0.28 * fade * (1.0 - d * d));
                }
            }
        }
    }
}

fn skyline(g: &mut World, f: u32, t: f64, bursts: &[Shell]) {
    let w = g.w();
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64;
        let bw = 5 + (rnd(fi + 3.0) * 8.0) as i32;
        let bh = 3 + (rnd(fi + 33.0) * 6.0) as i32 + if i % 7 == 3 { 6 } else { 0 };
        let top = HZ - bh;
        let body = if i % 2 == 0 { 0x0e1026 } else { 0x121530 };
        g.p(body, x as f64, top as f64, bw, bh);
        // Rim light on the roofline from the nearest burst.
        let (c, k) = light(bursts, x as f64 + bw as f64 / 2.0, top as f64);
        if k > 0.05 {
            for xx in x..x + bw {
                blend(g, c, xx, top, k * 0.8);
            }
        }
        for wy in (top + 2..HZ - 1).step_by(2) {
            for wx in (x + 1..x + bw - 1).step_by(2) {
                let r = rnd((wx * 7 + wy * 13) as f64);
                if r > 0.72 {
                    let c = if r > 0.9 { 0xffe6a0 } else { 0xb88a4a };
                    put(g, c, wx, wy);
                }
            }
        }
        if i % 7 == 3 {
            // A tower with a blinking aircraft light.
            g.p(body, (x + bw / 2) as f64, (top - 3) as f64, 1, 3);
            if (f / 6).is_multiple_of(2) {
                put(g, 0xff3a3a, x + bw / 2, top - 4);
                glow(g, 0xff3a3a, (x + bw / 2) as f64, (top - 4) as f64, 3.0, 0.4);
            }
        }
        x += bw;
        i += 1;
    }
    ferris_wheel(g, g.panel(1) as f64 + 48.0, HZ as f64 - 10.0, t);
}

fn ferris_wheel(g: &mut World, cx: f64, cy: f64, t: f64) {
    let r = 7.0;
    // Legs.
    super::fx::line(g, 0x1a1c3a, cx, cy, cx - 5.0, HZ as f64);
    super::fx::line(g, 0x1a1c3a, cx, cy, cx + 5.0, HZ as f64);
    g.ring(0x2a2c4a, cx, cy, r);
    let lights = [0xff6a8a, 0xffd060, 0x7ae0ff, 0xa0ff8a];
    for k in 0..12 {
        let a = k as f64 / 12.0 * PI * 2.0 + t * 0.35;
        let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
        if k % 3 == 0 {
            super::fx::line(g, 0x22243e, cx, cy, x, y);
        }
        let c = lights[(k + (t * 3.0) as usize) % 4];
        put(g, c, js_round(x), js_round(y));
    }
    put(g, 0xfff0c0, js_round(cx), js_round(cy));
}

/// The climb: a hot head and a sparking tail.
fn rocket(g: &mut World, x: f64, y0: f64, y1: f64, u: f64, seed: f64, f: u32) {
    let u = 1.0 - (1.0 - u.clamp(0.0, 1.0)).powi(2);
    let y = y0 + (y1 - y0) * u;
    let wob = ((u * 9.0 + seed).sin()) * 0.6;
    for k in 0..6 {
        let ty = y + k as f64 * 1.3;
        let c = mix(0xffd080, 0x8a3a20, k as f64 / 5.0);
        blend(
            g,
            c,
            js_round(x + wob * (1.0 - k as f64 / 6.0)),
            js_round(ty),
            1.0 - k as f64 * 0.14,
        );
    }
    for k in 0..3 {
        let s = seed + (f + k) as f64 * 3.1;
        if rnd(s) > 0.4 {
            put(
                g,
                0xffb060,
                js_round(x + (rnd(s + 1.0) - 0.5) * 4.0),
                js_round(y + 3.0 + rnd(s + 2.0) * 6.0),
            );
        }
    }
    put(g, 0xfffff0, js_round(x + wob), js_round(y));
    glow(g, 0xffc080, x, y, 4.0, 0.5);
}

/// Where star `dir` (unit) of a shell is at `e` seconds after the burst.
#[allow(clippy::too_many_arguments)]
fn star_at(
    cx: f64,
    cy: f64,
    dx: f64,
    dy: f64,
    reach: f64,
    drag: f64,
    grav: f64,
    e: f64,
) -> (f64, f64) {
    let r = reach * (1.0 - (-drag * e).exp());
    (cx + dx * r, cy + dy * r + grav * e * e)
}

/// The colour of a star by age `u` in `0..1` of the shell's life.
fn star_col(col: [u32; 3], u: f64) -> u32 {
    if u < 0.12 {
        col[0]
    } else if u < 0.55 {
        mix(col[0], col[1], (u - 0.12) / 0.2)
    } else {
        mix(col[1], col[2], (u - 0.55) / 0.45)
    }
}

/// One star with a short fading trail.
#[allow(clippy::too_many_arguments)]
fn star(
    g: &mut World,
    s: &Shell,
    dx: f64,
    dy: f64,
    reach: f64,
    drag: f64,
    grav: f64,
    trail: usize,
    k: usize,
    f: u32,
) {
    let e = s.e();
    let u = e / s.life();
    // Flicker out at the end.
    if u > 0.7 && rnd(s.seed + k as f64 * 3.0 + f as f64) < (u - 0.7) * 2.5 {
        return;
    }
    let c = star_col(s.col, u);
    for j in (1..=trail).rev() {
        let ee = (e - j as f64 * 0.05).max(0.0);
        let (x, y) = star_at(s.x, s.y, dx, dy, reach, drag, grav, ee);
        blend(
            g,
            star_col(s.col, u + 0.2),
            js_round(x),
            js_round(y),
            0.8 - j as f64 / (trail as f64 + 1.0) * 0.7,
        );
    }
    let (x, y) = star_at(s.x, s.y, dx, dy, reach, drag, grav, e);
    put(g, c, js_round(x), js_round(y));
}

fn burst(g: &mut World, s: &Shell, f: u32) {
    let e = s.e();
    if e < 0.0 {
        let y0 = if s.kind == Kind::Heart {
            HZ as f64 + 4.0
        } else {
            HZ as f64 - 2.0
        };
        rocket(g, s.x, y0, s.y, s.age / CLIMB, s.seed, f);
        return;
    }
    // The flash.
    glow(g, s.col[1], s.x, s.y, 30.0, 0.35 * s.flash());
    if e < 0.1 {
        glow(g, 0xffffff, s.x, s.y, 6.0, 0.8);
        g.disc(s.col[0], s.x, s.y, 1);
    }
    match s.kind {
        Kind::Peony => {
            for k in 0..30 {
                let a = k as f64 / 30.0 * PI * 2.0 + rnd(s.seed + k as f64) * 0.15;
                let reach = 13.0 + rnd(s.seed + k as f64 * 2.0) * 5.0;
                star(g, s, a.cos(), a.sin(), reach, 4.0, 5.0, 2, k, f);
            }
        }
        Kind::Willow => {
            for k in 0..24 {
                let a = k as f64 / 24.0 * PI * 2.0;
                let reach = 11.0 + rnd(s.seed + k as f64) * 3.0;
                star(g, s, a.cos(), a.sin() * 0.85, reach, 3.0, 4.5, 7, k, f);
            }
        }
        Kind::Ring => {
            let tilt = rnd(s.seed) * 0.8 - 0.4;
            for k in 0..28 {
                let a = k as f64 / 28.0 * PI * 2.0;
                let (cx, cy) = (a.cos(), a.sin() * 0.55);
                let (dx, dy) = (
                    cx * tilt.cos() - cy * tilt.sin(),
                    cx * tilt.sin() + cy * tilt.cos(),
                );
                star(g, s, dx, dy, 17.0, 4.5, 3.0, 1, k, f);
            }
            // A small core.
            for k in 0..8 {
                let a = k as f64 / 8.0 * PI * 2.0 + 0.3;
                star(g, s, a.cos(), a.sin(), 4.0, 5.0, 3.0, 1, k + 40, f);
            }
        }
        Kind::Crossette => {
            for k in 0..6 {
                let a = k as f64 / 6.0 * PI * 2.0 + 0.4;
                let split = 0.45;
                if e < split {
                    star(g, s, a.cos(), a.sin(), 11.0, 3.0, 2.0, 3, k, f);
                } else {
                    // Each star breaks into a cross.
                    let (ox, oy) = star_at(s.x, s.y, a.cos(), a.sin(), 11.0, 3.0, 2.0, split);
                    let sub = Shell {
                        x: ox,
                        y: oy,
                        age: s.age - split,
                        ..*s
                    };
                    if e < split + 0.08 {
                        glow(g, 0xffffff, ox, oy, 3.0, 0.7);
                    }
                    for q in 0..4 {
                        let b = a + PI / 4.0 + q as f64 * PI / 2.0;
                        star(g, &sub, b.cos(), b.sin(), 7.0, 4.0, 4.0, 2, k * 4 + q, f);
                    }
                }
            }
        }
        Kind::Strobe => {
            for k in 0..26 {
                let kf = k as f64;
                let a = kf / 26.0 * PI * 2.0 + rnd(s.seed + kf);
                let reach = 6.0 + rnd(s.seed + kf * 2.0) * 9.0;
                let (x, y) = star_at(s.x, s.y, a.cos(), a.sin(), reach, 3.5, 2.5, e);
                if e < 0.35 {
                    star(g, s, a.cos(), a.sin(), reach, 3.5, 2.5, 2, k, f);
                } else if rnd(kf * 5.0 + f as f64 * 1.7 + s.seed) > 0.55 {
                    put(g, 0xffffff, js_round(x), js_round(y));
                    blend(g, 0xd8e8ff, js_round(x) + 1, js_round(y), 0.5);
                    blend(g, 0xd8e8ff, js_round(x), js_round(y) - 1, 0.5);
                }
            }
        }
        Kind::Palm => {
            for k in 0..7 {
                let a = -PI / 2.0 + (k as f64 - 3.0) * 0.5;
                star(g, s, a.cos(), a.sin() * 0.7, 15.0, 2.6, 6.5, 9, k, f);
            }
        }
        Kind::Heart => {
            let grow = 1.0 - (-4.0 * e).exp();
            for k in 0..40 {
                let a = k as f64 / 40.0 * PI * 2.0;
                let hx = 16.0 * a.sin().powi(3);
                let hy = -(13.0 * a.cos()
                    - 5.0 * (2.0 * a).cos()
                    - 2.0 * (3.0 * a).cos()
                    - (4.0 * a).cos());
                let (x, y) = (
                    s.x + hx * 0.85 * grow,
                    s.y + hy * 0.85 * grow - 2.0 + e * e * 1.5,
                );
                let u = e / s.life();
                if u > 0.75 && rnd(k as f64 + f as f64) < (u - 0.75) * 3.0 {
                    continue;
                }
                put(g, star_col(s.col, u * 0.8), js_round(x), js_round(y));
                if (k + (f as usize / 2)).is_multiple_of(5) {
                    put(g, 0xffffff, js_round(x), js_round(y));
                }
            }
        }
    }
}

/// Where the dud is: `Some((x, y, lit))`, or nothing once it has gone up.
fn dud(g: &World, t: f64) -> Option<(f64, f64, bool)> {
    let (bx, by) = barge(g);
    let (x0, y0) = (bx + 12.0, by - 3.0);
    if t < T_DUD {
        return None;
    }
    if t < T_DUD_FIZZ {
        let u = (t - T_DUD) / (T_DUD_FIZZ - T_DUD);
        let y = y0 - (y0 - 24.0) * (1.0 - (1.0 - u).powi(3));
        return Some((x0 + (u * 14.0).sin() * 1.5, y, true));
    }
    if t < T_DUD_LAND {
        let u = (t - T_DUD_FIZZ) / (T_DUD_LAND - T_DUD_FIZZ);
        return Some((x0 + u * 2.0, 24.0 + (y0 - 24.0) * u * u, false));
    }
    if t < T_LATE {
        return Some((x0 + 2.0, y0, false));
    }
    None
}

fn barge_draw(g: &mut World, t: f64, f: u32) {
    let (bx, by) = barge(g);
    let bob = (t * 1.6).sin() * 0.4;
    let y = by + bob;
    g.p(0x0a0c1c, bx - 1.0, y, 30, 2);
    g.p(0x161a30, bx, y + 2.0, 28, 1);
    g.p(0x2a2440, bx - 1.0, y - 1.0, 30, 1);
    // Launch racks.
    for k in 0..5 {
        g.p(0x1e2038, bx + 2.0 + k as f64 * 3.0, y - 4.0, 2, 3);
    }
    // A worker in a hard hat.
    g.p(0x0a0c1c, bx + 21.0, y - 6.0, 3, 5);
    g.p(0xf0a030, bx + 21.0, y - 7.0, 3, 1);
    // Warning light.
    if (f / 4).is_multiple_of(2) {
        put(g, 0xffa030, js_round(bx), js_round(y) - 3);
        glow(g, 0xffa030, bx, y - 3.0, 4.0, 0.4);
    }
    if let Some((x, y, lit)) = dud(g, t) {
        if lit {
            rocket(g, x, y, y, 1.0, 5.0, f);
            if t > T_DUD_FIZZ - 0.25 {
                // Sputtering.
                for k in 0..3 {
                    let s = (f * 3 + k) as f64;
                    put(
                        g,
                        0xffd080,
                        js_round(x + (rnd(s) - 0.5) * 6.0),
                        js_round(y + (rnd(s + 1.0) - 0.5) * 6.0),
                    );
                }
            }
        } else {
            // The spent shell, then smouldering on the deck.
            g.p(0x4a3a3a, x, y - 3.0, 1, 3);
            let ember = if t > T_RELIGHT {
                0xffe080
            } else if (f / 3).is_multiple_of(3) {
                0xff6a3a
            } else {
                0xa03a2a
            };
            put(g, ember, js_round(x), js_round(y) - 3);
            // A wisp of smoke.
            for k in 0..6 {
                let kf = k as f64;
                let rise = (t * 3.0 + kf) % 6.0;
                blend(
                    g,
                    0x6a6a88,
                    js_round(x + (rise * 1.3 + t).sin() * 1.2 + rise * 0.4),
                    js_round(y - 4.0 - rise * 1.6),
                    0.45 * (1.0 - rise / 6.0),
                );
            }
            if t > T_RELIGHT {
                // It fizzes back to life.
                for k in 0..5 {
                    let s = (f * 5 + k) as f64;
                    let a = rnd(s) * PI;
                    put(
                        g,
                        0xffd080,
                        js_round(x + a.cos() * 3.0),
                        js_round(y - 3.0 - a.sin() * 3.0),
                    );
                }
                glow(g, 0xffb050, x, y - 3.0, 5.0, 0.6);
            }
        }
        // The puff where it gave up.
        let e = t - T_DUD_FIZZ;
        if (0.0..1.2).contains(&e) {
            let (px, py) = (bx + 12.0, 24.0);
            for k in 0..4 {
                let a = k as f64 * 1.7;
                g.disc(
                    mix(0x8a8aa8, 0x2a2a44, e / 1.2),
                    px + a.cos() * (1.0 + e * 2.0),
                    py + a.sin() * (1.0 + e * 2.0) - e * 2.0,
                    1,
                );
            }
        }
    }
}

/// The water mirrors the sky and skyline, darker and rippled; bright bursts
/// throw glitter columns down it.
fn water(g: &mut World, t: f64, f: u32, bursts: &[Shell]) {
    let w = g.w();
    for y in HZ..RAIL {
        let depth = (y - HZ) as f64 / (RAIL - HZ) as f64;
        let src = 2 * HZ - 1 - y;
        for x in 0..w {
            let wob = js_round(
                ((y as f64) * 1.1 + t * 2.5 + x as f64 * 0.07).sin() * (0.6 + depth * 1.6),
            );
            let c = get(g, x + wob, src.max(0));
            put(
                g,
                mix(mix(c, 0x1a2a5a, 0.25), 0x060a22, 0.2 + depth * 0.3),
                x,
                y,
            );
        }
    }
    for s in bursts {
        let k = s.flash();
        if k < 0.05 {
            continue;
        }
        for y in HZ..RAIL {
            for j in 0..2 {
                let seed = (y * 31 + j * 7) as f64 + s.seed + (f / 2) as f64 * 0.37;
                let x = s.x + (rnd(seed) - 0.5) * (8.0 + (y - HZ) as f64 * 0.8);
                let len = 1 + (rnd(seed + 1.0) * 3.0) as i32;
                for i in 0..len {
                    blend(g, s.col[1], js_round(x) + i, y, 0.7 * k);
                }
            }
        }
    }
    // Ripple lines.
    for k in 0..g.spread(28) {
        let kf = k as f64;
        let y = HZ + 1 + (rnd(kf + 40.0) * (RAIL - HZ - 1) as f64) as i32;
        let x = (rnd(kf + 41.0) * w as f64 + t * 2.0) % w as f64;
        for i in 0..(3 + (rnd(kf + 42.0) * 5.0) as i32) {
            blend(g, 0x5a5a8a, js_round(x) + i, y, 0.3);
        }
    }
}

fn quay(g: &mut World) {
    let w = g.w();
    g.p(0x0a0a18, 0.0, RAIL as f64, w, 60 - RAIL);
    g.p(0x2a2a40, 0.0, RAIL as f64, w, 1);
    g.p(0x1a1a2c, 0.0, (RAIL + 1) as f64, w, 1);
    // The rail.
    g.p(0x3a3a52, 0.0, (RAIL - 4) as f64, w, 1);
    for x in (2..w).step_by(7) {
        g.p(0x2a2a40, x as f64, (RAIL - 4) as f64, 1, 4);
    }
}

const DOGP: &[(char, u32)] = &[
    ('k', 0x0c0a16),
    ('b', 0x8a5a3a),
    ('l', 0xb8845a),
    ('e', 0x0c0a16),
    ('t', 0xe86a7a),
];
/// The dog, facing left: sitting, head tilted, lying low under the bench.
const DOG: [&[&str]; 3] = [
    &[
        ".kk.....", "kblk....", "kbbk.k..", ".kbbbbk.", "..kbllbk", "..kbbbbk", "..kk.kk.",
    ],
    &[
        "..kk....", ".kblk...", "kbbbk.k.", ".kbbbbk.", "..kbllbk", "..kbbbbk", "..kk.kk.",
    ],
    &[
        "........", "........", "........", "........", ".kk.....", "kbbkkkk.", "kkbbbbbk",
    ],
];

fn bench(g: &mut World, x: f64) {
    g.p(0x2a1e1a, x, 52.0, 12, 1);
    g.p(0x3a2a22, x, 51.0, 12, 1);
    g.p(0x2a1e1a, x, 48.0, 12, 1);
    g.p(0x1a1210, x + 1.0, 53.0, 1, 4);
    g.p(0x1a1210, x + 10.0, 53.0, 1, 4);
    g.p(0x1a1210, x + 1.0, 48.0, 1, 3);
    g.p(0x1a1210, x + 10.0, 48.0, 1, 3);
}

fn dog(g: &mut World, t: f64, f: u32) {
    let bx = g.panel(2) as f64 + 30.0;
    bench(g, bx);
    let hiding = (T_FINALE + 0.1..T_HEART + 0.4).contains(&t);
    if hiding {
        // Under the bench, trembling; only the tail shows a little.
        let shiver = if f.is_multiple_of(2) { 0.0 } else { 1.0 };
        g.spr(DOG[2], DOGP, bx + 2.0 + shiver, 50.0, true);
    } else if t >= T_FINALE {
        // Out again for the heart: crawls out, sits up and wags.
        let u = ((t - T_HEART - 0.5) / 0.5).clamp(0.0, 1.0);
        let x = bx + 2.0 - 11.0 * u;
        if u < 1.0 {
            g.spr(DOG[2], DOGP, x, 50.0, false);
        } else {
            g.spr(DOG[0], DOGP, x, 49.0, false);
            let wag = (f / 2) % 2;
            put(g, 0x8a5a3a, js_round(x) + 8, 52 + wag as i32);
            put(g, 0x0c0a16, js_round(x) + 8, 53 + wag as i32);
            // A little heart of its own.
            if t > T_HEART + 1.3 {
                let (hx, hy) = (js_round(x) - 1, 42 - ((t - T_HEART - 1.3) * 2.0) as i32);
                for (dx, dy) in [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1), (1, 2)] {
                    put(g, 0xff7ab8, hx + dx, hy + dy);
                }
            }
        }
    } else {
        let tilted = t >= T_DUD_LAND;
        let (x, y) = (bx - 9.0, 49.0);
        g.spr(DOG[usize::from(tilted)], DOGP, x, y, false);
        if tilted && (f / 6).is_multiple_of(2) {
            // A little "?".
            let (qx, qy) = (js_round(x) - 1, js_round(y) - 6);
            for (dx, dy) in [(0, 0), (1, -1), (2, 0), (2, 1), (1, 2), (1, 4)] {
                put(g, 0xfff0e0, qx + dx, qy + dy);
            }
        }
    }
}

/// The crowd along the rail: heads, shoulders, arms up when it's loud.
fn crowd(g: &mut World, t: f64, f: u32, bursts: &[Shell]) {
    let w = g.w();
    let cheer = if t > T_FINALE && t < T_HUSH + 0.4 {
        0.6
    } else if t > T_HEART && t < T_HEART + 1.0 {
        1.0
    } else {
        0.15
    };
    let dog_x = g.panel(2) as f64 + 30.0;
    let mut x = -2.0;
    let mut i = 0;
    while x < w as f64 + 4.0 {
        let fi = i as f64;
        let step = 4.5 + rnd(fi + 70.0) * 3.0;
        x += step;
        i += 1;
        if (x - dog_x - 4.0).abs() < 16.0 {
            continue;
        }
        let tall = rnd(fi + 71.0) * 4.0;
        let bounce = if rnd(fi + 72.0) < cheer {
            ((t * 6.0 + fi).sin()).abs() * 1.2
        } else {
            0.0
        };
        let hy = 45.0 - tall * 0.8 - bounce;
        let body = if i % 3 == 0 { 0x050410 } else { 0x08061a };
        g.p(body, x - 2.0, hy + 3.0, 6, 60 - (hy + 3.0) as i32);
        g.disc(body, x + 1.0, hy + 1.0, 2);
        // Rim light from the bursts.
        let (c, k) = light(bursts, x, hy - 10.0);
        let (c, k) = if k > 0.12 { (c, k) } else { (0x6a5a9a, 0.12) };
        for dx in 0..=2 {
            blend(g, c, js_round(x) + dx, js_round(hy) - 1, k * 1.4);
        }
        blend(g, c, js_round(x) - 1, js_round(hy), k);
        blend(g, c, js_round(x) + 3, js_round(hy), k);
        if rnd(fi + 73.0) < cheer {
            // Arms up.
            let wave = ((t * 5.0 + fi).sin() * 1.0).round();
            g.p(body, x - 3.0 + wave, hy - 4.0, 1, 7);
            g.p(body, x + 4.0 - wave, hy - 4.0, 1, 7);
        }
    }
    // A kid on a parent's shoulders, rim-lit like the rest.
    let kx = g.panel(1) as f64 + 20.0;
    let (rc, rk) = light(bursts, kx, 24.0);
    let (rc, rk) = if rk > 0.12 {
        (rc, rk)
    } else {
        (0x6a5a9a, 0.12)
    };
    let arms = if cheer > 0.3 { -3.0 } else { 0.0 };
    g.p(0x050410, kx - 3.0, 43.0, 7, 17);
    g.p(0x050410, kx - 2.0, 37.0, 5, 6);
    g.disc(0x050410, kx, 35.0, 2);
    g.p(0x050410, kx - 3.0, 37.0 + arms, 1, 4);
    g.p(0x050410, kx + 3.0, 37.0 + arms, 1, 4);
    for dx in -1..=1 {
        blend(g, rc, js_round(kx) + dx, 32, rk * 1.4);
    }
    // A phone held up, filming.
    let px = g.panel(0) as f64 + 40.0;
    g.p(0x050410, px, 37.0, 1, 7);
    g.p(0x1a1a2a, px - 1.0, 33.0, 3, 4);
    g.p(0x7ab8ff, px, 34.0, 1, 2);
    if (f / 8).is_multiple_of(2) {
        put(g, 0xff3a3a, js_round(px) + 1, 33);
    }
    glow(g, 0x7ab8ff, px, 35.0, 4.0, 0.35);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let bursts: Vec<Shell> = shells(g, t).into_iter().filter(|s| s.e() >= 0.0).collect();
    let all = shells(g, t);
    sky(g, t, &bursts);
    smoke(g, t, &bursts);
    skyline(g, f, t, &bursts);
    for s in &all {
        burst(g, s, f);
    }
    water(g, t, f, &bursts);
    barge_draw(g, t, f);
    // The late rocket climbs out of the barge, in front of the water.
    for s in all.iter().filter(|s| s.kind == Kind::Heart && s.e() < 0.0) {
        burst(g, s, f);
    }
    quay(g);
    crowd(g, t, f, &bursts);
    dog(g, t, f);
}

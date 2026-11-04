//! Lighthouse keeper: a stormy night at sea. Heavy dithered clouds race
//! over big rolling swells that throw spray off their crests, rain slants
//! in the wind and lightning forks down once over the open water. On the
//! last screen a red-and-white lighthouse stands on its rocks, waves
//! bursting against them, and its turning beam sweeps out across every
//! screen, lighting up the rain and the wave tops as it passes. A little
//! fishing boat is lost in the swell on the first screen; the beam finds
//! it and stops dead on it, then holds it in the light all the way in to
//! the landing under the tower. The storm eases, the fisher holds up the
//! catch of the night as thanks, and the keeper's cat on the gallery
//! lowers a bucket on a rope to collect its toll.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, tint, vgrad, vnoise};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

const T_BOLT: f64 = 2.3;
/// The beam stops on the boat.
const T_FIND: f64 = 4.6;
const T_SAIL: f64 = 5.4;
const T_DOCK: f64 = 9.0;
/// The keeper sets the lamp turning again.
const T_TURN: f64 = 9.3;
const T_FISH: f64 = 9.2;
const T_DOWN: f64 = 9.7;
const T_LOAD: f64 = 10.3;
const T_UP: f64 = 10.5;
const T_TOP: f64 = 11.0;

/// One turn of the lamp, in seconds.
const TURN: f64 = 2.4;
/// Mean sea level.
const SEA: f64 = 40.0;
const OUT: u32 = 0x0a0e18;
const BEAM: u32 = 0xfff4c8;

fn smooth(a: f64, b: f64, t: f64) -> f64 {
    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// `b` where the ordered dither turns `(x, y)` on at coverage `k`, else `a`.
fn dz(x: i32, y: i32, k: f64, a: u32, b: u32) -> u32 {
    if on(x, y, k) {
        b
    } else {
        a
    }
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}

/// How wild the sea and sky are: 1 in the storm, easing once the boat is in.
fn storm(t: f64) -> f64 {
    1.0 - 0.55 * smooth(T_DOCK - 0.5, SECS, t)
}

/// The lamp: (x, y).
fn lamp(g: &World) -> (f64, f64) {
    (g.panel(last(g)) as f64 + 40.0, 12.0)
}

/// The sea surface row at `x`.
fn surface(x: f64, t: f64) -> f64 {
    let a = 0.35 + 0.65 * storm(t);
    SEA - a
        * (3.0 * (x * 0.055 - t * 1.9).sin()
            + 1.6 * (x * 0.12 + t * 1.3 + 1.0).sin()
            + 0.7 * (x * 0.27 - t * 2.8 + 2.0).sin())
}

/// Where the boat lies alongside under the tower.
fn dock_x(g: &World) -> f64 {
    lamp(g).0 - 13.0
}

/// The boat's x.
fn boat_x(g: &World, t: f64) -> f64 {
    let x0 = g.panel(0) as f64 + 27.0;
    if t < T_SAIL {
        // Tossed about, getting nowhere.
        return x0 + 2.5 * (t * 0.9).sin() * (1.0 - smooth(T_FIND, T_SAIL, t));
    }
    let s0 = stage_at(g, 0, 27.0);
    let s1 = stage_at(g, last(g), dock_x(g) - g.panel(last(g)) as f64);
    let u = ((t - T_SAIL) / (T_DOCK - T_SAIL)).clamp(0.0, 1.0);
    let u = 0.45 * u + 0.55 * ease(u);
    stage_x(g, s0 + (s1 - s0) * u)
}

/// The boat's waterline y and tilt (rise per px towards the bow).
fn boat_pose(g: &World, t: f64) -> (f64, f64, f64) {
    let x = boat_x(g, t);
    let (a, b) = (surface(x - 6.0, t), surface(x + 6.0, t));
    let calm = 1.0 - 0.6 * smooth(T_DOCK - 0.4, T_DOCK + 0.6, t);
    let tilt = ((b - a) / 12.0).clamp(-0.45, 0.45) * calm;
    let y = (surface(x, t) + a + b) / 3.0;
    (x, y, tilt)
}

/// The lamp's turn angle: 0 points into the screen to the right, PI/2 at
/// the viewer, PI out to the left. `None` while it is locked on the boat.
fn turn(g: &World, t: f64) -> Option<f64> {
    let (lx, _) = lamp(g);
    let reach = lx + 14.0;
    let w = 2.0 * PI / TURN;
    if t < T_FIND {
        let x = boat_x(g, T_FIND) - 2.0;
        let at = ((x - lx) / reach).clamp(-1.0, 1.0).acos();
        Some(at - w * (T_FIND - t))
    } else if t >= T_TURN {
        let x = boat_x(g, T_TURN) - 2.0;
        let at = ((x - lx) / reach).clamp(-1.0, 1.0).acos();
        Some(at + w * 0.7 * (t - T_TURN))
    } else {
        None
    }
}

/// The beam: (far end x, far end y, strength, flare towards the viewer).
fn beam(g: &World, t: f64) -> (f64, f64, f64, f64) {
    let (lx, ly) = lamp(g);
    let reach = lx + 14.0;
    let (dir, flare) = match turn(g, t) {
        Some(a) => (a.cos(), a.sin().max(0.0).powi(6)),
        None => {
            let x = boat_x(g, t) - 2.0;
            (((x - lx) / reach).clamp(-1.0, 1.0), 0.0)
        }
    };
    let lock = if (T_FIND..T_FIND + 0.5).contains(&t) {
        0.35 * (1.0 - (t - T_FIND) / 0.5)
    } else {
        0.0
    };
    let ex = lx + dir * reach;
    let ey = if turn(g, t).is_some() {
        ly + dir.abs() * reach * 0.08
    } else {
        boat_pose(g, t).1 - 5.0
    };
    (ex, ey, 0.5 + lock - 0.2 * flare, flare)
}

/// How brightly the beam lights `(x, y)`.
fn lit(g: &World, b: (f64, f64, f64, f64), x: f64, y: f64) -> f64 {
    let (lx, ly) = lamp(g);
    let (ex, ey, k, _) = b;
    let span = ex - lx;
    if span.abs() < 1.0 {
        return 0.0;
    }
    let u = (x - lx) / span;
    if !(0.0..1.12).contains(&u) {
        return 0.0;
    }
    let cy = ly + (ey - ly) * u;
    let hw = 1.0 + (x - lx).abs() * 0.038;
    let d = ((y - cy) / hw).abs();
    if d >= 1.0 {
        return 0.0;
    }
    let tip = 1.0 - smooth(0.97, 1.12, u);
    k * (1.0 - d * d) * (1.0 - 0.2 * u) * tip
}

/// Lightning brightness this frame.
fn flash(t: f64) -> f64 {
    let d = t - T_BOLT;
    if (0.0..1.0 / 12.0).contains(&d) {
        0.7
    } else if (2.0 / 12.0..3.0 / 12.0).contains(&d) {
        0.45
    } else if (1.0 / 12.0..5.0 / 12.0).contains(&d) {
        0.12
    } else {
        0.0
    }
}

fn sky(g: &mut World, t: f64, fl: f64) {
    let w = g.w();
    let s = storm(t);
    let stops: Vec<u32> = [0x0a0d1c, 0x141a2e, 0x1e2840, 0x28344a]
        .iter()
        .map(|&c| mix(c, 0x8a96c8, fl * 0.6))
        .collect();
    vgrad(g, &stops, 0, w, 0, 32);
    // Stars in the break that opens as the storm moves off.
    let gap = smooth(T_DOCK, SECS, t);
    for i in 0..g.spread(30) {
        let fi = i as f64;
        let (x, y) = (rnd(fi + 300.0) * w as f64, rnd(fi + 301.0) * 18.0);
        let tw = 0.6 + 0.4 * (t * 3.0 + fi).sin();
        blend(g, 0xe8ecff, x as i32, y as i32, gap * tw * 0.8);
    }
    // Two decks of cloud, racing right on the wind.
    let drift = t * 9.0;
    let tones = [
        mix(0x4a5470, 0xd8e0ff, fl),
        mix(0x323a52, 0xa8b4e8, fl),
        mix(0x222838, 0x7a86b8, fl),
        mix(0x141824, 0x4a5488, fl),
    ];
    let cov = 0.25 + 0.75 * s;
    for y in 0..30 {
        let bias = 1.0 - y as f64 / 30.0;
        for x in 0..w {
            let nx = (x as f64 - drift) / 22.0;
            let n = 0.65 * vnoise(nx, y as f64 / 6.0, 3.0)
                + 0.35 * vnoise(nx * 2.4, y as f64 / 3.0, 11.0);
            let v = n + bias * 0.5 + cov * 0.62 - 1.0;
            if v <= 0.0 {
                continue;
            }
            let u = (v * 6.0).min(2.99);
            let i = u.floor() as usize;
            let c = dz(x, y, u - i as f64, tones[i], tones[i + 1]);
            put(g, c, x, y);
        }
    }
    // Ragged scud low down, faster still.
    for k in 0..g.spread(8) {
        let kf = k as f64;
        let span = w as f64 + 60.0;
        let x = (rnd(kf + 40.0) * span + t * 22.0) % span - 30.0;
        let y = 20.0 + rnd(kf + 41.0) * 7.0;
        let a = 0.55 * s;
        for i in -10..10 {
            for j in -1..2 {
                let (px, py) = (js_round(x) + i, js_round(y) + j);
                if on(px, py, a * (1.0 - (i as f64 / 10.0).abs())) {
                    put(g, tones[1], px, py);
                }
            }
        }
    }
}

/// The far coast: a dark headland with a few village lights.
fn coast(g: &mut World, t: f64) {
    let p = g.panel(1) as f64;
    for x in (p - 20.0) as i32..(p + 50.0) as i32 {
        let xf = x as f64 - p;
        let h = (6.0 - (xf - 12.0).abs() * 0.12 - (xf * 0.4).sin().abs() * 0.8).max(0.0);
        for y in (31.0 - h) as i32..32 {
            put(g, 0x121828, x, y);
        }
    }
    for (i, dx) in [4.0, 9.0, 13.0, 21.0].iter().enumerate() {
        let k = 0.5 + 0.5 * (t * 2.0 + i as f64 * 1.7).sin().abs();
        put(g, mix(0x8a6a30, 0xffd070, k), js_round(p + dx), 29);
    }
}

fn sea(g: &mut World, t: f64, b: (f64, f64, f64, f64)) {
    let w = g.w();
    let s = storm(t);
    // The horizon: a dark band of far swell.
    for x in 0..w {
        let xf = x as f64;
        let hz = 31.0 + 0.6 * (xf * 0.21 + t * 1.5).sin();
        for y in js_round(hz)..52 {
            let c = dz(x, y, ((y as f64 - hz) / 8.0).min(1.0), 0x16283a, 0x0c1a28);
            put(g, c, x, y);
        }
        put(g, 0x2a4458, x, js_round(hz));
    }
    let deep = [0x2c5a6a, 0x1e4656, 0x143444, 0x0c2434, 0x081828];
    for x in 0..w {
        let xf = x as f64;
        let sy = surface(xf, t);
        let slope = surface(xf + 1.0, t) - sy;
        let top = js_round(sy);
        for y in top..60 {
            let d = (y as f64 - sy) / 18.0 * 4.0;
            let i = (d.floor() as usize).min(3);
            let mut c = dz(x, y, d - i as f64, deep[i], deep[i + 1]);
            // Wave faces turned to the light are a touch brighter.
            if y - top < 3 && slope > 0.15 {
                c = mix(c, 0x4a7a88, 0.5);
            }
            put(g, c, x, y);
        }
        // Streaks of foam carried down the backs of the swells.
        for k in 0..3 {
            let ky = top + 3 + k * 5 + ((xf * 0.05 + k as f64 + t * 0.8).sin() * 2.0) as i32;
            if rnd(((x + js_round(t * 14.0)) / 3 * 7 + k * 13) as f64) < 0.25 * s {
                blend(g, 0x6a9aa8, x, ky, 0.5);
            }
        }
        // White caps on the high crests.
        let crest = SEA - sy;
        if crest > 3.0 * (0.35 + 0.65 * s) {
            let k = ((crest - 3.0) / 2.0).clamp(0.3, 1.0);
            put(g, mix(0x9ac0c8, 0xe8f4f8, k), x, top);
            if on(x, top + 1, k) {
                put(g, 0xc8e0e8, x, top + 1);
            }
            if slope < -0.1 && on(x, top - 1, k * 0.6) {
                put(g, 0xd8ecf0, x, top - 1);
            }
        } else {
            put(g, 0x4a7a8a, x, top);
        }
        // The beam glitters on the water it crosses.
        for y in top..(top + 4) {
            let k = lit(g, b, xf, y as f64 - 6.0);
            if k > 0.05 && rnd((x * 31 + y * 7 + js_round(t * 12.0)) as f64) < k * 0.8 {
                blend(g, BEAM, x, y, 0.8);
            }
        }
    }
}

/// Spray torn off the crests by the wind.
fn spray(g: &mut World, t: f64, b: (f64, f64, f64, f64)) {
    let w = g.w() as f64;
    let s = storm(t);
    for k in 0..(g.spread(40) as f64 * s) as u32 {
        let kf = k as f64;
        let life = 0.9;
        let ph = (t + rnd(kf + 60.0) * life) / life;
        let n = ph.floor();
        let u = ph - n;
        let x0 = rnd(kf * 3.1 + n * 17.0) * w;
        let y0 = surface(x0, t - u * life) - 1.0;
        let x = x0 + u * 16.0;
        let y = y0 - 7.0 * u + 9.0 * u * u;
        let c = mix(0xb8d0dc, BEAM, (lit(g, b, x, y) * 2.0).min(1.0));
        blend(g, c, js_round(x), js_round(y), 0.8 * (1.0 - u));
        blend(g, c, js_round(x) - 1, js_round(y), 0.4 * (1.0 - u));
    }
}

fn rain(g: &mut World, t: f64, b: (f64, f64, f64, f64), fl: f64) {
    let r = 0.35 + 0.65 * storm(t);
    let wf = g.w() as f64;
    let sl = 0.45;
    for i in 0..(g.spread(160) as f64 * r) as u32 {
        let fi = i as f64;
        let near = i % 3 == 0;
        let v = if near { 100.0 } else { 75.0 } + rnd(fi + 3.0) * 15.0;
        let y = (rnd(fi + 1.0) * 70.0 + t * v) % 70.0 - 6.0;
        let x = ((rnd(fi) * wf + y * sl + t * v * sl) % wf + wf) % wf;
        let len = if near { 4 } else { 2 };
        for s in 0..len {
            let (px, py) = (x + s as f64 * sl, y + s as f64);
            if py > surface(px, t) {
                break;
            }
            let k = lit(g, b, px, py);
            let base = if near { 0x8a9ab8 } else { 0x5a6888 };
            let c = mix(mix(base, 0xd8e0ff, fl), BEAM, (k * 2.2).min(1.0));
            let a = if near { 0.6 } else { 0.4 } + k;
            blend(g, c, js_round(px), js_round(py), a.min(1.0));
        }
    }
}

fn bolt(g: &mut World, t: f64) {
    if !(T_BOLT..T_BOLT + 3.0 / 12.0).contains(&t) {
        return;
    }
    let p = g.panel(if g.layout.panels >= 4 { 2 } else { 1 }) as f64;
    let (mut x, mut y) = (p + 20.0, 4.0);
    let mut pts = vec![(x, y)];
    for i in 1..=9 {
        x += (rnd(i as f64 + 90.0) - 0.4) * 6.0;
        y += 3.2;
        pts.push((x, y));
    }
    for (i, p) in pts.iter().enumerate() {
        glow(g, 0xb8c4ff, p.0, p.1, 9.0, 0.35);
        if i == 3 || i == 6 {
            line(g, 0x9aa8ff, p.0, p.1, p.0 + 5.0, p.1 + 4.0);
            line(g, 0x9aa8ff, p.0 + 5.0, p.1 + 4.0, p.0 + 7.0, p.1 + 9.0);
        }
    }
    for w in pts.windows(2) {
        line(g, 0xc8d0ff, w[0].0 + 1.0, w[0].1, w[1].0 + 1.0, w[1].1);
        line(g, 0xffffff, w[0].0, w[0].1, w[1].0, w[1].1);
    }
}

/// A bell buoy rocking in the swell, its green light blinking.
fn buoy(g: &mut World, t: f64) {
    let x = g.panel(1) as f64 + 40.0;
    let y = surface(x, t);
    let tilt = ((surface(x + 3.0, t) - surface(x - 3.0, t)) / 6.0).clamp(-0.5, 0.5);
    for r in 0..9 {
        let dx = -tilt * (8 - r) as f64;
        let w = [1, 1, 3, 3, 3, 3, 5, 5, 5][r as usize];
        let c = [0x6a7a8a, 0x2a3a4a, 0xc83a3a, 0xe8e0d8]
            [(r > 0) as usize + (r >= 6) as usize * (1 + r as usize % 2)];
        g.p(c, x + dx - (w / 2) as f64, y - 8.0 + r as f64, w, 1);
    }
    put(g, 0xc8a040, js_round(x - tilt * 4.0), js_round(y - 4.0));
    if (t * 1.25) % 1.0 < 0.35 {
        let (bx, by) = (x - tilt * 9.0, y - 9.0);
        glow(g, 0x6aff9a, bx, by, 6.0, 0.6);
        put(g, 0xd8ffe0, js_round(bx), js_round(by));
    }
}

fn boat(g: &mut World, t: f64, b: (f64, f64, f64, f64)) {
    let (bx, wy, tilt) = boat_pose(g, t);
    let deck = wy - 3.0;
    let (x0, y0) = (js_round(bx), deck);
    let at =
        |dx: i32, dy: i32| -> (i32, i32) { (x0 + dx, js_round(y0 + dy as f64 + tilt * dx as f64)) };
    let bp = |g: &mut World, c: u32, dx: i32, dy: i32| {
        let (x, y) = at(dx, dy);
        put(g, c, x, y);
    };
    // The hull: a sheer that rises to the bow on the right.
    for dx in -9i32..=9 {
        let rise = if dx > 4 { (dx - 4) / 2 } else { 0 } + if dx < -7 { 1 } else { 0 };
        let bot = 4 - if dx > 5 { (dx - 5) / 2 + 1 } else { 0 } - if dx < -7 { 1 } else { 0 };
        for dy in -rise..=bot {
            let c = if dy == -rise || dy == bot || dx.abs() == 9 {
                OUT
            } else if dy == -rise + 1 {
                0xe8e0d0
            } else if dy >= bot - 1 {
                0x5a1a24
            } else if dx < -2 && on(dx, dy, 0.3) {
                0xa02a30
            } else {
                0xc8383a
            };
            bp(g, c, dx, dy);
        }
    }
    bp(g, 0xf0e8d8, 5, 0);
    // The wheelhouse with its warm window, and the mast.
    for dy in -6..0 {
        for dx in -6..=-1 {
            let c = if dy == -6 {
                0x3a4a5a
            } else if dx == -6 || dx == -1 {
                OUT
            } else if (dy == -4 || dy == -3) && (dx == -3 || dx == -2) {
                0xffd070
            } else {
                0xd8d4c8
            };
            bp(g, c, dx, dy);
        }
    }
    let (wx, wyy) = at(-3, -4);
    glow(g, 0xffb050, wx as f64, wyy as f64, 4.0, 0.4);
    for dy in -12..-6 {
        bp(g, 0x4a3a2a, -4, dy);
    }
    bp(g, 0xc83a3a, -3, -12);
    bp(g, 0xc83a3a, -2, -12);
    bp(g, 0xc83a3a, -3, -11);
    let (mx, my) = at(-4, -13);
    glow(g, 0xffffff, mx as f64, my as f64, 3.0, 0.6);
    put(g, 0xffffff, mx, my);
    // The fisher in a yellow oilskin and sou'wester.
    let fisher = 2;
    for dy in -4..0 {
        bp(g, if dy == -1 { 0xb08a10 } else { 0xf0c020 }, fisher, dy);
        bp(g, 0xd8a818, fisher + 1, dy);
    }
    for (dx, dy, c) in [
        (0, -5, 0xe8b090),
        (1, -5, 0xe8b090),
        (0, -7, 0xf0c020),
        (1, -7, 0xf0c020),
    ] {
        bp(g, c, fisher + dx, dy);
    }
    for dx in fisher - 1..=fisher + 2 {
        bp(g, 0xc89810, dx, -6);
    }
    let catch = (T_FISH..T_LOAD).contains(&t);
    if (T_FIND + 0.1..T_SAIL + 0.6).contains(&t) {
        // A hand up against the glare.
        bp(g, 0xf0c020, fisher + 2, -4);
        bp(g, 0xe8b090, fisher + 2, -5);
    } else if catch {
        // Both arms up, the fish held high and wriggling.
        for (dx, dy) in [(-1, -5), (2, -5), (-1, -6), (2, -6)] {
            bp(g, 0xf0c020, fisher + dx, dy);
        }
        let (fx, fy) = at(fisher, -13);
        let wig = (t * 8.0) as i32 % 2;
        let rows = ["..k..", ".kbk.", ".kbs.", ".kbs.", "..k..", ".k.k."];
        g.spr(&rows, FISHP, (fx + wig - 2) as f64, fy as f64, false);
    } else if t >= T_LOAD {
        // Waving up at the gallery.
        let up = (t * 6.0) as i32 % 2;
        bp(g, 0xf0c020, fisher + 2, -4 - up);
        bp(g, 0xe8b090, fisher + 2 + up, -6 - up);
    } else {
        bp(g, 0xd8a818, fisher + 2, -2);
    }
    // Lit up by the beam.
    let k = lit(g, b, bx, deck - 3.0);
    if k > 0.05 {
        glow(g, BEAM, bx, deck - 4.0, 12.0, k * 0.5);
    }
}

const FISHP: &[(char, u32)] = &[('k', 0x2a3a4a), ('b', 0x7a9ab8), ('s', 0xe0f0fc)];
const HEART: &[&str] = &[".r.r.", "rrrrr", "rrrrr", ".rrr.", "..r.."];

fn rocks(g: &mut World, t: f64, fl: f64) {
    let (lx, _) = lamp(g);
    // The rock the tower stands on, with a low shelf to land on.
    for x in (lx - 16.0) as i32..(lx + 30.0) as i32 {
        let xf = x as f64 - lx;
        let shelf = if (-9.0..-3.0).contains(&xf) {
            39.5
        } else {
            60.0
        };
        let top = (35.0 + 0.045 * (xf - 6.0).powi(2) + (xf * 1.3).sin() * 0.8).min(shelf);
        if top >= 60.0 {
            continue;
        }
        for y in js_round(top)..60 {
            let d = y as f64 - top;
            let face = xf < -1.0 || (x * 7 + y * 3) % 11 == 0;
            let c = if d < 1.0 {
                mix(0x7a8090, 0xd8e0ff, fl)
            } else if face && d < 3.0 {
                0x4e5464
            } else {
                dz(x, y, 0.25 + d / 18.0, 0x30364a, 0x1c202c)
            };
            put(g, c, x, y);
        }
        // Weed and wet stone at the tide line.
        let wl = js_round(surface(x as f64, t)) + 1;
        if wl > js_round(top) {
            put(g, 0x2a4a3a, x, wl);
        }
    }
    // A mooring bollard on the shelf.
    g.p(OUT, lx - 8.0, 36.0, 3, 4);
    g.p(0x8a8a92, lx - 8.0, 36.0, 2, 1);
    g.p(0x5a5a62, lx - 8.0, 37.0, 2, 2);
    // Waves bursting on the rock's seaward face.
    for k in 0..3 {
        let per = 1.6;
        let u = ((t + k as f64 * 0.53) % per) / per;
        if u > 0.6 {
            continue;
        }
        let v = u / 0.6;
        let s = storm(t);
        for i in 0..14 {
            let fi = (i + k * 14) as f64;
            let a = -PI / 2.0 - 0.9 + rnd(fi + 70.0) * 1.2;
            let sp = (8.0 + rnd(fi + 71.0) * 10.0) * s;
            let x = lx - 9.0 + a.cos() * sp * v;
            let y = 40.0 + a.sin() * sp * v + 14.0 * v * v;
            blend(g, 0xe0f0f8, js_round(x), js_round(y), 0.9 * (1.0 - v));
            if i % 3 == 0 {
                blend(g, 0xe0f0f8, js_round(x) + 1, js_round(y), 0.6 * (1.0 - v));
            }
        }
    }
}

fn tower(g: &mut World, t: f64, f: u32, fl: f64) {
    let (lx, ly) = lamp(g);
    let (base, top) = (37.0, 19.0);
    for y in top as i32..base as i32 + 1 {
        let u = (y as f64 - top) / (base - top);
        let hw = 4.0 + u * 2.5;
        let band = ((y as f64 - top) / 5.0) as i32 % 2 == 1;
        let x0 = (lx - hw).round() as i32;
        let x1 = (lx + hw).round() as i32;
        for x in x0..=x1 {
            let k = (x - x0) as f64 / (x1 - x0) as f64;
            let (lite, dark) = if band {
                (0xd84a44, 0x7a2028)
            } else {
                (0xf0ece0, 0x8a8a98)
            };
            let c = if x == x0 || x == x1 {
                OUT
            } else {
                dz(x, y, ((k - 0.45) * 2.2).clamp(0.0, 1.0), lite, dark)
            };
            put(g, mix(c, 0xd8e0ff, fl * 0.5), x, y);
        }
    }
    // A lit window, the keeper looking out when the boat is found.
    let (wx, wy) = (js_round(lx) - 1, 24);
    g.p(0xffc860, wx as f64, wy as f64, 2, 3);
    if (T_FIND - 0.3..T_DOCK).contains(&t) {
        put(g, 0x3a2a20, wx, wy + 1);
        put(g, 0x3a2a20, wx, wy + 2);
    }
    glow(g, 0xffb050, lx, wy as f64 + 1.0, 4.0, 0.3);
    // A door at the foot.
    g.p(OUT, lx - 2.0, base - 5.0, 4, 5);
    g.p(0x4a2a1a, lx - 1.0, base - 4.0, 2, 4);
    put(g, 0xffc860, js_round(lx), js_round(base) - 2);
    // The gallery and its railing.
    g.p(0x1a1e2a, lx - 10.0, top - 1.0, 21, 2);
    g.p(0x7a8090, lx - 10.0, top - 1.0, 21, 1);
    for i in 0..11 {
        put(g, 0x6a7080, js_round(lx) - 10 + i * 2, js_round(top) - 3);
        put(g, 0x6a7080, js_round(lx) - 10 + i * 2, js_round(top) - 2);
    }
    g.p(0x8a90a0, lx - 10.0, top - 4.0, 21, 1);
    // The lamp room: glazing with its bars, the lamp, the dome and a vane.
    g.p(OUT, lx - 4.0, ly - 3.0, 9, 7);
    g.p(0x3a4458, lx - 3.0, ly - 2.0, 7, 5);
    for dx in [-3, 3] {
        g.p(0x1a1e2a, lx + dx as f64, ly - 2.0, 1, 5);
    }
    let pulse = 0.85 + 0.15 * ((f % 4) as f64 / 3.0);
    g.p(
        mix(0xffd070, 0xffffff, pulse - 0.5),
        lx - 1.0,
        ly - 1.0,
        3,
        3,
    );
    put(g, 0xffffff, js_round(lx), js_round(ly));
    for dy in 0..3 {
        g.p(
            0x2a3040,
            lx - 3.0 + dy as f64,
            ly - 4.0 - dy as f64,
            7 - 2 * dy,
            1,
        );
    }
    put(g, 0xc8a040, js_round(lx), js_round(ly) - 7);
    put(g, 0x6a7080, js_round(lx), js_round(ly) - 8);
}

fn light(g: &mut World, b: (f64, f64, f64, f64), locked: bool) {
    let (lx, ly) = lamp(g);
    let (ex, ey, k, flare) = b;
    let far = lx + (ex - lx) * 1.12;
    let x0 = lx.min(far).floor().max(0.0) as i32;
    let x1 = lx.max(far).ceil().min(g.w() as f64) as i32;
    for x in x0..x1 {
        let cy = ly + (ey - ly) * ((x as f64 - lx) / (ex - lx)).clamp(0.0, 1.12);
        let hw = 2.0 + (x as f64 - lx).abs() * 0.04;
        for y in (cy - hw) as i32..=(cy + hw) as i32 + 1 {
            let a = lit(g, b, x as f64, y as f64);
            if a > 0.0 {
                blend(g, BEAM, x, y, a * 0.75);
            }
        }
    }
    if locked && (ex - lx).abs() > 8.0 {
        // Locked on: a pool of light where it lands.
        glow(g, BEAM, ex + 3.0, ey + 2.0, 13.0, 0.55);
    }
    glow(g, 0xffe8a0, lx, ly, 22.0, 0.35 + 0.1 * k);
    glow(g, 0xfff8e0, lx, ly, 7.0, 0.6);
    if flare > 0.02 {
        // Turned full on the viewer: a flare with long spikes.
        glow(g, 0xffffff, lx, ly, 16.0, 0.9 * flare);
        for i in 1..(16.0 * flare) as i32 {
            let a = (1.0 - i as f64 / 16.0) * flare;
            for (dx, dy) in [(i, 0), (-i, 0), (0, i), (0, -i)] {
                blend(g, 0xffffff, js_round(lx) + dx, js_round(ly) + dy, a);
            }
        }
    }
}

const CATP: &[(char, u32)] = &[
    ('k', OUT),
    ('w', 0xeeeef4),
    ('s', 0xb0b0c4),
    ('g', 0x7a7a90),
    ('e', 0xb8f060),
    ('n', 0xf08aa0),
];
/// The keeper's white cat facing left: sitting, and leaning over the rail.
const CAT: [&[&str]; 2] = [
    &[
        ".k...k...",
        "kgkkkgk..",
        "kwwwwwk..",
        "kwekwek..",
        "kwwnwwk..",
        ".kwwwk...",
        ".kwwwwk..",
        "kwwwwssk.",
        "kwwwwwsk.",
        ".kkkkkkk.",
    ],
    &[
        ".........",
        ".........",
        "..k...k..",
        ".kgkkkgk.",
        ".kwwwwwkk",
        ".kwekwewk",
        ".kwwnwwsk",
        "..kwwwwsk",
        ".kwkwwwsk",
        ".kkkkkkk.",
    ],
];

/// Where the cat sits: top-left of its sprite.
fn cat_at(g: &World) -> (i32, i32) {
    let (lx, _) = lamp(g);
    (js_round(lx) - 13, 6)
}

/// The keeper's cat on the gallery, and the bucket it lowers for its toll.
fn cat(g: &mut World, t: f64, f: u32) {
    let (cx, cy) = cat_at(g);
    let lean = (T_DOWN - 0.15..T_TOP).contains(&t);
    // The tail hangs over the rail and flicks.
    let flick = (t * 3.0).sin();
    for i in 0..4 {
        put(g, 0xb0b0c4, cx + 7, cy + 10 + i);
    }
    let tip = (flick * 1.6).round().clamp(-1.0, 1.0) as i32;
    put(g, 0x7a7a90, cx + 7 + tip, cy + 14);
    g.spr(CAT[lean as usize], CATP, cx as f64, cy as f64, false);
    // Lamp light on its fur.
    for r in 2..9 {
        blend(g, 0xffe0a0, cx + 6, cy + r, 0.35);
    }
    if (T_FISH..T_FISH + 0.5).contains(&t) {
        // Ears up: fish!
        g.p(0xffe040, (cx - 2) as f64, (cy - 5) as f64, 1, 3);
        put(g, 0xffe040, cx - 2, cy - 1);
    }
    // The bucket on its rope, paid out from the cat's paws.
    let (_, wy, _) = boat_pose(g, t.min(T_UP));
    let top = (cy + 10) as f64;
    let low = wy - 16.0;
    let y = if t < T_LOAD {
        top + (low - top) * ease((t - T_DOWN) / (T_LOAD - 0.15 - T_DOWN))
    } else if t < T_UP {
        low
    } else {
        low + (top - low) * ease((t - T_UP) / (T_TOP - T_UP))
    };
    let bx = (cx + 2) as f64;
    if (T_DOWN..T_TOP).contains(&t) {
        put(g, 0xeeeef4, cx + 2, cy + 10);
        line(g, 0xc8b890, bx + 1.0, top + 1.0, bx + 1.0, y - 1.0);
        // The bucket; once loaded a tail fin sticks out of the top.
        let rows = if t >= T_LOAD {
            [".rfr.", "r.f.r", "kkkkk", "lmmmm", "lmmmm", ".nnn."]
        } else {
            [".r.r.", "r...r", "kkkkk", "lmmmm", "lmmmm", ".nnn."]
        };
        let pal = [
            ('r', 0xc8b890),
            ('f', 0x5a7a9a),
            ('k', OUT),
            ('l', 0xd8e4f0),
            ('m', 0x9aa8b8),
            ('n', 0x6a7a8a),
        ];
        g.spr(&rows, &pal, bx - 1.0, y - 2.0, false);
    }
    if t >= T_TOP {
        // Paid in full: the fish in its mouth, and a heart.
        let rows = [".kkk.k", "kbssbk", "ksbbsk", ".kkk.k"];
        g.spr(&rows, FISHP, (cx - 5) as f64, (cy + 3) as f64, false);
        let u = ((t - T_TOP) / 0.9).min(1.0);
        let hy = cy as f64 - 1.0 - u * 4.0;
        let hx = cx as f64 - 4.0 + (t * 4.0).sin();
        if f.is_multiple_of(2) || u < 0.9 {
            g.spr(HEART, &[('r', 0xff6a8a)], hx - 2.0, hy, false);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let fl = flash(t);
    let b = beam(g, t);
    sky(g, t, fl);
    bolt(g, t);
    coast(g, t);
    sea(g, t, b);
    buoy(g, t);
    tower(g, t, f, fl);
    rocks(g, t, fl);
    boat(g, t, b);
    light(g, b, turn(g, t).is_none());
    cat(g, t, f);
    spray(g, t, b);
    rain(g, t, b, fl);
    if fl > 0.0 {
        tint(g, 0xc8d0ff, fl * 0.25);
    }
}

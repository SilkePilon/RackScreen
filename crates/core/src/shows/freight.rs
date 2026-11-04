//! Dusk freight: the sun sinks behind layered hills, a farm lights its
//! window and swallows sit on sagging telegraph wires. A teal steam engine
//! thunders in with its headlamp blazing, trailing smoke, pulling a tender,
//! a boxcar, a tank wagon, a lit passenger coach and a cattle car with a
//! curious cow peeking out. At the crossing on the last screen a cow waits
//! behind the lowered barrier; the cousin in the cattle car makes her heart
//! flutter, the guard on the caboose waves his lantern and tosses her his
//! cap. It lands on her head, the barrier lifts and she turns to moo
//! proudly after the train.

use super::fx::{blend, glow, mix, put, vgrad};
use super::world::{ease, js_round, rnd, World, PANEL_W};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// Top of the rails; wheels stand on the row above.
const R: f64 = 45.0;
/// Coupler gap between cars.
const GAP: f64 = 2.0;
/// The cars from the front, with their lengths.
const CARS: [(Car, f64); 9] = [
    (Car::Loco, 36.0),
    (Car::Tender, 15.0),
    (Car::Box, 22.0),
    (Car::Logs, 22.0),
    (Car::Tank, 20.0),
    (Car::Coach, 26.0),
    (Car::Hopper, 18.0),
    (Car::Cattle, 20.0),
    (Car::Caboose, 18.0),
];

/// When the guard tosses his cap, it lands, and the cow turns to moo.
const T_TOSS: f64 = 8.6;
const T_LAND: f64 = 9.4;
const T_TURN: f64 = 9.9;
const T_MOO: f64 = 10.2;
/// The crossing lights blink and the barrier is down in between.
const T_LIGHTS: f64 = 2.8;
const T_UP: f64 = 9.5;

#[derive(Clone, Copy)]
enum Car {
    Loco,
    Tender,
    Box,
    Logs,
    Tank,
    Hopper,
    Coach,
    Cattle,
    Caboose,
}

const INK: u32 = 0x161020;
const TEAL: [u32; 4] = [0x4a9a8a, 0x3a8478, 0x2f6f6a, 0x1d4744];
const BRASS: u32 = 0xc89040;
const BRASS_HI: u32 = 0xf0c868;
const RIM: u32 = 0xe8a07a;
const LAMP: u32 = 0xffd98a;
const LAMP_HI: u32 = 0xfff4c8;

/// The cow, facing left: her body (the head is drawn on its own so it can
/// bob and lift to moo).
const COW: &[&str] = &[
    "......................",
    "......................",
    "......wwwwwwwbbbwwwww.",
    "......wwwbbwwwbbbbwwwt",
    "......wwbbbbwwwbbwwwwt",
    "......wwwbbwwwwwwwwwwt",
    "......swwwwwwwwwwwwwst",
    "......swwwwwwwwwwwwss.",
    "......sswwwwwwppwwss.t",
    "......s.s.......s.s...",
    "......s.s.......s.s...",
    "......k.k.......k.k...",
];
/// Her head; `m` is the jaw, which chews.
const COW_HEAD: &[&str] = &[
    "...h..h.", "..bwwwwb", "...wwwww", "..wewwww", "..wwwwww", ".pppwwww", ".pnppwws", ".mmmm.s.",
];
const COWP: &[(char, u32)] = &[
    ('h', 0xe8dcb8),
    ('b', 0x2a2028),
    ('w', 0xe8dce0),
    ('s', 0xa898a8),
    ('e', 0x101018),
    ('p', 0xe89a9a),
    ('m', 0xd08088),
    ('t', 0x5a4a58),
    ('k', 0x1a1020),
    ('n', 0x8a4a5a),
];

/// The cousin's head poking out of the cattle car, facing the viewer.
const COUSIN: &[&str] = &["h....h", "bwwwwb", ".wewe.", ".wwww.", ".pppp."];

/// The guard's peaked cap.
const CAP: [&[&str]; 2] = [&[".nnn.", "nnbnn", "kkkkk"], &["..nn.", ".nbnn", "kkkk."]];
const CAPP: &[(char, u32)] = &[('n', 0x2a3a6a), ('b', BRASS_HI), ('k', 0x10101a)];

/// 5×5 letters for the moo.
const MOO: [&[&str]; 2] = [
    &["k...k", "kk.kk", "k.k.k", "k...k", "k...k"],
    &[".kk.", "k..k", "k..k", "k..k", ".kk."],
];

const HEART: &[&str] = &["r.r", "rrr", ".r."];

/// World x of the train's front at `t`, and the train's length.
fn front(g: &World, t: f64) -> (f64, f64) {
    let len: f64 = CARS.iter().map(|c| c.1).sum::<f64>() + GAP * (CARS.len() - 1) as f64;
    // The caboose's rear platform passes the cow as the guard tosses his cap.
    let v = (road_x(g) + 4.0 + len + 6.0) / T_TOSS;
    (-6.0 + v * t, len)
}

/// Centre of the level crossing's road.
fn road_x(g: &World) -> f64 {
    (g.panel(3) + 30) as f64
}

/// Telegraph poles: one per screen plus one past each end.
fn pole_x(g: &World, j: i32) -> f64 {
    let step = (PANEL_W + g.layout.gap) as f64;
    12.0 + j as f64 * step
}

/// Height of wire `k` (0..3) at world x.
fn wire_y(g: &World, x: f64, k: i32) -> f64 {
    let step = (PANEL_W + g.layout.gap) as f64;
    let u = ((x - 12.0) / step).rem_euclid(1.0);
    let base = [17.0, 17.0][k as usize];
    base + 4.0 * u * (1.0 - u) * 5.0
}

fn wheel(g: &mut World, cx: f64, cy: f64, r: i32, ang: f64, body: u32, spoke: u32) {
    g.disc(INK, cx, cy, r);
    g.disc(body, cx, cy, r - 1);
    if r < 3 {
        // Small wheels: a hub and one turning glint.
        g.p(spoke, cx + ang.cos(), cy + ang.sin(), 1, 1);
        g.p(0x9a8a90, cx, cy, 1, 1);
        return;
    }
    let rf = (r - 1) as f64;
    for s in 0..2 {
        let a = ang + s as f64 * PI / 2.0;
        for d in [-rf, rf] {
            g.p(spoke, cx + a.cos() * d, cy + a.sin() * d, 1, 1);
        }
    }
    g.p(0xd8c8a0, cx, cy, 1, 1);
}

/// A two-axle bogie under a wagon.
fn bogie(g: &mut World, x: f64, ang: f64) {
    g.p(0x2a2230, x - 1.0, R - 5.0, 7, 2);
    wheel(g, x + 1.0, R - 3.0, 2, ang, 0x3a2e38, 0x6a5a64);
    wheel(g, x + 5.0, R - 3.0, 2, ang + 1.0, 0x3a2e38, 0x6a5a64);
}

/// A roof or top edge: dark with the sunset caught along its top.
fn roof(g: &mut World, x: f64, y: f64, w: i32, c: u32) {
    g.p(c, x, y, w, 2);
    g.p(RIM, x + 1.0, y, w - 2, 1);
}

/// Warm window light spilling onto the ballast under a lit window.
fn spill(g: &mut World, x: f64, w: i32, k: f64) {
    for dx in -1..=w {
        for dy in 0..3 {
            blend(
                g,
                0xffc070,
                js_round(x) + dx,
                (R as i32) + 2 + dy,
                k * (1.0 - dy as f64 * 0.3),
            );
        }
    }
}

fn loco(g: &mut World, x0: f64, ang: f64, f: u32) {
    // Firebox glow under the cab.
    let fl = 0.35 + 0.15 * ((f % 3) as f64);
    glow(g, 0xff8a3a, x0 + 8.0, R - 4.0, 5.0, fl);
    // Cab with a lit window and the driver's silhouette.
    g.p(TEAL[2], x0, R - 18.0, 12, 12);
    g.p(TEAL[1], x0, R - 18.0, 1, 12);
    g.p(TEAL[3], x0 + 11.0, R - 18.0, 1, 12);
    roof(g, x0 - 1.0, R - 20.0, 14, 0x2a1e2a);
    g.p(INK, x0 + 3.0, R - 16.0, 6, 5);
    g.p(LAMP, x0 + 4.0, R - 15.0, 4, 3);
    g.p(0x2a1a20, x0 + 5.0, R - 14.0, 2, 2);
    g.p(0x2a1a20, x0 + 4.0, R - 15.0, 3, 1);
    glow(g, 0xffb050, x0 + 6.0, R - 13.0, 5.0, 0.25);
    g.p(BRASS, x0 + 3.0, R - 9.0, 6, 1);
    g.p(BRASS_HI, x0 + 4.0, R - 9.0, 1, 1);
    // Boiler: shaded rows, brass bands, a dome and a whistle.
    let rows = [
        RIM, TEAL[0], TEAL[1], TEAL[2], TEAL[2], TEAL[2], TEAL[3], TEAL[3],
    ];
    for (i, &c) in rows.iter().enumerate() {
        g.p(c, x0 + 12.0, R - 15.0 + i as f64, 17, 1);
    }
    for bx in [17.0, 23.0] {
        g.p(BRASS, x0 + bx, R - 15.0, 1, 8);
        g.p(BRASS_HI, x0 + bx, R - 14.0, 1, 1);
    }
    g.p(BRASS, x0 + 20.0, R - 17.0, 3, 2);
    g.p(BRASS, x0 + 19.0, R - 16.0, 5, 1);
    g.p(BRASS_HI, x0 + 20.0, R - 17.0, 1, 1);
    g.p(BRASS, x0 + 14.0, R - 17.0, 1, 2);
    // Smokebox, chimney and headlamp.
    g.p(0x2a2430, x0 + 29.0, R - 15.0, 4, 8);
    g.p(0x4a4250, x0 + 29.0, R - 15.0, 1, 8);
    g.p(0x6a6070, x0 + 31.0, R - 11.0, 1, 1);
    g.p(0x2a2430, x0 + 29.0, R - 20.0, 3, 5);
    g.p(0x4a4250, x0 + 29.0, R - 20.0, 1, 5);
    g.p(0x2a2430, x0 + 28.0, R - 21.0, 5, 1);
    g.p(RIM, x0 + 28.0, R - 21.0, 3, 1);
    g.p(INK, x0 + 32.0, R - 18.0, 3, 3);
    g.p(LAMP_HI, x0 + 34.0, R - 17.0, 1, 1);
    // Running board with a red valance, buffer beam and cowcatcher.
    g.p(INK, x0 + 10.0, R - 7.0, 25, 1);
    g.p(0xa83a30, x0 + 12.0, R - 6.0, 21, 1);
    g.p(0xb03a30, x0 + 33.0, R - 7.0, 3, 3);
    for j in 0..4 {
        let jf = j as f64;
        g.p(0x5a4a58, x0 + 33.0, R - 4.0 + jf, 2 + j, 1);
        g.p(0x8a7a80, x0 + 34.0 + jf, R - 4.0 + jf, 1, 1);
    }
    // Cylinder.
    g.p(0x3a3040, x0 + 26.0, R - 6.0, 6, 3);
    g.p(0x6a6070, x0 + 26.0, R - 6.0, 6, 1);
    // Wheels: trailing, three drivers with the rods, a pony truck.
    wheel(g, x0 + 5.0, R - 3.0, 2, ang * 1.5, 0x8a2a28, 0xc85a4a);
    wheel(g, x0 + 31.0, R - 3.0, 2, ang * 1.5, 0x8a2a28, 0xc85a4a);
    let mut pins = [(0.0, 0.0); 3];
    for (i, dx) in [13.0, 19.0, 25.0].into_iter().enumerate() {
        wheel(g, x0 + dx, R - 4.0, 3, ang, 0x8a2a28, 0xc85a4a);
        pins[i] = (x0 + dx + ang.cos() * 2.0, R - 4.0 + ang.sin() * 2.0);
    }
    let rod = 0xd0d0dc;
    g.p(
        rod,
        pins[0].0,
        pins[0].1,
        (pins[2].0 - pins[0].0) as i32 + 1,
        1,
    );
    let (cx, cy) = (x0 + 30.0, R - 5.0);
    let n = (cx - pins[2].0).abs().ceil().max(1.0) as i32;
    for i in 0..=n {
        let u = i as f64 / n as f64;
        g.p(
            0xa0a0b0,
            pins[2].0 + (cx - pins[2].0) * u,
            pins[2].1 + (cy - pins[2].1) * u,
            1,
            1,
        );
    }
}

fn tender(g: &mut World, x0: f64, ang: f64) {
    g.p(TEAL[2], x0, R - 15.0, 15, 9);
    g.p(TEAL[1], x0, R - 15.0, 15, 1);
    g.p(TEAL[3], x0, R - 8.0, 15, 2);
    g.p(BRASS, x0 + 1.0, R - 12.0, 13, 1);
    // Coal heap with a few glints.
    for i in 0..13 {
        let h = 1 + js_round(2.0 * (i as f64 / 12.0 * PI).sin());
        g.p(0x1a161e, x0 + 1.0 + i as f64, R - 15.0 - h as f64, 1, h);
        if i % 4 == 1 {
            g.p(0x6a6478, x0 + 1.0 + i as f64, R - 15.0 - h as f64, 1, 1);
        }
    }
    g.p(INK, x0, R - 6.0, 15, 1);
    wheel(g, x0 + 3.0, R - 3.0, 2, ang * 1.5, 0x8a2a28, 0xc85a4a);
    wheel(
        g,
        x0 + 11.0,
        R - 3.0,
        2,
        ang * 1.5 + 1.0,
        0x8a2a28,
        0xc85a4a,
    );
}

fn boxcar(g: &mut World, x0: f64, ang: f64, t: f64) {
    g.p(0x8a3a30, x0, R - 17.0, 22, 11);
    let mut x = 2.0;
    while x < 22.0 {
        g.p(0x6e2e28, x0 + x, R - 16.0, 1, 10);
        x += 3.0;
    }
    g.p(0xa04a3a, x0, R - 16.0, 22, 1);
    roof(g, x0 - 1.0, R - 19.0, 24, 0x4a2a2a);
    // Sliding door left ajar: lamplight inside.
    g.p(0x5a2420, x0 + 8.0, R - 16.0, 7, 10);
    g.p(0x7a3a30, x0 + 9.0, R - 15.0, 5, 8);
    g.p(0xffb860, x0 + 14.0, R - 15.0, 1, 8);
    let k = 0.3 + 0.05 * (t * 9.0).sin();
    glow(g, 0xffa050, x0 + 14.5, R - 11.0, 4.0, k);
    spill(g, x0 + 14.0, 1, 0.25);
    // A painted diamond and a white number.
    g.p(0xe8d8c0, x0 + 3.0, R - 12.0, 3, 1);
    g.p(0xe8d8c0, x0 + 4.0, R - 13.0, 1, 3);
    g.p(0xe8d8c0, x0 + 17.0, R - 9.0, 3, 1);
    g.p(INK, x0, R - 6.0, 22, 1);
    bogie(g, x0 + 2.0, ang);
    bogie(g, x0 + 14.0, ang);
}

fn logs(g: &mut World, x0: f64, ang: f64) {
    g.p(0x5a3a2a, x0, R - 8.0, 22, 2);
    g.p(0x8a5a3a, x0, R - 8.0, 22, 1);
    // Two rows of logs, their cut ends showing rings.
    for row in 0..2 {
        let ly = R - 11.0 - row as f64 * 3.0;
        let lx = x0 + 1.0 + row as f64 * 1.5;
        g.p(0x6a4430, lx, ly, 20 - row * 3, 3);
        g.p(0x9a6a40, lx, ly, 20 - row * 3, 1);
        g.p(0x3a2418, lx, ly + 2.0, 20 - row * 3, 1);
        let ex = lx + (19 - row * 3) as f64;
        g.p(0xe8c088, ex, ly, 1, 3);
        put(g, 0xa87848, js_round(ex), js_round(ly + 1.0));
    }
    for sx in [0.0, 10.0, 21.0] {
        g.p(0x3a2418, x0 + sx, R - 17.0, 1, 9);
    }
    // A chain over the load.
    for i in 0..20 {
        if i % 2 == 0 {
            put(
                g,
                0x9a9aa8,
                js_round(x0 + 1.0 + i as f64),
                (R - 15.0) as i32 + (i % 4 == 0) as i32,
            );
        }
    }
    g.p(INK, x0, R - 6.0, 22, 1);
    bogie(g, x0 + 2.0, ang);
    bogie(g, x0 + 14.0, ang);
}

fn hopper(g: &mut World, x0: f64, ang: f64) {
    // Open-top coal hopper with sloped chutes.
    for r in 0..11 {
        let rf = r as f64;
        let inset = if r > 6 { (r - 6) as f64 * 1.5 } else { 0.0 };
        let c = if r == 0 {
            RIM
        } else if r < 4 {
            0x5a5266
        } else {
            0x48405a
        };
        g.p(c, x0 + inset, R - 17.0 + rf, (18.0 - 2.0 * inset) as i32, 1);
    }
    for rx in [4.0, 9.0, 14.0] {
        g.p(0x34304a, x0 + rx, R - 16.0, 1, 7);
    }
    for i in 0..16 {
        let h = js_round(1.5 * (i as f64 / 15.0 * PI).sin());
        g.p(0x1a161e, x0 + 1.0 + i as f64, R - 17.0 - h as f64, 1, h);
    }
    g.p(0xe8d8c0, x0 + 6.0, R - 13.0, 6, 1);
    g.p(INK, x0, R - 6.0, 18, 1);
    bogie(g, x0 + 1.0, ang);
    bogie(g, x0 + 11.0, ang);
}

fn tank(g: &mut World, x0: f64, ang: f64) {
    g.p(INK, x0, R - 6.0, 20, 1);
    g.p(0x3a3040, x0, R - 7.0, 20, 1);
    let rows = [
        RIM, 0xc8c4d0, 0xa8a8bc, 0x9898ac, 0x8888a0, 0x787890, 0x62627a, 0x4a4a62,
    ];
    for (i, &c) in rows.iter().enumerate() {
        let inset = if i == 0 || i == rows.len() - 1 {
            2.0
        } else {
            1.0
        };
        g.p(
            c,
            x0 + inset,
            R - 15.0 + i as f64,
            (20.0 - 2.0 * inset) as i32,
            1,
        );
    }
    g.p(0xd86a3a, x0 + 1.0, R - 12.0, 18, 1);
    g.p(0x6a6a84, x0 + 9.0, R - 17.0, 3, 2);
    g.p(RIM, x0 + 9.0, R - 17.0, 3, 1);
    for y in 0..7 {
        if y % 2 == 0 {
            g.p(0x3a3a50, x0 + 3.0, R - 14.0 + y as f64, 2, 1);
        }
    }
    bogie(g, x0 + 1.0, ang);
    bogie(g, x0 + 13.0, ang);
}

fn coach(g: &mut World, x0: f64, ang: f64, f: u32) {
    g.p(0x6a2a3a, x0, R - 17.0, 26, 11);
    g.p(0x4a1e2a, x0, R - 8.0, 26, 2);
    g.p(0xe0d0a8, x0, R - 16.0, 26, 6);
    roof(g, x0 - 1.0, R - 19.0, 28, 0x3a2a38);
    g.p(0x3a2a38, x0 + 4.0, R - 20.0, 18, 1);
    // Five lit windows with passengers.
    for i in 0..5 {
        let wx = x0 + 2.0 + i as f64 * 5.0;
        g.p(LAMP, wx, R - 15.0, 3, 4);
        g.p(LAMP_HI, wx, R - 15.0, 3, 1);
        match i {
            1 => {
                // Reading the paper.
                g.p(0x5a3a2a, wx, R - 13.0, 2, 2);
                g.p(0xe8e8e0, wx + 1.0, R - 13.0, 2, 2);
            }
            2 => {
                // A child waving.
                g.p(0x5a3a2a, wx + 1.0, R - 13.0, 2, 2);
                let up = (f / 2).is_multiple_of(2);
                g.p(0x5a3a2a, wx, R - if up { 15.0 } else { 14.0 }, 1, 2);
            }
            4 => g.p(0x5a3a2a, wx + 1.0, R - 13.0, 2, 2),
            _ => {}
        }
        spill(g, wx, 3, 0.18);
    }
    glow(g, 0xffc070, x0 + 13.0, R - 13.0, 14.0, 0.18);
    g.p(0x4a1e2a, x0, R - 16.0, 1, 10);
    g.p(0x4a1e2a, x0 + 25.0, R - 16.0, 1, 10);
    g.p(INK, x0, R - 6.0, 26, 1);
    bogie(g, x0 + 2.0, ang);
    bogie(g, x0 + 18.0, ang);
}

fn cattle(g: &mut World, x0: f64, ang: f64, f: u32) {
    g.p(0x2a1a1a, x0, R - 17.0, 20, 11);
    for y in 0..5 {
        g.p(0x9a6a3a, x0, R - 16.0 + y as f64 * 2.0, 20, 1);
    }
    for x in [0.0, 6.0, 13.0, 19.0] {
        g.p(0x6a4428, x0 + x, R - 17.0, 1, 11);
    }
    roof(g, x0 - 1.0, R - 19.0, 22, 0x4a3428);
    // The cousin pokes her head out between the slats.
    let pal = [
        ('h', 0xe8dcb8),
        ('b', 0x2a2028),
        ('w', 0xe8dce0),
        ('e', 0x101018),
        ('p', 0xe89a9a),
    ];
    let bob = if (f / 3).is_multiple_of(2) { 0.0 } else { 1.0 };
    g.spr(COUSIN, &pal, x0 + 7.0, R - 18.0 + bob, false);
    g.p(INK, x0, R - 6.0, 20, 1);
    bogie(g, x0 + 1.0, ang);
    bogie(g, x0 + 13.0, ang);
}

fn caboose(g: &mut World, x0: f64, ang: f64, f: u32, t: f64) {
    g.p(0xb03a30, x0, R - 15.0, 18, 9);
    g.p(0xc85a44, x0, R - 15.0, 18, 1);
    g.p(0x7a2a24, x0, R - 8.0, 18, 2);
    roof(g, x0 - 1.0, R - 17.0, 20, 0x3a2028);
    // Cupola with a lit lookout window.
    g.p(0xb03a30, x0 + 6.0, R - 21.0, 7, 4);
    roof(g, x0 + 5.0, R - 23.0, 9, 0x3a2028);
    g.p(LAMP, x0 + 8.0, R - 20.0, 3, 2);
    for wx in [3.0, 12.0] {
        g.p(LAMP, x0 + wx, R - 13.0, 3, 3);
        spill(g, x0 + wx, 3, 0.2);
    }
    // A stovepipe.
    g.p(0x2a2028, x0 + 15.0, R - 20.0, 1, 3);
    // Tail lamp, blinking red.
    if (f / 3).is_multiple_of(2) {
        g.p(0xff3a2a, x0 - 1.0, R - 14.0, 1, 2);
        glow(g, 0xff3a2a, x0 - 1.0, R - 13.0, 4.0, 0.5);
    } else {
        g.p(0x7a1a1a, x0 - 1.0, R - 14.0, 1, 2);
    }
    // Rear platform with a railing.
    g.p(INK, x0 - 5.0, R - 6.0, 23, 1);
    g.p(0x5a4a58, x0 - 5.0, R - 10.0, 1, 4);
    g.p(0x5a4a58, x0 - 5.0, R - 10.0, 5, 1);
    // The guard: waves his lantern, then tosses his cap.
    let gx = x0 - 3.0;
    g.p(0x2a3a6a, gx, R - 13.0, 2, 4);
    g.p(0x1a2040, gx, R - 9.0, 2, 3);
    g.p(0xf0c8a0, gx, R - 15.0, 2, 2);
    if t < T_TOSS {
        g.spr(CAP[0], CAPP, gx - 1.5, R - 17.5, false);
    } else {
        g.p(0x8a6a5a, gx, R - 16.0, 2, 1);
    }
    let swing = (t * 7.0).sin();
    let (lx, ly) = (gx - 2.0 + swing * 1.5, R - 16.0 + swing.abs() * 1.0);
    g.p(0xf0c8a0, gx - 1.0, R - 14.0, 1, 1);
    g.p(0x3a3040, lx, ly - 1.0, 1, 1);
    g.p(0xffd060, lx, ly, 1, 2);
    glow(g, 0xffb040, lx, ly + 1.0, 5.0, 0.55);
    g.p(INK, x0, R - 6.0, 18, 1);
    bogie(g, x0 + 1.0, ang);
    bogie(g, x0 + 11.0, ang);
}

/// A soft smoke puff; `age` 0..1. Lit warm from the sunset below.
fn puff(g: &mut World, x: f64, y: f64, age: f64, seed: f64) {
    let r = 1.5 + age * 4.0 + rnd(seed) * 1.2;
    let base = mix(0x9a7a96, 0x4e3a58, age);
    let lit = mix(0xf0c8c0, 0x8a6478, age);
    let shade = mix(0x7a6080, 0x3e2e4a, age);
    let ri = r.ceil() as i32;
    let fade = if age < 0.55 {
        1.0
    } else {
        1.0 - (age - 0.55) / 0.45
    };
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if d > r {
                continue;
            }
            let (px, py) = (js_round(x) + dx, js_round(y) + dy);
            // Light from the upper left, shadow at the lower right.
            let l = (-(dx as f64) - dy as f64) / r;
            let c = if l > 0.5 {
                lit
            } else if l < -0.6 {
                shade
            } else {
                base
            };
            if fade >= 1.0 {
                put(g, c, px, py);
            } else {
                blend(g, c, px, py, fade * 0.9);
            }
        }
    }
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    let dusk = ease(t / SECS) * 0.45;
    let day = [0x1e1a40, 0x3a2460, 0x6e2e6a, 0xb04a62, 0xe8784e, 0xf8b068];
    let night = [0x0e0c24, 0x1e1a40, 0x3a2460, 0x7a3a66, 0xc8604e, 0xf0985a];
    let stops: Vec<u32> = day
        .iter()
        .zip(night.iter())
        .map(|(&a, &b)| mix(a, b, dusk))
        .collect();
    vgrad(g, &stops, 0, w, 0, 38);
    // The first stars as it darkens.
    for k in 0..g.spread(26) {
        let kf = k as f64;
        let (sx, sy) = (rnd(kf + 1.0) * w as f64, rnd(kf + 2.0) * 13.0);
        let show = (t - 2.0 - rnd(kf + 3.0) * 6.0).clamp(0.0, 1.0);
        let tw = 0.5 + 0.5 * (t * 3.0 + kf * 1.7).sin();
        if show > 0.0 {
            blend(
                g,
                0xfff4e0,
                js_round(sx),
                js_round(sy),
                show * (0.4 + 0.5 * tw),
            );
        }
    }
}

fn sun(g: &mut World, t: f64) -> f64 {
    let sx = (g.panel(1) + 34) as f64;
    let sy = 25.0 + ease(t / SECS) * 7.0;
    glow(g, 0xffa060, sx, sy, 34.0, 0.45);
    glow(g, 0xffd890, sx, sy, 12.0, 0.5);
    g.disc(0xffb468, sx, sy, 6);
    g.disc(0xffd890, sx, sy, 5);
    g.disc(0xfff0c0, sx - 1.0, sy - 1.0, 2);
    // A thin cloud bar across its face.
    g.p(0xe0705a, sx - 9.0, sy + 1.0, 14, 1);
    g.p(0xf0906a, sx - 7.0, sy + 2.0, 9, 1);
    sx
}

/// A long sunset cloud: dusky top, pink belly, a gold-lit underside.
fn cloud(g: &mut World, x: f64, y: f64, len: i32) {
    let lf = len as f64;
    g.disc(0x6a3468, x + lf * 0.3, y, 2);
    g.disc(0x6a3468, x + lf * 0.55, y - 1.0, 3);
    g.p(0x6a3468, x, y + 1.0, len, 2);
    g.disc(0x9a4470, x + lf * 0.55, y, 2);
    g.p(0x9a4470, x + 2.0, y + 2.0, len - 3, 1);
    g.p(0xe8806a, x + 3.0, y + 3.0, len - 6, 1);
    g.dith(0xf8b878, js_round(x) + 5, js_round(y) + 3, len - 10, 1, 0);
}

fn clouds(g: &mut World, t: f64) {
    let span = g.w() as f64 + 60.0;
    for k in 0..g.spread(4) {
        let kf = k as f64;
        let x = (rnd(kf + 20.0) * span + t * (1.0 + kf % 3.0 * 0.6)) % span - 40.0;
        let y = 5.0 + rnd(kf + 21.0) * 11.0;
        let len = 14 + (rnd(kf + 22.0) * 14.0) as i32;
        cloud(g, x, y, len);
    }
}

fn hills(g: &mut World, sun_x: f64, t: f64) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let h = 5.0 + 3.0 * (xf * 0.045 + 0.4).sin() + 2.0 * (xf * 0.11).sin();
        let near = (1.0 - (xf - sun_x).abs() / 70.0).max(0.0);
        let c = mix(0x5a3466, 0x9a4a6a, near * 0.8);
        let top = js_round(35.0 - h);
        g.p(c, xf, top as f64, 1, 40 - top);
        if near > 0.2 && (x + top) % 3 == 0 {
            put(g, mix(c, 0xf8a070, near * 0.7), x, top);
        }
    }
    // Nearer rolling hills with tree clumps.
    for x in 0..w {
        let xf = x as f64;
        let h = 2.0 + 1.6 * (xf * 0.07 + 2.0).sin() + (xf * 0.19).sin();
        let top = js_round(38.0 - h);
        g.p(0x40284e, xf, top as f64, 1, 44 - top);
        put(g, 0x6a3e5e, x, top);
    }
    for k in 0..g.spread(14) {
        let kf = k as f64;
        let tx = rnd(kf + 40.0) * w as f64;
        let ty = 35.0 + rnd(kf + 41.0) * 2.0;
        g.disc(0x2e1e3c, tx, ty, 2);
        g.p(0x5a3a5a, tx - 1.0, ty - 2.0, 2, 1);
        g.p(0x2e1e3c, tx, ty + 2.0, 1, 2);
    }
    // A farm on the first screen: barn, silo and a window that lights up.
    let fx = (g.panel(0) + 34) as f64;
    g.p(0x4a2438, fx, 30.0, 10, 7);
    for r in 0..4 {
        g.p(
            0x30182a,
            fx - 1.0 + r as f64,
            26.0 + r as f64,
            12 - 2 * r,
            1,
        );
    }
    g.p(RIM, fx + 1.0, 27.0, 1, 1);
    g.p(0x5a3a50, fx + 11.0, 25.0, 4, 12);
    g.disc(0x5a3a50, fx + 12.5, 25.0, 2);
    g.p(0x8a5a6a, fx + 11.0, 24.0, 1, 12);
    let lit = t > 3.2 || (t > 2.8 && (t * 12.0) as i32 % 2 == 0);
    if lit {
        g.p(LAMP, fx + 3.0, 32.0, 2, 2);
        glow(g, 0xffb050, fx + 4.0, 33.0, 5.0, 0.35);
    } else {
        g.p(0x2a1420, fx + 3.0, 32.0, 2, 2);
    }
    // Fields: furrows catching the last light, hedgerows between.
    vgrad(g, &[0x4a2e50, 0x6a4058, 0x5a3450], 0, w, 38, 44);
    for x in 0..w {
        for y in 38..44 {
            if (x * 2 + y * 5) % 9 == 0 {
                put(g, 0x8a5a5a, x, y);
            }
        }
    }
    for k in 0..g.spread(7) {
        let hx = 20.0 + k as f64 * 54.0 + rnd(k as f64 + 50.0) * 20.0;
        g.p(0x2e1e3c, hx, 38.0, 1, 6);
    }
    // A hay bale catching the sun on screen 1.
    let bx = (g.panel(1) + 12) as f64;
    g.disc(0x8a5a3a, bx, 41.0, 2);
    g.p(0xe8a060, bx - 1.0, 39.0, 2, 1);
    g.p(0x5a3428, bx, 41.0, 1, 1);
}

fn wires(g: &mut World, front_x: f64, f: u32) {
    let n = g.layout.panels as i32;
    for j in 0..n {
        let px = pole_x(g, j);
        g.p(0x24142a, px, 16.0, 1, 29);
        g.p(0x4a2a48, px + 1.0, 18.0, 1, 27);
        g.p(0x24142a, px - 4.0, 18.0, 9, 1);
        g.p(0x24142a, px - 1.0, 19.0, 1, 2);
        for ix in [-4.0, 4.0] {
            g.p(0xd8b0a0, px + ix, 17.0, 1, 1);
        }
    }
    for x in 0..g.w() {
        let xf = x as f64;
        for (k, dx) in [(0, -4.0), (1, 4.0)] {
            let y = wire_y(g, xf - dx, k);
            put(g, 0x24142a, x, js_round(y));
        }
    }
    // Swallows on the wire on screen 2 take off as the engine nears.
    let base = (g.panel(2) + 16) as f64;
    for b in 0..4 {
        let bf = b as f64;
        let bx = base + bf * 7.0 + rnd(bf + 60.0) * 3.0;
        let lift = ((front_x - (bx - 30.0)) / 26.0 - bf * 0.2).max(0.0);
        let wy = (wire_y(g, bx + 4.0, 0)).round() - 1.0;
        if lift <= 0.0 {
            // Perched: dark back, rusty throat, forked tail.
            g.p(0x1a1430, bx, wy - 2.0, 2, 2);
            g.p(0x1a1430, bx + 2.0, wy - 1.0, 1, 1);
            g.p(0xe0704a, bx - 1.0, wy - 1.0, 1, 1);
            put(g, 0xe8d8d0, js_round(bx), js_round(wy));
        } else {
            let (x, y) = (bx + lift * 22.0, wy - 1.0 - lift * lift * 5.0 - lift * 6.0);
            let up = (f + b) % 4 < 2;
            let wing = if up { -1.0 } else { 1.0 };
            g.p(0x1a1430, x, y, 2, 1);
            g.p(0x1a1430, x - 2.0, y + wing, 2, 1);
            g.p(0x1a1430, x + 2.0, y + wing, 2, 1);
            g.p(0xe0704a, x + 2.0, y, 1, 1);
        }
    }
}

fn track(g: &mut World) {
    let w = g.w();
    vgrad(g, &[0x3a2a3a, 0x2e2030], 0, w, 44, 50);
    for x in 0..w {
        for y in 46..50 {
            if rnd((x * 61 + y * 7) as f64) > 0.78 {
                put(g, 0x5a4452, x, y);
            }
        }
        if x % 4 == 0 {
            g.p(0x22161e, x as f64, 46.0, 3, 1);
        }
    }
    g.p(0x7a6a78, 0.0, R, w, 1);
    for x in 0..w {
        if x % 2 == 0 {
            put(g, RIM, x, R as i32);
        }
    }
    g.p(0x1e1420, 0.0, R + 1.0, w, 1);
}

fn foreground(g: &mut World) {
    let w = g.w();
    vgrad(g, &[0x2a1a30, 0x1e1226, 0x140c1a], 0, w, 50, 60);
    for k in 0..g.spread(60) {
        let kf = k as f64;
        let x = js_round(rnd(kf + 80.0) * w as f64);
        let y = 51 + (rnd(kf + 81.0) * 8.0) as i32;
        let h = 1 + (rnd(kf + 82.0) * 3.0) as i32;
        g.p(0x3a2a40, x as f64, (y - h) as f64, 1, h);
        put(g, 0x140c1a, x + 1, y - 1);
    }
}

fn crossing(g: &mut World) {
    let rx = road_x(g);
    // The lane winding off to the farm behind the track.
    for y in 38..44 {
        let half = 1.0 + (y - 38) as f64 * 0.35;
        g.p(0x6a4a58, rx - half, y as f64, js_round(half * 2.0), 1);
    }
}

fn signal(g: &mut World, t: f64, f: u32) {
    let rx = road_x(g);
    let sx = rx + 11.0;
    g.p(0x3a3040, sx, 28.0, 1, 22);
    g.p(0x6a6070, sx, 28.0, 1, 1);
    // Crossbuck.
    for i in 0..6 {
        let i = i as f64;
        g.p(0xf0e8e0, sx - 3.0 + i, 25.0 + i * 0.8, 1, 1);
        g.p(0xf0e8e0, sx + 2.0 - i, 25.0 + i * 0.8, 1, 1);
    }
    // Two red lamps that wig-wag while the barrier is down.
    let active = (T_LIGHTS..T_UP).contains(&t);
    g.p(INK, sx - 3.0, 33.0, 7, 1);
    for (i, lx) in [sx - 2.0, sx + 2.0].into_iter().enumerate() {
        let on = active && ((f / 4) as usize + i).is_multiple_of(2);
        g.p(INK, lx - 1.0, 34.0, 3, 3);
        g.p(if on { 0xff4a3a } else { 0x5a2028 }, lx, 35.0, 1, 1);
        if on {
            glow(g, 0xff3a2a, lx, 35.0, 7.0, 0.6);
        }
    }
    // The barrier: lowers before the train, rises after the caboose.
    let down = ease((t - T_LIGHTS - 0.4) / 0.8) * (1.0 - ease((t - T_UP) / 0.7));
    let a = -PI / 2.0 + down * (PI / 2.0 + 0.04);
    let (px, py) = (sx - 1.0, 41.0);
    g.p(0x2a2230, px - 1.0, py - 1.0, 3, 3);
    for i in 0..17 {
        let r = i as f64;
        let c = if (i / 3) % 2 == 0 { 0xe83a3a } else { 0xf0e8e0 };
        g.p(c, px - a.cos() * r, py + a.sin() * r, 1, 1);
    }
    if active && down > 0.9 && (f / 4).is_multiple_of(2) {
        g.p(
            0xff4a3a,
            px - a.cos() * 16.0,
            py + a.sin() * 16.0 - 1.0,
            1,
            1,
        );
    }
}

fn cow(g: &mut World, t: f64, f: u32, cousin: f64) {
    let rx = road_x(g);
    let (cx, cy) = (rx - 11.0, 44.0);
    let turned = t > T_TURN;
    let mut pal: Vec<(char, u32)> = COWP.to_vec();
    let mooing = t > T_MOO && t < T_MOO + 1.2;
    // Chewing: the jaw drops every few frames; open wide to moo.
    if mooing {
        pal[6] = ('m', 0x3a1a20);
    } else if (f / 2).is_multiple_of(3) {
        pal[6] = ('m', 0xe89a9a);
    }
    // Tail swish.
    let swish = (f / 5) % 2 == 1;
    let rows: Vec<String> = COW
        .iter()
        .map(|r| {
            if swish {
                r.replace('t', ".")
            } else {
                (*r).to_string()
            }
        })
        .collect();
    let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
    g.spr(&refs, &pal, cx, cy, turned);
    if swish {
        let tx = if turned { cx - 1.0 } else { cx + 22.0 };
        g.p(0x5a4a58, tx, cy + 3.0, 1, 4);
        g.p(0x2a2028, tx, cy + 7.0, 1, 1);
    }
    let lift = if mooing { -1.0 } else { 0.0 };
    let head_x = if turned { cx + 14.0 } else { cx };
    g.spr(COW_HEAD, &pal, head_x, cy + lift, turned);
    if mooing {
        // Breath steaming in the cool evening air.
        let u = ((t - T_MOO) / 1.2).fract();
        let (bx, by) = if turned {
            (cx + 23.0 + u * 5.0, cy + 6.0 - u * 3.0)
        } else {
            (cx - 2.0 - u * 5.0, cy + 6.0 - u * 3.0)
        };
        glow(g, 0xf0e8f0, bx, by, 1.5 + u * 2.0, 0.5 * (1.0 - u));
    }
    // Head position, for the hearts and the cap.
    let hx = if turned { cx + 18.0 } else { cx + 4.0 };
    if cousin > 0.0 {
        for k in 0..3 {
            let u = (cousin - k as f64 * 0.25).clamp(0.0, 1.0);
            if u > 0.0 && u < 1.0 {
                let pal = [('r', mix(0xff5a8a, 0x8a3a6a, u))];
                g.spr(
                    HEART,
                    &pal,
                    hx - 1.0 + (u * 6.0 + k as f64).sin() * 2.0,
                    cy - 4.0 - u * 12.0,
                    false,
                );
            }
        }
    }
    if t >= T_LAND {
        g.spr(CAP[0], CAPP, hx - 2.0, cy - 1.0 + lift, false);
    }
    if t > T_MOO {
        // The bubble's tail points down at her raised head.
        let (bx, by) = (rx - 6.0, 30.0);
        g.p(INK, bx, by, 19, 9);
        g.p(0xfff8f0, bx + 1.0, by + 1.0, 17, 7);
        g.p(INK, bx + 10.0, by + 9.0, 4, 1);
        g.p(0xfff8f0, bx + 11.0, by + 8.0, 2, 2);
        g.p(INK, bx + 12.0, by + 10.0, 2, 1);
        g.p(0xfff8f0, bx + 12.0, by + 9.0, 1, 1);
        let pal = [('k', 0x3a1a2a)];
        let mut lx = bx + 2.0;
        for (i, l) in [0usize, 1, 1].into_iter().enumerate() {
            if t > T_MOO + i as f64 * 0.15 {
                g.spr(MOO[l], &pal, lx, by + 2.0, false);
            }
            lx += if l == 0 { 6.0 } else { 5.0 };
        }
    }
}

/// When the point `off` px behind the front passes world x `x`.
fn time_at(g: &World, off: f64, x: f64) -> f64 {
    let (f1, _) = front(g, 1.0);
    let (f0, _) = front(g, 0.0);
    (x + off - f0) / (f1 - f0)
}

/// Smoke from the chimney (it hangs where it was puffed and drifts back),
/// steam chuffs from the cylinders and the whistle before the crossing.
fn smoke(g: &mut World, t: f64) {
    let w = g.w() as f64;
    const LIFE: f64 = 2.6;
    let k1 = (t * 12.0).floor() as i32;
    let k0 = ((t - LIFE) * 12.0).ceil() as i32;
    for k in (k0..=k1).rev() {
        let tb = k as f64 / 12.0;
        let (fb, _) = front(g, tb);
        let cx = fb - 36.0 + 30.0;
        if tb < 0.0 || cx < -10.0 || cx > w + 10.0 {
            continue;
        }
        let a = t - tb;
        let u = a / LIFE;
        let kf = k as f64;
        let wob = (a * 2.5 + kf).sin() * 1.5;
        let x = cx - a * (6.0 + rnd(kf + 7.0) * 2.0) + wob;
        let y = R - 23.0 - (10.0 + rnd(kf + 9.0) * 3.0) * a.powf(0.7);
        puff(g, x, y, u, k as f64);
    }
    // Chuffs at the cylinders, four a turn.
    let k1 = (t * 6.0).floor() as i32;
    for k in k1 - 3..=k1 {
        let tb = k as f64 / 6.0;
        if tb < 0.0 {
            continue;
        }
        let (fb, _) = front(g, tb);
        let a = t - tb;
        let u = a / 0.6;
        if u >= 1.0 {
            continue;
        }
        let (x, y) = (
            fb - 3.0 + if k % 2 == 0 { 0.0 } else { -6.0 },
            R - 3.0 - a * 6.0,
        );
        let r = 1.0 + u * 2.5;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let d = ((dx * dx + dy * dy) as f64).sqrt();
                if d <= r {
                    blend(
                        g,
                        0xf8f0f0,
                        js_round(x) + dx,
                        js_round(y) + dy,
                        0.75 * (1.0 - u),
                    );
                }
            }
        }
    }
    // The whistle as the engine nears the crossing.
    let tw = time_at(g, 0.0, road_x(g) - 60.0);
    if t > tw && t < tw + 1.3 {
        for s in 0..6 {
            let a = (t - tw - s as f64 * 0.1).max(0.0);
            if a <= 0.0 || a > 0.7 {
                continue;
            }
            let (fs, _) = front(g, t - a);
            let x = fs - 36.0 + 14.0;
            glow(
                g,
                0xffffff,
                x,
                R - 20.0 - a * 12.0,
                2.0 + a * 3.0,
                0.9 * (1.0 - a / 0.7),
            );
        }
    }
}

/// The headlamp: a bright lamp and a long beam down the line.
fn headlight(g: &mut World, fx0: f64) {
    let (lx, ly) = (fx0 - 36.0 + 34.0, R - 17.0);
    if lx < -60.0 || lx > g.w() as f64 + 4.0 {
        return;
    }
    for d in 1..56 {
        let df = d as f64;
        let cy = ly + df * 0.14;
        let hs = 1.0 + df * 0.17;
        let k = 0.45 * (1.0 - df / 56.0);
        let y0 = (cy - hs).floor() as i32;
        let y1 = (cy + hs).ceil() as i32;
        for y in y0..=y1 {
            let e = 1.0 - ((y as f64 - cy).abs() / (hs + 0.5)).min(1.0);
            blend(g, 0xfff0c0, js_round(lx) + d, y, k * (0.4 + 0.6 * e));
        }
    }
    glow(g, 0xfff0c0, lx, ly, 6.0, 0.8);
    put(g, 0xffffff, js_round(lx), js_round(ly));
}

/// The guard's cap in flight from the caboose to the cow's head.
fn cap_flight(g: &mut World, t: f64, f: u32) {
    if !(T_TOSS..T_LAND).contains(&t) {
        return;
    }
    let (fb, len) = front(g, T_TOSS);
    let (x0, y0) = (fb - len - 3.0 - 1.5, R - 17.5);
    let (x1, y1) = (road_x(g) - 9.0, 43.0);
    let u = (t - T_TOSS) / (T_LAND - T_TOSS);
    let x = x0 + (x1 - x0) * u;
    let y = y0 + (y1 - y0) * u - (u * PI).sin() * 12.0;
    g.spr(CAP[((f / 2) % 2) as usize], CAPP, x, y, (f / 4) % 2 == 1);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (fx0, _) = front(g, t);
    sky(g, t);
    let sx = sun(g, t);
    clouds(g, t);
    hills(g, sx, t);
    wires(g, fx0, f);
    crossing(g);
    track(g);
    // The train, front to back.
    let ang = fx0 / 3.0;
    let mut x = fx0;
    let mut cattle_off = 0.0;
    for (car, l) in CARS {
        let x0 = x - l;
        if x0 < g.w() as f64 + 2.0 && x > -8.0 {
            match car {
                Car::Loco => loco(g, x0, ang, f),
                Car::Tender => tender(g, x0, ang),
                Car::Box => boxcar(g, x0, ang, t),
                Car::Logs => logs(g, x0, ang),
                Car::Tank => tank(g, x0, ang),
                Car::Hopper => hopper(g, x0, ang),
                Car::Coach => coach(g, x0, ang, f),
                Car::Cattle => cattle(g, x0, ang, f),
                Car::Caboose => caboose(g, x0, ang, f, t),
            }
            g.p(INK, x0 - GAP, R - 6.0, GAP as i32, 1);
        }
        if let Car::Cattle = car {
            cattle_off = fx0 - x0 - 10.0;
        }
        x = x0 - GAP;
    }
    smoke(g, t);
    foreground(g);
    crossing_near(g);
    signal(g, t, f);
    fireflies(g, t);
    headlight(g, fx0);
    // Hearts once the cousin in the cattle car has passed the cow.
    let th = time_at(g, cattle_off, road_x(g) - 4.0);
    cow(g, t, f, ((t - th) / 1.4).max(0.0));
    cap_flight(g, t, f);
}

/// Fireflies wake up over the grass as it gets dark.
fn fireflies(g: &mut World, t: f64) {
    let w = g.w() as f64;
    for k in 0..g.spread(16) {
        let kf = k as f64;
        let wake = 4.0 + rnd(kf + 90.0) * 4.0;
        if t < wake {
            continue;
        }
        let x = rnd(kf + 91.0) * w + (t * 0.7 + kf).sin() * 4.0;
        let y = 50.0 + rnd(kf + 92.0) * 7.0 + (t * 1.1 + kf * 2.0).cos() * 2.0;
        let pulse = ((t * 2.2 + kf * 1.3).sin() * 0.5 + 0.5) * ((t - wake) / 0.8).min(1.0);
        if pulse > 0.15 {
            glow(g, 0xd8ff7a, x, y, 3.0, 0.5 * pulse);
            blend(g, 0xf0ffb0, js_round(x), js_round(y), pulse);
        }
    }
}

fn crossing_near(g: &mut World) {
    let rx = road_x(g);
    // Planks over the ballast, then the road towards the viewer.
    g.p(0x6a5260, rx - 6.0, R + 1.0, 12, 5);
    for k in 0..4 {
        g.p(0x8a6a68, rx - 6.0, R + 1.0 + k as f64 * 1.0, 12, 1);
        g.p(0x4a3a48, rx - 6.0 + (k * 3) as f64, R + 1.0, 1, 5);
    }
    for y in 50..60 {
        let half = 6.0 + (y - 50) as f64 * 0.9;
        g.p(0x4a3a48, rx - half, y as f64, js_round(half * 2.0), 1);
        g.p(0x6a5866, rx - half, y as f64, 1, 1);
    }
}

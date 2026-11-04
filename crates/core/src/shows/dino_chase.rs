//! Dinosaur chase: a puddle on a dirt road ripples to heavy footsteps, a
//! jeep splashes through it, and a big two-legged dinosaur thunders after
//! it across a prehistoric valley (a smoking volcano, palm silhouettes,
//! pterosaurs). Its face fills the jeep's side mirror, the jeep jumps a
//! ridge and escapes, the driver's hat lands on the dinosaur's head and
//! the dinosaur roars.

use super::world::{ease, js_round, rnd, World};

pub const SECS: f64 = 10.0;

/// Jeep body, facing right; the wheels are drawn on top.
const JEEP: &[&str] = &[
    "..........c...........",
    ".....hhh..c...........",
    "...hhhhhh.c...........",
    "......ss..cm..........",
    ".....oooo.c...........",
    "..t.kkkkkkkkkkkkkkkk..",
    ".ttkyyyyyyyyyyyyyyyyyk",
    ".ttkyyyyyyyyyyyyyyyyyk",
    ".ttkgggggggggggggggggk",
    "..tkyyk....kyyyyk....k",
    "....kk......kkkk......",
];
/// The jeep without its driver's hat, once it has flown off.
const JEEP_NOHAT: &[&str] = &[
    "..........c...........",
    "..........c...........",
    "......ll..c...........",
    "......ss..cm..........",
    ".....oooo.c...........",
    "..t.kkkkkkkkkkkkkkkk..",
    ".ttkyyyyyyyyyyyyyyyyyk",
    ".ttkyyyyyyyyyyyyyyyyyk",
    ".ttkgggggggggggggggggk",
    "..tkyyk....kyyyyk....k",
    "....kk......kkkk......",
];
const JEEPP: &[(char, u32)] = &[
    ('k', 0x1c1a14),
    ('y', 0xd8b25a),
    ('g', 0x2f6a5a),
    ('t', 0x2a2620),
    ('c', 0x8a9aa0),
    ('m', 0xe8f4ff),
    ('h', 0xf0e0b0),
    ('l', 0x5a3a22),
    ('s', 0xf0b890),
    ('o', 0x6a8a4a),
];

const HAT: &[&str] = &["..kkk..", ".khhhk.", ".kbbbk.", "kkkkkkk"];
const HATP: &[(char, u32)] = &[('k', 0x3a2a1a), ('h', 0xf0e0b0), ('b', 0x7a4a2a)];

/// The dinosaur, facing right: two strides and a roar.
const DINO: [&[&str]; 3] = [
    &[
        "...............kkkkkkkk...",
        "..............kbbbbbbbbk..",
        "..............kbebbbbbbbk.",
        "..............kbbbbbbbbbbk",
        "..............kdbbbbbbbbbk",
        ".............kbbbbbwwwwwk.",
        ".............kdbbbkkkkkk..",
        ".............kbbbblwwwk...",
        "............kdbbbbbllk....",
        "kk.........kbbbbbbkk......",
        "kbkk......kdbbbbbbbk......",
        ".kbbkkkkkkbdbbbbbbbbkaa...",
        "..kbbbdbbbdbbbbbbbbllk.a..",
        "...kkbbbbbbbbbbbbllllk....",
        ".....kkbbbbbbbbbllllk.....",
        ".......kbbbbbbbbllk.......",
        ".......kbbbkkkbbbk........",
        "......kbbk...kbbbk........",
        "......kbk.....kbbk........",
        ".....kkkk.....kkkkk.......",
    ],
    &[
        "...............kkkkkkkk...",
        "..............kbbbbbbbbk..",
        "..............kbebbbbbbbk.",
        "..............kbbbbbbbbbbk",
        "..............kdbbbbbbbbbk",
        ".............kbbbbbwwwwwk.",
        ".............kdbbbkkkkkk..",
        ".............kbbbblwwwk...",
        "............kdbbbbbllk....",
        "kk.........kbbbbbbkk......",
        "kbkk......kdbbbbbbbk......",
        ".kbbkkkkkkbdbbbbbbbbkaa...",
        "..kbbbdbbbdbbbbbbbbllk.a..",
        "...kkbbbbbbbbbbbbllllk....",
        ".....kkbbbbbbbbbllllk.....",
        ".......kbbbbbbbbllk.......",
        "........kbbbkbbbk.........",
        "........kbbkkbbk..........",
        ".......kbbk.kbbk..........",
        "......kkkk..kkkkk.........",
    ],
    &[
        ".................kkkkk....",
        "...............kkbbbbbk...",
        "..............kbbebbbbbk..",
        "..............kbbbbbbwwk..",
        "..............kdbbbbw.....",
        ".............kbbbbbrr.....",
        ".............kdbbbbrr.....",
        ".............kbbbbbbwwk...",
        "............kdbbbbbllllk..",
        "kk.........kbbbbbbkkkk....",
        "kbkk......kdbbbbbbbk......",
        ".kbbkkkkkkbdbbbbbbbbkaa...",
        "..kbbbdbbbdbbbbbbbbllk.a..",
        "...kkbbbbbbbbbbbbllllk....",
        ".....kkbbbbbbbbbllllk.....",
        ".......kbbbbbbbbllk.......",
        ".......kbbbkkkbbbk........",
        "......kbbk...kbbbk........",
        "......kbk.....kbbk........",
        ".....kkkk.....kkkkk.......",
    ],
];
const DINOP: &[(char, u32)] = &[
    ('k', 0x161a10),
    ('b', 0x56683a),
    ('d', 0x3a4a26),
    ('l', 0x9aa060),
    ('a', 0x48582e),
    ('e', 0xffd23a),
    ('w', 0xf4f0e0),
    ('r', 0xb0303a),
];

/// The side mirror blown up: the dinosaur's eye and teeth fill it.
const MIRROR: &[&str] = &[
    "...ffffffffff...",
    ".ffssssssssgsff.",
    "fssssssssssgsssf",
    "fsddddddddddddsf",
    "fdbbebbbbbbnbbdf",
    "fdbbkbbbbbbbbbbf",
    "fdbbbbbbbbbbbbbf",
    "fdbbwbwbwbwbwbkf",
    "fdkkkkkkkkkkkkkf",
    ".fdbwbwbwbwbwbf.",
    "...ffffffffff...",
];
const MIRRORP: &[(char, u32)] = &[
    ('f', 0x2e2e34),
    ('s', 0x9ac0d0),
    ('g', 0xe8f4ff),
    ('d', 0x3a4a26),
    ('b', 0x56683a),
    ('e', 0xffd23a),
    ('k', 0x161a10),
    ('n', 0x2a3418),
    ('w', 0xf4f0e0),
];

const PTERO: [&[&str]; 2] = [
    &["k.....k..", ".kk..kk..", "...kkkkkk", "....k...."],
    &["...kkkkkk", ".kk.kk...", "k......k."],
];
const PTEROP: &[(char, u32)] = &[('k', 0x3a2a3a)];

const PALM: &[&str] = &[
    "...k...kk..",
    ".kkkk.kkkk.",
    "kk..kkk..kk",
    "k..kkkkk..k",
    "..k..k..k..",
];

/// Road surface height: flat, with a ramp-shaped ridge at `rx`.
fn road_y(x: f64, rx: f64) -> f64 {
    let d = x - rx;
    let hump = if d < 0.0 {
        (1.0 + d / 16.0).max(0.0) * 9.0
    } else {
        (1.0 - d / 6.0).max(0.0) * 9.0
    };
    46.0 - hump
}

/// How far the ground has scrolled: steady, then easing to a stop.
fn pan(t: f64) -> f64 {
    if t < 6.6 {
        28.0 * t
    } else {
        let u = (t - 6.6).min(1.0);
        28.0 * (6.6 + u - u * u / 2.0)
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let w = g.w();
    let wf = w as f64;
    let fi = f as i32;
    let n = g.layout.panels;
    let last = g.panel(3) as f64;
    // Hazy prehistoric dusk.
    g.bands(&[
        (0x2e4a5e, 8),
        (0x4a6a72, 7),
        (0x7a8a7a, 6),
        (0xc49a6a, 6),
        (0xe8b878, 6),
        (0xf2d08a, 4),
        (0x6a5a48, 23),
    ]);
    // A big low sun behind the haze on panel 2.
    let sun = (g.panel(2) + 40) as f64;
    g.disc(0xf8dc9a, sun, 30.0, 6);
    g.disc(0xfff0c0, sun, 30.0, 4);
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let cx = ((rnd(kf + 40.0) * wf - t * 1.5) % wf + wf) % wf;
        let cy = 6.0 + rnd(kf + 41.0) * 12.0;
        g.dith(0x6a8890, js_round(cx), js_round(cy), 14, 1, 0);
        g.dith(0x8aa0a0, js_round(cx) + 3, js_round(cy) - 1, 8, 1, 1);
    }
    // Far mountains, slowest layer.
    let p = pan(t);
    for x in 0..w {
        let xf = x as f64 + p * 0.08;
        let h = 6.0 + 4.0 * (xf * 0.045).sin().abs() + 3.0 * (xf * 0.11 + 1.0).sin();
        g.p(0x7a7a80, x as f64, 36.0 - h, 1, js_round(h) + 2);
        if (x + js_round(xf)) % 3 == 0 {
            g.p(0x8a8888, x as f64, 36.0 - h, 1, 1);
        }
    }
    // The volcano on panel 1, smoking, lava glowing in its flanks.
    let vx = (g.panel(1) + 28) as f64 - p * 0.03;
    for r in 0..18 {
        let half = 3 + r * 6 / 5;
        let y = 18.0 + r as f64;
        g.p(0x4a3a3e, vx - half as f64, y, half * 2 + 1, 1);
        g.dith(0x3a2c30, js_round(vx) + 1, r + 18, half, 1, r);
    }
    g.p(0xff7a2a, vx - 2.0, 18.0, 5, 1);
    g.p(0xffc04a, vx - 1.0, 18.0, 3, 1);
    for (k, len) in [(-1.0, 7), (2.0, 11)] {
        let glow = if (fi / 3 + len) % 4 == 0 {
            0xffb04a
        } else {
            0xd0502a
        };
        for j in 0..len {
            g.p(
                glow,
                vx + k + (j / 3) as f64 * k.signum(),
                19.0 + j as f64,
                1,
                1,
            );
        }
    }
    for k in 0..9 {
        let age = ((t * 4.0 + k as f64 * 1.1) % 9.0) / 9.0;
        let sx = vx + age * 22.0 + (age * 9.0 + k as f64).sin() * 2.0;
        let sy = 16.0 - age * 15.0;
        let r = 1 + js_round(age * 3.0);
        let c = if age < 0.3 { 0x5a4a50 } else { 0x7a6a70 };
        g.disc(c, sx, sy, r);
        g.p(0x8a7a7a, sx - r as f64 + 1.0, sy - r as f64 + 1.0, 1, 1);
    }
    // The roar shakes loose a few lava bombs from the crater.
    if t > 8.3 {
        for k in 0..5 {
            let kf = k as f64;
            let u = (t - 8.3 - kf * 0.12).max(0.0);
            if u <= 0.0 || u > 1.2 {
                continue;
            }
            let vxk = (rnd(kf + 90.0) - 0.5) * 14.0;
            let (lx, ly) = (vx + vxk * u, 17.0 - 16.0 * u + 18.0 * u * u);
            g.p(0xffc04a, lx, ly, 1, 1);
            g.p(0xd0502a, lx - vxk.signum(), ly + 1.0, 1, 1);
        }
    }
    // Pterosaurs drift over the valley, scattering at the roar.
    let roar = t > 8.25;
    for k in 0..g.spread(4) {
        let kf = k as f64;
        let mut px = ((rnd(kf + 7.0) * wf + t * (5.0 + kf * 2.0)) % (wf + 20.0)) - 10.0;
        let mut py = 6.0 + rnd(kf + 8.0) * 10.0 + (t * 1.5 + kf).sin() * 2.0;
        if roar {
            let e = t - 8.25;
            let away = if px > last - 30.0 { 1.0 } else { 0.0 };
            px += away * e * 25.0;
            py -= away * e * 14.0;
        }
        let ph = ((f + k * 3) / if roar { 1 } else { 3 }) % 2;
        g.spr(PTERO[ph as usize], PTEROP, px, py, false);
    }
    // Mid hills with palm silhouettes.
    for x in 0..w {
        let xf = x as f64 + p * 0.35;
        let h = 5.0 + 2.0 * (xf * 0.07).sin() + 1.5 * (xf * 0.19 + 2.0).sin();
        g.p(0x3a4a34, x as f64, 41.0 - h, 1, js_round(h) + 3);
    }
    let span = wf + 40.0;
    for k in 0..g.spread(9) {
        let kf = k as f64;
        let base = rnd(kf + 20.0) * span;
        let x = ((base - p * 0.35) % span + span) % span - 20.0;
        let ht = 8 + (rnd(kf + 21.0) * 8.0) as i32;
        let lean = if k % 2 == 0 { 1.0 } else { -1.0 };
        for j in 0..ht {
            let dx = lean * (j as f64 / ht as f64).powi(2) * 3.0;
            g.p(0x24301e, x + dx, (38 - j) as f64, 1, 1);
        }
        let sway = js_round((t * 2.0 + kf).sin() * 0.6) as f64;
        g.spr(
            PALM,
            &[('k', 0x24301e)],
            x + lean * 3.0 - 5.0 + sway,
            (38 - ht - 3) as f64,
            k % 3 == 0,
        );
    }
    // The road: dirt with ruts and pebbles scrolling under the chase.
    let ridge = last + 44.0 + 28.0 * 7.1 - p;
    for x in 0..w {
        let xf = x as f64;
        let top = js_round(road_y(xf, ridge));
        g.p(0x8a6a44, xf, top as f64, 1, 60 - top);
        g.p(0x5a7a3a, xf, top as f64 - 1.0, 1, 1);
        if top < 46 {
            let lit = xf < ridge;
            g.p(
                if lit { 0x9a7a4c } else { 0x6a5034 },
                xf,
                top as f64 + 1.0,
                1,
                46 - top,
            );
            if (x * 5 + top) % 7 == 0 {
                g.p(0x5a4028, xf, top as f64 + 3.0, 1, 1);
            }
            if x % 4 == 0 {
                g.p(0x6a8a3a, xf, top as f64 - 2.0, 1, 1);
            }
        }
    }
    let scroll = js_round(p);
    for x in 0..w {
        let s = x + scroll;
        if s % 7 < 4 {
            g.p(0x6e5234, x as f64, 49.0, 1, 1);
        }
        if s % 11 == 0 {
            g.p(0xa88a5a, x as f64, 47.0, 1, 1);
        }
        if s % 13 < 5 {
            g.p(0x6e5234, x as f64, 52.0, 1, 1);
        }
    }
    // The puddle on panel 0: rings to every footstep, then the splash.
    let pud = (g.panel(0) + 34) as f64 - p;
    if pud > -12.0 {
        g.p(0x5a7a8a, pud - 5.0, 50.0, 11, 2);
        g.p(0x8ab0c0, pud - 3.0, 50.0, 6, 1);
        for k in 0..3 {
            let lt = t - 0.15 - k as f64 * 0.45;
            if (0.0..0.4).contains(&lt) {
                let r = 1.0 + lt * 10.0;
                g.p(0xd8f0f8, pud - r, 50.0, 1, 1);
                g.p(0xd8f0f8, pud + r, 50.0, 1, 1);
                g.p(0xd8f0f8, pud - r * 0.5, 51.0, 1, 1);
                g.p(0xd8f0f8, pud + r * 0.5, 51.0, 1, 1);
            }
        }
    }
    // The jeep: from off panel 0 to the ridge, then up and away.
    let jump_t = 7.3;
    let (j0, j1) = (-24.0, last + 30.0);
    let jx = if t < jump_t {
        j0 + (t - 0.4).max(0.0) / (jump_t - 0.4) * (j1 - j0)
    } else {
        j1 + (t - jump_t) * 60.0
    };
    let air = t >= jump_t;
    let jy = if air {
        let u = t - jump_t;
        27.0 - 40.0 * u + 70.0 * u * u
    } else {
        road_y(jx + 11.0, ridge) - 10.0 + if (f / 2).is_multiple_of(2) { 0.0 } else { -1.0 }
    };
    // Splash when the jeep crosses the puddle.
    let sp = jx + 11.0 - pud;
    if (-2.0..30.0).contains(&sp) {
        for k in 0..6 {
            let kf = k as f64;
            let u = (sp / 30.0 + kf * 0.07).min(1.0);
            g.p(
                0xc8e8f4,
                pud - 4.0 + kf * 2.0 - u * 6.0,
                48.0 - (u * std::f64::consts::PI).sin() * (6.0 + kf),
                1,
                1,
            );
        }
    }
    // Dust kicked up behind the wheels.
    if !air {
        for k in 0..6 {
            let age = ((fi + k * 2) % 12) as f64;
            g.dith(
                if age < 6.0 { 0xb8946a } else { 0xa08060 },
                js_round(jx - 2.0 - age * 2.0),
                js_round(jy + 8.0 - age * 0.6),
                2 + js_round(age / 4.0),
                2,
                k,
            );
        }
    }
    let hat_t = jump_t + 0.05;
    g.spr(
        if t < hat_t { JEEP } else { JEEP_NOHAT },
        JEEPP,
        jx,
        jy,
        false,
    );
    for wx in [5.0, 16.0] {
        g.disc(0x1c1a14, jx + wx, jy + 10.0, 2);
        let spin = (f % 2) as f64;
        g.p(0x8a8a80, jx + wx - 1.0 + spin, jy + 10.0 - spin, 1, 1);
    }
    // The dinosaur: closes in, nearly bites, stops at the ridge and roars.
    let stop = last + 12.0;
    let close = (1.0 - ease((t - 1.0) / 4.4)) * 34.0 + 6.0;
    let dx = (jx - close - 24.0).min(stop);
    let stopped = dx >= stop;
    let frame = if t > 8.25 && t < 9.7 {
        2
    } else if stopped {
        0
    } else {
        ((f / 2) % 2) as usize
    };
    let bob = if frame < 2 && !stopped && (f / 2) % 2 == 1 {
        -1.0
    } else {
        0.0
    };
    let dy = 27.0 + bob;
    g.spr(DINO[frame], DINOP, dx, dy, false);
    // Footstep dust where each foot lands.
    if !stopped && (f / 2).is_multiple_of(2) {
        g.dith(0xb8946a, js_round(dx + 5.0), 45, 6, 2, fi);
    }
    // Objects in the mirror: a bubble over the jeep holds the dinosaur's
    // eye and teeth while it is about to bite.
    let mp = g.panel(n.saturating_sub(2)) as f64;
    if !air && jx > mp + 8.0 && jx < mp + 40.0 {
        let (bx, by) = (jx + 4.0, jy - 15.0);
        g.p(0x2a2620, jx + 11.0, by + 11.0, 1, 3);
        g.spr(MIRROR, MIRRORP, bx, by, false);
        // A blink: the eye closes for a frame now and then.
        if (f / 2).is_multiple_of(5) {
            g.p(0x56683a, bx + 4.0, by + 4.0, 3, 2);
            g.p(0x161a10, bx + 4.0, by + 5.0, 3, 1);
        }
    }
    // The hat flies off at the jump and lands on the dinosaur's head.
    if t >= hat_t {
        let u = ((t - hat_t) / 0.9).min(1.0);
        let (hx0, hy0) = (j1 + 3.0, 27.0);
        let (hx1, hy1) = (stop + 15.0, 24.0);
        let mut hx = hx0 + (hx1 - hx0) * u;
        let mut hy = hy0 + (hy1 - hy0) * u - (u * std::f64::consts::PI).sin() * 16.0;
        if u >= 1.0 && frame == 2 {
            hy -= if (f / 2).is_multiple_of(2) { 2.0 } else { 1.0 };
            hx -= 1.0;
        }
        g.spr(HAT, HATP, hx, hy, false);
    }
    // The roar: shock arcs from the open jaw, pebbles hopping.
    if frame == 2 {
        let e = t - 8.25;
        let (mx, my) = (dx + 22.0, dy + 6.0);
        for k in 0..3 {
            let r = ((e * 22.0 + k as f64 * 6.0) % 18.0) + 3.0;
            let c = if r < 10.0 { 0xfff4d0 } else { 0xe8c890 };
            for a in -6..=6 {
                let th = a as f64 * 0.12 - 0.35;
                g.p(c, mx + th.cos() * r, my + th.sin() * r, 1, 1);
            }
        }
        for k in 0..5 {
            let hop = ((e * 9.0 + k as f64).sin()).abs() * 3.0;
            g.p(0x5a4630, last + 10.0 + k as f64 * 9.0, 46.0 - hop, 1, 1);
        }
    }
    // Foreground ferns, fastest layer.
    let span = wf + 30.0;
    for k in 0..g.spread(14) {
        let kf = k as f64;
        let x = ((rnd(kf + 60.0) * span - p * 1.6) % span + span) % span - 15.0;
        let sz = 3 + (rnd(kf + 61.0) * 3.0) as i32;
        for j in 0..sz {
            let jf = j as f64;
            g.p(0x2a4a24, x - jf, 59.0 - jf, 1, 1);
            g.p(0x2a4a24, x + jf, 59.0 - jf, 1, 1);
            g.p(0x3a6a2e, x, 59.0 - jf * 1.6, 1, 2);
        }
    }
}

//! Snow day: a bright winter noon over a hillside village. Snow drifts
//! down out of a blue sky, far mountains glitter, chimneys smoke in the
//! valley and a log cabin sits on top of the hill, its roof thick with
//! snow. A kid in a bobble hat sits on a red sled outside, scoots to the
//! edge, and is off: down the long slope across the screens (dashing
//! through the gaps), past pines heavy with snow (one dumps its load as
//! the sled whizzes by), with the family dog barking and bounding after.
//! At the bottom a snow kicker launches the sled into the air, arms up,
//! straight into the snowman by the frozen pond. The snowman bursts; the
//! sled skids out onto the ice while a skater leaps clean over it, and
//! the snowman's head lands on the back of the sled and its top hat
//! drops neatly onto the dazed kid. Then the dog arrives, snatches the
//! carrot nose and sits there, very pleased with itself.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, vgrad, vnoise};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

mod cast;
use cast::{draw_dog, draw_kid, draw_skater, snowman};

pub const SECS: f64 = 12.0;

/// The kid pushes off.
const T_PUSH: f64 = 1.7;
/// Off the kicker.
const T_LAUNCH: f64 = 6.3;
/// Down into the snowman.
const T_LAND: f64 = 7.2;
/// The sled comes to rest on the ice.
const T_STOP: f64 = 8.6;
/// The top hat lands on the kid.
const T_HAT: f64 = 8.9;
/// The dog catches up.
const T_DOG: f64 = 9.7;
/// The carrot changes hands.
const T_SNATCH: f64 = 10.1;

/// The hilltop and the valley floor.
const TOP: f64 = 26.0;
const FLOOR: f64 = 50.0;
const OUT: u32 = 0x2a1e30;
const SNOW: u32 = 0xf4f8ff;
const SHADE: u32 = 0xc4d4ee;

fn smooth(a: f64, b: f64, t: f64) -> f64 {
    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn last(g: &World) -> u32 {
    g.layout.panels - 1
}

/// Stage position of world `x`: the inverse of `stage_x`.
fn stage_of(g: &World, x: f64) -> f64 {
    let gap = g.layout.gap as f64;
    let rush = stage_at(g, 1, 0.0) - 60.0;
    for i in 0..=last(g) {
        let x0 = g.px0(i) as f64;
        if x < x0 + 60.0 || i == last(g) {
            return stage_at(g, i, x - x0);
        }
        if x < x0 + 60.0 + gap {
            return stage_at(g, i, 60.0) + (x - x0 - 60.0) / gap * rush;
        }
    }
    x
}

/// The kicker's lip, on the screen before the last.
fn lip(g: &World) -> f64 {
    stage_at(g, last(g) - 1, 28.0)
}

/// Where the sled comes down, on the last screen.
fn landing(g: &World) -> f64 {
    stage_at(g, last(g), 3.0)
}

/// Height of the hill at stage position `s`.
fn ground_s(g: &World, s: f64) -> f64 {
    let (sa, sb) = (stage_at(g, 0, 40.0), stage_at(g, last(g), 2.0));
    let u = ((s - sa) / (sb - sa)).clamp(0.0, 1.0);
    let v = u.powf(0.6);
    let y = TOP
        + (FLOOR - TOP) * v * v * (3.0 - 2.0 * v)
        + (s * 0.13).sin() * 0.8 * 4.0 * u * (1.0 - u);
    let (k0, k1) = (lip(g) - 12.0, lip(g));
    let kick = if (k0..k1).contains(&s) {
        6.0 * ((s - k0) / (k1 - k0)).powf(1.5)
    } else if (k1..k1 + 2.0).contains(&s) {
        6.0 * (1.0 - (s - k1) / 2.0)
    } else {
        0.0
    };
    y - kick
}

fn ground(g: &World, x: f64) -> f64 {
    ground_s(g, stage_of(g, x))
}

#[derive(Clone, Copy, PartialEq)]
enum Pose {
    Sit,
    Ride,
    Fly,
    Dazed,
    Laugh,
}

struct Kid {
    /// Sled centre and runner bottom.
    x: f64,
    y: f64,
    /// Lean down the hill.
    slope: f64,
    pose: Pose,
    speed: f64,
}

fn start(g: &World) -> f64 {
    stage_at(g, 0, 34.0)
}

fn ride_s(g: &World, t: f64) -> f64 {
    let u = ((t - T_PUSH) / (T_LAUNCH - T_PUSH)).clamp(0.0, 1.0);
    start(g) + (lip(g) - start(g)) * u.powf(1.8)
}

/// The skid out across the ice, as an offset on the last screen.
fn slide_lx(t: f64) -> f64 {
    let u = ((t - T_LAND) / (T_STOP - T_LAND)).clamp(0.0, 1.0);
    3.0 + 39.0 * (1.0 - (1.0 - u).powi(2))
}

fn kid(g: &World, t: f64) -> Kid {
    if t < T_PUSH {
        // Scooting to the edge with both mittens.
        let wig = if t > 0.7 {
            (t * 10.0).sin().round()
        } else {
            0.0
        };
        let s = start(g) + smooth(0.7, T_PUSH, t) * 2.0;
        return Kid {
            x: stage_x(g, s) + wig,
            y: ground_s(g, s),
            slope: 0.0,
            pose: Pose::Sit,
            speed: 0.0,
        };
    }
    if t < T_LAUNCH {
        let s = ride_s(g, t);
        let slope = (ground_s(g, s + 3.0) - ground_s(g, s - 3.0)) / 6.0;
        return Kid {
            x: stage_x(g, s),
            y: ground_s(g, s),
            slope: slope.clamp(-0.6, 0.5),
            pose: Pose::Ride,
            speed: (ride_s(g, t + 0.05) - ride_s(g, t)) * 20.0,
        };
    }
    if t < T_LAND {
        let u = (t - T_LAUNCH) / (T_LAND - T_LAUNCH);
        let (s0, s1) = (lip(g), landing(g));
        let y0 = ground_s(g, s0);
        let y = |u: f64| y0 + (FLOOR - y0) * u - 13.0 * 4.0 * u * (1.0 - u);
        return Kid {
            x: stage_x(g, s0 + (s1 - s0) * u),
            y: y(u),
            slope: ((y(u + 0.05) - y(u)) / ((s1 - s0) * 0.05)).clamp(-0.6, 0.6),
            pose: Pose::Fly,
            speed: 40.0,
        };
    }
    let lx = slide_lx(t);
    let land = t - T_LAND;
    let bounce = if land < 0.35 {
        (land / 0.35 * PI).sin() * 3.0
    } else {
        0.0
    };
    Kid {
        x: g.panel(last(g)) as f64 + lx,
        y: FLOOR - bounce,
        slope: 0.0,
        pose: if t < T_DOG {
            Pose::Dazed
        } else if t > T_SNATCH + 0.4 {
            Pose::Laugh
        } else {
            Pose::Sit
        },
        speed: (slide_lx(t + 0.05) - lx) * 20.0,
    }
}

/// When the sled passes stage position `s` on its way down.
fn passes(g: &World, s: f64) -> f64 {
    let (mut a, mut b) = (T_PUSH, T_LAUNCH);
    for _ in 0..24 {
        let m = (a + b) / 2.0;
        if ride_s(g, m) < s {
            a = m;
        } else {
            b = m;
        }
    }
    a
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x4a84d4, 0x6c9ee0, 0x94bcec, 0xbcd6f4, 0xdce8f8],
        0,
        w,
        0,
        44,
    );
    let sx = g.panel(1) as f64 + 40.0;
    glow(g, 0xfff8e0, sx, 9.0, 34.0, 0.4);
    glow(g, 0xffffff, sx, 9.0, 9.0, 0.7);
    g.disc(0xfffce8, sx, 9.0, 4);
    // Soft winter clouds drifting east.
    for y in 1..22 {
        for x in 0..w {
            let n = vnoise(x as f64 * 0.045 - t * 0.12, y as f64 * 0.16, 7.0) * 0.7
                + vnoise(x as f64 * 0.11 - t * 0.2, y as f64 * 0.3, 9.0) * 0.3;
            let k = (n - 0.56) * 4.0 * (1.0 - (y as f64 - 9.0).abs() / 13.0);
            if k > 0.0 {
                let c = if n > 0.66 { 0xffffff } else { 0xe4ecf8 };
                blend(g, c, x, y, k.min(0.85));
            }
        }
    }
}

/// Far peaks, then the valley's snowy hills with a forest and a village.
fn valley(g: &mut World, t: f64) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let ridge = 26.0 - 8.0 * (xf * 0.031 + 1.0).sin().abs() - 3.0 * (xf * 0.097).sin().abs();
        let lean = (26.0 - 8.0 * ((xf + 1.0) * 0.031 + 1.0).sin().abs())
            - (26.0 - 8.0 * (xf * 0.031 + 1.0).sin().abs());
        for y in js_round(ridge)..44 {
            let depth = y as f64 - ridge;
            let snowy = depth < 3.5 + 2.0 * rnd(xf * 0.7);
            let c = match (snowy, lean > 0.0) {
                (true, false) => 0xf4f8ff,
                (true, true) => 0xc4d0ec,
                (false, false) => 0xa4b4d8,
                (false, true) => 0x8a9cc8,
            };
            put(g, c, x, y);
        }
        let mid = 37.0 - 3.0 * (xf * 0.027 + 2.0).sin() - 1.4 * (xf * 0.083).sin();
        for y in js_round(mid)..60 {
            let k = ((y as f64 - mid) / 8.0).min(1.0);
            put(
                g,
                if on(x, y, k * 0.7) {
                    0xb4c4e2
                } else {
                    0xc8d6ee
                },
                x,
                y,
            );
        }
        put(g, 0xe4ecf8, x, js_round(mid));
    }
    // A dark band of far pines with snowy tops.
    for k in 0..g.spread(60) {
        let kf = k as f64;
        let x = rnd(kf + 20.0) * w as f64;
        let mid = 37.0 - 3.0 * (x * 0.027 + 2.0).sin() - 1.4 * (x * 0.083).sin();
        let h = 3 + (rnd(kf + 21.0) * 3.0) as i32;
        let (xi, yi) = (js_round(x), js_round(mid + 1.0 + rnd(kf + 22.0) * 2.0));
        for r in 0..h {
            let half = r / 2;
            for i in -half..=half {
                let c = if r == 0 || (r == 2 && i == -half) {
                    0xf0f4fc
                } else {
                    0x46706a
                };
                put(g, c, xi + i, yi - h + r);
            }
        }
    }
    // The village: houses with snowy roofs, chimney smoke and a church.
    for i in 1..=last(g) {
        let p = g.px0(i) as f64;
        for (k, lx) in [10.0, 26.0, 47.0].into_iter().enumerate() {
            let id = (i * 3) as usize + k;
            let x = p + lx + rnd(id as f64) * 4.0;
            let mid = 37.0 - 3.0 * (x * 0.027 + 2.0).sin() - 1.4 * (x * 0.083).sin();
            if id % 4 == 2 {
                church(g, x, mid + 1.0);
                continue;
            }
            let walls = [0xc87a5a, 0xd8b070, 0x8aa0b8, 0xb86a6a];
            house(g, x, mid + 1.0, walls[id % 4], t + id as f64);
        }
    }
}

fn house(g: &mut World, x: f64, y: f64, wall: u32, t: f64) {
    let wall = mix(wall, 0xdce6f4, 0.3);
    g.p(wall, x, y - 4.0, 7, 4);
    g.p(mix(wall, 0x5a5a7a, 0.3), x + 5.0, y - 4.0, 2, 4);
    put(g, 0x6a7090, js_round(x) + 2, js_round(y) - 3);
    put(g, 0x6a7090, js_round(x) + 4, js_round(y) - 3);
    for r in 0..3 {
        g.p(
            if r == 0 { 0xffffff } else { 0xeef2fa },
            x - 1.0 + r as f64,
            y - 5.0 - r as f64,
            9 - 2 * r,
            1,
        );
    }
    g.p(0x8a6a6a, x + 5.0, y - 9.0, 1, 2);
    smoke(g, x + 5.0, y - 10.0, t, 0.35, 8.0);
}

fn church(g: &mut World, x: f64, y: f64) {
    let wall = 0xe6dcd0;
    g.p(wall, x, y - 5.0, 8, 5);
    g.p(0xc8c0bc, x + 6.0, y - 5.0, 2, 5);
    g.p(wall, x - 2.0, y - 10.0, 3, 10);
    put(g, 0x6a6a8a, js_round(x) - 1, js_round(y) - 8);
    for r in 0..4 {
        let half = (r + 1) / 2;
        g.p(
            0xf8fbff,
            x - 1.0 - half as f64,
            y - 14.0 + r as f64,
            1 + 2 * half,
            1,
        );
    }
    put(g, 0xd8b040, js_round(x) - 1, js_round(y) - 15);
    g.p(0xffffff, x, y - 6.0, 8, 1);
}

/// A thin column of chimney smoke, leaning east.
fn smoke(g: &mut World, x: f64, y: f64, t: f64, k: f64, len: f64) {
    for i in 0..6 {
        let u = (t * 0.3 + i as f64 / 6.0) % 1.0;
        let sx = x + u * len * 0.6 + (u * 6.0 + i as f64).sin() * 0.8;
        let sy = y - u * len;
        blend(g, 0xf8f8ff, js_round(sx), js_round(sy), k * (1.0 - u));
        blend(
            g,
            0xd8dcec,
            js_round(sx) + 1,
            js_round(sy),
            k * 0.7 * (1.0 - u),
        );
    }
}

/// The hillside the sled rides down, with its track.
fn hill(g: &mut World, t: f64) {
    let k = kid(g, t);
    let ks = stage_of(g, k.x);
    let (s0, lp, land) = (start(g), lip(g), landing(g));
    for x in 0..g.w() {
        let s = stage_of(g, x as f64);
        let gy = js_round(ground_s(g, s));
        let lee = ground_s(g, s + 1.0) < ground_s(g, s) - 0.5;
        for y in gy.max(0)..60 {
            let d = (y - gy) as f64 / 14.0;
            let drift = ((x as f64 * 0.09 + (y - gy) as f64 * 0.7).sin() * 0.5 + 0.5) * 0.25;
            let mut c = if on(x, y, d * 0.6 + drift) {
                0xe2ecf8
            } else {
                SNOW
            };
            if on(x, y, (d - 0.6).max(0.0)) {
                c = SHADE;
            }
            if lee && y - gy < 3 {
                c = SHADE;
            }
            put(g, c, x, y);
        }
        put(g, 0xffffff, x, gy);
        // Glints that come and go.
        let r = rnd(x as f64 * 1.7);
        if r > 0.93 && ((t * 2.0 + r * 9.0) % 3.0) < 0.4 {
            put(g, 0xffffff, x, gy + 2 + (r * 50.0) as i32 % 8);
            blend(g, 0xbfe0ff, x + 1, gy + 2 + (r * 50.0) as i32 % 8, 0.6);
        }
        // The sled's track.
        let tracked = (t > T_PUSH && s > s0 - 6.0 && s < ks.min(lp))
            || (t > T_LAND && s > land - 6.0 && s < ks - 6.0);
        if tracked {
            put(g, 0xb4c6e4, x, gy + 1);
            put(g, 0xd0dcf0, x, gy + 2);
        }
    }
}

/// The frozen pond on the last screen.
fn pond(g: &mut World, t: f64) {
    let p = g.panel(last(g)) as f64;
    let (cx, cy, rx, ry) = (p + 37.0, 54.5, 17.0, 5.0);
    for y in 48..60 {
        for x in js_round(cx - rx - 1.0)..=js_round(cx + rx + 1.0) {
            let d = ((x as f64 - cx) / rx).powi(2) + ((y as f64 - cy) / ry).powi(2);
            if d < 1.0 {
                let c = if on(x, y, d * 0.8) {
                    0xa8d4e8
                } else {
                    0xc8e8f4
                };
                put(g, c, x, y);
            } else if d < 1.35 {
                put(g, if y < cy as i32 { 0xffffff } else { SHADE }, x, y);
            }
        }
    }
    // Skate scratches and a slow glint sweeping the ice.
    for k in 0..5 {
        let kf = k as f64;
        let x = cx - 12.0 + rnd(kf + 60.0) * 24.0;
        let y = cy - 2.0 + rnd(kf + 61.0) * 4.0;
        line(
            g,
            0xe8f6fc,
            x,
            y,
            x + 3.0 + rnd(kf + 62.0) * 4.0,
            y + (rnd(kf + 63.0) - 0.5) * 2.0,
        );
    }
    let gx = cx - rx + ((t * 0.4) % 1.0) * rx * 2.0;
    for i in 0..3 {
        blend(g, 0xffffff, js_round(gx) + i, js_round(cy) - 2 + i, 0.7);
    }
}

fn cabin(g: &mut World, t: f64) {
    let x = g.panel(0) as f64 + 3.0;
    let y = TOP;
    // Log walls with a lit window and a door.
    for r in 0..9 {
        let c = if r % 2 == 0 { 0x8a5434 } else { 0x6e4028 };
        g.p(c, x, y - 9.0 + r as f64, 19, 1);
        put(g, 0x5a3420, js_round(x) - 1, js_round(y) - 9 + r);
        put(g, 0x5a3420, js_round(x) + 19, js_round(y) - 9 + r);
    }
    g.p(OUT, x + 3.0, y - 7.0, 6, 5);
    g.p(0xffd070, x + 4.0, y - 6.0, 4, 3);
    g.p(0xc88a40, x + 5.0, y - 6.0, 1, 3);
    g.p(0xc88a40, x + 4.0, y - 5.0, 4, 1);
    glow(g, 0xffd070, x + 6.0, y - 4.0, 5.0, 0.3);
    g.p(0x4a2a1a, x + 12.0, y - 7.0, 5, 7);
    g.p(0x5a3624, x + 13.0, y - 6.0, 3, 6);
    put(g, 0xd8b040, js_round(x) + 15, js_round(y) - 3);
    // The chimney, then a roof under a thick quilt of snow.
    g.p(0x7a6a6a, x + 14.0, y - 20.0, 3, 6);
    g.p(0x5a4a4a, x + 16.0, y - 20.0, 1, 6);
    g.p(SNOW, x + 13.0, y - 21.0, 5, 1);
    smoke(g, x + 15.0, y - 22.0, t, 0.55, 14.0);
    for r in 0..9 {
        let half = 1.0 + r as f64 * 1.35;
        let cx = x + 9.5;
        let (a, b) = (js_round(cx - half), js_round(cx + half));
        for xx in a..=b {
            let c = if r < 6 {
                if (xx as f64) < cx + 2.0 {
                    SNOW
                } else {
                    0xdce6f4
                }
            } else if r == 6 {
                0xe8f0fa
            } else {
                0x5a3a2a
            };
            put(g, c, xx, js_round(y) - 18 + r);
        }
    }
    // Snow lip hanging over the eaves, and icicles.
    g.p(0xffffff, x - 3.0, y - 11.0, 25, 1);
    for i in 0..8 {
        let ix = js_round(x) - 1 + i * 3;
        let len = 1 + (rnd(i as f64 + 30.0) * 3.0) as i32;
        for j in 0..len {
            put(
                g,
                if j == 0 { 0xe8f4ff } else { 0xb8dcf4 },
                ix,
                js_round(y) - 10 + j,
            );
        }
    }
    // A snow drift against the wall, and boot prints from the door.
    for i in 0..8 {
        put(g, SNOW, js_round(x) - 2 + i, js_round(y) - 1);
    }
    for i in 0..6 {
        put(g, SHADE, js_round(x) + 15 + i * 2, js_round(y) + 1 + i % 2);
    }
    // A rail fence half buried in the drifts below.
    let fy = 45.0;
    for k in 0..5 {
        let px = x + 4.0 + k as f64 * 10.0;
        g.p(0x6a4630, px, fy - 5.0, 2, 6);
        g.p(0x4a2e20, px + 1.0, fy - 4.0, 1, 5);
        g.p(0xffffff, px - 1.0, fy - 6.0, 4, 1);
    }
    for ry in [fy - 4.0, fy - 1.0] {
        g.p(0x7a5236, x + 4.0, ry, 41, 1);
        g.p(0xffffff, x + 6.0, ry - 1.0, 37, 1);
    }
    g.p(SNOW, x, fy + 1.0, 50, 2);
}

/// A pine bowed under snow. `snowy` thins the snow after it is shaken.
fn pine(g: &mut World, x: f64, base: f64, h: i32, snowy: f64, seed: f64) {
    let (xi, b) = (js_round(x), js_round(base));
    for r in 0..3 {
        put(g, 0x5a3a28, xi, b - r);
    }
    let top = b - 2 - h;
    for r in 0..h {
        let (tier, tn) = (r % 5, r / 5);
        let half = (tier as f64 * 0.9 + tn as f64 * 1.1 + 0.4) as i32;
        let y = top + r;
        for i in -half..=half {
            let id = seed + (r * 17 + i) as f64;
            let cap = tier == 0 || (tier == 1 && i.abs() < half) || (tier == 4 && i.abs() == half);
            let c = if cap && rnd(id) < snowy {
                if i > 0 {
                    0xd4e0f2
                } else {
                    0xffffff
                }
            } else if i > 0 {
                0x1e4a3e
            } else {
                0x2e6a50
            };
            put(g, c, xi + i, y);
        }
    }
    put(g, 0xffffff, xi, top - 1);
}

/// The pines on the slope: (screen, offset, height, the one that dumps).
fn pines(g: &World) -> Vec<(f64, i32, bool)> {
    let mut v = vec![(g.panel(0) as f64 + 54.0, 14, false)];
    for i in 1..last(g) {
        let p = g.px0(i) as f64;
        let list: &[(f64, i32)] = if i == last(g) - 1 {
            &[(5.0, 14), (42.0, 17), (54.0, 12)]
        } else {
            &[(9.0, 16), (27.0, 13), (48.0, 18)]
        };
        for (k, &(lx, h)) in list.iter().enumerate() {
            v.push((p + lx, h, i == 1 && k == 1));
        }
    }
    v
}

fn draw_pines(g: &mut World, t: f64) {
    for (k, (x, h, dumps)) in pines(g).into_iter().enumerate() {
        let base = ground(g, x) + 1.0;
        let tp = passes(g, stage_of(g, x));
        let shaken = dumps && t > tp + 0.1;
        pine(
            g,
            x,
            base,
            h,
            if shaken { 0.25 } else { 0.95 },
            k as f64 * 100.0,
        );
        if dumps && t > tp + 0.1 && t < tp + 1.2 {
            let u = t - tp - 0.1;
            for i in 0..28 {
                let fi = i as f64;
                let sx = x + (rnd(fi + 70.0) - 0.5) * 11.0;
                let sy = base - h as f64 + rnd(fi + 71.0) * h as f64 * 0.7 + 40.0 * u * u;
                if sy < ground(g, sx) {
                    let (c, xi, yi) = (
                        if i % 3 == 0 { SHADE } else { 0xffffff },
                        js_round(sx),
                        js_round(sy),
                    );
                    put(g, c, xi, yi);
                    if i % 2 == 0 {
                        put(g, c, xi + 1, yi);
                        put(g, c, xi, yi - 1);
                    }
                }
            }
            glow(
                g,
                0xffffff,
                x,
                base - h as f64 * 0.5 + 20.0 * u,
                6.0,
                0.5 * (1.0 - u),
            );
        }
    }
}

/// Snow drifting down over everything.
fn snowfall(g: &mut World, t: f64) {
    let w = g.w() as f64;
    for k in 0..g.spread(90) {
        let kf = k as f64;
        let near = k % 5 == 0;
        let v = if near { 9.0 } else { 4.5 } + rnd(kf + 3.0) * 3.0;
        let y = (rnd(kf + 1.0) * 64.0 + t * v) % 64.0 - 2.0;
        let x = (rnd(kf + 2.0) * w + t * 2.5 + (t * 1.3 + kf).sin() * 2.0).rem_euclid(w);
        let (xi, yi) = (js_round(x), js_round(y));
        blend(g, 0xffffff, xi, yi, if near { 1.0 } else { 0.7 });
        if near {
            blend(g, 0xffffff, xi + 1, yi, 0.6);
            blend(g, 0xffffff, xi, yi + 1, 0.6);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    valley(g, t);
    hill(g, t);
    pond(g, t);
    cabin(g, t);
    draw_pines(g, t);
    snowman(g, t);
    draw_dog(g, t, f);
    draw_kid(g, t, f);
    draw_skater(g, t, f);
    snowfall(g, t);
}

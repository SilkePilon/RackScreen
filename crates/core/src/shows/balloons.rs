//! Balloon festival: an early summer morning over a river valley. The sun
//! comes up behind lilac hills, mist lies in the hollows and drifts off the
//! river, and the fields are a patchwork of wheat, rapeseed and meadow
//! stitched with hedges. On the launch meadow the crews cold-inflate their
//! balloons from the grass; burners roar, the flames lighting the
//! envelopes from inside, and one by one they stand up and lift off,
//! drifting across the screens at their own heights while far balloons
//! hang in the haze. The last one up is a little polka-dot balloon with a
//! dog in the basket. It drifts low over a farmhouse and skims the washing
//! line, the dog snatches the line, and when the pilot fires the burner
//! the whole line of laundry comes away: the little balloon climbs off
//! into the morning dressed in socks and shirts like festival bunting, and
//! the farmer comes out to shake a fist at it.

use super::fx::{blend, glow, line, mix, on, put, stage_at, stage_x, vgrad, vnoise};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// The dog's balloon leaves the ground.
const T_LIFT_C: f64 = 4.6;
/// It comes down low over the farm.
const T_DIP: f64 = 8.1;
/// The dog grabs the washing line and the pilot fires the burner.
const T_GRAB: f64 = 8.7;

/// Where the valley floor meets the launch meadow.
const GROUND: f64 = 50.0;
const OUT: u32 = 0x2a1e2a;

#[derive(Clone, Copy)]
enum Pattern {
    /// Alternating vertical gores.
    Gores(u32, u32),
    /// Zigzag bands round the envelope.
    Chevron(u32, u32, u32),
    /// Polka dots on a ground colour.
    Dots(u32, u32),
    /// A gore of every colour.
    Rainbow,
}

struct Balloon {
    /// The envelope's mouth.
    x: f64,
    y: f64,
    /// Envelope radius.
    r: f64,
    /// 0 lying flat on the grass, 1 standing up full.
    inf: f64,
    /// How hard the burner is roaring.
    burn: f64,
    pat: Pattern,
    /// Haze towards the sky, for the far ones.
    haze: f64,
}

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

/// Burner bursts: on during `[a, b)`, flickering.
fn burner(t: f64, f: u32, bursts: &[(f64, f64)]) -> f64 {
    let on = bursts.iter().any(|&(a, b)| (a..b).contains(&t));
    if on {
        0.75 + 0.25 * ((f * 7 % 5) as f64 / 4.0)
    } else {
        0.0
    }
}

/// The sun: (x, y).
fn sun(g: &World, t: f64) -> (f64, f64) {
    let p = g.panel(last(g).saturating_sub(1)) as f64;
    (p + 34.0, 33.0 - t * 0.6)
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x6a88c8, 0x93a8d8, 0xc8bcd4, 0xf0cdb4, 0xfde2b0],
        0,
        w,
        0,
        38,
    );
    let (sx, sy) = sun(g, t);
    glow(g, 0xfff0c0, sx, sy, 40.0, 0.45);
    glow(g, 0xfff8e0, sx, sy, 12.0, 0.7);
    g.disc(0xfff2c8, sx, sy, 5);
    g.disc(0xfffcf0, sx, sy, 4);
    // Long thin clouds lit pink from below.
    for k in 0..g.spread(9) {
        let kf = k as f64;
        let x = (rnd(kf + 5.0) * w as f64 + t * 1.5) % (w as f64 + 40.0) - 20.0;
        let y = 5.0 + rnd(kf + 6.0) * 18.0;
        let len = 14 + (rnd(kf + 7.0) * 18.0) as i32;
        for i in 0..len {
            let e = 1.0 - ((i as f64 / len as f64) * 2.0 - 1.0).abs();
            let px = js_round(x) + i;
            blend(g, 0xfff0f0, px, js_round(y), 0.5 * e);
            blend(g, 0xf0a0a8, px, js_round(y) + 1, 0.45 * e);
        }
    }
}

/// Far hills and the valley's patchwork of fields.
fn land(g: &mut World, t: f64) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let far = 31.0 - 4.0 * (xf * 0.021 + 1.0).sin().max(0.0) - 1.5 * (xf * 0.07).sin();
        for y in js_round(far)..GROUND as i32 {
            put(g, dz(x, y, 0.3, 0xb0a0c8, 0xa494c0), x, y);
        }
        let mid = 36.0 - 3.0 * (xf * 0.033 + 2.5).sin() - (xf * 0.11).sin();
        for y in js_round(mid)..GROUND as i32 {
            let k = (y as f64 - mid) / 6.0;
            put(g, dz(x, y, k.min(1.0), 0x7a9aa0, 0x6a8a7a), x, y);
        }
        put(g, 0xa8c0b0, x, js_round(mid));
    }
    // A church on the far ridge.
    let cx = g.panel(1) as f64 + 14.0;
    let top = 36.0 - 3.0 * (cx * 0.033 + 2.5).sin() - (cx * 0.11).sin();
    g.p(0x8a90a8, cx, top - 4.0, 5, 4);
    g.p(0x8a90a8, cx + 4.0, top - 8.0, 2, 8);
    put(g, 0x8a90a8, js_round(cx) + 4, js_round(top) - 10);
    put(g, 0x8a90a8, js_round(cx) + 4, js_round(top) - 9);
    // Field rows, nearer ones deeper, split into plots.
    let rows = [(39, 42), (42, 46), (46, 51), (51, 60)];
    let crops = [0x9ab84a, 0xe8d050, 0x7aa848, 0xd8b868, 0x8a6a4a, 0xb0c860];
    for (ri, &(y0, y1)) in rows.iter().enumerate() {
        // Plot edges along the row, each leaning its own way.
        let mut edges = Vec::new();
        let mut x = -(rnd(ri as f64 * 9.0) * 30.0) - 10.0;
        while x < w as f64 + 20.0 {
            let k = edges.len();
            let lean = (rnd((ri * 50 + k) as f64 + 5.0) - 0.5) * (ri + 1) as f64 * 0.8;
            edges.push((x, lean));
            x += 26.0 + rnd((ri * 50 + k) as f64 + 1.0) * 40.0 + ri as f64 * 8.0;
        }
        for y in y0..y1 {
            let dy = (y - y0) as f64;
            let mut k = 0;
            for xx in 0..w {
                while k + 1 < edges.len() && edges[k + 1].0 + edges[k + 1].1 * dy <= xx as f64 {
                    k += 1;
                }
                let seed = (ri * 50 + k) as f64;
                let c = crops[(rnd(seed + 2.0) * crops.len() as f64) as usize];
                let edge = (edges[k].0 + edges[k].1 * dy).floor() as i32 == xx;
                let col = if edge || y == y0 {
                    dz(xx, y, 0.5, 0x3a5a30, 0x4a6a38)
                } else if c == 0x8a6a4a && (xx + y) % 3 == 0 {
                    0x6a4e38
                } else if c == 0xe8d050 && on(xx, y, 0.2) {
                    0xf8e870
                } else if on(xx, y, 0.15) {
                    mix(c, 0x3a5a2a, 0.3)
                } else {
                    c
                };
                put(g, col, xx, y);
            }
        }
        // Trees along the hedges.
        for (k, &(ex, _)) in edges.iter().enumerate() {
            let seed = (ri * 50 + k) as f64;
            if rnd(seed + 3.0) > 0.4 {
                let tx = ex + rnd(seed + 4.0) * 20.0;
                let tr = 1 + ri as i32 / 2;
                g.disc(0x2a4a2a, tx, (y0 - tr) as f64, tr);
                g.disc(0x3a6a3a, tx - 0.5, (y0 - tr - 1) as f64, tr - 1);
            }
        }
    }
    // The launch meadow on the first screens: smooth mown grass.
    let x1 = g.panel(1.min(last(g))) as f64 + 60.0;
    for x in 0..js_round(x1) {
        for y in GROUND as i32 - 1..60 {
            let stripe = ((x + y * 2) / 6) % 2 == 0;
            put(g, if stripe { 0x8ac05a } else { 0x7ab450 }, x, y);
        }
    }
    // The river winding through, the sky in it.
    for x in 0..w {
        let xf = x as f64;
        let cy = 44.5 + 2.5 * (xf * 0.028).sin() + 1.5 * (xf * 0.071 + 1.0).sin();
        if xf < x1 {
            continue;
        }
        let hw = 1.0 + (cy - 41.0) * 0.25;
        for y in js_round(cy - hw)..=js_round(cy + hw) {
            let glint = rnd(((x + js_round(t * 6.0)) / 2 * 13 + y) as f64) > 0.9;
            let c = if glint {
                0xfff8e0
            } else {
                dz(x, y, 0.4, 0xa8c4e8, 0x88aad8)
            };
            put(g, c, x, y);
        }
        put(g, 0x5a7a50, x, js_round(cy + hw) + 1);
    }
}

/// Mist in the hollows, thinning as the sun climbs.
fn mist(g: &mut World, t: f64) {
    let thin = 1.0 - 0.45 * smooth(3.0, SECS, t);
    for y in 36..52 {
        let band = 1.0 - ((y as f64 - 43.0) / 8.0).powi(2);
        for x in 0..g.w() {
            let n = vnoise(x as f64 / 26.0 - t * 0.25, y as f64 / 4.0, 4.0);
            let k = (n - 0.4).max(0.0) * band * thin * 0.8;
            if k > 0.0 {
                blend(g, 0xf8ece8, x, y, k.min(0.35));
            }
        }
    }
}

/// Half-width of the envelope at `v` (0 mouth .. 1 crown), in radii.
fn girth(v: f64) -> f64 {
    if v < 0.45 {
        0.22 + 0.78 * (v / 0.45 * PI / 2.0).sin()
    } else {
        (1.0 - ((v - 0.45) / 0.55).powi(2)).max(0.0).sqrt()
    }
}

/// The colour of the envelope at across `q` (-1..1) and up `v`.
fn pattern(p: Pattern, q: f64, v: f64) -> u32 {
    let gore = ((q.asin() / PI + 0.5) * 7.0).floor() as i32;
    match p {
        Pattern::Gores(a, b) => {
            if gore % 2 == 0 {
                a
            } else {
                b
            }
        }
        Pattern::Chevron(a, b, c) => {
            let z = v * 7.0 + (q * 3.5).sin().abs() * 0.8;
            [a, b, c, b][(z.floor() as usize) % 4]
        }
        Pattern::Dots(a, b) => {
            let (u, vv) = (q.asin() / PI * 6.0, v * 5.0);
            let row = vv.floor() as i32;
            let shift = if row % 2 == 0 { 0.0 } else { 0.5 };
            let (du, dv) = ((u + shift).fract().abs() - 0.5, vv.fract() - 0.5);
            if du * du + dv * dv < 0.07 {
                b
            } else {
                a
            }
        }
        Pattern::Rainbow => [
            0xe84a4a, 0xf09a3a, 0xf2d84a, 0x5ab85a, 0x4a8ae0, 0x8a5ad0, 0xe84a8a,
        ][gore.rem_euclid(7) as usize],
    }
}

fn balloon(g: &mut World, b: &Balloon, sky: u32) {
    let r = b.r;
    // Lying down it tips left, crown first, and is flat.
    let a = (1.0 - b.inf) * 1.35;
    let len = 2.25 * r * (0.75 + 0.25 * b.inf);
    let ws = 0.3 + 0.7 * b.inf.powf(0.7);
    let (ax, ay) = (-a.sin(), -a.cos());
    let (nx, ny) = (a.cos(), -a.sin());
    let ext = (len + r).ceil() as i32;
    let (mx, my) = (js_round(b.x), js_round(b.y));
    for py in my - ext..=my + ext / 3 {
        for px in mx - ext..=mx + ext / 2 {
            let (dx, dy) = (px as f64 - b.x, py as f64 - b.y);
            let v = (dx * ax + dy * ay) / len;
            if !(0.0..=1.0).contains(&v) {
                continue;
            }
            let half = girth(v) * r * ws;
            let c = dx * nx + dy * ny;
            if half < 0.5 || c.abs() > half {
                continue;
            }
            // Flat on the grass it never dips below the ground.
            if py as f64 > GROUND + 1.0 && b.inf < 0.5 {
                continue;
            }
            let q = (c / half).clamp(-1.0, 1.0);
            let mut col = pattern(b.pat, q, v);
            if v < 0.1 {
                col = mix(col, 0x2a2030, 0.45);
            }
            // Sun from the right; the shadow side dithers darker.
            let lit = q * 0.8 + (v - 0.5) * 0.3;
            if lit < -0.25 {
                col = dz(
                    px,
                    py,
                    ((-lit - 0.25) * 2.0).min(1.0),
                    col,
                    mix(col, 0x3a2a5a, 0.45),
                );
            } else if lit > 0.45 {
                col = dz(px, py, (lit - 0.45) * 2.5, col, mix(col, 0xfff4d8, 0.4));
            }
            if q.abs() > 0.88 || v > 0.985 {
                col = mix(col, OUT, 0.55);
            }
            // Burner light glowing through the fabric from inside.
            if b.burn > 0.0 && v < 0.55 {
                col = mix(col, 0xffc070, b.burn * 0.45 * (1.0 - v / 0.55));
            }
            put(g, mix(col, sky, b.haze), px, py);
        }
    }
    // Basket, ropes and burner.
    let bw = (r * 0.55).round().max(2.0) as i32;
    let bh = (r * 0.35).round().max(2.0) as i32;
    let (bx, by) = (
        mx - bw / 2,
        if b.inf < 1.0 {
            GROUND as i32 - bh
        } else {
            my + 3
        },
    );
    let mouth = girth(0.0) * r * ws;
    let ends = [(-1.0, bx), (1.0, bx + bw - 1)];
    for (sgn, kx) in ends {
        let (ex, ey) = (b.x + nx * mouth * sgn, b.y + ny * mouth * sgn);
        line(g, mix(0x5a4a3a, sky, b.haze), ex, ey, kx as f64, by as f64);
    }
    for j in 0..bh {
        for i in 0..bw {
            let c = if j == 0 {
                0xa07040
            } else if (i + j) % 2 == 0 {
                0x8a5a30
            } else {
                0x6a4424
            };
            put(g, mix(c, sky, b.haze), bx + i, by + j);
        }
    }
    if b.burn > 0.0 {
        // The flame climbs from the burner up the axis into the mouth.
        let n = (r * 0.5 * b.burn + 2.0) as i32;
        for i in 0..n {
            let u = i as f64 / n as f64;
            let fx = bx as f64 + bw as f64 / 2.0 + (b.x - bx as f64 - bw as f64 / 2.0) * u;
            let fy = by as f64 - 1.0 + (b.y - by as f64 + 1.0) * u - u * 2.0;
            let c = mix(0xffffff, 0xff8a30, u);
            put(g, c, js_round(fx), js_round(fy));
            if i % 2 == 0 {
                blend(g, 0xffd060, js_round(fx) + 1, js_round(fy), 0.6);
            }
        }
        glow(g, 0xffb050, b.x, b.y, r * 1.1, 0.5 * b.burn);
    }
}

/// The dog's balloon's mouth.
fn dog_balloon(g: &World, t: f64) -> (f64, f64) {
    let s0 = stage_at(g, 1.min(last(g)), 46.0);
    let s1 = stage_at(g, last(g), 35.0);
    let ground = GROUND - 2.0 - 3.0;
    if t < T_LIFT_C {
        return (stage_x(g, s0), ground);
    }
    if t < T_DIP {
        let u = (t - T_LIFT_C) / (T_DIP - T_LIFT_C);
        let s = s0 + (s1 - s0) * (0.35 * u + 0.65 * ease(u));
        let up = smooth(T_LIFT_C, T_LIFT_C + 1.2, t);
        let down = smooth(T_DIP - 1.1, T_DIP, t);
        let y = ground + (27.0 - ground) * up + (43.0 - 27.0) * down + (t * 2.4).sin() * 0.8 * up;
        return (stage_x(g, s), y);
    }
    let x = stage_x(g, s1);
    if t < T_GRAB {
        return (x + (t - T_DIP) * 2.0, 43.0);
    }
    let u = ease((t - T_GRAB - 0.2) / 3.0);
    (x + 1.2 + (t - T_GRAB) * 1.5, 43.0 - 21.0 * u)
}

fn sky_at(y: f64) -> u32 {
    mix(0x93a8d8, 0xf0cdb4, (y / 36.0).clamp(0.0, 1.0))
}

/// Balloons already up when the show starts.
fn far_balloons(g: &mut World, t: f64, f: u32) {
    let sky = sky_at;
    // Far balloons hanging in the haze, slow with distance.
    let n = g.layout.panels;
    for k in 0..(n * 2) {
        let kf = k as f64;
        let x = g.panel(k % n) as f64 + 8.0 + rnd(kf + 11.0) * 44.0 + t * (1.0 + rnd(kf) * 2.0);
        let y = 11.0 + rnd(kf + 12.0) * 11.0 + (t * 0.8 + kf).sin() * 0.6;
        let pats = [
            Pattern::Gores(0xe86a4a, 0xf8e0a0),
            Pattern::Rainbow,
            Pattern::Gores(0x4a8ae0, 0xf0f0f0),
            Pattern::Chevron(0x5ab85a, 0xf8f0d8, 0xe8a83a),
        ];
        let b = Balloon {
            x,
            y,
            r: 2.2 + rnd(kf + 13.0) * 1.6,
            inf: 1.0,
            burn: 0.0,
            pat: pats[k as usize % 4],
            haze: 0.45 - rnd(kf + 14.0) * 0.15,
        };
        balloon(g, &b, sky(y));
    }
    // Already up: a green and cream one drifting over the far fields.
    let p2 = g.panel(2.min(last(g))) as f64;
    let dy = (t * 1.1).sin() * 0.7;
    let d = Balloon {
        x: p2 + 12.0 + t * 3.0,
        y: 30.0 - t * 0.5 + dy,
        r: 6.5,
        inf: 1.0,
        burn: burner(t, f, &[(6.5, 7.2)]),
        pat: Pattern::Chevron(0x4a9a5a, 0xf4ecd0, 0x2a6a4a),
        haze: 0.15,
    };
    if g.layout.panels >= 4 {
        balloon(g, &d, sky(d.y));
    }
}

fn balloons(g: &mut World, t: f64, f: u32) {
    // The big ones off the launch meadow.
    let p0 = g.panel(0) as f64;
    let p1 = g.panel(1.min(last(g))) as f64;
    let lift = |t0: f64, t: f64, rise: f64| -> f64 { rise * ease((t - t0) / 3.2) };
    let a_inf = 0.45 + 0.55 * smooth(0.0, 1.7, t);
    let b_inf = 0.08 + 0.92 * smooth(0.6, 3.0, t);
    let a = Balloon {
        x: p0 + 30.0 + (t - 1.9).max(0.0) * 8.0,
        y: GROUND - 6.0 - lift(1.9, t, 17.0) + (t * 1.3).sin() * 0.5 * smooth(2.0, 4.0, t),
        r: 10.0,
        inf: a_inf,
        burn: burner(t, f, &[(0.4, 1.3), (1.7, 2.6), (6.0, 6.6), (9.8, 10.4)]),
        pat: Pattern::Gores(0xd83a3a, 0xf8c830),
        haze: 0.0,
    };
    let b = Balloon {
        x: p1 + 22.0 + (t - 3.3).max(0.0) * 4.0,
        y: GROUND - 6.0 - lift(3.3, t, 15.0) + (t * 1.1 + 1.0).sin() * 0.5 * smooth(3.4, 5.0, t),
        r: 9.0,
        inf: b_inf,
        burn: burner(t, f, &[(1.6, 2.2), (2.8, 3.8), (7.4, 8.0)]),
        pat: Pattern::Chevron(0x2a5ab8, 0xf4f4f8, 0x5ac8e8),
        haze: 0.0,
    };
    // A late rainbow one on the first meadow.
    let e = Balloon {
        x: p0 + 26.0 + (t - 7.4).max(0.0) * 3.0,
        y: GROUND - 6.0 - lift(7.4, t, 16.0) + (t * 1.2).sin() * 0.5 * smooth(7.5, 9.0, t),
        r: 9.0,
        inf: 0.05 + 0.95 * smooth(3.6, 6.8, t),
        burn: burner(t, f, &[(5.6, 6.4), (6.9, 7.8), (10.6, 11.2)]),
        pat: Pattern::Rainbow,
        haze: 0.0,
    };
    // The dog's polka-dot balloon.
    let (cx, cy) = dog_balloon(g, t);
    let c = Balloon {
        x: cx,
        y: cy,
        r: 7.0,
        inf: 0.1 + 0.9 * smooth(2.4, 4.2, t),
        burn: burner(t, f, &[(3.2, 4.4), (T_GRAB, T_GRAB + 1.3)]),
        pat: Pattern::Dots(0xf8e070, 0xe84a6a),
        haze: 0.0,
    };
    // Crews: two by each balloon until it goes, then waving it off.
    let crews = [
        (p0 + 30.0, 1.9, 0.0),
        (p1 + 22.0, 3.3, 0.0),
        (p1 + 46.0, T_LIFT_C, 0.0),
        (p0 + 26.0, 7.4, 4.5),
    ];
    for (bx, t0, from) in crews {
        if t < from {
            continue;
        }
        for (i, dx) in [-9.0, 7.0].iter().enumerate() {
            let x = js_round(bx + dx);
            let y = GROUND as i32 + [2, 4][i];
            let shirt = [0x3a5ab0, 0xd85a3a][i];
            put(g, 0x3a3040, x, y - 1);
            put(g, 0x3a3040, x + 1, y - 2);
            put(g, 0x3a3040, x, y - 2);
            g.p(shirt, x as f64, (y - 5) as f64, 2, 3);
            put(g, 0xe8b090, x, y - 6);
            put(g, [0x4a3020, 0xe8c050][i], x + 1, y - 6);
            put(g, [0x4a3020, 0xe8c050][i], x, y - 7);
            if t > t0 && ((t * 5.0) as i32 + i as i32) % 2 == 0 {
                put(g, 0xe8b090, x + if i == 0 { -1 } else { 2 }, y - 7);
            }
        }
    }
    balloon(g, &e, 0);
    balloon(g, &b, 0);
    balloon(g, &a, 0);
    balloon(g, &c, 0);
    pilot_and_dog(g, &c, t);
}

/// The pilot and the dog peeking over the little basket's rim.
fn pilot_and_dog(g: &mut World, c: &Balloon, t: f64) {
    let bw = (c.r * 0.55).round().max(2.0) as i32;
    let by = if c.inf < 1.0 {
        GROUND as i32 - 2
    } else {
        js_round(c.y) + 3
    };
    let bx = js_round(c.x) - bw / 2;
    // The pilot, cap and all.
    put(g, 0xe8b090, bx, by - 1);
    put(g, 0x2a4a8a, bx, by - 2);
    let lean = (T_DIP - 0.3..T_GRAB + 0.4).contains(&t);
    // A white terrier with a brown ear, leaning out over the rim to bite.
    let flap = ((t * 7.0) as i32) % 2;
    let pal = [
        ('e', 0x8a5a30),
        ('w', 0xf8f4ec),
        ('k', OUT),
        ('p', 0xf06a7a),
    ];
    let (rows, x, y): ([&str; 4], i32, i32) = if lean {
        (["e....", "ewwk.", "wwwwk", "w.wp."], bx + bw - 1, by - 1)
    } else {
        (["e....", "ewwk.", "wwwwk", ".ww.."], bx + bw - 3, by - 4)
    };
    g.spr(&rows, &pal, x as f64, y as f64, false);
    put(g, 0x8a5a30, x + flap, y - 1 + flap);
}

/// Washing line posts on the farm: (left x, right x, line y).
fn washing_line(g: &World) -> (f64, f64, f64) {
    let p = g.panel(last(g)) as f64;
    (p + 28.0, p + 50.0, 46.0)
}

const LAUNDRY: [(u32, i32, i32); 6] = [
    (0x4a7ad8, 4, 4),
    (0xe84a4a, 2, 3),
    (0xf8f8f0, 5, 5),
    (0xf0c840, 3, 4),
    (0x5ab85a, 2, 3),
    (0xf08ab8, 4, 4),
];

fn farm(g: &mut World, t: f64, dog: (f64, f64)) {
    let p = g.panel(last(g)) as f64;
    let (hx, hy) = (p + 6.0, 42.0);
    // The farmhouse: whitewashed, red tiles, smoke from the chimney.
    for k in 0..6 {
        let u = (t * 0.4 + k as f64 / 6.0) % 1.0;
        let x = hx + 15.0 + u * 8.0 + (u * 6.0 + k as f64).sin();
        blend(
            g,
            0xe8e4f0,
            js_round(x),
            js_round(hy - 10.0 - u * 16.0),
            0.6 * (1.0 - u),
        );
        blend(
            g,
            0xe8e4f0,
            js_round(x) + 1,
            js_round(hy - 10.0 - u * 16.0),
            0.4 * (1.0 - u),
        );
    }
    g.p(0x8a3a2a, hx + 15.0, hy - 10.0, 2, 4);
    for j in 0..6 {
        g.p(
            if j % 2 == 0 { 0xb84a34 } else { 0xd8603e },
            hx + j as f64 - 1.0,
            hy - 1.0 - j as f64,
            22 - 2 * j,
            1,
        );
    }
    g.p(OUT, hx - 1.0, hy, 22, 1);
    g.p(0xf4ecdc, hx, hy, 20, 8);
    g.p(0xd8ccc0, hx + 14.0, hy, 6, 8);
    let door = (T_GRAB + 0.5..SECS).contains(&t);
    g.p(
        if door { 0x2a1a14 } else { 0x6a4a30 },
        hx + 9.0,
        hy + 3.0,
        3,
        5,
    );
    for wx in [hx + 3.0, hx + 15.0] {
        g.p(0x4a6a8a, wx, hy + 2.0, 3, 3);
        put(g, 0xfff0c0, js_round(wx) + 2, js_round(hy) + 2);
    }
    // The farmer, out on the step shaking a fist.
    if t > T_GRAB + 0.7 {
        let x = js_round(hx) + 10;
        let y = js_round(hy) + 8;
        g.p(0x3a3a4a, x as f64, (y - 2) as f64, 2, 2);
        g.p(0x5a8a3a, x as f64, (y - 5) as f64, 2, 3);
        put(g, 0xe8b090, x, y - 6);
        put(g, 0xd8c070, x - 1, y - 7);
        put(g, 0xd8c070, x, y - 7);
        put(g, 0xd8c070, x + 1, y - 7);
        let shake = ((t * 8.0) as i32) % 2;
        put(g, 0x5a8a3a, x + 2, y - 5);
        put(g, 0xe8b090, x + 2, y - 6 - shake);
    }
    // The washing line, and then the laundry's flight.
    let (l0, l1, ly) = washing_line(g);
    let gone = smooth(T_GRAB, T_GRAB + 0.5, t);
    for (px, tip) in [(l0, -1.0), (l1, 1.0)] {
        let lean = gone * tip;
        line(g, 0x6a5040, px, ly + 7.0, px + lean, ly - 1.0);
    }
    let on_line = |u: f64| -> (f64, f64) {
        (
            l0 + (l1 - l0) * u,
            ly + 2.5 * (u * PI).sin() + (t * 2.0 + u * 5.0).sin() * 0.3,
        )
    };
    let (dx, dy) = dog;
    let by = dy + 4.0;
    let garland = |u: f64| -> (f64, f64) {
        let sag = (u * PI).sin();
        let sway = (t * 2.2).sin() * 1.5 * sag;
        (dx + (u - 0.5) * 24.0 + sway, by + 1.0 + 5.0 * sag)
    };
    let at = |u: f64| -> (f64, f64) {
        if gone <= 0.0 {
            return on_line(u);
        }
        let (a, b) = (on_line(u), garland(u));
        (a.0 + (b.0 - a.0) * gone, a.1 + (b.1 - a.1) * gone)
    };
    let n = 24;
    let mut prev = at(0.0);
    for i in 1..=n {
        let p = at(i as f64 / n as f64);
        line(g, mix(0xe8e0d0, 0x6a5040, gone), prev.0, prev.1, p.0, p.1);
        prev = p;
    }
    if gone > 0.0 {
        // The dog holds the middle; the ends are tied to the basket.
        let (a, b) = (at(0.0), at(1.0));
        line(g, 0x6a5040, dx - 1.0, by, a.0, a.1);
        line(g, 0x6a5040, dx + 1.0, by, b.0, b.1);
    }
    for (i, &(c, w, h)) in LAUNDRY.iter().enumerate() {
        let u = (i as f64 + 0.7) / (LAUNDRY.len() as f64 + 0.4);
        let (x, y) = at(u);
        let flap = (t * 4.0 + i as f64).sin() * gone;
        for j in 0..h {
            let skew = (flap * j as f64 * 0.5).round();
            g.p(c, x - (w / 2) as f64 + skew, y + j as f64, w, 1);
        }
        g.p(
            mix(c, OUT, 0.35),
            x - (w / 2) as f64,
            y + (h - 1) as f64,
            w,
            1,
        );
        put(g, 0x8a6a4a, js_round(x) - w / 2, js_round(y));
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    land(g, t);
    far_balloons(g, t, f);
    mist(g, t);
    farm(g, t, dog_balloon(g, t));
    balloons(g, t, f);
}

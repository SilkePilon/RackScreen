//! Desert caravan: a blazing noon over the dunes, far mesas wobbling in
//! the heat and the wind lifting plumes of sand off the crests. Three
//! camels, outlined dark against the bright sand, plod along roped nose to
//! tail: an indigo rider, a red rider and a little pack camel piled with
//! rugs and pots. A shimmering lake appears ahead; the caravan hurries
//! towards it, the lead rider pointing, and it melts into heat haze as
//! they arrive. So when a real oasis comes up, palms and all, the lead
//! rider waves it off and walks on. The pack camel doesn't: it digs its
//! heels in at the water, the rope snaps taut, the whole line jerks to a
//! halt, and it kneels down to drink while the riders look back in dismay.

use super::fx::{blend, get, glow, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// The ground line the camels walk on.
const FEET: f64 = 48.0;
/// Stage distance between roped camels.
const SPACING: f64 = 19.0;
/// The horizon.
const HZ: i32 = 33;

const OUT: u32 = 0x2a1608;

/// A dromedary facing right, two strides; body `b`, belly `l`.
const CAMEL: [&[&str]; 2] = [
    &[
        "..............kkk.",
        ".............kbbbk",
        ".............kbebk",
        "............kbbbbk",
        "......kk....kbkkk.",
        ".....kbbk...kbk...",
        "....kbbbbk..kbk...",
        "...kbbbbbbkkbbk...",
        "..kbbbbbbbbbbbk...",
        ".kbbbbbbbbbbbk....",
        "kbbbbbbbbbbbbk....",
        "kblllllllllbk.....",
        ".kkbkkkkkbkk......",
        "..kb.kb..kb.kb....",
        "..kb..kb.kb..kb...",
        ".kk...kk.kk...kk..",
    ],
    &[
        "..............kkk.",
        ".............kbbbk",
        ".............kbebk",
        "............kbbbbk",
        "......kk....kbkkk.",
        ".....kbbk...kbk...",
        "....kbbbbk..kbk...",
        "...kbbbbbbkkbbk...",
        "..kbbbbbbbbbbbk...",
        ".kbbbbbbbbbbbk....",
        "kbbbbbbbbbbbbk....",
        "kblllllllllbk.....",
        ".kkbkkkkkbkk......",
        "...kbkb...kbkb....",
        "...kbkb...kbkb....",
        "..kkkk...kkkk.....",
    ],
];
/// Kneeling, neck down to the water.
const KNEEL: &[&str] = &[
    "..................",
    "..................",
    "..................",
    "..................",
    "......kk..........",
    ".....kbbk.........",
    "....kbbbbk........",
    "...kbbbbbbkk......",
    "..kbbbbbbbbbkk....",
    ".kbbbbbbbbbbbbk...",
    "kbbbbbbbbbbbbbbk..",
    "kblllllllllbk.kbk..",
    ".kkkkkkkkkkkk..kbk.",
    "...............kbk.",
    "...............kbbk",
    "...............kek.",
];

/// A rider facing right: robe `r`, trim `t`, wrap `w`, skin `s`.
const RIDER: [&[&str]; 3] = [
    &["..ww.", ".wws.", ".rrr.", "rrrr.", ".trr.", "..r.."],
    // Pointing ahead.
    &["..ww.", ".wws.", ".rrrss", "rrrr.", ".trr.", "..r.."],
    // Arms up in dismay.
    &["s.ww.s", "rwwsr", ".rrr.", "rrrr.", ".trr.", "..r.."],
];

/// Sand colours: light, mid, shadow.
const SAND: [u32; 3] = [0xf6daa0, 0xeac080, 0xc89458];

struct Caravan {
    /// The lead camel's stage position.
    lead: f64,
    /// When the line stopped at the oasis, if it has.
    stopped: Option<f64>,
}

fn speed(t: f64) -> f64 {
    if t < 4.0 {
        27.0
    } else if t < 5.6 {
        38.0
    } else if t < 6.6 {
        21.0
    } else {
        28.0
    }
}

fn oasis_panel(g: &World) -> u32 {
    g.layout.panels - 1
}
fn mirage_panel(g: &World) -> u32 {
    if g.layout.panels >= 4 {
        2
    } else {
        1
    }
}

/// Where the lead must stop so the pack camel stands at the water.
fn stop_at(g: &World) -> f64 {
    stage_at(g, oasis_panel(g), 6.0) + 2.0 * SPACING
}

fn caravan(g: &World, t: f64) -> Caravan {
    let end = stop_at(g);
    let (mut s, mut tt): (f64, f64) = (-14.0, 0.0);
    let dt: f64 = 1.0 / 24.0;
    while tt < t {
        let step = speed(tt) * dt.min(t - tt);
        if s + step >= end {
            let at = tt + (end - s) / speed(tt);
            return Caravan {
                lead: end,
                stopped: Some(at),
            };
        }
        s += step;
        tt += dt;
    }
    Caravan {
        lead: s,
        stopped: None,
    }
}

/// When the lead camel reaches the mirage (it melts away then).
fn t_mirage(g: &World) -> f64 {
    let s = stage_at(g, mirage_panel(g), 18.0);
    let mut t = 0.0;
    while t < SECS && caravan(g, t).lead < s {
        t += 1.0 / 24.0;
    }
    t
}

fn sky(g: &mut World, t: f64) {
    let w = g.w();
    vgrad(
        g,
        &[0x2a78d0, 0x4a98e0, 0x88c4ec, 0xc8e4f0, 0xf4eedc],
        0,
        w,
        0,
        HZ + 8,
    );
    let (sx, sy) = (g.panel(0) as f64 + 40.0, 11.0);
    glow(g, 0xfff8e0, sx, sy, 34.0, 0.45);
    glow(g, 0xffffff, sx, sy, 12.0, 0.7);
    g.disc(0xfff8e8, sx, sy, 5);
    g.disc(0xffffff, sx, sy, 4);
    // Wisps of high cloud.
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let span = w as f64 + 40.0;
        let x = (rnd(kf + 3.0) * span + t * 2.0) % span - 20.0;
        let y = 4.0 + rnd(kf + 4.0) * 14.0;
        let len = 10 + (rnd(kf + 5.0) * 16.0) as i32;
        for i in 0..len {
            blend(g, 0xffffff, js_round(x) + i, js_round(y), 0.55);
            if i > 3 && i < len - 2 {
                blend(g, 0xffffff, js_round(x) + i - 2, js_round(y) - 1, 0.3);
            }
        }
    }
}

/// Far mesas and dunes, the near ones lit from the sun's side.
fn dunes(g: &mut World) {
    let w = g.w();
    // Flat-topped buttes in the haze, lit on the sun's side.
    for k in 0..g.spread(7) {
        let kf = k as f64;
        let cx = rnd(kf + 30.0) * w as f64;
        let half = 6.0 + rnd(kf + 31.0) * 10.0;
        let h = 3 + (rnd(kf + 32.0) * 7.0) as i32;
        for r in 0..=h {
            let y = HZ - h + r;
            let hw = half + r as f64 * 0.8 + if r > h - 2 { 2.0 } else { 0.0 };
            for x in js_round(cx - hw)..js_round(cx + hw) {
                let u = (x as f64 - (cx - hw)) / (2.0 * hw);
                let c = if r == 0 {
                    0xf0d0b8
                } else if u < 0.35 {
                    0xe4b8a0
                } else if on(x, y, 0.4) {
                    0xd0a088
                } else {
                    0xc89480
                };
                put(g, mix(c, 0xc8e4f0, 0.25), x, y);
            }
        }
    }
    let layers: [(f64, f64, f64, u32, u32); 3] = [
        (HZ as f64 + 2.0, 2.0, 0.9, 0xecc890, 0xf4d8a8),
        (40.0, 4.0, 2.1, 0xe2b474, 0xf2d094),
        (45.0, 3.0, 4.3, SAND[1], SAND[0]),
    ];
    for (base, amp, seed, shade, lit) in layers {
        for x in 0..w {
            let xf = x as f64;
            let h = |x: f64| {
                base - amp * ((x * 0.04 + seed).sin() + 0.5 * (x * 0.09 + seed * 2.0).sin())
            };
            let top = js_round(h(xf));
            // Slopes facing the sun (to the left) are bright, the far
            // sides fall into shade with a crisp crest line.
            let facing = h(xf + 1.0) > h(xf - 1.0) - 0.05;
            for y in top..60 {
                let d = (y - top) as f64;
                let c = if facing || on(x, y, (d / 6.0).min(0.8)) {
                    lit
                } else {
                    shade
                };
                put(g, c, x, y);
            }
            put(g, if facing { 0xfff0c8 } else { shade }, x, top);
        }
    }
    // Ripples in the near sand.
    for y in 50..60 {
        for x in 0..w {
            let v = ((x as f64) * 0.35 + (y as f64) * 1.7 + ((x as f64) * 0.05).sin() * 3.0).sin();
            if v > 0.82 {
                put(g, SAND[1], x, y);
            } else if v < -0.9 {
                put(g, 0xfff0c8, x, y);
            }
        }
    }
    // Pebbles and dry shrubs.
    for k in 0..g.spread(18) {
        let kf = k as f64;
        let x = (rnd(kf + 60.0) * w as f64) as i32;
        let y = 50 + (rnd(kf + 61.0) * 9.0) as i32;
        if rnd(kf + 62.0) > 0.6 {
            for i in -2..=2 {
                put(g, 0x8a6a3a, x + i, y - (2 - i.abs()));
            }
            put(g, 0x6a5030, x, y);
        } else {
            put(g, 0xa87a48, x, y);
            put(g, 0xfff0c8, x, y - 1);
        }
    }
}

/// The wind: streaks of sand blowing right, plumes off the dune crests.
fn wind(g: &mut World, t: f64) {
    let w = g.w() as f64;
    for k in 0..g.spread(40) {
        let kf = k as f64;
        let speed = 60.0 + rnd(kf + 90.0) * 50.0;
        let x = (rnd(kf + 91.0) * (w + 40.0) + t * speed) % (w + 40.0) - 20.0;
        let y = 36.0 + rnd(kf + 92.0) * 22.0 + (t * 3.0 + kf).sin();
        let len = 3 + (rnd(kf + 93.0) * 6.0) as i32;
        for i in 0..len {
            blend(
                g,
                0xfff4d8,
                js_round(x) - i,
                js_round(y),
                0.5 * (1.0 - i as f64 / len as f64),
            );
        }
    }
    // Spindrift: puffs lifting off the near crest.
    for k in 0..g.spread(10) {
        let kf = k as f64;
        let x0 = rnd(kf + 95.0) * w;
        let u = (t * 0.7 + rnd(kf + 96.0)) % 1.0;
        for j in 0..8 {
            let jf = j as f64;
            let x = x0 + u * 18.0 + jf * 1.5;
            let y = 41.0 - u * 4.0 - (jf * 0.8).sin() * 1.5;
            blend(
                g,
                0xfff4e0,
                js_round(x),
                js_round(y),
                0.4 * (1.0 - u) * (1.0 - jf / 8.0),
            );
        }
    }
}

/// Heat shimmer: rows near the horizon wobble sideways.
fn shimmer(g: &mut World, t: f64) {
    let w = g.w();
    for y in HZ - 9..HZ + 6 {
        let off = js_round(((y as f64) * 1.3 + t * 7.0).sin() * 0.9);
        if off == 0 {
            continue;
        }
        let row: Vec<u32> = (0..w).map(|x| get(g, x - off, y)).collect();
        for (x, c) in row.into_iter().enumerate() {
            put(g, c, x as i32, y);
        }
    }
}

/// The mirage: a lake that shimmers in and melts away.
fn mirage(g: &mut World, t: f64, tm: f64) {
    let a = ((t - (tm - 2.2)) / 0.8).clamp(0.0, 1.0) * (1.0 - ((t - tm) / 0.6).clamp(0.0, 1.0));
    if a <= 0.0 {
        return;
    }
    let x0 = g.panel(mirage_panel(g)) + 18;
    for y in 39..45 {
        let mid = 1.0 - ((y as f64 - 41.5) / 3.0).abs();
        for x in x0..x0 + 34 {
            let edge = ((x - x0).min(x0 + 34 - x) as f64 / 6.0).min(1.0);
            let wob = ((x as f64) * 0.4 + t * 9.0 + y as f64).sin() * 0.25;
            let k = a * mid * edge + wob * a;
            if on(x, y, k) {
                // The sky, upside down.
                let c = mix(0x88c4ec, 0x4a98e0, (y - 39) as f64 / 6.0);
                put(g, c, x, y);
            }
        }
    }
    // Glints.
    for k in 0..4 {
        let kf = k as f64 + (t * 4.0).floor();
        put(
            g,
            0xffffff,
            x0 + 6 + (rnd(kf) * 22.0) as i32,
            40 + (rnd(kf + 1.0) * 4.0) as i32,
        );
    }
}

fn palm(g: &mut World, x: f64, base: f64, h: f64, lean: f64, t: f64) {
    for i in 0..h as i32 {
        let u = i as f64 / h;
        let px = x + lean * u * u * h * 0.5;
        let c = if i % 3 == 0 { 0x6a4424 } else { 0x8a5a30 };
        g.p(c, px, base - i as f64, 2, 1);
        put(g, OUT, js_round(px) - 1, js_round(base) - i);
    }
    let (tx, ty) = (x + lean * h * 0.5, base - h);
    for k in 0..7 {
        let a = -PI + k as f64 / 6.0 * PI + (t * 1.8 + k as f64).sin() * 0.08;
        let len = 7.0 + (k % 2) as f64 * 2.0;
        for j in 0..len as i32 {
            let jf = j as f64;
            let droop = (jf / len).powi(2) * 4.0;
            let (fx, fy) = (tx + a.cos() * jf, ty + a.sin() * jf * 0.6 + droop);
            put(
                g,
                if j % 2 == 0 { 0x2e7a3a } else { 0x3a9a48 },
                js_round(fx),
                js_round(fy),
            );
            put(g, 0x1e5a2a, js_round(fx), js_round(fy) + 1);
        }
    }
    put(g, 0x6a3a1a, js_round(tx), js_round(ty) + 1);
    put(g, 0x6a3a1a, js_round(tx) + 1, js_round(ty) + 2);
}

fn oasis(g: &mut World, t: f64, f: u32) {
    let x0 = g.panel(oasis_panel(g)) as f64;
    palm(g, x0 + 6.0, 47.0, 20.0, 0.5, t);
    palm(g, x0 + 44.0, 46.0, 17.0, -0.6, t + 1.0);
    // Green bushes round the water.
    for (dx, dy) in [(9.0, 49.0), (34.0, 50.0), (39.0, 48.0)] {
        g.disc(0x2a6a30, x0 + dx, dy, 2);
        g.disc(0x4a9a40, x0 + dx - 1.0, dy - 1.0, 1);
    }
    // The pool.
    for y in 50..56 {
        let hw = 12.0 * (1.0 - ((y as f64 - 52.5) / 3.6).powi(2)).max(0.0).sqrt();
        for x in js_round(x0 + 24.0 - hw)..js_round(x0 + 24.0 + hw) {
            let c = if y == 50 {
                0xa8e0f0
            } else {
                mix(0x4aa8d8, 0x2a78b8, (y - 50) as f64 / 5.0)
            };
            put(g, c, x, y);
        }
    }
    for k in 0..3 {
        let kf = k as f64 + (f / 4) as f64 * 3.0;
        put(
            g,
            0xffffff,
            js_round(x0 + 16.0 + rnd(kf) * 16.0),
            51 + (rnd(kf + 1.0) * 4.0) as i32,
        );
    }
}

fn camel_pal(n: usize) -> [(char, u32); 4] {
    let (b, l) = [
        (0xb07840, 0xd8a868),
        (0x9a6434, 0xc89058),
        (0xd0a060, 0xeccc8c),
    ][n];
    [('k', OUT), ('b', b), ('l', l), ('e', 0x101010)]
}

fn camels(g: &mut World, t: f64, f: u32) {
    let c = caravan(g, t);
    let stop = c.stopped;
    let since = stop.map(|s| t - s).unwrap_or(-1.0);
    // The jolt when the rope snaps taut.
    let jolt = if since >= 0.0 {
        -2.5 * (since * 14.0).sin() * (-since * 5.0).exp()
    } else {
        0.0
    };
    let tm = t_mirage(g);
    let mut prev_tail: Option<(f64, f64)> = None;
    for n in 0..3usize {
        let s = c.lead - n as f64 * SPACING
            + if n < 2 {
                jolt * (2 - n) as f64 * 0.5
            } else {
                0.0
            };
        let x = stage_x(g, s);
        let walking = stop.is_none();
        let ph = if walking {
            ((f / 3) as usize + n) % 2
        } else {
            0
        };
        let bob = if walking && ph == 1 { 1.0 } else { 0.0 };
        let y = FEET - 16.0 + bob;
        let pal = camel_pal(n);
        // Shadow on the sand, thrown right by the sun.
        for i in 0..14 {
            blend(g, 0xa87040, js_round(x) + 2 + i, FEET as i32, 0.45);
            if (2..12).contains(&i) {
                blend(g, 0xa87040, js_round(x) + 3 + i, FEET as i32 + 1, 0.3);
            }
        }
        let kneel = n == 2 && since > 0.6;
        // Rope from this camel's head back to... drawn as head-to-tail.
        let head = (x + 15.0, y + 4.0);
        if let Some((tx, ty)) = prev_tail {
            let taut = since >= 0.0;
            let sag = if taut { 0.0 } else { 3.0 };
            let steps = 12;
            for i in 0..=steps {
                let u = i as f64 / steps as f64;
                let px = tx + (head.0 - tx) * u;
                let py = ty + (head.1 - ty) * u + (u * PI).sin() * sag;
                put(g, 0x7a5028, js_round(px), js_round(py));
            }
        }
        if kneel {
            g.spr(
                KNEEL,
                &pal,
                x,
                y + 2.0 + if (f / 4).is_multiple_of(2) { 0.0 } else { 0.5 },
                false,
            );
            // Slurping ripples.
            let u = (t * 2.0) % 1.0;
            g.ring(0xd8f4ff, x + 17.0, FEET + 4.0, 1.0 + u * 2.5);
        } else {
            g.spr(CAMEL[ph], &pal, x, y, false);
        }
        let dy = if kneel { 2.0 } else { 0.0 };
        // Saddle blanket.
        for i in 0..6 {
            let c = if i % 2 == 0 {
                [0xc83a3a, 0x3a5ac8, 0xe8a030][n]
            } else {
                0xf4e0b0
            };
            g.p(c, x + 4.0 + i as f64, y + 8.0 + dy, 1, 3);
        }
        if n < 2 {
            let pose = if since > 0.2 {
                2
            } else if n == 0 && (tm - 1.8..tm).contains(&t) {
                1
            } else {
                0
            };
            let (r, trim) = if n == 0 {
                (0x2a3a8a, 0xe8c040)
            } else {
                (0xb83030, 0xf0e0c0)
            };
            let rp = [('r', r), ('t', trim), ('w', 0xf4f0e8), ('s', 0x8a5a3a)];
            // Riders turn round to look back once the line stops.
            g.spr(RIDER[pose], &rp, x + 5.0, y + 2.0 + dy, since > 0.2);
            if since > 0.2 && since < 1.6 && n == 0 {
                bang(g, x + 7.0, y - 6.0);
            }
            // The lead rider waves the real oasis off.
            let ox = g.panel(oasis_panel(g)) as f64;
            if n == 0 && stop.is_none() && (x - ox).abs() < 30.0 && t > tm + 1.0 {
                let wv = if (f / 2).is_multiple_of(2) { -1.0 } else { 1.0 };
                put(g, 0x8a5a3a, js_round(x + 9.0), js_round(y + 2.0 + wv));
                put(g, 0x2a3a8a, js_round(x + 8.0), js_round(y + 4.0));
            }
        } else {
            // The pack camel's load: rolled rugs, a pot and a bundle.
            g.p(OUT, x + 3.0, y + 4.0 + dy, 7, 5);
            g.p(0xc83a3a, x + 4.0, y + 5.0 + dy, 5, 1);
            g.p(0xe8a030, x + 4.0, y + 6.0 + dy, 5, 1);
            g.p(0x3a5ac8, x + 4.0, y + 7.0 + dy, 5, 1);
            g.disc(0xb8683a, x + 6.0, y + 2.0 + dy, 1);
            put(g, OUT, js_round(x + 6.0), js_round(y + dy));
        }
        if n == 0 && (tm - 0.3..tm + 0.9).contains(&t) {
            // Where did the lake go?
            question(g, x + 9.0, y - 7.0);
        }
        prev_tail = Some((x + 1.0, y + 10.0 + dy));
    }
    // Footprints behind the caravan.
    let last = stage_x(g, c.lead - 2.0 * SPACING);
    for k in 0..40 {
        let fx = last - 2.0 - k as f64 * 3.0;
        if fx < 0.0 {
            break;
        }
        put(g, SAND[2], js_round(fx), FEET as i32 + 2 + (k % 2));
    }
}

fn bang(g: &mut World, x: f64, y: f64) {
    g.p(OUT, x - 1.0, y - 1.0, 3, 7);
    g.p(0xffe040, x, y, 1, 3);
    put(g, 0xffe040, js_round(x), js_round(y) + 4);
}

fn question(g: &mut World, x: f64, y: f64) {
    let (qx, qy) = (js_round(x), js_round(y));
    for (dx, dy) in [(0, 0), (1, -1), (2, 0), (2, 1), (1, 2), (1, 4)] {
        put(g, OUT, qx + dx + 1, qy + dy);
        put(g, 0xffffff, qx + dx, qy + dy);
    }
}

const SKULL: &[&str] = &[
    "k.....k", "wk...kw", ".wwwww.", ".wkwkw.", "..www..", "..wkw..",
];
const LIZARD: [&[&str]; 2] = [
    &["......gg", "gggggggk", ".g..g..."],
    &["......gg", "gggggggk", "..g..g.."],
];

/// Desert life between the stops: circling vultures, an old skull, a lizard.
fn wildlife(g: &mut World, t: f64, f: u32) {
    let mid = if g.layout.panels >= 4 { 1 } else { 0 };
    let (cx, cy) = (g.panel(mid) as f64 + 30.0, 13.0);
    for k in 0..2 {
        let a = t * 0.9 + k as f64 * PI;
        let (x, y) = (cx + a.cos() * 13.0, cy + a.sin() * 4.0);
        let up = (f / 4 + k).is_multiple_of(3);
        let (wx, wy) = (js_round(x), js_round(y));
        for i in -3..=3i32 {
            let lift = if up { -(i.abs() / 2) } else { i.abs() / 3 };
            put(g, 0x3a2a30, wx + i, wy + lift);
        }
        put(g, 0x3a2a30, wx, wy + 1);
        put(g, 0xd8b8a8, wx + if a.sin() > 0.0 { -1 } else { 1 }, wy - 1);
    }
    let sp = [('k', 0x5a4a3a), ('w', 0xf4ecdc)];
    g.spr(SKULL, &sp, g.panel(mid) as f64 + 40.0, 51.0, false);
    blend(g, SAND[2], g.panel(mid) + 39, 57, 0.6);
    // A lizard basks until the camels come, then darts off.
    let lp = g.panel(if g.layout.panels >= 4 { 2 } else { 1 }) as f64;
    let lead = stage_x(g, caravan(g, t).lead) + 16.0;
    let near = lead - (lp + 18.0);
    let run = if near > -6.0 { (near + 6.0) * 2.5 } else { 0.0 };
    let lx = lp + 20.0 + run;
    let ph = if run > 0.0 { (f % 2) as usize } else { 0 };
    let bob = if run == 0.0 && (f / 6).is_multiple_of(4) {
        -1.0
    } else {
        0.0
    };
    g.spr(
        LIZARD[ph],
        &[('g', 0x6a8a3a), ('k', 0x1a1a10)],
        lx,
        54.0 + bob,
        false,
    );
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t);
    dunes(g);
    mirage(g, t, t_mirage(g));
    shimmer(g, t);
    oasis(g, t, f);
    wildlife(g, t, f);
    camels(g, t, f);
    wind(g, t);
}

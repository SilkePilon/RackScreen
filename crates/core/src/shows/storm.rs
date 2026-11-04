//! Thunderstorm: a grey afternoon over a little town turns dark as dithered
//! storm clouds pile up; the street lamp flickers on, windows light, rain
//! sheets slant in the wind and splash in the puddles. A walker in a yellow
//! raincoat holds on to a red umbrella until a gust turns it inside out and
//! tears it away; lightning strikes the church spire and lights the whole
//! street for a frame while the walker runs for the bus shelter. The rain
//! eases, the clouds blow off, a rainbow arcs over the last screens and the
//! black cat that sheltered under the porch bench climbs into the umbrella,
//! now floating upside down in the big puddle, and sails it like a boat.

use super::fx::{blend, get, glow, line, mix, on, put, shake, stage_at, stage_x, tint, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.5;

const T_FLIP: f64 = 4.3;
const T_TORN: f64 = 5.0;
const T_SAFE: f64 = 6.3;
const T_LAND: f64 = 6.7;
const T_CAT: f64 = 8.9;
const T_HOP: f64 = 9.7;
const T_ABOARD: f64 = 10.1;
const T_SEE: f64 = 10.4;
/// Lightning: (time, strikes the spire).
const BOLTS: [(f64, bool); 3] = [(3.25, false), (5.75, true), (7.25, false)];

const GROUND: i32 = 41;
/// Row of the puddles' centres.
const PY: f64 = (GROUND + 8) as f64;

const COAT: &[(char, u32)] = &[
    ('k', 0x3a2a18),
    ('y', 0xf2c632),
    ('Y', 0xc8961e),
    ('s', 0xf0c8a0),
    ('e', 0x2a1a1a),
    ('b', 0x2a5a4a),
];
/// The walker, facing right: hood, coat; the legs come separately.
const WALKER: &[&str] = &[
    "..kkk..", ".kyyyk.", ".kyssk.", ".kysek.", "kyyyyyk", "kyyyyYk", "kyyyyYk", "kyyyyYk",
    ".kyyYk.",
];
const LEGS: [&[&str]; 3] = [
    &[".kb.bk.", ".bb.bb."],
    &["..kbk..", "..bbb.."],
    &["..kbk..", "..b.b.."],
];

const HEART: &[&str] = &["h.h.", "hhhh", "hhh.", ".h.."];
const CATP: &[(char, u32)] = &[
    ('k', 0x1e1a24),
    ('e', 0xe8e050),
    ('h', 0x3a3448),
    ('w', 0xf0ece8),
    ('p', 0xf09aa0),
];
/// The cat sitting, facing right.
const CAT_SIT: &[&str] = &[
    "....k.k", "....kpk", "....kek", "...hkww", "t..kkwk", "k.kkkw.", "kkkkkw.", ".kk.ww.",
];
/// The cat walking left, two frames.
const CAT_WALK: [&[&str]; 2] = [
    &[
        "k.k......",
        "kkk.....t",
        "ekkkkkkk.",
        "wwkkkkkk.",
        ".w.k..kk.",
    ],
    &[
        "k.k......",
        "kkk....t.",
        "ekkkkkkk.",
        "wwkkkkkk.",
        "..wk.k.k.",
    ],
];

fn last(g: &World) -> u32 {
    g.layout.panels.saturating_sub(1)
}
/// The screen with the church and the bus shelter.
fn shelter_screen(g: &World) -> u32 {
    g.layout.panels.saturating_sub(2)
}
/// The screen where the umbrella flips.
fn flip_screen(g: &World) -> u32 {
    if g.layout.panels >= 4 {
        1
    } else {
        0
    }
}

fn smooth(a: f64, b: f64, t: f64) -> f64 {
    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// How dark the storm is (0 clear, 1 black clouds).
fn dark(t: f64) -> f64 {
    0.3 + 0.7 * smooth(0.2, 2.6, t) - smooth(7.4, 9.4, t)
}
fn rain(t: f64) -> f64 {
    smooth(0.8, 2.6, t) * (1.0 - smooth(7.2, 8.9, t))
}
/// Sideways push of the wind, px per px fallen.
fn wind(t: f64) -> f64 {
    0.25 + 0.35 * smooth(1.5, 3.5, t) + 0.5 * (smooth(4.0, 4.4, t) - smooth(5.2, 6.2, t))
        - 0.4 * smooth(7.5, 9.0, t)
}

/// How bright the lightning lights the scene this frame.
fn flash(t: f64) -> f64 {
    let mut k: f64 = 0.0;
    for (tb, _) in BOLTS {
        let d = t - tb;
        if (0.0..1.0 / 12.0).contains(&d) {
            k = k.max(0.75);
        } else if (2.0 / 12.0..3.0 / 12.0).contains(&d) {
            k = k.max(0.45);
        } else if (1.0 / 12.0..4.0 / 12.0).contains(&d) {
            k = k.max(0.15);
        }
    }
    k
}

fn vnoise(x: f64, y: f64, seed: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |a: f64, b: f64| rnd(a * 57.0 + b * 131.0 + seed);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

fn sky(g: &mut World, t: f64, lit: f64) {
    let w = g.w();
    let d = dark(t);
    let clear = [0x3f86d2, 0x7ab8e8, 0xbfe0f2];
    let storm = [0x161a26, 0x262c3c, 0x3c4458];
    let stops: Vec<u32> = (0..3)
        .map(|i| mix(mix(clear[i], storm[i], d), 0xb8c2ec, lit))
        .collect();
    vgrad(g, &stops, 0, w, 0, 48);
    // The sun breaks through on the last screen.
    let sun = 1.0 - d.min(1.0);
    if sun > 0.05 {
        let (sx, sy) = ((g.panel(last(g)) + 44) as f64, 10.0);
        glow(g, 0xfff2b8, sx, sy, 22.0, 0.6 * sun);
        if sun > 0.4 {
            g.disc(0xfff4c0, sx, sy, 4);
            g.disc(0xfffbea, sx, sy, 3);
        }
    }
    rainbow(g, t);
    // Two layers of cloud: a high deck and fast low scud.
    let drift = t * 3.0 + smooth(7.0, 11.5, t) * 60.0;
    let cov = 0.25 + 0.75 * d;
    let tones = [
        mix(mix(0xffffff, 0x6a7290, d), 0xf0f2ff, lit),
        mix(mix(0xe4ecf4, 0x474e66, d), 0xc8d0f4, lit),
        mix(mix(0xc4d0de, 0x2c3144, d), 0x9aa4d4, lit),
        mix(mix(0xa8b4c8, 0x1a1d2a, d), 0x6a74a8, lit),
    ];
    for y in 0..34 {
        let bias = 1.0 - y as f64 / 34.0;
        for x in 0..w {
            let nx = (x as f64 - drift) / 20.0;
            let n = 0.65 * vnoise(nx, y as f64 / 7.0, 1.0)
                + 0.35 * vnoise(nx * 2.3, y as f64 / 3.5, 9.0);
            let v = n + bias * 0.55 + cov * 0.6 - 1.0;
            if v <= 0.0 {
                continue;
            }
            // Thicker cloud is darker; the rim is lit.
            let u = (v * 7.0).min(2.99);
            let i = u.floor() as usize;
            let c = if on(x, y, u - i as f64) {
                tones[i + 1]
            } else {
                tones[i]
            };
            put(g, c, x, y);
        }
    }
    // Low scud racing along under the deck.
    for k in 0..g.spread(7) {
        let kf = k as f64;
        let span = w as f64 + 50.0;
        let x = (rnd(kf + 40.0) * span + t * (14.0 + 10.0 * wind(t))) % span - 25.0;
        let y = 22.0 + rnd(kf + 41.0) * 8.0;
        let a = 0.5 * d * (1.0 - smooth(8.0, 9.5, t));
        for i in -8..8 {
            for j in -1..2 {
                if on(
                    js_round(x) + i,
                    js_round(y) + j,
                    a * (1.0 - (i as f64 / 8.0).abs()),
                ) {
                    put(g, tones[2], js_round(x) + i, js_round(y) + j);
                }
            }
        }
    }
}

fn rainbow(g: &mut World, t: f64) {
    let k = smooth(8.4, 9.8, t) * (1.0 - 0.3 * smooth(11.0, 11.5, t));
    if k <= 0.0 {
        return;
    }
    let l = last(g);
    let cx = if l >= 1 {
        (g.px0(l) - g.layout.gap as i32 / 2) as f64
    } else {
        30.0
    };
    let cy = 78.0;
    let cols = [0xe84a4a, 0xf09a3a, 0xf2e04a, 0x5ac85a, 0x4a8ae0, 0x8a5ad0];
    for y in 0..48 {
        for x in (cx as i32 - 80)..(cx as i32 + 80) {
            let r = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
            let band = 66.0 - r;
            if (0.0..12.0).contains(&band) {
                let i = (band / 2.0) as usize;
                let edge = 1.0 - ((band - 6.0).abs() / 6.0).powi(4);
                blend(g, cols[i], x, y, 0.55 * k * edge);
            }
        }
    }
}

/// Far town roofs, hazy behind the street.
fn skyline(g: &mut World, t: f64) {
    let w = g.w();
    let d = dark(t);
    let far = mix(0x9ab4cc, 0x2a3040, d);
    let mut x = 0;
    let mut k = 0.0;
    while x < w {
        let bw = 6 + (rnd(k + 200.0) * 10.0) as i32;
        let top = 30 + (rnd(k + 201.0) * 8.0) as i32;
        g.p(far, x as f64, top as f64, bw, 48 - top);
        for wx in (x + 1..x + bw - 1).step_by(3) {
            for wy in (top + 2..44).step_by(4) {
                if rnd(wx as f64 * 3.1 + wy as f64 + k) > 0.7 {
                    let lit = d > 0.6 && rnd(wx as f64 + wy as f64 * 7.0) > 0.75;
                    put(
                        g,
                        if lit {
                            0xd8b060
                        } else {
                            mix(far, 0xffffff, 0.15)
                        },
                        wx,
                        wy,
                    );
                }
            }
        }
        x += bw + (rnd(k + 202.0) * 4.0) as i32;
        k += 1.0;
    }
}

/// A row house with a gable and four windows that light up in the gloom.
fn house(g: &mut World, x: f64, top: f64, wall: u32, roof: u32, t: f64, seed: f64) {
    let d = dark(t);
    let wall = mix(wall, 0x2a2a38, d * 0.6);
    let roof = mix(roof, 0x1a1a24, d * 0.5);
    g.p(wall, x, top, 18, GROUND - top as i32);
    for r in 0..6 {
        g.p(
            roof,
            x - 1.0 + r as f64,
            top - 6.0 + r as f64,
            20 - 2 * r,
            1,
        );
    }
    g.p(mix(roof, 0x000000, 0.3), x - 1.0, top, 20, 1);
    g.p(mix(wall, 0x000000, 0.25), x + 15.0, top - 9.0, 3, 5);
    for (i, (wx, wy)) in [(3.0, 4.0), (11.0, 4.0), (3.0, 13.0)].iter().enumerate() {
        let lit = d > 0.55 + rnd(seed + i as f64) * 0.3;
        let c = if lit {
            0xf8d070
        } else {
            mix(0x8ab0d0, 0x2a3448, d)
        };
        g.p(0xe8e0d0, x + wx - 1.0, top + wy - 1.0, 6, 7);
        g.p(c, x + wx, top + wy, 4, 5);
        g.p(mix(c, 0xffffff, 0.3), x + wx, top + wy, 4, 1);
        g.p(0xe8e0d0, x + wx, top + wy + 2.0, 4, 1);
        if lit {
            glow(g, 0xf8c860, x + wx + 2.0, top + wy + 2.0, 6.0, 0.25);
        }
    }
    // Front door.
    g.p(
        mix(0x6a3a2a, 0x1a1a24, d * 0.5),
        x + 11.0,
        top + 12.0,
        5,
        GROUND - top as i32 - 12,
    );
    put(g, 0xd8b060, (x + 15.0) as i32 - 1, (top + 17.0) as i32);
}

fn church(g: &mut World, t: f64, struck: f64) {
    let d = dark(t);
    let x = (g.panel(shelter_screen(g)) + 10) as f64;
    let stone = mix(0xb8b0a0, 0x34343e, d * 0.7);
    let dk = mix(stone, 0x000000, 0.3);
    g.p(stone, x, 26.0, 14, GROUND - 26);
    for r in 0..12 {
        g.p(
            dk,
            x + 7.0 - r as f64 * 0.6,
            14.0 + r as f64,
            1 + (r as f64 * 1.2) as i32,
            1,
        );
    }
    g.p(dk, x + 13.0, 26.0, 1, GROUND - 26);
    // Belfry and clock.
    g.p(0x1a1a24, x + 5.0, 28.0, 4, 5);
    g.p(0x1a1a24, x + 6.0, 27.0, 2, 1);
    g.disc(0xe8e0c8, x + 7.0, 37.0, 2);
    put(g, 0x2a2a2a, x as i32 + 7, 36);
    put(g, 0x2a2a2a, x as i32 + 8, 37);
    g.p(
        mix(0x4a3a2a, 0x1a1a24, d * 0.5),
        x + 5.0,
        41.0,
        4,
        GROUND - 41,
    );
    // The lightning rod; it glows after the strike.
    g.p(0x6a6a7a, x + 7.0, 10.0, 1, 4);
    put(g, 0x8a8a9a, x as i32 + 6, 11);
    put(g, 0x8a8a9a, x as i32 + 8, 11);
    if struck > 0.0 {
        glow(g, 0xffd080, x + 7.0, 11.0, 5.0, struck);
        put(g, mix(0x6a6a7a, 0xffe8a0, struck), x as i32 + 7, 10);
    }
}

fn spire_tip(g: &World) -> (f64, f64) {
    ((g.panel(shelter_screen(g)) + 17) as f64, 10.0)
}

fn bus_shelter(g: &mut World, t: f64) {
    let d = dark(t);
    let x = (g.panel(shelter_screen(g)) + 30) as f64;
    // Glass back panel with the town seen through it.
    for y in 26..GROUND {
        for i in 0..22 {
            blend(g, 0xc8e0f0, x as i32 + i, y, 0.25);
        }
    }
    g.p(mix(0xc84a3a, 0x5a2a2a, d * 0.5), x - 1.0, 23.0, 24, 3);
    g.p(0x2a2a30, x - 1.0, 26.0, 24, 1);
    g.p(0x5a5a64, x, 26.0, 1, 18);
    g.p(0x5a5a64, x + 21.0, 26.0, 1, 18);
    // A poster and a bench.
    g.p(0xf0e0b0, x + 14.0, 28.0, 5, 8);
    g.p(0x4aa0c8, x + 15.0, 29.0, 3, 3);
    g.p(0xe0604a, x + 15.0, 33.0, 3, 1);
    g.p(0x7a5a3a, x + 2.0, (GROUND - 5) as f64, 10, 1);
    g.p(0x4a4a50, x + 3.0, (GROUND - 4) as f64, 1, 4);
    g.p(0x4a4a50, x + 10.0, (GROUND - 4) as f64, 1, 4);
    // Drips off the roof edge while it rains.
    if rain(t) > 0.2 {
        for i in 0..6 {
            let dy = ((t * 10.0 + i as f64 * 0.37) % 1.0) * 14.0;
            put(g, 0x9ab0d0, x as i32 + i * 4, 26 + dy as i32);
        }
    }
}

/// The street lamp: flickers on as it gets dark.
fn lamp(g: &mut World, x: f64, t: f64, f: u32) {
    g.p(0x3a3a44, x, 20.0, 1, 28);
    g.p(0x3a3a44, x - 1.0, (GROUND - 2) as f64, 3, 2);
    g.p(0x3a3a44, x, 19.0, 4, 1);
    g.p(0x2a2a34, x + 2.0, 20.0, 3, 1);
    let on_ = dark(t) > 0.7 && !(t < 2.6 && (f % 5 == 1 || f % 7 == 3));
    let c = if on_ { 0xfff0b0 } else { 0x8a8a8a };
    g.p(c, x + 2.0, 21.0, 3, 2);
    if on_ {
        glow(g, 0xf8d880, x + 3.5, 23.0, 14.0, 0.35);
        // The cone of light with rain in it.
        for y in 23..GROUND {
            let hw = (y - 23) as f64 * 0.35;
            for i in -(hw as i32)..=(hw as i32) {
                blend(g, 0xf8e0a0, js_round(x + 3.5) + i, y, 0.12);
            }
        }
    }
}

fn tree(g: &mut World, x: f64, t: f64, f: u32) {
    let d = dark(t);
    let sway = wind(t) * 3.0 + (t * 6.0).sin() * wind(t);
    g.p(0x4a3424, x, 30.0, 3, 18);
    g.p(0x3a2418, x + 2.0, 30.0, 1, 18);
    let leaf = [
        mix(0x5aa848, 0x1e3a2a, d),
        mix(0x3a8a3a, 0x142a20, d),
        mix(0x8ad060, 0x2a4a34, d),
    ];
    for (i, (ox, oy, r)) in [
        (-4.0, 24.0, 6),
        (5.0, 24.0, 6),
        (0.0, 18.0, 7),
        (1.0, 27.0, 5),
    ]
    .iter()
    .enumerate()
    {
        let s = sway * (1.0 - oy / 40.0) * 1.6;
        g.disc(leaf[1], x + 1.0 + ox + s, oy + 1.0, *r);
        g.disc(leaf[0], x + 1.0 + ox + s, *oy, r - 1);
        put(
            g,
            leaf[2],
            js_round(x + ox + s),
            (*oy - 3.0) as i32 + i as i32 % 2,
        );
    }
    // Leaves torn off by the gusts.
    if wind(t) > 0.5 {
        for k in 0..6 {
            let kf = k as f64;
            let u = (t * 0.9 + rnd(kf + 80.0)) % 1.0;
            let lx = x + u * 70.0 * wind(t);
            let ly = 20.0 + rnd(kf + 81.0) * 10.0 + (u * 9.0 + kf).sin() * 4.0 + u * 10.0;
            put(
                g,
                if (f + k).is_multiple_of(2) {
                    leaf[0]
                } else {
                    leaf[2]
                },
                js_round(lx),
                js_round(ly),
            );
        }
    }
}

fn cottage(g: &mut World, t: f64) {
    let d = dark(t);
    let x = g.panel(last(g)) as f64 + 34.0;
    let wall = mix(0xe8d8b8, 0x3a3640, d * 0.6);
    g.p(wall, x, 24.0, 30, 24);
    for r in 0..8 {
        g.p(
            mix(0x6a5a8a, 0x22202e, d * 0.5),
            x - 3.0 + r as f64,
            16.0 + r as f64,
            40,
            1,
        );
    }
    // Window with a flower box.
    let lit = d > 0.6;
    g.p(0xf0f0f0, x + 15.0, 28.0, 8, 7);
    g.p(
        if lit {
            0xf8d070
        } else {
            mix(0x8ab0d0, 0x2a3448, d)
        },
        x + 16.0,
        29.0,
        6,
        5,
    );
    g.p(0xf0f0f0, x + 18.0, 29.0, 1, 5);
    g.p(0x8a5a3a, x + 14.0, 35.0, 10, 2);
    for i in 0..5 {
        put(
            g,
            [0xf04a6a, 0xf8d040][i % 2],
            x as i32 + 15 + i as i32 * 2,
            34,
        );
    }
    // The porch roof and the bench the cat hides under.
    g.p(mix(0x8a4a3a, 0x2a1a1a, d * 0.5), x - 4.0, 23.0, 12, 2);
    g.p(0x6a4a30, x - 3.0, 25.0, 1, 16);
    g.p(0x8a6040, x - 2.0, (GROUND - 10) as f64, 10, 2);
    g.p(0x5a3a24, x - 1.0, (GROUND - 8) as f64, 1, 8);
    g.p(0x5a3a24, x + 6.0, (GROUND - 8) as f64, 1, 8);
}

/// The road and pavement; puddle centres are `(x, half width)`.
fn street(g: &mut World, t: f64, puddles: &[(f64, f64)]) {
    let w = g.w();
    let d = dark(t);
    let pave = mix(0xb0aca4, 0x3a3a44, d * 0.8);
    g.p(pave, 0.0, GROUND as f64, w, 3);
    g.p(mix(pave, 0xffffff, 0.2), 0.0, GROUND as f64, w, 1);
    g.p(mix(0x8a8680, 0x24242c, d), 0.0, GROUND as f64 + 3.0, w, 1);
    vgrad(
        g,
        &[mix(0x5a5a64, 0x1a1c24, d), mix(0x6a6a74, 0x23262e, d)],
        0,
        w,
        GROUND + 4,
        60,
    );
    // Wet sheen: the road mirrors what stands on it.
    let wet = 0.15 + 0.35 * rain(t).max(d * 0.6);
    for y in GROUND + 4..60 {
        let my = 2 * (GROUND + 3) - y;
        for x in 0..w {
            let wob = js_round(((y as f64) * 1.7 + t * 9.0).sin() * 0.8);
            let mut k = wet;
            let mut src = get(g, x + wob, my);
            for &(px, hw) in puddles {
                let e = ((x as f64 - px) / hw).powi(2) + ((y as f64 - PY) / 2.6).powi(2);
                if e < 1.0 {
                    k = 0.75;
                    src = mix(src, 0x8ab0e0, 0.1);
                }
            }
            blend(g, src, x, y, k * (0.5 + 0.5 * (y - GROUND) as f64 / 12.0));
        }
    }
    // Puddle rims.
    for &(px, hw) in puddles {
        for i in -(hw as i32)..=(hw as i32) {
            let e = (1.0 - (i as f64 / hw).powi(2)).sqrt() * 2.6;
            blend(g, 0x0a0c12, js_round(px) + i, js_round(PY + e), 0.4);
        }
    }
}

/// Rain streaks slanting with the wind, and splashes where they land.
fn rainfall(g: &mut World, t: f64, f: u32, lit: f64) {
    let r = rain(t);
    if r <= 0.0 {
        return;
    }
    let wf = g.w() as f64;
    let sl = wind(t);
    let n = (g.spread(150) as f64 * r) as u32;
    for i in 0..n {
        let fi = i as f64;
        let near = i % 3 == 0;
        let v = if near { 95.0 } else { 70.0 } + rnd(fi + 3.0) * 15.0;
        let y = (rnd(fi + 1.0) * 70.0 + t * v) % 70.0 - 6.0;
        let x = ((rnd(fi) * wf + y * sl + t * v * sl * 0.2) % wf + wf) % wf;
        let len = if near { 4 } else { 2 };
        let c = mix(if near { 0xa8b8d8 } else { 0x6a7898 }, 0xffffff, lit);
        for s in 0..len {
            let yy = y + s as f64;
            if yy > (GROUND + 2) as f64 {
                break;
            }
            blend(
                g,
                c,
                js_round(x + s as f64 * sl),
                js_round(yy),
                if near { 0.75 } else { 0.5 },
            );
        }
    }
    for k in 0..(g.spread(26) as f64 * r) as u32 {
        let seed = (k * 97 + f) as f64;
        let x = (rnd(seed) * wf).floor() as i32;
        let y = GROUND + 1 + (rnd(seed + 2.0) * 10.0) as i32;
        let c = 0xb8c8e0;
        blend(g, c, x, y - 1, 0.7);
        blend(g, c, x - 1, y - 2, 0.45);
        blend(g, c, x + 1, y - 2, 0.45);
    }
}

/// A forked bolt from the clouds to `(tx, ty)`.
fn bolt(g: &mut World, x0: f64, y0: f64, tx: f64, ty: f64, seed: f64) {
    let n = 10;
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let u = i as f64 / n as f64;
        let jit = if i == 0 || i == n {
            0.0
        } else {
            (rnd(seed + i as f64) - 0.5) * 9.0 * (1.0 - u * 0.5)
        };
        pts.push((x0 + (tx - x0) * u + jit, y0 + (ty - y0) * u));
    }
    for (i, p) in pts.iter().enumerate().skip(2).step_by(3) {
        if i == n {
            break;
        }
        let dir = if rnd(seed + 30.0 + i as f64) > 0.5 {
            1.0
        } else {
            -1.0
        };
        let (mut bx, mut by) = *p;
        for s in 0..4 {
            let nx = bx + dir * (2.0 + rnd(seed + 50.0 + (i * 4 + s) as f64) * 3.0);
            let ny = by + 2.0 + rnd(seed + 70.0 + s as f64) * 2.0;
            line(g, 0x9aa8ff, bx, by, nx, ny);
            bx = nx;
            by = ny;
        }
    }
    for p in &pts {
        glow(g, 0xb8c4ff, p.0, p.1, 8.0, 0.4);
    }
    for w in pts.windows(2) {
        line(g, 0xa8b4ff, w[0].0 - 1.0, w[0].1, w[1].0 - 1.0, w[1].1);
        line(g, 0xe8ecff, w[0].0 + 1.0, w[0].1, w[1].0 + 1.0, w[1].1);
        line(g, 0xffffff, w[0].0, w[0].1, w[1].0, w[1].1);
    }
}

/// The walker's stage position and pose: (stage, run, flinch).
fn walker_at(g: &World, t: f64) -> (f64, bool, f64) {
    let f0 = stage_at(g, 0, 4.0);
    let fp = stage_at(g, flip_screen(g), 28.0);
    let sh = stage_at(g, shelter_screen(g), 41.0);
    if t < T_FLIP {
        (f0 + (fp - f0) * t / T_FLIP, false, 0.0)
    } else if t < T_TORN {
        // Pushed back by the gust, heels dug in.
        let u = (t - T_FLIP) / (T_TORN - T_FLIP);
        (fp - (u * PI).sin() * 3.0, false, 1.0)
    } else if t < T_SAFE {
        (
            fp + (sh - fp) * ease((t - T_TORN) / (T_SAFE - T_TORN)),
            true,
            0.0,
        )
    } else {
        let peek = smooth(T_SEE - 0.6, T_SEE, t) * 4.0;
        (sh + peek, false, 0.0)
    }
}

fn walker(g: &mut World, t: f64, f: u32) {
    let (s, run, flinch) = walker_at(g, t);
    let x = stage_x(g, s) - 3.0;
    let y = (GROUND - 11) as f64;
    let moving = t < T_FLIP || (T_TORN..T_SAFE).contains(&t) || (T_SEE - 0.6..T_SEE).contains(&t);
    let step = if run { f % 2 } else { (f / 2) % 2 };
    let bob = if moving && step == 0 { -1.0 } else { 0.0 };
    let legs = if !moving {
        LEGS[1]
    } else if flinch > 0.0 {
        LEGS[0]
    } else {
        LEGS[(step as usize) * 2]
    };
    let lean = if run { 1.0 } else { 0.0 };
    g.spr(WALKER, COAT, x + lean, y + bob, false);
    g.spr(legs, COAT, x, y + 9.0, false);
    if run {
        // Hands over the hood.
        put(g, 0xf0c8a0, js_round(x + lean) + 2, js_round(y + bob) - 1);
        put(g, 0xf0c8a0, js_round(x + lean) + 4, js_round(y + bob) - 1);
        // Drips flying off.
        for k in 0..3 {
            let u = ((t * 4.0 + k as f64 * 0.33) % 1.0) * 4.0;
            put(
                g,
                0xa8c0e0,
                js_round(x - 1.0 - u),
                js_round(y + 2.0 + u * 0.6 + k as f64),
            );
        }
    } else if t < T_TORN {
        // The arm up to the umbrella handle.
        put(g, 0xc8961e, js_round(x) + 6, js_round(y + bob) + 4);
        put(g, 0xf0c8a0, js_round(x) + 6, js_round(y + bob) + 3);
    }
    if t > T_SEE {
        // Spots the cat: an exclamation mark.
        let k = js_round(x) + 3;
        let yy = 17 - ((t - T_SEE) * 6.0).min(2.0) as i32;
        g.p(0x1e2a4a, k as f64, yy as f64, 1, 3);
        put(g, 0x1e2a4a, k, yy + 4);
    }
}

/// The umbrella: shaft tip at `(x, y)`, rotated `a`; `s` 1 is open, below 0
/// it is blown inside out.
fn umbrella(g: &mut World, x: f64, y: f64, a: f64, s: f64) {
    let (c, sn) = (a.cos(), a.sin());
    let mut dot = |u: f64, v: f64, col: u32| {
        put(
            g,
            col,
            js_round(x + u * c - v * sn),
            js_round(y + u * sn + v * c),
        );
    };
    let hw = 7.0 - (1.0 - s).max(0.0) * 0.6;
    let lift = (1.0 - s).max(0.0) * 1.6;
    // Shaft and hooked handle.
    for v in 0..10 {
        dot(0.0, v as f64, 0xb0a8a0);
    }
    dot(-1.0, 10.0, 0x8a5a30);
    dot(-2.0, 9.0, 0x8a5a30);
    // Canopy: open it is a filled dome down to the rim; inside out, a cup.
    let mut u = -hw;
    while u <= hw {
        let k = u / hw;
        let v = lift + s * 3.5 * (k * k) - (1.0 - k * k) * 0.5 * s;
        let rim = lift + s * 3.5;
        let stripe = ((u + hw) / 2.4) as i32 % 2 == 0;
        let col = if stripe { 0xe63c3c } else { 0xc02a30 };
        if s > 0.0 {
            let mut vv = v;
            while vv <= rim + 0.01 {
                let shade = if vv < v + 0.8 && k < 0.2 {
                    0xf06a5a
                } else {
                    col
                };
                dot(u, vv, shade);
                vv += 0.5;
            }
            dot(u, rim + 0.6, 0x8a1a22);
        } else {
            // Inside out: the pale lining shows, the fabric cupped up.
            let mut vv = rim;
            while vv <= v {
                dot(u, vv, if vv > v - 0.8 { col } else { 0xf08a7a });
                vv += 0.5;
            }
            dot(u, v + 0.6, 0x8a1a22);
            if (u.abs() - hw * 0.55).abs() < 0.25 {
                // Bare spokes showing.
                for i in 0..5 {
                    let w = i as f64 / 5.0;
                    dot(u * w, v * w + lift * (1.0 - w), 0x6a6a74);
                }
            }
        }
        u += 0.35;
    }
    dot(0.0, -1.0 - 0.5 * s.max(0.0), 0x5a5a64);
}

/// The umbrella floating upside down: a striped bowl with its rim at
/// `y`, the handle up like a mast.
fn boat(g: &mut World, x: f64, y: f64, t: f64) {
    let (x, y) = (js_round(x), js_round(y));
    let tilt = js_round((t * 2.5).sin() * 1.2);
    // The mast first, so the cat can sit in front of it.
    for v in 0..9 {
        put(g, 0xb0a8a0, x + 1, y - 2 - v);
    }
    put(g, 0x8a5a30, x + 2, y - 11);
    put(g, 0x8a5a30, x + 3, y - 10);
    put(g, 0x8a5a30, x + 3, y - 9);
    for (r, hw) in [9, 9, 8, 7, 5].iter().enumerate() {
        for i in -hw..=*hw {
            let stripe = ((i + 8) / 3) % 2 == 0;
            let mut c = if stripe { 0xe63c3c } else { 0xc02a30 };
            if r == 0 {
                c = 0x8a1a22;
            } else if r == 4 || i == -hw || i == *hw {
                c = mix(c, 0x5a1018, 0.4);
            }
            let dy = (i as f64 / 8.0 * tilt as f64).round() as i32;
            put(g, c, x + i, y + r as i32 + dy);
        }
    }
    put(g, 0xf08a7a, x - 4, y + 1);
    put(g, 0xf08a7a, x - 3, y + 1);
}

/// Where the umbrella floats in the big puddle.
fn boat_at(g: &World, t: f64) -> (f64, f64) {
    let x = (g.panel(last(g)) + 27) as f64 + smooth(T_ABOARD, SECS, t) * 2.0;
    (x, PY - 1.0 + (t * 3.0).sin() * 0.5)
}

fn umbrella_now(g: &mut World, t: f64) {
    let (s, _, _) = walker_at(g, t.min(T_TORN));
    let hx = stage_x(g, s) + 3.0;
    let hy = (GROUND - 21) as f64;
    if t < T_FLIP {
        let tilt = -0.12 - wind(t) * 0.2 + (t * 5.0).sin() * 0.05;
        umbrella(g, hx - tilt * 10.0, hy, tilt, 1.0);
    } else if t < T_TORN {
        let u = (t - T_FLIP) / 0.25;
        let s = 1.0 - 2.4 * ease(u);
        let a = -0.3 + ((t - T_FLIP) * 30.0).sin() * 0.15;
        umbrella(g, hx + 4.0, hy, a, s);
    } else if t < T_LAND {
        let u = (t - T_TORN) / (T_LAND - T_TORN);
        let s0 = stage_at(g, flip_screen(g), 32.0);
        let s1 = stage_at(g, last(g), 0.0) + (boat_at(g, t).0 - g.panel(last(g)) as f64);
        let x = stage_x(g, s0 + (s1 - s0) * u);
        let (_, by) = boat_at(g, t);
        let y = hy + (by - hy) * u * u - (u * PI).sin() * 9.0;
        let s = -1.4 + 2.4 * smooth(0.3, 0.7, u);
        umbrella(g, x, y, u * 7.0 * PI, s);
    } else {
        let (bx, by) = boat_at(g, t);
        boat(g, bx, by, t);
        // Ripples around the hull.
        for k in 0..2 {
            let r = ((t - T_LAND) * 6.0 + k as f64 * 5.0) % 10.0 + 6.0;
            for i in 0..24 {
                let a = i as f64 / 24.0 * PI * 2.0;
                if a.sin() > -0.2 {
                    blend(
                        g,
                        0xc8d8f0,
                        js_round(bx + a.cos() * r),
                        js_round(by + 1.5 + a.sin() * r * 0.25),
                        0.5 * (1.0 - (r - 6.0) / 10.0),
                    );
                }
            }
        }
        if t < T_LAND + 0.4 {
            // The splash as it lands.
            let u = (t - T_LAND) / 0.4;
            for k in 0..8 {
                let a = PI + k as f64 / 7.0 * PI;
                let r = u * 7.0;
                put(
                    g,
                    0xd0e0f8,
                    js_round(bx + a.cos() * r * 1.4),
                    js_round(by + a.sin() * r + u * u * 8.0),
                );
            }
        }
    }
}

fn cat(g: &mut World, t: f64, f: u32) {
    let x0 = g.panel(last(g)) as f64 + 33.0;
    let (bx, by) = boat_at(g, t);
    if t < T_CAT {
        // Sheltering under the bench, blinking now and then.
        g.spr(CAT_SIT, CATP, x0, (GROUND - 8) as f64, true);
        if (f / 9).is_multiple_of(6) {
            put(g, 0x1e1a24, js_round(x0) + 1, GROUND - 6);
        }
    } else if t < T_HOP {
        let u = (t - T_CAT) / (T_HOP - T_CAT);
        let x = x0 - 2.0 + (bx + 4.0 - x0 + 2.0) * u;
        g.spr(
            CAT_WALK[((f / 2) % 2) as usize],
            CATP,
            x,
            (GROUND - 5) as f64,
            false,
        );
    } else if t < T_ABOARD {
        let u = (t - T_HOP) / (T_ABOARD - T_HOP);
        let x = bx + 4.0 + (bx - 6.0 - bx - 4.0) * u;
        let y = (GROUND - 8) as f64 + (by - 6.0 - GROUND as f64 + 8.0) * u - (u * PI).sin() * 7.0;
        g.spr(CAT_SIT, CATP, x, y, false);
    } else {
        g.spr(CAT_SIT, CATP, bx - 6.0, by - 6.0, false);
        // A swish of the tail, then a little heart.
        if t > T_ABOARD + 0.5 {
            let hy = by - 15.0 - (t - T_ABOARD - 0.5) * 3.0;
            g.spr(HEART, &[('h', 0xf06080)], bx - 5.0, hy, false);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let lit = flash(t);
    sky(g, t, lit);
    for (i, &(tb, spire)) in BOLTS.iter().enumerate() {
        if !spire && bolt_on(t, tb) {
            // Far-off strikes come down behind the roofs.
            let cx = g.panel(if i == 0 { 0 } else { last(g) }) as f64 + 22.0;
            bolt(g, cx + 12.0, -1.0, cx - 4.0, 38.0, 40.0 + i as f64 * 20.0);
        }
    }
    skyline(g, t);
    let x0 = g.panel(0) as f64;
    house(g, x0 + 6.0, 26.0, 0xd8a888, 0x8a4a3a, t, 1.0);
    house(g, x0 + 26.0, 22.0, 0xa8c0c8, 0x4a5a7a, t, 5.0);
    house(g, x0 + 46.0, 28.0, 0xe0d0a0, 0x7a4a5a, t, 9.0);
    if g.layout.panels >= 4 {
        let x1 = g.panel(1) as f64;
        house(g, x1 + 40.0, 26.0, 0xc8b0d0, 0x5a4a6a, t, 13.0);
        tree(g, x1 + 14.0, t, f);
        lamp(g, x1 + 32.0, t, f);
    } else {
        lamp(g, x0 + 52.0, t, f);
    }
    let struck = BOLTS
        .iter()
        .filter(|b| b.1 && t >= b.0)
        .map(|b| (1.0 - (t - b.0) / 1.2).max(0.0))
        .fold(0.0, f64::max);
    church(g, t, struck);
    bus_shelter(g, t);
    cottage(g, t);
    let puddles = [
        ((g.panel(last(g)) + 28) as f64, 13.0),
        ((g.panel(0) + 34) as f64, 8.0),
        ((g.panel(shelter_screen(g)) + 12) as f64, 7.0),
    ];
    street(g, t, &puddles);
    cat(g, t, f);
    walker(g, t, f);
    umbrella_now(g, t);
    rainfall(g, t, f, lit);
    if lit > 0.0 {
        tint(g, 0xeef2ff, lit * 0.3);
    }
    for &(tb, spire) in &BOLTS {
        if spire && bolt_on(t, tb) {
            let (tx, ty) = spire_tip(g);
            bolt(g, tx + 9.0, -1.0, tx, ty, 11.0);
            // Sparks off the rod.
            for k in 0..6 {
                let a = k as f64 / 6.0 * PI * 2.0 + t * 9.0;
                let r = 2.0 + (t - tb) * 14.0;
                let (sx, sy) = (tx + a.cos() * r, ty + a.sin() * r * 0.7);
                put(g, 0xfff0a0, js_round(sx), js_round(sy));
            }
        }
    }
    // Thunder shakes the frame just after the strike.
    for (tb, spire) in BOLTS {
        if spire && (tb + 0.1..tb + 0.5).contains(&t) {
            shake(g, if f.is_multiple_of(2) { 1 } else { -1 }, 0);
        }
    }
}

/// The bolt shows for four frames, blinking out on the second.
fn bolt_on(t: f64, tb: f64) -> bool {
    (tb..tb + 0.34).contains(&t) && !(tb + 1.0 / 12.0..tb + 2.0 / 12.0).contains(&t)
}

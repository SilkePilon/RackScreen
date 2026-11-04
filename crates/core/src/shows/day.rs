//! Day in a minute: a farm, a windmill, a village and a duck pond from
//! half past four in the morning to past midnight in twelve seconds. The
//! rooster on the fence post crows and the sun pops up on cue; it arcs over
//! every screen through a dithered sky that runs dawn pink, noon blue,
//! sunset orange and night navy, while shadows swing and shorten and
//! stretch, the windmill turns, the tractor and a cyclist go by, the café
//! opens its parasols and the ducks paddle. At dusk the windows and the
//! street lamp come on, the rooster goes to roost and the stars come out.
//! Then the full moon rises; the rooster wakes, takes it for the sun and
//! crows his loudest. The farmhouse window flies open, a pillow sails out
//! and knocks him off the post in a burst of feathers.

use super::fx::{blend, get, glow, line, mix, on, put, stage_at, stage_x, vgrad};
use super::world::{ease, js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 12.0;

/// Ground line: things stand on this row.
const BASE: i32 = 50;
const HORIZON: i32 = 42;

const T_CROW: f64 = 0.55;
const T_ROOST: f64 = 8.6;
const T_WAKE: f64 = 10.2;
const T_MOONCROW: f64 = 10.45;
const T_WINDOW: f64 = 10.9;
const T_THROW: f64 = 11.1;
const T_HIT: f64 = 11.45;

/// Clock hour at show time `t`.
fn hour(t: f64) -> f64 {
    4.5 + t * 1.75
}
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}
/// 0 at sunrise (just after the crow), 1 at sunset.
fn sun_u(t: f64) -> f64 {
    (hour(t) - 5.4) / 14.8
}

/// Sky (top, middle, horizon) through the day, keyed by hour.
const SKY: [(f64, [u32; 3]); 9] = [
    (4.5, [0x080c26, 0x141c46, 0x2a2e5e]),
    (5.6, [0x1e2462, 0x6a4a86, 0xe0806a]),
    (6.8, [0x3c78cc, 0x8ab4e0, 0xf8cc98]),
    (8.5, [0x3a84de, 0x78b8ec, 0xc4e4f6]),
    (17.0, [0x3a84de, 0x78b8ec, 0xc4e4f6]),
    (19.0, [0x3a4a9a, 0xc86a86, 0xf8a858]),
    (20.4, [0x181c4c, 0x46306a, 0x9a4a5a]),
    (21.6, [0x070b22, 0x10183c, 0x1c284e]),
    (26.0, [0x070b22, 0x10183c, 0x1c284e]),
];
/// The light on the land: (tint, strength) keyed by hour.
const AMB: [(f64, u32, f64); 8] = [
    (4.5, 0x101a40, 0.7),
    (5.6, 0x5a3a6a, 0.45),
    (6.8, 0xf0a070, 0.18),
    (8.5, 0xffffff, 0.0),
    (17.0, 0xffffff, 0.0),
    (19.0, 0xe86a3a, 0.28),
    (20.6, 0x1a1a4a, 0.6),
    (21.6, 0x0c1438, 0.7),
];

fn sky_at(h: f64) -> [u32; 3] {
    let i = SKY
        .iter()
        .rposition(|k| k.0 <= h)
        .unwrap_or(0)
        .min(SKY.len() - 2);
    let (a, b) = (SKY[i], SKY[i + 1]);
    let k = ((h - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    [0, 1, 2].map(|j| mix(a.1[j], b.1[j], k))
}
fn amb_at(h: f64) -> (u32, f64) {
    let h = h.min(25.0);
    let i = AMB
        .iter()
        .rposition(|k| k.0 <= h)
        .unwrap_or(0)
        .min(AMB.len() - 2);
    let (a, b) = (AMB[i], AMB[i + 1]);
    let k = ((h - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    (mix(a.1, b.1, k), a.2 + (b.2 - a.2) * k)
}
/// How dark it is, for lamps and windows (0 day, 1 night).
fn night(t: f64) -> f64 {
    let h = hour(t);
    1.0 - smooth(5.4, 6.4, h) + smooth(19.2, 20.4, h)
}

const ROO: &[(char, u32)] = &[
    ('r', 0xe03a2a),
    ('w', 0xf4efe4),
    ('b', 0xc8beb0),
    ('y', 0xf0b030),
    ('e', 0x1a1a1a),
    ('k', 0x1a3a2a),
    ('g', 0x2a7a5a),
    ('o', 0xe8a030),
];
/// The rooster facing left: standing, crowing, asleep, pecking.
const ROOSTER: [&[&str]; 4] = [
    &[
        "..rr.....",
        ".rrw...kg",
        "ywew..kgk",
        ".rww.kgk.",
        "..wwwwwk.",
        "..wbwwwg.",
        "...bbww..",
        "....o.o..",
        "...oo.oo.",
    ],
    &[
        "y.rr.....",
        "y.rw...kg",
        ".yew..kgk",
        ".rww.kgk.",
        "..wwwwwk.",
        ".wwbwwwg.",
        "...bbww..",
        "....o.o..",
        "...oo.oo.",
    ],
    &[
        ".........",
        ".........",
        "..rr...kg",
        ".rwbw.kgk",
        "..wwwwwk.",
        "..wbwwwg.",
        "...bbww..",
        "....o.o..",
        "...oo.oo.",
    ],
    &[
        ".........",
        ".......kg",
        "......kgk",
        "..wwwwgk.",
        ".wwwbwwk.",
        "rwwbwwww.",
        "ywe.bww..",
        "....o.o..",
        "...oo.oo.",
    ],
];
const HEN: [&[&str]; 2] = [
    &[".r...", "nen.n", ".nnnn", "..nn.", "..o.o"],
    &[".....", ".r..n", "nnnnn", "ynnn.", "..o.o"],
];
const HENP: &[(char, u32)] = &[
    ('r', 0xe03a2a),
    ('n', 0xb8743a),
    ('e', 0x1a1a1a),
    ('y', 0xf0b030),
    ('o', 0xe8a030),
];
const DUCK: &[&str] = &[".gg..", "ygeg.", "..www", ".wwww", "..www"];
const DUCKP: &[(char, u32)] = &[
    ('g', 0x2a7a4a),
    ('y', 0xf0a030),
    ('e', 0x1a1a1a),
    ('w', 0xe8e0d0),
];

/// A cast shadow on the ground beside an object `w` wide and `h` tall
/// standing at `x`, away from the sun.
fn shadow(g: &mut World, x: f64, w: f64, h: f64, t: f64) {
    let u = sun_u(t);
    if !(0.0..=1.0).contains(&u) {
        return;
    }
    let sx = sun_x(g, t);
    let elev = (u * PI).sin().max(0.12);
    let len = (h * 0.5 / elev.tan()).min(40.0);
    let dir = if x + w / 2.0 > sx { 1.0 } else { -1.0 };
    let k = 0.55 * smooth(0.0, 0.08, u) * smooth(1.0, 0.92, u);
    for r in 0..4 {
        let spread = len * (1.0 - r as f64 * 0.2);
        let (a, b) = if dir > 0.0 {
            (x, x + w + spread)
        } else {
            (x - spread, x + w)
        };
        for xx in js_round(a)..js_round(b) {
            blend(g, 0x1a3a20, xx, BASE - 2 + r, k);
        }
    }
}

fn sun_x(g: &World, t: f64) -> f64 {
    let s = stage_at(g, g.layout.panels - 1, 58.0);
    stage_x(g, 12.0 + sun_u(t) * (s - 12.0))
}

fn sky(g: &mut World, t: f64, f: u32) {
    let w = g.w();
    let h = hour(t);
    let st = sky_at(h);
    vgrad(g, &st, 0, w, 0, HORIZON + 4);
    // Stars fade in and out with the dark.
    let dark = night(t);
    if dark > 0.05 {
        for i in 0..g.spread(70) {
            let fi = i as f64;
            let (x, y) = ((rnd(fi) * w as f64) as i32, (rnd(fi + 3.0) * 36.0) as i32);
            let tw = (f + i * 7) % 29 < 2;
            let k = dark * (0.4 + 0.6 * rnd(fi + 5.0)) * if tw { 1.0 } else { 0.7 };
            blend(g, if tw { 0xffffff } else { 0xc8d0f0 }, x, y, k);
            if tw && i % 3 == 0 {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    blend(g, 0x8a9ad0, x + dx, y + dy, k * 0.6);
                }
            }
        }
        // A faint milky band.
        for x in 0..w {
            let yc = 8.0 + x as f64 * 0.06;
            for dy in -4..5 {
                let y = js_round(yc) + dy;
                if on(x, y, 0.25 * dark * (1.0 - (dy as f64 / 5.0).abs())) {
                    blend(g, 0x6a74b0, x, y, 0.35);
                }
            }
        }
    }
    // The sun.
    let u = sun_u(t);
    if (-0.05..1.05).contains(&u) {
        let (sx, sy) = (sun_x(g, t), sun_y(u));
        let warm = 1.0 - (u * PI).sin().clamp(0.0, 1.0);
        let col = mix(0xfff6c8, 0xffa850, warm);
        glow(g, mix(0xfff0c0, 0xff9050, warm), sx, sy, 20.0, 0.55);
        g.disc(mix(col, 0xff7a3a, warm * 0.5), sx, sy, 4);
        g.disc(col, sx, sy, 3);
        put(g, 0xffffff, js_round(sx) - 1, js_round(sy) - 1);
    }
    // The moon: setting on the right before dawn, rising on the left at night.
    let (mx, my, mk) = moon(g, t);
    if mk > 0.0 {
        glow(g, 0xc8d4ff, mx, my, 12.0, 0.3 * mk);
        g.disc(mix(st[1], 0xf0ecd8, mk), mx, my, 4);
        for (dx, dy) in [(-1, -1), (2, 1), (-2, 2), (1, -3)] {
            put(
                g,
                mix(st[1], 0xc8c0a8, mk),
                js_round(mx) + dx,
                js_round(my) + dy,
            );
        }
    }
    // Clouds, lit by the hour.
    let lit = mix(st[2], 0xffffff, 0.55);
    let shade = mix(st[1], st[2], 0.4);
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let span = w as f64 + 40.0;
        let x = (rnd(kf + 20.0) * span + t * 4.0) % span - 20.0;
        let y = 8.0 + rnd(kf + 21.0) * 14.0;
        let s = 0.7 + rnd(kf + 22.0) * 0.5;
        g.disc(shade, x + 1.0, y + 1.5, js_round(3.0 * s));
        g.disc(lit, x - 3.5 * s, y + 1.0, js_round(2.0 * s));
        g.disc(lit, x, y, js_round(3.0 * s));
        g.disc(lit, x + 4.0 * s, y + 1.0, js_round(2.4 * s));
        g.p(shade, x - 6.0 * s, y + 3.0, js_round(12.0 * s), 1);
    }
}

fn sun_y(u: f64) -> f64 {
    HORIZON as f64 + 2.0 - (u * PI).sin().max(0.0).powf(0.6) * 28.0
}

/// The moon's position and how much of it shows.
fn moon(g: &World, t: f64) -> (f64, f64, f64) {
    let h = hour(t);
    if h < 6.5 {
        let u = (h - 4.5) / 2.0;
        let x = (g.panel(g.layout.panels - 1) + 40) as f64 + u * 10.0;
        (x, 16.0 + u * 30.0, 1.0 - smooth(5.2, 6.5, h))
    } else if h > 20.4 {
        let u = (t - 8.9) / 2.0;
        let x = g.panel(0) as f64 + 6.0 + ease(u.min(1.0)) * 18.0 + (u - 1.0).max(0.0) * 3.0;
        (
            x,
            HORIZON as f64 - ease(u.min(1.0)) * 28.0,
            smooth(20.4, 21.0, h),
        )
    } else {
        (0.0, 0.0, 0.0)
    }
}

/// Far hills and the fields, in day colours.
fn land(g: &mut World) {
    let w = g.w();
    for x in 0..w {
        let xf = x as f64;
        let hh = 4.0 + 3.0 * (xf * 0.045).sin() + 2.0 * (xf * 0.11 + 1.0).sin();
        let top = HORIZON - js_round(hh);
        for y in top..HORIZON + 2 {
            put(g, if on(x, y, 0.4) { 0x7aa86a } else { 0x6a9a64 }, x, y);
        }
    }
    vgrad(g, &[0x6ab04a, 0x5aa040, 0x4a8a38], 0, w, HORIZON + 2, 60);
    // The dirt lane along the front.
    for x in 0..w {
        let y = 53 + js_round((x as f64 * 0.05).sin());
        g.p(0xc8a878, x as f64, y as f64, 1, 3);
        if rnd(x as f64) > 0.8 {
            put(g, 0xa88a60, x, y + 1);
        }
    }
    // Grass tufts.
    for k in 0..g.spread(60) {
        let kf = k as f64;
        let x = (rnd(kf + 90.0) * w as f64) as i32;
        let y = BASE + (rnd(kf + 91.0) * 9.0) as i32;
        if !(52..57).contains(&y) {
            put(g, 0x8ac85a, x, y);
            put(g, 0x3a7a30, x, y + 1);
        }
    }
}

fn farm(g: &mut World, t: f64) {
    let x = g.panel(0) as f64 + 6.0;
    shadow(g, x, 20.0, 18.0, t);
    // Farmhouse: whitewashed, thatched, a green door and the bedroom window.
    g.p(0xf0e8d8, x, 34.0, 20, 16);
    g.p(0xd8d0c0, x + 18.0, 34.0, 2, 16);
    for r in 0..8 {
        let c = if r % 2 == 0 { 0xc89a50 } else { 0xb88a40 };
        let i = 7 - r;
        g.p(c, x - 2.0 + i as f64, 26.0 + r as f64, 24 - 2 * i, 1);
    }
    g.p(0x8a5a3a, x + 14.0, 22.0, 3, 6);
    g.p(0x3a7a4a, x + 3.0, 42.0, 5, 8);
    put(g, 0xd8b040, js_round(x) + 7, 46);
    // The bedroom window, its shutters thrown open when the pillow flies.
    let open = smooth(T_WINDOW, T_WINDOW + 0.15, t);
    g.p(0x5a4a3a, x + 11.0, 37.0, 6, 6);
    g.p(0x7ab0d8, x + 12.0, 38.0, 4, 4);
    let sw = js_round(2.0 + open * 1.0);
    g.p(0x3a7a4a, x + 11.0 - sw as f64, 37.0, sw, 6);
    g.p(0x3a7a4a, x + 17.0, 37.0, sw, 6);
    // Fence to the rooster's post.
    for i in 0..9 {
        let px = x + 22.0 + i as f64 * 4.0;
        g.p(0x9a7448, px, 42.0, 1, 8);
    }
    g.p(0xb08a58, x + 22.0, 44.0, 34, 1);
    g.p(0xb08a58, x + 22.0, 47.0, 34, 1);
    let (px, py) = post(g);
    g.p(0x8a6440, px - 1.0, py, 3, BASE - py as i32);
    g.p(0xa88058, px - 1.0, py, 3, 1);
}

/// The top of the rooster's post.
fn post(g: &World) -> (f64, f64) {
    (g.panel(0) as f64 + 46.0, 41.0)
}

fn fields(g: &mut World, t: f64) {
    if g.layout.panels < 4 {
        return;
    }
    let x = g.panel(1) as f64;
    // Crop rows striping back to the hills.
    for r in 0..4 {
        g.p(
            if r % 2 == 0 { 0xc8b04a } else { 0xb89a3a },
            x - 6.0,
            (HORIZON + 2 + r) as f64,
            72,
            1,
        );
    }
    // The red barn and a haystack.
    shadow(g, x + 2.0, 18.0, 16.0, t);
    g.p(0xb83a2a, x + 2.0, 36.0, 18, 14);
    for r in 0..6 {
        let i = 5 - r;
        g.p(
            0x5a3a3a,
            x + 1.0 + i as f64 * 1.5,
            30.0 + r as f64,
            20 - 3 * i,
            1,
        );
    }
    g.p(0xf0e8d8, x + 7.0, 40.0, 8, 10);
    g.p(0xa83020, x + 8.0, 41.0, 6, 9);
    line(g, 0xf0e8d8, x + 8.0, 41.0, x + 13.0, 49.0);
    line(g, 0xf0e8d8, x + 13.0, 41.0, x + 8.0, 49.0);
    g.disc(0xe0c060, x + 24.0, 47.0, 3);
    g.p(0xc8a848, x + 21.0, 47.0, 7, 3);
    // The windmill turning in the breeze.
    let (mx, my) = (x + 42.0, 28.0);
    shadow(g, mx - 3.0, 7.0, 24.0, t);
    for r in 0..22 {
        let hw = 2.0 + r as f64 * 0.12;
        g.p(0xe8dcc8, mx - hw, my + r as f64, (2.0 * hw) as i32 + 1, 1);
    }
    g.p(0x5a4a3a, mx - 1.0, my + 16.0, 3, 6);
    g.p(0x7a5a4a, mx - 3.0, my - 3.0, 7, 3);
    let spin = t * 1.6;
    for k in 0..4 {
        let a = spin + k as f64 * PI / 2.0;
        let (dx, dy) = (a.cos(), a.sin());
        let (nx, ny) = (-dy, dx);
        let mut s = 1.0;
        while s < 13.0 {
            let (cx, cy) = (mx + dx * s, my - 1.0 + dy * s);
            put(g, 0x5a4030, js_round(cx), js_round(cy));
            if s > 3.5 {
                let mut o = 0.7;
                while o < 3.2 {
                    let lattice = (s as i32) % 2 == 0 || o > 2.6;
                    let c = if lattice { 0x8a6a50 } else { 0xf0ece0 };
                    put(g, c, js_round(cx + nx * o), js_round(cy + ny * o));
                    o += 0.5;
                }
            }
            s += 0.5;
        }
    }
    put(g, 0x3a2a20, js_round(mx), js_round(my - 1.0));
}

fn village(g: &mut World, t: f64) {
    let x = g.panel(g.layout.panels.saturating_sub(2)) as f64;
    let houses = [
        (x + 2.0, 34.0, 0xe8c8a0, 0x8a4a3a),
        (x + 14.0, 30.0, 0xb8d0d8, 0x4a5a7a),
        (x + 42.0, 33.0, 0xf0d8a8, 0x7a4a5a),
    ];
    for &(hx, top, wall, roof) in &houses {
        shadow(g, hx, 12.0, BASE as f64 - top + 4.0, t);
        g.p(wall, hx, top, 12, BASE - top as i32);
        for r in 0..5 {
            let i = 4 - r;
            g.p(
                roof,
                hx - 1.0 + i as f64 * 1.5,
                top - 5.0 + r as f64,
                14 - 3 * i,
                1,
            );
        }
        for (wx, wy) in [(2.0, 3.0), (7.0, 3.0), (2.0, 9.0)] {
            g.p(0x7aa8d0, hx + wx, top + wy, 3, 4);
            g.p(0xe8f0f8, hx + wx, top + wy, 3, 1);
        }
        g.p(0x6a4a3a, hx + 7.0, BASE as f64 - 6.0, 3, 6);
    }
    // The café with its awning and, in the daytime, parasols out.
    let cx = x + 26.0;
    shadow(g, cx, 14.0, 14.0, t);
    g.p(0xf4ece0, cx, 36.0, 14, 14);
    for i in 0..14 {
        let c = if (i / 2) % 2 == 0 { 0xd84a4a } else { 0xf4ece0 };
        g.p(c, cx + i as f64, 39.0, 1, 3);
    }
    g.p(0x7aa8d0, cx + 2.0, 43.0, 6, 5);
    let open = smooth(7.5, 8.5, hour(t)) * (1.0 - smooth(18.5, 19.5, hour(t)));
    if open > 0.5 {
        for (i, px) in [cx - 4.0, cx + 16.0].iter().enumerate() {
            let c = if i == 0 { 0x4a9ad8 } else { 0xf0c040 };
            g.p(c, px - 3.0, 43.0, 7, 1);
            g.p(c, px - 2.0, 42.0, 5, 1);
            g.p(0x5a5a5a, *px, 44.0, 1, 6);
            g.p(0x8a6a4a, px - 2.0, 47.0, 5, 1);
        }
    }
}

fn pond(g: &mut World, t: f64, f: u32) {
    let x = g.panel(g.layout.panels - 1) as f64;
    let st = sky_at(hour(t));
    // Water mirrors the sky.
    for y in 48..56 {
        let hw = 18.0 * (1.0 - ((y as f64 - 52.0) / 4.5).powi(2)).max(0.0).sqrt();
        for i in -(hw as i32)..=(hw as i32) {
            let c = mix(st[1], st[2], (y - 48) as f64 / 8.0);
            let c = if (i + y + (f / 3) as i32) % 9 == 0 {
                mix(c, 0xffffff, 0.3)
            } else {
                c
            };
            put(g, c, js_round(x + 24.0) + i, y);
        }
    }
    // Reeds.
    for k in 0..6 {
        let rx = js_round(x + 6.0 + k as f64 * 1.6);
        let hgt = 5 + (k * 3) % 4;
        g.p(0x4a7a3a, rx as f64, (51 - hgt) as f64, 1, hgt);
        if k % 2 == 0 {
            g.p(0x7a4a2a, rx as f64, (51 - hgt) as f64, 1, 2);
        }
    }
    // The big tree the owl lives in.
    let tx = x + 46.0;
    shadow(g, tx - 6.0, 13.0, 26.0, t);
    g.p(0x5a3a24, tx, 30.0, 3, 20);
    g.p(0x4a2a1a, tx + 2.0, 30.0, 1, 20);
    for (ox, oy, r) in [
        (-5.0, 24.0, 6),
        (5.0, 23.0, 6),
        (0.0, 18.0, 7),
        (1.0, 28.0, 5),
    ] {
        g.disc(0x3a7a34, tx + 1.0 + ox, oy + 1.0, r);
        g.disc(0x5aa848, tx + 1.0 + ox, oy, r - 1);
        put(g, 0x8ad060, js_round(tx + ox - 1.0), (oy - 3.0) as i32);
    }
    // Ducks paddle by day.
    if night(t) < 0.7 {
        for k in 0..2 {
            let dx = x + 14.0 + ((t * 2.0 + k as f64 * 9.0) % 18.0) + k as f64 * 3.0;
            let dy = 48.0 + k as f64 * 2.0;
            g.spr(DUCK, DUCKP, dx, dy, false);
        }
    }
}

/// Things that drive by on the lane: the tractor at mid-morning, a cyclist
/// in the late afternoon.
fn traffic(g: &mut World, t: f64, f: u32) {
    let s_end = stage_at(g, g.layout.panels - 1, 70.0);
    if (2.4..6.6).contains(&t) {
        let u = (t - 2.4) / 4.2;
        let x = stage_x(g, -12.0 + u * (s_end + 12.0));
        let y = 46.0;
        g.p(0x3a8a3a, x, y, 9, 4);
        g.p(0x3a8a3a, x + 5.0, y - 4.0, 4, 4);
        g.p(0x9ad0f0, x + 6.0, y - 3.0, 2, 2);
        g.p(0x4a4a4a, x + 1.0, y - 2.0, 1, 2);
        g.disc(0x2a2a2a, x + 2.0, y + 5.0, 2);
        g.disc(0x2a2a2a, x + 7.0, y + 6.0, 1);
        put(g, 0x8a8a8a, js_round(x) + 2, js_round(y) + 5);
        // A puff of smoke from the stack.
        let pf = ((f / 3) % 3) as f64;
        put(g, 0x9a9a9a, js_round(x) + 1, js_round(y - 4.0 - pf));
        put(
            g,
            0xb8b8b8,
            js_round(x) - js_round(pf),
            js_round(y - 6.0 - pf),
        );
    }
    if (6.9..9.4).contains(&t) {
        let u = (t - 6.9) / 2.5;
        let x = stage_x(g, s_end - u * (s_end + 12.0));
        let y = 46.0;
        let spin = (f % 2) as f64;
        g.ring(0x2a2a2a, x, y + 5.0, 2.0);
        g.ring(0x2a2a2a, x + 7.0, y + 5.0, 2.0);
        put(g, 0x6a6a6a, js_round(x + spin), js_round(y) + 5);
        line(g, 0xd84a4a, x, y + 5.0, x + 3.0, y + 1.0);
        line(g, 0xd84a4a, x + 3.0, y + 1.0, x + 7.0, y + 5.0);
        g.p(0x4a6ad8, x + 3.0, y - 3.0, 2, 4);
        g.p(0xf0c8a0, x + 3.0, y - 5.0, 2, 2);
        g.p(0xf0d040, x + 3.0, y - 6.0, 2, 1);
    }
}

/// The rooster and the hens: a crow at dawn, pecking all day, back to roost
/// at dusk, a crow at the moon and the pillow.
fn rooster(g: &mut World, t: f64, f: u32) {
    let (px, py) = post(g);
    let on_post = (px - 4.0, py - 9.0);
    let hens = night(t) < 0.5 && (1.4..T_ROOST).contains(&t);
    if hens {
        for k in 0..3 {
            let hx = g.panel(0) as f64 + 24.0 + k as f64 * 9.0 + (t * 0.8 + k as f64).sin() * 3.0;
            g.spr(
                HEN[((f / 3 + k) % 2) as usize],
                HENP,
                hx,
                51.0 + (k % 2) as f64,
                k % 2 == 1,
            );
        }
    }
    if t < 1.2 {
        let pose = if (T_CROW..T_CROW + 0.6).contains(&t) {
            1
        } else {
            0
        };
        g.spr(ROOSTER[pose], ROO, on_post.0, on_post.1, false);
        if pose == 1 {
            crow_lines(g, on_post.0, on_post.1, t - T_CROW);
        }
    } else if t < 1.6 {
        // Hops down.
        let u = (t - 1.2) / 0.4;
        let y = on_post.1 + (BASE as f64 - 9.0 - on_post.1) * u - (u * PI).sin() * 4.0;
        g.spr(ROOSTER[0], ROO, on_post.0 - u * 6.0, y, false);
    } else if t < T_ROOST - 0.4 {
        // Struts and pecks in the yard.
        let x = on_post.0 - 6.0 - ((t - 1.6) * 0.9).sin() * 10.0;
        let pose = if (f / 4).is_multiple_of(3) { 3 } else { 0 };
        g.spr(
            ROOSTER[pose],
            ROO,
            x,
            BASE as f64 - 9.0,
            ((t * 0.9).cos()) < 0.0,
        );
    } else if t < T_ROOST {
        let u = (t - T_ROOST + 0.4) / 0.4;
        let y = BASE as f64 - 9.0 + (on_post.1 - BASE as f64 + 9.0) * u - (u * PI).sin() * 5.0;
        g.spr(ROOSTER[0], ROO, on_post.0 - 6.0 * (1.0 - u), y, false);
    } else if t < T_HIT {
        let pose = if t < T_WAKE {
            2
        } else if (T_MOONCROW..T_HIT).contains(&t) {
            1
        } else {
            0
        };
        g.spr(ROOSTER[pose], ROO, on_post.0, on_post.1, false);
        if pose == 2 {
            zzz(g, on_post.0 + 2.0, on_post.1 - 2.0, t);
        }
        if pose == 1 {
            crow_lines(g, on_post.0, on_post.1, t - T_MOONCROW);
        }
    } else {
        // Knocked off the post: tumbling into the grass, seeing stars.
        let u = ((t - T_HIT) / 0.35).min(1.0);
        let x = on_post.0 - 3.0 * u;
        let y = on_post.1 + (BASE as f64 - 6.0 - on_post.1) * u - (u * PI).sin() * 4.0;
        g.spr(ROOSTER[if u < 1.0 { 0 } else { 2 }], ROO, x, y, u > 0.5);
        if u >= 1.0 {
            for k in 0..3 {
                let a = t * 6.0 + k as f64 * 2.1;
                put(
                    g,
                    0xf8e070,
                    js_round(x + 4.0 + a.cos() * 5.0),
                    js_round(y - 1.0 + a.sin() * 1.5),
                );
            }
        }
        // Feathers drifting down.
        let v = t - T_HIT;
        for k in 0..12 {
            let kf = k as f64;
            let burst = ease(v / 0.3);
            let fx = on_post.0
                + 4.0
                + (rnd(kf + 60.0) - 0.5) * 22.0 * burst
                + (v * 5.0 + kf).sin() * 1.5;
            let fy = on_post.1 + 3.0 - rnd(kf + 61.0) * 9.0 * burst + v * v * 6.0;
            let c = if k % 3 == 0 { 0x3a9a6a } else { 0xf4efe4 };
            put(g, c, js_round(fx), js_round(fy));
            put(g, c, js_round(fx) + 1, js_round(fy) - (k % 2));
        }
    }
}

fn crow_lines(g: &mut World, x: f64, y: f64, dt: f64) {
    for k in 0..3 {
        let r = 2.0 + k as f64 * 2.5 + (dt * 10.0) % 2.5;
        for a in [-0.5, -0.25, 0.0, 0.25] {
            let a = PI + a - 0.3;
            put(
                g,
                0xfff4c0,
                js_round(x - 1.0 + a.cos() * r),
                js_round(y + 2.0 + a.sin() * r),
            );
        }
    }
}

fn zzz(g: &mut World, x: f64, y: f64, t: f64) {
    const Z: &[&str] = &["zzz", ".z.", "zzz"];
    for k in 0..2 {
        let u = (t * 0.7 + k as f64 * 0.5) % 1.0;
        g.spr(
            Z,
            &[('z', 0xc8d4ff)],
            x + u * 5.0 + k as f64 * 2.0,
            y - u * 8.0,
            false,
        );
    }
}

/// The pillow from the bedroom window to the rooster.
fn pillow(g: &mut World, t: f64) {
    if !(T_THROW..T_HIT + 0.05).contains(&t) {
        return;
    }
    let (px, py) = post(g);
    let (wx, wy) = (g.panel(0) as f64 + 20.0, 38.0);
    let u = (t - T_THROW) / (T_HIT - T_THROW);
    let x = wx + (px - 2.0 - wx) * u;
    let y = wy + (py - 6.0 - wy) * u - (u * PI).sin() * 7.0;
    g.p(0xd8dcf0, x - 2.0, y, 5, 3);
    put(g, 0xc8c8d8, js_round(x) - 3, js_round(y));
    put(g, 0xc8c8d8, js_round(x) + 3, js_round(y) + 2);
    g.p(0xa8b0d0, x - 2.0, y + 2.0, 5, 1);
}

/// Lights, drawn after the grading so they glow at night.
fn lights(g: &mut World, t: f64, f: u32) {
    let n = night(t);
    let x0 = g.panel(0) as f64 + 6.0;
    let h = hour(t);
    // The farmhouse bedroom: lit before dawn, and again when the rooster
    // wakes them.
    if h < 5.9 || t > T_WINDOW - 0.1 {
        g.p(0xf8d070, x0 + 12.0, 38.0, 4, 4);
        glow(g, 0xf8c060, x0 + 14.0, 40.0, 8.0, 0.4);
        if t > T_WINDOW {
            // The farmer in a nightcap.
            g.p(0xf0c8a0, x0 + 13.0, 39.0, 2, 2);
            g.p(0x4a6ad8, x0 + 12.0, 38.0, 3, 1);
            put(g, 0xffffff, js_round(x0) + 11, 37);
        }
    }
    if n < 0.3 {
        return;
    }
    let vx = g.panel(g.layout.panels.saturating_sub(2)) as f64;
    for (k, &(hx, top)) in [(vx + 2.0, 34.0), (vx + 14.0, 30.0), (vx + 42.0, 33.0)]
        .iter()
        .enumerate()
    {
        for (j, (wx, wy)) in [(2.0, 3.0), (7.0, 3.0), (2.0, 9.0)].iter().enumerate() {
            let seed = (k * 3 + j) as f64;
            // Lights go on one by one at dusk and some go off later.
            let on_at = 19.2 + rnd(seed + 7.0) * 1.2;
            let off_at = 22.5 + rnd(seed + 8.0) * 4.0;
            if (on_at..off_at).contains(&h) {
                g.p(0xf8d070, hx + wx, top + wy, 3, 4);
                blend(g, 0xffffff, js_round(hx + wx), js_round(top + wy), 0.5);
            }
        }
    }
    // The street lamp in the village and the café sign.
    let lx = vx + 40.0;
    g.p(0x2a2a34, lx, 34.0, 1, 16);
    g.p(0x2a2a34, lx - 1.0, 33.0, 3, 1);
    g.p(0xfff0b0, lx - 1.0, 34.0, 3, 2);
    glow(g, 0xf8d880, lx, 35.0, 12.0, 0.35 * n);
    // Barn lantern.
    if g.layout.panels >= 4 {
        let bx = g.panel(1) as f64 + 11.0;
        put(g, 0xf8c060, js_round(bx), 38);
        glow(g, 0xf8b050, bx, 38.0, 6.0, 0.4 * n);
    }
    // Fireflies by the pond and the owl's eyes.
    let px = g.panel(g.layout.panels - 1) as f64;
    for k in 0..8 {
        let kf = k as f64;
        let x = px + 6.0 + rnd(kf + 30.0) * 36.0 + (t * 1.3 + kf).sin() * 3.0;
        let y = 38.0 + rnd(kf + 31.0) * 10.0 + (t * 1.7 + kf * 2.0).cos() * 2.0;
        if (f + k * 5) % 11 < 7 {
            blend(g, 0xd8ff70, js_round(x), js_round(y), n);
            glow(g, 0xb8f060, x, y, 2.5, 0.3 * n);
        }
    }
    let blink = (f / 2).is_multiple_of(17);
    if !blink {
        put(g, 0xf8e040, js_round(px + 44.0), 25);
        put(g, 0xf8e040, js_round(px + 46.0), 25);
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    sky(g, t, f);
    let sky_copy = g.frame.rgba.clone();
    land(g);
    farm(g, t);
    fields(g, t);
    village(g, t);
    pond(g, t, f);
    traffic(g, t, f);
    rooster(g, t, f);
    // Grade everything drawn over the sky with the hour's light.
    let (amb, k) = amb_at(hour(t));
    if k > 0.0 {
        let w = g.w();
        for y in 0..g.h() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if g.frame.rgba[i..i + 3] != sky_copy[i..i + 3] {
                    let c = get(g, x, y);
                    let lum = (((c >> 16) & 255) + ((c >> 8) & 255) + (c & 255)) as f64 / 765.0;
                    put(g, mix(c, mix(amb, 0x000000, 0.4 * (1.0 - lum)), k), x, y);
                }
            }
        }
    }
    lights(g, t, f);
    pillow(g, t);
}

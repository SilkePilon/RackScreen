//! Broadside: two sailing ships meet on rolling waves off a palm island.
//! They trade cannon fire (muzzle flashes, smoke, splashes), a ball
//! brings down the dark ship's mainmast, and in the confusion the
//! captain's parrot flies off with his hat.

use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.0;

/// Hull, bow to the right; gunports are `g`.
const HULL: &[&str] = &[
    "kkkkkkk..........................",
    "kbbbbbk..........................",
    "kbybybkkkkkkkkkkkkkkkkkkkkkkkk.kk",
    "kbbbbbbbbbbbbbbbbbbbbbbbbbbbbbkk.",
    "kssssssssssssssssssssssssssssssk.",
    "kbbbbbgbbbbgbbbbgbbbbgbbbbbbbbk..",
    ".kbbbbbbbbbbbbbbbbbbbbbbbbbbbbk..",
    "..kdddddddddddddddddddddddddddk..",
    "...kkkkkkkkkkkkkkkkkkkkkkkkkkk...",
];
/// Gunport columns in `HULL`, row 5.
const PORTS: [f64; 4] = [6.0, 11.0, 16.0, 21.0];

const CAPTAIN: &[&str] = &[
    ".kkkkk.", "kkkwkkk", "..sss..", "..sse..", ".rrrr..", ".rgrr..", ".rrrr..", "..k.k..",
];
const PARROT: [&[&str]; 2] = [
    &["..bb....", "...bbgw.", "rrggggko", "..ggg..."],
    &[".....gw.", "rrggggko", "..gbb...", "...bb..."],
];
const PARROTP: &[(char, u32)] = &[
    ('g', 0x3ac04a),
    ('w', 0xf8f8f8),
    ('k', 0x101014),
    ('o', 0xf0b02a),
    ('r', 0xe03a3a),
    ('b', 0x3a6ae0),
];
const GULL: [&[&str]; 2] = [&["k...k", ".k.k.", "..k.."], &[".....", "kk.kk", "..k.."]];

struct Ship {
    /// Left edge of the hull and its waterline row.
    x: f64,
    y: f64,
    flip: bool,
    hull: u32,
    dark: u32,
    stripe: u32,
    sail: u32,
    sail_shade: u32,
    flag: u32,
}

impl Ship {
    /// World position of a hull pixel, honouring the flip.
    fn at(&self, hx: f64, hy: f64) -> (f64, f64) {
        let x = if self.flip {
            self.x + 32.0 - hx
        } else {
            self.x + hx
        };
        (x, self.y + hy)
    }
}

/// A mast from `(bx, by)`, leaning `ang` radians (positive: towards +x),
/// with square sails at `(from, to, half width)` along its length.
#[allow(clippy::too_many_arguments)]
fn mast(
    g: &mut World,
    bx: f64,
    by: f64,
    len: f64,
    ang: f64,
    sails: &[(f64, f64, f64)],
    sail: u32,
    shade: u32,
    flag: u32,
    t: f64,
) {
    let (ux, uy) = (ang.sin(), -ang.cos());
    let (vx, vy) = (ang.cos(), ang.sin());
    for &(a, b, half) in sails {
        let mut r = a;
        while r <= b {
            let belly = ((r - a) / (b - a) * PI).sin() * 1.2;
            let mut s = -half;
            while s <= half {
                let c = if s > half * 0.4 { shade } else { sail };
                let rr = r - belly * (1.0 - (s / half).powi(2));
                g.p(c, bx + ux * rr + vx * s, by + uy * rr + vy * s, 1, 1);
                s += 0.5;
            }
            r += 0.5;
        }
    }
    let mut r = 0.0;
    while r <= len {
        g.p(0x4a2e1a, bx + ux * r, by + uy * r, 1, 1);
        r += 0.5;
    }
    // A pennant streaming from the masthead.
    for k in 0..6 {
        let kf = k as f64;
        let wave = ((t * 7.0) - kf * 0.9).sin() * 0.7;
        let (px, py) = (bx + ux * (len - 1.0) - kf, by + uy * (len - 1.0) + wave);
        g.p(flag, px, py, 1, if k < 3 { 2 } else { 1 });
    }
}

fn draw_hull(g: &mut World, s: &Ship) {
    let pal = [
        ('k', 0x2a1a12),
        ('b', s.hull),
        ('d', s.dark),
        ('s', s.stripe),
        ('y', 0xffd27a),
        ('g', 0x1a1210),
    ];
    g.spr(HULL, &pal, s.x, s.y, s.flip);
    // Bowsprit.
    for k in 0..7 {
        let (x, y) = s.at(31.0 + k as f64, 2.0 - k as f64 * 0.5);
        g.p(0x4a2e1a, x, y, 1, 1);
    }
}

/// A cannon shot from `(x0, y0)` towards `(x1, y1)`, `u` in `0..=1`.
fn ball(g: &mut World, x0: f64, y0: f64, x1: f64, y1: f64, u: f64) {
    let x = x0 + (x1 - x0) * u;
    let y = y0 + (y1 - y0) * u - (u * PI).sin() * 10.0;
    g.p(0x1a1a22, x, y, 2, 2);
    g.p(0x6a6a7a, x, y, 1, 1);
}

/// A white splash column at `(x, y)`, `e` seconds old.
fn splash(g: &mut World, x: f64, y: f64, e: f64) {
    if !(0.0..0.9).contains(&e) {
        return;
    }
    let h = (e * PI / 0.9).sin() * 8.0;
    for k in 0..7 {
        let kf = k as f64 - 3.0;
        let hh = h * (1.0 - kf.abs() * 0.22);
        g.p(0xe8f6ff, x + kf, y - hh, 1, js_round(hh).max(1));
        g.p(0xffffff, x + kf * (1.0 + e * 2.0), y - hh - 1.0, 1, 1);
    }
    g.p(0xb8e0f0, x - 4.0 - e * 4.0, y, 9 + js_round(e * 8.0), 1);
}

/// A muzzle flash and a smoke puff that drifts up and grows, `e` old.
fn shot(g: &mut World, x: f64, y: f64, dir: f64, e: f64) {
    if !(0.0..2.2).contains(&e) {
        return;
    }
    if e < 0.15 {
        g.disc(0xfff4c0, x + dir * 2.0, y, 2);
        g.p(0xffb040, x + dir * 4.0, y, 2, 1);
    }
    for k in 0..3 {
        let kf = k as f64;
        let r = js_round(1.0 + e * 2.0 - kf * 0.5).max(1);
        let c = if e < 0.8 { 0xe8e8ec } else { 0xb8bcc8 };
        g.disc(
            c,
            x + dir * (3.0 + kf * 3.0 + e * 4.0),
            y - e * 5.0 - kf,
            r - js_round(e * 0.4),
        );
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, wf) = (g.w(), g.w() as f64);
    let fi = f as i32;
    g.bands(&[
        (0x3a8ad8, 8),
        (0x5aa6e4, 8),
        (0x86c4ee, 8),
        (0xb8e0f4, 6),
        (0xe0f2f8, 3),
        (0x5aa8d0, 27),
    ]);
    // A bright sun, puffy clouds drifting.
    let sun = (g.panel(3) + 40) as f64;
    g.disc(0xfff8d8, sun, 9.0, 4);
    g.disc(0xffffff, sun, 9.0, 2);
    for k in 0..g.spread(6) {
        let kf = k as f64;
        let span = wf + 40.0;
        let x = ((rnd(kf + 3.0) * span + t * (2.0 + kf % 2.0)) % span) - 30.0;
        let y = 5.0 + rnd(kf + 4.0) * 14.0;
        let len = 10.0 + rnd(kf + 5.0) * 10.0;
        g.disc(0xffffff, x + len * 0.35, y, 3);
        g.disc(0xffffff, x + len * 0.65, y - 1.0, 4);
        g.p(0xffffff, x, y + 1.0, js_round(len), 3);
        g.p(0xd0e4f0, x + 1.0, y + 4.0, js_round(len) - 2, 1);
    }
    // The palm island on panel 0 and a far sail on the last panel.
    let ix = (g.panel(0) + 26) as f64;
    for r in 0..6 {
        let half = 4 + r * 2;
        g.p(0x4a8a5a, ix - half as f64, 27.0 + r as f64, half * 2, 1);
    }
    g.p(0xe8d8a0, ix - 16.0, 32.0, 32, 1);
    for (px, lean) in [(ix - 4.0, -1.0), (ix + 3.0, 1.0)] {
        for j in 0..9 {
            g.p(
                0x6a4a2a,
                px + lean * (j as f64 / 9.0).powi(2) * 2.0,
                27.0 - j as f64,
                1,
                1,
            );
        }
        let top = (px + lean * 2.0, 18.0);
        for (dx, dy) in [
            (-3.0, 1.0),
            (-2.0, 0.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (3.0, 1.0),
        ] {
            g.p(0x2a6a3a, top.0 + dx, top.1 + dy, 1, 1);
        }
        g.p(0x2a6a3a, top.0 - 4.0, top.1 + 2.0, 1, 1);
        g.p(0x2a6a3a, top.0 + 4.0, top.1 + 2.0, 1, 1);
    }
    let far = (g.panel(3) + 14) as f64 + t * 0.6;
    g.p(0x6a7a8a, far, 30.0, 6, 2);
    g.p(0x8a9aaa, far + 2.0, 25.0, 1, 5);
    g.p(0xc8d4dc, far + 1.0, 26.0, 3, 3);
    // The sea: far, middle and near bands of rolling crests.
    for (y0, y1, c, crest, sp, k) in [
        (33, 38, 0x5aa8d0, 0x9ad0e8, 3.0, 0.31),
        (38, 45, 0x3a86c0, 0x7ab8e0, 6.0, 0.22),
        (45, 60, 0x2a66a8, 0x5a9ad0, 10.0, 0.16),
    ] {
        g.p(c, 0.0, y0 as f64, w, y1 - y0);
        for y in (y0..y1).step_by(2) {
            for x in 0..w {
                let v = ((x as f64 + t * sp * if y % 4 == 0 { 1.0 } else { -0.6 }) * k
                    + y as f64 * 1.7)
                    .sin();
                if v > 0.75 {
                    g.p(crest, x as f64, y as f64, 1, 1);
                }
            }
        }
    }
    // Sun glitter on the water under the sun.
    for k in 0..12 {
        let kf = k as f64;
        if (fi + k * 5) % 7 < 3 {
            g.p(
                0xffffff,
                sun - 6.0 + rnd(kf + 80.0) * 12.0,
                34.0 + rnd(kf + 81.0) * 8.0,
                2,
                1,
            );
        }
    }
    // Gulls wheel overhead.
    for k in 0..g.spread(4) {
        let kf = k as f64;
        let x = ((rnd(kf + 20.0) * wf + t * 9.0 * (1.0 + kf % 2.0)) % (wf + 20.0)) - 10.0;
        let y = 8.0 + rnd(kf + 21.0) * 10.0 + (t * 2.0 + kf).sin() * 2.0;
        g.spr(
            GULL[((f / 3 + k) % 2) as usize],
            &[('k', 0xf8f8f8)],
            x,
            y,
            false,
        );
    }

    // The ships sail in and heave to, rocking on the swell.
    let sail_in = ((t / 2.2).min(1.0) * PI / 2.0).sin();
    let ax = (g.panel(1) + 10) as f64 - (1.0 - sail_in) * 60.0;
    let bx = (g.panel(2) + 16) as f64 + (1.0 - sail_in) * 60.0;
    let ay = 38.0 + js_round((t * 2.2).sin() * 0.8) as f64;
    let by = 38.0 + js_round((t * 2.2 + 1.7).sin() * 0.8) as f64;
    let a = Ship {
        x: ax,
        y: ay,
        flip: false,
        hull: 0x8a5a2a,
        dark: 0x5a3a1a,
        stripe: 0xe0c060,
        sail: 0xf8f0dc,
        sail_shade: 0xd8ccb0,
        flag: 0xd83a3a,
    };
    let b = Ship {
        x: bx,
        y: by,
        flip: true,
        hull: 0x3a2e3a,
        dark: 0x241a24,
        stripe: 0x9a2a2a,
        sail: 0x5a5260,
        sail_shade: 0x443c4a,
        flag: 0x101014,
    };
    let topple = 5.7;
    for s in [&a, &b] {
        let dir = if s.flip { -1.0 } else { 1.0 };
        let (fx, fy) = s.at(23.0, 2.0);
        mast(
            g,
            fx,
            fy,
            15.0,
            0.0,
            &[(6.0, 12.0, 4.0)],
            s.sail,
            s.sail_shade,
            s.flag,
            t,
        );
        let (mx, my) = s.at(14.0, 2.0);
        let ang = if s.flip && t > topple {
            let u = ((t - topple) / 1.1).min(1.0);
            u * u * 1.9 * dir
        } else {
            0.0
        };
        if !(s.flip && t > topple + 1.1) {
            mast(
                g,
                mx,
                my,
                22.0,
                ang,
                &[(7.0, 13.0, 6.0), (14.0, 19.0, 4.5)],
                s.sail,
                s.sail_shade,
                s.flag,
                t,
            );
        }
        // The dark ship flies a black flag with a white skull.
        if s.flip && t <= topple {
            g.p(0x101014, mx - 6.0, my - 24.0, 5, 4);
            g.p(0xf0f0f0, mx - 4.0, my - 23.0, 2, 1);
            g.p(0xf0f0f0, mx - 4.0, my - 22.0, 1, 1);
        }
        draw_hull(g, s);
    }
    splash(g, b.at(14.0, 2.0).0 - 20.0, 46.0, t - topple - 1.0);
    // The captain on the dark ship's quarterdeck, and his parrot.
    let (cx, cy) = b.at(3.0, -7.0);
    let lost = 6.6;
    let mut cap: Vec<&str> = CAPTAIN.to_vec();
    if t > lost {
        cap[0] = ".......";
        cap[1] = "..sss..";
    }
    g.spr(
        &cap,
        &[
            ('k', 0x101014),
            ('w', 0xf0f0f0),
            ('s', 0xf0c090),
            ('e', 0x2a1a12),
            ('r', 0xb02a2a),
            ('g', 0xe0c060),
        ],
        cx - 3.0,
        cy,
        false,
    );
    if t > lost + 0.3 {
        // He shakes his fist after the bird.
        let up = (f / 2).is_multiple_of(2);
        g.p(0xf0c090, cx - 5.0, cy + if up { 2.0 } else { 3.0 }, 2, 2);
        g.p(0xb02a2a, cx - 4.0, cy + 4.0, 1, 2);
    }
    let (px, py, pf) = if t < lost {
        (cx, cy + 2.0, 1)
    } else {
        // Off with the hat to the last screen, where it hovers wearing it.
        let u = ((t - lost) / 2.4).min(1.0);
        let e = 1.0 - (1.0 - u) * (1.0 - u);
        let (x0, y0) = (cx + 3.0, cy - 2.0);
        let (x1, y1) = ((g.panel(3) + 27) as f64, 22.0);
        (
            x0 + (x1 - x0) * e,
            y0 + (y1 - y0) * e - (u * PI).sin() * 8.0 + (t * 5.0).sin() * 1.5,
            ((f / 2) % 2) as usize,
        )
    };
    g.spr(PARROT[pf], PARROTP, px, py, false);
    if t >= lost {
        g.spr(
            &[".kkk.", "kkwkk"],
            &[('k', 0x101014), ('w', 0xf0f0f0)],
            px + 3.0,
            py - if pf == 0 { 1.0 } else { 2.0 },
            false,
        );
    }

    // The broadsides: (time, from A?, gunport, hits the mast?).
    let volleys: [(f64, bool, usize, bool); 8] = [
        (2.4, true, 0, false),
        (2.6, true, 2, false),
        (2.8, true, 3, false),
        (3.5, false, 1, false),
        (3.7, false, 2, false),
        (3.9, false, 3, false),
        (4.7, true, 1, false),
        (4.9, true, 2, true),
    ];
    let flight = 0.8;
    for (k, (t0, from_a, port, hit)) in volleys.into_iter().enumerate() {
        let (src, dst) = if from_a { (&a, &b) } else { (&b, &a) };
        let dir = if from_a { 1.0 } else { -1.0 };
        let (gx, gy) = src.at(PORTS[port], 5.0);
        let (gx, gy) = (gx + dir * 2.0, gy);
        shot(g, gx, gy, dir, t - t0);
        let e = t - t0;
        let (tx, ty) = if hit {
            dst.at(14.0, -12.0)
        } else {
            let miss = rnd(k as f64 + 40.0) * 6.0;
            let (x, _) = dst.at(16.0, 0.0);
            (x - dir * (if from_a { 18.0 } else { 20.0 } + miss), 44.0)
        };
        if (0.0..flight).contains(&e) {
            ball(g, gx, gy, tx, ty, e / flight);
        } else if !hit {
            splash(g, tx, ty, e - flight);
        } else if e < flight + 0.6 {
            // Splinters fly off the struck mast.
            let s = e - flight;
            for j in 0..8 {
                let a = j as f64 / 8.0 * PI * 2.0;
                let c = if j % 2 == 0 { 0xc89a5a } else { 0xffd060 };
                g.p(
                    c,
                    tx + a.cos() * s * 14.0,
                    ty + a.sin() * s * 10.0 + s * s * 10.0,
                    1,
                    1,
                );
            }
        }
    }
    // Near swell washes over the hulls' waterline, foam on its crests.
    for x in 0..w {
        let xf = x as f64;
        let y = 46.0 + (xf * 0.18 - t * 2.6).sin() * 1.2 + (xf * 0.07 + t).sin();
        let y = js_round(y);
        g.p(0x2a66a8, xf, y as f64, 1, 60 - y);
        let foam = (xf * 0.18 - t * 2.6).cos() > 0.6;
        g.p(if foam { 0xe8f6ff } else { 0x5a9ad0 }, xf, y as f64, 1, 1);
        if (x + fi / 2) % 9 == 0 {
            g.p(0x1e4e8a, xf, (y + 5) as f64, 3, 1);
        }
        if (x * 3 + fi / 3) % 17 == 0 {
            g.p(0x5a9ad0, xf, (y + 9) as f64, 2, 1);
        }
    }
    // The fallen mast lies in the water by the dark ship.
    if t > topple + 1.1 {
        let (mx, _) = b.at(14.0, 2.0);
        g.p(
            0x4a2e1a,
            mx - 22.0,
            47.0 + js_round((t * 2.0).sin() * 0.6) as f64,
            20,
            1,
        );
        g.p(
            0x5a5260,
            mx - 16.0,
            46.0 + js_round((t * 2.0).sin() * 0.6) as f64,
            6,
            2,
        );
        g.p(
            0x5a5260,
            mx - 10.0,
            46.0 + js_round((t * 2.0).sin() * 0.6) as f64,
            5,
            2,
        );
    }
    // A shark fin patrols off the island; a bottle bobs by.
    let p0 = g.panel(0) as f64;
    let sw = (t * 0.5).sin();
    let fx = p0 + 30.0 + sw * 20.0;
    let fy = 50.0 + js_round((t * 3.0).sin() * 0.6) as f64;
    let fin: &[&str] = &["..k", ".kk", "kkk"];
    g.spr(fin, &[('k', 0x3a4a5a)], fx, fy, (t * 0.5).cos() < 0.0);
    g.p(0xe8f6ff, fx - 1.0, fy + 3.0, 5, 1);
    let bx0 = p0 + 12.0 + t * 1.5;
    let bob = js_round((t * 3.0 + 1.0).sin()) as f64;
    g.p(0x6ac08a, bx0, 41.0 + bob, 2, 3);
    g.p(0x8a6a4a, bx0, 40.0 + bob, 1, 1);
    g.p(0xffffff, bx0 + 1.0, 42.0 + bob, 1, 1);
    // A sailor on the light ship cheers after the hit.
    let (sx, sy) = a.at(9.0, -4.0);
    let cheer = t > topple && (f / 2).is_multiple_of(2);
    g.p(0xf0c090, sx, sy, 1, 1);
    g.p(0xf0f0f0, sx, sy + 1.0, 1, 2);
    g.p(0x3a4a8a, sx, sy + 3.0, 1, 1);
    g.p(0xf0c090, sx - 1.0, sy + if cheer { 0.0 } else { 1.0 }, 1, 1);
    g.p(0xf0c090, sx + 1.0, sy + if cheer { 0.0 } else { 1.0 }, 1, 1);
}

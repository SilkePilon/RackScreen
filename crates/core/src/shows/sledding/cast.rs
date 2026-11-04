//! The cast of the snow day: the kid on the sled, the dog, the skater and
//! the snowman (with its flying head and hat).

use super::super::fx::{blend, glow, line, put, stage_at, stage_x};
use super::super::world::{ease, js_round, rnd, World};
use super::*;
use std::f64::consts::PI;

const KIDP: &[(char, u32)] = &[
    ('k', OUT),
    ('w', 0xffffff),
    ('y', 0xf0b830),
    ('Y', 0xc07a1c),
    ('s', 0xf8d0a8),
    ('c', 0xf09080),
    ('e', OUT),
    ('g', 0x3aa860),
    ('b', 0x3a64c8),
    ('B', 0x2a4490),
    ('o', 0x5a3a2a),
    ('d', 0xd83838),
    ('D', 0x8a2028),
    ('n', 0x7a7a8a),
];

/// The kid sitting, facing right, arms drawn separately.
const KID: &[&str] = &[
    "...kwk.....",
    "..kyyyk....",
    ".kyyyyyk...",
    ".kYYYYYYk..",
    ".ksssesk...",
    ".kcsssssk..",
    "..kssskk...",
    ".kgwgggk...",
    ".kbbbbbk...",
    ".kBbbbbbook",
    "..kkkkkkkk.",
];
const SLED: &[&str] = &[
    "............n.",
    "kdddddddddddkn",
    "kDDDDDDDDDDDkn",
    "..n....n...nn.",
    ".nnnnnnnnnnn..",
];

const DOGP: &[(char, u32)] = &[
    ('k', OUT),
    ('b', 0xc08048),
    ('E', 0x7a4a26),
    ('e', OUT),
    ('l', 0xf0dcb8),
    ('n', 0x1a1018),
    ('r', 0xd83838),
];
const DOG_RUN: [&[&str]; 2] = [
    &[
        "..........kkk...",
        "k........kEbbk..",
        "bk.......kEbebkk",
        ".bkkkkkkkrbbbbln",
        "..kbbbbbbbbbkkk.",
        "..kbbbbbbbbbk...",
        "..kklllllkkk....",
        ".kbk.....kbk....",
        "kk........kk....",
    ],
    &[
        "..........kkk...",
        "..k......kEbbk..",
        ".kbk.....kEbebkk",
        "...kkkkkkrbbbbln",
        "..kbbbbbbbbbkkk.",
        "..kbbbbbbbbbk...",
        "..kklllllkkk....",
        "...kbk..kbk.....",
        "...kk...kk......",
    ],
];
const DOG_SIT: &[&str] = &[
    "......kkk...",
    ".....kbbbk..",
    "....kEbebkkk",
    "....kEbbblln",
    "....kbrbbkk.",
    "...kbbllbk..",
    "..kbbbllbk..",
    "..kbbbbbbk..",
    "..kbkbbkbk..",
    "..kkkkkkkk..",
];

const SKATERP: &[(char, u32)] = &[
    ('k', OUT),
    ('h', 0x8a4a2a),
    ('s', 0xf8d0a8),
    ('p', 0xc04898),
    ('P', 0x7a2a6a),
    ('l', 0x3a2a4a),
    ('w', 0xffffff),
    ('N', 0xa8b4c8),
];
const SKATER: &[&str] = &[
    "..kkk..", ".khhhk.", ".khssk.", "..ksk..", ".kpppk.", ".kpppk.", "kpppppk", "kPPPPPk",
    "..klk..", "..klk..", ".kwwk..", ".NNNN..",
];

/// A sprite whose columns shift down by `slope` per pixel right of `px`.
fn spr_lean(
    g: &mut World,
    rows: &[&str],
    pal: &[(char, u32)],
    x: f64,
    y: f64,
    slope: f64,
    px: f64,
) {
    let (xi, yi) = (js_round(x), js_round(y));
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if let Some(&(_, col)) = pal.iter().find(|(k, _)| *k == ch) {
                let dy = js_round(slope * (xi as f64 + c as f64 - px));
                put(g, col, xi + c as i32, yi + r as i32 + dy);
            }
        }
    }
}

fn snowman_at(g: &World) -> f64 {
    g.panel(last(g)) as f64 + 15.0
}

/// The snowman, or what is left of it.
pub(super) fn snowman(g: &mut World, t: f64) {
    let x = snowman_at(g);
    let hit = t >= T_LAND;
    let ball = |g: &mut World, cx: f64, cy: f64, r: i32| {
        g.disc(OUT, cx, cy, r + 1);
        g.disc(SHADE, cx, cy, r);
        g.disc(SNOW, cx - 1.0, cy - 1.0, r - 1);
    };
    if !hit {
        ball(g, x, FLOOR - 6.0, 6);
        ball(g, x, FLOOR - 15.0, 4);
        line(g, 0x6a4028, x - 4.0, FLOOR - 16.0, x - 9.0, FLOOR - 20.0);
        line(g, 0x6a4028, x + 4.0, FLOOR - 16.0, x + 9.0, FLOOR - 19.0);
        put(g, 0x6a4028, js_round(x) - 9, js_round(FLOOR) - 21);
        for dy in [-16.0, -13.0, -8.0] {
            put(g, OUT, js_round(x), js_round(FLOOR + dy));
        }
        g.p(0xd83838, x - 3.0, FLOOR - 19.0, 7, 1);
        head(g, x, FLOOR - 22.0, 0.0, true);
        hat(g, x, FLOOR - 25.0, 0.0);
        return;
    }
    // A stump of the bottom ball, and the scattered rest.
    for r in 0..4 {
        let half = 6 - (r as f64 * 0.3) as i32;
        g.p(
            OUT,
            x - half as f64 - 1.0,
            FLOOR - 1.0 - r as f64,
            2 * half + 3,
            1,
        );
        g.p(
            if r == 3 { 0xffffff } else { SNOW },
            x - half as f64,
            FLOOR - 1.0 - r as f64,
            2 * half + 1,
            1,
        );
    }
    let u = t - T_LAND;
    for k in 0..30 {
        let kf = k as f64;
        let a = -PI * (0.1 + rnd(kf + 90.0) * 0.75);
        let sp = 14.0 + rnd(kf + 91.0) * 26.0;
        let fall = (1.0 - (-u * 2.5).exp()) / 2.5;
        let px = x + a.cos() * sp * fall;
        let py = (FLOOR - 12.0 + rnd(kf + 92.0) * 6.0 + a.sin() * sp * fall + 30.0 * u * u)
            .min(FLOOR - 1.0);
        let big = k % 4 == 0;
        let c = if k % 3 == 0 { SHADE } else { 0xffffff };
        put(g, c, js_round(px), js_round(py));
        if big {
            put(g, c, js_round(px) + 1, js_round(py));
            put(g, SHADE, js_round(px), js_round(py) + 1);
        }
    }
    if u < 0.35 {
        glow(g, 0xffffff, x, FLOOR - 12.0, 12.0, 0.9 * (1.0 - u / 0.35));
    }
    // The stick arms cartwheel away.
    for (side, sp) in [(-1.0, 7.0), (1.0, 13.0)] {
        let v = u.min(0.9);
        let cx = x + side * sp * v * 1.6;
        let cy = (FLOOR - 16.0 - 18.0 * v + 30.0 * v * v).min(FLOOR - 1.0);
        let a = if u < 0.9 { u * 14.0 * side } else { 0.2 };
        line(
            g,
            0x6a4028,
            cx - a.cos() * 3.0,
            cy - a.sin() * 3.0,
            cx + a.cos() * 3.0,
            cy + a.sin() * 3.0,
        );
    }
}

/// The snowman's head, carrot pointing along angle `a`.
fn head(g: &mut World, x: f64, y: f64, a: f64, nose: bool) {
    g.disc(OUT, x, y, 4);
    g.disc(SHADE, x, y, 3);
    g.disc(SNOW, x - 0.5, y - 0.5, 2);
    let (c, s) = (a.cos(), a.sin());
    for e in [-1.0, 1.0] {
        let (ex, ey) = (0.5 - 1.2 * e * s, -1.2 + 0.4 * e * c);
        let (rx, ry) = (ex * c - ey * s * 0.3, ey + ex * s * 0.3);
        put(g, OUT, js_round(x + rx), js_round(y + ry));
    }
    if nose {
        for i in 1..5 {
            let d = 1.5 + i as f64;
            put(
                g,
                if i < 4 { 0xf08a28 } else { 0xd86a18 },
                js_round(x + c * d),
                js_round(y + 0.5 + s * d),
            );
        }
    }
}

/// The top hat, `lean` shearing it as it tumbles.
fn hat(g: &mut World, x: f64, bottom: f64, lean: f64) {
    const HAT: &[&str] = &[".kkkkk.", ".khhhk.", ".khhhk.", ".krrrk.", "kkkkkkk"];
    let pal = [('k', 0x1a1622), ('h', 0x3a3448), ('r', 0xc83838)];
    spr_lean(g, HAT, &pal, x - 3.0, bottom - 5.0, lean, x);
    put(
        g,
        0x5a5468,
        js_round(x) - 1,
        js_round(bottom) - 4 + js_round(-lean),
    );
}

fn hat_pos(g: &World, t: f64) -> (f64, f64, f64) {
    let k = kid(g, T_HAT);
    let (x0, y0) = (snowman_at(g), FLOOR - 25.0);
    let (x1, y1) = (k.x + 1.0, FLOOR - 15.0);
    if t >= T_HAT {
        let kk = kid(g, t);
        let bob = if kk.pose == Pose::Laugh && (t * 6.0) as i32 % 2 == 0 {
            -1.0
        } else {
            0.0
        };
        return (x1, y1 + bob, -0.15);
    }
    let u = (t - T_LAND) / (T_HAT - T_LAND);
    let lean = (u * 9.0 * PI).sin() * 0.8;
    (
        x0 + (x1 - x0) * u,
        y0 + (y1 - y0) * u - 14.0 * 4.0 * u * (1.0 - u),
        lean,
    )
}

fn head_pos(g: &World, t: f64) -> (f64, f64, f64) {
    let (x0, y0) = (snowman_at(g), FLOOR - 22.0);
    let k = kid(g, t.max(T_STOP));
    let (x1, y1) = (k.x - 5.0, k.y - 8.0);
    let u = ((t - T_LAND) / (T_STOP - T_LAND)).clamp(0.0, 1.0);
    let a = PI * ease(u) * 3.0;
    (
        x0 + (x1 - x0) * u,
        y0 + (y1 - y0) * u - 11.0 * 4.0 * u * (1.0 - u),
        a,
    )
}

pub(super) fn draw_kid(g: &mut World, t: f64, f: u32) {
    let k = kid(g, t);
    let (sx, sy) = (k.x - 7.0, k.y - 5.0);
    let pivot = k.x;
    let dyc = |c: f64| js_round(k.slope * (c - pivot));
    // The snowman's head riding pillion, behind the kid.
    if t >= T_LAND {
        let (hx, hy, a) = head_pos(g, t);
        let nose = t < T_SNATCH;
        head(g, hx, hy, a, nose);
    }
    // The scarf streams out behind.
    let neck = (k.x - 3.0, k.y - 13.0 + dyc(k.x - 3.0) as f64);
    let len = (2.0 + k.speed * 0.12).min(8.0);
    for i in 0..js_round(len) {
        let fi = i as f64;
        let wave = (t * 18.0 - fi * 0.9).sin() * fi * 0.12 * (k.speed / 40.0).min(1.0);
        let droop = fi * (1.0 - (k.speed / 30.0).min(1.0)) * 0.8;
        let c = if (i / 2) % 2 == 0 { 0x3aa860 } else { 0xffffff };
        put(g, c, js_round(neck.0 - fi), js_round(neck.1 + wave + droop));
    }
    spr_lean(g, KID, KIDP, sx + 3.0, sy - 10.0, k.slope, pivot);
    // Arms: on the rope, flung up in the air, or waving.
    let sh = (k.x + 1.0, k.y - 7.0 + dyc(k.x + 1.0) as f64);
    let hands: &[(f64, f64)] = match k.pose {
        Pose::Ride | Pose::Sit => &[(5.0, 1.0)],
        Pose::Fly => &[(3.0, -6.0), (-1.0, -6.0)],
        Pose::Dazed => &[(3.0, 2.0)],
        Pose::Laugh => {
            if (f / 3).is_multiple_of(2) {
                &[(3.0, -6.0), (4.0, -4.0)]
            } else {
                &[(4.0, -4.0), (2.0, -7.0)]
            }
        }
    };
    for &(hx, hy) in hands {
        line(g, 0x3a64c8, sh.0, sh.1, sh.0 + hx, sh.1 + hy);
        put(g, 0xe04848, js_round(sh.0 + hx), js_round(sh.1 + hy));
    }
    // Mouth: wide open in the air and when laughing.
    if matches!(k.pose, Pose::Fly | Pose::Laugh) {
        put(
            g,
            0x8a2a3a,
            js_round(k.x) + 1,
            js_round(k.y) - 10 + dyc(k.x + 1.0),
        );
    }
    // The tow rope from the mittens to the sled's nose.
    if matches!(k.pose, Pose::Ride | Pose::Sit) {
        line(
            g,
            0xe8d8a0,
            sh.0 + 5.0,
            sh.1 + 1.0,
            k.x + 6.0,
            k.y - 4.0 + dyc(k.x + 6.0) as f64,
        );
    }
    spr_lean(g, SLED, KIDP, sx, sy, k.slope, pivot);
    // Dusted with snowman after the crash.
    if t >= T_LAND {
        for i in 0..7 {
            let fi = i as f64;
            let (dx, dy) = (
                (rnd(fi + 40.0) * 9.0) as i32,
                (rnd(fi + 41.0) * 10.0) as i32,
            );
            put(g, 0xffffff, js_round(sx) + 4 + dx, js_round(sy) - 9 + dy);
        }
    }
    // Spray kicked up behind a fast sled.
    if k.pose == Pose::Ride || (t > T_LAND && t < T_STOP) {
        let n = (k.speed * 0.25).min(10.0) as i32;
        for i in 0..n {
            let fi = i as f64 + (f % 4) as f64 * 3.0;
            let px = k.x - 8.0 - rnd(fi + 50.0) * 8.0;
            let py = k.y - 1.0 - rnd(fi + 51.0) * 4.0 + dyc(k.x - 8.0) as f64;
            blend(
                g,
                0xffffff,
                js_round(px),
                js_round(py),
                0.9 - rnd(fi + 52.0) * 0.4,
            );
        }
    }
    // Dazed stars circling the hat.
    if (T_STOP..T_DOG).contains(&t) {
        for i in 0..3 {
            let a = t * 6.0 + i as f64 * PI * 2.0 / 3.0;
            let (px, py) = (k.x + 1.0 + a.cos() * 6.0, k.y - 22.0 + a.sin() * 1.5);
            put(g, 0xffe040, js_round(px), js_round(py));
            blend(g, 0xfff4a0, js_round(px) + 1, js_round(py), 0.5);
        }
    }
    let (hx, hy, lean) = hat_pos(g, t);
    if t >= T_LAND {
        hat(g, hx, hy, lean);
    }
}

/// The dog: (centre x, feet y, sprite, facing left, carrot).
fn dog(g: &World, t: f64, f: u32) -> (f64, f64, &'static [&'static str], bool, bool) {
    let sd0 = stage_at(g, 0, 21.0);
    let sd1 = stage_at(g, last(g), 26.0);
    let td0 = T_PUSH + 0.3;
    if t < td0 {
        return (stage_x(g, sd0), TOP, DOG_SIT, false, false);
    }
    let run = DOG_RUN[(f / 2 % 2) as usize];
    if t < T_DOG {
        // A few bounds behind the sled, then flat out to the pond.
        let chase = |t: f64| (sd0 + (t - td0) * 30.0).min(ride_s(g, t) - 16.0);
        let s = if t < T_LAUNCH {
            chase(t)
        } else {
            let s0 = chase(T_LAUNCH);
            let u = (t - T_LAUNCH) / (T_DOG - T_LAUNCH);
            s0 + (sd1 - s0) * (u * 1.3 - 0.3 * u * u)
        };
        let hop = ((t * 12.0).sin().abs()) * 1.5;
        return (stage_x(g, s), ground_s(g, s) - hop, run, false, false);
    }
    let p = g.panel(last(g)) as f64;
    if t < T_SNATCH + 0.2 {
        return (p + 26.0, FLOOR, DOG_SIT, false, t >= T_SNATCH);
    }
    if t < T_SNATCH + 0.9 {
        let u = ease((t - T_SNATCH - 0.2) / 0.7);
        return (p + 26.0 - 9.0 * u, FLOOR, run, true, true);
    }
    (p + 17.0, FLOOR, DOG_SIT, false, true)
}

pub(super) fn draw_dog(g: &mut World, t: f64, f: u32) {
    let (x, y, rows, left, carrot) = dog(g, t, f);
    let top = y - rows.len() as f64;
    let half = (rows[0].len() / 2) as f64;
    g.spr(rows, DOGP, x - half, top, left);
    let dir = if left { -1.0 } else { 1.0 };
    let sit = rows.len() == DOG_SIT.len();
    // A wagging tail when it sits.
    if sit {
        let wag = if f % 4 < 2 { 5 } else { 6 };
        put(g, OUT, js_round(x) - 6, js_round(top) + wag);
        put(g, 0xc08048, js_round(x) - 5, js_round(top) + wag);
    }
    if carrot {
        let mx = x + dir * (rows[0].len() as f64 - half);
        for i in 0..4 {
            let c = if i < 3 { 0xf08a28 } else { 0xd86a18 };
            put(g, c, js_round(mx + dir * i as f64), js_round(top) + 4);
        }
        put(g, 0x4aa040, js_round(mx + dir * 4.0), js_round(top) + 3);
    }
    // A bark as the sled leaves.
    if (T_PUSH - 0.2..T_PUSH + 0.5).contains(&t) && (f / 2).is_multiple_of(2) {
        for (dx, dy) in [(8, 0), (9, -1), (8, 2), (10, 2)] {
            put(g, 0xffffff, js_round(x) + dx, js_round(top) + dy);
        }
    }
}

/// The skater: (x, feet y, facing left, airborne lift, pose).
fn skater(g: &World, t: f64) -> (f64, f64, bool, f64) {
    let p = g.panel(last(g)) as f64;
    let path = |t: f64| -> f64 {
        if t < 6.0 {
            40.0 + 12.0 * (1.1 * (t - 6.0)).cos()
        } else if t < T_STOP {
            52.0 - (t - 6.0) * 11.0
        } else {
            // Off she glides, out of the picture.
            52.0 - (T_STOP - 6.0) * 11.0 - (t - T_STOP) * 16.0
        }
    };
    let lx = path(t);
    let left = path(t + 0.05) < lx;
    // Leaps the sled where their paths cross.
    let (mut a, mut b) = (T_LAND, T_STOP);
    for _ in 0..20 {
        let m = (a + b) / 2.0;
        if slide_lx(m) < path(m) {
            a = m;
        } else {
            b = m;
        }
    }
    let u = (t - a + 0.3) / 0.6;
    let lift = if (0.0..1.0).contains(&u) {
        14.0 * 4.0 * u * (1.0 - u)
    } else {
        0.0
    };
    let y = if t < 6.0 {
        54.0 - 2.0 * (1.1 * (t - 6.0)).sin()
    } else {
        56.0
    };
    (p + lx, y, left, lift)
}

pub(super) fn draw_skater(g: &mut World, t: f64, f: u32) {
    let (x, y, left, lift) = skater(g, t);
    let flip = left ^ (lift > 0.0 && (f / 2).is_multiple_of(2));
    let top = y - lift - SKATER.len() as f64;
    g.spr(SKATER, SKATERP, x - 3.0, top, flip);
    let sh = top + 5.0;
    if lift > 0.0 {
        // Tucked arms, and a spin blur.
        blend(g, 0xffffff, js_round(x) - 4, js_round(sh) + 2, 0.5);
        blend(g, 0xffffff, js_round(x) + 4, js_round(sh) + 2, 0.5);
    } else {
        line(g, 0xc04898, x - 2.0, sh, x - 5.0, sh + 1.0);
        line(g, 0xc04898, x + 2.0, sh, x + 5.0, sh + 1.0);
    }
    // Her shadow on the ice.
    if lift > 0.0 {
        g.p(0x9ac4dc, x - 2.0, y, 5, 1);
    }
}

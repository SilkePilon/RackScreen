//! Giant robot at dusk: searchlights sweep a sunset over a city of lit
//! windows; a huge mech strides in from the left, every step shaking the
//! ground and scattering pigeons, stops on the third screen, turns its head
//! to the viewer with glowing eyes and waves.

use super::scifi::{bayer, glow, mix, shake, tint};
use super::world::{js_round, rnd, World};
use std::f64::consts::PI;

pub const SECS: f64 = 11.0;

/// The walk ends here; then the head turns and the arm comes up.
const T_STOP: f64 = 7.6;
const T_TURN: f64 = 8.0;
const T_WAVE: f64 = 8.6;

const STREET: f64 = 53.0;
const THIGH: f64 = 10.0;
const SHIN: f64 = 11.0;
/// Hip travel per full walk cycle, so a planted foot barely slides.
const CYCLE: f64 = 2.0 * PI * (THIGH + SHIN) * 0.45;
const CYCLES: f64 = 4.0;

const ARMOUR: u32 = 0x5a5874;
const PLATE: u32 = 0x7c7a96;
const JOINT: u32 = 0x1c1a28;
const RIM: u32 = 0xff9a5a;
const FAR: u32 = 0x302e46;
const VISOR: u32 = 0x5af0ff;

const TORSO: &[&str] = &[
    ".....kkkkkkkkkk.....",
    "....kSSSSSSSSSSkk...",
    "...kSSssssssssSSrk..",
    "..kbSsssssssssssSr..",
    ".kbbsssRRRRRssssssr.",
    "kbbbssssssssssssssr.",
    "kbbbsssvvvvvvsssssr.",
    "kbbbssssssssssssssr.",
    "kbbbsssvvvvvvssssr..",
    ".kbbssssssssssssssr.",
    "..kbssssssssssssssr.",
    "...kssssssssssssr...",
    "....kkkkkkkkkkkk....",
    ".....kpppppppk......",
    ".....kpppppppk......",
];
const HEAD_SIDE: &[&str] = &[
    "...k.....",
    "...k.....",
    "..kkkkkk.",
    ".kSSSSSSk",
    "kssssssrr",
    "ksssVVVVV",
    "kssssssrr",
    ".kkkkkkk.",
];
const HEAD_FRONT: &[&str] = &[
    "....k....",
    "....k....",
    "..kkkkk..",
    ".kSSSSSk.",
    "kssssssrk",
    "kVVsssVVk",
    "kssssssrk",
    ".kkkkkkk.",
];
const MECHP: &[(char, u32)] = &[
    ('k', JOINT),
    ('s', ARMOUR),
    ('S', PLATE),
    ('b', 0x3a3852),
    ('p', 0x3a3850),
    ('r', RIM),
    ('R', 0xd84a3a),
    ('v', 0xffb85a),
    ('V', VISOR),
];

/// A thick limb segment from `(x0, y0)` to `(x1, y1)`, lit on its right.
fn seg(g: &mut World, (x0, y0): (f64, f64), (x1, y1): (f64, f64), w: i32, c: u32, rim: bool) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as i32).max(1);
    let half = (w / 2) as f64;
    for pass in 0..1 + rim as i32 {
        for k in 0..=n {
            let u = k as f64 / n as f64;
            let (x, y) = (x0 + (x1 - x0) * u - half, y0 + (y1 - y0) * u - half);
            if rim && pass == 0 {
                g.p(RIM, x + 1.0, y, w, w);
            } else {
                g.p(c, x, y, w, w);
            }
        }
    }
}

/// How far the hips have travelled at `t`: steady, then easing to a stop.
fn travel(t: f64) -> f64 {
    let total = CYCLE * CYCLES;
    let (u, a) = ((t / T_STOP).min(1.0), 0.7);
    let m = 2.0 / (1.0 + a);
    let s = if u < a {
        m * u
    } else {
        m * a + m * (u - a) - m * (u - a).powi(2) / (2.0 * (1.0 - a))
    };
    total * s
}

struct Leg {
    knee: (f64, f64),
    foot: (f64, f64),
}

/// Both legs at walk phase `ph` from a hip at `(hx, 0)`: thigh swing, knee
/// bend while the leg swings forward.
fn legs(hx: f64, ph: f64, stance: f64) -> [Leg; 2] {
    [(0.0, stance), (PI, -stance)].map(|(off, spread)| {
        let p = ph + off;
        let a = 0.45 * p.sin() + spread;
        let bend = 0.9 * p.cos().max(0.0);
        let knee = (hx + THIGH * a.sin(), THIGH * a.cos());
        let s = a - bend;
        let foot = (knee.0 + SHIN * s.sin(), knee.1 + SHIN * s.cos());
        Leg { knee, foot }
    })
}

fn sky(g: &mut World) {
    let w = g.w();
    let stops = [0x1a1240, 0x46206a, 0x9a3a6a, 0xe0605a, 0xff9a50, 0xffd27a];
    for y in 0..53 {
        let u = (y as f64 / 46.0).min(1.0) * (stops.len() - 1) as f64;
        for x in 0..w {
            // Flat bands with a dithered seam into the next.
            let (k, fr) = (u.floor().min(5.0) as usize, u - u.floor());
            let k = if bayer(x, y) < ((fr - 0.55) / 0.45).clamp(0.0, 1.0) {
                (k + 1).min(5)
            } else {
                k
            };
            g.p(stops[k], x as f64, y as f64, 1, 1);
        }
    }
}

fn sun_and_clouds(g: &mut World, t: f64) {
    let (sx, sy) = ((g.panel(3) + 30) as f64, 41.0);
    glow(g, sx, sy, 22.0, 0xffe0a0, 0.45);
    g.disc(0xfff0c0, sx, sy, 9);
    // Bands across the sun.
    for (dy, c) in [(3.0, 0xffb860), (6.0, 0xff9a50)] {
        g.p(c, sx - 10.0, sy + dy, 21, 1);
    }
    // Long thin clouds, dark with lit bellies.
    let w = g.w() as f64;
    for (k, (y, len, sp)) in [
        (9.0, 54, 1.2),
        (16.0, 40, 2.0),
        (23.0, 64, 1.6),
        (29.0, 34, 2.6),
    ]
    .into_iter()
    .enumerate()
    {
        let kf = k as f64;
        let x0 = (rnd(kf * 3.0 + 1.0) * w + t * sp).rem_euclid(w + 80.0) - 60.0;
        let lf = len as f64;
        for i in 0..len {
            let u = i as f64 / lf;
            // A lumpy profile: thick in the middle, thinning to wisps.
            let th = (u * PI).sin() * (3.0 + (u * 9.0 + kf).sin() * 1.2);
            let hh = js_round(th).max(1);
            let x = x0 + i as f64;
            g.p(0x6a2a5e, x, y - hh as f64 + 1.0, 1, hh);
            g.p(0xb04a70, x, y - hh as f64 + 1.0, 1, 1);
            g.p(0xffa060, x, y + 1.0, 1, 1);
        }
    }
}

/// Searchlights sweeping the sky; at the end they all find the mech.
fn searchlights(g: &mut World, t: f64, head: (f64, f64)) {
    let n = g.layout.panels;
    let src = [(0u32, 50), (2, 6), (3.min(n - 1), 46)];
    for (k, (p, dx)) in src.into_iter().enumerate() {
        let (sx, sy) = ((g.panel(p) + dx) as f64, 50.0);
        let kf = k as f64;
        let sweep = 0.55 * (t * 0.7 + kf * 2.1).sin();
        let aim = (head.0 - sx).atan2(sy - head.1);
        let u = ((t - T_TURN) / 1.2).clamp(0.0, 1.0);
        let a = sweep + (aim - sweep) * u * u * (3.0 - 2.0 * u);
        let reach = if u > 0.0 { sy - head.1 + 4.0 } else { sy };
        for d in 0..js_round(reach.min(sy)) {
            let df = d as f64;
            let (cx, y) = (sx + df * a.tan(), sy - df);
            let half = 0.5 + df * 0.07;
            for x in js_round(cx - half)..=js_round(cx + half) {
                let core = (x as f64 - cx).abs() < 0.8;
                tint(g, x, js_round(y), 0xfff0c8, if core { 0.32 } else { 0.16 });
            }
        }
    }
}

/// A row of buildings: `(seed, least height, extra height)`, then the
/// wall and window colours and the share of windows lit.
fn skyline(
    g: &mut World,
    f: u32,
    (seed, tall, var): (f64, f64, f64),
    (c, win): (u32, u32),
    lit: f64,
) {
    let w = g.w();
    let (mut x, mut i) = (0, 0);
    while x < w {
        let fi = i as f64 + seed;
        let bw = 7 + (rnd(fi * 1.3) * 9.0) as i32;
        let bh = js_round(tall + rnd(fi * 2.7) * var);
        let top = STREET as i32 - bh;
        g.p(c, x as f64, top as f64, bw, bh);
        // Rooftop clutter: a water tower, an antenna with a red light, a
        // stepped crown.
        match (rnd(fi * 5.1) * 4.0) as i32 {
            0 => {
                g.p(c, (x + 2) as f64, (top - 4) as f64, 4, 3);
                g.p(c, (x + 2) as f64, (top - 1) as f64, 1, 1);
                g.p(c, (x + 5) as f64, (top - 1) as f64, 1, 1);
            }
            1 => {
                g.p(c, (x + bw / 2) as f64, (top - 7) as f64, 1, 7);
                if (f + i) % 16 < 8 {
                    g.p(0xff3a3a, (x + bw / 2) as f64, (top - 8) as f64, 1, 1);
                }
            }
            2 => g.p(c, (x + 2) as f64, (top - 2) as f64, bw - 4, 2),
            _ => {}
        }
        let mut wy = top + 2;
        while wy < STREET as i32 - 2 {
            let mut wx = x + 1;
            while wx < x + bw - 1 {
                let r = rnd((wx * 7 + wy * 13) as f64 + seed);
                if r < lit {
                    let flick = r < 0.02 && (f / 5 + wx as u32).is_multiple_of(3);
                    let col = if flick {
                        c
                    } else if r < lit * 0.3 {
                        mix(win, 0xffffff, 0.3)
                    } else {
                        win
                    };
                    g.p(col, wx as f64, wy as f64, 1, 1);
                }
                wx += 2;
            }
            wy += 3;
        }
        x += bw + (rnd(fi * 3.3) * 2.0) as i32;
        i += 1;
    }
}

fn street(g: &mut World, t: f64) {
    let w = g.w();
    g.p(0x2a2034, 0.0, STREET, w, 2);
    g.p(0x3a2c44, 0.0, STREET, w, 1);
    g.p(0x161020, 0.0, STREET + 2.0, w, 5);
    let mut x = 0;
    while x < w {
        g.p(0x6a5060, x as f64, STREET + 4.0, 3, 1);
        x += 8;
    }
    // Streetlamps.
    let mut x = 10;
    while x < w {
        let xf = x as f64;
        g.p(0x1a1424, xf, STREET - 8.0, 1, 8);
        g.p(0x1a1424, xf, STREET - 8.0, 3, 1);
        g.p(0xffe0a0, xf + 2.0, STREET - 7.0, 1, 1);
        glow(g, xf + 2.0, STREET - 6.0, 4.0, 0xffd080, 0.3);
        x += 37;
    }
    // Cars both ways, headlights and tail lights.
    let wf = w as f64;
    for (k, (sp, y)) in [
        (22.0, STREET + 2.0),
        (-30.0, STREET + 5.0),
        (16.0, STREET + 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        let x = (rnd(k as f64 + 70.0) * wf + t * sp).rem_euclid(wf + 20.0) - 10.0;
        let (front, back) = if sp > 0.0 { (x + 6.0, x) } else { (x, x + 6.0) };
        g.p(0x2a2238, x, y - 1.0, 7, 2);
        g.p(0x3a3050, x + 2.0, y - 2.0, 3, 1);
        g.p(0xfff0b0, front, y - 1.0, 1, 1);
        g.p(0xff3a3a, back, y - 1.0, 1, 1);
        let dir = sp.signum();
        for d in 1..6 {
            tint(
                g,
                js_round(front + dir * d as f64),
                js_round(y - 1.0),
                0xfff0b0,
                0.4 - d as f64 * 0.06,
            );
        }
    }
}

/// Pigeons on a roof of the second screen, flushed by the first step there.
fn pigeons(g: &mut World, f: u32, t: f64, hip: f64) {
    let (rx, ry) = ((g.panel(1) + 36) as f64, 36.0);
    let reach = rx - 30.0;
    let t0 = if hip >= reach {
        // When the hips passed `reach`; travel is monotonic so search frames.
        (0..=(t * 12.0) as u32)
            .map(|k| k as f64 / 12.0)
            .find(|&tt| (g.px0(0) as f64 + 2.0 + travel(tt)) >= reach)
    } else {
        None
    };
    for k in 0..6 {
        let kf = k as f64;
        let (mut x, mut y) = (rx + kf * 4.0, ry - (k % 2) as f64);
        let mut flap = false;
        if let Some(t0) = t0 {
            let e = t - t0;
            x -= e * (14.0 + kf * 3.0);
            y -= e * (9.0 + rnd(kf) * 6.0) - (e * 6.0 + kf).sin();
            flap = (f + k) % 4 < 2;
        }
        if flap {
            g.p(0x1a1020, x - 1.0, y - 1.0, 1, 1);
            g.p(0x1a1020, x + 1.0, y - 1.0, 1, 1);
            g.p(0x1a1020, x, y, 1, 1);
        } else {
            g.p(0x1a1020, x - 1.0, y, 3, 1);
            g.p(0x1a1020, x, y - 1.0, 1, 1);
        }
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let hip_x = g.px0(0) as f64 + 2.0 + travel(t);
    let ph = travel(t) / CYCLE * 2.0 * PI;
    // Standing still, the feet settle apart.
    let stance = 0.25 * ((t - T_STOP + 0.8) / 0.8).clamp(0.0, 1.0);
    let lg = legs(hip_x, ph, stance);
    let drop = lg[0].foot.1.max(lg[1].foot.1);
    let hip_y = STREET - drop;
    let head = (hip_x + 2.0, hip_y - 25.0);

    sky(g);
    sun_and_clouds(g, t);
    searchlights(g, t, head);
    skyline(g, f, (0.0, 12.0, 14.0), (0x4a2a5e, 0x9a6a7a), 0.25);
    skyline(g, f, (40.0, 5.0, 11.0), (0x1e1430, 0xffc870), 0.35);
    street(g, t);
    pigeons(g, f, t, hip_x);

    // The mech, back to front: far arm and leg, body, near leg and arm.
    let _off = |p: (f64, f64)| (p.0, p.1 + hip_y);
    let sh = (hip_x + 3.0, hip_y - 15.0);
    let swing = 0.4 * ph.sin();
    let far_hand = (sh.0 + (swing + 0.1).sin() * 16.0 - 6.0, sh.1 + 16.0);
    seg(g, (sh.0 - 6.0, sh.1), far_hand, 4, FAR, false);
    g.p(FAR, far_hand.0 - 2.0, far_hand.1 - 1.0, 4, 4);
    let [near, far] = &lg;
    let fh = |p: (f64, f64)| (p.0 - 3.0, p.1 + hip_y);
    seg(g, (hip_x - 3.0, hip_y), fh(far.knee), 5, FAR, false);
    seg(g, fh(far.knee), fh(far.foot), 5, FAR, false);
    g.p(FAR, far.foot.0 - 6.0, far.foot.1 + hip_y - 1.0, 9, 3);
    g.spr(TORSO, MECHP, hip_x - 10.0, hip_y - 19.0, false);
    let turned = t >= T_TURN;
    let head_spr = if turned { HEAD_FRONT } else { HEAD_SIDE };
    g.spr(head_spr, MECHP, head.0 - 4.0, head.1, false);
    // The visor glows; brighter once it looks at you.
    let eye_a = if turned {
        0.5 + 0.2 * (t * 6.0).sin()
    } else {
        0.25
    };
    glow(
        g,
        head.0 + if turned { 0.5 } else { 3.0 },
        head.1 + 5.0,
        6.0,
        VISOR,
        eye_a,
    );
    let nh = |p: (f64, f64)| (p.0 + 2.0, p.1 + hip_y);
    seg(g, (hip_x + 2.0, hip_y), nh(near.knee), 5, ARMOUR, true);
    seg(g, nh(near.knee), nh(near.foot), 5, ARMOUR, true);
    g.p(PLATE, near.knee.0, near.knee.1 + hip_y - 3.0, 5, 4);
    g.p(RIM, near.knee.0 + 5.0, near.knee.1 + hip_y - 3.0, 1, 4);
    g.p(PLATE, hip_x - 1.0, hip_y - 1.0, 6, 5);
    g.p(JOINT, near.foot.0 - 2.0, near.foot.1 + hip_y - 2.0, 9, 3);
    g.p(PLATE, near.foot.0 - 1.0, near.foot.1 + hip_y - 2.0, 7, 1);
    g.p(RIM, near.foot.0 + 7.0, near.foot.1 + hip_y - 2.0, 1, 3);
    // The near arm: swinging while walking, then up to wave.
    let up = ((t - T_WAVE) / 0.5).clamp(0.0, 1.0);
    let raise = up * up * (3.0 - 2.0 * up);
    let a_upper = -swing * (1.0 - raise) + raise * 2.2;
    let elbow = (sh.0 + a_upper.sin() * 9.0, sh.1 + a_upper.cos() * 9.0);
    let wave = if up >= 1.0 {
        0.5 * (t * 7.0).sin()
    } else {
        0.0
    };
    let a_fore = a_upper * (1.0 - raise) - 0.3 * (1.0 - raise) + raise * (3.0 + wave);
    let hand = (elbow.0 + a_fore.sin() * 9.0, elbow.1 + a_fore.cos() * 9.0);
    seg(g, sh, elbow, 5, ARMOUR, true);
    seg(g, elbow, hand, 4, ARMOUR, true);
    g.p(PLATE, sh.0 - 4.0, sh.1 - 4.0, 9, 6);
    g.p(0xd84a3a, sh.0 - 3.0, sh.1 - 2.0, 7, 1);
    g.p(RIM, sh.0 + 5.0, sh.1 - 4.0, 1, 6);
    g.p(JOINT, hand.0 - 2.0, hand.1 - 2.0, 4, 4);
    g.p(RIM, hand.0 + 2.0, hand.1 - 2.0, 1, 4);

    // Footfalls: dust puffs and a jolt as each foot lands.
    let mut jolt = 0;
    if t < T_STOP {
        for (k, leg) in lg.iter().enumerate() {
            let p = (ph + k as f64 * PI).rem_euclid(2.0 * PI);
            // The foot lands as the swing ends, at phase π/2.
            let since = p - PI / 2.0;
            if (0.0..1.4).contains(&since) {
                let e = since / 1.4;
                for s in 0..6 {
                    let sf = s as f64;
                    let dir = if s % 2 == 0 { 1.0 } else { -1.0 };
                    let x = leg.foot.0 + dir * (3.0 + e * (6.0 + sf * 2.0));
                    let y = STREET - 1.0 - e * (2.0 + sf * 0.6);
                    tint(g, js_round(x), js_round(y), 0xc8a0a0, 0.6 * (1.0 - e));
                }
                if since < 0.35 {
                    jolt = 1;
                }
            }
        }
    }
    shake(g, jolt);
}

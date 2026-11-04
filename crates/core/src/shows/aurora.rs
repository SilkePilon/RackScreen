//! Northern lights: a snowy night by a log cabin with a smoking chimney and
//! warm windows. Green and violet curtains ripple and brighten over the
//! pines, mirrored in a frozen lake; snow falls, a fox trots across the
//! ice, sits down to watch, and a shooting star crosses the sky.

use super::world::{ease, js_round, rnd, World};

pub const SECS: f64 = 10.0;

/// The fox, facing right: two trotting strides, then sitting (looking up).
const FOX: [&[&str]; 3] = [
    &[
        "...........oo",
        "..........ooow",
        "ww.oooooooooo.",
        "wooooooooooo..",
        "..oooooowoo...",
        "...k.k..k.k...",
    ],
    &[
        "...........oo",
        "..........ooow",
        "ww.oooooooooo.",
        "wooooooooooo..",
        "..oooooowoo...",
        "..k...kk...k..",
    ],
    &[
        ".........o.o..",
        ".........ooo..",
        ".........ooook",
        "........ooww..",
        "........ooww..",
        ".......oooww..",
        "......ooooow..",
        "wwoooooooook..",
    ],
];
const FOXP: &[(char, u32)] = &[('o', 0xe0782a), ('w', 0xf4f0ea), ('k', 0x1a1418)];

const PINE: &[&str] = &[
    "...s...", "..sks..", "..kkk..", ".skkks.", ".kkkkk.", "sskkkss", "kkkkkkk", "...t...",
];

/// 4×4 ordered dither threshold in `0..1`.
fn bayer(x: i32, y: i32) -> f64 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[(y & 3) as usize][(x & 3) as usize] as f64 + 0.5) / 16.0
}

fn rgb(c: [u8; 4]) -> u32 {
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
}

/// Mix two colours, `k` of the way from `a` to `b`.
fn mix(a: u32, b: u32, k: f64) -> u32 {
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 255) as f64, ((b >> s) & 255) as f64);
        ((x + (y - x) * k).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, wf) = (g.w(), g.w() as f64);
    let fi = f as i32;
    g.bands(&[
        (0x070a1e, 10),
        (0x0c1430, 10),
        (0x12204a, 10),
        (0x1a2c5a, 14),
        (0x1a2c5a, 16),
    ]);
    for i in 0..g.spread(60) {
        let fv = i as f64;
        let (x, y) = ((rnd(fv) * wf).floor(), (rnd(fv + 9.0) * 36.0).floor());
        let tw = (f + i * 7) % 23 < 2;
        g.p(if tw { 0xffffff } else { 0x8a96c8 }, x, y, 1, 1);
    }
    // A thin crescent moon on panel 0.
    let mx = (g.panel(0) + 20) as f64;
    g.disc(0xf0f0dc, mx, 9.0, 4);
    g.disc(0x0a0e26, mx + 2.0, 8.0, 4);

    // The aurora: two rippling curtains whose glow grows to a peak.
    let glow = 0.45 + 0.55 * ease(t / 6.5) - 0.2 * ease((t - 8.5) / 1.5);
    for (base, height, ph, sp, back) in
        [(17.0, 12.0, 2.1, -0.7, true), (27.0, 21.0, 0.0, 1.0, false)]
    {
        for x in 0..w {
            let xf = x as f64;
            let fold = (xf * 0.045 + t * 0.6 * sp + ph).sin() * 4.0
                + (xf * 0.11 - t * 1.1 * sp).sin() * 1.5;
            let b = base + fold;
            let h = height * (0.7 + 0.3 * (xf * 0.07 + t * 0.8 + ph).sin());
            let rays = 0.7 + 0.3 * (xf * 1.3 + t * 2.5 * sp + (xf * 0.13).sin() * 3.0).sin();
            let wave = 0.7 + 0.3 * (xf * 0.05 - t * 1.4 * sp + ph).sin();
            let top = js_round(b - h);
            for y in top.max(0)..js_round(b).min(42) {
                let v = (b - y as f64) / h; // 0 at the hem, 1 at the top
                let prof = if v < 0.08 {
                    0.7 + v * 3.75
                } else {
                    (1.0 - v).powf(0.6)
                };
                let k = prof * rays * wave * glow * if back { 0.75 } else { 1.0 };
                if k <= 0.1 + bayer(x, y) * 0.18 {
                    continue;
                }
                let c = if v > 0.7 || back && v > 0.35 {
                    if k > 0.35 {
                        0x9a5ad0
                    } else {
                        0x4a2a7a
                    }
                } else if k > 0.72 {
                    0xd0ffe0
                } else if k > 0.5 {
                    0x5af09a
                } else if k > 0.3 {
                    0x2ab88a
                } else {
                    0x1a5a6a
                };
                g.p(c, xf, y as f64, 1, 1);
            }
        }
    }
    // Snowy peaks in front of the lights: straight-sloped summits.
    let span = 26.0;
    let peak = |x: f64| -> (f64, f64) {
        let i = (x / span).floor();
        let mut best = (0.0, 0.0);
        for k in [i - 1.0, i, i + 1.0] {
            let c = k * span + rnd(k + 300.0) * 10.0;
            let h = 6.0 + rnd(k + 301.0) * 9.0 - (x - c).abs() * 0.75;
            if h > best.0 {
                best = (h, x - c);
            }
        }
        best
    };
    for x in 0..w {
        let (h, d) = peak(x as f64);
        let top = js_round(43.0 - h);
        g.p(0x222e58, x as f64, top as f64, 1, 44 - top);
        if d < 0.0 {
            g.p(0x2e3c6e, x as f64, (top + 1) as f64, 1, 44 - top - 1);
        }
        if top < 37 {
            let depth = 1 + (37 - top) / 2;
            g.p(
                if d < 0.0 { 0xe0e8ff } else { 0xa8b8e0 },
                x as f64,
                top as f64,
                1,
                depth,
            );
            g.dith(0x8a9ac8, x, top + depth, 1, 2, x);
        }
    }
    // The lake on the right two screens, snow on the left two.
    let lake = (g.panel(2) - 14) as f64;
    let hz = 42;
    // Far shore pines along the horizon.
    for k in 0..g.spread(40) {
        let x = rnd(k as f64 + 30.0) * wf;
        let ht = 3 + (rnd(k as f64 + 31.0) * 4.0) as i32;
        for r in 0..ht {
            g.p(0x0e1630, x - (r / 2) as f64, (hz - ht + r) as f64, 1 + r, 1);
        }
    }
    g.p(0x0e1630, 0.0, hz as f64, w, 1);
    // Snow ground with soft drifts and blue shadows.
    for x in 0..js_round(lake) + 12 {
        let xf = x as f64;
        let top = js_round(44.0 + (xf * 0.08).sin() * 1.5 + (xf * 0.21).sin() * 0.6);
        g.p(0xd8e4f8, xf, top as f64, 1, 60 - top);
        g.p(0xf4f8ff, xf, top as f64, 1, 1);
        if (x * 7 + top) % 9 < 2 {
            g.p(0xaab8dc, xf, (top + 4) as f64, 2, 1);
        }
    }
    // The frozen lake mirrors the sky and the lights, rippled and dimmed.
    let l0 = js_round(lake).max(0);
    for y in hz + 2..60 {
        let src = 2 * hz - y;
        let ripple = js_round(((y as f64) * 0.9 + t * 2.0).sin() * 0.6);
        for x in l0..w {
            let sx = (x + ripple).clamp(0, w - 1);
            let c = rgb(g.frame.at(sx as u32, src.max(0) as u32));
            let deep = (y - hz) as f64 / 18.0;
            g.p(
                mix(c, 0x0c1638, 0.45 + deep * 0.3),
                x as f64,
                y as f64,
                1,
                1,
            );
        }
    }
    for x in l0..w {
        let xf = x as f64;
        // Shore ice edge and a few cracks and glints.
        g.p(0x8aa0d0, xf, (hz + 1) as f64, 1, 1);
        if (x * 13) % 47 == 0 {
            g.p(0x9ab0e0, xf, (hz + 5 + x % 9) as f64, 4, 1);
        }
        if (x * 5 + fi) % 61 == 0 {
            g.p(0xffffff, xf, (hz + 3 + x % 12) as f64, 1, 1);
        }
    }
    // Lakeside shore curve where snow meets ice.
    for y in hz + 1..60 {
        let edge = lake + ((y - hz) as f64 * 0.7) + (y as f64 * 0.5).sin();
        g.p(0xd8e4f8, edge - 2.0, y as f64, 3, 1);
        g.p(0xf4f8ff, edge, y as f64, 1, 1);
    }

    // Snowy pines around the cabin.
    let cab = (g.panel(1) + 22) as f64;
    for k in 0..g.spread(14) {
        let kf = k as f64;
        let x = rnd(kf + 50.0) * (lake - 8.0);
        if (x - cab - 8.0).abs() < 14.0 {
            continue;
        }
        let y = 37.0 + rnd(kf + 51.0) * 4.0;
        g.spr(
            PINE,
            &[('k', 0x14283a), ('s', 0xe8f0ff), ('t', 0x3a2a24)],
            x - 3.0,
            y,
            k % 2 == 0,
        );
    }
    // The log cabin: snowy roof, chimney smoke, warm windows and a glow.
    let (cx, cy) = (cab, 36.0);
    for k in 0..5 {
        let kf = k as f64;
        let age = ((t * 0.6 + kf * 0.2) % 1.0) * 1.0;
        let r = js_round(age * 2.5);
        g.disc(
            if age < 0.5 { 0x8a90a8 } else { 0x5a6080 },
            cx + 13.0 + age * 8.0 + (age * 6.0 + t).sin() * 1.5,
            cy - 6.0 - age * 18.0,
            r,
        );
    }
    g.p(0x4a3a3a, cx + 12.0, cy - 6.0, 3, 5);
    g.p(0xf4f8ff, cx + 12.0, cy - 7.0, 3, 1);
    for r in 0..8 {
        let rf = r as f64;
        g.p(
            0xe8f0ff,
            cx + 8.0 - rf * 1.3,
            cy - 7.0 + rf,
            js_round(rf * 2.6) + 2,
            1,
        );
        if r > 0 {
            g.p(
                0xaab8dc,
                cx + 9.0 - rf * 1.3 + rf * 2.6 - 1.0,
                cy - 7.0 + rf,
                2,
                1,
            );
        }
    }
    g.p(0x5a3a26, cx - 1.0, cy + 1.0, 21, 9);
    for r in 0..4 {
        g.p(0x3a2418, cx - 1.0, cy + 2.0 + r as f64 * 2.0, 21, 1);
    }
    for (wx, lit) in [(cx + 2.0, true), (cx + 13.0, t < 9.3)] {
        let c = if lit { 0xffc860 } else { 0x2a2030 };
        g.p(c, wx, cy + 3.0, 4, 4);
        g.p(0x3a2418, wx + 2.0, cy + 3.0, 1, 4);
        g.p(0x3a2418, wx, cy + 5.0, 4, 1);
        if lit {
            for r in 0..4 {
                g.dith(
                    0xe8d0a0,
                    js_round(wx) - r,
                    js_round(cy) + 10 + r,
                    4 + r * 2,
                    1,
                    r,
                );
            }
        }
    }
    g.p(0x2a1a14, cx + 8.0, cy + 4.0, 3, 6);
    let flick = if (f / 2).is_multiple_of(3) {
        0xffe080
    } else {
        0xffb040
    };
    g.p(flick, cx + 11.0, cy + 3.0, 1, 2);
    // A woodpile and a fence half buried in snow.
    g.p(0x6a4a2a, cx - 6.0, cy + 6.0, 4, 4);
    g.p(0xf4f8ff, cx - 6.0, cy + 5.0, 4, 1);
    for k in 0..6 {
        let px = cx - 22.0 + k as f64 * 4.0;
        g.p(0x4a3424, px, cy + 9.0, 1, 4);
        g.p(0xf4f8ff, px, cy + 8.0, 1, 1);
    }
    g.p(0x4a3424, cx - 22.0, cy + 10.0, 21, 1);

    // The fox trots in, over the ice, then sits and looks up.
    let sit = 6.8;
    let (x0, x1) = (-16.0, (g.panel(3) + 18) as f64);
    let fox_x = x0 + ((t - 0.3).max(0.0) / (sit - 0.3)).min(1.0) * (x1 - x0);
    let sitting = t >= sit;
    let fr = if sitting { 2 } else { ((f / 2) % 2) as usize };
    let fy = if sitting { 47.0 } else { 49.0 };
    // Its reflection in the ice, then the fox; pawprints behind it.
    if fox_x > lake {
        let pal: Vec<(char, u32)> = FOXP
            .iter()
            .map(|&(k, c)| (k, mix(c, 0x0c1638, 0.6)))
            .collect();
        let rows: Vec<&str> = FOX[fr].iter().rev().copied().collect();
        g.spr(&rows, &pal, fox_x, fy + FOX[fr].len() as f64, false);
    }
    for k in 1..14 {
        let px = fox_x + 4.0 - k as f64 * 5.0;
        if px < 0.0 || px > fox_x {
            break;
        }
        let c = if px > lake { 0x5a6a9a } else { 0x9aaad0 };
        g.p(c, px, if k % 2 == 0 { 55.0 } else { 56.0 }, 1, 1);
    }
    g.spr(FOX[fr], FOXP, fox_x, fy, false);
    if sitting {
        // The tail swishes.
        let sw = ((t - sit) * 3.0).sin() > 0.0;
        g.p(0xf4f0ea, fox_x, fy + if sw { 6.0 } else { 7.0 }, 2, 1);
    }

    // A shooting star while the fox watches.
    if (7.6..8.4).contains(&t) {
        let u = (t - 7.6) / 0.8;
        let (a, b) = ((g.panel(2) + 14) as f64, (g.panel(3) + 44) as f64);
        let (sx, sy) = (a + u * (b - a), 8.0 + u * 9.0);
        for k in 0..14 {
            let c = if k < 3 {
                0xffffff
            } else if k < 8 {
                0xc8e8ff
            } else {
                0x6a8ab8
            };
            g.p(c, sx - k as f64, sy - k as f64 * 0.12, 1, 1);
        }
    }
    // Snow falls in two layers.
    for (n, sp, c, sway) in [(70, 5.0, 0x8a9ac0, 0.6), (45, 11.0, 0xffffff, 1.4)] {
        for i in 0..g.spread(n) {
            let fv = i as f64 + sp;
            let x = rnd(fv) * wf + (t * 1.3 + fv).sin() * sway * 2.0 - t * sp * 0.3;
            let y = (rnd(fv + 3.0) * 60.0 + t * sp) % 60.0;
            g.p(c, (x % wf + wf) % wf, y, 1, 1);
        }
    }
}

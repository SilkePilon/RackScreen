//! Dragon over the castle: at sunset a red dragon flaps in over a village
//! with lit windows and smoking chimneys, hovers before a castle on a cliff
//! and breathes fire at the knight on the wall. He raises his shield; when
//! the smoke clears he is sooty but proud, holding out a perfectly toasted
//! marshmallow on his sword. The dragon flies off over the keep.

use super::world::{ease, js_round, rnd, World};

pub const SECS: f64 = 10.0;

const BODY: &[&str] = &[
    "......................k.k.",
    ".....................kbkbk",
    "....................kbbebbk",
    "...................kbbbbbbbk",
    "..................kbbbkkkkk.",
    "...........kkkkkkbbbbk......",
    "kk.......kkbbbbbbbbbk.......",
    "kbk...kkbbbbbbbbllbk........",
    ".kbbbbbbbbbbbllllk..........",
    "..kkkkkkbbbbkkkk............",
    ".......kbk.kbk..............",
    ".......kk..kk...............",
];
/// The jaw dropped for the fire breath.
const JAW: &[&str] = &[
    "...................kbbbbbbbk",
    "..................kbbbrr....",
    "..................kbbbbkk...",
];
/// Wings: up, level, down; anchored over the shoulders.
const WING: [&[&str]; 3] = [
    &[
        "k.........",
        "kk........",
        "kmk.......",
        "kmmk......",
        ".kmmk.....",
        ".kmmmk....",
        "..kmmmkkk.",
        "...kmmmmmk",
        "....kkkkk.",
    ],
    &[
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "kkkkk.....",
        "kmmmmkkkk.",
        ".kkmmmmmmk",
        "...kkkkkk.",
    ],
    &[
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "...kkkkkk.",
        "..kmmmmmmk",
        ".kmmmmkkk.",
        "kmmmk.....",
        "kmmk......",
        "kmk.......",
        "kk........",
    ],
];
const DRAGONP: &[(char, u32)] = &[
    ('k', 0x3a1020),
    ('b', 0xc0403a),
    ('l', 0xf0b060),
    ('e', 0xfff070),
    ('m', 0x8a2a4a),
    ('r', 0xffd060),
];

/// The knight, facing left: plume, visor, armour.
const KNIGHT: &[&str] = &[
    "...rr..", "..rr...", "..hhh..", ".hvhhh.", ".hhhhh.", "..aaa..", ".aaaaa.", ".aaaaa.",
    "..bbb..", "..a.a..", "..a.a..", ".kk.kk.",
];
const KNIGHTP: &[(char, u32)] = &[
    ('r', 0xd83a3a),
    ('h', 0xc8ccd8),
    ('v', 0x1a1a24),
    ('a', 0x9aa0b4),
    ('b', 0x6a4a2a),
    ('k', 0x2a2a34),
];
const SHIELD: &[&str] = &[".ssss.", "sgyygs", "sgyygs", "sggggs", ".sggs.", "..ss.."];
const SHIELDP: &[(char, u32)] = &[('s', 0x4a4a5a), ('g', 0x2a5aa0), ('y', 0xf0d050)];

const HOUSE: [&[&str]; 2] = [
    &[
        "..c.....", "..crr...", ".rrrrrr.", "rrrrrrrr", ".wwwwww.", ".wywwyw.", ".wwwddw.",
        ".wwwddw.",
    ],
    &[
        ".....c...",
        "...rrc...",
        "..rrrrr..",
        ".rrrrrrr.",
        "rrrrrrrrr",
        ".wwwwwww.",
        ".wywwwyw.",
        ".wwwwwww.",
        ".wywddww.",
        ".wwwddww.",
    ],
];

/// A puffy sunset cloud: dusky top, pink middle, a gold-lit underside.
fn cloud(g: &mut World, x: f64, y: f64, len: i32) {
    let lf = len as f64;
    g.disc(0x8a4a7a, x + lf * 0.35, y, 3);
    g.disc(0x8a4a7a, x + lf * 0.6, y - 1.0, 4);
    g.p(0x8a4a7a, x, y + 1.0, len, 3);
    g.disc(0xb45a7e, x + lf * 0.35, y + 1.0, 2);
    g.disc(0xb45a7e, x + lf * 0.6, y + 1.0, 3);
    g.p(0xb45a7e, x + 1.0, y + 2.0, len - 2, 2);
    g.p(0xf0a070, x + 2.0, y + 4.0, len - 4, 1);
    g.dith(0xf8c88a, js_round(x) + 3, js_round(y) + 4, len - 6, 1, 0);
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, wf) = (g.w(), g.w() as f64);
    let fi = f as i32;
    g.bands(&[
        (0x241a48, 7),
        (0x3e2462, 7),
        (0x6a2e6e, 6),
        (0xa8466e, 6),
        (0xd86a5e, 6),
        (0xf09a5a, 5),
        (0xf8c472, 4),
        (0x4a3050, 19),
    ]);
    // The sun sinking on panel 1, and long sunset clouds drifting.
    let sun = (g.panel(1) + 22) as f64;
    g.disc(0xffd890, sun, 37.0 + t * 0.3, 6);
    g.disc(0xfff0c0, sun, 37.0 + t * 0.3, 4);
    for k in 0..g.spread(5) {
        let kf = k as f64;
        let span = wf + 40.0;
        let x = ((rnd(kf + 3.0) * span + t * (1.5 + kf % 3.0)) % span) - 30.0;
        let y = 8.0 + rnd(kf + 4.0) * 12.0;
        cloud(g, x, y, 14 + (rnd(kf + 5.0) * 10.0) as i32);
    }
    // Distant hills, then the valley floor with the village.
    for x in 0..w {
        let xf = x as f64;
        let h = 4.0 + 3.0 * (xf * 0.05 + 0.6).sin() + 2.0 * (xf * 0.13).sin();
        g.p(0x5a3a62, xf, 41.0 - h, 1, js_round(h) + 2);
    }
    for x in 0..w {
        let xf = x as f64;
        let h = 2.0 + 1.5 * (xf * 0.09 + 2.0).sin();
        g.p(0x3a2a48, xf, 46.0 - h, 1, js_round(h) + 14);
    }
    // A river catching the sunset.
    for x in 0..w {
        let y = 53 + js_round((x as f64 * 0.04).sin() * 1.5);
        g.p(0x7a4a7a, x as f64, y as f64, 1, 3);
        if (x * 7 + fi) % 13 < 2 {
            g.p(0xf8c472, x as f64, y as f64, 1, 1);
        }
    }
    // Village: houses with lit windows, smoke from the chimneys, trees.
    let cliff = (g.panel(2) + 30) as f64;
    let mut x = 2.0;
    let mut k = 0;
    while x < cliff - 30.0 {
        let kf = k as f64;
        let v = (rnd(kf + 11.0) * 2.0) as usize;
        let hy = if v == 0 { 41.0 } else { 39.0 };
        let roof = [0x8a3a3a, 0x5a4a6a, 0x9a5a3a][k % 3];
        let lit = if rnd(kf + 12.0 + (fi / 18) as f64) > 0.15 {
            0xffd27a
        } else {
            0x6a5040
        };
        let pal = [
            ('r', roof),
            ('w', 0x6a5a6a),
            ('y', lit),
            ('d', 0x2a1e2a),
            ('c', 0x3a2a3a),
        ];
        g.spr(HOUSE[v], &pal, x, hy, k % 2 == 1);
        // Chimney smoke curling up.
        let cx = x + if v == 0 { 2.0 } else { 5.0 } + if k % 2 == 1 { 1.0 } else { 0.0 };
        for s in 0..4 {
            let age = ((t * 3.0 + s as f64 * 0.9 + kf) % 3.6) / 3.6;
            g.p(
                if age < 0.5 { 0x8a7a8a } else { 0x6a5a72 },
                cx + age * 6.0 + (age * 8.0 + kf).sin(),
                hy - 1.0 - age * 10.0,
                1 + js_round(age),
                1,
            );
        }
        // A round tree between some houses.
        if rnd(kf + 13.0) > 0.4 {
            let tx = x + 11.0;
            g.p(0x2a1a22, tx, 43.0, 1, 4);
            g.disc(0x2e3a3a, tx, 41.0, 2);
            g.p(0x4a5a4a, tx - 1.0, 40.0, 1, 1);
        }
        x += 12.0 + rnd(kf + 14.0) * 8.0;
        k += 1;
    }
    // A windmill on panel 0, its sails turning.
    let (mx, my) = ((g.panel(0) + 46) as f64, 31.0);
    for r in 0..12 {
        let half = 1 + r / 4;
        g.p(
            0x6a5a6a,
            mx - half as f64,
            my + 2.0 + r as f64,
            half * 2 + 1,
            1,
        );
    }
    g.p(0x4a3040, mx - 2.0, my, 5, 2);
    g.p(0xffd27a, mx, my + 7.0, 1, 2);
    for arm in 0..4 {
        let a = t * 1.6 + arm as f64 * std::f64::consts::FRAC_PI_2;
        for r in 1..9 {
            let rf = r as f64;
            let c = if r > 3 { 0xd8c8b0 } else { 0x4a3040 };
            g.p(c, mx + a.cos() * rf, my + a.sin() * rf, 1, 1);
            if r > 3 {
                let b = a + 0.25;
                g.p(0xa89888, mx + b.cos() * rf, my + b.sin() * rf, 1, 1);
            }
        }
    }
    // Villagers run for the castle as the dragon passes over.
    for k in 0..g.spread(5) {
        let kf = k as f64;
        let base = rnd(kf + 70.0) * (cliff - 40.0) + 4.0;
        let start = 0.4 + base / (cliff + 30.0) * 3.4;
        let run = (t - start).max(0.0);
        let vx = base + run * (14.0 + kf % 3.0 * 3.0);
        if vx > cliff - 10.0 {
            continue;
        }
        let step = run > 0.0 && (f / 2 + k).is_multiple_of(2);
        let shirt = [0x4a6aa0, 0xa04a4a, 0x6a8a4a][k as usize % 3];
        g.p(0xf0c8a0, vx, 46.0, 1, 1);
        g.p(shirt, vx, 47.0, 1, 2);
        g.p(0x2a1e2a, vx - if step { 1.0 } else { 0.0 }, 49.0, 1, 1);
        g.p(0x2a1e2a, vx + if step { 1.0 } else { 0.0 }, 49.0, 1, 1);
        if run > 0.0 {
            // Arms up in panic.
            g.p(0xf0c8a0, vx - 1.0, 45.0 + (f % 2) as f64, 1, 1);
            g.p(0xf0c8a0, vx + 1.0, 45.0 + ((f + 1) % 2) as f64, 1, 1);
        }
    }
    // The cliff, then the castle: a wall with the knight on panel 2, a
    // tall tower behind him and the keep on panel 3.
    for x in js_round(cliff) - 8..w {
        let d = (x as f64 - cliff).max(0.0);
        let top = 46.0 - (d * 0.9).min(10.0) + ((x as f64 * 0.7).sin() * 0.8);
        let top = js_round(top);
        g.p(0x3a2a3e, x as f64, top as f64, 1, 60 - top);
        if (x * 3 + top) % 5 == 0 {
            g.p(0x5a4458, x as f64, (top + 2) as f64, 1, 1);
        }
        if x % 6 == 0 {
            g.p(0x2a1e2e, x as f64, (top + 5) as f64, 1, 3);
        }
    }
    let wall_x = (g.panel(2) + 33) as f64;
    let stone = (0x8a8098, 0x6a627a, 0x4a4458);
    // A turret behind the knight, its banner waving.
    let tw = wall_x + 9.0;
    g.p(stone.1, tw, 18.0, 12, 22);
    g.p(stone.0, tw, 18.0, 2, 22);
    g.p(stone.2, tw + 10.0, 18.0, 2, 22);
    for r in 0..6 {
        let off = if r % 2 == 0 { 3.0 } else { 6.0 };
        g.p(stone.2, tw + off, 20.0 + r as f64 * 3.0, 1, 2);
    }
    let mut cx = tw - 1.0;
    while cx < tw + 13.0 {
        g.p(stone.1, cx, 16.0, 2, 2);
        g.p(stone.0, cx, 16.0, 2, 1);
        cx += 4.0;
    }
    g.p(0x2a2a34, tw + 6.0, 7.0, 1, 9);
    for k in 0..6 {
        let wave = js_round(((t * 6.0) - k as f64 * 0.8).sin() * 0.8);
        g.p(
            0xe0c040,
            tw + 7.0 + k as f64,
            (7 + wave) as f64,
            1,
            3 - k / 3,
        );
    }
    let win = if t > 5.0 && (f / 2).is_multiple_of(3) {
        0xff9a4a
    } else {
        0xffd27a
    };
    g.p(win, tw + 4.0, 23.0, 2, 3);
    g.p(0x2a1e2a, tw + 4.0, 22.0, 2, 1);
    // Wall with battlements; the knight stands on its walkway.
    let wy = 36.0;
    g.p(
        stone.1,
        wall_x - 4.0,
        wy,
        (w as f64 - wall_x + 4.0) as i32,
        12,
    );
    g.p(
        stone.0,
        wall_x - 4.0,
        wy,
        (w as f64 - wall_x + 4.0) as i32,
        1,
    );
    let mut bx = wall_x - 4.0;
    while bx < wf {
        g.p(stone.1, bx, wy - 3.0, 3, 3);
        g.p(stone.0, bx, wy - 3.0, 3, 1);
        bx += 5.0;
    }
    for r in 0..4 {
        let off = if r % 2 == 0 { 0 } else { 3 };
        let mut sx = wall_x - 4.0 + off as f64;
        while sx < wf {
            g.p(stone.2, sx, wy + 2.0 + r as f64 * 3.0, 1, 2);
            sx += 6.0;
        }
    }
    // The keep on the last panel: two towers with slate roofs and
    // banners, lit windows, a gate with torches.
    let keep = (g.panel(3) + 14) as f64;
    for (dx, top) in [(0.0, 14.0), (22.0, 18.0)] {
        let kx = keep + dx;
        let ht = 36 - top as i32;
        g.p(stone.1, kx, top, 14, ht);
        g.p(stone.0, kx, top, 2, ht);
        g.p(stone.2, kx + 12.0, top, 2, ht);
        for r in 0..ht / 3 {
            let off = if r % 2 == 0 { 4.0 } else { 8.0 };
            g.p(stone.2, kx + off, top + 1.0 + r as f64 * 3.0, 1, 2);
        }
        for r in 0..9 {
            let half = r as f64 * 0.85;
            g.p(
                0x3a3a6a,
                kx + 7.0 - half,
                top - 9.0 + r as f64,
                js_round(half * 2.0) + 1,
                1,
            );
            g.p(0x5a5a8a, kx + 7.0 - half, top - 9.0 + r as f64, 1, 1);
        }
        g.p(0x2a2a34, kx + 7.0, top - 15.0, 1, 6);
        for k in 0..5 {
            let wave = js_round(((t * 5.0 + dx) - k as f64 * 0.8).sin() * 0.8);
            g.p(
                0xd83a3a,
                kx + 8.0 + k as f64,
                top - 15.0 + wave as f64,
                1,
                3 - k / 3,
            );
        }
        g.p(0x2a1e2a, kx + 6.0, top + 4.0, 2, 1);
        g.p(0xffd27a, kx + 6.0, top + 5.0, 2, 3);
        g.p(0xffd27a, kx + 3.0, top + 12.0, 1, 2);
    }
    // Gate with a portcullis and two flickering torches.
    g.p(0x2a1e2a, keep + 14.0, 24.0, 8, 12);
    g.p(0x2a1e2a, keep + 15.0, 23.0, 6, 1);
    for k in 0..3 {
        g.p(0x6a6a74, keep + 15.0 + k as f64 * 2.5, 24.0, 1, 12);
    }
    g.p(0x6a6a74, keep + 14.0, 28.0, 8, 1);
    g.p(0x6a6a74, keep + 14.0, 32.0, 8, 1);
    for tx in [keep + 12.0, keep + 23.0] {
        g.p(0x4a3428, tx, 27.0, 1, 3);
        let c = if (f + tx as u32).is_multiple_of(3) {
            0xffe080
        } else {
            0xff8a3a
        };
        g.p(c, tx, 25.0 + (f % 2) as f64, 1, 2);
    }
    // The dragon: flaps in, hovers before the wall, breathes, leaves.
    let hover = (g.panel(2) - 6) as f64;
    let (fly_in, breath0, breath1, leave) = (4.0, 4.6, 6.6, 7.6);
    let (mut dx, mut dy) = if t < fly_in {
        let u = ease(t / fly_in);
        (
            -30.0 + u * (hover + 30.0),
            14.0 + (1.0 - u) * 6.0 + (t * 3.0).sin() * 3.0,
        )
    } else if t < leave {
        (hover, 16.0 + ((t - fly_in) * 4.0).sin() * 1.5)
    } else {
        let u = (t - leave) / 1.9;
        (
            hover + u * u * (g.panel(3) as f64 + 40.0 - hover),
            16.0 - u * 6.0,
        )
    };
    let breathing = (breath0..breath1).contains(&t);
    if breathing {
        dx -= 1.0;
        dy = dy.round();
    }
    let wing = if breathing {
        1
    } else {
        [0, 1, 2, 1][((f / 2) % 4) as usize]
    };
    g.spr(BODY, DRAGONP, dx, dy, false);
    if breathing {
        g.spr(JAW, DRAGONP, dx, dy + 3.0, false);
    }
    g.spr(WING[wing], DRAGONP, dx + 9.0, dy - 2.0, false);
    // The knight: steps up, raises his shield with his sword held out.
    let kx = wall_x + 1.0;
    let ky = wy - 12.0;
    let raised = t > breath0 - 0.5 && t < breath1 + 0.6;
    let soot = t > breath1;
    let mut kp: Vec<(char, u32)> = KNIGHTP.to_vec();
    if soot {
        kp[1] = ('h', 0x4a4a54);
        kp[3] = ('a', 0x5a5a6a);
    }
    g.spr(KNIGHT, &kp, kx, ky, false);
    if (3.6..4.2).contains(&t) {
        g.p(0xffffff, kx + 3.0, ky - 6.0, 1, 3);
        g.p(0xffffff, kx + 3.0, ky - 2.0, 1, 1);
    }
    // Sword held out towards the dragon, a marshmallow on its tip.
    let (sx, sy) = (kx - 9.0, ky + 6.0);
    g.p(0xd8dce8, sx, sy, 9, 1);
    g.p(0x6a4a2a, kx - 1.0, sy - 1.0, 1, 3);
    let toast = ((t - breath0) / (breath1 - breath0)).clamp(0.0, 1.0);
    let mallow = if toast < 0.3 {
        0xfff8f0
    } else if toast < 0.7 {
        0xf0d08a
    } else {
        0xd08a3a
    };
    let eaten = t > 9.0;
    if !eaten {
        g.p(mallow, sx - 2.0, sy - 1.0, 3, 3);
        if toast >= 0.7 {
            g.p(0x8a4a1a, sx - 2.0, sy + 1.0, 3, 1);
        }
    }
    // A last little flame on the marshmallow, blown out; then a sparkle.
    if (breath1..breath1 + 0.7).contains(&t) {
        let c = if (f / 2).is_multiple_of(2) {
            0xffd060
        } else {
            0xff7a2a
        };
        g.p(c, sx - 1.0, sy - 3.0, 1, 2);
    }
    if (breath1 + 0.9..9.0).contains(&t) && (f / 3).is_multiple_of(2) {
        g.p(0xffffff, sx - 4.0, sy - 3.0, 1, 1);
        g.p(0xffffff, sx - 5.0, sy - 2.0, 3, 1);
        g.p(0xffffff, sx - 4.0, sy - 1.0, 1, 1);
    }
    if eaten {
        // Munching: a happy heart over the helmet.
        let hy = ky - 6.0 - (t - 9.0) * 3.0;
        g.p(0xff5a7a, kx + 1.0, hy, 2, 1);
        g.p(0xff5a7a, kx + 4.0, hy, 2, 1);
        g.p(0xff5a7a, kx + 1.0, hy + 1.0, 5, 1);
        g.p(0xff5a7a, kx + 2.0, hy + 2.0, 3, 1);
        g.p(0xff5a7a, kx + 3.0, hy + 3.0, 1, 1);
    }
    if raised {
        g.spr(SHIELD, SHIELDP, kx - 3.0, ky + 1.0, false);
    }
    // Fire: a flickering cone from the jaw that splashes on the shield.
    if breathing {
        let (mx, my) = (dx + 26.0, dy + 4.0);
        let tx = kx - 3.0;
        g.p(0xfff0a0, mx - 1.0, my, 2, 2);
        for k in 0..64 {
            let kf = k as f64;
            let u = (t * 3.0 + rnd(kf)) % 1.0;
            let spread = u * 5.0 * (rnd(kf + 1.0) - 0.5) * 2.0;
            let fx = mx + u * (tx - mx);
            let fy = my + u * 2.0 + spread + ((t * 20.0 + kf).sin() * 0.6);
            let c = if u < 0.25 {
                0xfffbe0
            } else if u < 0.55 {
                0xffd060
            } else if u < 0.8 {
                0xff8a2a
            } else {
                0xd8402a
            };
            g.p(c, fx, fy, 1 + (u * 2.0) as i32, 1 + (u * 1.6) as i32);
        }
        // Splash and embers off the shield.
        for k in 0..10 {
            let kf = k as f64;
            let e = (t * 2.5 + rnd(kf + 30.0)) % 1.0;
            let ang = -1.6 + (rnd(kf + 31.0) - 0.5) * 2.4;
            let r = e * 9.0;
            g.p(
                if e < 0.5 { 0xffd060 } else { 0xd8402a },
                tx + ang.cos() * r * 0.5 - 1.0,
                my + 2.0 + ang.sin() * r,
                1,
                1,
            );
        }
    }
    // Smoke rolls up from the wall after the blast, then thins out.
    if t > breath0 + 1.0 && t < breath1 + 1.4 {
        let fade = ((breath1 + 1.4 - t) / 1.0).min(1.0);
        for k in 0..7 {
            let kf = k as f64;
            let age = ((t - breath0) * 0.8 + kf * 0.14) % 1.0;
            let r = js_round(age * 2.5 * fade);
            let x = kx - 2.0 + rnd(kf + 50.0) * 8.0 + age * 5.0;
            let y = ky - 2.0 - age * 16.0;
            g.disc(if age < 0.5 { 0x6a6070 } else { 0x8a8090 }, x, y, r);
        }
    }
    // Smoke rings as the dragon leaves.
    if t > leave + 0.4 {
        let e = t - leave - 0.4;
        let (rx, ry) = (hover + 28.0 + e * 4.0, 14.0 - e * 3.0);
        g.ring(0x9a90a0, rx, ry, 1.5 + e * 1.5);
    }
}

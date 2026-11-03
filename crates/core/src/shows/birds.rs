//! Birds at sunset: a V of birds flaps across a pixel sunset over the sea,
//! glints on the water flickering. Port of the round-1 mockup `birds`.

use super::world::{js_round, rnd, World};

pub const SECS: f64 = 9.0;

const BIRD: [&[&str]; 3] = [
    &["k...k", ".k.k.", "..k.."],
    &["kk.kk", "..k.."],
    &["..k..", "kk.kk"],
];
const BIRDP: &[(char, u32)] = &[('k', 0x1c1426)];

/// The V: leader first, then pairs trailing behind it.
const V: [(f64, f64); 7] = [
    (0.0, 0.0),
    (-5.0, -3.0),
    (-5.0, 3.0),
    (-10.0, -6.0),
    (-10.0, 6.0),
    (-15.0, -9.0),
    (-15.0, 9.0),
];

pub fn draw(g: &mut World, f: u32, t: f64) {
    let wf = g.w() as f64;
    g.bands(&[
        (0xf2a65a, 14),
        (0xe2685a, 12),
        (0xa8466a, 8),
        (0x3a2a5e, 4),
        (0x1d2b55, 22),
    ]);
    // The sun sits on panel 2 + 4 (mockup x 140), its lower half sunk below
    // the sea line by a 40 px rect centred on it (mockup x 120).
    let sun = (g.panel(2) + 4) as f64;
    g.disc(0xffe08a, sun, 37.0, 10);
    g.p(0x1d2b55, sun - 20.0, 38.0, 40, 8);
    for k in 0..g.spread(30) {
        let kf = k as f64;
        let x = (rnd(kf) * wf).floor();
        let y = 40.0 + (rnd(kf + 9.0) * 18.0).floor();
        if ((f >> 1) + k) % 7 < 3 {
            let c = if (x - sun).abs() < 26.0 {
                0xffd27a
            } else {
                0x5a6aa0
            };
            g.p(c, x, y, 2 + (rnd(kf + 4.0) * 3.0).floor() as i32, 1);
        }
    }
    let bx = -30.0 + (t / SECS) * (wf + 60.0);
    for (n, &(ox, oy)) in V.iter().enumerate() {
        let ph = ((f >> 1) as usize + n) % 3;
        let y = 20.0 + oy + js_round((t * 2.0 + n as f64).sin()) as f64;
        g.spr(BIRD[ph], BIRDP, bx + ox, y, false);
    }
}

//! Paper plane: a paper plane glides through drifting clouds and loops the
//! loop on the third screen, leaving a dotted trail.
//! Port of the round-1 mockup `plane`.

use super::world::{js_round, World};

pub const SECS: f64 = 8.0;

const PLANE: [&[&str]; 2] = [
    &["wwww.....", "swwwwww..", "..sswwwww", "...sss..."],
    &["..wwww...", "wwwwwww..", "sswwwwwww", "....sss.."],
];
const PLP: &[(char, u32)] = &[('w', 0xffffff), ('s', 0x9ec3dd)];

const CLOUD: &[&str] = &[
    ".....cccc.......",
    "...cccccccc.....",
    "..cccccccccccc..",
    ".cccccccccccccc.",
    "cddddddddddddddc",
];
const CLOUDP: &[(char, u32)] = &[('c', 0xf4f8ff), ('d', 0xcfe3f5)];

const LOOP_A: f64 = 0.55;
const LOOP_B: f64 = 0.75;

/// The plane's position at progress `p`: glide in from the left to the loop
/// centre `c`, loop, then glide out past the right edge `w`. The mockup's
/// 170 and 120 are `c + 12` and `w + 14 - c` with `c = 158`, `w = 264`.
fn path(p: f64, c: f64, w: f64) -> (f64, f64) {
    if p < LOOP_A {
        let x = -12.0 + p / LOOP_A * (c + 12.0);
        (x, (28 + js_round((p * 14.0).sin() * 3.0)) as f64)
    } else if p < LOOP_B {
        let a = (p - LOOP_A) / (LOOP_B - LOOP_A) * std::f64::consts::PI * 2.0;
        (c + a.sin() * 12.0, 28.0 - (1.0 - a.cos()) * 12.0)
    } else {
        let x = c + (p - LOOP_B) / (1.0 - LOOP_B) * (w + 14.0 - c);
        (x, (28 + js_round((p * 12.0).sin() * 2.0)) as f64)
    }
}

pub fn draw(g: &mut World, f: u32, t: f64) {
    let wf = g.w() as f64;
    g.bands(&[(0x5aaee6, 20), (0x7cc6f0, 24), (0xa6dcf5, 16)]);
    for k in 0..g.spread(7) {
        let kf = k as f64;
        let speed = if k % 2 == 1 { 4.0 } else { 7.0 };
        let cx = ((kf * 53.0 + t * speed) % (wf + 30.0)) - 20.0;
        let cy = (8 + (k * 11) % 36) as f64;
        g.spr(CLOUD, CLOUDP, cx, cy, false);
    }
    // The loop is centred on panel 2 + 22 (mockup x 158).
    let c = (g.panel(2) + 22) as f64;
    let p = t / SECS;
    let (x, y) = path(p, c, wf);
    for s in (2..14).step_by(2) {
        let q = (p - s as f64 * 0.006).max(0.0);
        let (tx, ty) = path(q, c, wf);
        g.p(0xe8f4fc, tx, ty + 2.0, 1, 1);
    }
    g.spr(PLANE[((f >> 2) % 2) as usize], PLP, x, y, false);
}

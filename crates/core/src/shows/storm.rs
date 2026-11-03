//! Thunderstorm: clouds roll and rain streaks fall; then the whole sky
//! flashes (held two frames), a bolt cracks on the third screen, and a
//! second flicker follows. Port of the round-1 mockup `storm`.

use super::world::{js_round, rnd, World};

pub const SECS: f64 = 8.0;

pub fn draw(g: &mut World, f: u32, t: f64) {
    let (w, h) = (g.w(), g.h());
    let (wf, hf) = (w as f64, h as f64);
    let fl = t > 3.9 && t < 4.05;
    let fl2 = t > 4.3 && t < 4.38;
    let sky = if fl {
        0xcdd6ff
    } else if fl2 {
        0x4a5378
    } else {
        0x1a1d2b
    };
    g.p(sky, 0.0, 0.0, w, h);
    for k in 0..g.spread(16) {
        let kf = k as f64;
        let cx = ((kf * 29.0 + t * 5.0) % (wf + 40.0)) - 20.0;
        let cy = (4 + (k * 7) % 10) as f64;
        g.disc(
            if fl { 0x9aa6d8 } else { 0x2b3043 },
            cx,
            cy,
            7 + (k % 3) as i32,
        );
        g.disc(if fl { 0xb8c2ec } else { 0x3a4058 }, cx + 3.0, cy - 2.0, 4);
    }
    for d in 0..g.spread(90) {
        let df = d as f64;
        let x = (rnd(df) * wf + t * 40.0) % wf;
        let y = (rnd(df + 7.0) * hf + t * 110.0) % hf;
        g.p(if fl { 0xffffff } else { 0x6f7fa8 }, x, y, 1, 2);
        g.p(0x4b5679, x - 1.0, y - 1.0, 1, 1);
    }
    g.p(if fl { 0x3a3f58 } else { 0x14161f }, 0.0, 54.0, w, 6);
    for s in 0..g.spread(12) {
        let sx = (rnd((s + f / 3) as f64) * wf).floor();
        g.p(0x5d6a92, sx, 53.0, 1, 1);
        g.p(0x5d6a92, sx - 1.0, 54.0, 1, 1);
        g.p(0x5d6a92, sx + 1.0, 54.0, 1, 1);
    }
    // The bolt from panel 2 + 12 (mockup x 148), skipping the 4.15-4.3 gap.
    if t > 3.9 && t < 4.6 && !(t > 4.15 && t < 4.3) {
        let (mut x, mut y) = (g.panel(2) + 12, 12);
        for s in 0..14 {
            let nx = x + js_round((rnd((s * 3 + 1) as f64) - 0.5) * 7.0);
            let ny = y + 3;
            // The mockup sets white once, then each glow `P` leaves its
            // colour as the canvas fill: only the first segment is white.
            let c = if s == 0 { 0xffffff } else { 0xa9b8ff };
            for i in 0..=3 {
                let bx = x as f64 + (nx - x) as f64 * i as f64 / 3.0;
                g.p(c, bx, (y + i) as f64, 1, 1);
            }
            g.p(0xa9b8ff, (x + 1) as f64, y as f64, 1, 1);
            x = nx;
            y = ny;
        }
    }
}

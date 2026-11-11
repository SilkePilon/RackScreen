//! How many bytes a SEN panel pushes per frame, exact diff vs the video
//! diff, through the real decoder, crop, diff and round clip:
//! `cargo run --release -p rackscreen-render --example pushsim -- seg.ts...`

use rackscreen_core::event::SenEvent;
use rackscreen_render::frame::{
    copy_rect, dirty_rects, new_pixmap, round_rects, video_rects, Rect,
};
use rackscreen_sources::sen::player::{Player, Segment};

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let mut frames = Vec::new();
    let mut player = Player::new(false).expect("player");
    for (i, f) in files.iter().enumerate() {
        let data = std::fs::read(f).expect("read");
        let seg = Segment {
            sq: i as u64 + 1,
            data,
        };
        player.play(&seg, &mut |_| true, &mut |e| {
            if let SenEvent::Frame(v) = e {
                let mut px = new_pixmap();
                px.data_mut().copy_from_slice(v.0.rgba());
                frames.push(px);
            }
        });
    }
    let full: u64 = round_rects(&[Rect::full()], 240)
        .iter()
        .map(Rect::area)
        .sum();
    let (mut exact, mut video, mut wins) = (0u64, 0u64, 0usize);
    let mut shown = frames[0].clone();
    for w in frames.windows(2) {
        let e = round_rects(&dirty_rects(&w[0], &w[1]), 240);
        exact += e.iter().map(Rect::area).sum::<u64>();
        let v = round_rects(&video_rects(&shown, &w[1]), 240);
        video += v.iter().map(Rect::area).sum::<u64>();
        wins += v.len();
        for r in &v {
            copy_rect(&w[1], &mut shown, *r);
        }
    }
    let n = (frames.len() - 1).max(1) as u64;
    println!(
        "{} frames: exact {:.1} KB ({:.0} %), video {:.1} KB ({:.0} %), {:.1} windows",
        frames.len(),
        exact as f64 * 2.0 / n as f64 / 1024.0,
        100.0 * exact as f64 / (full * n) as f64,
        video as f64 * 2.0 / n as f64 / 1024.0,
        100.0 * video as f64 / (full * n) as f64,
        wins as f64 / n as f64
    );
}

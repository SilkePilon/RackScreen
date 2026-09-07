//! Renders the README media: one GIF per screen role, one of a screen cycling
//! through all ten roles, and one of the download monitor.
//!
//!     cargo run -p rackscreen-app --example gifs -- .github/media/screens
//!
//! Frames come from the model, the fake source and the renderer the simulator
//! uses, so a GIF shows exactly what a panel shows. Everything outside the
//! 120 px panel radius is written transparent, and every frame after the first
//! is stored as the rectangle that changed, which keeps the files small.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use gif::{DisposalMethod, Encoder, Frame, Repeat};
use rackscreen_core::anim::Secs;
use rackscreen_core::event::Event;
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::screens::TRANSITION_SECS;
use rackscreen_core::theme::layout::{CX, CY, SIZE};
use rackscreen_core::theme::Role;
use rackscreen_render::frame::{dirty_rect, new_pixmap, Rect};
use rackscreen_render::renderer::Renderer;
use rackscreen_sources::fake::{FakeCmd, FakeState};
use tiny_skia::Pixmap;

const OUT_DIR: &str = ".github/media/screens";
const FPS: Secs = 20.0;
/// Frame delay in hundredths of a second: 5 is 20 fps.
const DELAY: u16 = 5;
/// Radius of the round panel; the corners around it are transparent.
const MASK_R: f32 = 120.0;
/// Seed for the fake source, picked so the drifting values stay in a good band.
const SEED: u64 = 7;
/// Simulated seconds the model runs before the first kept frame: rings have
/// eased in, prices are loaded and the solar curve is halfway down its arc.
/// The half second is what the badges that alternate every five seconds need to
/// have faded in, so the first frame of a GIF is never a half-drawn one.
const WARMUP: Secs = 100.5;
/// The shortest dwell the model accepts; anything less is clamped to it.
const CYCLE: Secs = 3.0;
/// Local hour the price ring marks as "now".
const HOUR: u32 = 14;
/// Ten roles, each dwelling `CYCLE` and then irising for half a second: one
/// whole round of the cycling screen, so its GIF loops seamlessly.
const ALL_SECS: Secs = 10.0 * (CYCLE + TRANSITION_SECS);
/// Three whole rounds, so the cycling screen starts its recording on CPU with a
/// fresh dwell and warmed-up data.
const ALL_WARMUP: Secs = 3.0 * ALL_SECS;

/// One scripted moment: a simulator command, or an event built by hand.
enum Step {
    Cmd(FakeCmd),
    Event(Event),
}

struct Clip {
    /// File stem, so `cpu` becomes `cpu.gif`.
    name: &'static str,
    roles: Vec<Role>,
    cycle_secs: Secs,
    /// Seconds recorded, after the warm-up.
    secs: Secs,
    warmup: Secs,
    /// Seconds into the clip, and what happens then.
    script: Vec<(Secs, Step)>,
}

impl Clip {
    fn role(role: Role, script: Vec<(Secs, Step)>) -> Clip {
        Clip {
            name: role.name(),
            roles: vec![role],
            cycle_secs: CYCLE,
            secs: 8.0,
            warmup: WARMUP,
            script,
        }
    }
}

/// A CPU metrics sample hot enough to raise the flame splash. The node name is
/// not one the fake source reports, so its own hot-node debounce cannot swallow it.
fn hot_cpu() -> Event {
    Event::Metrics {
        cpu_pct: 93.0,
        mem_pct: 71.0,
        mem_used_gb: 11.4,
        mem_total_gb: 16.0,
        hot_cpu: Some(("hp-elitedesk-800-g5-i7".into(), 96.0)),
        hot_mem: None,
    }
}

fn clips() -> Vec<Clip> {
    let mut out: Vec<Clip> = Role::ALL
        .into_iter()
        .map(|role| {
            let script = match role {
                Role::Cpu => vec![(3.0, Step::Event(hot_cpu()))],
                Role::Pods => vec![(2.0, Step::Cmd(FakeCmd::PodCrashed))],
                Role::Thermal => vec![(3.0, Step::Cmd(FakeCmd::HotTemp))],
                Role::Storage => vec![
                    (2.0, Step::Cmd(FakeCmd::DegradeVolume)),
                    (5.0, Step::Cmd(FakeCmd::HealVolume)),
                ],
                _ => Vec::new(),
            };
            Clip::role(role, script)
        })
        .collect();
    out.push(Clip {
        name: "health-torrent",
        secs: 10.0,
        script: vec![
            (0.5, Step::Cmd(FakeCmd::ToggleTorrent)),
            (5.0, Step::Cmd(FakeCmd::TorrentDone)),
        ],
        ..Clip::role(Role::Health, Vec::new())
    });
    out.push(Clip {
        name: "all",
        roles: Role::ALL.to_vec(),
        cycle_secs: CYCLE,
        secs: ALL_SECS,
        warmup: ALL_WARMUP,
        script: Vec::new(),
    });
    out
}

fn main() -> Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| OUT_DIR.into()));
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let mut renderer = Renderer::new()?;
    for clip in clips() {
        let path = dir.join(format!("{}.gif", clip.name));
        let frames = record(&mut renderer, &clip, &path)?;
        let kib = fs::metadata(&path)?.len() as f64 / 1024.0;
        println!(
            "{:<16} {frames:>4} frames  {:>4.1} s  {kib:>7.1} KiB",
            path.display(),
            clip.secs
        );
    }
    Ok(())
}

/// Play one clip and write it out. Returns the number of frames written.
fn record(renderer: &mut Renderer, clip: &Clip, path: &Path) -> Result<u64> {
    let mut model = Model::new(Thresholds::default());
    model.set_screens(vec![clip.roles.clone()], vec![clip.cycle_secs]);
    // One screen: nothing to serialise, and no gap between two irises.
    model.set_one_at_a_time(false);
    // An Electricity Maps token is configured, so the grid roles show data.
    model.set_token_present(true);
    model.set_local_hour(HOUR);

    let mut fake = FakeState::new(SEED);
    for ev in fake.initial() {
        model.apply(ev, 0.0);
    }

    let warmup_frames = (clip.warmup * FPS).round() as u64;
    let total = warmup_frames + (clip.secs * FPS).round() as u64;
    let steps: Vec<(u64, &Step)> = clip
        .script
        .iter()
        .map(|(at, step)| (((clip.warmup + at) * FPS).round() as u64, step))
        .collect();

    let mut out = GifOut::new(path)?;
    let mut px = new_pixmap();
    let mut seconds = 0u64;
    for f in 0..total {
        let now = f as Secs / FPS;
        while (seconds as Secs) < now.floor() {
            seconds += 1;
            for ev in fake.tick() {
                model.apply(ev, now);
            }
        }
        for (_, step) in steps.iter().filter(|(at, _)| *at == f) {
            match step {
                Step::Cmd(cmd) => {
                    for ev in fake.command(*cmd) {
                        model.apply(ev, now);
                    }
                }
                Step::Event(ev) => model.apply(ev.clone(), now),
            }
        }
        model.tick(now);
        if f < warmup_frames {
            continue;
        }
        renderer.render(&model.scene(0, now), &mut px);
        round_mask(&mut px);
        out.push(&px)?;
    }
    out.finish()?;
    Ok(total - warmup_frames)
}

/// Clear the corners outside the round panel, so the GIF is a circle.
fn round_mask(px: &mut Pixmap) {
    let data = px.data_mut();
    for y in 0..SIZE {
        let dy = y as f32 + 0.5 - CY;
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - CX;
            if dx * dx + dy * dy > MASK_R * MASK_R {
                let i = ((y * SIZE + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
}

/// GIF writer that stores each frame as the rectangle that changed, kept over
/// the frames before it. Transparent pixels are the ones a frame does not
/// repaint, which is also what makes the corners stay clear.
struct GifOut {
    encoder: Encoder<BufWriter<File>>,
    prev: Pixmap,
    first: bool,
    buf: Vec<u8>,
}

impl GifOut {
    fn new(path: &Path) -> Result<GifOut> {
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let size = SIZE as u16;
        let mut encoder = Encoder::new(BufWriter::new(file), size, size, &[])?;
        encoder.set_repeat(Repeat::Infinite)?;
        Ok(GifOut {
            encoder,
            prev: new_pixmap(),
            first: true,
            buf: Vec::new(),
        })
    }

    fn push(&mut self, px: &Pixmap) -> Result<()> {
        // A frame identical to the last one still has to take up its delay, so
        // it repaints one corner pixel, which is transparent either way.
        let one_pixel = Rect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        };
        let rect = if self.first {
            Rect::full()
        } else {
            dirty_rect(&self.prev, px).unwrap_or(one_pixel)
        };
        self.buf.clear();
        let data = px.data();
        for y in rect.y..rect.y + rect.h {
            let start = ((y * SIZE + rect.x) * 4) as usize;
            self.buf
                .extend_from_slice(&data[start..start + (rect.w * 4) as usize]);
        }
        let mut frame = Frame::from_rgba_speed(rect.w as u16, rect.h as u16, &mut self.buf, 10);
        frame.left = rect.x as u16;
        frame.top = rect.y as u16;
        frame.delay = DELAY;
        frame.dispose = DisposalMethod::Keep;
        self.encoder.write_frame(&frame)?;
        self.prev.data_mut().copy_from_slice(px.data());
        self.first = false;
        Ok(())
    }

    /// Write the trailer and flush, rather than leaving it to the drop glue,
    /// where a failed write would be lost.
    fn finish(self) -> Result<()> {
        self.encoder.into_inner()?.flush()?;
        Ok(())
    }
}

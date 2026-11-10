//! Renders the README media: one GIF per screen role, one of a screen cycling
//! through every role, one of the download monitor, and one wide GIF per show.
//!
//!     cargo run -p rackscreen-app --example gifs -- .github/media/screens .github/media/shows
//!
//! A third argument makes only the screen GIF of that name (`sen`, `all`).
//!
//! A show GIF is the four panels side by side, 240 px circles 32 px apart on the
//! default layout (25 mm gaps), played once from start to end at 12 fps.
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
use rackscreen_core::p2::ui::ScreenUi;
use rackscreen_core::screens::TRANSITION_SECS;
use rackscreen_core::theme::layout::{CX, CY, SIZE};
use rackscreen_core::theme::Role;
use rackscreen_render::frame::{dirty_rect, new_pixmap, Rect};
use rackscreen_render::renderer::Renderer;
use rackscreen_sources::fake::{FakeCmd, FakeState};
use tiny_skia::Pixmap;

const OUT_DIR: &str = ".github/media/screens";
const SHOWS_DIR: &str = ".github/media/shows";
/// Shows run at 12 fps.
const SHOW_FPS: f64 = rackscreen_core::shows::FPS;
/// Transparent space between two circles of a show strip.
const BEZEL: u32 = 32;
const FPS: Secs = 20.0;
/// Frame delay in hundredths of a second: 5 is 20 fps.
const DELAY: u16 = 5;
/// Radius of the round panel; the corners around it are transparent.
const MASK_R: f32 = 120.0;
/// Seed for the fake source, picked so the drifting values stay in a good band.
const SEED: u64 = 7;
/// Simulated seconds the model runs before the first kept frame: rings have
/// eased in and the solar curve is halfway down its arc.
/// The half second is what the badges that alternate every five seconds need to
/// have faded in, so the first frame of a GIF is never a half-drawn one.
const WARMUP: Secs = 100.5;
/// The shortest dwell the model accepts; anything less is clamped to it.
const CYCLE: Secs = 3.0;
/// Every role, each dwelling `CYCLE` and then crossfading for `TRANSITION_SECS`
/// (0.4 s): roughly one whole round of the cycling screen. Only a guide: at 20 fps
/// a dwell or a fade can take one frame more than its nominal length, so the clip
/// itself is cut at the transitions the model reports (see `Clip::whole_round`).
const ALL_SECS: Secs = Role::ALL.len() as Secs * (CYCLE + TRANSITION_SECS);
/// Fixed clock: 2026-09-07 12:00 UTC in Amsterdam, so the sun dial, rain ring
/// and ISS countdown render the same every time.
const UNIX_START: i64 = 1_788_782_400;
const UTC_OFFSET: i32 = 7200;
/// About three rounds of warm-up data before the cycling screen's recording
/// starts, on the next fresh dwell of its first role.
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
    /// Seconds recorded, after the warm-up. With `whole_round` only a guide.
    secs: Secs,
    warmup: Secs,
    /// Record exactly one round of the cycling screen instead of `secs`: start
    /// on the first frame after a transition lands on the first role, and stop
    /// just before the next time it does, so the GIF loops without a jump and
    /// every transition in it is whole.
    whole_round: bool,
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
            whole_round: false,
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
                Role::GhActivity => vec![
                    (2.0, Step::Cmd(FakeCmd::GithubPush)),
                    (5.0, Step::Cmd(FakeCmd::CiFailure)),
                ],
                Role::Weather => vec![(3.0, Step::Cmd(FakeCmd::Thunder))],
                Role::Rain => vec![(3.0, Step::Cmd(FakeCmd::RainSoon))],
                Role::Iss => vec![(4.0, Step::Cmd(FakeCmd::IssPassNow))],
                Role::Ups => vec![
                    (3.0, Step::Cmd(FakeCmd::UpsToggle)),
                    (7.0, Step::Cmd(FakeCmd::UpsToggle)),
                ],
                Role::Deploys => vec![
                    (3.0, Step::Cmd(FakeCmd::AppToggle)),
                    (6.0, Step::Cmd(FakeCmd::AppToggle)),
                ],
                Role::Media => vec![(3.0, Step::Cmd(FakeCmd::MediaDone))],
                Role::Restarts => vec![(2.5, Step::Cmd(FakeCmd::KickRestart))],
                Role::Mains => vec![
                    (2.5, Step::Cmd(FakeCmd::Brownout)),
                    (6.0, Step::Cmd(FakeCmd::Brownout)),
                ],
                Role::Ci => vec![
                    (2.0, Step::Cmd(FakeCmd::CiFailure)),
                    (5.0, Step::Cmd(FakeCmd::CiPass)),
                ],
                Role::Jellyfin => vec![
                    (3.0, Step::Cmd(FakeCmd::JellyfinToggle)),
                    (6.0, Step::Cmd(FakeCmd::JellyfinToggle)),
                ],
                Role::Music => vec![
                    (2.5, Step::Cmd(FakeCmd::MusicToggle)),
                    (4.5, Step::Cmd(FakeCmd::MusicToggle)),
                    (6.5, Step::Cmd(FakeCmd::MusicToggle)),
                ],
                _ => Vec::new(),
            };
            Clip {
                // video does not shrink in a GIF: one pass of the segment
                secs: if role == Role::Sen { 5.0 } else { 8.0 },
                ..Clip::role(role, script)
            }
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
        whole_round: true,
        script: Vec::new(),
    });
    out
}

fn main() -> Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| OUT_DIR.into()));
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let shows_dir = PathBuf::from(std::env::args().nth(2).unwrap_or_else(|| SHOWS_DIR.into()));
    fs::create_dir_all(&shows_dir).with_context(|| format!("create {}", shows_dir.display()))?;
    let only = std::env::args().nth(3);
    let mut renderer = Renderer::new()?;
    for clip in clips() {
        if only.as_deref().is_some_and(|name| name != clip.name) {
            continue;
        }
        let path = dir.join(format!("{}.gif", clip.name));
        let frames = record(&mut renderer, &clip, &path)?;
        report(&path, frames, frames as Secs / FPS)?;
    }
    if only.is_some() {
        return Ok(());
    }
    for def in rackscreen_core::shows::SHOWS {
        let path = shows_dir.join(format!("{}.gif", def.id));
        let frames = record_show(&mut renderer, def, &path)?;
        report(&path, frames, frames as f64 / SHOW_FPS)?;
    }
    Ok(())
}

fn report(path: &Path, frames: u64, secs: f64) -> Result<()> {
    let kib = fs::metadata(path)?.len() as f64 / 1024.0;
    println!(
        "{:<16} {frames:>4} frames  {secs:>4.1} s  {kib:>7.1} KiB",
        path.display()
    );
    Ok(())
}

/// One show from start to end as the four-circle strip. Returns the number
/// of frames written.
fn record_show(
    renderer: &mut Renderer,
    def: &rackscreen_core::shows::ShowDef,
    path: &Path,
) -> Result<u64> {
    use rackscreen_core::scene::{Drawable, Scene, WorldRef};
    use rackscreen_core::shows::world::{gap_px, Layout};
    let layout = Layout {
        panels: 4,
        gap: gap_px(25.0),
    };
    let width = layout.panels * SIZE + (layout.panels - 1) * BEZEL;
    let frames = (def.secs * SHOW_FPS).round() as u64;
    let mut out = GifOut::new(path, width, SIZE)?;
    let mut panel = new_pixmap();
    for f in 0..frames {
        let t = f as f64 / SHOW_FPS;
        let world = WorldRef(std::sync::Arc::new(rackscreen_core::shows::render(
            def, layout, t,
        )));
        let mut strip = Pixmap::new(width, SIZE).context("strip pixmap")?;
        for i in 0..layout.panels {
            let mut scene = Scene::new();
            scene.push(Drawable::World {
                frame: world.clone(),
                x0: layout.x0(i),
            });
            renderer.render(&scene, &mut panel);
            round_mask(&mut panel);
            let ox = i * (SIZE + BEZEL);
            let row = (SIZE * 4) as usize;
            for y in 0..SIZE as usize {
                let dst = (y * width as usize + ox as usize) * 4;
                strip.data_mut()[dst..dst + row]
                    .copy_from_slice(&panel.data()[y * row..(y + 1) * row]);
            }
        }
        // 100/12 cs is not whole: alternate 8 and 9 (8, 8, 9, ...) so every
        // three frames take exactly a quarter second.
        let cs = |n: u64| (n as f64 * 100.0 / SHOW_FPS).round() as u16;
        out.push(&strip, cs(f + 1) - cs(f))?;
    }
    out.finish()?;
    Ok(frames)
}

/// The simulator's recorded SEN segment as panel frames, 15 a second.
fn sen_frames() -> Result<Vec<rackscreen_core::video::VideoRef>> {
    use rackscreen_core::event::SenEvent;
    use rackscreen_sources::sen::player::{Player, Segment};
    let mut player = Player::new(false)?;
    let mut frames = Vec::new();
    // the second pass is live from its first picture: a whole loop
    for sq in 0..2 {
        frames.clear();
        let seg = Segment {
            sq,
            data: rackscreen_sources::fake::SEN_DEMO_SEGMENT.to_vec(),
        };
        player.play(&seg, &mut |_| true, &mut |e| {
            if let SenEvent::Frame(f) = e {
                frames.push(f);
            }
        });
    }
    anyhow::ensure!(!frames.is_empty(), "the demo segment gave no frames");
    Ok(frames)
}

/// Play one clip and write it out. Returns the number of frames written.
fn record(renderer: &mut Renderer, clip: &Clip, path: &Path) -> Result<u64> {
    let mut model = Model::new(Thresholds::default());
    model.set_screens(vec![clip.roles.clone()], vec![clip.cycle_secs]);
    // One screen: nothing to serialise, and no gap between two transitions.
    model.set_one_at_a_time(false);
    // An Electricity Maps token is configured, so the grid roles show data.
    model.set_token_present(true);
    model.set_location_present(true);
    model.set_github_token_present(true);
    model.set_jellyfin_configured(true);
    model.set_printer_configured(true);
    model.set_music_configured(true);
    model.set_arr_configured(true);
    model.set_utc_offset_secs(UTC_OFFSET);
    // the clock before the first events: day-bucketed data (deploys per hour) needs it
    model.set_unix_now(UNIX_START);

    let mut fake = FakeState::new(SEED);
    fake.set_clock(UNIX_START, UTC_OFFSET);
    for ev in fake.initial() {
        model.apply(ev, 0.0);
    }

    // The sen role plays the recorded segment, as the simulator does.
    let sen = if clip.roles.contains(&Role::Sen) {
        model.apply(
            Event::Sen(rackscreen_core::event::SenEvent::Live(true)),
            0.0,
        );
        sen_frames()?
    } else {
        Vec::new()
    };
    let mut sen_at = usize::MAX;

    let warmup_frames = (clip.warmup * FPS).round() as u64;
    let clip_frames = (clip.secs * FPS).round() as u64;
    // A whole round may run a little past `secs`; twice that means it never lands.
    let total = warmup_frames
        + if clip.whole_round {
            2 * clip_frames
        } else {
            clip_frames
        };
    let steps: Vec<(u64, &Step)> = clip
        .script
        .iter()
        .map(|(at, step)| (((clip.warmup + at) * FPS).round() as u64, step))
        .collect();

    let mut out = GifOut::new(path, SIZE, SIZE)?;
    let mut px = new_pixmap();
    let mut ui = ScreenUi::new();
    let mut seconds = 0u64;
    let mut was_transitioning = false;
    let mut first: Option<u64> = None;
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
        if !sen.is_empty() {
            let at = (now * 15.0) as usize % sen.len();
            if at != sen_at {
                sen_at = at;
                let frame = sen[at].clone();
                model.apply(
                    Event::Sen(rackscreen_core::event::SenEvent::Frame(frame)),
                    now,
                );
            }
        }
        model.set_unix_now(UNIX_START + now as i64);
        model.tick(now);
        if f < warmup_frames {
            continue;
        }
        if clip.whole_round {
            let screen = model.screen(0).context("the clip has one screen")?;
            let landed =
                was_transitioning && !screen.transitioning() && screen.current() == clip.roles[0];
            was_transitioning = screen.transitioning();
            match first {
                None if landed => first = Some(f),
                None => continue,
                Some(start) if landed => {
                    out.finish()?;
                    return Ok(f - start);
                }
                Some(_) => {}
            }
        }
        renderer.render(&model.scene(0, now, &mut ui), &mut px);
        round_mask(&mut px);
        out.push(&px, DELAY)?;
    }
    anyhow::ensure!(!clip.whole_round, "{}: the round never closed", clip.name);
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
    width: u32,
    height: u32,
    prev: Pixmap,
    first: bool,
    buf: Vec<u8>,
}

impl GifOut {
    fn new(path: &Path, width: u32, height: u32) -> Result<GifOut> {
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let mut encoder = Encoder::new(BufWriter::new(file), width as u16, height as u16, &[])?;
        encoder.set_repeat(Repeat::Infinite)?;
        Ok(GifOut {
            encoder,
            width,
            height,
            prev: Pixmap::new(width, height).context("gif pixmap")?,
            first: true,
            buf: Vec::new(),
        })
    }

    /// Add `px` (the GIF's size), shown for `delay` hundredths of a second.
    fn push(&mut self, px: &Pixmap, delay: u16) -> Result<()> {
        // A frame identical to the last one still has to take up its delay, so
        // it repaints one corner pixel, which is transparent either way.
        let one_pixel = Rect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        };
        let rect = if self.first {
            Rect {
                x: 0,
                y: 0,
                w: self.width,
                h: self.height,
            }
        } else {
            dirty_rect(&self.prev, px).unwrap_or(one_pixel)
        };
        self.buf.clear();
        let data = px.data();
        for y in rect.y..rect.y + rect.h {
            let start = ((y * self.width + rect.x) * 4) as usize;
            self.buf
                .extend_from_slice(&data[start..start + (rect.w * 4) as usize]);
        }
        let mut frame = Frame::from_rgba_speed(rect.w as u16, rect.h as u16, &mut self.buf, 10);
        frame.left = rect.x as u16;
        frame.top = rect.y as u16;
        frame.delay = delay;
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

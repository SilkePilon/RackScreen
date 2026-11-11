//! SEN pipeline tools: measure it on a recorded segment, or look at the
//! real stream for a few seconds.
//!
//!     cargo run --release -p rackscreen-sources --example sen -- bench [segment.ts]
//!     cargo run --release -p rackscreen-sources --example sen -- live [seconds]
//!     cargo run --release -p rackscreen-sources --example sen -- run [seconds]
//!     cargo run --release -p rackscreen-sources --example sen -- dump <segment.ts> <out-dir>
//!
//! `SEN_PROBE_SAVE=<dir>` makes `live` keep the segments it fetched.
use std::time::{Duration, Instant};

use rackscreen_core::event::SenEvent;
use rackscreen_core::video::{CropMap, Planes, VideoFrame, VideoRef};
use rackscreen_sources::sen::decode::H264;
use rackscreen_sources::sen::detect::classify;
use rackscreen_sources::sen::player::{Player, Segment};
use rackscreen_sources::sen::ts::Demuxer;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sen_live.ts");

/// Resident set size in KB.
fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |pages| pages * 4)
}

/// CPU time this process has used, user and system.
fn cpu_time() -> Duration {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // fields 14 and 15 after the command name, in clock ticks (100 Hz)
    let ticks: u64 = stat
        .rsplit(')')
        .next()
        .map(|rest| {
            rest.split_whitespace()
                .skip(11)
                .take(2)
                .filter_map(|v| v.parse::<u64>().ok())
                .sum()
        })
        .unwrap_or(0);
    Duration::from_millis(ticks * 10)
}

struct Picture {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    width: usize,
    height: usize,
    y_stride: usize,
    c_stride: usize,
}

impl Picture {
    fn planes(&self) -> Planes<'_> {
        Planes {
            y: &self.y,
            u: &self.u,
            v: &self.v,
            width: self.width,
            height: self.height,
            y_stride: self.y_stride,
            c_stride: self.c_stride,
        }
    }
}

fn bench(path: &str) {
    rackscreen_sources::mem::tune();
    let data = std::fs::read(path).expect("read segment");
    let loops = 20;

    // decode alone
    let mut dec = H264::new().expect("decoder");
    let mut demux = Demuxer::new();
    let mut pictures = 0u32;
    let mut kept: Option<Picture> = None;
    let t = Instant::now();
    for _ in 0..loops {
        demux.begin();
        while let Some(au) = demux.next(&data) {
            dec.decode(&au, |_, p| {
                pictures += 1;
                if kept.is_none() {
                    kept = Some(Picture {
                        y: p.y.to_vec(),
                        u: p.u.to_vec(),
                        v: p.v.to_vec(),
                        width: p.width,
                        height: p.height,
                        y_stride: p.y_stride,
                        c_stride: p.c_stride,
                    });
                }
            });
        }
    }
    let decode_ms = t.elapsed().as_secs_f64() * 1000.0 / pictures.max(1) as f64;
    // RACKSCREEN_SEN_DECODER=software|v4l2 picks one
    println!("decoder {}", dec.name());
    drop(dec);
    let pic = kept.expect("a picture");
    println!("{}x{} from {path}", pic.width, pic.height);
    println!("demux + decode    {decode_ms:.3} ms per picture ({pictures} pictures)");

    // crop + convert and classify alone
    let map = CropMap::new(pic.width, pic.height).expect("map");
    let mut frame = VideoFrame::new();
    let n = 2000;
    let t = Instant::now();
    for _ in 0..n {
        map.convert(std::hint::black_box(&pic.planes()), &mut frame);
    }
    let convert_ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("crop + convert    {convert_ms:.3} ms per shown frame");
    let t = Instant::now();
    for _ in 0..n {
        std::hint::black_box(classify(std::hint::black_box(&pic.planes())));
    }
    let classify_ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("classify          {classify_ms:.3} ms per shown frame");
    println!(
        "per second of stream (30 decoded, 15 shown): {:.1} ms",
        30.0 * decode_ms + 15.0 * (convert_ms + classify_ms)
    );

    // the whole player, unpaced, holding frames as the model and a worker do
    rackscreen_sources::mem::trim();
    let before = rss_kb();
    let mut player = Player::new(false).expect("player");
    let mut held: Vec<VideoRef> = Vec::new();
    let mut frames = 0u32;
    let mut peak = before;
    let t = Instant::now();
    for sq in 0..loops {
        // a fresh copy per segment, as from the network
        let seg = Segment {
            sq,
            data: data.clone(),
        };
        player.play(&seg, &mut |_| true, &mut |e| {
            if let SenEvent::Frame(f) = e {
                frames += 1;
                held.push(f);
                if held.len() > 2 {
                    held.remove(0);
                }
            }
        });
        peak = peak.max(rss_kb());
    }
    let total = t.elapsed();
    println!(
        "player            {:.3} ms per decoded picture, {frames} frames shown ({:.0}x real time)",
        total.as_secs_f64() * 1000.0 / (loops as f64 * 150.0),
        loops as f64 * 5.0 / total.as_secs_f64()
    );
    println!(
        "RSS               {before} KB before, {peak} KB while playing (+{} KB)",
        peak.saturating_sub(before)
    );
    drop(player);
    drop(held);
    rackscreen_sources::mem::trim();
    let after = rss_kb();
    println!(
        "RSS               {after} KB after stopping ({:+} KB against before)",
        after as i64 - before as i64
    );

    // paced like the real stream: CPU share of one core
    let mut player = Player::new(true).expect("player");
    let (wall, cpu) = (Instant::now(), cpu_time());
    for sq in 0..2 {
        let seg = Segment {
            sq,
            data: data.clone(),
        };
        player.play(
            &seg,
            &mut |d| {
                std::thread::sleep(d);
                true
            },
            &mut |_| {},
        );
    }
    let wall = wall.elapsed();
    let cpu = cpu_time().saturating_sub(cpu);
    println!(
        "paced             {:.1} s of stream in {:.1} s, {:.1} % of one core",
        10.0,
        wall.as_secs_f64(),
        100.0 * cpu.as_secs_f64() / wall.as_secs_f64()
    );
}

fn write_png(path: &std::path::Path, w: usize, h: usize, rgb: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(rgb))
        .expect("write png");
}

/// What the detector says about every shown picture of a segment, and the
/// whole picture as a PNG wherever the verdict changes.
fn dump(path: &str, out: &str) {
    let data = std::fs::read(path).expect("read segment");
    std::fs::create_dir_all(out).expect("out dir");
    let mut dec = H264::new().expect("decoder");
    let mut demux = Demuxer::new();
    demux.begin();
    let (mut n, mut last) = (0u32, None);
    while let Some(au) = demux.next(&data) {
        dec.decode(&au, |_, p| {
            n += 1;
            if n % 2 != 0 {
                return;
            }
            let v = classify(p);
            if last == Some(v) {
                return;
            }
            last = Some(v);
            println!("picture {n}: {v:?}");
            let mut rgb = Vec::with_capacity(p.width * p.height * 3);
            for y in 0..p.height {
                for x in 0..p.width {
                    let c = 298 * (p.y[y * p.y_stride + x] as i32 - 16) + 128;
                    let d = p.u[(y / 2) * p.c_stride + x / 2] as i32 - 128;
                    let e = p.v[(y / 2) * p.c_stride + x / 2] as i32 - 128;
                    rgb.push(((c + 459 * e) >> 8).clamp(0, 255) as u8);
                    rgb.push(((c - 55 * d - 136 * e) >> 8).clamp(0, 255) as u8);
                    rgb.push(((c + 541 * d) >> 8).clamp(0, 255) as u8);
                }
            }
            let name = format!("{n:03}-{v:?}.png");
            write_png(
                &std::path::Path::new(out).join(name),
                p.width,
                p.height,
                &rgb,
            );
        });
    }
}

fn live(secs: u64) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let t = Instant::now();
    let result = rt.block_on(rackscreen_sources::sen::probe(secs, |line| {
        println!("[{:5.1} s] {line}", t.elapsed().as_secs_f64());
    }));
    if let Err(e) = result {
        eprintln!("failed: {e:#}");
        std::process::exit(1);
    }
}

/// The source as the app runs it: demand on for `secs` seconds, then off.
fn run(secs: u64) {
    use rackscreen_core::event::Event;
    use rackscreen_sources::sen::{run_sen, Feed};
    rackscreen_sources::mem::tune();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = rackscreen_sources::SourceCtx {
        tx,
        shutdown: tokio_util::sync::CancellationToken::new(),
    };
    let (demand, demand_rx) = tokio::sync::watch::channel(false);
    let before = rss_kb();
    rt.spawn(run_sen(Feed::Live, ctx.clone(), demand_rx));
    demand.send(true).expect("source runs");
    let (t, cpu) = (Instant::now(), cpu_time());
    let mut frames = 0;
    let mut second = 0;
    let mut held: Vec<VideoRef> = Vec::new();
    while t.elapsed() < Duration::from_secs(secs) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Sen(SenEvent::Frame(f))) => {
                frames += 1;
                held.push(f);
                if held.len() > 2 {
                    held.remove(0);
                }
            }
            Ok(Event::Sen(e)) => println!("[{:5.1} s] {e:?}", t.elapsed().as_secs_f64()),
            _ => {}
        }
        if t.elapsed().as_secs() > second {
            second = t.elapsed().as_secs();
            println!(
                "[{second:3} s] {frames:2} frames, RSS {} KB (+{} KB)",
                rss_kb(),
                rss_kb().saturating_sub(before)
            );
            frames = 0;
        }
    }
    let used = cpu_time().saturating_sub(cpu);
    println!(
        "{:.1} % of one core over {secs} s (fetch, decode, convert)",
        100.0 * used.as_secs_f64() / t.elapsed().as_secs_f64()
    );
    // demand off: the stream lingers ten seconds, then everything is freed
    demand.send(false).expect("source runs");
    drop(held);
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(14) {
        if let Ok(Event::Sen(SenEvent::Stopped)) = rx.recv_timeout(Duration::from_millis(100)) {
            std::thread::sleep(Duration::from_millis(200));
            println!(
                "stopped {:.1} s after demand ended, RSS {} KB ({:+} KB against before)",
                t.elapsed().as_secs_f64(),
                rss_kb(),
                rss_kb() as i64 - before as i64
            );
            return;
        }
    }
    println!("the stream did not stop");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["bench"] => bench(FIXTURE),
        ["bench", segment] => bench(segment),
        ["live"] => live(10),
        ["live", secs] => live(secs.parse().unwrap_or(10)),
        ["dump", segment, out] => dump(segment, out),
        ["run"] => run(15),
        ["run", secs] => run(secs.parse().unwrap_or(15)),
        _ => eprintln!("usage: sen bench [segment.ts] | sen live [seconds] | sen run [seconds] | sen dump <segment.ts> <dir>"),
    }
}

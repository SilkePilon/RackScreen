//! Scene + render time per role. Ignored by default: run in release with
//!     cargo test --release -p rackscreen-render --test bench -- --ignored --nocapture
//! The Pi 3B+ budget is 4 ms per screen at 60 fps with four render threads;
//! the desktop must stay under 1 ms (4 × margin).

use std::time::Instant;

use rackscreen_core::event::{Event, LinkTarget};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::p2::ui::ScreenUi;
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;

const BUDGET_MS: f64 = 1.0;
const ITERATIONS: u32 = 200;

fn bench_model() -> Model {
    let mut m = Model::new(Thresholds::default());
    m.apply(
        Event::Link {
            target: LinkTarget::K8sApi,
            up: true,
        },
        0.0,
    );
    m.apply(
        Event::Link {
            target: LinkTarget::Prometheus,
            up: true,
        },
        0.0,
    );
    m.apply(
        Event::Metrics {
            cpu_pct: 42.0,
            mem_pct: 67.0,
            mem_used_gb: 10.7,
            mem_total_gb: 16.0,
            hot_cpu: None,
            hot_mem: None,
        },
        0.0,
    );
    m.apply(
        Event::MetricsHistory {
            cpu: (0..31)
                .map(|i| 34.0 + 16.0 * ((i as f32) * 0.45).sin())
                .collect(),
            mem: vec![66.0; 31],
            net_rx_mbps: vec![40.0; 31],
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 7,
            total: 7,
            not_ready: vec![],
        },
        0.0,
    );
    m.tick(0.0);
    m
}

#[test]
#[ignore]
fn every_role_under_a_millisecond() {
    let m = bench_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    let mut worst = (Role::Cpu, 0.0f64);
    for role in Role::ALL {
        let mut ui = ScreenUi::new();
        // warm the glyph caches
        r.render(&m.scene_for_role(role, 0.0, &mut ui), &mut px);
        let t0 = Instant::now();
        for i in 0..ITERATIONS {
            let now = 1.0 + i as f64 / 30.0;
            r.render(&m.scene_for_role(role, now, &mut ui), &mut px);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
        println!("{:<12} {ms:.3} ms", role.name());
        if ms > worst.1 {
            worst = (role, ms);
        }
    }
    let mut ui = ScreenUi::new();
    let mut s = m.scene_for_role(Role::Cpu, 0.0, &mut ui);
    s.fade = 0.5;
    r.render(&s, &mut px);
    let t0 = Instant::now();
    for _ in 0..ITERATIONS {
        r.render(&s, &mut px);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
    println!("{:<12} {ms:.3} ms", "fade");
    assert!(ms < 2.0 * BUDGET_MS, "mid-fade frame {ms:.3} ms");
    assert!(worst.1 < BUDGET_MS, "{:?} takes {:.3} ms", worst.0, worst.1);
}

fn metrics(cpu_pct: f32) -> Event {
    Event::Metrics {
        cpu_pct,
        mem_pct: 67.0,
        mem_used_gb: 10.7,
        mem_total_gb: 16.0,
        hot_cpu: None,
        hot_mem: None,
    }
}

/// The shipped per-frame path of one screen worker, for the cpu screen with
/// a new cpu value every 30 frames (the hero rolls): render into `cur`, then
/// flush the way `ScreenFrames::flush` does: `dirty_rects` on the unrotated
/// frames, rects mapped to the panel, `pack_oriented` straight from `cur`
/// (no oriented copy), then swap `prev` and `cur`. The panel push itself is
/// not timed. `render` and `flush` are the numbers the Pi stats log.
#[test]
#[ignore]
fn pipeline_per_frame() {
    use rackscreen_render::frame::{brightness_lut, dirty_rects, pack_oriented, Orient};
    let mut m = bench_model();
    let mut r = Renderer::new().unwrap();
    let orient = Orient::new(270, false);
    let lut = brightness_lut(1.0);
    let mut cur = new_pixmap();
    let mut prev = new_pixmap();
    let mut ui = ScreenUi::new();
    let mut frame = |i: u32, m: &mut Model, timed: bool| -> (f64, f64, bool) {
        let now = i as f64 / 60.0;
        if i.is_multiple_of(30) {
            m.apply(metrics(20.0 + (i / 30 % 7) as f32 * 11.0), now);
        }
        m.tick(now);
        let t0 = Instant::now();
        r.render(&m.scene_for_role(Role::Cpu, now, &mut ui), &mut cur);
        let t1 = Instant::now();
        let rects: Vec<_> = dirty_rects(&prev, &cur)
            .into_iter()
            .map(|rc| orient.map_rect(rc))
            .collect();
        let changed = !rects.is_empty();
        if changed {
            std::hint::black_box(pack_oriented(&cur, &orient, &rects, &lut));
        }
        std::mem::swap(&mut prev, &mut cur);
        let t2 = Instant::now();
        if !timed {
            return (0.0, 0.0, changed);
        }
        (
            (t1 - t0).as_secs_f64() * 1000.0,
            (t2 - t1).as_secs_f64() * 1000.0,
            changed,
        )
    };
    // warm-up: glyph caches, the base layer, the first full diff
    for i in 0..ITERATIONS {
        frame(i, &mut m, false);
    }
    let (mut render, mut flush, mut pushed) = (0.0, 0.0, 0);
    for i in ITERATIONS..2 * ITERATIONS {
        let (a, b, changed) = frame(i, &mut m, true);
        render += a;
        flush += b;
        pushed += changed as u32;
    }
    let n = ITERATIONS as f64;
    println!("{:<12} {:.3} ms", "render", render / n);
    println!("{:<12} {:.3} ms", "flush", flush / n);
    println!("{:<12} {:.3} ms", "pipeline", (render + flush) / n);
    println!("{:<12} {pushed}/{ITERATIONS}", "changed");
    assert!(pushed > 0, "the cpu screen never changed");
}

/// The ISS globe: decoding its maps (once per process), one raster of the
/// globe alone, and a whole frame of the iss screen in a new second (globe,
/// track, station, ring, pill, then the diff and pack of `flush`). The
/// screen changes once a second, so this is its cost per second. The Pi's
/// Cortex-A53 is 8 to 12 times slower than a desktop core.
#[test]
#[ignore]
fn globe_raster() {
    use rackscreen_core::event::{IssPass, IssTrack};
    use rackscreen_render::frame::{brightness_lut, dirty_rects, pack_oriented, Orient};
    const UNIX: i64 = 1_788_782_400;
    let t = Instant::now();
    let bytes = rackscreen_render::globe::map_bytes();
    println!(
        "{:<14} {:.3} ms, {} KB held",
        "map decode",
        t.elapsed().as_secs_f64() * 1000.0,
        bytes / 1024
    );
    let mut px = new_pixmap();
    // views round the Earth and the day: coasts, oceans, the terminator
    let views: Vec<(f32, f32, f32, f32)> = (0..ITERATIONS)
        .map(|i| {
            let f = i as f32;
            (
                28.0 * (f * 0.37).sin(),
                f * 7.3 - 180.0,
                20.0 * (f * 0.11).cos(),
                180.0 - f * 4.1,
            )
        })
        .collect();
    let mut times: Vec<f64> = views
        .iter()
        .map(|v| {
            let t = Instant::now();
            rackscreen_render::globe::draw_globe(&mut px, v.0, v.1, v.2, v.3, 95.0);
            std::hint::black_box(px.data());
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "{:<14} {mean:.3} ms mean, {:.3} best, {:.3} worst",
        "globe raster",
        times[0],
        times[times.len() - 1]
    );

    // the whole screen, one second apart: a 51.6° orbit for the track
    let inc = 51.6f64.to_radians();
    let points = (0..481)
        .map(|i| {
            let t = i as f64 * 20.0;
            let u = t / 5_560.0 * std::f64::consts::TAU;
            let lat = (inc.sin() * u.sin()).asin();
            let lon = (inc.cos() * u.sin()).atan2(u.cos()) - t / 86_164.0 * std::f64::consts::TAU;
            (
                lat.to_degrees() as f32,
                ((lon.to_degrees() + 540.0).rem_euclid(360.0) - 180.0) as f32,
            )
        })
        .collect();
    let mut m = bench_model();
    m.apply(
        Event::IssPass(Some(IssPass {
            start: UNIX + 3000,
            end: UNIX + 3400,
            max_elevation_deg: 62.0,
            visible: true,
            vis: None,
        })),
        0.0,
    );
    m.apply(
        Event::IssTrack(IssTrack {
            t0: UNIX - 3600,
            step_secs: 20,
            points,
            home: (52.37, 4.89),
        }),
        0.0,
    );
    let mut r = Renderer::new().unwrap();
    let orient = Orient::new(270, false);
    let lut = brightness_lut(1.0);
    let (mut cur, mut prev) = (new_pixmap(), new_pixmap());
    let mut ui = ScreenUi::new();
    let (mut scene, mut render, mut flush, mut still) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..2 * ITERATIONS {
        let now = i as f64;
        m.set_unix_now(UNIX + i as i64);
        let t0 = Instant::now();
        let s = m.scene_for_role(Role::Iss, now, &mut ui);
        let t1 = Instant::now();
        r.render(&s, &mut cur);
        let t2 = Instant::now();
        let rects: Vec<_> = dirty_rects(&prev, &cur)
            .into_iter()
            .map(|rc| orient.map_rect(rc))
            .collect();
        std::hint::black_box(pack_oriented(&cur, &orient, &rects, &lut));
        std::mem::swap(&mut prev, &mut cur);
        let t3 = Instant::now();
        // a later frame of the same second: the scene is built and compared
        let again = m.scene_for_role(Role::Iss, now + 0.5, &mut ui);
        assert!(again == s, "the screen is still within a second");
        let t4 = Instant::now();
        if i >= ITERATIONS {
            scene += (t1 - t0).as_secs_f64() * 1000.0;
            render += (t2 - t1).as_secs_f64() * 1000.0;
            flush += (t3 - t2).as_secs_f64() * 1000.0;
            still += (t4 - t3).as_secs_f64() * 1000.0;
        }
    }
    let n = ITERATIONS as f64;
    println!("{:<14} {:.3} ms", "iss scene", scene / n);
    println!("{:<14} {:.3} ms", "iss render", render / n);
    println!("{:<14} {:.3} ms", "iss flush", flush / n);
    println!("{:<14} {:.4} ms", "skipped frame", still / n);
    assert_eq!(r.globe_rasters(), 2 * ITERATIONS, "one raster per second");
}

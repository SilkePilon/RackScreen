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
    let cpu = m.scene_for_role(Role::Cpu, 1.0, &mut ScreenUi::new());
    let mem = m.scene_for_role(Role::Mem, 1.0, &mut ScreenUi::new());
    let (out_at, inc_at) = rackscreen_core::screens::slide(0.5);
    let mut s = rackscreen_core::scene::Scene::new();
    s.slide = Some(Box::new(rackscreen_core::scene::Slide {
        out: cpu,
        out_at,
        inc: mem,
        inc_at,
    }));
    r.render(&s, &mut px);
    let t0 = Instant::now();
    for _ in 0..ITERATIONS {
        r.render(&s, &mut px);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
    println!("{:<12} {ms:.3} ms", "slide");
    assert!(ms < 2.0 * BUDGET_MS, "mid-slide frame {ms:.3} ms");
    assert!(worst.1 < BUDGET_MS, "{:?} takes {:.3} ms", worst.0, worst.1);
}

/// Render + orient + diff + pack per frame for a moving screen (cpu, the
/// hero rolls) and a slide frame: the numbers the Pi stats should match.
#[test]
#[ignore]
fn pipeline_per_frame() {
    use rackscreen_render::frame::{dirty_rect, pack_rgb565, Orient};
    let m = bench_model();
    let mut r = Renderer::new().unwrap();
    let orient = Orient::new(270, false);
    let mut cur = new_pixmap();
    let mut oriented = new_pixmap();
    let mut prev = new_pixmap();
    let mut ui = ScreenUi::new();
    let t0 = Instant::now();
    for i in 0..ITERATIONS {
        let now = i as f64 / 60.0;
        r.render(&m.scene_for_role(Role::Cpu, now, &mut ui), &mut cur);
        orient.apply(&cur, &mut oriented);
        if let Some(rect) = dirty_rect(&prev, &oriented) {
            let _ = pack_rgb565(&oriented, rect, 1.0);
        }
        std::mem::swap(&mut prev, &mut oriented);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
    println!("{:<12} {ms:.3} ms", "pipeline");
}

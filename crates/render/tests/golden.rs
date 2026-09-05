use std::path::PathBuf;

use rackscreen_core::event::{Event, LinkTarget, Torrent};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens").join(format!("{name}.png"))
}

/// Compare against the stored golden. Writes it when missing or when UPDATE_GOLDENS is set.
fn check(name: &str, px: &Pixmap) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDENS").is_some() || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, px.encode_png().unwrap()).unwrap();
        eprintln!("wrote golden {}", path.display());
        return;
    }
    let want = Pixmap::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!((want.width(), want.height()), (px.width(), px.height()));
    let bad = want
        .data()
        .chunks(4)
        .zip(px.data().chunks(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > 8))
        .count();
    let total = (px.width() * px.height()) as usize;
    assert!(bad * 200 < total, "{name}: {bad} of {total} pixels differ (run with UPDATE_GOLDENS=1 to accept)");
}

fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    (p.data()[i], p.data()[i + 1], p.data()[i + 2])
}

fn ready_model() -> Model {
    let mut m = Model::new(Thresholds::default());
    m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
    m.apply(Event::Link { target: LinkTarget::Prometheus, up: true }, 0.0);
    m.apply(
        Event::Metrics { cpu_pct: 42.0, mem_pct: 67.0, mem_used_gb: 10.7, mem_total_gb: 16.0, hot_cpu: None, hot_mem: None },
        0.0,
    );
    m.apply(Event::PodSnapshot { running: 53, pending: 2, failed: 0, total: 55 }, 0.0);
    m.apply(Event::NodeSnapshot { ready: 4, total: 4, not_ready: vec![] }, 0.0);
    m
}

#[test]
fn cpu_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Cpu, 5.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(red > 200 && g > 140 && b < 80, "segment 0 amber, got {red},{g},{b}");
    let (o, ..) = pixel(&px, 120, 222);
    assert!((20..=40).contains(&o), "segment 30 is off, got {o}");
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0));
    check("cpu_idle", &px);
}

#[test]
fn all_roles_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    for role in Role::ALL {
        let mut px = new_pixmap();
        r.render(&m.scene(role, 5.0), &mut px);
        check(&format!("idle_{}", role.icon()), &px);
    }
}

#[test]
fn connecting_and_no_data() {
    let mut r = Renderer::new().unwrap();
    let m = Model::new(Thresholds::default());
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Cpu, 0.0), &mut px);
    check("connecting", &px);
    let mut m2 = Model::new(Thresholds::default());
    m2.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
    let mut px2 = new_pixmap();
    r.render(&m2.scene(Role::Cpu, 0.5), &mut px2);
    check("no_data", &px2);
}

#[test]
fn pod_crash_splash_mid_swap() {
    let mut m = ready_model();
    m.apply(Event::PodCrashed { ns: "a".into(), name: "b".into() }, 5.0);
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Pods, 5.35), &mut px);
    check("pods_splash", &px);
}

#[test]
fn torrent_mode() {
    let mut m = ready_model();
    m.apply(Event::Link { target: LinkTarget::QBittorrent, up: true }, 0.0);
    m.apply(
        Event::Torrents(vec![
            Torrent { name: "a".into(), progress: 78.0, eta_secs: 900, speed_bps: 9_000_000 },
            Torrent { name: "b".into(), progress: 41.0, eta_secs: 3000, speed_bps: 3_000_000 },
            Torrent { name: "c".into(), progress: 12.0, eta_secs: 9000, speed_bps: 1_000_000 },
        ]),
        0.0,
    );
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Health, 5.0), &mut px);
    check("torrent", &px);
}

#[test]
fn sweep_hold() {
    let mut m = ready_model();
    m.apply(Event::NodeReady { name: "n".into(), ready: false }, 5.0);
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Health, 6.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(red > 140 && g < 120 && b < 120, "red ring at 60% pulse alpha, got {red},{g},{b}");
    check("sweep_hold", &px);
}

use std::path::PathBuf;

use rackscreen_core::event::{Event, LinkTarget, Robustness, Torrent};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(format!("{name}.png"))
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
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| (*x as i32 - *y as i32).abs() > 8)
        })
        .count();
    let total = (px.width() * px.height()) as usize;
    assert!(
        bad * 200 < total,
        "{name}: {bad} of {total} pixels differ (run with UPDATE_GOLDENS=1 to accept)"
    );
}

fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    (p.data()[i], p.data()[i + 1], p.data()[i + 2])
}

fn ready_model() -> Model {
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
        Event::PodSnapshot {
            running: 53,
            pending: 2,
            failed: 0,
            total: 55,
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 4,
            total: 4,
            not_ready: vec![],
        },
        0.0,
    );
    m.set_screens(
        vec![
            vec![Role::Cpu],
            vec![Role::Mem],
            vec![Role::Pods],
            vec![Role::Health],
        ],
        vec![15.0; 4],
    );
    m
}

/// The four roles with goldens; the six newer roles get theirs with their scenes.
const CLASSIC: [Role; 4] = [Role::Cpu, Role::Mem, Role::Pods, Role::Health];

#[test]
fn cpu_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Cpu, 5.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 200 && g > 140 && b < 80,
        "segment 0 amber, got {red},{g},{b}"
    );
    let (o, ..) = pixel(&px, 120, 222);
    assert!((20..=40).contains(&o), "segment 30 is off, got {o}");
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0));
    check("cpu_idle", &px);
}

#[test]
fn all_roles_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    for role in CLASSIC {
        let mut px = new_pixmap();
        r.render(&m.scene_for_role(role, 5.0), &mut px);
        check(&format!("idle_{}", role.icon()), &px);
    }
}

#[test]
fn connecting_and_no_data() {
    let mut r = Renderer::new().unwrap();
    let m = Model::new(Thresholds::default());
    let mut px = new_pixmap();
    // `scene` (not `scene_for_role`): the connecting scene comes from the link check.
    r.render(&m.scene(0, 0.0), &mut px);
    check("connecting", &px);
    let mut m2 = Model::new(Thresholds::default());
    m2.apply(
        Event::Link {
            target: LinkTarget::K8sApi,
            up: true,
        },
        0.0,
    );
    let mut px2 = new_pixmap();
    r.render(&m2.scene_for_role(Role::Cpu, 0.5), &mut px2);
    check("no_data", &px2);
}

#[test]
fn pod_crash_splash_mid_swap() {
    let mut m = ready_model();
    m.apply(
        Event::PodCrashed {
            ns: "a".into(),
            name: "b".into(),
        },
        5.0,
    );
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Pods.index(), 5.35), &mut px);
    check("pods_splash", &px);
}

#[test]
fn torrent_mode() {
    let mut m = ready_model();
    m.apply(
        Event::Link {
            target: LinkTarget::QBittorrent,
            up: true,
        },
        0.0,
    );
    m.apply(
        Event::Torrents(vec![
            Torrent {
                name: "a".into(),
                progress: 78.0,
                eta_secs: 900,
                speed_bps: 9_000_000,
            },
            Torrent {
                name: "b".into(),
                progress: 41.0,
                eta_secs: 3000,
                speed_bps: 3_000_000,
            },
            Torrent {
                name: "c".into(),
                progress: 12.0,
                eta_secs: 9000,
                speed_bps: 1_000_000,
            },
        ]),
        0.0,
    );
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Health, 5.0), &mut px);
    check("torrent", &px);
}

fn temps_model() -> Model {
    let mut m = ready_model();
    m.apply(
        Event::NodeTemps(
            [
                ("rack-1", 44.0),
                ("rack-2", 51.0),
                ("rack-3", 68.0),
                ("rack-4", 39.0),
                ("rack-5", 47.0),
                ("rack-6", 55.0),
                ("rack-7", 42.0),
            ]
            .iter()
            .map(|(n, c)| (n.to_string(), *c))
            .collect(),
        ),
        0.0,
    );
    m
}

fn storage_model(volumes: Vec<(String, Robustness)>) -> Model {
    let mut m = ready_model();
    m.apply(
        Event::Storage {
            volumes,
            used_bytes: 49 * (1 << 30),
            capacity_bytes: 128 * (1 << 30),
        },
        0.0,
    );
    m
}

fn volumes(n: usize, degraded: &[usize]) -> Vec<(String, Robustness)> {
    (0..n)
        .map(|i| {
            let r = if degraded.contains(&i) {
                Robustness::Degraded
            } else {
                Robustness::Healthy
            };
            (format!("pvc-{i}"), r)
        })
        .collect()
}

#[test]
fn thermal_idle() {
    let m = temps_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    // 2.5 s, not 5.0: the temperature badge fades in over each 5 s window, so at
    // exactly 5.0 it would be fully transparent.
    r.render(&m.scene_for_role(Role::Thermal, 2.5), &mut px);
    // 68 °C is deep in the amber-to-red half of the ramp
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 200 && g < 160 && b < 100,
        "segment 0 hot, got {red},{g},{b}"
    );
    check("thermal", &px);
}

#[test]
fn storage_idle_and_degraded() {
    let mut r = Renderer::new().unwrap();
    let m = storage_model(volumes(21, &[]));
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Storage, 5.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 130 && b > 200 && g < 180,
        "segment 0 violet, got {red},{g},{b}"
    );
    check("storage", &px);

    let m2 = storage_model(volumes(6, &[2]));
    let mut px2 = new_pixmap();
    r.render(&m2.scene_for_role(Role::Storage, 5.0), &mut px2);
    check("storage_degraded", &px2);
}

#[test]
fn sweep_hold() {
    let mut m = ready_model();
    m.apply(
        Event::NodeReady {
            name: "n".into(),
            ready: false,
        },
        5.0,
    );
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(3, 6.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 140 && g < 120 && b < 120,
        "red ring at 60% pulse alpha, got {red},{g},{b}"
    );
    check("sweep_hold", &px);
}

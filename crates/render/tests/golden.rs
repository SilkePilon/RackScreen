use std::path::PathBuf;

use rackscreen_core::electricity::Source;
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

/// The outer ring unwinding after its torrent finished, with the other two
/// holding their slots until it is gone.
#[test]
fn torrent_unwind() {
    let mut m = ready_model();
    m.apply(
        Event::Link {
            target: LinkTarget::QBittorrent,
            up: true,
        },
        0.0,
    );
    let rest = vec![
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
    ];
    let mut all = vec![Torrent {
        name: "a".into(),
        progress: 78.0,
        eta_secs: 900,
        speed_bps: 9_000_000,
    }];
    all.extend(rest.iter().cloned());
    m.apply(Event::Torrents(all), 0.0);
    m.tick(1.0);
    m.apply(Event::Torrents(rest), 1.0);
    m.tick(1.3);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Health, 1.3), &mut px);
    check("torrent_unwind", &px);
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

fn electricity_model() -> Model {
    let mut m = ready_model();
    m.set_token_present(true);
    // the pollers raise their link before the first payload; without it the
    // roles are treated as having no live data
    for target in [LinkTarget::Electricity, LinkTarget::Prices] {
        m.apply(Event::Link { target, up: true }, 0.0);
    }
    m.apply(
        Event::Electricity {
            zone: "NL".into(),
            mix_mw: vec![
                (Source::Solar, 4800.0),
                (Source::Wind, 2400.0),
                (Source::Gas, 1300.0),
                (Source::Coal, 700.0),
                (Source::Nuclear, 400.0),
                (Source::Biomass, 200.0),
                (Source::Hydro, 200.0),
            ],
            renewable_pct: 61.0,
            fossil_free_pct: 73.0,
            carbon_gco2: 214.0,
            updated_at: "2026-09-07T12:00:00Z".into(),
        },
        0.0,
    );
    // a cheap night, a dear evening, 22.1 ct at 14:00
    let mut ct: Vec<f32> = (0..24).map(|h| 6.2 + h as f32).collect();
    ct[14] = 22.1;
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            ct_per_kwh: ct,
            currency: "EUR".into(),
        },
        0.0,
    );
    m.set_local_hour(14);
    m
}

#[test]
fn power_mix_ring() {
    let m = electricity_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::PowerMix, 5.0), &mut px);
    // segment 0 is the head of the solar section
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 200 && g > 150 && b < 80,
        "segment 0 solar yellow, got {red},{g},{b}"
    );
    check("power_mix", &px);
}

#[test]
fn price_carbon_and_renewable() {
    let m = electricity_model();
    let mut r = Renderer::new().unwrap();

    // 1.2 s: the badge has faded in and sits in the "current price" window.
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Price, 1.2), &mut px);
    check("price", &px);

    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Carbon, 5.0), &mut px);
    check("carbon", &px);

    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Renewable, 1.2), &mut px);
    check("renewable", &px);
}

#[test]
fn electricity_without_a_token_shows_a_key() {
    let m = ready_model();
    assert!(!m.token_present());
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Carbon, 2.0), &mut px);
    check("no_token", &px);
}

fn new_roles_model() -> Model {
    use rackscreen_core::event::{App, AppHealth, AppSync, IssPass, MoonPhase};
    let mut m = ready_model();
    m.set_location_present(true);
    m.set_github_token_present(true);
    m.set_unix_now(1_788_782_400); // 2026-09-07T12:00Z
    m.set_utc_offset_secs(7200);
    for target in [
        LinkTarget::Weather,
        LinkTarget::Rain,
        LinkTarget::Github,
        LinkTarget::ArgoCd,
    ] {
        m.apply(Event::Link { target, up: true }, 0.0);
    }
    m.apply(
        Event::Weather {
            temp_c: 18.0,
            code: 2,
            is_day: true,
            wind_kmh: 23.0,
            gust_kmh: 39.0,
            wind_from_deg: 40.0,
            at: String::new(),
        },
        0.0,
    );
    m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
    let mut rain = vec![0.0f32; 24];
    for (i, v) in [
        (5, 0.3),
        (6, 1.0),
        (7, 2.5),
        (8, 5.5),
        (9, 6.5),
        (10, 4.0),
        (11, 2.0),
    ] {
        rain[i] = v;
    }
    m.apply(
        Event::Rain {
            from: 1_788_782_400,
            mm_per_h: rain,
        },
        0.0,
    );
    m.apply(
        Event::Sky {
            sunrise: Some(1_788_782_400 - 7 * 3600),
            sunset: Some(1_788_782_400 + 6 * 3600),
            sun_elevation_deg: 45.0,
            moon_illumination: 0.63,
            moon_waxing: true,
            moon_phase: MoonPhase::WaxingGibbous,
        },
        0.0,
    );
    m.apply(
        Event::IssPass(Some(IssPass {
            start: 1_788_782_400 + 42 * 60,
            end: 1_788_782_400 + 48 * 60,
            max_elevation_deg: 62.0,
            visible: true,
        })),
        0.0,
    );
    let days = (0..30)
        .map(|i| (format!("d{i}"), [5u32, 12, 30, 0, 8, 25, 43, 28][i % 8]))
        .collect();
    m.apply(Event::GithubActivity { days }, 0.0);
    m.apply(
        Event::Ups {
            on_battery: false,
            low_battery: false,
            charge_pct: 100.0,
            load_pct: 18.0,
            runtime_secs: 42 * 60,
        },
        0.0,
    );
    m.apply(
        Event::Network {
            rx_bps: 40e6,
            tx_bps: 1.3e6,
        },
        0.0,
    );
    let apps: Vec<App> = (0..16)
        .map(|i| App {
            name: format!("app-{i:02}"),
            sync: if i == 5 {
                AppSync::OutOfSync
            } else {
                AppSync::Synced
            },
            health: AppHealth::Healthy,
            operating: false,
        })
        .collect();
    m.apply(Event::Apps(apps), 0.0);
    // let the smooths settle
    let mut t = 0.0;
    while t < 2.0 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    m
}

#[test]
fn new_roles_idle() {
    let m = new_roles_model();
    let mut r = Renderer::new().unwrap();
    for (role, name) in [
        (Role::GhActivity, "gh_activity"),
        (Role::Weather, "weather"),
        (Role::Wind, "wind"),
        (Role::Aqi, "aqi"),
        (Role::Rain, "rain"),
        (Role::Sun, "sun"),
        (Role::Moon, "moon"),
        (Role::Iss, "iss"),
        (Role::Ups, "ups"),
        (Role::Net, "net"),
        (Role::Deploys, "deploys"),
    ] {
        let mut px = new_pixmap();
        // 2.5 s: the alternating badges are in their first window and faded in
        r.render(&m.scene_for_role(role, 2.5), &mut px);
        check(name, &px);
    }
}

#[test]
fn ups_on_battery_and_iss_during_pass() {
    let mut m = new_roles_model();
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 18.0,
            runtime_secs: 38 * 60,
        },
        2.0,
    );
    m.set_unix_now(1_788_782_400 + 44 * 60);
    let mut t = 2.0;
    while t < 4.0 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Ups, 4.0), &mut px);
    check("ups_on_battery", &px);
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Iss, 4.0), &mut px);
    check("iss_pass", &px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(
        red > 120 && b > 200 && g < 200,
        "violet ring during the pass, got {red},{g},{b}"
    );
}

#[test]
fn sky_without_location_shows_a_pin() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Sun, 2.0), &mut px);
    check("no_location", &px);
}

#[test]
fn iris_mid_transition() {
    // one screen cycling cpu -> mem: tick past the dwell and catch the iris in
    // its outgoing half, where the scene is both shrunk and partly unlit
    let mut m = ready_model();
    m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
    let mut t = 0.0;
    while t < 3.05 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    let s = m.scene(0, 3.25);
    assert!(s.zoom < 0.5 && s.zoom > 0.1, "zoom {}", s.zoom);
    assert!(s.ring_reveal < 1.0, "reveal {}", s.ring_reveal);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&s, &mut px);
    check("iris_mid", &px);
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

use std::path::PathBuf;

use rackscreen_core::electricity::Source;
use rackscreen_core::event::{
    App, AppHealth, AppSync, CiRun, CiStatus, Download, Event, IssPass, JfKind, JfSession,
    JfStatus, LinkTarget, MoonPhase, NodeLoad, Robustness, Torrent,
};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::mood::Mood;
use rackscreen_core::p2::ui::ScreenUi;
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

fn render(m: &Model, role: Role, now: f64) -> Pixmap {
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(role, now, &mut ScreenUi::new()), &mut px);
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0), "{role:?}: the corner is black");
    px
}

const UNIX: i64 = 1_788_782_400; // 2026-09-07 12:00 UTC, Amsterdam (+2): a Monday, 14:00 local
const OFFSET: i32 = 7200;

fn link(m: &mut Model, target: LinkTarget) {
    m.apply(Event::Link { target, up: true }, 0.0);
}

/// Every source healthy and busy, the clock fixed.
/// Twelve runs, newest first: the newest in `newest`, the fifth-oldest failed, the rest passed.
fn ci_runs(newest: CiStatus) -> Vec<CiRun> {
    (1..=12)
        .rev()
        .map(|i| {
            let status = if i == 12 {
                newest
            } else if i == 5 {
                CiStatus::Failure
            } else {
                CiStatus::Success
            };
            let duration = (status != CiStatus::InProgress).then_some(120 + i as i64 * 3);
            CiRun {
                id: i,
                repo: "silkepilon/RackScreen".into(),
                status,
                created_at: UNIX - (13 - i as i64) * 1800,
                updated_at: UNIX - (13 - i as i64) * 1800 + duration.unwrap_or(0),
                duration_secs: duration,
            }
        })
        .collect()
}

fn jf_movie(paused: bool) -> JfSession {
    JfSession {
        title: "Blade Runner 2049".into(),
        kind: JfKind::Movie,
        series: None,
        season: None,
        episode: None,
        year: Some(2017),
        runtime_secs: 9840.0,
        position_secs: 3720.0,
        paused,
        transcoding: false,
        user: "silke".into(),
        device: "Living room TV".into(),
        last_activity: "2026-09-07T12:00:05.000Z".into(),
    }
}

fn jf_episode() -> JfSession {
    JfSession {
        title: "Home".into(),
        kind: JfKind::Episode,
        series: Some("The Expanse".into()),
        season: Some(2),
        episode: Some(5),
        year: None,
        runtime_secs: 2640.0,
        position_secs: 1480.0,
        paused: false,
        transcoding: true,
        user: "sam".into(),
        device: "iPad".into(),
        last_activity: "2026-09-07T12:00:02.000Z".into(),
    }
}

fn full_model() -> Model {
    let mut m = Model::new(Thresholds::default());
    m.set_unix_now(UNIX);
    m.set_utc_offset_secs(OFFSET);
    m.set_local_slot(56);
    m.set_token_present(true);
    m.set_location_present(true);
    m.set_github_token_present(true);
    m.set_jellyfin_configured(true);
    for t in [
        LinkTarget::K8sApi,
        LinkTarget::Prometheus,
        LinkTarget::Electricity,
        LinkTarget::Prices,
        LinkTarget::Weather,
        LinkTarget::Rain,
        LinkTarget::Github,
        LinkTarget::ArgoCd,
    ] {
        link(&mut m, t);
    }
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
            mem: (0..31)
                .map(|i| 66.0 + 3.0 * ((i as f32) * 0.3).sin())
                .collect(),
            net_rx_mbps: (0..31)
                .map(|i| 40.0 + 25.0 * ((i as f32) * 0.5).sin())
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::PodSnapshot {
            running: 38,
            pending: 2,
            failed: 0,
            total: 40,
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
    m.apply(
        Event::NodeTemps(
            [
                ("hp-elitedesk-800-g5-i7", 46.0),
                ("hp-elitedesk-800-g6-i5", 44.0),
                ("hp-elitedesk-800-g6-i7", 47.0),
                ("raspberrypi-5-16gb-1", 52.0),
                ("raspberrypi-5-8gb-1", 49.0),
                ("raspberrypi-5-8gb-2", 54.0),
                ("raspberrypi-5-8gb-3", 51.0),
            ]
            .iter()
            .map(|(n, c)| (n.to_string(), *c))
            .collect(),
        ),
        0.0,
    );
    let tb = 1u64 << 40;
    m.apply(
        Event::Storage {
            volumes: vec![
                ("prom".into(), Robustness::Healthy),
                ("qbit".into(), Robustness::Healthy),
                ("graf".into(), Robustness::Healthy),
            ],
            used_bytes: tb + tb / 5,
            capacity_bytes: 3 * tb + tb / 10,
        },
        0.0,
    );
    m.apply(
        Event::VolumeSizes(vec![
            ("prom".into(), tb / 2),
            ("qbit".into(), 2 * tb / 5),
            ("graf".into(), 3 * tb / 10),
        ]),
        0.0,
    );
    for h in 0..24 {
        m.set_unix_now(UNIX - (23 - h) * 3600);
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: vec![
                    (Source::Wind, 3800.0),
                    (Source::Solar, 2400.0),
                    (Source::Gas, 1400.0),
                    (Source::Nuclear, 1200.0),
                    (Source::Coal, 700.0),
                    (Source::Biomass, 500.0),
                ],
                renewable_pct: 55.0
                    + 20.0 * (((h as f32) - 6.0) / 24.0 * std::f32::consts::TAU).sin(),
                fossil_free_pct: 82.0,
                carbon_gco2: 230.0 + 80.0 * (-((h as f32 - 19.0).powi(2)) / 8.0).exp()
                    - 90.0 * (-((h as f32 - 13.0).powi(2)) / 10.0).exp(),
                updated_at: String::new(),
            },
            0.0,
        );
    }
    m.set_unix_now(UNIX);
    let eur: Vec<f32> = (0..96)
        .map(|i| {
            let h = (i / 4) as f32;
            0.11 + 0.06 * (-((h - 19.0).powi(2)) / 5.0).exp()
                + 0.035 * (-((h - 8.0).powi(2)) / 3.0).exp()
                - 0.05 * (-((h - 13.0).powi(2)) / 7.0).exp()
        })
        .collect();
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            avg_eur_per_kwh: eur.iter().sum::<f32>() / 96.0,
            eur_per_kwh: eur,
            currency: "EUR".into(),
        },
        0.0,
    );
    m.apply(
        Event::Weather {
            temp_c: 17.0,
            code: 2,
            is_day: true,
            wind_kmh: 8.0,
            gust_kmh: 15.0,
            wind_from_deg: 225.0,
            at: String::new(),
        },
        0.0,
    );
    m.apply(
        Event::Forecast {
            hourly_temp_c: (0..12)
                .map(|i| 13.0 + 7.0 * (((i as f32) + 4.0) / 24.0 * std::f32::consts::TAU).sin())
                .collect(),
            hourly_precip_mm: vec![0.0, 0.0, 0.0, 0.0, 1.1, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            day_min_c: 11.0,
            day_max_c: 21.0,
        },
        0.0,
    );
    m.apply(Event::AirQuality { eaqi: 27.0 }, 0.0);
    m.apply(
        Event::AirQualityHistory {
            hourly: (0..24)
                .map(|i| 22.0 + 10.0 * ((i as f32) / 24.0 * std::f32::consts::TAU * 1.5).sin())
                .collect(),
        },
        0.0,
    );
    let mut rain = vec![0.0f32; 24];
    for (i, v) in [
        (1, 0.4),
        (2, 1.0),
        (3, 1.4),
        (4, 1.8),
        (5, 1.6),
        (6, 1.2),
        (7, 0.8),
        (8, 0.3),
    ] {
        rain[i] = v;
    }
    m.apply(
        Event::Rain {
            from: UNIX,
            mm_per_h: rain,
        },
        0.0,
    );
    m.apply(
        Event::Sky {
            sunrise: Some(UNIX - 7 * 3600),
            sunset: Some(UNIX + 6 * 3600),
            sun_elevation_deg: 45.0,
            moon_illumination: 0.64,
            moon_waxing: true,
            moon_phase: MoonPhase::WaxingGibbous,
        },
        0.0,
    );
    m.apply(
        Event::IssPass(Some(IssPass {
            start: UNIX + 14 * 60 + 32,
            end: UNIX + 20 * 60 + 32,
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
            load_pct: 7.0,
            runtime_secs: 49 * 60,
        },
        0.0,
    );
    m.apply(
        Event::Network {
            rx_bps: 68e6,
            tx_bps: 6e6,
        },
        0.0,
    );
    let apps: Vec<App> = [
        "arr-stack",
        "cloudflared",
        "grafana",
        "homeassistant",
        "longhorn",
        "monitoring",
        "n8n",
        "pihole",
        "tailscale",
    ]
    .iter()
    .map(|n| App {
        name: n.to_string(),
        sync: AppSync::Synced,
        health: AppHealth::Healthy,
        operating: false,
    })
    .collect();
    m.apply(Event::Apps(apps), 0.0);
    m.apply(
        Event::AppSyncHistory {
            finished_at: vec![
                UNIX - 6 * 3600,
                UNIX - 5 * 3600,
                UNIX - 5 * 3600 + 900,
                UNIX - 3 * 3600,
                UNIX - 1800,
            ],
        },
        0.0,
    );
    m.apply(
        Event::AppSynced {
            name: "grafana".into(),
        },
        0.0,
    );
    let nodes = [
        "hp-elitedesk-800-g5-i7",
        "hp-elitedesk-800-g6-i5",
        "hp-elitedesk-800-g6-i7",
        "raspberrypi-5-16gb-1",
        "raspberrypi-5-8gb-1",
        "raspberrypi-5-8gb-2",
        "raspberrypi-5-8gb-3",
    ];
    m.apply(
        Event::TrafficHistory {
            req_per_sec: (0..31)
                .map(|i| (3.2 + 1.6 * ((i as f32) * 0.4).sin()).max(0.0))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Traffic {
            req_per_sec: 4.1,
            top: vec![
                ("n8n".into(), 1.9),
                ("portfolio".into(), 1.1),
                ("grafana".into(), 0.6),
            ],
            service_rps: 4.0,
            error_share: 0.0,
        },
        0.0,
    );
    let hour = UNIX.div_euclid(3600);
    m.apply(
        Event::RestartsHistory {
            hourly: (0..24)
                .map(|i| {
                    (
                        hour - 23 + i,
                        [
                            0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 7.0, 9.0, 11.0, 6.0, 3.0,
                            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                        ][i as usize],
                    )
                })
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Restarts {
            total_24h: 41,
            worst: Some(("n8n-worker".into(), 33)),
        },
        0.0,
    );
    m.apply(
        Event::Uptime(
            [79.6, 79.6, 79.6, 82.1, 41.0, 3.4, 4.4]
                .iter()
                .zip(nodes)
                .map(|(d, n)| (n.to_string(), d * 86_400.0))
                .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::Fans(
            [4885.0, 4538.0, 6059.0, 5104.0]
                .iter()
                .zip(&nodes[3..])
                .map(|(r, n)| (n.to_string(), *r))
                .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::MainsHistory {
            hourly: (0..24)
                .map(|i| (hour - 23 + i, 233.0 + 2.0 * ((i as f32) / 3.0).sin()))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Mains {
            volts: 234.0,
            ups_load_pct: 7.0,
        },
        0.0,
    );
    m.apply(
        Event::Load(
            [
                (0.81, 8),
                (0.02, 12),
                (0.12, 16),
                (0.22, 4),
                (0.13, 4),
                (3.1, 4),
                (1.02, 4),
            ]
            .iter()
            .zip(nodes)
            .map(|((l1, cores), n)| NodeLoad {
                node: n.to_string(),
                load1: *l1,
                load5: l1 * 0.85,
                load15: l1 * 0.7,
                cores: *cores,
            })
            .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::MediaLibrary {
            movies: 37,
            series: 30,
            artists: 54,
        },
        0.0,
    );
    m.apply(
        Event::Download(Some(Download {
            title: "The Lord of the Rings: The Return of the King".into(),
            progress: 0.35,
            eta_secs: 19 * 60 + 12,
        })),
        0.0,
    );
    m.apply(Event::CiRuns(ci_runs(CiStatus::InProgress)), 0.0);
    m.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(false), jf_episode()],
        },
        0.0,
    );
    m.tick(0.0);
    m
}

#[test]
fn every_role_normal() {
    let m = full_model();
    for role in Role::ALL {
        let px = render(&m, role, 120.0);
        check(&format!("p2_{}", role.name()), &px);
    }
}

#[test]
fn warning_and_alternate_states() {
    let mut m = full_model();
    m.apply(
        Event::Metrics {
            cpu_pct: 93.0,
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
            running: 38,
            pending: 1,
            failed: 2,
            total: 41,
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 6,
            total: 7,
            not_ready: vec!["raspberrypi-5-8gb-3".into()],
        },
        0.0,
    );
    m.apply(
        Event::AlertSnapshot {
            firing: vec!["KubeNodeNotReady".into()],
        },
        0.0,
    );
    let tb = 1u64 << 40;
    m.apply(
        Event::Storage {
            volumes: vec![
                ("prom".into(), Robustness::Healthy),
                ("qbit".into(), Robustness::Degraded),
                ("graf".into(), Robustness::Healthy),
            ],
            used_bytes: tb + tb / 5,
            capacity_bytes: 3 * tb + tb / 10,
        },
        0.0,
    );
    m.apply(
        Event::NodeTemps(vec![
            ("hp-elitedesk-800-g5-i7".into(), 46.0),
            ("raspberrypi-5-8gb-1".into(), 72.0),
            ("raspberrypi-5-8gb-2".into(), 66.0),
        ]),
        0.0,
    );
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 7.0,
            runtime_secs: 38 * 60,
        },
        0.0,
    );
    let mut apps = m.apps().apps.clone();
    apps[2].health = AppHealth::Degraded;
    apps[5].sync = AppSync::OutOfSync;
    apps[5].operating = true;
    m.apply(Event::Apps(apps), 0.0);
    m.apply(
        Event::Rain {
            from: UNIX,
            mm_per_h: vec![2.3; 24],
        },
        0.0,
    );
    m.apply(Event::AirQuality { eaqi: 72.0 }, 0.0);
    m.tick(0.0);
    for role in [
        Role::Cpu,
        Role::Pods,
        Role::Health,
        Role::Storage,
        Role::Thermal,
        Role::Ups,
        Role::Deploys,
        Role::Rain,
        Role::Aqi,
    ] {
        check(
            &format!("p2_{}_warn", role.name()),
            &render(&m, role, 120.0),
        );
    }
    // health in torrent mode
    let mut t = full_model();
    link(&mut t, LinkTarget::QBittorrent);
    t.apply(
        Event::Torrents(vec![
            Torrent {
                name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
                progress: 78.0,
                eta_secs: 900,
                speed_bps: 9_000_000,
            },
            Torrent {
                name: "debian-13.iso".into(),
                progress: 41.0,
                eta_secs: 3000,
                speed_bps: 3_000_000,
            },
        ]),
        0.0,
    );
    t.tick(0.0);
    check("p2_health_torrent", &render(&t, Role::Health, 120.0));
    // the ISS overhead and a dry nowcast
    let mut i = full_model();
    i.set_unix_now(UNIX + 16 * 60);
    check("p2_iss_pass", &render(&i, Role::Iss, 120.0));
    let mut d = full_model();
    d.apply(
        Event::Rain {
            from: UNIX,
            mm_per_h: vec![0.0; 24],
        },
        0.0,
    );
    check("p2_rain_dry", &render(&d, Role::Rain, 120.0));
    // sunrise side of the sun role
    let mut n = full_model();
    n.set_unix_now(UNIX + 9 * 3600);
    check("p2_sun_night", &render(&n, Role::Sun, 120.0));
}

#[test]
fn no_data_token_location_and_connecting() {
    let mut m = Model::new(Thresholds::default());
    link(&mut m, LinkTarget::K8sApi);
    for role in Role::ALL {
        if role == Role::Face {
            continue;
        }
        check(
            &format!("p2_{}_nodata", role.name()),
            &render(&m, role, 0.0),
        );
    }
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    let bare = Model::new(Thresholds::default());
    r.render(&bare.scene(0, 0.4, &mut ScreenUi::new()), &mut px);
    check("connecting", &px);
}

#[test]
fn face_moods() {
    let mut m = full_model();
    // walk the priority table from the bottom up by adding one condition at a time
    let capture = |m: &mut Model, mood: Mood| {
        let mut t = 0.0;
        while t < 2.0 {
            m.tick(t);
            t += 1.0 / 30.0;
        }
        assert_eq!(m.face_mood(), mood);
        let px = render(m, Role::Face, 2.0);
        check(&format!("face_{}", mood.name()), &px);
    };
    // a flat price day: the afternoon dip in `full_model` would otherwise read as cheap and happy
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            eur_per_kwh: vec![0.15; 96],
            avg_eur_per_kwh: 0.15,
            currency: "EUR".into(),
        },
        0.0,
    );
    capture(&mut m, Mood::Content);
    m.apply(
        Event::Weather {
            temp_c: 21.0,
            code: 0,
            is_day: true,
            wind_kmh: 1.0,
            gust_kmh: 1.0,
            wind_from_deg: 0.0,
            at: String::new(),
        },
        0.0,
    );
    capture(&mut m, Mood::Happy);
    m.set_bedtime_near(true);
    capture(&mut m, Mood::Sleepy);
    m.apply(
        Event::PodSnapshot {
            running: 38,
            pending: 0,
            failed: 1,
            total: 39,
        },
        0.0,
    );
    capture(&mut m, Mood::Worried);
    m.apply(
        Event::NodeTemps(vec![("raspberrypi-5-8gb-1".into(), 80.0)]),
        0.0,
    );
    capture(&mut m, Mood::Hot);
    m.apply(
        Event::AlertSnapshot {
            firing: vec!["Down".into()],
        },
        0.0,
    );
    capture(&mut m, Mood::Angry);
    m.apply(
        Event::NodeSnapshot {
            ready: 6,
            total: 7,
            not_ready: vec!["pi".into()],
        },
        0.0,
    );
    capture(&mut m, Mood::Sad);
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 7.0,
            runtime_secs: 38 * 60,
        },
        0.0,
    );
    capture(&mut m, Mood::Scared);
}

#[test]
fn iris_mid_transition() {
    let mut m = full_model();
    m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
    let mut t = 0.0;
    while t < 3.05 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    let mut ui = ScreenUi::new();
    let s = m.scene(0, 3.25, &mut ui);
    assert!(s.zoom < 0.5 && s.zoom > 0.1, "zoom {}", s.zoom);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&s, &mut px);
    check("iris_mid", &px);
}

#[test]
fn p2_sample_screen() {
    // kept from Task 7: the composer on hand-written data
    use rackscreen_core::p2::chart::Card;
    use rackscreen_core::p2::ring::RingSpec;
    use rackscreen_core::p2::screen::{compose, P2Screen};
    use rackscreen_core::theme::p2::{BLUE, TEXT2};
    let mut s = P2Screen::new("cpu", "42");
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(0.42, BLUE);
    s.pill = Some(("7 nodes · avg".into(), TEXT2));
    s.card = Some(Card::Spark {
        series: (0..31)
            .map(|i| 30.0 + 20.0 * ((i as f32) * 0.4).sin())
            .collect(),
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max: 100.0,
        now_idx: None,
        color: BLUE,
        caption: "last 60 s".into(),
    });
    let mut ui = ScreenUi::new();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&compose(&s, &mut ui, 5.0), &mut px);
    check("p2_sample", &px);
}

#[test]
fn p2_sample_download_and_lines() {
    use rackscreen_core::p2::chart::{Card, CardLine};
    use rackscreen_core::p2::ring::RingSpec;
    use rackscreen_core::p2::screen::{compose, P2Screen};
    use rackscreen_core::scene::FontId;
    use rackscreen_core::theme::p2::{BLUE, CYAN, ORANGE, TEXT, TEXT2, TEXT3, VIOLET};
    let mut s = P2Screen::new("media", "121");
    s.ring = RingSpec::parts(vec![
        (37.0 / 121.0, ORANGE),
        (30.0 / 121.0, CYAN),
        (54.0 / 121.0, VIOLET),
    ]);
    s.pill = Some(("1 downloading".into(), BLUE));
    s.card = Some(Card::Download {
        title: "The Lord of the Rings: The Return of the King".into(),
        progress: 0.35,
        label: "35 % · 19 min left".into(),
        done: false,
    });
    let mut ui = ScreenUi::new();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&compose(&s, &mut ui, 3.0), &mut px);
    check("p2_sample_download", &px);
    let mut j = P2Screen::new("now playing", "1:41:54");
    j.hero_scale = 0.62;
    j.unit = Some("left".into());
    j.unit_below = true;
    j.ring = RingSpec::fill(0.38, ORANGE);
    j.pill = Some(("Direct play".into(), rackscreen_core::theme::p2::OK));
    j.card = Some(Card::Lines(vec![
        CardLine {
            text: "Blade Runner 2049".into(),
            font: FontId::Bold,
            px: 11.0,
            color: TEXT,
            dy: 7.0,
        },
        CardLine {
            text: "2017 · 2h 44m".into(),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT2,
            dy: 21.0,
        },
        CardLine {
            text: "silke · Living room TV  ·  +1 more".into(),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT3,
            dy: 32.0,
        },
    ]));
    let mut ui = ScreenUi::new();
    r.render(&compose(&j, &mut ui, 3.0), &mut px);
    check("p2_sample_lines", &px);
}

#[test]
fn rack_roles_warning_states() {
    let mut m = full_model();
    let hour = UNIX.div_euclid(3600);
    m.apply(
        Event::Traffic {
            req_per_sec: 12.4,
            top: vec![
                ("n8n".into(), 9.0),
                ("portfolio".into(), 2.0),
                ("grafana".into(), 1.0),
            ],
            service_rps: 12.0,
            error_share: 0.06,
        },
        0.0,
    );
    m.apply(
        Event::RestartsHistory {
            hourly: (0..24)
                .map(|i| (hour - 23 + i, if i >= 20 { 8.0 } else { 0.0 }))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Restarts {
            total_24h: 32,
            worst: Some(("signal-cli".into(), 32)),
        },
        0.0,
    );
    m.apply(
        Event::Uptime(vec![
            ("hp-elitedesk-800-g5-i7".into(), 79.6 * 86_400.0),
            ("raspberrypi-5-8gb-2".into(), 9.0 * 3600.0),
            ("raspberrypi-5-8gb-3".into(), 4.4 * 86_400.0),
        ]),
        0.0,
    );
    m.apply(
        Event::Fans(vec![
            ("raspberrypi-5-8gb-3".into(), 7420.0),
            ("raspberrypi-5-8gb-1".into(), 5100.0),
        ]),
        0.0,
    );
    m.apply(
        Event::Mains {
            volts: 212.0,
            ups_load_pct: 7.0,
        },
        0.0,
    );
    let mut load = m.load().nodes.clone();
    load[5].load1 = 3.9;
    m.apply(Event::Load(load), 0.0);
    m.tick(0.0);
    for role in [
        Role::Traffic,
        Role::Restarts,
        Role::Uptime,
        Role::Fans,
        Role::Mains,
        Role::Load,
    ] {
        check(
            &format!("p2_{}_warn", role.name()),
            &render(&m, role, 120.0),
        );
    }
    let mut e = full_model();
    e.apply(
        Event::NoExporter {
            role: Role::Traffic,
        },
        0.0,
    );
    check("p2_traffic_noexporter", &render(&e, Role::Traffic, 120.0));
}

#[test]
fn media_alternate_states() {
    let mut a = full_model();
    a.apply(Event::Download(None), 0.0);
    a.apply(
        Event::MediaAdded {
            title: "Dune: Part Two".into(),
        },
        100.0,
    );
    check("p2_media_added", &render(&a, Role::Media, 120.0));
    let mut t = full_model();
    t.apply(Event::Download(None), 0.0);
    link(&mut t, LinkTarget::QBittorrent);
    t.apply(
        Event::Torrents(vec![Torrent {
            name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
            progress: 78.0,
            eta_secs: 900,
            speed_bps: 9_000_000,
        }]),
        0.0,
    );
    check("p2_media_torrent", &render(&t, Role::Media, 120.0));
    let mut i = full_model();
    i.apply(Event::Download(None), 0.0);
    check("p2_media_idle", &render(&i, Role::Media, 120.0));
}

#[test]
fn ci_alternate_states() {
    let mut f = full_model();
    f.apply(Event::CiRuns(ci_runs(CiStatus::Failure)), 0.0);
    check("p2_ci_warn", &render(&f, Role::Ci, 120.0));
    let mut p = full_model();
    p.apply(Event::CiRuns(ci_runs(CiStatus::Success)), 0.0);
    check("p2_ci_passed", &render(&p, Role::Ci, 120.0));
}

#[test]
fn jellyfin_alternate_states() {
    let mut e = full_model();
    e.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(false)],
        },
        0.0,
    );
    check("p2_jellyfin_movie", &render(&e, Role::Jellyfin, 120.0));
    let mut p = full_model();
    p.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(true)],
        },
        0.0,
    );
    check("p2_jellyfin_paused", &render(&p, Role::Jellyfin, 120.0));
    let mut i = full_model();
    i.set_unix_now(UNIX + 2 * 3600);
    i.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_idle", &render(&i, Role::Jellyfin, 120.0));
    let mut o = full_model();
    o.apply(
        Event::Jellyfin {
            status: JfStatus::Offline,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_offline", &render(&o, Role::Jellyfin, 120.0));
    let mut k = full_model();
    k.apply(
        Event::Jellyfin {
            status: JfStatus::Unauthorized,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_key", &render(&k, Role::Jellyfin, 120.0));
    let mut c = full_model();
    c.set_jellyfin_configured(false);
    check("p2_jellyfin_config", &render(&c, Role::Jellyfin, 120.0));
}

/// Brightest channel of a colour as it lands on black.
fn lit(c: rackscreen_core::theme::Color, alpha: f32) -> u8 {
    let k = (c.a as f32 / 255.0) * alpha.clamp(0.0, 1.0);
    [c.r, c.g, c.b]
        .iter()
        .map(|v| (*v as f32 * k).round() as u8)
        .max()
        .unwrap()
}

fn floor_violations(s: &rackscreen_core::scene::Scene) -> Vec<String> {
    use rackscreen_core::scene::Drawable as D;
    use rackscreen_core::theme::p2::INK;
    use rackscreen_core::theme::BLACK;
    let cut_out = |c: rackscreen_core::theme::Color| {
        (c.r, c.g, c.b) == (BLACK.r, BLACK.g, BLACK.b) || (c.r, c.g, c.b) == (INK.r, INK.g, INK.b)
    };
    let mut bad = Vec::new();
    for d in &s.items {
        let (what, color, alpha, min) = match d {
            D::Clear(_) => continue,
            D::Arc { color, alpha, .. } => ("arc", *color, *alpha, 0x2A),
            D::RoundRect { color, alpha, .. } => ("rect", *color, *alpha, 0x2A),
            D::Polygon { color, alpha, .. } => ("polygon", *color, *alpha, 0x2A),
            D::Ellipse { color, alpha, .. } => ("ellipse", *color, *alpha, 0x2A),
            D::Polyline { color, alpha, .. } => ("polyline", *color, *alpha, 0x2A),
            D::Icon { color, alpha, .. } => ("icon", *color, *alpha, 0x2A),
            D::Run {
                color, alpha, text, ..
            } => {
                if text.trim().is_empty() {
                    continue;
                }
                ("run", *color, *alpha, 0x78)
            }
            D::CurvedText { color, alpha, .. } => ("title", *color, *alpha, 0x78),
        };
        if cut_out(color) || alpha <= 0.0 {
            continue;
        }
        let v = lit(color, alpha);
        if v < min {
            bad.push(format!(
                "{what} {color:?} × {alpha} = {v:#04x} < {min:#04x}"
            ));
        }
    }
    bad
}

#[test]
fn contrast_floor_on_every_role() {
    let full = full_model();
    let mut bare = Model::new(Thresholds::default());
    bare.set_token_present(true);
    bare.set_location_present(true);
    bare.set_github_token_present(true);
    bare.set_jellyfin_configured(true);
    link(&mut bare, LinkTarget::K8sApi);
    link(&mut bare, LinkTarget::Prometheus);
    let mut problems = Vec::new();
    for (label, m) in [("normal", &full), ("no data", &bare)] {
        for role in Role::ALL {
            let s = m.scene_for_role(role, 120.0, &mut ScreenUi::new());
            for p in floor_violations(&s) {
                problems.push(format!("{} ({label}): {p}", role.name()));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "too dark for the panel:\n{}",
        problems.join("\n")
    );
}

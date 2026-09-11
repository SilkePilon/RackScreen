//! Scripted data for the simulator. Drifts metrics, churns pods, and reacts to keyboard commands.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use rackscreen_core::electricity::Source;
use rackscreen_core::event::{
    App, AppHealth, AppSync, Event, IssPass, LinkTarget, Robustness, Torrent,
};

use crate::astro::sky_event;
use crate::SourceCtx;

/// Cluster nodes and their idle temperature in degrees Celsius.
const NODES: [(&str, f32); 7] = [
    ("hp-elitedesk-800-g5-i7", 53.9),
    ("hp-elitedesk-800-g6-i5", 41.9),
    ("hp-elitedesk-800-g6-i7", 51.0),
    ("raspberrypi-5-16gb-1", 42.1),
    ("raspberrypi-5-8gb-1", 48.5),
    ("raspberrypi-5-8gb-2", 40.4),
    ("raspberrypi-5-8gb-3", 45.6),
];

/// The node the `h` key overheats, and the temperature it is held at.
const HOT_NODE: usize = 0;
const HOT_C: f32 = 78.0;

const VOLUME_COUNT: usize = 21;
const STORAGE_USED: u64 = 49 * 1024 * 1024 * 1024;
const STORAGE_CAP: u64 = 128 * 1024 * 1024 * 1024;

const ZONE: &str = "NL";

/// Production per source in MW, roughly a sunny Dutch midday.
const MIX_SEED: [(Source, f32); 9] = [
    (Source::Solar, 9200.0),
    (Source::Wind, 3100.0),
    (Source::Gas, 2600.0),
    (Source::Coal, 800.0),
    (Source::Biomass, 300.0),
    (Source::Nuclear, 120.0),
    (Source::HydroStorage, 90.0),
    (Source::Unknown, 40.0),
    (Source::Hydro, 30.0),
];

const SOLAR_PEAK: f32 = 9200.0;
/// Ticks in the compressed demo "day" the solar curve runs through.
const SOLAR_CYCLE: u64 = 240;

/// Day-ahead price curve in €/kWh per hour; `prices()` interpolates it to quarter-hours.
const PRICE_CURVE: [f32; 24] = [
    0.22, 0.19, 0.17, 0.16, 0.15, 0.15, 0.17, 0.21, 0.24, 0.22, 0.18, 0.13, 0.09, 0.07, 0.06, 0.08,
    0.12, 0.19, 0.26, 0.29, 0.27, 0.24, 0.22, 0.21,
];
const PRICE_DATE: &str = "2026-09-06";

/// Simulated observer: Amsterdam.
const FAKE_LAT: f64 = 52.37;
const FAKE_LON: f64 = 4.89;
/// WMO codes the demo weather cycles through: clear, partly cloudy, rain, thunder.
const WEATHER_CYCLE: [u16; 4] = [0, 2, 61, 95];
/// Rain nowcast seed, mm/h per five-minute slot: dry, a shower, dry.
const RAIN_SEED: [f32; 24] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.3, 1.0, 2.5, 5.5, 6.5, 6.0, 4.0, 3.0, 2.0, 1.5, 0.8, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0,
];
/// Contribution counts for the last 30 days, today last.
const GH_SEED: [u32; 30] = [
    3, 7, 0, 12, 5, 9, 2, 0, 14, 6, 8, 1, 4, 11, 3, 0, 9, 17, 6, 2, 5, 8, 0, 3, 13, 7, 9, 4, 43, 28,
];
/// Argo CD applications, named like the real cluster.
const APP_NAMES: [&str; 16] = [
    "arr-stack",
    "cloudflared",
    "hermes",
    "homeassistant",
    "longhorn",
    "monitoring",
    "n8n",
    "node-maintenance",
    "open-webui",
    "pihole",
    "plane",
    "portfolio",
    "root",
    "stirling-pdf",
    "tailscale",
    "twenty",
];
/// The app the `d` key degrades.
const DEGRADED_APP: usize = 5;
/// Minutes until the demo ISS pass, and its length.
const ISS_PASS_IN_SECS: i64 = 42 * 60;
const ISS_PASS_SECS: i64 = 6 * 60;

/// Sources Electricity Maps counts as renewable; fossil-free adds nuclear.
fn is_renewable(s: Source) -> bool {
    matches!(
        s,
        Source::Solar
            | Source::Wind
            | Source::Hydro
            | Source::HydroStorage
            | Source::Biomass
            | Source::Geothermal
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCmd {
    PodStarted,
    PodCrashed,
    NodeDown,
    NodeUp,
    AlertToggle,
    TorrentDone,
    LinkDown,
    LinkUp,
    ToggleTorrent,
    ToggleNight,
    Boot,
    DegradeVolume,
    HealVolume,
    HotTemp,
    PriceOutage,
    UpsToggle,
    AppToggle,
    GithubPush,
    GithubStar,
    CiFailure,
    Thunder,
    RainSoon,
    IssPassNow,
}

impl FakeCmd {
    pub fn from_key(c: char) -> Option<FakeCmd> {
        Some(match c {
            '1' => FakeCmd::PodStarted,
            '2' => FakeCmd::PodCrashed,
            '3' => FakeCmd::NodeDown,
            '4' => FakeCmd::NodeUp,
            '5' => FakeCmd::AlertToggle,
            '6' => FakeCmd::TorrentDone,
            '7' => FakeCmd::LinkDown,
            '8' => FakeCmd::LinkUp,
            't' => FakeCmd::ToggleTorrent,
            'n' => FakeCmd::ToggleNight,
            'b' => FakeCmd::Boot,
            '9' => FakeCmd::DegradeVolume,
            '0' => FakeCmd::HealVolume,
            'h' => FakeCmd::HotTemp,
            'p' => FakeCmd::PriceOutage,
            'u' => FakeCmd::UpsToggle,
            'd' => FakeCmd::AppToggle,
            'g' => FakeCmd::GithubPush,
            'r' => FakeCmd::GithubStar,
            'f' => FakeCmd::CiFailure,
            'l' => FakeCmd::Thunder,
            'w' => FakeCmd::RainSoon,
            'i' => FakeCmd::IssPassNow,
            _ => return None,
        })
    }
}

pub struct FakeState {
    rng: fastrand::Rng,
    cpu: f32,
    mem: f32,
    running: u32,
    pending: u32,
    failed: u32,
    nodes_total: u32,
    nodes_ready: u32,
    alerts: Vec<String>,
    torrents_on: bool,
    torrents: Vec<Torrent>,
    link_up: bool,
    night: Option<bool>,
    temps: Vec<(String, f32)>,
    hot_temp_on: bool,
    volumes: Vec<(String, Robustness)>,
    storage_used: u64,
    storage_cap: u64,
    mix: Vec<(Source, f32)>,
    carbon: f32,
    prices_ok: bool,
    /// Hours the price curve is rotated by, so the current-hour marker moves.
    price_rot: usize,
    /// Wall clock the sky, rain and ISS events are built against; advances with ticks.
    unix_start: i64,
    utc_offset: i32,
    weather_idx: usize,
    temp: f32,
    wind_from: f32,
    wind: f32,
    eaqi: f32,
    rain: Vec<f32>,
    ups_on_battery: bool,
    ups_charge: f32,
    net_rx: f64,
    net_tx: f64,
    apps: Vec<App>,
    gh_days: Vec<u32>,
    iss_start: i64,
    ticks: u64,
}

impl FakeState {
    pub fn new(seed: u64) -> Self {
        let now = chrono::Local::now();
        let mut s = Self {
            rng: fastrand::Rng::with_seed(seed),
            cpu: 42.0,
            mem: 67.0,
            running: 53,
            pending: 0,
            failed: 0,
            nodes_total: 4,
            nodes_ready: 4,
            alerts: Vec::new(),
            torrents_on: false,
            torrents: Vec::new(),
            link_up: true,
            night: None,
            temps: NODES.iter().map(|(n, c)| ((*n).to_string(), *c)).collect(),
            hot_temp_on: false,
            volumes: (1..=VOLUME_COUNT)
                .map(|i| (format!("pvc-{i:02}"), Robustness::Healthy))
                .collect(),
            storage_used: STORAGE_USED,
            storage_cap: STORAGE_CAP,
            mix: MIX_SEED.to_vec(),
            carbon: 214.0,
            prices_ok: true,
            price_rot: 0,
            unix_start: now.timestamp(),
            utc_offset: now.offset().local_minus_utc(),
            weather_idx: 1,
            temp: 18.0,
            wind_from: 232.0,
            wind: 19.0,
            eaqi: 32.0,
            rain: RAIN_SEED.to_vec(),
            ups_on_battery: false,
            ups_charge: 100.0,
            net_rx: 41e6,
            net_tx: 4e6,
            apps: APP_NAMES
                .iter()
                .map(|n| App {
                    name: (*n).to_string(),
                    sync: AppSync::Synced,
                    health: AppHealth::Healthy,
                    operating: false,
                })
                .collect(),
            gh_days: GH_SEED.to_vec(),
            iss_start: 0,
            ticks: 0,
        };
        s.iss_start = s.unix_start + ISS_PASS_IN_SECS;
        s
    }

    /// Pin the clock (the GIF generator and tests want reproducible skies).
    pub fn set_clock(&mut self, unix: i64, utc_offset_secs: i32) {
        self.unix_start = unix;
        self.utc_offset = utc_offset_secs;
        self.iss_start = unix + ISS_PASS_IN_SECS;
    }

    pub fn unix_now(&self) -> i64 {
        self.unix_start + self.ticks as i64
    }

    fn total(&self) -> u32 {
        self.running + self.pending + self.failed
    }

    fn pod_snapshot(&self) -> Event {
        Event::PodSnapshot {
            running: self.running,
            pending: self.pending,
            failed: self.failed,
            total: self.total(),
        }
    }

    fn node_snapshot(&self) -> Event {
        let not_ready = (self.nodes_ready..self.nodes_total)
            .map(|i| format!("node-{}", i + 1))
            .collect();
        Event::NodeSnapshot {
            ready: self.nodes_ready,
            total: self.nodes_total,
            not_ready,
        }
    }

    fn metrics(&self) -> Event {
        Event::Metrics {
            cpu_pct: self.cpu,
            mem_pct: self.mem,
            mem_used_gb: 16.0 * self.mem / 100.0,
            mem_total_gb: 16.0,
            hot_cpu: Some((".5".into(), self.cpu + 20.0)),
            hot_mem: Some((".7".into(), self.mem + 5.0)),
        }
    }

    fn node_temps(&self) -> Event {
        let mut list = self.temps.clone();
        if self.hot_temp_on {
            if let Some(t) = list.get_mut(HOT_NODE) {
                t.1 = HOT_C;
            }
        }
        Event::NodeTemps(list)
    }

    fn storage(&self) -> Event {
        Event::Storage {
            volumes: self.volumes.clone(),
            used_bytes: self.storage_used,
            capacity_bytes: self.storage_cap,
        }
    }

    /// A timestamp derived from the tick counter, so ticks stay reproducible.
    fn updated_at(&self) -> String {
        let h = self.ticks / 3600 % 24;
        let m = self.ticks / 60 % 60;
        format!("{PRICE_DATE}T{h:02}:{m:02}:00Z")
    }

    fn electricity(&self) -> Event {
        let total: f32 = self.mix.iter().map(|(_, mw)| *mw).sum::<f32>().max(1.0);
        let share = |f: fn(Source) -> bool| -> f32 {
            self.mix
                .iter()
                .filter(|(s, _)| f(*s))
                .map(|(_, mw)| *mw)
                .sum::<f32>()
                / total
                * 100.0
        };
        Event::Electricity {
            zone: ZONE.into(),
            mix_mw: self.mix.clone(),
            renewable_pct: share(is_renewable),
            fossil_free_pct: share(|s| is_renewable(s) || s == Source::Nuclear),
            carbon_gco2: self.carbon,
            updated_at: self.updated_at(),
        }
    }

    fn prices(&self) -> Event {
        let mut hourly = PRICE_CURVE.to_vec();
        hourly.rotate_left(self.price_rot % PRICE_CURVE.len());
        let eur = (0..96)
            .map(|i| {
                let h = i / 4;
                let a = hourly[h];
                let b = hourly[(h + 1) % 24];
                a + (b - a) * (i % 4) as f32 / 4.0
            })
            .collect();
        Event::Prices {
            date: PRICE_DATE.into(),
            eur_per_kwh: eur,
            avg_eur_per_kwh: PRICE_CURVE.iter().sum::<f32>() / PRICE_CURVE.len() as f32,
            currency: "EUR".into(),
        }
    }

    fn weather(&self) -> Event {
        Event::Weather {
            temp_c: self.temp,
            code: WEATHER_CYCLE[self.weather_idx % WEATHER_CYCLE.len()],
            is_day: true,
            wind_kmh: self.wind,
            gust_kmh: self.wind * 1.8,
            wind_from_deg: self.wind_from,
            at: self.updated_at(),
        }
    }

    fn air(&self) -> Event {
        Event::AirQuality { eaqi: self.eaqi }
    }

    fn rain(&self) -> Event {
        Event::Rain {
            from: self.unix_now(),
            mm_per_h: self.rain.clone(),
        }
    }

    fn sky(&self) -> Event {
        sky_event(FAKE_LAT, FAKE_LON, self.unix_now(), self.utc_offset)
    }

    fn iss(&self) -> Event {
        Event::IssPass(Some(IssPass {
            start: self.iss_start,
            end: self.iss_start + ISS_PASS_SECS,
            max_elevation_deg: 62.0,
            visible: true,
        }))
    }

    fn github(&self) -> Event {
        let today = chrono::DateTime::from_timestamp(self.unix_now() + self.utc_offset as i64, 0)
            .map(|d| d.date_naive())
            .unwrap_or_default();
        let days = self
            .gh_days
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let back = (self.gh_days.len() - 1 - i) as i64;
                (
                    (today - chrono::Duration::days(back))
                        .format("%Y-%m-%d")
                        .to_string(),
                    *n,
                )
            })
            .collect();
        Event::GithubActivity { days }
    }

    fn ups(&self) -> Event {
        Event::Ups {
            on_battery: self.ups_on_battery,
            low_battery: self.ups_charge <= 20.0,
            charge_pct: self.ups_charge,
            load_pct: 6.0,
            runtime_secs: (self.ups_charge / 100.0 * 3014.0) as u32,
        }
    }

    fn net(&self) -> Event {
        Event::Network {
            rx_bps: self.net_rx,
            tx_bps: self.net_tx,
        }
    }

    fn apps(&self) -> Event {
        Event::Apps(self.apps.clone())
    }

    pub fn initial(&self) -> Vec<Event> {
        vec![
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Prometheus,
                up: true,
            },
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Electricity,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Prices,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Weather,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Rain,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Github,
                up: true,
            },
            Event::Link {
                target: LinkTarget::ArgoCd,
                up: true,
            },
            self.metrics(),
            self.pod_snapshot(),
            self.node_snapshot(),
            Event::AlertSnapshot {
                firing: self.alerts.clone(),
            },
            Event::Torrents(self.torrents.clone()),
            self.node_temps(),
            self.storage(),
            self.electricity(),
            self.prices(),
            self.weather(),
            self.air(),
            self.rain(),
            self.sky(),
            self.iss(),
            self.github(),
            self.ups(),
            self.net(),
            self.apps(),
        ]
    }

    /// One second of simulated time.
    pub fn tick(&mut self) -> Vec<Event> {
        self.ticks += 1;
        if !self.link_up {
            return Vec::new();
        }
        self.cpu = (self.cpu + self.rng.f32() * 6.0 - 3.0).clamp(5.0, 98.0);
        self.mem = (self.mem + self.rng.f32() * 2.0 - 1.0).clamp(20.0, 95.0);
        for (_, c) in self.temps.iter_mut() {
            *c = (*c + self.rng.f32() * 0.6 - 0.3).clamp(30.0, 80.0);
        }
        let phase = (self.ticks % SOLAR_CYCLE) as f32 / (SOLAR_CYCLE as f32 / 2.0);
        let solar = (std::f32::consts::PI * phase).sin().max(0.0) * SOLAR_PEAK;
        for (s, mw) in self.mix.iter_mut() {
            *mw = (*mw * (1.0 + self.rng.f32() * 0.04 - 0.02)).max(0.0);
            if *s == Source::Solar {
                *mw = solar;
            }
        }
        self.carbon = (self.carbon + self.rng.f32() * 6.0 - 3.0).clamp(20.0, 600.0);
        self.net_rx = (self.net_rx * (1.0 + self.rng.f64() * 0.4 - 0.2)).clamp(2e6, 400e6);
        self.net_tx = (self.net_tx * (1.0 + self.rng.f64() * 0.4 - 0.2)).clamp(2e5, 40e6);
        self.temp = (self.temp + self.rng.f32() * 0.4 - 0.2).clamp(-5.0, 35.0);
        self.wind = (self.wind + self.rng.f32() * 2.0 - 1.0).clamp(3.0, 60.0);
        self.wind_from = (self.wind_from + self.rng.f32() * 6.0 - 3.0).rem_euclid(360.0);
        self.eaqi = (self.eaqi + self.rng.f32() * 2.0 - 1.0).clamp(5.0, 95.0);
        if self.ups_on_battery {
            self.ups_charge = (self.ups_charge - 0.5).max(0.0);
        } else {
            self.ups_charge = (self.ups_charge + 1.0).min(100.0);
        }
        let mut out = vec![self.metrics()];
        out.push(self.net());
        if self.pending > 0 && self.rng.f32() < 0.5 {
            self.pending -= 1;
            self.running += 1;
            out.push(Event::PodStarted {
                ns: "fake".into(),
                name: format!("pod-{}", self.ticks),
            });
            out.push(self.pod_snapshot());
        } else if self.ticks.is_multiple_of(20) {
            self.pending += 1;
            out.push(self.pod_snapshot());
        }
        if self.torrents_on {
            for (i, t) in self.torrents.iter_mut().enumerate() {
                t.progress = (t.progress + 0.4 / (i as f32 + 1.0)).min(99.0);
                t.eta_secs = (t.eta_secs - 1).max(1);
            }
            out.push(Event::Torrents(self.torrents.clone()));
        }
        if self.ticks.is_multiple_of(5) {
            out.push(self.node_snapshot());
            out.push(Event::AlertSnapshot {
                firing: self.alerts.clone(),
            });
            out.push(self.ups());
            self.rain.rotate_left(1);
            out.push(self.rain());
        }
        if self.ticks.is_multiple_of(10) {
            out.push(self.node_temps());
            out.push(self.storage());
            out.push(self.electricity());
            out.push(self.air());
        }
        if self.ticks.is_multiple_of(30) {
            self.weather_idx += 1;
            out.push(self.weather());
        }
        if self.ticks.is_multiple_of(60) {
            out.push(self.sky());
            out.push(self.iss());
            out.push(self.github());
        }
        if self.ticks.is_multiple_of(60) && self.prices_ok {
            self.price_rot += 1;
            out.push(self.prices());
        }
        out
    }

    pub fn command(&mut self, cmd: FakeCmd) -> Vec<Event> {
        match cmd {
            FakeCmd::PodStarted => {
                if self.failed > 0 {
                    self.failed -= 1;
                }
                self.running += 1;
                vec![
                    Event::PodStarted {
                        ns: "fake".into(),
                        name: "manual".into(),
                    },
                    self.pod_snapshot(),
                ]
            }
            FakeCmd::PodCrashed => {
                self.running = self.running.saturating_sub(1);
                self.failed += 1;
                vec![
                    Event::PodCrashed {
                        ns: "fake".into(),
                        name: "manual".into(),
                    },
                    self.pod_snapshot(),
                ]
            }
            FakeCmd::NodeDown => {
                if self.nodes_ready == 0 {
                    return Vec::new();
                }
                self.nodes_ready -= 1;
                let name = format!("node-{}", self.nodes_ready + 1);
                vec![
                    Event::NodeReady { name, ready: false },
                    self.node_snapshot(),
                ]
            }
            FakeCmd::NodeUp => {
                if self.nodes_ready == self.nodes_total {
                    return Vec::new();
                }
                self.nodes_ready += 1;
                let name = format!("node-{}", self.nodes_ready);
                vec![Event::NodeReady { name, ready: true }, self.node_snapshot()]
            }
            FakeCmd::AlertToggle => {
                if self.alerts.is_empty() {
                    self.alerts.push("HighLoad".into());
                    vec![
                        Event::AlertChanged {
                            name: "HighLoad".into(),
                            firing: true,
                        },
                        Event::AlertSnapshot {
                            firing: self.alerts.clone(),
                        },
                    ]
                } else {
                    self.alerts.clear();
                    vec![
                        Event::AlertChanged {
                            name: "HighLoad".into(),
                            firing: false,
                        },
                        Event::AlertSnapshot { firing: vec![] },
                    ]
                }
            }
            FakeCmd::TorrentDone => {
                if self.torrents.is_empty() {
                    return Vec::new();
                }
                let done = self.torrents.remove(0);
                if self.torrents.is_empty() {
                    self.torrents_on = false;
                }
                vec![
                    Event::TorrentDone { name: done.name },
                    Event::Torrents(self.torrents.clone()),
                ]
            }
            FakeCmd::LinkDown => {
                self.link_up = false;
                vec![Event::Link {
                    target: LinkTarget::K8sApi,
                    up: false,
                }]
            }
            FakeCmd::LinkUp => {
                self.link_up = true;
                vec![Event::Link {
                    target: LinkTarget::K8sApi,
                    up: true,
                }]
            }
            FakeCmd::ToggleTorrent => {
                self.torrents_on = !self.torrents_on;
                if self.torrents_on {
                    self.torrents = vec![
                        Torrent {
                            name: "ubuntu-24.04.iso".into(),
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
                        Torrent {
                            name: "fedora-44.iso".into(),
                            progress: 12.0,
                            eta_secs: 9000,
                            speed_bps: 1_000_000,
                        },
                    ];
                    vec![
                        Event::TorrentAdded {
                            name: "ubuntu-24.04.iso".into(),
                        },
                        Event::Torrents(self.torrents.clone()),
                    ]
                } else {
                    self.torrents.clear();
                    vec![Event::Torrents(vec![])]
                }
            }
            FakeCmd::ToggleNight => {
                self.night = match self.night {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                vec![Event::ForceNight(self.night)]
            }
            FakeCmd::Boot => vec![Event::Boot],
            FakeCmd::DegradeVolume => {
                let Some(v) = self
                    .volumes
                    .iter_mut()
                    .find(|(_, r)| *r == Robustness::Healthy)
                else {
                    return Vec::new();
                };
                v.1 = Robustness::Degraded;
                let name = v.0.clone();
                vec![
                    Event::VolumeDegraded {
                        name,
                        robustness: Robustness::Degraded,
                    },
                    self.storage(),
                ]
            }
            FakeCmd::HealVolume => {
                let Some(v) = self
                    .volumes
                    .iter_mut()
                    .find(|(_, r)| *r != Robustness::Healthy)
                else {
                    return Vec::new();
                };
                v.1 = Robustness::Healthy;
                let name = v.0.clone();
                vec![Event::VolumeHealthy { name }, self.storage()]
            }
            FakeCmd::HotTemp => {
                self.hot_temp_on = !self.hot_temp_on;
                vec![self.node_temps()]
            }
            FakeCmd::PriceOutage => {
                self.prices_ok = !self.prices_ok;
                vec![Event::Link {
                    target: LinkTarget::Prices,
                    up: self.prices_ok,
                }]
            }
            FakeCmd::UpsToggle => {
                self.ups_on_battery = !self.ups_on_battery;
                vec![
                    if self.ups_on_battery {
                        Event::UpsOnBattery
                    } else {
                        Event::UpsOnline
                    },
                    self.ups(),
                ]
            }
            FakeCmd::AppToggle => {
                let app = &mut self.apps[DEGRADED_APP];
                let name = app.name.clone();
                let edge = if app.health == AppHealth::Healthy {
                    app.health = AppHealth::Degraded;
                    Event::AppDegraded { name }
                } else {
                    app.health = AppHealth::Healthy;
                    Event::AppHealthy { name }
                };
                vec![edge, self.apps()]
            }
            FakeCmd::GithubPush => vec![Event::GithubPush {
                repo: "silkepilon/RackScreen".into(),
                commits: 3,
            }],
            FakeCmd::GithubStar => vec![Event::GithubStar {
                repo: "silkepilon/RackScreen".into(),
            }],
            FakeCmd::CiFailure => vec![Event::GithubRun {
                repo: "silkepilon/RackScreen".into(),
                ok: false,
            }],
            FakeCmd::Thunder => {
                self.weather_idx = 3; // WEATHER_CYCLE[3] is thunder
                vec![self.weather()]
            }
            FakeCmd::RainSoon => {
                self.rain = vec![0.0; 24];
                for (i, v) in [(2, 1.5), (3, 3.0), (4, 4.0), (5, 2.0), (6, 0.5)] {
                    self.rain[i] = v;
                }
                vec![self.rain()]
            }
            FakeCmd::IssPassNow => {
                self.iss_start = self.unix_now();
                vec![self.iss()]
            }
        }
    }
}

pub async fn run_fake(ctx: SourceCtx, cmds: Receiver<FakeCmd>, seed: u64) {
    let mut st = FakeState::new(seed);
    ctx.emit_all(st.initial());
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = ticker.tick() => ctx.emit_all(st.tick()),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                while let Ok(cmd) = cmds.try_recv() {
                    ctx.emit_all(st.command(cmd));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map() {
        assert_eq!(FakeCmd::from_key('1'), Some(FakeCmd::PodStarted));
        assert_eq!(FakeCmd::from_key('n'), Some(FakeCmd::ToggleNight));
        assert_eq!(FakeCmd::from_key('9'), Some(FakeCmd::DegradeVolume));
        assert_eq!(FakeCmd::from_key('0'), Some(FakeCmd::HealVolume));
        assert_eq!(FakeCmd::from_key('h'), Some(FakeCmd::HotTemp));
        assert_eq!(FakeCmd::from_key('p'), Some(FakeCmd::PriceOutage));
        assert_eq!(FakeCmd::from_key('u'), Some(FakeCmd::UpsToggle));
        assert_eq!(FakeCmd::from_key('d'), Some(FakeCmd::AppToggle));
        assert_eq!(FakeCmd::from_key('g'), Some(FakeCmd::GithubPush));
        assert_eq!(FakeCmd::from_key('r'), Some(FakeCmd::GithubStar));
        assert_eq!(FakeCmd::from_key('f'), Some(FakeCmd::CiFailure));
        assert_eq!(FakeCmd::from_key('l'), Some(FakeCmd::Thunder));
        assert_eq!(FakeCmd::from_key('w'), Some(FakeCmd::RainSoon));
        assert_eq!(FakeCmd::from_key('i'), Some(FakeCmd::IssPassNow));
        assert_eq!(FakeCmd::from_key('x'), None);
    }

    #[test]
    fn initial_has_every_new_role_fed() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.initial();
        for target in [
            LinkTarget::Weather,
            LinkTarget::Rain,
            LinkTarget::Github,
            LinkTarget::ArgoCd,
        ] {
            assert!(
                evs.iter()
                    .any(|e| matches!(e, Event::Link { target: t, up: true } if *t == target)),
                "{target:?} link up missing"
            );
        }
        assert!(evs.iter().any(|e| matches!(e, Event::Weather { .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::AirQuality { .. })));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Rain { mm_per_h, .. } if mm_per_h.len() == 24)));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Sky {
                sunrise: Some(_),
                ..
            }
        )));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::IssPass(Some(p)) if p.start == 1_788_782_400 + 42 * 60 && p.visible
        )));
        assert!(evs.iter().any(
            |e| matches!(e, Event::GithubActivity { days } if days.len() == 30 && days[29].1 == 28)
        ));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Ups {
                on_battery: false,
                ..
            }
        )));
        assert!(evs.iter().any(|e| matches!(e, Event::Network { .. })));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Apps(a) if a.len() == 16)));
    }

    #[test]
    fn new_commands_emit_their_edges() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.command(FakeCmd::UpsToggle);
        assert!(evs.contains(&Event::UpsOnBattery));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Ups {
                on_battery: true,
                ..
            }
        )));
        assert!(s.command(FakeCmd::UpsToggle).contains(&Event::UpsOnline));
        let evs = s.command(FakeCmd::AppToggle);
        assert!(evs.iter().any(|e| matches!(e, Event::AppDegraded { .. })));
        assert!(s
            .command(FakeCmd::AppToggle)
            .iter()
            .any(|e| matches!(e, Event::AppHealthy { .. })));
        assert!(matches!(
            s.command(FakeCmd::GithubPush).as_slice(),
            [Event::GithubPush { commits: 3, .. }]
        ));
        assert!(matches!(
            s.command(FakeCmd::GithubStar).as_slice(),
            [Event::GithubStar { .. }]
        ));
        assert!(matches!(
            s.command(FakeCmd::CiFailure).as_slice(),
            [Event::GithubRun { ok: false, .. }]
        ));
        assert!(matches!(
            s.command(FakeCmd::Thunder).as_slice(),
            [Event::Weather { code: 95, .. }]
        ));
        assert!(matches!(
            s.command(FakeCmd::RainSoon).as_slice(),
            [Event::Rain { mm_per_h, .. }] if mm_per_h[2] > 0.1 && mm_per_h[0] == 0.0
        ));
        assert!(matches!(
            s.command(FakeCmd::IssPassNow).as_slice(),
            [Event::IssPass(Some(p))] if p.start == s.unix_now()
        ));
    }

    #[test]
    fn initial_brings_links_up() {
        let s = FakeState::new(1);
        let evs = s.initial();
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true
            }
        )));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::PodSnapshot { running: 53, .. })));
    }

    #[test]
    fn initial_has_temps_storage_and_electricity() {
        let s = FakeState::new(1);
        let evs = s.initial();
        for target in [LinkTarget::Electricity, LinkTarget::Prices] {
            assert!(
                evs.iter()
                    .any(|e| matches!(e, Event::Link { target: t, up: true } if *t == target)),
                "{target:?} link up missing"
            );
        }
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::NodeTemps(t) if t.len() == 7)),
            "seven node temperatures"
        );
        assert!(
            evs.iter().any(|e| matches!(
                e,
                Event::Storage {
                    volumes,
                    used_bytes,
                    capacity_bytes,
                } if volumes.len() == 21
                    && *used_bytes == 49 * 1024 * 1024 * 1024
                    && *capacity_bytes == 128 * 1024 * 1024 * 1024
            )),
            "21 volumes and 49/128 GiB"
        );
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Electricity { zone, mix_mw, renewable_pct, .. }
                if zone == "NL" && mix_mw.len() == 9 && *renewable_pct > 50.0
        )));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Prices { eur_per_kwh, avg_eur_per_kwh, currency, .. }
                if eur_per_kwh.len() == 96 && eur_per_kwh[0] == 0.22 && *avg_eur_per_kwh > 0.17 && *avg_eur_per_kwh < 0.19 && currency == "EUR"
        )));
    }

    #[test]
    fn tick_emits_temps_storage_electricity_and_prices() {
        let mut s = FakeState::new(3);
        let ten: Vec<Event> = (0..10).flat_map(|_| s.tick()).collect();
        assert!(ten.iter().any(|e| matches!(e, Event::NodeTemps(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Storage { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Electricity { .. })));
        assert!(!ten.iter().any(|e| matches!(e, Event::Prices { .. })));
        let rest: Vec<Event> = (10..60).flat_map(|_| s.tick()).collect();
        let rotated = rest
            .iter()
            .find_map(|e| match e {
                Event::Prices { eur_per_kwh, .. } => Some(eur_per_kwh.clone()),
                _ => None,
            })
            .expect("prices at tick 60");
        assert_eq!(rotated[0], 0.19, "curve rotated by one hour");
        assert!(
            (rotated[2] - 0.18).abs() < 1e-6,
            "quarter-hours interpolate toward the next hour"
        );
    }

    #[test]
    fn degrade_then_heal_round_trip() {
        let mut s = FakeState::new(1);
        let evs = s.command(FakeCmd::DegradeVolume);
        assert_eq!(
            evs[0],
            Event::VolumeDegraded {
                name: "pvc-01".into(),
                robustness: Robustness::Degraded,
            }
        );
        assert!(matches!(
            &evs[1],
            Event::Storage { volumes, .. }
                if volumes[0] == ("pvc-01".to_string(), Robustness::Degraded)
                    && volumes[1].1 == Robustness::Healthy
        ));
        let evs = s.command(FakeCmd::HealVolume);
        assert_eq!(
            evs[0],
            Event::VolumeHealthy {
                name: "pvc-01".into()
            }
        );
        assert!(matches!(
            &evs[1],
            Event::Storage { volumes, .. }
                if volumes.iter().all(|(_, r)| *r == Robustness::Healthy)
        ));
        assert!(s.command(FakeCmd::HealVolume).is_empty(), "all healthy");
    }

    #[test]
    fn hot_temp_toggles_a_node_over_the_threshold() {
        let mut s = FakeState::new(1);
        let hottest = |evs: &[Event]| match &evs[0] {
            Event::NodeTemps(t) => t.iter().map(|(_, c)| *c).fold(0.0_f32, f32::max),
            other => panic!("expected NodeTemps, got {other:?}"),
        };
        let evs = s.command(FakeCmd::HotTemp);
        assert!(hottest(&evs) >= 70.0, "hot node is hot");
        let ten: Vec<Event> = (0..10).flat_map(|_| s.tick()).collect();
        let held = ten
            .iter()
            .find_map(|e| match e {
                Event::NodeTemps(t) => Some(t.clone()),
                _ => None,
            })
            .expect("temps at tick 10");
        assert!(
            held.iter().any(|(_, c)| *c >= 70.0),
            "still hot while the flag is on"
        );
        let evs = s.command(FakeCmd::HotTemp);
        assert!(hottest(&evs) < 70.0, "back to normal");
    }

    #[test]
    fn price_outage_toggles_link_and_silences_prices() {
        let mut s = FakeState::new(1);
        assert_eq!(
            s.command(FakeCmd::PriceOutage),
            vec![Event::Link {
                target: LinkTarget::Prices,
                up: false
            }]
        );
        let out: Vec<Event> = (0..120).flat_map(|_| s.tick()).collect();
        assert!(!out.iter().any(|e| matches!(e, Event::Prices { .. })));
        assert_eq!(
            s.command(FakeCmd::PriceOutage),
            vec![Event::Link {
                target: LinkTarget::Prices,
                up: true
            }]
        );
        let out: Vec<Event> = (0..60).flat_map(|_| s.tick()).collect();
        assert!(out.iter().any(|e| matches!(e, Event::Prices { .. })));
    }

    #[test]
    fn tick_is_deterministic_and_emits_metrics() {
        let mut a = FakeState::new(7);
        let mut b = FakeState::new(7);
        a.set_clock(1_788_782_400, 7200);
        b.set_clock(1_788_782_400, 7200);
        for _ in 0..30 {
            assert_eq!(a.tick(), b.tick());
        }
        assert!(matches!(a.tick()[0], Event::Metrics { .. }));
    }

    #[test]
    fn crash_and_recover() {
        let mut s = FakeState::new(1);
        let evs = s.command(FakeCmd::PodCrashed);
        assert!(matches!(evs[0], Event::PodCrashed { .. }));
        assert!(matches!(
            evs[1],
            Event::PodSnapshot {
                failed: 1,
                running: 52,
                ..
            }
        ));
        let evs = s.command(FakeCmd::PodStarted);
        assert!(matches!(
            evs[1],
            Event::PodSnapshot {
                failed: 0,
                running: 53,
                ..
            }
        ));
    }

    #[test]
    fn node_down_up_and_bounds() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::NodeUp).is_empty());
        let evs = s.command(FakeCmd::NodeDown);
        assert!(matches!(&evs[0], Event::NodeReady { ready: false, .. }));
        assert!(matches!(
            &evs[1],
            Event::NodeSnapshot {
                ready: 3,
                total: 4,
                ..
            }
        ));
        let evs = s.command(FakeCmd::NodeUp);
        assert!(matches!(&evs[1], Event::NodeSnapshot { ready: 4, .. }));
    }

    #[test]
    fn torrent_toggle_and_done() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::TorrentDone).is_empty());
        let evs = s.command(FakeCmd::ToggleTorrent);
        assert!(matches!(evs[0], Event::TorrentAdded { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 3));
        let evs = s.command(FakeCmd::TorrentDone);
        assert!(matches!(evs[0], Event::TorrentDone { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 2));
    }

    #[test]
    fn link_down_silences_ticks() {
        let mut s = FakeState::new(1);
        s.command(FakeCmd::LinkDown);
        assert!(s.tick().is_empty());
        s.command(FakeCmd::LinkUp);
        assert!(!s.tick().is_empty());
    }

    #[test]
    fn night_cycles() {
        let mut s = FakeState::new(1);
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(Some(true))]
        );
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(Some(false))]
        );
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(None)]
        );
    }
}

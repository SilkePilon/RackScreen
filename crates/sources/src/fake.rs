//! Scripted data for the simulator. Drifts metrics, churns pods, and reacts to keyboard commands.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use rackscreen_core::electricity::Source;
use rackscreen_core::event::{
    App, AppHealth, AppSync, CiRun, CiStatus, CoverRef, Download, Event, IssPass, JfKind,
    JfSession, JfStatus, LinkTarget, MusicSource, NodeLoad, Robustness, Torrent, Track,
};
use rackscreen_core::music::demo_cover;
use rackscreen_core::theme::Color;

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

/// The demo day the Electricity Maps timestamps fall on.
const DEMO_DATE: &str = "2026-09-06";

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
/// Media library counts and the demo download.
const MEDIA_LIBRARY: (u32, u32, u32) = (37, 30, 54);
const DOWNLOAD_TITLE: &str = "The Lord of the Rings: The Return of the King";
/// Restarts per hour over the last day, oldest first.
const RESTART_SEED: [f32; 24] = [
    0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 7.0, 9.0, 11.0, 6.0, 3.0, 1.0, 0.0, 0.0, 0.0, 1.0,
    0.0, 0.0, 0.0, 0.0, 0.0,
];
const WORST_POD: &str = "n8n-worker";
/// Days up per node (same order as `NODES`); the two short ones rebooted lately.
const UPTIME_DAYS: [f64; 7] = [79.6, 79.6, 79.6, 82.1, 41.0, 0.375, 4.4];
/// Fan power in percent for the four Pis (`NODES[3..]`).
const FAN_SEED: [f32; 4] = [45.0, 41.0, 54.0, 49.0];
const MAINS_V: f32 = 234.0;
const BROWNOUT_V: f32 = 212.0;
/// 1-minute load and cores per node.
const LOAD_SEED: [(f32, u32); 7] = [
    (0.81, 8),
    (0.02, 12),
    (0.12, 16),
    (0.22, 4),
    (0.13, 4),
    (3.1, 4),
    (1.02, 4),
];
const CI_REPO: &str = "silkepilon/RackScreen";

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
    DegradeVolume,
    HealVolume,
    HotTemp,
    UpsToggle,
    AppToggle,
    GithubPush,
    GithubStar,
    CiFailure,
    Thunder,
    RainSoon,
    IssPassNow,
    MediaDone,
    KickRestart,
    Brownout,
    JellyfinToggle,
    CiPass,
    /// Music: Spotify playing, paused, Jellyfin playing, idle, round again.
    MusicToggle,
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
            '9' => FakeCmd::DegradeVolume,
            '0' => FakeCmd::HealVolume,
            'h' => FakeCmd::HotTemp,
            'u' => FakeCmd::UpsToggle,
            'd' => FakeCmd::AppToggle,
            'g' => FakeCmd::GithubPush,
            'r' => FakeCmd::GithubStar,
            'f' => FakeCmd::CiFailure,
            'l' => FakeCmd::Thunder,
            'w' => FakeCmd::RainSoon,
            'i' => FakeCmd::IssPassNow,
            'm' => FakeCmd::MediaDone,
            'k' => FakeCmd::KickRestart,
            'b' => FakeCmd::Brownout,
            'j' => FakeCmd::JellyfinToggle,
            'c' => FakeCmd::CiPass,
            's' => FakeCmd::MusicToggle,
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
    /// Download progress 0..1, `None` between downloads; `dl_restart_at` is the tick a new one starts.
    dl_progress: Option<f32>,
    dl_restart_at: u64,
    traffic_rps: f32,
    restart_hours: Vec<f32>,
    pub(crate) restart_total: u32,
    uptime_days: Vec<f64>,
    fans: Vec<f32>,
    brownout: bool,
    loads: Vec<f32>,
    /// Newest first, at most 12.
    ci_runs: Vec<CiStatus>,
    /// 0 playing, 1 paused, 2 idle.
    jf_state: u8,
    jf_pos: f64,
    /// 0 Spotify playing, 1 Spotify paused, 2 Jellyfin playing, 3 idle.
    music_state: u8,
    music_pos: f64,
    /// When the music state last changed (unix seconds).
    music_since: i64,
    covers: [CoverRef; 2],
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
            dl_progress: Some(0.35),
            dl_restart_at: 0,
            traffic_rps: 3.4,
            restart_hours: RESTART_SEED.to_vec(),
            restart_total: RESTART_SEED.iter().sum::<f32>() as u32,
            uptime_days: UPTIME_DAYS.to_vec(),
            fans: FAN_SEED.to_vec(),
            brownout: false,
            loads: LOAD_SEED.iter().map(|(l, _)| *l).collect(),
            ci_runs: (0..12)
                .map(|i| match i {
                    0 => CiStatus::InProgress,
                    7 => CiStatus::Failure,
                    _ => CiStatus::Success,
                })
                .collect(),
            jf_state: 0,
            jf_pos: 3720.0,
            music_state: 0,
            music_pos: 134.0,
            music_since: now.timestamp() - 134,
            covers: [
                CoverRef::new(demo_cover(
                    "fake-sp",
                    Color::hex(0x2B3A67),
                    Color::hex(0xE8A15A),
                )),
                CoverRef::new(demo_cover(
                    "fake-jf",
                    Color::hex(0x5B3F8C),
                    Color::hex(0x3FA7A0),
                )),
            ],
        };
        s.iss_start = s.unix_start + ISS_PASS_IN_SECS;
        s
    }

    /// Pin the clock (the GIF generator and tests want reproducible skies).
    pub fn set_clock(&mut self, unix: i64, utc_offset_secs: i32) {
        self.music_since = unix - 134;
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
        format!("{DEMO_DATE}T{h:02}:{m:02}:00Z")
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
            updated_at: self.updated_at(),
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

    /// A plausible last minute, so the simulator's sparklines start full.
    fn metrics_history(&self) -> Event {
        let wave = |i: usize, base: f32, amp: f32, k: f32| base + amp * ((i as f32) * k).sin();
        Event::MetricsHistory {
            cpu: (0..31).map(|i| wave(i, self.cpu, 8.0, 0.45)).collect(),
            mem: (0..31).map(|i| wave(i, self.mem, 1.5, 0.3)).collect(),
            net_rx_mbps: (0..31)
                .map(|i| wave(i, self.net_rx as f32 / 1e6, 12.0, 0.5).max(0.0))
                .collect(),
        }
    }

    /// Twelve hours that cool off after a shower in the afternoon.
    fn forecast(&self) -> Event {
        let hourly_temp_c: Vec<f32> = (0..12)
            .map(|i| self.temp + 3.0 * ((i as f32 + 2.0) / 12.0 * std::f32::consts::PI).sin() - 1.0)
            .collect();
        let hourly_precip_mm: Vec<f32> = (0..12)
            .map(|i| if (5..8).contains(&i) { 0.8 } else { 0.0 })
            .collect();
        Event::Forecast {
            day_min_c: hourly_temp_c.iter().cloned().fold(f32::MAX, f32::min) - 2.0,
            day_max_c: hourly_temp_c.iter().cloned().fold(f32::MIN, f32::max) + 1.0,
            hourly_temp_c,
            hourly_precip_mm,
        }
    }

    fn air_history(&self) -> Event {
        Event::AirQualityHistory {
            hourly: (0..24)
                .map(|i| {
                    (self.eaqi + 10.0 * ((i as f32) / 24.0 * std::f32::consts::TAU * 1.5).sin())
                        .max(1.0)
                })
                .collect(),
        }
    }

    fn volume_sizes(&self) -> Event {
        let n = self.volumes.len().max(1) as u64;
        Event::VolumeSizes(
            self.volumes
                .iter()
                .enumerate()
                .map(|(i, (name, _))| {
                    (
                        name.clone(),
                        self.storage_used * (n - i as u64) * 2 / (n * (n + 1)),
                    )
                })
                .collect(),
        )
    }

    /// Yesterday evening and this morning, on the fake clock.
    fn app_sync_history(&self) -> Event {
        let now = self.unix_now();
        Event::AppSyncHistory {
            finished_at: vec![
                now - 5 * 3600,
                now - 3 * 3600,
                now - 3 * 3600 + 600,
                now - 20 * 3600,
            ],
        }
    }

    fn media_library(&self) -> Event {
        Event::MediaLibrary {
            movies: MEDIA_LIBRARY.0,
            series: MEDIA_LIBRARY.1,
            artists: MEDIA_LIBRARY.2,
        }
    }

    fn download(&self) -> Event {
        Event::Download(self.dl_progress.map(|p| Download {
            title: DOWNLOAD_TITLE.into(),
            progress: p,
            eta_secs: ((1.0 - p) * 1800.0) as i64,
        }))
    }

    fn traffic(&self) -> Event {
        let total = self.traffic_rps.max(0.1);
        Event::Traffic {
            req_per_sec: self.traffic_rps,
            top: vec![
                ("n8n".into(), total * 0.48),
                ("portfolio".into(), total * 0.27),
                ("grafana".into(), total * 0.15),
            ],
            service_rps: total,
            error_share: 0.0,
        }
    }

    fn traffic_history(&self) -> Event {
        Event::TrafficHistory {
            req_per_sec: (0..31)
                .map(|i| (self.traffic_rps + 1.6 * ((i as f32) * 0.4).sin()).max(0.0))
                .collect(),
        }
    }

    fn restarts(&self) -> Event {
        let worst_n = self.restart_hours.iter().cloned().fold(0.0_f32, f32::max) as u32 + 22;
        Event::Restarts {
            total_24h: self.restart_total,
            worst: (self.restart_total > 0)
                .then(|| (WORST_POD.to_string(), worst_n.min(self.restart_total))),
        }
    }

    fn restarts_history(&self) -> Event {
        let hour = self.unix_now().div_euclid(3600);
        Event::RestartsHistory {
            hourly: self
                .restart_hours
                .iter()
                .enumerate()
                .map(|(i, v)| (hour - 23 + i as i64, *v))
                .collect(),
        }
    }

    fn uptime(&self) -> Event {
        Event::Uptime(
            NODES
                .iter()
                .zip(&self.uptime_days)
                .map(|((n, _), d)| ((*n).to_string(), d * 86_400.0 + self.ticks as f64))
                .collect(),
        )
    }

    fn fans(&self) -> Event {
        Event::Fans(
            NODES[3..]
                .iter()
                .zip(&self.fans)
                .map(|((n, _), pct)| ((*n).to_string(), *pct))
                .collect(),
        )
    }

    fn mains(&self) -> Event {
        Event::Mains {
            volts: if self.brownout { BROWNOUT_V } else { MAINS_V },
            ups_load_pct: 7.0,
        }
    }

    fn mains_history(&self) -> Event {
        let hour = self.unix_now().div_euclid(3600);
        Event::MainsHistory {
            hourly: (0..24)
                .map(|i| {
                    (
                        hour - 23 + i,
                        MAINS_V - 1.0 + 2.0 * ((i as f32) / 3.0).sin(),
                    )
                })
                .collect(),
        }
    }

    fn load(&self) -> Event {
        Event::Load(
            NODES
                .iter()
                .zip(LOAD_SEED.iter().zip(&self.loads))
                .map(|((n, _), ((_, cores), l1))| NodeLoad {
                    node: (*n).to_string(),
                    load1: *l1,
                    load5: l1 * 0.85,
                    load15: l1 * 0.7,
                    cores: *cores,
                })
                .collect(),
        )
    }

    fn ci_runs(&self) -> Event {
        let now = self.unix_now();
        Event::CiRuns(
            self.ci_runs
                .iter()
                .enumerate()
                .map(|(i, status)| {
                    let created_at = now - (i as i64 + 1) * 1800;
                    let duration = (*status != CiStatus::InProgress).then_some(120 + i as i64 * 3);
                    CiRun {
                        id: 500 - i as u64,
                        repo: CI_REPO.into(),
                        status: *status,
                        created_at,
                        updated_at: created_at + duration.unwrap_or(0),
                        duration_secs: duration,
                    }
                })
                .collect(),
        )
    }

    fn jellyfin(&self) -> Event {
        let sessions = match self.jf_state {
            2 => Vec::new(),
            st => vec![
                JfSession {
                    title: "Blade Runner 2049".into(),
                    kind: JfKind::Movie,
                    series: None,
                    season: None,
                    episode: None,
                    year: Some(2017),
                    runtime_secs: 9840.0,
                    position_secs: self.jf_pos,
                    paused: st == 1,
                    transcoding: false,
                    user: "silke".into(),
                    device: "Living room TV".into(),
                    last_activity: "2026-09-07T12:00:05.000Z".into(),
                },
                JfSession {
                    title: "Home".into(),
                    kind: JfKind::Episode,
                    series: Some("The Expanse".into()),
                    season: Some(2),
                    episode: Some(5),
                    year: None,
                    runtime_secs: 2640.0,
                    position_secs: self.jf_pos + 2000.0,
                    paused: true,
                    transcoding: true,
                    user: "sam".into(),
                    device: "iPad".into(),
                    last_activity: "2026-09-07T11:50:02.000Z".into(),
                },
            ],
        };
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions,
        }
    }

    /// One poll of both music sources; no random draws.
    fn music(&self) -> Vec<Event> {
        let (title, artist, device, duration, source) = match self.music_state {
            0 | 1 => (
                "Holocene",
                "Bon Iver",
                "Kitchen speaker",
                337.0,
                MusicSource::Spotify,
            ),
            _ => (
                "Teardrop",
                "Massive Attack",
                "silke · Living room",
                331.0,
                MusicSource::Jellyfin,
            ),
        };
        let track = (self.music_state < 3).then(|| Track {
            source,
            title: title.into(),
            artist: artist.into(),
            device: device.into(),
            duration_secs: duration,
            position_secs: self.music_pos.min(duration),
            playing: self.music_state != 1,
            since: self.music_since,
            cover: Some(self.covers[source.index()].clone()),
        });
        [MusicSource::Spotify, MusicSource::Jellyfin]
            .into_iter()
            .map(|s| Event::Music {
                source: s,
                track: track.clone().filter(|t| t.source == s),
            })
            .collect()
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
            self.metrics_history(),
            self.pod_snapshot(),
            self.node_snapshot(),
            Event::AlertSnapshot {
                firing: self.alerts.clone(),
            },
            Event::Torrents(self.torrents.clone()),
            self.node_temps(),
            self.storage(),
            self.volume_sizes(),
            self.electricity(),
            self.weather(),
            self.forecast(),
            self.air(),
            self.air_history(),
            self.rain(),
            self.sky(),
            self.iss(),
            self.github(),
            self.ups(),
            self.net(),
            self.apps(),
            self.app_sync_history(),
            self.media_library(),
            self.download(),
            self.traffic_history(),
            self.traffic(),
            self.restarts_history(),
            self.restarts(),
            self.uptime(),
            self.fans(),
            self.mains_history(),
            self.mains(),
            self.load(),
            self.ci_runs(),
            self.jellyfin(),
            self.printer(),
            self.home_assistant(),
        ]
        .into_iter()
        .chain(self.music())
        .collect()
    }

    /// A print that runs for ten minutes, finishes and starts again. Built
    /// from the tick count alone: no draws, so the seeded stream stays put.
    fn printer(&self) -> Event {
        use rackscreen_core::event::{PrintStage, PrinterReport, PrinterStatus};
        let t = self.ticks % 660;
        let pct = (t as f32 / 6.0).min(100.0);
        let stage = match t {
            0..=14 => PrintStage::Preparing,
            600.. => PrintStage::Finished,
            _ => PrintStage::Printing,
        };
        Event::Printer {
            status: PrinterStatus::Ok,
            report: Some(PrinterReport {
                stage,
                percent: pct,
                remaining_mins: Some(((600u64.saturating_sub(t)) / 60 * 7) as u32),
                layer: Some((pct * 3.0) as u32),
                total_layers: Some(300),
                nozzle_c: Some(220.0),
                bed_c: Some(60.0),
                job: "Articulated dragon".into(),
                filament: Some((rackscreen_core::theme::Color::hex(0xFF6A13), "PLA".into())),
            }),
        }
    }

    /// `ha:<first entity>`: a living-room temperature drifting slowly.
    fn home_assistant(&self) -> Event {
        let v = 21.0 + (self.ticks as f32 / 90.0).sin() * 1.2;
        Event::HomeAssistant {
            index: 0,
            status: rackscreen_core::event::HaStatus::Ok,
            entity: Some(rackscreen_core::event::HaEntity {
                state: format!("{v:.1}"),
                name: "Living room".into(),
                unit: Some("°C".into()),
                device_class: Some("temperature".into()),
                domain: "sensor".into(),
                last_changed: Some(self.unix_start + self.ticks as i64),
            }),
        }
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
        // the draw that drifted the removed carbon intensity; kept so the
        // seeded demo stream, and the gifs built from it, stay as they were
        self.rng.f32();
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
        self.traffic_rps = (self.traffic_rps + self.rng.f32() * 0.8 - 0.4).clamp(0.5, 9.0);
        out.push(self.traffic());
        if let Some(p) = self.dl_progress.as_mut() {
            *p = (*p + 0.004).min(0.99);
        } else if self.ticks >= self.dl_restart_at {
            self.dl_progress = Some(0.05);
        }
        out.push(self.download());
        if self.jf_state == 0 {
            self.jf_pos = (self.jf_pos + 1.0).min(9839.0);
        }
        if matches!(self.music_state, 0 | 2) {
            self.music_pos = (self.music_pos + 1.0) % 337.0;
        }
        if self.ticks.is_multiple_of(10) {
            out.push(self.printer());
            out.push(self.home_assistant());
        }
        if self.ticks.is_multiple_of(2) {
            out.push(self.jellyfin());
            out.extend(self.music());
        }
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
            for l in self.loads.iter_mut() {
                *l = (*l + self.rng.f32() * 0.1 - 0.05).max(0.01);
            }
            for f in self.fans.iter_mut() {
                *f = (*f + self.rng.f32() * 2.0 - 1.0).clamp(35.0, 60.0);
            }
            out.push(self.load());
            out.push(self.fans());
            out.push(self.mains());
            out.push(self.uptime());
        }
        if self.ticks.is_multiple_of(10) {
            out.push(self.node_temps());
            out.push(self.storage());
            out.push(self.volume_sizes());
            out.push(self.electricity());
            out.push(self.air());
            out.push(self.air_history());
            out.push(self.media_library());
            out.push(self.restarts());
        }
        if self.ticks.is_multiple_of(30) {
            self.weather_idx += 1;
            out.push(self.weather());
            out.push(self.forecast());
            out.push(self.ci_runs());
        }
        if self.ticks.is_multiple_of(60) {
            out.push(self.sky());
            out.push(self.iss());
            out.push(self.github());
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
            FakeCmd::CiFailure => {
                self.ci_runs.insert(0, CiStatus::Failure);
                self.ci_runs.truncate(12);
                vec![
                    Event::GithubRun {
                        repo: CI_REPO.into(),
                        ok: false,
                    },
                    self.ci_runs(),
                ]
            }
            FakeCmd::MediaDone => {
                self.dl_progress = None;
                self.dl_restart_at = self.ticks + 45;
                vec![
                    Event::MediaAdded {
                        title: DOWNLOAD_TITLE.into(),
                    },
                    self.download(),
                ]
            }
            FakeCmd::KickRestart => {
                self.restart_hours[23] += 6.0;
                self.restart_total += 6;
                vec![self.restarts(), self.restarts_history()]
            }
            FakeCmd::Brownout => {
                self.brownout = !self.brownout;
                vec![self.mains()]
            }
            FakeCmd::JellyfinToggle => {
                self.jf_state = (self.jf_state + 1) % 3;
                vec![self.jellyfin()]
            }
            FakeCmd::MusicToggle => {
                self.music_state = (self.music_state + 1) % 4;
                self.music_since = self.unix_now();
                if self.music_state == 2 {
                    self.music_pos = 40.0;
                }
                self.music()
            }
            FakeCmd::CiPass => {
                self.ci_runs.insert(0, CiStatus::Success);
                self.ci_runs.truncate(12);
                vec![self.ci_runs()]
            }
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
        assert_eq!(FakeCmd::from_key('p'), None, "the price outage key is gone");
        assert_eq!(FakeCmd::from_key('u'), Some(FakeCmd::UpsToggle));
        assert_eq!(FakeCmd::from_key('d'), Some(FakeCmd::AppToggle));
        assert_eq!(FakeCmd::from_key('g'), Some(FakeCmd::GithubPush));
        assert_eq!(FakeCmd::from_key('r'), Some(FakeCmd::GithubStar));
        assert_eq!(FakeCmd::from_key('f'), Some(FakeCmd::CiFailure));
        assert_eq!(FakeCmd::from_key('l'), Some(FakeCmd::Thunder));
        assert_eq!(FakeCmd::from_key('w'), Some(FakeCmd::RainSoon));
        assert_eq!(FakeCmd::from_key('i'), Some(FakeCmd::IssPassNow));
        assert_eq!(FakeCmd::from_key('m'), Some(FakeCmd::MediaDone));
        assert_eq!(FakeCmd::from_key('k'), Some(FakeCmd::KickRestart));
        assert_eq!(FakeCmd::from_key('b'), Some(FakeCmd::Brownout));
        assert_eq!(FakeCmd::from_key('j'), Some(FakeCmd::JellyfinToggle));
        assert_eq!(FakeCmd::from_key('c'), Some(FakeCmd::CiPass));
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
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::MetricsHistory { cpu, .. } if cpu.len() == 31)));
        assert!(evs.iter().any(|e| matches!(e, Event::Forecast { .. })));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::AirQualityHistory { .. })));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::AppSyncHistory { .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::VolumeSizes(_))));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::MediaLibrary {
                movies: 37,
                series: 30,
                artists: 54
            }
        )));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Download(Some(d)) if d.progress > 0.3)));
        assert!(evs.iter().any(
            |e| matches!(e, Event::TrafficHistory { req_per_sec } if req_per_sec.len() == 31)
        ));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Traffic { top, .. } if top.len() == 3)));
        assert!(evs.iter().any(|e| matches!(e, Event::RestartsHistory { hourly } if hourly.len() == 24 && hourly[23].0 == 1_788_782_400 / 3600)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Restarts { total_24h, worst: Some(_) } if *total_24h > 0)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Uptime(n) if n.len() == 7)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Fans(f) if f.len() == 4)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::MainsHistory { hourly } if hourly.len() == 24)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Mains { volts, .. } if (*volts - 234.0).abs() < 1e-3)));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Load(n) if n.len() == 7)));
        assert!(evs.iter().any(|e| matches!(e, Event::CiRuns(r) if r.len() == 12 && r[0].status == CiStatus::InProgress)));
        assert!(evs.iter().any(|e| matches!(e, Event::Jellyfin { status: JfStatus::Ok, sessions } if sessions.len() == 2)));
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
        assert!(s
            .command(FakeCmd::CiFailure)
            .iter()
            .any(|e| matches!(e, Event::GithubRun { ok: false, .. })));
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
        assert!(
            evs.iter().any(|e| matches!(
                e,
                Event::Link {
                    target: LinkTarget::Electricity,
                    up: true
                }
            )),
            "electricity link up missing"
        );
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
    }

    #[test]
    fn tick_emits_temps_storage_and_electricity() {
        let mut s = FakeState::new(3);
        let ten: Vec<Event> = (0..10).flat_map(|_| s.tick()).collect();
        assert!(ten.iter().any(|e| matches!(e, Event::NodeTemps(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Storage { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Electricity { .. })));
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

    /// The seeded stream other roles (and the README gifs) are built from:
    /// music must not add or remove draws. Pinned before the music role.
    #[test]
    fn rng_draws_per_tick_are_unchanged() {
        let mut s = FakeState::new(7);
        s.set_clock(1_788_782_400, 7200);
        for _ in 0..120 {
            s.tick();
        }
        assert_eq!(s.rng.u64(..), 14_678_636_064_419_734_158);
    }

    fn music(evs: &[Event]) -> Vec<(MusicSource, Option<Track>)> {
        evs.iter()
            .filter_map(|e| match e {
                Event::Music { source, track } => Some((*source, track.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn music_cycles_spotify_paused_jellyfin_and_idle() {
        let mut s = FakeState::new(7);
        s.set_clock(1_788_782_400, 7200);
        let first = music(&s.initial());
        assert_eq!(first.len(), 2, "both sources report");
        let sp = first[0].1.as_ref().unwrap();
        assert_eq!(first[0].0, MusicSource::Spotify);
        assert!(sp.playing && sp.cover.is_some() && sp.title == "Holocene");
        assert!(first[1].1.is_none(), "jellyfin quiet at first");
        let t1 = music(&s.tick());
        let t2 = music(&s.tick());
        assert!(t1.is_empty(), "every second tick");
        let pos = |m: &[(MusicSource, Option<Track>)]| m[0].1.as_ref().unwrap().position_secs;
        assert!(pos(&t2) > pos(&first), "the position moves while playing");
        let paused = music(&s.command(FakeCmd::MusicToggle));
        assert!(!paused[0].1.as_ref().unwrap().playing, "spotify paused");
        let jf = music(&s.command(FakeCmd::MusicToggle));
        assert!(jf[0].1.is_none(), "spotify gone");
        let t = jf[1].1.as_ref().unwrap();
        assert!(t.playing && t.source == MusicSource::Jellyfin && t.cover.is_some());
        let idle = music(&s.command(FakeCmd::MusicToggle));
        assert!(idle.iter().all(|(_, t)| t.is_none()));
        let back = music(&s.command(FakeCmd::MusicToggle));
        assert!(back[0].1.as_ref().unwrap().playing);
        assert_eq!(FakeCmd::from_key('s'), Some(FakeCmd::MusicToggle));
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
    #[test]
    fn new_screen_commands_drive_their_tiles() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.command(FakeCmd::MediaDone);
        assert_eq!(
            evs[0],
            Event::MediaAdded {
                title: "The Lord of the Rings: The Return of the King".into()
            }
        );
        assert!(matches!(evs[1], Event::Download(None)));
        let later: Vec<Event> = (0..60).flat_map(|_| s.tick()).collect();
        assert!(
            later.iter().any(|e| matches!(e, Event::Download(Some(_)))),
            "a new download starts within a minute"
        );
        let before = s.restart_total;
        let evs = s.command(FakeCmd::KickRestart);
        assert!(
            matches!(&evs[0], Event::Restarts { total_24h, worst: Some((_, n)) } if *total_24h == before + 6 && *n >= 6)
        );
        assert!(matches!(&evs[1], Event::RestartsHistory { hourly } if hourly[23].1 >= 6.0));
        assert!(
            matches!(s.command(FakeCmd::Brownout).as_slice(), [Event::Mains { volts, .. }] if *volts < 220.0)
        );
        assert!(
            matches!(s.command(FakeCmd::Brownout).as_slice(), [Event::Mains { volts, .. }] if *volts > 230.0)
        );
        assert!(
            matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if sessions[0].paused)
        );
        assert!(
            matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if sessions.is_empty())
        );
        assert!(
            matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if !sessions.is_empty() && !sessions[0].paused)
        );
        let evs = s.command(FakeCmd::CiFailure);
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::GithubRun { ok: false, .. })));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::CiRuns(r) if r[0].status == CiStatus::Failure)));
        let evs = s.command(FakeCmd::CiPass);
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::CiRuns(r) if r[0].status == CiStatus::Success)));
        let ten: Vec<Event> = (0..10).flat_map(|_| s.tick()).collect();
        assert!(ten.iter().any(|e| matches!(e, Event::Traffic { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Jellyfin { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Load(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Fans(_))));
        let pcts: Vec<f32> = (0..200)
            .flat_map(|_| s.tick())
            .filter_map(|e| match e {
                Event::Fans(f) => Some(f.into_iter().map(|(_, p)| p)),
                _ => None,
            })
            .flatten()
            .collect();
        assert!(!pcts.is_empty());
        assert!(
            pcts.iter().all(|p| (35.0..=60.0).contains(p)),
            "fan power stays a plausible 35–60 %"
        );
        assert!(ten.iter().any(|e| matches!(e, Event::Mains { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Uptime(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Restarts { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::MediaLibrary { .. })));
    }
}

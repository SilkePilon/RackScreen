//! Events emitted by sources and folded into the model.

use crate::electricity::Source;

#[derive(Clone, Debug, PartialEq)]
pub struct Torrent {
    pub name: String,
    /// 0.0 ..= 100.0
    pub progress: f32,
    /// seconds, negative or huge means unknown
    pub eta_secs: i64,
    pub speed_bps: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Robustness {
    Healthy,
    Degraded,
    Faulted,
    Unknown,
}

impl Robustness {
    /// Longhorn's `longhorn_volume_robustness` value.
    pub fn from_code(v: f64) -> Robustness {
        match v as i64 {
            1 => Robustness::Healthy,
            2 => Robustness::Degraded,
            3 => Robustness::Faulted,
            _ => Robustness::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoonPhase {
    New,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    Full,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl MoonPhase {
    /// Short badge text.
    pub fn label(self) -> &'static str {
        match self {
            MoonPhase::New => "new",
            MoonPhase::WaxingCrescent | MoonPhase::WaxingGibbous => "waxing",
            MoonPhase::FirstQuarter => "first q",
            MoonPhase::Full => "full",
            MoonPhase::WaningGibbous | MoonPhase::WaningCrescent => "waning",
            MoonPhase::LastQuarter => "last q",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppSync {
    Synced,
    OutOfSync,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppHealth {
    Healthy,
    Progressing,
    Degraded,
    Suspended,
    Missing,
    Unknown,
}

/// One Argo CD application: name, sync state, health, and whether an operation is running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub sync: AppSync,
    pub health: AppHealth,
    pub operating: bool,
}

/// The next ISS pass over the observer, unix seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IssPass {
    pub start: i64,
    pub end: i64,
    pub max_elevation_deg: f32,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    K8sApi,
    Prometheus,
    QBittorrent,
    Electricity,
    Prices,
    Weather,
    Rain,
    Github,
    ArgoCd,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Metrics {
        cpu_pct: f32,
        mem_pct: f32,
        mem_used_gb: f32,
        mem_total_gb: f32,
        hot_cpu: Option<(String, f32)>,
        hot_mem: Option<(String, f32)>,
    },
    PodSnapshot {
        running: u32,
        pending: u32,
        failed: u32,
        total: u32,
    },
    PodStarted {
        ns: String,
        name: String,
    },
    PodCrashed {
        ns: String,
        name: String,
    },
    PodGone {
        ns: String,
        name: String,
    },
    NodeSnapshot {
        ready: u32,
        total: u32,
        not_ready: Vec<String>,
    },
    NodeReady {
        name: String,
        ready: bool,
    },
    AlertSnapshot {
        firing: Vec<String>,
    },
    AlertChanged {
        name: String,
        firing: bool,
    },
    /// Per-node temperature in degrees Celsius.
    NodeTemps(Vec<(String, f32)>),
    Storage {
        volumes: Vec<(String, Robustness)>,
        used_bytes: u64,
        capacity_bytes: u64,
    },
    HotTemp {
        node: String,
        celsius: f32,
    },
    VolumeDegraded {
        name: String,
        robustness: Robustness,
    },
    VolumeHealthy {
        name: String,
    },
    /// One Electricity Maps power-breakdown sample for a zone.
    Electricity {
        zone: String,
        /// Production per source in MW, only the sources the zone reports.
        mix_mw: Vec<(Source, f32)>,
        renewable_pct: f32,
        fossil_free_pct: f32,
        carbon_gco2: f32,
        updated_at: String,
    },
    /// Day-ahead prices, one entry per local quarter-hour of `date` starting
    /// at 00:00 (NaN where unknown), plus the mean over that day and the two
    /// before it.
    Prices {
        date: String,
        eur_per_kwh: Vec<f32>,
        avg_eur_per_kwh: f32,
        currency: String,
    },
    /// Current conditions from Open-Meteo.
    Weather {
        temp_c: f32,
        /// WMO weather interpretation code.
        code: u16,
        is_day: bool,
        wind_kmh: f32,
        gust_kmh: f32,
        /// Direction the wind comes from, degrees clockwise from north.
        wind_from_deg: f32,
        at: String,
    },
    AirQuality {
        eaqi: f32,
    },
    /// Buienradar nowcast: 24 five-minute slots from `from`.
    Rain {
        from: i64,
        mm_per_h: Vec<f32>,
    },
    /// Sun and moon, computed on the Pi once a minute.
    Sky {
        sunrise: Option<i64>,
        sunset: Option<i64>,
        sun_elevation_deg: f32,
        /// 0..1
        moon_illumination: f32,
        moon_waxing: bool,
        moon_phase: MoonPhase,
    },
    /// `None` when no pass clears `iss.min_elevation` in the next 24 h.
    IssPass(Option<IssPass>),
    /// Thirty days of contribution counts, oldest first, last entry today.
    GithubActivity {
        days: Vec<(String, u32)>,
    },
    GithubPush {
        repo: String,
        commits: u32,
    },
    GithubStar {
        repo: String,
    },
    GithubMerge {
        repo: String,
    },
    GithubRelease {
        repo: String,
        tag: String,
    },
    GithubRun {
        repo: String,
        ok: bool,
    },
    Ups {
        on_battery: bool,
        low_battery: bool,
        charge_pct: f32,
        load_pct: f32,
        runtime_secs: u32,
    },
    UpsOnBattery,
    UpsOnline,
    Network {
        rx_bps: f64,
        tx_bps: f64,
    },
    /// Every Argo CD application, sorted by name.
    Apps(Vec<App>),
    AppSynced {
        name: String,
    },
    AppDegraded {
        name: String,
    },
    AppHealthy {
        name: String,
    },
    Torrents(Vec<Torrent>),
    TorrentAdded {
        name: String,
    },
    TorrentDone {
        name: String,
    },
    Link {
        target: LinkTarget,
        up: bool,
    },
    /// Replay the boot animation (simulator key `b`, and on startup).
    Boot,
    /// Simulator override: Some(true) forces night, Some(false) forces day, None back to clock.
    ForceNight(Option<bool>),
}

//! Events emitted by sources and folded into the model.

use crate::electricity::Source;
use crate::theme::Role;

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
    /// The P2 pill text.
    pub fn name(self) -> &'static str {
        match self {
            MoonPhase::New => "New moon",
            MoonPhase::WaxingCrescent => "Waxing crescent",
            MoonPhase::FirstQuarter => "First quarter",
            MoonPhase::WaxingGibbous => "Waxing gibbous",
            MoonPhase::Full => "Full moon",
            MoonPhase::WaningGibbous => "Waning gibbous",
            MoonPhase::LastQuarter => "Last quarter",
            MoonPhase::WaningCrescent => "Waning crescent",
        }
    }

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
    /// The stretch of the pass that can be seen by eye (the station at
    /// least 10° up, the observer in the dark, the station still sunlit).
    pub vis: Option<(i64, i64)>,
}

/// Where the ISS is over the ground around now: sub-satellite points every
/// `step_secs` from `t0` (unix seconds), about an hour back and an orbit
/// ahead. The globe screen interpolates between them.
#[derive(Clone, Debug, PartialEq)]
pub struct IssTrack {
    pub t0: i64,
    pub step_secs: u32,
    /// `(lat, lon)` in degrees.
    pub points: Vec<(f32, f32)>,
    /// The observer, `(lat, lon)` in degrees.
    pub home: (f32, f32),
}

/// One node's load averages and core count, for the load role.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeLoad {
    pub node: String,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub cores: u32,
}

/// The arr download shown on the media card.
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub title: String,
    /// 0.0 ..= 1.0
    pub progress: f32,
    /// seconds; negative means unknown
    pub eta_secs: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiStatus {
    Success,
    Failure,
    InProgress,
    /// cancelled, skipped, neutral, …
    Other,
}

/// One GitHub Actions workflow run.
#[derive(Clone, Debug, PartialEq)]
pub struct CiRun {
    pub id: u64,
    pub repo: String,
    pub status: CiStatus,
    /// unix seconds
    pub created_at: i64,
    pub updated_at: i64,
    /// `updated_at − run_started_at` for completed runs with a start.
    pub duration_secs: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JfKind {
    Movie,
    Episode,
    Other,
}

/// One Jellyfin session with something playing.
#[derive(Clone, Debug, PartialEq)]
pub struct JfSession {
    pub title: String,
    pub kind: JfKind,
    pub series: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub year: Option<u32>,
    pub runtime_secs: f64,
    /// Where playback was at the poll.
    pub position_secs: f64,
    pub paused: bool,
    pub transcoding: bool,
    pub user: String,
    pub device: String,
    /// `LastActivityDate` as sent, ISO 8601 UTC; sorts lexically.
    pub last_activity: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JfStatus {
    Ok,
    Offline,
    Unauthorized,
}

/// Where a `Track` comes from; also picks the music screen's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MusicSource {
    Spotify,
    Jellyfin,
}

impl MusicSource {
    pub fn index(self) -> usize {
        match self {
            MusicSource::Spotify => 0,
            MusicSource::Jellyfin => 1,
        }
    }
}

/// Album art, decoded and squared: `size × size` straight RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Cover {
    /// Stable id of the art (album id); equal keys mean equal pixels.
    pub key: String,
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// A shared cover, compared by key and size, not by pixels or pointer, so
/// scenes stay cheap to compare.
#[derive(Clone)]
pub struct CoverRef(pub std::sync::Arc<Cover>);

impl CoverRef {
    pub fn new(cover: Cover) -> Self {
        CoverRef(std::sync::Arc::new(cover))
    }
}

impl PartialEq for CoverRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.key == other.0.key && self.0.size == other.0.size
    }
}

impl std::fmt::Debug for CoverRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CoverRef({} {}px)", self.0.key, self.0.size)
    }
}

/// The one track a music source reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub source: MusicSource,
    pub title: String,
    pub artist: String,
    /// Spotify device name, or `user · device` for Jellyfin.
    pub device: String,
    pub duration_secs: f64,
    /// Where playback was at the poll.
    pub position_secs: f64,
    pub playing: bool,
    /// Unix seconds of the last start, pause or track change (see
    /// `music::stamp_since`).
    pub since: i64,
    pub cover: Option<CoverRef>,
}

/// Where a 3D print stands, from Bambu's `gcode_state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintStage {
    Idle,
    /// Heating, levelling, loading filament: the job started, the layers
    /// have not.
    Preparing,
    Printing,
    Paused,
    Finished,
    Failed,
}

/// Whether the printer's MQTT report can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrinterStatus {
    Ok,
    Offline,
    /// The broker refused the access code.
    Unauthorized,
    /// Cloud mode without a usable Bambu Lab login.
    LoggedOut,
}

/// The printer's state, merged from its (partial) MQTT reports.
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterReport {
    pub stage: PrintStage,
    /// 0 to 100.
    pub percent: f32,
    pub remaining_mins: Option<u32>,
    pub layer: Option<u32>,
    pub total_layers: Option<u32>,
    pub nozzle_c: Option<f32>,
    pub bed_c: Option<f32>,
    /// The job's name (`subtask_name`), empty when unknown.
    pub job: String,
    /// The loaded filament's colour and type (`PLA`), when the AMS or the
    /// external spool says.
    pub filament: Option<(crate::theme::Color, String)>,
}

/// Whether a Home Assistant entity can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaStatus {
    Ok,
    Offline,
    /// The token was refused.
    Unauthorized,
    /// Home Assistant has no entity by that id.
    NotFound,
}

/// One Home Assistant entity state.
#[derive(Clone, Debug, PartialEq)]
pub struct HaEntity {
    /// The raw state: `21.4`, `on`, `unavailable`.
    pub state: String,
    /// `friendly_name`, or the entity id without one.
    pub name: String,
    /// `unit_of_measurement`.
    pub unit: Option<String>,
    pub device_class: Option<String>,
    /// The part of the entity id before the dot: `sensor`, `light`.
    pub domain: String,
    /// Unix seconds of `last_changed`.
    pub last_changed: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    K8sApi,
    Prometheus,
    QBittorrent,
    Electricity,
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
    /// Recent samples from a Prometheus range query at start, oldest first:
    /// cpu % every 2 s, mem % every 3 s, download Mb/s every 2 s.
    MetricsHistory {
        cpu: Vec<f32>,
        mem: Vec<f32>,
        net_rx_mbps: Vec<f32>,
    },
    /// Hourly forecast from Open-Meteo: the next 12 hours from the current
    /// hour, plus today's low and high.
    Forecast {
        hourly_temp_c: Vec<f32>,
        hourly_precip_mm: Vec<f32>,
        day_min_c: f32,
        day_max_c: f32,
    },
    /// The last 24 hourly EAQI values ending at the current hour, oldest first,
    /// gaps skipped.
    AirQualityHistory {
        hourly: Vec<f32>,
    },
    /// Longhorn actual size per volume in bytes.
    VolumeSizes(Vec<(String, u64)>),
    /// `finishedAt` of every Argo CD application's last operation, unix seconds,
    /// sent once after the initial list so today's sync count survives a restart.
    AppSyncHistory {
        finished_at: Vec<i64>,
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
        updated_at: String,
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
    /// The ISS ephemeris for the globe; re-sent every few minutes.
    IssTrack(IssTrack),
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
    /// Library counts from the scraparr exporter.
    MediaLibrary {
        movies: u32,
        series: u32,
        artists: u32,
    },
    /// Traefik: requests per second at the entrypoints, the three busiest
    /// services (short name, req/s), the summed service rate and the 5xx share.
    Traffic {
        req_per_sec: f32,
        top: Vec<(String, f32)>,
        service_rps: f32,
        error_share: f32,
    },
    /// The last 90 s of entrypoint req/s every 3 s, oldest first.
    TrafficHistory {
        req_per_sec: Vec<f32>,
    },
    /// Container restarts over 24 h and the worst pod (short name, count).
    Restarts {
        total_24h: u32,
        worst: Option<(String, u32)>,
    },
    /// Restarts per hour, `(unix hour, count)`, oldest first.
    RestartsHistory {
        hourly: Vec<(i64, f32)>,
    },
    /// Seconds since boot per node.
    Uptime(Vec<(String, f64)>),
    /// Fan power per node in percent (pwm duty, 0–100); nodes without a fan
    /// are absent.
    Fans(Vec<(String, f32)>),
    /// UPS input voltage and load.
    Mains {
        volts: f32,
        ups_load_pct: f32,
    },
    /// Hourly mean input voltage, `(unix hour, volts)`, oldest first.
    MainsHistory {
        hourly: Vec<(i64, f32)>,
    },
    /// Load averages per node.
    Load(Vec<NodeLoad>),
    /// The arr download with the least time left, `None` when the queues are empty.
    Download(Option<Download>),
    /// An item left the arr queue completed.
    MediaAdded {
        title: String,
    },
    /// The last 12 Actions runs across the watched repos, newest first.
    CiRuns(Vec<CiRun>),
    /// One poll of `/Sessions`: the sessions with an item, or why there are none.
    Jellyfin {
        status: JfStatus,
        sessions: Vec<JfSession>,
    },
    /// One poll of a music source: its current track, `None` when nothing is
    /// loaded (or the source is unreachable).
    Music {
        source: MusicSource,
        track: Option<Track>,
    },
    /// The printer's merged report; `None` while it is unreachable.
    Printer {
        status: PrinterStatus,
        report: Option<PrinterReport>,
    },
    /// One poll of the Home Assistant entity at `index` of the config's
    /// `homeassistant.entities` list.
    HomeAssistant {
        index: u16,
        status: HaStatus,
        entity: Option<HaEntity>,
    },
    /// Prometheus answered but has no series for the role (exporter not
    /// installed); the next data event for the role clears it.
    NoExporter {
        role: Role,
    },
    /// Simulator override: Some(true) forces night, Some(false) forces day, None back to clock.
    ForceNight(Option<bool>),
    /// The GIF at `index` of the config's `gifs:` list is decoded.
    Gif {
        index: u16,
        frames: crate::gif::GifRef,
    },
    /// The GIF at `index` could not be fetched or decoded (the reason is
    /// logged by the source); it retries later.
    GifFailed {
        index: u16,
        reason: String,
    },
    /// The SEN live stream: a frame, or a change of what it shows.
    Sen(SenEvent),
}

/// What the SEN stream source reports.
#[derive(Clone, Debug, PartialEq)]
pub enum SenEvent {
    /// The next picture of live footage; only sent while the stream is live.
    Frame(crate::video::VideoRef),
    /// The stream shows live Earth footage (or stopped doing so).
    Live(bool),
    /// The stream is no longer being fetched: nothing is held any more.
    Stopped,
}

//! Cluster state and the fold of events into it.

use crate::anim::Secs;
use crate::electricity::Source;
use crate::event::{
    App, CiRun, Download, Event, IssPass, JfSession, JfStatus, LinkTarget, MoonPhase, NodeLoad,
    Robustness, Torrent,
};
use crate::history::{History, Hourly};
use crate::screens::ScreenState;
use crate::shows::schedule::{Request, ShowPhase, ShowRun, ShowScheduler};
use crate::shows::world::{Layout, WorldFrame};
use crate::theme::Role;
use std::sync::Arc;

/// Dwell per role on a cycling screen when the caller gives none.
pub const DEFAULT_CYCLE_SECS: Secs = 15.0;
/// Seed the transition picker starts from; `set_rng_seed` replaces it per boot.
const RNG_SEED: u64 = 0x2545_F491_4F6C_DD1D;
/// Extra first dwell per screen index, so equal dwells do not transition together.
/// Shrunk when the dwell is too short to spread every screen over one cycle.
pub const SCREEN_STAGGER_SECS: Secs = 2.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    pub hot_cpu: f32,
    pub hot_mem: f32,
    /// Node temperature in °C that raises a hot-temp splash.
    pub hot_temp: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            hot_cpu: 90.0,
            hot_mem: 90.0,
            hot_temp: 70.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkState {
    pub api: bool,
    pub prom: bool,
    pub qbit: bool,
    pub electricity: bool,
    pub weather: bool,
    pub rain: bool,
    pub github: bool,
    pub argocd: bool,
}

/// The latest Electricity Maps sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElectricityState {
    pub zone: String,
    /// Production per source in MW, as reported.
    pub mix: Vec<(Source, f32)>,
    pub renewable_pct: f32,
    pub fossil_free_pct: f32,
    pub updated_at: String,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpsState {
    pub on_battery: bool,
    pub low_battery: bool,
    pub charge_pct: f32,
    pub load_pct: f32,
    pub runtime_secs: u32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetState {
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppsState {
    pub apps: Vec<App>,
    pub have: bool,
}

/// Thirty days of contribution counts, oldest first, last entry today.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GithubState {
    pub days: Vec<(String, u32)>,
    pub have: bool,
}

impl GithubState {
    pub fn today(&self) -> u32 {
        self.days.last().map(|d| d.1).unwrap_or(0)
    }
    /// Best day in the window, at least 1 so ratios stay finite.
    pub fn best(&self) -> u32 {
        self.days.iter().map(|d| d.1).max().unwrap_or(0).max(1)
    }
}

/// Argo syncs per local hour today, and the last one seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeployLog {
    /// Local day index (days since the epoch in local time) the buckets belong to.
    pub day: i64,
    pub hours: [u32; 24],
    /// App name and the monotonic time of the last sync.
    pub last: Option<(String, Secs)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherState {
    pub temp_c: f32,
    pub code: u16,
    pub is_day: bool,
    pub wind_kmh: f32,
    pub gust_kmh: f32,
    pub wind_from_deg: f32,
    pub at: String,
    pub have: bool,
    pub hourly_temp_c: Vec<f32>,
    pub hourly_precip_mm: Vec<f32>,
    pub day_min_c: Option<f32>,
    pub day_max_c: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AirState {
    pub eaqi: f32,
    pub have: bool,
    pub hourly: Vec<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RainState {
    pub from: i64,
    pub mm_per_h: Vec<f32>,
    pub have: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkyState {
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub sun_elevation_deg: f32,
    pub moon_illumination: f32,
    pub moon_waxing: bool,
    pub moon_phase: MoonPhase,
    pub have: bool,
}

impl Default for SkyState {
    fn default() -> Self {
        Self {
            sunrise: None,
            sunset: None,
            sun_elevation_deg: 0.0,
            moon_illumination: 0.0,
            moon_waxing: true,
            moon_phase: MoonPhase::New,
            have: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssState {
    pub pass: Option<IssPass>,
    pub have: bool,
}

/// Library counts from the arr exporter; the download card state arrives in Task 5.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub movies: u32,
    pub series: u32,
    pub artists: u32,
    pub have: bool,
    /// The arr download with the least time left.
    pub download: Option<Download>,
    /// The last item that left the queue completed, and when (monotonic).
    pub last_added: Option<(String, Secs)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrafficState {
    pub req_per_sec: f32,
    /// The three busiest services, `(short name, req/s)`, busiest first.
    pub top: Vec<(String, f32)>,
    pub service_rps: f32,
    /// 0..=1 share of 5xx responses over the last 5 min.
    pub error_share: f32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestartsState {
    pub total_24h: u32,
    pub worst: Option<(String, u32)>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UptimeState {
    /// `(node, seconds up)`, sorted by node name.
    pub nodes: Vec<(String, f64)>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FansState {
    /// `(node, fan power %)`, sorted by node name.
    pub fans: Vec<(String, f32)>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainsState {
    pub volts: f32,
    pub ups_load_pct: f32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadState {
    /// Sorted by node name.
    pub nodes: Vec<NodeLoad>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CiState {
    /// Newest first, at most 12.
    pub runs: Vec<CiRun>,
    pub have: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JellyfinState {
    pub status: JfStatus,
    pub sessions: Vec<JfSession>,
    /// Monotonic time of the last poll; positions extrapolate from it.
    pub polled_at: Secs,
    pub have: bool,
    /// The last item seen playing this run and when (unix seconds).
    pub last_seen: Option<(String, i64)>,
}

impl Default for JellyfinState {
    fn default() -> Self {
        Self {
            status: JfStatus::Offline,
            sessions: Vec::new(),
            polled_at: 0.0,
            have: false,
            last_seen: None,
        }
    }
}

/// The music sources' latest polls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MusicState {
    pub spotify: Option<crate::event::Track>,
    pub jellyfin: Option<crate::event::Track>,
    /// Monotonic time of each source's last poll, by `MusicSource::index`.
    pub polled_at: [Secs; 2],
    pub have: bool,
    /// The last track heard and when (unix seconds): stamped on every poll
    /// while it plays, kept while it is paused or gone.
    pub last: Option<(crate::event::Track, i64)>,
}

impl MusicState {
    /// The track the screen shows (`music::pick`).
    pub fn current(&self) -> Option<&crate::event::Track> {
        crate::music::pick(self.spotify.as_ref(), self.jellyfin.as_ref())
    }
    pub fn polled_at(&self, source: crate::event::MusicSource) -> Secs {
        self.polled_at[source.index()]
    }
}

/// Ring fill for a bit rate: 100 kbit/s is (almost) empty, 1 Gbit/s is full,
/// four decades in between, and a floor so idle still shows a little.
pub fn net_fill(bps: f64) -> f32 {
    if bps <= 0.0 {
        return 0.03;
    }
    (((bps / 1e5).log10() / 4.0) as f32).clamp(0.03, 1.0)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClusterState {
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub pods_running: u32,
    pub pods_pending: u32,
    pub pods_failed: u32,
    pub pods_total: u32,
    pub nodes_ready: u32,
    pub nodes_total: u32,
    pub nodes_not_ready: Vec<String>,
    pub alerts: Vec<String>,
    pub torrents: Vec<Torrent>,
    /// Per-node temperature in °C.
    pub temps: Vec<(String, f32)>,
    pub volumes: Vec<(String, Robustness)>,
    pub storage_used: u64,
    pub storage_capacity: u64,
    pub have_metrics: bool,
    pub have_pods: bool,
    pub have_nodes: bool,
    pub have_temps: bool,
    pub have_storage: bool,
    pub volume_bytes: Vec<(String, u64)>,
}

pub struct Model {
    state: ClusterState,
    link: LinkState,
    thresholds: Thresholds,
    electricity: ElectricityState,
    ups: UpsState,
    net: NetState,
    apps: AppsState,
    github: GithubState,
    weather: WeatherState,
    air: AirState,
    rain: RainState,
    sky: SkyState,
    iss: IssState,
    media: MediaState,
    traffic: TrafficState,
    restarts: RestartsState,
    uptime: UptimeState,
    fans: FansState,
    mains: MainsState,
    load: LoadState,
    ci: CiState,
    jellyfin: JellyfinState,
    music: MusicState,
    /// Entrypoint req/s every 3 s, 90 s.
    traffic_hist: History,
    /// Restarts and mean input voltage per unix hour, 24 h.
    restarts_hist: Hourly,
    mains_hist: Hourly,
    /// The sparkline buffers: cpu % at 2 s, mem % at 3 s, download Mb/s at 2 s.
    cpu_hist: History,
    mem_hist: History,
    net_hist: History,
    /// Renewable share per unix hour, 24 h.
    renewable_hist: Hourly,
    deploys: DeployLog,
    /// An Electricity Maps token is configured; without one the electricity
    /// roles show a key instead of the offline cloud.
    token_present: bool,
    /// Unix seconds, set by the render loop every frame; the sun dial, rain
    /// ring and ISS countdown are clock-based.
    unix_now: i64,
    /// Seconds east of UTC for the Pi's local zone.
    utc_offset_secs: i32,
    /// `location.lat`/`lon` are set; without them the sky roles show a pin.
    location_present: bool,
    /// A GitHub token is set; without one `gh-activity` shows a key.
    github_token_present: bool,
    night_override: Option<bool>,
    /// One entry per physical screen, top to bottom.
    screens: Vec<ScreenState>,
    /// Only one screen may transition at a time, in random order: simultaneous
    /// transitions on the Pi make four displays fight over one SPI bus.
    one_at_a_time: bool,
    /// While serialising: the instant the next transition may start, i.e. the
    /// end of the last one plus `TRANSITION_GAP_SECS`.
    transition_free_at: Secs,
    /// xorshift64 state, for picking which waiting screen goes next.
    rng: u64,
    /// Roles whose Prometheus query came back empty on the last poll.
    exporter_missing: Vec<Role>,
    /// An arr API key is configured: the media card never falls back to the
    /// qBittorrent torrent.
    arr_configured: bool,
    /// `jellyfin.url` is set; without it the role says "Check config".
    jellyfin_configured: bool,
    /// A Spotify login or a Jellyfin URL exists; without either the music
    /// role says "Check config".
    music_configured: bool,
    /// When the four-screen shows play; off until `set_shows`.
    shows: ShowScheduler,
    show_layout: Layout,
    /// Config index of each screen from the left; empty is config order.
    show_order: Vec<usize>,
    /// A play-now request waiting for the screens to be free.
    show_request: Option<Request>,
    /// The night machine is in `Day`: shows may start.
    night_day: bool,
    /// The night just ended: the next tick restarts the show schedule.
    show_wake: bool,
    /// The current world frame, keyed by (show id, 12 fps frame index): one
    /// `Arc` per frame for every screen and tick, so equal scenes stay equal.
    show_frame: Option<(&'static str, u32, Arc<WorldFrame>)>,
    /// Decoded GIFs by index into the config's `gifs:` list.
    gifs: Vec<crate::gif::GifState>,
}

/// xorshift64: a whole PRNG in three shifts, so `core` keeps no rand dependency.
fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

impl Model {
    pub fn new(thresholds: Thresholds) -> Self {
        Self {
            state: ClusterState::default(),
            link: LinkState {
                api: false,
                prom: false,
                qbit: false,
                electricity: false,
                weather: false,
                rain: false,
                github: false,
                argocd: false,
            },
            thresholds,
            electricity: ElectricityState::default(),
            ups: UpsState::default(),
            net: NetState::default(),
            apps: AppsState::default(),
            github: GithubState::default(),
            weather: WeatherState::default(),
            air: AirState::default(),
            rain: RainState::default(),
            sky: SkyState::default(),
            iss: IssState::default(),
            media: MediaState::default(),
            traffic: TrafficState::default(),
            restarts: RestartsState::default(),
            uptime: UptimeState::default(),
            fans: FansState::default(),
            mains: MainsState::default(),
            load: LoadState::default(),
            ci: CiState::default(),
            jellyfin: JellyfinState::default(),
            music: MusicState::default(),
            traffic_hist: History::new(31, 3.0),
            restarts_hist: Hourly::new(),
            mains_hist: Hourly::new(),
            cpu_hist: History::new(31, 2.0),
            mem_hist: History::new(31, 3.0),
            net_hist: History::new(31, 2.0),
            renewable_hist: Hourly::new(),
            deploys: DeployLog::default(),
            token_present: false,
            unix_now: 0,
            utc_offset_secs: 0,
            location_present: false,
            github_token_present: false,
            night_override: None,
            screens: [Role::Cpu, Role::Mem, Role::Pods, Role::Health]
                .into_iter()
                .map(|r| ScreenState::new(vec![r], DEFAULT_CYCLE_SECS))
                .collect(),
            one_at_a_time: false,
            transition_free_at: 0.0,
            rng: RNG_SEED,
            exporter_missing: Vec::new(),
            arr_configured: false,
            jellyfin_configured: false,
            music_configured: false,
            shows: ShowScheduler::new(false, 15, Vec::new()),
            show_layout: Layout { panels: 4, gap: 46 },
            show_order: Vec::new(),
            show_request: None,
            night_day: true,
            show_wake: false,
            show_frame: None,
            gifs: Vec::new(),
        }
    }

    /// The GIF at `index` of the config's `gifs:` list.
    pub fn gif(&self, index: u16) -> &crate::gif::GifState {
        static LOADING: crate::gif::GifState = crate::gif::GifState::Loading;
        self.gifs.get(index as usize).unwrap_or(&LOADING)
    }
    fn gif_slot(&mut self, index: u16) -> &mut crate::gif::GifState {
        let i = index as usize;
        if self.gifs.len() <= i {
            self.gifs.resize(i + 1, crate::gif::GifState::Loading);
        }
        &mut self.gifs[i]
    }
    pub fn state(&self) -> &ClusterState {
        &self.state
    }
    pub fn link(&self) -> LinkState {
        self.link
    }
    pub fn thresholds(&self) -> Thresholds {
        self.thresholds
    }
    pub fn night_override(&self) -> Option<bool> {
        self.night_override
    }
    pub fn cpu_history(&self) -> &History {
        &self.cpu_hist
    }
    pub fn mem_history(&self) -> &History {
        &self.mem_hist
    }
    pub fn net_history(&self) -> &History {
        &self.net_hist
    }
    pub fn renewable_hourly(&self) -> &Hourly {
        &self.renewable_hist
    }
    /// The current unix hour, the hourly buffers' key.
    pub fn unix_hour(&self) -> i64 {
        self.unix_now.div_euclid(3600)
    }
    pub fn deploys_log(&self) -> &DeployLog {
        &self.deploys
    }
    /// Seconds since the epoch on the local clock.
    pub fn local_secs(&self) -> i64 {
        self.unix_now + self.utc_offset_secs as i64
    }
    pub fn local_day(&self) -> i64 {
        self.local_secs().div_euclid(86_400)
    }
    pub fn local_hour(&self) -> usize {
        (self.local_secs().rem_euclid(86_400) / 3600) as usize
    }
    /// The unix hour at local midnight, the first hour "today" in the hourly buffers.
    pub fn local_midnight_unix_hour(&self) -> i64 {
        (self.local_day() * 86_400 - self.utc_offset_secs as i64).div_euclid(3600)
    }
    fn roll_deploy_day(&mut self) {
        let day = self.local_day();
        if self.deploys.day != day {
            self.deploys.day = day;
            self.deploys.hours = [0; 24];
        }
    }
    pub fn electricity(&self) -> &ElectricityState {
        &self.electricity
    }
    pub fn ups(&self) -> &UpsState {
        &self.ups
    }
    pub fn net(&self) -> &NetState {
        &self.net
    }
    pub fn apps(&self) -> &AppsState {
        &self.apps
    }
    pub fn github(&self) -> &GithubState {
        &self.github
    }
    pub fn weather(&self) -> &WeatherState {
        &self.weather
    }
    pub fn air(&self) -> &AirState {
        &self.air
    }
    pub fn rain(&self) -> &RainState {
        &self.rain
    }
    pub fn sky(&self) -> &SkyState {
        &self.sky
    }
    pub fn iss(&self) -> &IssState {
        &self.iss
    }
    pub fn media(&self) -> &MediaState {
        &self.media
    }
    pub fn traffic(&self) -> &TrafficState {
        &self.traffic
    }
    pub fn traffic_history(&self) -> &History {
        &self.traffic_hist
    }
    pub fn restarts(&self) -> &RestartsState {
        &self.restarts
    }
    pub fn restarts_hourly(&self) -> &Hourly {
        &self.restarts_hist
    }
    pub fn uptime(&self) -> &UptimeState {
        &self.uptime
    }
    pub fn fans(&self) -> &FansState {
        &self.fans
    }
    pub fn mains(&self) -> &MainsState {
        &self.mains
    }
    pub fn mains_hourly(&self) -> &Hourly {
        &self.mains_hist
    }
    pub fn load(&self) -> &LoadState {
        &self.load
    }
    pub fn ci(&self) -> &CiState {
        &self.ci
    }
    pub fn jellyfin(&self) -> &JellyfinState {
        &self.jellyfin
    }
    pub fn music(&self) -> &MusicState {
        &self.music
    }
    pub fn set_music_configured(&mut self, on: bool) {
        self.music_configured = on;
    }
    pub fn music_configured(&self) -> bool {
        self.music_configured
    }
    pub fn set_token_present(&mut self, present: bool) {
        self.token_present = present;
    }
    pub fn token_present(&self) -> bool {
        self.token_present
    }
    pub fn set_unix_now(&mut self, secs: i64) {
        self.unix_now = secs;
    }
    pub fn unix_now(&self) -> i64 {
        self.unix_now
    }
    pub fn set_utc_offset_secs(&mut self, secs: i32) {
        self.utc_offset_secs = secs;
    }
    pub fn utc_offset_secs(&self) -> i32 {
        self.utc_offset_secs
    }
    pub fn set_location_present(&mut self, present: bool) {
        self.location_present = present;
    }
    pub fn location_present(&self) -> bool {
        self.location_present
    }
    pub fn exporter_missing(&self, role: Role) -> bool {
        self.exporter_missing.contains(&role)
    }
    pub(crate) fn clear_exporter(&mut self, role: Role) {
        self.exporter_missing.retain(|r| *r != role);
    }
    pub fn set_arr_configured(&mut self, on: bool) {
        self.arr_configured = on;
    }
    pub fn arr_configured(&self) -> bool {
        self.arr_configured
    }
    pub fn set_jellyfin_configured(&mut self, on: bool) {
        self.jellyfin_configured = on;
    }
    pub fn jellyfin_configured(&self) -> bool {
        self.jellyfin_configured
    }
    pub fn set_github_token_present(&mut self, present: bool) {
        self.github_token_present = present;
    }
    pub fn github_token_present(&self) -> bool {
        self.github_token_present
    }

    /// Replace the per-screen role lists. `cycle_secs[i]` is the dwell for screen
    /// `i`; missing entries use `DEFAULT_CYCLE_SECS`. An empty layout keeps one
    /// CPU screen so `scene(0, ..)` always has something to show.
    pub fn set_screens(&mut self, roles: Vec<Vec<Role>>, cycle_secs: Vec<Secs>) {
        let count = roles.len().max(1);
        self.screens = roles
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let cycle = cycle_secs
                    .get(i)
                    .copied()
                    .unwrap_or(DEFAULT_CYCLE_SECS)
                    .max(crate::screens::MIN_CYCLE_SECS);
                // Spread the first transitions over at most one dwell, so screens
                // that share a dwell never transition on the same frame again.
                let step = SCREEN_STAGGER_SECS.min(cycle / count as Secs);
                ScreenState::new_with_offset(r, cycle, i as Secs * step)
            })
            .collect();
        if self.screens.is_empty() {
            self.screens
                .push(ScreenState::new(vec![Role::Cpu], DEFAULT_CYCLE_SECS));
        }
    }
    pub fn screen_count(&self) -> usize {
        self.screens.len()
    }
    pub fn current_role(&self, screen: usize) -> Role {
        self.screens
            .get(screen)
            .map(|s| s.current())
            .unwrap_or(Role::Cpu)
    }
    pub fn screen(&self, screen: usize) -> Option<&ScreenState> {
        self.screens.get(screen)
    }

    /// Show `role` on `screen` now; false when the screen or the role (in
    /// that screen's list) does not exist. Ignored while a show plays.
    pub fn jump_to(&mut self, screen: usize, role: Role, now: Secs) -> bool {
        if self.shows.current().is_some() {
            return false;
        }
        self.screens
            .get_mut(screen)
            .is_some_and(|s| s.jump_to(role, now))
    }

    /// Serialise transitions: at most one screen at a time, the next one
    /// picked at random from those whose dwell has elapsed.
    pub fn set_one_at_a_time(&mut self, on: bool) {
        self.one_at_a_time = on;
    }

    /// Reseed the picker (zero is bumped, xorshift dies on it).
    pub fn set_rng_seed(&mut self, seed: u64) {
        self.rng = if seed == 0 { RNG_SEED } else { seed };
    }

    fn next_rand(&mut self) -> u64 {
        xorshift(&mut self.rng)
    }

    /// Install the show schedule, the world layout and the left-to-right
    /// order (`order[pos]` is the config index of the screen at `pos`; empty
    /// means config order).
    pub fn set_shows(&mut self, scheduler: ShowScheduler, layout: Layout, order: Vec<usize>) {
        self.shows = scheduler;
        self.show_layout = layout;
        self.show_order = order;
        self.show_frame = None;
    }

    /// Fed by the run loop: true only while the night machine is in `Day`.
    /// On waking the schedule restarts, so the first show comes `every_mins`
    /// after the night rather than at once.
    pub fn set_night_day(&mut self, day: bool) {
        if day && !self.night_day {
            self.show_wake = true;
        }
        self.night_day = day;
        if !day {
            self.show_request = None;
        }
    }

    /// Ask for a show now. Refused (false) at night, with fewer than two
    /// screens or while a show runs; otherwise it starts on the first tick no
    /// screen is mid-crossfade.
    pub fn request_show(&mut self, request: Request) -> bool {
        if !self.night_day || self.screens.len() < 2 || self.shows.current().is_some() {
            return false;
        }
        self.show_request = Some(request);
        true
    }

    /// The running show sequence, if any.
    pub fn show_run(&self) -> Option<&ShowRun> {
        self.shows.current()
    }

    /// Left-to-right position of `screen` in the show world.
    fn show_position(&self, screen: usize) -> u32 {
        self.show_order
            .iter()
            .position(|&c| c == screen)
            .unwrap_or(screen) as u32
    }

    /// The world frame of `run` at `now`: the cached `Arc` when `tick` built
    /// this frame, else a fresh one.
    pub(crate) fn show_world(&self, run: &ShowRun, now: Secs) -> Arc<WorldFrame> {
        let t = run.local_t(now);
        let f = (t * crate::shows::FPS).floor() as u32;
        match &self.show_frame {
            Some((id, ff, a)) if *id == run.def.id && *ff == f => a.clone(),
            _ => Arc::new(crate::shows::render(run.def, self.show_layout, t)),
        }
    }

    /// The world slice's left edge for `screen`.
    pub(crate) fn show_x0(&self, screen: usize) -> u32 {
        self.show_layout.x0(self.show_position(screen))
    }

    /// Start a show when due or requested and the screens are free; on start,
    /// push every dwell back by the whole sequence. Keep the frame cache.
    fn tick_shows(&mut self, now: Secs) {
        if self.screens.len() < 2 {
            self.show_request = None;
        }
        let allowed = self.night_day
            && self.screens.len() >= 2
            && !self.screens.iter().any(|s| s.transitioning());
        let request = if allowed {
            self.show_request.take()
        } else {
            None
        };
        if std::mem::take(&mut self.show_wake) {
            self.shows.restart(now);
        }
        let mut rng = self.rng;
        let started = self
            .shows
            .tick(now, allowed, request, &mut || xorshift(&mut rng));
        self.rng = rng;
        if started {
            let total = self.shows.current().map_or(0.0, |r| r.total());
            for s in &mut self.screens {
                s.extend_dwell(now, total);
            }
        }
        self.show_frame = match self.shows.current().copied() {
            Some(run)
                if matches!(
                    run.phase(now),
                    Some((ShowPhase::Playing | ShowPhase::ShowOut, _))
                ) =>
            {
                let frame = self.show_world(&run, now);
                let f = (run.local_t(now) * crate::shows::FPS).floor() as u32;
                Some((run.def.id, f, frame))
            }
            _ => None,
        };
    }

    /// Advance the deploy day, each screen's cycle and the show schedule.
    /// Call once per frame, before `scene`.
    pub fn tick(&mut self, now: Secs) {
        self.roll_deploy_day();
        // Role cycling pauses while a show runs (the dwells cover it anyway).
        let show_on = self.shows.current().is_some_and(|r| r.phase(now).is_some());
        let mut waiting: Vec<usize> = Vec::new();
        for (i, s) in self.screens.iter_mut().enumerate() {
            if show_on {
                s.advance(now);
            } else if self.one_at_a_time {
                // Finish a running transition; starting one is the slot holder's call.
                s.advance(now);
                if s.ready_to_transition(now) {
                    waiting.push(i);
                }
            } else {
                s.tick(now);
            }
        }
        self.start_one_transition(now, &waiting);
        self.tick_shows(now);
    }

    /// Hand the free transition slot to a random waiting screen. The others keep
    /// their elapsed dwell, so they take the following slots and none starves.
    fn start_one_transition(&mut self, now: Secs, waiting: &[usize]) {
        if waiting.is_empty()
            || now < self.transition_free_at
            || self.screens.iter().any(|s| s.transitioning())
        {
            return;
        }
        let i = waiting[(self.next_rand() % waiting.len() as u64) as usize];
        self.screens[i].start_transition(now);
        self.transition_free_at =
            now + crate::screens::TRANSITION_SECS + crate::screens::TRANSITION_GAP_SECS;
    }

    pub fn all_healthy(&self) -> bool {
        let s = &self.state;
        s.have_nodes && s.nodes_total > 0 && s.nodes_ready == s.nodes_total && s.alerts.is_empty()
    }

    /// True while HEALTH shows torrent rings: a healthy cluster, qBittorrent up
    /// and at least one ring left on screen. A ring that is only unwinding still
    /// counts, so the last torrent plays out before the heart comes back.
    pub fn torrent_mode(&self) -> bool {
        self.all_healthy() && self.link.qbit && !self.state.torrents.is_empty()
    }

    /// The torrents qBittorrent reports right now.
    pub fn torrent_badge_list(&self) -> Vec<&Torrent> {
        self.state.torrents.iter().collect()
    }

    fn fold_torrents(&mut self, list: Vec<Torrent>, _now: Secs) {
        self.state.torrents = list;
    }

    pub fn apply(&mut self, ev: Event, now: Secs) {
        match ev {
            Event::Metrics {
                cpu_pct,
                mem_pct,
                mem_used_gb,
                mem_total_gb,
                hot_cpu: _,
                hot_mem: _,
            } => {
                self.state.cpu_pct = cpu_pct;
                self.state.mem_pct = mem_pct;
                self.state.mem_used_gb = mem_used_gb;
                self.state.mem_total_gb = mem_total_gb;
                self.state.have_metrics = true;
                self.cpu_hist.push(cpu_pct, now);
                self.mem_hist.push(mem_pct, now);
            }
            Event::PodSnapshot {
                running,
                pending,
                failed,
                total,
            } => {
                self.state.pods_running = running;
                self.state.pods_pending = pending;
                self.state.pods_failed = failed;
                self.state.pods_total = total;
                self.state.have_pods = true;
            }
            Event::PodStarted { .. } => {}
            Event::PodCrashed { .. } => {}
            Event::PodGone { .. } => {}
            Event::NodeSnapshot {
                ready,
                total,
                not_ready,
            } => {
                self.state.nodes_ready = ready;
                self.state.nodes_total = total;
                self.state.nodes_not_ready = not_ready;
                self.state.have_nodes = true;
            }
            Event::NodeReady { .. } => {}
            Event::AlertSnapshot { firing } => self.state.alerts = firing,
            Event::AlertChanged { .. } => {}
            Event::NodeTemps(list) => {
                self.state.temps = list;
                self.state.have_temps = true;
            }
            Event::Storage {
                volumes,
                used_bytes,
                capacity_bytes,
            } => {
                self.state.volumes = volumes;
                self.state.storage_used = used_bytes;
                self.state.storage_capacity = capacity_bytes;
                self.state.have_storage = true;
            }
            Event::HotTemp { .. } => {}
            Event::VolumeDegraded { .. } => {}
            Event::VolumeHealthy { .. } => {}
            Event::Electricity {
                zone,
                mix_mw,
                renewable_pct,
                fossil_free_pct,
                updated_at,
            } => {
                let hour = self.unix_hour();
                self.renewable_hist.push(hour, renewable_pct);
                self.electricity = ElectricityState {
                    zone,
                    mix: mix_mw,
                    renewable_pct,
                    fossil_free_pct,
                    updated_at,
                    have: true,
                };
            }
            Event::Ups {
                on_battery,
                low_battery,
                charge_pct,
                load_pct,
                runtime_secs,
            } => {
                self.ups = UpsState {
                    on_battery,
                    low_battery,
                    charge_pct,
                    load_pct,
                    runtime_secs,
                    have: true,
                };
            }
            Event::UpsOnBattery => {}
            Event::UpsOnline => {}
            Event::MetricsHistory {
                cpu,
                mem,
                net_rx_mbps,
            } => {
                self.cpu_hist.prefill(&cpu, now);
                self.mem_hist.prefill(&mem, now);
                self.net_hist.prefill(&net_rx_mbps, now);
            }
            Event::Network { rx_bps, tx_bps } => {
                self.net_hist.push((rx_bps / 1e6) as f32, now);
                self.net = NetState {
                    rx_bps,
                    tx_bps,
                    have: true,
                };
            }
            Event::Torrents(list) => self.fold_torrents(list, now),
            Event::TorrentAdded { .. } => {}
            Event::TorrentDone { .. } => {}
            Event::Link { target, up } => match target {
                LinkTarget::K8sApi => self.link.api = up,
                LinkTarget::Prometheus => self.link.prom = up,
                LinkTarget::QBittorrent => self.link.qbit = up,
                LinkTarget::Electricity => self.link.electricity = up,
                LinkTarget::Weather => self.link.weather = up,
                LinkTarget::Rain => self.link.rain = up,
                LinkTarget::Github => self.link.github = up,
                LinkTarget::ArgoCd => self.link.argocd = up,
            },
            Event::MediaLibrary {
                movies,
                series,
                artists,
            } => {
                self.clear_exporter(Role::Media);
                self.media.movies = movies;
                self.media.series = series;
                self.media.artists = artists;
                self.media.have = true;
            }
            Event::Traffic {
                req_per_sec,
                top,
                service_rps,
                error_share,
            } => {
                self.clear_exporter(Role::Traffic);
                self.traffic_hist.push(req_per_sec, now);
                self.traffic = TrafficState {
                    req_per_sec,
                    top,
                    service_rps,
                    error_share,
                    have: true,
                };
            }
            Event::TrafficHistory { req_per_sec } => self.traffic_hist.prefill(&req_per_sec, now),
            Event::Restarts { total_24h, worst } => {
                self.clear_exporter(Role::Restarts);
                self.restarts.total_24h = total_24h;
                self.restarts.worst = worst;
                self.restarts.have = true;
            }
            Event::RestartsHistory { hourly } => self.restarts_hist.replace(&hourly),
            Event::Uptime(mut nodes) => {
                self.clear_exporter(Role::Uptime);
                nodes.sort_by(|a, b| a.0.cmp(&b.0));
                self.uptime = UptimeState { nodes, have: true };
            }
            Event::Fans(mut fans) => {
                self.clear_exporter(Role::Fans);
                fans.sort_by(|a, b| a.0.cmp(&b.0));
                self.fans = FansState { fans, have: true };
            }
            Event::Mains {
                volts,
                ups_load_pct,
            } => {
                self.clear_exporter(Role::Mains);
                let hour = self.unix_hour();
                self.mains_hist.push(hour, volts);
                self.mains = MainsState {
                    volts,
                    ups_load_pct,
                    have: true,
                };
            }
            Event::MainsHistory { hourly } => self.mains_hist.replace(&hourly),
            Event::Load(mut nodes) => {
                self.clear_exporter(Role::Load);
                nodes.sort_by(|a, b| a.node.cmp(&b.node));
                self.load = LoadState { nodes, have: true };
            }
            Event::Download(d) => self.media.download = d,
            Event::MediaAdded { title } => self.media.last_added = Some((title, now)),
            Event::CiRuns(runs) => {
                self.ci.runs = runs;
                self.ci.have = true;
            }
            Event::Jellyfin { status, sessions } => {
                if status == JfStatus::Ok {
                    if let Some(s) = crate::p2::roles::media::pick_session(&sessions) {
                        self.jellyfin.last_seen =
                            Some((crate::p2::roles::media::display_title(s), self.unix_now));
                    }
                }
                self.jellyfin.status = status;
                self.jellyfin.sessions = sessions;
                self.jellyfin.polled_at = now;
                self.jellyfin.have = true;
            }
            Event::Music { source, track } => {
                match source {
                    crate::event::MusicSource::Spotify => self.music.spotify = track,
                    crate::event::MusicSource::Jellyfin => self.music.jellyfin = track,
                }
                self.music.polled_at[source.index()] = now;
                self.music.have = true;
                if let Some(t) = self.music.current().cloned() {
                    let at = match &self.music.last {
                        Some((prev, at)) if !t.playing && prev.title == t.title => *at,
                        _ => self.unix_now,
                    };
                    self.music.last = Some((t, at));
                }
            }
            Event::NoExporter { role } => {
                if !self.exporter_missing.contains(&role) {
                    self.exporter_missing.push(role);
                }
            }
            Event::ForceNight(v) => self.night_override = v,
            Event::Gif { index, frames } => {
                *self.gif_slot(index) = crate::gif::GifState::Ready(frames);
            }
            Event::GifFailed { index, reason } => {
                let slot = self.gif_slot(index);
                // A GIF that plays keeps playing.
                if !matches!(slot, crate::gif::GifState::Ready(_)) {
                    *slot = crate::gif::GifState::Failed(reason);
                }
            }
            Event::Weather {
                temp_c,
                code,
                is_day,
                wind_kmh,
                gust_kmh,
                wind_from_deg,
                at,
            } => {
                self.weather = WeatherState {
                    temp_c,
                    code,
                    is_day,
                    wind_kmh,
                    gust_kmh,
                    wind_from_deg,
                    at,
                    have: true,
                    ..std::mem::take(&mut self.weather)
                };
            }
            Event::AirQuality { eaqi } => {
                self.air = AirState {
                    eaqi,
                    have: true,
                    ..std::mem::take(&mut self.air)
                };
            }
            Event::Rain { from, mm_per_h } => {
                self.rain = RainState {
                    from,
                    mm_per_h,
                    have: true,
                };
            }
            Event::Sky {
                sunrise,
                sunset,
                sun_elevation_deg,
                moon_illumination,
                moon_waxing,
                moon_phase,
            } => {
                self.sky = SkyState {
                    sunrise,
                    sunset,
                    sun_elevation_deg,
                    moon_illumination,
                    moon_waxing,
                    moon_phase,
                    have: true,
                };
            }
            Event::IssPass(pass) => {
                self.iss = IssState { pass, have: true };
            }
            Event::GithubActivity { days } => {
                self.github = GithubState { days, have: true };
            }
            Event::GithubPush { .. } => {}
            Event::GithubStar { .. } => {}
            Event::GithubMerge { .. } => {}
            Event::GithubRelease { .. } => {}
            Event::GithubRun { .. } => {}
            Event::Apps(list) => {
                self.apps = AppsState {
                    apps: list,
                    have: true,
                };
            }
            Event::AppSynced { name } => {
                self.roll_deploy_day();
                let h = self.local_hour();
                self.deploys.hours[h] += 1;
                self.deploys.last = Some((name, now));
            }
            Event::AppSyncHistory { finished_at } => {
                self.roll_deploy_day();
                let offset = self.utc_offset_secs as i64;
                for t in finished_at {
                    let local = t + offset;
                    if local.div_euclid(86_400) == self.deploys.day {
                        self.deploys.hours[(local.rem_euclid(86_400) / 3600) as usize] += 1;
                    }
                }
            }
            Event::Forecast {
                hourly_temp_c,
                hourly_precip_mm,
                day_min_c,
                day_max_c,
            } => {
                self.weather.hourly_temp_c = hourly_temp_c;
                self.weather.hourly_precip_mm = hourly_precip_mm;
                self.weather.day_min_c = Some(day_min_c);
                self.weather.day_max_c = Some(day_max_c);
            }
            Event::AirQualityHistory { hourly } => self.air.hourly = hourly,
            Event::VolumeSizes(sizes) => self.state.volume_bytes = sizes,
            Event::AppDegraded { .. } => {}
            Event::AppHealthy { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{CiRun, CiStatus, Download, JfKind, JfSession, JfStatus, NodeLoad};

    fn metrics(cpu: f32, hot: Option<(&str, f32)>) -> Event {
        Event::Metrics {
            cpu_pct: cpu,
            mem_pct: 50.0,
            mem_used_gb: 8.0,
            mem_total_gb: 16.0,
            hot_cpu: hot.map(|(n, v)| (n.to_string(), v)),
            hot_mem: None,
        }
    }

    #[test]
    fn metrics_update_state() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(42.0, None), 0.0);
        assert_eq!(m.state().cpu_pct, 42.0);
        assert!(m.state().have_metrics);
    }

    #[test]
    fn storage_fold_computes_used_percentage() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Storage {
                volumes: vec![
                    ("a".into(), Robustness::Healthy),
                    ("b".into(), Robustness::Degraded),
                ],
                used_bytes: 64 << 30,
                capacity_bytes: 128 << 30,
            },
            0.0,
        );
        assert!(m.state().have_storage);
        assert_eq!(m.state().volumes.len(), 2);
        assert_eq!(m.state().storage_capacity, 128 << 30);
        assert_eq!(m.state().storage_used, 64 << 30);
        m.apply(
            Event::Storage {
                volumes: vec![],
                used_bytes: 0,
                capacity_bytes: 0,
            },
            6.0,
        );
        assert_eq!(m.state().storage_capacity, 0);
    }

    #[test]
    fn screens_are_staggered_by_index() {
        let mut m = Model::new(Thresholds::default());
        let roles = vec![Role::Cpu, Role::Mem];
        m.set_screens(vec![roles.clone(), roles.clone(), roles], vec![15.0; 3]);
        let mut started = [None; 3];
        for i in 0..640 {
            let now = i as f64 / 30.0;
            m.tick(now);
            for (k, slot) in started.iter_mut().enumerate() {
                if slot.is_none() && m.screen(k).unwrap().transition(now).is_some() {
                    *slot = Some(i);
                }
            }
        }
        let t: Vec<usize> = started.iter().map(|s| s.expect("transitioned")).collect();
        assert!(t[0] < t[1] && t[1] < t[2], "staggered, got {t:?}");
        assert_eq!(t[1] - t[0], 75, "2.5 s apart at 30 Hz");
        assert_eq!(t[2] - t[1], 75);
        // a short dwell shrinks the step instead of overflowing past a cycle
        let mut short = Model::new(Thresholds::default());
        let roles = vec![Role::Cpu, Role::Mem];
        short.set_screens(vec![roles.clone(), roles.clone(), roles], vec![3.0; 3]);
        for k in 0..3 {
            assert!(short.screen(k).unwrap().transition(0.0).is_none());
        }
    }

    /// Two cycling screens with the same 3 s dwell and no stagger, so both want
    /// to transition on the very same tick: the case serialisation exists for.
    fn two_cycling_screens(one_at_a_time: bool) -> Model {
        let mut m = Model::new(Thresholds::default());
        let roles = vec![Role::Cpu, Role::Mem];
        m.set_screens(vec![roles.clone(), roles], vec![3.0; 2]);
        for s in &mut m.screens {
            s.set_since(0.0);
        }
        m.set_one_at_a_time(one_at_a_time);
        m
    }

    /// Tick at 30 Hz for `secs` and return `(screen, time)` for every transition that
    /// started. With `exclusive`, assert no two screens are ever transitioning together.
    fn run_transitions(m: &mut Model, secs: Secs, exclusive: bool) -> Vec<(usize, Secs)> {
        let mut was_on = vec![false; m.screen_count()];
        let mut starts = Vec::new();
        for i in 0..(secs * 30.0) as usize {
            let now = i as Secs / 30.0;
            m.tick(now);
            let mut live = 0;
            for (k, was) in was_on.iter_mut().enumerate() {
                let on = m.screen(k).unwrap().transition(now).is_some();
                live += on as usize;
                if on && !*was {
                    starts.push((k, now));
                }
                *was = on;
            }
            assert!(
                !exclusive || live <= 1,
                "{live} screens transitioning at {now}"
            );
        }
        starts
    }

    #[test]
    fn one_at_a_time_serialises_transitions_with_a_gap() {
        let mut m = two_cycling_screens(true);
        let starts = run_transitions(&mut m, 10.0, true);
        assert!(starts.iter().any(|(k, _)| *k == 0), "screen 0 got a turn");
        assert!(starts.iter().any(|(k, _)| *k == 1), "screen 1 got a turn");
        for w in starts.windows(2) {
            let gap = w[1].1 - w[0].1 - crate::screens::TRANSITION_SECS;
            assert!(
                gap >= crate::screens::TRANSITION_GAP_SECS - 1e-9,
                "{gap} s between {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn without_one_at_a_time_screens_transition_together() {
        let mut m = two_cycling_screens(false);
        let starts = run_transitions(&mut m, 4.0, false);
        assert_eq!(starts.len(), 2, "both screens, got {starts:?}");
        assert_ne!(starts[0].0, starts[1].0);
        assert_eq!(starts[0].1, starts[1].1, "same tick, as before");
    }

    #[test]
    fn the_waiting_screen_is_picked_at_random() {
        let mut seen = [false; 2];
        for seed in 0..200u64 {
            let mut m = two_cycling_screens(true);
            m.set_rng_seed(seed);
            let starts = run_transitions(&mut m, 4.0, true);
            seen[starts.first().expect("one screen went first").0] = true;
        }
        assert_eq!(seen, [true, true], "both screens win the draw sometimes");
    }

    #[test]
    fn default_layout_is_four_static_screens() {
        let m = Model::new(Thresholds::default());
        assert_eq!(m.screen_count(), 4);
        for (i, r) in [Role::Cpu, Role::Mem, Role::Pods, Role::Health]
            .into_iter()
            .enumerate()
        {
            assert_eq!(m.current_role(i), r);
        }
        assert_eq!(m.current_role(99), Role::Cpu, "out of range falls back");
    }

    #[test]
    fn set_screens_replaces_layout_and_never_leaves_it_empty() {
        let mut m = Model::new(Thresholds::default());
        m.set_screens(
            vec![vec![Role::Thermal, Role::Renewable], vec![]],
            vec![5.0],
        );
        assert_eq!(m.screen_count(), 2);
        assert_eq!(m.current_role(0), Role::Thermal);
        assert_eq!(m.current_role(1), Role::Cpu, "empty list falls back to cpu");
        m.set_screens(vec![], vec![]);
        assert_eq!(m.screen_count(), 1);
        assert!(m.screen(0).is_some());
        assert!(m.screen(1).is_none());
    }

    #[test]
    fn clock_and_presence_flags() {
        let mut m = Model::new(Thresholds::default());
        assert_eq!(m.unix_now(), 0);
        assert_eq!(m.utc_offset_secs(), 0);
        assert!(!m.location_present() && !m.github_token_present());
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        m.set_location_present(true);
        m.set_github_token_present(true);
        assert_eq!(m.unix_now(), 1_788_782_400);
        assert_eq!(m.utc_offset_secs(), 7200);
        assert!(m.location_present() && m.github_token_present());
    }

    #[test]
    fn net_fill_is_a_clamped_log_scale() {
        assert_eq!(net_fill(0.0), 0.03);
        assert_eq!(net_fill(1e5), 0.03);
        assert!((net_fill(1e7) - 0.5).abs() < 1e-6);
        assert_eq!(net_fill(1e9), 1.0);
        assert_eq!(net_fill(5e9), 1.0);
    }

    #[test]
    fn healthy_and_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.all_healthy());
        m.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 3,
                not_ready: vec![],
            },
            0.0,
        );
        assert!(m.all_healthy());
        assert!(!m.torrent_mode());
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![Torrent {
                name: "x".into(),
                progress: 10.0,
                eta_secs: 100,
                speed_bps: 1000,
            }]),
            0.0,
        );
        assert!(m.torrent_mode());
        m.apply(
            Event::AlertSnapshot {
                firing: vec!["Down".into()],
            },
            0.0,
        );
        assert!(!m.all_healthy());
        assert!(!m.torrent_mode());
    }

    #[test]
    fn metrics_and_network_feed_the_histories_and_prefill_fills_them() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(42.0, None), 0.0);
        m.apply(metrics(44.0, None), 2.0);
        assert_eq!(m.cpu_history().series(), vec![42.0, 44.0]);
        assert_eq!(m.mem_history().series(), vec![50.0, 50.0]);
        m.apply(
            Event::Network {
                rx_bps: 40e6,
                tx_bps: 1e6,
            },
            3.0,
        );
        assert_eq!(m.net_history().series(), vec![40.0], "download in Mb/s");
        m.apply(
            Event::MetricsHistory {
                cpu: (0..40).map(|i| i as f32).collect(),
                mem: vec![1.0; 10],
                net_rx_mbps: vec![],
            },
            4.0,
        );
        assert_eq!(m.cpu_history().len(), 31);
        assert_eq!(m.cpu_history().series()[30], 39.0);
        assert_eq!(m.cpu_history().prev(), Some(8.0));
        assert_eq!(m.mem_history().len(), 10);
        assert_eq!(m.net_history().len(), 0, "an empty prefill clears");
    }

    #[test]
    fn electricity_samples_land_in_hourly_slots_by_unix_hour() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 100);
        m.apply(electricity_sample(61.0), 0.0);
        m.set_unix_now(3600 * 1000 + 1800);
        m.apply(electricity_sample(62.0), 1.0);
        assert_eq!(
            m.renewable_hourly().values(),
            vec![62.0],
            "same hour replaces"
        );
        m.set_unix_now(3600 * 1001);
        m.apply(electricity_sample(70.0), 2.0);
        assert_eq!(m.renewable_hourly().values(), vec![62.0, 70.0]);
        assert_eq!(m.unix_hour(), 1001);
    }

    fn electricity_sample(renewable: f32) -> Event {
        Event::Electricity {
            zone: "NL".into(),
            mix_mw: vec![(Source::Wind, 50.0), (Source::Gas, 50.0)],
            renewable_pct: renewable,
            fossil_free_pct: renewable + 5.0,
            updated_at: String::new(),
        }
    }
    #[test]
    fn forecast_air_history_and_volume_sizes_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Forecast {
                hourly_temp_c: vec![13.4, 13.6],
                hourly_precip_mm: vec![0.0, 1.1],
                day_min_c: 10.3,
                day_max_c: 13.6,
            },
            0.0,
        );
        assert_eq!(m.weather().hourly_temp_c, vec![13.4, 13.6]);
        assert_eq!(m.weather().day_min_c, Some(10.3));
        assert!(!m.weather().have, "a forecast alone is not current weather");
        m.apply(
            Event::AirQualityHistory {
                hourly: vec![58.0, 55.0],
            },
            0.0,
        );
        assert_eq!(m.air().hourly, vec![58.0, 55.0]);
        m.apply(Event::VolumeSizes(vec![("pvc-a".into(), 7)]), 0.0);
        assert_eq!(m.state().volume_bytes, vec![("pvc-a".to_string(), 7)]);
    }

    #[test]
    fn deploys_are_counted_per_local_hour_today() {
        let mut m = Model::new(Thresholds::default());
        // 2026-09-07 12:00 UTC in Amsterdam (+2): local 14:00
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        assert_eq!((m.local_hour(), m.local_secs() % 86_400), (14, 14 * 3600));
        m.apply(
            Event::AppSyncHistory {
                finished_at: vec![
                    1_788_782_400 - 3 * 3600,  // 11:00 today
                    1_788_782_400 - 20 * 3600, // 18:00 yesterday
                    1_788_782_400 + 60,        // 14:01 today
                ],
            },
            0.0,
        );
        let log = m.deploys_log();
        assert_eq!(log.hours[11], 1);
        assert_eq!(log.hours[14], 1);
        assert_eq!(log.hours.iter().sum::<u32>(), 2);
        m.apply(
            Event::AppSynced {
                name: "grafana".into(),
            },
            5.0,
        );
        assert_eq!(m.deploys_log().hours[14], 2);
        assert_eq!(m.deploys_log().last, Some(("grafana".to_string(), 5.0)));
        // the next local day empties the buckets but keeps the last sync
        m.set_unix_now(1_788_782_400 + 12 * 3600);
        m.tick(6.0);
        assert_eq!(m.deploys_log().hours.iter().sum::<u32>(), 0);
        assert!(m.deploys_log().last.is_some());
    }
    #[test]
    fn node_temps_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeTemps(vec![("a".into(), 44.0), ("b".into(), 71.0)]),
            0.0,
        );
        assert!(m.state().have_temps);
        assert_eq!(m.state().temps.len(), 2);
    }
    #[test]
    fn no_exporter_flags_a_role_until_cleared() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.exporter_missing(Role::Fans));
        m.apply(Event::NoExporter { role: Role::Fans }, 0.0);
        m.apply(Event::NoExporter { role: Role::Fans }, 1.0);
        assert!(m.exporter_missing(Role::Fans));
        assert!(!m.exporter_missing(Role::Mains));
        m.clear_exporter(Role::Fans);
        assert!(!m.exporter_missing(Role::Fans));
        assert!(!m.arr_configured() && !m.jellyfin_configured());
        m.set_arr_configured(true);
        m.set_jellyfin_configured(true);
        assert!(m.arr_configured() && m.jellyfin_configured());
    }
    fn node_load(node: &str, l1: f32, cores: u32) -> NodeLoad {
        NodeLoad {
            node: node.into(),
            load1: l1,
            load5: l1 * 0.9,
            load15: l1 * 0.8,
            cores,
        }
    }

    #[test]
    fn new_prometheus_events_fill_their_states_and_clear_the_exporter_flag() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 100);
        for role in [
            Role::Media,
            Role::Traffic,
            Role::Restarts,
            Role::Uptime,
            Role::Fans,
            Role::Mains,
            Role::Load,
        ] {
            m.apply(Event::NoExporter { role }, 0.0);
        }
        m.apply(
            Event::MediaLibrary {
                movies: 37,
                series: 30,
                artists: 54,
            },
            0.0,
        );
        assert_eq!(
            (
                m.media().movies,
                m.media().series,
                m.media().artists,
                m.media().have
            ),
            (37, 30, 54, true)
        );
        assert!(!m.exporter_missing(Role::Media));
        m.apply(
            Event::TrafficHistory {
                req_per_sec: (0..40).map(|i| i as f32).collect(),
            },
            0.0,
        );
        assert_eq!(m.traffic_history().len(), 31);
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![("n8n".into(), 2.0), ("grafana".into(), 1.0)],
                service_rps: 4.0,
                error_share: 0.02,
            },
            1.0,
        );
        assert!(m.traffic().have && m.traffic().req_per_sec == 4.2);
        assert_eq!(m.traffic().top[0].0, "n8n");
        assert_eq!(m.traffic_history().series()[30], 4.2);
        assert!(!m.exporter_missing(Role::Traffic));
        m.apply(
            Event::RestartsHistory {
                hourly: (0..24)
                    .map(|i| (1000 - 23 + i, if i == 20 { 7.0 } else { 0.0 }))
                    .collect(),
            },
            1.0,
        );
        assert_eq!(m.restarts_hourly().len(), 24);
        assert_eq!(m.restarts_hourly().last_24(m.unix_hour())[20], Some(7.0));
        m.apply(
            Event::Restarts {
                total_24h: 56,
                worst: Some(("signal-cli".into(), 36)),
            },
            2.0,
        );
        assert_eq!(m.restarts().total_24h, 56);
        assert_eq!(m.restarts().worst, Some(("signal-cli".to_string(), 36)));
        m.apply(
            Event::Uptime(vec![("pi-2".into(), 500.0), ("pi-1".into(), 6_884_104.0)]),
            2.0,
        );
        assert_eq!(m.uptime().nodes[0].0, "pi-1", "sorted by name");
        m.apply(Event::Fans(vec![("pi-1".into(), 49.0)]), 2.0);
        assert!(m.fans().have && m.fans().fans.len() == 1);
        m.apply(
            Event::MainsHistory {
                hourly: (0..24)
                    .map(|i| (1000 - 23 + i, 230.0 + i as f32 * 0.1))
                    .collect(),
            },
            2.0,
        );
        m.apply(
            Event::Mains {
                volts: 234.0,
                ups_load_pct: 7.0,
            },
            2.0,
        );
        assert_eq!(
            (m.mains().volts, m.mains().ups_load_pct, m.mains().have),
            (234.0, 7.0, true)
        );
        assert_eq!(
            m.mains_hourly().values().last(),
            Some(&234.0),
            "the live sample replaces the current hour"
        );
        m.apply(
            Event::Load(vec![node_load("pi-1", 0.81, 4), node_load("hp-1", 0.73, 8)]),
            2.0,
        );
        assert_eq!(m.load().nodes.len(), 2);
        assert_eq!(m.load().nodes[0].node, "hp-1", "sorted by name");
        for role in [
            Role::Media,
            Role::Traffic,
            Role::Restarts,
            Role::Uptime,
            Role::Fans,
            Role::Mains,
            Role::Load,
        ] {
            assert!(!m.exporter_missing(role), "{role:?} cleared by its data");
        }
    }

    #[test]
    fn download_and_added_items_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Download(Some(Download {
                title: "Dune".into(),
                progress: 0.35,
                eta_secs: 1152,
            })),
            0.0,
        );
        assert_eq!(m.media().download.as_ref().unwrap().title, "Dune");
        m.apply(Event::Download(None), 1.0);
        assert!(m.media().download.is_none());
        m.apply(
            Event::MediaAdded {
                title: "Dune".into(),
            },
            2.0,
        );
        assert_eq!(m.media().last_added, Some(("Dune".to_string(), 2.0)));
        assert!(!m.media().have, "the card alone is not library data");
    }
    fn run(id: u64, status: CiStatus, at: i64) -> CiRun {
        CiRun {
            id,
            repo: "silkepilon/RackScreen".into(),
            status,
            created_at: at,
            updated_at: at + 120,
            duration_secs: Some(120),
        }
    }

    #[test]
    fn ci_runs_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        m.apply(
            Event::CiRuns(vec![
                run(2, CiStatus::InProgress, 100),
                run(1, CiStatus::Success, 50),
            ]),
            0.0,
        );
        assert!(m.ci().have);
        assert_eq!(m.ci().runs.len(), 2);
    }
    fn session(title: &str, pos: f64, paused: bool) -> JfSession {
        JfSession {
            title: title.into(),
            kind: JfKind::Movie,
            series: None,
            season: None,
            episode: None,
            year: Some(2017),
            runtime_secs: 9840.0,
            position_secs: pos,
            paused,
            transcoding: false,
            user: "silke".into(),
            device: "TV".into(),
            last_activity: "2026-09-07T12:00:05.000Z".into(),
        }
    }

    #[test]
    fn jellyfin_polls_are_stamped_and_remember_the_last_item() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        assert!(!m.jellyfin().have);
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Ok,
                sessions: vec![session("Blade Runner 2049", 3720.0, false)],
            },
            5.0,
        );
        let j = m.jellyfin();
        assert!(j.have && j.status == JfStatus::Ok);
        assert_eq!((j.sessions.len(), j.polled_at), (1, 5.0));
        assert_eq!(
            j.last_seen,
            Some(("Blade Runner 2049".to_string(), 1_788_782_400))
        );
        m.set_unix_now(1_788_782_400 + 7200);
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Ok,
                sessions: vec![],
            },
            10.0,
        );
        assert_eq!(
            m.jellyfin().last_seen,
            Some(("Blade Runner 2049".to_string(), 1_788_782_400)),
            "kept while idle"
        );
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Offline,
                sessions: vec![],
            },
            11.0,
        );
        assert_eq!(m.jellyfin().status, JfStatus::Offline);
    }

    fn tune(
        source: crate::event::MusicSource,
        title: &str,
        playing: bool,
        since: i64,
    ) -> crate::event::Track {
        crate::event::Track {
            source,
            title: title.into(),
            artist: "Artist".into(),
            device: "Desk".into(),
            duration_secs: 240.0,
            position_secs: 30.0,
            playing,
            since,
            cover: None,
        }
    }

    #[test]
    fn music_polls_are_kept_per_source_and_remember_the_last_track() {
        use crate::event::MusicSource::{Jellyfin, Spotify};
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_000);
        assert!(!m.music().have && !m.music_configured());
        m.set_music_configured(true);
        assert!(m.music_configured());
        m.apply(
            Event::Music {
                source: Spotify,
                track: Some(tune(Spotify, "One", true, 900)),
            },
            5.0,
        );
        m.apply(
            Event::Music {
                source: Jellyfin,
                track: Some(tune(Jellyfin, "Two", false, 950)),
            },
            6.0,
        );
        let mu = m.music();
        assert!(mu.have);
        assert_eq!(mu.spotify.as_ref().unwrap().title, "One");
        assert_eq!(mu.jellyfin.as_ref().unwrap().title, "Two");
        assert_eq!(mu.polled_at, [5.0, 6.0]);
        assert_eq!(mu.current().unwrap().title, "One", "playing beats paused");
        assert_eq!(
            mu.last.as_ref().map(|(t, at)| (t.title.as_str(), *at)),
            Some(("One", 1_000))
        );
        m.set_unix_now(1_300);
        m.apply(
            Event::Music {
                source: Spotify,
                track: Some(tune(Spotify, "One", false, 1_290)),
            },
            8.0,
        );
        assert_eq!(
            m.music()
                .last
                .as_ref()
                .map(|(t, at)| (t.title.as_str(), *at)),
            Some(("One", 1_000)),
            "paused keeps the time it was last heard"
        );
        m.set_unix_now(1_400);
        m.apply(
            Event::Music {
                source: Jellyfin,
                track: None,
            },
            9.0,
        );
        m.apply(
            Event::Music {
                source: Spotify,
                track: None,
            },
            9.0,
        );
        assert!(m.music().current().is_none());
        assert_eq!(
            m.music()
                .last
                .as_ref()
                .map(|(t, at)| (t.title.as_str(), *at)),
            Some(("One", 1_000)),
            "kept while idle"
        );
    }

    fn cycling_show_model() -> Model {
        use crate::shows::{schedule::ShowScheduler, world::Layout, SHOWS};
        let mut m = Model::new(Thresholds::default());
        let roles = vec![Role::Cpu, Role::Mem];
        m.set_screens(vec![roles.clone(), roles.clone(), roles], vec![15.0; 3]);
        m.set_one_at_a_time(true);
        m.set_shows(
            ShowScheduler::new(true, 2, SHOWS.iter().collect()),
            Layout { panels: 3, gap: 46 },
            vec![],
        );
        m
    }

    #[test]
    fn a_scheduled_show_pauses_role_cycling_and_extends_every_dwell() {
        let mut m = cycling_show_model();
        let mut run = None;
        let mut total = 0.0;
        let mut last_role_change = 0.0;
        let mut changes_after = Vec::new();
        for i in 0..(200 * 30) {
            let now = i as Secs / 30.0;
            m.tick(now);
            let transitioning = (0..3).any(|k| m.screen(k).unwrap().transitioning());
            if transitioning {
                last_role_change = now;
            }
            if let Some(r) = m.show_run() {
                assert!(run.is_none() || run == Some(r.started), "one run");
                if run.is_none() {
                    assert!(now >= 120.0, "due after every_mins, got {now}");
                    assert!(now - last_role_change > 0.0, "not mid-transition");
                }
                run = Some(r.started);
                total = r.total();
                assert!(!transitioning, "no role change during a show at {now}");
            } else if let Some(start) = run {
                if transitioning && changes_after.is_empty() {
                    changes_after.push(now - (start + total));
                }
            }
        }
        assert!(run.is_some(), "a show ran");
        let gap = changes_after
            .first()
            .copied()
            .expect("roles cycle again afterwards");
        assert!(
            gap >= crate::screens::MIN_CYCLE_SECS - 1e-6,
            "no role change right after the show ({gap} s)"
        );
    }

    #[test]
    fn the_first_show_after_the_night_waits_every_mins() {
        // every_mins is 2: one is long overdue when the night ends at 1000 s
        let mut m = cycling_show_model();
        m.set_night_day(false);
        for i in 0..10_000 {
            m.tick(i as Secs / 10.0);
        }
        assert!(m.show_run().is_none());
        m.set_night_day(true);
        let mut first = None;
        for i in 10_000..(10_000 + 200 * 10) {
            let now = i as Secs / 10.0;
            m.set_night_day(true);
            m.tick(now);
            if first.is_none() {
                first = m.show_run().map(|r| r.started);
            }
        }
        let first = first.expect("a show once every_mins has passed");
        assert!(
            (1120.0..1125.0).contains(&first),
            "every_mins after waking, got {first}"
        );
    }

    #[test]
    fn night_falling_mid_show_lets_the_run_finish() {
        use crate::shows::schedule::{Request, ShowPhase};
        let freight = || Request::Id(crate::shows::by_id("freight").unwrap());
        let mut m = cycling_show_model();
        m.tick(0.0);
        assert!(m.request_show(freight()));
        let mut now = 0.0;
        while !matches!(
            m.show_run().and_then(|r| r.phase(now)),
            Some((ShowPhase::Playing, _))
        ) {
            now += 0.05;
            m.tick(now);
            assert!(now < 5.0, "never reached Playing");
        }
        let run = *m.show_run().unwrap();
        m.set_night_day(false);
        assert!(!m.request_show(freight()), "a night request is dropped");
        let end = run.started + run.total();
        // the run plays out unchanged
        while now < end - 0.05 {
            now += 0.05;
            m.tick(now);
            assert_eq!(m.show_run().map(|r| r.started), Some(run.started));
        }
        // and nothing new while it is not day, though one is due (2 min)
        while now < end + 600.0 {
            now += 0.5;
            m.tick(now);
            if now > end + 0.1 {
                assert!(m.show_run().is_none(), "no run at night ({now})");
            }
        }
        assert!(!m.request_show(freight()), "still dropped at night");
    }

    #[test]
    fn a_request_during_a_running_show_is_refused() {
        use crate::shows::schedule::Request;
        let freight = || Request::Id(crate::shows::by_id("freight").unwrap());
        let mut m = cycling_show_model();
        m.tick(0.0);
        assert!(m.request_show(freight()));
        m.tick(0.1);
        let run = *m.show_run().expect("the request started a show");
        assert!(!m.request_show(freight()), "refused mid-show");
        let end = run.started + run.total();
        m.tick(end + 0.01);
        assert!(m.show_run().is_none(), "nothing was kept for later");
        m.tick(end + 1.0);
        assert!(m.show_run().is_none());
        assert!(m.request_show(freight()), "accepted again once it is over");
    }

    #[test]
    fn no_show_at_night_with_one_screen_or_mid_transition() {
        use crate::shows::schedule::Request;
        let freight = || Request::Id(crate::shows::by_id("freight").unwrap());
        // night: the request is refused and nothing is scheduled
        let mut m = cycling_show_model();
        m.set_night_day(false);
        assert!(!m.request_show(freight()));
        for i in 0..(300 * 10) {
            m.tick(i as Secs / 10.0);
            assert!(m.show_run().is_none());
        }
        m.set_night_day(true);
        m.tick(300.1);
        assert!(
            m.show_run().is_none(),
            "not straight after the night, though one is overdue"
        );
        // one screen: never
        let mut one = cycling_show_model();
        one.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![15.0]);
        assert!(!one.request_show(freight()));
        one.tick(500.0);
        assert!(one.show_run().is_none());
        // mid-transition: the request waits for the crossfade to end
        let mut m = cycling_show_model();
        for s in &mut m.screens {
            s.set_since(-100.0);
        }
        m.tick(0.0);
        assert!(
            m.screen(0).unwrap().transitioning()
                || m.screen(1).unwrap().transitioning()
                || m.screen(2).unwrap().transitioning()
        );
        assert!(m.request_show(freight()));
        m.tick(0.1);
        assert!(m.show_run().is_none(), "waits for the crossfade");
        m.tick(0.5);
        assert_eq!(m.show_run().map(|r| r.started), Some(0.5));
    }
}

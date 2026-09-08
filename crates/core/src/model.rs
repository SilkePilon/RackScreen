//! Cluster state and the fold of events into it.

use std::collections::{HashMap, HashSet};

use crate::anim::{Secs, Smooth};
use crate::electricity::{partition, Section, Source};
use crate::event::{App, Event, IssPass, LinkTarget, MoonPhase, Robustness, Torrent};
use crate::fx::Fx;
use crate::scene_rain::{current_slots, first_wet_minutes, SOON_MINUTES, WET_MM};
use crate::screens::ScreenState;
use crate::theme::eaqi_band;
use crate::theme::layout::{SEG_N, TORRENT_RADII};
use crate::theme::Role;

pub const SMOOTH_SECS: Secs = 0.8;
const HOT_DEBOUNCE_SECS: Secs = 300.0;
/// Dwell per role on a cycling screen when the caller gives none.
pub const DEFAULT_CYCLE_SECS: Secs = 15.0;
/// Seed the transition picker starts from; `set_rng_seed` replaces it per boot.
const RNG_SEED: u64 = 0x2545_F491_4F6C_DD1D;
/// Extra first dwell per screen index, so equal dwells do not iris together.
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
    pub prices: bool,
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
    pub carbon_gco2: f32,
    pub updated_at: String,
    pub have: bool,
}

/// Day-ahead prices, one entry per local hour starting at 00:00.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PriceState {
    pub date: String,
    pub ct: Vec<f32>,
    pub currency: String,
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AirState {
    pub eaqi: f32,
    pub have: bool,
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

/// WMO codes 95, 96 and 99 are thunderstorms.
fn is_thunder(code: u16) -> bool {
    (95..=99).contains(&code)
}

/// Ring fill for a bit rate: 100 kbit/s is (almost) empty, 1 Gbit/s is full,
/// four decades in between, and a floor so idle still shows a little.
pub fn net_fill(bps: f64) -> f32 {
    if bps <= 0.0 {
        return 0.03;
    }
    (((bps / 1e5).log10() / 4.0) as f32).clamp(0.03, 1.0)
}

/// Laps per second of the net pulse at full fill (one lap every 6 s). The
/// fill is floored at 0.1 when it drives the phase, so the slowest lap is
/// 60 s: the plan's global 6..60 s constraint, which the 0.03 idle floor
/// would otherwise stretch to 200 s.
const NET_LAPS_PER_SEC: f64 = 1.0 / 6.0;

/// A smoothed share below this is treated as gone: no segment, no icon.
const SHARE_EPS: f32 = 0.05;

/// An unwinding torrent ring below this percent is finished and is dropped.
const TORRENT_GONE_PCT: f32 = 0.5;
/// How long a torrent ring takes to slide to another radius slot.
const TORRENT_SLOT_SECS: Secs = 0.4;

/// One animated torrent ring: eased progress and radius, plus the values last
/// reported for it so the badge can keep summarising a ring that is unwinding.
#[derive(Clone, Debug)]
struct TorrentAnim {
    /// Torrent name: the identity that survives across updates.
    name: String,
    progress: Smooth,
    radius: Smooth,
    /// Index into `TORRENT_RADII`, or `None` while the entry waits for a slot.
    slot: Option<usize>,
    /// The torrent is gone from qBittorrent; its ring is unwinding to zero.
    leaving: bool,
    /// The last values reported, for the badge once the live list is empty.
    last: Torrent,
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
}

/// What the fold wants the animation layer to do. Consumed by Task 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FxRequest {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode(Role),
    HotTemp,
    VolumeDegraded,
    VolumeHealthy,
    TorrentAdded,
    TorrentDone,
    NodeNotReady,
    NodeReady,
    AlertFiring,
    AlertResolved,
    LinkUp,
    Boot,
    GithubPush,
    GithubStar,
    GithubMerge,
    GithubRelease,
    GithubRunFailed,
    GithubRunPassed,
    Thunder,
    RainSoon,
    AirWorse,
    AppSynced,
    AppDegraded,
    AppHealthy,
    UpsOnBattery,
    UpsOnline,
    IssPass,
}

pub struct Model {
    state: ClusterState,
    link: LinkState,
    thresholds: Thresholds,
    cpu: Smooth,
    mem: Smooth,
    pods: Smooth,
    hot_temp: Smooth,
    storage_pct: Smooth,
    electricity: ElectricityState,
    prices: PriceState,
    ups: UpsState,
    net: NetState,
    apps: AppsState,
    github: GithubState,
    weather: WeatherState,
    air: AirState,
    rain: RainState,
    /// Minutes to rain as of the previous nowcast, for the umbrella edge.
    rain_prev_wet: Option<u32>,
    sky: SkyState,
    iss: IssState,
    /// Start time of the pass whose sweep has played.
    iss_swept: Option<i64>,
    temp: Smooth,
    eaqi: Smooth,
    wind: Smooth,
    ups_charge: Smooth,
    ups_load: Smooth,
    /// Today's contribution count, eased.
    gh_today: Smooth,
    /// Ring fill (0..1) for download and upload, eased.
    net_rx: Smooth,
    net_tx: Smooth,
    /// Position of the travelling pulse on each net ring, in laps (fractional).
    net_rx_phase: f64,
    net_tx_phase: f64,
    net_last_tick: Option<Secs>,
    /// Percent of total production per source, eased.
    shares: HashMap<Source, Smooth>,
    renewable: Smooth,
    fossil_free: Smooth,
    carbon: Smooth,
    /// Rotation of the power-mix ring in degrees; non-zero only while a new
    /// leader is being turned to 12 o'clock.
    mix_start: Smooth,
    /// Leader and full partition as last drawn, to know where the new leader
    /// was before it took the lead.
    mix_leader: Option<Source>,
    mix_sections: Vec<Section>,
    /// An Electricity Maps token is configured; without one the electricity
    /// roles show a key instead of the offline cloud.
    token_present: bool,
    /// Local wall-clock hour, set by the render loop.
    local_hour: u32,
    /// Unix seconds, set by the render loop every frame; the sun dial, rain
    /// ring and ISS countdown are clock-based.
    unix_now: i64,
    /// Seconds east of UTC for the Pi's local zone.
    utc_offset_secs: i32,
    /// `location.lat`/`lon` are set; without them the sky roles show a pin.
    location_present: bool,
    /// A GitHub token is set; without one `gh-activity` shows a key.
    github_token_present: bool,
    hot_last: HashMap<(Role, String), Secs>,
    hot_temp_last: HashMap<String, Secs>,
    night_override: Option<bool>,
    fx: Vec<FxRequest>,
    fx_state: Fx,
    /// One animated ring per torrent, alive until its ring has unwound.
    torrent_anims: Vec<TorrentAnim>,
    /// One entry per physical screen, top to bottom.
    screens: Vec<ScreenState>,
    /// Only one screen may iris at a time, in random order: simultaneous
    /// transitions on the Pi make four displays fight over one SPI bus.
    one_at_a_time: bool,
    /// While serialising: the instant the next transition may start, i.e. the
    /// end of the last one plus `TRANSITION_GAP_SECS`.
    transition_free_at: Secs,
    /// xorshift64 state, for picking which waiting screen goes next.
    rng: u64,
    seen_api_up: bool,
    /// A `Boot` that arrived while the API link was down; it plays on first connect
    /// so the sweep is not aged out behind the connecting scene.
    boot_pending: bool,
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
                prices: false,
                weather: false,
                rain: false,
                github: false,
                argocd: false,
            },
            thresholds,
            cpu: Smooth::new(0.0, SMOOTH_SECS),
            mem: Smooth::new(0.0, SMOOTH_SECS),
            pods: Smooth::new(0.0, SMOOTH_SECS),
            hot_temp: Smooth::new(0.0, SMOOTH_SECS),
            storage_pct: Smooth::new(0.0, SMOOTH_SECS),
            electricity: ElectricityState::default(),
            prices: PriceState::default(),
            ups: UpsState::default(),
            net: NetState::default(),
            apps: AppsState::default(),
            github: GithubState::default(),
            weather: WeatherState::default(),
            air: AirState::default(),
            rain: RainState::default(),
            rain_prev_wet: None,
            sky: SkyState::default(),
            iss: IssState::default(),
            iss_swept: None,
            temp: Smooth::new(0.0, SMOOTH_SECS),
            eaqi: Smooth::new(0.0, SMOOTH_SECS),
            wind: Smooth::new(0.0, SMOOTH_SECS),
            ups_charge: Smooth::new(0.0, SMOOTH_SECS),
            ups_load: Smooth::new(0.0, SMOOTH_SECS),
            gh_today: Smooth::new(0.0, SMOOTH_SECS),
            net_rx: Smooth::new(0.03, SMOOTH_SECS),
            net_tx: Smooth::new(0.03, SMOOTH_SECS),
            net_rx_phase: 0.0,
            net_tx_phase: 0.0,
            net_last_tick: None,
            shares: HashMap::new(),
            renewable: Smooth::new(0.0, SMOOTH_SECS),
            fossil_free: Smooth::new(0.0, SMOOTH_SECS),
            carbon: Smooth::new(0.0, SMOOTH_SECS),
            mix_start: Smooth::new(0.0, SMOOTH_SECS),
            mix_leader: None,
            mix_sections: Vec::new(),
            token_present: false,
            local_hour: 12,
            unix_now: 0,
            utc_offset_secs: 0,
            location_present: false,
            github_token_present: false,
            hot_last: HashMap::new(),
            hot_temp_last: HashMap::new(),
            night_override: None,
            fx: Vec::new(),
            fx_state: Fx::default(),
            torrent_anims: Vec::new(),
            screens: [Role::Cpu, Role::Mem, Role::Pods, Role::Health]
                .into_iter()
                .map(|r| ScreenState::new(vec![r], DEFAULT_CYCLE_SECS))
                .collect(),
            one_at_a_time: false,
            transition_free_at: 0.0,
            rng: RNG_SEED,
            seen_api_up: false,
            boot_pending: false,
        }
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
    pub fn smooth_cpu(&self, now: Secs) -> f32 {
        self.cpu.value(now)
    }
    pub fn smooth_mem(&self, now: Secs) -> f32 {
        self.mem.value(now)
    }
    pub fn smooth_pods(&self, now: Secs) -> f32 {
        self.pods.value(now)
    }
    pub fn smooth_hot_temp(&self, now: Secs) -> f32 {
        self.hot_temp.value(now)
    }
    pub fn smooth_storage_pct(&self, now: Secs) -> f32 {
        self.storage_pct.value(now)
    }
    pub fn electricity(&self) -> &ElectricityState {
        &self.electricity
    }
    pub fn prices(&self) -> &PriceState {
        &self.prices
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
    pub fn smooth_gh_today(&self, now: Secs) -> f32 {
        self.gh_today.value(now)
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
    pub fn smooth_temp(&self, now: Secs) -> f32 {
        self.temp.value(now)
    }
    pub fn smooth_eaqi(&self, now: Secs) -> f32 {
        self.eaqi.value(now)
    }
    pub fn smooth_wind(&self, now: Secs) -> f32 {
        self.wind.value(now)
    }
    pub fn smooth_ups_charge(&self, now: Secs) -> f32 {
        self.ups_charge.value(now)
    }
    pub fn smooth_ups_load(&self, now: Secs) -> f32 {
        self.ups_load.value(now)
    }
    pub fn smooth_net_rx(&self, now: Secs) -> f32 {
        self.net_rx.value(now)
    }
    pub fn smooth_net_tx(&self, now: Secs) -> f32 {
        self.net_tx.value(now)
    }
    /// Pulse positions (download, upload) in laps, 0..1.
    pub fn net_phases(&self) -> (f32, f32) {
        (self.net_rx_phase as f32, self.net_tx_phase as f32)
    }
    /// Eased share of total production for one source, in percent.
    pub fn smooth_share(&self, source: Source, now: Secs) -> f32 {
        self.shares
            .get(&source)
            .map(|s| s.value(now))
            .unwrap_or(0.0)
    }
    /// Every source still worth drawing, in `Source::ALL` order.
    pub fn smooth_shares(&self, now: Secs) -> Vec<(Source, f32)> {
        let mut out: Vec<(Source, f32)> = self
            .shares
            .iter()
            .map(|(s, sm)| (*s, sm.value(now)))
            .filter(|(_, v)| *v >= SHARE_EPS)
            .collect();
        out.sort_by_key(|(s, _)| s.index());
        out
    }
    pub fn smooth_renewable(&self, now: Secs) -> f32 {
        self.renewable.value(now)
    }
    pub fn smooth_fossil_free(&self, now: Secs) -> f32 {
        self.fossil_free.value(now)
    }
    pub fn smooth_carbon(&self, now: Secs) -> f32 {
        self.carbon.value(now)
    }
    /// Degrees the power-mix ring (icons and ticks included) is turned
    /// clockwise from its natural leader-at-12-o'clock position. Zero unless a
    /// leader change is being animated.
    pub fn smooth_mix_start(&self, now: Secs) -> f32 {
        self.mix_start.value(now)
    }
    pub fn set_token_present(&mut self, present: bool) {
        self.token_present = present;
    }
    pub fn token_present(&self) -> bool {
        self.token_present
    }
    /// The local wall-clock hour (0..=23) the price ring marks as "now".
    pub fn set_local_hour(&mut self, h: u32) {
        self.local_hour = h.min(23);
    }
    pub fn local_hour(&self) -> u32 {
        self.local_hour
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
    pub fn set_github_token_present(&mut self, present: bool) {
        self.github_token_present = present;
    }
    pub fn github_token_present(&self) -> bool {
        self.github_token_present
    }
    pub fn pending_fx(&self) -> &[FxRequest] {
        &self.fx
    }
    pub fn take_fx(&mut self) -> Vec<FxRequest> {
        std::mem::take(&mut self.fx)
    }

    pub fn fx(&self) -> &Fx {
        &self.fx_state
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
                // that share a dwell never iris on the same frame again.
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

    /// Serialise iris transitions: at most one screen at a time, the next one
    /// picked at random from those whose dwell has elapsed.
    pub fn set_one_at_a_time(&mut self, on: bool) {
        self.one_at_a_time = on;
    }

    /// Reseed the picker (zero is bumped, xorshift dies on it).
    pub fn set_rng_seed(&mut self, seed: u64) {
        self.rng = if seed == 0 { RNG_SEED } else { seed };
    }

    /// xorshift64: a whole PRNG in three shifts, so `core` keeps no rand dependency.
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Drain animation requests into the queues and advance them, then advance
    /// each screen's cycle. Call once per frame.
    pub fn tick(&mut self, now: Secs) {
        // sources that left the mix ease to zero, then stop costing anything
        self.shares
            .retain(|_, sm| sm.target() > 0.0 || sm.value(now) >= SHARE_EPS);
        // rings that have finished unwinding leave, and the rest close ranks
        let before = self.torrent_anims.len();
        self.torrent_anims
            .retain(|a| !a.leaving || a.progress.value(now) >= TORRENT_GONE_PCT);
        if self.torrent_anims.len() != before {
            self.reslot_torrents(now);
        }
        self.track_mix_leader(now);
        let dt = self.net_last_tick.map_or(0.0, |l| (now - l).max(0.0));
        self.net_last_tick = Some(now);
        self.net_rx_phase = (self.net_rx_phase
            + dt * self.net_rx.value(now).max(0.1) as f64 * NET_LAPS_PER_SEC)
            .fract();
        self.net_tx_phase = (self.net_tx_phase
            + dt * self.net_tx.value(now).max(0.1) as f64 * NET_LAPS_PER_SEC)
            .fract();
        if let Some(p) = self.iss.pass {
            let inside = (p.start..p.end).contains(&self.unix_now);
            if p.visible && inside && self.iss_swept != Some(p.start) {
                self.iss_swept = Some(p.start);
                self.fx.push(FxRequest::IssPass);
            }
        }
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        let screens = self.screens.len();
        self.fx_state.tick(now, screens);
        let sweep_active = self.fx_state.sweeps.active().is_some();
        let mut waiting: Vec<usize> = Vec::new();
        for (i, s) in self.screens.iter_mut().enumerate() {
            let busy = sweep_active
                || self.fx_state.splashes[s.current().index()]
                    .active()
                    .is_some();
            if self.one_at_a_time {
                // Finish a running iris; starting one is the slot holder's call.
                s.advance(now);
                if s.ready_to_transition(now, busy) {
                    waiting.push(i);
                }
            } else {
                s.tick(now, busy);
            }
        }
        self.start_one_transition(now, &waiting);
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

    /// When the drawn partition gets a new leader, `partition` moves that
    /// source to segment 0 at once. Compensate by turning the whole ring so the
    /// new leader stays where it was, then ease the turn back to zero: the ring
    /// rotates the new leader up to 12 o'clock over `SMOOTH_SECS`.
    fn track_mix_leader(&mut self, now: Secs) {
        let sections = partition(&self.smooth_shares(now), SEG_N);
        let leader = sections.first().map(|s| s.source);
        if let (Some(new), Some(old)) = (leader, self.mix_leader) {
            if new != old {
                if let Some(prev) = self.mix_sections.iter().find(|s| s.source == new) {
                    let pitch = 360.0 / SEG_N as f32;
                    let deg = prev.start as f32 * pitch + self.mix_start.value(now);
                    // shortest way round: (-180, 180]
                    let deg = deg.rem_euclid(360.0);
                    let deg = if deg > 180.0 { deg - 360.0 } else { deg };
                    self.mix_start = Smooth::new(deg, SMOOTH_SECS);
                    self.mix_start.set(0.0, now);
                }
            }
        }
        self.mix_leader = leader;
        self.mix_sections = sections;
    }

    pub fn all_healthy(&self) -> bool {
        let s = &self.state;
        s.have_nodes && s.nodes_total > 0 && s.nodes_ready == s.nodes_total && s.alerts.is_empty()
    }

    /// True while HEALTH shows torrent rings: a healthy cluster, qBittorrent up
    /// and at least one ring left on screen. A ring that is only unwinding still
    /// counts, so the last torrent plays out before the heart comes back.
    pub fn torrent_mode(&self) -> bool {
        self.all_healthy() && self.link.qbit && !self.torrent_anims.is_empty()
    }

    /// The torrent rings to draw, outermost first: radius, eased progress in
    /// percent and the index into the accent palette.
    pub fn torrent_rings(&self, now: Secs) -> Vec<(f32, f32, usize)> {
        let mut out: Vec<(f32, f32, usize)> = self
            .torrent_anims
            .iter()
            .filter_map(|a| {
                a.slot
                    .map(|slot| (a.radius.value(now), a.progress.value(now), slot))
            })
            .collect();
        out.sort_by_key(|(_, _, slot)| *slot);
        out
    }

    /// The torrents the HEALTH badge summarises: the live list, or the last
    /// known values of the entries that are still unwinding once it is empty.
    pub fn torrent_badge_list(&self) -> Vec<&Torrent> {
        if !self.state.torrents.is_empty() {
            return self.state.torrents.iter().collect();
        }
        self.torrent_anims.iter().map(|a| &a.last).collect()
    }

    /// Fold a torrent list into the animated entries: retarget the ones still
    /// there, sweep new ones in from zero and unwind the ones that vanished.
    fn fold_torrents(&mut self, list: Vec<Torrent>, now: Secs) {
        for t in &list {
            match self.torrent_anims.iter_mut().find(|a| a.name == t.name) {
                Some(a) => {
                    a.leaving = false;
                    a.progress.set(t.progress, now);
                    a.last = t.clone();
                }
                None => {
                    let mut progress = Smooth::new(0.0, SMOOTH_SECS);
                    progress.set(t.progress, now);
                    self.torrent_anims.push(TorrentAnim {
                        name: t.name.clone(),
                        progress,
                        radius: Smooth::new(0.0, TORRENT_SLOT_SECS),
                        slot: None,
                        leaving: false,
                        last: t.clone(),
                    });
                }
            }
        }
        for a in self.torrent_anims.iter_mut() {
            if !a.leaving && !list.iter().any(|t| t.name == a.name) {
                a.leaving = true;
                a.progress.set(0.0, now);
            }
        }
        self.state.torrents = list;
        self.reslot_torrents(now);
    }

    /// Hand out radius slots. While anything is unwinding the entries on screen
    /// keep the slot they have and only free slots are handed out; otherwise the
    /// three most complete torrents take the slots from the outside in.
    fn reslot_torrents(&mut self, now: Secs) {
        let mut want: Vec<Option<usize>> = self.torrent_anims.iter().map(|a| a.slot).collect();
        if self.torrent_anims.iter().any(|a| a.leaving) {
            let mut taken = [false; TORRENT_RADII.len()];
            for slot in want.iter().flatten() {
                taken[*slot] = true;
            }
            for i in self.torrents_by_progress() {
                if want[i].is_some() {
                    continue;
                }
                if let Some(free) = taken.iter().position(|t| !t) {
                    taken[free] = true;
                    want[i] = Some(free);
                }
            }
        } else {
            want = vec![None; self.torrent_anims.len()];
            for (slot, i) in self
                .torrents_by_progress()
                .into_iter()
                .take(TORRENT_RADII.len())
                .enumerate()
            {
                want[i] = Some(slot);
            }
        }
        for (i, slot) in want.into_iter().enumerate() {
            let a = &mut self.torrent_anims[i];
            match (a.slot, slot) {
                (_, None) => a.slot = None,
                (Some(prev), Some(next)) if prev == next => {}
                (Some(_), Some(next)) => {
                    a.radius.set(TORRENT_RADII[next], now);
                    a.slot = Some(next);
                }
                // A ring nobody has seen yet starts at its slot instead of
                // sliding in from wherever the entry was last drawn.
                (None, Some(next)) => {
                    a.radius = Smooth::new(TORRENT_RADII[next], TORRENT_SLOT_SECS);
                    a.slot = Some(next);
                }
            }
        }
    }

    /// Entry indices by target progress, most complete first. The name breaks
    /// ties so the ring order never depends on the order of an update.
    fn torrents_by_progress(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.torrent_anims.len()).collect();
        idx.sort_by(|&a, &b| {
            let (a, b) = (&self.torrent_anims[a], &self.torrent_anims[b]);
            b.progress
                .target()
                .partial_cmp(&a.progress.target())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });
        idx
    }

    pub fn apply(&mut self, ev: Event, now: Secs) {
        match ev {
            Event::Metrics {
                cpu_pct,
                mem_pct,
                mem_used_gb,
                mem_total_gb,
                hot_cpu,
                hot_mem,
            } => {
                self.state.cpu_pct = cpu_pct;
                self.state.mem_pct = mem_pct;
                self.state.mem_used_gb = mem_used_gb;
                self.state.mem_total_gb = mem_total_gb;
                self.state.have_metrics = true;
                self.cpu.set(cpu_pct, now);
                self.mem.set(mem_pct, now);
                let th = self.thresholds;
                self.check_hot(Role::Cpu, hot_cpu, th.hot_cpu, now);
                self.check_hot(Role::Mem, hot_mem, th.hot_mem, now);
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
                self.pods.set(running as f32, now);
            }
            Event::PodStarted { .. } => self.fx.push(FxRequest::PodStarted),
            Event::PodCrashed { .. } => self.fx.push(FxRequest::PodCrashed),
            Event::PodGone { .. } => self.fx.push(FxRequest::PodGone),
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
            Event::NodeReady { ready, .. } => {
                self.fx.push(if ready {
                    FxRequest::NodeReady
                } else {
                    FxRequest::NodeNotReady
                });
            }
            Event::AlertSnapshot { firing } => self.state.alerts = firing,
            Event::AlertChanged { firing, .. } => {
                self.fx.push(if firing {
                    FxRequest::AlertFiring
                } else {
                    FxRequest::AlertResolved
                });
            }
            Event::NodeTemps(list) => {
                let hottest = list.iter().map(|(_, c)| *c).fold(0.0_f32, f32::max);
                self.hot_temp.set(hottest, now);
                let th = self.thresholds.hot_temp;
                for (node, c) in &list {
                    if *c >= th {
                        let recently = self
                            .hot_temp_last
                            .get(node)
                            .is_some_and(|t| now - t < HOT_DEBOUNCE_SECS);
                        if !recently {
                            self.hot_temp_last.insert(node.clone(), now);
                            self.fx.push(FxRequest::HotTemp);
                        }
                    }
                }
                self.state.temps = list;
                self.state.have_temps = true;
            }
            Event::Storage {
                volumes,
                used_bytes,
                capacity_bytes,
            } => {
                let pct = if capacity_bytes > 0 {
                    used_bytes as f32 / capacity_bytes as f32 * 100.0
                } else {
                    0.0
                };
                self.storage_pct.set(pct, now);
                self.state.volumes = volumes;
                self.state.storage_used = used_bytes;
                self.state.storage_capacity = capacity_bytes;
                self.state.have_storage = true;
            }
            Event::HotTemp { .. } => self.fx.push(FxRequest::HotTemp),
            Event::VolumeDegraded { .. } => self.fx.push(FxRequest::VolumeDegraded),
            Event::VolumeHealthy { .. } => self.fx.push(FxRequest::VolumeHealthy),
            Event::Electricity {
                zone,
                mix_mw,
                renewable_pct,
                fossil_free_pct,
                carbon_gco2,
                updated_at,
            } => {
                let total: f32 = mix_mw.iter().map(|(_, mw)| mw.max(0.0)).sum();
                for (src, mw) in &mix_mw {
                    let pct = if total > 0.0 {
                        mw.max(0.0) / total * 100.0
                    } else {
                        0.0
                    };
                    self.shares
                        .entry(*src)
                        .or_insert_with(|| Smooth::new(0.0, SMOOTH_SECS))
                        .set(pct, now);
                }
                let present: HashSet<Source> = mix_mw.iter().map(|(s, _)| *s).collect();
                for (src, sm) in self.shares.iter_mut() {
                    if !present.contains(src) {
                        sm.set(0.0, now);
                    }
                }
                self.renewable.set(renewable_pct, now);
                self.fossil_free.set(fossil_free_pct, now);
                self.carbon.set(carbon_gco2, now);
                self.electricity = ElectricityState {
                    zone,
                    mix: mix_mw,
                    renewable_pct,
                    fossil_free_pct,
                    carbon_gco2,
                    updated_at,
                    have: true,
                };
            }
            Event::Prices {
                date,
                ct_per_kwh,
                currency,
            } => {
                self.prices = PriceState {
                    date,
                    ct: ct_per_kwh,
                    currency,
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
                self.ups_charge.set(charge_pct, now);
                self.ups_load.set(load_pct, now);
                self.ups = UpsState {
                    on_battery,
                    low_battery,
                    charge_pct,
                    load_pct,
                    runtime_secs,
                    have: true,
                };
            }
            Event::UpsOnBattery => self.fx.push(FxRequest::UpsOnBattery),
            Event::UpsOnline => self.fx.push(FxRequest::UpsOnline),
            Event::Network { rx_bps, tx_bps } => {
                self.net_rx.set(net_fill(rx_bps), now);
                self.net_tx.set(net_fill(tx_bps), now);
                self.net = NetState {
                    rx_bps,
                    tx_bps,
                    have: true,
                };
            }
            Event::Torrents(list) => self.fold_torrents(list, now),
            Event::TorrentAdded { .. } => self.fx.push(FxRequest::TorrentAdded),
            Event::TorrentDone { .. } => self.fx.push(FxRequest::TorrentDone),
            Event::Link { target, up } => match target {
                LinkTarget::K8sApi => {
                    let was = self.link.api;
                    self.link.api = up;
                    if up && !was {
                        if self.boot_pending {
                            self.boot_pending = false;
                            self.fx.push(FxRequest::Boot);
                        } else if self.seen_api_up {
                            self.fx.push(FxRequest::LinkUp);
                        }
                    }
                    if up {
                        self.seen_api_up = true;
                    }
                }
                LinkTarget::Prometheus => self.link.prom = up,
                LinkTarget::QBittorrent => self.link.qbit = up,
                LinkTarget::Electricity => self.link.electricity = up,
                LinkTarget::Prices => self.link.prices = up,
                LinkTarget::Weather => self.link.weather = up,
                LinkTarget::Rain => self.link.rain = up,
                LinkTarget::Github => self.link.github = up,
                LinkTarget::ArgoCd => self.link.argocd = up,
            },
            Event::Boot => {
                // Only cluster roles wait for the API link; a layout of pure
                // electricity screens would otherwise never see its boot sweep.
                let needs_api = self
                    .screens
                    .iter()
                    .any(|s| s.roles().iter().any(|r| r.is_cluster()));
                if self.link.api || !needs_api {
                    self.fx.push(FxRequest::Boot);
                } else {
                    self.boot_pending = true;
                }
            }
            Event::ForceNight(v) => self.night_override = v,
            Event::Weather {
                temp_c,
                code,
                is_day,
                wind_kmh,
                gust_kmh,
                wind_from_deg,
                at,
            } => {
                if self.weather.have && is_thunder(code) && !is_thunder(self.weather.code) {
                    self.fx.push(FxRequest::Thunder);
                }
                self.temp.set(temp_c, now);
                self.wind.set(wind_kmh, now);
                self.weather = WeatherState {
                    temp_c,
                    code,
                    is_day,
                    wind_kmh,
                    gust_kmh,
                    wind_from_deg,
                    at,
                    have: true,
                };
            }
            Event::AirQuality { eaqi } => {
                if self.air.have && eaqi_band(eaqi) > eaqi_band(self.air.eaqi) {
                    self.fx.push(FxRequest::AirWorse);
                }
                self.eaqi.set(eaqi, now);
                self.air = AirState { eaqi, have: true };
            }
            Event::Rain { from, mm_per_h } => {
                let slots = current_slots(from, &mm_per_h, self.unix_now);
                let wet = first_wet_minutes(&slots);
                let dry_now = slots.first().is_none_or(|v| *v < WET_MM);
                let inside = |w: Option<u32>| w.is_some_and(|m| m <= SOON_MINUTES);
                if self.rain.have && dry_now && inside(wet) && !inside(self.rain_prev_wet) {
                    self.fx.push(FxRequest::RainSoon);
                }
                self.rain_prev_wet = wet;
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
                self.gh_today.set(self.github.today() as f32, now);
            }
            Event::GithubPush { commits, .. } => {
                // one request per commit: the splash queue collapses them into `+N`
                for _ in 0..commits.max(1) {
                    self.fx.push(FxRequest::GithubPush);
                }
            }
            Event::GithubStar { .. } => self.fx.push(FxRequest::GithubStar),
            Event::GithubMerge { .. } => self.fx.push(FxRequest::GithubMerge),
            Event::GithubRelease { .. } => self.fx.push(FxRequest::GithubRelease),
            Event::GithubRun { ok, .. } => self.fx.push(if ok {
                FxRequest::GithubRunPassed
            } else {
                FxRequest::GithubRunFailed
            }),
            Event::Apps(list) => {
                self.apps = AppsState {
                    apps: list,
                    have: true,
                };
            }
            Event::AppSynced { .. } => self.fx.push(FxRequest::AppSynced),
            Event::AppDegraded { .. } => self.fx.push(FxRequest::AppDegraded),
            Event::AppHealthy { .. } => self.fx.push(FxRequest::AppHealthy),
        }
    }

    fn check_hot(&mut self, role: Role, hot: Option<(String, f32)>, threshold: f32, now: Secs) {
        let Some((node, value)) = hot else { return };
        if value < threshold {
            return;
        }
        let key = (role, node);
        let recently = self
            .hot_last
            .get(&key)
            .is_some_and(|t| now - t < HOT_DEBOUNCE_SECS);
        if !recently {
            self.hot_last.insert(key, now);
            self.fx.push(FxRequest::HotNode(role));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn metrics_update_state_and_smooth() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(42.0, None), 0.0);
        assert_eq!(m.state().cpu_pct, 42.0);
        assert!(m.state().have_metrics);
        assert!(m.smooth_cpu(0.0) < 1.0);
        assert!((m.smooth_cpu(5.0) - 42.0).abs() < 1e-4);
    }

    #[test]
    fn pod_events_request_fx() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::PodStarted {
                ns: "a".into(),
                name: "b".into(),
            },
            0.0,
        );
        m.apply(
            Event::PodCrashed {
                ns: "a".into(),
                name: "b".into(),
            },
            0.0,
        );
        assert_eq!(
            m.take_fx(),
            vec![FxRequest::PodStarted, FxRequest::PodCrashed]
        );
        assert!(m.pending_fx().is_empty());
    }

    #[test]
    fn node_flip_requests_sweep() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeReady {
                name: "n1".into(),
                ready: false,
            },
            0.0,
        );
        assert_eq!(m.take_fx(), vec![FxRequest::NodeNotReady]);
    }

    #[test]
    fn hot_node_debounced_five_minutes() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(50.0, Some(("n1", 95.0))), 0.0);
        m.apply(metrics(50.0, Some(("n1", 96.0))), 10.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotNode(Role::Cpu)]);
        m.apply(metrics(50.0, Some(("n1", 96.0))), 301.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotNode(Role::Cpu)]);
        m.apply(metrics(50.0, Some(("n2", 50.0))), 302.0);
        assert!(m.take_fx().is_empty());
    }

    fn temps(list: &[(&str, f32)]) -> Event {
        Event::NodeTemps(list.iter().map(|(n, c)| (n.to_string(), *c)).collect())
    }

    #[test]
    fn node_temps_track_the_hottest_and_debounce_the_splash() {
        let mut m = Model::new(Thresholds::default());
        m.apply(temps(&[("n1", 41.0), ("n2", 58.0), ("n3", 47.0)]), 0.0);
        assert!(m.state().have_temps);
        assert_eq!(m.state().temps.len(), 3);
        assert!((m.smooth_hot_temp(5.0) - 58.0).abs() < 1e-4);
        assert!(m.take_fx().is_empty(), "58 is under the 70 threshold");
        m.apply(temps(&[("n1", 41.0), ("n2", 72.0)]), 1.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotTemp]);
        m.apply(temps(&[("n1", 41.0), ("n2", 74.0)]), 60.0);
        assert!(m.take_fx().is_empty(), "same node debounced for 5 min");
        m.apply(temps(&[("n1", 41.0), ("n2", 74.0)]), 400.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotTemp]);
        m.apply(temps(&[("n1", 90.0), ("n2", 74.0)]), 401.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotTemp], "a new node splashes");
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
        assert!((m.smooth_storage_pct(5.0) - 50.0).abs() < 1e-4);
        m.apply(
            Event::Storage {
                volumes: vec![],
                used_bytes: 0,
                capacity_bytes: 0,
            },
            6.0,
        );
        assert!(m.smooth_storage_pct(12.0) < 1e-4, "no capacity means 0%");
    }

    #[test]
    fn volume_and_temp_events_request_splashes() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::HotTemp {
                node: "n1".into(),
                celsius: 80.0,
            },
            0.0,
        );
        m.apply(
            Event::VolumeDegraded {
                name: "v".into(),
                robustness: Robustness::Degraded,
            },
            0.0,
        );
        m.apply(Event::VolumeHealthy { name: "v".into() }, 0.0);
        assert_eq!(
            m.take_fx(),
            vec![
                FxRequest::HotTemp,
                FxRequest::VolumeDegraded,
                FxRequest::VolumeHealthy
            ]
        );
    }

    #[test]
    fn link_up_sweep_only_after_a_previous_up() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            0.0,
        );
        assert!(m.take_fx().is_empty(), "first connect is not a recovery");
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: false,
            },
            1.0,
        );
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            2.0,
        );
        assert_eq!(m.take_fx(), vec![FxRequest::LinkUp]);
        assert!(m.link().api);
    }

    fn api_link(up: bool) -> Event {
        Event::Link {
            target: LinkTarget::K8sApi,
            up,
        }
    }

    #[test]
    fn boot_is_deferred_until_api_link_comes_up() {
        use crate::fx::SweepKind;
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Boot, 0.0);
        m.tick(0.0);
        assert!(
            m.fx().sweeps.active().is_none(),
            "no sweep while the API link is down"
        );
        m.tick(5.0);
        m.apply(api_link(true), 5.0);
        m.tick(5.0);
        let sw = m.fx().sweeps.active().expect("boot sweep plays on connect");
        assert_eq!(sw.kind, SweepKind::Boot);
        // first connect plays only Boot, not LinkUp as well
        assert!(m.pending_fx().is_empty());
        m.apply(api_link(false), 20.0);
        m.apply(api_link(true), 21.0);
        assert_eq!(
            m.take_fx(),
            vec![FxRequest::LinkUp],
            "recovery plays LinkUp only"
        );
    }

    #[test]
    fn boot_with_api_up_queues_immediately() {
        use crate::fx::SweepKind;
        let mut m = Model::new(Thresholds::default());
        m.apply(api_link(true), 0.0);
        m.apply(Event::Boot, 0.0);
        assert_eq!(m.pending_fx(), &[FxRequest::Boot]);
        m.tick(0.0);
        assert_eq!(m.fx().sweeps.active().unwrap().kind, SweepKind::Boot);
    }

    #[test]
    fn boot_plays_at_once_when_no_screen_needs_the_cluster() {
        use crate::fx::SweepKind;
        let mut m = Model::new(Thresholds::default());
        m.set_screens(
            vec![vec![Role::PowerMix, Role::Price], vec![Role::Carbon]],
            vec![15.0; 2],
        );
        m.apply(Event::Boot, 0.0);
        assert_eq!(m.pending_fx(), &[FxRequest::Boot]);
        m.tick(0.0);
        assert_eq!(m.fx().sweeps.active().unwrap().kind, SweepKind::Boot);
        // one cluster role anywhere in the layout is enough to defer again
        let mut m2 = Model::new(Thresholds::default());
        m2.set_screens(vec![vec![Role::PowerMix, Role::Thermal]], vec![15.0]);
        m2.apply(Event::Boot, 0.0);
        assert!(m2.pending_fx().is_empty());
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
    /// to iris on the very same tick: the case serialisation exists for.
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

    /// Tick at 30 Hz for `secs` and return `(screen, time)` for every iris that
    /// started. With `exclusive`, assert no two screens are ever irising together.
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
            assert!(!exclusive || live <= 1, "{live} screens irising at {now}");
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
    fn without_one_at_a_time_screens_iris_together() {
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
    fn tick_moves_requests_into_queues() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::PodStarted {
                ns: "a".into(),
                name: "b".into(),
            },
            0.0,
        );
        m.tick(0.0);
        assert!(m.pending_fx().is_empty());
        assert!(m.fx().splashes[Role::Pods.index()].active().is_some());
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
        m.set_screens(vec![vec![Role::Thermal, Role::Price], vec![]], vec![5.0]);
        assert_eq!(m.screen_count(), 2);
        assert_eq!(m.current_role(0), Role::Thermal);
        assert_eq!(m.current_role(1), Role::Cpu, "empty list falls back to cpu");
        m.set_screens(vec![], vec![]);
        assert_eq!(m.screen_count(), 1);
        assert!(m.screen(0).is_some());
        assert!(m.screen(1).is_none());
    }

    fn mix(list: &[(Source, f32)], carbon: f32) -> Event {
        Event::Electricity {
            zone: "NL".into(),
            mix_mw: list.to_vec(),
            renewable_pct: 61.0,
            fossil_free_pct: 73.0,
            carbon_gco2: carbon,
            updated_at: "2026-09-07T12:00:00Z".into(),
        }
    }

    #[test]
    fn electricity_fold_normalises_shares_to_percent() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.electricity().have);
        m.apply(
            mix(
                &[
                    (Source::Wind, 2400.0),
                    (Source::Solar, 4800.0),
                    (Source::Gas, 1300.0),
                    (Source::Coal, -10.0),
                ],
                214.0,
            ),
            0.0,
        );
        assert!(m.electricity().have);
        assert_eq!(m.electricity().zone, "NL");
        assert_eq!(m.electricity().mix.len(), 4);
        let shares = m.smooth_shares(5.0);
        let sum: f32 = shares.iter().map(|(_, v)| v).sum();
        assert!((sum - 100.0).abs() < 1e-3, "shares sum to 100, got {sum}");
        assert!((m.smooth_share(Source::Solar, 5.0) - 4800.0 / 8500.0 * 100.0).abs() < 1e-3);
        assert_eq!(
            shares
                .iter()
                .find(|(s, _)| *s == Source::Coal)
                .map(|(_, v)| *v),
            None,
            "a negative reading is clamped to zero and drops out"
        );
        assert!((m.smooth_renewable(5.0) - 61.0).abs() < 1e-4);
        assert!((m.smooth_fossil_free(5.0) - 73.0).abs() < 1e-4);
        assert!((m.smooth_carbon(5.0) - 214.0).abs() < 1e-4);
        assert!(m.smooth_carbon(0.0) < 1.0, "carbon eases in");
    }

    #[test]
    fn a_source_that_leaves_the_mix_eases_to_zero_and_is_dropped() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            mix(&[(Source::Solar, 50.0), (Source::Wind, 50.0)], 100.0),
            0.0,
        );
        assert_eq!(m.smooth_shares(5.0).len(), 2);
        m.apply(mix(&[(Source::Wind, 50.0)], 100.0), 5.0);
        let mid = m.smooth_share(Source::Solar, 5.2);
        assert!(mid > 0.0 && mid < 50.0, "solar eases out, got {mid}");
        assert_eq!(m.smooth_shares(5.2).len(), 2, "still drawn while it fades");
        assert_eq!(m.smooth_shares(10.0).len(), 1, "gone once it reaches zero");
        m.tick(10.0);
        assert_eq!(m.smooth_share(Source::Solar, 10.0), 0.0);
        assert!((m.smooth_share(Source::Wind, 10.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn prices_and_links_and_token() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.prices().have);
        assert_eq!(
            m.local_hour(),
            12,
            "noon until the render loop says otherwise"
        );
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                ct_per_kwh: vec![10.0, 12.5, 22.1],
                currency: "EUR".into(),
            },
            0.0,
        );
        assert!(m.prices().have);
        assert_eq!(m.prices().ct.len(), 3);
        assert_eq!(m.prices().currency, "EUR");
        assert_eq!(m.prices().date, "2026-09-07");
        m.apply(
            Event::Link {
                target: LinkTarget::Electricity,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Link {
                target: LinkTarget::Prices,
                up: true,
            },
            0.0,
        );
        assert!(m.link().electricity && m.link().prices);
        assert!(!m.token_present());
        m.set_token_present(true);
        assert!(m.token_present());
        m.set_local_hour(14);
        assert_eq!(m.local_hour(), 14);
        m.set_local_hour(99);
        assert_eq!(m.local_hour(), 23, "clamped into the day");
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
    fn ups_and_network_fold_into_state_and_smooths() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 6.0,
                runtime_secs: 3014,
            },
            0.0,
        );
        assert!(m.ups().have);
        assert_eq!(m.ups().runtime_secs, 3014);
        assert_eq!(m.smooth_ups_charge(5.0), 100.0);
        assert_eq!(m.smooth_ups_load(5.0), 6.0);
        m.apply(Event::UpsOnBattery, 1.0);
        assert_eq!(m.pending_fx(), &[FxRequest::UpsOnBattery]);
        m.apply(Event::UpsOnline, 2.0);
        assert_eq!(m.pending_fx().last(), Some(&FxRequest::UpsOnline));

        m.apply(
            Event::Network {
                rx_bps: 1e7,
                tx_bps: 1e5,
            },
            0.0,
        );
        assert!(m.net().have);
        assert!(
            (m.smooth_net_rx(5.0) - 0.5).abs() < 1e-6,
            "10 Mbit is half the log scale"
        );
        assert!((m.smooth_net_tx(5.0) - 0.03).abs() < 1e-6, "idle floor");
        // the pulse phase advances with fill: one lap per 6 s at full fill
        m.tick(10.0);
        let (a, _) = m.net_phases();
        m.tick(13.0);
        let (b, _) = m.net_phases();
        assert!(
            ((b - a).rem_euclid(1.0) - 0.25).abs() < 0.01,
            "half fill, 3 s = quarter lap"
        );
    }

    #[test]
    fn apps_fold_and_edge_events_splash() {
        use crate::event::{App, AppHealth, AppSync};
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Apps(vec![App {
                name: "argocd".into(),
                sync: AppSync::Synced,
                health: AppHealth::Healthy,
                operating: false,
            }]),
            0.0,
        );
        assert!(m.apps().have);
        assert_eq!(m.apps().apps.len(), 1);
        m.apply(Event::AppDegraded { name: "x".into() }, 1.0);
        m.apply(Event::AppSynced { name: "x".into() }, 1.0);
        m.apply(Event::AppHealthy { name: "x".into() }, 1.0);
        assert_eq!(
            m.pending_fx(),
            &[
                FxRequest::AppDegraded,
                FxRequest::AppSynced,
                FxRequest::AppHealthy
            ]
        );
    }

    #[test]
    fn github_activity_folds_and_pushes_splash_per_commit() {
        let mut m = Model::new(Thresholds::default());
        let days: Vec<(String, u32)> = (0..30)
            .map(|i| {
                (
                    format!("2026-08-{:02}", i + 1),
                    if i == 29 {
                        28
                    } else if i == 28 {
                        43
                    } else {
                        5
                    },
                )
            })
            .collect();
        m.apply(Event::GithubActivity { days: days.clone() }, 0.0);
        assert!(m.github().have);
        assert_eq!(m.github().today(), 28);
        assert_eq!(m.github().best(), 43);
        assert_eq!(m.smooth_gh_today(5.0), 28.0);
        m.apply(
            Event::GithubPush {
                repo: "r".into(),
                commits: 3,
            },
            1.0,
        );
        assert_eq!(m.pending_fx(), &vec![FxRequest::GithubPush; 3]);
        m.take_fx();
        m.apply(Event::GithubStar { repo: "r".into() }, 1.0);
        m.apply(Event::GithubMerge { repo: "r".into() }, 1.0);
        m.apply(
            Event::GithubRelease {
                repo: "r".into(),
                tag: "v1".into(),
            },
            1.0,
        );
        m.apply(
            Event::GithubRun {
                repo: "r".into(),
                ok: false,
            },
            1.0,
        );
        m.apply(
            Event::GithubRun {
                repo: "r".into(),
                ok: true,
            },
            1.0,
        );
        assert_eq!(
            m.pending_fx(),
            &[
                FxRequest::GithubStar,
                FxRequest::GithubMerge,
                FxRequest::GithubRelease,
                FxRequest::GithubRunFailed,
                FxRequest::GithubRunPassed,
            ]
        );
    }

    fn weather(code: u16, temp: f32) -> Event {
        Event::Weather {
            temp_c: temp,
            code,
            is_day: true,
            wind_kmh: 19.0,
            gust_kmh: 39.0,
            wind_from_deg: 232.0,
            at: "2026-09-07T21:45".into(),
        }
    }

    #[test]
    fn weather_folds_and_thunder_splashes_on_the_edge() {
        let mut m = Model::new(Thresholds::default());
        m.apply(weather(95, 18.0), 0.0);
        assert!(m.weather().have);
        assert_eq!(m.weather().code, 95);
        assert_eq!(m.smooth_temp(5.0), 18.0);
        assert_eq!(m.smooth_wind(5.0), 19.0);
        assert!(m.pending_fx().is_empty(), "the first sample never splashes");
        m.apply(weather(3, 18.0), 1.0);
        m.apply(weather(96, 18.0), 2.0);
        assert_eq!(m.pending_fx(), &[FxRequest::Thunder]);
        m.apply(weather(99, 18.0), 3.0);
        assert_eq!(
            m.pending_fx().len(),
            1,
            "staying thundery is not a new edge"
        );
    }

    #[test]
    fn air_quality_splashes_when_the_band_worsens() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        assert!(m.air().have);
        assert_eq!(m.smooth_eaqi(5.0), 32.0);
        assert!(m.pending_fx().is_empty());
        m.apply(Event::AirQuality { eaqi: 38.0 }, 1.0);
        assert!(m.pending_fx().is_empty(), "same band");
        m.apply(Event::AirQuality { eaqi: 41.0 }, 2.0);
        assert_eq!(m.pending_fx(), &[FxRequest::AirWorse]);
        m.apply(Event::AirQuality { eaqi: 10.0 }, 3.0);
        assert_eq!(m.pending_fx().len(), 1, "improving is quiet");
    }

    #[test]
    fn rain_folds_and_splashes_when_rain_moves_inside_fifteen_minutes() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_000_000);
        let mut far = vec![0.0f32; 24];
        far[6] = 2.0; // 30 minutes out
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: far.clone(),
            },
            0.0,
        );
        assert!(m.rain().have);
        assert!(m.pending_fx().is_empty(), "first sample is quiet");
        let mut soon = vec![0.0f32; 24];
        soon[2] = 2.0; // 10 minutes out
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: soon.clone(),
            },
            1.0,
        );
        assert_eq!(m.pending_fx(), &[FxRequest::RainSoon]);
        m.apply(
            Event::Rain {
                from: 1_000_300,
                mm_per_h: soon,
            },
            2.0,
        );
        assert_eq!(
            m.pending_fx().len(),
            1,
            "already inside the window: no repeat"
        );
        // raining now: no umbrella, it is too late for one
        let mut wet_now = vec![0.0f32; 24];
        wet_now[0] = 1.0;
        m.take_fx();
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: far,
            },
            3.0,
        );
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: wet_now,
            },
            4.0,
        );
        assert!(m.pending_fx().is_empty());
    }

    #[test]
    fn sky_and_iss_fold_and_a_visible_pass_sweeps_once() {
        use crate::event::{IssPass, MoonPhase};
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Sky {
                sunrise: Some(100),
                sunset: Some(200),
                sun_elevation_deg: 30.0,
                moon_illumination: 0.63,
                moon_waxing: true,
                moon_phase: MoonPhase::WaxingGibbous,
            },
            0.0,
        );
        assert!(m.sky().have);
        assert_eq!(m.sky().sunset, Some(200));
        let pass = IssPass {
            start: 1_000,
            end: 1_400,
            max_elevation_deg: 62.0,
            visible: true,
        };
        m.apply(Event::IssPass(Some(pass)), 0.0);
        assert!(m.iss().have);
        m.set_unix_now(900);
        m.tick(1.0);
        assert!(m.fx().sweeps.active().is_none(), "not yet");
        m.set_unix_now(1_000);
        m.tick(2.0);
        assert_eq!(
            m.fx().sweeps.active().map(|s| s.kind),
            Some(crate::fx::SweepKind::IssPass)
        );
        m.set_unix_now(1_010);
        m.tick(3.0);
        assert!(m.pending_fx().is_empty(), "one sweep per pass");
        // a pass that is not visible stays quiet
        let mut m2 = Model::new(Thresholds::default());
        m2.apply(
            Event::IssPass(Some(IssPass {
                visible: false,
                ..pass
            })),
            0.0,
        );
        m2.set_unix_now(1_000);
        m2.tick(1.0);
        assert!(m2.fx().sweeps.active().is_none());
    }

    #[test]
    fn net_fill_is_a_clamped_log_scale() {
        assert_eq!(net_fill(0.0), 0.03);
        assert_eq!(net_fill(1e5), 0.03);
        assert!((net_fill(1e7) - 0.5).abs() < 1e-6);
        assert_eq!(net_fill(1e9), 1.0);
        assert_eq!(net_fill(5e9), 1.0);
    }

    fn torrent(name: &str, progress: f32) -> Torrent {
        Torrent {
            name: name.into(),
            progress,
            eta_secs: 600,
            speed_bps: 1_000_000,
        }
    }

    /// A model in torrent mode with the given list applied at `now = 0`.
    fn torrenting(list: Vec<Torrent>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 3,
                not_ready: vec![],
            },
            0.0,
        );
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(Event::Torrents(list), 0.0);
        m
    }

    #[test]
    fn new_torrent_ring_sweeps_in_from_zero() {
        let m = torrenting(vec![torrent("a", 80.0)]);
        let rings = m.torrent_rings(0.0);
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].0, 102.0, "outermost slot");
        assert_eq!(rings[0].1, 0.0, "empty at the arrival instant");
        assert_eq!(rings[0].2, 0);
        let mid = m.torrent_rings(0.2)[0].1;
        assert!(mid > 0.0 && mid < 80.0, "sweeping, got {mid}");
        assert!((m.torrent_rings(1.0)[0].1 - 80.0).abs() < 1e-4);
    }

    #[test]
    fn removed_torrent_unwinds_then_leaves() {
        let mut m = torrenting(vec![torrent("a", 80.0)]);
        m.tick(1.0);
        m.apply(Event::Torrents(vec![]), 1.0);
        let a = m.torrent_rings(1.2)[0].1;
        let b = m.torrent_rings(1.4)[0].1;
        assert!(a < 80.0 && b < a, "unwinding: {a} then {b}");
        m.tick(1.2);
        assert!(m.torrent_mode(), "the ring is still on screen");
        assert_eq!(m.torrent_rings(1.2).len(), 1);
        m.tick(1.7);
        assert!(!m.torrent_mode(), "the ring finished unwinding");
        assert!(m.torrent_rings(1.7).is_empty());
    }

    #[test]
    fn remaining_ring_eases_to_the_freed_slot() {
        let mut m = torrenting(vec![torrent("a", 80.0), torrent("b", 40.0)]);
        m.tick(1.0);
        assert_eq!(m.torrent_rings(1.0)[1].0, 86.0, "b starts one slot in");
        m.apply(Event::Torrents(vec![torrent("b", 40.0)]), 1.0);
        assert_eq!(
            m.torrent_rings(1.3)[1].0,
            86.0,
            "b holds its slot while a unwinds"
        );
        m.tick(1.7);
        let rings = m.torrent_rings(1.9);
        assert_eq!(rings.len(), 1);
        assert!(
            rings[0].0 > 86.0 && rings[0].0 < 102.0,
            "b is easing outward, got {}",
            rings[0].0
        );
        assert!((m.torrent_rings(2.2)[0].0 - 102.0).abs() < 1e-4);
        assert_eq!(m.torrent_rings(2.2)[0].2, 0, "and takes the outer accent");
    }

    #[test]
    fn badge_falls_back_to_the_last_values_while_unwinding() {
        let mut m = torrenting(vec![torrent("a", 80.0)]);
        m.apply(Event::Torrents(vec![]), 1.0);
        assert!(m.state().torrents.is_empty());
        let list = m.torrent_badge_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].speed_bps, 1_000_000);
    }

    #[test]
    fn only_three_torrents_get_a_ring() {
        let m = torrenting(vec![
            torrent("a", 10.0),
            torrent("b", 90.0),
            torrent("c", 50.0),
            torrent("d", 70.0),
        ]);
        let rings = m.torrent_rings(2.0);
        assert_eq!(rings.len(), 3);
        let pcts: Vec<f32> = rings.iter().map(|r| r.1.round()).collect();
        assert_eq!(pcts, vec![90.0, 70.0, 50.0], "most complete outermost");
    }

    /// Churn like the fake source's `t` / `6` keys, one frame at a time: the
    /// slot bookkeeping must never index out of range or draw a fourth ring.
    #[test]
    fn torrent_churn_keeps_at_most_three_rings() {
        let mut m = torrenting(vec![torrent("a", 78.0), torrent("b", 41.0)]);
        let names = ["a", "b", "c", "d"];
        let mut now = 0.0;
        for frame in 0..600 {
            now += 1.0 / 30.0;
            if frame % 47 == 0 {
                let keep = (frame / 47) % 5;
                let list = names
                    .iter()
                    .take(keep)
                    .enumerate()
                    .map(|(i, n)| torrent(n, 10.0 + i as f32 * 25.0))
                    .collect();
                m.apply(Event::Torrents(list), now);
            }
            m.tick(now);
            let rings = m.torrent_rings(now);
            assert!(rings.len() <= 3, "frame {frame}: {} rings", rings.len());
            for (radius, progress, accent) in rings {
                assert!((70.0..=102.0).contains(&radius), "radius {radius}");
                assert!((-0.01..=100.01).contains(&progress), "progress {progress}");
                assert!(accent < 3);
            }
        }
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
}

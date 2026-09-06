//! Cluster state and the fold of events into it.

use std::collections::{HashMap, HashSet};

use crate::anim::{Secs, Smooth};
use crate::electricity::Source;
use crate::event::{Event, LinkTarget, Robustness, Torrent};
use crate::fx::Fx;
use crate::screens::ScreenState;
use crate::theme::Role;

pub const SMOOTH_SECS: Secs = 0.8;
const HOT_DEBOUNCE_SECS: Secs = 300.0;
/// Dwell per role on a cycling screen when the caller gives none.
pub const DEFAULT_CYCLE_SECS: Secs = 15.0;

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

/// A smoothed share below this is treated as gone: no segment, no icon.
const SHARE_EPS: f32 = 0.05;

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
    /// Percent of total production per source, eased.
    shares: HashMap<Source, Smooth>,
    renewable: Smooth,
    fossil_free: Smooth,
    carbon: Smooth,
    /// An Electricity Maps token is configured; without one the electricity
    /// roles show a key instead of the offline cloud.
    token_present: bool,
    /// Local wall-clock hour, set by the render loop.
    local_hour: u32,
    hot_last: HashMap<(Role, String), Secs>,
    hot_temp_last: HashMap<String, Secs>,
    night_override: Option<bool>,
    fx: Vec<FxRequest>,
    fx_state: Fx,
    /// One entry per physical screen, top to bottom.
    screens: Vec<ScreenState>,
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
            },
            thresholds,
            cpu: Smooth::new(0.0, SMOOTH_SECS),
            mem: Smooth::new(0.0, SMOOTH_SECS),
            pods: Smooth::new(0.0, SMOOTH_SECS),
            hot_temp: Smooth::new(0.0, SMOOTH_SECS),
            storage_pct: Smooth::new(0.0, SMOOTH_SECS),
            electricity: ElectricityState::default(),
            prices: PriceState::default(),
            shares: HashMap::new(),
            renewable: Smooth::new(0.0, SMOOTH_SECS),
            fossil_free: Smooth::new(0.0, SMOOTH_SECS),
            carbon: Smooth::new(0.0, SMOOTH_SECS),
            token_present: false,
            local_hour: 12,
            hot_last: HashMap::new(),
            hot_temp_last: HashMap::new(),
            night_override: None,
            fx: Vec::new(),
            fx_state: Fx::default(),
            screens: [Role::Cpu, Role::Mem, Role::Pods, Role::Health]
                .into_iter()
                .map(|r| ScreenState::new(vec![r], DEFAULT_CYCLE_SECS))
                .collect(),
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
        self.screens = roles
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                ScreenState::new(r, cycle_secs.get(i).copied().unwrap_or(DEFAULT_CYCLE_SECS))
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

    /// Drain animation requests into the queues and advance them, then advance
    /// each screen's cycle. Call once per frame.
    pub fn tick(&mut self, now: Secs) {
        // sources that left the mix ease to zero, then stop costing anything
        self.shares
            .retain(|_, sm| sm.target() > 0.0 || sm.value(now) >= SHARE_EPS);
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        let screens = self.screens.len();
        self.fx_state.tick(now, screens);
        let sweep_active = self.fx_state.sweeps.active().is_some();
        for s in &mut self.screens {
            let busy = sweep_active
                || self.fx_state.splashes[s.current().index()]
                    .active()
                    .is_some();
            s.tick(now, busy);
        }
    }

    pub fn all_healthy(&self) -> bool {
        let s = &self.state;
        s.have_nodes && s.nodes_total > 0 && s.nodes_ready == s.nodes_total && s.alerts.is_empty()
    }

    pub fn torrent_mode(&self) -> bool {
        self.all_healthy() && self.link.qbit && !self.state.torrents.is_empty()
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
            Event::Torrents(list) => self.state.torrents = list,
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
            },
            Event::Boot => {
                if self.link.api {
                    self.fx.push(FxRequest::Boot);
                } else {
                    self.boot_pending = true;
                }
            }
            Event::ForceNight(v) => self.night_override = v,
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

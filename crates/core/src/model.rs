//! Cluster state and the fold of events into it.

use std::collections::HashMap;

use crate::anim::{Secs, Smooth};
use crate::event::{Event, LinkTarget, Torrent};
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
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            hot_cpu: 90.0,
            hot_mem: 90.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkState {
    pub api: bool,
    pub prom: bool,
    pub qbit: bool,
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
    pub have_metrics: bool,
    pub have_pods: bool,
    pub have_nodes: bool,
}

/// What the fold wants the animation layer to do. Consumed by Task 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FxRequest {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode(Role),
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
    hot_last: HashMap<(Role, String), Secs>,
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
            },
            thresholds,
            cpu: Smooth::new(0.0, SMOOTH_SECS),
            mem: Smooth::new(0.0, SMOOTH_SECS),
            pods: Smooth::new(0.0, SMOOTH_SECS),
            hot_last: HashMap::new(),
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

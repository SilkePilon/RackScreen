//! Cluster state and the fold of events into it.

use std::collections::HashMap;

use crate::anim::{Secs, Smooth};
use crate::event::{Event, LinkTarget, Torrent};
use crate::fx::Fx;
use crate::theme::Role;

pub const SMOOTH_SECS: Secs = 0.8;
const HOT_DEBOUNCE_SECS: Secs = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    pub hot_cpu: f32,
    pub hot_mem: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self { hot_cpu: 90.0, hot_mem: 90.0 }
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
    seen_api_up: bool,
}

impl Model {
    pub fn new(thresholds: Thresholds) -> Self {
        Self {
            state: ClusterState::default(),
            link: LinkState { api: false, prom: false, qbit: false },
            thresholds,
            cpu: Smooth::new(0.0, SMOOTH_SECS),
            mem: Smooth::new(0.0, SMOOTH_SECS),
            pods: Smooth::new(0.0, SMOOTH_SECS),
            hot_last: HashMap::new(),
            night_override: None,
            fx: Vec::new(),
            fx_state: Fx::default(),
            seen_api_up: false,
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

    /// Drain animation requests into the queues and advance them. Call once per frame.
    pub fn tick(&mut self, now: Secs) {
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        self.fx_state.tick(now);
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
            Event::Metrics { cpu_pct, mem_pct, mem_used_gb, mem_total_gb, hot_cpu, hot_mem } => {
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
            Event::PodSnapshot { running, pending, failed, total } => {
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
            Event::NodeSnapshot { ready, total, not_ready } => {
                self.state.nodes_ready = ready;
                self.state.nodes_total = total;
                self.state.nodes_not_ready = not_ready;
                self.state.have_nodes = true;
            }
            Event::NodeReady { ready, .. } => {
                self.fx.push(if ready { FxRequest::NodeReady } else { FxRequest::NodeNotReady });
            }
            Event::AlertSnapshot { firing } => self.state.alerts = firing,
            Event::AlertChanged { firing, .. } => {
                self.fx.push(if firing { FxRequest::AlertFiring } else { FxRequest::AlertResolved });
            }
            Event::Torrents(list) => self.state.torrents = list,
            Event::TorrentAdded { .. } => self.fx.push(FxRequest::TorrentAdded),
            Event::TorrentDone { .. } => self.fx.push(FxRequest::TorrentDone),
            Event::Link { target, up } => match target {
                LinkTarget::K8sApi => {
                    let was = self.link.api;
                    self.link.api = up;
                    if up && !was && self.seen_api_up {
                        self.fx.push(FxRequest::LinkUp);
                    }
                    if up {
                        self.seen_api_up = true;
                    }
                }
                LinkTarget::Prometheus => self.link.prom = up,
                LinkTarget::QBittorrent => self.link.qbit = up,
            },
            Event::Boot => self.fx.push(FxRequest::Boot),
            Event::ForceNight(v) => self.night_override = v,
        }
    }

    fn check_hot(&mut self, role: Role, hot: Option<(String, f32)>, threshold: f32, now: Secs) {
        let Some((node, value)) = hot else { return };
        if value < threshold {
            return;
        }
        let key = (role, node);
        let recently = self.hot_last.get(&key).is_some_and(|t| now - t < HOT_DEBOUNCE_SECS);
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
        m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 0.0);
        m.apply(Event::PodCrashed { ns: "a".into(), name: "b".into() }, 0.0);
        assert_eq!(m.take_fx(), vec![FxRequest::PodStarted, FxRequest::PodCrashed]);
        assert!(m.pending_fx().is_empty());
    }

    #[test]
    fn node_flip_requests_sweep() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::NodeReady { name: "n1".into(), ready: false }, 0.0);
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
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
        assert!(m.take_fx().is_empty(), "first connect is not a recovery");
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: false }, 1.0);
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 2.0);
        assert_eq!(m.take_fx(), vec![FxRequest::LinkUp]);
        assert!(m.link().api);
    }

    #[test]
    fn tick_moves_requests_into_queues() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 0.0);
        m.tick(0.0);
        assert!(m.pending_fx().is_empty());
        assert!(m.fx().splashes[Role::Pods.index()].active().is_some());
    }

    #[test]
    fn healthy_and_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.all_healthy());
        m.apply(Event::NodeSnapshot { ready: 3, total: 3, not_ready: vec![] }, 0.0);
        assert!(m.all_healthy());
        assert!(!m.torrent_mode());
        m.apply(Event::Link { target: LinkTarget::QBittorrent, up: true }, 0.0);
        m.apply(
            Event::Torrents(vec![Torrent { name: "x".into(), progress: 10.0, eta_secs: 100, speed_bps: 1000 }]),
            0.0,
        );
        assert!(m.torrent_mode());
        m.apply(Event::AlertSnapshot { firing: vec!["Down".into()] }, 0.0);
        assert!(!m.all_healthy());
        assert!(!m.torrent_mode());
    }
}

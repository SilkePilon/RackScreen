//! Pod and node watchers via kube-rs. Trackers are pure and unit tested.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::StreamExt;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Api, Client, Config};
use rackscreen_core::event::{Event, LinkTarget};

use crate::SourceCtx;

pub async fn make_client(kubeconfig: &Path) -> Result<Client> {
    let kc = Kubeconfig::read_from(kubeconfig)
        .with_context(|| format!("read {}", kubeconfig.display()))?;
    let cfg = Config::from_custom_kubeconfig(kc, &KubeConfigOptions::default())
        .await
        .context("kubeconfig")?;
    Client::try_from(cfg).context("client")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodInfo {
    pub phase: Phase,
    pub restarts: i32,
    pub crashloop: bool,
}

pub fn pod_key(pod: &Pod) -> String {
    pod.metadata.uid.clone().unwrap_or_else(|| {
        format!(
            "{}/{}",
            pod.metadata.namespace.as_deref().unwrap_or(""),
            pod.metadata.name.as_deref().unwrap_or("")
        )
    })
}

pub fn pod_info(pod: &Pod) -> PodInfo {
    let status = pod.status.as_ref();
    let phase = match status.and_then(|s| s.phase.as_deref()) {
        Some("Pending") => Phase::Pending,
        Some("Running") => Phase::Running,
        Some("Succeeded") => Phase::Succeeded,
        Some("Failed") => Phase::Failed,
        _ => Phase::Unknown,
    };
    let mut restarts = 0;
    let mut crashloop = false;
    if let Some(cs) = status.and_then(|s| s.container_statuses.as_ref()) {
        for c in cs {
            restarts += c.restart_count;
            let waiting = c
                .state
                .as_ref()
                .and_then(|s| s.waiting.as_ref())
                .and_then(|w| w.reason.as_deref());
            if waiting == Some("CrashLoopBackOff") {
                crashloop = true;
            }
        }
    }
    PodInfo {
        phase,
        restarts,
        crashloop,
    }
}

#[derive(Default)]
pub struct PodTracker {
    pods: HashMap<String, PodInfo>,
    synced: bool,
}

impl PodTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_init(&mut self) {
        self.pods.clear();
        self.synced = false;
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        let mut running = 0;
        let mut pending = 0;
        let mut failed = 0;
        for p in self.pods.values() {
            if p.crashloop || p.phase == Phase::Failed {
                failed += 1;
            } else if p.phase == Phase::Running {
                running += 1;
            } else if p.phase == Phase::Pending {
                pending += 1;
            }
        }
        Event::PodSnapshot {
            running,
            pending,
            failed,
            total: self.pods.len() as u32,
        }
    }

    pub fn apply(&mut self, pod: &Pod) -> Vec<Event> {
        let key = pod_key(pod);
        let new = pod_info(pod);
        let old = self.pods.insert(key, new.clone());
        if !self.synced {
            return Vec::new();
        }
        let ns = pod.metadata.namespace.clone().unwrap_or_default();
        let name = pod.metadata.name.clone().unwrap_or_default();
        let mut out = Vec::new();
        let became_running = new.phase == Phase::Running
            && !new.crashloop
            && old
                .as_ref()
                .is_none_or(|o| o.phase != Phase::Running || o.crashloop);
        let crashed = match &old {
            Some(o) => {
                new.restarts > o.restarts
                    || (new.crashloop && !o.crashloop)
                    || (new.phase == Phase::Failed && o.phase != Phase::Failed)
            }
            None => new.crashloop || new.phase == Phase::Failed,
        };
        if became_running {
            out.push(Event::PodStarted {
                ns: ns.clone(),
                name: name.clone(),
            });
        }
        if crashed {
            out.push(Event::PodCrashed { ns, name });
        }
        if old.as_ref() != Some(&new) {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, pod: &Pod) -> Vec<Event> {
        let existed = self.pods.remove(&pod_key(pod)).is_some();
        if !self.synced || !existed {
            return Vec::new();
        }
        vec![
            Event::PodGone {
                ns: pod.metadata.namespace.clone().unwrap_or_default(),
                name: pod.metadata.name.clone().unwrap_or_default(),
            },
            self.snapshot(),
        ]
    }
}

pub fn node_ready(node: &Node) -> bool {
    node.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|cs| cs.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
        .unwrap_or(false)
}

#[derive(Default)]
pub struct NodeTracker {
    nodes: HashMap<String, bool>,
    synced: bool,
}

impl NodeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_init(&mut self) {
        self.nodes.clear();
        self.synced = false;
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        let mut not_ready: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, r)| !**r)
            .map(|(n, _)| n.clone())
            .collect();
        not_ready.sort();
        Event::NodeSnapshot {
            ready: self.nodes.values().filter(|r| **r).count() as u32,
            total: self.nodes.len() as u32,
            not_ready,
        }
    }

    pub fn apply(&mut self, node: &Node) -> Vec<Event> {
        let name = node.metadata.name.clone().unwrap_or_default();
        let ready = node_ready(node);
        let old = self.nodes.insert(name.clone(), ready);
        if !self.synced {
            return Vec::new();
        }
        let mut out = Vec::new();
        if old.is_some_and(|o| o != ready) {
            out.push(Event::NodeReady { name, ready });
        }
        if old != Some(ready) {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, node: &Node) -> Vec<Event> {
        let name = node.metadata.name.clone().unwrap_or_default();
        if self.nodes.remove(&name).is_none() || !self.synced {
            return Vec::new();
        }
        vec![self.snapshot()]
    }
}

/// How long the pod watch may keep failing before the API link is reported down.
pub const LINK_GRACE: Duration = Duration::from_secs(10);

/// Turns a noisy watch stream (410 Gone after a long watch, a single TCP reset)
/// into link edges: down only after errors have persisted for `grace`, up on
/// the first completed list after that (or the very first one).
#[derive(Debug)]
pub struct LinkEdge {
    target: LinkTarget,
    grace: Duration,
    up: bool,
    first_err: Option<Instant>,
}

impl LinkEdge {
    pub fn new(grace: Duration) -> Self {
        Self::new_for(LinkTarget::K8sApi, grace)
    }

    pub fn new_for(target: LinkTarget, grace: Duration) -> Self {
        Self {
            target,
            grace,
            up: false,
            first_err: None,
        }
    }

    pub fn is_up(&self) -> bool {
        self.up
    }

    pub fn on_error(&mut self, now: Instant) -> Option<Event> {
        let first = *self.first_err.get_or_insert(now);
        if self.up && now.duration_since(first) > self.grace {
            self.up = false;
            return Some(Event::Link {
                target: self.target,
                up: false,
            });
        }
        None
    }

    /// `synced` is true when the item completes a (re)list, i.e. `InitDone`;
    /// the link only comes up once the tracker holds a full snapshot again.
    pub fn on_ok(&mut self, _now: Instant, synced: bool) -> Option<Event> {
        self.first_err = None;
        if synced && !self.up {
            self.up = true;
            return Some(Event::Link {
                target: self.target,
                up: true,
            });
        }
        None
    }
}

pub async fn run_pod_watch(client: Client, ctx: SourceCtx) {
    let api: Api<Pod> = Api::all(client);
    let mut stream = watcher(api, watcher::Config::default().any_semantic())
        .default_backoff()
        .boxed();
    let mut tracker = PodTracker::new();
    let mut link = LinkEdge::new(LINK_GRACE);
    loop {
        let item = tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            item = stream.next() => item,
        };
        let Some(item) = item else { return };
        match item {
            Ok(ev) => {
                let synced = matches!(ev, watcher::Event::InitDone);
                let evs = match ev {
                    watcher::Event::Init => {
                        tracker.begin_init();
                        Vec::new()
                    }
                    watcher::Event::InitApply(p) => tracker.apply(&p),
                    watcher::Event::InitDone => tracker.init_done(),
                    watcher::Event::Apply(p) => tracker.apply(&p),
                    watcher::Event::Delete(p) => tracker.delete(&p),
                };
                if let Some(edge) = link.on_ok(Instant::now(), synced) {
                    ctx.emit(edge);
                }
                ctx.emit_all(evs);
            }
            Err(e) => {
                tracing::warn!("pod watch: {e}");
                if let Some(edge) = link.on_error(Instant::now()) {
                    ctx.emit(edge);
                }
            }
        }
    }
}

pub async fn run_node_watch(client: Client, ctx: SourceCtx) {
    let api: Api<Node> = Api::all(client);
    let mut stream = watcher(api, watcher::Config::default().any_semantic())
        .default_backoff()
        .boxed();
    let mut tracker = NodeTracker::new();
    loop {
        let item = tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            item = stream.next() => item,
        };
        let Some(item) = item else { return };
        match item {
            Ok(watcher::Event::Init) => tracker.begin_init(),
            Ok(watcher::Event::InitApply(n)) => ctx.emit_all(tracker.apply(&n)),
            Ok(watcher::Event::InitDone) => ctx.emit_all(tracker.init_done()),
            Ok(watcher::Event::Apply(n)) => ctx.emit_all(tracker.apply(&n)),
            Ok(watcher::Event::Delete(n)) => ctx.emit_all(tracker.delete(&n)),
            Err(e) => tracing::warn!("node watch: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateWaiting, ContainerStatus, NodeCondition, NodeStatus,
        PodStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn pod(name: &str, phase: &str, restarts: i32, waiting: Option<&str>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                namespace: Some("ns".into()),
                uid: Some(format!("uid-{name}")),
                ..Default::default()
            },
            status: Some(PodStatus {
                phase: Some(phase.into()),
                container_statuses: Some(vec![ContainerStatus {
                    restart_count: restarts,
                    state: Some(ContainerState {
                        waiting: waiting.map(|r| ContainerStateWaiting {
                            reason: Some(r.into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn node(name: &str, ready: bool) -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            status: Some(NodeStatus {
                conditions: Some(vec![NodeCondition {
                    type_: "Ready".into(),
                    status: if ready { "True" } else { "False" }.into(),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn pod_info_reads_phase_restarts_crashloop() {
        let i = pod_info(&pod("a", "Running", 3, Some("CrashLoopBackOff")));
        assert_eq!(
            i,
            PodInfo {
                phase: Phase::Running,
                restarts: 3,
                crashloop: true
            }
        );
        assert_eq!(pod_info(&pod("a", "Weird", 0, None)).phase, Phase::Unknown);
    }

    #[test]
    fn initial_sync_is_silent_then_snapshot() {
        let mut t = PodTracker::new();
        t.begin_init();
        assert!(t.apply(&pod("a", "Running", 0, None)).is_empty());
        assert!(t.apply(&pod("b", "Pending", 0, None)).is_empty());
        let evs = t.init_done();
        assert_eq!(
            evs,
            vec![Event::PodSnapshot {
                running: 1,
                pending: 1,
                failed: 0,
                total: 2
            }]
        );
    }

    #[test]
    fn pod_lifecycle_events() {
        let mut t = PodTracker::new();
        t.begin_init();
        t.init_done();
        let evs = t.apply(&pod("a", "Pending", 0, None));
        assert_eq!(
            evs,
            vec![Event::PodSnapshot {
                running: 0,
                pending: 1,
                failed: 0,
                total: 1
            }]
        );
        let evs = t.apply(&pod("a", "Running", 0, None));
        assert_eq!(
            evs[0],
            Event::PodStarted {
                ns: "ns".into(),
                name: "a".into()
            }
        );
        let evs = t.apply(&pod("a", "Running", 1, None));
        assert_eq!(
            evs[0],
            Event::PodCrashed {
                ns: "ns".into(),
                name: "a".into()
            }
        );
        let evs = t.apply(&pod("a", "Running", 1, Some("CrashLoopBackOff")));
        assert_eq!(
            evs[0],
            Event::PodCrashed {
                ns: "ns".into(),
                name: "a".into()
            }
        );
        assert_eq!(
            evs[1],
            Event::PodSnapshot {
                running: 0,
                pending: 0,
                failed: 1,
                total: 1
            }
        );
        assert!(
            t.apply(&pod("a", "Running", 1, Some("CrashLoopBackOff")))
                .is_empty(),
            "no change, no events"
        );
        let evs = t.delete(&pod("a", "Running", 1, None));
        assert_eq!(
            evs[0],
            Event::PodGone {
                ns: "ns".into(),
                name: "a".into()
            }
        );
        assert_eq!(
            evs[1],
            Event::PodSnapshot {
                running: 0,
                pending: 0,
                failed: 0,
                total: 0
            }
        );
        assert!(t.delete(&pod("zzz", "Running", 0, None)).is_empty());
    }

    #[test]
    fn new_running_pod_after_sync_is_started() {
        let mut t = PodTracker::new();
        t.begin_init();
        t.init_done();
        let evs = t.apply(&pod("fresh", "Running", 0, None));
        assert!(matches!(evs[0], Event::PodStarted { .. }));
    }

    #[test]
    fn link_edge_tolerates_short_outages() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let up = Event::Link {
            target: LinkTarget::K8sApi,
            up: true,
        };
        let down = Event::Link {
            target: LinkTarget::K8sApi,
            up: false,
        };
        let mut e = LinkEdge::new(Duration::from_secs(10));
        assert_eq!(
            e.on_error(at(0)),
            None,
            "errors before first sync are silent"
        );
        assert_eq!(
            e.on_ok(at(1), false),
            None,
            "Init/InitApply do not raise the link"
        );
        assert_eq!(e.on_ok(at(2), true), Some(up.clone()), "first InitDone");
        assert!(e.is_up());
        // a single 410 Gone then a quick re-list: no edge at all
        assert_eq!(e.on_error(at(100)), None);
        assert_eq!(e.on_ok(at(101), false), None);
        assert_eq!(
            e.on_ok(at(102), true),
            None,
            "no up without a preceding down"
        );
        // errors persisting past the grace period drop the link exactly once
        assert_eq!(e.on_error(at(200)), None);
        assert_eq!(e.on_error(at(205)), None);
        assert_eq!(
            e.on_error(at(210)),
            None,
            "not strictly older than grace yet"
        );
        assert_eq!(e.on_error(at(211)), Some(down.clone()));
        assert!(!e.is_up());
        assert_eq!(e.on_error(at(230)), None, "down emitted once");
        // recovery: up only once the re-list completes
        assert_eq!(e.on_ok(at(240), false), None);
        assert_eq!(e.on_ok(at(241), true), Some(up));
        // an ok item resets the error clock
        assert_eq!(e.on_error(at(300)), None);
        assert_eq!(e.on_ok(at(305), false), None);
        assert_eq!(e.on_error(at(312)), None, "clock restarted at 312");
        assert_eq!(e.on_error(at(323)), Some(down));
    }

    #[test]
    fn node_flip() {
        let mut t = NodeTracker::new();
        t.begin_init();
        t.apply(&node("n1", true));
        t.apply(&node("n2", true));
        assert_eq!(
            t.init_done(),
            vec![Event::NodeSnapshot {
                ready: 2,
                total: 2,
                not_ready: vec![]
            }]
        );
        let evs = t.apply(&node("n2", false));
        assert_eq!(
            evs[0],
            Event::NodeReady {
                name: "n2".into(),
                ready: false
            }
        );
        assert_eq!(
            evs[1],
            Event::NodeSnapshot {
                ready: 1,
                total: 2,
                not_ready: vec!["n2".into()]
            }
        );
        assert!(t.apply(&node("n2", false)).is_empty());
        let evs = t.apply(&node("n2", true));
        assert_eq!(
            evs[0],
            Event::NodeReady {
                name: "n2".into(),
                ready: true
            }
        );
    }
}

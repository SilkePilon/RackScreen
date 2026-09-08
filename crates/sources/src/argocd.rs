//! Argo CD Application watcher: sync and health per app, with edges when an
//! operation finishes, an app degrades, or it recovers.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use futures::StreamExt;
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};
use kube::runtime::{watcher, WatchStreamExt};
use kube::Client;
use rackscreen_core::event::{App, AppHealth, AppSync, Event, LinkTarget};
use serde_json::Value;

use crate::k8s::{LinkEdge, LINK_GRACE};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct ArgoConfig {
    pub namespace: String,
}

/// How long to wait before looking for the CRD again when it is absent.
const CRD_RETRY: Duration = Duration::from_secs(600);

fn status_str<'a>(status: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut v = status;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_str()
}

/// The app plus the raw operation phase (`Running`, `Succeeded`, ...), which
/// the tracker needs for the sync-finished edge.
pub fn parse_app(obj: &DynamicObject) -> (App, Option<String>) {
    let status = obj.data.get("status").cloned().unwrap_or(Value::Null);
    let sync = match status_str(&status, &["sync", "status"]) {
        Some("Synced") => AppSync::Synced,
        Some("OutOfSync") => AppSync::OutOfSync,
        _ => AppSync::Unknown,
    };
    let health = match status_str(&status, &["health", "status"]) {
        Some("Healthy") => AppHealth::Healthy,
        Some("Progressing") => AppHealth::Progressing,
        Some("Degraded") => AppHealth::Degraded,
        Some("Suspended") => AppHealth::Suspended,
        Some("Missing") => AppHealth::Missing,
        _ => AppHealth::Unknown,
    };
    let phase = status_str(&status, &["operationState", "phase"]).map(str::to_string);
    let app = App {
        name: obj.metadata.name.clone().unwrap_or_default(),
        sync,
        health,
        operating: phase.as_deref() == Some("Running"),
    };
    (app, phase)
}

fn is_bad(h: AppHealth) -> bool {
    matches!(h, AppHealth::Degraded | AppHealth::Missing)
}

/// Pure state behind the watch: sorted apps, last phases, and whether the
/// initial list has completed (before that, nothing is emitted).
#[derive(Debug, Default)]
pub struct AppTracker {
    apps: BTreeMap<String, App>,
    phases: HashMap<String, String>,
    synced: bool,
}

impl AppTracker {
    pub fn begin_init(&mut self) {
        self.synced = false;
        self.apps.clear();
        self.phases.clear();
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        Event::Apps(self.apps.values().cloned().collect())
    }

    pub fn apply(&mut self, (app, phase): (App, Option<String>)) -> Vec<Event> {
        let mut out = Vec::new();
        let prev = self.apps.get(&app.name).cloned();
        if self.synced {
            let prev_phase = self.phases.get(&app.name).map(String::as_str);
            if prev_phase == Some("Running") && phase.as_deref() == Some("Succeeded") {
                out.push(Event::AppSynced {
                    name: app.name.clone(),
                });
            }
            let was_bad = prev.as_ref().is_some_and(|p| is_bad(p.health));
            if is_bad(app.health) && !was_bad {
                out.push(Event::AppDegraded {
                    name: app.name.clone(),
                });
            } else if was_bad && app.health == AppHealth::Healthy {
                out.push(Event::AppHealthy {
                    name: app.name.clone(),
                });
            }
        }
        let changed = prev.as_ref() != Some(&app);
        match phase {
            Some(p) => {
                self.phases.insert(app.name.clone(), p);
            }
            None => {
                self.phases.remove(&app.name);
            }
        }
        self.apps.insert(app.name.clone(), app);
        if self.synced && changed {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, name: &str) -> Vec<Event> {
        self.phases.remove(name);
        if self.apps.remove(name).is_none() || !self.synced {
            return Vec::new();
        }
        vec![self.snapshot()]
    }
}

pub async fn run_argocd(client: Client, cfg: ArgoConfig, ctx: SourceCtx) {
    let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client, &cfg.namespace, &ar);
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        // The CRD may simply not be installed: say so once, retry later.
        let wait = match api.list(&ListParams::default().limit(1)).await {
            Ok(_) => None,
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                tracing::info!(
                    "argocd: no applications.argoproj.io in {}; deploys stays on no-data, retry in {}s",
                    cfg.namespace,
                    CRD_RETRY.as_secs()
                );
                Some(CRD_RETRY)
            }
            Err(e) => {
                tracing::warn!("argocd: {e}; retry in 30s");
                Some(Duration::from_secs(30))
            }
        };
        if let Some(w) = wait {
            ctx.emit(Event::Link {
                target: LinkTarget::ArgoCd,
                up: false,
            });
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(w) => {}
            }
            continue;
        }
        tracing::info!("argocd: watching applications in {}", cfg.namespace);
        let mut stream = watcher(api.clone(), watcher::Config::default().any_semantic())
            .default_backoff()
            .boxed();
        let mut tracker = AppTracker::default();
        let mut link = LinkEdge::new_for(LinkTarget::ArgoCd, LINK_GRACE);
        loop {
            let item = tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                item = stream.next() => item,
            };
            let Some(item) = item else { break };
            match item {
                Ok(ev) => {
                    let synced = matches!(ev, watcher::Event::InitDone);
                    let evs = match ev {
                        watcher::Event::Init => {
                            tracker.begin_init();
                            Vec::new()
                        }
                        watcher::Event::InitApply(o) => tracker.apply(parse_app(&o)),
                        watcher::Event::InitDone => tracker.init_done(),
                        watcher::Event::Apply(o) => tracker.apply(parse_app(&o)),
                        watcher::Event::Delete(o) => {
                            tracker.delete(o.metadata.name.as_deref().unwrap_or(""))
                        }
                    };
                    if let Some(edge) = link.on_ok(Instant::now(), synced) {
                        ctx.emit(edge);
                    }
                    ctx.emit_all(evs);
                }
                Err(e) => {
                    tracing::warn!("argocd: watch: {e}");
                    if let Some(edge) = link.on_error(Instant::now()) {
                        ctx.emit(edge);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = include_str!("../tests/fixtures/argocd-application.json");

    fn obj(sync: &str, health: &str, phase: Option<&str>) -> DynamicObject {
        let mut v: serde_json::Value = serde_json::from_str(APP).unwrap();
        v["status"]["sync"]["status"] = sync.into();
        v["status"]["health"]["status"] = health.into();
        match phase {
            Some(p) => v["status"]["operationState"]["phase"] = p.into(),
            None => {
                v["status"]
                    .as_object_mut()
                    .unwrap()
                    .remove("operationState");
            }
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn parses_the_fixture() {
        let o: DynamicObject = serde_json::from_str(APP).unwrap();
        let (app, phase) = parse_app(&o);
        assert_eq!(
            app,
            App {
                name: "arr-stack".into(),
                sync: AppSync::Synced,
                health: AppHealth::Healthy,
                operating: false,
            }
        );
        assert_eq!(phase.as_deref(), Some("Succeeded"));
        let (running, _) = parse_app(&obj("OutOfSync", "Progressing", Some("Running")));
        assert!(running.operating);
        assert_eq!(running.sync, AppSync::OutOfSync);
        assert_eq!(running.health, AppHealth::Progressing);
        let (bare, phase) = parse_app(&obj("Weird", "Odd", None));
        assert_eq!(
            (bare.sync, bare.health, phase),
            (AppSync::Unknown, AppHealth::Unknown, None)
        );
    }

    #[test]
    fn tracker_snapshots_after_init_and_emits_edges() {
        let mut t = AppTracker::default();
        t.begin_init();
        assert!(t
            .apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded"))))
            .is_empty());
        let done = t.init_done();
        assert_eq!(done.len(), 1);
        assert!(matches!(&done[0], Event::Apps(list) if list.len() == 1));
        // an operation starting then finishing: a sync splash on the finish
        let evs = t.apply(parse_app(&obj("OutOfSync", "Healthy", Some("Running"))));
        assert!(matches!(evs.as_slice(), [Event::Apps(_)]));
        let evs = t.apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded"))));
        assert_eq!(evs.len(), 2);
        assert_eq!(
            evs[0],
            Event::AppSynced {
                name: "arr-stack".into()
            }
        );
        // degraded, then back
        let evs = t.apply(parse_app(&obj("Synced", "Degraded", Some("Succeeded"))));
        assert_eq!(
            evs[0],
            Event::AppDegraded {
                name: "arr-stack".into()
            }
        );
        let evs = t.apply(parse_app(&obj("Synced", "Degraded", Some("Succeeded"))));
        assert!(evs.is_empty(), "unchanged: nothing");
        let evs = t.apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded"))));
        assert_eq!(
            evs[0],
            Event::AppHealthy {
                name: "arr-stack".into()
            }
        );
        // deletion re-snapshots
        let evs = t.delete("arr-stack");
        assert!(matches!(evs.as_slice(), [Event::Apps(list)] if list.is_empty()));
        assert!(t.delete("arr-stack").is_empty());
    }
}

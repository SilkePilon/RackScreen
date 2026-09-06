//! Prometheus poller: cluster CPU/MEM, hot nodes, firing alerts, node
//! temperatures and Longhorn volume health.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Result};
use kube::Client;
use rackscreen_core::event::{Event, LinkTarget, Robustness};
use serde_json::Value;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct PromConfig {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub poll_secs: u64,
    /// Alert names that never count as firing (e.g. kube-prometheus-stack's
    /// permanently firing `Watchdog`).
    pub ignore_alerts: Vec<String>,
}

/// Diffs the set of firing alerts between polls. The first poll only primes
/// the set (a snapshot, no `AlertChanged`) so a process restart does not replay
/// every already-firing alert as a fresh red sweep.
#[derive(Debug, Default)]
pub struct AlertTracker {
    primed: bool,
    prev: HashSet<String>,
    ignore: HashSet<String>,
}

impl AlertTracker {
    pub fn new(ignore: &[String]) -> Self {
        Self {
            primed: false,
            prev: HashSet::new(),
            ignore: ignore.iter().cloned().collect(),
        }
    }

    pub fn diff(&mut self, mut firing: HashSet<String>) -> Vec<Event> {
        firing.retain(|a| !self.ignore.contains(a));
        let mut out = Vec::new();
        if self.primed {
            let mut added: Vec<&String> = firing.difference(&self.prev).collect();
            added.sort();
            for a in added {
                out.push(Event::AlertChanged {
                    name: a.clone(),
                    firing: true,
                });
            }
            let mut gone: Vec<&String> = self.prev.difference(&firing).collect();
            gone.sort();
            for a in gone {
                out.push(Event::AlertChanged {
                    name: a.clone(),
                    firing: false,
                });
            }
        }
        let mut list: Vec<String> = firing.iter().cloned().collect();
        list.sort();
        out.push(Event::AlertSnapshot { firing: list });
        self.prev = firing;
        self.primed = true;
        out
    }
}

pub fn parse_scalar(json: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(json).ok()?;
    let first = v.get("data")?.get("result")?.as_array()?.first()?;
    first
        .get("value")?
        .as_array()?
        .get(1)?
        .as_str()?
        .parse()
        .ok()
}

pub fn parse_vector(json: &str) -> Vec<(HashMap<String, String>, f64)> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(items) = v
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|it| {
            let metric = it.get("metric")?.as_object()?;
            let labels = metric
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect();
            let val: f64 = it
                .get("value")?
                .as_array()?
                .get(1)?
                .as_str()?
                .parse()
                .ok()?;
            Some((labels, val))
        })
        .collect()
}

/// `192.168.1.5:9100` -> `.5`
pub fn node_tag(instance: &str) -> String {
    let ip = instance.split(':').next().unwrap_or(instance);
    match ip.rsplit('.').next() {
        Some(last) if last != ip => format!(".{last}"),
        _ => ip.to_string(),
    }
}

const Q_CPU: &str = "100-avg(rate(node_cpu_seconds_total{mode=\"idle\"}[2m]))*100";
const Q_CPU_BY: &str = "100-avg by(instance)(rate(node_cpu_seconds_total{mode=\"idle\"}[2m]))*100";
const Q_MEM: &str = "(1-sum(node_memory_MemAvailable_bytes)/sum(node_memory_MemTotal_bytes))*100";
const Q_MEM_USED: &str =
    "(sum(node_memory_MemTotal_bytes)-sum(node_memory_MemAvailable_bytes))/1073741824";
const Q_MEM_TOTAL: &str = "sum(node_memory_MemTotal_bytes)/1073741824";
const Q_MEM_BY: &str = "(1-node_memory_MemAvailable_bytes/node_memory_MemTotal_bytes)*100";
const Q_ALERTS: &str = "ALERTS{alertstate=\"firing\"}";
const Q_TEMPS: &str =
    "max by (nodename) (node_hwmon_temp_celsius * on(instance) group_left(nodename) node_uname_info)";
const Q_TEMPS_FALLBACK: &str =
    "max by (nodename) (node_thermal_zone_temp * on(instance) group_left(nodename) node_uname_info)";
const Q_LH_ROBUST: &str = "longhorn_volume_robustness";
const Q_LH_USED: &str = "sum(longhorn_volume_actual_size_bytes)";
const Q_LH_CAP: &str = "sum(longhorn_volume_capacity_bytes)";

/// Node temperatures keyed by the `nodename` label the `node_uname_info` join
/// carries over. Samples outside a plausible range are dropped rather than
/// shown, and the result is sorted so the scene ordering is stable.
pub fn temps_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = v
        .iter()
        .filter_map(|(m, val)| m.get("nodename").map(|n| (n.clone(), *val as f32)))
        .filter(|(_, c)| c.is_finite() && *c > -50.0 && *c < 150.0)
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Reads one `longhorn_volume_robustness` sample.
///
/// Current Longhorn exports the metric one-hot: four series per volume labelled
/// `state="healthy"|"degraded"|"faulted"|"unknown"`, exactly one of them 1 and
/// the rest 0. Older builds export a single unlabelled series carrying the
/// state code, which [`Robustness::from_code`] decodes. Both shapes are
/// accepted; an inactive one-hot series carries no information and is skipped.
fn robustness_of(labels: &HashMap<String, String>, val: f64) -> Option<Robustness> {
    let Some(state) = labels.get("state") else {
        return Some(Robustness::from_code(val));
    };
    if val < 0.5 {
        return None;
    }
    Some(match state.as_str() {
        "healthy" => Robustness::Healthy,
        "degraded" => Robustness::Degraded,
        "faulted" => Robustness::Faulted,
        _ => Robustness::Unknown,
    })
}

/// Longhorn robustness per volume; keeps the highest-severity sample per volume
/// name (Longhorn exports one series per node that hosts a replica).
pub fn volumes_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, Robustness)> {
    fn rank(x: Robustness) -> u8 {
        match x {
            Robustness::Faulted => 3,
            Robustness::Degraded => 2,
            Robustness::Unknown => 1,
            Robustness::Healthy => 0,
        }
    }
    let mut map: std::collections::BTreeMap<String, Robustness> = Default::default();
    for (m, val) in v {
        let Some(name) = m.get("volume") else {
            continue;
        };
        let Some(r) = robustness_of(m, *val) else {
            continue;
        };
        let e = map.entry(name.clone()).or_insert(r);
        if rank(r) > rank(*e) {
            *e = r;
        }
    }
    map.into_iter().collect()
}

/// Diffs Longhorn volume robustness between polls. Like [`AlertTracker`], the
/// first poll only primes so a restart does not replay every already-degraded
/// volume as a fresh splash.
#[derive(Debug, Default)]
pub struct StorageTracker {
    seen: HashMap<String, Robustness>,
    primed: bool,
}

impl StorageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&mut self, volumes: &[(String, Robustness)]) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed {
            for (name, r) in volumes {
                match (self.seen.get(name).copied(), *r) {
                    (Some(Robustness::Healthy), Robustness::Degraded | Robustness::Faulted) => out
                        .push(Event::VolumeDegraded {
                            name: name.clone(),
                            robustness: *r,
                        }),
                    (Some(Robustness::Degraded | Robustness::Faulted), Robustness::Healthy) => {
                        out.push(Event::VolumeHealthy { name: name.clone() })
                    }
                    _ => {}
                }
            }
        }
        self.seen = volumes.iter().cloned().collect();
        self.primed = true;
        out
    }
}

async fn query(t: &mut Tunnel, q: &str) -> Result<String> {
    let path = format!("/api/v1/query?query={}", urlencoding::encode(q));
    let r = t.get(&path, &[]).await?;
    anyhow::ensure!(r.status == 200, "prometheus HTTP {}", r.status);
    Ok(r.body)
}

fn hottest(v: &[(HashMap<String, String>, f64)]) -> Option<(String, f32)> {
    v.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(m, val)| {
            (
                node_tag(m.get("instance").map(String::as_str).unwrap_or("")),
                *val as f32,
            )
        })
}

/// A missing series is an error, not 0%: two consecutive misses drop the
/// Prometheus link so the No-data scene shows instead of a confident zero.
fn metrics_from(
    cpu: Option<f64>,
    mem: Option<f64>,
    mem_used: Option<f64>,
    mem_total: Option<f64>,
    cpu_by: &[(HashMap<String, String>, f64)],
    mem_by: &[(HashMap<String, String>, f64)],
) -> Result<Event> {
    let need =
        |v: Option<f64>, q: &str| v.ok_or_else(|| anyhow!("prometheus returned no data for {q}"));
    Ok(Event::Metrics {
        cpu_pct: need(cpu, Q_CPU)? as f32,
        mem_pct: need(mem, Q_MEM)? as f32,
        mem_used_gb: need(mem_used, Q_MEM_USED)? as f32,
        mem_total_gb: need(mem_total, Q_MEM_TOTAL)? as f32,
        hot_cpu: hottest(cpu_by),
        hot_mem: hottest(mem_by),
    })
}

async fn poll(
    t: &mut Tunnel,
    alerts: &mut AlertTracker,
    storage: &mut StorageTracker,
) -> Result<Vec<Event>> {
    let cpu = parse_scalar(&query(t, Q_CPU).await?);
    let cpu_by = parse_vector(&query(t, Q_CPU_BY).await?);
    let mem = parse_scalar(&query(t, Q_MEM).await?);
    let mem_used = parse_scalar(&query(t, Q_MEM_USED).await?);
    let mem_total = parse_scalar(&query(t, Q_MEM_TOTAL).await?);
    let mem_by = parse_vector(&query(t, Q_MEM_BY).await?);
    let firing_raw = parse_vector(&query(t, Q_ALERTS).await?);
    let mut out = vec![metrics_from(
        cpu, mem, mem_used, mem_total, &cpu_by, &mem_by,
    )?];
    let firing: HashSet<String> = firing_raw
        .iter()
        .filter_map(|(m, _)| m.get("alertname").cloned())
        .collect();
    out.extend(alerts.diff(firing));
    let mut temps = parse_vector(&query(t, Q_TEMPS).await?);
    if temps.is_empty() {
        temps = parse_vector(&query(t, Q_TEMPS_FALLBACK).await?);
    }
    let temps = temps_from(&temps);
    if !temps.is_empty() {
        out.push(Event::NodeTemps(temps));
    }
    let vols = volumes_from(&parse_vector(&query(t, Q_LH_ROBUST).await?));
    if !vols.is_empty() {
        let used = parse_scalar(&query(t, Q_LH_USED).await?).unwrap_or(0.0) as u64;
        let cap = parse_scalar(&query(t, Q_LH_CAP).await?).unwrap_or(0.0) as u64;
        out.extend(storage.diff(&vols));
        out.push(Event::Storage {
            volumes: vols,
            used_bytes: used,
            capacity_bytes: cap,
        });
    }
    Ok(out)
}

pub async fn run_prometheus(client: Client, cfg: PromConfig, ctx: SourceCtx) {
    let mut alerts = AlertTracker::new(&cfg.ignore_alerts);
    let mut storage = StorageTracker::new();
    let mut backoff = 5u64;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let tunnel = async {
            let (pod, svc) = find_pod_for_service(
                &client,
                &cfg.namespace,
                &cfg.service,
                cfg.port,
                "prometheus",
            )
            .await?;
            tracing::info!(
                "prometheus: forwarding to {}/{pod} (service {svc})",
                cfg.namespace
            );
            Tunnel::open(&client, &cfg.namespace, &pod, cfg.port).await
        }
        .await;
        let mut t = match tunnel {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("prometheus: {e:#}; retry in {backoff}s");
                ctx.emit(Event::Link {
                    target: LinkTarget::Prometheus,
                    up: false,
                });
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(30);
                continue;
            }
        };
        backoff = 5;
        let mut failures = 0;
        loop {
            match poll(&mut t, &mut alerts, &mut storage).await {
                Ok(evs) => {
                    failures = 0;
                    ctx.emit(Event::Link {
                        target: LinkTarget::Prometheus,
                        up: true,
                    });
                    ctx.emit_all(evs);
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("prometheus poll: {e:#}");
                    if failures >= 2 {
                        ctx.emit(Event::Link {
                            target: LinkTarget::Prometheus,
                            up: false,
                        });
                        break;
                    }
                }
            }
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(1))) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALAR: &str = r#"{"status":"success","data":{"resultType":"vector","result":[{"metric":{},"value":[1700000000,"42.5"]}]}}"#;
    const VECTOR: &str = r#"{"status":"success","data":{"resultType":"vector","result":[
        {"metric":{"instance":"192.168.1.5:9100"},"value":[1,"91.2"]},
        {"metric":{"instance":"192.168.1.7:9100"},"value":[1,"12.0"]}]}}"#;

    #[test]
    fn scalar_and_empty() {
        assert_eq!(parse_scalar(SCALAR), Some(42.5));
        assert_eq!(parse_scalar(r#"{"data":{"result":[]}}"#), None);
        assert_eq!(parse_scalar("nope"), None);
    }

    #[test]
    fn vector_and_hottest() {
        let v = parse_vector(VECTOR);
        assert_eq!(v.len(), 2);
        assert_eq!(hottest(&v), Some((".5".into(), 91.2)));
        assert!(parse_vector("{}").is_empty());
    }

    #[test]
    fn node_tags() {
        assert_eq!(node_tag("192.168.1.5:9100"), ".5");
        assert_eq!(node_tag("nodename:9100"), "nodename");
    }

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn alert_tracker_primes_silently_then_diffs() {
        let mut t = AlertTracker::new(&[]);
        let evs = t.diff(set(&["Down", "Slow"]));
        assert_eq!(
            evs,
            vec![Event::AlertSnapshot {
                firing: vec!["Down".into(), "Slow".into()]
            }],
            "first poll only primes: no AlertChanged"
        );
        let evs = t.diff(set(&["Down", "New"]));
        assert_eq!(
            evs,
            vec![
                Event::AlertChanged {
                    name: "New".into(),
                    firing: true
                },
                Event::AlertChanged {
                    name: "Slow".into(),
                    firing: false
                },
                Event::AlertSnapshot {
                    firing: vec!["Down".into(), "New".into()]
                },
            ]
        );
        assert_eq!(
            t.diff(set(&["Down", "New"])),
            vec![Event::AlertSnapshot {
                firing: vec!["Down".into(), "New".into()]
            }],
            "steady state is snapshot only"
        );
    }

    #[test]
    fn alert_tracker_ignores_configured_names() {
        let mut t = AlertTracker::new(&["Watchdog".to_string(), "InfoInhibitor".to_string()]);
        let evs = t.diff(set(&["Watchdog"]));
        assert_eq!(evs, vec![Event::AlertSnapshot { firing: vec![] }]);
        let evs = t.diff(set(&["Watchdog", "InfoInhibitor", "Real"]));
        assert_eq!(
            evs,
            vec![
                Event::AlertChanged {
                    name: "Real".into(),
                    firing: true
                },
                Event::AlertSnapshot {
                    firing: vec!["Real".into()]
                },
            ]
        );
        let evs = t.diff(set(&["Real"]));
        assert_eq!(
            evs,
            vec![Event::AlertSnapshot {
                firing: vec!["Real".into()]
            }],
            "ignored alert going away is not a resolve"
        );
    }

    #[test]
    fn missing_series_is_an_error_not_zero() {
        let ok = metrics_from(Some(1.0), Some(2.0), Some(3.0), Some(4.0), &[], &[]).unwrap();
        assert!(matches!(ok, Event::Metrics { cpu_pct, hot_cpu: None, .. } if cpu_pct == 1.0));
        for i in 0..4 {
            let v = |j: usize| if i == j { None } else { Some(1.0) };
            let err = metrics_from(v(0), v(1), v(2), v(3), &[], &[]).unwrap_err();
            assert!(err.to_string().contains("no data"), "{err}");
        }
    }

    #[test]
    fn temps_join_and_sort() {
        let v = parse_vector(
            r#"{"data":{"result":[
            {"metric":{"nodename":"raspberrypi-5-8gb-1"},"value":[1,"48.5"]},
            {"metric":{"nodename":"hp-elitedesk-800-g5-i7"},"value":[1,"53.9"]},
            {"metric":{"instance":"x"},"value":[1,"99"]}]}}"#,
        );
        let t = temps_from(&v);
        assert_eq!(
            t,
            vec![
                ("hp-elitedesk-800-g5-i7".into(), 53.9),
                ("raspberrypi-5-8gb-1".into(), 48.5)
            ]
        );
    }

    #[test]
    fn volumes_keep_worst_sample_and_tracker_diffs() {
        let v = parse_vector(
            r#"{"data":{"result":[
            {"metric":{"volume":"pvc-a","node":"n1"},"value":[1,"1"]},
            {"metric":{"volume":"pvc-a","node":"n2"},"value":[1,"2"]},
            {"metric":{"volume":"pvc-b","node":"n1"},"value":[1,"1"]}]}}"#,
        );
        let vols = volumes_from(&v);
        assert_eq!(
            vols,
            vec![
                ("pvc-a".into(), Robustness::Degraded),
                ("pvc-b".into(), Robustness::Healthy)
            ]
        );
        let mut tr = StorageTracker::new();
        assert!(tr.diff(&vols).is_empty(), "priming is silent");
        let healed = vec![
            ("pvc-a".into(), Robustness::Healthy),
            ("pvc-b".into(), Robustness::Faulted),
        ];
        let evs = tr.diff(&healed);
        assert!(matches!(evs[0], Event::VolumeHealthy { .. }));
        assert!(matches!(
            evs[1],
            Event::VolumeDegraded {
                robustness: Robustness::Faulted,
                ..
            }
        ));
        assert!(tr.diff(&healed).is_empty());
    }

    /// The live cluster exports the one-hot shape: four `state` series per
    /// volume, only the active one at 1. Decoding those values as state codes
    /// would report every volume as `Unknown`.
    #[test]
    fn volumes_read_the_one_hot_state_label() {
        let v = parse_vector(
            r#"{"data":{"result":[
            {"metric":{"volume":"pvc-a","state":"degraded"},"value":[1,"0"]},
            {"metric":{"volume":"pvc-a","state":"faulted"},"value":[1,"0"]},
            {"metric":{"volume":"pvc-a","state":"healthy"},"value":[1,"1"]},
            {"metric":{"volume":"pvc-a","state":"unknown"},"value":[1,"0"]},
            {"metric":{"volume":"pvc-b","state":"degraded"},"value":[1,"1"]},
            {"metric":{"volume":"pvc-b","state":"healthy"},"value":[1,"0"]}]}}"#,
        );
        assert_eq!(
            volumes_from(&v),
            vec![
                ("pvc-a".into(), Robustness::Healthy),
                ("pvc-b".into(), Robustness::Degraded)
            ]
        );
    }

    #[test]
    fn query_path_is_encoded() {
        let p = format!("/api/v1/query?query={}", urlencoding::encode(Q_CPU));
        assert!(p.contains("%7Bmode%3D%22idle%22%7D"));
    }
}

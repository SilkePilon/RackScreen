//! Prometheus poller: cluster CPU/MEM, hot nodes, firing alerts, node
//! temperatures, Longhorn volume health, UPS state and network throughput.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Result};
use kube::Client;
use rackscreen_core::event::{Event, LinkTarget, NodeLoad, Robustness};
use rackscreen_core::theme::Role;
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

/// Values of the first series of a `query_range` matrix, in time order;
/// Prometheus' `"NaN"` strings come through as NaN.
pub fn parse_matrix(json: &str) -> Vec<f64> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(first) = v
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
    else {
        return Vec::new();
    };
    first
        .get("values")
        .and_then(|vs| vs.as_array())
        .map(|vs| {
            vs.iter()
                .filter_map(|pair| pair.as_array()?.get(1)?.as_str()?.parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Actual size per volume; Longhorn exports one series per node holding a
/// replica, so the largest sample per name wins.
pub fn volume_sizes_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, u64)> {
    let mut map: std::collections::BTreeMap<String, u64> = Default::default();
    for (m, val) in v {
        let Some(name) = m.get("volume") else {
            continue;
        };
        let bytes = val.max(0.0) as u64;
        let e = map.entry(name.clone()).or_insert(bytes);
        *e = (*e).max(bytes);
    }
    map.into_iter().collect()
}

pub fn range_path(q: &str, start: i64, end: i64, step: u32) -> String {
    format!(
        "/api/v1/query_range?query={}&start={start}&end={end}&step={step}",
        urlencoding::encode(q)
    )
}

pub fn history_event(cpu: Vec<f64>, mem: Vec<f64>, net_rx_bps: Vec<f64>) -> Event {
    Event::MetricsHistory {
        cpu: cpu.into_iter().map(|v| v as f32).collect(),
        mem: mem.into_iter().map(|v| v as f32).collect(),
        net_rx_mbps: net_rx_bps.into_iter().map(|v| (v / 1e6) as f32).collect(),
    }
}

async fn query_range(t: &mut Tunnel, q: &str, start: i64, end: i64, step: u32) -> Result<String> {
    let r = t.get(&range_path(q, start, end, step), &[]).await?;
    anyhow::ensure!(r.status == 200, "prometheus HTTP {}", r.status);
    Ok(r.body)
}

/// One range query per sparkline so the lines are full at boot: cpu and
/// download over the last 60 s at 2 s, memory over 90 s at 3 s, entrypoint
/// req/s over 90 s at 3 s.
async fn prefill(t: &mut Tunnel) -> Result<Vec<Event>> {
    let end = unix_now();
    let cpu = parse_matrix(&query_range(t, Q_CPU, end - 60, end, 2).await?);
    let mem = parse_matrix(&query_range(t, Q_MEM, end - 90, end, 3).await?);
    let net = parse_matrix(&query_range(t, Q_NET_RX, end - 60, end, 2).await?);
    let mut out = vec![history_event(cpu, mem, net)];
    let rps = parse_matrix(&query_range(t, Q_TRAFFIC_RPS, end - 90, end, 3).await?);
    if !rps.is_empty() {
        out.push(Event::TrafficHistory {
            req_per_sec: rps.into_iter().map(|v| v as f32).collect(),
        });
    }
    Ok(out)
}

/// Restarts per hour and the hourly mean mains voltage over the last 24 h,
/// one sample per hour ending now; re-run every `HOURLY_REFRESH_SECS`.
async fn hourly_prefill(t: &mut Tunnel) -> Result<Vec<Event>> {
    let end = unix_now();
    let mut out = Vec::new();
    let restarts = hourly_from(&parse_matrix_ts(
        &query_range(t, Q_RESTARTS_HOURLY, end - 23 * 3600, end, 3600).await?,
    ));
    if !restarts.is_empty() {
        out.push(Event::RestartsHistory { hourly: restarts });
    }
    let mains = hourly_from(&parse_matrix_ts(
        &query_range(t, Q_MAINS_HOURLY, end - 23 * 3600, end, 3600).await?,
    ));
    if !mains.is_empty() {
        out.push(Event::MainsHistory { hourly: mains });
    }
    Ok(out)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `192.168.1.5:9100` -> `.5`
pub fn node_tag(instance: &str) -> String {
    let ip = instance.split(':').next().unwrap_or(instance);
    match ip.rsplit('.').next() {
        Some(last) if last != ip => format!(".{last}"),
        _ => ip.to_string(),
    }
}

/// The node a series belongs to: the `node` label kube-prometheus adds, the
/// `nodename` of a `node_uname_info` join, else the instance's last octet.
pub fn node_name(m: &HashMap<String, String>) -> String {
    m.get("node")
        .or_else(|| m.get("nodename"))
        .cloned()
        .unwrap_or_else(|| node_tag(m.get("instance").map(String::as_str).unwrap_or("")))
}

/// `portfolio-portfolio-http@kubernetes` → `portfolio`: drop the provider,
/// a trailing port name or number, and a repeated `namespace-name` half.
pub fn short_service(s: &str) -> String {
    let base = s.split('@').next().unwrap_or(s);
    let mut toks: Vec<&str> = base.split('-').collect();
    if toks.len() > 1 {
        let last = toks[toks.len() - 1];
        let port_name = matches!(
            last,
            "http" | "https" | "web" | "webui" | "tcp" | "udp" | "svc"
        );
        if port_name || last.chars().all(|c| c.is_ascii_digit()) {
            toks.pop();
        }
    }
    let n = toks.len();
    if n >= 2 && n.is_multiple_of(2) && toks[..n / 2] == toks[n / 2..] {
        toks.truncate(n / 2);
    }
    toks.join("-")
}

/// `signal-cli-65cd44c9db-tf9t6` → `signal-cli`: drop the pod suffix and the
/// replica-set hash.
pub fn short_pod(pod: &str) -> String {
    let alnum = |t: &str| {
        t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    let mut toks: Vec<&str> = pod.split('-').collect();
    if toks.len() > 1 && toks[toks.len() - 1].len() == 5 && alnum(toks[toks.len() - 1]) {
        toks.pop();
    }
    if toks.len() > 1 {
        let last = toks[toks.len() - 1];
        if (8..=10).contains(&last.len()) && alnum(last) && last.chars().any(|c| c.is_ascii_digit())
        {
            toks.pop();
        }
    }
    toks.join("-")
}

/// `(timestamp, value)` pairs of the first series of a `query_range` matrix.
pub fn parse_matrix_ts(json: &str) -> Vec<(i64, f64)> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(first) = v
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
    else {
        return Vec::new();
    };
    first
        .get("values")
        .and_then(|vs| vs.as_array())
        .map(|vs| {
            vs.iter()
                .filter_map(|pair| {
                    let p = pair.as_array()?;
                    let ts = p.first()?.as_f64()? as i64;
                    let val = p.get(1)?.as_str()?.parse::<f64>().ok()?;
                    Some((ts, val))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `(unix hour, value)` per matrix sample, NaN samples dropped.
pub fn hourly_from(m: &[(i64, f64)]) -> Vec<(i64, f32)> {
    m.iter()
        .filter(|(_, v)| v.is_finite())
        .map(|(ts, v)| (ts.div_euclid(3600), *v as f32))
        .collect()
}

/// The largest sample per node, sorted by node name.
pub fn per_node(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, f32)> {
    let mut map: std::collections::BTreeMap<String, f32> = Default::default();
    for (m, val) in v {
        if !val.is_finite() {
            continue;
        }
        let e = map.entry(node_name(m)).or_insert(*val as f32);
        *e = e.max(*val as f32);
    }
    map.into_iter().collect()
}

/// `None` when Traefik exports nothing at all.
pub fn traffic_from(
    rps: Option<f64>,
    top: &[(HashMap<String, String>, f64)],
    all: Option<f64>,
    err: Option<f64>,
) -> Option<Event> {
    let rps = rps?;
    let mut top: Vec<(String, f32)> = top
        .iter()
        .map(|(m, v)| {
            (
                short_service(m.get("service").map(String::as_str).unwrap_or("?")),
                *v as f32,
            )
        })
        .collect();
    top.sort_by(|a, b| b.1.total_cmp(&a.1));
    top.truncate(3);
    let all = all.unwrap_or(0.0);
    let error_share = if all > 0.0 {
        (err.unwrap_or(0.0) / all) as f32
    } else {
        0.0
    };
    Some(Event::Traffic {
        req_per_sec: rps as f32,
        top,
        service_rps: all as f32,
        error_share,
    })
}

/// `None` when kube-state-metrics exports nothing.
pub fn restarts_from(total: Option<f64>, top: &[(HashMap<String, String>, f64)]) -> Option<Event> {
    let total = total?;
    let worst = top
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, v)| v.round() >= 1.0)
        .map(|(m, v)| {
            (
                short_pod(m.get("pod").map(String::as_str).unwrap_or("?")),
                v.round() as u32,
            )
        });
    Some(Event::Restarts {
        total_24h: total.round().max(0.0) as u32,
        worst,
    })
}

/// Load averages joined by node name; a node missing from a series reads 0.
pub fn load_from(
    l1: &[(HashMap<String, String>, f64)],
    l5: &[(HashMap<String, String>, f64)],
    l15: &[(HashMap<String, String>, f64)],
    cores: &[(HashMap<String, String>, f64)],
) -> Vec<NodeLoad> {
    let by = |v: &[(HashMap<String, String>, f64)]| -> HashMap<String, f32> {
        per_node(v).into_iter().collect()
    };
    let (m5, m15, mc) = (by(l5), by(l15), by(cores));
    per_node(l1)
        .into_iter()
        .map(|(node, load1)| NodeLoad {
            load5: m5.get(&node).copied().unwrap_or(0.0),
            load15: m15.get(&node).copied().unwrap_or(0.0),
            cores: mc.get(&node).copied().unwrap_or(1.0).max(1.0) as u32,
            node,
            load1,
        })
        .collect()
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
const Q_LH_SIZE: &str = "longhorn_volume_actual_size_bytes";
const Q_UPS_STATUS: &str = "nut_ups_status";
const Q_UPS_CHARGE: &str = "nut_battery_charge";
const Q_UPS_LOAD: &str = "nut_load";
const Q_RADARR: &str = "sum(radarr_movies_total)";
const Q_SONARR: &str = "sum(sonarr_series_total)";
const Q_LIDARR: &str = "sum(lidarr_artists_total)";
const Q_TRAFFIC_RPS: &str = "sum(rate(traefik_entrypoint_requests_total[1m]))";
const Q_TRAFFIC_TOP: &str = "topk(3, sum by (service)(rate(traefik_service_requests_total[5m])))";
/// The 5xx share is divided here, not in PromQL: a quiet site makes the
/// quotient an empty result, which would read as "no exporter".
const Q_TRAFFIC_ALL: &str = "sum(rate(traefik_service_requests_total[5m]))";
const Q_TRAFFIC_5XX: &str = "sum(rate(traefik_service_requests_total{code=~\"5..\"}[5m]))";
const Q_RESTARTS: &str = "sum(increase(kube_pod_container_status_restarts_total[24h]))";
const Q_RESTARTS_TOP: &str =
    "topk(1, sum by (namespace, pod)(increase(kube_pod_container_status_restarts_total[24h])))";
const Q_RESTARTS_HOURLY: &str = "sum(increase(kube_pod_container_status_restarts_total[1h]))";
const Q_UPTIME: &str = "time() - node_boot_time_seconds";
/// Fan power in percent: pwm duty 0–255 scaled to 0–100.
const Q_FANS: &str = "node_hwmon_pwm / 255 * 100";
const Q_MAINS_V: &str = "nut_input_voltage_volts";
const Q_MAINS_HOURLY: &str = "avg(nut_input_voltage_volts)";
const Q_LOAD1: &str = "node_load1";
const Q_LOAD5: &str = "node_load5";
const Q_LOAD15: &str = "node_load15";
const Q_CORES: &str = "count by (node, instance)(node_cpu_seconds_total{mode=\"idle\"})";
/// The hourly range queries (restarts, mains) are refreshed this often.
pub const HOURLY_REFRESH_SECS: u64 = 300;
const Q_UPS_RUNTIME: &str = "nut_battery_runtime_seconds";
/// Physical interfaces only: no veth, cni, flannel or bridge traffic, which
/// would count every pod packet twice. Two minutes so a 60 s scrape interval
/// still yields two samples for `rate`.
const Q_NET_RX: &str =
    "sum(rate(node_network_receive_bytes_total{device=~\"eth.*|end.*|enp.*|eno.*|enx.*|wlan.*|wlp.*|wlx.*\"}[2m]))*8";
const Q_NET_TX: &str = "sum(rate(node_network_transmit_bytes_total{device=~\"eth.*|end.*|enp.*|eno.*|enx.*|wlan.*|wlp.*|wlx.*\"}[2m]))*8";

/// The two common nut exporters disagree: one reports charge and load as
/// 0..1, the other as 0..100. At or below 1 is a fraction.
pub fn as_percent(v: f64) -> f32 {
    if v <= 1.0 {
        (v * 100.0) as f32
    } else {
        v as f32
    }
}

/// One `Event::Ups` from the status vector and the three gauges; `None`
/// when no UPS is exported at all.
pub fn ups_from(
    status: &[(HashMap<String, String>, f64)],
    charge: Option<f64>,
    load: Option<f64>,
    runtime: Option<f64>,
) -> Option<Event> {
    if status.is_empty() {
        return None;
    }
    let flag = |f: &str| {
        status
            .iter()
            .any(|(m, v)| m.get("status").map(String::as_str) == Some(f) && *v > 0.0)
    };
    Some(Event::Ups {
        on_battery: flag("OB"),
        low_battery: flag("LB"),
        charge_pct: charge.map(as_percent).unwrap_or(0.0),
        load_pct: load.map(as_percent).unwrap_or(0.0),
        runtime_secs: runtime.unwrap_or(0.0).max(0.0) as u32,
    })
}

/// On-battery edges between polls; the first poll only primes so a restart
/// during an outage does not replay the red sweep.
#[derive(Debug, Default)]
pub struct UpsTracker {
    primed: bool,
    on_battery: bool,
}

impl UpsTracker {
    pub fn diff(&mut self, on_battery: bool) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed && on_battery != self.on_battery {
            out.push(if on_battery {
                Event::UpsOnBattery
            } else {
                Event::UpsOnline
            });
        }
        self.on_battery = on_battery;
        self.primed = true;
        out
    }
}

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
    ups: &mut UpsTracker,
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
        let sizes = volume_sizes_from(&parse_vector(&query(t, Q_LH_SIZE).await?));
        if !sizes.is_empty() {
            out.push(Event::VolumeSizes(sizes));
        }
    }
    let status = parse_vector(&query(t, Q_UPS_STATUS).await?);
    if !status.is_empty() {
        let charge = parse_scalar(&query(t, Q_UPS_CHARGE).await?);
        let load = parse_scalar(&query(t, Q_UPS_LOAD).await?);
        let runtime = parse_scalar(&query(t, Q_UPS_RUNTIME).await?);
        if let Some(ev) = ups_from(&status, charge, load, runtime) {
            if let Event::Ups { on_battery, .. } = &ev {
                out.extend(ups.diff(*on_battery));
            }
            out.push(ev);
        }
    }
    let rx = parse_scalar(&query(t, Q_NET_RX).await?);
    let tx = parse_scalar(&query(t, Q_NET_TX).await?);
    if let (Some(rx_bps), Some(tx_bps)) = (rx, tx) {
        out.push(Event::Network { rx_bps, tx_bps });
    }
    // media library
    let movies = parse_scalar(&query(t, Q_RADARR).await?);
    let series = parse_scalar(&query(t, Q_SONARR).await?);
    let artists = parse_scalar(&query(t, Q_LIDARR).await?);
    out.push(
        if movies.is_none() && series.is_none() && artists.is_none() {
            Event::NoExporter { role: Role::Media }
        } else {
            Event::MediaLibrary {
                movies: movies.unwrap_or(0.0).max(0.0) as u32,
                series: series.unwrap_or(0.0).max(0.0) as u32,
                artists: artists.unwrap_or(0.0).max(0.0) as u32,
            }
        },
    );
    // traffic
    let rps = parse_scalar(&query(t, Q_TRAFFIC_RPS).await?);
    let top = parse_vector(&query(t, Q_TRAFFIC_TOP).await?);
    let all = parse_scalar(&query(t, Q_TRAFFIC_ALL).await?);
    let err = parse_scalar(&query(t, Q_TRAFFIC_5XX).await?);
    out.push(
        traffic_from(rps, &top, all, err).unwrap_or(Event::NoExporter {
            role: Role::Traffic,
        }),
    );
    // restarts
    let total = parse_scalar(&query(t, Q_RESTARTS).await?);
    let worst = parse_vector(&query(t, Q_RESTARTS_TOP).await?);
    out.push(restarts_from(total, &worst).unwrap_or(Event::NoExporter {
        role: Role::Restarts,
    }));
    // uptime
    let up = per_node(&parse_vector(&query(t, Q_UPTIME).await?));
    out.push(if up.is_empty() {
        Event::NoExporter { role: Role::Uptime }
    } else {
        Event::Uptime(up.into_iter().map(|(n, s)| (n, s as f64)).collect())
    });
    // fans
    let fans = per_node(&parse_vector(&query(t, Q_FANS).await?));
    out.push(if fans.is_empty() {
        Event::NoExporter { role: Role::Fans }
    } else {
        Event::Fans(fans)
    });
    // mains
    out.push(match parse_scalar(&query(t, Q_MAINS_V).await?) {
        Some(volts) => Event::Mains {
            volts: volts as f32,
            ups_load_pct: parse_scalar(&query(t, Q_UPS_LOAD).await?)
                .map(as_percent)
                .unwrap_or(0.0),
        },
        None => Event::NoExporter { role: Role::Mains },
    });
    // load
    let l1 = parse_vector(&query(t, Q_LOAD1).await?);
    let l5 = parse_vector(&query(t, Q_LOAD5).await?);
    let l15 = parse_vector(&query(t, Q_LOAD15).await?);
    let cores = parse_vector(&query(t, Q_CORES).await?);
    let nodes = load_from(&l1, &l5, &l15, &cores);
    out.push(if nodes.is_empty() {
        Event::NoExporter { role: Role::Load }
    } else {
        Event::Load(nodes)
    });
    Ok(out)
}

pub async fn run_prometheus(client: Client, cfg: PromConfig, ctx: SourceCtx) {
    let mut alerts = AlertTracker::new(&cfg.ignore_alerts);
    let mut storage = StorageTracker::new();
    let mut ups = UpsTracker::default();
    let mut backoff = 5u64;
    let mut prefilled = false;
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
        if !prefilled {
            match prefill(&mut t).await {
                Ok(evs) => {
                    ctx.emit_all(evs);
                    prefilled = true;
                }
                Err(e) => tracing::warn!("prometheus prefill: {e:#}"),
            }
        }
        let mut hourly_at: Option<std::time::Instant> = None;
        let mut failures = 0;
        loop {
            if hourly_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(HOURLY_REFRESH_SECS)) {
                match hourly_prefill(&mut t).await {
                    Ok(evs) => {
                        ctx.emit_all(evs);
                        hourly_at = Some(std::time::Instant::now());
                    }
                    Err(e) => tracing::warn!("prometheus hourly ranges: {e:#}"),
                }
            }
            match poll(&mut t, &mut alerts, &mut storage, &mut ups).await {
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

    fn status(flags: &[(&str, f64)]) -> Vec<(HashMap<String, String>, f64)> {
        flags
            .iter()
            .map(|(f, v)| {
                let mut m = HashMap::new();
                m.insert("status".to_string(), f.to_string());
                m.insert("ups".to_string(), "apc".to_string());
                (m, *v)
            })
            .collect()
    }

    #[test]
    fn ups_event_reads_flags_and_normalises_fractions() {
        let ev = ups_from(
            &status(&[("OL", 1.0), ("OB", 0.0), ("LB", 0.0)]),
            Some(1.0),
            Some(0.06),
            Some(3014.0),
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 6.0,
                runtime_secs: 3014,
            }
        );
        let ev = ups_from(
            &status(&[("OL", 0.0), ("OB", 1.0), ("LB", 1.0)]),
            Some(18.0),
            Some(42.0),
            None,
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Ups {
                on_battery: true,
                low_battery: true,
                charge_pct: 18.0,
                load_pct: 42.0,
                runtime_secs: 0,
            }
        );
        assert!(
            ups_from(&[], Some(1.0), None, None).is_none(),
            "no UPS, no event"
        );
        assert_eq!(as_percent(0.5), 50.0);
        assert_eq!(as_percent(1.0), 100.0);
        assert_eq!(as_percent(75.0), 75.0);
    }

    #[test]
    fn ups_tracker_edges_after_priming() {
        let mut t = UpsTracker::default();
        assert!(t.diff(true).is_empty(), "first poll only primes");
        assert!(t.diff(true).is_empty());
        assert_eq!(t.diff(false), vec![Event::UpsOnline]);
        assert_eq!(t.diff(true), vec![Event::UpsOnBattery]);
    }

    #[test]
    fn query_path_is_encoded() {
        let p = format!("/api/v1/query?query={}", urlencoding::encode(Q_CPU));
        assert!(p.contains("%7Bmode%3D%22idle%22%7D"));
    }
    const MATRIX: &str = r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1,"4.5"],[3,"4.7"],[5,"NaN"],[7,"5.1"]]}]}}"#;

    #[test]
    fn matrix_values_in_order_with_nan_as_nan() {
        let v = parse_matrix(MATRIX);
        assert_eq!(v.len(), 4);
        assert_eq!((v[0], v[1], v[3]), (4.5, 4.7, 5.1));
        assert!(v[2].is_nan());
        assert!(parse_matrix(r#"{"data":{"result":[]}}"#).is_empty());
        assert!(parse_matrix("nope").is_empty());
    }

    #[test]
    fn range_path_covers_the_spark_window() {
        let p = range_path(Q_CPU, 1000, 1060, 2);
        assert!(p.starts_with("/api/v1/query_range?query="));
        assert!(p.ends_with("&start=1000&end=1060&step=2"));
        let ev = history_event(vec![1.0, 2.0], vec![3.0], vec![4e6, 8e6]);
        assert_eq!(
            ev,
            Event::MetricsHistory {
                cpu: vec![1.0, 2.0],
                mem: vec![3.0],
                net_rx_mbps: vec![4.0, 8.0],
            }
        );
    }
    #[test]
    fn volume_sizes_keep_the_largest_sample_per_volume() {
        let v = parse_vector(
            r#"{"data":{"result":[
            {"metric":{"volume":"pvc-a","node":"n1"},"value":[1,"100"]},
            {"metric":{"volume":"pvc-a","node":"n2"},"value":[1,"120"]},
            {"metric":{"volume":"pvc-b","node":"n1"},"value":[1,"7"]}]}}"#,
        );
        assert_eq!(
            volume_sizes_from(&v),
            vec![("pvc-a".into(), 120), ("pvc-b".into(), 7)]
        );
    }
    #[test]
    fn node_name_prefers_the_node_label() {
        let mut m = HashMap::new();
        m.insert("instance".to_string(), "192.168.0.98:9100".to_string());
        assert_eq!(node_name(&m), ".98");
        m.insert("node".to_string(), "raspberrypi-5-16gb-1".to_string());
        assert_eq!(node_name(&m), "raspberrypi-5-16gb-1");
        let mut n = HashMap::new();
        n.insert("nodename".to_string(), "hp-1".to_string());
        assert_eq!(node_name(&n), "hp-1");
        assert_eq!(node_name(&HashMap::new()), "");
    }

    #[test]
    fn service_and_pod_names_are_shortened() {
        assert_eq!(
            short_service("portfolio-portfolio-http@kubernetes"),
            "portfolio"
        );
        assert_eq!(short_service("n8n-n8n-http@kubernetes"), "n8n");
        assert_eq!(
            short_service("windesheim-readers-windesheim-readers-http@kubernetes"),
            "windesheim-readers"
        );
        assert_eq!(
            short_service("arr-stack-sonarr-webui@kubernetes"),
            "arr-stack-sonarr"
        );
        assert_eq!(
            short_service("jotihunt-client-http@kubernetes"),
            "jotihunt-client"
        );
        assert_eq!(short_service("grafana-3000@kubernetes"), "grafana");
        assert_eq!(short_service("api@file"), "api");
        assert_eq!(short_pod("signal-cli-65cd44c9db-tf9t6"), "signal-cli");
        assert_eq!(short_pod("csi-smb-node-jvfhl"), "csi-smb-node");
        assert_eq!(
            short_pod("longhorn-csi-plugin-f9zw2"),
            "longhorn-csi-plugin"
        );
        assert_eq!(short_pod("prometheus-0"), "prometheus-0");
        assert_eq!(short_pod("redis"), "redis");
    }

    #[test]
    fn matrix_with_timestamps_and_hourly_keys() {
        let v = parse_matrix_ts(MATRIX);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0], (1, 4.5));
        assert!(v[2].1.is_nan());
        let h = hourly_from(&[(3_600_000, 1.0), (3_603_600, 2.5), (3_607_200, f64::NAN)]);
        assert_eq!(h, vec![(1000, 1.0), (1001, 2.5)], "NaN hours are dropped");
        assert!(parse_matrix_ts("nope").is_empty());
    }

    fn labelled(pairs: &[(&[(&str, &str)], f64)]) -> Vec<(HashMap<String, String>, f64)> {
        pairs
            .iter()
            .map(|(ls, v)| {
                (
                    ls.iter()
                        .map(|(k, x)| (k.to_string(), x.to_string()))
                        .collect(),
                    *v,
                )
            })
            .collect()
    }

    #[test]
    fn traffic_restarts_and_load_events() {
        let top = labelled(&[
            (&[("service", "n8n-n8n-http@kubernetes")], 0.022),
            (&[("service", "portfolio-portfolio-http@kubernetes")], 0.015),
            (&[("service", "jotihunt-client-http@kubernetes")], 0.0),
        ]);
        let ev = traffic_from(Some(0.0333), &top, Some(0.037), Some(0.002)).unwrap();
        assert_eq!(
            ev,
            Event::Traffic {
                req_per_sec: 0.0333,
                top: vec![
                    ("n8n".into(), 0.022),
                    ("portfolio".into(), 0.015),
                    ("jotihunt-client".into(), 0.0)
                ],
                service_rps: 0.037,
                error_share: 0.002 / 0.037,
            }
        );
        assert!(
            matches!(traffic_from(Some(1.0), &[], Some(0.0), None).unwrap(), Event::Traffic { error_share, .. } if error_share == 0.0)
        );
        assert!(
            traffic_from(None, &top, None, None).is_none(),
            "no entrypoint series: no exporter"
        );
        let worst = labelled(&[(
            &[
                ("namespace", "hermes"),
                ("pod", "signal-cli-65cd44c9db-tf9t6"),
            ],
            36.14,
        )]);
        assert_eq!(
            restarts_from(Some(56.148), &worst),
            Some(Event::Restarts {
                total_24h: 56,
                worst: Some(("signal-cli".into(), 36)),
            })
        );
        assert_eq!(
            restarts_from(Some(0.0), &[]),
            Some(Event::Restarts {
                total_24h: 0,
                worst: None,
            })
        );
        assert_eq!(restarts_from(None, &[]), None);
        let l1 = labelled(&[
            (&[("node", "pi-1"), ("instance", "a")], 0.81),
            (&[("node", "hp-1"), ("instance", "b")], 0.19),
        ]);
        let l5 = labelled(&[(&[("node", "pi-1")], 0.7), (&[("node", "hp-1")], 0.2)]);
        let l15 = labelled(&[(&[("node", "pi-1")], 0.6)]);
        let cores = labelled(&[(&[("node", "pi-1")], 4.0), (&[("node", "hp-1")], 12.0)]);
        let nodes = load_from(&l1, &l5, &l15, &cores);
        assert_eq!(nodes.len(), 2);
        let pi = nodes.iter().find(|n| n.node == "pi-1").unwrap();
        assert_eq!(
            (pi.load1, pi.load5, pi.load15, pi.cores),
            (0.81, 0.7, 0.6, 4)
        );
        let hp = nodes.iter().find(|n| n.node == "hp-1").unwrap();
        assert_eq!((hp.load15, hp.cores), (0.0, 12), "a missing series reads 0");
        assert!(load_from(&[], &l5, &l15, &cores).is_empty());
        // `node_hwmon_pwm / 255 * 100`: 125 → 49 %, 255 → 100 %.
        assert!(Q_FANS.contains("node_hwmon_pwm") && Q_FANS.contains("/ 255 * 100"));
        assert!(!Q_FANS.contains("rpm"));
        let fans = labelled(&[
            (
                &[("node", "pi-1"), ("sensor", "pwm1")],
                125.0 / 255.0 * 100.0,
            ),
            (&[("node", "pi-1"), ("sensor", "pwm2")], 100.0),
            (
                &[("node", "pi-2"), ("sensor", "pwm1")],
                125.0 / 255.0 * 100.0,
            ),
        ]);
        let got = per_node(&fans);
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].0.as_str(), got[0].1),
            ("pi-1", 100.0),
            "max per node"
        );
        assert_eq!(got[1].0, "pi-2", "sorted");
        assert!((got[1].1 - 49.02).abs() < 0.01);
    }
}

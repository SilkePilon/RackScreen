//! Summary tiles for the web UI's overview: a short, free list of what the
//! model knows right now, rendered generically by the browser.

use crate::model::Model;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Bad,
    Muted,
}

impl Tone {
    pub fn name(self) -> &'static str {
        match self {
            Tone::Ok => "ok",
            Tone::Warn => "warn",
            Tone::Bad => "bad",
            Tone::Muted => "muted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub unit: &'static str,
    pub tone: Tone,
}

fn tile(
    key: &'static str,
    label: &'static str,
    value: String,
    unit: &'static str,
    tone: Tone,
) -> Tile {
    Tile {
        key,
        label,
        value,
        unit,
        tone,
    }
}

impl Model {
    /// The summary tiles, in a fixed order; only what has data.
    pub fn tiles(&self) -> Vec<Tile> {
        let mut out = Vec::new();
        let s = self.state();
        let th = self.thresholds();
        if s.have_metrics {
            let tone = if s.cpu_pct >= th.hot_cpu {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "cpu",
                "Cluster CPU",
                format!("{:.0}", s.cpu_pct),
                "%",
                tone,
            ));
            let tone = if s.mem_pct >= th.hot_mem {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "mem",
                "Memory",
                format!("{:.0}", s.mem_pct),
                "%",
                tone,
            ));
        }
        if s.have_pods {
            let tone = if s.pods_failed > 0 {
                Tone::Bad
            } else if s.pods_pending > 0 {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "pods",
                "Pods running",
                format!("{}/{}", s.pods_running, s.pods_total),
                "",
                tone,
            ));
        }
        if s.have_nodes {
            let tone = if s.nodes_ready < s.nodes_total {
                Tone::Bad
            } else {
                Tone::Ok
            };
            out.push(tile(
                "nodes",
                "Nodes ready",
                format!("{}/{}", s.nodes_ready, s.nodes_total),
                "",
                tone,
            ));
        }
        if s.have_nodes || s.have_metrics {
            let n = s.alerts.len();
            let tone = if n == 0 { Tone::Ok } else { Tone::Warn };
            out.push(tile("alerts", "Alerts firing", n.to_string(), "", tone));
        }
        if s.have_temps {
            if let Some((_, t)) = s.temps.iter().max_by(|a, b| a.1.total_cmp(&b.1)) {
                let tone = if *t >= th.hot_temp {
                    Tone::Bad
                } else if *t >= th.hot_temp - 5.0 {
                    Tone::Warn
                } else {
                    Tone::Ok
                };
                out.push(tile("temp", "Hottest node", format!("{t:.0}"), "°C", tone));
            }
        }
        if s.have_storage && s.storage_capacity > 0 {
            let pct = s.storage_used as f64 / s.storage_capacity as f64 * 100.0;
            let degraded = s
                .volumes
                .iter()
                .any(|(_, r)| !matches!(r, crate::event::Robustness::Healthy));
            let tone = if degraded {
                Tone::Bad
            } else if pct >= 90.0 {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "storage",
                "Storage used",
                format!("{pct:.0}"),
                "%",
                tone,
            ));
        }
        let e = self.electricity();
        if e.have {
            out.push(tile(
                "renewable",
                "Renewable share",
                format!("{:.0}", e.renewable_pct),
                "%",
                Tone::Ok,
            ));
        }
        let w = self.weather();
        if w.have {
            out.push(tile(
                "weather",
                "Outside",
                format!("{:.1}", w.temp_c),
                "°C",
                Tone::Ok,
            ));
        }
        let u = self.ups();
        if u.have {
            let tone = if u.low_battery {
                Tone::Bad
            } else if u.on_battery {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "ups",
                "UPS charge",
                format!("{:.0}", u.charge_pct),
                "%",
                tone,
            ));
        }
        let d = self.deploys_log();
        let syncs: u32 = d.hours.iter().sum();
        if self.apps().have {
            out.push(tile(
                "deploys",
                "Deploys today",
                syncs.to_string(),
                "",
                Tone::Ok,
            ));
        }
        let r = self.restarts();
        if r.have {
            let tone = if r.total_24h >= 6 {
                Tone::Bad
            } else if r.total_24h > 0 {
                Tone::Warn
            } else {
                Tone::Ok
            };
            out.push(tile(
                "restarts",
                "Restarts 24 h",
                r.total_24h.to_string(),
                "",
                tone,
            ));
        }
        let g = self.github();
        if g.have {
            out.push(tile(
                "github",
                "Contributions today",
                g.today().to_string(),
                "",
                Tone::Ok,
            ));
        }
        if let Some(t) = self.music().current() {
            let tone = if t.playing { Tone::Ok } else { Tone::Muted };
            out.push(tile(
                "music",
                "Now playing",
                format!("{} · {}", t.title, t.artist),
                "",
                tone,
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn model() -> Model {
        Model::new(Thresholds {
            hot_cpu: 90.0,
            hot_mem: 90.0,
            hot_temp: 70.0,
        })
    }

    #[test]
    fn empty_model_has_no_tiles() {
        assert!(model().tiles().is_empty());
    }

    #[test]
    fn tiles_follow_the_data_and_thresholds() {
        let mut m = model();
        m.apply(
            Event::Metrics {
                cpu_pct: 95.0,
                mem_pct: 20.0,
                mem_used_gb: 1.0,
                mem_total_gb: 8.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m.apply(
            Event::PodSnapshot {
                running: 10,
                pending: 1,
                failed: 0,
                total: 11,
            },
            0.0,
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 2,
                total: 3,
                not_ready: vec!["n3".into()],
            },
            0.0,
        );
        m.apply(
            Event::NodeTemps(vec![("a".into(), 50.0), ("b".into(), 68.0)]),
            0.0,
        );
        let tiles = m.tiles();
        let keys: Vec<&str> = tiles.iter().map(|t| t.key).collect();
        assert_eq!(keys, vec!["cpu", "mem", "pods", "nodes", "alerts", "temp"]);
        let by = |k: &str| tiles.iter().find(|t| t.key == k).unwrap();
        assert_eq!(
            (by("cpu").value.as_str(), by("cpu").unit, by("cpu").tone),
            ("95", "%", Tone::Warn)
        );
        assert_eq!(by("mem").tone, Tone::Ok);
        assert_eq!(
            (by("pods").value.as_str(), by("pods").tone),
            ("10/11", Tone::Warn)
        );
        assert_eq!(
            (by("nodes").value.as_str(), by("nodes").tone),
            ("2/3", Tone::Bad)
        );
        assert_eq!(
            (by("alerts").value.as_str(), by("alerts").tone),
            ("0", Tone::Ok)
        );
        assert_eq!(
            (by("temp").value.as_str(), by("temp").unit, by("temp").tone),
            ("68", "°C", Tone::Warn)
        );
        assert_eq!(Tone::Muted.name(), "muted");
    }
}

//! Scripted data for the simulator. Drifts metrics, churns pods, and reacts to keyboard commands.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use rackscreen_core::event::{Event, LinkTarget, Torrent};

use crate::SourceCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCmd {
    PodStarted,
    PodCrashed,
    NodeDown,
    NodeUp,
    AlertToggle,
    TorrentDone,
    LinkDown,
    LinkUp,
    ToggleTorrent,
    ToggleNight,
    Boot,
}

impl FakeCmd {
    pub fn from_key(c: char) -> Option<FakeCmd> {
        Some(match c {
            '1' => FakeCmd::PodStarted,
            '2' => FakeCmd::PodCrashed,
            '3' => FakeCmd::NodeDown,
            '4' => FakeCmd::NodeUp,
            '5' => FakeCmd::AlertToggle,
            '6' => FakeCmd::TorrentDone,
            '7' => FakeCmd::LinkDown,
            '8' => FakeCmd::LinkUp,
            't' => FakeCmd::ToggleTorrent,
            'n' => FakeCmd::ToggleNight,
            'b' => FakeCmd::Boot,
            _ => return None,
        })
    }
}

pub struct FakeState {
    rng: fastrand::Rng,
    cpu: f32,
    mem: f32,
    running: u32,
    pending: u32,
    failed: u32,
    nodes_total: u32,
    nodes_ready: u32,
    alerts: Vec<String>,
    torrents_on: bool,
    torrents: Vec<Torrent>,
    link_up: bool,
    night: Option<bool>,
    ticks: u64,
}

impl FakeState {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: fastrand::Rng::with_seed(seed),
            cpu: 42.0,
            mem: 67.0,
            running: 53,
            pending: 0,
            failed: 0,
            nodes_total: 4,
            nodes_ready: 4,
            alerts: Vec::new(),
            torrents_on: false,
            torrents: Vec::new(),
            link_up: true,
            night: None,
            ticks: 0,
        }
    }

    fn total(&self) -> u32 {
        self.running + self.pending + self.failed
    }

    fn pod_snapshot(&self) -> Event {
        Event::PodSnapshot {
            running: self.running,
            pending: self.pending,
            failed: self.failed,
            total: self.total(),
        }
    }

    fn node_snapshot(&self) -> Event {
        let not_ready = (self.nodes_ready..self.nodes_total)
            .map(|i| format!("node-{}", i + 1))
            .collect();
        Event::NodeSnapshot {
            ready: self.nodes_ready,
            total: self.nodes_total,
            not_ready,
        }
    }

    fn metrics(&self) -> Event {
        Event::Metrics {
            cpu_pct: self.cpu,
            mem_pct: self.mem,
            mem_used_gb: 16.0 * self.mem / 100.0,
            mem_total_gb: 16.0,
            hot_cpu: Some((".5".into(), self.cpu + 20.0)),
            hot_mem: Some((".7".into(), self.mem + 5.0)),
        }
    }

    pub fn initial(&self) -> Vec<Event> {
        vec![
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            Event::Link {
                target: LinkTarget::Prometheus,
                up: true,
            },
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            self.metrics(),
            self.pod_snapshot(),
            self.node_snapshot(),
            Event::AlertSnapshot {
                firing: self.alerts.clone(),
            },
            Event::Torrents(self.torrents.clone()),
        ]
    }

    /// One second of simulated time.
    pub fn tick(&mut self) -> Vec<Event> {
        self.ticks += 1;
        if !self.link_up {
            return Vec::new();
        }
        self.cpu = (self.cpu + self.rng.f32() * 6.0 - 3.0).clamp(5.0, 98.0);
        self.mem = (self.mem + self.rng.f32() * 2.0 - 1.0).clamp(20.0, 95.0);
        let mut out = vec![self.metrics()];
        if self.pending > 0 && self.rng.f32() < 0.5 {
            self.pending -= 1;
            self.running += 1;
            out.push(Event::PodStarted {
                ns: "fake".into(),
                name: format!("pod-{}", self.ticks),
            });
            out.push(self.pod_snapshot());
        } else if self.ticks.is_multiple_of(20) {
            self.pending += 1;
            out.push(self.pod_snapshot());
        }
        if self.torrents_on {
            for (i, t) in self.torrents.iter_mut().enumerate() {
                t.progress = (t.progress + 0.4 / (i as f32 + 1.0)).min(99.0);
                t.eta_secs = (t.eta_secs - 1).max(1);
            }
            out.push(Event::Torrents(self.torrents.clone()));
        }
        if self.ticks.is_multiple_of(5) {
            out.push(self.node_snapshot());
            out.push(Event::AlertSnapshot {
                firing: self.alerts.clone(),
            });
        }
        out
    }

    pub fn command(&mut self, cmd: FakeCmd) -> Vec<Event> {
        match cmd {
            FakeCmd::PodStarted => {
                if self.failed > 0 {
                    self.failed -= 1;
                }
                self.running += 1;
                vec![
                    Event::PodStarted {
                        ns: "fake".into(),
                        name: "manual".into(),
                    },
                    self.pod_snapshot(),
                ]
            }
            FakeCmd::PodCrashed => {
                self.running = self.running.saturating_sub(1);
                self.failed += 1;
                vec![
                    Event::PodCrashed {
                        ns: "fake".into(),
                        name: "manual".into(),
                    },
                    self.pod_snapshot(),
                ]
            }
            FakeCmd::NodeDown => {
                if self.nodes_ready == 0 {
                    return Vec::new();
                }
                self.nodes_ready -= 1;
                let name = format!("node-{}", self.nodes_ready + 1);
                vec![
                    Event::NodeReady { name, ready: false },
                    self.node_snapshot(),
                ]
            }
            FakeCmd::NodeUp => {
                if self.nodes_ready == self.nodes_total {
                    return Vec::new();
                }
                self.nodes_ready += 1;
                let name = format!("node-{}", self.nodes_ready);
                vec![Event::NodeReady { name, ready: true }, self.node_snapshot()]
            }
            FakeCmd::AlertToggle => {
                if self.alerts.is_empty() {
                    self.alerts.push("HighLoad".into());
                    vec![
                        Event::AlertChanged {
                            name: "HighLoad".into(),
                            firing: true,
                        },
                        Event::AlertSnapshot {
                            firing: self.alerts.clone(),
                        },
                    ]
                } else {
                    self.alerts.clear();
                    vec![
                        Event::AlertChanged {
                            name: "HighLoad".into(),
                            firing: false,
                        },
                        Event::AlertSnapshot { firing: vec![] },
                    ]
                }
            }
            FakeCmd::TorrentDone => {
                if self.torrents.is_empty() {
                    return Vec::new();
                }
                let done = self.torrents.remove(0);
                if self.torrents.is_empty() {
                    self.torrents_on = false;
                }
                vec![
                    Event::TorrentDone { name: done.name },
                    Event::Torrents(self.torrents.clone()),
                ]
            }
            FakeCmd::LinkDown => {
                self.link_up = false;
                vec![Event::Link {
                    target: LinkTarget::K8sApi,
                    up: false,
                }]
            }
            FakeCmd::LinkUp => {
                self.link_up = true;
                vec![Event::Link {
                    target: LinkTarget::K8sApi,
                    up: true,
                }]
            }
            FakeCmd::ToggleTorrent => {
                self.torrents_on = !self.torrents_on;
                if self.torrents_on {
                    self.torrents = vec![
                        Torrent {
                            name: "ubuntu-24.04.iso".into(),
                            progress: 78.0,
                            eta_secs: 900,
                            speed_bps: 9_000_000,
                        },
                        Torrent {
                            name: "debian-13.iso".into(),
                            progress: 41.0,
                            eta_secs: 3000,
                            speed_bps: 3_000_000,
                        },
                        Torrent {
                            name: "fedora-44.iso".into(),
                            progress: 12.0,
                            eta_secs: 9000,
                            speed_bps: 1_000_000,
                        },
                    ];
                    vec![
                        Event::TorrentAdded {
                            name: "ubuntu-24.04.iso".into(),
                        },
                        Event::Torrents(self.torrents.clone()),
                    ]
                } else {
                    self.torrents.clear();
                    vec![Event::Torrents(vec![])]
                }
            }
            FakeCmd::ToggleNight => {
                self.night = match self.night {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                vec![Event::ForceNight(self.night)]
            }
            FakeCmd::Boot => vec![Event::Boot],
        }
    }
}

pub async fn run_fake(ctx: SourceCtx, cmds: Receiver<FakeCmd>, seed: u64) {
    let mut st = FakeState::new(seed);
    ctx.emit_all(st.initial());
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = ticker.tick() => ctx.emit_all(st.tick()),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                while let Ok(cmd) = cmds.try_recv() {
                    ctx.emit_all(st.command(cmd));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map() {
        assert_eq!(FakeCmd::from_key('1'), Some(FakeCmd::PodStarted));
        assert_eq!(FakeCmd::from_key('n'), Some(FakeCmd::ToggleNight));
        assert_eq!(FakeCmd::from_key('x'), None);
    }

    #[test]
    fn initial_brings_links_up() {
        let s = FakeState::new(1);
        let evs = s.initial();
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true
            }
        )));
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::PodSnapshot { running: 53, .. })));
    }

    #[test]
    fn tick_is_deterministic_and_emits_metrics() {
        let mut a = FakeState::new(7);
        let mut b = FakeState::new(7);
        for _ in 0..30 {
            assert_eq!(a.tick(), b.tick());
        }
        assert!(matches!(a.tick()[0], Event::Metrics { .. }));
    }

    #[test]
    fn crash_and_recover() {
        let mut s = FakeState::new(1);
        let evs = s.command(FakeCmd::PodCrashed);
        assert!(matches!(evs[0], Event::PodCrashed { .. }));
        assert!(matches!(
            evs[1],
            Event::PodSnapshot {
                failed: 1,
                running: 52,
                ..
            }
        ));
        let evs = s.command(FakeCmd::PodStarted);
        assert!(matches!(
            evs[1],
            Event::PodSnapshot {
                failed: 0,
                running: 53,
                ..
            }
        ));
    }

    #[test]
    fn node_down_up_and_bounds() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::NodeUp).is_empty());
        let evs = s.command(FakeCmd::NodeDown);
        assert!(matches!(&evs[0], Event::NodeReady { ready: false, .. }));
        assert!(matches!(
            &evs[1],
            Event::NodeSnapshot {
                ready: 3,
                total: 4,
                ..
            }
        ));
        let evs = s.command(FakeCmd::NodeUp);
        assert!(matches!(&evs[1], Event::NodeSnapshot { ready: 4, .. }));
    }

    #[test]
    fn torrent_toggle_and_done() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::TorrentDone).is_empty());
        let evs = s.command(FakeCmd::ToggleTorrent);
        assert!(matches!(evs[0], Event::TorrentAdded { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 3));
        let evs = s.command(FakeCmd::TorrentDone);
        assert!(matches!(evs[0], Event::TorrentDone { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 2));
    }

    #[test]
    fn link_down_silences_ticks() {
        let mut s = FakeState::new(1);
        s.command(FakeCmd::LinkDown);
        assert!(s.tick().is_empty());
        s.command(FakeCmd::LinkUp);
        assert!(!s.tick().is_empty());
    }

    #[test]
    fn night_cycles() {
        let mut s = FakeState::new(1);
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(Some(true))]
        );
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(Some(false))]
        );
        assert_eq!(
            s.command(FakeCmd::ToggleNight),
            vec![Event::ForceNight(None)]
        );
    }
}

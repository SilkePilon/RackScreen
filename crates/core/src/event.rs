//! Events emitted by sources and folded into the model.

#[derive(Clone, Debug, PartialEq)]
pub struct Torrent {
    pub name: String,
    /// 0.0 ..= 100.0
    pub progress: f32,
    /// seconds, negative or huge means unknown
    pub eta_secs: i64,
    pub speed_bps: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Robustness {
    Healthy,
    Degraded,
    Faulted,
    Unknown,
}

impl Robustness {
    /// Longhorn's `longhorn_volume_robustness` value.
    pub fn from_code(v: f64) -> Robustness {
        match v as i64 {
            1 => Robustness::Healthy,
            2 => Robustness::Degraded,
            3 => Robustness::Faulted,
            _ => Robustness::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    K8sApi,
    Prometheus,
    QBittorrent,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Metrics {
        cpu_pct: f32,
        mem_pct: f32,
        mem_used_gb: f32,
        mem_total_gb: f32,
        hot_cpu: Option<(String, f32)>,
        hot_mem: Option<(String, f32)>,
    },
    PodSnapshot {
        running: u32,
        pending: u32,
        failed: u32,
        total: u32,
    },
    PodStarted {
        ns: String,
        name: String,
    },
    PodCrashed {
        ns: String,
        name: String,
    },
    PodGone {
        ns: String,
        name: String,
    },
    NodeSnapshot {
        ready: u32,
        total: u32,
        not_ready: Vec<String>,
    },
    NodeReady {
        name: String,
        ready: bool,
    },
    AlertSnapshot {
        firing: Vec<String>,
    },
    AlertChanged {
        name: String,
        firing: bool,
    },
    /// Per-node temperature in degrees Celsius.
    NodeTemps(Vec<(String, f32)>),
    Storage {
        volumes: Vec<(String, Robustness)>,
        used_bytes: u64,
        capacity_bytes: u64,
    },
    HotTemp {
        node: String,
        celsius: f32,
    },
    VolumeDegraded {
        name: String,
        robustness: Robustness,
    },
    VolumeHealthy {
        name: String,
    },
    Torrents(Vec<Torrent>),
    TorrentAdded {
        name: String,
    },
    TorrentDone {
        name: String,
    },
    Link {
        target: LinkTarget,
        up: bool,
    },
    /// Replay the boot animation (simulator key `b`, and on startup).
    Boot,
    /// Simulator override: Some(true) forces night, Some(false) forces day, None back to clock.
    ForceNight(Option<bool>),
}

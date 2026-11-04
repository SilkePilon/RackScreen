//! `/api/system`: the Pi itself for the System page. Every fact is read from
//! `/proc` and `/sys` (no shelling out) and is `null` where it cannot be
//! read, so the same code answers on the Pi and on a laptop running `--sim`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::host;
use crate::{ApiResult, AppState};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Two CPU samples closer than this reuse the last answer, so several open
/// pages do not shrink the window to nothing.
pub const MIN_SAMPLE_GAP: Duration = Duration::from_secs(1);

pub const THROTTLED_PATH: &str = "/sys/devices/platform/soc/soc:firmware/get_throttled";

// ---------------------------------------------------------------- parsing

/// Jiffies from the `cpu` line of `/proc/stat`: all of them, and the idle
/// ones (idle + iowait).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTimes {
    pub total: u64,
    pub idle: u64,
}

pub fn parse_proc_stat(text: &str) -> Option<CpuTimes> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    if v.len() < 4 {
        return None;
    }
    Some(CpuTimes {
        total: v.iter().sum(),
        idle: v[3] + v.get(4).copied().unwrap_or(0),
    })
}

/// The `cpuN` lines of `/proc/stat`.
pub fn parse_cores(text: &str) -> Option<usize> {
    let n = text
        .lines()
        .filter(|l| {
            l.strip_prefix("cpu")
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        })
        .count();
    (n > 0).then_some(n)
}

/// utime + stime (jiffies) from `/proc/self/stat`. The name in parentheses
/// may hold spaces, so fields are counted from the last `)`.
pub fn parse_self_stat(text: &str) -> Option<u64> {
    let rest = &text[text.rfind(')')? + 1..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    // rest starts at field 3 (state); utime is field 14, stime 15
    let utime: u64 = f.get(11)?.parse().ok()?;
    let stime: u64 = f.get(12)?.parse().ok()?;
    Some(utime + stime)
}

/// `(VmRSS in MB, Threads)` from `/proc/self/status`.
pub fn parse_self_status(text: &str) -> (Option<f64>, Option<u32>) {
    let mut rss = None;
    let mut threads = None;
    for line in text.lines() {
        let mut p = line.split_whitespace();
        match p.next() {
            Some("VmRSS:") => {
                rss = p
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|k| k / 1024.0)
            }
            Some("Threads:") => threads = p.next().and_then(|v| v.parse().ok()),
            _ => {}
        }
    }
    (rss, threads)
}

/// Seconds since boot from `/proc/uptime`.
pub fn parse_uptime(text: &str) -> Option<u64> {
    let s: f64 = text.split_whitespace().next()?.parse().ok()?;
    (s.is_finite() && s >= 0.0).then_some(s as u64)
}

/// `PRETTY_NAME` from `/etc/os-release`.
pub fn parse_os_release(text: &str) -> Option<String> {
    let v = text
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))?
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// The device-tree model ("Raspberry Pi 3 Model B Plus Rev 1.3"), which
/// ends in a NUL.
pub fn parse_model(bytes: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(bytes);
    let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!s.is_empty()).then(|| s.to_string())
}

/// MHz from `scaling_cur_freq` (kHz).
pub fn parse_freq_mhz(text: &str) -> Option<u32> {
    text.trim().parse::<u64>().ok().map(|k| (k / 1000) as u32)
}

/// The 1, 5 and 15 minute load from `/proc/loadavg`.
pub fn parse_load(text: &str) -> Option<[f32; 3]> {
    let mut it = text.split_whitespace().map(|v| v.parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

/// `/proc/meminfo` in MB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Memory {
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
}

pub fn parse_meminfo(text: &str) -> Option<Memory> {
    let kb = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|r| r.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
    };
    let total = kb("MemTotal:")?;
    let avail = kb("MemAvailable:")?;
    let swap_total = kb("SwapTotal:").unwrap_or(0);
    let swap_free = kb("SwapFree:").unwrap_or(swap_total);
    Some(Memory {
        total_mb: total / 1024,
        available_mb: avail / 1024,
        used_mb: total.saturating_sub(avail) / 1024,
        swap_total_mb: swap_total / 1024,
        swap_used_mb: swap_total.saturating_sub(swap_free) / 1024,
    })
}

/// The firmware's throttling word, decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Throttled {
    /// Some condition holds right now.
    pub now: bool,
    /// Some condition happened since boot.
    pub since_boot: bool,
    /// `under_voltage`, `freq_capped`, `throttled`, `soft_temp_limit` for
    /// what holds now, the same with `_since_boot` for what happened.
    pub flags: Vec<&'static str>,
    pub raw: u32,
}

const THROTTLE_BITS: [(u32, &str, &str); 4] = [
    (0, "under_voltage", "under_voltage_since_boot"),
    (1, "freq_capped", "freq_capped_since_boot"),
    (2, "throttled", "throttled_since_boot"),
    (3, "soft_temp_limit", "soft_temp_limit_since_boot"),
];

/// `get_throttled` holds the word in hex (`50005`, maybe `0x50005`).
pub fn parse_throttled(text: &str) -> Option<Throttled> {
    let t = text.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("throttled=0x"))
        .unwrap_or(t);
    let raw = u32::from_str_radix(t, 16).ok()?;
    let mut flags = Vec::new();
    for (bit, now, _) in THROTTLE_BITS {
        if raw & (1 << bit) != 0 {
            flags.push(now);
        }
    }
    for (bit, _, past) in THROTTLE_BITS {
        if raw & (1 << (bit + 16)) != 0 {
            flags.push(past);
        }
    }
    Some(Throttled {
        now: raw & 0xf != 0,
        since_boot: raw & 0xf_0000 != 0,
        flags,
        raw,
    })
}

// ---------------------------------------------------------------- sampling

#[derive(Clone, Copy, Debug)]
struct Sample {
    at: Instant,
    cpu: CpuTimes,
    proc_ticks: Option<u64>,
}

/// The previous `/proc/stat` and `/proc/self/stat` reading, so usage is
/// the share of the time between two polls.
#[derive(Debug, Default)]
pub struct CpuSampler {
    last: Option<Sample>,
    /// `(system %, process %)` of the last window.
    usage: (Option<f32>, Option<f32>),
}

impl CpuSampler {
    /// Feed a reading; returns `(system %, process %)`, both of all cores.
    /// The first reading answers the average since boot for the system and
    /// nothing for the process.
    pub fn update(
        &mut self,
        now: Instant,
        cpu: Option<CpuTimes>,
        proc_ticks: Option<u64>,
    ) -> (Option<f32>, Option<f32>) {
        let Some(cpu) = cpu else {
            return (None, None);
        };
        if let Some(last) = self.last {
            if now.duration_since(last.at) < MIN_SAMPLE_GAP {
                return self.usage;
            }
        }
        let pct = |part: u64, whole: u64| {
            (whole > 0).then(|| (part as f64 * 100.0 / whole as f64).clamp(0.0, 100.0) as f32)
        };
        self.usage = match self.last {
            Some(last) if cpu.total > last.cpu.total => {
                let total = cpu.total - last.cpu.total;
                let idle = cpu.idle.saturating_sub(last.cpu.idle);
                let proc = match (proc_ticks, last.proc_ticks) {
                    (Some(a), Some(b)) => pct(a.saturating_sub(b), total),
                    _ => None,
                };
                (pct(total.saturating_sub(idle), total), proc)
            }
            Some(_) => self.usage,
            None => (pct(cpu.total.saturating_sub(cpu.idle), cpu.total), None),
        };
        self.last = Some(Sample {
            at: now,
            cpu,
            proc_ticks,
        });
        self.usage
    }
}

fn read(p: &str) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// Take a reading now (the server does once at start, so the first page
/// load already has a window).
pub fn sample_cpu(sampler: &mut CpuSampler) -> (Option<f32>, Option<f32>) {
    sampler.update(
        Instant::now(),
        read("/proc/stat").and_then(|t| parse_proc_stat(&t)),
        read("/proc/self/stat").and_then(|t| parse_self_stat(&t)),
    )
}

// ---------------------------------------------------------------- answer

#[derive(Clone, Debug, Serialize)]
pub struct Cpu {
    pub cores: Option<usize>,
    pub usage_pct: Option<f32>,
    pub temp_c: Option<f32>,
    pub freq_mhz: Option<u32>,
    pub load: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Disk {
    pub path: &'static str,
    pub total_gb: f64,
    pub used_gb: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Process {
    pub rss_mb: Option<f64>,
    pub cpu_pct: Option<f32>,
    pub threads: Option<u32>,
    pub uptime_secs: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Iface {
    pub iface: String,
    pub ipv4: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemInfo {
    pub model: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub arch: &'static str,
    pub hostname: String,
    pub uptime_secs: Option<u64>,
    pub cpu: Cpu,
    pub memory: Option<Memory>,
    pub disk: Option<Disk>,
    pub process: Process,
    pub throttled: Option<Throttled>,
    pub network: Vec<Iface>,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

pub fn disk(path: &'static str) -> Option<Disk> {
    let s = nix::sys::statvfs::statvfs(path).ok()?;
    let frag = s.fragment_size() as f64;
    let total = s.blocks() as f64 * frag;
    let free = s.blocks_free() as f64 * frag;
    (total > 0.0).then(|| Disk {
        path,
        total_gb: round1(total / GIB),
        used_gb: round1((total - free) / GIB),
    })
}

/// RSS of this process in MB.
pub fn process_rss_mb() -> Option<f64> {
    read("/proc/self/status").and_then(|t| parse_self_status(&t).0)
}

pub fn system_info(state: &AppState) -> SystemInfo {
    let (usage, proc_pct) = sample_cpu(&mut state.cpu_sampler.lock().unwrap());
    let stat = read("/proc/stat");
    let (rss, threads) = read("/proc/self/status")
        .map(|t| parse_self_status(&t))
        .unwrap_or_default();
    SystemInfo {
        model: std::fs::read("/proc/device-tree/model")
            .ok()
            .and_then(|b| parse_model(&b)),
        os: read("/etc/os-release").and_then(|t| parse_os_release(&t)),
        kernel: read("/proc/sys/kernel/osrelease")
            .map(|t| t.trim().to_string())
            .filter(|k| !k.is_empty()),
        arch: std::env::consts::ARCH,
        hostname: host::hostname(),
        uptime_secs: read("/proc/uptime").and_then(|t| parse_uptime(&t)),
        cpu: Cpu {
            cores: stat.as_deref().and_then(parse_cores),
            usage_pct: usage.map(|v| (v * 10.0).round() / 10.0),
            temp_c: read("/sys/class/thermal/thermal_zone0/temp")
                .and_then(|t| host::parse_temp(&t))
                .map(|v| (v * 10.0).round() / 10.0),
            freq_mhz: read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
                .and_then(|t| parse_freq_mhz(&t)),
            load: read("/proc/loadavg").and_then(|t| parse_load(&t)),
        },
        memory: read("/proc/meminfo").and_then(|t| parse_meminfo(&t)),
        disk: disk("/"),
        process: Process {
            rss_mb: rss.map(round1),
            cpu_pct: proc_pct.map(|v| (v * 10.0).round() / 10.0),
            threads,
            uptime_secs: rackscreen_app::hub::unix_now().saturating_sub(state.hub.started_unix),
        },
        throttled: read(THROTTLED_PATH).and_then(|t| parse_throttled(&t)),
        network: host::ipv4_interfaces()
            .into_iter()
            .map(|(iface, ip)| Iface {
                iface,
                ipv4: ip.to_string(),
            })
            .collect(),
    }
}

pub async fn system(State(state): State<Arc<AppState>>) -> ApiResult<SystemInfo> {
    Ok(Json(system_info(&state)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  1000 50 300 8000 200 10 20 0 0 0
cpu0 250 10 80 2000 50 2 5 0 0 0
cpu1 250 10 70 2000 50 3 5 0 0 0
cpu2 250 15 70 2000 50 2 5 0 0 0
cpu3 250 15 80 2000 50 3 5 0 0 0
intr 123456 0 0
ctxt 987654
btime 1700000000
";

    #[test]
    fn proc_stat_gives_jiffies_and_cores() {
        let t = parse_proc_stat(STAT).unwrap();
        assert_eq!(t.total, 1000 + 50 + 300 + 8000 + 200 + 10 + 20);
        assert_eq!(t.idle, 8200, "idle + iowait");
        assert_eq!(parse_cores(STAT), Some(4));
        assert_eq!(parse_proc_stat("intr 1 2\n"), None);
        assert_eq!(parse_cores("cpu  1 2 3 4\n"), None);
    }

    #[test]
    fn self_stat_survives_a_name_with_spaces() {
        let s = "4242 (rack screen) S 1 4242 4242 0 -1 4194560 3000 0 2 0 1234 567 0 0 20 0 9 0 1500 300000000 9000 18446744073709551615";
        assert_eq!(parse_self_stat(s), Some(1234 + 567));
        assert_eq!(parse_self_stat("4242 (x) S 1"), None);
    }

    #[test]
    fn self_status_gives_rss_and_threads() {
        let s = "Name:\trackscreen\nVmPeak:\t  300000 kB\nVmRSS:\t   91136 kB\nThreads:\t9\n";
        assert_eq!(parse_self_status(s), (Some(89.0), Some(9)));
        assert_eq!(parse_self_status("Name: x\n"), (None, None));
    }

    #[test]
    fn small_files_parse() {
        assert_eq!(parse_uptime("356512.43 1301234.88\n"), Some(356512));
        assert_eq!(parse_uptime("junk"), None);
        let os = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nNAME=\"Debian GNU/Linux\"\nID=debian\n";
        assert_eq!(
            parse_os_release(os).as_deref(),
            Some("Debian GNU/Linux 12 (bookworm)")
        );
        assert_eq!(parse_os_release("ID=debian\n"), None);
        assert_eq!(
            parse_model(b"Raspberry Pi 3 Model B Plus Rev 1.3\0").as_deref(),
            Some("Raspberry Pi 3 Model B Plus Rev 1.3")
        );
        assert_eq!(parse_model(b"\0"), None);
        assert_eq!(parse_freq_mhz("1400000\n"), Some(1400));
        assert_eq!(parse_freq_mhz(""), None);
        assert_eq!(
            parse_load("0.42 0.30 0.25 1/300 1234\n"),
            Some([0.42, 0.30, 0.25])
        );
        assert_eq!(parse_load("0.42 0.30"), None);
    }

    #[test]
    fn meminfo_gives_ram_and_swap() {
        let mi = "MemTotal:         927892 kB
MemFree:           80000 kB
MemAvailable:     610000 kB
Buffers:           20000 kB
SwapTotal:        102396 kB
SwapFree:          81916 kB
";
        assert_eq!(
            parse_meminfo(mi),
            Some(Memory {
                total_mb: 906,
                available_mb: 595,
                used_mb: 310,
                swap_total_mb: 99,
                swap_used_mb: 20,
            })
        );
        let no_swap = "MemTotal: 2048 kB\nMemAvailable: 1024 kB\n";
        assert_eq!(parse_meminfo(no_swap).unwrap().swap_used_mb, 0);
        assert_eq!(parse_meminfo("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn throttled_bits_decode() {
        let ok = parse_throttled("0\n").unwrap();
        assert!(!ok.now && !ok.since_boot && ok.flags.is_empty());
        let t = parse_throttled("50005\n").unwrap();
        assert!(t.now && t.since_boot);
        assert_eq!(
            t.flags,
            [
                "under_voltage",
                "throttled",
                "under_voltage_since_boot",
                "throttled_since_boot"
            ]
        );
        let past = parse_throttled("0x80000").unwrap();
        assert!(!past.now && past.since_boot);
        assert_eq!(past.flags, ["soft_temp_limit_since_boot"]);
        assert_eq!(
            parse_throttled("throttled=0x2").unwrap().flags,
            ["freq_capped"]
        );
        assert_eq!(parse_throttled("zz"), None);
    }

    #[test]
    fn cpu_usage_is_the_window_between_polls() {
        let mut s = CpuSampler::default();
        let t0 = Instant::now();
        let cpu = |total, idle| Some(CpuTimes { total, idle });
        let (sys, proc) = s.update(t0, cpu(1000, 750), Some(10));
        assert_eq!((sys, proc), (Some(25.0), None), "since boot first");
        let t1 = t0 + Duration::from_secs(5);
        let (sys, proc) = s.update(t1, cpu(1400, 850), Some(30));
        assert_eq!(sys, Some(75.0), "300 of 400 busy");
        assert_eq!(proc, Some(5.0), "20 of 400");
        let soon = t1 + Duration::from_millis(200);
        assert_eq!(
            s.update(soon, cpu(1401, 851), Some(31)),
            (Some(75.0), Some(5.0)),
            "too soon: the last answer"
        );
        let t2 = t1 + Duration::from_secs(5);
        assert_eq!(
            s.update(t2, cpu(1400, 850), Some(30)),
            (Some(75.0), Some(5.0)),
            "no jiffies passed: kept"
        );
        assert_eq!(s.update(t2, None, None), (None, None));
    }

    #[test]
    fn the_live_host_answers_something() {
        let d = disk("/").unwrap();
        assert!(d.total_gb > 0.0 && d.used_gb <= d.total_gb);
        if std::path::Path::new("/proc/self/status").exists() {
            assert!(process_rss_mb().unwrap() > 0.0);
        }
    }
}

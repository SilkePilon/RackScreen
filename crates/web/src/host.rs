//! The Pi itself: hostname, LAN addresses, temperature, load, memory, and
//! whether the service can write its config.

use std::path::Path;

use serde::Serialize;

pub const FIX_HINT: &str = "Run `sudo rackscreen install` once to let the web UI save the config";

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct HostInfo {
    pub cpu_temp_c: Option<f32>,
    pub load1: Option<f32>,
    pub mem_used_mb: Option<u64>,
    pub mem_total_mb: Option<u64>,
}

pub fn parse_temp(text: &str) -> Option<f32> {
    text.trim().parse::<f32>().ok().map(|m| m / 1000.0)
}

pub fn parse_loadavg(text: &str) -> Option<f32> {
    text.split_whitespace().next()?.parse().ok()
}

/// `(used, total)` in MB from `/proc/meminfo`.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let mut total = None;
    let mut avail = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total = parts.next()?.parse::<u64>().ok(),
            Some("MemAvailable:") => avail = parts.next()?.parse::<u64>().ok(),
            _ => {}
        }
    }
    let (total, avail) = (total?, avail?);
    Some(((total.saturating_sub(avail)) / 1024, total / 1024))
}

pub fn host_info() -> HostInfo {
    let read = |p: &str| std::fs::read_to_string(p).ok();
    let mem = read("/proc/meminfo").and_then(|t| parse_meminfo(&t));
    HostInfo {
        cpu_temp_c: read("/sys/class/thermal/thermal_zone0/temp").and_then(|t| parse_temp(&t)),
        load1: read("/proc/loadavg").and_then(|t| parse_loadavg(&t)),
        mem_used_mb: mem.map(|m| m.0),
        mem_total_mb: mem.map(|m| m.1),
    }
}

pub fn hostname() -> String {
    nix::unistd::gethostname()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "rackscreen".into())
}

/// Interfaces that are not the LAN: container bridges and their veths.
pub fn is_virtual_interface(name: &str) -> bool {
    [
        "docker",
        "br-",
        "veth",
        "virbr",
        "cni",
        "flannel",
        "cali",
        "lxc",
        "tailscale",
    ]
    .iter()
    .any(|p| name.starts_with(p))
}

/// `http://<ip>:<port>` for every non-loopback IPv4 address on a real
/// interface, sorted.
pub fn addresses(port: u16) -> Vec<String> {
    let mut out: Vec<String> = match nix::ifaddrs::getifaddrs() {
        Ok(addrs) => addrs
            .filter(|a| !is_virtual_interface(&a.interface_name))
            .filter_map(|a| {
                let sa = a.address?;
                let v4 = sa.as_sockaddr_in()?;
                let ip = v4.ip();
                (!ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified())
                    .then(|| format!("http://{ip}:{port}"))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out.dedup();
    out
}

/// The config file (or, when missing, its directory) can be written and the
/// directory allows the temp-file rename `Config::save` does.
pub fn config_writable(path: &Path) -> bool {
    use nix::unistd::{access, AccessFlags};
    let Some(dir) = path.parent() else {
        return false;
    };
    let dir_ok = access(dir, AccessFlags::W_OK).is_ok();
    let file_ok = !path.exists() || access(path, AccessFlags::W_OK).is_ok();
    dir_ok && file_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_files() {
        assert_eq!(parse_temp("51234\n"), Some(51.234));
        assert_eq!(parse_temp("junk"), None);
        assert_eq!(parse_loadavg("0.42 0.30 0.25 1/300 1234\n"), Some(0.42));
        assert_eq!(parse_loadavg(""), None);
        let mi =
            "MemTotal:        927892 kB\nMemFree:          80000 kB\nMemAvailable:    610000 kB\n";
        assert_eq!(parse_meminfo(mi), Some((310, 906)));
        assert_eq!(parse_meminfo("MemTotal: 1 kB\n"), None);
        let h = host_info();
        if let Some(t) = h.mem_total_mb {
            assert!(t > 0);
        }
    }

    #[test]
    fn writability_follows_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.yaml");
        assert!(config_writable(&p), "missing file in a writable dir");
        std::fs::write(&p, "x").unwrap();
        assert!(config_writable(&p));
        assert!(!config_writable(Path::new("/proc/nope/config.yaml")));
        assert!(!hostname().is_empty());
        assert!(is_virtual_interface("docker0") && is_virtual_interface("br-1a2b"));
        assert!(!is_virtual_interface("eth0") && !is_virtual_interface("wlan0"));
        for a in addresses(8740) {
            assert!(a.starts_with("http://") && a.ends_with(":8740"), "{a}");
        }
    }
}

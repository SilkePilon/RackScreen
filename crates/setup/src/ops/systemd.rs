//! systemd unit, service control and journal reading through the Shell trait.

use anyhow::Result;

use crate::ops::shell::Shell;

pub fn unit_text() -> String {
    "[Unit]\nDescription=RackScreen Kubernetes rack monitor\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nUser=%i\nExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml\nRestart=always\nRestartSec=3\nEnvironment=RUST_LOG=info\n\n[Install]\nWantedBy=multi-user.target\n".to_string()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceInfo {
    pub active: String,
    pub sub: String,
    pub uptime_secs: Option<u64>,
}

pub struct Systemd<'a> {
    sh: &'a dyn Shell,
    unit: String,
}

impl<'a> Systemd<'a> {
    pub fn new(sh: &'a dyn Shell, user: &str) -> Systemd<'a> {
        Systemd {
            sh,
            unit: format!("rackscreen@{user}"),
        }
    }
    pub fn unit(&self) -> &str {
        &self.unit
    }
    pub fn daemon_reload(&self) -> Result<()> {
        self.sh.check("systemctl", &["daemon-reload"]).map(|_| ())
    }
    pub fn enable_now(&self) -> Result<()> {
        self.sh
            .check("systemctl", &["enable", "--now", &self.unit])
            .map(|_| ())
    }
    pub fn disable_now(&self) -> Result<()> {
        // Not an error if the unit was never installed.
        let _ = self
            .sh
            .run("systemctl", &["disable", "--now", &self.unit])?;
        Ok(())
    }
    pub fn start(&self) -> Result<()> {
        self.sh
            .check("systemctl", &["start", &self.unit])
            .map(|_| ())
    }
    pub fn stop(&self) -> Result<()> {
        self.sh
            .check("systemctl", &["stop", &self.unit])
            .map(|_| ())
    }
    pub fn restart(&self) -> Result<()> {
        self.sh
            .check("systemctl", &["restart", &self.unit])
            .map(|_| ())
    }
    pub fn is_active(&self) -> Result<bool> {
        Ok(self
            .sh
            .run("systemctl", &["is-active", "--quiet", &self.unit])?
            .success())
    }
    pub fn is_enabled(&self) -> Result<bool> {
        Ok(self
            .sh
            .run("systemctl", &["is-enabled", "--quiet", &self.unit])?
            .success())
    }
    pub fn info(&self) -> Result<ServiceInfo> {
        let out = self.sh.run(
            "systemctl",
            &[
                "show",
                &self.unit,
                "-p",
                "ActiveState,SubState,ActiveEnterTimestampMonotonic",
            ],
        )?;
        Ok(parse_show(&out.stdout, uptime_now_usecs()))
    }
    pub fn journal_tail(&self, n: usize) -> Result<Vec<String>> {
        let n = n.to_string();
        let out = self.sh.run(
            "journalctl",
            &["-u", &self.unit, "-n", &n, "--no-pager", "-o", "cat"],
        )?;
        Ok(out.stdout.lines().map(str::to_string).collect())
    }
}

fn uptime_now_usecs() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = text.split_whitespace().next()?.parse().ok()?;
    Some((secs * 1_000_000.0) as u64)
}

pub fn parse_show(stdout: &str, now_monotonic_usecs: Option<u64>) -> ServiceInfo {
    let mut info = ServiceInfo::default();
    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "ActiveState" => info.active = v.to_string(),
                "SubState" => info.sub = v.to_string(),
                "ActiveEnterTimestampMonotonic" => {
                    if let (Ok(t), Some(now)) = (v.parse::<u64>(), now_monotonic_usecs) {
                        if t > 0 && now >= t {
                            info.uptime_secs = Some((now - t) / 1_000_000);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    info
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Up,
    Down,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkDots {
    pub api: Dot,
    pub prometheus: Dot,
    pub qbittorrent: Dot,
    pub electricity: Dot,
    pub prices: Dot,
    pub weather: Dot,
    pub rain: Dot,
    pub github: Dot,
    pub argocd: Dot,
}

/// Newest matching log line decides each dot. Lines come oldest first.
pub fn links_from_logs(lines: &[String]) -> LinkDots {
    let mut d = LinkDots {
        api: Dot::Unknown,
        prometheus: Dot::Unknown,
        qbittorrent: Dot::Unknown,
        electricity: Dot::Unknown,
        prices: Dot::Unknown,
        weather: Dot::Unknown,
        rain: Dot::Unknown,
        github: Dot::Unknown,
        argocd: Dot::Unknown,
    };
    for l in lines {
        let lower = l.to_ascii_lowercase();
        let warn = lower.contains("warn") || lower.contains("error");
        if lower.contains("prometheus: forwarding") {
            d.prometheus = Dot::Up;
        } else if lower.contains("prometheus") && warn {
            d.prometheus = Dot::Down;
        }
        if lower.contains("qbittorrent: forwarding") {
            d.qbittorrent = Dot::Up;
        } else if lower.contains("qbittorrent") && warn {
            d.qbittorrent = Dot::Down;
        }
        if lower.contains("electricity:") {
            d.electricity = if warn { Dot::Down } else { Dot::Up };
        }
        if lower.contains("prices:") {
            d.prices = if warn { Dot::Down } else { Dot::Up };
        }
        for (needle, dot) in [
            ("weather:", &mut d.weather),
            ("rain:", &mut d.rain),
            ("github:", &mut d.github),
            ("argocd:", &mut d.argocd),
        ] {
            if lower.contains(needle) {
                *dot = if warn { Dot::Down } else { Dot::Up };
            }
        }
        if lower.contains("pod watch") && warn {
            d.api = Dot::Down;
        } else if lower.contains("running (") {
            d.api = Dot::Up;
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};

    #[test]
    fn unit_text_has_required_lines() {
        let u = unit_text();
        for needle in [
            "User=%i",
            "ExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml",
            "Restart=always",
            "RestartSec=3",
            "After=network-online.target",
            "Environment=RUST_LOG=info",
        ] {
            assert!(u.contains(needle), "{needle}");
        }
    }

    #[test]
    fn commands_and_states() {
        let sh = FakeShell::new();
        sh.respond("systemctl is-active", Output::fail(3, ""));
        let sd = Systemd::new(&sh, "silke");
        assert_eq!(sd.unit(), "rackscreen@silke");
        assert!(!sd.is_active().unwrap());
        sd.enable_now().unwrap();
        assert!(sh.called("systemctl enable --now rackscreen@silke"));
        sh.respond("systemctl stop", Output::fail(1, "boom"));
        assert!(sd.stop().is_err());
        assert!(sd.disable_now().is_ok());
    }

    #[test]
    fn parse_show_and_uptime() {
        let i = parse_show(
            "ActiveState=active\nSubState=running\nActiveEnterTimestampMonotonic=5000000\n",
            Some(65_000_000),
        );
        assert_eq!(
            i,
            ServiceInfo {
                active: "active".into(),
                sub: "running".into(),
                uptime_secs: Some(60)
            }
        );
        assert_eq!(parse_show("ActiveState=inactive\n", None).uptime_secs, None);
    }

    #[test]
    fn journal_and_links() {
        let sh = FakeShell::new();
        sh.respond("journalctl", Output::ok("INFO prometheus: forwarding to monitoring/p (service prometheus)\nWARN qbittorrent: login failed\nINFO electricity: poll ok (7 sources)\nWARN prices: energy-charts status: 503\nINFO running (4 screens, 30 fps, source K8s)\n"));
        let sd = Systemd::new(&sh, "pi");
        let lines = sd.journal_tail(20).unwrap();
        assert_eq!(lines.len(), 5);
        assert!(sh.called("journalctl -u rackscreen@pi -n 20"));
        let d = links_from_logs(&lines);
        assert_eq!(
            d,
            LinkDots {
                api: Dot::Up,
                prometheus: Dot::Up,
                qbittorrent: Dot::Down,
                electricity: Dot::Up,
                prices: Dot::Down,
                weather: Dot::Unknown,
                rain: Dot::Unknown,
                github: Dot::Unknown,
                argocd: Dot::Unknown,
            }
        );
        // the newest line wins for each dot
        let later = links_from_logs(&[
            "WARN electricity: token rejected".to_string(),
            "INFO prices: 24 hours for 2026-09-07".to_string(),
        ]);
        assert_eq!(later.electricity, Dot::Down);
        assert_eq!(later.prices, Dot::Up);
        assert_eq!(links_from_logs(&[]).api, Dot::Unknown);
    }

    #[test]
    fn new_link_dots_follow_their_log_prefixes() {
        let d = links_from_logs(&[
            "INFO weather: poll ok (21.2 °C, code 3)".to_string(),
            "WARN rain: request: timed out".to_string(),
            "INFO github: calendar ok (28 today, 230 this window)".to_string(),
            "INFO argocd: watching applications in argocd".to_string(),
        ]);
        assert_eq!(d.weather, Dot::Up);
        assert_eq!(d.rain, Dot::Down);
        assert_eq!(d.github, Dot::Up);
        assert_eq!(d.argocd, Dot::Up);
        let d = links_from_logs(&["WARN argocd: watch: 410 Gone".to_string()]);
        assert_eq!(d.argocd, Dot::Down);
        assert_eq!(d.weather, Dot::Unknown);
        // a cluster without the CRD is a degraded deploys role, not a healthy one
        let d = links_from_logs(&[
            "WARN argocd: no applications.argoproj.io in argocd; deploys stays on no-data, retry in 600s"
                .to_string(),
        ]);
        assert_eq!(d.argocd, Dot::Down);
        // failures that leave their link up must miss the dot's needle
        let d = links_from_logs(&["WARN air quality: request timed out".to_string()]);
        assert_eq!(d.weather, Dot::Unknown);
        let d = links_from_logs(&["WARN github runs for x/y: HTTP 404".to_string()]);
        assert_eq!(d.github, Dot::Unknown);
    }
}

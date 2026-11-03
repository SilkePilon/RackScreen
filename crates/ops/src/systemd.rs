//! systemd unit, service control and journal reading through the Shell trait.

use anyhow::Result;

use crate::shell::Shell;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{FakeShell, Output};

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
    fn journal_tail_reads_the_unit() {
        let sh = FakeShell::new();
        sh.respond("journalctl", Output::ok("INFO one\nWARN two\n"));
        let sd = Systemd::new(&sh, "pi");
        let lines = sd.journal_tail(20).unwrap();
        assert_eq!(lines, vec!["INFO one", "WARN two"]);
        assert!(sh.called("journalctl -u rackscreen@pi -n 20"));
    }
}

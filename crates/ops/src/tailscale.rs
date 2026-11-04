//! `rackscreen tailscale-setup`: the one root step the web UI's Tailscale
//! page needs. Installs Tailscale with the official installer when it is
//! missing, starts `tailscaled`, and makes the service user its operator so
//! the service can run `tailscale up/down/login/logout/set/serve` without
//! root from then on. Safe to run again.

use std::time::Duration;

use anyhow::{bail, Result};

use crate::shell::Shell;

/// The official installer, run by bash so a failed download fails the
/// pipeline instead of feeding `sh` an empty script.
pub const INSTALL_SCRIPT: &str =
    "set -euo pipefail; exec 2>&1; curl -fsSL --retry 2 --max-time 120 https://tailscale.com/install.sh | sh";

/// A user name we are willing to hand to `tailscale set --operator=`.
pub fn valid_user(user: &str) -> bool {
    let mut chars = user.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && user.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

pub struct Setup<'a> {
    pub sh: &'a dyn Shell,
    pub user: String,
    /// How long to wait between tries while `tailscaled` comes up.
    pub retry_wait: Duration,
}

impl Setup<'_> {
    /// Run every step, reporting progress through `say`.
    pub fn run(&self, say: &mut dyn FnMut(&str)) -> Result<()> {
        if !valid_user(&self.user) {
            bail!("'{}' does not look like a user name", self.user);
        }
        if self.user == "root" {
            bail!("run this through sudo from the service user, not as root itself");
        }
        match self.version() {
            Some(v) => say(&format!("Tailscale {v} is installed")),
            None => {
                say(
                    "Installing Tailscale with the official installer (this takes a minute or two)",
                );
                let mut failed_download = false;
                let ok = self
                    .sh
                    .run_streaming("bash", &["-c", INSTALL_SCRIPT], &mut |l| {
                        if l.starts_with("curl:") {
                            failed_download = true;
                        }
                        say(l);
                    })?;
                if !ok {
                    if failed_download {
                        bail!("could not download the Tailscale installer; is the Pi online?");
                    }
                    bail!("the Tailscale installer failed (see above)");
                }
                match self.version() {
                    Some(v) => say(&format!("Installed Tailscale {v}")),
                    None => bail!("the installer finished but `tailscale` is still missing"),
                }
            }
        }
        say("Starting tailscaled");
        self.sh
            .check("systemctl", &["enable", "--now", "tailscaled"])?;
        let operator = format!("--operator={}", self.user);
        let mut last = String::new();
        for attempt in 0..15 {
            if attempt > 0 {
                std::thread::sleep(self.retry_wait);
            }
            let out = self.sh.run("tailscale", &["set", &operator])?;
            if out.success() {
                say(&format!(
                    "{} may now manage Tailscale without sudo; finish in the web UI",
                    self.user
                ));
                return Ok(());
            }
            last = format!("{}{}", out.stderr.trim(), out.stdout.trim());
        }
        bail!("tailscale set {operator} failed: {last}")
    }

    /// `tailscale version`'s first line, None when it is not installed.
    fn version(&self) -> Option<String> {
        let out = self.sh.run("tailscale", &["version"]).ok()?;
        out.success()
            .then(|| out.stdout.lines().next().unwrap_or("").trim().to_string())
            .filter(|v| !v.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{FakeShell, Output};

    fn setup(sh: &FakeShell) -> Setup<'_> {
        Setup {
            sh,
            user: "silke".into(),
            retry_wait: Duration::ZERO,
        }
    }

    #[test]
    fn installed_tailscale_is_only_enabled_and_given_an_operator() {
        let sh = FakeShell::new();
        sh.respond(
            "tailscale version",
            Output::ok("1.90.0\n  tailscale commit: x\n"),
        );
        let mut log = Vec::new();
        setup(&sh).run(&mut |l| log.push(l.to_string())).unwrap();
        assert!(!sh.called("bash"), "no install when present");
        assert!(sh.called("systemctl enable --now tailscaled"));
        assert!(sh.called("tailscale set --operator=silke"));
        assert_eq!(log[0], "Tailscale 1.90.0 is installed");
    }

    #[test]
    fn a_missing_tailscale_is_installed_and_offline_fails_cleanly() {
        let sh = FakeShell::new();
        sh.respond("tailscale version", Output::fail(127, ""));
        sh.respond("bash", Output::fail(6, "curl: (6) Could not resolve host"));
        let e = setup(&sh).run(&mut |_| {}).unwrap_err().to_string();
        assert!(e.contains("is the Pi online"), "{e}");
        assert!(sh.called(&format!("bash -c {INSTALL_SCRIPT}")));
        assert!(!sh.called("systemctl"));
    }

    #[test]
    fn the_operator_is_retried_while_tailscaled_starts() {
        let sh = FakeShell::new();
        sh.respond("tailscale version", Output::ok("1.90.0"));
        sh.respond(
            "tailscale set",
            Output::fail(1, "failed to connect to local tailscaled"),
        );
        let e = setup(&sh).run(&mut |_| {}).unwrap_err().to_string();
        assert!(e.contains("failed to connect"), "{e}");
        let tries = sh
            .calls()
            .iter()
            .filter(|c| c.starts_with("tailscale set"))
            .count();
        assert_eq!(tries, 15);
    }

    #[test]
    fn odd_user_names_are_refused() {
        assert!(valid_user("silke") && valid_user("pi") && valid_user("_svc-1"));
        assert!(
            !valid_user("") && !valid_user("Root;rm") && !valid_user("-x") && !valid_user("a b")
        );
        let sh = FakeShell::new();
        let s = Setup {
            sh: &sh,
            user: "root".into(),
            retry_wait: Duration::ZERO,
        };
        assert!(s.run(&mut |_| {}).is_err());
        assert!(sh.calls().is_empty());
    }
}

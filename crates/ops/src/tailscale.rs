//! `rackscreen tailscale-setup`: the one root step the web UI's Tailscale
//! page needs. Installs Tailscale with the official installer when it is
//! missing, starts `tailscaled`, and makes the service user its operator so
//! the service can run `tailscale up/down/login/logout/set/serve` without
//! root from then on. Safe to run again.

use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::shell::Shell;

/// The official installer. It is downloaded to a file first (https only,
/// also on redirects, TLS 1.2 or newer) and run only once the download
/// finished, so a cut-off download never runs half a script.
pub const INSTALL_URL: &str = "https://tailscale.com/install.sh";

/// `curl` arguments that save the installer to `dest`.
pub fn download_args(dest: &str) -> Vec<&str> {
    vec![
        "-fsSL",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--tlsv1.2",
        "--retry",
        "2",
        "--max-time",
        "120",
        "-o",
        dest,
        INSTALL_URL,
    ]
}

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
                self.install(say)?;
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

    /// Download the official installer into a fresh directory only root
    /// can enter, run it, and remove the directory (also on errors).
    fn install(&self, say: &mut dyn FnMut(&str)) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix("rackscreen-tailscale-")
            .tempdir()
            .context("create a directory for the Tailscale installer")?;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
        let script = dir.path().join("install.sh");
        let script = script.to_str().context("temp path is not UTF-8")?;
        say(&format!("Downloading {INSTALL_URL}"));
        let out = self.sh.run("curl", &download_args(script))?;
        if !out.success() {
            bail!(
                "could not download the Tailscale installer; is the Pi online? ({})",
                out.stderr.trim()
            );
        }
        if !self.sh.run_streaming("sh", &[script], &mut |l| say(l))? {
            bail!("the Tailscale installer failed (see above)");
        }
        Ok(())
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
        assert!(
            !sh.called("curl") && !sh.called("sh "),
            "no install when present"
        );
        assert!(sh.called("systemctl enable --now tailscaled"));
        assert!(sh.called("tailscale set --operator=silke"));
        assert_eq!(log[0], "Tailscale 1.90.0 is installed");
    }

    #[test]
    fn a_missing_tailscale_is_installed_and_offline_fails_cleanly() {
        let sh = FakeShell::new();
        sh.respond("tailscale version", Output::fail(127, ""));
        sh.respond("curl", Output::fail(6, "curl: (6) Could not resolve host"));
        let e = setup(&sh).run(&mut |_| {}).unwrap_err().to_string();
        assert!(e.contains("is the Pi online"), "{e}");
        assert!(!sh.called("sh "), "nothing runs without a full download");
        assert!(!sh.called("systemctl"));
    }

    #[test]
    fn the_installer_is_downloaded_over_https_to_a_private_file_then_run() {
        let sh = FakeShell::new();
        sh.respond("tailscale version", Output::fail(127, ""));
        let _ = setup(&sh).run(&mut |_| {});
        let calls = sh.calls();
        let curl = calls.iter().find(|c| c.starts_with("curl ")).unwrap();
        assert!(
            curl.starts_with("curl -fsSL --proto =https --proto-redir =https --tlsv1.2 "),
            "{curl}"
        );
        assert!(
            curl.ends_with(" https://tailscale.com/install.sh"),
            "{curl}"
        );
        let dest = curl
            .split(" -o ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap();
        assert!(dest.ends_with("/install.sh"), "{dest}");
        assert!(calls.contains(&format!("sh {dest}")), "{calls:?}");
        let dir = std::path::Path::new(dest).parent().unwrap();
        assert!(!dir.exists(), "the download is removed afterwards");
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

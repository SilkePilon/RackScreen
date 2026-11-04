//! Where things live on the device. `root` lets tests point everything into a temp dir.

use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn system() -> Paths {
        Paths {
            root: PathBuf::from("/"),
        }
    }
    pub fn under(root: impl Into<PathBuf>) -> Paths {
        Paths { root: root.into() }
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    pub fn binary(&self) -> PathBuf {
        self.p("usr/local/bin/rackscreen")
    }
    pub fn config_dir(&self) -> PathBuf {
        self.p("etc/rackscreen")
    }
    pub fn config(&self) -> PathBuf {
        self.p("etc/rackscreen/config.yaml")
    }
    pub fn unit(&self) -> PathBuf {
        self.p("etc/systemd/system/rackscreen@.service")
    }
    /// The sudoers drop-in that lets the service user run `rackscreen update`
    /// and `rackscreen tailscale-setup`.
    pub fn sudoers(&self) -> PathBuf {
        self.p("etc/sudoers.d/rackscreen")
    }
    /// `/boot/firmware` on current Raspberry Pi OS, `/boot` on older images, None elsewhere.
    pub fn boot_dir(&self) -> Option<PathBuf> {
        [self.p("boot/firmware"), self.p("boot")]
            .into_iter()
            .find(|d| d.join("config.txt").exists())
    }
    pub fn config_txt(&self) -> Option<PathBuf> {
        self.boot_dir().map(|d| d.join("config.txt"))
    }
    pub fn cmdline_txt(&self) -> Option<PathBuf> {
        self.boot_dir().map(|d| d.join("cmdline.txt"))
    }
}

/// The user the service runs as: whoever invoked sudo, else the current user, never root.
pub fn service_user() -> String {
    let candidate = std::env::var("SUDO_USER")
        .ok()
        .filter(|u| !u.is_empty())
        .or_else(|| std::env::var("USER").ok());
    match candidate {
        Some(u) if u != "root" => u,
        _ => "pi".to_string(),
    }
}

/// The user of a `rackscreen@<user>.service` in `systemctl list-units
/// 'rackscreen@*' --plain --no-legend` output, an active one first; never root.
pub fn unit_user(list_units: &str) -> Option<String> {
    let units: Vec<(&str, bool)> = list_units
        .lines()
        .filter_map(|l| {
            let mut cols = l.split_whitespace();
            let unit = cols.next()?;
            let active = cols.nth(1) == Some("active");
            let user = unit.strip_prefix("rackscreen@")?.strip_suffix(".service")?;
            (!user.is_empty() && user != "root").then_some((user, active))
        })
        .collect();
    units
        .iter()
        .find(|(_, active)| *active)
        .or(units.first())
        .map(|(u, _)| u.to_string())
}

/// The user the installed service runs as, read from systemd.
pub fn running_service_user(sh: &dyn crate::shell::Shell) -> Option<String> {
    let out = sh
        .run(
            "systemctl",
            &["list-units", "rackscreen@*", "--plain", "--no-legend"],
        )
        .ok()?;
    out.success().then(|| unit_user(&out.stdout)).flatten()
}

pub fn is_root() -> bool {
    nix::unistd::geteuid().is_root()
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("current executable path")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_under_root() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::under(dir.path());
        assert!(p.binary().ends_with("usr/local/bin/rackscreen"));
        assert!(p.sudoers().ends_with("etc/sudoers.d/rackscreen"));
        assert_eq!(p.boot_dir(), None);
        std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
        std::fs::write(dir.path().join("boot/firmware/config.txt"), "").unwrap();
        assert!(p
            .config_txt()
            .unwrap()
            .ends_with("boot/firmware/config.txt"));
    }

    #[test]
    fn the_unit_names_its_user() {
        let out = "rackscreen@old.service   loaded failed failed  RackScreen for old\n\
                   rackscreen@silke.service loaded active running RackScreen for silke\n";
        assert_eq!(unit_user(out).as_deref(), Some("silke"));
        assert_eq!(
            unit_user("rackscreen@pi.service loaded inactive dead x\n").as_deref(),
            Some("pi")
        );
        assert_eq!(
            unit_user("rackscreen@root.service loaded active running x"),
            None
        );
        assert_eq!(
            unit_user("other@silke.service loaded active running x"),
            None
        );
        assert_eq!(unit_user(""), None);
        let sh = crate::shell::FakeShell::new();
        sh.respond(
            "systemctl list-units",
            crate::shell::Output::ok("rackscreen@silke.service loaded active running RackScreen\n"),
        );
        assert_eq!(running_service_user(&sh).as_deref(), Some("silke"));
        assert!(sh.called("systemctl list-units rackscreen@* --plain --no-legend"));
    }

    #[test]
    fn service_user_never_root() {
        let u = service_user();
        assert_ne!(u, "root");
        assert!(!u.is_empty());
    }
}

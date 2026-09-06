//! The install steps. Pure planning plus effects through Shell and Paths.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::ops::boot::{self, BootChange};
use crate::ops::config_file::{set_spi_chunk, Config};
use crate::ops::paths::Paths;
use crate::ops::shell::Shell;
use crate::ops::systemd::{unit_text, Systemd};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Platform,
    Binary,
    Config,
    Groups,
    Service,
    Boot,
}

impl StepId {
    pub const ALL: [StepId; 6] = [
        StepId::Platform,
        StepId::Binary,
        StepId::Config,
        StepId::Groups,
        StepId::Service,
        StepId::Boot,
    ];

    pub fn title(self) -> &'static str {
        match self {
            StepId::Platform => "Check platform",
            StepId::Binary => "Install binary to /usr/local/bin",
            StepId::Config => "Write config /etc/rackscreen/config.yaml",
            StepId::Groups => "Add user to spi and gpio groups",
            StepId::Service => "Install and start systemd service",
            StepId::Boot => "Enable SPI in boot files",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done(String),
    Skipped(String),
    Warn(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum Event {
    Started(StepId),
    Finished(StepId, Outcome),
    /// Boot changes need a yes/no on the `replies` channel before the step continues.
    AskBoot(Vec<BootChange>),
    Complete {
        reboot_needed: bool,
    },
}

pub struct Installer {
    pub sh: Arc<dyn Shell>,
    pub paths: Paths,
    pub user: String,
    pub self_exe: PathBuf,
}

fn sha256(path: &std::path::Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(Sha256::digest(&data).to_vec())
}

impl Installer {
    /// Run every step in order, reporting on `events`. Blocks; call from a worker thread.
    pub fn run(&self, events: Sender<Event>, replies: Receiver<bool>) {
        let mut reboot = false;
        for id in StepId::ALL {
            let _ = events.send(Event::Started(id));
            let outcome = if id == StepId::Boot {
                match self.boot_plan() {
                    Ok(changes) if changes.is_empty() => {
                        Ok(Outcome::Skipped("already enabled".into()))
                    }
                    Ok(changes) => {
                        let _ = events.send(Event::AskBoot(changes));
                        let yes = replies.recv().unwrap_or(false);
                        let r = self.step(id, Some(yes));
                        if yes && matches!(r, Ok(Outcome::Done(_))) {
                            reboot = true;
                        }
                        r
                    }
                    Err(e) => Ok(Outcome::Warn(format!("{e:#}"))),
                }
            } else {
                self.step(id, None)
            };
            let outcome = outcome.unwrap_or_else(|e| Outcome::Failed(format!("{e:#}")));
            let failed = matches!(outcome, Outcome::Failed(_));
            let _ = events.send(Event::Finished(id, outcome));
            if failed {
                break;
            }
        }
        let _ = events.send(Event::Complete {
            reboot_needed: reboot,
        });
    }

    pub fn boot_plan(&self) -> Result<Vec<BootChange>> {
        let (Some(cfg), Some(cmd)) = (self.paths.config_txt(), self.paths.cmdline_txt()) else {
            anyhow::bail!("no /boot/firmware/config.txt (not a Raspberry Pi?)");
        };
        let c = std::fs::read_to_string(&cfg).unwrap_or_default();
        let l = std::fs::read_to_string(&cmd).unwrap_or_default();
        Ok(boot::needed_changes(&c, &l))
    }

    /// One step. `answer` is only used by the Boot step.
    pub fn step(&self, id: StepId, answer: Option<bool>) -> Result<Outcome> {
        match id {
            StepId::Platform => {
                let mut warns = Vec::new();
                if std::env::consts::ARCH != "aarch64" {
                    warns.push(format!(
                        "arch is {}, expected aarch64",
                        std::env::consts::ARCH
                    ));
                }
                if self.paths.boot_dir().is_none() {
                    warns.push("no /boot/firmware (not a Raspberry Pi?)".into());
                }
                if !self.sh.run("getent", &["group", "spi"])?.success() {
                    warns.push("no 'spi' group; enable SPI and reboot first".into());
                }
                Ok(if warns.is_empty() {
                    Outcome::Done("Raspberry Pi, aarch64".into())
                } else {
                    Outcome::Warn(warns.join("; "))
                })
            }
            StepId::Binary => {
                let dst = self.paths.binary();
                if dst.exists() && sha256(&dst)? == sha256(&self.self_exe)? {
                    return Ok(Outcome::Skipped("already up to date".into()));
                }
                if let Some(dir) = dst.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let tmp = dst.with_extension("tmp");
                std::fs::copy(&self.self_exe, &tmp).context("copy binary")?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
                }
                std::fs::rename(&tmp, &dst).context("replace binary")?;
                Ok(Outcome::Done(dst.display().to_string()))
            }
            StepId::Config => {
                let path = self.paths.config();
                if path.exists() {
                    return Ok(Outcome::Skipped("kept existing config".into()));
                }
                Config::default().save(&path)?;
                Ok(Outcome::Done(path.display().to_string()))
            }
            StepId::Groups => {
                let mut missing = Vec::new();
                for g in ["spi", "gpio"] {
                    if !self.sh.run("getent", &["group", g])?.success() {
                        missing.push(g);
                    }
                }
                if !missing.is_empty() {
                    return Ok(Outcome::Warn(format!(
                        "groups missing: {}",
                        missing.join(", ")
                    )));
                }
                self.sh.check("usermod", &["-aG", "spi,gpio", &self.user])?;
                Ok(Outcome::Done(format!("{} in spi,gpio", self.user)))
            }
            StepId::Service => {
                let unit = self.paths.unit();
                if let Some(dir) = unit.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&unit, unit_text())
                    .with_context(|| format!("write {}", unit.display()))?;
                let sd = Systemd::new(self.sh.as_ref(), &self.user);
                sd.daemon_reload()?;
                sd.enable_now()?;
                Ok(Outcome::Done(format!("{} enabled and started", sd.unit())))
            }
            StepId::Boot => {
                if answer != Some(true) {
                    return Ok(Outcome::Warn("skipped; SPI must be enabled by hand".into()));
                }
                let (Some(cfg_path), Some(cmd_path)) =
                    (self.paths.config_txt(), self.paths.cmdline_txt())
                else {
                    anyhow::bail!("boot files not found");
                };
                let mut c = std::fs::read_to_string(&cfg_path).unwrap_or_default();
                for l in boot::CONFIG_LINES {
                    c = boot::ensure_line(&c, l).0;
                }
                std::fs::write(&cfg_path, c).context("write config.txt")?;
                let l = std::fs::read_to_string(&cmd_path).unwrap_or_default();
                std::fs::write(
                    &cmd_path,
                    boot::ensure_cmdline_token(&l, boot::CMDLINE_TOKEN).0,
                )
                .context("write cmdline.txt")?;
                let mut cfg = Config::load_or_default(&self.paths.config())?;
                set_spi_chunk(&mut cfg, 65536);
                cfg.save(&self.paths.config())?;
                Ok(Outcome::Done(
                    "SPI enabled, spi_chunk set to 65536; reboot required".into(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};
    use std::sync::mpsc;

    fn setup(with_boot: bool) -> (tempfile::TempDir, Installer, Arc<FakeShell>) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("rackscreen-src");
        std::fs::write(&exe, b"binary-v1").unwrap();
        if with_boot {
            std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
            std::fs::write(
                dir.path().join("boot/firmware/config.txt"),
                "dtparam=audio=on\n",
            )
            .unwrap();
            std::fs::write(
                dir.path().join("boot/firmware/cmdline.txt"),
                "root=x rootwait\n",
            )
            .unwrap();
        }
        let sh = Arc::new(FakeShell::new());
        let inst = Installer {
            sh: sh.clone(),
            paths: Paths::under(dir.path()),
            user: "silke".into(),
            self_exe: exe,
        };
        (dir, inst, sh)
    }

    #[test]
    fn fresh_install_runs_all_steps_and_applies_boot() {
        let (dir, inst, sh) = setup(true);
        let (tx, rx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        rtx.send(true).unwrap();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(matches!(
            events.last(),
            Some(Event::Complete {
                reboot_needed: true
            })
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::AskBoot(c) if c.len() == 3)));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Finished(StepId::Binary, Outcome::Done(_)))));
        assert_eq!(
            std::fs::read(dir.path().join("usr/local/bin/rackscreen")).unwrap(),
            b"binary-v1"
        );
        assert!(dir.path().join("etc/rackscreen/config.yaml").exists());
        assert!(
            std::fs::read_to_string(dir.path().join("etc/systemd/system/rackscreen@.service"))
                .unwrap()
                .contains("User=%i")
        );
        assert!(sh.called("usermod -aG spi,gpio silke"));
        assert!(sh.called("systemctl enable --now rackscreen@silke"));
        let c = std::fs::read_to_string(dir.path().join("boot/firmware/config.txt")).unwrap();
        assert!(c.contains("dtoverlay=spi1-2cs"));
        let l = std::fs::read_to_string(dir.path().join("boot/firmware/cmdline.txt")).unwrap();
        assert!(l.trim().ends_with("spidev.bufsiz=65536"));
        assert_eq!(
            Config::load_or_default(&inst.paths.config())
                .unwrap()
                .display
                .spi_chunk,
            65536
        );
    }

    #[test]
    fn second_install_skips_binary_and_config_and_declined_boot_warns() {
        let (_dir, inst, _sh) = setup(true);
        assert!(matches!(
            inst.step(StepId::Binary, None).unwrap(),
            Outcome::Done(_)
        ));
        assert!(matches!(
            inst.step(StepId::Binary, None).unwrap(),
            Outcome::Skipped(_)
        ));
        assert!(matches!(
            inst.step(StepId::Config, None).unwrap(),
            Outcome::Done(_)
        ));
        assert!(matches!(
            inst.step(StepId::Config, None).unwrap(),
            Outcome::Skipped(_)
        ));
        assert!(matches!(
            inst.step(StepId::Boot, Some(false)).unwrap(),
            Outcome::Warn(_)
        ));
    }

    #[test]
    fn desktop_without_boot_dir_warns_and_completes() {
        let (_dir, inst, sh) = setup(false);
        sh.respond("getent group spi", Output::fail(2, ""));
        let (tx, rx) = mpsc::channel();
        let (_rtx, rrx) = mpsc::channel();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(matches!(
            events
                .iter()
                .find(|e| matches!(e, Event::Finished(StepId::Platform, _))),
            Some(Event::Finished(_, Outcome::Warn(_)))
        ));
        assert!(matches!(
            events
                .iter()
                .find(|e| matches!(e, Event::Finished(StepId::Groups, _))),
            Some(Event::Finished(_, Outcome::Warn(_)))
        ));
        assert!(matches!(
            events
                .iter()
                .find(|e| matches!(e, Event::Finished(StepId::Boot, _))),
            Some(Event::Finished(_, Outcome::Warn(_)))
        ));
        assert!(matches!(
            events.last(),
            Some(Event::Complete {
                reboot_needed: false
            })
        ));
    }

    #[test]
    fn service_failure_stops_the_run() {
        let (_dir, inst, sh) = setup(true);
        sh.respond("systemctl enable", Output::fail(1, "unit masked"));
        let (tx, rx) = mpsc::channel();
        let (_rtx, rrx) = mpsc::channel();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(events.iter().any(
            |e| matches!(e, Event::Finished(StepId::Service, Outcome::Failed(m)) if m.contains("unit masked"))
        ));
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::Started(StepId::Boot))));
    }
}

//! The install steps. Pure planning plus effects through Shell and Paths.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::boot::{self, BootChange};
use crate::config_file::{save_config, set_spi_chunk, Config};
use crate::paths::Paths;
use crate::shell::Shell;
use crate::systemd::{unit_text, Systemd};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Platform,
    Binary,
    Config,
    Owner,
    Sudoers,
    Groups,
    Service,
    Boot,
}

impl StepId {
    pub const ALL: [StepId; 8] = [
        StepId::Platform,
        StepId::Binary,
        StepId::Config,
        StepId::Owner,
        StepId::Sudoers,
        StepId::Groups,
        StepId::Service,
        StepId::Boot,
    ];

    pub fn title(self) -> &'static str {
        match self {
            StepId::Platform => "Check platform",
            StepId::Binary => "Install binary to /usr/local/bin",
            StepId::Config => "Write config /etc/rackscreen/config.yaml",
            StepId::Owner => "Hand /etc/rackscreen to the service user",
            StepId::Sudoers => "Allow the service user to run rackscreen update",
            StepId::Groups => "Add user to spi and gpio groups",
            StepId::Service => "Install and start systemd service",
            StepId::Boot => "Enable SPI in boot files",
        }
    }
}

/// The sudoers drop-in: only `rackscreen update`, as root, no password.
pub fn sudoers_text(user: &str, binary: &str) -> String {
    format!("{user} ALL=(root) NOPASSWD: {binary} update\n")
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

/// `(uid, gid)` of `user`, if it exists.
pub fn user_ids(user: &str) -> Option<(u32, u32)> {
    nix::unistd::User::from_name(user)
        .ok()
        .flatten()
        .map(|u| (u.uid.as_raw(), u.gid.as_raw()))
}

/// Give `dir` and every file directly in it to `(uid, gid)`.
pub fn chown_tree(dir: &std::path::Path, (uid, gid): (u32, u32)) -> Result<()> {
    use nix::unistd::{chown, Gid, Uid};
    let own = |p: &std::path::Path| {
        chown(p, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))
            .with_context(|| format!("chown {}", p.display()))
    };
    own(dir)?;
    for e in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let e = e?;
        if e.file_type()?.is_file() {
            own(&e.path())?;
        }
    }
    Ok(())
}

/// `<file>.rackscreen.bak` next to `path`.
pub fn backup_path(path: &std::path::Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".rackscreen.bak");
    path.with_file_name(name)
}

/// Replace a boot file without ever leaving it half written: keep a backup
/// of the original (only the first time, so it stays the pristine one),
/// write a temp file in the same directory, fsync it and rename it over.
/// /boot/firmware is vfat; Linux renames over an existing file there too.
pub fn replace_boot_file(path: &std::path::Path, contents: &str) -> Result<()> {
    use std::io::Write;
    let backup = backup_path(path);
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup)
            .with_context(|| format!("back up {} to {}", path.display(), backup.display()))?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".rackscreen.tmp");
    let tmp = path.with_file_name(name);
    let write = || -> Result<()> {
        let mut f =
            std::fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("rename over {}", path.display()));
    }
    if let Some(dir) = path.parent() {
        // best effort: make the rename itself durable
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
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
                    // The cmdline token is already there (an upgrade, or a reboot after
                    // an earlier install), so the config may use the large chunk too.
                    Ok(changes) if changes.is_empty() => match self.ensure_spi_chunk() {
                        Ok(true) => Ok(Outcome::Skipped(
                            "already enabled; spi_chunk set to 65536".into(),
                        )),
                        Ok(false) => Ok(Outcome::Skipped("already enabled".into())),
                        Err(e) => Ok(Outcome::Warn(format!(
                            "already enabled; config not updated: {e:#}"
                        ))),
                    },
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

    /// Set `display.spi_chunk` to 65536 (the value `spidev.bufsiz=65536` allows) if it
    /// is not already. Returns whether the config was changed.
    fn ensure_spi_chunk(&self) -> Result<bool> {
        let path = self.paths.config();
        let mut cfg = Config::load_or_default(&path)?;
        if cfg.display.spi_chunk == 65536 {
            return Ok(false);
        }
        set_spi_chunk(&mut cfg, 65536);
        save_config(&cfg, &path)?;
        Ok(true)
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
                save_config(&Config::default(), &path)?;
                Ok(Outcome::Done(path.display().to_string()))
            }
            StepId::Owner => {
                let dir = self.paths.config_dir();
                std::fs::create_dir_all(&dir)?;
                let Some(ids) = user_ids(&self.user) else {
                    return Ok(Outcome::Warn(format!(
                        "user {} not found; the web UI cannot save the config",
                        self.user
                    )));
                };
                chown_tree(&dir, ids)?;
                Ok(Outcome::Done(format!(
                    "{} owned by {}",
                    dir.display(),
                    self.user
                )))
            }
            StepId::Sudoers => {
                let path = self.paths.sudoers();
                // the real binary path, not the temp download this runs from
                let text = sudoers_text(&self.user, "/usr/local/bin/rackscreen");
                if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
                    return Ok(Outcome::Skipped("already in place".into()));
                }
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let tmp = path.with_extension("tmp");
                std::fs::write(&tmp, &text).with_context(|| format!("write {}", tmp.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o440))?;
                }
                let tmp_s = tmp.display().to_string();
                match self.sh.run("visudo", &["-cf", &tmp_s]) {
                    Ok(o) if o.success() => {}
                    Ok(o) => {
                        let _ = std::fs::remove_file(&tmp);
                        anyhow::bail!("visudo rejected the drop-in: {}", o.stderr.trim());
                    }
                    Err(_) => {
                        let _ = std::fs::remove_file(&tmp);
                        return Ok(Outcome::Warn(
                            "visudo not found; skipped (the web UI cannot self-update)".into(),
                        ));
                    }
                }
                std::fs::rename(&tmp, &path).context("move sudoers drop-in")?;
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
                // `enable --now` leaves an already running (older) service untouched,
                // so an upgrade has to restart it to pick up the new binary.
                let was_active = sd.is_active()?;
                sd.enable_now()?;
                if was_active {
                    sd.restart()?;
                    return Ok(Outcome::Done(format!(
                        "{} enabled and restarted",
                        sd.unit()
                    )));
                }
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
                let orig = std::fs::read_to_string(&cfg_path).unwrap_or_default();
                let mut c = orig.clone();
                for l in boot::CONFIG_LINES {
                    c = boot::ensure_line(&c, l).0;
                }
                if c != orig {
                    replace_boot_file(&cfg_path, &c).context("write config.txt")?;
                }
                let l = std::fs::read_to_string(&cmd_path).unwrap_or_default();
                let (line, changed) = boot::ensure_cmdline_token(&l, boot::CMDLINE_TOKEN);
                if changed {
                    // the kernel reads only the first line
                    debug_assert_eq!(line.lines().count(), 1);
                    replace_boot_file(&cmd_path, &line).context("write cmdline.txt")?;
                }
                self.ensure_spi_chunk()?;
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
    use crate::shell::{FakeShell, Output};
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
    fn owner_step_chowns_the_config_dir_or_warns_for_an_unknown_user() {
        let (dir, mut inst, _sh) = setup(false);
        inst.step(StepId::Config, None).unwrap();
        assert!(
            matches!(
                inst.step(StepId::Owner, None).unwrap(),
                Outcome::Warn(m) if m.contains("silke")
            ) || user_ids("silke").is_some()
        );
        // the current user exists and may own its own files
        let me = nix::unistd::User::from_uid(nix::unistd::getuid())
            .unwrap()
            .unwrap()
            .name;
        inst.user = me.clone();
        assert!(matches!(
            inst.step(StepId::Owner, None).unwrap(),
            Outcome::Done(m) if m.contains(&me)
        ));
        assert!(dir.path().join("etc/rackscreen/config.yaml").exists());
    }

    #[test]
    fn sudoers_step_checks_with_visudo_and_installs_mode_0440() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, inst, sh) = setup(false);
        let out = inst.step(StepId::Sudoers, None).unwrap();
        assert!(matches!(out, Outcome::Done(_)), "{out:?}");
        let path = dir.path().join("etc/sudoers.d/rackscreen");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "silke ALL=(root) NOPASSWD: /usr/local/bin/rackscreen update\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o440
        );
        assert!(sh
            .calls()
            .iter()
            .any(|c| c.starts_with("visudo -cf ") && c.ends_with(".tmp")));
        assert!(!path.with_extension("tmp").exists());
        assert!(matches!(
            inst.step(StepId::Sudoers, None).unwrap(),
            Outcome::Skipped(_)
        ));
        // a rejected drop-in is never moved into place
        std::fs::remove_file(&path).unwrap();
        sh.respond("visudo", Output::fail(1, "syntax error"));
        let e = inst.step(StepId::Sudoers, None).unwrap_err().to_string();
        assert!(e.contains("syntax error"), "{e}");
        assert!(!path.exists() && !path.with_extension("tmp").exists());
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
        assert!(dir.path().join("etc/sudoers.d/rackscreen").exists());
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
    fn boot_step_backs_up_once_and_replaces_atomically() {
        let (dir, inst, _sh) = setup(true);
        let fw = dir.path().join("boot/firmware");
        std::fs::write(
            fw.join("cmdline.txt"),
            "root=x rootwait\nstray second line\n",
        )
        .unwrap();
        let out = inst.step(StepId::Boot, Some(true)).unwrap();
        assert!(matches!(out, Outcome::Done(_)), "{out:?}");
        assert_eq!(
            std::fs::read_to_string(fw.join("config.txt.rackscreen.bak")).unwrap(),
            "dtparam=audio=on\n"
        );
        assert_eq!(
            std::fs::read_to_string(fw.join("cmdline.txt.rackscreen.bak")).unwrap(),
            "root=x rootwait\nstray second line\n"
        );
        let l = std::fs::read_to_string(fw.join("cmdline.txt")).unwrap();
        assert_eq!(l, "root=x rootwait spidev.bufsiz=65536\n");
        assert_eq!(l.lines().count(), 1, "cmdline.txt stays one line");
        let c = std::fs::read_to_string(fw.join("config.txt")).unwrap();
        assert!(c.starts_with("dtparam=audio=on\n") && c.contains("dtoverlay=spi1-2cs"));
        // no temp files left behind
        let names: Vec<String> = std::fs::read_dir(&fw)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.ends_with(".tmp")), "{names:?}");
        // a second write keeps the first (pristine) backup
        replace_boot_file(&fw.join("config.txt"), "changed\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(fw.join("config.txt.rackscreen.bak")).unwrap(),
            "dtparam=audio=on\n"
        );
        assert_eq!(
            std::fs::read_to_string(fw.join("config.txt")).unwrap(),
            "changed\n"
        );
    }

    #[test]
    fn replace_boot_file_without_an_original_writes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cmdline.txt");
        replace_boot_file(&p, "a b\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a b\n");
        assert!(!backup_path(&p).exists());
        assert_eq!(
            backup_path(&p),
            dir.path().join("cmdline.txt.rackscreen.bak")
        );
    }

    #[test]
    fn boot_already_enabled_bumps_spi_chunk_in_config() {
        let (dir, inst, _sh) = setup(true);
        std::fs::write(
            dir.path().join("boot/firmware/config.txt"),
            "dtparam=spi=on\ndtoverlay=spi1-2cs\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("boot/firmware/cmdline.txt"),
            "root=x rootwait spidev.bufsiz=65536\n",
        )
        .unwrap();
        let (tx, rx) = mpsc::channel();
        let (_rtx, rrx) = mpsc::channel();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(!events.iter().any(|e| matches!(e, Event::AskBoot(_))));
        assert!(events.iter().any(
            |e| matches!(e, Event::Finished(StepId::Boot, Outcome::Skipped(m)) if m.contains("65536"))
        ));
        assert_eq!(
            Config::load_or_default(&inst.paths.config())
                .unwrap()
                .display
                .spi_chunk,
            65536
        );
        assert!(matches!(
            events.last(),
            Some(Event::Complete {
                reboot_needed: false
            })
        ));
    }

    #[test]
    fn service_step_restarts_an_already_running_service() {
        let (_dir, inst, sh) = setup(true);
        // FakeShell answers every command with success, so `is-active` reports running.
        let out = inst.step(StepId::Service, None).unwrap();
        assert!(matches!(out, Outcome::Done(m) if m.contains("restarted")));
        assert!(sh.called("systemctl enable --now rackscreen@silke"));
        assert!(sh.called("systemctl restart rackscreen@silke"));
    }

    #[test]
    fn service_step_does_not_restart_a_fresh_install() {
        let (_dir, inst, sh) = setup(true);
        sh.respond("systemctl is-active", Output::fail(3, ""));
        let out = inst.step(StepId::Service, None).unwrap();
        assert!(matches!(out, Outcome::Done(m) if m.contains("started")));
        assert!(!sh.called("systemctl restart"));
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

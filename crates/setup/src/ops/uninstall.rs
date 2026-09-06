//! Remove the service, config and binary. Boot file lines are left alone.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::ops::install::Outcome;
use crate::ops::paths::Paths;
use crate::ops::shell::Shell;
use crate::ops::systemd::Systemd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UStep {
    Service,
    Unit,
    Reload,
    Config,
    Binary,
}

impl UStep {
    pub const ALL: [UStep; 5] = [
        UStep::Service,
        UStep::Unit,
        UStep::Reload,
        UStep::Config,
        UStep::Binary,
    ];
    pub fn title(self) -> &'static str {
        match self {
            UStep::Service => "Stop and disable service",
            UStep::Unit => "Remove systemd unit",
            UStep::Reload => "Reload systemd",
            UStep::Config => "Remove /etc/rackscreen",
            UStep::Binary => "Remove /usr/local/bin/rackscreen",
        }
    }
}

#[derive(Clone, Debug)]
pub enum UEvent {
    Started(UStep),
    Finished(UStep, Outcome),
    Complete,
}

pub struct Uninstaller {
    pub sh: Arc<dyn Shell>,
    pub paths: Paths,
    pub user: String,
}

fn remove_if_exists(path: &std::path::Path, dir: bool) -> Result<Outcome> {
    if !path.exists() {
        return Ok(Outcome::Skipped("not present".into()));
    }
    if dir {
        std::fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    } else {
        std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(Outcome::Done(path.display().to_string()))
}

impl Uninstaller {
    pub fn run(&self, events: Sender<UEvent>) {
        for id in UStep::ALL {
            let _ = events.send(UEvent::Started(id));
            let outcome = self
                .step(id)
                .unwrap_or_else(|e| Outcome::Failed(format!("{e:#}")));
            let _ = events.send(UEvent::Finished(id, outcome));
        }
        let _ = events.send(UEvent::Complete);
    }

    pub fn step(&self, id: UStep) -> Result<Outcome> {
        let sd = Systemd::new(self.sh.as_ref(), &self.user);
        match id {
            UStep::Service => {
                sd.disable_now()?;
                Ok(Outcome::Done(sd.unit().to_string()))
            }
            UStep::Unit => remove_if_exists(&self.paths.unit(), false),
            UStep::Reload => {
                sd.daemon_reload()?;
                Ok(Outcome::Done(String::new()))
            }
            UStep::Config => remove_if_exists(&self.paths.config_dir(), true),
            UStep::Binary => remove_if_exists(&self.paths.binary(), false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::FakeShell;
    use std::sync::mpsc;

    #[test]
    fn removes_everything_it_installed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(paths.config(), "x").unwrap();
        std::fs::create_dir_all(paths.unit().parent().unwrap()).unwrap();
        std::fs::write(paths.unit(), "x").unwrap();
        std::fs::create_dir_all(paths.binary().parent().unwrap()).unwrap();
        std::fs::write(paths.binary(), "x").unwrap();
        let sh = Arc::new(FakeShell::new());
        let u = Uninstaller {
            sh: sh.clone(),
            paths: paths.clone(),
            user: "silke".into(),
        };
        let (tx, rx) = mpsc::channel();
        u.run(tx);
        let events: Vec<UEvent> = rx.iter().collect();
        assert!(matches!(events.last(), Some(UEvent::Complete)));
        assert!(!paths.config_dir().exists());
        assert!(!paths.unit().exists());
        assert!(!paths.binary().exists());
        assert!(sh.called("systemctl disable --now rackscreen@silke"));
        assert!(sh.called("systemctl daemon-reload"));
        // second run: everything skipped, nothing fails
        let (tx, rx) = mpsc::channel();
        u.run(tx);
        assert!(rx
            .iter()
            .all(|e| !matches!(e, UEvent::Finished(_, Outcome::Failed(_)))));
    }
}

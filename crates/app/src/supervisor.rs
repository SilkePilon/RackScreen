//! The supervisor `rackscreen run` runs on its main thread: start the
//! monitor, wait for a signal or a request from the web UI, and restart the
//! monitor in-process on request (the web server lives on across that).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use crate::config::Config;
use crate::hub::{Hub, SupervisorCmd};
use crate::run::{Monitor, RunOptions};

/// Why the wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// SIGINT or SIGTERM.
    Signal,
    /// The render loop stopped on its own (window closed, renderer error).
    MonitorStopped,
    Cmd(SupervisorCmd),
}

/// What the supervisor loop decided to do when it returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Normal end: a signal, or in `--sim` the window was closed.
    Clean,
    /// The `web:` block changed: exit 0 so systemd starts us again.
    RestartProcess,
    /// The monitor died on the Pi: exit non-zero so systemd restarts us.
    MonitorDied,
}

/// Wait for a signal, a supervisor command or the stop flag of the running
/// monitor (`None` when it failed to start: then only signals and commands).
async fn wait(stop: Option<&AtomicBool>, rx: &Receiver<SupervisorCmd>) -> Wake {
    let poll = async {
        loop {
            if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
                return Wake::MonitorStopped;
            }
            match rx.try_recv() {
                Ok(cmd) => return Wake::Cmd(cmd),
                Err(TryRecvError::Disconnected) => {
                    // the hub went away: nothing else can wake us but a signal
                    std::future::pending::<()>().await;
                }
                Err(TryRecvError::Empty) => {}
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("SIGINT, stopping");
            Wake::Signal
        }
        _ = crate::run::sigterm() => {
            tracing::info!("SIGTERM, stopping");
            Wake::Signal
        }
        w = poll => w,
    }
}

/// Run until a signal. `load` reads the config for each (re)start; the
/// first start uses `first` so a config that was already loaded (and
/// printed its warnings) is not parsed twice.
pub fn supervise(
    hub: Arc<Hub>,
    first: Config,
    opts: RunOptions,
    load: impl Fn() -> Result<Config>,
) -> Result<Exit> {
    let rx = hub.supervisor_mailbox();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut cfg = Some(first);
    loop {
        let config = match cfg.take() {
            Some(c) => Ok(c),
            None => load(),
        };
        let monitor =
            config.and_then(|c| Monitor::start_with_hub(&c, opts.clone(), Some(hub.clone())));
        let (monitor, stop) = match monitor {
            Ok(m) => {
                hub.set_monitor("running", None);
                let stop = m.stop_flag();
                (Some(m), Some(stop))
            }
            Err(e) => {
                let msg = format!("{e:#}");
                tracing::error!("monitor: {msg}");
                hub.set_monitor("error", Some(msg));
                (None, None)
            }
        };
        let wake = runtime.block_on(wait(stop.as_deref(), &rx));
        // The monitor is dropped (and its threads joined) before anything else.
        drop(monitor);
        match wake {
            Wake::Signal => {
                hub.set_monitor("stopped", None);
                return Ok(Exit::Clean);
            }
            Wake::MonitorStopped => {
                if opts.sim {
                    tracing::info!("simulator window closed, exiting");
                    hub.set_monitor("stopped", None);
                    return Ok(Exit::Clean);
                }
                tracing::error!("the render loop stopped; exiting so the service restarts");
                hub.set_monitor("error", Some("the render loop stopped".into()));
                return Ok(Exit::MonitorDied);
            }
            Wake::Cmd(SupervisorCmd::RestartMonitor) => {
                tracing::info!("restarting the monitor");
                hub.set_monitor("starting", None);
            }
            Wake::Cmd(SupervisorCmd::RestartProcess) => {
                if opts.sim {
                    tracing::warn!(
                        "the web: block changed; restart `rackscreen run` by hand to apply it"
                    );
                    hub.set_monitor("starting", None);
                    continue;
                }
                tracing::info!("the web: block changed; exiting so the service restarts");
                hub.set_monitor("stopped", None);
                return Ok(Exit::RestartProcess);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[tokio::test]
    async fn wait_wakes_on_stop_flag_and_commands() {
        let (tx, rx) = mpsc::channel();
        let stop = AtomicBool::new(false);
        tx.send(SupervisorCmd::RestartMonitor).unwrap();
        assert_eq!(
            wait(Some(&stop), &rx).await,
            Wake::Cmd(SupervisorCmd::RestartMonitor)
        );
        stop.store(true, Ordering::Relaxed);
        assert_eq!(wait(Some(&stop), &rx).await, Wake::MonitorStopped);
        // without a monitor the stop flag is not consulted
        tx.send(SupervisorCmd::RestartProcess).unwrap();
        assert_eq!(
            wait(None, &rx).await,
            Wake::Cmd(SupervisorCmd::RestartProcess)
        );
    }
}

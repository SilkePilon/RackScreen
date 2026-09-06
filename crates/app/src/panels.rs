//! Opening the four displays (simulator or GC9A01) and their display threads.
// With neither backend feature both branches bail, so everything below is unused.
#![cfg_attr(
    not(any(feature = "sim", feature = "pi")),
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unreachable_code,
        dead_code
    )
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use rackscreen_core::theme::Role;
#[cfg(feature = "pi")]
use rackscreen_display::Display;
use rackscreen_display::{spawn_display_thread, DisplayCmd, Mailbox};
use rackscreen_render::frame::Orient;

use crate::config::Config;

pub struct PanelHandle {
    pub role: Role,
    pub index: usize,
    pub orient: Orient,
    pub mailbox: Mailbox,
}

pub struct Panels {
    pub handles: Vec<PanelHandle>,
    pub threads: Vec<JoinHandle<()>>,
    pub hub_thread: Option<JoinHandle<()>>,
    pub stop: Arc<AtomicBool>,
}

impl Panels {
    /// Quit every display thread, stop the simulator window if any, join everything.
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in &self.handles {
            h.mailbox.put(DisplayCmd::Quit);
        }
        for t in self.threads {
            let _ = t.join();
        }
        if let Some(t) = self.hub_thread {
            let _ = t.join();
        }
    }
}

/// A `Panels` under construction: dropped without `finish` (any `?` in `open_panels`)
/// it shuts down whatever was already started, so a failure part way through never
/// leaves display threads blocked in `Mailbox::take` holding their GPIO lines, or the
/// simulator window thread running.
struct Building(Option<Panels>);

impl Building {
    fn get(&mut self) -> &mut Panels {
        self.0.as_mut().expect("panels present until finished")
    }
    fn finish(mut self) -> Panels {
        self.0.take().expect("panels present until finished")
    }
}

impl Drop for Building {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            p.shutdown();
        }
    }
}

/// Open one display per configured screen. In simulator mode the window loop runs on
/// its own thread and `key_tx` (if given) receives the simulator's key presses.
/// Orientation: identity in the simulator, per-config rotate/hflip on real panels.
/// On error everything opened so far is shut down again before returning.
pub fn open_panels(
    cfg: &Config,
    sim: bool,
    sim_grid: bool,
    key_tx: Option<Sender<char>>,
) -> Result<Panels> {
    let stop = Arc::new(AtomicBool::new(false));
    let mut building = Building(Some(Panels {
        handles: Vec::new(),
        threads: Vec::new(),
        hub_thread: None,
        stop: stop.clone(),
    }));

    if sim {
        #[cfg(feature = "sim")]
        {
            let (tx, rx) = std::sync::mpsc::channel::<char>();
            // The minifb window is not `Send`, so it is built on the window thread and
            // only the panels (plain shared buffers) come back here.
            let (ready_tx, ready_rx) =
                std::sync::mpsc::sync_channel::<Result<Vec<rackscreen_display::sim::SimPanel>>>(1);
            let n = cfg.screens.len();
            let stop2 = stop.clone();
            let thread = std::thread::Builder::new()
                .name("sim-window".into())
                .spawn(
                    move || match rackscreen_display::sim::SimHub::new(n, sim_grid, tx) {
                        Ok((hub, panels)) => {
                            if ready_tx.send(Ok(panels)).is_err() {
                                return;
                            }
                            hub.run(stop2.clone());
                            // Window closed or Escape pressed: stop the rest of the app too.
                            stop2.store(true, Ordering::Relaxed);
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                        }
                    },
                )?;
            // Registered before anything can fail so an error below stops and joins it.
            building.get().hub_thread = Some(thread);
            let sim_panels = ready_rx
                .recv()
                .context("simulator window thread stopped before opening the window")??;
            for (i, (scr, panel)) in cfg.screens.iter().zip(sim_panels).enumerate() {
                let role = scr.role()?;
                let mb = Mailbox::new();
                let p = building.get();
                p.handles.push(PanelHandle {
                    role,
                    index: i,
                    orient: Orient::identity(),
                    mailbox: mb.clone(),
                });
                p.threads
                    .push(spawn_display_thread(scr.role.clone(), Box::new(panel), mb));
            }
            if let Some(out) = key_tx {
                std::thread::spawn(move || {
                    for ch in rx {
                        if out.send(ch).is_err() {
                            break;
                        }
                    }
                });
            } else {
                drop(rx);
            }
        }
        #[cfg(not(feature = "sim"))]
        {
            let _ = (sim_grid, key_tx);
            anyhow::bail!("built without the `sim` feature");
        }
    } else {
        #[cfg(feature = "pi")]
        {
            let _ = (sim_grid, key_tx);
            for (i, scr) in cfg.screens.iter().enumerate() {
                let role = scr.role()?;
                let pins = rackscreen_display::gc9a01::Pins {
                    bus: scr.spi,
                    cs: scr.cs,
                    dc: scr.dc,
                    rst: scr.rst,
                    hz: scr.hz,
                };
                let dev = rackscreen_display::gc9a01::Gc9a01::open(
                    pins,
                    cfg.display.spi_chunk,
                    cfg.display.brightness,
                )
                .with_context(|| format!("open display {}", scr.role))?;
                let d: Box<dyn Display> = Box::new(dev);
                let mb = Mailbox::new();
                let p = building.get();
                p.handles.push(PanelHandle {
                    role,
                    index: i,
                    orient: Orient::new(scr.rotate, scr.hflip),
                    mailbox: mb.clone(),
                });
                p.threads
                    .push(spawn_display_thread(scr.role.clone(), d, mb));
                tracing::info!("{} display online", scr.role);
            }
        }
        #[cfg(not(feature = "pi"))]
        {
            let _ = (sim_grid, key_tx);
            anyhow::bail!("built without the `pi` feature; use --sim");
        }
    }
    Ok(building.finish())
}

#[cfg(all(test, feature = "sim"))]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A bad role on the second screen fails `open_panels` after the simulator hub (and
    /// the first display thread) are up; the call must still return promptly, which it
    /// only does if the window thread was stopped and joined. Without a display server
    /// the hub itself fails to open, which exercises the same early-error path.
    #[test]
    fn sim_failure_after_hub_up_stops_window_thread() {
        let mut cfg = Config::default();
        cfg.screens[1].role = "nope".into();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let failed = match open_panels(&cfg, true, false, None) {
                Ok(p) => {
                    p.shutdown();
                    false
                }
                Err(_) => true,
            };
            let _ = tx.send(failed);
        });
        let failed = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("open_panels returned within 2 s");
        assert!(failed, "unknown role must fail open_panels");
    }
}

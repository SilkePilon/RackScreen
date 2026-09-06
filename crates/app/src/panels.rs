//! Opening the four displays (simulator or GC9A01) and their display threads.
// With neither backend feature both branches bail, so everything below is unused.
#![cfg_attr(
    not(any(feature = "sim", feature = "pi")),
    allow(unused_imports, unused_variables, unused_mut, unreachable_code)
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

/// Open one display per configured screen. In simulator mode the window loop runs on
/// its own thread and `key_tx` (if given) receives the simulator's key presses.
/// Orientation: identity in the simulator, per-config rotate/hflip on real panels.
pub fn open_panels(
    cfg: &Config,
    sim: bool,
    sim_grid: bool,
    key_tx: Option<Sender<char>>,
) -> Result<Panels> {
    let stop = Arc::new(AtomicBool::new(false));
    #[cfg_attr(not(any(feature = "sim", feature = "pi")), allow(unused_mut))]
    let mut handles = Vec::new();
    #[cfg_attr(not(any(feature = "sim", feature = "pi")), allow(unused_mut))]
    let mut threads = Vec::new();
    // Without `pi` the non-sim branch bails, so the initial None is never read;
    // without `sim` the value is never reassigned.
    #[cfg_attr(not(feature = "sim"), allow(unused_mut))]
    #[cfg_attr(not(feature = "pi"), allow(unused_assignments))]
    let mut hub_thread = None;

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
            let sim_panels = ready_rx
                .recv()
                .context("simulator window thread stopped before opening the window")??;
            for (i, (scr, panel)) in cfg.screens.iter().zip(sim_panels).enumerate() {
                let mb = Mailbox::new();
                handles.push(PanelHandle {
                    role: scr.role()?,
                    index: i,
                    orient: Orient::identity(),
                    mailbox: mb.clone(),
                });
                threads.push(spawn_display_thread(scr.role.clone(), Box::new(panel), mb));
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
            hub_thread = Some(thread);
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
                handles.push(PanelHandle {
                    role: scr.role()?,
                    index: i,
                    orient: Orient::new(scr.rotate, scr.hflip),
                    mailbox: mb.clone(),
                });
                threads.push(spawn_display_thread(scr.role.clone(), d, mb));
                tracing::info!("{} display online", scr.role);
            }
        }
        #[cfg(not(feature = "pi"))]
        {
            let _ = (sim_grid, key_tx);
            anyhow::bail!("built without the `pi` feature; use --sim");
        }
    }
    Ok(Panels {
        handles,
        threads,
        hub_thread,
        stop,
    })
}

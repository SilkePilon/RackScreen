//! Start and stop the monitor: sources on tokio, displays on threads, the render loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::model::Thresholds;
use rackscreen_core::night::parse_hhmm;
use rackscreen_core::theme::Role;
use rackscreen_sources::SourceCtx;
use tokio_util::sync::CancellationToken;

use crate::config::{expand_home, Config};
use crate::panels::{open_panels, Panels};
use crate::runloop::{NightWindow, RenderLoop, ScreenSlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Fake,
    K8s,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub sim: bool,
    pub sim_grid: bool,
    pub source: Option<SourceKind>,
    pub fps: Option<u32>,
    pub seed: u64,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            sim: false,
            sim_grid: false,
            source: None,
            fps: None,
            seed: 1,
        }
    }
}

pub struct Monitor {
    stop: Arc<AtomicBool>,
    shutdown: CancellationToken,
    runtime: tokio::runtime::Runtime,
    render_thread: Option<JoinHandle<()>>,
    panels: Option<Panels>,
}

impl Monitor {
    pub fn start(cfg: &Config, opts: RunOptions) -> Result<Monitor> {
        let source = opts.source.unwrap_or(if opts.sim {
            SourceKind::Fake
        } else {
            SourceKind::K8s
        });
        let fps = opts.fps.unwrap_or(cfg.display.fps);
        let (tx, rx) = mpsc::channel();
        let shutdown = CancellationToken::new();
        let ctx = SourceCtx {
            tx,
            shutdown: shutdown.clone(),
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        // sources
        let key_tx = match source {
            SourceKind::Fake => {
                let (cmd_tx, cmd_rx) = mpsc::channel();
                runtime.spawn(rackscreen_sources::fake::run_fake(ctx, cmd_rx, opts.seed));
                let (key_tx, key_rx) = mpsc::channel::<char>();
                std::thread::spawn(move || {
                    for ch in key_rx {
                        if let Some(cmd) = rackscreen_sources::fake::FakeCmd::from_key(ch) {
                            if cmd_tx.send(cmd).is_err() {
                                break;
                            }
                        }
                    }
                });
                Some(key_tx)
            }
            SourceKind::K8s => {
                spawn_k8s_sources(&runtime, cfg, ctx);
                None
            }
        };

        // displays
        let panels = open_panels(cfg, opts.sim, opts.sim_grid, key_tx)?;
        let stop = panels.stop.clone();
        let slots: Vec<ScreenSlot> = panels
            .handles
            .iter()
            .enumerate()
            .map(|(i, h)| ScreenSlot::new(i, h.first_role(), h.orient.clone(), h.mailbox.clone()))
            .collect();
        // Each screen cycles through its configured role list.
        let screens_roles: Vec<Vec<Role>> =
            panels.handles.iter().map(|h| h.roles.clone()).collect();
        let cycle_secs: Vec<f64> = panels.handles.iter().map(|h| h.cycle_secs as f64).collect();

        // render loop
        let night = NightWindow {
            enabled: cfg.night.enabled,
            start_min: parse_hhmm(&cfg.night.start).context("night.start")?,
            end_min: parse_hhmm(&cfg.night.end).context("night.end")?,
        };
        let render = RenderLoop {
            rx,
            screens: slots,
            screens_roles,
            cycle_secs,
            thresholds: Thresholds {
                hot_cpu: cfg.thresholds.hot_cpu,
                hot_mem: cfg.thresholds.hot_mem,
                hot_temp: cfg.thresholds.hot_temp,
            },
            night,
            fps,
            stop: stop.clone(),
        };
        let render_thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                if let Err(e) = render.run() {
                    tracing::error!("render loop: {e:#}");
                }
            })?;
        tracing::info!(
            "running ({} screens, {} fps, source {:?})",
            cfg.screens.len(),
            fps,
            source
        );
        Ok(Monitor {
            stop,
            shutdown,
            runtime,
            render_thread: Some(render_thread),
            panels: Some(panels),
        })
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Block until SIGINT, SIGTERM or the stop flag, then shut down.
    pub fn run_blocking(self) -> Result<()> {
        let stop = self.stop.clone();
        self.runtime
            .block_on(async move { wait_for_shutdown(&stop).await });
        self.shutdown();
        Ok(())
    }

    /// Stop everything and wait for the threads. Equivalent to dropping the monitor.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.shutdown.cancel();
        if let Some(t) = self.render_thread.take() {
            let _ = t.join();
        }
        if let Some(p) = self.panels.take() {
            p.shutdown();
        }
    }
}

fn spawn_k8s_sources(runtime: &tokio::runtime::Runtime, cfg: &Config, ctx: SourceCtx) {
    let kubeconfig = expand_home(&cfg.k8s.kubeconfig);
    let cfg2 = cfg.clone();
    runtime.spawn(async move {
        let client = match rackscreen_sources::k8s::make_client(&kubeconfig).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("kubernetes client: {e:#}");
                return;
            }
        };
        tokio::spawn(rackscreen_sources::k8s::run_pod_watch(
            client.clone(),
            ctx.clone(),
        ));
        tokio::spawn(rackscreen_sources::k8s::run_node_watch(
            client.clone(),
            ctx.clone(),
        ));
        let prom = rackscreen_sources::prometheus::PromConfig {
            namespace: cfg2.prometheus.namespace.clone(),
            service: cfg2.prometheus.service.clone(),
            port: cfg2.prometheus.port,
            poll_secs: cfg2.prometheus.poll_secs,
            ignore_alerts: cfg2.prometheus.ignore_alerts.clone(),
        };
        tokio::spawn(rackscreen_sources::prometheus::run_prometheus(
            client.clone(),
            prom,
            ctx.clone(),
        ));
        if cfg2.qbittorrent.enabled {
            let q = rackscreen_sources::qbittorrent::QbitConfig {
                namespace: cfg2.qbittorrent.namespace.clone(),
                service: cfg2.qbittorrent.service.clone(),
                port: cfg2.qbittorrent.port,
                user: cfg2.qbittorrent.user.clone(),
                pass: cfg2.qbittorrent.pass.clone(),
                poll_secs: cfg2.qbittorrent.poll_secs,
            };
            tokio::spawn(rackscreen_sources::qbittorrent::run_qbittorrent(
                client, q, ctx,
            ));
        }
    });
}

/// Resolves on SIGINT, SIGTERM (systemd stop/restart) or when the render loop
/// has already stopped on its own (renderer error, panic, or window closed).
async fn wait_for_shutdown(stop: &AtomicBool) {
    let render_stopped = async {
        while !stop.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT, stopping"),
        _ = sigterm() => tracing::info!("SIGTERM, stopping"),
        _ = render_stopped => tracing::info!("stop flag set, shutting down"),
    }
}

async fn sigterm() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
                return;
            }
            Err(e) => tracing::warn!("cannot install SIGTERM handler: {e}"),
        }
    }
    std::future::pending::<()>().await
}

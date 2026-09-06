//! RackScreen: animated Kubernetes monitor for four round displays.

mod config;
mod runloop;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use rackscreen_core::model::Thresholds;
use rackscreen_core::night::parse_hhmm;
use rackscreen_display::{spawn_display_thread, Display, Mailbox};
use rackscreen_render::frame::Orient;
use rackscreen_sources::SourceCtx;
use tokio_util::sync::CancellationToken;

use config::{expand_home, Config};
use runloop::{NightWindow, RenderLoop, ScreenSlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum SourceKind {
    Fake,
    K8s,
}

#[derive(Parser, Debug)]
#[command(name = "rackscreen", version, about)]
struct Cli {
    /// Config file (default: ~/.config/rackscreen/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,
    /// Desktop simulator window instead of SPI displays
    #[arg(long)]
    sim: bool,
    /// Simulator: 2x2 grid instead of a column
    #[arg(long)]
    sim_grid: bool,
    /// Data source (default: fake with --sim, k8s otherwise)
    #[arg(long, value_enum)]
    source: Option<SourceKind>,
    /// Render frames per second
    #[arg(long)]
    fps: Option<u32>,
    /// Seed for the fake source
    #[arg(long, default_value_t = 1)]
    seed: u64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    let cfg = Config::load(cli.config.as_deref())?;
    let source = cli.source.unwrap_or(if cli.sim {
        SourceKind::Fake
    } else {
        SourceKind::K8s
    });
    let fps = cli.fps.unwrap_or(cfg.display.fps);

    let (tx, rx) = mpsc::channel();
    let shutdown = CancellationToken::new();
    let stop = Arc::new(AtomicBool::new(false));
    let ctx = SourceCtx {
        tx,
        shutdown: shutdown.clone(),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    // ---- sources ----
    let fake_cmds: Option<mpsc::Sender<rackscreen_sources::fake::FakeCmd>> = match source {
        SourceKind::Fake => {
            let (cmd_tx, cmd_rx) = mpsc::channel();
            runtime.spawn(rackscreen_sources::fake::run_fake(
                ctx.clone(),
                cmd_rx,
                cli.seed,
            ));
            Some(cmd_tx)
        }
        SourceKind::K8s => {
            let kubeconfig = expand_home(&cfg.k8s.kubeconfig);
            let cfg2 = cfg.clone();
            let ctx2 = ctx.clone();
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
                    ctx2.clone(),
                ));
                tokio::spawn(rackscreen_sources::k8s::run_node_watch(
                    client.clone(),
                    ctx2.clone(),
                ));
                let prom = rackscreen_sources::prometheus::PromConfig {
                    namespace: cfg2.prometheus.namespace.clone(),
                    service: cfg2.prometheus.service.clone(),
                    port: cfg2.prometheus.port,
                    poll_secs: cfg2.prometheus.poll_secs,
                };
                tokio::spawn(rackscreen_sources::prometheus::run_prometheus(
                    client.clone(),
                    prom,
                    ctx2.clone(),
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
                        client, q, ctx2,
                    ));
                }
            });
            None
        }
    };

    // ---- displays ----
    let mut slots = Vec::new();
    let mut display_threads = Vec::new();
    #[cfg(feature = "sim")]
    let mut sim_hub = None;

    if cli.sim {
        #[cfg(feature = "sim")]
        {
            let (key_tx, key_rx) = mpsc::channel::<char>();
            let (hub, panels) =
                rackscreen_display::sim::SimHub::new(cfg.screens.len(), cli.sim_grid, key_tx)?;
            for (scr, panel) in cfg.screens.iter().zip(panels) {
                let mb = Mailbox::new();
                slots.push(ScreenSlot::new(scr.role()?, Orient::identity(), mb.clone()));
                display_threads.push(spawn_display_thread(scr.role.clone(), Box::new(panel), mb));
            }
            std::thread::spawn(move || {
                for ch in key_rx {
                    match (&fake_cmds, rackscreen_sources::fake::FakeCmd::from_key(ch)) {
                        (Some(tx), Some(cmd)) => {
                            let _ = tx.send(cmd);
                        }
                        (None, Some(_)) => tracing::info!("keys only work with --source fake"),
                        _ => {}
                    }
                }
            });
            sim_hub = Some(hub);
        }
        #[cfg(not(feature = "sim"))]
        anyhow::bail!("built without the `sim` feature");
    } else {
        #[cfg(feature = "pi")]
        {
            let _ = &fake_cmds;
            for scr in &cfg.screens {
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
                slots.push(ScreenSlot::new(
                    scr.role()?,
                    Orient::new(scr.rotate, scr.hflip),
                    mb.clone(),
                ));
                display_threads.push(spawn_display_thread(scr.role.clone(), d, mb));
                tracing::info!("{} display online", scr.role);
            }
        }
        #[cfg(not(feature = "pi"))]
        anyhow::bail!("built without the `pi` feature; use --sim");
    }

    // ---- render loop ----
    let night = NightWindow {
        enabled: cfg.night.enabled,
        start_min: parse_hhmm(&cfg.night.start).context("night.start")?,
        end_min: parse_hhmm(&cfg.night.end).context("night.end")?,
    };
    let render = RenderLoop {
        rx,
        screens: slots,
        thresholds: Thresholds {
            hot_cpu: cfg.thresholds.hot_cpu,
            hot_mem: cfg.thresholds.hot_mem,
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

    // ---- block main thread ----
    #[cfg(feature = "sim")]
    if let Some(hub) = sim_hub {
        hub.run(stop.clone());
    }
    if !stop.load(Ordering::Relaxed) {
        runtime.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
        });
        tracing::info!("stopping");
        stop.store(true, Ordering::Relaxed);
    }

    shutdown.cancel();
    let _ = render_thread.join();
    for t in display_threads {
        let _ = t.join();
    }
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    Ok(())
}

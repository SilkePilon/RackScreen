//! RackScreen: animated Kubernetes monitor for four round displays.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use rackscreen_app::config::Config;
use rackscreen_app::hub::Hub;
use rackscreen_app::logs::LogSink;
use rackscreen_app::run::{RunOptions, SourceKind};
use rackscreen_app::supervisor::{supervise, Exit};
use rackscreen_ops::install::{self, Installer};
use rackscreen_ops::paths::{service_user, Paths};
use rackscreen_ops::shell::RealShell;
use rackscreen_ops::uninstall::{UEvent, Uninstaller};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Lines the web UI's log page keeps.
const LOG_LINES: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum SourceArg {
    Fake,
    K8s,
}

#[derive(Parser, Debug)]
#[command(name = "rackscreen", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run the monitor and its web UI (what the systemd service runs)
    Run(RunArgs),
    /// Install the binary, config, service and sudoers drop-in (idempotent)
    Install {
        /// Answer yes to the boot file changes (SPI) without asking
        #[arg(long)]
        yes: bool,
        /// Never touch the boot files
        #[arg(long)]
        no_boot: bool,
    },
    /// Remove the service, sudoers drop-in and binary
    Uninstall {
        /// Do not ask for confirmation
        #[arg(long)]
        yes: bool,
        /// Remove /etc/rackscreen (the config) too
        #[arg(long)]
        purge: bool,
    },
    /// Update to the latest GitHub release
    Update {
        /// Only report whether an update exists (0 up to date, 1 available, 2 unknown)
        #[arg(long)]
        check: bool,
    },
    /// Install Tailscale if missing, start it and let the service user manage it
    /// (what the web UI's Tailscale page runs through sudo)
    TailscaleSetup,
    /// Play a show now on every screen (not at night)
    Play {
        /// Show id (freight, ...) or `random`
        show: String,
    },
}

#[derive(clap::Args, Debug)]
struct RunArgs {
    /// Config file (default: /etc/rackscreen/config.yaml)
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
    source: Option<SourceArg>,
    /// Render frames per second
    #[arg(long)]
    fps: Option<u32>,
    /// Seed for the fake source
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Do not serve the web UI
    #[arg(long)]
    no_web: bool,
    /// Serve the web UI on this port instead of `web.port`
    #[arg(long)]
    web_port: Option<u16>,
}

/// Stdout (for journald) and the in-memory sink the web UI reads.
fn init_logging(sink: &LogSink) {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(sink.layer())
        .init();
}

/// Install, uninstall and update change system files; re-run ourselves under
/// sudo when not root.
fn ensure_root() -> Result<()> {
    if nix::unistd::geteuid().is_root() {
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    eprintln!("rackscreen needs root; re-running with sudo");
    let status = std::process::Command::new("sudo")
        .arg(exe)
        .args(&args)
        .status()?;
    std::process::exit(status.code().unwrap_or(1));
}

/// `y/N` on a terminal; anything else (a pipe) is no.
fn ask_yes_no(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim(), "y" | "Y" | "yes")
}

fn web_urls(port: u16) -> Vec<String> {
    let mut urls = rackscreen_web::host::addresses(port);
    if urls.is_empty() {
        urls.push(format!("http://localhost:{port}"));
    }
    urls
}

fn run(args: RunArgs) -> Result<()> {
    let sink = LogSink::new(LOG_LINES);
    init_logging(&sink);
    let path = args.config.clone().unwrap_or_else(Config::default_path);
    let cfg = Config::load(Some(&path))?;
    let opts = RunOptions {
        sim: args.sim,
        sim_grid: args.sim_grid,
        source: args.source.map(|s| match s {
            SourceArg::Fake => SourceKind::Fake,
            SourceArg::K8s => SourceKind::K8s,
        }),
        fps: args.fps,
        seed: args.seed,
    };
    let hub = Arc::new(Hub::new(
        path.clone(),
        args.sim,
        VERSION,
        sink,
        rackscreen_app::hub::MAX_SCREENS,
    ));
    let web = if args.no_web || !cfg.web.enabled {
        tracing::info!("web UI off");
        None
    } else {
        let port = args.web_port.unwrap_or(cfg.web.port);
        match rackscreen_web::serve(hub.clone(), &cfg.web.bind, port, cfg.web.password.clone()) {
            Ok(w) => Some(w),
            Err(e) => {
                tracing::error!("web UI not started: {e:#}");
                None
            }
        }
    };
    let exit = supervise(hub, cfg, opts, move || {
        Config::load_or_default(&path).with_context(|| format!("reload {}", path.display()))
    })?;
    // The web thread is detached: the process exits and takes it along.
    drop(web);
    match exit {
        Exit::Clean => Ok(()),
        Exit::RestartProcess => std::process::exit(0),
        Exit::MonitorDied => std::process::exit(1),
    }
}

fn install_cmd(yes: bool, no_boot: bool) -> Result<()> {
    ensure_root()?;
    let user = service_user();
    let installer = Installer {
        sh: Arc::new(RealShell),
        paths: Paths::system(),
        user: user.clone(),
        self_exe: std::env::current_exe().context("current executable path")?,
    };
    println!("Installing RackScreen v{VERSION} for user {user}");
    let (tx, rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("install".into())
        .spawn(move || installer.run(tx, reply_rx))?;
    let mut reboot = false;
    let mut failed = false;
    for ev in rx {
        match ev {
            install::Event::Started(id) => println!(".. {}", id.title()),
            install::Event::Finished(_, install::Outcome::Done(n)) => println!("ok   {n}"),
            install::Event::Finished(_, install::Outcome::Skipped(n)) => println!("skip {n}"),
            install::Event::Finished(_, install::Outcome::Warn(n)) => println!("warn {n}"),
            install::Event::Finished(_, install::Outcome::Failed(n)) => {
                println!("FAIL {n}");
                failed = true;
            }
            install::Event::AskBoot(changes) => {
                println!("   SPI needs these boot file changes:");
                for c in &changes {
                    println!("     {}", c.describe());
                }
                let answer = if no_boot {
                    println!("   skipped (--no-boot)");
                    false
                } else if yes {
                    true
                } else if std::io::stdin().is_terminal() {
                    ask_yes_no("   Apply them?")
                } else {
                    println!("   skipped: not a terminal; run with --yes to apply them");
                    false
                };
                let _ = reply_tx.send(answer);
            }
            install::Event::Complete { reboot_needed } => reboot = reboot_needed,
        }
    }
    let _ = worker.join();
    if failed {
        anyhow::bail!("install failed");
    }
    let cfg = Config::load_or_default(&Paths::system().config()).unwrap_or_default();
    println!();
    for u in web_urls(cfg.web.port) {
        println!("Open {u}");
    }
    if reboot {
        println!("reboot needed: the boot files changed; the panels light up after a reboot");
    }
    Ok(())
}

fn uninstall_cmd(yes: bool, purge: bool) -> Result<()> {
    ensure_root()?;
    if !yes {
        let what = if purge {
            "Remove the RackScreen service, binary and config?"
        } else {
            "Remove the RackScreen service and binary (the config stays)?"
        };
        if !ask_yes_no(what) {
            println!("nothing removed (run with --yes to skip this question)");
            return Ok(());
        }
    }
    let u = Uninstaller {
        sh: Arc::new(RealShell),
        paths: Paths::system(),
        user: service_user(),
        purge,
    };
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("uninstall".into())
        .spawn(move || u.run(tx))?;
    let mut failed = false;
    for ev in rx {
        match ev {
            UEvent::Started(id) => println!(".. {}", id.title()),
            UEvent::Finished(_, install::Outcome::Done(n)) => println!("ok   {n}"),
            UEvent::Finished(_, install::Outcome::Skipped(n)) => println!("skip {n}"),
            UEvent::Finished(_, install::Outcome::Warn(n)) => println!("warn {n}"),
            UEvent::Finished(_, install::Outcome::Failed(n)) => {
                println!("FAIL {n}");
                failed = true;
            }
            UEvent::Complete => {}
        }
    }
    let _ = worker.join();
    if failed {
        anyhow::bail!("uninstall failed");
    }
    println!("removed; the SPI lines in the boot files were left alone");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Run(args)) => run(args),
        Some(Cmd::Install { yes, no_boot }) => install_cmd(yes, no_boot),
        Some(Cmd::Uninstall { yes, purge }) => uninstall_cmd(yes, purge),
        Some(Cmd::Update { check }) => {
            // A read-only check needs no privileges; installing the new binary does.
            let code = if check {
                rackscreen_ops::update::check_and_print(VERSION)
            } else {
                ensure_root()?;
                rackscreen_ops::update::run_cli(VERSION)
            };
            std::process::exit(code);
        }
        Some(Cmd::TailscaleSetup) => {
            ensure_root()?;
            let setup = rackscreen_ops::tailscale::Setup {
                sh: &RealShell,
                user: service_user(),
                retry_wait: std::time::Duration::from_secs(1),
            };
            setup.run(&mut |l| {
                println!("{l}");
                let _ = std::io::stdout().flush();
            })
        }
        Some(Cmd::Play { show }) => {
            let (_, warning) = rackscreen_app::play::write_for_service_user(&show)?;
            if let Some(w) = warning {
                eprintln!("warning: {w}");
            }
            println!("requested {}", show.trim());
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            let path = Config::default_path();
            if path.exists() {
                let port = Config::load_or_default(&path)
                    .map(|c| c.web.port)
                    .unwrap_or(8740);
                println!();
                println!("The web UI of the running service:");
                for u in web_urls(port) {
                    println!("  {u}");
                }
            }
            std::process::exit(2);
        }
    }
}

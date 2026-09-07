//! RackScreen: animated Kubernetes monitor for four round displays.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use rackscreen_app::config::Config;
use rackscreen_app::run::{Monitor, RunOptions, SourceKind};

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
    /// Run the monitor (what the systemd service runs)
    Run(RunArgs),
    /// Fix screen rotation and mirroring interactively
    Calibrate {
        /// Desktop simulator window instead of SPI displays
        #[arg(long)]
        sim: bool,
        /// Config file (default: /etc/rackscreen/config.yaml)
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Update to the latest GitHub release
    Update {
        /// Only report whether an update exists (0 up to date, 1 available, 2 unknown)
        #[arg(long)]
        check: bool,
    },
    /// Interactive setup (default when run in a terminal)
    Setup {
        /// Desktop simulator window instead of SPI displays
        #[arg(long)]
        sim: bool,
        /// Config file (default: /etc/rackscreen/config.yaml)
        #[arg(long)]
        config: Option<PathBuf>,
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
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
}

/// Setup and calibrate change system files; re-run ourselves under sudo when not root.
/// `--sim` runs (desktop testing) stay unprivileged.
fn ensure_root(sim: bool) -> Result<()> {
    if sim || nix::unistd::geteuid().is_root() {
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

fn setup(start: rackscreen_setup::Start, sim: bool, config: Option<PathBuf>) -> Result<()> {
    ensure_root(sim)?;
    let ctx = rackscreen_setup::Ctx {
        config_path: config.unwrap_or_else(Config::default_path),
        sim,
        version: env!("CARGO_PKG_VERSION"),
    };
    rackscreen_setup::run(start, ctx)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Run(args)) => {
            init_logging();
            let cfg = Config::load(args.config.as_deref())?;
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
            Monitor::start(&cfg, opts)?.run_blocking()
        }
        Some(Cmd::Update { check }) => {
            let version = env!("CARGO_PKG_VERSION");
            // A read-only check needs no privileges; installing the new binary does.
            let code = if check {
                rackscreen_setup::ops::update::check_and_print(version)
            } else {
                ensure_root(false)?;
                rackscreen_setup::ops::update::run_cli(version)
            };
            std::process::exit(code);
        }
        Some(Cmd::Setup { sim, config }) => setup(rackscreen_setup::Start::Menu, sim, config),
        Some(Cmd::Calibrate { sim, config }) => {
            setup(rackscreen_setup::Start::Calibrate, sim, config)
        }
        None => {
            if std::io::stdin().is_terminal() {
                setup(rackscreen_setup::Start::Menu, false, None)
            } else {
                Cli::command().print_help()?;
                std::process::exit(2);
            }
        }
    }
}

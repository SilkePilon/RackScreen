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

fn setup(start: rackscreen_setup::Start, sim: bool, config: Option<PathBuf>) -> Result<()> {
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

# Setup TUI, YAML config and calibration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give RackScreen a self-contained setup experience: `rackscreen` opens a modern animated terminal UI with Install, Calibrate screens, Configure, Status, Run here and Uninstall; config moves to YAML at `/etc/rackscreen/config.yaml`; a one-line `install.sh` replaces cloning the repo.

**Architecture:** A new library crate `rackscreen-app` absorbs config, the render loop and the monitor wiring so both the binary and the new `rackscreen-setup` crate (ratatui + crossterm) can use them. The binary becomes a thin clap dispatcher: `run`, `setup`, `calibrate`. Setup screens are small state machines behind a `Screen` trait; system side effects go through a `Shell` trait (real vs fake) and a root-prefixed `Paths` struct so everything is unit-testable without a Pi.

**Tech Stack:** Rust 2021, ratatui 0.30 (crossterm 0.29 via `ratatui::crossterm`), serde_yaml_ng 0.10, nix 0.31 (`user` feature), sha2 0.11, tempfile (dev), existing crates (core, render, display, sources).

Spec: `docs/superpowers/specs/2026-09-06-setup-tui-design.md`

## Global Constraints

- Config path default `/etc/rackscreen/config.yaml`; `--config` overrides. YAML only; TOML dependency, `config.example.toml` and its tests are removed. Field names and defaults unchanged from v0.1.0.
- CLI: `rackscreen` (no args, TTY) opens the TUI, non-TTY prints help and exits 2; `rackscreen setup`; `rackscreen calibrate [--sim]`; `rackscreen run [--sim] [--sim-grid] [--source fake|k8s] [--config PATH] [--fps N] [--seed N]`.
- Root re-exec: setup/calibrate/install/uninstall need root; if `geteuid() != 0` the process re-executes as `sudo <current_exe> <same args>` before touching the terminal. Service user = `SUDO_USER`, else current user, else `pi` (never root).
- Install steps in order: Platform check, Binary, Config, Groups, Service, Boot files (confirm dialog), then "Calibrate now?" prompt. Uninstall: confirm, disable service, remove unit, daemon-reload, remove `/etc/rackscreen`, remove binary. Boot lines are never removed.
- Unit file: `ExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml`, `User=%i`, `Restart=always`, `RestartSec=3`, `After=network-online.target`, `Environment=RUST_LOG=info`.
- Boot files: `dtparam=spi=on`, `dtoverlay=spi1-2cs` in `config.txt`; token `spidev.bufsiz=65536` in `cmdline.txt`; on apply set `display.spi_chunk: 65536`.
- Visuals: palette from `rackscreen_core::theme` (amber primary), header ring glyph cycles `◐ ◓ ◑ ◒` on 2.4 s, menu highlight slides 150 ms ease-out, screens slide in 200 ms, braille spinner, step glyphs `○ ✓ ✗ !`, ASCII fallback when locale is not UTF-8. 60 Hz while animating, 10 Hz idle.
- Calibrate keys: `1-4` select, `r` rotate +90, `R` rotate -90, `f` flip, `a` apply to all, `s` save, `Esc` discard. Test pattern: white `arrow-up` 120 px centred, screen number in the badge, amber dot top-right, ring fully lit in the role accent.
- Deviation from spec (structural, approved reasoning): config/runloop/run wiring live in a new `crates/app` library instead of the binary so the setup crate can call them without a cycle.
- All existing tests keep passing; `cargo clippy --workspace --all-targets --features sim,pi -- -D warnings` plus the `pi`-only and `sim`-only clippy builds stay clean; `cargo fmt --all` before each commit.

## File map

```
Cargo.toml                          members += crates/app, crates/setup; root deps
config.example.yaml                 embedded default (replaces config.example.toml)
install.sh                          release one-liner (replaces deploy/)
src/main.rs                         clap dispatch only
crates/app/Cargo.toml
crates/app/src/lib.rs               pub mod config, runloop, run, panels, calibrate, logs
crates/app/src/config.rs            moved from src/config.rs, YAML, Serialize, save()
crates/app/src/runloop.rs           moved verbatim from src/runloop.rs
crates/app/src/panels.rs            open displays (sim or gc9a01) -> Panels
crates/app/src/run.rs               Monitor::start/stop/run_blocking (from old main.rs)
crates/app/src/calibrate.rs         test pattern scene
crates/app/src/logs.rs              LogSink MakeWriter for in-TUI logs
crates/setup/Cargo.toml
crates/setup/src/lib.rs             Ctx, Start, run(), App loop, Screen trait, navigation
crates/setup/src/theme.rs           Theme, Glyphs
crates/setup/src/anim.rs            Slide, spinner, ring glyph
crates/setup/src/widgets.rs         header, footer, step list, confirm dialog
crates/setup/src/screens/{mod,menu,install,uninstall,calibrate,configure,status,run}.rs
crates/setup/src/ops/{mod,shell,paths,boot,systemd,config_file,install,uninstall}.rs
.github/workflows/release.yml       also uploads install.sh
README.md                           new setup section
assets/icons/arrow-up.svg           new icon
```

---

### Task 1: `rackscreen-app` crate with YAML config

**Files:**
- Create: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/config.rs`, `config.example.yaml`
- Delete: `src/config.rs`, `config.example.toml`
- Modify: `Cargo.toml` (workspace members, root deps), `src/main.rs` (import path only)

**Interfaces:**
- Produces: `rackscreen_app::config::{Config, K8sCfg, PromCfg, QbitCfg, NightCfg, ThresholdsCfg, DisplayCfg, ScreenCfg, expand_home}`; `Config::default_path() -> PathBuf` (`/etc/rackscreen/config.yaml`), `Config::load(Option<&Path>) -> Result<Config>`, `Config::load_or_default(&Path) -> Result<Config>` (missing file gives defaults without warning), `Config::save(&self, &Path) -> Result<()>`, `Config::validate(&self) -> Result<()>`, `Config::to_yaml(&self) -> Result<String>`, `Config::from_yaml(&str) -> Result<Config>`. All config structs derive `Serialize`.

- [ ] **Step 1: Workspace and crate manifest**

Root `Cargo.toml`: in `[workspace] members` add `"crates/app"`. In root `[dependencies]` remove `toml = "1"`, add `rackscreen-app = { path = "crates/app" }`.

`crates/app/Cargo.toml`:
```toml
[package]
name = "rackscreen-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rackscreen-core = { path = "../core" }
rackscreen-render = { path = "../render" }
rackscreen-display = { path = "../display", default-features = false }
rackscreen-sources = { path = "../sources" }
anyhow.workspace = true
serde.workspace = true
serde_yaml_ng = "0.10"
tokio.workspace = true
tokio-util.workspace = true
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tiny-skia.workspace = true
chrono = "0.4"
dirs = "7"

[dev-dependencies]
tempfile = "3"

[features]
default = []
sim = ["rackscreen-display/sim"]
pi = ["rackscreen-display/pi"]
```

Root features become:
```toml
[features]
default = ["sim", "pi"]
sim = ["rackscreen-display/sim", "rackscreen-app/sim"]
pi = ["rackscreen-display/pi", "rackscreen-app/pi"]
```

- [ ] **Step 2: Write config.example.yaml**

```yaml
# RackScreen configuration. Installed to /etc/rackscreen/config.yaml.

k8s:
  kubeconfig: ~/k8s-monitor.yaml

prometheus:
  namespace: monitoring
  service: auto            # "auto" picks the first service containing "prometheus" that exposes `port`
  port: 9090
  poll_secs: 5
  ignore_alerts: [Watchdog, InfoInhibitor]

qbittorrent:
  enabled: true
  namespace: arr-stack
  service: qbittorrent
  port: 8080
  user: ""                 # empty = rely on "bypass auth for localhost"
  pass: ""
  poll_secs: 3

night:
  enabled: true
  start: "23:00"
  end: "07:00"

thresholds:
  hot_cpu: 90
  hot_mem: 90

display:
  brightness: 1.0
  fps: 30
  spi_chunk: 4096          # 65536 once spidev.bufsiz=65536 is in cmdline.txt

screens:
  - { role: cpu,    spi: 0, cs: 0, dc: 6,  rst: 5,  rotate: 270, hflip: false, hz: 40000000 }
  - { role: mem,    spi: 0, cs: 1, dc: 13, rst: 26, rotate: 270, hflip: true,  hz: 40000000 }
  - { role: pods,   spi: 1, cs: 0, dc: 23, rst: 22, rotate: 270, hflip: true,  hz: 16000000 }
  - { role: health, spi: 1, cs: 1, dc: 4,  rst: 27, rotate: 270, hflip: true,  hz: 16000000 }
```

Delete `config.example.toml`.

- [ ] **Step 3: Write crates/app/src/config.rs**

Move `src/config.rs` to `crates/app/src/config.rs` (`git mv`), then apply these changes:

```rust
//! YAML configuration with defaults matching config.example.yaml.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rackscreen_core::theme::Role;
use serde::{Deserialize, Serialize};
```

- Every `#[derive(Debug, Clone, Deserialize)]` becomes `#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]`.
- Remove the `#[cfg_attr(not(feature = "pi"), allow(dead_code))]` line above `ScreenCfg` (pub fields in a library crate do not warn).
- Replace `impl Default for Config`:
```rust
impl Default for Config {
    fn default() -> Self {
        Config::from_yaml(include_str!("../../../config.example.yaml"))
            .expect("config.example.yaml is valid")
    }
}
```
- Replace `default_path`, `load`, and add the new methods:
```rust
pub const DEFAULT_PATH: &str = "/etc/rackscreen/config.yaml";

impl Config {
    pub fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_PATH)
    }

    pub fn from_yaml(text: &str) -> Result<Config> {
        let cfg: Config = serde_yaml_ng::from_str(text).context("parse yaml")?;
        Ok(cfg)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml_ng::to_string(self).context("serialise yaml")
    }

    /// Load from `path` (or the default path). Missing file: warn and use defaults.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let path = path.map(Path::to_path_buf).unwrap_or_else(Config::default_path);
        if !path.exists() {
            tracing::warn!("config {} not found, using built-in defaults", path.display());
            return Ok(Config::default());
        }
        let cfg = Config::load_or_default(&path)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load from `path`; a missing file silently yields defaults (used by setup tools).
    pub fn load_or_default(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Config::from_yaml(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let text = format!("# RackScreen configuration (written by rackscreen setup)\n{}", self.to_yaml()?);
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
    }
}
```
Keep `validate()` exactly as it is (roles, rotate values, at least one screen) and keep the existing `expand_home`.

Replace the tests module with:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses_with_four_screens() {
        let c = Config::default();
        assert_eq!(c.screens.len(), 4);
        assert_eq!(c.screens[3].role().unwrap(), Role::Health);
        assert_eq!(c.screens[2].hz, 16_000_000);
        assert_eq!(c.prometheus.port, 9090);
        assert_eq!(c.prometheus.ignore_alerts, vec!["Watchdog", "InfoInhibitor"]);
        assert!(c.qbittorrent.enabled);
        assert_eq!(c.display.spi_chunk, 4096);
        c.validate().unwrap();
    }

    #[test]
    fn partial_yaml_fills_defaults() {
        let c = Config::from_yaml("night:\n  enabled: false\nscreens:\n  - { role: cpu, spi: 0, cs: 0, dc: 6, rst: 5 }\n").unwrap();
        assert!(!c.night.enabled);
        assert_eq!(c.night.start, "23:00");
        assert_eq!(c.screens[0].hz, 40_000_000);
        assert_eq!(c.screens[0].rotate, 0);
        assert_eq!(c.display.fps, 30);
    }

    #[test]
    fn yaml_round_trip_preserves_everything() {
        let mut c = Config::default();
        c.screens[1].rotate = 90;
        c.screens[1].hflip = false;
        c.qbittorrent.pass = "s3cret".into();
        let back = Config::from_yaml(&c.to_yaml().unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn save_and_load_or_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.yaml");
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
        let mut c = Config::default();
        c.display.spi_chunk = 65536;
        c.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# RackScreen configuration"));
        assert_eq!(Config::load_or_default(&path).unwrap().display.spi_chunk, 65536);
    }

    #[test]
    fn bad_role_and_bad_rotate_rejected() {
        let s = ScreenCfg { role: "nope".into(), spi: 0, cs: 0, dc: 0, rst: 0, rotate: 0, hflip: false, hz: 1 };
        assert!(s.role().is_err());
        let mut c = Config::default();
        c.screens[0].rotate = 45;
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("rotate") && err.contains("45"));
    }

    #[test]
    fn expand_home_works() {
        assert!(expand_home("/abs").starts_with("/abs"));
        assert!(!expand_home("~/x").to_string_lossy().starts_with('~'));
    }
}
```

`crates/app/src/lib.rs`:
```rust
//! Application wiring shared by the `rackscreen` binary and the setup TUI.
pub mod config;
```

- [ ] **Step 4: Point the binary at the new crate**

In `src/main.rs`: delete `mod config;`, replace `use config::{expand_home, Config};` with `use rackscreen_app::config::{expand_home, Config};`, and change the `--config` doc comment to `/// Config file (default: /etc/rackscreen/config.yaml)`.

- [ ] **Step 5: Test and commit**

Run: `cargo test --workspace --features sim,pi` and `cargo clippy --workspace --all-targets --features sim,pi -- -D warnings`
Expected: all pass (the 6 config tests now live in `rackscreen-app`), clippy clean. Run `cargo run -- --sim` for 10 s to confirm the binary still starts (it will warn that `/etc/rackscreen/config.yaml` is missing and use defaults).

```bash
cargo fmt --all
git add -A
git commit -m "feat(app): rackscreen-app crate with YAML config"
```

---

### Task 2: Move runloop, extract monitor wiring, clap subcommands

**Files:**
- Create: `crates/app/src/panels.rs`, `crates/app/src/run.rs`
- Move: `src/runloop.rs` -> `crates/app/src/runloop.rs` (verbatim, `git mv`)
- Modify: `crates/app/src/lib.rs`, `src/main.rs` (rewrite), root `Cargo.toml` (root no longer needs tokio-util/chrono/tiny-skia; keep clap, anyhow, tokio, tracing, tracing-subscriber, dirs)

**Interfaces:**
- Produces:
  - `rackscreen_app::panels::{PanelHandle { role: Role, index: usize, orient: Orient, mailbox: Mailbox }, Panels { handles: Vec<PanelHandle>, threads: Vec<JoinHandle<()>>, hub_thread: Option<JoinHandle<()>>, stop: Arc<AtomicBool> }, open_panels(cfg: &Config, sim: bool, sim_grid: bool, key_tx: Option<Sender<char>>) -> Result<Panels>`; `Panels::shutdown(self)` sends Quit to every mailbox, sets stop, joins threads.
  - `rackscreen_app::run::{SourceKind { Fake, K8s }, RunOptions { sim, sim_grid, source: Option<SourceKind>, fps: Option<u32>, seed: u64 }, Monitor}`; `Monitor::start(cfg: &Config, opts: RunOptions) -> Result<Monitor>`, `Monitor::stop(&self)`, `Monitor::is_stopped(&self) -> bool`, `Monitor::run_blocking(self) -> Result<()>` (waits for SIGINT/SIGTERM/stop then shuts down), `Monitor::shutdown(self)`.
  - `rackscreen_app::runloop::*` unchanged.

- [ ] **Step 1: Write panels.rs**

```rust
//! Opening the four displays (simulator or GC9A01) and their display threads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use rackscreen_core::theme::Role;
use rackscreen_display::{spawn_display_thread, Display, DisplayCmd, Mailbox};
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
pub fn open_panels(cfg: &Config, sim: bool, sim_grid: bool, key_tx: Option<Sender<char>>) -> Result<Panels> {
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();
    let mut threads = Vec::new();
    let mut hub_thread = None;

    if sim {
        #[cfg(feature = "sim")]
        {
            let (tx, rx) = std::sync::mpsc::channel::<char>();
            let (hub, panels) = rackscreen_display::sim::SimHub::new(cfg.screens.len(), sim_grid, tx)?;
            for (i, (scr, panel)) in cfg.screens.iter().zip(panels).enumerate() {
                let mb = Mailbox::new();
                handles.push(PanelHandle { role: scr.role()?, index: i, orient: Orient::identity(), mailbox: mb.clone() });
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
            let stop2 = stop.clone();
            hub_thread = Some(std::thread::Builder::new().name("sim-window".into()).spawn(move || hub.run(stop2))?);
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
                let pins = rackscreen_display::gc9a01::Pins { bus: scr.spi, cs: scr.cs, dc: scr.dc, rst: scr.rst, hz: scr.hz };
                let dev = rackscreen_display::gc9a01::Gc9a01::open(pins, cfg.display.spi_chunk, cfg.display.brightness)
                    .with_context(|| format!("open display {}", scr.role))?;
                let d: Box<dyn Display> = Box::new(dev);
                let mb = Mailbox::new();
                handles.push(PanelHandle { role: scr.role()?, index: i, orient: Orient::new(scr.rotate, scr.hflip), mailbox: mb.clone() });
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
    Ok(Panels { handles, threads, hub_thread, stop })
}
```

- [ ] **Step 2: Write run.rs (monitor wiring extracted from main.rs)**

```rust
//! Start and stop the monitor: sources on tokio, displays on threads, the render loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::model::Thresholds;
use rackscreen_core::night::parse_hhmm;
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
        Self { sim: false, sim_grid: false, source: None, fps: None, seed: 1 }
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
        let source = opts.source.unwrap_or(if opts.sim { SourceKind::Fake } else { SourceKind::K8s });
        let fps = opts.fps.unwrap_or(cfg.display.fps);
        let (tx, rx) = mpsc::channel();
        let shutdown = CancellationToken::new();
        let ctx = SourceCtx { tx, shutdown: shutdown.clone() };
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;

        // sources
        let key_tx = match source {
            SourceKind::Fake => {
                let (cmd_tx, cmd_rx) = mpsc::channel();
                runtime.spawn(rackscreen_sources::fake::run_fake(ctx.clone(), cmd_rx, opts.seed));
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
                spawn_k8s_sources(&runtime, cfg, ctx.clone());
                None
            }
        };

        // displays
        let panels = open_panels(cfg, opts.sim, opts.sim_grid, key_tx)?;
        let stop = panels.stop.clone();
        let slots: Vec<ScreenSlot> = panels
            .handles
            .iter()
            .map(|h| ScreenSlot::new(h.role, h.orient.clone(), h.mailbox.clone()))
            .collect();

        // render loop
        let night = NightWindow {
            enabled: cfg.night.enabled,
            start_min: parse_hhmm(&cfg.night.start).context("night.start")?,
            end_min: parse_hhmm(&cfg.night.end).context("night.end")?,
        };
        let render = RenderLoop {
            rx,
            screens: slots,
            thresholds: Thresholds { hot_cpu: cfg.thresholds.hot_cpu, hot_mem: cfg.thresholds.hot_mem },
            night,
            fps,
            stop: stop.clone(),
        };
        let render_thread = std::thread::Builder::new().name("render".into()).spawn(move || {
            if let Err(e) = render.run() {
                tracing::error!("render loop: {e:#}");
            }
        })?;
        tracing::info!("running ({} screens, {} fps, source {:?})", cfg.screens.len(), fps, source);
        Ok(Monitor { stop, shutdown, runtime, render_thread: Some(render_thread), panels: Some(panels) })
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
        self.runtime.block_on(async move { wait_for_shutdown(&stop).await });
        self.shutdown();
        Ok(())
    }

    pub fn shutdown(mut self) {
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
        tokio::spawn(rackscreen_sources::k8s::run_pod_watch(client.clone(), ctx.clone()));
        tokio::spawn(rackscreen_sources::k8s::run_node_watch(client.clone(), ctx.clone()));
        let prom = rackscreen_sources::prometheus::PromConfig {
            namespace: cfg2.prometheus.namespace.clone(),
            service: cfg2.prometheus.service.clone(),
            port: cfg2.prometheus.port,
            poll_secs: cfg2.prometheus.poll_secs,
            ignore_alerts: cfg2.prometheus.ignore_alerts.clone(),
        };
        tokio::spawn(rackscreen_sources::prometheus::run_prometheus(client.clone(), prom, ctx.clone()));
        if cfg2.qbittorrent.enabled {
            let q = rackscreen_sources::qbittorrent::QbitConfig {
                namespace: cfg2.qbittorrent.namespace.clone(),
                service: cfg2.qbittorrent.service.clone(),
                port: cfg2.qbittorrent.port,
                user: cfg2.qbittorrent.user.clone(),
                pass: cfg2.qbittorrent.pass.clone(),
                poll_secs: cfg2.qbittorrent.poll_secs,
            };
            tokio::spawn(rackscreen_sources::qbittorrent::run_qbittorrent(client, q, ctx));
        }
    });
}

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
```

Note: `Orient` needs `Clone`; add `#[derive(Clone)]` to `pub struct Orient` in `crates/render/src/frame.rs`. `ScreenSlot::new` takes `Orient` by value; the clone above covers it. Also make the `Drop` impl and `shutdown` share code: keep `shutdown(self)` as `drop(self)` if simpler (`pub fn shutdown(self) { drop(self) }`), with all logic in `Drop`.

- [ ] **Step 3: Rewrite src/main.rs as a dispatcher**

```rust
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
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
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
        None => {
            if std::io::stdin().is_terminal() {
                eprintln!("setup TUI arrives in a later task; use `rackscreen run --sim` for now");
                Ok(())
            } else {
                Cli::command().print_help()?;
                std::process::exit(2);
            }
        }
    }
}
```

(The `None` branch is replaced in Task 3 by the TUI call. `IsTerminal` is std, stable.)

- [ ] **Step 4: Test, run, commit**

Run: `cargo test --workspace --features sim,pi`, clippy (three feature sets), `RUST_LOG=info timeout 15 cargo run -- run --sim` (exit 124 expected, `running (4 screens ...)` in the log, window opens from a background thread). Also `timeout -s TERM 10 cargo run -- run --sim` must exit 0 with a "SIGTERM, stopping" line.

```bash
cargo fmt --all
git add -A
git commit -m "refactor: move runloop and monitor wiring into rackscreen-app; clap subcommands"
```

---

### Task 3: Setup crate scaffold: theme, animation, widgets, App loop, Menu screen

**Files:**
- Create: `crates/setup/Cargo.toml`, `crates/setup/src/lib.rs`, `crates/setup/src/theme.rs`, `crates/setup/src/anim.rs`, `crates/setup/src/widgets.rs`, `crates/setup/src/screens/mod.rs`, `crates/setup/src/screens/menu.rs`, `crates/setup/src/screens/placeholder.rs`
- Modify: root `Cargo.toml` (member + dep), `src/main.rs` (`setup` subcommand and no-arg TTY path)

**Interfaces:**
- Produces:
  - `rackscreen_setup::{Ctx { config_path: PathBuf, sim: bool, version: &'static str }, Start { Menu, Calibrate }, run(start: Start, ctx: Ctx) -> Result<()>}`
  - crate-internal: `Screen` trait, `Action`, `ScreenId`, `Shared`, `theme::{Theme, Glyphs}`, `anim::{Slide, spinner_frame, ring_glyph, ease_out}`, `widgets::{header, footer, confirm_dialog, StepView, StepState, step_list}`
  - `screens::make(id: ScreenId, shared: &Shared) -> Box<dyn Screen>`; later tasks replace the placeholder entries in `make` one by one.

- [ ] **Step 1: Manifest and wiring**

`crates/setup/Cargo.toml`:
```toml
[package]
name = "rackscreen-setup"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rackscreen-app = { path = "../app", default-features = false }
rackscreen-core = { path = "../core" }
rackscreen-render = { path = "../render" }
rackscreen-display = { path = "../display", default-features = false }
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
ratatui = "0.30"
nix = { version = "0.31", features = ["user"] }
sha2 = "0.11"

[dev-dependencies]
tempfile = "3"

[features]
default = []
sim = ["rackscreen-app/sim", "rackscreen-display/sim"]
pi = ["rackscreen-app/pi", "rackscreen-display/pi"]
```

Root `Cargo.toml`: add `"crates/setup"` to members, `rackscreen-setup = { path = "crates/setup", default-features = false }` to deps, and extend the features: `sim = [..., "rackscreen-setup/sim"]`, `pi = [..., "rackscreen-setup/pi"]`.

- [ ] **Step 2: theme.rs**

```rust
//! Colours and glyphs. Same palette as the panels; ASCII fallback for non-UTF-8 locales.

use rackscreen_core::theme as pal;
use ratatui::style::{Color, Modifier, Style};

pub struct Glyphs {
    pub pointer: &'static str,
    pub pending: &'static str,
    pub done: &'static str,
    pub failed: &'static str,
    pub warn: &'static str,
    pub dot: &'static str,
    pub arrow_up: &'static str,
    pub spinner: &'static [&'static str],
    pub ring: &'static [&'static str],
}

const UNICODE: Glyphs = Glyphs {
    pointer: "▸",
    pending: "○",
    done: "✓",
    failed: "✗",
    warn: "!",
    dot: "●",
    arrow_up: "↑",
    spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    ring: &["◐", "◓", "◑", "◒"],
};

const ASCII: Glyphs = Glyphs {
    pointer: ">",
    pending: "-",
    done: "OK",
    failed: "X",
    warn: "!",
    dot: "*",
    arrow_up: "^",
    spinner: &["|", "/", "-", "\\"],
    ring: &["(", "^", ")", "v"],
};

#[derive(Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub violet: Color,
    pub blue: Color,
    pub ok: Color,
    pub err: Color,
    pub warn: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub unicode: bool,
}

fn rgb(c: pal::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

impl Theme {
    pub fn detect() -> Theme {
        let lang = std::env::var("LC_ALL").or_else(|_| std::env::var("LC_CTYPE")).or_else(|_| std::env::var("LANG")).unwrap_or_default();
        Theme::new(lang.to_ascii_uppercase().contains("UTF-8") || lang.to_ascii_uppercase().contains("UTF8"))
    }

    pub fn new(unicode: bool) -> Theme {
        Theme {
            accent: rgb(pal::AMBER),
            violet: rgb(pal::VIOLET),
            blue: rgb(pal::BLUE),
            ok: rgb(pal::GREEN),
            err: rgb(pal::RED),
            warn: rgb(pal::AMBER),
            text: Color::Rgb(0xee, 0xee, 0xee),
            dim: rgb(pal::GREY),
            faint: Color::Rgb(0x44, 0x44, 0x44),
            unicode,
        }
    }

    pub fn glyphs(&self) -> &'static Glyphs {
        if self.unicode {
            &UNICODE
        } else {
            &ASCII
        }
    }

    pub fn title(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    pub fn normal(&self) -> Style {
        Style::new().fg(self.text)
    }
    pub fn muted(&self) -> Style {
        Style::new().fg(self.dim)
    }
    pub fn faint_style(&self) -> Style {
        Style::new().fg(self.faint)
    }
    pub fn selected(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    pub fn good(&self) -> Style {
        Style::new().fg(self.ok)
    }
    pub fn bad(&self) -> Style {
        Style::new().fg(self.err)
    }
    pub fn warning(&self) -> Style {
        Style::new().fg(self.warn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_sets_switch() {
        assert_eq!(Theme::new(true).glyphs().done, "✓");
        assert_eq!(Theme::new(false).glyphs().done, "OK");
    }
}
```

- [ ] **Step 3: anim.rs**

```rust
//! Small time-based animations for the TUI. Times are seconds on a monotonic clock.

use rackscreen_core::anim::{Easing, Secs, Tween};

/// A value sliding from `from` to `to` with ease-out.
#[derive(Clone, Copy, Debug)]
pub struct Slide {
    tween: Tween,
}

impl Slide {
    pub fn fixed(v: f32) -> Slide {
        Slide { tween: Tween::new(v, v, 0.0, 0.0, Easing::OutCubic) }
    }
    pub fn to(&self, target: f32, now: Secs, secs: Secs) -> Slide {
        let from = self.value(now);
        Slide { tween: Tween::new(from, target, now, secs, Easing::OutCubic) }
    }
    pub fn value(&self, now: Secs) -> f32 {
        self.tween.value(now)
    }
    pub fn target(&self) -> f32 {
        self.tween.to
    }
    pub fn done(&self, now: Secs) -> bool {
        self.tween.done(now)
    }
}

pub fn spinner_frame<'a>(frames: &'a [&'a str], now: Secs) -> &'a str {
    let i = ((now * 12.0) as usize) % frames.len().max(1);
    frames[i]
}

/// Header glyph cycling on a 2.4 s loop, echoing the panels' breathing ring.
pub fn ring_glyph<'a>(frames: &'a [&'a str], now: Secs) -> &'a str {
    let i = ((now / 2.4 * frames.len() as f64) as usize) % frames.len().max(1);
    frames[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_eases_and_finishes() {
        let s = Slide::fixed(0.0).to(10.0, 1.0, 0.15);
        assert_eq!(s.value(1.0), 0.0);
        assert!(s.value(1.05) > 5.0, "ease-out is front loaded");
        assert!((s.value(2.0) - 10.0).abs() < 1e-5);
        assert!(s.done(1.15));
    }

    #[test]
    fn spinner_and_ring_cycle() {
        let f = ["a", "b", "c", "d"];
        assert_eq!(ring_glyph(&f, 0.0), "a");
        assert_eq!(ring_glyph(&f, 0.6), "b");
        assert_eq!(ring_glyph(&f, 2.4), "a");
        assert_ne!(spinner_frame(&f, 0.0), spinner_frame(&f, 0.1));
    }
}
```

- [ ] **Step 4: widgets.rs**

```rust
//! Shared drawing helpers: header, footer, step list, confirm dialog.

use rackscreen_core::anim::Secs;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;

use crate::anim::{ring_glyph, spinner_frame};
use crate::theme::Theme;

pub fn header(f: &mut Frame, area: Rect, th: &Theme, now: Secs, version: &str, subtitle: &str) {
    let g = th.glyphs();
    let title = Line::from(vec![Span::styled(" RackScreen ".to_string(), th.title())]);
    let block = Block::new().borders(Borders::TOP).title(title).title(Line::from(Span::styled(format!(" v{version} "), th.muted())).alignment(Alignment::Right));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(ring_glyph(g.ring, now).to_string(), th.title()),
        Span::raw("  "),
        Span::styled(subtitle.to_string(), th.normal()),
    ]);
    f.render_widget(Paragraph::new(line), Rect { y: inner.y + inner.height.saturating_sub(1).min(1), height: 1, ..inner });
}

pub fn footer(f: &mut Frame, area: Rect, th: &Theme, keys: &str) {
    let block = Block::new().borders(Borders::TOP).title(Line::from(Span::styled(format!(" {keys} "), th.muted())));
    f.render_widget(block, area);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Done(String),
    Skipped(String),
    Warn(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct StepView {
    pub title: String,
    pub state: StepState,
}

/// Rows of steps with a state glyph, plus an eased progress bar underneath.
pub fn step_list(f: &mut Frame, area: Rect, th: &Theme, steps: &[StepView], progress: f32, now: Secs) {
    let g = th.glyphs();
    let [list, bar] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let mut lines = Vec::new();
    for s in steps {
        let (glyph, style, note): (String, Style, Option<&str>) = match &s.state {
            StepState::Pending => (g.pending.into(), th.faint_style(), None),
            StepState::Running => (spinner_frame(g.spinner, now).into(), th.selected(), None),
            StepState::Done(n) => (g.done.into(), th.good(), Some(n)),
            StepState::Skipped(n) => (g.done.into(), th.muted(), Some(n)),
            StepState::Warn(n) => (g.warn.into(), th.warning(), Some(n)),
            StepState::Failed(n) => (g.failed.into(), th.bad(), Some(n)),
        };
        let title_style = if matches!(s.state, StepState::Pending) { th.muted() } else { th.normal() };
        lines.push(Line::from(vec![Span::raw("  "), Span::styled(format!("{glyph:<2}"), style), Span::styled(s.title.clone(), title_style)]));
        if let Some(n) = note {
            if !n.is_empty() {
                lines.push(Line::from(vec![Span::raw("      "), Span::styled(n.to_string(), style)]));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), list);
    let gauge = Gauge::default().ratio(progress.clamp(0.0, 1.0) as f64).gauge_style(Style::new().fg(th.accent).bg(th.faint)).label("");
    f.render_widget(gauge, Rect { x: bar.x + 2, width: bar.width.saturating_sub(4), ..bar });
}

/// Centred modal with a coloured border. `lines` are the body; the last footer line lists keys.
pub fn confirm_dialog(f: &mut Frame, area: Rect, th: &Theme, title: &str, lines: &[String], keys: &str, danger: bool) {
    let height = (lines.len() as u16 + 4).min(area.height);
    let width = (lines.iter().map(|l| l.len()).max().unwrap_or(20) as u16 + 6).clamp(30, area.width);
    let [dialog] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    let [dialog] = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas(dialog);
    f.render_widget(Clear, dialog);
    let border = if danger { th.bad() } else { th.selected() };
    let block = Block::bordered().border_style(border).title(Line::from(Span::styled(format!(" {title} "), border))).title_bottom(Line::from(Span::styled(format!(" {keys} "), th.muted())).alignment(Alignment::Right));
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);
    let body: Vec<Line> = lines.iter().map(|l| Line::from(Span::styled(format!(" {l}"), th.normal()))).collect();
    f.render_widget(Paragraph::new(body), Rect { y: inner.y + 1, height: inner.height.saturating_sub(1), ..inner });
}
```

- [ ] **Step 5: lib.rs (App loop, Screen trait, navigation)**

```rust
//! Interactive setup: install, calibrate, configure, status, run, uninstall.

pub mod anim;
pub mod ops;
pub mod screens;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::{DefaultTerminal, Frame};

use crate::anim::Slide;
use crate::theme::Theme;

#[derive(Clone, Debug)]
pub struct Ctx {
    pub config_path: PathBuf,
    pub sim: bool,
    pub version: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Menu,
    Calibrate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenId {
    Menu,
    Install,
    Calibrate,
    Configure,
    Status,
    RunHere,
    Uninstall,
}

pub enum Action {
    None,
    Go(ScreenId),
    Back,
    Quit,
}

/// State shared by all screens.
pub struct Shared {
    pub ctx: Ctx,
    pub theme: Theme,
    pub service_active: Option<bool>,
    pub banner: Option<String>,
}

pub trait Screen {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, now: Secs) -> Action;
    fn tick(&mut self, _shared: &mut Shared, _now: Secs) {}
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs);
    fn keys(&self) -> String;
    fn subtitle(&self) -> String;
    fn animating(&self, _now: Secs) -> bool {
        false
    }
}

const SLIDE_SECS: Secs = 0.2;

struct App {
    shared: Shared,
    current: Box<dyn Screen>,
    current_id: ScreenId,
    slide: Slide, // 1.0 = fully off to the right, 0.0 = in place
}

impl App {
    fn new(start: Start, ctx: Ctx) -> App {
        let shared = Shared { ctx, theme: Theme::detect(), service_active: None, banner: None };
        let id = match start {
            Start::Menu => ScreenId::Menu,
            Start::Calibrate => ScreenId::Calibrate,
        };
        let current = screens::make(id, &shared);
        App { shared, current, current_id: id, slide: Slide::fixed(0.0) }
    }

    fn go(&mut self, id: ScreenId, now: Secs) {
        self.current = screens::make(id, &self.shared);
        self.current_id = id;
        self.slide = Slide::fixed(1.0).to(0.0, now, SLIDE_SECS);
    }

    fn handle(&mut self, key: KeyEvent, now: Secs) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match self.current.handle(key, &mut self.shared, now) {
            Action::None => {}
            Action::Go(id) => self.go(id, now),
            Action::Back => {
                if self.current_id == ScreenId::Menu {
                    return false;
                }
                self.go(ScreenId::Menu, now);
            }
            Action::Quit => return false,
        }
        true
    }

    fn draw(&self, f: &mut Frame, now: Secs) {
        let area = f.area();
        let [head, body, foot] = Layout::vertical([Constraint::Length(3), Constraint::Min(3), Constraint::Length(1)]).areas(area);
        widgets::header(f, head, &self.shared.theme, now, self.shared.ctx.version, &self.current.subtitle());
        let offset = (self.slide.value(now) * body.width as f32) as u16;
        let shifted = Rect { x: body.x + offset, width: body.width.saturating_sub(offset), ..body };
        if shifted.width > 0 {
            self.current.draw(f, shifted, &self.shared, now);
        }
        widgets::footer(f, foot, &self.shared.theme, &self.current.keys());
    }

    fn animating(&self, now: Secs) -> bool {
        !self.slide.done(now) || self.current.animating(now)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let t0 = Instant::now();
        loop {
            let now = t0.elapsed().as_secs_f64();
            self.current.tick(&mut self.shared, now);
            terminal.draw(|f| self.draw(f, now))?;
            // header glyph animates continuously; 60 Hz only while something moves, else 10 Hz
            let wait = if self.animating(now) { 16 } else { 100 };
            if event::poll(Duration::from_millis(wait))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press && !self.handle(k, now) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Run the TUI. Installs a tracing subscriber that writes into the in-memory log sink so
/// nothing is printed over the UI.
pub fn run(start: Start, ctx: Ctx) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new(start, ctx).run(&mut terminal);
    ratatui::restore();
    result
}
```

(`ops` module is created in Task 4; for this task create `crates/setup/src/ops/mod.rs` containing only `//! System operations.` so `pub mod ops;` compiles.)

- [ ] **Step 6: screens/mod.rs, menu.rs, placeholder.rs**

`screens/mod.rs`:
```rust
pub mod menu;
pub mod placeholder;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        other => Box::new(placeholder::Placeholder::new(other)),
    }
}
```

`screens/placeholder.rs` (replaced screen by screen in later tasks; deleted in Task 10):
```rust
//! Stand-in for screens that later tasks implement.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::{Action, Screen, ScreenId, Shared};

pub struct Placeholder {
    id: ScreenId,
}

impl Placeholder {
    pub fn new(id: ScreenId) -> Self {
        Self { id }
    }
}

impl Screen for Placeholder {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::Back,
            _ => Action::None,
        }
    }
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        f.render_widget(Paragraph::new(Line::styled(format!("  {:?}: not implemented yet", self.id), shared.theme.muted())), area);
    }
    fn keys(&self) -> String {
        "Esc back".into()
    }
    fn subtitle(&self) -> String {
        format!("{:?}", self.id)
    }
}
```

`screens/menu.rs`:
```rust
//! Main menu with a sliding highlight bar.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::{Action, Screen, ScreenId, Shared};

pub const ITEMS: [(ScreenId, &str, &str); 6] = [
    (ScreenId::Install, "Install", "set up service + config"),
    (ScreenId::Calibrate, "Calibrate screens", "fix rotation / mirroring"),
    (ScreenId::Configure, "Configure", "cluster, night, display"),
    (ScreenId::Status, "Status", "service, links, logs"),
    (ScreenId::RunHere, "Run here", "foreground with logs"),
    (ScreenId::Uninstall, "Uninstall", "remove everything"),
];

pub struct Menu {
    selected: usize,
    bar: Slide,
}

impl Menu {
    pub fn new(_shared: &Shared) -> Menu {
        Menu { selected: 0, bar: Slide::fixed(0.0) }
    }

    fn select(&mut self, idx: usize, now: Secs) {
        self.selected = idx;
        self.bar = self.bar.to(idx as f32, now, 0.15);
    }
}

impl Screen for Menu {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, now: Secs) -> Action {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = (self.selected + ITEMS.len() - 1) % ITEMS.len();
                self.select(i, now);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                let i = (self.selected + 1) % ITEMS.len();
                self.select(i, now);
                Action::None
            }
            KeyCode::Enter => Action::Go(ITEMS[self.selected].0),
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            _ => Action::None,
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, _, status] = Layout::vertical([Constraint::Length(1), Constraint::Length(ITEMS.len() as u16), Constraint::Min(1), Constraint::Length(2)]).areas(area);
        let bar_pos = self.bar.value(now);
        let mut lines = Vec::new();
        for (i, (_, title, desc)) in ITEMS.iter().enumerate() {
            let dist = (bar_pos - i as f32).abs();
            let hot = dist < 0.5;
            let pointer = if hot { g.pointer } else { " " };
            let title_style = if hot { th.selected() } else { th.normal() };
            let desc_style = if hot { th.muted() } else { th.faint_style() };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(format!("{pointer} "), th.selected()),
                Span::styled(format!("{title:<20}"), title_style),
                Span::styled((*desc).to_string(), desc_style),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);

        let (svc_style, svc_text) = match shared.service_active {
            Some(true) => (th.good(), "service: active"),
            Some(false) => (th.bad(), "service: inactive"),
            None => (th.muted(), "service: unknown"),
        };
        let mut foot = vec![Line::from(vec![
            Span::raw("   "),
            Span::styled(g.dot, svc_style),
            Span::styled(format!(" {svc_text}     "), th.muted()),
            Span::styled(g.dot, th.muted()),
            Span::styled(format!(" config: {}", shared.ctx.config_path.display()), th.muted()),
        ])];
        if let Some(b) = &shared.banner {
            foot.push(Line::from(vec![Span::raw("   "), Span::styled(b.clone(), th.warning())]));
        }
        f.render_widget(Paragraph::new(foot), status);
    }

    fn keys(&self) -> String {
        "↑↓ move  ⏎ select  q quit".into()
    }
    fn subtitle(&self) -> String {
        "Kubernetes rack monitor".into()
    }
    fn animating(&self, now: Secs) -> bool {
        !self.bar.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn shared() -> Shared {
        Shared {
            ctx: Ctx { config_path: "/etc/rackscreen/config.yaml".into(), sim: true, version: "0.2.0" },
            theme: Theme::new(true),
            service_active: Some(true),
            banner: None,
        }
    }

    #[test]
    fn menu_renders_items_and_pointer() {
        let sh = shared();
        let menu = Menu::new(&sh);
        let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
        term.draw(|f| menu.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("▸ Install"));
        assert!(text.contains("Uninstall"));
        assert!(text.contains("service: active"));
    }

    #[test]
    fn navigation_wraps_and_enter_goes() {
        let mut sh = shared();
        let mut menu = Menu::new(&sh);
        let up = KeyEvent::from(KeyCode::Up);
        assert!(matches!(menu.handle(up, &mut sh, 0.0), Action::None));
        assert_eq!(menu.selected, ITEMS.len() - 1);
        assert!(matches!(menu.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 1.0), Action::Go(ScreenId::Uninstall)));
        assert!(menu.animating(0.01));
    }
}
```

- [ ] **Step 7: Wire `setup` into main.rs**

Add to `Cmd`: `/// Interactive setup (default when run in a terminal)` `Setup { #[arg(long)] sim: bool, #[arg(long)] config: Option<PathBuf> }`. Replace the `None` branch and add the `Setup` branch:

```rust
        Some(Cmd::Setup { sim, config }) => setup(rackscreen_setup::Start::Menu, sim, config),
        None => {
            if std::io::stdin().is_terminal() {
                setup(rackscreen_setup::Start::Menu, false, None)
            } else {
                Cli::command().print_help()?;
                std::process::exit(2);
            }
        }
```
and the helper:
```rust
fn setup(start: rackscreen_setup::Start, sim: bool, config: Option<PathBuf>) -> Result<()> {
    let ctx = rackscreen_setup::Ctx {
        config_path: config.unwrap_or_else(Config::default_path),
        sim,
        version: env!("CARGO_PKG_VERSION"),
    };
    rackscreen_setup::run(start, ctx)
}
```
(Root re-exec via sudo is added in Task 11.)

- [ ] **Step 8: Test, try, commit**

Run: `cargo test -p rackscreen-setup` (4 tests), then the full workspace tests and clippy. Manually: `cargo run` in a terminal shows the menu with the animated glyph; arrows move the pointer with a short slide; Enter on any item shows the placeholder; Esc returns; `q` quits and the terminal is restored.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): TUI scaffold with animated menu"
```

---

### Task 4: System operations: Shell, Paths, boot files, systemd, config helpers

**Files:**
- Create: `crates/setup/src/ops/shell.rs`, `ops/paths.rs`, `ops/boot.rs`, `ops/systemd.rs`, `ops/config_file.rs`
- Modify: `crates/setup/src/ops/mod.rs`

**Interfaces:**
- Produces:
  - `ops::shell::{Output { status: i32, stdout: String, stderr: String }, Shell (trait: fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>), RealShell, FakeShell}`; `FakeShell::new()`, `.respond(prefix: &str, out: Output)`, `.calls() -> Vec<String>`; `Output::ok(stdout)`, `Output::fail(status, stderr)`, `.success() -> bool`.
  - `ops::paths::{Paths { root: PathBuf }, Paths::system(), Paths::under(root), .binary(), .config_dir(), .config(), .unit(), .boot_dir() -> Option<PathBuf>, .config_txt(), .cmdline_txt()}`; `paths::service_user() -> String`; `paths::is_root() -> bool`; `paths::current_exe() -> Result<PathBuf>`.
  - `ops::boot::{ensure_line(text, line) -> (String, bool), ensure_cmdline_token(text, token) -> (String, bool), BootChange { ConfigLine(&'static str), CmdlineToken(&'static str) }, needed_changes(config_txt: &str, cmdline: &str) -> Vec<BootChange>, CONFIG_LINES, CMDLINE_TOKEN, Readiness { spi_on, spi1_overlay, bufsiz }, readiness(config_txt, cmdline) -> Readiness}`
  - `ops::systemd::{unit_text() -> String, Systemd<'a>::new(sh: &'a dyn Shell, user: &str), .enable_now(), .disable_now(), .restart(), .stop(), .start(), .daemon_reload(), .is_active() -> Result<bool>, .is_enabled() -> Result<bool>, .info() -> Result<ServiceInfo { active: String, sub: String, uptime_secs: Option<u64> }>, .journal_tail(n) -> Result<Vec<String>>, LinkDots { api, prometheus, qbittorrent: Dot }, Dot { Up, Down, Unknown }, links_from_logs(&[String]) -> LinkDots}`
  - `ops::config_file::{set_orientation(cfg: &mut Config, index, rotate, hflip), set_spi_chunk(cfg, chunk)}` and re-export of `Config`.

- [ ] **Step 1: shell.rs**

```rust
//! Command execution behind a trait so operations are testable without a Pi.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Context, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(stdout: &str) -> Output {
        Output { status: 0, stdout: stdout.into(), stderr: String::new() }
    }
    pub fn fail(status: i32, stderr: &str) -> Output {
        Output { status, stdout: String::new(), stderr: stderr.into() }
    }
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

pub trait Shell: Send + Sync {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>;

    /// Run and turn a non-zero exit into an error carrying stderr.
    fn check(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let out = self.run(cmd, args)?;
        if !out.success() {
            anyhow::bail!("{cmd} {} failed ({}): {}", args.join(" "), out.status, out.stderr.trim());
        }
        Ok(out)
    }
}

pub struct RealShell;

impl Shell for RealShell {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let out = Command::new(cmd).args(args).output().with_context(|| format!("spawn {cmd}"))?;
        Ok(Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Records every call and answers by longest matching prefix of "cmd arg arg ...".
#[derive(Default)]
pub struct FakeShell {
    calls: Mutex<Vec<String>>,
    responses: Mutex<HashMap<String, Output>>,
}

impl FakeShell {
    pub fn new() -> FakeShell {
        FakeShell::default()
    }
    pub fn respond(&self, prefix: &str, out: Output) {
        self.responses.lock().unwrap().insert(prefix.to_string(), out);
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    pub fn called(&self, prefix: &str) -> bool {
        self.calls().iter().any(|c| c.starts_with(prefix))
    }
}

impl Shell for FakeShell {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let line = std::iter::once(cmd).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        self.calls.lock().unwrap().push(line.clone());
        let responses = self.responses.lock().unwrap();
        let best = responses.iter().filter(|(k, _)| line.starts_with(k.as_str())).max_by_key(|(k, _)| k.len());
        Ok(best.map(|(_, o)| o.clone()).unwrap_or_else(|| Output::ok("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_records_and_answers_by_prefix() {
        let sh = FakeShell::new();
        sh.respond("systemctl is-active", Output::fail(3, "inactive"));
        assert!(sh.run("systemctl", &["is-active", "x"]).unwrap().status == 3);
        assert!(sh.run("systemctl", &["daemon-reload"]).unwrap().success());
        assert!(sh.called("systemctl daemon-reload"));
        assert!(sh.check("systemctl", &["is-active", "x"]).is_err());
    }

    #[test]
    fn real_shell_runs_true() {
        assert!(RealShell.run("true", &[]).unwrap().success());
        assert!(!RealShell.run("false", &[]).unwrap().success());
    }
}
```

- [ ] **Step 2: paths.rs**

```rust
//! Where things live on the device. `root` lets tests point everything into a temp dir.

use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn system() -> Paths {
        Paths { root: PathBuf::from("/") }
    }
    pub fn under(root: impl Into<PathBuf>) -> Paths {
        Paths { root: root.into() }
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    pub fn binary(&self) -> PathBuf {
        self.p("usr/local/bin/rackscreen")
    }
    pub fn config_dir(&self) -> PathBuf {
        self.p("etc/rackscreen")
    }
    pub fn config(&self) -> PathBuf {
        self.p("etc/rackscreen/config.yaml")
    }
    pub fn unit(&self) -> PathBuf {
        self.p("etc/systemd/system/rackscreen@.service")
    }
    /// `/boot/firmware` on current Raspberry Pi OS, `/boot` on older images, None elsewhere.
    pub fn boot_dir(&self) -> Option<PathBuf> {
        [self.p("boot/firmware"), self.p("boot")].into_iter().find(|d| d.join("config.txt").exists())
    }
    pub fn config_txt(&self) -> Option<PathBuf> {
        self.boot_dir().map(|d| d.join("config.txt"))
    }
    pub fn cmdline_txt(&self) -> Option<PathBuf> {
        self.boot_dir().map(|d| d.join("cmdline.txt"))
    }
}

/// The user the service runs as: whoever invoked sudo, else the current user, never root.
pub fn service_user() -> String {
    let candidate = std::env::var("SUDO_USER").ok().filter(|u| !u.is_empty()).or_else(|| std::env::var("USER").ok());
    match candidate {
        Some(u) if u != "root" => u,
        _ => "pi".to_string(),
    }
}

pub fn is_root() -> bool {
    nix::unistd::geteuid().is_root()
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("current executable path")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_under_root() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::under(dir.path());
        assert!(p.binary().ends_with("usr/local/bin/rackscreen"));
        assert_eq!(p.boot_dir(), None);
        std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
        std::fs::write(dir.path().join("boot/firmware/config.txt"), "").unwrap();
        assert!(p.config_txt().unwrap().ends_with("boot/firmware/config.txt"));
    }

    #[test]
    fn service_user_never_root() {
        let u = service_user();
        assert_ne!(u, "root");
        assert!(!u.is_empty());
    }
}
```

- [ ] **Step 3: boot.rs**

```rust
//! Pure edits to /boot/firmware/config.txt and cmdline.txt.

pub const CONFIG_LINES: [&str; 2] = ["dtparam=spi=on", "dtoverlay=spi1-2cs"];
pub const CMDLINE_TOKEN: &str = "spidev.bufsiz=65536";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootChange {
    ConfigLine(&'static str),
    CmdlineToken(&'static str),
}

impl BootChange {
    pub fn describe(&self) -> String {
        match self {
            BootChange::ConfigLine(l) => format!("config.txt  + {l}"),
            BootChange::CmdlineToken(t) => format!("cmdline.txt + {t}"),
        }
    }
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().map(|l| l.trim().trim_end_matches('\r')).any(|l| l == line)
}

/// Append `line` if no non-comment line equals it. Returns (new text, changed).
pub fn ensure_line(text: &str, line: &str) -> (String, bool) {
    if has_line(text, line) {
        return (text.to_string(), false);
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    (out, true)
}

/// cmdline.txt is one line of space separated tokens. Append `token` if missing.
pub fn ensure_cmdline_token(text: &str, token: &str) -> (String, bool) {
    let line = text.lines().next().unwrap_or("").trim_end_matches('\r');
    if line.split_whitespace().any(|t| t == token) {
        return (text.to_string(), false);
    }
    let mut out = line.trim_end().to_string();
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(token);
    out.push('\n');
    (out, true)
}

pub fn needed_changes(config_txt: &str, cmdline: &str) -> Vec<BootChange> {
    let mut v = Vec::new();
    for l in CONFIG_LINES {
        if !has_line(config_txt, l) {
            v.push(BootChange::ConfigLine(l));
        }
    }
    if !ensure_cmdline_token(cmdline, CMDLINE_TOKEN).1 {
        // token present
    } else {
        v.push(BootChange::CmdlineToken(CMDLINE_TOKEN));
    }
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readiness {
    pub spi_on: bool,
    pub spi1_overlay: bool,
    pub bufsiz: bool,
}

pub fn readiness(config_txt: &str, cmdline: &str) -> Readiness {
    Readiness {
        spi_on: has_line(config_txt, CONFIG_LINES[0]),
        spi1_overlay: has_line(config_txt, CONFIG_LINES[1]),
        bufsiz: !ensure_cmdline_token(cmdline, CMDLINE_TOKEN).1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_line_cases() {
        assert_eq!(ensure_line("", "a=b"), ("a=b\n".into(), true));
        assert_eq!(ensure_line("x=1\na=b\n", "a=b"), ("x=1\na=b\n".into(), false));
        assert_eq!(ensure_line("x=1", "a=b"), ("x=1\na=b\n".into(), true));
        assert_eq!(ensure_line("  a=b  \r\n", "a=b").1, false, "trailing spaces and CRLF still count");
        assert_eq!(ensure_line("#a=b\n", "a=b").1, true, "commented line does not count");
    }

    #[test]
    fn cmdline_token_cases() {
        let base = "console=serial0,115200 root=PARTUUID=abc rootwait";
        let (t, changed) = ensure_cmdline_token(base, CMDLINE_TOKEN);
        assert!(changed);
        assert_eq!(t, format!("{base} {CMDLINE_TOKEN}\n"));
        assert!(!ensure_cmdline_token(&t, CMDLINE_TOKEN).1);
        assert_eq!(ensure_cmdline_token("", CMDLINE_TOKEN).0, format!("{CMDLINE_TOKEN}\n"));
    }

    #[test]
    fn needed_and_readiness() {
        let all = needed_changes("", "");
        assert_eq!(all.len(), 3);
        let none = needed_changes("dtparam=spi=on\ndtoverlay=spi1-2cs\n", "root=x spidev.bufsiz=65536");
        assert!(none.is_empty());
        let r = readiness("dtparam=spi=on\n", "");
        assert_eq!(r, Readiness { spi_on: true, spi1_overlay: false, bufsiz: false });
        assert_eq!(BootChange::CmdlineToken(CMDLINE_TOKEN).describe(), "cmdline.txt + spidev.bufsiz=65536");
    }
}
```

- [ ] **Step 4: systemd.rs**

```rust
//! systemd unit, service control and journal reading through the Shell trait.

use anyhow::Result;

use crate::ops::shell::Shell;

pub fn unit_text() -> String {
    "[Unit]\nDescription=RackScreen Kubernetes rack monitor\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nUser=%i\nExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml\nRestart=always\nRestartSec=3\nEnvironment=RUST_LOG=info\n\n[Install]\nWantedBy=multi-user.target\n".to_string()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceInfo {
    pub active: String,
    pub sub: String,
    pub uptime_secs: Option<u64>,
}

pub struct Systemd<'a> {
    sh: &'a dyn Shell,
    unit: String,
}

impl<'a> Systemd<'a> {
    pub fn new(sh: &'a dyn Shell, user: &str) -> Systemd<'a> {
        Systemd { sh, unit: format!("rackscreen@{user}") }
    }
    pub fn unit(&self) -> &str {
        &self.unit
    }
    pub fn daemon_reload(&self) -> Result<()> {
        self.sh.check("systemctl", &["daemon-reload"]).map(|_| ())
    }
    pub fn enable_now(&self) -> Result<()> {
        self.sh.check("systemctl", &["enable", "--now", &self.unit]).map(|_| ())
    }
    pub fn disable_now(&self) -> Result<()> {
        // Not an error if the unit was never installed.
        let _ = self.sh.run("systemctl", &["disable", "--now", &self.unit])?;
        Ok(())
    }
    pub fn start(&self) -> Result<()> {
        self.sh.check("systemctl", &["start", &self.unit]).map(|_| ())
    }
    pub fn stop(&self) -> Result<()> {
        self.sh.check("systemctl", &["stop", &self.unit]).map(|_| ())
    }
    pub fn restart(&self) -> Result<()> {
        self.sh.check("systemctl", &["restart", &self.unit]).map(|_| ())
    }
    pub fn is_active(&self) -> Result<bool> {
        Ok(self.sh.run("systemctl", &["is-active", "--quiet", &self.unit])?.success())
    }
    pub fn is_enabled(&self) -> Result<bool> {
        Ok(self.sh.run("systemctl", &["is-enabled", "--quiet", &self.unit])?.success())
    }
    pub fn info(&self) -> Result<ServiceInfo> {
        let out = self.sh.run("systemctl", &["show", &self.unit, "-p", "ActiveState,SubState,ActiveEnterTimestampMonotonic"])?;
        Ok(parse_show(&out.stdout, uptime_now_usecs()))
    }
    pub fn journal_tail(&self, n: usize) -> Result<Vec<String>> {
        let n = n.to_string();
        let out = self.sh.run("journalctl", &["-u", &self.unit, "-n", &n, "--no-pager", "-o", "cat"])?;
        Ok(out.stdout.lines().map(str::to_string).collect())
    }
}

fn uptime_now_usecs() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = text.split_whitespace().next()?.parse().ok()?;
    Some((secs * 1_000_000.0) as u64)
}

pub fn parse_show(stdout: &str, now_monotonic_usecs: Option<u64>) -> ServiceInfo {
    let mut info = ServiceInfo::default();
    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "ActiveState" => info.active = v.to_string(),
                "SubState" => info.sub = v.to_string(),
                "ActiveEnterTimestampMonotonic" => {
                    if let (Ok(t), Some(now)) = (v.parse::<u64>(), now_monotonic_usecs) {
                        if t > 0 && now >= t {
                            info.uptime_secs = Some((now - t) / 1_000_000);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    info
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Up,
    Down,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkDots {
    pub api: Dot,
    pub prometheus: Dot,
    pub qbittorrent: Dot,
}

/// Newest matching log line decides each dot. Lines come oldest first.
pub fn links_from_logs(lines: &[String]) -> LinkDots {
    let mut d = LinkDots { api: Dot::Unknown, prometheus: Dot::Unknown, qbittorrent: Dot::Unknown };
    for l in lines {
        let lower = l.to_ascii_lowercase();
        let warn = lower.contains("warn") || lower.contains("error");
        if lower.contains("prometheus: forwarding") {
            d.prometheus = Dot::Up;
        } else if lower.contains("prometheus") && warn {
            d.prometheus = Dot::Down;
        }
        if lower.contains("qbittorrent: forwarding") {
            d.qbittorrent = Dot::Up;
        } else if lower.contains("qbittorrent") && warn {
            d.qbittorrent = Dot::Down;
        }
        if lower.contains("pod watch") && warn {
            d.api = Dot::Down;
        } else if lower.contains("running (") {
            d.api = Dot::Up;
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};

    #[test]
    fn unit_text_has_required_lines() {
        let u = unit_text();
        for needle in ["User=%i", "ExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml", "Restart=always", "RestartSec=3", "After=network-online.target", "Environment=RUST_LOG=info"] {
            assert!(u.contains(needle), "{needle}");
        }
    }

    #[test]
    fn commands_and_states() {
        let sh = FakeShell::new();
        sh.respond("systemctl is-active", Output::fail(3, ""));
        let sd = Systemd::new(&sh, "silke");
        assert_eq!(sd.unit(), "rackscreen@silke");
        assert!(!sd.is_active().unwrap());
        sd.enable_now().unwrap();
        assert!(sh.called("systemctl enable --now rackscreen@silke"));
        sh.respond("systemctl stop", Output::fail(1, "boom"));
        assert!(sd.stop().is_err());
        assert!(sd.disable_now().is_ok());
    }

    #[test]
    fn parse_show_and_uptime() {
        let i = parse_show("ActiveState=active\nSubState=running\nActiveEnterTimestampMonotonic=5000000\n", Some(65_000_000));
        assert_eq!(i, ServiceInfo { active: "active".into(), sub: "running".into(), uptime_secs: Some(60) });
        assert_eq!(parse_show("ActiveState=inactive\n", None).uptime_secs, None);
    }

    #[test]
    fn journal_and_links() {
        let sh = FakeShell::new();
        sh.respond("journalctl", Output::ok("INFO prometheus: forwarding to monitoring/p (service prometheus)\nWARN qbittorrent: login failed\nINFO running (4 screens, 30 fps, source K8s)\n"));
        let sd = Systemd::new(&sh, "pi");
        let lines = sd.journal_tail(20).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(sh.called("journalctl -u rackscreen@pi -n 20"));
        let d = links_from_logs(&lines);
        assert_eq!(d, LinkDots { api: Dot::Up, prometheus: Dot::Up, qbittorrent: Dot::Down });
        assert_eq!(links_from_logs(&[]).api, Dot::Unknown);
    }
}
```

- [ ] **Step 5: config_file.rs and ops/mod.rs**

`ops/config_file.rs`:
```rust
//! Small mutations on the YAML config used by the setup screens.

pub use rackscreen_app::config::Config;

pub fn set_orientation(cfg: &mut Config, index: usize, rotate: u32, hflip: bool) {
    if let Some(s) = cfg.screens.get_mut(index) {
        s.rotate = rotate;
        s.hflip = hflip;
    }
}

pub fn set_spi_chunk(cfg: &mut Config, chunk: usize) {
    cfg.display.spi_chunk = chunk;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutations_touch_only_their_fields() {
        let mut c = Config::default();
        let before = c.clone();
        set_orientation(&mut c, 1, 90, false);
        set_orientation(&mut c, 99, 0, true);
        set_spi_chunk(&mut c, 65536);
        assert_eq!(c.screens[1].rotate, 90);
        assert!(!c.screens[1].hflip);
        assert_eq!(c.display.spi_chunk, 65536);
        assert_eq!(c.screens[0], before.screens[0]);
        assert_eq!(c.prometheus, before.prometheus);
    }
}
```

`ops/mod.rs`:
```rust
//! System operations (files, systemd, boot config) behind testable seams.
pub mod boot;
pub mod config_file;
pub mod paths;
pub mod shell;
pub mod systemd;
```

- [ ] **Step 6: Test and commit**

Run: `cargo test -p rackscreen-setup` (expect 15 tests), clippy.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): shell, paths, boot-file, systemd and config helpers"
```

---

### Task 5: Install and uninstall operations

**Files:**
- Create: `crates/setup/src/ops/install.rs`, `crates/setup/src/ops/uninstall.rs`
- Modify: `crates/setup/src/ops/mod.rs`

**Interfaces:**
- Produces:
  - `ops::install::{StepId { Platform, Binary, Config, Groups, Service, Boot }, StepId::ALL, .title() -> &'static str, Outcome { Done(String), Skipped(String), Warn(String), Failed(String) }, Event { Started(StepId), Finished(StepId, Outcome), AskBoot(Vec<BootChange>), Complete { reboot_needed: bool } }, Installer { sh: Arc<dyn Shell>, paths: Paths, user: String, self_exe: PathBuf }, Installer::run(&self, events: Sender<Event>, replies: Receiver<bool>)}` (blocking; run on a worker thread), plus `Installer::step(&self, id, answer: Option<bool>) -> Result<Outcome>` for tests.
  - `ops::uninstall::{Uninstaller { sh, paths, user }, StepId { Service, Unit, Reload, Config, Binary }, Uninstaller::run(&self, events: Sender<Event>)}` reusing `install::{Outcome, Event}` shape via a shared `Event` enum defined in install.rs with `StepId` being an enum `Step { Install(install::StepId), Uninstall(uninstall::StepId) }`... Simpler: uninstall defines its own `UEvent { Started(UStep), Finished(UStep, Outcome), Complete }`.

- [ ] **Step 1: install.rs**

```rust
//! The install steps. Pure planning plus effects through Shell and Paths.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::ops::boot::{self, BootChange};
use crate::ops::config_file::{set_spi_chunk, Config};
use crate::ops::paths::Paths;
use crate::ops::shell::Shell;
use crate::ops::systemd::{unit_text, Systemd};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Platform,
    Binary,
    Config,
    Groups,
    Service,
    Boot,
}

impl StepId {
    pub const ALL: [StepId; 6] = [StepId::Platform, StepId::Binary, StepId::Config, StepId::Groups, StepId::Service, StepId::Boot];

    pub fn title(self) -> &'static str {
        match self {
            StepId::Platform => "Check platform",
            StepId::Binary => "Install binary to /usr/local/bin",
            StepId::Config => "Write config /etc/rackscreen/config.yaml",
            StepId::Groups => "Add user to spi and gpio groups",
            StepId::Service => "Install and start systemd service",
            StepId::Boot => "Enable SPI in boot files",
        }
    }
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
    Complete { reboot_needed: bool },
}

pub struct Installer {
    pub sh: Arc<dyn Shell>,
    pub paths: Paths,
    pub user: String,
    pub self_exe: PathBuf,
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
                    Ok(changes) if changes.is_empty() => Ok(Outcome::Skipped("already enabled".into())),
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
        let _ = events.send(Event::Complete { reboot_needed: reboot });
    }

    pub fn boot_plan(&self) -> Result<Vec<BootChange>> {
        let (Some(cfg), Some(cmd)) = (self.paths.config_txt(), self.paths.cmdline_txt()) else {
            anyhow::bail!("no /boot/firmware/config.txt (not a Raspberry Pi?)");
        };
        let c = std::fs::read_to_string(&cfg).unwrap_or_default();
        let l = std::fs::read_to_string(&cmd).unwrap_or_default();
        Ok(boot::needed_changes(&c, &l))
    }

    /// One step. `answer` is only used by the Boot step.
    pub fn step(&self, id: StepId, answer: Option<bool>) -> Result<Outcome> {
        match id {
            StepId::Platform => {
                let mut warns = Vec::new();
                if std::env::consts::ARCH != "aarch64" {
                    warns.push(format!("arch is {}, expected aarch64", std::env::consts::ARCH));
                }
                if self.paths.boot_dir().is_none() {
                    warns.push("no /boot/firmware (not a Raspberry Pi?)".into());
                }
                if !self.sh.run("getent", &["group", "spi"])?.success() {
                    warns.push("no 'spi' group; enable SPI and reboot first".into());
                }
                Ok(if warns.is_empty() { Outcome::Done("Raspberry Pi, aarch64".into()) } else { Outcome::Warn(warns.join("; ")) })
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
                Config::default().save(&path)?;
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
                    return Ok(Outcome::Warn(format!("groups missing: {}", missing.join(", "))));
                }
                self.sh.check("usermod", &["-aG", "spi,gpio", &self.user])?;
                Ok(Outcome::Done(format!("{} in spi,gpio", self.user)))
            }
            StepId::Service => {
                let unit = self.paths.unit();
                if let Some(dir) = unit.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&unit, unit_text()).with_context(|| format!("write {}", unit.display()))?;
                let sd = Systemd::new(self.sh.as_ref(), &self.user);
                sd.daemon_reload()?;
                sd.enable_now()?;
                Ok(Outcome::Done(format!("{} enabled and started", sd.unit())))
            }
            StepId::Boot => {
                if answer != Some(true) {
                    return Ok(Outcome::Warn("skipped; SPI must be enabled by hand".into()));
                }
                let (Some(cfg_path), Some(cmd_path)) = (self.paths.config_txt(), self.paths.cmdline_txt()) else {
                    anyhow::bail!("boot files not found");
                };
                let mut c = std::fs::read_to_string(&cfg_path).unwrap_or_default();
                for l in boot::CONFIG_LINES {
                    c = boot::ensure_line(&c, l).0;
                }
                std::fs::write(&cfg_path, c).context("write config.txt")?;
                let l = std::fs::read_to_string(&cmd_path).unwrap_or_default();
                std::fs::write(&cmd_path, boot::ensure_cmdline_token(&l, boot::CMDLINE_TOKEN).0).context("write cmdline.txt")?;
                let mut cfg = Config::load_or_default(&self.paths.config())?;
                set_spi_chunk(&mut cfg, 65536);
                cfg.save(&self.paths.config())?;
                Ok(Outcome::Done("SPI enabled, spi_chunk set to 65536; reboot required".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};
    use std::sync::mpsc;

    fn setup(with_boot: bool) -> (tempfile::TempDir, Installer, Arc<FakeShell>) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("rackscreen-src");
        std::fs::write(&exe, b"binary-v1").unwrap();
        if with_boot {
            std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
            std::fs::write(dir.path().join("boot/firmware/config.txt"), "dtparam=audio=on\n").unwrap();
            std::fs::write(dir.path().join("boot/firmware/cmdline.txt"), "root=x rootwait\n").unwrap();
        }
        let sh = Arc::new(FakeShell::new());
        let inst = Installer { sh: sh.clone(), paths: Paths::under(dir.path()), user: "silke".into(), self_exe: exe };
        (dir, inst, sh)
    }

    #[test]
    fn fresh_install_runs_all_steps_and_applies_boot() {
        let (dir, inst, sh) = setup(true);
        let (tx, rx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        rtx.send(true).unwrap();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(matches!(events.last(), Some(Event::Complete { reboot_needed: true })));
        assert!(events.iter().any(|e| matches!(e, Event::AskBoot(c) if c.len() == 3)));
        assert!(events.iter().any(|e| matches!(e, Event::Finished(StepId::Binary, Outcome::Done(_)))));
        assert_eq!(std::fs::read(dir.path().join("usr/local/bin/rackscreen")).unwrap(), b"binary-v1");
        assert!(dir.path().join("etc/rackscreen/config.yaml").exists());
        assert!(std::fs::read_to_string(dir.path().join("etc/systemd/system/rackscreen@.service")).unwrap().contains("User=%i"));
        assert!(sh.called("usermod -aG spi,gpio silke"));
        assert!(sh.called("systemctl enable --now rackscreen@silke"));
        let c = std::fs::read_to_string(dir.path().join("boot/firmware/config.txt")).unwrap();
        assert!(c.contains("dtoverlay=spi1-2cs"));
        let l = std::fs::read_to_string(dir.path().join("boot/firmware/cmdline.txt")).unwrap();
        assert!(l.trim().ends_with("spidev.bufsiz=65536"));
        assert_eq!(Config::load_or_default(&inst.paths.config()).unwrap().display.spi_chunk, 65536);
    }

    #[test]
    fn second_install_skips_binary_and_config_and_declined_boot_warns() {
        let (_dir, inst, _sh) = setup(true);
        assert!(matches!(inst.step(StepId::Binary, None).unwrap(), Outcome::Done(_)));
        assert!(matches!(inst.step(StepId::Binary, None).unwrap(), Outcome::Skipped(_)));
        assert!(matches!(inst.step(StepId::Config, None).unwrap(), Outcome::Done(_)));
        assert!(matches!(inst.step(StepId::Config, None).unwrap(), Outcome::Skipped(_)));
        assert!(matches!(inst.step(StepId::Boot, Some(false)).unwrap(), Outcome::Warn(_)));
    }

    #[test]
    fn desktop_without_boot_dir_warns_and_completes() {
        let (_dir, inst, sh) = setup(false);
        sh.respond("getent group spi", Output::fail(2, ""));
        let (tx, rx) = mpsc::channel();
        let (_rtx, rrx) = mpsc::channel();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(matches!(events.iter().find(|e| matches!(e, Event::Finished(StepId::Platform, _))), Some(Event::Finished(_, Outcome::Warn(_)))));
        assert!(matches!(events.iter().find(|e| matches!(e, Event::Finished(StepId::Groups, _))), Some(Event::Finished(_, Outcome::Warn(_)))));
        assert!(matches!(events.iter().find(|e| matches!(e, Event::Finished(StepId::Boot, _))), Some(Event::Finished(_, Outcome::Warn(_)))));
        assert!(matches!(events.last(), Some(Event::Complete { reboot_needed: false })));
    }

    #[test]
    fn service_failure_stops_the_run() {
        let (_dir, inst, sh) = setup(true);
        sh.respond("systemctl enable", Output::fail(1, "unit masked"));
        let (tx, rx) = mpsc::channel();
        let (_rtx, rrx) = mpsc::channel();
        inst.run(tx, rrx);
        let events: Vec<Event> = rx.iter().collect();
        assert!(events.iter().any(|e| matches!(e, Event::Finished(StepId::Service, Outcome::Failed(m)) if m.contains("unit masked"))));
        assert!(!events.iter().any(|e| matches!(e, Event::Started(StepId::Boot))));
    }
}
```

- [ ] **Step 2: uninstall.rs**

```rust
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
    pub const ALL: [UStep; 5] = [UStep::Service, UStep::Unit, UStep::Reload, UStep::Config, UStep::Binary];
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
            let outcome = self.step(id).unwrap_or_else(|e| Outcome::Failed(format!("{e:#}")));
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
        let u = Uninstaller { sh: sh.clone(), paths: paths.clone(), user: "silke".into() };
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
        assert!(rx.iter().all(|e| !matches!(e, UEvent::Finished(_, Outcome::Failed(_)))));
    }
}
```

`ops/mod.rs` add `pub mod install;` and `pub mod uninstall;`.

- [ ] **Step 3: Test and commit**

Run: `cargo test -p rackscreen-setup` (20 tests), clippy.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): install and uninstall operations with fake-shell tests"
```

---

### Task 6: Install and Uninstall screens

**Files:**
- Create: `crates/setup/src/screens/install.rs`, `crates/setup/src/screens/uninstall.rs`
- Modify: `crates/setup/src/screens/mod.rs`

**Interfaces:**
- Consumes: `ops::install::{Installer, Event, StepId, Outcome}`, `ops::uninstall::{Uninstaller, UEvent, UStep}`, `widgets::{step_list, confirm_dialog, StepView, StepState}`, `ops::paths::{Paths, service_user, current_exe}`, `ops::shell::RealShell`.
- Produces: `screens::install::Install::new(shared) -> Install` (starts the worker immediately), `screens::uninstall::Uninstall::new(shared)` (starts after confirm). Both implement `Screen`. `screens::make` routes `ScreenId::Install` and `ScreenId::Uninstall`.

- [ ] **Step 1: install.rs**

```rust
//! Install screen: runs the installer on a worker thread and animates the step list.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::ops::boot::BootChange;
use crate::ops::install::{Event, Installer, Outcome, StepId};
use crate::ops::paths::{current_exe, service_user, Paths};
use crate::ops::shell::RealShell;
use crate::widgets::{confirm_dialog, step_list, StepState, StepView};
use crate::{Action, Screen, ScreenId, Shared};

enum Phase {
    Running,
    AskBoot(Vec<BootChange>),
    Finished { reboot: bool, ok: bool },
}

pub struct Install {
    steps: Vec<StepView>,
    events: Receiver<Event>,
    replies: Sender<bool>,
    phase: Phase,
    progress: Slide,
}

impl Install {
    pub fn new(shared: &Shared) -> Install {
        let (etx, erx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        let steps = StepId::ALL.iter().map(|s| StepView { title: s.title().into(), state: StepState::Pending }).collect();
        let installer = Installer {
            sh: Arc::new(RealShell),
            paths: Paths::system(),
            user: service_user(),
            self_exe: current_exe().unwrap_or_default(),
        };
        let _ = &shared.ctx; // config path is fixed to the system path for install
        std::thread::Builder::new().name("install".into()).spawn(move || installer.run(etx, rrx)).expect("spawn installer");
        Install { steps, events: erx, replies: rtx, phase: Phase::Running, progress: Slide::fixed(0.0) }
    }

    /// Build a screen in a given state without a worker (tests and previews).
    pub fn preview(steps: Vec<StepView>) -> Install {
        let (_etx, erx) = mpsc::channel();
        let (rtx, _rrx) = mpsc::channel();
        Install { steps, events: erx, replies: rtx, phase: Phase::Running, progress: Slide::fixed(0.4) }
    }

    fn idx(id: StepId) -> usize {
        StepId::ALL.iter().position(|s| *s == id).unwrap_or(0)
    }

    fn done_count(&self) -> usize {
        self.steps.iter().filter(|s| !matches!(s.state, StepState::Pending | StepState::Running)).count()
    }
}

fn state_of(o: Outcome) -> StepState {
    match o {
        Outcome::Done(n) => StepState::Done(n),
        Outcome::Skipped(n) => StepState::Skipped(n),
        Outcome::Warn(n) => StepState::Warn(n),
        Outcome::Failed(n) => StepState::Failed(n),
    }
}

impl Screen for Install {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        // Decide on copies of the phase flags first; matching on `&self.phase` would keep
        // it borrowed while the arms assign to it.
        let asking = matches!(self.phase, Phase::AskBoot(_));
        let finished_ok = matches!(self.phase, Phase::Finished { ok: true, .. });
        let finished = matches!(self.phase, Phase::Finished { .. });
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') if asking => {
                let _ = self.replies.send(true);
                self.phase = Phase::Running;
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('n') if asking => {
                let _ = self.replies.send(false);
                self.phase = Phase::Running;
                Action::None
            }
            KeyCode::Enter if finished_ok => {
                shared.service_active = Some(true);
                Action::Go(ScreenId::Calibrate)
            }
            KeyCode::Esc | KeyCode::Char('q') if finished => Action::Back,
            _ => Action::None, // while running, let it finish
        }
    }

    fn tick(&mut self, shared: &mut Shared, now: Secs) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                Event::Started(id) => self.steps[Self::idx(id)].state = StepState::Running,
                Event::Finished(id, o) => self.steps[Self::idx(id)].state = state_of(o),
                Event::AskBoot(changes) => self.phase = Phase::AskBoot(changes),
                Event::Complete { reboot_needed } => {
                    let ok = !self.steps.iter().any(|s| matches!(s.state, StepState::Failed(_)));
                    if reboot_needed {
                        shared.banner = Some("reboot required to enable SPI".into());
                    }
                    shared.service_active = Some(ok);
                    self.phase = Phase::Finished { reboot: reboot_needed, ok };
                }
            }
            let target = self.done_count() as f32 / self.steps.len() as f32;
            if (self.progress.target() - target).abs() > f32::EPSILON {
                self.progress = self.progress.to(target, now, 0.3);
            }
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let [_, list, msg] = Layout::vertical([Constraint::Length(1), Constraint::Min(6), Constraint::Length(3)]).areas(area);
        step_list(f, list, th, &self.steps, self.progress.value(now), now);
        let line = match &self.phase {
            Phase::Running => Line::from(Span::styled("  installing...", th.muted())),
            Phase::AskBoot(_) => Line::from(Span::styled("  waiting for confirmation", th.muted())),
            Phase::Finished { ok: false, .. } => Line::from(Span::styled("  install failed, see the step above", th.bad())),
            Phase::Finished { reboot: true, .. } => Line::from(Span::styled("  done. Reboot, then run `rackscreen` again to calibrate.  ⏎ calibrate now  Esc menu", th.warning())),
            Phase::Finished { .. } => Line::from(Span::styled("  done.  ⏎ calibrate screens now   Esc back to menu", th.good())),
        };
        f.render_widget(Paragraph::new(line), msg);
        if let Phase::AskBoot(changes) = &self.phase {
            let mut lines: Vec<String> = vec!["Edit the boot files to enable SPI?".into(), String::new()];
            lines.extend(changes.iter().map(BootChange::describe));
            lines.push(String::new());
            lines.push("A reboot is needed afterwards.".into());
            confirm_dialog(f, area, th, "Boot files", &lines, "y/⏎ yes   n/Esc skip", false);
        }
    }

    fn keys(&self) -> String {
        match self.phase {
            Phase::Running => "please wait".into(),
            Phase::AskBoot(_) => "y yes  n skip".into(),
            Phase::Finished { .. } => "⏎ calibrate  Esc menu".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Install".into()
    }
    fn animating(&self, now: Secs) -> bool {
        matches!(self.phase, Phase::Running) || !self.progress.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn step_list_renders_states() {
        let sh = Shared {
            ctx: Ctx { config_path: "/etc/rackscreen/config.yaml".into(), sim: true, version: "0.2.0" },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
        };
        let steps = vec![
            StepView { title: "Check platform".into(), state: StepState::Done("Raspberry Pi".into()) },
            StepView { title: "Install binary".into(), state: StepState::Running },
            StepView { title: "Write config".into(), state: StepState::Pending },
            StepView { title: "Boot".into(), state: StepState::Failed("no permission".into()) },
        ];
        let screen = Install::preview(steps);
        let mut term = Terminal::new(TestBackend::new(70, 16)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("✓  Check platform"));
        assert!(text.contains("Raspberry Pi"));
        assert!(text.contains("○  Write config"));
        assert!(text.contains("✗  Boot"));
        assert!(text.contains("no permission"));
    }
}
```

- [ ] **Step 2: uninstall.rs**

```rust
//! Uninstall screen: confirm, then run the uninstaller with the same step list look.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::ops::install::Outcome;
use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::RealShell;
use crate::ops::uninstall::{UEvent, UStep, Uninstaller};
use crate::widgets::{confirm_dialog, step_list, StepState, StepView};
use crate::{Action, Screen, Shared};

enum Phase {
    Confirm,
    Running(Receiver<UEvent>),
    Done,
}

pub struct Uninstall {
    steps: Vec<StepView>,
    phase: Phase,
    progress: Slide,
}

impl Uninstall {
    pub fn new(_shared: &Shared) -> Uninstall {
        let steps = UStep::ALL.iter().map(|s| StepView { title: s.title().into(), state: StepState::Pending }).collect();
        Uninstall { steps, phase: Phase::Confirm, progress: Slide::fixed(0.0) }
    }

    fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        let u = Uninstaller { sh: Arc::new(RealShell), paths: Paths::system(), user: service_user() };
        std::thread::Builder::new().name("uninstall".into()).spawn(move || u.run(tx)).expect("spawn uninstaller");
        self.phase = Phase::Running(rx);
    }

    fn idx(id: UStep) -> usize {
        UStep::ALL.iter().position(|s| *s == id).unwrap_or(0)
    }
}

impl Screen for Uninstall {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let confirm = matches!(self.phase, Phase::Confirm);
        let done = matches!(self.phase, Phase::Done);
        match key.code {
            KeyCode::Char('y') if confirm => {
                self.start();
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') if confirm => Action::Back,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') if done => {
                shared.service_active = Some(false);
                Action::Quit
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, _shared: &mut Shared, now: Secs) {
        let mut finished = false;
        if let Phase::Running(rx) = &self.phase {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    UEvent::Started(id) => self.steps[Self::idx(id)].state = StepState::Running,
                    UEvent::Finished(id, o) => {
                        self.steps[Self::idx(id)].state = match o {
                            Outcome::Done(n) => StepState::Done(n),
                            Outcome::Skipped(n) => StepState::Skipped(n),
                            Outcome::Warn(n) => StepState::Warn(n),
                            Outcome::Failed(n) => StepState::Failed(n),
                        }
                    }
                    UEvent::Complete => finished = true,
                }
            }
            let done = self.steps.iter().filter(|s| !matches!(s.state, StepState::Pending | StepState::Running)).count();
            let target = done as f32 / self.steps.len() as f32;
            if (self.progress.target() - target).abs() > f32::EPSILON {
                self.progress = self.progress.to(target, now, 0.3);
            }
        }
        if finished {
            self.phase = Phase::Done;
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let [_, list, msg] = Layout::vertical([Constraint::Length(1), Constraint::Min(6), Constraint::Length(2)]).areas(area);
        step_list(f, list, th, &self.steps, self.progress.value(now), now);
        if matches!(self.phase, Phase::Done) {
            f.render_widget(Paragraph::new(Line::from(Span::styled("  RackScreen removed. Press ⏎ to exit.", th.good()))), msg);
        }
        if matches!(self.phase, Phase::Confirm) {
            let lines = vec![
                "This removes:".into(),
                "  the systemd service (stopped and disabled)".into(),
                "  /etc/rackscreen (your config)".into(),
                "  /usr/local/bin/rackscreen".into(),
                String::new(),
                "Boot file lines (SPI overlays) are left in place.".into(),
            ];
            confirm_dialog(f, area, th, "Uninstall RackScreen?", &lines, "y remove   n/Esc cancel", true);
        }
    }

    fn keys(&self) -> String {
        match self.phase {
            Phase::Confirm => "y remove  n cancel".into(),
            Phase::Running(_) => "please wait".into(),
            Phase::Done => "⏎ exit".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Uninstall".into()
    }
    fn animating(&self, now: Secs) -> bool {
        matches!(self.phase, Phase::Running(_)) || !self.progress.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn confirm_dialog_shows_first() {
        let sh = Shared {
            ctx: Ctx { config_path: "/etc/rackscreen/config.yaml".into(), sim: true, version: "0.2.0" },
            theme: Theme::new(true),
            service_active: Some(true),
            banner: None,
        };
        let screen = Uninstall::new(&sh);
        let mut term = Terminal::new(TestBackend::new(70, 18)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("Uninstall RackScreen?"));
        assert!(text.contains("/etc/rackscreen"));
        assert!(text.contains("left in place"));
    }
}
```

- [ ] **Step 3: Route in screens/mod.rs**

```rust
pub mod install;
pub mod menu;
pub mod placeholder;
pub mod uninstall;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        ScreenId::Install => Box::new(install::Install::new(shared)),
        ScreenId::Uninstall => Box::new(uninstall::Uninstall::new(shared)),
        other => Box::new(placeholder::Placeholder::new(other)),
    }
}
```

- [ ] **Step 4: Test, try on the desktop, commit**

Run: `cargo test -p rackscreen-setup` (22 tests), clippy. Desktop try: `cargo run -- setup --sim` → Install as a normal user: steps fail at Binary with a permission error (expected without sudo; sudo re-exec lands in Task 11). `sudo -E target/debug/rackscreen setup --sim` on the desktop is allowed but will really install to /usr/local/bin and /etc; only do it if you are willing to uninstall afterwards via the same TUI. Uninstall shows the red confirm dialog first.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): install and uninstall screens"
```

---

### Task 7: Screen calibration

**Files:**
- Create: `crates/app/src/calibrate.rs`, `crates/setup/src/screens/calibrate.rs`, `assets/icons/arrow-up.svg`
- Modify: `scripts/fetch-assets.sh` (add `arrow-up` to ICONS), `crates/render/src/assets.rs` (add `"arrow-up"` to the `icons!` list), `crates/app/src/lib.rs`, `crates/setup/src/screens/mod.rs`, `src/main.rs` (`calibrate` subcommand)

**Interfaces:**
- Consumes: `rackscreen_app::panels::{open_panels, Panels, PanelHandle}`, `rackscreen_render::{renderer::Renderer, frame::{new_pixmap, Orient, Rect}}`, `rackscreen_display::DisplayCmd`, `ops::systemd::Systemd`, `ops::config_file::set_orientation`.
- Produces: `rackscreen_app::calibrate::test_pattern(index: usize, role: Role) -> Scene`; `screens::calibrate::{Calibrate::new(shared), mini_panel(rotate, hflip, number, unicode) -> Vec<String>}`; `rackscreen calibrate [--sim] [--config]` CLI.

- [ ] **Step 1: Icon**

Run `curl -sSL https://unpkg.com/lucide-static@0.544.0/icons/arrow-up.svg -o assets/icons/arrow-up.svg`, add `arrow-up` to the `ICONS` list in `scripts/fetch-assets.sh`, and add `"arrow-up",` to the `icons!(...)` list in `crates/render/src/assets.rs`. `cargo test -p rackscreen-render` still passes (the icon parse test covers it).

- [ ] **Step 2: crates/app/src/calibrate.rs**

```rust
//! The calibration test pattern: unmistakable up, number, and a corner dot for mirroring.

use rackscreen_core::scene::{Drawable, Scene, SegState};
use rackscreen_core::theme::layout::{BADGE_CY, CX, CY, RING_R, SEG_N};
use rackscreen_core::theme::{Role, AMBER, BADGE_FILL, WHITE};

pub fn test_pattern(index: usize, role: Role) -> Scene {
    let mut s = Scene::new();
    s.push(Drawable::Ring { cx: CX, cy: CY, radius: RING_R, n: SEG_N, states: vec![SegState::On(role.accent(), 1.0); SEG_N] });
    s.push(Drawable::Icon { name: "arrow-up", cx: CX, cy: CY - 14.0, size: 120.0, color: WHITE, alpha: 1.0, scale: 1.0, dy: 0.0 });
    s.push(Drawable::Badge {
        cx: CX,
        cy: BADGE_CY + 12.0,
        w: 40.0,
        h: 26.0,
        radius: 7.0,
        stroke: role.accent(),
        fill: BADGE_FILL,
        text: format!("{}", index + 1),
        text_px: 15.0,
        text_color: WHITE,
        alpha: 1.0,
    });
    // top-right marker: visible mirror indicator
    s.push(Drawable::Dots { cx: 186.0, cy: 54.0, spacing: 0.0, r: 7.0, colors: vec![AMBER] });
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_has_arrow_number_and_marker() {
        let s = test_pattern(2, Role::Pods);
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Icon { name: "arrow-up", .. })));
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Badge { text, .. } if text == "3")));
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Dots { cx, cy, .. } if *cx > 120.0 && *cy < 120.0)));
        assert_eq!(s.lit_count(), 60);
    }
}
```

`crates/app/src/lib.rs`: add `pub mod calibrate; pub mod panels; pub mod run; pub mod runloop;` (whichever are not yet listed).

- [ ] **Step 3: screens/calibrate.rs**

```rust
//! Calibrate: drive the panels with a test pattern and adjust rotate/hflip per screen.

use rackscreen_app::calibrate::test_pattern;
use rackscreen_app::config::Config;
use rackscreen_app::panels::{open_panels, Panels};
use rackscreen_core::anim::Secs;
use rackscreen_display::DisplayCmd;
use rackscreen_render::frame::{new_pixmap, Orient, Rect};
use rackscreen_render::renderer::Renderer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect as TRect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tiny_skia::Pixmap;

use crate::ops::config_file::set_orientation;
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::{Action, Screen, Shared};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orientation {
    pub rotate: u32,
    pub hflip: bool,
}

pub struct Calibrate {
    cfg: Config,
    orient: Vec<Orientation>,
    selected: usize,
    panels: Option<Panels>,
    renderer: Option<Renderer>,
    frames: Vec<Pixmap>,
    scratch: Pixmap,
    error: Option<String>,
    service_was_active: bool,
    message: String,
}

/// The terminal preview of one panel after rotate/hflip: 5 text rows with an arrow,
/// the number, and a marker in the corner the amber dot ends up in.
pub fn mini_panel(rotate: u32, hflip: bool, number: usize, unicode: bool) -> Vec<String> {
    // Arrow direction after clockwise rotation, then mirrored horizontally.
    let arrows = if unicode { ["↑", "→", "↓", "←"] } else { ["^", ">", "v", "<"] };
    let mut dir = ((rotate % 360) / 90) as usize; // 0 up, 1 right, 2 down, 3 left
    if hflip && (dir == 1 || dir == 3) {
        dir = 4 - dir;
    }
    // Marker starts top-right; rotation moves it clockwise; hflip swaps left/right.
    let mut corner = dir; // 0 TR, 1 BR, 2 BL, 3 TL (same clockwise stepping as the arrow)
    if hflip {
        corner = match corner {
            0 => 3,
            1 => 2,
            2 => 1,
            _ => 0,
        };
    }
    let (o, sp) = if unicode { ("●", " ") } else { ("*", " ") };
    let a = arrows[dir];
    let tl = if corner == 3 { o } else { sp };
    let tr = if corner == 0 { o } else { sp };
    let bl = if corner == 2 { o } else { sp };
    let br = if corner == 1 { o } else { sp };
    vec![
        format!(" {tl}     {tr} "),
        "   .---.   ".to_string(),
        format!("   | {a} |   "),
        format!("   | {number} |   "),
        format!(" {bl} '---' {br} "),
    ]
}

impl Calibrate {
    pub fn new(shared: &Shared) -> Calibrate {
        let cfg = Config::load_or_default(&shared.ctx.config_path).unwrap_or_default();
        let orient = cfg.screens.iter().map(|s| Orientation { rotate: s.rotate, hflip: s.hflip }).collect();
        let sh = RealShell;
        let sd = Systemd::new(&sh, &service_user());
        let service_was_active = !shared.ctx.sim && sd.is_active().unwrap_or(false);
        if service_was_active {
            let _ = sd.stop();
        }
        let mut me = Calibrate {
            cfg,
            orient,
            selected: 0,
            panels: None,
            renderer: None,
            frames: Vec::new(),
            scratch: new_pixmap(),
            error: None,
            service_was_active,
            message: "adjust each screen until the arrow points up and the dot is top-right".into(),
        };
        match (open_panels(&me.cfg, shared.ctx.sim, false, None), Renderer::new()) {
            (Ok(p), Ok(r)) => {
                me.frames = (0..p.handles.len()).map(|_| new_pixmap()).collect();
                me.panels = Some(p);
                me.renderer = Some(r);
                for i in 0..me.orient.len() {
                    me.push(i);
                }
            }
            (Err(e), _) | (_, Err(e)) => me.error = Some(format!("{e:#}")),
        }
        me
    }

    fn push(&mut self, i: usize) {
        let (Some(p), Some(r)) = (&self.panels, &mut self.renderer) else { return };
        let Some(h) = p.handles.get(i) else { return };
        let o = self.orient[i];
        let base = &mut self.frames[i];
        r.render(&test_pattern(i, h.role), base);
        // In the simulator the panel orientation is identity, so apply the candidate
        // orientation here; on real panels do the same (the handle's orient is only used by
        // the monitor, calibration always orients explicitly).
        Orient::new(o.rotate, o.hflip).apply(base, &mut self.scratch);
        h.mailbox.put(DisplayCmd::Frame(self.scratch.clone(), Rect::full()));
    }

    fn save(&mut self, shared: &mut Shared) -> Result<(), String> {
        for (i, o) in self.orient.iter().enumerate() {
            set_orientation(&mut self.cfg, i, o.rotate, o.hflip);
        }
        self.cfg.save(&shared.ctx.config_path).map_err(|e| format!("{e:#}"))
    }

    fn close(&mut self) {
        if let Some(p) = self.panels.take() {
            p.shutdown();
        }
        if self.service_was_active {
            let sh = RealShell;
            let _ = Systemd::new(&sh, &service_user()).start();
            self.service_was_active = false;
        }
    }
}

impl Drop for Calibrate {
    fn drop(&mut self) {
        self.close();
    }
}

impl Screen for Calibrate {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let n = self.orient.len();
        match key.code {
            KeyCode::Char(c @ '1'..='4') => {
                let i = (c as u8 - b'1') as usize;
                if i < n {
                    self.selected = i;
                }
            }
            KeyCode::Left | KeyCode::Up => self.selected = (self.selected + n - 1) % n.max(1),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => self.selected = (self.selected + 1) % n.max(1),
            KeyCode::Char('r') => {
                let o = &mut self.orient[self.selected];
                o.rotate = (o.rotate + 90) % 360;
                self.push(self.selected);
            }
            KeyCode::Char('R') => {
                let o = &mut self.orient[self.selected];
                o.rotate = (o.rotate + 270) % 360;
                self.push(self.selected);
            }
            KeyCode::Char('f') => {
                self.orient[self.selected].hflip = !self.orient[self.selected].hflip;
                self.push(self.selected);
            }
            KeyCode::Char('a') => {
                let o = self.orient[self.selected];
                for i in 0..n {
                    self.orient[i] = o;
                    self.push(i);
                }
            }
            KeyCode::Char('s') => {
                return match self.save(shared) {
                    Ok(()) => {
                        self.close();
                        shared.banner = Some("orientation saved".into());
                        Action::Back
                    }
                    Err(e) => {
                        self.error = Some(e);
                        Action::None
                    }
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close();
                return Action::Back;
            }
            _ => {}
        }
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: TRect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let [left, right] = Layout::horizontal([Constraint::Length(14 * self.orient.len().max(1) as u16 + 2), Constraint::Min(20)]).areas(area);
        // mini panels side by side
        let cols = Layout::horizontal(vec![Constraint::Length(14); self.orient.len().max(1)]).split(TRect { y: left.y + 1, height: 6, ..left });
        for (i, o) in self.orient.iter().enumerate() {
            let lines = mini_panel(o.rotate, o.hflip, i + 1, th.unicode);
            let style = if i == self.selected { th.selected() } else { th.muted() };
            let body: Vec<Line> = lines.into_iter().map(|l| Line::from(Span::styled(l, style))).collect();
            f.render_widget(Paragraph::new(body), cols[i]);
        }
        let mut info = vec![
            Line::from(Span::styled(self.message.clone(), th.normal())),
            Line::from(""),
            Line::from(vec![Span::styled("1-4", th.selected()), Span::styled(" select screen", th.muted())]),
            Line::from(vec![Span::styled("r / R", th.selected()), Span::styled(" rotate +90 / -90", th.muted())]),
            Line::from(vec![Span::styled("f", th.selected()), Span::styled(" flip horizontally", th.muted())]),
            Line::from(vec![Span::styled("a", th.selected()), Span::styled(" apply this orientation to all", th.muted())]),
            Line::from(vec![Span::styled("s", th.selected()), Span::styled(" save and exit    ", th.muted()), Span::styled("Esc", th.selected()), Span::styled(" discard", th.muted())]),
            Line::from(""),
        ];
        if let Some(o) = self.orient.get(self.selected) {
            info.push(Line::from(Span::styled(format!("screen {}: rotate {}  hflip {}", self.selected + 1, o.rotate, o.hflip), th.normal())));
        }
        if let Some(e) = &self.error {
            info.push(Line::from(Span::styled(format!("error: {e}"), th.bad())));
        }
        f.render_widget(Paragraph::new(info), TRect { y: right.y + 1, height: right.height.saturating_sub(1), ..right });
    }

    fn keys(&self) -> String {
        "1-4 screen  r/R rotate  f flip  a all  s save  Esc discard".into()
    }
    fn subtitle(&self) -> String {
        "Calibrate screens".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mini_panel_follows_rotation_and_flip() {
        let up = mini_panel(0, false, 1, true);
        assert!(up[2].contains('↑'));
        assert!(up[0].ends_with("● "), "marker top-right: {:?}", up[0]);
        let right = mini_panel(90, false, 2, true);
        assert!(right[2].contains('→'));
        assert!(right[4].ends_with("● "), "marker bottom-right after 90 cw");
        let flipped = mini_panel(0, true, 3, true);
        assert!(flipped[2].contains('↑'));
        assert!(flipped[0].starts_with(" ●"), "marker top-left when mirrored");
        let both = mini_panel(90, true, 4, true);
        assert!(both[2].contains('←'), "right arrow mirrored becomes left");
        assert!(both[4].starts_with(" ●"));
        assert!(mini_panel(0, false, 1, false)[2].contains('^'));
    }
}
```

- [ ] **Step 4: Route and CLI**

`screens/mod.rs`: add `pub mod calibrate;` and `ScreenId::Calibrate => Box::new(calibrate::Calibrate::new(shared)),`.

`src/main.rs`: add `Calibrate { #[arg(long)] sim: bool, #[arg(long)] config: Option<PathBuf> }` to `Cmd` with doc `/// Fix screen rotation and mirroring interactively` and the arm `Some(Cmd::Calibrate { sim, config }) => setup(rackscreen_setup::Start::Calibrate, sim, config),`.

- [ ] **Step 5: Test, try, commit**

Run: `cargo test --workspace --features sim,pi`, clippy (all three feature sets). Desktop: `cargo run -- calibrate --sim --config /tmp/rs.yaml`: the simulator window shows four panels with a white arrow, numbers 1 to 4, and an amber dot top-right; pressing `r` rotates panel 1's pattern on screen and the terminal preview; `f` mirrors it; `s` writes `/tmp/rs.yaml` with the new `rotate`/`hflip` and returns to the menu with the "orientation saved" banner.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): screen calibration with live test pattern"
```

---

### Task 8: Configure screen

**Files:**
- Create: `crates/setup/src/screens/configure.rs`
- Modify: `crates/setup/src/screens/mod.rs`

**Interfaces:**
- Consumes: `Config` and its `validate()`/`save()`, `rackscreen_core::night::parse_hhmm`, `ops::systemd::Systemd`.
- Produces: `screens::configure::{Field, FIELDS, FieldKind, get(cfg, field) -> String, set(cfg, field, text) -> Result<(), String>, Configure::new(shared)}`.

- [ ] **Step 1: configure.rs**

```rust
//! Configure: a form over the YAML fields with inline editing and validation.

use rackscreen_app::config::Config;
use rackscreen_core::anim::Secs;
use rackscreen_core::night::parse_hhmm;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::widgets::confirm_dialog;
use crate::{Action, Screen, Shared};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Secret,
    Number,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Kubeconfig,
    PromNamespace,
    PromService,
    PromPort,
    PromPoll,
    QbitEnabled,
    QbitNamespace,
    QbitService,
    QbitPort,
    QbitUser,
    QbitPass,
    QbitPoll,
    NightEnabled,
    NightStart,
    NightEnd,
    HotCpu,
    HotMem,
    Brightness,
    Fps,
    SpiChunk,
}

pub const FIELDS: [(Field, &str, FieldKind); 20] = [
    (Field::Kubeconfig, "kubeconfig path", FieldKind::Text),
    (Field::PromNamespace, "prometheus namespace", FieldKind::Text),
    (Field::PromService, "prometheus service", FieldKind::Text),
    (Field::PromPort, "prometheus port", FieldKind::Number),
    (Field::PromPoll, "prometheus poll secs", FieldKind::Number),
    (Field::QbitEnabled, "qbittorrent enabled", FieldKind::Bool),
    (Field::QbitNamespace, "qbittorrent namespace", FieldKind::Text),
    (Field::QbitService, "qbittorrent service", FieldKind::Text),
    (Field::QbitPort, "qbittorrent port", FieldKind::Number),
    (Field::QbitUser, "qbittorrent user", FieldKind::Text),
    (Field::QbitPass, "qbittorrent password", FieldKind::Secret),
    (Field::QbitPoll, "qbittorrent poll secs", FieldKind::Number),
    (Field::NightEnabled, "night mode", FieldKind::Bool),
    (Field::NightStart, "night start (HH:MM)", FieldKind::Text),
    (Field::NightEnd, "night end (HH:MM)", FieldKind::Text),
    (Field::HotCpu, "hot node cpu %", FieldKind::Number),
    (Field::HotMem, "hot node mem %", FieldKind::Number),
    (Field::Brightness, "brightness 0.1-1.0", FieldKind::Number),
    (Field::Fps, "fps", FieldKind::Number),
    (Field::SpiChunk, "spi chunk bytes", FieldKind::Number),
];

pub fn get(cfg: &Config, f: Field) -> String {
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig.clone(),
        Field::PromNamespace => cfg.prometheus.namespace.clone(),
        Field::PromService => cfg.prometheus.service.clone(),
        Field::PromPort => cfg.prometheus.port.to_string(),
        Field::PromPoll => cfg.prometheus.poll_secs.to_string(),
        Field::QbitEnabled => cfg.qbittorrent.enabled.to_string(),
        Field::QbitNamespace => cfg.qbittorrent.namespace.clone(),
        Field::QbitService => cfg.qbittorrent.service.clone(),
        Field::QbitPort => cfg.qbittorrent.port.to_string(),
        Field::QbitUser => cfg.qbittorrent.user.clone(),
        Field::QbitPass => cfg.qbittorrent.pass.clone(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs.to_string(),
        Field::NightEnabled => cfg.night.enabled.to_string(),
        Field::NightStart => cfg.night.start.clone(),
        Field::NightEnd => cfg.night.end.clone(),
        Field::HotCpu => format!("{}", cfg.thresholds.hot_cpu),
        Field::HotMem => format!("{}", cfg.thresholds.hot_mem),
        Field::Brightness => format!("{}", cfg.display.brightness),
        Field::Fps => cfg.display.fps.to_string(),
        Field::SpiChunk => cfg.display.spi_chunk.to_string(),
    }
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    s.trim().parse::<T>().map_err(|_| format!("{what}: not a number"))
}

pub fn set(cfg: &mut Config, f: Field, text: &str) -> Result<(), String> {
    let t = text.trim();
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig = t.into(),
        Field::PromNamespace => cfg.prometheus.namespace = t.into(),
        Field::PromService => cfg.prometheus.service = t.into(),
        Field::PromPort => cfg.prometheus.port = num(t, "port")?,
        Field::PromPoll => cfg.prometheus.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::QbitEnabled => cfg.qbittorrent.enabled = t == "true",
        Field::QbitNamespace => cfg.qbittorrent.namespace = t.into(),
        Field::QbitService => cfg.qbittorrent.service = t.into(),
        Field::QbitPort => cfg.qbittorrent.port = num(t, "port")?,
        Field::QbitUser => cfg.qbittorrent.user = t.into(),
        Field::QbitPass => cfg.qbittorrent.pass = text.into(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::NightEnabled => cfg.night.enabled = t == "true",
        Field::NightStart => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.start = t.into();
        }
        Field::NightEnd => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.end = t.into();
        }
        Field::HotCpu => cfg.thresholds.hot_cpu = num(t, "cpu %")?,
        Field::HotMem => cfg.thresholds.hot_mem = num(t, "mem %")?,
        Field::Brightness => {
            let b: f32 = num(t, "brightness")?;
            if !(0.1..=1.0).contains(&b) {
                return Err("brightness must be between 0.1 and 1.0".into());
            }
            cfg.display.brightness = b;
        }
        Field::Fps => cfg.display.fps = num::<u32>(t, "fps")?.clamp(1, 60),
        Field::SpiChunk => cfg.display.spi_chunk = num::<usize>(t, "spi chunk")?.max(64),
    }
    Ok(())
}

enum Mode {
    Browse,
    Edit(String),
    AskRestart,
}

pub struct Configure {
    cfg: Config,
    row: usize,
    mode: Mode,
    error: Option<String>,
    dirty: bool,
    scroll: usize,
}

impl Configure {
    pub fn new(shared: &Shared) -> Configure {
        let cfg = Config::load_or_default(&shared.ctx.config_path).unwrap_or_default();
        Configure { cfg, row: 0, mode: Mode::Browse, error: None, dirty: false, scroll: 0 }
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if let Err(e) = self.cfg.validate() {
            self.error = Some(format!("{e:#}"));
            return Action::None;
        }
        match self.cfg.save(&shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                shared.banner = Some("config saved".into());
                if shared.service_active == Some(true) && !shared.ctx.sim {
                    self.mode = Mode::AskRestart;
                    Action::None
                } else {
                    Action::Back
                }
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                Action::None
            }
        }
    }
}

impl Screen for Configure {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let (field, _, kind) = FIELDS[self.row];
        // Take the mode out so the arms can replace it without a live borrow.
        let mode = std::mem::replace(&mut self.mode, Mode::Browse);
        match mode {
            Mode::Browse => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.row = (self.row + FIELDS.len() - 1) % FIELDS.len(),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.row = (self.row + 1) % FIELDS.len(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if kind == FieldKind::Bool {
                        let cur = get(&self.cfg, field) == "true";
                        let _ = set(&mut self.cfg, field, if cur { "false" } else { "true" });
                        self.dirty = true;
                    } else {
                        self.mode = Mode::Edit(get(&self.cfg, field));
                    }
                }
                KeyCode::Char('s') => return self.save(shared),
                KeyCode::Esc | KeyCode::Char('q') => return Action::Back,
                _ => {}
            },
            Mode::Edit(mut buf) => match key.code {
                KeyCode::Enter => match set(&mut self.cfg, field, &buf) {
                    Ok(()) => {
                        self.error = None;
                        self.dirty = true;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.mode = Mode::Edit(buf);
                    }
                },
                KeyCode::Esc => self.error = None,
                KeyCode::Backspace => {
                    buf.pop();
                    self.mode = Mode::Edit(buf);
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.mode = Mode::Edit(buf);
                }
                _ => self.mode = Mode::Edit(buf),
            },
            Mode::AskRestart => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                    let sh = RealShell;
                    let _ = Systemd::new(&sh, &service_user()).restart();
                }
                return Action::Back;
            }
        }
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, foot] = Layout::vertical([Constraint::Length(1), Constraint::Min(4), Constraint::Length(2)]).areas(area);
        let visible = list.height as usize;
        let scroll = if self.row >= visible { self.row + 1 - visible } else { 0 };
        let _ = self.scroll;
        let mut lines = Vec::new();
        for (i, (field, label, kind)) in FIELDS.iter().enumerate().skip(scroll).take(visible) {
            let selected = i == self.row;
            let raw = get(&self.cfg, *field);
            let shown = match (&self.mode, selected, kind) {
                (Mode::Edit(buf), true, FieldKind::Secret) => format!("{}_", "*".repeat(buf.chars().count())),
                (Mode::Edit(buf), true, _) => format!("{buf}_"),
                (_, _, FieldKind::Secret) => "*".repeat(raw.chars().count()),
                (_, _, FieldKind::Bool) => if raw == "true" { format!("{} on", g.done) } else { format!("{} off", g.pending) },
                _ => raw,
            };
            let pointer = if selected { g.pointer } else { " " };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{pointer} "), th.selected()),
                Span::styled(format!("{label:<24}"), if selected { th.selected() } else { th.normal() }),
                Span::styled(shown, if matches!(self.mode, Mode::Edit(_)) && selected { th.title() } else { th.muted() }),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);
        let msg = match (&self.error, self.dirty) {
            (Some(e), _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (None, true) => Line::from(Span::styled("  unsaved changes: s to save", th.warning())),
            (None, false) => Line::from(Span::styled(format!("  {}", shared.ctx.config_path.display()), th.faint_style())),
        };
        f.render_widget(Paragraph::new(msg), foot);
        if matches!(self.mode, Mode::AskRestart) {
            confirm_dialog(f, area, th, "Restart service?", &["Apply the new config to the running service now?".to_string()], "y/⏎ restart   Esc later", false);
        }
    }

    fn keys(&self) -> String {
        match self.mode {
            Mode::Browse => "↑↓ move  ⏎ edit/toggle  s save  Esc back".into(),
            Mode::Edit(_) => "type  ⏎ apply  Esc cancel".into(),
            Mode::AskRestart => "y restart  Esc later".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Configure".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_round_trip_and_validation() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::PromPort), "9090");
        set(&mut c, Field::PromPort, " 9091 ").unwrap();
        assert_eq!(c.prometheus.port, 9091);
        assert!(set(&mut c, Field::PromPort, "x").unwrap_err().contains("not a number"));
        assert!(set(&mut c, Field::NightStart, "25:00").is_err());
        set(&mut c, Field::NightStart, "22:30").unwrap();
        assert_eq!(c.night.start, "22:30");
        assert!(set(&mut c, Field::Brightness, "1.5").is_err());
        set(&mut c, Field::Brightness, "0.7").unwrap();
        set(&mut c, Field::QbitEnabled, "false").unwrap();
        assert!(!c.qbittorrent.enabled);
        set(&mut c, Field::Fps, "500").unwrap();
        assert_eq!(c.display.fps, 60);
        for (f, _, _) in FIELDS {
            let v = get(&c, f);
            assert!(set(&mut c, f, &v).is_ok(), "{f:?} round trip with {v:?}");
        }
    }
}
```

- [ ] **Step 2: Route, test, commit**

`screens/mod.rs`: add `pub mod configure;` and `ScreenId::Configure => Box::new(configure::Configure::new(shared)),`.

Run: `cargo test -p rackscreen-setup`, clippy. Desktop: `cargo run -- setup --sim --config /tmp/rs.yaml` → Configure → edit the prometheus port, toggle qbittorrent, `s` saves to `/tmp/rs.yaml`.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): configure form"
```

---

### Task 9: Status screen

**Files:**
- Create: `crates/setup/src/screens/status.rs`
- Modify: `crates/setup/src/screens/mod.rs`

**Interfaces:**
- Consumes: `ops::systemd::{Systemd, ServiceInfo, links_from_logs, LinkDots, Dot}`, `ops::boot::{readiness, Readiness}`, `ops::paths::Paths`.
- Produces: `screens::status::{Snapshot, collect(sh, paths, user) -> Snapshot, Status::new(shared)}`.

- [ ] **Step 1: status.rs**

```rust
//! Status: service state, boot readiness, link dots and a log tail, refreshed every 2 s.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::boot::{readiness, Readiness};
use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::{RealShell, Shell};
use crate::ops::systemd::{links_from_logs, Dot, LinkDots, ServiceInfo, Systemd};
use crate::{Action, Screen, Shared};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub info: ServiceInfo,
    pub enabled: bool,
    pub binary_present: bool,
    pub config_present: bool,
    pub ready: Option<Readiness>,
    pub logs: Vec<String>,
    pub links: LinkDots,
}

pub fn collect(sh: &dyn Shell, paths: &Paths, user: &str) -> Snapshot {
    let sd = Systemd::new(sh, user);
    let info = sd.info().unwrap_or_default();
    let enabled = sd.is_enabled().unwrap_or(false);
    let logs = sd.journal_tail(60).unwrap_or_default();
    let ready = match (paths.config_txt(), paths.cmdline_txt()) {
        (Some(c), Some(l)) => Some(readiness(&std::fs::read_to_string(c).unwrap_or_default(), &std::fs::read_to_string(l).unwrap_or_default())),
        _ => None,
    };
    Snapshot {
        links: links_from_logs(&logs),
        info,
        enabled,
        binary_present: paths.binary().exists(),
        config_present: paths.config().exists(),
        ready,
        logs,
    }
}

pub struct Status {
    snap: Option<Snapshot>,
    rx: Receiver<Snapshot>,
    log_view: bool,
    scroll: usize,
    restart_note: Option<String>,
}

impl Status {
    pub fn new(_shared: &Shared) -> Status {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("status".into())
            .spawn(move || {
                let sh = RealShell;
                let paths = Paths::system();
                let user = service_user();
                loop {
                    if tx.send(collect(&sh, &paths, &user)).is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            })
            .expect("spawn status");
        Status { snap: None, rx, log_view: false, scroll: 0, restart_note: None }
    }

    pub fn with_snapshot(snap: Snapshot) -> Status {
        let (_tx, rx) = mpsc::channel();
        Status { snap: Some(snap), rx, log_view: false, scroll: 0, restart_note: None }
    }
}

fn dot_style(d: Dot, th: &crate::theme::Theme) -> Style {
    match d {
        Dot::Up => th.good(),
        Dot::Down => th.bad(),
        Dot::Unknown => th.muted(),
    }
}

fn fmt_uptime(secs: u64) -> String {
    if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

impl Screen for Status {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.log_view {
                    self.log_view = false;
                    Action::None
                } else {
                    Action::Back
                }
            }
            KeyCode::Char('l') => {
                self.log_view = !self.log_view;
                self.scroll = 0;
                Action::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_add(1);
                Action::None
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('r') => {
                let sh = RealShell;
                self.restart_note = Some(match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "service restarted".into(),
                    Err(e) => format!("restart failed: {e:#}"),
                });
                let _ = &shared;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        while let Ok(s) = self.rx.try_recv() {
            shared.service_active = Some(s.info.active == "active");
            self.snap = Some(s);
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let Some(s) = &self.snap else {
            f.render_widget(Paragraph::new(Line::from(Span::styled("  collecting...", th.muted()))), area);
            return;
        };
        if self.log_view {
            let lines: Vec<Line> = s.logs.iter().rev().skip(self.scroll).take(area.height as usize).collect::<Vec<_>>().into_iter().rev().map(|l| Line::from(Span::styled(l.clone(), th.normal()))).collect();
            f.render_widget(Paragraph::new(lines), area);
            return;
        }
        let [_, top, logs] = Layout::vertical([Constraint::Length(1), Constraint::Length(9), Constraint::Min(3)]).areas(area);
        let (svc_style, svc_text) = match s.info.active.as_str() {
            "active" => (th.good(), format!("active ({})", s.info.sub)),
            "failed" => (th.bad(), "failed".to_string()),
            other => (th.warning(), if other.is_empty() { "not installed".into() } else { other.to_string() }),
        };
        let mut lines = vec![
            Line::from(vec![Span::raw("  "), Span::styled(g.dot, svc_style), Span::styled(format!(" service {svc_text}"), th.normal()), Span::styled(s.info.uptime_secs.map(|u| format!("   up {}", fmt_uptime(u))).unwrap_or_default(), th.muted())]),
            Line::from(vec![Span::raw("  "), Span::styled(g.dot, if s.enabled { th.good() } else { th.muted() }), Span::styled(if s.enabled { " starts at boot" } else { " not enabled at boot" }, th.normal())]),
            Line::from(vec![Span::raw("  "), Span::styled(g.dot, if s.binary_present { th.good() } else { th.bad() }), Span::styled(" /usr/local/bin/rackscreen", th.normal()), Span::raw("   "), Span::styled(g.dot, if s.config_present { th.good() } else { th.bad() }), Span::styled(" /etc/rackscreen/config.yaml", th.normal())]),
        ];
        match s.ready {
            Some(r) => lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(g.dot, if r.spi_on { th.good() } else { th.bad() }),
                Span::styled(" spi on   ", th.normal()),
                Span::styled(g.dot, if r.spi1_overlay { th.good() } else { th.bad() }),
                Span::styled(" spi1 overlay   ", th.normal()),
                Span::styled(g.dot, if r.bufsiz { th.good() } else { th.warning() }),
                Span::styled(" spidev.bufsiz", th.normal()),
            ])),
            None => lines.push(Line::from(Span::styled("  boot files: not a Raspberry Pi", th.muted()))),
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(g.dot, dot_style(s.links.api, th)),
            Span::styled(" kubernetes   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.prometheus, th)),
            Span::styled(" prometheus   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.qbittorrent, th)),
            Span::styled(" qbittorrent", th.normal()),
        ]));
        if let Some(n) = &self.restart_note {
            lines.push(Line::from(Span::styled(format!("  {n}"), th.warning())));
        }
        f.render_widget(Paragraph::new(lines), top);
        let tail: Vec<Line> = s.logs.iter().rev().take(logs.height as usize).collect::<Vec<_>>().into_iter().rev().map(|l| Line::from(Span::styled(format!("  {l}"), th.faint_style()))).collect();
        f.render_widget(Paragraph::new(tail), logs);
    }

    fn keys(&self) -> String {
        if self.log_view { "↑↓ scroll  Esc back".into() } else { "r restart  l logs  Esc back".into() }
    }
    fn subtitle(&self) -> String {
        "Status".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn collect_uses_shell_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
        std::fs::write(dir.path().join("boot/firmware/config.txt"), "dtparam=spi=on\n").unwrap();
        std::fs::write(dir.path().join("boot/firmware/cmdline.txt"), "root=x\n").unwrap();
        let sh = FakeShell::new();
        sh.respond("systemctl show", Output::ok("ActiveState=active\nSubState=running\nActiveEnterTimestampMonotonic=1\n"));
        sh.respond("journalctl", Output::ok("prometheus: forwarding to monitoring/p\n"));
        let s = collect(&sh, &paths, "pi");
        assert_eq!(s.info.active, "active");
        assert_eq!(s.links.prometheus, Dot::Up);
        assert_eq!(s.ready.unwrap().spi_on, true);
        assert!(!s.binary_present);
    }

    #[test]
    fn renders_snapshot() {
        let snap = Snapshot {
            info: ServiceInfo { active: "active".into(), sub: "running".into(), uptime_secs: Some(4000) },
            enabled: true,
            binary_present: true,
            config_present: true,
            ready: Some(Readiness { spi_on: true, spi1_overlay: false, bufsiz: true }),
            logs: vec!["line one".into(), "line two".into()],
            links: LinkDots { api: Dot::Up, prometheus: Dot::Down, qbittorrent: Dot::Unknown },
        };
        let sh = Shared { ctx: Ctx { config_path: "/etc/rackscreen/config.yaml".into(), sim: false, version: "0.2.0" }, theme: Theme::new(true), service_active: None, banner: None };
        let screen = Status::with_snapshot(snap);
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("service active (running)"));
        assert!(text.contains("up 1h 06m"));
        assert!(text.contains("spi1 overlay"));
        assert!(text.contains("line two"));
        assert_eq!(fmt_uptime(90_000), "1d 1h");
    }
}
```

- [ ] **Step 2: Route, test, commit**

`screens/mod.rs`: add `pub mod status;` and `ScreenId::Status => Box::new(status::Status::new(shared)),`.

Run: `cargo test -p rackscreen-setup`, clippy.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): status screen"
```

---

### Task 10: Run here (in-TUI monitor with captured logs)

**Files:**
- Create: `crates/app/src/logs.rs`, `crates/setup/src/screens/run.rs`
- Modify: `crates/app/src/lib.rs`, `crates/setup/src/lib.rs` (install the log sink subscriber), `crates/setup/src/screens/mod.rs`
- Delete: `crates/setup/src/screens/placeholder.rs` (every screen now exists)

**Interfaces:**
- Produces: `rackscreen_app::logs::{LogSink::new(capacity) -> LogSink (Clone), .lines() -> Vec<String>, .make_writer()}` implementing `tracing_subscriber::fmt::MakeWriter`; `rackscreen_app::logs::install_sink_subscriber(sink: &LogSink)` (sets the global subscriber once; ignores the error if one is already set).
- Consumes: `rackscreen_app::run::{Monitor, RunOptions}`.

- [ ] **Step 1: crates/app/src/logs.rs**

```rust
//! An in-memory ring of log lines that tracing can write into (for the TUI's Run screen).

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
pub struct LogSink {
    inner: Arc<Mutex<VecDeque<String>>>,
    capacity: usize,
    partial: Arc<Mutex<String>>,
}

impl LogSink {
    pub fn new(capacity: usize) -> LogSink {
        LogSink { inner: Arc::new(Mutex::new(VecDeque::new())), capacity, partial: Arc::new(Mutex::new(String::new())) }
    }

    pub fn lines(&self) -> Vec<String> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    fn push_bytes(&self, buf: &[u8]) {
        let mut partial = self.partial.lock().unwrap();
        partial.push_str(&String::from_utf8_lossy(buf));
        let mut lines = self.inner.lock().unwrap();
        while let Some(pos) = partial.find('\n') {
            let line: String = partial.drain(..=pos).collect();
            let line = strip_ansi(line.trim_end_matches('\n'));
            if !line.is_empty() {
                if lines.len() >= self.capacity {
                    lines.pop_front();
                }
                lines.push_back(line);
            }
        }
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub struct SinkWriter(LogSink);

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.push_bytes(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogSink {
    type Writer = SinkWriter;
    fn make_writer(&'a self) -> SinkWriter {
        SinkWriter(self.clone())
    }
}

/// Route all tracing output into the sink (no stderr). Safe to call more than once.
pub fn install_sink_subscriber(sink: &LogSink) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(sink.clone()).try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_lines_and_caps() {
        let sink = LogSink::new(3);
        let mut w = sink.make_writer();
        w.write_all(b"one\ntw").unwrap();
        w.write_all(b"o\n\x1b[32mthree\x1b[0m\nfour\n").unwrap();
        assert_eq!(sink.lines(), vec!["two", "three", "four"]);
        sink.clear();
        assert!(sink.lines().is_empty());
    }

    #[test]
    fn tracing_goes_into_sink() {
        let sink = LogSink::new(10);
        let sub = tracing_subscriber::fmt().with_ansi(false).with_writer(sink.clone()).finish();
        tracing::subscriber::with_default(sub, || {
            tracing::info!("hello sink");
        });
        assert!(sink.lines().iter().any(|l| l.contains("hello sink")));
    }
}
```

`crates/app/src/lib.rs`: add `pub mod logs;`.

- [ ] **Step 2: Install the sink at TUI start**

In `crates/setup/src/lib.rs`, add `pub log_sink: rackscreen_app::logs::LogSink` to `Shared`, create it in `App::new` with capacity 500, and in `run()` call `rackscreen_app::logs::install_sink_subscriber(&sink)` before `ratatui::init()` (create the sink first, then pass it into `App::new`). Update every `Shared { .. }` literal in tests to include `log_sink: LogSink::new(10)`.

- [ ] **Step 3: screens/run.rs**

```rust
//! Run here: the monitor in-process, logs streaming into the TUI, q stops it cleanly.

use rackscreen_app::config::Config;
use rackscreen_app::run::{Monitor, RunOptions};
use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::{Action, Screen, Shared};

pub struct RunHere {
    monitor: Option<Monitor>,
    error: Option<String>,
    service_was_active: bool,
    stopping: bool,
}

impl RunHere {
    pub fn new(shared: &Shared) -> RunHere {
        shared.log_sink.clear();
        let sh = RealShell;
        let sd = Systemd::new(&sh, &service_user());
        let service_was_active = !shared.ctx.sim && sd.is_active().unwrap_or(false);
        if service_was_active {
            tracing::info!("stopping the service while running in the foreground");
            let _ = sd.stop();
        }
        let cfg = match Config::load_or_default(&shared.ctx.config_path) {
            Ok(c) => c,
            Err(e) => return RunHere { monitor: None, error: Some(format!("{e:#}")), service_was_active, stopping: false },
        };
        let opts = RunOptions { sim: shared.ctx.sim, ..RunOptions::default() };
        match Monitor::start(&cfg, opts) {
            Ok(m) => RunHere { monitor: Some(m), error: None, service_was_active, stopping: false },
            Err(e) => RunHere { monitor: None, error: Some(format!("{e:#}")), service_was_active, stopping: false },
        }
    }

    fn finish(&mut self) {
        if let Some(m) = self.monitor.take() {
            m.shutdown();
        }
        if self.service_was_active {
            let sh = RealShell;
            let _ = Systemd::new(&sh, &service_user()).start();
            self.service_was_active = false;
        }
    }
}

impl Drop for RunHere {
    fn drop(&mut self) {
        self.finish();
    }
}

impl Screen for RunHere {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.stopping = true;
                if let Some(m) = &self.monitor {
                    m.stop();
                }
                self.finish();
                Action::Back
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, _shared: &mut Shared, _now: Secs) {
        if let Some(m) = &self.monitor {
            if m.is_stopped() && !self.stopping {
                self.error = Some("monitor stopped on its own (see log)".into());
                self.finish();
            }
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let [head, logs] = Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(area);
        let status = match (&self.error, self.monitor.is_some()) {
            (Some(e), _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (None, true) => Line::from(Span::styled("  running in the foreground, q to stop", th.good())),
            (None, false) => Line::from(Span::styled("  stopped", th.muted())),
        };
        f.render_widget(Paragraph::new(status), head);
        let lines = shared.log_sink.lines();
        let tail: Vec<Line> = lines.iter().rev().take(logs.height as usize).collect::<Vec<_>>().into_iter().rev().map(|l| {
            let style = if l.contains("WARN") || l.contains("ERROR") { th.warning() } else { th.faint_style() };
            Line::from(Span::styled(format!("  {l}"), style))
        }).collect();
        f.render_widget(Paragraph::new(tail), logs);
    }

    fn keys(&self) -> String {
        "q stop and back".into()
    }
    fn subtitle(&self) -> String {
        "Run here".into()
    }
    fn animating(&self, _now: Secs) -> bool {
        self.monitor.is_some()
    }
}
```

- [ ] **Step 4: Route, delete the placeholder, test, commit**

`screens/mod.rs` final form:
```rust
pub mod calibrate;
pub mod configure;
pub mod install;
pub mod menu;
pub mod run;
pub mod status;
pub mod uninstall;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        ScreenId::Install => Box::new(install::Install::new(shared)),
        ScreenId::Calibrate => Box::new(calibrate::Calibrate::new(shared)),
        ScreenId::Configure => Box::new(configure::Configure::new(shared)),
        ScreenId::Status => Box::new(status::Status::new(shared)),
        ScreenId::RunHere => Box::new(run::RunHere::new(shared)),
        ScreenId::Uninstall => Box::new(uninstall::Uninstall::new(shared)),
    }
}
```
Delete `screens/placeholder.rs`.

Run: `cargo test --workspace --features sim,pi`, clippy. Desktop: `cargo run -- setup --sim --config /tmp/rs.yaml` → Run here: the simulator window opens, log lines stream in the TUI (`running (4 screens ...)`), `q` closes the window and returns to the menu with the terminal intact.

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): run-here screen with in-TUI logs"
```

---

### Task 11: Root re-exec, install.sh, release workflow, README, version 0.2.0

**Files:**
- Create: `install.sh`
- Delete: `deploy/rackscreen.service`, `deploy/install.sh`
- Modify: `src/main.rs`, `.github/workflows/release.yml`, `README.md`, `Cargo.toml` (workspace version `0.2.0`)

- [ ] **Step 1: sudo re-exec in main.rs**

Add to `src/main.rs`:
```rust
/// Setup and calibrate change system files; re-run ourselves under sudo when not root.
/// `--sim` runs (desktop testing) stay unprivileged.
fn ensure_root(sim: bool) -> Result<()> {
    if sim || nix::unistd::geteuid().is_root() {
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    eprintln!("rackscreen setup needs root; re-running with sudo");
    let status = std::process::Command::new("sudo").arg(exe).args(&args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}
```
Call `ensure_root(sim)?;` at the top of the `setup()` helper (before building `Ctx`). Add `nix = { version = "0.31", features = ["user"] }` to the root `[dependencies]`.

- [ ] **Step 2: install.sh (repo root)**

```bash
#!/usr/bin/env bash
# RackScreen installer: downloads the latest release binary and opens the setup TUI.
#   curl -fsSL https://github.com/silkepilon/RackScreen/releases/latest/download/install.sh | bash
set -euo pipefail
REPO="silkepilon/RackScreen"
ARCH="$(uname -m)"
if [ "$ARCH" != "aarch64" ]; then
  echo "RackScreen needs a 64-bit Raspberry Pi OS (aarch64); this machine is $ARCH" >&2
  exit 1
fi
URL="https://github.com/$REPO/releases/latest/download/rackscreen-aarch64"
TMP="$(mktemp -d)"
echo "downloading $URL"
curl -fsSL "$URL" -o "$TMP/rackscreen"
chmod +x "$TMP/rackscreen"
exec sudo "$TMP/rackscreen" setup < /dev/tty
```
`chmod +x install.sh`. Delete the `deploy/` directory.

- [ ] **Step 3: release.yml**

Change the last step to upload both files:
```yaml
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            rackscreen-aarch64
            install.sh
```

- [ ] **Step 4: README**

Replace the "Pi setup" and "Develop on the desktop" sections:

```markdown
## Install on the Pi

One line, no clone:

    curl -fsSL https://github.com/silkepilon/RackScreen/releases/latest/download/install.sh | bash

It downloads the latest release and opens the setup menu. **Install** copies the binary to `/usr/local/bin`, writes `/etc/rackscreen/config.yaml`, installs the `rackscreen@<user>` service and offers to enable SPI in the boot files (reboot afterwards). **Calibrate screens** shows a test pattern on the panels; press `r`/`f` per screen until the arrow points up and the dot is top-right, then `s`. **Configure** edits the config in a form. **Status** shows the service and link states with logs. **Uninstall** removes everything except the boot file lines.

Later runs: just type `rackscreen` (it asks for sudo). The service runs `rackscreen run --config /etc/rackscreen/config.yaml`.

Config is YAML (`/etc/rackscreen/config.yaml`, defaults in `config.example.yaml`). Kubeconfig defaults to `~/k8s-monitor.yaml` of the service user.

## Develop on the desktop

    cargo run -- run --sim                       # fake data, keyboard drives events
    cargo run -- run --sim --source k8s          # real cluster through the config's kubeconfig
    cargo run -- setup --sim --config /tmp/rs.yaml   # the TUI against a scratch config
    cargo run -- calibrate --sim --config /tmp/rs.yaml
```
Keep the simulator keys paragraph, tests paragraph, wiring table, "Build for the Pi yourself" and licences. Drop every mention of `deploy/` and TOML.

- [ ] **Step 5: Version bump, full verification, commit**

Set `version = "0.2.0"` in `[workspace.package]`. Run the three clippy builds, `cargo test --workspace --features sim,pi`, `cargo build --no-default-features --features pi`, `bash -n install.sh`. Desktop end-to-end: `cargo run` (no args, in a terminal) must print the sudo notice and re-exec; `cargo run -- setup --sim --config /tmp/rs.yaml` must open the menu without sudo.

```bash
cargo fmt --all
git add -A
git commit -m "feat: sudo re-exec, one-line installer, README for the setup TUI; v0.2.0"
```

---

## Handoff to hardware

Push, tag `v0.2.0`, wait for the release, then on the Pi run the one-liner. Install, reboot if asked, run `rackscreen` again, Calibrate until all four arrows point up, Status should show three green dots after a minute.

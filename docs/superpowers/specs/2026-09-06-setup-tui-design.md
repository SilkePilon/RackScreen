# RackScreen setup TUI, YAML config and screen calibration

Date: 2026-09-06
Status: approved by Silke in brainstorming session
Builds on: `2026-09-05-rackscreen-design.md` (v0.1.0, running on the Pi)

## Purpose

Three user-facing problems after the first Pi deployment:

1. Screen content is rotated (landscape) and screens 1 to 3 are mirrored. Orientation must be fixable on the device without guessing config values.
2. Config should be YAML, not TOML.
3. Setup must not require cloning the repo. The binary itself provides an interactive installer, uninstaller and tools, with a clean modern terminal UI with animations.

## Scope

- New crate `crates/setup`: a ratatui + crossterm TUI with six screens: Install, Calibrate screens, Configure, Status, Run here, Uninstall.
- CLI restructured into subcommands.
- YAML config at `/etc/rackscreen/config.yaml`; TOML dropped.
- Calibration mode that drives the real panels (or the simulator) with a test pattern and writes rotate/hflip to config.
- `install.sh` one-liner attached to releases.

Out of scope (later): self-update from GitHub releases, editing the ignore-alerts list in the form, touch input.

## CLI

- `rackscreen` with no arguments and stdin a TTY: opens the TUI. Not a TTY: prints help and exits 2.
- `rackscreen setup`: opens the TUI.
- `rackscreen calibrate [--sim]`: opens the TUI directly on the Calibrate screen.
- `rackscreen run [--sim] [--sim-grid] [--source fake|k8s] [--config PATH] [--fps N] [--seed N]`: the monitor. This is what systemd runs. All previous top-level flags move here unchanged.
- `--config` default everywhere: `/etc/rackscreen/config.yaml`.

## Privileges

Install, Uninstall, Calibrate, Configure and boot-file edits need root. On TUI start, if `geteuid() != 0` and the requested action needs root, the process re-executes itself as `sudo <same argv>` before initialising the terminal UI, so the sudo password prompt appears normally. The service user is `SUDO_USER`; if unset, the current user; if that is root, `pi`. Status and Run-here work without root when possible (journalctl may need the `systemd-journal` group; errors are shown, not fatal).

## Config (YAML)

Same fields and defaults as the current TOML, expressed in YAML. Loaded with `serde_yaml`. Embedded default is `config.example.yaml`. `config.example.toml`, the TOML dependency and any TOML tests are removed. Validation unchanged (roles, rotate in {0, 90, 180, 270}, at least one screen). New key `prometheus.ignore_alerts` already exists; kept.

```yaml
k8s:
  kubeconfig: ~/k8s-monitor.yaml
prometheus:
  namespace: monitoring
  service: auto
  port: 9090
  poll_secs: 5
  ignore_alerts: [Watchdog, InfoInhibitor]
qbittorrent:
  enabled: true
  namespace: arr-stack
  service: qbittorrent
  port: 8080
  user: ""
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
  spi_chunk: 4096
screens:
  - { role: cpu,    spi: 0, cs: 0, dc: 6,  rst: 5,  rotate: 270, hflip: false, hz: 40000000 }
  - { role: mem,    spi: 0, cs: 1, dc: 13, rst: 26, rotate: 270, hflip: true,  hz: 40000000 }
  - { role: pods,   spi: 1, cs: 0, dc: 23, rst: 22, rotate: 270, hflip: true,  hz: 16000000 }
  - { role: health, spi: 1, cs: 1, dc: 4,  rst: 27, rotate: 270, hflip: true,  hz: 16000000 }
```

`~` in `kubeconfig` expands to the service user's home, not root's, when the service runs; the `run` subcommand expands against `$HOME` of the process, which under systemd is the instance user.

## Crate layout

```
crates/setup/src/
  lib.rs            pub fn run(start: Start, ctx: Ctx) -> Result<()>; App state machine; 60 Hz loop
  theme.rs          palette (same accents as the panels), styles, glyphs with ASCII fallback
  anim.rs           Ease (reuse core::anim), Slide, Pulse, Spinner, ProgressBar
  widgets.rs        header, footer key bar, step list, confirm dialog, mini panel
  screens/menu.rs
  screens/install.rs
  screens/calibrate.rs
  screens/configure.rs
  screens/status.rs
  screens/run.rs
  screens/uninstall.rs
  ops/shell.rs      trait Shell { run(cmd, args) -> Output }; RealShell; FakeShell for tests
  ops/paths.rs      binary, config, unit, boot file paths; service user detection
  ops/install.rs    step planner + executor (pure plan, side effects via Shell + fs trait)
  ops/uninstall.rs
  ops/systemd.rs    unit text, enable/disable/stop/restart, is-active, journal tail
  ops/boot.rs       ensure_line for config.txt / cmdline.txt (pure text functions)
  ops/config_file.rs YAML load/save/default, field edit helpers
```

Root binary: `src/main.rs` becomes subcommand dispatch; the monitor wiring moves to `src/run.rs`; `src/config.rs` switches to YAML.

Dependencies added: `ratatui`, `crossterm`, `serde_yaml`, `nix` (geteuid) or `libc`, `sha2` (binary hash compare). Removed: `toml`.

## Visual language

Dark, matches the screens. Palette from `rackscreen_core::theme`: amber (primary), violet, blue, green, red, greys.

```
┌ RackScreen ───────────────────────────── v0.2.0 ┐
│                                                  │
│   ◐  Kubernetes rack monitor                     │
│                                                  │
│   ▸ Install            set up service + config   │
│     Calibrate screens  fix rotation / mirroring  │
│     Configure          cluster, night, display   │
│     Status             service, links, logs      │
│     Run here           foreground with logs      │
│     Uninstall          remove everything         │
│                                                  │
│   ● service: active     ● config: /etc/rackscreen│
│                                                  │
└ ↑↓ move  ⏎ select  q quit ────────────────────────┘
```

- Header: title left, version right. The `◐` glyph cycles `◐ ◓ ◑ ◒` on a 2.4 s loop in amber (ring breathing echo).
- Menu: selection bar slides between rows with a 150 ms ease-out; selected row amber with `▸`; descriptions dim unless selected. Footer shows the service state and config path with green/red/grey dots.
- Screen transitions: the incoming screen slides in from the right over 200 ms; leaving slides out. `Esc` goes back to the menu.
- Step lists (Install, Uninstall): rows `○ pending`, braille spinner while running, `✓` green done, `✗` red failed with the error text beneath. A progress bar at the bottom eases toward `done/total`. Destructive or system-changing steps open a centred confirm dialog with a red border listing exactly what will change.
- Bottom key bar is contextual per screen.
- Glyph fallback: when the locale is not UTF-8, use `>`, `-`, `*`, `OK`, `X`, `|/-\`.
- 60 Hz redraw while an animation is active, otherwise 10 Hz idle to keep the Pi cool.

## Screens

### Install

Steps, in order, each visible as a row:

1. Platform check: Linux aarch64, `/boot/firmware` exists, `spi` group exists. Any miss is a warning row (yellow `!`), not a failure, so desktop test runs work.
2. Install binary: copy the running executable to `/usr/local/bin/rackscreen` (mode 755). Skipped with "already up to date" when SHA-256 matches.
3. Config: write `/etc/rackscreen/config.yaml` from the embedded default if missing; if present, keep and show "kept existing".
4. Groups: `usermod -aG spi,gpio <user>` (skip if groups missing, warn).
5. Service: write `/etc/systemd/system/rackscreen@.service`, `systemctl daemon-reload`, `systemctl enable --now rackscreen@<user>`.
6. Boot files: read `/boot/firmware/config.txt` and `/boot/firmware/cmdline.txt` (fallback `/boot/`). Compute missing lines: `dtparam=spi=on`, `dtoverlay=spi1-2cs`, and the `spidev.bufsiz=65536` token. If any missing, confirm dialog listing them; on yes, apply, set `display.spi_chunk: 65536` in the config, and show a persistent "reboot required" banner; on no, skip with a warning row.

After the last step: prompt "Calibrate screens now?" (Enter) or back to menu (Esc).

Unit file:

```ini
[Unit]
Description=RackScreen Kubernetes rack monitor
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=%i
ExecStart=/usr/local/bin/rackscreen run --config /etc/rackscreen/config.yaml
Restart=always
RestartSec=3
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

### Uninstall

Confirm dialog first (lists: stop service, remove unit, remove `/etc/rackscreen`, remove binary; notes that boot file lines are left in place). Steps: `systemctl disable --now rackscreen@<user>`, remove unit file, `daemon-reload`, remove `/etc/rackscreen`, remove `/usr/local/bin/rackscreen`. If the running executable is the one being removed, deletion still works on Linux (unlink of a running binary); the TUI shows a final "done, this process will exit" message.

### Calibrate screens

Purpose: fix rotation and mirroring on the device by eye.

- On entry, if the service is active, stop it (remember to restart on exit). Open the four panels with the display crate using the current config (or the simulator when `--sim`).
- Test pattern per screen, rendered with the render crate: black background, big white up-arrow (Lucide `arrow-up`, 120 px) centred, the screen number (1 to 4) in the badge below, a small amber dot at the top-right of the ring (so mirroring is visible), and the role name colour on the ring (all segments lit in the role accent).
- TUI left column: four mini panels drawn as ASCII circles with an `↑`, the number, and a `•` at the top-right, transformed by the same rotate/hflip so the terminal preview matches the physical expectation. Selected panel highlighted.
- Keys: `1` to `4` select screen, `r` rotate +90, `R` rotate -90, `f` toggle hflip, `a` copy the selected screen's orientation to all, `s` save and exit, `Esc` discard and exit.
- Each key press re-renders the affected panel through `Orient` immediately.
- Save writes the YAML (only the `rotate`/`hflip` fields change, other content preserved by loading, mutating and serialising the whole struct). On exit, restart the service if it was running.
- Lucide `arrow-up` is added to the embedded icon set.

### Configure

Form over the YAML fields: kubeconfig, prometheus namespace/service/port/poll_secs, qbittorrent enabled/namespace/service/port/user/pass/poll_secs, night enabled/start/end, thresholds, brightness, fps, spi_chunk. `↑↓`/`Tab` move, `Enter` edits inline (text, number, toggle), `s` saves (validates first; errors inline in red under the field), `Esc` cancels. Password field masked. Saving restarts the service if it is active (ask first).

### Status

Shows: service active/inactive/failed and uptime (`systemctl show`), enabled or not, binary path and version, config path, boot-file readiness (SPI on, spi1 overlay, bufsiz), and the last 20 journal lines (`journalctl -u rackscreen@<user> -n 20 --no-pager`) refreshed every 2 s. Link dots derived from the newest matching log lines: `prometheus: forwarding` (green) / `prometheus:` warn (red); same for qbittorrent and pod watch. Keys: `r` restart service, `l` open a scrolling log view, `Esc` back.

### Run here

Runs the monitor in-process (same code path as `rackscreen run`, config from disk) with `tracing` output captured into a scrolling pane inside the TUI. If the service is active it is stopped first (and restarted on exit) so the panels are not fought over. `q` stops cleanly (same shutdown path as SIGTERM).

## install.sh

Attached to each GitHub release next to `rackscreen-aarch64`:

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO="silkepilon/RackScreen"
ARCH="$(uname -m)"
[ "$ARCH" = "aarch64" ] || { echo "RackScreen needs a 64-bit Raspberry Pi OS (aarch64), got $ARCH"; exit 1; }
URL="https://github.com/$REPO/releases/latest/download/rackscreen-aarch64"
TMP="$(mktemp -d)"
curl -fsSL "$URL" -o "$TMP/rackscreen"
chmod +x "$TMP/rackscreen"
exec sudo "$TMP/rackscreen" setup
```

README one-liner: `curl -fsSL https://github.com/silkepilon/RackScreen/releases/latest/download/install.sh | bash`. The release workflow uploads `install.sh` together with the binary.

## Testing

- `ops/boot.rs`: pure `ensure_line(text, line) -> (String, bool)` and `ensure_cmdline_token(text, token) -> (String, bool)` tested on fixtures (empty file, already present, present with trailing spaces, CRLF).
- `ops/systemd.rs`: unit text snapshot; enable/disable/status through `FakeShell` recording calls and returning scripted outputs; journal parsing to link dots tested with sample log lines.
- `ops/install.rs`: planner produces the expected step list for states (fresh Pi, already installed, desktop without `/boot/firmware`); executor tested against `FakeShell` plus a temp dir for file writes.
- `ops/config_file.rs`: YAML default parses to the same values as today's TOML defaults; round-trip preserves fields; calibration mutation changes only rotate/hflip.
- TUI: ratatui `TestBackend` snapshot tests for menu, a step list mid-run, the confirm dialog, and the calibrate layout with a known orientation.
- Existing workspace tests keep passing; `run` subcommand behaviour unchanged.
- Manual: desktop `rackscreen setup` (install steps warn, calibrate with `--sim`), then on the Pi via the one-liner.

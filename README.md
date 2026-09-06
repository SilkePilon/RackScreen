# RackScreen

Animated Kubernetes monitor for four GC9A01 240x240 round displays on a Raspberry Pi 3B+ (64-bit). Written in Rust.

Four screens, top to bottom: **CPU**, **MEM**, **PODS**, **HEALTH**. Icon-first "Minimal Mono" look: black background, segmented ring, white Lucide icon, small outlined badge with the number. Screens are never static: values ease, the last segment breathes, icons have micro-loops. Cluster events splash on their screen (pod started, crashed, gone, hot node, torrent added). Big events sweep the whole rack (node down/up, alert firing/resolved, torrent done, link restored). HEALTH turns into a download monitor when the cluster is healthy and qBittorrent is pulling.

Design: `docs/superpowers/specs/2026-09-05-rackscreen-design.md`.

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

Simulator keys: `1` pod started, `2` pod crashed, `3` node down, `4` node up, `5` alert toggle, `6` torrent done, `7` link down, `8` link up, `t` torrent mode, `n` night cycle, `b` boot, `Esc` quit. `--sim-grid` shows 2x2.

Tests: `cargo test --workspace`. Golden images live in `crates/render/tests/goldens`; regenerate with `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden` and review the PNGs.

Wiring (BCM numbers, from the config):

| Screen | SPI | CS | DC | RST |
|---|---|---|---|---|
| CPU | 0 | 0 | 6 | 5 |
| MEM | 0 | 1 | 13 | 26 |
| PODS | 1 | 0 | 23 | 22 |
| HEALTH | 1 | 1 | 4 | 27 |

If a screen is rotated or mirrored, change `rotate` (0/90/180/270) and `hflip` for that screen in the config; **Calibrate screens** in the setup TUI writes those values for you.

## Build for the Pi yourself

    cargo install cross --version 0.2.5
    cross build --release --target aarch64-unknown-linux-gnu --no-default-features --features pi

Binary: `target/aarch64-unknown-linux-gnu/release/rackscreen`.

## Licences

Code MIT. Icons: Lucide (ISC), `assets/icons/LICENSE`. Font: JetBrains Mono (OFL), `assets/fonts/OFL.txt`.

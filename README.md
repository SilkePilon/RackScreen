# RackScreen

Animated Kubernetes monitor for four GC9A01 240x240 round displays on a Raspberry Pi 3B+ (64-bit). Written in Rust.

Four screens, top to bottom: **CPU**, **MEM**, **PODS**, **HEALTH**. Icon-first "Minimal Mono" look: black background, segmented ring, white Lucide icon, small outlined badge with the number. Screens are never static: values ease, the last segment breathes, icons have micro-loops. Cluster events splash on their screen (pod started, crashed, gone, hot node, torrent added). Big events sweep the whole rack (node down/up, alert firing/resolved, torrent done, link restored). HEALTH turns into a download monitor when the cluster is healthy and qBittorrent is pulling.

Design: `docs/superpowers/specs/2026-09-05-rackscreen-design.md`.

## Develop on the desktop

    cargo run -- --sim                 # fake data, keyboard drives events
    cargo run -- --sim --source k8s    # real cluster, simulator window

`--source k8s` reads the kubeconfig path from the config file (`[k8s] kubeconfig`, default `~/k8s-monitor.yaml`). Use `--config <file>` to point at an alternative config; without it the binary reads `~/.config/rackscreen/config.toml`.

Simulator keys: `1` pod started, `2` pod crashed, `3` node down, `4` node up, `5` alert toggle, `6` torrent done, `7` link down, `8` link up, `t` torrent mode, `n` night cycle, `b` boot, `Esc` quit. `--sim-grid` shows 2x2.

Tests: `cargo test --workspace`. Golden images live in `crates/render/tests/goldens`; regenerate with `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden` and review the PNGs.

## Pi setup

1. `raspi-config` -> Interface Options -> SPI on. In `/boot/firmware/config.txt` add `dtoverlay=spi1-2cs`. In `/boot/firmware/cmdline.txt` append `spidev.bufsiz=65536`, then set `spi_chunk = 65536` in the config. Reboot.
2. Copy your kubeconfig to `~/k8s-monitor.yaml`.
3. Download the `rackscreen-aarch64` binary from the latest GitHub release.
4. `git clone` this repo (for `deploy/` and `config.example.toml`) and run `deploy/install.sh ./rackscreen-aarch64`.
5. Edit `~/.config/rackscreen/config.toml` (namespaces, pins, night window), then `sudo systemctl restart rackscreen@$USER`.

Wiring (BCM numbers, from the config):

| Screen | SPI | CS | DC | RST |
|---|---|---|---|---|
| CPU | 0 | 0 | 6 | 5 |
| MEM | 0 | 1 | 13 | 26 |
| PODS | 1 | 0 | 23 | 22 |
| HEALTH | 1 | 1 | 4 | 27 |

If a screen is rotated or mirrored, change `rotate` (0/90/180/270) and `hflip` for that `[[screens]]` entry.

## Build for the Pi yourself

    cargo install cross --version 0.2.5
    cross build --release --target aarch64-unknown-linux-gnu --no-default-features --features pi

Binary: `target/aarch64-unknown-linux-gnu/release/rackscreen`.

## Licences

Code MIT. Icons: Lucide (ISC), `assets/icons/LICENSE`. Font: JetBrains Mono (OFL), `assets/fonts/OFL.txt`.

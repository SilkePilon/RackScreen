# RackScreen

Animated Kubernetes monitor for four GC9A01 240x240 round displays on a Raspberry Pi 3B+ (64-bit). Written in Rust.

Four screens, top to bottom: **CPU**, **MEM**, **PODS**, **HEALTH**. Icon-first "Minimal Mono" look: black background, segmented ring, white Lucide icon, small outlined badge with the number. Screens are never static: values ease, the last segment breathes, icons have micro-loops. Cluster events splash on their screen (pod started, crashed, gone, hot node, torrent added). Big events sweep the whole rack (node down/up, alert firing/resolved, torrent done, link restored). HEALTH turns into a download monitor when the cluster is healthy and qBittorrent is pulling.

Design: `docs/superpowers/specs/2026-09-05-rackscreen-design.md`.

## Install on the Pi

One line, no clone:

    curl -fsSL https://github.com/silkepilon/RackScreen/releases/latest/download/install.sh | bash

It downloads the latest release and opens the setup menu. **Install** copies the binary to `/usr/local/bin`, writes `/etc/rackscreen/config.yaml`, installs the `rackscreen@<user>` service and offers to enable SPI in the boot files (reboot afterwards). **Calibrate screens** shows a test pattern on the panels; press `r`/`f` per screen until the arrow points up and the dot is top-right, then `s`. **Configure** edits the config in a form. **Status** shows the service and link states with logs. **Uninstall** removes everything except the boot file lines.

Later runs: just type `rackscreen` (it asks for sudo). The service runs `rackscreen run --config /etc/rackscreen/config.yaml`.

Upgrading from v0.1: `~/.config/rackscreen/config.toml` is no longer read; re-enter your settings via Configure.

Config is YAML (`/etc/rackscreen/config.yaml`, defaults in `config.example.yaml`). Kubeconfig defaults to `~/k8s-monitor.yaml` of the service user.

## Screens and roles

Every screen shows one or more **roles** and cycles through them; the switch is an iris transition. Ten roles exist:

| Role | Shows |
|---|---|
| `cpu` | cluster CPU load |
| `mem` | cluster memory use |
| `pods` | running pods, with pod events |
| `health` | cluster health, or the download monitor when idle and qBittorrent pulls |
| `thermal` | the hottest node temperature (`hot node temp °C` marks the danger band) |
| `storage` | persistent volumes with their robustness |
| `power-mix` | the grid production mix of your zone, per source |
| `price` | today's day-ahead price per hour, current hour marked |
| `carbon` | grid carbon intensity in gCO2eq/kWh |
| `renewable` | renewable and fossil-free share |

**Screens** in the setup TUI edits them: `↑↓` pick a screen, `⏎` opens the role picker (`space` toggles a role, `K`/`J` reorder, `⏎` closes), `+`/`-` change the cycle interval in 5 s steps, `s` saves. Three presets fill all four screens at once: `c` cluster (`cpu`, `mem`, `pods`, `health`), `e` electricity (`power-mix`, `price`, `carbon`, `renewable`) and `m` mixed (each screen alternates a cluster role with an electricity one). A screen with a single role never cycles.

In the config each screen has a `roles` list and a `cycle_secs`; the old single `role` key is still read and upgraded on load.

## Electricity mode

The grid roles (`power-mix`, `carbon`, `renewable`) use [Electricity Maps](https://app.electricitymaps.com). Sign in there for a free personal API token, which is tied to one zone. In **Configure** set `electricity enabled` to on, `electricity zone` to your zone code (`NL`, `DE`, `FR`, ...) and paste the token into `electricity api token`. Polling is every `electricity poll secs` (300 by default, never faster than a minute) so the free quota lasts.

The `price` role is separate and has its own `price source`, cycled with `⏎`:

- `energyzero` — the default, no token, but Dutch prices only.
- `entsoe` — the European transparency platform; ask for a free API token by mail and fill in `entsoe token` plus `entsoe zone (EIC)`, the EIC code of your bidding zone (for example `10YNL----------L` for the Netherlands). Left empty, the zone is derived from the electricity zone when that country is known; without a token or a zone, prices stay off and the log says so.
- `none` — no price polling; the `price` role then shows no data.

`price incl. VAT` asks EnergyZero for prices with VAT and levies included; ENTSO-E always reports the raw exchange price. The price ring is bucketed into the Pi's own local hours, the same clock that marks the current hour, so set the system time zone once: `sudo timedatectl set-timezone Europe/Amsterdam`. **Status** shows a dot per link, including `electricity` and `prices`: green after a successful poll, red after failures, grey while nothing has been logged yet.

## Develop on the desktop

    cargo run -- run --sim                       # fake data, keyboard drives events
    cargo run -- run --sim --source k8s          # real cluster through the config's kubeconfig
    cargo run -- setup --sim --config /tmp/rs.yaml   # the TUI against a scratch config
    cargo run -- calibrate --sim --config /tmp/rs.yaml

Simulator keys: `1` pod started, `2` pod crashed, `3` node down, `4` node up, `5` alert toggle, `6` torrent done, `7` link down, `8` link up, `9` degrade volume, `0` heal volume, `h` hot node, `p` price outage, `t` torrent mode, `n` night cycle, `b` boot, `Esc` quit. `--sim-grid` shows 2x2.

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

The electricity source icons (`assets/icons/em-*.svg`) come from the Electricity Maps web app and stay under the GNU Affero General Public License v3.0; see `assets/icons/EM-LICENSE.md`. Grid data from Electricity Maps and day-ahead prices from EnergyZero or ENTSO-E belong to those services and are subject to their own terms.

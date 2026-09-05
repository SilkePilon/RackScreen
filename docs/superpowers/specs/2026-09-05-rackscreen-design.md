# RackScreen design

Date: 2026-09-05
Status: approved by Silke in brainstorming session

## Purpose

Rewrite the Python `k8s_monitor.py` Kubernetes rack monitor as a Rust project. Four GC9A01 round 240x240 displays, stacked in a rack, driven by a Raspberry Pi 3B+ (64-bit OS). Screens must never be static: smooth idle motion, icon-based UI, animated splashes for cluster events, rack-wide choreographed sweeps for major events.

## Constraints

- Hardware fixed: Pi 3B+, 64-bit Raspberry Pi OS. Target triple `aarch64-unknown-linux-gnu`.
- SPI bandwidth is the wall: full frame is 115 KB. SPI0 at 40 MHz shared by two screens gives roughly 20 fps each; SPI1 at 16 MHz gives roughly 8 fps each. Partial (dirty rect) updates are mandatory.
- Development happens on a Fedora desktop with a simulator window. This machine has cluster access. GitHub Actions builds release binaries only on tags to keep minutes low.
- Language: Rust, all of it.
- Icons over text. Lucide icon set. Text limited to digits inside a badge.

## Visual language: Minimal Mono

- Pure black background. White Lucide icon, upper centre. Segmented ring around the edge. One accent hue per screen. Red reserved for trouble on every screen.
- Ring: 60 segments at radius 102 px, segment length 12 px, stroke 4 px, round caps. Lit = accent, off = `#1c1c1c`.
- Badge under icon: 64x26 px, corner radius 7, fill `#0e0e0e`, 2 px accent stroke, white bold monospace digits 15 px (JetBrains Mono Bold, embedded).
- Accents: CPU amber `#ffb020`, MEM violet `#a78bfa`, PODS blue `#4f8dff`, HEALTH green `#3ddc97`. Red `#ff4d4d`.
- Global brightness curve in config because GC9A01 panels run dark.

### Screens

| Screen | Ring | Icon | Badge |
|---|---|---|---|
| CPU | cluster CPU % | `cpu` | `42%` |
| MEM | cluster MEM % | `memory-stick` | `67%` |
| PODS | segments = pods: lit blue running, pulsing dim blue pending, red failed | `box` | running count |
| HEALTH | full green when all nodes ready and no alerts; red segments per NotReady node | `heart-pulse` | node dots, one per node |
| HEALTH torrent mode | up to 3 concentric segmented rings (r 102, 86, 70), one per active download, outer = most complete | `download` | combined speed, flips to ETA every 5 s |

HEALTH switches to torrent mode when all nodes ready, no alerts, and at least one active non-stalled download.

### Global states

- Connecting (Prometheus or API link down): all screens grey, centred `plug-zap` icon breathing, comet of 4 grey segments chasing round the ring (2.4 s per lap). No badge.
- No data (link up, queries failing): dim full ring, centred `cloud-off` icon, single segment ripple every 4 s. No badge.
- Night mode: fade to black over 1 s, controllers put to sleep, no rendering. Wake plays boot animation.

### Idle motion (always running)

- Displayed values are smoothed (ease toward target over ~800 ms). Segments light one by one; digits count with easing. Nothing jumps.
- Last lit segment breathes (sine, 2.4 s).
- Icon micro-loops: CPU opacity pulse 3.5 s; MEM none; PODS box bob 3 px at 2.6 s; HEALTH heart beats once every 4 s (scale 1.0 to 1.12 to 1.0 in 300 ms). Torrent arrow drops on 1.6 s loop.
- Pending pod segments pulse at 1.2 s.
- No orbiting dots or scanners (rejected).

### Splash (screen-local), 2.5 s

1. 0 to 300 ms: ripple ring expands from centre 0 to 110 px, alpha 0.8 to 0; ring segments flash event colour in a wave, 10 ms stagger per segment.
2. 100 to 500 ms: role icon fades out, event icon pops in with spring overshoot.
3. 500 to 2000 ms: hold. Number retargets, segments ease.
4. 2000 to 2500 ms: event icon fades, role icon fades in.

Persistent marker (small red dot under badge) stays while the condition holds. One active splash per screen, next queued. Same event type arriving within 1 s collapses into one splash with a `+N` counter in the badge.

### Rack sweep (rack-wide), about 4 s

1. Origin screen ring wipes 360 degrees to event colour in 250 ms.
2. Wave travels one screen per 120 ms. Bad news sweeps up from HEALTH. Good news sweeps down from CPU.
3. All four show event icon, ring pulses in sync at 1.2 s, hold 2.5 s.
4. Reverse wipe back to role screens, staggered in the same direction.

Sweeps queue globally and never overlap. Local splashes are suppressed during a sweep and resume after.

### Event to animation map

| Event | Scope | Colour | Icon |
|---|---|---|---|
| PodStarted | local PODS | blue | `package-plus` |
| PodCrashed | local PODS | red | `package-x` |
| PodGone | local PODS | dim blue | `package-minus` |
| HotNode (cpu or mem over threshold) | local CPU or MEM | amber | `flame` |
| TorrentAdded | local HEALTH | blue | `download` |
| TorrentDone | rack, down | green | `circle-check` |
| NodeNotReady | rack, up | red | `server-off` |
| NodeReady | rack, down | green | `server` |
| AlertFiring | rack, up | red | `triangle-alert` |
| AlertResolved | rack, down | green | `shield-check` |
| LinkDown | all screens to Connecting state | grey | `plug-zap` |
| LinkUp | rack, down | green | `plug` |
| Boot | rack, down, rings wipe in one by one | each accent | role icon |

## Architecture

Cargo workspace, one binary `rackscreen`.

```
rackscreen/
  crates/
    core/      state model, events, tweens, animation queues, scenes. No I/O, no hardware.
    render/    software rasterizer: Frame, segment rings, icons, text, badges, dirty rects.
    display/   Display trait, gc9a01 backend (rppal), sim backend (minifb).
    sources/   Source trait, k8s (kube-rs), prometheus, qbittorrent, fake.
  src/main.rs  wiring, config, CLI, tokio runtime for sources, render thread.
  assets/      Lucide SVGs, JetBrains Mono Bold TTF, embedded with include_bytes.
  deploy/      systemd unit, install script.
  config.example.toml
```

Data flow, one direction:

```
sources --(Event channel)--> core::Model --(per tick)--> core::Scene --> render --> display
```

- Sources push `Event` values into an `mpsc` channel.
- `Model` folds events into cluster state and pushes splash and sweep requests into animation queues.
- `Scene` for each screen is a pure function of `(&Model, now)`: list of drawables plus a dirty rect hint.
- `render` rasterizes a Scene into a `Frame` (240x240 RGBA8) and computes the dirty rect against the previous frame.
- `display` converts the dirty rect to RGB565 (rotation and flip via precomputed index map) and pushes it.

Render loop: one OS thread at a fixed 30 Hz tick. Each tick drains the event channel with `try_recv`, advances the model, builds and renders four scenes, and hands each frame to that screen's display thread through a single-slot mailbox. Display threads (one per screen) push over SPI; if a display is still busy the slot is overwritten with the newer frame, so slow SPI1 screens never stall SPI0 screens.

`core` and `render` have no hardware dependencies and are identical in simulator and on the Pi.

## Sources

Common trait:

```rust
pub trait Source {
    fn run(self, tx: mpsc::Sender<Event>, shutdown: CancellationToken) -> JoinHandle<()>;
}
```

Event enum (core):

```rust
pub enum Event {
    Metrics { cpu_pct, mem_pct, mem_used_gb, mem_total_gb, hot_cpu: Option<(String, f32)>, hot_mem: Option<(String, f32)> },
    PodSnapshot { running, pending, failed, total },
    PodStarted { ns, name },
    PodCrashed { ns, name },
    PodGone { ns, name },
    NodeSnapshot { ready, total, names_not_ready: Vec<String> },
    NodeReady { name, ready: bool },
    AlertSnapshot { firing: Vec<String> },
    AlertChanged { name, firing: bool },
    Torrents(Vec<Torrent>),
    TorrentAdded { name },
    TorrentDone { name },
    Link { target: LinkTarget, up: bool },   // LinkTarget = K8sApi | Prometheus | QBittorrent
}
```

### k8s (kube-rs)

- Loads kubeconfig from configured path. One `Client`.
- `kube_runtime::watcher` on `Pod` (all namespaces) and `Node`, with reflector caches. Watcher handles reconnect with backoff.
- Diffs on each applied object produce events: Pending to Running gives `PodStarted`; transition to Failed, a container in CrashLoopBackOff, or restart count increase gives `PodCrashed`; deletion gives `PodGone`; node Ready condition flip gives `NodeReady`. The initial list sync produces snapshots only, no splashes.
- Pod and node counts come from the caches, not Prometheus.
- Watch failing for more than 10 s emits `Link { K8sApi, false }`; recovery emits `Link { K8sApi, true }`.

### prometheus

- In-process port-forward via `Api<Pod>::portforward` to the first ready pod behind the Prometheus service (service chosen by name, or by `service = "auto"` which picks the first service in the namespace whose name contains "prometheus"). HTTP over the forwarded stream with hyper. No kubectl binary.
- Polls every `poll_secs` (default 5): cluster CPU %, MEM %, memory used and total GB, per-node CPU and MEM (for hot node), firing alert names.
- Alert set diff emits `AlertChanged`. Node CPU or MEM over `thresholds.hot_cpu` or `hot_mem` (default 90) emits a HotNode splash via `Metrics.hot_*` fields; the model debounces to one splash per node per 5 minutes.
- Port-forward failure for two consecutive polls emits `Link { Prometheus, false }`; the port-forward is rebuilt with backoff.

### qbittorrent

- Same in-process port-forward to the qBittorrent pod. `auth/login` with configured user and pass (empty values rely on localhost bypass). Polls `torrents/info?filter=downloading` every 3 s.
- Active means state in `downloading`, `forcedDL`, `metaDL` and speed at least 500 B/s. Emits `Torrents` (state only, no splash), `TorrentAdded`, `TorrentDone` (progress reaches 100 or torrent leaves the downloading filter with progress 1.0).
- Disabled entirely when `qbittorrent.enabled = false`.

### fake

- Deterministic scripted generator for the simulator: metrics drift with noise, periodic pod churn, occasional torrent.
- Keyboard in simulator window fires events on demand: `1` PodStarted, `2` PodCrashed, `3` NodeNotReady, `4` NodeReady, `5` AlertFiring, `6` TorrentDone, `7` LinkDown, `8` LinkUp, `t` toggle torrent mode, `n` toggle night, `b` replay boot.

## Render engine

- `tiny-skia` for anti-aliased path fills and strokes. `fontdue` for glyph rasterizing. `usvg` parses Lucide SVGs once at startup into `tiny_skia::Path`.
- `Frame`: 240x240 RGBA8 `Pixmap`. Conversion to RGB565 big-endian happens only for the dirty rect at push time.
- Primitives:
  - `segment_ring(center, radius, n, len, width, states: &[SegState])`; segment paths precomputed and cached per `(radius, n)`.
  - `icon(name, center, size, color, alpha, scale, rotation)`; cached raster masks for static sizes, path draw when animating scale.
  - `badge(center, w, h, radius, stroke, text)`.
  - `text(str, size, color)`; glyph cache keyed by `(char, size)`.
  - `ripple(center, r, thickness, color, alpha)`.
  - `dots(centers, r, colors)`.
- Dirty rect: scene reports union of bounding boxes of elements that changed since last tick; full frame on scene switch. Idle breathing touches a single segment (about 12x12 px). Splash and sweep are full-frame for their duration.
- Performance target on Pi 3B+: under 4 ms render per screen per frame worst case; idle SPI traffic near zero.

## Animation system

- `Tween { from, to, duration, easing, start }` with `value(now)`. Easings: linear, ease-out-cubic, ease-in-out-cubic, spring (overshoot). Driven by monotonic clock, never frame count.
- `Smooth<f32>`: retargetable ease toward a value over 800 ms. All numbers shown go through it.
- Per-screen splash queue: one active, one pending; collapse rule above.
- Global sweep queue: FIFO, one active; suppresses local splashes while active.
- Night mode: `is_night(now, start, end)` handles windows that wrap midnight; fade out over 1 s then sleep displays; on wake play Boot.

## Display backends

```rust
pub trait Display: Send {
    fn push(&mut self, frame: &Frame, dirty: Rect) -> Result<()>;
    fn sleep(&mut self) -> Result<()>;
    fn wake(&mut self) -> Result<()>;
}
```

- `gc9a01`: `rppal` SPI and GPIO. Init sequence ported verbatim from the Python driver. Writes in chunks of at most 4096 bytes unless `spidev.bufsiz` is raised (documented in README). Per-screen rotation and flip from config, applied via precomputed index map during RGB565 conversion. Sleep: DISPLAY OFF then SLEEP IN. Wake: SLEEP OUT, 120 ms, DISPLAY ON.
- `sim`: one `minifb` window with four round panels in a column (`--sim-grid` for 2x2), circular mask, bezel shadow. Title bar shows fps and render ms per screen. Keyboard forwarded to the fake source.

## Config

Path from `--config`, default `~/.config/rackscreen/config.toml`. `config.example.toml` ships in the repo.

```toml
[k8s]
kubeconfig = "~/k8s-monitor.yaml"

[prometheus]
namespace = "monitoring"
service = "auto"
port = 9090
poll_secs = 5

[qbittorrent]
enabled = true
namespace = "arr-stack"
service = "qbittorrent"
port = 8080
user = ""
pass = ""
poll_secs = 3

[night]
enabled = true
start = "23:00"
end = "07:00"

[thresholds]
hot_cpu = 90
hot_mem = 90

[display]
brightness = 1.0
fps = 30

[[screens]]
role = "cpu"
spi = 0
cs = 0
dc = 6
rst = 5
rotate = 270
hflip = false
hz = 40_000_000

[[screens]]
role = "mem"
spi = 0
cs = 1
dc = 13
rst = 26
rotate = 270
hflip = true
hz = 40_000_000

[[screens]]
role = "pods"
spi = 1
cs = 0
dc = 23
rst = 22
rotate = 270
hflip = true
hz = 16_000_000

[[screens]]
role = "health"
spi = 1
cs = 1
dc = 4
rst = 27
rotate = 270
hflip = true
hz = 16_000_000
```

CLI: `rackscreen [--config PATH] [--sim] [--sim-grid] [--source fake|k8s] [--fps N]`. Default source is `k8s`; `--sim` without `--source` defaults to `fake`.

## Deploy and CI

- `deploy/rackscreen.service`: systemd unit, `Restart=always`, `After=network-online.target`, runs as a user in the `spi` and `gpio` groups.
- `deploy/install.sh`: copies binary to `/usr/local/bin`, config to `~/.config/rackscreen/`, enables the unit.
- README: enable SPI0 and SPI1 in `config.txt`, add `spidev.bufsiz=65536` to `cmdline.txt`, wiring table.
- GitHub Actions: `cargo test` and `cargo clippy` on push to `main`. On tag `v*`: build `aarch64-unknown-linux-gnu` with `cross`, attach binary to a GitHub Release. Nothing else runs in CI.

## Testing

- `core`: unit tests for event folding, splash collapse, sweep queueing, night window math, tween and easing values.
- `render`: golden-image tests. Render fixed scenes to PNG, compare against stored PNGs with a small tolerance. Goldens updated deliberately and reviewed in the simulator.
- `sources`: k8s diff logic tested with hand-built `Pod` and `Node` objects, no cluster. Fake source scripts are deterministic and asserted.
- Manual: simulator keyboard scenarios, then on the Pi.

## Out of scope

- Touch or button input.
- More than four screens or non-GC9A01 panels.
- Web UI or remote control.
- Metrics history or graphs.

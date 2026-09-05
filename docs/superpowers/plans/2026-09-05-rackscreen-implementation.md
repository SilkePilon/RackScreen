# RackScreen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust rewrite of the Kubernetes rack monitor for four GC9A01 round displays on a Pi 3B+, with a desktop simulator, icon-based Minimal Mono UI, smooth idle motion, event splashes and rack-wide sweeps.

**Architecture:** Cargo workspace with four library crates (`core`: state, events, animation, scenes; `render`: tiny-skia rasterizer; `display`: GC9A01 over rppal and a minifb simulator; `sources`: kube-rs, Prometheus, qBittorrent, fake) and one binary. Sources push `Event`s into a channel; a 30 Hz render thread folds them into a `Model`, builds one `Scene` per screen, rasterizes, orients, diffs, and hands frames to per-screen display threads.

**Tech Stack:** Rust 2021 (rustc 1.97), tiny-skia 0.12, usvg 0.48, fontdue 0.9, minifb 0.28, rppal 0.22, kube 4.2 + k8s-openapi 0.28 (v1_33), tokio 1, hyper 1, serde + toml, clap 4, cross 0.2.5 for aarch64 release builds.

Spec: `docs/superpowers/specs/2026-09-05-rackscreen-design.md`

## Global Constraints

- Target hardware: Pi 3B+, 64-bit OS, triple `aarch64-unknown-linux-gnu`.
- Screen size 240x240, ring radius 102 px, 60 segments, segment length 12 px, stroke 4 px round caps.
- Badge 64x26 px, corner radius 7, fill `#0e0e0e`, 2 px accent stroke, white bold 15 px JetBrains Mono digits.
- Accents: CPU `#ffb020`, MEM `#a78bfa`, PODS `#4f8dff`, HEALTH `#3ddc97`, red `#ff4d4d`, off segment `#1c1c1c`.
- Icon set: Lucide (ISC). Font: JetBrains Mono Bold (OFL). Both embedded with `include_bytes!`.
- Splash 2.5 s, sweep about 4 s, smoothing 800 ms, breathe 2.4 s. All timing from a monotonic clock in seconds (`f64`), never frame counts.
- Render thread at 30 Hz. One display thread per screen. Dirty-rect SPI updates only.
- `core` and `render` have no hardware or I/O deps.
- Commit after every task with a conventional message. Run `cargo test --workspace` before each commit.
- Deviation from spec (approved reasoning): dirty rect is computed by pixel diff of consecutive oriented frames instead of scene-reported bounding boxes. Same SPI savings, never misses a change, less code.

## File map

```
Cargo.toml                          workspace + binary package
src/main.rs                         CLI, wiring, render loop, display threads
src/config.rs                       TOML config structs + load
src/runloop.rs                      render loop + night mode + mailboxes
assets/icons/*.svg                  17 Lucide icons + LICENSE
assets/fonts/JetBrainsMono-Bold.ttf + OFL.txt
scripts/fetch-assets.sh             downloads assets
crates/core/src/lib.rs              re-exports
crates/core/src/theme.rs            Color, Role, palette, layout constants
crates/core/src/anim.rs             Easing, Tween, Smooth, breathe, pulse
crates/core/src/event.rs            Event, Torrent, LinkTarget
crates/core/src/fx.rs               SplashKind/Splash/SplashQueue, SweepKind/Sweep/SweepQueue
crates/core/src/night.rs            is_night
crates/core/src/model.rs            ClusterState, Model::apply/tick
crates/core/src/scene.rs            Drawable, Scene, role scenes, connecting/nodata, torrent mode
crates/core/src/scene_fx.rs         splash overlay and sweep scenes, Model::scene
crates/core/src/format.rs           fmt_speed, fmt_eta
crates/render/src/lib.rs            re-exports
crates/render/src/assets.rs         embedded font + icons
crates/render/src/frame.rs          Rect, Orient, dirty_rect, pack_rgb565
crates/render/src/prims.rs          segment cache, rounded rect, ripple, dots
crates/render/src/text.rs           fontdue text drawing
crates/render/src/icons.rs          usvg icon cache + draw
crates/render/src/renderer.rs       Renderer::render(Scene) -> Pixmap
crates/render/tests/golden.rs       golden image tests
crates/render/tests/goldens/*.png
crates/display/src/lib.rs           Display trait, DisplayCmd, Mailbox
crates/display/src/gc9a01.rs        rppal SPI driver (feature pi)
crates/display/src/sim.rs           minifb hub + panels (feature sim)
crates/sources/src/lib.rs           SourceCtx, spawn helpers
crates/sources/src/fake.rs          scripted generator + FakeCmd
crates/sources/src/tunnel.rs        kube port-forward HTTP/1.1 client
crates/sources/src/k8s.rs           PodTracker, NodeTracker, watchers
crates/sources/src/prometheus.rs    queries, parsing, poller
crates/sources/src/qbittorrent.rs   login, parsing, poller
deploy/rackscreen.service, deploy/install.sh
config.example.toml, README.md, LICENSE
.github/workflows/ci.yml, .github/workflows/release.yml
```

---

### Task 1: Workspace scaffold

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/render/Cargo.toml`, `crates/render/src/lib.rs`, `crates/display/Cargo.toml`, `crates/display/src/lib.rs`, `crates/sources/Cargo.toml`, `crates/sources/src/lib.rs`, `LICENSE`, `README.md`, `rust-toolchain.toml`

**Interfaces:**
- Produces: crate names `rackscreen-core`, `rackscreen-render`, `rackscreen-display`, `rackscreen-sources`, binary `rackscreen`. Features on root: `sim` (default), `pi` (default).

- [ ] **Step 1: Write root Cargo.toml**

```toml
[workspace]
members = ["crates/core", "crates/render", "crates/display", "crates/sources"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/silkepilon/RackScreen"

[workspace.dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tiny-skia = "0.12"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
tokio-util = "0.7"
tracing = "0.1"
futures = "0.3"

[package]
name = "rackscreen"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Animated Kubernetes rack monitor for four GC9A01 round displays"

[[bin]]
name = "rackscreen"
path = "src/main.rs"

[dependencies]
rackscreen-core = { path = "crates/core" }
rackscreen-render = { path = "crates/render" }
rackscreen-display = { path = "crates/display", default-features = false }
rackscreen-sources = { path = "crates/sources" }
anyhow.workspace = true
serde.workspace = true
tokio.workspace = true
tokio-util.workspace = true
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
toml = "1"
clap = { version = "4", features = ["derive"] }
dirs = "7"
tiny-skia.workspace = true

[features]
default = ["sim", "pi"]
sim = ["rackscreen-display/sim"]
pi = ["rackscreen-display/pi"]

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

- [ ] **Step 2: Write crate manifests and empty libs**

`crates/core/Cargo.toml`:
```toml
[package]
name = "rackscreen-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
```

`crates/render/Cargo.toml`:
```toml
[package]
name = "rackscreen-render"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rackscreen-core = { path = "../core" }
tiny-skia.workspace = true
usvg = "0.48"
fontdue = "0.9"
anyhow.workspace = true
```

`crates/display/Cargo.toml`:
```toml
[package]
name = "rackscreen-display"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rackscreen-render = { path = "../render" }
tiny-skia.workspace = true
anyhow.workspace = true
tracing.workspace = true
rppal = { version = "0.22", optional = true }
minifb = { version = "0.28", optional = true }

[features]
default = []
pi = ["dep:rppal"]
sim = ["dep:minifb"]
```

`crates/sources/Cargo.toml`:
```toml
[package]
name = "rackscreen-sources"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rackscreen-core = { path = "../core" }
anyhow.workspace = true
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tokio-util.workspace = true
tracing.workspace = true
futures.workspace = true
fastrand = "2"
kube = { version = "4.2", default-features = false, features = ["client", "config", "runtime", "rustls-tls", "ring", "ws"] }
k8s-openapi = { version = "0.28", features = ["v1_33"] }
hyper = { version = "1", features = ["client", "http1"] }
hyper-util = { version = "0.1", features = ["tokio"] }
http-body-util = "0.1"
bytes = "1"
urlencoding = "2"
```

Each `crates/*/src/lib.rs`:
```rust
//! (crate purpose, one line)
```

`src/main.rs`:
```rust
fn main() {
    println!("rackscreen");
}
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
```

`LICENSE`: MIT text with `Copyright (c) 2026 Silke Pilon`.

`README.md`:
```markdown
# RackScreen

Animated Kubernetes monitor for four GC9A01 round displays on a Raspberry Pi. Rust. See `docs/superpowers/specs/` for the design.
```

- [ ] **Step 3: Build**

Run: `cargo build --workspace`
Expected: `Finished` with no errors (first build downloads kube + tokio, a few minutes).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: scaffold cargo workspace"
```

---

### Task 2: Embedded assets (icons + font)

**Files:**
- Create: `scripts/fetch-assets.sh`, `assets/icons/*.svg`, `assets/icons/LICENSE`, `assets/fonts/JetBrainsMono-Bold.ttf`, `assets/fonts/OFL.txt`, `crates/render/src/assets.rs`
- Modify: `crates/render/src/lib.rs`

**Interfaces:**
- Produces: `rackscreen_render::assets::{FONT_BOLD: &[u8], ICON_NAMES: &[&str], icon_svg(name: &str) -> Option<&'static [u8]>}`

- [ ] **Step 1: Write fetch script**

`scripts/fetch-assets.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/icons assets/fonts
ICONS="cpu memory-stick box heart-pulse download package-plus package-x package-minus flame circle-check server-off server triangle-alert shield-check plug-zap plug cloud-off"
for i in $ICONS; do
  curl -sSL "https://unpkg.com/lucide-static@0.544.0/icons/$i.svg" -o "assets/icons/$i.svg"
  grep -q "<svg" "assets/icons/$i.svg" || { echo "bad svg: $i"; exit 1; }
done
curl -sSL "https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE" -o assets/icons/LICENSE
TMP=$(mktemp -d)
curl -sSL "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip" -o "$TMP/jb.zip"
unzip -q -o "$TMP/jb.zip" -d "$TMP"
cp "$TMP/fonts/ttf/JetBrainsMono-Bold.ttf" assets/fonts/JetBrainsMono-Bold.ttf
cp "$TMP/OFL.txt" assets/fonts/OFL.txt
rm -rf "$TMP"
echo "assets ok"
```

Run: `chmod +x scripts/fetch-assets.sh && ./scripts/fetch-assets.sh && ls assets/icons | wc -l`
Expected: `assets ok` and `18` (17 svg + LICENSE). If the pinned lucide-static version 404s, replace `0.544.0` with `latest`.

- [ ] **Step 2: Write failing asset test**

`crates/render/src/assets.rs`:
```rust
//! Embedded font and Lucide icons.

pub const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        pub const ICON_NAMES: &[&str] = &[$($name),*];
        pub fn icon_svg(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_bytes!(concat!("../../../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }
    };
}

icons!(
    "cpu", "memory-stick", "box", "heart-pulse", "download",
    "package-plus", "package-x", "package-minus", "flame", "circle-check",
    "server-off", "server", "triangle-alert", "shield-check",
    "plug-zap", "plug", "cloud-off",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_parses_as_svg() {
        for name in ICON_NAMES {
            let data = icon_svg(name).expect(name);
            let tree = usvg::Tree::from_data(data, &usvg::Options::default())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(tree.size().width(), 24.0, "{name}");
        }
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(icon_svg("nope").is_none());
    }

    #[test]
    fn font_loads() {
        let font = fontdue::Font::from_bytes(FONT_BOLD, fontdue::FontSettings::default()).unwrap();
        let (m, _) = font.rasterize('4', 15.0);
        assert!(m.width > 0 && m.height > 0);
    }
}
```

`crates/render/src/lib.rs`:
```rust
//! Software rasterizer for RackScreen scenes.
pub mod assets;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rackscreen-render`
Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(render): embed Lucide icons and JetBrains Mono"
```

---

### Task 3: core theme + animation primitives

**Files:**
- Create: `crates/core/src/theme.rs`, `crates/core/src/anim.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  - `theme::Color { r, g, b, a: u8 }` with `Color::rgb(r,g,b)`, `Color::hex(u32)`, `.with_alpha(f32) -> Color`, `.mix(other, t) -> Color`
  - `theme::{AMBER, VIOLET, BLUE, GREEN, RED, OFF, BADGE_FILL, WHITE, GREY, DIM_GREY, BLACK}`
  - `theme::Role { Cpu, Mem, Pods, Health }` with `Role::ALL`, `.index() -> usize`, `.accent() -> Color`, `.icon() -> &'static str`, `Role::from_index(usize) -> Option<Role>`
  - `theme::layout::{SIZE, CX, CY, RING_R, SEG_N, SEG_LEN, SEG_W, ICON_CY, ICON_SIZE, BADGE_CY, BADGE_W, BADGE_H, BADGE_RADIUS, BADGE_TEXT_PX, MARKER_CY, TORRENT_RADII, TORRENT_ICON_CY, TORRENT_ICON_SIZE, TORRENT_BADGE_CY, BIG_ICON_SIZE, DOT_R, DOT_SPACING}`
  - `anim::Secs = f64`, `anim::Easing { Linear, OutCubic, InOutCubic, Spring }.apply(t: f32) -> f32`
  - `anim::Tween::new(from, to, start, duration, easing)`, `.value(now) -> f32`, `.done(now) -> bool`, `.progress(now) -> f32`
  - `anim::Smooth::new(initial, duration)`, `.set(target, now)`, `.value(now) -> f32`, `.target() -> f32`
  - `anim::breathe(now, period) -> f32` in 0.35..1.0; `anim::pulse(now, period) -> f32` in 0..1; `anim::clamp01(f32)`

- [ ] **Step 1: Write theme.rs**

```rust
//! Colours, screen roles and fixed layout constants (all in 240x240 px space).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn hex(v: u32) -> Self {
        Self::rgb(((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8)
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..self }
    }
    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: l(self.a, other.a) }
    }
}

pub const AMBER: Color = Color::hex(0xffb020);
pub const VIOLET: Color = Color::hex(0xa78bfa);
pub const BLUE: Color = Color::hex(0x4f8dff);
pub const GREEN: Color = Color::hex(0x3ddc97);
pub const RED: Color = Color::hex(0xff4d4d);
pub const OFF: Color = Color::hex(0x1c1c1c);
pub const BADGE_FILL: Color = Color::hex(0x0e0e0e);
pub const WHITE: Color = Color::hex(0xffffff);
pub const GREY: Color = Color::hex(0x888888);
pub const DIM_GREY: Color = Color::hex(0x2a2a2a);
pub const BLACK: Color = Color::hex(0x000000);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Cpu,
    Mem,
    Pods,
    Health,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Cpu, Role::Mem, Role::Pods, Role::Health];

    pub fn index(self) -> usize {
        match self {
            Role::Cpu => 0,
            Role::Mem => 1,
            Role::Pods => 2,
            Role::Health => 3,
        }
    }
    pub fn from_index(i: usize) -> Option<Role> {
        Role::ALL.get(i).copied()
    }
    pub fn accent(self) -> Color {
        match self {
            Role::Cpu => AMBER,
            Role::Mem => VIOLET,
            Role::Pods => BLUE,
            Role::Health => GREEN,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "memory-stick",
            Role::Pods => "box",
            Role::Health => "heart-pulse",
        }
    }
}

pub mod layout {
    pub const SIZE: u32 = 240;
    pub const CX: f32 = 120.0;
    pub const CY: f32 = 120.0;
    pub const RING_R: f32 = 102.0;
    pub const SEG_N: usize = 60;
    pub const SEG_LEN: f32 = 12.0;
    pub const SEG_W: f32 = 4.0;
    pub const ICON_CY: f32 = 98.0;
    pub const ICON_SIZE: f32 = 72.0;
    pub const BADGE_CY: f32 = 165.0;
    pub const BADGE_W: f32 = 64.0;
    pub const BADGE_H: f32 = 26.0;
    pub const BADGE_RADIUS: f32 = 7.0;
    pub const BADGE_TEXT_PX: f32 = 15.0;
    pub const MARKER_CY: f32 = 188.0;
    pub const TORRENT_RADII: [f32; 3] = [102.0, 86.0, 70.0];
    pub const TORRENT_ICON_CY: f32 = 104.0;
    pub const TORRENT_ICON_SIZE: f32 = 44.0;
    pub const TORRENT_BADGE_CY: f32 = 151.0;
    pub const BIG_ICON_SIZE: f32 = 96.0;
    pub const DOT_R: f32 = 5.0;
    pub const DOT_SPACING: f32 = 14.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_channels() {
        assert_eq!(Color::hex(0xffb020), Color { r: 0xff, g: 0xb0, b: 0x20, a: 255 });
    }

    #[test]
    fn mix_midpoint() {
        let c = BLACK.mix(WHITE, 0.5);
        assert_eq!((c.r, c.g, c.b), (128, 128, 128));
    }

    #[test]
    fn role_index_roundtrip() {
        for r in Role::ALL {
            assert_eq!(Role::from_index(r.index()), Some(r));
        }
    }
}
```

- [ ] **Step 2: Write failing anim tests**

`crates/core/src/anim.rs`:
```rust
//! Time-based animation primitives. All times are seconds on a monotonic clock.

pub type Secs = f64;

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    OutCubic,
    InOutCubic,
    Spring,
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = clamp01(t);
        match self {
            Easing::Linear => t,
            Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Spring => {
                // easeOutBack: overshoots to ~1.10 around t=0.6 then settles at 1.0
                let c1 = 1.70158_f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: Secs,
    pub duration: Secs,
    pub easing: Easing,
}

impl Tween {
    pub fn new(from: f32, to: f32, start: Secs, duration: Secs, easing: Easing) -> Self {
        Self { from, to, start, duration, easing }
    }
    pub fn progress(&self, now: Secs) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        clamp01(((now - self.start) / self.duration) as f32)
    }
    pub fn value(&self, now: Secs) -> f32 {
        let e = self.easing.apply(self.progress(now));
        self.from + (self.to - self.from) * e
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.start >= self.duration
    }
}

/// A value that eases toward a retargetable goal without ever jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Smooth {
    current: f32,
    target: f32,
    tween: Option<Tween>,
    duration: Secs,
}

impl Smooth {
    pub fn new(initial: f32, duration: Secs) -> Self {
        Self { current: initial, target: initial, tween: None, duration }
    }
    pub fn set(&mut self, target: f32, now: Secs) {
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }
        let from = self.value(now);
        self.current = from;
        self.target = target;
        self.tween = Some(Tween::new(from, target, now, self.duration, Easing::OutCubic));
    }
    pub fn value(&self, now: Secs) -> f32 {
        match self.tween {
            Some(t) => t.value(now),
            None => self.current,
        }
    }
    pub fn target(&self) -> f32 {
        self.target
    }
}

/// Sine breathe between 0.35 and 1.0.
pub fn breathe(now: Secs, period: Secs) -> f32 {
    let p = pulse(now, period);
    0.35 + 0.65 * p
}

/// Sine pulse 0..1 with the given period, starting at 0.
pub fn pulse(now: Secs, period: Secs) -> f32 {
    let phase = (now / period) * std::f64::consts::TAU;
    (0.5 - 0.5 * phase.cos()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_midpoint() {
        assert_eq!(Easing::Linear.apply(0.5), 0.5);
    }

    #[test]
    fn out_cubic_known_value() {
        assert!((Easing::OutCubic.apply(0.5) - 0.875).abs() < 1e-6);
    }

    #[test]
    fn spring_overshoots_then_settles() {
        assert!(Easing::Spring.apply(0.6) > 1.0);
        assert!((Easing::Spring.apply(1.0) - 1.0).abs() < 1e-6);
        assert!((Easing::Spring.apply(0.0)).abs() < 1e-6);
    }

    #[test]
    fn tween_clamps_and_reports_done() {
        let t = Tween::new(0.0, 10.0, 1.0, 2.0, Easing::Linear);
        assert_eq!(t.value(0.0), 0.0);
        assert_eq!(t.value(2.0), 5.0);
        assert_eq!(t.value(5.0), 10.0);
        assert!(!t.done(2.9));
        assert!(t.done(3.0));
    }

    #[test]
    fn smooth_retarget_is_continuous() {
        let mut s = Smooth::new(0.0, 1.0);
        s.set(100.0, 0.0);
        let mid = s.value(0.5);
        assert!(mid > 50.0 && mid < 100.0);
        s.set(0.0, 0.5);
        assert!((s.value(0.5) - mid).abs() < 1e-4);
        assert!((s.value(2.0)).abs() < 1e-4);
    }

    #[test]
    fn smooth_ignores_same_target() {
        let mut s = Smooth::new(5.0, 1.0);
        s.set(5.0, 0.0);
        assert_eq!(s.value(0.5), 5.0);
    }

    #[test]
    fn breathe_range() {
        for i in 0..100 {
            let v = breathe(i as f64 * 0.05, 2.4);
            assert!((0.35..=1.0).contains(&v));
        }
        assert!((breathe(0.0, 2.4) - 0.35).abs() < 1e-6);
        assert!((breathe(1.2, 2.4) - 1.0).abs() < 1e-6);
    }
}
```

`crates/core/src/lib.rs`:
```rust
//! RackScreen core: state, events, animation and scene description. No I/O.
pub mod anim;
pub mod theme;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rackscreen-core`
Expected: 10 passed.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(core): theme constants and animation primitives"
```

---

### Task 4: Events, cluster state, Model folding

**Files:**
- Create: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/format.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  - `event::{Event, Torrent, LinkTarget}` exactly as below
  - `model::ClusterState` (pub fields), `model::LinkState { api, prom, qbit: bool }`, `model::Thresholds { hot_cpu: f32, hot_mem: f32 }`
  - `model::Model::new(thresholds) -> Model`, `.apply(&mut self, ev: Event, now: Secs)`, `.state() -> &ClusterState`, `.link() -> LinkState`, `.smooth_cpu(now)`, `.smooth_mem(now)`, `.smooth_pods(now)`, `.all_healthy() -> bool`, `.torrent_mode() -> bool`, `.night_override() -> Option<bool>`, `.pending_fx() -> &[FxRequest]` drained by Task 5 (`take_fx()`)
  - `format::{fmt_speed(bps: i64) -> String, fmt_eta(secs: i64) -> String}`
- Note: Task 5 adds `fx` queues to the Model; in this task the model records `FxRequest`s in a `Vec` so Task 5 can wire them.

- [ ] **Step 1: Write event.rs**

```rust
//! Events emitted by sources and folded into the model.

#[derive(Clone, Debug, PartialEq)]
pub struct Torrent {
    pub name: String,
    /// 0.0 ..= 100.0
    pub progress: f32,
    /// seconds, negative or huge means unknown
    pub eta_secs: i64,
    pub speed_bps: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    K8sApi,
    Prometheus,
    QBittorrent,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Metrics {
        cpu_pct: f32,
        mem_pct: f32,
        mem_used_gb: f32,
        mem_total_gb: f32,
        hot_cpu: Option<(String, f32)>,
        hot_mem: Option<(String, f32)>,
    },
    PodSnapshot { running: u32, pending: u32, failed: u32, total: u32 },
    PodStarted { ns: String, name: String },
    PodCrashed { ns: String, name: String },
    PodGone { ns: String, name: String },
    NodeSnapshot { ready: u32, total: u32, not_ready: Vec<String> },
    NodeReady { name: String, ready: bool },
    AlertSnapshot { firing: Vec<String> },
    AlertChanged { name: String, firing: bool },
    Torrents(Vec<Torrent>),
    TorrentAdded { name: String },
    TorrentDone { name: String },
    Link { target: LinkTarget, up: bool },
    /// Replay the boot animation (simulator key `b`, and on startup).
    Boot,
    /// Simulator override: Some(true) forces night, Some(false) forces day, None back to clock.
    ForceNight(Option<bool>),
}
```

- [ ] **Step 2: Write format.rs with tests**

```rust
//! Compact number formatting for the badge.

/// Bytes per second to a short badge string, e.g. `12.4M`, `850K`, `12B`.
pub fn fmt_speed(bps: i64) -> String {
    let b = bps.max(0) as f64;
    if b >= 1_048_576.0 {
        format!("{:.1}M", b / 1_048_576.0)
    } else if b >= 1024.0 {
        format!("{:.0}K", b / 1024.0)
    } else {
        format!("{}B", b as i64)
    }
}

/// Seconds to a short ETA, e.g. `45s`, `12m`, `1h05`. Unknown becomes `--`.
pub fn fmt_eta(secs: i64) -> String {
    if secs < 0 || secs > 864_000 {
        return "--".into();
    }
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{:02}", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_units() {
        assert_eq!(fmt_speed(13_002_342), "12.4M");
        assert_eq!(fmt_speed(870_400), "850K");
        assert_eq!(fmt_speed(12), "12B");
        assert_eq!(fmt_speed(-5), "0B");
    }

    #[test]
    fn eta_units() {
        assert_eq!(fmt_eta(45), "45s");
        assert_eq!(fmt_eta(720), "12m");
        assert_eq!(fmt_eta(3900), "1h05");
        assert_eq!(fmt_eta(-1), "--");
        assert_eq!(fmt_eta(8_640_000), "--");
    }
}
```

- [ ] **Step 3: Write failing model tests and model.rs**

```rust
//! Cluster state and the fold of events into it.

use std::collections::HashMap;

use crate::anim::{Secs, Smooth};
use crate::event::{Event, LinkTarget, Torrent};
use crate::theme::Role;

pub const SMOOTH_SECS: Secs = 0.8;
const HOT_DEBOUNCE_SECS: Secs = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    pub hot_cpu: f32,
    pub hot_mem: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self { hot_cpu: 90.0, hot_mem: 90.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkState {
    pub api: bool,
    pub prom: bool,
    pub qbit: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClusterState {
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub pods_running: u32,
    pub pods_pending: u32,
    pub pods_failed: u32,
    pub pods_total: u32,
    pub nodes_ready: u32,
    pub nodes_total: u32,
    pub nodes_not_ready: Vec<String>,
    pub alerts: Vec<String>,
    pub torrents: Vec<Torrent>,
    pub have_metrics: bool,
    pub have_pods: bool,
    pub have_nodes: bool,
}

/// What the fold wants the animation layer to do. Consumed by Task 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FxRequest {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode(Role),
    TorrentAdded,
    TorrentDone,
    NodeNotReady,
    NodeReady,
    AlertFiring,
    AlertResolved,
    LinkUp,
    Boot,
}

pub struct Model {
    state: ClusterState,
    link: LinkState,
    thresholds: Thresholds,
    cpu: Smooth,
    mem: Smooth,
    pods: Smooth,
    hot_last: HashMap<(Role, String), Secs>,
    night_override: Option<bool>,
    fx: Vec<FxRequest>,
    seen_api_up: bool,
}

impl Model {
    pub fn new(thresholds: Thresholds) -> Self {
        Self {
            state: ClusterState::default(),
            link: LinkState { api: false, prom: false, qbit: false },
            thresholds,
            cpu: Smooth::new(0.0, SMOOTH_SECS),
            mem: Smooth::new(0.0, SMOOTH_SECS),
            pods: Smooth::new(0.0, SMOOTH_SECS),
            hot_last: HashMap::new(),
            night_override: None,
            fx: Vec::new(),
            seen_api_up: false,
        }
    }

    pub fn state(&self) -> &ClusterState {
        &self.state
    }
    pub fn link(&self) -> LinkState {
        self.link
    }
    pub fn thresholds(&self) -> Thresholds {
        self.thresholds
    }
    pub fn night_override(&self) -> Option<bool> {
        self.night_override
    }
    pub fn smooth_cpu(&self, now: Secs) -> f32 {
        self.cpu.value(now)
    }
    pub fn smooth_mem(&self, now: Secs) -> f32 {
        self.mem.value(now)
    }
    pub fn smooth_pods(&self, now: Secs) -> f32 {
        self.pods.value(now)
    }
    pub fn pending_fx(&self) -> &[FxRequest] {
        &self.fx
    }
    pub fn take_fx(&mut self) -> Vec<FxRequest> {
        std::mem::take(&mut self.fx)
    }

    pub fn all_healthy(&self) -> bool {
        let s = &self.state;
        s.have_nodes && s.nodes_total > 0 && s.nodes_ready == s.nodes_total && s.alerts.is_empty()
    }

    pub fn torrent_mode(&self) -> bool {
        self.all_healthy() && self.link.qbit && !self.state.torrents.is_empty()
    }

    pub fn apply(&mut self, ev: Event, now: Secs) {
        match ev {
            Event::Metrics { cpu_pct, mem_pct, mem_used_gb, mem_total_gb, hot_cpu, hot_mem } => {
                self.state.cpu_pct = cpu_pct;
                self.state.mem_pct = mem_pct;
                self.state.mem_used_gb = mem_used_gb;
                self.state.mem_total_gb = mem_total_gb;
                self.state.have_metrics = true;
                self.cpu.set(cpu_pct, now);
                self.mem.set(mem_pct, now);
                let th = self.thresholds;
                self.check_hot(Role::Cpu, hot_cpu, th.hot_cpu, now);
                self.check_hot(Role::Mem, hot_mem, th.hot_mem, now);
            }
            Event::PodSnapshot { running, pending, failed, total } => {
                self.state.pods_running = running;
                self.state.pods_pending = pending;
                self.state.pods_failed = failed;
                self.state.pods_total = total;
                self.state.have_pods = true;
                self.pods.set(running as f32, now);
            }
            Event::PodStarted { .. } => self.fx.push(FxRequest::PodStarted),
            Event::PodCrashed { .. } => self.fx.push(FxRequest::PodCrashed),
            Event::PodGone { .. } => self.fx.push(FxRequest::PodGone),
            Event::NodeSnapshot { ready, total, not_ready } => {
                self.state.nodes_ready = ready;
                self.state.nodes_total = total;
                self.state.nodes_not_ready = not_ready;
                self.state.have_nodes = true;
            }
            Event::NodeReady { ready, .. } => {
                self.fx.push(if ready { FxRequest::NodeReady } else { FxRequest::NodeNotReady });
            }
            Event::AlertSnapshot { firing } => self.state.alerts = firing,
            Event::AlertChanged { firing, .. } => {
                self.fx.push(if firing { FxRequest::AlertFiring } else { FxRequest::AlertResolved });
            }
            Event::Torrents(list) => self.state.torrents = list,
            Event::TorrentAdded { .. } => self.fx.push(FxRequest::TorrentAdded),
            Event::TorrentDone { .. } => self.fx.push(FxRequest::TorrentDone),
            Event::Link { target, up } => match target {
                LinkTarget::K8sApi => {
                    let was = self.link.api;
                    self.link.api = up;
                    if up && !was && self.seen_api_up {
                        self.fx.push(FxRequest::LinkUp);
                    }
                    if up {
                        self.seen_api_up = true;
                    }
                }
                LinkTarget::Prometheus => self.link.prom = up,
                LinkTarget::QBittorrent => self.link.qbit = up,
            },
            Event::Boot => self.fx.push(FxRequest::Boot),
            Event::ForceNight(v) => self.night_override = v,
        }
    }

    fn check_hot(&mut self, role: Role, hot: Option<(String, f32)>, threshold: f32, now: Secs) {
        let Some((node, value)) = hot else { return };
        if value < threshold {
            return;
        }
        let key = (role, node);
        let recently = self.hot_last.get(&key).is_some_and(|t| now - t < HOT_DEBOUNCE_SECS);
        if !recently {
            self.hot_last.insert(key, now);
            self.fx.push(FxRequest::HotNode(role));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(cpu: f32, hot: Option<(&str, f32)>) -> Event {
        Event::Metrics {
            cpu_pct: cpu,
            mem_pct: 50.0,
            mem_used_gb: 8.0,
            mem_total_gb: 16.0,
            hot_cpu: hot.map(|(n, v)| (n.to_string(), v)),
            hot_mem: None,
        }
    }

    #[test]
    fn metrics_update_state_and_smooth() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(42.0, None), 0.0);
        assert_eq!(m.state().cpu_pct, 42.0);
        assert!(m.state().have_metrics);
        assert!(m.smooth_cpu(0.0) < 1.0);
        assert!((m.smooth_cpu(5.0) - 42.0).abs() < 1e-4);
    }

    #[test]
    fn pod_events_request_fx() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 0.0);
        m.apply(Event::PodCrashed { ns: "a".into(), name: "b".into() }, 0.0);
        assert_eq!(m.take_fx(), vec![FxRequest::PodStarted, FxRequest::PodCrashed]);
        assert!(m.pending_fx().is_empty());
    }

    #[test]
    fn node_flip_requests_sweep() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::NodeReady { name: "n1".into(), ready: false }, 0.0);
        assert_eq!(m.take_fx(), vec![FxRequest::NodeNotReady]);
    }

    #[test]
    fn hot_node_debounced_five_minutes() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(50.0, Some(("n1", 95.0))), 0.0);
        m.apply(metrics(50.0, Some(("n1", 96.0))), 10.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotNode(Role::Cpu)]);
        m.apply(metrics(50.0, Some(("n1", 96.0))), 301.0);
        assert_eq!(m.take_fx(), vec![FxRequest::HotNode(Role::Cpu)]);
        m.apply(metrics(50.0, Some(("n2", 50.0))), 302.0);
        assert!(m.take_fx().is_empty());
    }

    #[test]
    fn link_up_sweep_only_after_a_previous_up() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
        assert!(m.take_fx().is_empty(), "first connect is not a recovery");
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: false }, 1.0);
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 2.0);
        assert_eq!(m.take_fx(), vec![FxRequest::LinkUp]);
        assert!(m.link().api);
    }

    #[test]
    fn healthy_and_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.all_healthy());
        m.apply(Event::NodeSnapshot { ready: 3, total: 3, not_ready: vec![] }, 0.0);
        assert!(m.all_healthy());
        assert!(!m.torrent_mode());
        m.apply(Event::Link { target: LinkTarget::QBittorrent, up: true }, 0.0);
        m.apply(
            Event::Torrents(vec![Torrent { name: "x".into(), progress: 10.0, eta_secs: 100, speed_bps: 1000 }]),
            0.0,
        );
        assert!(m.torrent_mode());
        m.apply(Event::AlertSnapshot { firing: vec!["Down".into()] }, 0.0);
        assert!(!m.all_healthy());
        assert!(!m.torrent_mode());
    }
}
```

`crates/core/src/lib.rs`:
```rust
//! RackScreen core: state, events, animation and scene description. No I/O.
pub mod anim;
pub mod event;
pub mod format;
pub mod model;
pub mod theme;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rackscreen-core`
Expected: all pass (18 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): events, cluster state and model fold"
```

---

### Task 5: Splash and sweep queues, wired into the Model

**Files:**
- Create: `crates/core/src/fx.rs`
- Modify: `crates/core/src/model.rs`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `model::FxRequest`, `theme::Role`, `anim::Secs`
- Produces:
  - `fx::{SPLASH_SECS, SPLASH_COLLAPSE_SECS, SWEEP_WIPE_SECS, SWEEP_STAGGER_SECS, SWEEP_HOLD_SECS, BOOT_HOLD_SECS}`
  - `fx::SplashKind { PodStarted, PodCrashed, PodGone, HotNode, TorrentAdded }` with `.icon() -> &'static str`, `.color() -> Color`
  - `fx::Splash { kind, role, started, count }` with `.elapsed(now) -> f32`, `.done(now) -> bool`
  - `fx::SplashQueue::default()`, `.push(kind, role, now)`, `.tick(now, frozen: bool)`, `.active() -> Option<&Splash>`
  - `fx::SweepKind { TorrentDone, NodeNotReady, NodeReady, AlertFiring, AlertResolved, LinkUp, Boot }` with `.direction() -> Direction`, `.color(role) -> Color`, `.icon(role) -> &'static str`, `.hold() -> Secs`
  - `fx::Direction { Up, Down }`, `fx::SweepPhase { Idle, WipeIn(f32), Hold(f32), WipeOut(f32) }`
  - `fx::Sweep { kind, started }` with `.phase(role, now) -> SweepPhase`, `.done(now) -> bool`, `Sweep::order_index(direction, role) -> usize`
  - `fx::SweepQueue::default()`, `.push(kind)`, `.tick(now)`, `.active() -> Option<&Sweep>`
  - `fx::Fx { pub splashes: [SplashQueue; 4], pub sweeps: SweepQueue }` with `.apply(req: FxRequest, now)`, `.tick(now)`
  - `model::Model::tick(&mut self, now)` (drains requests into `Fx`, advances queues), `model::Model::fx(&self) -> &Fx`

- [ ] **Step 1: Write fx.rs with tests**

```rust
//! Splash (screen-local) and sweep (rack-wide) animation bookkeeping.

use std::collections::VecDeque;

use crate::anim::Secs;
use crate::model::FxRequest;
use crate::theme::{Color, Role, AMBER, BLUE, GREEN, RED};

pub const SPLASH_SECS: Secs = 2.5;
pub const SPLASH_COLLAPSE_SECS: Secs = 1.0;
pub const SWEEP_WIPE_SECS: Secs = 0.25;
pub const SWEEP_STAGGER_SECS: Secs = 0.12;
pub const SWEEP_HOLD_SECS: Secs = 2.5;
pub const BOOT_HOLD_SECS: Secs = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplashKind {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode,
    TorrentAdded,
}

impl SplashKind {
    pub fn icon(self) -> &'static str {
        match self {
            SplashKind::PodStarted => "package-plus",
            SplashKind::PodCrashed => "package-x",
            SplashKind::PodGone => "package-minus",
            SplashKind::HotNode => "flame",
            SplashKind::TorrentAdded => "download",
        }
    }
    pub fn color(self) -> Color {
        match self {
            SplashKind::PodStarted => BLUE,
            SplashKind::PodCrashed => RED,
            SplashKind::PodGone => BLUE.with_alpha(0.6),
            SplashKind::HotNode => AMBER,
            SplashKind::TorrentAdded => BLUE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Splash {
    pub kind: SplashKind,
    pub role: Role,
    pub started: Secs,
    pub count: u32,
}

impl Splash {
    pub fn elapsed(&self, now: Secs) -> f32 {
        (now - self.started).max(0.0) as f32
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.started >= SPLASH_SECS
    }
}

#[derive(Debug, Default)]
pub struct SplashQueue {
    active: Option<Splash>,
    pending: Option<Splash>,
    last_tick: Option<Secs>,
}

impl SplashQueue {
    pub fn push(&mut self, kind: SplashKind, role: Role, now: Secs) {
        if let Some(a) = &mut self.active {
            if a.kind == kind && now - a.started < SPLASH_COLLAPSE_SECS {
                a.count += 1;
                return;
            }
        }
        if let Some(p) = &mut self.pending {
            if p.kind == kind {
                p.count += 1;
                return;
            }
        }
        let s = Splash { kind, role, started: now, count: 1 };
        if self.active.is_none() {
            self.active = Some(s);
        } else {
            self.pending = Some(s);
        }
    }

    /// Advance. While `frozen` (a sweep is running) the active splash does not age.
    pub fn tick(&mut self, now: Secs, frozen: bool) {
        let dt = self.last_tick.map_or(0.0, |l| (now - l).max(0.0));
        self.last_tick = Some(now);
        if frozen {
            if let Some(a) = &mut self.active {
                a.started += dt;
            }
            return;
        }
        if self.active.is_some_and(|a| a.done(now)) {
            self.active = self.pending.take().map(|mut p| {
                p.started = now;
                p
            });
        }
    }

    pub fn active(&self) -> Option<&Splash> {
        self.active.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SweepKind {
    TorrentDone,
    NodeNotReady,
    NodeReady,
    AlertFiring,
    AlertResolved,
    LinkUp,
    Boot,
}

impl SweepKind {
    pub fn direction(self) -> Direction {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring => Direction::Up,
            _ => Direction::Down,
        }
    }
    pub fn color(self, role: Role) -> Color {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring => RED,
            SweepKind::Boot => role.accent(),
            _ => GREEN,
        }
    }
    pub fn icon(self, role: Role) -> &'static str {
        match self {
            SweepKind::TorrentDone => "circle-check",
            SweepKind::NodeNotReady => "server-off",
            SweepKind::NodeReady => "server",
            SweepKind::AlertFiring => "triangle-alert",
            SweepKind::AlertResolved => "shield-check",
            SweepKind::LinkUp => "plug",
            SweepKind::Boot => role.icon(),
        }
    }
    pub fn hold(self) -> Secs {
        match self {
            SweepKind::Boot => BOOT_HOLD_SECS,
            _ => SWEEP_HOLD_SECS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SweepPhase {
    Idle,
    WipeIn(f32),
    Hold(f32),
    WipeOut(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    pub kind: SweepKind,
    pub started: Secs,
}

impl Sweep {
    /// Position of a screen in the wave: 0 is the origin screen.
    pub fn order_index(direction: Direction, role: Role) -> usize {
        match direction {
            Direction::Down => role.index(),
            Direction::Up => 3 - role.index(),
        }
    }

    fn hold_end(&self) -> Secs {
        self.started + 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS + self.kind.hold()
    }

    pub fn phase(&self, role: Role, now: Secs) -> SweepPhase {
        let idx = Sweep::order_index(self.kind.direction(), role) as f64;
        let in_start = self.started + idx * SWEEP_STAGGER_SECS;
        let in_end = in_start + SWEEP_WIPE_SECS;
        let out_start = self.hold_end() + idx * SWEEP_STAGGER_SECS;
        let out_end = out_start + SWEEP_WIPE_SECS;
        if now < in_start || now >= out_end {
            SweepPhase::Idle
        } else if now < in_end {
            SweepPhase::WipeIn(((now - in_start) / SWEEP_WIPE_SECS) as f32)
        } else if now < out_start {
            SweepPhase::Hold(((now - in_end) / (out_start - in_end)) as f32)
        } else {
            SweepPhase::WipeOut(((now - out_start) / SWEEP_WIPE_SECS) as f32)
        }
    }

    pub fn done(&self, now: Secs) -> bool {
        now >= self.hold_end() + 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS
    }
}

#[derive(Debug, Default)]
pub struct SweepQueue {
    active: Option<Sweep>,
    queue: VecDeque<SweepKind>,
}

impl SweepQueue {
    pub fn push(&mut self, kind: SweepKind) {
        if !self.queue.contains(&kind) {
            self.queue.push_back(kind);
        }
    }
    pub fn tick(&mut self, now: Secs) {
        if self.active.is_some_and(|s| s.done(now)) {
            self.active = None;
        }
        if self.active.is_none() {
            if let Some(kind) = self.queue.pop_front() {
                self.active = Some(Sweep { kind, started: now });
            }
        }
    }
    pub fn active(&self) -> Option<&Sweep> {
        self.active.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct Fx {
    pub splashes: [SplashQueue; 4],
    pub sweeps: SweepQueue,
}

impl Fx {
    pub fn apply(&mut self, req: FxRequest, now: Secs) {
        let mut splash = |kind: SplashKind, role: Role| {
            self.splashes[role.index()].push(kind, role, now);
        };
        match req {
            FxRequest::PodStarted => splash(SplashKind::PodStarted, Role::Pods),
            FxRequest::PodCrashed => splash(SplashKind::PodCrashed, Role::Pods),
            FxRequest::PodGone => splash(SplashKind::PodGone, Role::Pods),
            FxRequest::HotNode(role) => splash(SplashKind::HotNode, role),
            FxRequest::TorrentAdded => splash(SplashKind::TorrentAdded, Role::Health),
            FxRequest::TorrentDone => self.sweeps.push(SweepKind::TorrentDone),
            FxRequest::NodeNotReady => self.sweeps.push(SweepKind::NodeNotReady),
            FxRequest::NodeReady => self.sweeps.push(SweepKind::NodeReady),
            FxRequest::AlertFiring => self.sweeps.push(SweepKind::AlertFiring),
            FxRequest::AlertResolved => self.sweeps.push(SweepKind::AlertResolved),
            FxRequest::LinkUp => self.sweeps.push(SweepKind::LinkUp),
            FxRequest::Boot => self.sweeps.push(SweepKind::Boot),
        }
    }

    pub fn tick(&mut self, now: Secs) {
        self.sweeps.tick(now);
        let frozen = self.sweeps.active().is_some();
        for q in &mut self.splashes {
            q.tick(now, frozen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_collapses_same_kind_within_a_second() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.push(SplashKind::PodStarted, Role::Pods, 0.5);
        q.push(SplashKind::PodStarted, Role::Pods, 0.9);
        let a = q.active().unwrap();
        assert_eq!(a.count, 3);
        assert_eq!(a.started, 0.0);
    }

    #[test]
    fn splash_queues_one_pending_and_promotes_when_done() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.push(SplashKind::PodCrashed, Role::Pods, 1.5);
        q.push(SplashKind::PodCrashed, Role::Pods, 1.6);
        assert_eq!(q.active().unwrap().kind, SplashKind::PodStarted);
        q.tick(2.0, false);
        assert_eq!(q.active().unwrap().kind, SplashKind::PodStarted);
        q.tick(2.6, false);
        let a = q.active().unwrap();
        assert_eq!(a.kind, SplashKind::PodCrashed);
        assert_eq!(a.count, 2);
        assert_eq!(a.started, 2.6);
        q.tick(5.2, false);
        assert!(q.active().is_none());
    }

    #[test]
    fn frozen_splash_does_not_age() {
        let mut q = SplashQueue::default();
        q.push(SplashKind::PodStarted, Role::Pods, 0.0);
        q.tick(0.0, false);
        q.tick(1.0, true);
        q.tick(2.0, true);
        q.tick(3.0, false);
        let a = q.active().expect("still active after being frozen 2 s");
        assert!((a.elapsed(3.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sweep_phases_follow_direction() {
        let s = Sweep { kind: SweepKind::NodeNotReady, started: 0.0 };
        assert_eq!(Sweep::order_index(Direction::Up, Role::Health), 0);
        assert_eq!(Sweep::order_index(Direction::Up, Role::Cpu), 3);
        assert!(matches!(s.phase(Role::Health, 0.1), SweepPhase::WipeIn(_)));
        assert_eq!(s.phase(Role::Cpu, 0.1), SweepPhase::Idle);
        assert!(matches!(s.phase(Role::Cpu, 0.5), SweepPhase::WipeIn(_)));
        assert!(matches!(s.phase(Role::Health, 1.5), SweepPhase::Hold(_)));
        assert!(matches!(s.phase(Role::Cpu, 1.5), SweepPhase::Hold(_)));
        let hold_end = 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS + SWEEP_HOLD_SECS;
        assert!(matches!(s.phase(Role::Health, hold_end + 0.1), SweepPhase::WipeOut(_)));
        assert!(matches!(s.phase(Role::Cpu, hold_end + 0.1), SweepPhase::Hold(_)));
        assert!(!s.done(hold_end + 0.5));
        assert!(s.done(hold_end + 3.0 * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS));
        assert_eq!(s.phase(Role::Cpu, 10.0), SweepPhase::Idle);
    }

    #[test]
    fn sweep_queue_serialises_and_dedupes() {
        let mut q = SweepQueue::default();
        q.push(SweepKind::AlertFiring);
        q.push(SweepKind::AlertFiring);
        q.push(SweepKind::TorrentDone);
        q.tick(0.0);
        assert_eq!(q.active().unwrap().kind, SweepKind::AlertFiring);
        q.tick(1.0);
        assert_eq!(q.active().unwrap().kind, SweepKind::AlertFiring);
        q.tick(4.0);
        assert_eq!(q.active().unwrap().kind, SweepKind::TorrentDone);
        q.tick(8.0);
        assert!(q.active().is_none());
    }

    #[test]
    fn fx_routes_requests() {
        let mut fx = Fx::default();
        fx.apply(FxRequest::HotNode(Role::Mem), 0.0);
        fx.apply(FxRequest::NodeNotReady, 0.0);
        fx.tick(0.0);
        assert_eq!(fx.splashes[Role::Mem.index()].active().unwrap().kind, SplashKind::HotNode);
        assert_eq!(fx.sweeps.active().unwrap().kind, SweepKind::NodeNotReady);
    }
}
```

- [ ] **Step 2: Wire Fx into Model**

In `crates/core/src/model.rs` add the import `use crate::fx::Fx;`, add field `fx_state: Fx` to `Model` (initialise with `Fx::default()` in `new`), and add:

```rust
    pub fn fx(&self) -> &Fx {
        &self.fx_state
    }

    /// Drain animation requests into the queues and advance them. Call once per frame.
    pub fn tick(&mut self, now: Secs) {
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        self.fx_state.tick(now);
    }
```

Add a test to `model.rs`:
```rust
    #[test]
    fn tick_moves_requests_into_queues() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 0.0);
        m.tick(0.0);
        assert!(m.pending_fx().is_empty());
        assert!(m.fx().splashes[Role::Pods.index()].active().is_some());
    }
```

`crates/core/src/lib.rs` add `pub mod fx;`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p rackscreen-core`
Expected: all pass (25 tests).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(core): splash and sweep queues"
```

---

### Task 6: Night window

**Files:**
- Create: `crates/core/src/night.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `night::parse_hhmm(&str) -> Option<u32>` (minutes since midnight), `night::is_night(now_min: u32, start_min: u32, end_min: u32) -> bool`

- [ ] **Step 1: Write night.rs with tests**

```rust
//! Quiet-hours window math. Time zone handling happens in the binary.

pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// True when `now_min` lies inside [start, end). Windows may wrap midnight.
/// start == end disables the window.
pub fn is_night(now_min: u32, start_min: u32, end_min: u32) -> bool {
    if start_min == end_min {
        return false;
    }
    if start_min < end_min {
        (start_min..end_min).contains(&now_min)
    } else {
        now_min >= start_min || now_min < end_min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("23:00"), Some(1380));
        assert_eq!(parse_hhmm("07:05"), Some(425));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("x"), None);
    }

    #[test]
    fn wrapping_window() {
        let (s, e) = (1380, 420);
        assert!(is_night(1390, s, e));
        assert!(is_night(0, s, e));
        assert!(is_night(419, s, e));
        assert!(!is_night(420, s, e));
        assert!(!is_night(720, s, e));
    }

    #[test]
    fn plain_window_and_disabled() {
        assert!(is_night(120, 60, 360));
        assert!(!is_night(360, 60, 360));
        assert!(!is_night(120, 300, 300));
    }
}
```

`crates/core/src/lib.rs` add `pub mod night;`.

- [ ] **Step 2: Run tests, commit**

Run: `cargo test -p rackscreen-core night`
Expected: 3 passed.

```bash
git add -A
git commit -m "feat(core): night window"
```

---

### Task 7: Scene description and role scenes

**Files:**
- Create: `crates/core/src/scene.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `Model` accessors, `theme::layout`, `anim::{breathe, pulse}`, `format`
- Produces:
  - `scene::SegState { Off, On(Color, f32) }`
  - `scene::Drawable` enum: `Clear(Color)`, `Ring { cx, cy, radius, n, states: Vec<SegState> }`, `Icon { name: &'static str, cx, cy, size, color, alpha, scale, dy }`, `Badge { cx, cy, w, h, radius, stroke, fill, text, text_px, text_color, alpha }`, `Ripple { cx, cy, r, thickness, color, alpha }`, `Dots { cx, cy, spacing, r, colors: Vec<Color> }`
  - `scene::Scene { items: Vec<Drawable> }` with `Scene::new()`, `.push(d)`, `.ring_mut() -> Option<&mut Drawable>`, `.main_icon_mut() -> Option<&mut Drawable>`
  - `scene::ring_states(pct: f32, accent: Color, n: usize, now: Secs) -> Vec<SegState>`
  - `scene::pod_segments(running: f32, pending: u32, failed: u32, total: u32, now: Secs) -> Vec<SegState>`
  - `scene::role_scene(model: &Model, role: Role, now: Secs) -> Scene`
  - `scene::connecting_scene(now) -> Scene`, `scene::no_data_scene(now) -> Scene`

- [ ] **Step 1: Write scene.rs with tests**

```rust
//! Pure scene description: what to draw on one screen at one instant.

use crate::anim::{breathe, pulse, Secs};
use crate::event::Torrent;
use crate::format::{fmt_eta, fmt_speed};
use crate::model::Model;
use crate::theme::layout::*;
use crate::theme::{Color, Role, BADGE_FILL, BLACK, BLUE, DIM_GREY, GREEN, GREY, RED, VIOLET, WHITE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegState {
    Off,
    On(Color, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drawable {
    Clear(Color),
    Ring { cx: f32, cy: f32, radius: f32, n: usize, states: Vec<SegState> },
    Icon { name: &'static str, cx: f32, cy: f32, size: f32, color: Color, alpha: f32, scale: f32, dy: f32 },
    Badge {
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        radius: f32,
        stroke: Color,
        fill: Color,
        text: String,
        text_px: f32,
        text_color: Color,
        alpha: f32,
    },
    Ripple { cx: f32, cy: f32, r: f32, thickness: f32, color: Color, alpha: f32 },
    Dots { cx: f32, cy: f32, spacing: f32, r: f32, colors: Vec<Color> },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub items: Vec<Drawable>,
}

impl Scene {
    pub fn new() -> Self {
        Self { items: vec![Drawable::Clear(BLACK)] }
    }
    pub fn push(&mut self, d: Drawable) {
        self.items.push(d);
    }
    /// The outermost ring (first Ring pushed).
    pub fn ring_mut(&mut self) -> Option<&mut Drawable> {
        self.items.iter_mut().find(|d| matches!(d, Drawable::Ring { .. }))
    }
    /// The role icon (first Icon pushed).
    pub fn main_icon_mut(&mut self) -> Option<&mut Drawable> {
        self.items.iter_mut().find(|d| matches!(d, Drawable::Icon { .. }))
    }
    pub fn lit_count(&self) -> usize {
        match self.items.iter().find(|d| matches!(d, Drawable::Ring { .. })) {
            Some(Drawable::Ring { states, .. }) => states.iter().filter(|s| matches!(s, SegState::On(..))).count(),
            _ => 0,
        }
    }
}

fn icon(name: &'static str, cy: f32, size: f32, color: Color, alpha: f32) -> Drawable {
    Drawable::Icon { name, cx: CX, cy, size, color, alpha, scale: 1.0, dy: 0.0 }
}

fn badge(cy: f32, stroke: Color, text: String) -> Drawable {
    Drawable::Badge {
        cx: CX,
        cy,
        w: BADGE_W,
        h: BADGE_H,
        radius: BADGE_RADIUS,
        stroke,
        fill: BADGE_FILL,
        text,
        text_px: BADGE_TEXT_PX,
        text_color: WHITE,
        alpha: 1.0,
    }
}

fn ring(radius: f32, states: Vec<SegState>) -> Drawable {
    let n = states.len();
    Drawable::Ring { cx: CX, cy: CY, radius, n, states }
}

/// Segment count for a ring of the given radius (60 at r=102, fewer inside).
pub fn seg_count(radius: f32) -> usize {
    ((SEG_N as f32) * radius / RING_R).round() as usize
}

/// Lit segments for a percentage; the last lit one breathes.
pub fn ring_states(pct: f32, accent: Color, n: usize, now: Secs) -> Vec<SegState> {
    let lit = ((pct.clamp(0.0, 100.0) / 100.0) * n as f32).round() as usize;
    (0..n)
        .map(|i| {
            if i + 1 < lit {
                SegState::On(accent, 1.0)
            } else if i + 1 == lit {
                SegState::On(accent, breathe(now, 2.4))
            } else {
                SegState::Off
            }
        })
        .collect()
}

/// Segments for the PODS ring: running (blue), pending (pulsing dim blue), failed (red).
pub fn pod_segments(running: f32, pending: u32, failed: u32, total: u32, now: Secs) -> Vec<SegState> {
    let n = SEG_N;
    if total == 0 {
        return vec![SegState::Off; n];
    }
    let scale = if total as usize <= n { 1.0 } else { n as f32 / total as f32 };
    let run = (running.max(0.0) * scale).round() as usize;
    let pend = ((pending as f32) * scale).round() as usize;
    let mut fail = ((failed as f32) * scale).round() as usize;
    if failed > 0 {
        fail = fail.max(1);
    }
    let pend_alpha = 0.3 + 0.5 * pulse(now, 1.2);
    let mut out = vec![SegState::Off; n];
    let mut i = 0;
    for _ in 0..run.min(n) {
        out[i] = SegState::On(BLUE, 1.0);
        i += 1;
    }
    if run > 0 && i > 0 {
        out[i - 1] = SegState::On(BLUE, breathe(now, 2.4));
    }
    for _ in 0..pend.min(n - i) {
        out[i] = SegState::On(BLUE, pend_alpha);
        i += 1;
    }
    for _ in 0..fail.min(n - i) {
        out[i] = SegState::On(RED, 1.0);
        i += 1;
    }
    out
}

fn heartbeat(now: Secs) -> f32 {
    let phase = now % 4.0;
    if phase < 0.3 {
        ((phase / 0.3) * std::f64::consts::PI).sin() as f32
    } else {
        0.0
    }
}

pub fn role_scene(model: &Model, role: Role, now: Secs) -> Scene {
    match role {
        Role::Cpu => {
            let pct = model.smooth_cpu(now);
            let mut s = Scene::new();
            s.push(ring(RING_R, ring_states(pct, role.accent(), SEG_N, now)));
            s.push(icon(role.icon(), ICON_CY, ICON_SIZE, WHITE, 0.6 + 0.4 * pulse(now, 3.5)));
            s.push(badge(BADGE_CY, role.accent(), format!("{:.0}%", pct)));
            s
        }
        Role::Mem => {
            let pct = model.smooth_mem(now);
            let mut s = Scene::new();
            s.push(ring(RING_R, ring_states(pct, role.accent(), SEG_N, now)));
            s.push(icon(role.icon(), ICON_CY, ICON_SIZE, WHITE, 1.0));
            s.push(badge(BADGE_CY, role.accent(), format!("{:.0}%", pct)));
            s
        }
        Role::Pods => {
            let st = model.state();
            let running = model.smooth_pods(now);
            let mut s = Scene::new();
            s.push(ring(RING_R, pod_segments(running, st.pods_pending, st.pods_failed, st.pods_total, now)));
            let mut ic = icon(role.icon(), ICON_CY, ICON_SIZE, WHITE, 1.0);
            if let Drawable::Icon { dy, .. } = &mut ic {
                *dy = -3.0 * pulse(now, 2.6);
            }
            s.push(ic);
            s.push(badge(BADGE_CY, role.accent(), format!("{}", running.round() as u32)));
            if st.pods_failed > 0 {
                s.push(Drawable::Dots { cx: CX, cy: MARKER_CY, spacing: 0.0, r: 3.0, colors: vec![RED.with_alpha(breathe(now, 2.4))] });
            }
            s
        }
        Role::Health => {
            if model.torrent_mode() {
                return torrent_scene(&model.state().torrents, now);
            }
            let st = model.state();
            let mut s = Scene::new();
            let states = if st.nodes_total == 0 {
                vec![SegState::Off; SEG_N]
            } else {
                let ready_pct = st.nodes_ready as f32 / st.nodes_total as f32 * 100.0;
                let mut v = ring_states(ready_pct, GREEN, SEG_N, now);
                let lit = v.iter().filter(|x| matches!(x, SegState::On(..))).count();
                for x in v.iter_mut().skip(lit) {
                    *x = SegState::On(RED, 0.9);
                }
                v
            };
            s.push(ring(RING_R, states));
            let alert = !st.alerts.is_empty();
            let mut heart = icon(role.icon(), ICON_CY, ICON_SIZE, if alert { RED } else { WHITE }, 1.0);
            if let Drawable::Icon { scale, .. } = &mut heart {
                *scale = 1.0 + 0.12 * heartbeat(now);
            }
            s.push(heart);
            s.push(badge(BADGE_CY, if alert { RED } else { GREEN }, String::new()));
            let total = st.nodes_total as usize;
            if total > 0 {
                let ready = st.nodes_ready as usize;
                let colors = (0..total).map(|i| if i < ready { GREEN } else { RED }).collect();
                s.push(Drawable::Dots { cx: CX, cy: BADGE_CY, spacing: DOT_SPACING, r: DOT_R, colors });
            }
            if alert {
                s.push(Drawable::Dots { cx: CX, cy: MARKER_CY, spacing: 0.0, r: 3.0, colors: vec![RED.with_alpha(breathe(now, 2.4))] });
            }
            s
        }
    }
}

const TORRENT_ACCENTS: [Color; 3] = [GREEN, BLUE, VIOLET];

pub fn torrent_scene(torrents: &[Torrent], now: Secs) -> Scene {
    let mut sorted: Vec<&Torrent> = torrents.iter().collect();
    sorted.sort_by(|a, b| b.progress.partial_cmp(&a.progress).unwrap_or(std::cmp::Ordering::Equal));
    let mut s = Scene::new();
    for (i, t) in sorted.iter().take(3).enumerate() {
        let r = TORRENT_RADII[i];
        s.push(ring(r, ring_states(t.progress, TORRENT_ACCENTS[i], seg_count(r), now)));
    }
    let mut ic = icon("download", TORRENT_ICON_CY, TORRENT_ICON_SIZE, WHITE, 1.0);
    if let Drawable::Icon { dy, .. } = &mut ic {
        *dy = -2.0 + 4.0 * pulse(now, 1.6);
    }
    s.push(ic);
    let window = (now / 5.0).floor();
    let frac = now - window * 5.0;
    let text = if (window as i64) % 2 == 0 {
        fmt_speed(sorted.iter().map(|t| t.speed_bps).sum())
    } else {
        let eta = sorted.iter().map(|t| t.eta_secs).filter(|e| (0..864_000).contains(e)).min();
        fmt_eta(eta.unwrap_or(-1))
    };
    let mut b = badge(TORRENT_BADGE_CY, GREEN, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = ((frac / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s
}

pub fn connecting_scene(now: Secs) -> Scene {
    let n = SEG_N;
    let head = ((now / 2.4) * n as f64).floor() as usize % n;
    let tail = [1.0, 0.7, 0.45, 0.25];
    let mut states = vec![SegState::Off; n];
    for (k, a) in tail.iter().enumerate() {
        let idx = (head + n - k) % n;
        states[idx] = SegState::On(GREY, *a);
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(icon("plug-zap", CY, BIG_ICON_SIZE, GREY, breathe(now, 2.4)));
    s
}

pub fn no_data_scene(now: Secs) -> Scene {
    let n = SEG_N;
    let mut states = vec![SegState::On(DIM_GREY, 1.0); n];
    let phase = now % 4.0;
    if phase < 1.0 {
        let idx = ((phase * n as f64).floor() as usize).min(n - 1);
        states[idx] = SegState::On(GREY, 1.0);
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(icon("cloud-off", CY, BIG_ICON_SIZE, Color::hex(0x666666), 1.0));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget};
    use crate::model::Thresholds;

    fn model_with_cpu(pct: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Metrics { cpu_pct: pct, mem_pct: 10.0, mem_used_gb: 1.0, mem_total_gb: 8.0, hot_cpu: None, hot_mem: None },
            0.0,
        );
        m
    }

    #[test]
    fn ring_states_lights_rounded_share() {
        let v = ring_states(42.0, BLUE, 60, 0.0);
        assert_eq!(v.iter().filter(|s| matches!(s, SegState::On(..))).count(), 25);
        assert!(matches!(v[24], SegState::On(_, a) if a < 1.0), "last lit breathes");
        assert!(matches!(v[0], SegState::On(_, a) if a == 1.0));
        assert_eq!(ring_states(0.0, BLUE, 60, 0.0).iter().filter(|s| matches!(s, SegState::On(..))).count(), 0);
        assert_eq!(ring_states(100.0, BLUE, 60, 0.0).len(), 60);
    }

    #[test]
    fn pod_segments_order_and_colours() {
        let v = pod_segments(50.0, 2, 1, 53, 0.0);
        assert!(matches!(v[0], SegState::On(c, _) if c == BLUE));
        assert!(matches!(v[50], SegState::On(c, a) if c == BLUE && a < 1.0), "pending dim");
        assert!(matches!(v[52], SegState::On(c, _) if c == RED));
        assert_eq!(v[53], SegState::Off);
    }

    #[test]
    fn pod_segments_scale_when_more_than_sixty() {
        let v = pod_segments(180.0, 0, 1, 200, 0.0);
        let lit = v.iter().filter(|s| matches!(s, SegState::On(..))).count();
        assert_eq!(lit, 55, "54 running + at least 1 failed");
        assert!(matches!(v[54], SegState::On(c, _) if c == RED));
    }

    #[test]
    fn cpu_scene_after_settling_shows_value() {
        let m = model_with_cpu(42.0);
        let s = role_scene(&m, Role::Cpu, 5.0);
        assert_eq!(s.lit_count(), 25);
        let badge_text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(badge_text.as_deref(), Some("42%"));
        assert!(matches!(s.items[0], Drawable::Clear(_)));
    }

    #[test]
    fn health_scene_switches_to_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::NodeSnapshot { ready: 2, total: 2, not_ready: vec![] }, 0.0);
        let s = role_scene(&m, Role::Health, 0.0);
        assert_eq!(s.lit_count(), 60);
        let dots = s.items.iter().filter(|d| matches!(d, Drawable::Dots { .. })).count();
        assert_eq!(dots, 1);
        m.apply(Event::Link { target: LinkTarget::QBittorrent, up: true }, 0.0);
        m.apply(
            Event::Torrents(vec![
                Torrent { name: "a".into(), progress: 50.0, eta_secs: 60, speed_bps: 2_097_152 },
                Torrent { name: "b".into(), progress: 90.0, eta_secs: 30, speed_bps: 1_048_576 },
            ]),
            0.0,
        );
        let s = role_scene(&m, Role::Health, 0.0);
        let rings: Vec<_> = s.items.iter().filter(|d| matches!(d, Drawable::Ring { .. })).collect();
        assert_eq!(rings.len(), 2);
        if let Drawable::Ring { radius, .. } = rings[0] {
            assert_eq!(*radius, 102.0);
        }
        assert_eq!(s.lit_count(), 54, "outer ring is the 90% torrent");
        let text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("3.0M"));
        let s2 = role_scene(&m, Role::Health, 5.5);
        let text2 = s2.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text2.as_deref(), Some("30s"));
    }

    #[test]
    fn health_not_ready_shows_red_tail() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::NodeSnapshot { ready: 3, total: 4, not_ready: vec!["n4".into()] }, 0.0);
        let s = role_scene(&m, Role::Health, 0.0);
        assert_eq!(s.lit_count(), 60);
        if let Some(Drawable::Ring { states, .. }) = s.items.iter().find(|d| matches!(d, Drawable::Ring { .. })) {
            assert!(matches!(states[59], SegState::On(c, _) if c == RED));
            assert!(matches!(states[0], SegState::On(c, _) if c == GREEN));
        }
    }

    #[test]
    fn connecting_has_comet_and_big_icon() {
        let s = connecting_scene(0.0);
        assert_eq!(s.lit_count(), 4);
        assert!(matches!(s.items[2], Drawable::Icon { name: "plug-zap", size, .. } if size == BIG_ICON_SIZE));
        let s2 = connecting_scene(0.6);
        assert_eq!(s2.lit_count(), 4);
        assert_ne!(s, s2);
    }

    #[test]
    fn no_data_is_dim_full_ring() {
        let s = no_data_scene(2.0);
        assert_eq!(s.lit_count(), 60);
        assert!(matches!(s.items[2], Drawable::Icon { name: "cloud-off", .. }));
    }
}
```

`crates/core/src/lib.rs` add `pub mod scene;`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p rackscreen-core scene`
Expected: 8 passed.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(core): role, torrent, connecting and no-data scenes"
```

---

### Task 8: Splash overlay, sweep scenes, Model::scene

**Files:**
- Create: `crates/core/src/scene_fx.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/model.rs`

**Interfaces:**
- Consumes: `scene::*`, `fx::*`, `anim::Easing`
- Produces:
  - `scene_fx::splash_overlay(base: Scene, splash: &Splash, now) -> Scene`
  - `scene_fx::sweep_scene(sweep: &Sweep, role: Role, phase: SweepPhase, now) -> Scene`
  - `Model::scene(&self, role: Role, now: Secs) -> Scene` (the one call the render loop makes)

- [ ] **Step 1: Write scene_fx.rs with tests**

```rust
//! Splash overlays and sweep scenes layered on top of role scenes.

use crate::anim::{pulse, Easing, Secs};
use crate::fx::{Splash, Sweep, SweepPhase, SPLASH_SECS};
use crate::scene::{connecting_scene, no_data_scene, role_scene, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Role, WHITE};

const RIPPLE_SECS: f32 = 0.3;
const ICON_SWAP_START: f32 = 0.1;
const ICON_SWAP_SECS: f32 = 0.4;
const FADE_BACK_START: f32 = 2.0;
const FLASH_STAGGER: f32 = 0.01;
const FLASH_SECS: f32 = 0.3;

fn unit(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn splash_overlay(mut base: Scene, splash: &Splash, now: Secs) -> Scene {
    let e = splash.elapsed(now);
    if e >= SPLASH_SECS as f32 {
        return base;
    }
    let color = splash.kind.color();

    // 1. ring flash wave
    if let Some(Drawable::Ring { states, .. }) = base.ring_mut() {
        for (i, st) in states.iter_mut().enumerate() {
            let t0 = i as f32 * FLASH_STAGGER;
            let f = 1.0 - unit((e - t0) / FLASH_SECS);
            if e >= t0 && f > 0.0 {
                let (c, a) = match *st {
                    SegState::On(c, a) => (c, a),
                    SegState::Off => (crate::theme::OFF, 1.0),
                };
                *st = SegState::On(c.mix(color, f), a.max(f));
            }
        }
    }

    // 2. role icon fades out then back in
    let swap = unit((e - ICON_SWAP_START) / ICON_SWAP_SECS);
    let back = unit((e - FADE_BACK_START) / (SPLASH_SECS as f32 - FADE_BACK_START));
    let role_alpha = (1.0 - swap).max(back);
    let (icx, icy, isize) = match base.main_icon_mut() {
        Some(Drawable::Icon { alpha, cx, cy, size, .. }) => {
            *alpha *= role_alpha;
            (*cx, *cy, *size)
        }
        _ => (CX, ICON_CY, ICON_SIZE),
    };

    // 3. event icon pops in with spring, fades out at the end
    if swap > 0.0 {
        let scale = Easing::Spring.apply(swap);
        let alpha = swap.min(1.0 - back);
        base.push(Drawable::Icon { name: splash.kind.icon(), cx: icx, cy: icy, size: isize, color, alpha, scale, dy: 0.0 });
    }

    // 4. ripple
    if e < RIPPLE_SECS {
        let t = e / RIPPLE_SECS;
        base.push(Drawable::Ripple {
            cx: CX,
            cy: CY,
            r: 110.0 * Easing::OutCubic.apply(t),
            thickness: 3.0,
            color,
            alpha: 0.8 * (1.0 - t),
        });
    }

    // 5. collapsed counter in the badge
    if splash.count > 1 {
        for d in base.items.iter_mut() {
            if let Drawable::Badge { text, .. } = d {
                *text = format!("+{}", splash.count);
            }
        }
    }
    base
}

pub fn sweep_scene(sweep: &Sweep, role: Role, phase: SweepPhase, now: Secs) -> Scene {
    let color = sweep.kind.color(role);
    let n = SEG_N;
    let (lit, ring_alpha, icon_alpha) = match phase {
        SweepPhase::Idle => (0, 1.0, 0.0),
        SweepPhase::WipeIn(p) => {
            let p = Easing::OutCubic.apply(p);
            ((p * n as f32).round() as usize, 1.0, p)
        }
        SweepPhase::Hold(_) => (n, 0.6 + 0.4 * pulse(now, 1.2), 1.0),
        SweepPhase::WipeOut(p) => (((1.0 - p) * n as f32).round() as usize, 1.0, 1.0 - p),
    };
    let states = (0..n).map(|i| if i < lit { SegState::On(color, ring_alpha) } else { SegState::Off }).collect();
    let mut s = Scene::new();
    s.push(Drawable::Ring { cx: CX, cy: CY, radius: RING_R, n, states });
    s.push(Drawable::Icon {
        name: sweep.kind.icon(role),
        cx: CX,
        cy: CY,
        size: BIG_ICON_SIZE,
        color: if matches!(sweep.kind, crate::fx::SweepKind::Boot) { WHITE } else { color },
        alpha: icon_alpha,
        scale: 1.0,
        dy: 0.0,
    });
    s
}

impl crate::model::Model {
    fn needs_data(&self, role: Role) -> bool {
        let st = self.state();
        match role {
            Role::Cpu | Role::Mem => !(self.link().prom && st.have_metrics),
            Role::Pods => !st.have_pods,
            Role::Health => !st.have_nodes,
        }
    }

    /// Everything the render loop needs for one screen at one instant.
    pub fn scene(&self, role: Role, now: Secs) -> Scene {
        if !self.link().api {
            return connecting_scene(now);
        }
        let sweep = self.fx().sweeps.active();
        if let Some(sw) = sweep {
            match sw.phase(role, now) {
                SweepPhase::Idle => {}
                ph => return sweep_scene(sw, role, ph, now),
            }
        }
        let base = if self.needs_data(role) { no_data_scene(now) } else { role_scene(self, role, now) };
        if sweep.is_some() {
            return base;
        }
        match self.fx().splashes[role.index()].active() {
            Some(sp) => splash_overlay(base, sp, now),
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget};
    use crate::fx::{SplashKind, SweepKind};
    use crate::model::{Model, Thresholds};

    fn ready_model() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
        m.apply(Event::Link { target: LinkTarget::Prometheus, up: true }, 0.0);
        m.apply(
            Event::Metrics { cpu_pct: 42.0, mem_pct: 60.0, mem_used_gb: 1.0, mem_total_gb: 8.0, hot_cpu: None, hot_mem: None },
            0.0,
        );
        m.apply(Event::PodSnapshot { running: 10, pending: 0, failed: 0, total: 10 }, 0.0);
        m.apply(Event::NodeSnapshot { ready: 2, total: 2, not_ready: vec![] }, 0.0);
        m
    }

    fn icons(s: &Scene) -> Vec<(&'static str, f32, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, alpha, scale, .. } => Some((*name, *alpha, *scale)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn connecting_when_api_down() {
        let m = Model::new(Thresholds::default());
        let s = m.scene(Role::Cpu, 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "plug-zap"));
    }

    #[test]
    fn no_data_until_metrics_arrive() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
        let s = m.scene(Role::Cpu, 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cloud-off"));
        let s = m.scene(Role::Pods, 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cloud-off"));
    }

    #[test]
    fn splash_swaps_icon_with_overshoot_and_ripple() {
        let mut m = ready_model();
        m.apply(Event::PodCrashed { ns: "a".into(), name: "b".into() }, 1.0);
        m.tick(1.0);
        let s = m.scene(Role::Pods, 1.15);
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Ripple { .. })));
        let s = m.scene(Role::Pods, 1.35);
        let ic = icons(&s);
        let (_, role_alpha, _) = ic.iter().find(|(n, ..)| *n == "box").unwrap();
        let (_, ev_alpha, ev_scale) = ic.iter().find(|(n, ..)| *n == "package-x").unwrap();
        assert!(*role_alpha < 0.6);
        assert!(*ev_alpha > 0.5);
        assert!(*ev_scale > 1.0, "spring overshoot around 60% of swap");
        let s = m.scene(Role::Pods, 3.6);
        assert!(icons(&s).iter().all(|(n, ..)| *n != "package-x"));
    }

    #[test]
    fn splash_counter_in_badge() {
        let mut m = ready_model();
        for _ in 0..3 {
            m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 1.0);
        }
        m.tick(1.0);
        let s = m.scene(Role::Pods, 1.5);
        let text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("+3"));
    }

    #[test]
    fn sweep_takes_over_all_screens_and_suppresses_splash() {
        let mut m = ready_model();
        m.apply(Event::PodStarted { ns: "a".into(), name: "b".into() }, 1.0);
        m.apply(Event::NodeReady { name: "n".into(), ready: false }, 1.0);
        m.tick(1.0);
        let s = m.scene(Role::Health, 1.1);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "server-off"));
        assert!(s.lit_count() > 0 && s.lit_count() < 60);
        let s = m.scene(Role::Cpu, 1.1);
        assert!(icons(&s).iter().all(|(n, ..)| *n == "cpu"), "cpu not yet reached, shows role, no splash");
        let s = m.scene(Role::Cpu, 2.0);
        assert_eq!(s.lit_count(), 60);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "server-off"));
        let s = m.scene(Role::Pods, 2.0);
        assert!(icons(&s).iter().all(|(n, ..)| *n != "package-plus"));
        // tick at ~30 Hz like the real loop so the frozen splash is shifted correctly
        for i in 33..=200 {
            m.tick(i as f64 / 33.0);
        }
        let s = m.scene(Role::Pods, 200.0 / 33.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "package-plus"), "splash resumes after sweep");
    }

    #[test]
    fn boot_uses_role_colours() {
        let sw = Sweep { kind: SweepKind::Boot, started: 0.0 };
        let s = sweep_scene(&sw, Role::Mem, SweepPhase::Hold(0.5), 0.0);
        if let Some(Drawable::Ring { states, .. }) = s.items.iter().find(|d| matches!(d, Drawable::Ring { .. })) {
            assert!(matches!(states[0], SegState::On(c, _) if c == Role::Mem.accent()));
        }
        assert!(icons(&s).iter().any(|(n, ..)| *n == "memory-stick"));
    }

    #[test]
    fn splash_kinds_have_icons() {
        for k in [SplashKind::PodStarted, SplashKind::PodCrashed, SplashKind::PodGone, SplashKind::HotNode, SplashKind::TorrentAdded] {
            assert!(!k.icon().is_empty());
        }
    }
}
```

`crates/core/src/lib.rs` add `pub mod scene_fx;`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p rackscreen-core`
Expected: all pass (43 tests).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(core): splash overlay, sweep scenes and Model::scene"
```

---

### Task 9: Frame orientation, dirty rect, RGB565 packing

**Files:**
- Create: `crates/render/src/frame.rs`
- Modify: `crates/render/src/lib.rs`

**Interfaces:**
- Produces:
  - `frame::Rect { x, y, w, h: u32 }` with `Rect::full()` (240x240), `.union(other)`, `.is_empty()`
  - `frame::new_pixmap() -> Pixmap` (240x240, black)
  - `frame::Orient::new(rotate_cw: u32, hflip: bool) -> Orient`, `Orient::identity()`, `Orient::new_sized(size: u32, rotate_cw, hflip)`, `.apply(&self, src: &Pixmap, dst: &mut Pixmap)`
  - `frame::dirty_rect(prev: &Pixmap, next: &Pixmap) -> Option<Rect>`
  - `frame::pack_rgb565(px: &Pixmap, rect: Rect, brightness: f32) -> Vec<u8>` (big-endian, row-major within rect)

- [ ] **Step 1: Write frame.rs with tests**

```rust
//! Frame buffers, per-screen orientation, dirty rectangles and RGB565 packing.

use rackscreen_core::theme::layout::SIZE;
use tiny_skia::Pixmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn full() -> Rect {
        Rect { x: 0, y: 0, w: SIZE, h: SIZE }
    }
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
    pub fn union(self, o: Rect) -> Rect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w).max(o.x + o.w);
        let y1 = (self.y + self.h).max(o.y + o.h);
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

pub fn new_pixmap() -> Pixmap {
    let mut p = Pixmap::new(SIZE, SIZE).expect("pixmap");
    p.fill(tiny_skia::Color::BLACK);
    p
}

/// Maps destination pixels to source pixels for a rotation (clockwise, multiples of 90)
/// followed by an optional horizontal flip.
pub struct Orient {
    size: u32,
    map: Vec<u32>,
}

impl Orient {
    pub fn identity() -> Orient {
        Orient::new_sized(SIZE, 0, false)
    }

    pub fn new(rotate_cw: u32, hflip: bool) -> Orient {
        Orient::new_sized(SIZE, rotate_cw, hflip)
    }

    pub fn new_sized(size: u32, rotate_cw: u32, hflip: bool) -> Orient {
        let n = size;
        let mut map = Vec::with_capacity((n * n) as usize);
        for y in 0..n {
            for x in 0..n {
                let (rx, ry) = if hflip { (n - 1 - x, y) } else { (x, y) };
                let (sx, sy) = match rotate_cw % 360 {
                    0 => (rx, ry),
                    90 => (ry, n - 1 - rx),
                    180 => (n - 1 - rx, n - 1 - ry),
                    270 => (n - 1 - ry, rx),
                    other => panic!("rotate must be a multiple of 90, got {other}"),
                };
                map.push(sy * n + sx);
            }
        }
        Orient { size: n, map }
    }

    pub fn is_identity(&self) -> bool {
        self.map.iter().enumerate().all(|(i, &m)| i as u32 == m)
    }

    pub fn apply(&self, src: &Pixmap, dst: &mut Pixmap) {
        debug_assert_eq!(src.width(), self.size);
        let s = src.data();
        let d = dst.data_mut();
        for (i, &m) in self.map.iter().enumerate() {
            let (di, si) = (i * 4, m as usize * 4);
            d[di..di + 4].copy_from_slice(&s[si..si + 4]);
        }
    }
}

/// Smallest rectangle covering every pixel that differs. None when identical.
pub fn dirty_rect(prev: &Pixmap, next: &Pixmap) -> Option<Rect> {
    let w = prev.width() as usize;
    let h = prev.height() as usize;
    let a = prev.data();
    let b = next.data();
    let mut y0 = usize::MAX;
    let mut y1 = 0usize;
    let mut x0 = usize::MAX;
    let mut x1 = 0usize;
    for y in 0..h {
        let ra = &a[y * w * 4..(y + 1) * w * 4];
        let rb = &b[y * w * 4..(y + 1) * w * 4];
        if ra == rb {
            continue;
        }
        y0 = y0.min(y);
        y1 = y;
        let differs = |x: usize| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4];
        let first = (0..w).find(|&x| differs(x)).unwrap();
        let last = (0..w).rev().find(|&x| differs(x)).unwrap();
        x0 = x0.min(first);
        x1 = x1.max(last);
    }
    if y0 == usize::MAX {
        return None;
    }
    Some(Rect { x: x0 as u32, y: y0 as u32, w: (x1 - x0 + 1) as u32, h: (y1 - y0 + 1) as u32 })
}

/// Pack the given rectangle as big-endian RGB565 with a brightness multiplier.
pub fn pack_rgb565(px: &Pixmap, rect: Rect, brightness: f32) -> Vec<u8> {
    let w = px.width() as usize;
    let data = px.data();
    let mut out = Vec::with_capacity((rect.w * rect.h * 2) as usize);
    let k = brightness.clamp(0.0, 1.0);
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            let i = (y as usize * w + x as usize) * 4;
            let r = (data[i] as f32 * k) as u16;
            let g = (data[i + 1] as f32 * k) as u16;
            let b = (data[i + 2] as f32 * k) as u16;
            let v = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
            out.push((v >> 8) as u8);
            out.push((v & 0xFF) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(p: &mut Pixmap, x: u32, y: u32, rgb: (u8, u8, u8)) {
        let w = p.width() as usize;
        let i = (y as usize * w + x as usize) * 4;
        let d = p.data_mut();
        d[i] = rgb.0;
        d[i + 1] = rgb.1;
        d[i + 2] = rgb.2;
        d[i + 3] = 255;
    }

    fn get(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let w = p.width() as usize;
        let i = (y as usize * w + x as usize) * 4;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    fn marked(size: u32) -> Pixmap {
        let mut p = Pixmap::new(size, size).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        px(&mut p, 0, 0, (255, 0, 0)); // top-left red
        px(&mut p, size - 1, 0, (0, 255, 0)); // top-right green
        p
    }

    #[test]
    fn rotate_90_cw_moves_top_left_to_top_right() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 90, false).apply(&src, &mut dst);
        assert_eq!(get(&dst, 2, 0), (255, 0, 0));
        assert_eq!(get(&dst, 2, 2), (0, 255, 0));
    }

    #[test]
    fn rotate_270_cw_moves_top_left_to_bottom_left() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 270, false).apply(&src, &mut dst);
        assert_eq!(get(&dst, 0, 2), (255, 0, 0));
        assert_eq!(get(&dst, 0, 0), (0, 255, 0));
    }

    #[test]
    fn hflip_after_rotation() {
        let src = marked(3);
        let mut dst = Pixmap::new(3, 3).unwrap();
        Orient::new_sized(3, 0, true).apply(&src, &mut dst);
        assert_eq!(get(&dst, 2, 0), (255, 0, 0));
        Orient::new_sized(3, 180, true).apply(&src, &mut dst);
        assert_eq!(get(&dst, 0, 2), (255, 0, 0));
    }

    #[test]
    fn identity_detected() {
        assert!(Orient::new_sized(4, 0, false).is_identity());
        assert!(!Orient::new_sized(4, 90, false).is_identity());
    }

    #[test]
    fn dirty_rect_bounds_changes() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        assert_eq!(dirty_rect(&a, &b), None);
        px(&mut b, 10, 20, (1, 2, 3));
        px(&mut b, 15, 25, (4, 5, 6));
        assert_eq!(dirty_rect(&a, &b), Some(Rect { x: 10, y: 20, w: 6, h: 6 }));
    }

    #[test]
    fn pack_known_colours() {
        let mut p = Pixmap::new(2, 2).unwrap();
        px(&mut p, 0, 0, (255, 0, 0));
        px(&mut p, 1, 0, (0, 255, 0));
        px(&mut p, 0, 1, (0, 0, 255));
        px(&mut p, 1, 1, (255, 255, 255));
        let out = pack_rgb565(&p, Rect { x: 0, y: 0, w: 2, h: 2 }, 1.0);
        assert_eq!(out, vec![0xF8, 0x00, 0x07, 0xE0, 0x00, 0x1F, 0xFF, 0xFF]);
        let half = pack_rgb565(&p, Rect { x: 0, y: 0, w: 1, h: 1 }, 0.5);
        assert_eq!(half, vec![0x78, 0x00]);
        let sub = pack_rgb565(&p, Rect { x: 1, y: 1, w: 1, h: 1 }, 1.0);
        assert_eq!(sub, vec![0xFF, 0xFF]);
    }

    #[test]
    fn rect_union() {
        let a = Rect { x: 0, y: 0, w: 2, h: 2 };
        let b = Rect { x: 5, y: 5, w: 1, h: 1 };
        assert_eq!(a.union(b), Rect { x: 0, y: 0, w: 6, h: 6 });
    }
}
```

`crates/render/src/lib.rs`:
```rust
//! Software rasterizer for RackScreen scenes.
pub mod assets;
pub mod frame;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p rackscreen-render frame`
Expected: 7 passed.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(render): orientation, dirty rect and RGB565 packing"
```

---

### Task 10: Drawing primitives, text, icons

**Files:**
- Create: `crates/render/src/prims.rs`, `crates/render/src/text.rs`, `crates/render/src/icons.rs`
- Modify: `crates/render/src/lib.rs`

**Interfaces:**
- Consumes: `assets::{FONT_BOLD, icon_svg, ICON_NAMES}`, `rackscreen_core::theme::Color`
- Produces:
  - `prims::skia_color(c: Color, alpha: f32) -> tiny_skia::Color`, `prims::paint(c, alpha) -> Paint<'static>`
  - `prims::fill(px: &mut Pixmap, path: &Path, c: Color, alpha: f32)`
  - `prims::segment_outline(cx, cy, radius, angle_deg, len, width) -> Path`
  - `prims::SegmentCache::new()`, `.segments(&mut self, cx, cy, radius, n) -> &[Path]`
  - `prims::rounded_rect(x, y, w, h, r) -> Path`, `prims::outline(path: &Path, width: f32) -> Option<Path>`, `prims::circle(cx, cy, r) -> Path`, `prims::circle_stroke(cx, cy, r, thickness) -> Option<Path>`
  - `text::TextRenderer::new() -> Result<TextRenderer>`, `.draw_centered(&mut self, px: &mut Pixmap, text: &str, size_px: f32, cx: f32, cy: f32, color: Color, alpha: f32)`, `.measure(&mut self, text, size_px) -> (f32 width, f32 height)`
  - `icons::IconCache::new() -> Result<IconCache>`, `.draw(&self, px: &mut Pixmap, name: &str, cx, cy, size, color, alpha, scale, dy)`, `.has(name) -> bool`

- [ ] **Step 1: Write prims.rs with tests**

```rust
//! tiny-skia helpers: colours, paths for segments, badges, ripples, dots.

use std::collections::HashMap;

use rackscreen_core::theme::Color;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

pub fn skia_color(c: Color, alpha: f32) -> tiny_skia::Color {
    let a = (c.a as f32 / 255.0) * alpha.clamp(0.0, 1.0);
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, (a * 255.0).round() as u8)
}

pub fn paint(c: Color, alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(skia_color(c, alpha));
    p.anti_alias = true;
    p
}

pub fn fill(px: &mut Pixmap, path: &Path, c: Color, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    px.fill_path(path, &paint(c, alpha), FillRule::Winding, Transform::identity(), None);
}

/// Stroke a path into a fillable outline.
pub fn outline(path: &Path, width: f32) -> Option<Path> {
    let stroke = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
    path.stroke(&stroke, 1.0)
}

/// One ring segment: a radial tick centred on `radius`, at `angle_deg` clockwise from 12 o'clock.
pub fn segment_outline(cx: f32, cy: f32, radius: f32, angle_deg: f32, len: f32, width: f32) -> Path {
    let a = angle_deg.to_radians();
    let (s, c) = (a.sin(), a.cos());
    let r0 = radius - len / 2.0;
    let r1 = radius + len / 2.0;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r0 * s, cy - r0 * c);
    pb.line_to(cx + r1 * s, cy - r1 * c);
    let line = pb.finish().expect("segment path");
    outline(&line, width).expect("segment outline")
}

pub struct SegmentCache {
    map: HashMap<(u32, u32, u32, usize), Vec<Path>>,
}

impl SegmentCache {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn segments(&mut self, cx: f32, cy: f32, radius: f32, n: usize) -> &[Path] {
        use rackscreen_core::theme::layout::{SEG_LEN, SEG_W};
        let key = ((cx * 10.0) as u32, (cy * 10.0) as u32, (radius * 10.0) as u32, n);
        self.map
            .entry(key)
            .or_insert_with(|| {
                (0..n)
                    .map(|i| segment_outline(cx, cy, radius, i as f32 * 360.0 / n as f32, SEG_LEN, SEG_W))
                    .collect()
            })
            .as_slice()
    }
}

impl Default for SegmentCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.5523 * r;
    let (x1, y1) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x1 - r, y);
    pb.cubic_to(x1 - r + k, y, x1, y + r - k, x1, y + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x + r, y1);
    pb.cubic_to(x + r - k, y1, x, y1 - r + k, x, y1 - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish().expect("rounded rect")
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Path {
    PathBuilder::from_circle(cx, cy, r.max(0.01)).expect("circle")
}

pub fn circle_stroke(cx: f32, cy: f32, r: f32, thickness: f32) -> Option<Path> {
    if r < 0.5 {
        return None;
    }
    outline(&circle(cx, cy, r), thickness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::{AMBER, WHITE};

    fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let i = (y as usize * p.width() as usize + x as usize) * 4;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    #[test]
    fn segment_zero_is_at_top() {
        let mut px = Pixmap::new(240, 240).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let seg = segment_outline(120.0, 120.0, 102.0, 0.0, 12.0, 4.0);
        fill(&mut px, &seg, AMBER, 1.0);
        let (r, g, b) = pixel(&px, 120, 18);
        assert!(r > 200 && g > 140 && b < 80, "got {r},{g},{b}");
        assert_eq!(pixel(&px, 120, 222), (0, 0, 0));
    }

    #[test]
    fn segment_cache_reuses_and_counts() {
        let mut c = SegmentCache::new();
        let n1 = c.segments(120.0, 120.0, 102.0, 60).len();
        let n2 = c.segments(120.0, 120.0, 102.0, 60).len();
        assert_eq!((n1, n2), (60, 60));
        assert_eq!(c.map.len(), 1);
        c.segments(120.0, 120.0, 86.0, 51);
        assert_eq!(c.map.len(), 2);
    }

    #[test]
    fn rounded_rect_fills_centre_not_corner() {
        let mut px = Pixmap::new(100, 100).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let rr = rounded_rect(10.0, 10.0, 64.0, 26.0, 7.0);
        fill(&mut px, &rr, WHITE, 1.0);
        assert_eq!(pixel(&px, 42, 23), (255, 255, 255));
        assert_eq!(pixel(&px, 10, 10), (0, 0, 0), "corner is rounded away");
    }

    #[test]
    fn ripple_is_hollow() {
        let mut px = Pixmap::new(100, 100).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let ring = circle_stroke(50.0, 50.0, 30.0, 3.0).unwrap();
        fill(&mut px, &ring, WHITE, 1.0);
        assert_eq!(pixel(&px, 50, 50), (0, 0, 0));
        assert!(pixel(&px, 80, 50).0 > 200);
        assert!(circle_stroke(50.0, 50.0, 0.0, 3.0).is_none());
    }

    #[test]
    fn alpha_blends_toward_black() {
        let mut px = Pixmap::new(10, 10).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        fill(&mut px, &circle(5.0, 5.0, 4.0), WHITE, 0.5);
        let (r, ..) = pixel(&px, 5, 5);
        assert!((120..=136).contains(&r), "got {r}");
    }
}
```

- [ ] **Step 2: Write text.rs with tests**

```rust
//! Glyph rasterizing with fontdue and centred text placement.

use std::collections::HashMap;

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings, Metrics};
use rackscreen_core::theme::Color;
use tiny_skia::Pixmap;

use crate::assets::FONT_BOLD;

pub struct TextRenderer {
    font: Font,
    cache: HashMap<(char, u32), (Metrics, Vec<u8>)>,
}

struct Glyph {
    x: i32,
    top: i32,
    metrics: Metrics,
    bitmap: Vec<u8>,
}

impl TextRenderer {
    pub fn new() -> Result<Self> {
        let font = Font::from_bytes(FONT_BOLD, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("font: {e}"))
            .context("load embedded font")?;
        Ok(Self { font, cache: HashMap::new() })
    }

    fn glyph(&mut self, ch: char, px: f32) -> (Metrics, Vec<u8>) {
        let key = (ch, (px * 4.0) as u32);
        if let Some(g) = self.cache.get(&key) {
            return g.clone();
        }
        let g = self.font.rasterize(ch, px);
        self.cache.insert(key, g.clone());
        g
    }

    /// Lay out glyphs with the baseline at y=0. Returns glyphs and (width, top, bottom).
    fn layout(&mut self, text: &str, px: f32) -> (Vec<Glyph>, f32, i32, i32) {
        let mut x = 0.0f32;
        let mut glyphs = Vec::new();
        let mut top = i32::MAX;
        let mut bottom = i32::MIN;
        for ch in text.chars() {
            let (m, bitmap) = self.glyph(ch, px);
            let gx = x.round() as i32 + m.xmin;
            let gtop = -(m.height as i32 + m.ymin);
            if m.width > 0 && m.height > 0 {
                top = top.min(gtop);
                bottom = bottom.max(gtop + m.height as i32);
            }
            glyphs.push(Glyph { x: gx, top: gtop, metrics: m, bitmap });
            x += m.advance_width;
        }
        if top == i32::MAX {
            top = 0;
            bottom = 0;
        }
        (glyphs, x, top, bottom)
    }

    pub fn measure(&mut self, text: &str, px: f32) -> (f32, f32) {
        let (_, w, top, bottom) = self.layout(text, px);
        (w, (bottom - top) as f32)
    }

    pub fn draw_centered(&mut self, pix: &mut Pixmap, text: &str, px: f32, cx: f32, cy: f32, color: Color, alpha: f32) {
        if alpha <= 0.0 || text.is_empty() {
            return;
        }
        let (glyphs, width, top, bottom) = self.layout(text, px);
        let x0 = (cx - width / 2.0).round() as i32;
        let baseline = (cy - (top + bottom) as f32 / 2.0).round() as i32;
        let pw = pix.width() as i32;
        let ph = pix.height() as i32;
        let data = pix.data_mut();
        for g in glyphs {
            for gy in 0..g.metrics.height as i32 {
                let y = baseline + g.top + gy;
                if y < 0 || y >= ph {
                    continue;
                }
                for gx in 0..g.metrics.width as i32 {
                    let x = x0 + g.x + gx;
                    if x < 0 || x >= pw {
                        continue;
                    }
                    let cov = g.bitmap[(gy as usize) * g.metrics.width + gx as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = (cov as f32 / 255.0) * alpha * (color.a as f32 / 255.0);
                    let i = ((y * pw + x) * 4) as usize;
                    blend(&mut data[i..i + 4], color, a);
                }
            }
        }
    }
}

/// Source-over onto an opaque pixel.
fn blend(dst: &mut [u8], c: Color, a: f32) {
    let a = a.clamp(0.0, 1.0);
    let mix = |d: u8, s: u8| (d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
    dst[0] = mix(dst[0], c.r);
    dst[1] = mix(dst[1], c.g);
    dst[2] = mix(dst[2], c.b);
    dst[3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::WHITE;

    fn lit_bbox(p: &Pixmap) -> Option<(u32, u32, u32, u32)> {
        let w = p.width();
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..p.height() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if p.data()[i] > 40 {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        b
    }

    #[test]
    fn text_is_centred() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(120, 60).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_centered(&mut p, "42%", 15.0, 60.0, 30.0, WHITE, 1.0);
        let (x0, y0, x1, y1) = lit_bbox(&p).expect("something drawn");
        let cx = (x0 + x1) as f32 / 2.0;
        let cy = (y0 + y1) as f32 / 2.0;
        assert!((cx - 60.0).abs() <= 1.5, "cx {cx}");
        assert!((cy - 30.0).abs() <= 1.5, "cy {cy}");
        assert!(y1 - y0 >= 9 && y1 - y0 <= 14, "height {}", y1 - y0);
    }

    #[test]
    fn measure_grows_with_text() {
        let mut t = TextRenderer::new().unwrap();
        let (w1, _) = t.measure("4", 15.0);
        let (w3, _) = t.measure("444", 15.0);
        assert!(w3 > w1 * 2.5);
    }

    #[test]
    fn zero_alpha_draws_nothing() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(50, 50).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_centered(&mut p, "9", 15.0, 25.0, 25.0, WHITE, 0.0);
        assert!(lit_bbox(&p).is_none());
    }
}
```

- [ ] **Step 3: Write icons.rs with tests**

```rust
//! Lucide icons parsed once with usvg, drawn as strokes in any colour, size and scale.

use std::collections::HashMap;

use anyhow::{Context, Result};
use rackscreen_core::theme::Color;
use tiny_skia::{FillRule, LineCap, LineJoin, Path, Pixmap, Stroke, Transform};

use crate::assets::{icon_svg, ICON_NAMES};
use crate::prims::paint;

const ICON_UNITS: f32 = 24.0;

struct IconPaths {
    strokes: Vec<(Path, f32)>,
    fills: Vec<Path>,
}

pub struct IconCache {
    icons: HashMap<&'static str, IconPaths>,
}

fn collect(group: &usvg::Group, out: &mut IconPaths) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, out),
            usvg::Node::Path(p) => {
                let Some(path) = p.data().clone().transform(p.abs_transform()) else { continue };
                if let Some(st) = p.stroke() {
                    out.strokes.push((path.clone(), st.width().get()));
                }
                if p.fill().is_some() {
                    out.fills.push(path);
                }
            }
            _ => {}
        }
    }
}

fn load(svg: &[u8]) -> Result<IconPaths> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut out = IconPaths { strokes: Vec::new(), fills: Vec::new() };
    collect(tree.root(), &mut out);
    anyhow::ensure!(!out.strokes.is_empty() || !out.fills.is_empty(), "icon has no paths");
    Ok(out)
}

impl IconCache {
    pub fn new() -> Result<Self> {
        let mut icons = HashMap::new();
        for name in ICON_NAMES {
            let svg = icon_svg(name).context(*name)?;
            icons.insert(*name, load(svg).with_context(|| format!("icon {name}"))?);
        }
        Ok(Self { icons })
    }

    pub fn has(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    /// Draw `name` centred at (cx, cy + dy), `size` px across at scale 1.0.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, px: &mut Pixmap, name: &str, cx: f32, cy: f32, size: f32, color: Color, alpha: f32, scale: f32, dy: f32) {
        if alpha <= 0.0 || scale <= 0.0 {
            return;
        }
        let Some(icon) = self.icons.get(name) else { return };
        let k = scale * size / ICON_UNITS;
        let ts = Transform::from_translate(cx, cy + dy).pre_scale(k, k).pre_translate(-ICON_UNITS / 2.0, -ICON_UNITS / 2.0);
        let p = paint(color, alpha);
        for (path, width) in &icon.strokes {
            let stroke = Stroke { width: *width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
            px.stroke_path(path, &p, &stroke, ts, None);
        }
        for path in &icon.fills {
            px.fill_path(path, &p, FillRule::Winding, ts, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::WHITE;

    fn lit_bbox(p: &Pixmap) -> Option<(u32, u32, u32, u32)> {
        let w = p.width();
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..p.height() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if p.data()[i] > 40 {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        b
    }

    #[test]
    fn all_icons_load() {
        let c = IconCache::new().unwrap();
        for n in ICON_NAMES {
            assert!(c.has(n), "{n}");
        }
        assert!(!c.has("nope"));
    }

    #[test]
    fn cpu_icon_is_centred_and_sized() {
        let c = IconCache::new().unwrap();
        let mut p = Pixmap::new(240, 240).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        c.draw(&mut p, "cpu", 120.0, 98.0, 72.0, WHITE, 1.0, 1.0, 0.0);
        let (x0, y0, x1, y1) = lit_bbox(&p).unwrap();
        let cx = (x0 + x1) as f32 / 2.0;
        let cy = (y0 + y1) as f32 / 2.0;
        assert!((cx - 120.0).abs() <= 1.5, "cx {cx}");
        assert!((cy - 98.0).abs() <= 1.5, "cy {cy}");
        let w = (x1 - x0) as f32;
        assert!(w > 56.0 && w <= 74.0, "width {w}");
    }

    #[test]
    fn scale_and_dy_apply() {
        let c = IconCache::new().unwrap();
        let mut a = Pixmap::new(240, 240).unwrap();
        a.fill(tiny_skia::Color::BLACK);
        c.draw(&mut a, "box", 120.0, 120.0, 72.0, WHITE, 1.0, 1.0, 0.0);
        let mut b = Pixmap::new(240, 240).unwrap();
        b.fill(tiny_skia::Color::BLACK);
        c.draw(&mut b, "box", 120.0, 120.0, 72.0, WHITE, 1.0, 1.2, 10.0);
        let (ax0, ay0, ax1, _) = lit_bbox(&a).unwrap();
        let (bx0, by0, bx1, _) = lit_bbox(&b).unwrap();
        assert!(bx1 - bx0 > ax1 - ax0, "scaled wider");
        assert!(by0 > ay0, "moved down");
    }
}
```

`crates/render/src/lib.rs`:
```rust
//! Software rasterizer for RackScreen scenes.
pub mod assets;
pub mod frame;
pub mod icons;
pub mod prims;
pub mod text;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rackscreen-render`
Expected: all pass (21 tests). If `text_is_centred` height assertion fails by a pixel or two, widen the range to `8..=16` and note the measured value in the commit message.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(render): segment, badge, text and icon primitives"
```

---

### Task 11: Scene renderer and golden images

**Files:**
- Create: `crates/render/src/renderer.rs`, `crates/render/tests/golden.rs`, `crates/render/tests/goldens/.gitkeep`
- Modify: `crates/render/src/lib.rs`

**Interfaces:**
- Consumes: `rackscreen_core::scene::{Scene, Drawable, SegState}`, prims/text/icons
- Produces: `renderer::Renderer::new() -> Result<Renderer>`, `.render(&mut self, scene: &Scene, px: &mut Pixmap)`

- [ ] **Step 1: Write renderer.rs**

```rust
//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::scene::{Drawable, Scene, SegState};
use rackscreen_core::theme::OFF;
use tiny_skia::Pixmap;

use crate::icons::IconCache;
use crate::prims::{circle, circle_stroke, fill, outline, rounded_rect, skia_color, SegmentCache};
use crate::text::TextRenderer;

pub struct Renderer {
    segs: SegmentCache,
    icons: IconCache,
    text: TextRenderer,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self { segs: SegmentCache::new(), icons: IconCache::new()?, text: TextRenderer::new()? })
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        for d in &scene.items {
            match d {
                Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
                Drawable::Ring { cx, cy, radius, n, states } => {
                    let segs = self.segs.segments(*cx, *cy, *radius, *n);
                    for (path, st) in segs.iter().zip(states) {
                        match st {
                            SegState::Off => fill(px, path, OFF, 1.0),
                            SegState::On(c, a) => {
                                if *a < 1.0 {
                                    fill(px, path, OFF, 1.0);
                                }
                                fill(px, path, *c, *a);
                            }
                        }
                    }
                }
                Drawable::Icon { name, cx, cy, size, color, alpha, scale, dy } => {
                    self.icons.draw(px, name, *cx, *cy, *size, *color, *alpha, *scale, *dy);
                }
                Drawable::Badge { cx, cy, w, h, radius, stroke, fill: fill_c, text, text_px, text_color, alpha } => {
                    let rr = rounded_rect(cx - w / 2.0, cy - h / 2.0, *w, *h, *radius);
                    fill(px, &rr, *fill_c, *alpha);
                    if let Some(border) = outline(&rr, 2.0) {
                        fill(px, &border, *stroke, *alpha);
                    }
                    self.text.draw_centered(px, text, *text_px, *cx, *cy, *text_color, *alpha);
                }
                Drawable::Ripple { cx, cy, r, thickness, color, alpha } => {
                    if let Some(ring) = circle_stroke(*cx, *cy, *r, *thickness) {
                        fill(px, &ring, *color, *alpha);
                    }
                }
                Drawable::Dots { cx, cy, spacing, r, colors } => {
                    let n = colors.len();
                    if n == 0 {
                        continue;
                    }
                    let x0 = cx - (n as f32 - 1.0) * spacing / 2.0;
                    for (i, c) in colors.iter().enumerate() {
                        let a = c.a as f32 / 255.0;
                        let solid = rackscreen_core::theme::Color { a: 255, ..*c };
                        fill(px, &circle(x0 + i as f32 * spacing, *cy, *r), solid, a);
                    }
                }
            }
        }
    }
}
```

`crates/render/src/lib.rs` add `pub mod renderer;`.

- [ ] **Step 2: Write golden test harness**

`crates/render/tests/golden.rs`:
```rust
use std::path::PathBuf;

use rackscreen_core::event::{Event, LinkTarget, Torrent};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens").join(format!("{name}.png"))
}

/// Compare against the stored golden. Writes it when missing or when UPDATE_GOLDENS is set.
fn check(name: &str, px: &Pixmap) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDENS").is_some() || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, px.encode_png().unwrap()).unwrap();
        eprintln!("wrote golden {}", path.display());
        return;
    }
    let want = Pixmap::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!((want.width(), want.height()), (px.width(), px.height()));
    let bad = want
        .data()
        .chunks(4)
        .zip(px.data().chunks(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > 8))
        .count();
    let total = (px.width() * px.height()) as usize;
    assert!(bad * 200 < total, "{name}: {bad} of {total} pixels differ (run with UPDATE_GOLDENS=1 to accept)");
}

fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    (p.data()[i], p.data()[i + 1], p.data()[i + 2])
}

fn ready_model() -> Model {
    let mut m = Model::new(Thresholds::default());
    m.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
    m.apply(Event::Link { target: LinkTarget::Prometheus, up: true }, 0.0);
    m.apply(
        Event::Metrics { cpu_pct: 42.0, mem_pct: 67.0, mem_used_gb: 10.7, mem_total_gb: 16.0, hot_cpu: None, hot_mem: None },
        0.0,
    );
    m.apply(Event::PodSnapshot { running: 53, pending: 2, failed: 0, total: 55 }, 0.0);
    m.apply(Event::NodeSnapshot { ready: 4, total: 4, not_ready: vec![] }, 0.0);
    m
}

#[test]
fn cpu_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Cpu, 5.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(red > 200 && g > 140 && b < 80, "segment 0 amber, got {red},{g},{b}");
    let (o, ..) = pixel(&px, 120, 222);
    assert!((20..=40).contains(&o), "segment 30 is off, got {o}");
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0));
    check("cpu_idle", &px);
}

#[test]
fn all_roles_idle() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    for role in Role::ALL {
        let mut px = new_pixmap();
        r.render(&m.scene(role, 5.0), &mut px);
        check(&format!("idle_{}", role.icon()), &px);
    }
}

#[test]
fn connecting_and_no_data() {
    let mut r = Renderer::new().unwrap();
    let m = Model::new(Thresholds::default());
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Cpu, 0.0), &mut px);
    check("connecting", &px);
    let mut m2 = Model::new(Thresholds::default());
    m2.apply(Event::Link { target: LinkTarget::K8sApi, up: true }, 0.0);
    let mut px2 = new_pixmap();
    r.render(&m2.scene(Role::Cpu, 0.5), &mut px2);
    check("no_data", &px2);
}

#[test]
fn pod_crash_splash_mid_swap() {
    let mut m = ready_model();
    m.apply(Event::PodCrashed { ns: "a".into(), name: "b".into() }, 5.0);
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Pods, 5.35), &mut px);
    check("pods_splash", &px);
}

#[test]
fn torrent_mode() {
    let mut m = ready_model();
    m.apply(Event::Link { target: LinkTarget::QBittorrent, up: true }, 0.0);
    m.apply(
        Event::Torrents(vec![
            Torrent { name: "a".into(), progress: 78.0, eta_secs: 900, speed_bps: 9_000_000 },
            Torrent { name: "b".into(), progress: 41.0, eta_secs: 3000, speed_bps: 3_000_000 },
            Torrent { name: "c".into(), progress: 12.0, eta_secs: 9000, speed_bps: 1_000_000 },
        ]),
        0.0,
    );
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Health, 5.0), &mut px);
    check("torrent", &px);
}

#[test]
fn sweep_hold() {
    let mut m = ready_model();
    m.apply(Event::NodeReady { name: "n".into(), ready: false }, 5.0);
    m.tick(5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene(Role::Health, 6.0), &mut px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(red > 140 && g < 120 && b < 120, "red ring at 60% pulse alpha, got {red},{g},{b}");
    check("sweep_hold", &px);
}
```

Create empty `crates/render/tests/goldens/.gitkeep`.

- [ ] **Step 3: Generate goldens, inspect, run again**

Run: `cargo test -p rackscreen-render --test golden`
Expected: all pass, stderr shows `wrote golden ...` for 10 files.

Open the PNGs (`xdg-open crates/render/tests/goldens/cpu_idle.png`, etc.) and confirm they match the approved mockups: black background, segmented ring, white icon upper centre, outlined badge with white digits. If something is wrong, fix the renderer, delete the affected PNG, re-run.

Run again: `cargo test -p rackscreen-render --test golden`
Expected: all pass, no `wrote golden` lines.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(render): scene renderer with golden image tests"
```

---

### Task 12: Display trait, mailbox, GC9A01 driver, simulator

**Files:**
- Create: `crates/display/src/gc9a01.rs`, `crates/display/src/sim.rs`
- Modify: `crates/display/src/lib.rs`

**Interfaces:**
- Consumes: `rackscreen_render::frame::{Rect, pack_rgb565}`
- Produces:
  - `Display` trait: `fn push(&mut self, frame: &Pixmap, dirty: Rect) -> Result<()>; fn sleep(&mut self) -> Result<()>; fn wake(&mut self) -> Result<()>;`
  - `DisplayCmd { Frame(Pixmap, Rect), Sleep, Wake, Quit }`
  - `Mailbox::new()`, `.clone()`, `.put(cmd)`, `.take() -> DisplayCmd`
  - `spawn_display_thread(name: String, display: Box<dyn Display>, mailbox: Mailbox) -> JoinHandle<()>`
  - feature `pi`: `gc9a01::Pins { bus: u8, cs: u8, dc: u8, rst: u8, hz: u32 }`, `gc9a01::Gc9a01::open(pins, chunk: usize, brightness: f32) -> Result<Gc9a01>`, `gc9a01::window_bytes(x0, y0, x1, y1) -> ([u8; 4], [u8; 4])`, `gc9a01::INIT: &[Step]`
  - feature `sim`: `sim::SimHub::new(n: usize, grid: bool, key_tx: Sender<char>) -> Result<(SimHub, Vec<SimPanel>)>`, `SimHub::run(self, stop: Arc<AtomicBool>)`, `SimPanel` implements `Display`

- [ ] **Step 1: Write lib.rs with the trait and mailbox, plus tests**

```rust
//! Display backends and the per-screen mailbox between render loop and display threads.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use tiny_skia::Pixmap;

pub use rackscreen_render::frame::Rect;

#[cfg(feature = "pi")]
pub mod gc9a01;
#[cfg(feature = "sim")]
pub mod sim;

pub trait Display: Send {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> Result<()>;
    fn sleep(&mut self) -> Result<()>;
    fn wake(&mut self) -> Result<()>;
}

#[derive(Debug)]
pub enum DisplayCmd {
    Frame(Pixmap, Rect),
    Sleep,
    Wake,
    Quit,
}

impl DisplayCmd {
    fn is_control(&self) -> bool {
        !matches!(self, DisplayCmd::Frame(..))
    }
}

/// Single-slot mailbox. A newer frame replaces an unconsumed older frame;
/// control commands are never replaced by frames.
#[derive(Clone)]
pub struct Mailbox {
    inner: Arc<(Mutex<Option<DisplayCmd>>, Condvar)>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    pub fn new() -> Self {
        Self { inner: Arc::new((Mutex::new(None), Condvar::new())) }
    }

    pub fn put(&self, cmd: DisplayCmd) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        match (&*slot, &cmd) {
            (Some(existing), DisplayCmd::Frame(..)) if existing.is_control() => return,
            _ => *slot = Some(cmd),
        }
        cv.notify_one();
    }

    pub fn take(&self) -> DisplayCmd {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        loop {
            if let Some(cmd) = slot.take() {
                return cmd;
            }
            slot = cv.wait(slot).unwrap();
        }
    }

    pub fn try_take(&self) -> Option<DisplayCmd> {
        self.inner.0.lock().unwrap().take()
    }
}

pub fn spawn_display_thread(name: String, mut display: Box<dyn Display>, mailbox: Mailbox) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("display-{name}"))
        .spawn(move || loop {
            let result = match mailbox.take() {
                DisplayCmd::Frame(px, rect) => display.push(&px, rect),
                DisplayCmd::Sleep => display.sleep(),
                DisplayCmd::Wake => display.wake(),
                DisplayCmd::Quit => {
                    let _ = display.sleep();
                    return;
                }
            };
            if let Err(e) = result {
                tracing::warn!(screen = %name, "display error: {e:#}");
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        })
        .expect("spawn display thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tag: u8) -> DisplayCmd {
        let mut p = Pixmap::new(2, 2).unwrap();
        p.data_mut()[0] = tag;
        DisplayCmd::Frame(p, Rect { x: 0, y: 0, w: 2, h: 2 })
    }

    #[test]
    fn newer_frame_replaces_older() {
        let mb = Mailbox::new();
        mb.put(frame(1));
        mb.put(frame(2));
        match mb.take() {
            DisplayCmd::Frame(p, _) => assert_eq!(p.data()[0], 2),
            _ => panic!(),
        }
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn control_is_not_replaced_by_frame() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Sleep);
        mb.put(frame(1));
        assert!(matches!(mb.take(), DisplayCmd::Sleep));
        mb.put(frame(1));
        mb.put(DisplayCmd::Quit);
        assert!(matches!(mb.take(), DisplayCmd::Quit));
    }

    #[test]
    fn take_blocks_until_put() {
        let mb = Mailbox::new();
        let mb2 = mb.clone();
        let h = std::thread::spawn(move || mb2.take());
        std::thread::sleep(std::time::Duration::from_millis(50));
        mb.put(DisplayCmd::Wake);
        assert!(matches!(h.join().unwrap(), DisplayCmd::Wake));
    }
}
```

- [ ] **Step 2: Write gc9a01.rs**

```rust
//! GC9A01 240x240 round LCD over SPI, using rppal. Init sequence ported from the Python driver.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use rppal::gpio::{Gpio, OutputPin};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use tiny_skia::Pixmap;

use rackscreen_render::frame::{pack_rgb565, Rect};

use crate::Display;

#[derive(Clone, Copy, Debug)]
pub struct Pins {
    pub bus: u8,
    pub cs: u8,
    pub dc: u8,
    pub rst: u8,
    pub hz: u32,
}

pub enum Step {
    Cmd(u8, &'static [u8]),
    Delay(u64),
}

use Step::{Cmd, Delay};

pub const INIT: &[Step] = &[
    Cmd(0xEF, &[]), Cmd(0xEB, &[0x14]), Cmd(0xFE, &[]), Cmd(0xEF, &[]), Cmd(0xEB, &[0x14]),
    Cmd(0x84, &[0x40]), Cmd(0x85, &[0xFF]), Cmd(0x86, &[0xFF]), Cmd(0x87, &[0xFF]),
    Cmd(0x88, &[0x0A]), Cmd(0x89, &[0x21]), Cmd(0x8A, &[0x00]), Cmd(0x8B, &[0x80]),
    Cmd(0x8C, &[0x01]), Cmd(0x8D, &[0x01]), Cmd(0x8E, &[0xFF]), Cmd(0x8F, &[0xFF]),
    Cmd(0xB6, &[0x00, 0x20]), Cmd(0x36, &[0x08]), Cmd(0x3A, &[0x05]),
    Cmd(0x90, &[0x08, 0x08, 0x08, 0x08]), Cmd(0xBD, &[0x06]), Cmd(0xBC, &[0x00]),
    Cmd(0xFF, &[0x60, 0x01, 0x04]), Cmd(0xC3, &[0x13]), Cmd(0xC4, &[0x13]),
    Cmd(0xC9, &[0x22]), Cmd(0xBE, &[0x11]), Cmd(0xE1, &[0x10, 0x0E]),
    Cmd(0xDF, &[0x21, 0x0C, 0x02]),
    Cmd(0xF0, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]),
    Cmd(0xF1, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]),
    Cmd(0xF2, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]),
    Cmd(0xF3, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]),
    Cmd(0xED, &[0x1B, 0x0B]), Cmd(0xAE, &[0x77]), Cmd(0xCD, &[0x63]),
    Cmd(0x70, &[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03]),
    Cmd(0xE8, &[0x34]),
    Cmd(0x62, &[0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70]),
    Cmd(0x63, &[0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70]),
    Cmd(0x64, &[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07]),
    Cmd(0x66, &[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00]),
    Cmd(0x67, &[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98]),
    Cmd(0x74, &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00]),
    Cmd(0x98, &[0x3E, 0x07]), Cmd(0x35, &[]), Cmd(0x21, &[]), Cmd(0x11, &[]),
    Delay(120), Cmd(0x29, &[]), Delay(20),
];

/// Column/row address bytes for an inclusive window.
pub fn window_bytes(x0: u16, y0: u16, x1: u16, y1: u16) -> ([u8; 4], [u8; 4]) {
    (
        [(x0 >> 8) as u8, (x0 & 0xFF) as u8, (x1 >> 8) as u8, (x1 & 0xFF) as u8],
        [(y0 >> 8) as u8, (y0 & 0xFF) as u8, (y1 >> 8) as u8, (y1 & 0xFF) as u8],
    )
}

pub struct Gc9a01 {
    spi: Spi,
    dc: OutputPin,
    rst: OutputPin,
    chunk: usize,
    brightness: f32,
}

impl Gc9a01 {
    pub fn open(pins: Pins, chunk: usize, brightness: f32) -> Result<Self> {
        let bus = match pins.bus {
            0 => Bus::Spi0,
            1 => Bus::Spi1,
            b => anyhow::bail!("unsupported spi bus {b}"),
        };
        let ss = match pins.cs {
            0 => SlaveSelect::Ss0,
            1 => SlaveSelect::Ss1,
            2 => SlaveSelect::Ss2,
            c => anyhow::bail!("unsupported chip select {c}"),
        };
        let spi = Spi::new(bus, ss, pins.hz, Mode::Mode0).context("open spi")?;
        let gpio = Gpio::new().context("open gpio")?;
        let dc = gpio.get(pins.dc).context("dc pin")?.into_output_low();
        let rst = gpio.get(pins.rst).context("rst pin")?.into_output_high();
        let mut d = Self { spi, dc, rst, chunk: chunk.max(64), brightness };
        d.reset();
        d.init()?;
        d.clear()?;
        Ok(d)
    }

    fn reset(&mut self) {
        self.rst.set_high();
        sleep(Duration::from_millis(10));
        self.rst.set_low();
        sleep(Duration::from_millis(10));
        self.rst.set_high();
        sleep(Duration::from_millis(120));
    }

    fn cmd(&mut self, c: u8, data: &[u8]) -> Result<()> {
        self.dc.set_low();
        self.spi.write(&[c]).context("spi cmd")?;
        if !data.is_empty() {
            self.dc.set_high();
            self.spi.write(data).context("spi cmd data")?;
        }
        Ok(())
    }

    fn data(&mut self, bytes: &[u8]) -> Result<()> {
        self.dc.set_high();
        for part in bytes.chunks(self.chunk) {
            self.spi.write(part).context("spi data")?;
        }
        Ok(())
    }

    fn init(&mut self) -> Result<()> {
        for step in INIT {
            match step {
                Cmd(c, d) => self.cmd(*c, d)?,
                Delay(ms) => sleep(Duration::from_millis(*ms)),
            }
        }
        Ok(())
    }

    fn set_window(&mut self, r: Rect) -> Result<()> {
        let (col, row) = window_bytes(r.x as u16, r.y as u16, (r.x + r.w - 1) as u16, (r.y + r.h - 1) as u16);
        self.cmd(0x2A, &col)?;
        self.cmd(0x2B, &row)?;
        self.cmd(0x2C, &[])
    }

    fn clear(&mut self) -> Result<()> {
        self.set_window(Rect::full())?;
        self.data(&vec![0u8; 240 * 240 * 2])
    }
}

impl Display for Gc9a01 {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> Result<()> {
        if dirty.is_empty() {
            return Ok(());
        }
        let bytes = pack_rgb565(frame, dirty, self.brightness);
        self.set_window(dirty)?;
        self.data(&bytes)
    }

    fn sleep(&mut self) -> Result<()> {
        self.clear()?;
        self.cmd(0x28, &[])?;
        sleep(Duration::from_millis(20));
        self.cmd(0x10, &[])?;
        sleep(Duration::from_millis(120));
        Ok(())
    }

    fn wake(&mut self) -> Result<()> {
        self.cmd(0x11, &[])?;
        sleep(Duration::from_millis(120));
        self.cmd(0x29, &[])?;
        sleep(Duration::from_millis(20));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_bytes_inclusive_big_endian() {
        let (c, r) = window_bytes(0, 0, 239, 239);
        assert_eq!(c, [0, 0, 0, 0xEF]);
        assert_eq!(r, [0, 0, 0, 0xEF]);
        let (c, _) = window_bytes(10, 5, 25, 9);
        assert_eq!(c, [0, 10, 0, 25]);
    }

    #[test]
    fn init_sequence_ends_with_display_on() {
        let cmds: Vec<u8> = INIT.iter().filter_map(|s| if let Cmd(c, _) = s { Some(*c) } else { None }).collect();
        assert_eq!(cmds.first(), Some(&0xEF));
        assert_eq!(cmds.last(), Some(&0x29));
        assert!(cmds.contains(&0x11));
        assert!(matches!(INIT[INIT.len() - 1], Delay(20)));
    }
}
```

- [ ] **Step 3: Write sim.rs**

```rust
//! Desktop simulator: one minifb window showing every panel as a round screen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use tiny_skia::Pixmap;

use rackscreen_render::frame::Rect;

use crate::Display;

const PANEL: usize = 240;
const PAD: usize = 20;
const CELL: usize = PANEL + PAD;
const BG: u32 = 0x0a0a0a;
const BEZEL_A: u32 = 0x141414;
const BEZEL_B: u32 = 0x202020;

pub struct SimHub {
    window: Window,
    buf: Arc<Mutex<Vec<u32>>>,
    w: usize,
    h: usize,
    key_tx: Sender<char>,
}

pub struct SimPanel {
    buf: Arc<Mutex<Vec<u32>>>,
    stride: usize,
    ox: usize,
    oy: usize,
    asleep: bool,
}

fn layout(n: usize, grid: bool) -> (usize, usize, Vec<(usize, usize)>) {
    if grid {
        let cols = 2;
        let rows = n.div_ceil(cols);
        let origins = (0..n).map(|i| (PAD + (i % cols) * CELL, PAD + (i / cols) * CELL)).collect();
        (PAD + cols * CELL, PAD + rows * CELL, origins)
    } else {
        let origins = (0..n).map(|i| (PAD, PAD + i * CELL)).collect();
        (PAD + CELL, PAD + n * CELL, origins)
    }
}

fn draw_bezel(buf: &mut [u32], stride: usize, ox: usize, oy: usize) {
    let c = PANEL as f32 / 2.0 - 0.5;
    for y in 0..PANEL {
        for x in 0..PANEL {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let v = if d <= 120.0 { 0 } else if d <= 125.0 { BEZEL_A } else if d <= 127.0 { BEZEL_B } else { continue };
            buf[(oy + y) * stride + ox + x] = v;
        }
    }
}

impl SimHub {
    pub fn new(n: usize, grid: bool, key_tx: Sender<char>) -> Result<(SimHub, Vec<SimPanel>)> {
        let (w, h, origins) = layout(n, grid);
        let mut init = vec![BG; w * h];
        for &(ox, oy) in &origins {
            draw_bezel(&mut init, w, ox, oy);
        }
        let buf = Arc::new(Mutex::new(init));
        let window = Window::new("RackScreen sim", w, h, WindowOptions::default()).context("open window")?;
        let panels = origins
            .iter()
            .map(|&(ox, oy)| SimPanel { buf: buf.clone(), stride: w, ox, oy, asleep: false })
            .collect();
        Ok((SimHub { window, buf, w, h, key_tx }, panels))
    }

    /// Blocks on the window loop until the window closes, Escape is pressed, or `stop` is set.
    pub fn run(mut self, stop: Arc<AtomicBool>) {
        self.window.set_target_fps(60);
        let mut frames = 0u32;
        let mut last = Instant::now();
        while self.window.is_open() && !self.window.is_key_down(Key::Escape) && !stop.load(Ordering::Relaxed) {
            for key in self.window.get_keys_pressed(KeyRepeat::No) {
                let ch = match key {
                    Key::Key1 => '1',
                    Key::Key2 => '2',
                    Key::Key3 => '3',
                    Key::Key4 => '4',
                    Key::Key5 => '5',
                    Key::Key6 => '6',
                    Key::Key7 => '7',
                    Key::Key8 => '8',
                    Key::T => 't',
                    Key::N => 'n',
                    Key::B => 'b',
                    _ => continue,
                };
                let _ = self.key_tx.send(ch);
            }
            let snapshot = self.buf.lock().unwrap().clone();
            if self.window.update_with_buffer(&snapshot, self.w, self.h).is_err() {
                break;
            }
            frames += 1;
            if last.elapsed() >= Duration::from_secs(1) {
                self.window.set_title(&format!("RackScreen sim  {frames} fps  [1-8 events, t torrent, n night, b boot, Esc quit]"));
                frames = 0;
                last = Instant::now();
            }
        }
        stop.store(true, Ordering::Relaxed);
    }
}

impl SimPanel {
    /// For tests: a panel drawing into a caller-provided buffer.
    pub fn with_buffer(buf: Arc<Mutex<Vec<u32>>>, stride: usize, ox: usize, oy: usize) -> Self {
        Self { buf, stride, ox, oy, asleep: false }
    }

    fn blit(&self, frame: &Pixmap, dirty: Rect) {
        let c = PANEL as f32 / 2.0 - 0.5;
        let data = frame.data();
        let mut buf = self.buf.lock().unwrap();
        for y in dirty.y..dirty.y + dirty.h {
            for x in dirty.x..dirty.x + dirty.w {
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                if d > 120.0 {
                    continue;
                }
                let i = ((y * PANEL as u32 + x) * 4) as usize;
                let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
                buf[(self.oy + y as usize) * self.stride + self.ox + x as usize] = v;
            }
        }
    }
}

impl Display for SimPanel {
    fn push(&mut self, frame: &Pixmap, dirty: Rect) -> anyhow::Result<()> {
        if self.asleep {
            return Ok(());
        }
        self.blit(frame, dirty);
        Ok(())
    }

    fn sleep(&mut self) -> anyhow::Result<()> {
        let mut black = Pixmap::new(PANEL as u32, PANEL as u32).unwrap();
        black.fill(tiny_skia::Color::BLACK);
        self.blit(&black, Rect::full());
        self.asleep = true;
        Ok(())
    }

    fn wake(&mut self) -> anyhow::Result<()> {
        self.asleep = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let (w, h, o) = layout(4, false);
        assert_eq!((w, h), (280, 1060));
        assert_eq!(o[3], (20, 800));
        let (w, h, o) = layout(4, true);
        assert_eq!((w, h), (540, 540));
        assert_eq!(o[3], (280, 280));
    }

    #[test]
    fn panel_masks_to_circle_and_sleeps() {
        let stride = 280;
        let buf = Arc::new(Mutex::new(vec![BG; stride * 280]));
        {
            let mut b = buf.lock().unwrap();
            draw_bezel(&mut b, stride, 20, 20);
        }
        let mut panel = SimPanel::with_buffer(buf.clone(), stride, 20, 20);
        let mut white = Pixmap::new(240, 240).unwrap();
        white.fill(tiny_skia::Color::WHITE);
        panel.push(&white, Rect::full()).unwrap();
        {
            let b = buf.lock().unwrap();
            assert_eq!(b[(20 + 120) * stride + 20 + 120], 0xffffff, "centre is white");
            assert_eq!(b[(20) * stride + 20], BG, "corner outside circle keeps bezel background");
        }
        panel.sleep().unwrap();
        panel.push(&white, Rect::full()).unwrap();
        assert_eq!(buf.lock().unwrap()[(20 + 120) * stride + 20 + 120], 0, "asleep panel ignores frames");
        panel.wake().unwrap();
        panel.push(&white, Rect::full()).unwrap();
        assert_eq!(buf.lock().unwrap()[(20 + 120) * stride + 20 + 120], 0xffffff);
    }
}
```

- [ ] **Step 4: Run tests with both features**

Run: `cargo test -p rackscreen-display --features sim,pi`
Expected: all pass (7 tests). rppal compiles on x86 (it only touches `/dev` at runtime). If minifb fails to link, install `libxkbcommon-devel libwayland-dev libX11-devel` (Fedora: `sudo dnf install libxkbcommon-devel wayland-devel libX11-devel`). If `Window::new(..).context(..)` fails to compile because `minifb::Error` lacks `std::error::Error`, use `.map_err(|e| anyhow::anyhow!("open window: {e:?}"))` instead.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(display): mailbox, GC9A01 driver and minifb simulator"
```

---

### Task 13: Sources crate skeleton and fake source

**Files:**
- Create: `crates/sources/src/fake.rs`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Produces:
  - `rackscreen_sources::EventTx = std::sync::mpsc::Sender<Event>`
  - `rackscreen_sources::SourceCtx { pub tx: EventTx, pub shutdown: CancellationToken }` with `.emit(&self, ev)`, `.emit_all(&self, evs: Vec<Event>)`
  - `fake::FakeCmd` enum + `FakeCmd::from_key(char) -> Option<FakeCmd>`
  - `fake::FakeState::new(seed: u64)`, `.initial() -> Vec<Event>`, `.tick() -> Vec<Event>`, `.command(cmd) -> Vec<Event>`
  - `fake::run_fake(ctx: SourceCtx, cmds: std::sync::mpsc::Receiver<FakeCmd>, seed: u64) -> impl Future<Output = ()>`

- [ ] **Step 1: Write lib.rs**

```rust
//! Data sources: they run on tokio and push `Event`s into a std mpsc channel
//! that the render thread drains with `try_recv`.

use rackscreen_core::event::Event;
use tokio_util::sync::CancellationToken;

pub mod fake;

pub type EventTx = std::sync::mpsc::Sender<Event>;

#[derive(Clone)]
pub struct SourceCtx {
    pub tx: EventTx,
    pub shutdown: CancellationToken,
}

impl SourceCtx {
    pub fn emit(&self, ev: Event) {
        let _ = self.tx.send(ev);
    }
    pub fn emit_all(&self, evs: Vec<Event>) {
        for ev in evs {
            self.emit(ev);
        }
    }
}
```

- [ ] **Step 2: Write fake.rs with tests**

```rust
//! Scripted data for the simulator. Drifts metrics, churns pods, and reacts to keyboard commands.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use rackscreen_core::event::{Event, LinkTarget, Torrent};

use crate::SourceCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCmd {
    PodStarted,
    PodCrashed,
    NodeDown,
    NodeUp,
    AlertToggle,
    TorrentDone,
    LinkDown,
    LinkUp,
    ToggleTorrent,
    ToggleNight,
    Boot,
}

impl FakeCmd {
    pub fn from_key(c: char) -> Option<FakeCmd> {
        Some(match c {
            '1' => FakeCmd::PodStarted,
            '2' => FakeCmd::PodCrashed,
            '3' => FakeCmd::NodeDown,
            '4' => FakeCmd::NodeUp,
            '5' => FakeCmd::AlertToggle,
            '6' => FakeCmd::TorrentDone,
            '7' => FakeCmd::LinkDown,
            '8' => FakeCmd::LinkUp,
            't' => FakeCmd::ToggleTorrent,
            'n' => FakeCmd::ToggleNight,
            'b' => FakeCmd::Boot,
            _ => return None,
        })
    }
}

pub struct FakeState {
    rng: fastrand::Rng,
    cpu: f32,
    mem: f32,
    running: u32,
    pending: u32,
    failed: u32,
    nodes_total: u32,
    nodes_ready: u32,
    alerts: Vec<String>,
    torrents_on: bool,
    torrents: Vec<Torrent>,
    link_up: bool,
    night: Option<bool>,
    ticks: u64,
}

impl FakeState {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: fastrand::Rng::with_seed(seed),
            cpu: 42.0,
            mem: 67.0,
            running: 53,
            pending: 0,
            failed: 0,
            nodes_total: 4,
            nodes_ready: 4,
            alerts: Vec::new(),
            torrents_on: false,
            torrents: Vec::new(),
            link_up: true,
            night: None,
            ticks: 0,
        }
    }

    fn total(&self) -> u32 {
        self.running + self.pending + self.failed
    }

    fn pod_snapshot(&self) -> Event {
        Event::PodSnapshot { running: self.running, pending: self.pending, failed: self.failed, total: self.total() }
    }

    fn node_snapshot(&self) -> Event {
        let not_ready = (self.nodes_ready..self.nodes_total).map(|i| format!("node-{}", i + 1)).collect();
        Event::NodeSnapshot { ready: self.nodes_ready, total: self.nodes_total, not_ready }
    }

    fn metrics(&self) -> Event {
        Event::Metrics {
            cpu_pct: self.cpu,
            mem_pct: self.mem,
            mem_used_gb: 16.0 * self.mem / 100.0,
            mem_total_gb: 16.0,
            hot_cpu: Some((".5".into(), self.cpu + 20.0)),
            hot_mem: Some((".7".into(), self.mem + 5.0)),
        }
    }

    pub fn initial(&self) -> Vec<Event> {
        vec![
            Event::Link { target: LinkTarget::K8sApi, up: true },
            Event::Link { target: LinkTarget::Prometheus, up: true },
            Event::Link { target: LinkTarget::QBittorrent, up: true },
            self.metrics(),
            self.pod_snapshot(),
            self.node_snapshot(),
            Event::AlertSnapshot { firing: self.alerts.clone() },
            Event::Torrents(self.torrents.clone()),
        ]
    }

    /// One second of simulated time.
    pub fn tick(&mut self) -> Vec<Event> {
        self.ticks += 1;
        if !self.link_up {
            return Vec::new();
        }
        self.cpu = (self.cpu + self.rng.f32() * 6.0 - 3.0).clamp(5.0, 98.0);
        self.mem = (self.mem + self.rng.f32() * 2.0 - 1.0).clamp(20.0, 95.0);
        let mut out = vec![self.metrics()];
        if self.pending > 0 && self.rng.f32() < 0.5 {
            self.pending -= 1;
            self.running += 1;
            out.push(Event::PodStarted { ns: "fake".into(), name: format!("pod-{}", self.ticks) });
            out.push(self.pod_snapshot());
        } else if self.ticks % 20 == 0 {
            self.pending += 1;
            out.push(self.pod_snapshot());
        }
        if self.torrents_on {
            for (i, t) in self.torrents.iter_mut().enumerate() {
                t.progress = (t.progress + 0.4 / (i as f32 + 1.0)).min(99.0);
                t.eta_secs = (t.eta_secs - 1).max(1);
            }
            out.push(Event::Torrents(self.torrents.clone()));
        }
        if self.ticks % 5 == 0 {
            out.push(self.node_snapshot());
            out.push(Event::AlertSnapshot { firing: self.alerts.clone() });
        }
        out
    }

    pub fn command(&mut self, cmd: FakeCmd) -> Vec<Event> {
        match cmd {
            FakeCmd::PodStarted => {
                if self.failed > 0 {
                    self.failed -= 1;
                }
                self.running += 1;
                vec![Event::PodStarted { ns: "fake".into(), name: "manual".into() }, self.pod_snapshot()]
            }
            FakeCmd::PodCrashed => {
                self.running = self.running.saturating_sub(1);
                self.failed += 1;
                vec![Event::PodCrashed { ns: "fake".into(), name: "manual".into() }, self.pod_snapshot()]
            }
            FakeCmd::NodeDown => {
                if self.nodes_ready == 0 {
                    return Vec::new();
                }
                self.nodes_ready -= 1;
                let name = format!("node-{}", self.nodes_ready + 1);
                vec![Event::NodeReady { name, ready: false }, self.node_snapshot()]
            }
            FakeCmd::NodeUp => {
                if self.nodes_ready == self.nodes_total {
                    return Vec::new();
                }
                self.nodes_ready += 1;
                let name = format!("node-{}", self.nodes_ready);
                vec![Event::NodeReady { name, ready: true }, self.node_snapshot()]
            }
            FakeCmd::AlertToggle => {
                if self.alerts.is_empty() {
                    self.alerts.push("HighLoad".into());
                    vec![Event::AlertChanged { name: "HighLoad".into(), firing: true }, Event::AlertSnapshot { firing: self.alerts.clone() }]
                } else {
                    self.alerts.clear();
                    vec![Event::AlertChanged { name: "HighLoad".into(), firing: false }, Event::AlertSnapshot { firing: vec![] }]
                }
            }
            FakeCmd::TorrentDone => {
                if self.torrents.is_empty() {
                    return Vec::new();
                }
                let done = self.torrents.remove(0);
                if self.torrents.is_empty() {
                    self.torrents_on = false;
                }
                vec![Event::TorrentDone { name: done.name }, Event::Torrents(self.torrents.clone())]
            }
            FakeCmd::LinkDown => {
                self.link_up = false;
                vec![Event::Link { target: LinkTarget::K8sApi, up: false }]
            }
            FakeCmd::LinkUp => {
                self.link_up = true;
                vec![Event::Link { target: LinkTarget::K8sApi, up: true }]
            }
            FakeCmd::ToggleTorrent => {
                self.torrents_on = !self.torrents_on;
                if self.torrents_on {
                    self.torrents = vec![
                        Torrent { name: "ubuntu-24.04.iso".into(), progress: 78.0, eta_secs: 900, speed_bps: 9_000_000 },
                        Torrent { name: "debian-13.iso".into(), progress: 41.0, eta_secs: 3000, speed_bps: 3_000_000 },
                        Torrent { name: "fedora-44.iso".into(), progress: 12.0, eta_secs: 9000, speed_bps: 1_000_000 },
                    ];
                    vec![Event::TorrentAdded { name: "ubuntu-24.04.iso".into() }, Event::Torrents(self.torrents.clone())]
                } else {
                    self.torrents.clear();
                    vec![Event::Torrents(vec![])]
                }
            }
            FakeCmd::ToggleNight => {
                self.night = match self.night {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                vec![Event::ForceNight(self.night)]
            }
            FakeCmd::Boot => vec![Event::Boot],
        }
    }
}

pub async fn run_fake(ctx: SourceCtx, cmds: Receiver<FakeCmd>, seed: u64) {
    let mut st = FakeState::new(seed);
    ctx.emit_all(st.initial());
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = ticker.tick() => ctx.emit_all(st.tick()),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                while let Ok(cmd) = cmds.try_recv() {
                    ctx.emit_all(st.command(cmd));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map() {
        assert_eq!(FakeCmd::from_key('1'), Some(FakeCmd::PodStarted));
        assert_eq!(FakeCmd::from_key('n'), Some(FakeCmd::ToggleNight));
        assert_eq!(FakeCmd::from_key('x'), None);
    }

    #[test]
    fn initial_brings_links_up() {
        let s = FakeState::new(1);
        let evs = s.initial();
        assert!(evs.iter().any(|e| matches!(e, Event::Link { target: LinkTarget::K8sApi, up: true })));
        assert!(evs.iter().any(|e| matches!(e, Event::PodSnapshot { running: 53, .. })));
    }

    #[test]
    fn tick_is_deterministic_and_emits_metrics() {
        let mut a = FakeState::new(7);
        let mut b = FakeState::new(7);
        for _ in 0..30 {
            assert_eq!(a.tick(), b.tick());
        }
        assert!(matches!(a.tick()[0], Event::Metrics { .. }));
    }

    #[test]
    fn crash_and_recover() {
        let mut s = FakeState::new(1);
        let evs = s.command(FakeCmd::PodCrashed);
        assert!(matches!(evs[0], Event::PodCrashed { .. }));
        assert!(matches!(evs[1], Event::PodSnapshot { failed: 1, running: 52, .. }));
        let evs = s.command(FakeCmd::PodStarted);
        assert!(matches!(evs[1], Event::PodSnapshot { failed: 0, running: 53, .. }));
    }

    #[test]
    fn node_down_up_and_bounds() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::NodeUp).is_empty());
        let evs = s.command(FakeCmd::NodeDown);
        assert!(matches!(&evs[0], Event::NodeReady { ready: false, .. }));
        assert!(matches!(&evs[1], Event::NodeSnapshot { ready: 3, total: 4, .. }));
        let evs = s.command(FakeCmd::NodeUp);
        assert!(matches!(&evs[1], Event::NodeSnapshot { ready: 4, .. }));
    }

    #[test]
    fn torrent_toggle_and_done() {
        let mut s = FakeState::new(1);
        assert!(s.command(FakeCmd::TorrentDone).is_empty());
        let evs = s.command(FakeCmd::ToggleTorrent);
        assert!(matches!(evs[0], Event::TorrentAdded { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 3));
        let evs = s.command(FakeCmd::TorrentDone);
        assert!(matches!(evs[0], Event::TorrentDone { .. }));
        assert!(matches!(&evs[1], Event::Torrents(t) if t.len() == 2));
    }

    #[test]
    fn link_down_silences_ticks() {
        let mut s = FakeState::new(1);
        s.command(FakeCmd::LinkDown);
        assert!(s.tick().is_empty());
        s.command(FakeCmd::LinkUp);
        assert!(!s.tick().is_empty());
    }

    #[test]
    fn night_cycles() {
        let mut s = FakeState::new(1);
        assert_eq!(s.command(FakeCmd::ToggleNight), vec![Event::ForceNight(Some(true))]);
        assert_eq!(s.command(FakeCmd::ToggleNight), vec![Event::ForceNight(Some(false))]);
        assert_eq!(s.command(FakeCmd::ToggleNight), vec![Event::ForceNight(None)]);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rackscreen-sources`
Expected: 8 passed.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(sources): source context and scripted fake source"
```

---

### Task 14: Kubernetes watchers (pods, nodes)

**Files:**
- Create: `crates/sources/src/k8s.rs`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Produces:
  - `k8s::make_client(kubeconfig: &Path) -> Result<kube::Client>`
  - `k8s::Phase { Pending, Running, Succeeded, Failed, Unknown }`, `k8s::PodInfo { phase, restarts: i32, crashloop: bool }`, `k8s::pod_info(&Pod) -> PodInfo`, `k8s::pod_key(&Pod) -> String`
  - `k8s::PodTracker::new()`, `.begin_init()`, `.init_done() -> Vec<Event>`, `.apply(&Pod) -> Vec<Event>`, `.delete(&Pod) -> Vec<Event>`, `.snapshot() -> Event`
  - `k8s::NodeTracker::new()`, same methods for `Node`
  - `k8s::run_pod_watch(client, ctx) -> impl Future`, `k8s::run_node_watch(client, ctx) -> impl Future`

- [ ] **Step 1: Write k8s.rs with tests**

```rust
//! Pod and node watchers via kube-rs. Trackers are pure and unit tested.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use futures::StreamExt;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Api, Client, Config};
use rackscreen_core::event::{Event, LinkTarget};

use crate::SourceCtx;

pub async fn make_client(kubeconfig: &Path) -> Result<Client> {
    let kc = Kubeconfig::read_from(kubeconfig).with_context(|| format!("read {}", kubeconfig.display()))?;
    let cfg = Config::from_custom_kubeconfig(kc, &KubeConfigOptions::default()).await.context("kubeconfig")?;
    Client::try_from(cfg).context("client")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodInfo {
    pub phase: Phase,
    pub restarts: i32,
    pub crashloop: bool,
}

pub fn pod_key(pod: &Pod) -> String {
    pod.metadata.uid.clone().unwrap_or_else(|| {
        format!("{}/{}", pod.metadata.namespace.as_deref().unwrap_or(""), pod.metadata.name.as_deref().unwrap_or(""))
    })
}

pub fn pod_info(pod: &Pod) -> PodInfo {
    let status = pod.status.as_ref();
    let phase = match status.and_then(|s| s.phase.as_deref()) {
        Some("Pending") => Phase::Pending,
        Some("Running") => Phase::Running,
        Some("Succeeded") => Phase::Succeeded,
        Some("Failed") => Phase::Failed,
        _ => Phase::Unknown,
    };
    let mut restarts = 0;
    let mut crashloop = false;
    if let Some(cs) = status.and_then(|s| s.container_statuses.as_ref()) {
        for c in cs {
            restarts += c.restart_count;
            let waiting = c.state.as_ref().and_then(|s| s.waiting.as_ref()).and_then(|w| w.reason.as_deref());
            if waiting == Some("CrashLoopBackOff") {
                crashloop = true;
            }
        }
    }
    PodInfo { phase, restarts, crashloop }
}

#[derive(Default)]
pub struct PodTracker {
    pods: HashMap<String, PodInfo>,
    synced: bool,
}

impl PodTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_init(&mut self) {
        self.pods.clear();
        self.synced = false;
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        let mut running = 0;
        let mut pending = 0;
        let mut failed = 0;
        for p in self.pods.values() {
            if p.crashloop || p.phase == Phase::Failed {
                failed += 1;
            } else if p.phase == Phase::Running {
                running += 1;
            } else if p.phase == Phase::Pending {
                pending += 1;
            }
        }
        Event::PodSnapshot { running, pending, failed, total: self.pods.len() as u32 }
    }

    pub fn apply(&mut self, pod: &Pod) -> Vec<Event> {
        let key = pod_key(pod);
        let new = pod_info(pod);
        let old = self.pods.insert(key, new.clone());
        if !self.synced {
            return Vec::new();
        }
        let ns = pod.metadata.namespace.clone().unwrap_or_default();
        let name = pod.metadata.name.clone().unwrap_or_default();
        let mut out = Vec::new();
        let became_running = new.phase == Phase::Running && !new.crashloop && old.as_ref().is_none_or(|o| o.phase != Phase::Running || o.crashloop);
        let crashed = match &old {
            Some(o) => new.restarts > o.restarts || (new.crashloop && !o.crashloop) || (new.phase == Phase::Failed && o.phase != Phase::Failed),
            None => new.crashloop || new.phase == Phase::Failed,
        };
        if became_running {
            out.push(Event::PodStarted { ns: ns.clone(), name: name.clone() });
        }
        if crashed {
            out.push(Event::PodCrashed { ns, name });
        }
        if old.as_ref() != Some(&new) {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, pod: &Pod) -> Vec<Event> {
        let existed = self.pods.remove(&pod_key(pod)).is_some();
        if !self.synced || !existed {
            return Vec::new();
        }
        vec![
            Event::PodGone {
                ns: pod.metadata.namespace.clone().unwrap_or_default(),
                name: pod.metadata.name.clone().unwrap_or_default(),
            },
            self.snapshot(),
        ]
    }
}

pub fn node_ready(node: &Node) -> bool {
    node.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|cs| cs.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
        .unwrap_or(false)
}

#[derive(Default)]
pub struct NodeTracker {
    nodes: HashMap<String, bool>,
    synced: bool,
}

impl NodeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_init(&mut self) {
        self.nodes.clear();
        self.synced = false;
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        let mut not_ready: Vec<String> = self.nodes.iter().filter(|(_, r)| !**r).map(|(n, _)| n.clone()).collect();
        not_ready.sort();
        Event::NodeSnapshot {
            ready: self.nodes.values().filter(|r| **r).count() as u32,
            total: self.nodes.len() as u32,
            not_ready,
        }
    }

    pub fn apply(&mut self, node: &Node) -> Vec<Event> {
        let name = node.metadata.name.clone().unwrap_or_default();
        let ready = node_ready(node);
        let old = self.nodes.insert(name.clone(), ready);
        if !self.synced {
            return Vec::new();
        }
        let mut out = Vec::new();
        if old.is_some_and(|o| o != ready) {
            out.push(Event::NodeReady { name, ready });
        }
        if old != Some(ready) {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, node: &Node) -> Vec<Event> {
        let name = node.metadata.name.clone().unwrap_or_default();
        if self.nodes.remove(&name).is_none() || !self.synced {
            return Vec::new();
        }
        vec![self.snapshot()]
    }
}

pub async fn run_pod_watch(client: Client, ctx: SourceCtx) {
    let api: Api<Pod> = Api::all(client);
    let mut stream = watcher(api, watcher::Config::default().any_semantic()).default_backoff().boxed();
    let mut tracker = PodTracker::new();
    let mut link_up = false;
    loop {
        let item = tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            item = stream.next() => item,
        };
        let Some(item) = item else { return };
        match item {
            Ok(ev) => {
                let evs = match ev {
                    watcher::Event::Init => {
                        tracker.begin_init();
                        Vec::new()
                    }
                    watcher::Event::InitApply(p) => tracker.apply(&p),
                    watcher::Event::InitDone => {
                        if !link_up {
                            link_up = true;
                            ctx.emit(Event::Link { target: LinkTarget::K8sApi, up: true });
                        }
                        tracker.init_done()
                    }
                    watcher::Event::Apply(p) => tracker.apply(&p),
                    watcher::Event::Delete(p) => tracker.delete(&p),
                };
                ctx.emit_all(evs);
            }
            Err(e) => {
                tracing::warn!("pod watch: {e}");
                if link_up {
                    link_up = false;
                    ctx.emit(Event::Link { target: LinkTarget::K8sApi, up: false });
                }
            }
        }
    }
}

pub async fn run_node_watch(client: Client, ctx: SourceCtx) {
    let api: Api<Node> = Api::all(client);
    let mut stream = watcher(api, watcher::Config::default().any_semantic()).default_backoff().boxed();
    let mut tracker = NodeTracker::new();
    loop {
        let item = tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            item = stream.next() => item,
        };
        let Some(item) = item else { return };
        match item {
            Ok(watcher::Event::Init) => tracker.begin_init(),
            Ok(watcher::Event::InitApply(n)) => ctx.emit_all(tracker.apply(&n)),
            Ok(watcher::Event::InitDone) => ctx.emit_all(tracker.init_done()),
            Ok(watcher::Event::Apply(n)) => ctx.emit_all(tracker.apply(&n)),
            Ok(watcher::Event::Delete(n)) => ctx.emit_all(tracker.delete(&n)),
            Err(e) => tracing::warn!("node watch: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ContainerState, ContainerStateWaiting, ContainerStatus, NodeCondition, NodeStatus, PodStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn pod(name: &str, phase: &str, restarts: i32, waiting: Option<&str>) -> Pod {
        Pod {
            metadata: ObjectMeta { name: Some(name.into()), namespace: Some("ns".into()), uid: Some(format!("uid-{name}")), ..Default::default() },
            status: Some(PodStatus {
                phase: Some(phase.into()),
                container_statuses: Some(vec![ContainerStatus {
                    restart_count: restarts,
                    state: Some(ContainerState {
                        waiting: waiting.map(|r| ContainerStateWaiting { reason: Some(r.into()), ..Default::default() }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn node(name: &str, ready: bool) -> Node {
        Node {
            metadata: ObjectMeta { name: Some(name.into()), ..Default::default() },
            status: Some(NodeStatus {
                conditions: Some(vec![NodeCondition { type_: "Ready".into(), status: if ready { "True" } else { "False" }.into(), ..Default::default() }]),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn pod_info_reads_phase_restarts_crashloop() {
        let i = pod_info(&pod("a", "Running", 3, Some("CrashLoopBackOff")));
        assert_eq!(i, PodInfo { phase: Phase::Running, restarts: 3, crashloop: true });
        assert_eq!(pod_info(&pod("a", "Weird", 0, None)).phase, Phase::Unknown);
    }

    #[test]
    fn initial_sync_is_silent_then_snapshot() {
        let mut t = PodTracker::new();
        t.begin_init();
        assert!(t.apply(&pod("a", "Running", 0, None)).is_empty());
        assert!(t.apply(&pod("b", "Pending", 0, None)).is_empty());
        let evs = t.init_done();
        assert_eq!(evs, vec![Event::PodSnapshot { running: 1, pending: 1, failed: 0, total: 2 }]);
    }

    #[test]
    fn pod_lifecycle_events() {
        let mut t = PodTracker::new();
        t.begin_init();
        t.init_done();
        let evs = t.apply(&pod("a", "Pending", 0, None));
        assert_eq!(evs, vec![Event::PodSnapshot { running: 0, pending: 1, failed: 0, total: 1 }]);
        let evs = t.apply(&pod("a", "Running", 0, None));
        assert_eq!(evs[0], Event::PodStarted { ns: "ns".into(), name: "a".into() });
        let evs = t.apply(&pod("a", "Running", 1, None));
        assert_eq!(evs[0], Event::PodCrashed { ns: "ns".into(), name: "a".into() });
        let evs = t.apply(&pod("a", "Running", 1, Some("CrashLoopBackOff")));
        assert_eq!(evs[0], Event::PodCrashed { ns: "ns".into(), name: "a".into() });
        assert_eq!(evs[1], Event::PodSnapshot { running: 0, pending: 0, failed: 1, total: 1 });
        assert!(t.apply(&pod("a", "Running", 1, Some("CrashLoopBackOff"))).is_empty(), "no change, no events");
        let evs = t.delete(&pod("a", "Running", 1, None));
        assert_eq!(evs[0], Event::PodGone { ns: "ns".into(), name: "a".into() });
        assert_eq!(evs[1], Event::PodSnapshot { running: 0, pending: 0, failed: 0, total: 0 });
        assert!(t.delete(&pod("zzz", "Running", 0, None)).is_empty());
    }

    #[test]
    fn new_running_pod_after_sync_is_started() {
        let mut t = PodTracker::new();
        t.begin_init();
        t.init_done();
        let evs = t.apply(&pod("fresh", "Running", 0, None));
        assert!(matches!(evs[0], Event::PodStarted { .. }));
    }

    #[test]
    fn node_flip() {
        let mut t = NodeTracker::new();
        t.begin_init();
        t.apply(&node("n1", true));
        t.apply(&node("n2", true));
        assert_eq!(t.init_done(), vec![Event::NodeSnapshot { ready: 2, total: 2, not_ready: vec![] }]);
        let evs = t.apply(&node("n2", false));
        assert_eq!(evs[0], Event::NodeReady { name: "n2".into(), ready: false });
        assert_eq!(evs[1], Event::NodeSnapshot { ready: 1, total: 2, not_ready: vec!["n2".into()] });
        assert!(t.apply(&node("n2", false)).is_empty());
        let evs = t.apply(&node("n2", true));
        assert_eq!(evs[0], Event::NodeReady { name: "n2".into(), ready: true });
    }
}
```

`crates/sources/src/lib.rs` add `pub mod k8s;`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p rackscreen-sources k8s`
Expected: 5 passed. If `is_none_or` is unavailable, replace with `map_or(true, |o| ...)`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(sources): pod and node watchers with pure trackers"
```

---

### Task 15: In-process port-forward tunnel and Prometheus poller

**Files:**
- Create: `crates/sources/src/tunnel.rs`, `crates/sources/src/prometheus.rs`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Produces:
  - `tunnel::Tunnel::open(client: &Client, ns: &str, pod: &str, port: u16) -> Result<Tunnel>`, `.get(path: &str, headers: &[(&str, &str)]) -> Result<Response>`, `.post_form(path, body: &str, headers) -> Result<Response>`
  - `tunnel::Response { status: u16, headers: hyper::HeaderMap, body: String }`
  - `tunnel::find_pod_for_service(client, ns, service_hint: &str, port: u16, needle: &str) -> Result<(String, String)>` returns `(pod_name, service_name)`
  - `prometheus::{parse_scalar(&str) -> Option<f64>, parse_vector(&str) -> Vec<(HashMap<String,String>, f64)>, node_tag(&str) -> String}`
  - `prometheus::PromConfig { namespace, service, port, poll_secs }`, `prometheus::run_prometheus(client, cfg, ctx)`

- [ ] **Step 1: Write tunnel.rs**

```rust
//! HTTP/1.1 client over a kube port-forward, so no kubectl binary is needed on the Pi.

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1::{self, SendRequest};
use hyper::{HeaderMap, Request};
use hyper_util::rt::TokioIo;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::api::{ListParams, Portforwarder};
use kube::{Api, Client};

pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: String,
}

pub struct Tunnel {
    send: SendRequest<Full<Bytes>>,
    _pf: Portforwarder,
    port: u16,
}

impl Tunnel {
    pub async fn open(client: &Client, ns: &str, pod: &str, port: u16) -> Result<Tunnel> {
        let pods: Api<Pod> = Api::namespaced(client.clone(), ns);
        let mut pf = pods.portforward(pod, &[port]).await.with_context(|| format!("port-forward {ns}/{pod}:{port}"))?;
        let stream = pf.take_stream(port).context("port-forward stream")?;
        let (send, conn) = http1::handshake(TokioIo::new(stream)).await.context("http handshake")?;
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::debug!("tunnel connection closed: {e}");
            }
        });
        Ok(Tunnel { send, _pf: pf, port })
    }

    fn base(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    async fn send(&mut self, req: Request<Full<Bytes>>) -> Result<Response> {
        self.send.ready().await.context("tunnel not ready")?;
        let resp = self.send.send_request(req).await.context("request")?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.context("body")?.to_bytes();
        Ok(Response { status, headers, body: String::from_utf8_lossy(&bytes).into_owned() })
    }

    pub async fn get(&mut self, path: &str, headers: &[(&str, &str)]) -> Result<Response> {
        let mut b = Request::builder().method("GET").uri(format!("{}{path}", self.base())).header("host", "localhost");
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        self.send(b.body(Full::new(Bytes::new()))?).await
    }

    pub async fn post_form(&mut self, path: &str, body: &str, headers: &[(&str, &str)]) -> Result<Response> {
        let mut b = Request::builder()
            .method("POST")
            .uri(format!("{}{path}", self.base()))
            .header("host", "localhost")
            .header("content-type", "application/x-www-form-urlencoded");
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        self.send(b.body(Full::new(Bytes::from(body.to_owned())))?).await
    }
}

/// Resolve a service (exact name, or "auto" = first service whose name contains `needle`
/// and exposes `port`) to one Running, Ready pod behind it.
pub async fn find_pod_for_service(client: &Client, ns: &str, service_hint: &str, port: u16, needle: &str) -> Result<(String, String)> {
    let svcs: Api<Service> = Api::namespaced(client.clone(), ns);
    let svc = if service_hint == "auto" {
        let list = svcs.list(&ListParams::default()).await.context("list services")?;
        list.items
            .into_iter()
            .find(|s| {
                let name = s.metadata.name.as_deref().unwrap_or("");
                let has_port = s.spec.as_ref().and_then(|sp| sp.ports.as_ref()).is_some_and(|ps| ps.iter().any(|p| p.port == port as i32));
                name.contains(needle) && has_port
            })
            .with_context(|| format!("no service containing '{needle}' with port {port} in {ns}"))?
    } else {
        svcs.get(service_hint).await.with_context(|| format!("service {ns}/{service_hint}"))?
    };
    let svc_name = svc.metadata.name.clone().unwrap_or_default();
    let selector = svc.spec.as_ref().and_then(|s| s.selector.as_ref()).context("service has no selector")?;
    let label = selector.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",");
    let pods: Api<Pod> = Api::namespaced(client.clone(), ns);
    let list = pods.list(&ListParams::default().labels(&label)).await.context("list pods")?;
    let pod = list
        .items
        .iter()
        .find(|p| {
            let st = p.status.as_ref();
            st.and_then(|s| s.phase.as_deref()) == Some("Running")
                && st
                    .and_then(|s| s.conditions.as_ref())
                    .is_some_and(|cs| cs.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
        })
        .with_context(|| format!("no ready pod for service {svc_name}"))?;
    Ok((pod.metadata.name.clone().unwrap_or_default(), svc_name))
}
```

- [ ] **Step 2: Write prometheus.rs with parsing tests**

```rust
//! Prometheus poller: cluster CPU/MEM, hot nodes, firing alerts.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result};
use kube::Client;
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct PromConfig {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub poll_secs: u64,
}

pub fn parse_scalar(json: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(json).ok()?;
    let first = v.get("data")?.get("result")?.as_array()?.first()?;
    first.get("value")?.as_array()?.get(1)?.as_str()?.parse().ok()
}

pub fn parse_vector(json: &str) -> Vec<(HashMap<String, String>, f64)> {
    let Ok(v) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    let Some(items) = v.get("data").and_then(|d| d.get("result")).and_then(|r| r.as_array()) else { return Vec::new() };
    items
        .iter()
        .filter_map(|it| {
            let metric = it.get("metric")?.as_object()?;
            let labels = metric.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
            let val: f64 = it.get("value")?.as_array()?.get(1)?.as_str()?.parse().ok()?;
            Some((labels, val))
        })
        .collect()
}

/// `192.168.1.5:9100` -> `.5`
pub fn node_tag(instance: &str) -> String {
    let ip = instance.split(':').next().unwrap_or(instance);
    match ip.rsplit('.').next() {
        Some(last) if last != ip => format!(".{last}"),
        _ => ip.to_string(),
    }
}

const Q_CPU: &str = "100-avg(rate(node_cpu_seconds_total{mode=\"idle\"}[2m]))*100";
const Q_CPU_BY: &str = "100-avg by(instance)(rate(node_cpu_seconds_total{mode=\"idle\"}[2m]))*100";
const Q_MEM: &str = "(1-sum(node_memory_MemAvailable_bytes)/sum(node_memory_MemTotal_bytes))*100";
const Q_MEM_USED: &str = "(sum(node_memory_MemTotal_bytes)-sum(node_memory_MemAvailable_bytes))/1073741824";
const Q_MEM_TOTAL: &str = "sum(node_memory_MemTotal_bytes)/1073741824";
const Q_MEM_BY: &str = "(1-node_memory_MemAvailable_bytes/node_memory_MemTotal_bytes)*100";
const Q_ALERTS: &str = "ALERTS{alertstate=\"firing\"}";

async fn query(t: &mut Tunnel, q: &str) -> Result<String> {
    let path = format!("/api/v1/query?query={}", urlencoding::encode(q));
    let r = t.get(&path, &[]).await?;
    anyhow::ensure!(r.status == 200, "prometheus HTTP {}", r.status);
    Ok(r.body)
}

fn hottest(v: &[(HashMap<String, String>, f64)]) -> Option<(String, f32)> {
    v.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(m, val)| (node_tag(m.get("instance").map(String::as_str).unwrap_or("")), *val as f32))
}

async fn poll(t: &mut Tunnel, prev_alerts: &mut HashSet<String>) -> Result<Vec<Event>> {
    let cpu = parse_scalar(&query(t, Q_CPU).await?).unwrap_or(0.0);
    let cpu_by = parse_vector(&query(t, Q_CPU_BY).await?);
    let mem = parse_scalar(&query(t, Q_MEM).await?).unwrap_or(0.0);
    let mem_used = parse_scalar(&query(t, Q_MEM_USED).await?).unwrap_or(0.0);
    let mem_total = parse_scalar(&query(t, Q_MEM_TOTAL).await?).unwrap_or(0.0);
    let mem_by = parse_vector(&query(t, Q_MEM_BY).await?);
    let alerts = parse_vector(&query(t, Q_ALERTS).await?);
    let mut out = vec![Event::Metrics {
        cpu_pct: cpu as f32,
        mem_pct: mem as f32,
        mem_used_gb: mem_used as f32,
        mem_total_gb: mem_total as f32,
        hot_cpu: hottest(&cpu_by),
        hot_mem: hottest(&mem_by),
    }];
    let firing: HashSet<String> = alerts.iter().filter_map(|(m, _)| m.get("alertname").cloned()).collect();
    for a in firing.difference(prev_alerts) {
        out.push(Event::AlertChanged { name: a.clone(), firing: true });
    }
    for a in prev_alerts.difference(&firing) {
        out.push(Event::AlertChanged { name: a.clone(), firing: false });
    }
    let mut list: Vec<String> = firing.iter().cloned().collect();
    list.sort();
    out.push(Event::AlertSnapshot { firing: list });
    *prev_alerts = firing;
    Ok(out)
}

pub async fn run_prometheus(client: Client, cfg: PromConfig, ctx: SourceCtx) {
    let mut prev_alerts = HashSet::new();
    let mut backoff = 5u64;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let tunnel = async {
            let (pod, svc) = find_pod_for_service(&client, &cfg.namespace, &cfg.service, cfg.port, "prometheus").await?;
            tracing::info!("prometheus: forwarding to {}/{pod} (service {svc})", cfg.namespace);
            Tunnel::open(&client, &cfg.namespace, &pod, cfg.port).await
        }
        .await;
        let mut t = match tunnel {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("prometheus: {e:#}; retry in {backoff}s");
                ctx.emit(Event::Link { target: LinkTarget::Prometheus, up: false });
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(30);
                continue;
            }
        };
        backoff = 5;
        let mut failures = 0;
        loop {
            match poll(&mut t, &mut prev_alerts).await {
                Ok(evs) => {
                    failures = 0;
                    ctx.emit(Event::Link { target: LinkTarget::Prometheus, up: true });
                    ctx.emit_all(evs);
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("prometheus poll: {e:#}");
                    if failures >= 2 {
                        ctx.emit(Event::Link { target: LinkTarget::Prometheus, up: false });
                        break;
                    }
                }
            }
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(1))) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALAR: &str = r#"{"status":"success","data":{"resultType":"vector","result":[{"metric":{},"value":[1700000000,"42.5"]}]}}"#;
    const VECTOR: &str = r#"{"status":"success","data":{"resultType":"vector","result":[
        {"metric":{"instance":"192.168.1.5:9100"},"value":[1,"91.2"]},
        {"metric":{"instance":"192.168.1.7:9100"},"value":[1,"12.0"]}]}}"#;

    #[test]
    fn scalar_and_empty() {
        assert_eq!(parse_scalar(SCALAR), Some(42.5));
        assert_eq!(parse_scalar(r#"{"data":{"result":[]}}"#), None);
        assert_eq!(parse_scalar("nope"), None);
    }

    #[test]
    fn vector_and_hottest() {
        let v = parse_vector(VECTOR);
        assert_eq!(v.len(), 2);
        assert_eq!(hottest(&v), Some((".5".into(), 91.2)));
        assert!(parse_vector("{}").is_empty());
    }

    #[test]
    fn node_tags() {
        assert_eq!(node_tag("192.168.1.5:9100"), ".5");
        assert_eq!(node_tag("nodename:9100"), "nodename");
    }

    #[test]
    fn query_path_is_encoded() {
        let p = format!("/api/v1/query?query={}", urlencoding::encode(Q_CPU));
        assert!(p.contains("%7Bmode%3D%22idle%22%7D"));
    }
}
```

`crates/sources/src/lib.rs` add `pub mod prometheus;` and `pub mod tunnel;`.

- [ ] **Step 3: Build and test**

Run: `cargo test -p rackscreen-sources`
Expected: all pass (17 tests). The tunnel code is exercised against the real cluster in Task 17.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(sources): port-forward tunnel and Prometheus poller"
```

---

### Task 16: qBittorrent poller

**Files:**
- Create: `crates/sources/src/qbittorrent.rs`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Produces:
  - `qbittorrent::QbitConfig { namespace, service, port, user, pass, poll_secs }`
  - `qbittorrent::RawTorrent` (serde), `qbittorrent::parse_torrents(&str) -> Vec<RawTorrent>`, `qbittorrent::active_torrents(&[RawTorrent]) -> Vec<Torrent>`
  - `qbittorrent::TorrentTracker::new()`, `.diff(&[RawTorrent]) -> Vec<Event>`
  - `qbittorrent::run_qbittorrent(client, cfg, ctx)`

- [ ] **Step 1: Write qbittorrent.rs with tests**

```rust
//! qBittorrent Web API poller over a port-forward tunnel.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use kube::Client;
use rackscreen_core::event::{Event, LinkTarget, Torrent};
use serde::Deserialize;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct QbitConfig {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RawTorrent {
    pub hash: String,
    pub name: String,
    pub state: String,
    /// 0.0 ..= 1.0
    pub progress: f64,
    #[serde(default)]
    pub eta: i64,
    #[serde(default)]
    pub dlspeed: i64,
}

const ACTIVE_STATES: [&str; 3] = ["downloading", "forcedDL", "metaDL"];
const MIN_SPEED_BPS: i64 = 500;

pub fn parse_torrents(json: &str) -> Vec<RawTorrent> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn active_torrents(raw: &[RawTorrent]) -> Vec<Torrent> {
    let mut out: Vec<Torrent> = raw
        .iter()
        .filter(|t| ACTIVE_STATES.contains(&t.state.as_str()) && t.dlspeed >= MIN_SPEED_BPS)
        .map(|t| Torrent {
            name: t.name.chars().take(30).collect(),
            progress: (t.progress * 100.0) as f32,
            eta_secs: t.eta,
            speed_bps: t.dlspeed,
        })
        .collect();
    out.sort_by(|a, b| b.progress.partial_cmp(&a.progress).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(3);
    out
}

/// Detects newly added and finished torrents between polls.
#[derive(Default)]
pub struct TorrentTracker {
    seen: HashMap<String, (String, f64)>,
    primed: bool,
}

impl TorrentTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&mut self, raw: &[RawTorrent]) -> Vec<Event> {
        let mut out = Vec::new();
        let mut now: HashMap<String, (String, f64)> = HashMap::new();
        for t in raw {
            now.insert(t.hash.clone(), (t.name.clone(), t.progress));
            if self.primed && !self.seen.contains_key(&t.hash) {
                out.push(Event::TorrentAdded { name: t.name.clone() });
            }
            if self.primed && t.progress >= 1.0 && self.seen.get(&t.hash).is_some_and(|(_, p)| *p < 1.0) {
                out.push(Event::TorrentDone { name: t.name.clone() });
            }
        }
        if self.primed {
            for (hash, (name, progress)) in &self.seen {
                if !now.contains_key(hash) && *progress > 0.99 {
                    out.push(Event::TorrentDone { name: name.clone() });
                }
            }
        }
        self.seen = now;
        self.primed = true;
        out
    }
}

struct Session {
    tunnel: Tunnel,
    cookie: Option<String>,
    port: u16,
}

impl Session {
    fn origin(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    async fn login(&mut self, user: &str, pass: &str) -> Result<()> {
        let origin = self.origin();
        let body = format!("username={}&password={}", urlencoding::encode(user), urlencoding::encode(pass));
        let r = self
            .tunnel
            .post_form("/api/v2/auth/login", &body, &[("referer", origin.as_str()), ("origin", origin.as_str())])
            .await?;
        anyhow::ensure!(r.status == 200 && (r.body.trim() == "Ok." || r.body.trim().is_empty()), "login HTTP {} {:?}", r.status, r.body.trim());
        self.cookie = r
            .headers
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| c.split(';').next().filter(|p| p.starts_with("SID=")).map(str::to_string));
        Ok(())
    }

    async fn torrents(&mut self) -> Result<Vec<RawTorrent>> {
        let origin = self.origin();
        let cookie = self.cookie.clone().unwrap_or_default();
        let r = self
            .tunnel
            .get("/api/v2/torrents/info?filter=downloading", &[("referer", origin.as_str()), ("cookie", cookie.as_str())])
            .await?;
        anyhow::ensure!(r.status == 200, "torrents/info HTTP {}", r.status);
        Ok(parse_torrents(&r.body))
    }
}

pub async fn run_qbittorrent(client: Client, cfg: QbitConfig, ctx: SourceCtx) {
    let mut tracker = TorrentTracker::new();
    let mut backoff = 5u64;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let session = async {
            let (pod, svc) = find_pod_for_service(&client, &cfg.namespace, &cfg.service, cfg.port, "qbit").await?;
            tracing::info!("qbittorrent: forwarding to {}/{pod} (service {svc})", cfg.namespace);
            let tunnel = Tunnel::open(&client, &cfg.namespace, &pod, cfg.port).await?;
            let mut s = Session { tunnel, cookie: None, port: cfg.port };
            s.login(&cfg.user, &cfg.pass).await.context("login")?;
            Ok::<_, anyhow::Error>(s)
        }
        .await;
        let mut s = match session {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("qbittorrent: {e:#}; retry in {backoff}s");
                ctx.emit(Event::Link { target: LinkTarget::QBittorrent, up: false });
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(60);
                continue;
            }
        };
        backoff = 5;
        let mut failures = 0;
        loop {
            match s.torrents().await {
                Ok(raw) => {
                    failures = 0;
                    ctx.emit(Event::Link { target: LinkTarget::QBittorrent, up: true });
                    ctx.emit_all(tracker.diff(&raw));
                    ctx.emit(Event::Torrents(active_torrents(&raw)));
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("qbittorrent poll: {e:#}");
                    if failures >= 2 {
                        ctx.emit(Event::Link { target: LinkTarget::QBittorrent, up: false });
                        break;
                    }
                }
            }
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(1))) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(hash: &str, state: &str, progress: f64, speed: i64) -> RawTorrent {
        RawTorrent { hash: hash.into(), name: format!("t-{hash}"), state: state.into(), progress, eta: 100, dlspeed: speed }
    }

    #[test]
    fn parses_api_json() {
        let json = r#"[{"hash":"abc","name":"ubuntu.iso","state":"downloading","progress":0.42,"eta":900,"dlspeed":123456}]"#;
        let v = parse_torrents(json);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "ubuntu.iso");
        assert!(parse_torrents("garbage").is_empty());
    }

    #[test]
    fn active_filters_sorts_truncates() {
        let list = vec![
            raw("a", "downloading", 0.1, 1000),
            raw("b", "stalledDL", 0.5, 1000),
            raw("c", "downloading", 0.9, 100),
            raw("d", "forcedDL", 0.7, 5000),
            raw("e", "metaDL", 0.0, 800),
            raw("f", "downloading", 0.3, 800),
        ];
        let act = active_torrents(&list);
        assert_eq!(act.len(), 3);
        assert_eq!(act[0].name, "t-d");
        assert_eq!(act[0].progress, 70.0);
        assert_eq!(act[2].name, "t-a");
    }

    #[test]
    fn tracker_added_and_done() {
        let mut t = TorrentTracker::new();
        assert!(t.diff(&[raw("a", "downloading", 0.5, 1000)]).is_empty(), "first poll primes silently");
        let evs = t.diff(&[raw("a", "downloading", 0.6, 1000), raw("b", "downloading", 0.0, 1000)]);
        assert_eq!(evs, vec![Event::TorrentAdded { name: "t-b".into() }]);
        let evs = t.diff(&[raw("a", "downloading", 1.0, 0), raw("b", "downloading", 0.1, 1000)]);
        assert_eq!(evs, vec![Event::TorrentDone { name: "t-a".into() }]);
        let evs = t.diff(&[raw("b", "downloading", 0.995, 1000)]);
        assert!(evs.is_empty());
        let evs = t.diff(&[]);
        assert_eq!(evs, vec![Event::TorrentDone { name: "t-b".into() }]);
    }
}
```

`crates/sources/src/lib.rs` add `pub mod qbittorrent;`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p rackscreen-sources`
Expected: all pass (20 tests).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(sources): qBittorrent poller"
```

---

### Task 17: Config, render loop, CLI wiring

**Files:**
- Create: `src/config.rs`, `src/runloop.rs`, `config.example.toml`
- Modify: `src/main.rs`, `Cargo.toml`

**Interfaces:**
- Consumes: everything above
- Produces: `rackscreen` binary with flags `--config`, `--sim`, `--sim-grid`, `--source fake|k8s`, `--fps`, `--seed`

- [ ] **Step 1: Add dependencies to root Cargo.toml**

In `[dependencies]` of the root package add:
```toml
chrono = "0.4"
```
and change the tokio line to:
```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync", "signal"] }
```

- [ ] **Step 2: Write config.example.toml**

```toml
# RackScreen configuration. Copy to ~/.config/rackscreen/config.toml on the Pi.

[k8s]
kubeconfig = "~/k8s-monitor.yaml"

[prometheus]
namespace = "monitoring"
service = "auto"        # "auto" picks the first service containing "prometheus" that exposes `port`
port = 9090
poll_secs = 5

[qbittorrent]
enabled = true
namespace = "arr-stack"
service = "qbittorrent"
port = 8080
user = ""               # empty = rely on "bypass auth for localhost"
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
spi_chunk = 4096        # raise to 65536 after adding spidev.bufsiz=65536 to cmdline.txt

[[screens]]
role = "cpu"
spi = 0
cs = 0
dc = 6
rst = 5
rotate = 270
hflip = false
hz = 40000000

[[screens]]
role = "mem"
spi = 0
cs = 1
dc = 13
rst = 26
rotate = 270
hflip = true
hz = 40000000

[[screens]]
role = "pods"
spi = 1
cs = 0
dc = 23
rst = 22
rotate = 270
hflip = true
hz = 16000000

[[screens]]
role = "health"
spi = 1
cs = 1
dc = 4
rst = 27
rotate = 270
hflip = true
hz = 16000000
```

- [ ] **Step 3: Write config.rs with tests**

```rust
//! TOML configuration with defaults matching config.example.toml.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rackscreen_core::theme::Role;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub k8s: K8sCfg,
    pub prometheus: PromCfg,
    pub qbittorrent: QbitCfg,
    pub night: NightCfg,
    pub thresholds: ThresholdsCfg,
    pub display: DisplayCfg,
    pub screens: Vec<ScreenCfg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct K8sCfg {
    pub kubeconfig: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PromCfg {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub poll_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct QbitCfg {
    pub enabled: bool,
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub poll_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NightCfg {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThresholdsCfg {
    pub hot_cpu: f32,
    pub hot_mem: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DisplayCfg {
    pub brightness: f32,
    pub fps: u32,
    pub spi_chunk: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScreenCfg {
    pub role: String,
    pub spi: u8,
    pub cs: u8,
    pub dc: u8,
    pub rst: u8,
    #[serde(default)]
    pub rotate: u32,
    #[serde(default)]
    pub hflip: bool,
    #[serde(default = "default_hz")]
    pub hz: u32,
}

fn default_hz() -> u32 {
    40_000_000
}

impl ScreenCfg {
    pub fn role(&self) -> Result<Role> {
        Ok(match self.role.as_str() {
            "cpu" => Role::Cpu,
            "mem" => Role::Mem,
            "pods" => Role::Pods,
            "health" => Role::Health,
            other => anyhow::bail!("unknown screen role '{other}'"),
        })
    }
}

impl Default for K8sCfg {
    fn default() -> Self {
        Self { kubeconfig: "~/k8s-monitor.yaml".into() }
    }
}
impl Default for PromCfg {
    fn default() -> Self {
        Self { namespace: "monitoring".into(), service: "auto".into(), port: 9090, poll_secs: 5 }
    }
}
impl Default for QbitCfg {
    fn default() -> Self {
        Self { enabled: true, namespace: "arr-stack".into(), service: "qbittorrent".into(), port: 8080, user: String::new(), pass: String::new(), poll_secs: 3 }
    }
}
impl Default for NightCfg {
    fn default() -> Self {
        Self { enabled: true, start: "23:00".into(), end: "07:00".into() }
    }
}
impl Default for ThresholdsCfg {
    fn default() -> Self {
        Self { hot_cpu: 90.0, hot_mem: 90.0 }
    }
}
impl Default for DisplayCfg {
    fn default() -> Self {
        Self { brightness: 1.0, fps: 30, spi_chunk: 4096 }
    }
}
impl Default for Config {
    fn default() -> Self {
        toml::from_str(include_str!("../config.example.toml")).expect("config.example.toml is valid")
    }
}

pub fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("rackscreen/config.toml")
    }

    pub fn load(path: Option<&Path>) -> Result<Config> {
        let path = path.map(Path::to_path_buf).unwrap_or_else(Config::default_path);
        if !path.exists() {
            tracing::warn!("config {} not found, using built-in defaults", path.display());
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Config = toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        anyhow::ensure!(!cfg.screens.is_empty(), "config needs at least one [[screens]] entry");
        for s in &cfg.screens {
            s.role()?;
        }
        Ok(cfg)
    }
}

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
        assert!(c.qbittorrent.enabled);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let c: Config = toml::from_str("[night]\nenabled = false\n[[screens]]\nrole = \"cpu\"\nspi = 0\ncs = 0\ndc = 6\nrst = 5\n").unwrap();
        assert!(!c.night.enabled);
        assert_eq!(c.night.start, "23:00");
        assert_eq!(c.screens[0].hz, 40_000_000);
        assert_eq!(c.display.fps, 30);
    }

    #[test]
    fn bad_role_rejected() {
        let s = ScreenCfg { role: "nope".into(), spi: 0, cs: 0, dc: 0, rst: 0, rotate: 0, hflip: false, hz: 1 };
        assert!(s.role().is_err());
    }

    #[test]
    fn expand_home_works() {
        assert!(expand_home("/abs").starts_with("/abs"));
        assert!(!expand_home("~/x").to_string_lossy().starts_with('~'));
    }
}
```

- [ ] **Step 4: Write runloop.rs with tests**

```rust
//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_core::anim::Secs;
use rackscreen_core::event::Event;
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::night::is_night;
use rackscreen_core::theme::Role;
use rackscreen_display::{DisplayCmd, Mailbox};
use rackscreen_render::frame::{dirty_rect, new_pixmap, Orient, Rect};
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

const NIGHT_FADE_SECS: Secs = 1.0;

pub struct ScreenSlot {
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    cur: Pixmap,
    oriented: Pixmap,
    prev: Pixmap,
    first: bool,
}

impl ScreenSlot {
    pub fn new(role: Role, orient: Orient, mailbox: Mailbox) -> Self {
        Self { role, orient, mailbox, cur: new_pixmap(), oriented: new_pixmap(), prev: new_pixmap(), first: true }
    }

    /// Orient the freshly rendered frame, diff, push. Returns the dirty rect pushed.
    fn flush(&mut self) -> Option<Rect> {
        self.orient.apply(&self.cur, &mut self.oriented);
        let rect = if self.first { Some(Rect::full()) } else { dirty_rect(&self.prev, &self.oriented) };
        self.first = false;
        if let Some(r) = rect {
            self.mailbox.put(DisplayCmd::Frame(self.oriented.clone(), r));
        }
        std::mem::swap(&mut self.prev, &mut self.oriented);
        rect
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NightWindow {
    pub enabled: bool,
    pub start_min: u32,
    pub end_min: u32,
}

/// Multiply every channel by `k` (0 = black, 1 = unchanged).
pub fn darken(px: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    for p in px.data_mut().chunks_exact_mut(4) {
        p[0] = (p[0] as f32 * k) as u8;
        p[1] = (p[1] as f32 * k) as u8;
        p[2] = (p[2] as f32 * k) as u8;
    }
}

fn local_minutes() -> u32 {
    use chrono::Timelike;
    let t = chrono::Local::now();
    t.hour() * 60 + t.minute()
}

pub struct RenderLoop {
    pub rx: Receiver<Event>,
    pub screens: Vec<ScreenSlot>,
    pub thresholds: Thresholds,
    pub night: NightWindow,
    pub fps: u32,
    pub stop: Arc<AtomicBool>,
}

impl RenderLoop {
    pub fn run(mut self) -> Result<()> {
        let mut renderer = Renderer::new()?;
        let start = Instant::now();
        let mut model = Model::new(self.thresholds);
        model.apply(Event::Boot, 0.0);
        let tick = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let mut asleep = false;
        let mut fade_started: Option<Secs> = None;

        while !self.stop.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            let now = start.elapsed().as_secs_f64();
            while let Ok(ev) = self.rx.try_recv() {
                model.apply(ev, now);
            }
            model.tick(now);

            let night = match model.night_override() {
                Some(v) => v,
                None => self.night.enabled && is_night(local_minutes(), self.night.start_min, self.night.end_min),
            };

            if night && asleep {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            if !night && asleep {
                tracing::info!("night mode over, waking displays");
                for s in &self.screens {
                    s.mailbox.put(DisplayCmd::Wake);
                }
                asleep = false;
                for s in &mut self.screens {
                    s.first = true;
                }
                model.apply(Event::Boot, now);
                model.tick(now);
            }
            let fade = if night {
                let f0 = *fade_started.get_or_insert(now);
                Some((((now - f0) / NIGHT_FADE_SECS) as f32).min(1.0))
            } else {
                fade_started = None;
                None
            };

            for s in &mut self.screens {
                let scene = model.scene(s.role, now);
                renderer.render(&scene, &mut s.cur);
                if let Some(f) = fade {
                    darken(&mut s.cur, 1.0 - f);
                }
                s.flush();
            }

            if fade == Some(1.0) {
                tracing::info!("night mode, sleeping displays");
                for s in &self.screens {
                    s.mailbox.put(DisplayCmd::Sleep);
                }
                asleep = true;
                continue;
            }

            let spent = t0.elapsed();
            if spent < tick {
                std::thread::sleep(tick - spent);
            }
        }
        for s in &self.screens {
            s.mailbox.put(DisplayCmd::Quit);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darken_scales_channels() {
        let mut p = Pixmap::new(2, 1).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        darken(&mut p, 0.5);
        assert_eq!(p.data()[0], 127);
        darken(&mut p, 0.0);
        assert_eq!(p.data()[0], 0);
    }

    #[test]
    fn first_flush_is_full_then_dirty_only() {
        let mb = Mailbox::new();
        let mut slot = ScreenSlot::new(Role::Cpu, Orient::identity(), mb.clone());
        assert_eq!(slot.flush(), Some(Rect::full()));
        assert!(matches!(mb.take(), DisplayCmd::Frame(_, r) if r == Rect::full()));
        assert_eq!(slot.flush(), None, "identical frame pushes nothing");
        slot.cur.data_mut()[(10 * 240 + 10) * 4] = 200;
        assert_eq!(slot.flush(), Some(Rect { x: 10, y: 10, w: 1, h: 1 }));
    }
}
```

- [ ] **Step 5: Write main.rs**

```rust
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
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cli = Cli::parse();
    let cfg = Config::load(cli.config.as_deref())?;
    let source = cli.source.unwrap_or(if cli.sim { SourceKind::Fake } else { SourceKind::K8s });
    let fps = cli.fps.unwrap_or(cfg.display.fps);

    let (tx, rx) = mpsc::channel();
    let shutdown = CancellationToken::new();
    let stop = Arc::new(AtomicBool::new(false));
    let ctx = SourceCtx { tx, shutdown: shutdown.clone() };

    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;

    // ---- sources ----
    let fake_cmds: Option<mpsc::Sender<rackscreen_sources::fake::FakeCmd>> = match source {
        SourceKind::Fake => {
            let (cmd_tx, cmd_rx) = mpsc::channel();
            runtime.spawn(rackscreen_sources::fake::run_fake(ctx.clone(), cmd_rx, cli.seed));
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
                tokio::spawn(rackscreen_sources::k8s::run_pod_watch(client.clone(), ctx2.clone()));
                tokio::spawn(rackscreen_sources::k8s::run_node_watch(client.clone(), ctx2.clone()));
                let prom = rackscreen_sources::prometheus::PromConfig {
                    namespace: cfg2.prometheus.namespace.clone(),
                    service: cfg2.prometheus.service.clone(),
                    port: cfg2.prometheus.port,
                    poll_secs: cfg2.prometheus.poll_secs,
                };
                tokio::spawn(rackscreen_sources::prometheus::run_prometheus(client.clone(), prom, ctx2.clone()));
                if cfg2.qbittorrent.enabled {
                    let q = rackscreen_sources::qbittorrent::QbitConfig {
                        namespace: cfg2.qbittorrent.namespace.clone(),
                        service: cfg2.qbittorrent.service.clone(),
                        port: cfg2.qbittorrent.port,
                        user: cfg2.qbittorrent.user.clone(),
                        pass: cfg2.qbittorrent.pass.clone(),
                        poll_secs: cfg2.qbittorrent.poll_secs,
                    };
                    tokio::spawn(rackscreen_sources::qbittorrent::run_qbittorrent(client, q, ctx2));
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
            let (hub, panels) = rackscreen_display::sim::SimHub::new(cfg.screens.len(), cli.sim_grid, key_tx)?;
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
                let pins = rackscreen_display::gc9a01::Pins { bus: scr.spi, cs: scr.cs, dc: scr.dc, rst: scr.rst, hz: scr.hz };
                let dev = rackscreen_display::gc9a01::Gc9a01::open(pins, cfg.display.spi_chunk, cfg.display.brightness)
                    .with_context(|| format!("open display {}", scr.role))?;
                let d: Box<dyn Display> = Box::new(dev);
                let mb = Mailbox::new();
                slots.push(ScreenSlot::new(scr.role()?, Orient::new(scr.rotate, scr.hflip), mb.clone()));
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
```

- [ ] **Step 6: Build, unit test, run the simulator**

Run: `cargo test --workspace`
Expected: all pass.

Run: `cargo run -- --sim`
Expected: window with four round panels. Boot sweep runs top to bottom, then CPU/MEM/PODS/HEALTH idle with breathing segments and drifting numbers. Press `2`: red ripple, package-x pops on PODS, red dot stays. Press `3`: red wave up from HEALTH, all four show server-off, then return. Press `t`: HEALTH shows three concentric rings. Press `n`: fade to black, panels sleep; `n` again forces day (boot replays); `n` a third time returns to clock. `Esc` quits cleanly. Title bar shows ~60 fps.

Run: `cargo run -- --sim --source k8s --config ./config.example.toml` (with `~/k8s-monitor.yaml` present on this box, or edit the kubeconfig path)
Expected: connecting state for a moment, then real values. Logs show `prometheus: forwarding to monitoring/...` and `qbittorrent: forwarding to arr-stack/...`. Fix any tunnel or query issue here before moving on. If `service = "auto"` picks the wrong service, set the exact name.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: config, render loop and CLI wiring; simulator runs end to end"
```

---

### Task 18: Deploy files and README

**Files:**
- Create: `deploy/rackscreen.service`, `deploy/install.sh`
- Modify: `README.md`

- [ ] **Step 1: Write the systemd unit**

`deploy/rackscreen.service`:
```ini
[Unit]
Description=RackScreen Kubernetes rack monitor
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=%i
ExecStart=/usr/local/bin/rackscreen
Restart=always
RestartSec=3
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 2: Write install.sh**

```bash
#!/usr/bin/env bash
# Usage: ./install.sh <path-to-rackscreen-binary> [user]
set -euo pipefail
BIN="${1:?binary path}"
USER_NAME="${2:-$USER}"
sudo install -m 755 "$BIN" /usr/local/bin/rackscreen
mkdir -p "$HOME/.config/rackscreen"
[ -f "$HOME/.config/rackscreen/config.toml" ] || cp "$(dirname "$0")/../config.example.toml" "$HOME/.config/rackscreen/config.toml"
sudo usermod -aG spi,gpio "$USER_NAME"
sudo install -m 644 "$(dirname "$0")/rackscreen.service" /etc/systemd/system/rackscreen@.service
sudo systemctl daemon-reload
sudo systemctl enable --now "rackscreen@${USER_NAME}"
echo "installed; logs: journalctl -u rackscreen@${USER_NAME} -f"
```

- [ ] **Step 3: Write README.md**

```markdown
# RackScreen

Animated Kubernetes monitor for four GC9A01 240x240 round displays on a Raspberry Pi 3B+ (64-bit). Written in Rust.

Four screens, top to bottom: **CPU**, **MEM**, **PODS**, **HEALTH**. Icon-first "Minimal Mono" look: black background, segmented ring, white Lucide icon, small outlined badge with the number. Screens are never static: values ease, the last segment breathes, icons have micro-loops. Cluster events splash on their screen (pod started, crashed, gone, hot node, torrent added). Big events sweep the whole rack (node down/up, alert firing/resolved, torrent done, link restored). HEALTH turns into a download monitor when the cluster is healthy and qBittorrent is pulling.

Design: `docs/superpowers/specs/2026-09-05-rackscreen-design.md`.

## Develop on the desktop

    cargo run -- --sim                 # fake data, keyboard drives events
    cargo run -- --sim --source k8s    # real cluster through ~/k8s-monitor.yaml

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
```

- [ ] **Step 4: Commit**

```bash
chmod +x deploy/install.sh
git add -A
git commit -m "docs: README, systemd unit and install script"
```

---

### Task 19: GitHub Actions

**Files:**
- Create: `.github/workflows/ci.yml`, `.github/workflows/release.yml`

- [ ] **Step 1: Write ci.yml**

```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: sudo apt-get update && sudo apt-get install -y libxkbcommon-dev libwayland-dev libx11-dev
      - run: cargo test --workspace --features sim,pi
      - run: cargo clippy --workspace --features sim,pi -- -D warnings
```

- [ ] **Step 2: Write release.yml**

```yaml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  build-aarch64:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cross --version 0.2.5 --locked
      - run: cross build --release --target aarch64-unknown-linux-gnu --no-default-features --features pi
      - run: cp target/aarch64-unknown-linux-gnu/release/rackscreen rackscreen-aarch64
      - uses: softprops/action-gh-release@v2
        with:
          files: rackscreen-aarch64
```

- [ ] **Step 3: Verify the Pi feature set builds natively without the simulator**

Run: `cargo build --no-default-features --features pi`
Expected: builds (minifb not compiled). Run `cargo clippy --workspace --features sim,pi -- -D warnings` and fix anything it reports.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "ci: test on push, cross-build aarch64 release on tags"
```

---

## Handoff to hardware

After Task 19: push to GitHub, tag `v0.1.0`, wait for the release asset, follow README "Pi setup". First run on the Pi: `RUST_LOG=debug rackscreen` in a terminal before enabling the service, check each screen's `rotate`/`hflip`, and adjust `brightness` if panels look washed out. Tune `hz` on SPI1 upward (try 31_250_000) while watching for artifacts.

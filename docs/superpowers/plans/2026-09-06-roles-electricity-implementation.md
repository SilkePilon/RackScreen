# Screen roles, Electricity mode, Thermal and Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every physical screen cycles through a configurable list of roles (with a TUI editor), two new cluster roles (Thermal, Storage), and four Electricity-mode roles (Power mix, Price, Carbon, Renewable) fed by the Electricity Maps API and a day-ahead price source, using the site's colours and icons.

**Architecture:** `Role` grows from 4 to 10 variants; rack sweeps are ordered by physical screen index instead of role; the `Model` owns per-screen `ScreenState` (role list, current index, cycle timing, iris transition) and `Model::scene(screen_index, now)` replaces `scene(role, now)`. New events/state for temperatures, Longhorn, electricity and prices flow through the existing `Event` channel from new Prometheus queries, a `reqwest`-based Electricity Maps poller and an EnergyZero/ENTSO-E price poller. The renderer gains ring pitch/start, per-icon viewBox units, a tick drawable and scene zoom/reveal for transitions. The setup TUI gains a Screens editor and new Configure fields.

**Tech Stack:** existing workspace (Rust 2021, tiny-skia, usvg, ratatui 0.30, serde_yaml_ng) plus `reqwest 0.13` (rustls, json), `quick-xml` for ENTSO-E, `chrono` for local dates.

Spec: `docs/superpowers/specs/2026-09-06-screen-roles-electricity-design.md`

## Global Constraints

- Role ids: `cpu`, `mem`, `pods`, `health`, `thermal`, `storage`, `power-mix`, `price`, `carbon`, `renewable`. Config `screens[].roles` (list, at least one, unknown names fail validation), `cycle_secs` default 15, min 3, max 300; legacy `role:` accepted and upgraded to `roles: [..]`.
- Iris transition 500 ms: 250 ms shrink/ring-unlight (ease-in), 250 ms grow/ring-light (ease-out); transitions wait while a splash or sweep is active on that screen.
- Presets: cluster `[cpu] [mem] [pods] [health]`; electricity `[power-mix] [price] [carbon] [renewable]`; mixed `[cpu, power-mix] [mem, price] [pods, carbon] [health, renewable]`, all 15 s.
- Site colours: biomass `#008043`, geothermal `#A73C15`, hydro `#1878EA`, solar `#FFC700`, wind `#69D6F8`, nuclear `#9D71F7`, battery storage `#1DA484`, hydro storage `#2B3CD8`, coal `#ac8c35`, gas `#AAA189`, oil `#584745`, unknown `#ACACAC`. Carbon scale stops: 0 `#2AA364`, 150 `#F5EB4D`, 600 `#9E4229`, 800 `#381D02`. Temperature scale: 35 °C `#4f8dff`, 55 °C `#ffb020`, 70 °C `#ff4d4d`. Price scale: cheap `#3ddc97` to expensive `#ff4d4d`, negative in `#4f8dff`.
- Power mix geometry: 60 segments, biggest source at 12 o'clock clockwise, one unlit gap segment between sources (taken from the larger neighbour), at least one segment per producing source, zero sources absent; icons 26 px in source colour on radius 60 for sources with ≥ 3 segments; tick 3 px round-capped in `#1c1c1c` from radius 80 to 88; no centre icon, no badge.
- Price ring: 48 segments at 7.5° pitch (two per hour), midnight at 12 o'clock; past hours 35 % alpha, current hour breathing; badge `22.1 ct` (`1.02 €` at ≥ 100 ct), flips to `min 6.2` every 5 s.
- Wide badge: `w = max(64, 12·n + 16)`, dots 12 px apart, radius 4.5; used by Health and Thermal.
- Electricity Maps: `https://api.electricitymap.org/v3/power-breakdown/latest?zone=` and `carbon-intensity/latest`, header `auth-token`; poll 300 s default; two-strike link rule. EnergyZero URL and ENTSO-E URL as in the spec; prices in ct/kWh; 24 hourly values, missing = NaN.
- Prometheus: temps `max by (nodename) (node_hwmon_temp_celsius * on(instance) group_left(nodename) node_uname_info)` with `node_thermal_zone_temp` fallback; Longhorn `longhorn_volume_robustness` (1 healthy, 2 degraded, 3 faulted), `sum(longhorn_volume_actual_size_bytes)`, `sum(longhorn_volume_capacity_bytes)`. `thresholds.hot_temp` default 70, 5-minute debounce per node.
- Electricity Maps icons ship under AGPL-3.0 with attribution in `assets/icons/EM-LICENSE.md`.
- All existing tests keep passing; clippy `-D warnings` on `sim,pi`, `pi`-only and `sim`-only; `cargo fmt --all` before commits; commit bodies end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## File map

```
crates/core/src/theme.rs        Role (10 variants, name/parse), palettes for sources/carbon/temp/price
crates/core/src/fx.rs           splashes Vec, sweeps by screen index
crates/core/src/screens.rs      ScreenState, cycling + iris transition (new)
crates/core/src/model.rs        set_screens, new state (temps, storage, electricity, prices), new folds
crates/core/src/scene.rs        Scene { zoom, ring_reveal }, Drawable::Tick, Ring pitch/start, wide badge helper
crates/core/src/scene_thermal.rs, scene_storage.rs, scene_electricity.rs   new scenes
crates/core/src/scene_fx.rs     Model::scene(screen_index, now), sweep by screen
crates/core/src/event.rs        new events, Source, Robustness, LinkTarget additions
crates/render/src/icons.rs      per-icon units
crates/render/src/prims.rs      segments with pitch/start
crates/render/src/renderer.rs   Tick, zoom/reveal, ring pitch
crates/render/src/assets.rs     new Lucide + em-* icons
assets/icons/em-*.svg, assets/icons/EM-LICENSE.md
crates/sources/src/prometheus.rs   temps + longhorn queries, StorageTracker
crates/sources/src/electricity.rs  Electricity Maps poller (new)
crates/sources/src/prices.rs       EnergyZero / ENTSO-E poller (new)
crates/sources/src/fake.rs         new data + keys
crates/display/src/sim.rs          keys 9 0 h p
crates/app/src/config.rs           roles/cycle_secs, electricity, price, hot_temp
crates/app/src/run.rs, panels.rs, runloop.rs, calibrate.rs   screen index wiring
crates/setup/src/screens/screens.rs   Screens editor (new)
crates/setup/src/screens/{menu,configure,status}.rs, lib.rs   menu item, fields, dots
config.example.yaml, README.md, Cargo.toml (0.3.0)
```

---

### Task 1: Ten roles, screen-indexed sweeps, per-screen cycling with iris transition

**Files:**
- Create: `crates/core/src/screens.rs`
- Modify: `crates/core/src/theme.rs`, `crates/core/src/fx.rs`, `crates/core/src/model.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`, `crates/core/src/lib.rs`, `crates/app/src/runloop.rs`, `crates/app/src/run.rs`, `crates/app/src/calibrate.rs`, `crates/render/src/renderer.rs`, `crates/render/tests/golden.rs`

**Interfaces:**
- Produces:
  - `theme::Role { Cpu, Mem, Pods, Health, Thermal, Storage, PowerMix, Price, Carbon, Renewable }`, `Role::ALL: [Role; 10]`, `.index()`, `Role::from_index`, `.name() -> &'static str` (config id), `Role::parse(&str) -> Option<Role>`, `.accent()`, `.icon()`.
  - `fx::Fx { splashes: Vec<SplashQueue> /* len Role::ALL.len() */, sweeps }`; `Sweep::order_index(direction, screen: usize, screens: usize) -> usize`; `Sweep::phase(&self, screen: usize, screens: usize, now) -> SweepPhase`; `Sweep::done(&self, screens, now)`; `SweepQueue::tick(now, screens)`; `Fx::tick(now, screens)`.
  - `screens::{ScreenState::new(roles: Vec<Role>, cycle_secs: Secs), .current() -> Role, .roles(), .tick(now, busy: bool), .transition(now) -> Option<Transition { from: Role, to: Role, t: f32 /*0..1*/ }>, .set_roles(roles, cycle_secs)}`, `screens::TRANSITION_SECS = 0.5`.
  - `scene::Scene { items, zoom: f32, ring_reveal: f32 }` (`Scene::new()` sets 1.0/1.0).
  - `Model::set_screens(&mut self, roles: Vec<Vec<Role>>, cycle_secs: Vec<Secs>)`, `Model::screen_count()`, `Model::current_role(screen) -> Role`, `Model::scene(&self, screen: usize, now) -> Scene`, `Model::scene_for_role(&self, role, now) -> Scene` (no transition, used by tests and goldens).
  - `runloop::ScreenSlot::new(index: usize, role: Role, orient, mailbox)`; render loop calls `model.scene(s.index, now)`; `run.rs` calls `model.set_screens(...)` (interim: one role per screen from `PanelHandle.role`; Task 2 switches to config lists).

- [ ] **Step 1: theme.rs: grow Role**

Replace the `Role` enum and impl:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Cpu,
    Mem,
    Pods,
    Health,
    Thermal,
    Storage,
    PowerMix,
    Price,
    Carbon,
    Renewable,
}

impl Role {
    pub const ALL: [Role; 10] = [
        Role::Cpu, Role::Mem, Role::Pods, Role::Health, Role::Thermal,
        Role::Storage, Role::PowerMix, Role::Price, Role::Carbon, Role::Renewable,
    ];

    pub fn index(self) -> usize {
        Role::ALL.iter().position(|r| *r == self).expect("role in ALL")
    }
    pub fn from_index(i: usize) -> Option<Role> {
        Role::ALL.get(i).copied()
    }
    /// Config identifier.
    pub fn name(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "mem",
            Role::Pods => "pods",
            Role::Health => "health",
            Role::Thermal => "thermal",
            Role::Storage => "storage",
            Role::PowerMix => "power-mix",
            Role::Price => "price",
            Role::Carbon => "carbon",
            Role::Renewable => "renewable",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|r| r.name() == s)
    }
    pub fn accent(self) -> Color {
        match self {
            Role::Cpu => AMBER,
            Role::Mem => VIOLET,
            Role::Pods => BLUE,
            Role::Health => GREEN,
            Role::Thermal => AMBER,
            Role::Storage => VIOLET,
            Role::PowerMix => Color::hex(0xFFC700),
            Role::Price => GREEN,
            Role::Carbon => Color::hex(0x2AA364),
            Role::Renewable => GREEN,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "memory-stick",
            Role::Pods => "box",
            Role::Health => "heart-pulse",
            Role::Thermal => "thermometer",
            Role::Storage => "database",
            Role::PowerMix => "zap",
            Role::Price => "euro",
            Role::Carbon => "cloud",
            Role::Renewable => "leaf",
        }
    }
}
```
Add to the theme tests: `assert_eq!(Role::parse("power-mix"), Some(Role::PowerMix)); assert_eq!(Role::parse("nope"), None); for r in Role::ALL { assert_eq!(Role::parse(r.name()), Some(r)); }`. (Icons `thermometer`, `database`, `zap`, `euro`, `cloud`, `leaf` are added to the asset set in Task 3; until then the renderer draws nothing for unknown names, which is already its behaviour.)

- [ ] **Step 2: fx.rs: sweeps by screen index, splash vector**

Replace `order_index`, `phase`, `done`, `SweepQueue::tick`, `Fx` and `Fx::tick`:
```rust
impl Sweep {
    /// Position of a physical screen in the wave: 0 is the origin screen.
    /// Down starts at the top screen (index 0), Up at the bottom (index screens-1).
    pub fn order_index(direction: Direction, screen: usize, screens: usize) -> usize {
        match direction {
            Direction::Down => screen,
            Direction::Up => screens.saturating_sub(1).saturating_sub(screen),
        }
    }

    fn hold_end(&self, screens: usize) -> Secs {
        let last = screens.saturating_sub(1) as f64;
        self.started + last * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS + self.kind.hold()
    }

    pub fn phase(&self, screen: usize, screens: usize, now: Secs) -> SweepPhase {
        let idx = Sweep::order_index(self.kind.direction(), screen, screens) as f64;
        let in_start = self.started + idx * SWEEP_STAGGER_SECS;
        let in_end = in_start + SWEEP_WIPE_SECS;
        let out_start = self.hold_end(screens) + idx * SWEEP_STAGGER_SECS;
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

    pub fn done(&self, screens: usize, now: Secs) -> bool {
        let last = screens.saturating_sub(1) as f64;
        now >= self.hold_end(screens) + last * SWEEP_STAGGER_SECS + SWEEP_WIPE_SECS
    }
}

impl SweepQueue {
    pub fn tick(&mut self, now: Secs, screens: usize) {
        if self.active.is_some_and(|s| s.done(screens, now)) {
            self.active = None;
        }
        if self.active.is_none() {
            if let Some(kind) = self.queue.pop_front() {
                self.active = Some(Sweep { kind, started: now });
            }
        }
    }
}

#[derive(Debug)]
pub struct Fx {
    pub splashes: Vec<SplashQueue>,
    pub sweeps: SweepQueue,
}

impl Default for Fx {
    fn default() -> Self {
        Fx { splashes: (0..Role::ALL.len()).map(|_| SplashQueue::default()).collect(), sweeps: SweepQueue::default() }
    }
}

impl Fx {
    pub fn tick(&mut self, now: Secs, screens: usize) {
        self.sweeps.tick(now, screens);
        let frozen = self.sweeps.active().is_some();
        for q in &mut self.splashes {
            q.tick(now, frozen);
        }
    }
}
```
Keep `Fx::apply` as is (it indexes `self.splashes[role.index()]`). Update the fx tests: `sweep_phases_follow_direction` uses `s.phase(3, 4, ..)` for the bottom screen and `s.phase(0, 4, ..)` for the top (Up direction: origin is screen 3), `s.done(4, ..)`; `sweep_queue_serialises_and_dedupes` passes `4`; `fx_routes_requests` calls `fx.tick(0.0, 4)`. Add a test that with `screens = 1` the whole sweep lasts `SWEEP_WIPE_SECS + hold + SWEEP_WIPE_SECS`.

- [ ] **Step 3: screens.rs (new)**

```rust
//! Per-physical-screen role list, cycling and the iris transition.

use crate::anim::{Easing, Secs};
use crate::theme::Role;

pub const TRANSITION_SECS: Secs = 0.5;
pub const MIN_CYCLE_SECS: Secs = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: Role,
    pub to: Role,
    /// 0..1 over the whole transition; < 0.5 is the outgoing half.
    pub t: f32,
}

#[derive(Clone, Debug)]
pub struct ScreenState {
    roles: Vec<Role>,
    cycle_secs: Secs,
    current: usize,
    since: Secs,
    transition_started: Option<Secs>,
}

impl ScreenState {
    pub fn new(roles: Vec<Role>, cycle_secs: Secs) -> ScreenState {
        let roles = if roles.is_empty() { vec![Role::Cpu] } else { roles };
        ScreenState { roles, cycle_secs: cycle_secs.max(MIN_CYCLE_SECS), current: 0, since: 0.0, transition_started: None }
    }

    pub fn set_roles(&mut self, roles: Vec<Role>, cycle_secs: Secs) {
        *self = ScreenState::new(roles, cycle_secs);
    }

    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// The role being shown (during a transition: the outgoing one until t >= 0.5).
    pub fn current(&self) -> Role {
        self.roles[self.current]
    }

    pub fn next_role(&self) -> Role {
        self.roles[(self.current + 1) % self.roles.len()]
    }

    /// Advance timers. `busy` = a splash or sweep is active on this screen; the
    /// transition is postponed while busy.
    pub fn tick(&mut self, now: Secs, busy: bool) {
        if self.roles.len() < 2 {
            return;
        }
        match self.transition_started {
            Some(t0) => {
                if now - t0 >= TRANSITION_SECS {
                    self.current = (self.current + 1) % self.roles.len();
                    self.since = now;
                    self.transition_started = None;
                }
            }
            None => {
                if !busy && now - self.since >= self.cycle_secs {
                    self.transition_started = Some(now);
                }
            }
        }
    }

    pub fn transition(&self, now: Secs) -> Option<Transition> {
        let t0 = self.transition_started?;
        let t = ((now - t0) / TRANSITION_SECS).clamp(0.0, 1.0) as f32;
        Some(Transition { from: self.current(), to: self.next_role(), t })
    }
}

/// Zoom (scale about the centre) and ring reveal fraction for a transition instant.
/// First half: outgoing scene shrinks with ease-in and its ring unlights backwards.
/// Second half: incoming scene grows with ease-out and its ring lights clockwise.
pub fn transition_transform(t: f32) -> (f32, f32) {
    if t < 0.5 {
        let p = t / 0.5;
        let e = 1.0 - Easing::OutCubic.apply(1.0 - p); // ease-in
        (1.0 - e, 1.0 - e)
    } else {
        let p = (t - 0.5) / 0.5;
        let e = Easing::OutCubic.apply(p);
        (e, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_screen_never_transitions() {
        let mut s = ScreenState::new(vec![Role::Cpu], 3.0);
        s.tick(100.0, false);
        assert!(s.transition(100.0).is_none());
        assert_eq!(s.current(), Role::Cpu);
    }

    #[test]
    fn cycles_after_dwell_and_waits_while_busy() {
        let mut s = ScreenState::new(vec![Role::Cpu, Role::Thermal], 5.0);
        s.tick(4.9, false);
        assert!(s.transition(4.9).is_none());
        s.tick(5.0, true);
        assert!(s.transition(5.0).is_none(), "busy postpones");
        s.tick(5.1, false);
        let tr = s.transition(5.2).unwrap();
        assert_eq!((tr.from, tr.to), (Role::Cpu, Role::Thermal));
        assert!(tr.t > 0.0 && tr.t < 0.5);
        s.tick(5.7, false);
        assert!(s.transition(5.7).is_none());
        assert_eq!(s.current(), Role::Thermal);
        s.tick(10.8, false);
        assert_eq!(s.transition(10.8).unwrap().to, Role::Cpu, "wraps around");
    }

    #[test]
    fn cycle_floor_and_transform_shape() {
        let s = ScreenState::new(vec![Role::Cpu, Role::Mem], 1.0);
        assert_eq!(s.cycle_secs, MIN_CYCLE_SECS);
        assert_eq!(transition_transform(0.0), (1.0, 1.0));
        let (z, r) = transition_transform(0.49);
        assert!(z < 0.1 && r < 0.1);
        assert_eq!(transition_transform(0.5), (0.0, 0.0));
        assert_eq!(transition_transform(1.0), (1.0, 1.0));
    }
}
```
`lib.rs`: add `pub mod screens;`.

- [ ] **Step 4: scene.rs: zoom and ring_reveal on Scene**

Change the struct and constructor:
```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub items: Vec<Drawable>,
    /// Scale about the centre applied by the renderer (1.0 = none).
    pub zoom: f32,
    /// Fraction of every ring's segments that are drawn (1.0 = all).
    pub ring_reveal: f32,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self { items: vec![Drawable::Clear(BLACK)], zoom: 1.0, ring_reveal: 1.0 }
    }
    ...
}
```
Remove the `Default` derive. Fix any `Scene { items: .. }` literal in tests to `Scene { items: .., ..Scene::new() }`.

- [ ] **Step 5: model.rs: screens and scene dispatch**

Add fields `screens: Vec<ScreenState>` (default: four static screens `Cpu, Mem, Pods, Health`) and methods:
```rust
    pub fn set_screens(&mut self, roles: Vec<Vec<Role>>, cycle_secs: Vec<Secs>) {
        self.screens = roles
            .into_iter()
            .enumerate()
            .map(|(i, r)| ScreenState::new(r, cycle_secs.get(i).copied().unwrap_or(15.0)))
            .collect();
        if self.screens.is_empty() {
            self.screens.push(ScreenState::new(vec![Role::Cpu], 15.0));
        }
    }
    pub fn screen_count(&self) -> usize {
        self.screens.len()
    }
    pub fn current_role(&self, screen: usize) -> Role {
        self.screens.get(screen).map(|s| s.current()).unwrap_or(Role::Cpu)
    }
    pub fn screen(&self, screen: usize) -> Option<&ScreenState> {
        self.screens.get(screen)
    }
```
`Model::tick` becomes:
```rust
    pub fn tick(&mut self, now: Secs) {
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        let screens = self.screens.len();
        self.fx_state.tick(now, screens);
        let sweep_active = self.fx_state.sweeps.active().is_some();
        for s in &mut self.screens {
            let busy = sweep_active || self.fx_state.splashes[s.current().index()].active().is_some();
            s.tick(now, busy);
        }
    }
```
(Borrow note: compute `busy` per screen with an immutable borrow of `fx_state` before mutating `s`; the code above compiles because `self.fx_state` and `self.screens` are disjoint fields.)

- [ ] **Step 6: scene_fx.rs: sweep by screen, scene by screen index, transition**

Change `sweep_scene(sweep: &Sweep, role: Role, phase: SweepPhase, now: Secs)` to keep its signature (role = the screen's current role, used for Boot colour/icon) and replace `Model::scene`:
```rust
    /// Scene for one screen at one instant; the only call the render loop makes.
    pub fn scene(&self, screen: usize, now: Secs) -> Scene {
        let screens = self.screen_count();
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene(now),
        };
        let role = state.current();
        if !self.link().api {
            return connecting_scene(now);
        }
        if let Some(sw) = self.fx().sweeps.active() {
            match sw.phase(screen, screens, now) {
                SweepPhase::Idle => {}
                ph => return sweep_scene(sw, role, ph, now),
            }
        }
        if let Some(tr) = state.transition(now) {
            let (zoom, reveal) = crate::screens::transition_transform(tr.t);
            let shown = if tr.t < 0.5 { tr.from } else { tr.to };
            let mut s = self.scene_for_role(shown, now);
            s.zoom = zoom;
            s.ring_reveal = reveal;
            return s;
        }
        let base = self.scene_for_role(role, now);
        if self.fx().sweeps.active().is_some() {
            return base;
        }
        match self.fx().splashes[role.index()].active() {
            Some(sp) => splash_overlay(base, sp, now),
            None => base,
        }
    }

    /// Role scene without transition or splash (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs) -> Scene {
        if self.needs_data(role) { no_data_scene(now) } else { role_scene(self, role, now) }
    }
```
`needs_data` for the six new roles returns `true` for now (Tasks 4 and 5 fill them in). `role_scene` in `scene.rs` gets a `_ => no_data_scene(now)` arm for the new roles until Tasks 4/5 replace it.

Update tests in `scene_fx.rs`: `ready_model()` calls `m.set_screens(vec![vec![Role::Cpu], vec![Role::Mem], vec![Role::Pods], vec![Role::Health]], vec![15.0; 4])`; every `m.scene(Role::X, t)` becomes `m.scene(Role::X.index(), t)` for the four classic roles (index equals screen index in that layout). Add:
```rust
    #[test]
    fn cycling_screen_transitions_with_iris() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        // at 3.3 s: transition started at ~3.0, first half shows cpu shrinking
        let s = m.scene(0, 3.15);
        assert!(s.zoom < 1.0 && s.zoom > 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cpu"));
        let s = m.scene(0, 3.4);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "memory-stick"), "second half shows the incoming role");
        for i in 101..=200 {
            m.tick(i as f64 * 0.033);
        }
        assert_eq!(m.current_role(0), Role::Mem);
        assert_eq!(m.scene(0, 6.6).zoom, 1.0);
    }
```
In `boot_uses_role_colours` nothing changes. In `sweep_takes_over_all_screens_and_suppresses_splash` use `sw.phase(screen, 4, ..)` semantics via `m.scene(index, ..)`.

- [ ] **Step 7: render: apply zoom and ring_reveal**

`crates/render/src/renderer.rs`: in `render`, if `scene.zoom < 0.999 || scene.ring_reveal < 0.999`, render the items into a scratch pixmap (`self.scratch`, a `new_pixmap()` kept in the struct) with rings truncated, then clear `px` to black and `px.draw_pixmap(0, 0, scratch.as_ref(), &PixmapPaint::default(), Transform::from_translate(120.0, 120.0).pre_scale(zoom, zoom).pre_translate(-120.0, -120.0), None)`. Ring truncation: for `Drawable::Ring { states, .. }` compute `keep = (states.len() as f32 * reveal).round() as usize` and treat indices `>= keep` as `SegState::Off`. Factor the per-drawable drawing into `fn draw_item(&mut self, px, item, reveal)`. A zoom of 0 draws nothing but the black clear. Test in `renderer.rs`: render the CPU idle scene with `zoom = 0.5` and assert the pixel at (120, 18) (segment 0 at full scale) is black while (120, 69) (segment 0 at half scale) is amber-ish; with `ring_reveal = 0.5` on a full ring, the pixel at (120, 222) (segment 30) is OFF grey.

- [ ] **Step 8: app wiring**

`runloop.rs`: `ScreenSlot` gets `pub index: usize`; `ScreenSlot::new(index, role, orient, mailbox)`; the loop uses `model.scene(s.index, now)`. `run.rs`: after `Model::new(...)` inside `RenderLoop::run` the model needs the screens: add `pub screens_roles: Vec<Vec<Role>>` and `pub cycle_secs: Vec<f64>` to `RenderLoop`, and in `run()` call `model.set_screens(self.screens_roles.clone(), self.cycle_secs.clone())` right after `Model::new`. In `run.rs` build them from the handles: `handles.iter().map(|h| vec![h.role]).collect()` and `vec![15.0; n]` (Task 2 replaces this with config lists). `calibrate.rs` unchanged. Golden tests: `m.scene(Role::Cpu, 5.0)` becomes `m.scene_for_role(Role::Cpu, 5.0)` everywhere in `crates/render/tests/golden.rs`; the `sweep_hold` golden uses `m.scene(3, 6.0)` after `set_screens` with the classic layout (add `set_screens` to `ready_model()` there too).

- [ ] **Step 9: Test, commit**

Run: `cargo test --workspace --features sim,pi` (goldens unchanged), the three clippy builds, `cargo fmt --all`, `timeout 12 target/debug/rackscreen run --sim` still runs.

```bash
git add -A
git commit -m "refactor(core): ten roles, screen-indexed sweeps, per-screen cycling with iris transition"
```

---

### Task 2: Config: role lists, cycle_secs, electricity, price, hot_temp

**Files:**
- Modify: `crates/app/src/config.rs`, `config.example.yaml`, `crates/app/src/panels.rs`, `crates/app/src/run.rs`, `crates/app/src/calibrate.rs` (callers), `crates/setup/src/screens/calibrate.rs` (uses `scr.role()` if any)

**Interfaces:**
- Produces:
  - `ScreenCfg { roles: Vec<String>, cycle_secs: u64, spi, cs, dc, rst, rotate, hflip, hz }` with `#[serde(default, skip_serializing)] role: Option<String>` for the legacy key; `ScreenCfg::roles(&self) -> Result<Vec<Role>>`, `ScreenCfg::first_role(&self) -> Result<Role>`; `Config::normalize(&mut self)` (moves `role` into `roles`, called by `from_yaml`).
  - `ElectricityCfg { enabled: bool, zone: String, token: String, poll_secs: u64 }`, `PriceCfg { source: String /* energyzero|entsoe|none */, entsoe_token: String, entsoe_zone: String, include_vat: bool, poll_secs: u64 }`, `ThresholdsCfg.hot_temp: f32`, `Config.electricity`, `Config.price`.
  - `PanelHandle.roles: Vec<Role>` (replaces `role`), `PanelHandle.cycle_secs: u64`; `run.rs` passes config lists to `RenderLoop`.

- [ ] **Step 1: config.rs changes**

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ScreenCfg {
    /// Legacy single role; upgraded into `roles` by `Config::normalize`.
    #[serde(default, skip_serializing)]
    pub role: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default = "default_cycle")]
    pub cycle_secs: u64,
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

fn default_cycle() -> u64 {
    15
}

impl ScreenCfg {
    pub fn roles(&self) -> Result<Vec<Role>> {
        anyhow::ensure!(!self.roles.is_empty(), "screen has no roles");
        self.roles
            .iter()
            .map(|r| Role::parse(r).with_context(|| format!("unknown screen role '{r}'")))
            .collect()
    }
    pub fn first_role(&self) -> Result<Role> {
        Ok(self.roles()?[0])
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ElectricityCfg {
    pub enabled: bool,
    pub zone: String,
    pub token: String,
    pub poll_secs: u64,
}
impl Default for ElectricityCfg {
    fn default() -> Self {
        Self { enabled: false, zone: "NL".into(), token: String::new(), poll_secs: 300 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PriceCfg {
    pub source: String,
    pub entsoe_token: String,
    pub entsoe_zone: String,
    pub include_vat: bool,
    pub poll_secs: u64,
}
impl Default for PriceCfg {
    fn default() -> Self {
        Self { source: "energyzero".into(), entsoe_token: String::new(), entsoe_zone: String::new(), include_vat: true, poll_secs: 900 }
    }
}
```
`ThresholdsCfg` gains `pub hot_temp: f32` (default 70.0). `Config` gains `#[serde(default)] pub electricity: ElectricityCfg` and `#[serde(default)] pub price: PriceCfg`. Add:
```rust
impl Config {
    /// Upgrade legacy fields in place.
    pub fn normalize(&mut self) {
        for s in &mut self.screens {
            if let Some(r) = s.role.take() {
                if s.roles.is_empty() {
                    s.roles = vec![r];
                }
            }
        }
    }
}
```
`from_yaml` calls `cfg.normalize()` before returning. `validate()` checks every screen's `roles()`, `cycle_secs` in `3..=300`, `price.source` in `{energyzero, entsoe, none}`, and `electricity.zone` non-empty when enabled. Replace uses of `ScreenCfg.role` with `first_role()` (calibrate accent, thread names use `roles[0]` string) and `s.role()` with `s.roles()` in `panels.rs` (`PanelHandle { roles, cycle_secs, .. }`, `role` removed; `PanelHandle.first_role()` helper). `run.rs`: `screens_roles: handles.iter().map(|h| h.roles.clone()).collect()`, `cycle_secs: handles.iter().map(|h| h.cycle_secs as f64).collect()`; `ScreenSlot::new(i, h.first_role(), ..)`. The `panels.rs` test that sets `cfg.screens[1].role = "nope"` now sets `roles = vec!["nope".into()]`.

`config.example.yaml`: screens use `roles: [cpu]` etc. with `cycle_secs: 15`, add:
```yaml
thresholds:
  hot_cpu: 90
  hot_mem: 90
  hot_temp: 70

electricity:
  enabled: false           # set true and add your free token from app.electricitymaps.com
  zone: NL
  token: ""
  poll_secs: 300

price:
  source: energyzero       # energyzero (NL, no token) | entsoe (EU, token) | none
  entsoe_token: ""
  entsoe_zone: ""          # EIC code, e.g. 10YNL----------L; derived from zone when empty
  include_vat: true
  poll_secs: 900
```

- [ ] **Step 2: Tests**

In `config.rs` tests: legacy `role: cpu` parses to `roles == ["cpu"]` and serialises without `role`; `roles: [cpu, thermal]` with `cycle_secs: 20` round-trips; `roles: [nope]` fails validation mentioning `nope`; `cycle_secs: 1` fails; `price.source: foo` fails; defaults `electricity.enabled == false`, `price.source == "energyzero"`, `thresholds.hot_temp == 70.0`. Update `example_parses_with_four_screens` to check `screens[3].first_role() == Role::Health`.

- [ ] **Step 3: Test, commit**

Run the full suite, clippy, fmt; the simulator still starts with the example config.

```bash
git add -A
git commit -m "feat(config): role lists with cycling, electricity, price and hot_temp settings"
```

---

### Task 3: Render support: ring pitch/start, tick drawable, per-icon units, new icons, wide badge

**Files:**
- Create: `assets/icons/em-biomass.svg`, `em-geothermal.svg`, `em-hydro.svg`, `em-solar.svg`, `em-wind.svg`, `em-nuclear.svg`, `em-battery-storage.svg`, `em-hydro-storage.svg`, `em-coal.svg`, `em-gas.svg`, `em-oil.svg`, `em-unknown.svg`, `assets/icons/EM-LICENSE.md`
- Modify: `scripts/fetch-assets.sh`, `crates/render/src/assets.rs`, `crates/render/src/icons.rs`, `crates/render/src/prims.rs`, `crates/render/src/renderer.rs`, `crates/core/src/scene.rs`, `crates/core/src/theme.rs`

**Interfaces:**
- Produces:
  - `Drawable::Ring { cx, cy, radius, n, states, pitch_deg: f32, start_deg: f32 }` (`scene::ring()` helper fills `360/n` and `0.0`); `Drawable::Tick { cx, cy, angle_deg, r0, r1, width, color, alpha }`.
  - `scene::node_dots_badge(cy, stroke, colors: Vec<Color>, breathe_idx: Option<usize>, now) -> Vec<Drawable>` returning a wide Badge plus Dots (`w = max(64, 12·n + 16)`, spacing 12, radius 4.5).
  - `prims::SegmentCache::segments_pitched(&mut self, cx, cy, radius, n, pitch_deg, start_deg) -> &[Path]`.
  - Icons: per-icon `units` from the SVG viewBox; new Lucide icons `euro`, `cloud`, `leaf`, `thermometer`, `database`, `database-zap`, `key-round`, `zap`; EM icons `em-biomass … em-unknown`.
  - `theme::em::{Source colours}` lives in core in Task 5; this task only ships the icon files.

- [ ] **Step 1: Icon files**

Write the twelve files exactly (viewBox 0 0 8 8 unless noted, single `<path>` with `fill="currentColor"` so the loader collects a fill path; the original markup used white/black fills which the renderer replaces with the drawable colour anyway):

`assets/icons/em-biomass.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M4.79688 3.23438C6.09375 3.09375 7.10938 2.0625 7.23438 0.75H7.125C5.92188 0.75 4.90625 1.53125 4.51562 2.59375C4.39062 2.32812 4.21875 2.07812 4.03125 1.85938C4.625 0.75 5.78125 0 7.125 0H7.5C7.76562 0 8 0.234375 8 0.5C8 2.28125 6.67188 3.75 4.95312 3.96875C4.92188 3.71875 4.875 3.48438 4.79688 3.23438ZM0.75 1.75V2C0.75 3.53125 1.96875 4.75 3.5 4.75H3.625V4.5C3.625 2.98438 2.39062 1.75 0.875 1.75H0.75ZM4.375 4.5V4.75V5.5V7.625C4.375 7.84375 4.20312 8 4 8C3.78125 8 3.625 7.84375 3.625 7.625V5.5H3.5C1.5625 5.5 0 3.9375 0 2V1.5C0 1.23438 0.21875 1 0.5 1H0.875C2.79688 1 4.375 2.57812 4.375 4.5Z" fill="currentColor"/></svg>
```
`assets/icons/em-geothermal.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M3.14286 0C3.375 0 3.57143 0.196429 3.57143 0.428571V2C3.57143 2.66071 3.76786 3.28571 4.16071 3.80357L4.5 4.26786C5 4.92857 5.28571 5.75 5.28571 6.57143V7.57143C5.28571 7.82143 5.08929 8 4.85714 8C4.60714 8 4.42857 7.82143 4.42857 7.57143V6.57143C4.42857 5.92857 4.21429 5.30357 3.82143 4.78571L3.48214 4.32143C2.98214 3.66071 2.71429 2.83929 2.71429 2V0.428571C2.71429 0.196429 2.89286 0 3.14286 0ZM0.428571 1.14286C0.660714 1.14286 0.857143 1.33929 0.857143 1.57143V2.25C0.857143 2.85714 1.01786 3.42857 1.35714 3.92857L1.91071 4.76786C2.33929 5.39286 2.57143 6.14286 2.57143 6.91071V7.57143C2.57143 7.82143 2.375 8 2.14286 8C1.89286 8 1.71429 7.82143 1.71429 7.57143V6.91071C1.71429 6.30357 1.53571 5.73214 1.19643 5.23214L0.642857 4.39286C0.214286 3.76786 0 3.01786 0 2.25V1.57143C0 1.33929 0.178571 1.14286 0.428571 1.14286ZM6.28571 1.57143V2.25C6.28571 2.85714 6.44643 3.42857 6.78571 3.92857L7.33929 4.76786C7.76786 5.39286 8 6.14286 8 6.91071V7.57143C8 7.82143 7.80357 8 7.57143 8C7.32143 8 7.14286 7.82143 7.14286 7.57143V6.91071C7.14286 6.30357 6.96429 5.73214 6.625 5.23214L6.07143 4.39286C5.64286 3.76786 5.42857 3.01786 5.42857 2.25V1.57143C5.42857 1.33929 5.60714 1.14286 5.85714 1.14286C6.08929 1.14286 6.28571 1.33929 6.28571 1.57143Z" fill="currentColor"/></svg>
```
`assets/icons/em-hydro.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M1.11945 1.1075C1.2444 0.964167 1.42489 0.964167 1.54985 1.1075C1.85529 1.39416 2.2718 1.63305 2.66055 1.63305C3.06318 1.63305 3.47969 1.39416 3.77125 1.1075C3.91009 0.964167 4.09058 0.964167 4.21553 1.1075C4.52098 1.39416 4.93749 1.63305 5.32624 1.63305C5.72887 1.63305 6.14538 1.39416 6.43694 1.1075C6.5619 0.964167 6.75627 0.964167 6.88122 1.1075C7.11725 1.34638 7.43658 1.52157 7.74202 1.60119C7.90862 1.64897 8.03358 1.85601 7.99193 2.06304C7.95028 2.27007 7.76979 2.39748 7.5893 2.3497C7.18667 2.25415 6.86734 2.03119 6.65908 1.87193C6.27034 2.17452 5.81217 2.4134 5.32624 2.4134C4.85419 2.4134 4.38214 2.17452 3.99339 1.87193C3.60465 2.17452 3.14648 2.4134 2.66055 2.4134C2.1885 2.4134 1.71645 2.17452 1.3277 1.87193C1.13333 2.03119 0.80012 2.25415 0.411374 2.3497C0.230884 2.39748 0.050395 2.27007 0.00874358 2.06304C-0.0329078 1.85601 0.0781626 1.64897 0.258652 1.60119C0.577979 1.52157 0.869539 1.33046 1.11945 1.1075ZM1.11945 5.69409C1.2444 5.55076 1.42489 5.55076 1.54985 5.69409C1.85529 5.98076 2.2718 6.21964 2.66055 6.21964C3.06318 6.21964 3.47969 5.98076 3.77125 5.69409C3.91009 5.55076 4.09058 5.55076 4.21553 5.69409C4.52098 5.98076 4.93749 6.21964 5.32624 6.21964C5.72887 6.21964 6.14538 5.98076 6.43694 5.69409C6.5619 5.55076 6.75627 5.55076 6.88122 5.69409C7.11725 5.93298 7.43658 6.10816 7.74202 6.18779C7.90862 6.23557 8.03358 6.4426 7.99193 6.64963C7.95028 6.85667 7.76979 6.98407 7.5893 6.9363C7.18667 6.84074 6.86734 6.61778 6.65908 6.45853C6.27034 6.76111 5.81217 7 5.32624 7C4.85419 7 4.38214 6.76111 3.99339 6.45853C3.60465 6.76111 3.14648 7 2.66055 7C2.1885 7 1.71645 6.76111 1.3277 6.45853C1.13333 6.61778 0.80012 6.84074 0.411374 6.9363C0.230884 6.98407 0.050395 6.85667 0.00874358 6.64963C-0.0329078 6.4426 0.0781626 6.23557 0.258652 6.18779C0.564095 6.10816 0.869539 5.93298 1.10556 5.69409H1.11945ZM1.54985 3.4008C1.85529 3.68746 2.2718 3.92634 2.66055 3.92634C3.06318 3.92634 3.47969 3.68746 3.77125 3.4008C3.91009 3.25747 4.09058 3.25747 4.21553 3.4008C4.52098 3.68746 4.93749 3.92634 5.32624 3.92634C5.72887 3.92634 6.14538 3.68746 6.43694 3.4008C6.5619 3.25747 6.75627 3.25747 6.88122 3.4008C7.11725 3.63968 7.43658 3.81486 7.74202 3.89449C7.90862 3.94227 8.03358 4.1493 7.99193 4.35634C7.95028 4.56337 7.76979 4.69078 7.5893 4.643C7.18667 4.54745 6.86734 4.32449 6.65908 4.16523C6.27034 4.46782 5.81217 4.7067 5.32624 4.7067C4.85419 4.7067 4.38214 4.46782 3.99339 4.16523C3.60465 4.46782 3.14648 4.7067 2.66055 4.7067C2.1885 4.7067 1.71645 4.46782 1.3277 4.16523C1.13333 4.32449 0.80012 4.54745 0.411374 4.643C0.230884 4.69078 0.050395 4.56337 0.00874358 4.35634C-0.0329078 4.1493 0.0781626 3.94227 0.258652 3.89449C0.564095 3.81486 0.869539 3.63968 1.10556 3.4008C1.23052 3.27339 1.42489 3.27339 1.54985 3.4008Z" fill="currentColor"/></svg>
```
`assets/icons/em-solar.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M5.86582 0.307392L6.14686 1.85313L7.69261 2.13418C7.81752 2.14979 7.92681 2.24347 7.97365 2.35277C8.02049 2.47768 8.00488 2.60259 7.92681 2.71188L7.03684 3.99219L7.92681 5.28812C8.00488 5.39741 8.02049 5.52232 7.97365 5.64723C7.92681 5.75653 7.81752 5.85021 7.69261 5.86582L6.14686 6.14686L5.86582 7.69261C5.85021 7.81752 5.75653 7.92681 5.64723 7.97365C5.52232 8.02049 5.39741 8.00488 5.28812 7.92681L4.00781 7.03684L2.71188 7.92681C2.60259 8.00488 2.47768 8.02049 2.35277 7.97365C2.24347 7.9112 2.14979 7.81752 2.13418 7.69261L1.85313 6.14686L0.307392 5.86582C0.182484 5.85021 0.0731886 5.75653 0.0263479 5.64723C-0.0204928 5.52232 -0.00487924 5.39741 0.0731886 5.28812L0.963162 3.99219L0.0731886 2.71188C-0.00487924 2.60259 -0.0204928 2.47768 0.0263479 2.35277C0.0888021 2.24347 0.182484 2.14979 0.307392 2.13418L1.85313 1.85313L2.13418 0.307392C2.14979 0.182484 2.24347 0.0731886 2.35277 0.0263479C2.47768 -0.0204928 2.60259 -0.00487924 2.71188 0.0731886L4.00781 0.963162L5.28812 0.0731886C5.39741 -0.00487924 5.52232 -0.0204928 5.64723 0.0263479C5.75653 0.0731886 5.85021 0.182484 5.86582 0.307392ZM4.21078 1.71261C4.08587 1.80629 3.91413 1.80629 3.78922 1.71261L2.75872 1.01L2.54013 2.24347C2.5089 2.384 2.384 2.5089 2.24347 2.54013L1.01 2.75872L1.72823 3.78922C1.80629 3.91413 1.80629 4.08587 1.72823 4.21078L1.01 5.24128L2.24347 5.45987C2.384 5.4911 2.5089 5.616 2.54013 5.75653L2.75872 6.99L3.78922 6.27177C3.91413 6.19371 4.08587 6.19371 4.21078 6.27177L5.24128 6.99L5.45987 5.75653C5.4911 5.616 5.616 5.4911 5.75653 5.45987L6.99 5.24128L6.28739 4.21078C6.19371 4.08587 6.19371 3.91413 6.28739 3.78922L6.99 2.75872L5.75653 2.54013C5.616 2.5089 5.4911 2.384 5.45987 2.24347L5.24128 1.01L4.21078 1.71261ZM4.00781 5.74091C3.36765 5.74091 2.80556 5.41303 2.49329 4.86655C2.16541 4.33569 2.16541 3.66431 2.49329 3.11783C2.80556 2.58697 3.36765 2.24347 4.00781 2.24347C4.63235 2.24347 5.19444 2.58697 5.50671 3.11783C5.83459 3.66431 5.83459 4.33569 5.50671 4.86655C5.19444 5.41303 4.63235 5.74091 4.00781 5.74091ZM3.00854 3.99219C3.00854 4.35131 3.1959 4.67919 3.50817 4.86655C3.80483 5.0383 4.19517 5.0383 4.50744 4.86655C4.8041 4.67919 5.00707 4.35131 5.00707 3.99219C5.00707 3.64869 4.8041 3.32081 4.50744 3.13345C4.19517 2.9617 3.80483 2.9617 3.50817 3.13345C3.1959 3.32081 3.00854 3.64869 3.00854 3.99219Z" fill="currentColor"/></svg>
```
`assets/icons/em-wind.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M4.5 0.375C4.5 0.171875 4.65625 0 4.875 0H5.5625C6.34375 0 7 0.65625 7 1.4375C7 2.23438 6.34375 2.875 5.5625 2.875H0.375C0.15625 2.875 0 2.71875 0 2.5C0 2.29688 0.15625 2.125 0.375 2.125H5.5625C5.9375 2.125 6.25 1.82812 6.25 1.4375C6.25 1.0625 5.9375 0.75 5.5625 0.75H4.875C4.65625 0.75 4.5 0.59375 4.5 0.375ZM5.5 6.125C5.5 5.92188 5.65625 5.75 5.875 5.75H6.5625C6.9375 5.75 7.25 5.45312 7.25 5.0625C7.25 4.6875 6.9375 4.375 6.5625 4.375H0.375C0.15625 4.375 0 4.21875 0 4C0 3.79688 0.15625 3.625 0.375 3.625H6.5625C7.34375 3.625 8 4.28125 8 5.0625C8 5.85938 7.34375 6.5 6.5625 6.5H5.875C5.65625 6.5 5.5 6.34375 5.5 6.125ZM1.875 8C1.65625 8 1.5 7.84375 1.5 7.625C1.5 7.42188 1.65625 7.25 1.875 7.25H2.5625C2.9375 7.25 3.25 6.95312 3.25 6.5625C3.25 6.1875 2.9375 5.875 2.5625 5.875H0.375C0.15625 5.875 0 5.71875 0 5.5C0 5.29688 0.15625 5.125 0.375 5.125H2.5625C3.34375 5.125 4 5.78125 4 6.5625C4 7.35938 3.34375 8 2.5625 8H1.875Z" fill="currentColor"/></svg>
```
`assets/icons/em-nuclear.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M1.02787 6.97213C1.17068 7.11493 1.59909 7.27559 2.47377 6.97213C2.70583 6.90072 2.95574 6.79362 3.20565 6.66867C2.86649 6.40091 2.52733 6.1153 2.20601 5.79399C1.8847 5.47267 1.59909 5.13351 1.33133 4.79435C1.20638 5.04426 1.09928 5.29417 1.02787 5.52623C0.724414 6.40091 0.88507 6.82932 1.02787 6.97213ZM0.795817 3.99107C-0.0788629 2.52733 -0.27522 1.11713 0.420954 0.420954C1.11713 -0.27522 2.52733 -0.0788629 4.00893 0.795817C5.47267 -0.0788629 6.88287 -0.27522 7.57905 0.420954C8.27522 1.11713 8.07886 2.52733 7.20418 3.99107C8.07886 5.47267 8.27522 6.88287 7.57905 7.57905C6.88287 8.27522 5.47267 8.07886 4.00893 7.20418C2.52733 8.07886 1.11713 8.27522 0.420954 7.57905C-0.27522 6.88287 -0.0788629 5.47267 0.795817 3.99107ZM1.34919 3.20565C1.59909 2.86649 1.8847 2.52733 2.20601 2.20601C2.54518 1.8847 2.86649 1.59909 3.20565 1.33133C2.95574 1.20638 2.70583 1.09928 2.47377 1.01002C1.59909 0.724414 1.17068 0.88507 1.02787 1.02787C0.88507 1.17068 0.724414 1.58124 1.02787 2.45592C1.09928 2.70583 1.20638 2.95574 1.34919 3.20565ZM4.00893 1.8133C3.59836 2.09891 3.20565 2.43807 2.81293 2.81293C2.43807 3.20565 2.09891 3.59836 1.83115 3.99107C2.09891 4.40164 2.43807 4.79435 2.81293 5.18707C3.20565 5.56193 3.59836 5.90109 4.00893 6.16885C4.40164 5.90109 4.79435 5.56193 5.18707 5.18707C5.56193 4.79435 5.90109 4.40164 6.1867 3.99107C5.90109 3.59836 5.56193 3.20565 5.18707 2.81293C4.79435 2.43807 4.40164 2.09891 4.00893 1.8133ZM6.66867 3.20565C6.79362 2.95574 6.90072 2.70583 6.97213 2.47377C7.27559 1.59909 7.11493 1.17068 6.97213 1.02787C6.82932 0.88507 6.40091 0.724414 5.52623 1.02787C5.29417 1.09928 5.04426 1.20638 4.79435 1.33133C5.13351 1.59909 5.47267 1.8847 5.79399 2.20601C6.1153 2.52733 6.40091 2.86649 6.66867 3.20565ZM6.66867 4.79435C6.40091 5.13351 6.1153 5.47267 5.79399 5.79399C5.47267 6.1153 5.13351 6.40091 4.79435 6.66867C5.04426 6.79362 5.29417 6.90072 5.52623 6.97213C6.40091 7.27559 6.82932 7.11493 6.97213 6.97213C7.11493 6.82932 7.27559 6.40091 6.97213 5.52623C6.90072 5.29417 6.79362 5.04426 6.66867 4.79435ZM3.43771 3.99107C3.43771 3.68761 3.68761 3.41986 4.00893 3.41986C4.31239 3.41986 4.58014 3.68761 4.58014 3.99107C4.58014 4.31239 4.31239 4.56229 4.00893 4.56229C3.68761 4.56229 3.43771 4.31239 3.43771 3.99107Z" fill="currentColor"/></svg>
```
`assets/icons/em-battery-storage.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M3.32996 1.99999C3.32996 1.88953 3.4195 1.79999 3.52996 1.79999H4.45996C4.57041 1.79999 4.65996 1.88953 4.65996 1.99999V6.39999C4.65996 6.51045 4.57041 6.59999 4.45996 6.59999H3.52996C3.4195 6.59999 3.32996 6.51045 3.32996 6.39999V1.99999Z" fill="currentColor"/><path fill-rule="evenodd" clip-rule="evenodd" d="M3.62996 0C3.46427 0 3.32996 0.134315 3.32996 0.3V0.5H3C2.44772 0.5 2 0.947715 2 1.5V7C2 7.55228 2.44772 8 3 8H5C5.55228 8 6 7.55228 6 7V1.5C6 0.947715 5.55228 0.5 5 0.5H4.65996V0.3C4.65996 0.134315 4.52564 0 4.35996 0H3.62996ZM3 1.25H5C5.13807 1.25 5.25 1.36193 5.25 1.5V7C5.25 7.13807 5.13807 7.25 5 7.25H3C2.86193 7.25 2.75 7.13807 2.75 7V1.5C2.75 1.36193 2.86193 1.25 3 1.25Z" fill="currentColor"/></svg>
```
`assets/icons/em-hydro-storage.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M4.375 0.875V1.20312L6 1C6.26562 1 6.5 1.23438 6.5 1.5C6.5 1.78125 6.26562 2 6 2L4.375 1.79688V3H7.25V2.875C7.25 2.67188 7.40625 2.5 7.625 2.5C7.82812 2.5 8 2.67188 8 2.875V3.375V6.625V7.125C8 7.34375 7.82812 7.5 7.625 7.5C7.40625 7.5 7.25 7.34375 7.25 7.125V7H0.75V7.125C0.75 7.34375 0.578125 7.5 0.375 7.5C0.15625 7.5 0 7.34375 0 7.125V6.625V3.375V2.875C0 2.67188 0.15625 2.5 0.375 2.5C0.578125 2.5 0.75 2.67188 0.75 2.875V3H3.625V1.79688L2 2C1.71875 2 1.5 1.78125 1.5 1.5C1.5 1.23438 1.71875 1 2 1L3.625 1.20312V0.875C3.625 0.671875 3.78125 0.5 4 0.5C4.20312 0.5 4.375 0.671875 4.375 0.875ZM0.75 6.25H7.25V3.75H4H0.75V6.25Z" fill="currentColor"/></svg>
```
`assets/icons/em-coal.svg` (16-unit viewBox, two rocks as on the site; the second is stroked):
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><g transform="matrix(1.75,0,0,1.75,0,0)"><path fill-rule="evenodd" clip-rule="evenodd" d="M4.787,7.675L4.601,7.792C4.243,8.018 3.8,8.063 3.404,7.912L0.421,7.232C-0.029,6.805 -0.131,4.983 0.176,4.444L2.314,0.682C2.687,0.027 3.524,-0.196 4.173,0.188L6.47,1.548C6.727,1.7 6.926,1.934 7.036,2.212L7.905,4.406C8.047,4.766 8.025,5.155 7.868,5.484L7.213,4.685C7.198,4.668 7.183,4.651 7.167,4.634L6.32,2.495C6.273,2.376 6.188,2.276 6.077,2.211L3.781,0.851C3.503,0.686 3.144,0.782 2.984,1.063L0.846,4.825C0.714,5.056 0.758,6.49 0.951,6.672L3.677,7.192C3.846,7.257 4.036,7.237 4.19,7.14L4.626,6.865L4.787,7.675Z" fill="currentColor"/></g><g transform="matrix(-0.358028,0.932047,-0.932047,-0.358028,16.827379,9.62654)"><path fill-rule="nonzero" d="M3.781,0.851C3.503,0.686 3.144,0.782 2.984,1.063L0.846,4.825C0.714,5.056 0.758,5.347 0.951,5.53L1.974,6.499C2.03,6.552 2.095,6.593 2.167,6.62L3.677,7.192C3.846,7.257 4.036,7.237 4.19,7.14L6.96,5.391C7.195,5.243 7.291,4.948 7.189,4.69L6.32,2.495C6.273,2.376 6.188,2.276 6.077,2.211L3.781,0.851ZM2.314,0.682C2.687,0.027 3.524,-0.196 4.173,0.188L6.47,1.548C6.727,1.7 6.926,1.934 7.036,2.212L7.905,4.406C8.144,5.009 7.92,5.696 7.372,6.042L4.601,7.792C4.243,8.018 3.8,8.063 3.404,7.912L1.894,7.34C1.727,7.277 1.574,7.181 1.445,7.058L0.421,6.089C-0.029,5.662 -0.131,4.983 0.176,4.444L2.314,0.682Z" fill="currentColor" stroke="currentColor" stroke-width="0.45"/></g></svg>
```
`assets/icons/em-gas.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M1.89062 2.44499C1.5 3.1335 1.25 3.83765 1.25 4.41663C1.25 6.02836 2.45312 7.2489 4 7.2489C5.51562 7.2489 6.75 6.02836 6.75 4.41663C6.75 3.96284 6.57812 3.35257 6.29688 2.75795C6.04688 2.19462 5.71875 1.69389 5.40625 1.36528C5.34375 1.44352 5.26562 1.55306 5.17188 1.66259C5.14062 1.72518 5.09375 1.78778 5.04688 1.85037C4.98438 1.94425 4.90625 2.05379 4.85938 2.11638C4.79688 2.21027 4.6875 2.25721 4.5625 2.27286C4.45312 2.27286 4.34375 2.21027 4.26562 2.11638C4.21875 2.05379 4.15625 1.97555 4.09375 1.91296C3.79688 1.52176 3.45312 1.08362 3.15625 0.786308C2.73438 1.19315 2.26562 1.78778 1.89062 2.44499ZM3.60938 0.176039C3.90625 0.457702 4.25 0.880196 4.54688 1.25575L4.5625 1.2401C4.67188 1.06797 4.8125 0.880196 4.9375 0.755012C5.20312 0.520293 5.59375 0.520293 5.85938 0.755012C6.26562 1.16186 6.67188 1.78778 6.96875 2.44499C7.28125 3.08655 7.5 3.80636 7.5 4.41663C7.5 6.43521 5.9375 8 4 8C2.03125 8 0.5 6.41956 0.5 4.41663C0.5 3.63423 0.8125 2.80489 1.23438 2.06944C1.65625 1.33399 2.20312 0.645477 2.70312 0.176039C2.96875 -0.0586797 3.35938 -0.0586797 3.60938 0.176039ZM4.01562 6.4978C3.03125 6.4978 2.25 5.87188 2.23438 4.79218C2.23438 4.32274 2.5 3.90024 3.03125 3.24303C3.14062 3.11785 3.32812 3.11785 3.42188 3.24303C3.6875 3.57164 4.14062 4.16626 4.40625 4.49487C4.5 4.62005 4.6875 4.62005 4.78125 4.49487L5.1875 4.04108C5.28125 3.91589 5.46875 3.93154 5.53125 4.07237C5.92188 4.79218 5.75 5.7154 5.09375 6.16919C4.76562 6.38826 4.42188 6.4978 4.01562 6.4978Z" fill="currentColor"/></svg>
```
`assets/icons/em-oil.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M4 7.25C5.23438 7.25 6.25 6.25 6.25 5C6.25 4.8125 6.15625 4.48438 5.98438 4.04688C5.79688 3.64062 5.54688 3.1875 5.26562 2.73438C4.8125 2 4.3125 1.3125 4 0.90625C3.67188 1.3125 3.17188 2 2.71875 2.73438C2.4375 3.1875 2.1875 3.64062 2.01562 4.04688C1.82812 4.48438 1.75 4.79688 1.75 5C1.75 6.25 2.75 7.25 4 7.25ZM1 5C1 3.57812 3.03125 0.90625 3.59375 0.1875C3.6875 0.078125 3.82812 0 3.98438 0H4C4.15625 0 4.29688 0.078125 4.39062 0.1875C4.95312 0.90625 7 3.57812 7 5C7 6.65625 5.65625 8 4 8C2.34375 8 1 6.65625 1 5ZM3.25 4.875C3.25 5.35938 3.64062 5.75 4.125 5.75C4.32812 5.75 4.5 5.92188 4.5 6.125C4.5 6.34375 4.32812 6.5 4.125 6.5C3.21875 6.5 2.5 5.78125 2.5 4.875C2.5 4.67188 2.65625 4.5 2.875 4.5C3.07812 4.5 3.25 4.67188 3.25 4.875Z" fill="currentColor"/></svg>
```
`assets/icons/em-unknown.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="none"><path d="M2.50781 2.5C2.50781 2.71875 2.33594 2.875 2.13281 2.875C1.91406 2.875 1.75781 2.71875 1.75781 2.5C1.75781 1.40625 2.64844 0.5 3.75781 0.5H4.24219C5.35156 0.5 6.24219 1.40625 6.24219 2.5V2.57812C6.24219 3.20312 5.92969 3.79688 5.41406 4.14062L4.60156 4.6875C4.46094 4.78125 4.38281 4.9375 4.38281 5.09375V5.125C4.38281 5.34375 4.21094 5.5 4.00781 5.5C3.78906 5.5 3.63281 5.34375 3.63281 5.125V5.09375C3.63281 4.6875 3.83594 4.29688 4.17969 4.0625L4.99219 3.51562C5.32031 3.3125 5.50781 2.95312 5.50781 2.57812V2.5C5.50781 1.8125 4.94531 1.25 4.25781 1.25H3.75781C3.05469 1.25 2.50781 1.8125 2.50781 2.5ZM4.00781 7.5C3.72656 7.5 3.50781 7.28125 3.50781 7C3.50781 6.73438 3.72656 6.5 4.00781 6.5C4.27344 6.5 4.50781 6.73438 4.50781 7C4.50781 7.28125 4.27344 7.5 4.00781 7.5Z" fill="currentColor"/></svg>
```
`assets/icons/EM-LICENSE.md`:
```markdown
The `em-*.svg` icons are the electricity source icons of the Electricity Maps web app
(https://app.electricitymaps.com, source: https://github.com/electricitymaps/electricitymaps-contrib),
licensed under the GNU Affero General Public License v3.0. They are redistributed here unchanged
apart from the fill colour being set to `currentColor`. See https://www.gnu.org/licenses/agpl-3.0.html.
Everything else in this repository is MIT licensed.
```
`scripts/fetch-assets.sh`: add `euro cloud leaf thermometer database database-zap key-round zap` to `ICONS` and run it once (the eight Lucide files land in `assets/icons/`). `crates/render/src/assets.rs`: add the eight Lucide names and the twelve `em-*` names to the `icons!` list. The existing `every_icon_parses_as_svg` test asserts `width == 24`; change it to `assert!(matches!(tree.size().width() as u32, 8 | 16 | 24))`.

- [ ] **Step 2: icons.rs: per-icon units**

`IconPaths` gains `units: f32`; `load` sets `units = tree.size().width()`; `draw` replaces `ICON_UNITS` with `icon.units` (`k = scale * size / icon.units`, `pre_translate(-icon.units / 2.0, -icon.units / 2.0)`). Add a test: draw `em-solar` at size 26 centred at (60, 60) and assert the lit bbox is centred within 1.5 px and 18..28 px wide; draw `em-coal` and assert something is lit.

- [ ] **Step 3: prims.rs: pitched segments**

```rust
    pub fn segments_pitched(&mut self, cx: f32, cy: f32, radius: f32, n: usize, pitch_deg: f32, start_deg: f32) -> &[Path] {
        use rackscreen_core::theme::layout::{SEG_LEN, SEG_W};
        let key = ((cx * 10.0) as u32, (cy * 10.0) as u32, (radius * 10.0) as u32, n, (pitch_deg * 100.0) as u32, ((start_deg.rem_euclid(360.0)) * 100.0) as u32);
        self.map
            .entry(key)
            .or_insert_with(|| (0..n).map(|i| segment_outline(cx, cy, radius, start_deg + i as f32 * pitch_deg, SEG_LEN, SEG_W)).collect())
            .as_slice()
    }
```
Change the cache key type to the six-tuple and make `segments(cx, cy, radius, n)` call `segments_pitched(cx, cy, radius, n, 360.0 / n as f32, 0.0)`. Note the cache grows with rotating rings (start_deg quantised to 0.01°); cap it: if `self.map.len() > 512`, clear it before inserting. Test: `segments_pitched(120, 120, 102, 48, 7.5, 0.0)` returns 48 paths and a 90° start puts segment 0 at 3 o'clock (pixel (222, 120) lit after fill).

- [ ] **Step 4: core scene.rs: Ring pitch/start, Tick, wide badge helper**

Add to `Drawable::Ring` the fields `pitch_deg: f32, start_deg: f32`; `scene::ring()` sets `360.0 / n as f32` and `0.0`. Add:
```rust
    Tick { cx: f32, cy: f32, angle_deg: f32, r0: f32, r1: f32, width: f32, color: Color, alpha: f32 },
```
and the helper:
```rust
/// A badge wide enough for `colors.len()` dots plus the dots themselves.
pub fn node_dots_badge(cy: f32, stroke: Color, colors: Vec<Color>, breathe_idx: Option<usize>, now: Secs) -> Vec<Drawable> {
    let n = colors.len();
    let w = (12.0 * n as f32 + 16.0).max(BADGE_W);
    let mut b = badge(cy, stroke, String::new());
    if let Drawable::Badge { w: bw, .. } = &mut b {
        *bw = w;
    }
    let colors = colors
        .into_iter()
        .enumerate()
        .map(|(i, c)| if Some(i) == breathe_idx { c.with_alpha(breathe(now, 2.4)) } else { c })
        .collect();
    vec![b, Drawable::Dots { cx: CX, cy, spacing: 12.0, r: 4.5, colors }]
}
```
Use it in the Health scene for the node dots (replacing the fixed-width badge + `DOT_SPACING` dots). Fix every `Drawable::Ring { .. }` literal (scene_fx.rs `sweep_scene`, app `calibrate.rs`) by adding `pitch_deg: 6.0, start_deg: 0.0`. Test: `node_dots_badge(165.0, GREEN, vec![GREEN; 8], None, 0.0)` gives a badge `w == 112.0` and 8 dots; with 3 dots `w == 64.0`.

- [ ] **Step 5: renderer.rs: Tick and pitched rings**

Ring arm uses `self.segs.segments_pitched(*cx, *cy, *radius, *n, *pitch_deg, *start_deg)`. Add:
```rust
                Drawable::Tick { cx, cy, angle_deg, r0, r1, width, color, alpha } => {
                    let path = segment_outline(*cx, *cy, (r0 + r1) / 2.0, *angle_deg, r1 - r0, *width);
                    fill(px, &path, *color, *alpha);
                }
```
(`segment_outline(cx, cy, radius, angle_deg, len, width)` draws a radial tick centred on `radius`, so passing the midpoint radius and `len = r1 - r0` spans exactly `r0..r1`.) Test: a `Tick` at angle 0 from r0 80 to r1 88 lights pixel (120, 36) and not (120, 30).

- [ ] **Step 6: Test, commit**

Full suite (goldens must stay unchanged since Health's badge is still 64 wide with 4 nodes: `12·4+16 = 64`), clippy, fmt.

```bash
git add -A
git commit -m "feat(render): pitched rings, tick drawable, per-icon units, Electricity Maps and new Lucide icons"
```

---

### Task 4: Thermal and Storage: events, model, scenes, splashes

**Files:**
- Create: `crates/core/src/scene_thermal.rs`, `crates/core/src/scene_storage.rs`
- Modify: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/fx.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`, `crates/core/src/theme.rs`, `crates/core/src/lib.rs`, `crates/render/tests/golden.rs`

**Interfaces:**
- Produces:
  - `event::Robustness { Healthy, Degraded, Faulted, Unknown }` with `Robustness::from_code(f64)`; `Event::NodeTemps(Vec<(String, f32)>)`, `Event::Storage { volumes: Vec<(String, Robustness)>, used_bytes: u64, capacity_bytes: u64 }`, `Event::HotTemp { node, celsius }`, `Event::VolumeDegraded { name, robustness }`, `Event::VolumeHealthy { name }`.
  - `theme::temp_color(celsius: f32) -> Color` (35 blue → 55 amber → 70 red, clamped), `theme::lerp(a, b, t)`.
  - `model::ClusterState { temps: Vec<(String, f32)>, have_temps, volumes: Vec<(String, Robustness)>, storage_used: u64, storage_capacity: u64, have_storage }`, `Model::smooth_hot_temp(now)`, `Model::smooth_storage_pct(now)`; `Thresholds.hot_temp: f32` (default 70).
  - `FxRequest::{HotTemp, VolumeDegraded, VolumeHealthy}`; `SplashKind::{HotTemp (red, "flame", role Thermal), VolumeDegraded (amber, "database-zap", role Storage), VolumeHealthy (green, "database", role Storage)}`.
  - `scene_thermal::thermal_scene(model, now) -> Scene`, `scene_storage::storage_scene(model, now) -> Scene`; `role_scene` dispatches `Role::Thermal`/`Role::Storage`; `needs_data` uses `have_temps`/`have_storage`.

- [ ] **Step 1: event.rs**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Robustness {
    Healthy,
    Degraded,
    Faulted,
    Unknown,
}

impl Robustness {
    /// Longhorn's `longhorn_volume_robustness` value.
    pub fn from_code(v: f64) -> Robustness {
        match v as i64 {
            1 => Robustness::Healthy,
            2 => Robustness::Degraded,
            3 => Robustness::Faulted,
            _ => Robustness::Unknown,
        }
    }
}
```
Event variants:
```rust
    NodeTemps(Vec<(String, f32)>),
    Storage { volumes: Vec<(String, Robustness)>, used_bytes: u64, capacity_bytes: u64 },
    HotTemp { node: String, celsius: f32 },
    VolumeDegraded { name: String, robustness: Robustness },
    VolumeHealthy { name: String },
```

- [ ] **Step 2: theme.rs colour helpers**

```rust
pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    a.mix(b, t)
}

/// Node temperature colour: 35 °C blue, 55 °C amber, 70 °C red.
pub fn temp_color(c: f32) -> Color {
    if c <= 55.0 {
        BLUE.mix(AMBER, ((c - 35.0) / 20.0).clamp(0.0, 1.0))
    } else {
        AMBER.mix(RED, ((c - 55.0) / 15.0).clamp(0.0, 1.0))
    }
}
```
Test: `temp_color(35.0) == BLUE`, `temp_color(55.0) == AMBER`, `temp_color(80.0) == RED`, `temp_color(45.0)` is between (r between BLUE.r and AMBER.r).

- [ ] **Step 3: model.rs**

State fields and `Thresholds { hot_cpu, hot_mem, hot_temp }` (default 70.0). `Model` gets `hot_temp: Smooth::new(0.0, SMOOTH_SECS)`, `storage_pct: Smooth::new(0.0, SMOOTH_SECS)`, `hot_temp_last: HashMap<String, Secs>`. Folds:
```rust
            Event::NodeTemps(list) => {
                let hottest = list.iter().map(|(_, c)| *c).fold(0.0_f32, f32::max);
                self.hot_temp.set(hottest, now);
                let th = self.thresholds.hot_temp;
                for (node, c) in &list {
                    if *c >= th {
                        let recently = self.hot_temp_last.get(node).is_some_and(|t| now - t < HOT_DEBOUNCE_SECS);
                        if !recently {
                            self.hot_temp_last.insert(node.clone(), now);
                            self.fx.push(FxRequest::HotTemp);
                        }
                    }
                }
                self.state.temps = list;
                self.state.have_temps = true;
            }
            Event::Storage { volumes, used_bytes, capacity_bytes } => {
                let pct = if capacity_bytes > 0 { used_bytes as f32 / capacity_bytes as f32 * 100.0 } else { 0.0 };
                self.storage_pct.set(pct, now);
                self.state.volumes = volumes;
                self.state.storage_used = used_bytes;
                self.state.storage_capacity = capacity_bytes;
                self.state.have_storage = true;
            }
            Event::HotTemp { .. } => self.fx.push(FxRequest::HotTemp),
            Event::VolumeDegraded { .. } => self.fx.push(FxRequest::VolumeDegraded),
            Event::VolumeHealthy { .. } => self.fx.push(FxRequest::VolumeHealthy),
```
(`Event::HotTemp` from sources is accepted too, but the model's own threshold check is the primary path; a source that also emits it is harmless because splashes collapse.) Accessors `smooth_hot_temp(now)`, `smooth_storage_pct(now)`. Tests: temps fold sets hottest, hot temp splash debounced; storage pct computed; both `have_*` flags.

- [ ] **Step 4: fx.rs**

`SplashKind::{HotTemp, VolumeDegraded, VolumeHealthy}` with icons `flame`, `database-zap`, `database` and colours `RED`, `AMBER`, `GREEN`; `Fx::apply` routes `FxRequest::HotTemp => splash(SplashKind::HotTemp, Role::Thermal)`, `VolumeDegraded => (.., Role::Storage)`, `VolumeHealthy => (.., Role::Storage)`.

- [ ] **Step 5: scene_thermal.rs**

```rust
//! Thermal role: hottest node on the ring, temperature-coloured node dots in a wide badge.

use crate::anim::Secs;
use crate::model::Model;
use crate::scene::{badge_text_alternate, icon_at, node_dots_badge, ring, ring_states, Scene};
use crate::theme::layout::*;
use crate::theme::{temp_color, WHITE};

pub fn thermal_scene(model: &Model, now: Secs) -> Scene {
    let st = model.state();
    let hot = model.smooth_hot_temp(now);
    let color = temp_color(hot);
    let mut s = Scene::new();
    s.push(ring(RING_R, ring_states(hot.clamp(0.0, 100.0), color, SEG_N, now)));
    s.push(icon_at("thermometer", ICON_CY, ICON_SIZE, WHITE, 1.0));
    let colors: Vec<_> = st.temps.iter().map(|(_, c)| temp_color(*c)).collect();
    let hottest = st.temps.iter().enumerate().max_by(|a, b| a.1 .1.total_cmp(&b.1 .1)).map(|(i, _)| i);
    let avg = if st.temps.is_empty() { 0.0 } else { st.temps.iter().map(|(_, c)| c).sum::<f32>() / st.temps.len() as f32 };
    for d in node_dots_badge(BADGE_CY, color, colors, hottest, now) {
        s.push(d);
    }
    // the badge holds dots; the temperature text goes in a second small badge below
    s.push(badge_text_alternate(MARKER_CY + 4.0, color, format!("{hot:.0}°"), format!("{avg:.0}°"), now));
    s
}
```
`scene.rs` gains `pub fn icon_at(name, cy, size, color, alpha) -> Drawable` (the existing private `icon` made public under that name) and `pub fn badge_text_alternate(cy, stroke, a: String, b: String, now) -> Drawable` (a 48x20 badge with 12 px text that shows `a` in even 5 s windows and `b` in odd ones, alpha fading in over the first 0.25 s of each window like the torrent badge). Test: at now 0 text is `a`, at 5.5 it is `b`.

- [ ] **Step 6: scene_storage.rs**

```rust
//! Storage role: Longhorn volume health on the outer ring, used capacity inside.

use crate::anim::{breathe, Secs};
use crate::event::Robustness;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, BLUE, GREEN, GREY, RED, VIOLET, WHITE};

fn robustness_color(r: Robustness) -> Color {
    match r {
        Robustness::Healthy => GREEN,
        Robustness::Degraded => AMBER,
        Robustness::Faulted => RED,
        Robustness::Unknown => GREY,
    }
}

/// Ring states for the volumes: equal sections with an unlit gap when there is room.
pub fn volume_states(volumes: &[(String, Robustness)], now: Secs) -> Vec<SegState> {
    let n = SEG_N;
    if volumes.is_empty() {
        return vec![SegState::Off; n];
    }
    if volumes.iter().all(|(_, r)| *r == Robustness::Healthy) {
        return ring_states(100.0, VIOLET, n, now);
    }
    let count = volumes.len();
    let per = n / count; // segments per volume (>= 1 when count <= 60)
    let gap = if per >= 3 { 1 } else { 0 };
    let mut out = vec![SegState::Off; n];
    for (vi, (_, r)) in volumes.iter().enumerate().take(n) {
        let start = vi * per;
        let end = if vi == count - 1 { n } else { start + per };
        let lit_end = end - gap;
        for st in out.iter_mut().take(lit_end).skip(start) {
            let a = if *r == Robustness::Healthy { 1.0 } else { breathe(now, 1.2) };
            *st = SegState::On(robustness_color(*r), a);
        }
    }
    out
}

pub fn storage_scene(model: &Model, now: Secs) -> Scene {
    let st = model.state();
    let healthy = st.volumes.iter().filter(|(_, r)| *r == Robustness::Healthy).count();
    let used_pct = model.smooth_storage_pct(now);
    let all_ok = healthy == st.volumes.len();
    let mut s = Scene::new();
    s.push(ring(RING_R, volume_states(&st.volumes, now)));
    s.push(ring(80.0, ring_states(used_pct, BLUE, crate::scene::seg_count(80.0), now)));
    s.push(icon_at("database", 92.0, 60.0, WHITE, 1.0));
    let window = ((now / 4.0).floor() as i64).rem_euclid(3);
    let gb = st.storage_used as f32 / 1_073_741_824.0;
    let (text, stroke) = match window {
        0 => (format!("{healthy}/{}", st.volumes.len()), if all_ok { VIOLET } else { AMBER }),
        1 => (if gb >= 1000.0 { format!("{:.1} TB", gb / 1024.0) } else { format!("{gb:.0} GB") }, BLUE),
        _ => (format!("{used_pct:.0}%"), BLUE),
    };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 4.0).floor() * 4.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    if !all_ok {
        s.push(Drawable::Dots { cx: CX, cy: MARKER_CY, spacing: 0.0, r: 3.0, colors: vec![AMBER.with_alpha(breathe(now, 2.4))] });
    }
    s
}
```
Make `scene::badge` and `scene::seg_count` `pub`. Tests: `volume_states` with 21 healthy → 60 violet; with 20 volumes one degraded → 60/20 = 3 per volume, gap 1, degraded section amber; with 70 volumes no panic and the first 60 map one each; `storage_scene` badge text cycles `21/21` → `49 GB` → `38%` across `now = 0, 4.5, 8.5` for used 49 GiB / 128 GiB.

- [ ] **Step 7: Wire scenes**

`scene.rs::role_scene`: `Role::Thermal => scene_thermal::thermal_scene(model, now)`, `Role::Storage => scene_storage::storage_scene(model, now)`. `scene_fx::needs_data`: `Role::Thermal => !st.have_temps`, `Role::Storage => !st.have_storage`. `lib.rs`: `pub mod scene_storage; pub mod scene_thermal;`. Golden tests: add `thermal` (7 named temps as in the cluster) and `storage` (21 healthy, 49 GiB of 128 GiB) goldens rendered via `scene_for_role`, plus `storage_degraded`.

- [ ] **Step 8: Test, commit**

```bash
git add -A
git commit -m "feat(core): thermal and storage roles with splashes"
```

---

### Task 5: Electricity: events, model, power-mix / price / carbon / renewable scenes

**Files:**
- Create: `crates/core/src/scene_electricity.rs`, `crates/core/src/electricity.rs`
- Modify: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`, `crates/core/src/theme.rs`, `crates/core/src/lib.rs`, `crates/render/tests/golden.rs`

**Interfaces:**
- Produces:
  - `electricity::Source { Nuclear, Geothermal, Biomass, Coal, Wind, Solar, Hydro, Gas, Oil, Unknown, HydroStorage, BatteryStorage }` with `Source::ALL`, `.color() -> Color`, `.icon() -> &'static str` (`em-*`), `Source::from_api_key(&str) -> Option<Source>` (`"hydro discharge" → HydroStorage`, `"battery discharge" → BatteryStorage`).
  - `Event::Electricity { zone, mix_mw: Vec<(Source, f32)>, renewable_pct, fossil_free_pct, carbon_gco2, updated_at }`, `Event::Prices { date, ct_per_kwh: Vec<f32>, currency }`, `LinkTarget::{Electricity, Prices}`.
  - `theme::carbon_color(g: f32) -> Color`, `theme::price_color(t: f32) -> Color` (0 cheap → 1 expensive).
  - `electricity::partition(shares: &[(Source, f32)], n: usize) -> Vec<Section { source, start, len }>` (pure), `electricity::MIX_ICON_MIN_SEGS = 3`.
  - `model::ElectricityState { mix: Vec<(Source, f32)>, renewable_pct, fossil_free_pct, carbon_gco2, have }`, `PriceState { date, ct: Vec<f32>, have }`, `Model::electricity()`, `Model::prices()`, `Model::smooth_share(source, now)`, `Model::smooth_renewable(now)`, `Model::smooth_fossil_free(now)`, `Model::smooth_carbon(now)`, `Model::set_token_present(bool)`, `Model::token_present()`.
  - `scene_electricity::{power_mix_scene, price_scene, carbon_scene, renewable_scene}(model, now) -> Scene`; `scene::no_data_scene_with(now, icon)`.
  - `LinkState.electricity`, `LinkState.prices`.

- [ ] **Step 1: electricity.rs (pure)**

```rust
//! Electricity Maps sources, colours, icons and the power-mix ring partition.

use crate::theme::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Nuclear,
    Geothermal,
    Biomass,
    Coal,
    Wind,
    Solar,
    Hydro,
    Gas,
    Oil,
    Unknown,
    HydroStorage,
    BatteryStorage,
}

impl Source {
    pub const ALL: [Source; 12] = [
        Source::Nuclear, Source::Geothermal, Source::Biomass, Source::Coal, Source::Wind, Source::Solar,
        Source::Hydro, Source::Gas, Source::Oil, Source::Unknown, Source::HydroStorage, Source::BatteryStorage,
    ];

    pub fn from_api_key(k: &str) -> Option<Source> {
        Some(match k {
            "nuclear" => Source::Nuclear,
            "geothermal" => Source::Geothermal,
            "biomass" => Source::Biomass,
            "coal" => Source::Coal,
            "wind" => Source::Wind,
            "solar" => Source::Solar,
            "hydro" => Source::Hydro,
            "gas" => Source::Gas,
            "oil" => Source::Oil,
            "unknown" => Source::Unknown,
            "hydro discharge" => Source::HydroStorage,
            "battery discharge" => Source::BatteryStorage,
            _ => return None,
        })
    }

    pub fn color(self) -> Color {
        Color::hex(match self {
            Source::Biomass => 0x008043,
            Source::Geothermal => 0xA73C15,
            Source::Hydro => 0x1878EA,
            Source::Solar => 0xFFC700,
            Source::Wind => 0x69D6F8,
            Source::Nuclear => 0x9D71F7,
            Source::BatteryStorage => 0x1DA484,
            Source::HydroStorage => 0x2B3CD8,
            Source::Coal => 0xac8c35,
            Source::Gas => 0xAAA189,
            Source::Oil => 0x584745,
            Source::Unknown => 0xACACAC,
        })
    }

    pub fn icon(self) -> &'static str {
        match self {
            Source::Biomass => "em-biomass",
            Source::Geothermal => "em-geothermal",
            Source::Hydro => "em-hydro",
            Source::Solar => "em-solar",
            Source::Wind => "em-wind",
            Source::Nuclear => "em-nuclear",
            Source::BatteryStorage => "em-battery-storage",
            Source::HydroStorage => "em-hydro-storage",
            Source::Coal => "em-coal",
            Source::Gas => "em-gas",
            Source::Oil => "em-oil",
            Source::Unknown => "em-unknown",
        }
    }
}

pub const MIX_ICON_MIN_SEGS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub source: Source,
    /// First segment index (0 = 12 o'clock).
    pub start: usize,
    /// Lit segments, gap excluded.
    pub len: usize,
}

/// Split `n` segments by share. Input is (source, share) with shares > 0, any order.
/// Output is sorted by share descending; each source gets at least one lit segment;
/// between two sources one segment stays unlit (taken from the larger neighbour).
pub fn partition(shares: &[(Source, f32)], n: usize) -> Vec<Section> {
    let mut items: Vec<(Source, f32)> = shares.iter().copied().filter(|(_, s)| *s > 0.0).collect();
    if items.is_empty() || n == 0 {
        return Vec::new();
    }
    items.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
    let count = items.len().min(n);
    items.truncate(count);
    let total: f32 = items.iter().map(|(_, s)| s).sum();
    let gaps = if count > 1 { count } else { 0 };
    let usable = n.saturating_sub(gaps).max(count);
    // largest remainder apportionment with a floor of 1
    let mut alloc: Vec<usize> = items.iter().map(|(_, s)| ((s / total) * usable as f32).floor() as usize).collect();
    for a in alloc.iter_mut() {
        if *a == 0 {
            *a = 1;
        }
    }
    let mut used: usize = alloc.iter().sum();
    let mut i = 0;
    while used > usable {
        let idx = alloc.iter().enumerate().max_by_key(|(_, a)| **a).map(|(i, _)| i).unwrap();
        alloc[idx] -= 1;
        used -= 1;
    }
    let mut remainders: Vec<(usize, f32)> = items.iter().enumerate().map(|(i, (_, s))| (i, (s / total) * usable as f32 - alloc[i] as f32)).collect();
    remainders.sort_by(|a, b| b.1.total_cmp(&a.1));
    while used < usable {
        let idx = remainders[i % remainders.len()].0;
        alloc[idx] += 1;
        used += 1;
        i += 1;
    }
    let mut out = Vec::with_capacity(count);
    let mut start = 0;
    for (k, (source, _)) in items.iter().enumerate() {
        out.push(Section { source: *source, start, len: alloc[k] });
        start += alloc[k] + if gaps > 0 { 1 } else { 0 };
    }
    out
}

impl Source {
    fn index(self) -> usize {
        Source::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keys_and_palette() {
        assert_eq!(Source::from_api_key("hydro discharge"), Some(Source::HydroStorage));
        assert_eq!(Source::from_api_key("battery discharge"), Some(Source::BatteryStorage));
        assert_eq!(Source::from_api_key("fusion"), None);
        assert_eq!(Source::Solar.color(), Color::hex(0xFFC700));
        assert_eq!(Source::Coal.icon(), "em-coal");
    }

    #[test]
    fn partition_sorts_gaps_and_floors() {
        let p = partition(&[(Source::Wind, 24.0), (Source::Solar, 48.0), (Source::Gas, 13.0), (Source::Hydro, 0.4), (Source::Nuclear, 0.0)], 60);
        assert_eq!(p.len(), 4, "zero share absent");
        assert_eq!(p[0].source, Source::Solar);
        assert_eq!(p[0].start, 0);
        let lit: usize = p.iter().map(|s| s.len).sum();
        assert_eq!(lit + 4, 60, "one gap per section, including after the last");
        assert!(p.iter().all(|s| s.len >= 1));
        for w in p.windows(2) {
            assert_eq!(w[1].start, w[0].start + w[0].len + 1);
        }
        assert!(p[0].len > p[1].len && p[1].len > p[2].len);
    }

    #[test]
    fn partition_single_and_empty() {
        let p = partition(&[(Source::Solar, 1.0)], 60);
        assert_eq!(p, vec![Section { source: Source::Solar, start: 0, len: 60 }]);
        assert!(partition(&[], 60).is_empty());
        assert!(partition(&[(Source::Solar, 0.0)], 60).is_empty());
    }
}
```

- [ ] **Step 2: theme.rs scales**

```rust
/// Electricity Maps carbon intensity scale (gCO2eq/kWh).
pub fn carbon_color(g: f32) -> Color {
    const STOPS: [(f32, u32); 4] = [(0.0, 0x2AA364), (150.0, 0xF5EB4D), (600.0, 0x9E4229), (800.0, 0x381D02)];
    let g = g.clamp(0.0, 800.0);
    for w in STOPS.windows(2) {
        let (g0, c0) = w[0];
        let (g1, c1) = w[1];
        if g <= g1 {
            return Color::hex(c0).mix(Color::hex(c1), (g - g0) / (g1 - g0));
        }
    }
    Color::hex(STOPS[3].1)
}

/// 0 = cheapest of the day (green) .. 1 = most expensive (red).
pub fn price_color(t: f32) -> Color {
    GREEN.mix(RED, t.clamp(0.0, 1.0))
}
```
Tests: `carbon_color(0.0) == #2AA364`, `carbon_color(150.0) == #F5EB4D`, `carbon_color(1000.0) == #381D02`, `price_color(0.0) == GREEN`.

- [ ] **Step 3: event.rs and model.rs**

Event variants:
```rust
    Electricity { zone: String, mix_mw: Vec<(Source, f32)>, renewable_pct: f32, fossil_free_pct: f32, carbon_gco2: f32, updated_at: String },
    Prices { date: String, ct_per_kwh: Vec<f32>, currency: String },
```
`LinkTarget::{Electricity, Prices}`; `LinkState { api, prom, qbit, electricity, prices }`.

Model: `ElectricityState`, `PriceState` structs (pub fields as in Interfaces), `shares: HashMap<Source, Smooth>` (targets are percent of total production; sources absent from the latest mix are retargeted to 0 and dropped once their smooth value is below 0.05), `renewable: Smooth`, `fossil_free: Smooth`, `carbon: Smooth`, `token_present: bool` (set by the app from config; default false). Folds:
```rust
            Event::Electricity { zone, mix_mw, renewable_pct, fossil_free_pct, carbon_gco2, updated_at } => {
                let total: f32 = mix_mw.iter().map(|(_, mw)| mw.max(0.0)).sum();
                for (src, mw) in &mix_mw {
                    let pct = if total > 0.0 { mw.max(0.0) / total * 100.0 } else { 0.0 };
                    self.shares.entry(*src).or_insert_with(|| Smooth::new(0.0, SMOOTH_SECS)).set(pct, now);
                }
                let present: std::collections::HashSet<Source> = mix_mw.iter().map(|(s, _)| *s).collect();
                for (src, sm) in self.shares.iter_mut() {
                    if !present.contains(src) {
                        sm.set(0.0, now);
                    }
                }
                self.renewable.set(renewable_pct, now);
                self.fossil_free.set(fossil_free_pct, now);
                self.carbon.set(carbon_gco2, now);
                self.electricity = ElectricityState { zone, mix: mix_mw, renewable_pct, fossil_free_pct, carbon_gco2, updated_at, have: true };
            }
            Event::Prices { date, ct_per_kwh, currency } => {
                self.prices = PriceState { date, ct: ct_per_kwh, currency, have: true };
            }
```
`Model::smooth_shares(now) -> Vec<(Source, f32)>` returns every source with smoothed share ≥ 0.05 (used by the power-mix scene). Tests: shares sum to ~100 after settling; a source dropped from the mix eases to 0 and disappears; prices stored.

- [ ] **Step 4: scene_electricity.rs**

```rust
//! Electricity roles: power mix, price, carbon intensity, renewable / carbon-free.

use crate::anim::{breathe, Secs};
use crate::electricity::{partition, MIX_ICON_MIN_SEGS};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{carbon_color, price_color, Color, BLUE, GREEN, OFF, WHITE};

const MIX_ICON_R: f32 = 60.0;
const MIX_ICON_SIZE: f32 = 26.0;
const MIX_TICK_R0: f32 = 80.0;
const MIX_TICK_R1: f32 = 88.0;

pub fn power_mix_scene(model: &Model, now: Secs) -> Scene {
    let shares = model.smooth_shares(now);
    let sections = partition(&shares, SEG_N);
    let mut states = vec![SegState::Off; SEG_N];
    for (k, sec) in sections.iter().enumerate() {
        for i in sec.start..(sec.start + sec.len).min(SEG_N) {
            let last_of_leader = k == 0 && i + 1 == sec.start + sec.len;
            states[i] = SegState::On(sec.source.color(), if last_of_leader { breathe(now, 2.4) } else { 1.0 });
        }
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    for sec in &sections {
        if sec.len < MIX_ICON_MIN_SEGS {
            continue;
        }
        let angle = (sec.start as f32 + (sec.len as f32 - 1.0) / 2.0) * (360.0 / SEG_N as f32);
        let a = angle.to_radians();
        let (sin, cos) = (a.sin(), a.cos());
        s.push(Drawable::Tick { cx: CX, cy: CY, angle_deg: angle, r0: MIX_TICK_R0, r1: MIX_TICK_R1, width: 3.0, color: OFF, alpha: 1.0 });
        s.push(Drawable::Icon {
            name: sec.source.icon(),
            cx: CX + MIX_ICON_R * sin,
            cy: CY - MIX_ICON_R * cos,
            size: MIX_ICON_SIZE,
            color: sec.source.color(),
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        });
    }
    s
}

pub const PRICE_SEGS: usize = 48;

fn price_badge_text(ct: f32) -> String {
    if ct >= 100.0 {
        format!("{:.2} €", ct / 100.0)
    } else {
        format!("{ct:.1} ct")
    }
}

pub fn price_scene(model: &Model, now: Secs, local_hour: u32) -> Scene {
    let p = model.prices();
    let valid: Vec<f32> = p.ct.iter().copied().filter(|v| v.is_finite()).collect();
    let (min, max) = valid.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let span = (max - min).max(0.01);
    let mut states = vec![SegState::Off; PRICE_SEGS];
    for h in 0..24usize {
        let Some(v) = p.ct.get(h).copied().filter(|v| v.is_finite()) else { continue };
        let color = if v < 0.0 { BLUE } else { price_color((v - min) / span) };
        let alpha = if (h as u32) < local_hour { 0.35 } else if h as u32 == local_hour { breathe(now, 2.4) } else { 1.0 };
        states[h * 2] = SegState::On(color, alpha);
        states[h * 2 + 1] = SegState::On(color, alpha);
    }
    let mut s = Scene::new();
    s.push(Drawable::Ring { cx: CX, cy: CY, radius: RING_R, n: PRICE_SEGS, states, pitch_deg: 360.0 / PRICE_SEGS as f32, start_deg: 0.0 });
    s.push(icon_at("euro", ICON_CY, ICON_SIZE, WHITE, 1.0));
    let cur = p.ct.get(local_hour as usize).copied().filter(|v| v.is_finite());
    let stroke = match cur {
        Some(v) if v < 0.0 => BLUE,
        Some(v) => price_color((v - min) / span),
        None => crate::theme::GREY,
    };
    let window_odd = ((now / 5.0).floor() as i64) % 2 == 1;
    let text = if window_odd && !valid.is_empty() {
        format!("min {:.1}", min)
    } else {
        cur.map(price_badge_text).unwrap_or_else(|| "--".into())
    };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s
}

pub fn carbon_scene(model: &Model, now: Secs) -> Scene {
    let g = model.smooth_carbon(now);
    let color = carbon_color(g);
    let mut s = Scene::new();
    s.push(ring(RING_R, ring_states((g / 800.0 * 100.0).clamp(0.0, 100.0), color, SEG_N, now)));
    s.push(icon_at("cloud", ICON_CY, ICON_SIZE, color, 1.0));
    s.push(badge(BADGE_CY, color, format!("{g:.0} g")));
    s
}

pub fn renewable_scene(model: &Model, now: Secs) -> Scene {
    let ren = model.smooth_renewable(now);
    let ff = model.smooth_fossil_free(now);
    let inner_color = Color::hex(0x69D6F8);
    let mut s = Scene::new();
    s.push(ring(RING_R, ring_states(ren, GREEN, SEG_N, now)));
    s.push(ring(84.0, ring_states(ff, inner_color, seg_count(84.0), now)));
    s.push(icon_at("leaf", 92.0, 60.0, WHITE, 1.0));
    let odd = ((now / 5.0).floor() as i64) % 2 == 1;
    let (text, stroke, dot) = if odd { (format!("{ff:.0}%"), inner_color, inner_color) } else { (format!("{ren:.0}%"), GREEN, GREEN) };
    let mut b = badge(BADGE_CY, stroke, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s.push(Drawable::Dots { cx: CX - 26.0, cy: BADGE_CY, spacing: 0.0, r: 3.0, colors: vec![dot] });
    s
}
```
`price_scene` needs the local hour: `Model` gets `pub fn set_local_hour(&mut self, h: u32)` (the render loop sets it every tick from chrono) and `local_hour()`; `role_scene` passes `model.local_hour()`. `role_scene` dispatch: `PowerMix`, `Price`, `Carbon`, `Renewable`. `needs_data`: `PowerMix | Carbon | Renewable => !electricity.have`, `Price => !prices.have`. No-data variant: in `Model::scene_for_role`, when `needs_data` and the role is an electricity role and `!self.token_present()`, return `no_data_scene_with(now, "key-round")` (add `pub fn no_data_scene_with(now, icon: &'static str)` to `scene.rs`; `no_data_scene` calls it with `cloud-off`).

Tests: power mix with `Solar 48, Wind 24, Gas 13, Coal 7, Nuclear 4, Biomass 2, Hydro 2` yields 7 sections, icons for the sources with ≥ 3 segments only, first icon at angle within the solar section, tick count equals icon count; price scene at local hour 14 has 48-segment ring, past hours alpha 0.35, badge `22.1 ct` for `ct[14] = 22.1`, `min 6.2` in an odd window, `1.02 €` for 102 ct; carbon scene badge `214 g`; renewable alternates `61%`/`73%`.

- [ ] **Step 5: Goldens and commit**

Add goldens `power_mix`, `price`, `carbon`, `renewable` (fixed data, `now = 5.0`, local hour 14) and `no_token` (electricity role without token). Full suite, clippy, fmt.

```bash
git add -A
git commit -m "feat(core): electricity roles: power mix, price, carbon, renewable"
```

---

### Task 6: Prometheus: node temperatures and Longhorn volumes

**Files:**
- Modify: `crates/sources/src/prometheus.rs`

**Interfaces:**
- Produces: `prometheus::{Q_TEMPS, Q_TEMPS_FALLBACK, Q_LH_ROBUST, Q_LH_USED, Q_LH_CAP}`, `prometheus::temps_from(v: &[(HashMap<String,String>, f64)]) -> Vec<(String, f32)>` (sorted by name), `prometheus::StorageTracker::new()`, `.diff(volumes: Vec<(String, Robustness)>) -> Vec<Event>` (VolumeDegraded / VolumeHealthy on transitions after priming), `poll` emits `NodeTemps` and `Storage` when the queries return data.

- [ ] **Step 1: Queries and helpers**

```rust
const Q_TEMPS: &str = "max by (nodename) (node_hwmon_temp_celsius * on(instance) group_left(nodename) node_uname_info)";
const Q_TEMPS_FALLBACK: &str = "max by (nodename) (node_thermal_zone_temp * on(instance) group_left(nodename) node_uname_info)";
const Q_LH_ROBUST: &str = "longhorn_volume_robustness";
const Q_LH_USED: &str = "sum(longhorn_volume_actual_size_bytes)";
const Q_LH_CAP: &str = "sum(longhorn_volume_capacity_bytes)";

pub fn temps_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = v
        .iter()
        .filter_map(|(m, val)| m.get("nodename").map(|n| (n.clone(), *val as f32)))
        .filter(|(_, c)| c.is_finite() && *c > -50.0 && *c < 150.0)
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Longhorn robustness per volume; keeps the highest-severity sample per volume name
/// (Longhorn exports one series per node that hosts a replica).
pub fn volumes_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, Robustness)> {
    let mut map: std::collections::BTreeMap<String, Robustness> = Default::default();
    for (m, val) in v {
        let Some(name) = m.get("volume") else { continue };
        let r = Robustness::from_code(*val);
        let e = map.entry(name.clone()).or_insert(r);
        let rank = |x: Robustness| match x { Robustness::Faulted => 3, Robustness::Degraded => 2, Robustness::Unknown => 1, Robustness::Healthy => 0 };
        if rank(r) > rank(*e) {
            *e = r;
        }
    }
    map.into_iter().collect()
}

#[derive(Default)]
pub struct StorageTracker {
    seen: HashMap<String, Robustness>,
    primed: bool,
}

impl StorageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&mut self, volumes: &[(String, Robustness)]) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed {
            for (name, r) in volumes {
                match (self.seen.get(name).copied(), *r) {
                    (Some(Robustness::Healthy), Robustness::Degraded | Robustness::Faulted) => {
                        out.push(Event::VolumeDegraded { name: name.clone(), robustness: *r })
                    }
                    (Some(Robustness::Degraded | Robustness::Faulted), Robustness::Healthy) => {
                        out.push(Event::VolumeHealthy { name: name.clone() })
                    }
                    _ => {}
                }
            }
        }
        self.seen = volumes.iter().cloned().collect();
        self.primed = true;
        out
    }
}
```
In `poll` (it now also takes `storage: &mut StorageTracker`): after the alert diff,
```rust
    let mut temps = parse_vector(&query(t, Q_TEMPS).await?);
    if temps.is_empty() {
        temps = parse_vector(&query(t, Q_TEMPS_FALLBACK).await?);
    }
    let temps = temps_from(&temps);
    if !temps.is_empty() {
        out.push(Event::NodeTemps(temps));
    }
    let vols = volumes_from(&parse_vector(&query(t, Q_LH_ROBUST).await?));
    if !vols.is_empty() {
        let used = parse_scalar(&query(t, Q_LH_USED).await?).unwrap_or(0.0) as u64;
        let cap = parse_scalar(&query(t, Q_LH_CAP).await?).unwrap_or(0.0) as u64;
        out.extend(storage.diff(&vols));
        out.push(Event::Storage { volumes: vols, used_bytes: used, capacity_bytes: cap });
    }
```
`run_prometheus` creates `let mut storage = StorageTracker::new();` next to the alert tracker and passes it.

- [ ] **Step 2: Tests**

```rust
    #[test]
    fn temps_join_and_sort() {
        let v = parse_vector(r#"{"data":{"result":[
            {"metric":{"nodename":"raspberrypi-5-8gb-1"},"value":[1,"48.5"]},
            {"metric":{"nodename":"hp-elitedesk-800-g5-i7"},"value":[1,"53.9"]},
            {"metric":{"instance":"x"},"value":[1,"99"]}]}}"#);
        let t = temps_from(&v);
        assert_eq!(t, vec![("hp-elitedesk-800-g5-i7".into(), 53.9), ("raspberrypi-5-8gb-1".into(), 48.5)]);
    }

    #[test]
    fn volumes_keep_worst_sample_and_tracker_diffs() {
        let v = parse_vector(r#"{"data":{"result":[
            {"metric":{"volume":"pvc-a","node":"n1"},"value":[1,"1"]},
            {"metric":{"volume":"pvc-a","node":"n2"},"value":[1,"2"]},
            {"metric":{"volume":"pvc-b","node":"n1"},"value":[1,"1"]}]}}"#);
        let vols = volumes_from(&v);
        assert_eq!(vols, vec![("pvc-a".into(), Robustness::Degraded), ("pvc-b".into(), Robustness::Healthy)]);
        let mut tr = StorageTracker::new();
        assert!(tr.diff(&vols).is_empty(), "priming is silent");
        let healed = vec![("pvc-a".into(), Robustness::Healthy), ("pvc-b".into(), Robustness::Faulted)];
        let evs = tr.diff(&healed);
        assert!(matches!(evs[0], Event::VolumeHealthy { .. }));
        assert!(matches!(evs[1], Event::VolumeDegraded { robustness: Robustness::Faulted, .. }));
        assert!(tr.diff(&healed).is_empty());
    }
```

- [ ] **Step 3: Test, commit**

```bash
git add -A
git commit -m "feat(sources): node temperatures and Longhorn volume health from Prometheus"
```

---

### Task 7: Electricity Maps and day-ahead price pollers

**Files:**
- Create: `crates/sources/src/electricity.rs`, `crates/sources/src/prices.rs`, `crates/sources/tests/fixtures/em-power-breakdown.json`, `em-carbon.json`, `energyzero.json`, `entsoe.xml`
- Modify: `crates/sources/src/lib.rs`, `crates/sources/Cargo.toml`

**Interfaces:**
- Produces:
  - `electricity::{ElectricityConfig { zone, token, poll_secs }, parse_power_breakdown(&str) -> Result<PowerBreakdown { mix_mw: Vec<(Source, f32)>, renewable_pct, fossil_free_pct, datetime }>, parse_carbon(&str) -> Result<f32>, run_electricity(cfg, ctx)}`
  - `prices::{PriceConfig { source: PriceSource { EnergyZero { include_vat }, Entsoe { token, zone } }, poll_secs }, parse_energyzero(&str) -> Result<Vec<(String /*ISO ts*/, f32 /*€/kWh*/)>>, parse_entsoe(&str) -> Result<Vec<(String, f32 /*€/MWh*/)>>, hourly_ct_for_day(points: &[(String, f32)], day: chrono::NaiveDate, tz: chrono_tz::Tz, per_kwh: bool) -> Vec<f32>, entsoe_zone_for(country: &str) -> Option<&'static str>, run_prices(cfg, ctx)}`
  - `sources::http::client() -> reqwest::Client` (shared, 10 s timeout, ring provider installed).

- [ ] **Step 1: Dependencies**

`crates/sources/Cargo.toml`:
```toml
reqwest = { version = "0.13", default-features = false, features = ["rustls-no-provider", "json", "charset"] }
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
quick-xml = "0.42"
chrono = "0.4"
chrono-tz = "0.10"
```
Add `crates/sources/src/http.rs`:
```rust
//! Shared HTTP client for the public APIs (Electricity Maps, price sources).

use std::sync::OnceLock;
use std::time::Duration;

pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client")
        })
        .clone()
}
```
`lib.rs`: `pub mod electricity; pub mod http; pub mod prices;`.

- [ ] **Step 2: electricity.rs**

```rust
//! Electricity Maps API poller: production mix, renewable / fossil-free share, carbon intensity.

use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::electricity::Source;
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

const BASE: &str = "https://api.electricitymap.org/v3";

#[derive(Clone, Debug)]
pub struct ElectricityConfig {
    pub zone: String,
    pub token: String,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerBreakdown {
    pub mix_mw: Vec<(Source, f32)>,
    pub renewable_pct: f32,
    pub fossil_free_pct: f32,
    pub datetime: String,
}

pub fn parse_power_breakdown(json: &str) -> Result<PowerBreakdown> {
    let v: Value = serde_json::from_str(json).context("power-breakdown json")?;
    let prod = v.get("powerProductionBreakdown").and_then(|p| p.as_object()).context("powerProductionBreakdown missing")?;
    let mut mix_mw = Vec::new();
    for (k, val) in prod {
        let Some(src) = Source::from_api_key(k) else { continue };
        let mw = val.as_f64().unwrap_or(0.0) as f32;
        if mw > 0.0 {
            mix_mw.push((src, mw));
        }
    }
    let pct = |key: &str| v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    Ok(PowerBreakdown {
        mix_mw,
        renewable_pct: pct("renewablePercentage"),
        fossil_free_pct: pct("fossilFreePercentage"),
        datetime: v.get("datetime").and_then(|d| d.as_str()).unwrap_or("").to_string(),
    })
}

pub fn parse_carbon(json: &str) -> Result<f32> {
    let v: Value = serde_json::from_str(json).context("carbon-intensity json")?;
    v.get("carbonIntensity").and_then(|c| c.as_f64()).map(|c| c as f32).context("carbonIntensity missing")
}

async fn fetch(path: &str, zone: &str, token: &str) -> Result<String> {
    let url = format!("{BASE}/{path}/latest?zone={zone}");
    let resp = client().get(&url).header("auth-token", token).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    anyhow::ensure!(status.is_success(), "electricity maps HTTP {status}: {}", body.chars().take(120).collect::<String>());
    Ok(body)
}

pub async fn poll_once(cfg: &ElectricityConfig) -> Result<Event> {
    let pb = parse_power_breakdown(&fetch("power-breakdown", &cfg.zone, &cfg.token).await?)?;
    let carbon = parse_carbon(&fetch("carbon-intensity", &cfg.zone, &cfg.token).await?)?;
    Ok(Event::Electricity {
        zone: cfg.zone.clone(),
        mix_mw: pb.mix_mw,
        renewable_pct: pb.renewable_pct,
        fossil_free_pct: pb.fossil_free_pct,
        carbon_gco2: carbon,
        updated_at: pb.datetime,
    })
}

pub async fn run_electricity(cfg: ElectricityConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match poll_once(&cfg).await {
            Ok(ev) => {
                failures = 0;
                ctx.emit(Event::Link { target: LinkTarget::Electricity, up: true });
                ctx.emit(ev);
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("electricity: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link { target: LinkTarget::Electricity, up: false });
                }
            }
        }
        let wait = if failures > 0 { 60 } else { cfg.poll_secs.max(60) };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PB: &str = include_str!("../tests/fixtures/em-power-breakdown.json");
    const CI: &str = include_str!("../tests/fixtures/em-carbon.json");

    #[test]
    fn parses_breakdown_with_storage_and_nulls() {
        let pb = parse_power_breakdown(PB).unwrap();
        assert!(pb.mix_mw.iter().any(|(s, mw)| *s == Source::Solar && *mw > 1000.0));
        assert!(pb.mix_mw.iter().any(|(s, _)| *s == Source::HydroStorage));
        assert!(!pb.mix_mw.iter().any(|(s, _)| *s == Source::Oil), "null and zero omitted");
        assert!((pb.renewable_pct - 61.0).abs() < 0.01);
        assert!((pb.fossil_free_pct - 73.0).abs() < 0.01);
        assert_eq!(pb.datetime, "2026-09-06T12:00:00.000Z");
    }

    #[test]
    fn parses_carbon() {
        assert_eq!(parse_carbon(CI).unwrap(), 214.0);
        assert!(parse_carbon("{}").is_err());
    }
}
```
Fixture `crates/sources/tests/fixtures/em-power-breakdown.json`:
```json
{"zone":"NL","datetime":"2026-09-06T12:00:00.000Z","updatedAt":"2026-09-06T12:05:00.000Z","createdAt":"2026-09-06T09:00:00.000Z",
 "powerConsumptionBreakdown":{"nuclear":120,"geothermal":0,"biomass":300,"coal":800,"wind":3100,"solar":9200,"hydro":30,"gas":2600,"oil":null,"unknown":40,"hydro discharge":90,"battery discharge":0},
 "powerProductionBreakdown":{"nuclear":120,"geothermal":0,"biomass":300,"coal":800,"wind":3100,"solar":9200,"hydro":30,"gas":2600,"oil":null,"unknown":40,"hydro discharge":90,"battery discharge":0},
 "powerImportBreakdown":{"BE":500},"powerExportBreakdown":{"DE":1200},
 "fossilFreePercentage":73,"renewablePercentage":61,"powerConsumptionTotal":15600,"powerProductionTotal":16280,"powerImportTotal":500,"powerExportTotal":1200,"isEstimated":false,"estimationMethod":null}
```
`em-carbon.json`:
```json
{"zone":"NL","carbonIntensity":214,"datetime":"2026-09-06T12:00:00.000Z","updatedAt":"2026-09-06T12:05:00.000Z","createdAt":"2026-09-06T09:00:00.000Z","emissionFactorType":"lifecycle","isEstimated":false,"estimationMethod":null}
```

- [ ] **Step 3: prices.rs**

```rust
//! Day-ahead electricity prices: EnergyZero (NL, public) or ENTSO-E (EU, token).

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub enum PriceSource {
    EnergyZero { include_vat: bool },
    Entsoe { token: String, zone: String },
}

#[derive(Clone, Debug)]
pub struct PriceConfig {
    pub source: PriceSource,
    pub poll_secs: u64,
    pub tz: Tz,
}

/// ENTSO-E bidding-zone EIC codes for the countries Electricity Maps zones map to directly.
pub fn entsoe_zone_for(country: &str) -> Option<&'static str> {
    Some(match country.to_ascii_uppercase().as_str() {
        "NL" => "10YNL----------L",
        "BE" => "10YBE----------2",
        "DE" | "DE-LU" => "10Y1001A1001A82H",
        "FR" => "10YFR-RTE------C",
        "AT" => "10YAT-APG------L",
        "DK-DK1" => "10YDK-1--------W",
        "DK-DK2" => "10YDK-2--------M",
        "ES" => "10YES-REE------0",
        "IT-NO" => "10Y1001A1001A73I",
        "PL" => "10YPL-AREA-----S",
        "SE-SE3" => "10Y1001A1001A46L",
        "NO-NO1" => "10YNO-1--------2",
        "FI" => "10YFI-1--------U",
        "CH" => "10YCH-SWISSGRIDZ",
        "CZ" => "10YCZ-CEPS-----N",
        "PT" => "10YPT-REN------W",
        _ => return None,
    })
}

/// EnergyZero JSON -> (ISO timestamp, €/kWh).
pub fn parse_energyzero(json: &str) -> Result<Vec<(String, f32)>> {
    let v: Value = serde_json::from_str(json).context("energyzero json")?;
    let arr = v.get("Prices").and_then(|p| p.as_array()).context("Prices missing")?;
    Ok(arr
        .iter()
        .filter_map(|p| Some((p.get("readingDate")?.as_str()?.to_string(), p.get("price")?.as_f64()? as f32)))
        .collect())
}

/// ENTSO-E Publication_MarketDocument -> (ISO timestamp, €/MWh) for every hourly Point.
pub fn parse_entsoe(xml: &str) -> Result<Vec<(String, f32)>> {
    use quick_xml::events::Event as X;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut start: Option<DateTime<Utc>> = None;
    let mut resolution_min: i64 = 60;
    let mut position: i64 = 0;
    let mut text = String::new();
    loop {
        match reader.read_event().context("xml")? {
            X::Start(e) => {
                path.push(String::from_utf8_lossy(e.name().as_ref()).to_string());
                text.clear();
            }
            X::Text(t) => text = t.unescape().map(|s| s.to_string()).unwrap_or_default(),
            X::End(_) => {
                let tag = path.pop().unwrap_or_default();
                let parent = path.last().map(String::as_str).unwrap_or("");
                match (parent, tag.as_str()) {
                    ("timeInterval", "start") => start = DateTime::parse_from_rfc3339(&text.replace('Z', "+00:00")).ok().map(|d| d.with_timezone(&Utc)),
                    ("Period", "resolution") => resolution_min = match text.as_str() { "PT15M" => 15, "PT30M" => 30, _ => 60 },
                    ("Point", "position") => position = text.trim().parse().unwrap_or(0),
                    ("Point", "price.amount") => {
                        if let (Some(s), Ok(p)) = (start, text.trim().parse::<f32>()) {
                            let ts = s + chrono::Duration::minutes((position - 1) * resolution_min);
                            out.push((ts.to_rfc3339(), p));
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            X::Eof => break,
            _ => {}
        }
    }
    anyhow::ensure!(!out.is_empty(), "no price points in ENTSO-E document");
    Ok(out)
}

/// Bucket timestamped prices into the 24 local hours of `day`, converting to ct/kWh.
/// `per_kwh` = input is €/kWh (EnergyZero) else €/MWh (ENTSO-E). Missing hours are NaN;
/// sub-hourly points average into their hour.
pub fn hourly_ct_for_day(points: &[(String, f32)], day: NaiveDate, tz: Tz, per_kwh: bool) -> Vec<f32> {
    let mut sum = [0.0f32; 24];
    let mut cnt = [0u32; 24];
    for (ts, price) in points {
        let Ok(t) = DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")) else { continue };
        let local = t.with_timezone(&tz);
        if local.date_naive() != day {
            continue;
        }
        let h = local.hour() as usize;
        sum[h] += if per_kwh { price * 100.0 } else { price / 10.0 };
        cnt[h] += 1;
    }
    (0..24).map(|h| if cnt[h] == 0 { f32::NAN } else { sum[h] / cnt[h] as f32 }).collect()
}

fn day_bounds_utc(day: NaiveDate, tz: Tz) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = tz.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap()).single().unwrap_or_else(|| tz.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap()));
    let end = start + chrono::Duration::days(1);
    (start.with_timezone(&Utc), end.with_timezone(&Utc))
}

async fn fetch_day(cfg: &PriceConfig, day: NaiveDate) -> Result<Vec<f32>> {
    let (from, till) = day_bounds_utc(day, cfg.tz);
    match &cfg.source {
        PriceSource::EnergyZero { include_vat } => {
            let url = format!(
                "https://api.energyzero.nl/v1/energyprices?fromDate={}&tillDate={}&interval=4&usageType=1&inclBtw={}",
                from.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                (till - chrono::Duration::milliseconds(1)).format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                include_vat
            );
            let body = client().get(&url).send().await.context("energyzero request")?.error_for_status().context("energyzero status")?.text().await?;
            Ok(hourly_ct_for_day(&parse_energyzero(&body)?, day, cfg.tz, true))
        }
        PriceSource::Entsoe { token, zone } => {
            let url = format!(
                "https://web-api.tp.entsoe.eu/api?securityToken={token}&documentType=A44&in_Domain={zone}&out_Domain={zone}&periodStart={}&periodEnd={}",
                from.format("%Y%m%d%H%M"),
                till.format("%Y%m%d%H%M")
            );
            let body = client().get(&url).send().await.context("entsoe request")?.error_for_status().context("entsoe status")?.text().await?;
            Ok(hourly_ct_for_day(&parse_entsoe(&body)?, day, cfg.tz, false))
        }
    }
}

pub async fn run_prices(cfg: PriceConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let today = Utc::now().with_timezone(&cfg.tz).date_naive();
        match fetch_day(&cfg, today).await {
            Ok(ct) => {
                failures = 0;
                ctx.emit(Event::Link { target: LinkTarget::Prices, up: true });
                ctx.emit(Event::Prices { date: today.to_string(), ct_per_kwh: ct, currency: "EUR".into() });
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("prices: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link { target: LinkTarget::Prices, up: false });
                }
            }
        }
        // re-poll at the top of the next hour at the latest so the current-hour marker moves
        let now_local = Utc::now().with_timezone(&cfg.tz);
        let to_next_hour = 3600 - (now_local.minute() * 60 + now_local.second()) as u64 + 5;
        let wait = if failures > 0 { 120 } else { cfg.poll_secs.max(60).min(to_next_hour) };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EZ: &str = include_str!("../tests/fixtures/energyzero.json");
    const ENTSOE: &str = include_str!("../tests/fixtures/entsoe.xml");

    #[test]
    fn energyzero_to_local_hours() {
        let pts = parse_energyzero(EZ).unwrap();
        assert_eq!(pts.len(), 24);
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, chrono_tz::Europe::Amsterdam, true);
        assert_eq!(ct.len(), 24);
        // fixture: 22:00Z on the 5th is 00:00 local on the 6th and costs 0.22 €/kWh
        assert!((ct[0] - 22.0).abs() < 0.01);
        assert!(ct[23].is_finite());
    }

    #[test]
    fn entsoe_points_and_units() {
        let pts = parse_entsoe(ENTSOE).unwrap();
        assert_eq!(pts.len(), 24);
        assert_eq!(pts[0].1, 85.5);
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, chrono_tz::Europe::Amsterdam, false);
        assert!((ct[0] - 8.55).abs() < 0.01, "€/MWh to ct/kWh");
        assert!(parse_entsoe("<x/>").is_err());
    }

    #[test]
    fn missing_hours_are_nan_and_zones_map() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&[("2026-09-06T10:00:00Z".into(), 0.1)], day, chrono_tz::Europe::Amsterdam, true);
        assert!(ct[12].is_finite() && ct[11].is_nan());
        assert_eq!(entsoe_zone_for("nl"), Some("10YNL----------L"));
        assert_eq!(entsoe_zone_for("XX"), None);
    }
}
```
Fixture `energyzero.json`: 24 entries from `2026-09-05T22:00:00Z` to `2026-09-06T21:00:00Z` (hourly), prices `0.22, 0.19, 0.17, 0.16, 0.15, 0.15, 0.17, 0.21, 0.24, 0.22, 0.18, 0.13, 0.09, 0.07, 0.06, 0.08, 0.12, 0.19, 0.26, 0.29, 0.27, 0.24, 0.22, 0.21`, shaped as `{"Prices":[{"readingDate":"2026-09-05T22:00:00Z","price":0.22}, ...],"intervalType":4,"average":0.17,"fromDate":"2026-09-05T22:00:00Z","tillDate":"2026-09-06T21:59:59.999Z"}`.
Fixture `entsoe.xml`: a minimal `Publication_MarketDocument` with one `TimeSeries` → `Period` with `timeInterval/start` `2026-09-05T22:00Z`, `resolution` `PT60M`, and 24 `Point` elements `position` 1..24 with `price.amount` `85.5, 80.1, ...` (any 24 numbers, first 85.5).

- [ ] **Step 4: Test, commit**

`cargo test -p rackscreen-sources`, clippy (the `pi`-only build must still link: reqwest with rustls-no-provider and the ring provider is fine for aarch64), fmt.

```bash
git add -A
git commit -m "feat(sources): Electricity Maps and day-ahead price pollers"
```

---

### Task 8: Fake source data and simulator keys

**Files:**
- Modify: `crates/sources/src/fake.rs`, `crates/display/src/sim.rs`, `crates/app/src/config.rs` (nothing), README simulator keys line (Task 11)

**Interfaces:**
- Produces: `FakeCmd::{DegradeVolume ('9'), HealVolume ('0'), HotTemp ('h'), PriceOutage ('p')}`; fake `initial()` and `tick()` emit `NodeTemps`, `Storage`, `Electricity`, `Prices` and `Link` for `Electricity`/`Prices`; simulator forwards keys `9 0 h p`.

- [ ] **Step 1: fake.rs**

State additions: `temps: Vec<(String, f32)>` seeded with `hp-elitedesk-800-g5-i7 53.9, hp-elitedesk-800-g6-i5 41.9, hp-elitedesk-800-g6-i7 51.0, raspberrypi-5-16gb-1 42.1, raspberrypi-5-8gb-1 48.5, raspberrypi-5-8gb-2 40.4, raspberrypi-5-8gb-3 45.6`; `volumes: Vec<(String, Robustness)>` 21 `pvc-01..pvc-21` healthy; `storage_used: u64 = 49 GiB`, `storage_cap: u64 = 128 GiB`; `mix: Vec<(Source, f32)>` seeded `Solar 9200, Wind 3100, Gas 2600, Coal 800, Biomass 300, Nuclear 120, HydroStorage 90, Unknown 40, Hydro 30`; `carbon: f32 = 214`; `prices_ok: bool = true`; `hot_temp_on: bool`.
`initial()` adds `Link Electricity up`, `Link Prices up`, `NodeTemps`, `Storage`, `Electricity`, `Prices` (the daily curve from the EnergyZero fixture values ×100).
`tick()` every second: temps drift ±0.3 clamped 30..80 (a hot node held at 78 while `hot_temp_on`); mix drifts each source ±2 % of its value; solar follows a day curve `max(0, sin(π·(tick % 86400)/43200))·9200` compressed to a 4-minute cycle for the demo (`tick % 240`); carbon drifts ±3; every 10 ticks emit `NodeTemps`, `Storage`, `Electricity`; every 60 ticks emit `Prices` (rotated by one hour so the current-hour marker visibly moves); prices skipped while `!prices_ok`.
`command()`: `DegradeVolume` sets the first healthy volume Degraded and emits `VolumeDegraded` + `Storage`; `HealVolume` heals the first non-healthy and emits `VolumeHealthy` + `Storage`; `HotTemp` toggles `hot_temp_on` and emits `NodeTemps` immediately (with the hot node at 78 or its normal value); `PriceOutage` toggles `prices_ok` and emits `Link Prices` accordingly.
Tests: keys map; initial contains the four new event kinds; degrade then heal round trip; hot temp toggle puts a node ≥ 70; determinism test still passes (run it with the new state).

- [ ] **Step 2: sim.rs keys**

Add `Key::Key9 => '9'`, `Key::Key0 => '0'`, `Key::H => 'h'`, `Key::P => 'p'` to the key map and to the title bar hint.

- [ ] **Step 3: Test, commit**

```bash
git add -A
git commit -m "feat(sources): fake electricity, prices, temperatures and storage; new sim keys"
```

---

### Task 9: App wiring: config to model, pollers, local hour, thresholds

**Files:**
- Modify: `crates/app/src/run.rs`, `crates/app/src/runloop.rs`, `crates/app/Cargo.toml` (chrono-tz)

**Interfaces:**
- Consumes: everything above.
- Produces: `RenderLoop { .., screens_roles, cycle_secs, token_present: bool }`; the loop calls `model.set_local_hour(chrono::Local::now().hour())` every tick; `Monitor::start` spawns `run_electricity` when `cfg.electricity.enabled && !token.is_empty()`, and `run_prices` when `cfg.price.source != "none"` (EnergyZero needs no token; ENTSO-E needs `entsoe_token`, zone from `entsoe_zone` or `entsoe_zone_for(cfg.electricity.zone)`, else it logs a warning and does not start). Both run in every source mode (fake too) unless `--source fake`, where the fake source provides them instead.

- [ ] **Step 1: run.rs**

After the k8s/fake branch, add:
```rust
        if !matches!(source, SourceKind::Fake) {
            if cfg.electricity.enabled && !cfg.electricity.token.is_empty() {
                let ecfg = rackscreen_sources::electricity::ElectricityConfig {
                    zone: cfg.electricity.zone.clone(),
                    token: cfg.electricity.token.clone(),
                    poll_secs: cfg.electricity.poll_secs,
                };
                runtime.spawn(rackscreen_sources::electricity::run_electricity(ecfg, ctx.clone()));
            } else if cfg.electricity.enabled {
                tracing::warn!("electricity enabled but no token set; electricity screens stay on no-data");
            }
            let tz: chrono_tz::Tz = std::env::var("TZ").ok().and_then(|t| t.parse().ok()).unwrap_or(chrono_tz::Europe::Amsterdam);
            let price_source = match cfg.price.source.as_str() {
                "energyzero" => Some(rackscreen_sources::prices::PriceSource::EnergyZero { include_vat: cfg.price.include_vat }),
                "entsoe" => {
                    let zone = if cfg.price.entsoe_zone.is_empty() {
                        rackscreen_sources::prices::entsoe_zone_for(&cfg.electricity.zone).map(str::to_string)
                    } else {
                        Some(cfg.price.entsoe_zone.clone())
                    };
                    match (zone, cfg.price.entsoe_token.is_empty()) {
                        (Some(zone), false) => Some(rackscreen_sources::prices::PriceSource::Entsoe { token: cfg.price.entsoe_token.clone(), zone }),
                        _ => {
                            tracing::warn!("price source entsoe needs entsoe_token and a known zone; prices disabled");
                            None
                        }
                    }
                }
                _ => None,
            };
            if let Some(src) = price_source {
                let pcfg = rackscreen_sources::prices::PriceConfig { source: src, poll_secs: cfg.price.poll_secs, tz };
                runtime.spawn(rackscreen_sources::prices::run_prices(pcfg, ctx.clone()));
            }
        }
```
`RenderLoop` gets `token_present: cfg.electricity.enabled && !cfg.electricity.token.is_empty()` (true for the fake source) and `thresholds` includes `hot_temp`. In `RenderLoop::run`, after `set_screens`, call `model.set_token_present(self.token_present)`; each tick call `model.set_local_hour(chrono::Local::now().hour())` (import `chrono::Timelike`).

- [ ] **Step 2: Runtime check, commit**

`RUST_LOG=info timeout 20 target/debug/rackscreen run --sim --config /tmp/rs-roles.yaml` with a config whose screens are `[power-mix, thermal]`, `[price, storage]`, `[carbon, cpu]`, `[renewable, health]` at `cycle_secs: 5`: log shows `running (4 screens`, no panics; the window shows the electricity screens (fake data) and flips every 5 s. With `--source k8s` and your real config (token set), the log shows the first `Electricity` poll succeeding (no `electricity:` warning) and prices arriving.

```bash
git add -A
git commit -m "feat(app): wire role lists, electricity and price pollers, local hour"
```

---

### Task 10: TUI Screens editor

**Files:**
- Create: `crates/setup/src/screens/screens.rs`
- Modify: `crates/setup/src/lib.rs` (`ScreenId::Screens`), `crates/setup/src/screens/mod.rs`, `crates/setup/src/screens/menu.rs` (menu item between Calibrate and Configure), `crates/setup/src/screens/install.rs` (nothing), `crates/setup/src/ops/config_file.rs` (`set_screen_roles`)

**Interfaces:**
- Produces:
  - `ops::config_file::set_screen_roles(cfg: &mut Config, index: usize, roles: &[Role], cycle_secs: u64)`, `ops::config_file::apply_preset(cfg: &mut Config, preset: Preset)`, `Preset { Cluster, Electricity, Mixed }` with `Preset::rows(&self) -> Vec<Vec<Role>>` (applied to the first four screens; extra screens keep their roles).
  - `screens::screens::{Editor (pure state), Screens (the Screen impl)}`; `Editor::new(rows: Vec<(Vec<Role>, u64)>)`, `.rows()`, `.selected`, `.move_selection(delta)`, `.adjust_cycle(delta_secs)`, `.open_picker()`, `.picker_toggle()`, `.picker_move(delta)`, `.picker_shift(delta)` (reorder), `.close_picker()`, `.apply_preset(Preset)`, `.validate() -> Result<(), String>`.

- [ ] **Step 1: config_file.rs additions**

```rust
use rackscreen_core::theme::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Cluster,
    Electricity,
    Mixed,
}

impl Preset {
    pub fn rows(self) -> Vec<Vec<Role>> {
        use Role::*;
        match self {
            Preset::Cluster => vec![vec![Cpu], vec![Mem], vec![Pods], vec![Health]],
            Preset::Electricity => vec![vec![PowerMix], vec![Price], vec![Carbon], vec![Renewable]],
            Preset::Mixed => vec![vec![Cpu, PowerMix], vec![Mem, Price], vec![Pods, Carbon], vec![Health, Renewable]],
        }
    }
}

pub fn set_screen_roles(cfg: &mut Config, index: usize, roles: &[Role], cycle_secs: u64) {
    if let Some(s) = cfg.screens.get_mut(index) {
        s.roles = roles.iter().map(|r| r.name().to_string()).collect();
        s.cycle_secs = cycle_secs.clamp(3, 300);
    }
}

pub fn apply_preset(cfg: &mut Config, preset: Preset) {
    for (i, roles) in preset.rows().into_iter().enumerate() {
        set_screen_roles(cfg, i, &roles, 15);
    }
}
```
Test: `apply_preset(Electricity)` on the default config gives `screens[0].roles == ["power-mix"]`, and a fifth screen (push a clone) keeps its roles.

- [ ] **Step 2: screens.rs**

```rust
//! Screens editor: which roles each physical screen cycles through, and how fast.

use rackscreen_app::config::Config;
use rackscreen_core::anim::Secs;
use rackscreen_core::theme::Role;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::config_file::{apply_preset, save_config, set_screen_roles, Preset};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::widgets::confirm_dialog;
use crate::{Action, Screen, Shared};

/// Pure editor state, tested without a terminal.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    rows: Vec<(Vec<Role>, u64)>,
    pub selected: usize,
    picker: Option<Picker>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    /// Ordered roles of the screen being edited.
    pub chosen: Vec<Role>,
    /// Cursor over `Role::ALL`.
    pub cursor: usize,
}

impl Editor {
    pub fn new(rows: Vec<(Vec<Role>, u64)>) -> Editor {
        Editor { rows, selected: 0, picker: None }
    }
    pub fn rows(&self) -> &[(Vec<Role>, u64)] {
        &self.rows
    }
    pub fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }
    pub fn move_selection(&mut self, delta: i32) {
        let n = self.rows.len().max(1) as i32;
        self.selected = ((self.selected as i32 + delta).rem_euclid(n)) as usize;
    }
    pub fn adjust_cycle(&mut self, delta: i64) {
        if let Some(r) = self.rows.get_mut(self.selected) {
            r.1 = ((r.1 as i64 + delta).clamp(3, 300)) as u64;
        }
    }
    pub fn open_picker(&mut self) {
        let chosen = self.rows.get(self.selected).map(|r| r.0.clone()).unwrap_or_default();
        self.picker = Some(Picker { chosen, cursor: 0 });
    }
    pub fn picker_move(&mut self, delta: i32) {
        if let Some(p) = &mut self.picker {
            let n = Role::ALL.len() as i32;
            p.cursor = ((p.cursor as i32 + delta).rem_euclid(n)) as usize;
        }
    }
    /// Space: add the role under the cursor (at the end) or remove it.
    pub fn picker_toggle(&mut self) {
        if let Some(p) = &mut self.picker {
            let role = Role::ALL[p.cursor];
            if let Some(i) = p.chosen.iter().position(|r| *r == role) {
                p.chosen.remove(i);
            } else {
                p.chosen.push(role);
            }
        }
    }
    /// J/K: move the role under the cursor within the chosen order.
    pub fn picker_shift(&mut self, delta: i32) {
        if let Some(p) = &mut self.picker {
            let role = Role::ALL[p.cursor];
            if let Some(i) = p.chosen.iter().position(|r| *r == role) {
                let j = (i as i32 + delta).clamp(0, p.chosen.len() as i32 - 1) as usize;
                p.chosen.swap(i, j);
            }
        }
    }
    /// Enter: commit the picker (empty selection keeps the old roles).
    pub fn close_picker(&mut self) {
        if let Some(p) = self.picker.take() {
            if !p.chosen.is_empty() {
                if let Some(r) = self.rows.get_mut(self.selected) {
                    r.0 = p.chosen;
                }
            }
        }
    }
    pub fn cancel_picker(&mut self) {
        self.picker = None;
    }
    pub fn apply_preset(&mut self, preset: Preset) {
        for (i, roles) in preset.rows().into_iter().enumerate() {
            if let Some(r) = self.rows.get_mut(i) {
                *r = (roles, 15);
            }
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        for (i, (roles, secs)) in self.rows.iter().enumerate() {
            if roles.is_empty() {
                return Err(format!("screen {} has no roles", i + 1));
            }
            if !(3..=300).contains(secs) {
                return Err(format!("screen {} cycle must be 3..300 s", i + 1));
            }
        }
        Ok(())
    }
}

enum Mode {
    Edit,
    AskRestart,
    Restarting(std::sync::mpsc::Receiver<String>),
}

pub struct Screens {
    cfg: Result<Config, String>,
    editor: Editor,
    mode: Mode,
    error: Option<String>,
    dirty: bool,
}

fn rows_from(cfg: &Config) -> Vec<(Vec<Role>, u64)> {
    cfg.screens.iter().map(|s| (s.roles().unwrap_or_else(|_| vec![Role::Cpu]), s.cycle_secs)).collect()
}

impl Screens {
    pub fn new(shared: &Shared) -> Screens {
        let cfg = Config::load_or_default(&shared.ctx.config_path).map_err(|e| format!("config unreadable: {e:#}; fix the file by hand"));
        let editor = Editor::new(cfg.as_ref().map(rows_from).unwrap_or_else(|_| Preset::Cluster.rows().into_iter().map(|r| (r, 15)).collect()));
        Screens { error: cfg.as_ref().err().cloned(), cfg, editor, mode: Mode::Edit, dirty: false }
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if let Err(e) = self.editor.validate() {
            self.error = Some(e);
            return Action::None;
        }
        let Ok(cfg) = &mut self.cfg else {
            return Action::None;
        };
        for (i, (roles, secs)) in self.editor.rows().iter().enumerate() {
            set_screen_roles(cfg, i, roles, *secs);
        }
        match save_config(cfg, &shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                shared.banner = Some("screens saved".into());
                let sh = RealShell;
                let active = !shared.ctx.sim && Systemd::new(&sh, &service_user()).is_active().unwrap_or(false);
                shared.service_active = Some(active);
                if active {
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

    fn spawn_restart(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("restart".into())
            .spawn(move || {
                let sh = RealShell;
                let msg = match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "service restarted".to_string(),
                    Err(e) => format!("restart failed: {e:#}"),
                };
                let _ = tx.send(msg);
            })
            .expect("spawn restart");
        self.mode = Mode::Restarting(rx);
    }
}

impl Screen for Screens {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        if matches!(self.mode, Mode::Restarting(_)) {
            return Action::None;
        }
        if matches!(self.mode, Mode::AskRestart) {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                self.spawn_restart();
                return Action::None;
            }
            self.mode = Mode::Edit;
            return Action::Back;
        }
        if self.editor.picker().is_some() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.editor.picker_move(-1),
                KeyCode::Down | KeyCode::Char('j') => self.editor.picker_move(1),
                KeyCode::Char(' ') => {
                    self.editor.picker_toggle();
                    self.dirty = true;
                }
                KeyCode::Char('K') => self.editor.picker_shift(-1),
                KeyCode::Char('J') => self.editor.picker_shift(1),
                KeyCode::Enter => self.editor.close_picker(),
                KeyCode::Esc => self.editor.cancel_picker(),
                _ => {}
            }
            return Action::None;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.editor.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.editor.move_selection(1),
            KeyCode::Enter => self.editor.open_picker(),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.editor.adjust_cycle(5);
                self.dirty = true;
            }
            KeyCode::Char('-') => {
                self.editor.adjust_cycle(-5);
                self.dirty = true;
            }
            KeyCode::Char('c') => {
                self.editor.apply_preset(Preset::Cluster);
                self.dirty = true;
            }
            KeyCode::Char('e') => {
                self.editor.apply_preset(Preset::Electricity);
                self.dirty = true;
                if let Ok(cfg) = &self.cfg {
                    if cfg.electricity.token.is_empty() {
                        self.error = Some("electricity preset needs an API token: set it under Configure".into());
                    }
                }
            }
            KeyCode::Char('m') => {
                self.editor.apply_preset(Preset::Mixed);
                self.dirty = true;
            }
            KeyCode::Char('s') => return self.save(shared),
            KeyCode::Esc | KeyCode::Char('q') => return Action::Back,
            _ => {}
        }
        Action::None
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        let done = if let Mode::Restarting(rx) = &self.mode {
            match rx.try_recv() {
                Ok(msg) => Some(msg),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some("restart thread died".into()),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            }
        } else {
            None
        };
        if let Some(msg) = done {
            shared.banner = Some(msg);
            self.mode = Mode::Edit;
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, roles, foot] = Layout::vertical([Constraint::Length(1), Constraint::Length(self.editor.rows().len() as u16 + 1), Constraint::Min(4), Constraint::Length(2)]).areas(area);
        let mut lines = Vec::new();
        for (i, (rs, secs)) in self.editor.rows().iter().enumerate() {
            let sel = i == self.editor.selected;
            let names = rs.iter().map(|r| r.name()).collect::<Vec<_>>().join(" › ");
            let timing = if rs.len() > 1 { format!("every {secs} s") } else { "static".into() };
            let warn = rs.iter().any(|r| self.role_needs_token(*r));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{} ", if sel { g.pointer } else { " " }), th.selected()),
                Span::styled(format!("{}  ", i + 1), th.muted()),
                Span::styled(format!("{names:<30}"), if sel { th.selected() } else { th.normal() }),
                Span::styled(timing, th.muted()),
                Span::styled(if warn { "  ! no token" } else { "" }, th.warning()),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);

        let mut body = vec![Line::from(Span::styled("  Roles:", th.muted()))];
        if let Some(p) = self.editor.picker() {
            for (i, r) in Role::ALL.iter().enumerate() {
                let on = p.chosen.iter().position(|x| x == r);
                let mark = match on {
                    Some(k) => format!("{}{}", g.done, k + 1),
                    None => g.pending.to_string(),
                };
                let cur = i == p.cursor;
                body.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("{} ", if cur { g.pointer } else { " " }), th.selected()),
                    Span::styled(format!("{mark:<3}"), if on.is_some() { th.good() } else { th.faint_style() }),
                    Span::styled(r.name().to_string(), if cur { th.selected() } else { th.normal() }),
                ]));
            }
        } else {
            body.push(Line::from(Span::styled(format!("    {}", Role::ALL.iter().map(|r| r.name()).collect::<Vec<_>>().join("  ")), th.faint_style())));
            body.push(Line::from(""));
            body.push(Line::from(vec![Span::styled("  Presets: ", th.muted()), Span::styled("c", th.selected()), Span::styled(" cluster   ", th.muted()), Span::styled("e", th.selected()), Span::styled(" electricity   ", th.muted()), Span::styled("m", th.selected()), Span::styled(" mixed", th.muted())]));
        }
        f.render_widget(Paragraph::new(body), roles);

        let msg = match (&self.error, self.dirty) {
            (Some(e), _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (None, true) => Line::from(Span::styled("  unsaved changes: s to save", th.warning())),
            (None, false) => Line::from(Span::styled(format!("  {}", shared.ctx.config_path.display()), th.faint_style())),
        };
        f.render_widget(Paragraph::new(msg), foot);
        if matches!(self.mode, Mode::AskRestart) {
            confirm_dialog(f, area, th, "Restart service?", &["Apply the new screen layout to the running service now?".to_string()], "y/⏎ restart   Esc later", false);
        }
    }

    fn keys(&self) -> String {
        if self.editor.picker().is_some() {
            "↑↓ move  space toggle  J/K reorder  ⏎ done  Esc cancel".into()
        } else {
            "↑↓ screen  ⏎ roles  +/- interval  c/e/m preset  s save  Esc back".into()
        }
    }
    fn subtitle(&self) -> String {
        "Screens".into()
    }
}

impl Screens {
    fn role_needs_token(&self, r: Role) -> bool {
        matches!(r, Role::PowerMix | Role::Carbon | Role::Renewable) && self.cfg.as_ref().map(|c| c.electricity.token.is_empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_toggle_reorder_and_presets() {
        let mut e = Editor::new(vec![(vec![Role::Cpu], 15), (vec![Role::Mem], 15)]);
        e.move_selection(-1);
        assert_eq!(e.selected, 1);
        e.adjust_cycle(-20);
        assert_eq!(e.rows()[1].1, 3);
        e.adjust_cycle(500);
        assert_eq!(e.rows()[1].1, 300);
        e.open_picker();
        e.picker_move(Role::Thermal.index() as i32);
        e.picker_toggle();
        assert_eq!(e.picker().unwrap().chosen, vec![Role::Mem, Role::Thermal]);
        e.picker_shift(-1);
        assert_eq!(e.picker().unwrap().chosen, vec![Role::Thermal, Role::Mem]);
        e.close_picker();
        assert_eq!(e.rows()[1].0, vec![Role::Thermal, Role::Mem]);
        e.open_picker();
        e.picker_move(Role::Thermal.index() as i32);
        e.picker_toggle();
        e.picker_move(Role::Mem.index() as i32 - Role::Thermal.index() as i32);
        e.picker_toggle();
        e.close_picker();
        assert_eq!(e.rows()[1].0, vec![Role::Thermal, Role::Mem], "empty selection keeps old roles");
        e.apply_preset(Preset::Electricity);
        assert_eq!(e.rows()[0].0, vec![Role::PowerMix]);
        assert_eq!(e.rows().len(), 2, "preset only touches existing rows");
        assert!(e.validate().is_ok());
    }

    #[test]
    fn screens_render_rows_and_picker() {
        use crate::theme::Theme;
        use crate::Ctx;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let dir = tempfile::tempdir().unwrap();
        let sh = Shared {
            ctx: Ctx { config_path: dir.path().join("c.yaml"), sim: true, version: "0.3.0" },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: rackscreen_app::logs::LogSink::new(10),
        };
        let mut s = Screens::new(&sh);
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("▸ 1  cpu"));
        assert!(text.contains("static"));
        assert!(text.contains("Presets"));
        s.handle(KeyEvent::from(KeyCode::Enter), &mut sh.clone_for_test(), 0.0);
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("power-mix"));
        assert!(text.contains("✓1 cpu") || text.contains("✓1  cpu"));
    }
}
```
`Shared` gets a `#[cfg(test)] pub fn clone_for_test(&self) -> Shared` helper in `lib.rs` (clone of ctx/theme/service_active/banner/log_sink) or the test uses a second `Shared` literal; either is fine. Save uses `save_config` from `ops::config_file` (added in the v0.2 fixes).

- [ ] **Step 3: Menu and routing**

`lib.rs`: add `ScreenId::Screens`. `menu.rs`: `ITEMS` becomes 7 entries with `(ScreenId::Screens, "Screens", "what each screen shows, cycling")` inserted after Calibrate; the `Layout` constraint for the list uses `ITEMS.len()` already. `screens/mod.rs`: `pub mod screens;` and `ScreenId::Screens => Box::new(screens::Screens::new(shared))`.

- [ ] **Step 4: Test, commit**

`cargo test -p rackscreen-setup`, clippy, fmt; pty harness: open Screens from the menu, press `e`, `s`, then check the temp config has `roles: [power-mix]` on screen 1.

```bash
git add -A
git commit -m "feat(setup): screens editor with role picker and presets"
```

---

### Task 11: Configure fields, Status dots, README, version 0.3.0

**Files:**
- Modify: `crates/setup/src/screens/configure.rs`, `crates/setup/src/screens/status.rs`, `crates/setup/src/ops/systemd.rs` (`LinkDots` + `links_from_logs`), `README.md`, `Cargo.toml`

- [ ] **Step 1: Configure fields**

Add to `Field`: `ElecEnabled (Bool)`, `ElecZone (Text)`, `ElecToken (Secret)`, `ElecPoll (Number)`, `PriceSource (Choice)`, `EntsoeToken (Secret)`, `EntsoeZone (Text)`, `IncludeVat (Bool)`, `PricePoll (Number)`, `HotTemp (Number)`. `FieldKind::Choice` cycles through `["energyzero", "entsoe", "none"]` on Enter/space (like Bool). `FIELDS` grows to 30 entries with labels `electricity enabled`, `electricity zone`, `electricity api token`, `electricity poll secs`, `price source`, `entsoe token`, `entsoe zone (EIC)`, `price incl. VAT`, `price poll secs`, `hot node temp °C`. `get`/`set` map to `cfg.electricity.*`, `cfg.price.*`, `cfg.thresholds.hot_temp`; `set` for `PriceSource` accepts only the three values. Extend the round-trip test to cover the new fields and a `PriceSource` cycle.

- [ ] **Step 2: Status link dots**

`LinkDots` gains `electricity: Dot` and `prices: Dot`; `links_from_logs` sets `electricity` Up on a line containing `electricity:` without warn/error and Down on `electricity:` with warn/error (the poller logs `electricity: poll ok` at info on success: add that `tracing::info!` in `run_electricity`); same for `prices:` (add `tracing::info!("prices: {} hours for {}", n, date)` on success). Status draw shows two more dots on the links line: `electricity`, `prices`. Update the `journal_and_links` test with sample lines.

- [ ] **Step 3: README and version**

README: new section "Screens and roles" (role list, cycling, presets, the Screens editor), "Electricity mode" (get a free token at app.electricitymaps.com, set zone and token in Configure, price source choice, EnergyZero is NL-only and token-free, ENTSO-E needs a token and an EIC zone), simulator keys line adds `9 degrade volume, 0 heal volume, h hot node, p price outage`, licence section adds the Electricity Maps icons under AGPL-3.0 with a link to `assets/icons/EM-LICENSE.md`. Version `0.3.0` in `[workspace.package]`.

- [ ] **Step 4: Full verification, commit**

`cargo test --workspace --features sim,pi`, the three clippy builds, `cargo build --no-default-features --features pi`, `cargo fmt --all`. Simulator run with the `mixed` preset for 30 s: transitions visible, no panics.

```bash
git add -A
git commit -m "feat: configure electricity and price settings, status dots, README; v0.3.0"
```

---

## Handoff

Push, tag `v0.3.0`. On the Pi: run `rackscreen`, Configure → set the Electricity Maps token and zone, Screens → `e` or `m` preset, save and restart. Check Status shows green dots for electricity and prices after a minute.

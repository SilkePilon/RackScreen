# Pixel Shows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every 15 minutes in daytime, all four panels pause their roles and play one of 11 original pixel-art animations across the four screens as one wide scene.

**Architecture:** A pure `shows` module in `rackscreen-core` draws a `W×60` RGBA world per 12 fps frame with mockup-ported primitives; the model schedules shows and hands each screen a scene with `Drawable::World { frame, x0 }`, which the renderer blits ×4 nearest-neighbour. Config `shows:`, a play-now request file, a `rackscreen play` subcommand and a setup TUI group drive it.

**Tech Stack:** Rust 2021, tiny-skia, existing render workers.

Spec: `docs/superpowers/specs/2026-11-03-pixel-shows-design.md`. **Visual source of truth:** `docs/superpowers/specs/assets/2026-11-03-pixel-shows-round1.html` and `…-round2.html` — each show is a port of one JS function there (see the spec table), with the same palettes, sprites (string arrays), timings and helper semantics.

## Global Constraints

- World: `H = 60`, panel slice 60×60, `W = n·60 + (n−1)·gap`, panel `i` at `x0 = i·(60 + gap)`; `gap = round(gap_mm / 0.54)` (`gap_mm` default 25 → 46). Scale ×4 nearest-neighbour.
- 12 fps: frame index `f = floor(t·12)`, local time `t` quantised to `f/12`; each show is a pure function of `(f, t)`.
- `rnd(i) = fract(sin(i·127.1 + 311.7)·43758.5453)` in `f64`; colours exactly the mockups' hex values.
- Mockup absolute x positions are re-expressed as `panel + offset` so each element stays on the same screen (the mockups used gap 8: panel `i` started at `i·68`).
- Schedule: `every_mins` default 15 (min 2), only in `NightPhase::Day`, not while any screen transitions, ≥ 2 screens; random from `list` without immediate repeat; sequence roles-out 0.2 s → show → show-out 0.2 s → roles-in 0.2 s; dwells extended by the whole sequence.
- Config `shows: { enabled: true, every_mins: 15, gap_mm: 25, list: [all 11 ids], order: optional permutation }`; ids: `freight, deep-sea, paper-plane, storm, day, space, birds, fireworks, alpine, caravan, desert-night`.
- Play-now file: `<state dir>/rackscreen/play` where state dir is `$XDG_STATE_HOME` or `~/.local/state` of the service user; content an id or `random`; polled once a second; deleted when consumed; ignored at night.
- No new external crates. Commit after every task; messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Before every commit: `cargo test --workspace -q`, `cargo clippy --workspace --all-targets -q -- -D warnings`, `cargo fmt --check`. Steady-scene goldens stay byte-identical (the golden test has a tolerance; compare exactly with a throwaway check where a task must not change them).

---

### Task 1: World engine, `Drawable::World`, first show (freight)

**Files:**
- Create: `crates/core/src/shows/mod.rs` (registry, `ShowDef`), `crates/core/src/shows/world.rs` (`World`, layout, primitives, `rnd`), `crates/core/src/shows/freight.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs` (`Drawable::World`), `crates/render/src/renderer.rs` (blit arm), `crates/render/tests/golden.rs` (freight strip golden)

**Interfaces (produce exactly):**

```rust
// shows/world.rs
pub const WORLD_H: u32 = 60;
pub const PANEL_W: u32 = 60;
pub const SCALE: u32 = 4;
pub fn gap_px(gap_mm: f32) -> u32 { (gap_mm / 0.54).round().max(0.0) as u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout { pub panels: u32, pub gap: u32 }
impl Layout {
    pub fn width(&self) -> u32;            // panels*60 + (panels-1)*gap
    pub fn x0(&self, i: u32) -> u32;        // i*(60+gap)
}
pub fn rnd(i: f64) -> f64;                 // mockup hash
/// RGBA8, row-major, width × 60. Drawing clips to the raster.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldFrame { pub width: u32, pub rgba: Vec<u8> }
pub struct World<'a> { pub frame: &'a mut WorldFrame, pub layout: Layout }
impl World<'_> {
    pub fn w(&self) -> i32; pub fn h(&self) -> i32;
    pub fn px0(&self, i: u32) -> i32;      // layout.x0 as i32
    pub fn p(&mut self, c: u32, x: f64, y: f64, w: i32, h: i32);   // P(): rounds x,y
    pub fn spr(&mut self, rows: &[&str], pal: &[(char, u32)], x: f64, y: f64, flip: bool);
    pub fn dith(&mut self, c: u32, x: i32, y: i32, w: i32, h: i32, ph: i32);
    pub fn disc(&mut self, c: u32, cx: f64, cy: f64, r: i32);
    pub fn ring(&mut self, c: u32, cx: f64, cy: f64, r: f64);
    pub fn bands(&mut self, list: &[(u32, i32)]);
    pub fn clear(&mut self, c: u32);
}
// colours are 0xRRGGBB
// shows/mod.rs
pub struct ShowDef { pub id: &'static str, pub name: &'static str, pub secs: f64, pub draw: fn(&mut World, f: u32, t: f64) }
pub const SHOWS: &[ShowDef];               // in spec order; Task 1 has only freight
pub fn by_id(id: &str) -> Option<&'static ShowDef>;
pub fn render(def: &ShowDef, layout: Layout, t: f64) -> WorldFrame; // quantises t to 12 fps, f = floor(t*12), clears, draws
```

`Drawable::World { frame: std::sync::Arc<WorldFrame>, x0: u32 }` with a manual `PartialEq` arm semantics: equal when `Arc::ptr_eq(a, b) && x0 == x0'` (implement `PartialEq` for a small newtype `WorldRef(pub Arc<WorldFrame>)` comparing by pointer, and use `Drawable::World { frame: WorldRef, x0 }`, so `Drawable`'s derive keeps working).

Renderer: `Drawable::World { frame, x0 }` writes, for each target pixel `(px, py)` in 0..240, the world pixel `(x0 + px/4, py/4)` (opaque; alpha 255) — a straight copy into the RGBA target (tiny-skia stores premultiplied; opaque colours are identical).

- [ ] **Step 1: Failing tests** in `world.rs`: `gap_px(25.0) == 46`; `Layout{panels:4,gap:46}.width() == 378`, `x0(3) == 318`; `rnd(1.0)` equals the JS value `0.10468242550996365` within 1e-9; `p` clips off-raster writes; `spr` with `"ab"`/palette draws two pixels and skips `.`; `dith` sets alternate pixels; `bands` fills rows and writes the seam dither like the mockup (row before each boundary gets the next colour on even pixels, the boundary row the previous colour on odd).
- [ ] **Step 2: Implement `world.rs`** by porting the mockup helpers verbatim (`P` = `p`, `spr`, `dith`, `disc` uses `w = floor(sqrt(r*r - y*y + r*0.6))`, `ring` uses `n = max(8, floor(r*6.3))` points, `bands` as in the mockup). JS `Math.round` rounds .5 up (towards +∞); use `(v + 0.5).floor()` for it, not `f64::round`.
- [ ] **Step 3: `Drawable::World` and renderer blit** with a renderer test: a 2-panel layout world with panel 1 filled red renders `x0 = 66` (gap 6) as a red 240×240 frame; `x0 = 0` stays the other colour.
- [ ] **Step 4: Port `freight`** from round-1 `freight(g,f,t,L)`: same sprites (`LOCO`, `CARR`), palette `TRP`, sky bands, sun, hills formulas, track, smoke and poles; L = 9. Replace `px0(i)` with `world.px0(i)`; the mockup's absolute positions (sun at x 120 = panel 1 + 52, sun-cut line x 100..140) become `px0(1) + 52` etc.; full-width loops use `world.w()`; train timing uses `world.w()` so it still crosses all screens.
- [ ] **Step 5: Show tests** in `shows/mod.rs`, generic over `SHOWS` (later tasks get them for free): every show renders every frame `0..secs*12` for layout `{4, 46}` and `{3, 46}` without panic; `render(def, l, 1.0) == render(def, l, 1.0)`; frame 0 differs from frame 6 somewhere; no pixel is left at the clear value `0x000000` with alpha 0 (every show paints its whole world).
- [ ] **Step 6: Golden**: in `golden.rs`, `show_freight` — render freight at t = 4.5 with layout `{4, 46}` and compose the four panels into one 1056×240 PNG (240 px panels, 32 px black between, each panel clipped to its circle like the mockup strip). Add a helper `show_strip(def, t) -> Pixmap` reused by later tasks. Look at the PNG with the Read tool and compare with the mockup's look (sky bands, sun, teal train with lit windows, smoke, poles).
- [ ] **Step 7: Commit** `feat(core,render): pixel world engine for four-screen shows; dusk freight`.

### Task 2: Scheduler, config, play-now request

**Files:** `crates/core/src/shows/schedule.rs` (new), `crates/core/src/model.rs`, `crates/core/src/scene_model.rs`, `crates/core/src/screens.rs` (dwell extension), `crates/app/src/config.rs`, `crates/app/src/runloop.rs`, `crates/app/src/run.rs`, `src/main.rs` (`Play` subcommand)

**Interfaces:**

```rust
// schedule.rs
pub const SHOW_FADE_SECS: f64 = 0.2;
#[derive(Clone, Debug, PartialEq)]
pub enum ShowPhase { RolesOut, Playing, ShowOut, RolesIn }
pub struct ShowRun { pub def: &'static ShowDef, pub started: Secs }
impl ShowRun {
    pub fn total(&self) -> Secs;                       // 3*fade + def.secs
    pub fn phase(&self, now: Secs) -> Option<(ShowPhase, f32)>;  // brightness 0..1 for the phase; None when over
}
pub struct ShowScheduler { /* enabled, every, list, last_id, last_end, current: Option<ShowRun> */ }
impl ShowScheduler {
    pub fn new(enabled: bool, every_mins: u32, list: Vec<&'static ShowDef>) -> Self;
    /// Start one when due (or `request` is set) and allowed; returns true when one started.
    pub fn tick(&mut self, now: Secs, allowed: bool, request: Option<Request>, rng: &mut impl FnMut() -> u64) -> bool;
    pub fn current(&self) -> Option<&ShowRun>;
}
pub enum Request { Id(&'static ShowDef), Random }
```

Model: `set_shows(scheduler, layout, order: Vec<usize>)`, `request_show(Request)`, and in `tick`: `allowed = night_day && !any screen transitioning && screens.len() >= 2`; on start, every `ScreenState` gets `extend_dwell(run.total())`. `Model::set_night_day(bool)` is fed by the run loop (true only in `NightPhase::Day`). The model caches `(show id, frame index) -> Arc<WorldFrame>`.

`scene(screen, now, ui)` during a run: `RolesOut` → the role scene with `fade = brightness` (as the crossfade out-half); `Playing`/`ShowOut` → a scene `[Clear(BLACK), World { frame, x0: layout.x0(position of this screen) }]` with `fade = brightness`; `RolesIn` → role scene with `fade`. Screen position = index of `screen` in `order` (default identity).

Config: `ShowsCfg { enabled: bool = true, every_mins: u32 = 15, gap_mm: f32 = 25.0, list: Vec<String> = all ids, order: Option<Vec<usize>> }`; validation: `every_mins >= 2`, every id known (error names it), `order` a permutation of `0..screens.len()`.

Play-now: run loop checks the request file once a second (`state_dir()/rackscreen/play`, `state_dir` = `$XDG_STATE_HOME` or `$HOME/.local/state`), parses `random` or an id, calls `model.request_show`, deletes the file (log a warn on unknown id and delete). `rackscreen play <id|random>` writes that file for the service user: reuse the existing `sudo_user()`/home logic in `config.rs` (when run with sudo, write into the `SUDO_USER`'s state dir and chown the file to them).

- [ ] Tests first for: `ShowRun::phase` boundaries (0, 0.2, 0.2+secs, total); scheduler due after `every_mins`, not when `allowed` false (and starts as soon as allowed again), no immediate repeat with ≥ 2 shows, request starts immediately but not when not allowed (kept until allowed? **no** — a request at night is dropped with an info log); model: dwell extended (no role transition during or right after a run), scenes per phase (world drawable with the right `x0` per screen and `order`), fade values; config defaults/validation; request-file parsing.
- [ ] Implement; wire `run.rs` to build the scheduler from config and pass `gap_px(cfg.shows.gap_mm)`; run loop feeds `set_night_day(phase == NightPhase::Day)`.
- [ ] Simulator smoke: `cargo run -q -- run --sim --source fake` with a temp config `shows: {every_mins: 2}` or `rackscreen play freight` into the temp state dir (set `XDG_STATE_HOME`), watch the strip play; note in report (if no display, skip and say so).
- [ ] Commit `feat: four-screen show scheduler, shows config and rackscreen play`.

### Task 3: Port deep-sea, paper-plane, storm, day, space

**Files:** `crates/core/src/shows/{deep_sea,paper_plane,storm,day,space}.rs`, `shows/mod.rs` (register), `golden.rs` (one strip golden each)

- [ ] For each: port the round-1 JS function (`deep`, `plane`, `storm`, `daycycle`, `space`) with identical palettes, sprites and timing; re-express absolute x as panel + offset (round-1 panel `i` started at `i·68`; e.g. storm's bolt at x 148 = panel 2 + 12, space's planet at x 178 = panel 2 + 42, paper-plane's loop centre x 158 = panel 2 + 22, birds' sun x 140 = panel 2 + 4); full-width loops and travel ranges use `world.w()`. The generic show tests from Task 1 cover them; add a strip golden per show at a characteristic moment (deep-sea t 5, paper-plane t 5.2 mid-loop, storm t 3.95 flash, day t 3 and t 11, space t 4) and look at each with the Read tool against the mockup.
- [ ] Commit `feat(core): shows deep sea, paper plane, storm, day in a minute, space cruise`.

### Task 4: Port birds, fireworks, alpine, caravan, desert-night

- [ ] Same method for round-1 `birds`, `fireworks` and round-2 `alpine`, `caravan`, `desertNight` (round-2 file uses the same `i·68` panel origin). Strip goldens: birds t 4, fireworks t 5.6 finale, alpine t 7, caravan t 5, desert-night t 5.2 (shooting star). Look at each.
- [ ] Commit `feat(core): shows birds, fireworks, alpine dawn, caravan, desert night`.

### Task 5: Setup TUI, README gifs, version

- [ ] Setup TUI Configure group "Shows": enabled toggle, every N minutes (2..240), gap mm (5..80), per-show checklist (at least one must stay on; refuse with a message otherwise), "Play now" action that writes the request file (same helper as `rackscreen play`) and shows "requested <id>". Follow the existing Configure group patterns and tests.
- [ ] `crates/app/examples/gifs.rs`: one gif per show, `.github/media/shows/<id>.gif`, as the 4-circle strip (reuse `show_strip` logic: 240 px panels, 32 px gaps, transparent outside the circles), 12 fps, one full loop. README: a "Shows" section with the 11 gifs, the config block, `rackscreen play`, and the note that shows are original pixel art.
- [ ] Version 0.11.0; `config.example.yaml` gets the `shows:` block.
- [ ] Commit `feat: shows in the setup TUI, README gifs; version 0.11.0`.

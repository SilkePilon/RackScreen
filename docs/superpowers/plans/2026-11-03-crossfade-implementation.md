# Crossfade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the vertical slide between roles with a 0.4 s crossfade via black: the outgoing role fades to black in 0.2 s, the incoming role fades in over 0.2 s.

**Architecture:** During a transition the model builds only one role scene (the outgoing one in the first half, the incoming one in the second) and sets `Scene::fade` to its brightness. The renderer draws the scene normally (base cache included) and then darkens the whole frame through a 256-entry lookup table. The slide types, the renderer's scratch pixmap and round mask are removed. Night fade uses the same darken function.

**Tech Stack:** Rust 2021 workspace, tiny-skia 0.12.

Spec: `docs/superpowers/specs/2026-11-03-crossfade-design.md`. Builds on the speed pipeline (merged, 0.9.0 unreleased): per-screen render workers skip a frame whose `Scene` (and night fade) equals the last one, so the quantised fade brightness makes equal fade frames skip.

## Global Constraints

- `TRANSITION_SECS` = 0.4; `fade(t)`: for `t < 0.5` the outgoing scene with brightness `1 − ease_in_out_cubic(2t)`, else the incoming scene with brightness `ease_in_out_cubic(2t − 1)`; brightness quantised to 1/64.
- `Slide` and `Placement` are removed; `Scene` gains `fade: f32` (1.0 = normal).
- One screen transitions at a time stays (`one_at_a_time`, `TRANSITION_GAP_SECS` unchanged).
- `transition_into_a_cluster_role_with_api_down_shows_connecting` keeps passing (adapted to the fade).
- Goldens of steady scenes stay byte-identical (no re-capture); the golden test has a pixel tolerance, so the plan's golden checks compare exactly where stated.
- No new crates. Version stays 0.9.0 (still unreleased).
- Commit after every task; messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Before every commit: `cargo test --workspace -q`, `cargo clippy --workspace --all-targets -q -- -D warnings`, `cargo fmt --check`.

## File Structure

| file | change |
|---|---|
| `crates/render/src/frame.rs` | `darken(px, k)` via `brightness_lut` |
| `crates/render/src/renderer.rs` | apply `scene.fade`; later drop slide path, scratch, round mask |
| `crates/core/src/scene.rs` | `Scene::fade`; later drop `Slide`, `Placement`, `slide` field |
| `crates/core/src/screens.rs` | `TRANSITION_SECS` 0.4, `Half`, `fade(t)`; drop `slide(t)` |
| `crates/core/src/scene_model.rs` | build one half with `fade` |
| `crates/core/src/p2/ui.rs` | rename `end_slide` → `end_transition`, docs |
| `crates/app/src/worker.rs`, `runloop.rs` | night fade uses `rackscreen_render::frame::darken`; `runloop::darken` removed |
| `crates/render/tests/golden.rs`, `bench.rs` | fade frame instead of slide |
| `.github/media/screens/*.gif`, `README.md` | regenerated gifs, wording |

---

### Task 1: `Scene::fade` and a lookup-table darken in the renderer (additive)

**Files:**
- Modify: `crates/render/src/frame.rs`
- Modify: `crates/core/src/scene.rs`
- Modify: `crates/render/src/renderer.rs`

**Interfaces:**
- Produces: `rackscreen_render::frame::darken(px: &mut Pixmap, k: f32)` (same truncation as today's `runloop::darken`: `(v as f32 * k) as u8`, `k` clamped to 0..1); `Scene { …, pub fade: f32 }` with `Scene::new()` setting `fade: 1.0`; `Renderer::render` darkens the result by `scene.fade` when `fade < 1.0`.

- [ ] **Step 1: Failing tests**

In `crates/render/src/frame.rs` tests:

```rust
    #[test]
    fn darken_scales_channels_and_keeps_alpha() {
        let mut p = Pixmap::new(2, 1).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        darken(&mut p, 0.5);
        assert_eq!(&p.data()[..4], &[127, 127, 127, 255]);
        darken(&mut p, 0.0);
        assert_eq!(&p.data()[..4], &[0, 0, 0, 255]);
        let mut q = noise(8, 3);
        let before = q.data().to_vec();
        darken(&mut q, 1.0);
        assert_eq!(q.data(), &before[..], "k = 1 is a no-op");
    }
```

In `crates/render/src/renderer.rs` tests:

```rust
    #[test]
    fn scene_fade_darkens_the_whole_frame() {
        let mut r = Renderer::new().unwrap();
        let mut s = based_scene(50.0);
        let mut full = new_pixmap();
        r.render(&s, &mut full);
        s.fade = 0.5;
        let mut half = new_pixmap();
        r.render(&s, &mut half);
        for (a, b) in full.data().chunks_exact(4).zip(half.data().chunks_exact(4)) {
            for c in 0..3 {
                assert_eq!(b[c], (a[c] as f32 * 0.5) as u8);
            }
        }
        s.fade = 0.0;
        r.render(&s, &mut half);
        assert!(half.data().chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
        // the cached base raster was not darkened
        s.fade = 1.0;
        let mut again = new_pixmap();
        r.render(&s, &mut again);
        assert_eq!(again.data(), full.data());
    }
```

Run: `cargo test -p rackscreen-render --lib -q`
Expected: compile errors (`darken`, `fade`).

- [ ] **Step 2: Implement**

`crates/render/src/frame.rs`:

```rust
/// Multiply every colour channel by `k` (0 = black, 1 = unchanged); alpha stays.
pub fn darken(px: &mut Pixmap, k: f32) {
    if k >= 1.0 {
        return;
    }
    let lut = brightness_lut(k);
    for p in px.data_mut().chunks_exact_mut(4) {
        p[0] = lut[p[0] as usize];
        p[1] = lut[p[1] as usize];
        p[2] = lut[p[2] as usize];
    }
}
```

`crates/core/src/scene.rs`: add to `Scene`

```rust
    /// Brightness of the whole frame, 1.0 = normal; below 1 during a screen
    /// change (crossfade via black).
    pub fade: f32,
```

and `fade: 1.0` in `Scene::new()`.

`crates/render/src/renderer.rs`, at the end of the non-slide path of `render` (after the item loop):

```rust
        if scene.fade < 1.0 {
            crate::frame::darken(px, scene.fade);
        }
```

The base raster is stored before this, so the cache stays undarkened.

- [ ] **Step 3: Run and commit**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo fmt --check`
Expected: PASS, goldens unchanged.

```bash
git add crates/render/src/frame.rs crates/render/src/renderer.rs crates/core/src/scene.rs
git commit -m "feat(render,core): scene fade, darkened through a lookup table after drawing

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Crossfade replaces the slide

**Files:**
- Modify: `crates/core/src/screens.rs`
- Modify: `crates/core/src/scene.rs`
- Modify: `crates/core/src/scene_model.rs`
- Modify: `crates/core/src/p2/ui.rs`
- Modify: `crates/core/src/p2/screen.rs` (one assert)
- Modify: `crates/render/src/renderer.rs`
- Modify: `crates/render/tests/golden.rs`, `crates/render/tests/bench.rs`
- Modify: `crates/app/src/worker.rs`, `crates/app/src/runloop.rs`

**Interfaces:**
- Consumes: Task 1 `Scene::fade`, `frame::darken`.
- Produces: `screens::Half { Out, In }`; `screens::fade(t: f32) -> (Half, f32)`; `screens::TRANSITION_SECS = 0.4`; `ScreenUi::end_transition()` (renamed from `end_slide`). Removed: `scene::Slide`, `scene::Placement`, `Scene::slide`, `screens::slide`, `runloop::darken`, `Renderer`'s `scratch`, `round`, `draw_placed`.

- [ ] **Step 1: Failing tests for `fade`**

In `crates/core/src/screens.rs` tests:

```rust
    #[test]
    fn fade_goes_out_to_black_then_in() {
        assert_eq!(fade(0.0), (Half::Out, 1.0));
        assert_eq!(fade(0.5), (Half::In, 0.0));
        assert_eq!(fade(1.0), (Half::In, 1.0));
        let (h, b) = fade(0.25);
        assert_eq!(h, Half::Out);
        assert!(b > 0.0 && b < 1.0);
        // monotonic within each half, quantised to 1/64
        let mut last = 2.0;
        for i in 0..50 {
            let (h, b) = fade(i as f32 / 100.0);
            assert_eq!(h, Half::Out);
            assert!(b <= last);
            assert_eq!((b * 64.0).fract(), 0.0);
            last = b;
        }
        let mut last = -1.0;
        for i in 50..=100 {
            let (h, b) = fade(i as f32 / 100.0);
            assert_eq!(h, Half::In);
            assert!(b >= last);
            last = b;
        }
    }

    #[test]
    fn transition_lasts_four_tenths() {
        assert_eq!(TRANSITION_SECS, 0.4);
    }
```

Run: `cargo test -p rackscreen-core screens -q`
Expected: compile errors.

- [ ] **Step 2: Implement `fade`, remove `slide`**

`crates/core/src/screens.rs`: `pub const TRANSITION_SECS: Secs = 0.4;`; module doc "the crossfade transition"; replace `slide` and its `Placement` import with:

```rust
/// Which role a transition frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Out,
    In,
}

/// Crossfade via black at `t ∈ 0..1`: the outgoing role fades to black over
/// the first half, the incoming one fades in over the second. Brightness is
/// quantised to 1/64 so equal frames are skipped by the render workers.
pub fn fade(t: f32) -> (Half, f32) {
    let t = t.clamp(0.0, 1.0);
    let q = |b: f32| (b * 64.0).round() / 64.0;
    if t < 0.5 {
        (Half::Out, q(1.0 - Easing::InOutCubic.apply(2.0 * t)))
    } else {
        (Half::In, q(Easing::InOutCubic.apply(2.0 * t - 1.0)))
    }
}
```

Replace remaining "iris" wording in this file's doc comments with "transition".

- [ ] **Step 3: Model builds one half**

`crates/core/src/scene_model.rs`, in `scene`:

```rust
        if let Some(tr) = state.transition(now) {
            let (half, brightness) = crate::screens::fade(tr.t);
            let mut s = match half {
                crate::screens::Half::Out => self.scene_for_role(tr.from, now, ui.outgoing(tr.from)),
                crate::screens::Half::In => self.scene_for_role(tr.to, now, ui),
            };
            s.fade = brightness;
            return s;
        }
        ui.end_transition();
        self.scene_for_role(role, now, ui)
```

Remove the `Slide` import. In `crates/core/src/p2/ui.rs` rename `end_slide` → `end_transition` (and its test call) and change "slide" to "transition" in the doc comments of `outgoing`/`end_transition`.

Add a public accessor to `ScreenState` in `crates/core/src/screens.rs`:

```rust
    /// When the running transition began, if one is running.
    pub fn transition_started(&self) -> Option<Secs> {
        self.transition_started
    }
```

Add a test helper in `scene_model.rs` tests, then rewrite the two transition tests with it:

```rust
    /// Tick at 30 Hz until screen 0 starts a transition; returns its start time.
    fn tick_until_transition(m: &mut Model) -> Secs {
        for i in 0..=1000 {
            m.tick(i as f64 / 30.0);
            if let Some(t0) = m.screen(0).unwrap().transition_started() {
                return t0;
            }
        }
        panic!("no transition within 33 s");
    }

    #[test]
    fn cycling_screen_fades_between_roles() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        let t0 = tick_until_transition(&mut m);
        let mut ui = ScreenUi::new();
        let out = m.scene(0, t0 + 0.05, &mut ui);
        assert_eq!(title_of(&out).as_deref(), Some("CPU"));
        assert!(out.fade > 0.0 && out.fade < 1.0, "outgoing half fading");
        let inc = m.scene(0, t0 + 0.35, &mut ui);
        assert_eq!(title_of(&inc).as_deref(), Some("MEMORY"));
        assert!(inc.fade > 0.0 && inc.fade < 1.0, "incoming half fading in");
        m.tick(t0 + 0.5);
        assert_eq!(m.current_role(0), Role::Mem);
        let settled = m.scene(0, t0 + 0.6, &mut ui);
        assert_eq!(settled.fade, 1.0, "settled on mem");
        assert_eq!(ui.outgoing(Role::Cpu).role(), None, "the outgoing slot was dropped");
    }
```

`transition_into_a_cluster_role_with_api_down_shows_connecting`: after the same setup, `let t0 = tick_until_transition(&mut m);` and assert `connecting(&m.scene(0, t0 + 0.3, &mut ScreenUi::new()))` (incoming half, `t ≥ 0.5`).

`crates/core/src/p2/screen.rs`: `assert!(scene.slide.is_none());` → `assert_eq!(scene.fade, 1.0);`. Other `slide.is_none()` asserts in `scene_model.rs` → `fade == 1.0`.

- [ ] **Step 4: Remove the slide types and renderer path**

`crates/core/src/scene.rs`: delete `Placement`, `Slide`, the `slide` field and its initialiser.

`crates/render/src/renderer.rs`: delete `scratch`, `round`, `draw_placed`, the `if let Some(sl) = &scene.slide` branch and the imports only they used (`Placement`, `Mask`, `FillRule`, `PathBuilder`, `PixmapPaint`, `Transform`, `CX`, `CY`, `SIZE`, `new_pixmap` if unused). Replace the renderer test `slide_draws_each_half_moved_faded_and_round` with nothing (Task 1's fade test covers the new path).

`crates/render/tests/golden.rs` (hit-path test near "also with a slide frame in between"): replace the slide frame with a fade frame of the same role:

```rust
        let mut faded = scene.clone();
        faded.fade = 0.5;
        r.render(&faded, &mut px);
```

and keep the assertion that the next normal render equals a fresh render. Update its doc comment ("also with a fade frame in between").

`crates/render/tests/bench.rs`: replace the slide bench with a mid-fade frame:

```rust
    let mut s = m.scene_for_role(Role::Cpu, 0.0, &mut ui);
    s.fade = 0.5;
```

timed like the old slide case, printed as `fade`, and assert it costs under `2.0 * BUDGET_MS`.

- [ ] **Step 5: Night fade uses the render darken**

`crates/app/src/worker.rs`: `use rackscreen_render::frame::darken;` instead of `crate::runloop::darken`. `crates/app/src/runloop.rs`: delete `darken` and `darken_scales_channels` (moved to the render crate in Task 1).

- [ ] **Step 6: Exact golden check and full run**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo fmt --check`.

Steady-scene goldens must not change at all: write a throwaway check (not committed) that renders every golden case and compares bytes exactly with the stored PNGs, or reuse the exact comparison the speed-pipeline fix wave used if it is in `golden.rs`; record the result (n of n identical) in the report.

Run the bench: `cargo test --release -q -p rackscreen-render --test bench -- --ignored --nocapture`; the `fade` frame should be close to a steady changing frame plus one darken pass (well under the old 0.8 ms slide).

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat(core,render,app): crossfade via black replaces the vertical slide

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Gifs and docs

**Files:**
- Modify: `.github/media/screens/*.gif` (regenerated)
- Modify: `README.md`, `crates/app/examples/gifs.rs` (only if it mentions the slide)

- [ ] **Step 1: Regenerate the gifs**

Run: `cargo run --release -q -p rackscreen-app --example gifs -- .github/media/screens`
Expected: the cycling gif (`all.gif`) shows the fade between roles; per-role gifs change only where a transition appears in them. Check with `git status --short .github/media/screens` which files changed, and open `all.gif` frames around a transition (e.g. extract with `python3 -c` + Pillow if available, or note that it was not inspected).

- [ ] **Step 2: Docs**

`README.md`: wherever the transition is described (search "slide", "transition", "iris"), say the screens crossfade via black in 0.4 s. The performance paragraph's "a fully changing screen (a transition) runs at roughly 50 fps" stays true; keep it.

- [ ] **Step 3: Commit**

```bash
cargo test --workspace -q
git add .github/media/screens README.md crates/app/examples/gifs.rs
git commit -m "docs: gifs and README show the crossfade

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

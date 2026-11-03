# Speed Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 60 fps on any RackScreen panel whose content moves, all four panels at once, with less render CPU than today's 66 % of one core at 30 fps, and byte-identical goldens.

**Architecture:** Measure first (frame stats in the log, a pipeline bench). Then: diff and pack in source orientation with several dirty rects and packed RGB565 through the mailbox instead of cloned pixmaps; one persistent render worker thread per screen that skips unchanged scenes; a cached base layer (clear, ring track, title, card background) per screen; a 60 Hz deadline-paced loop with 50 MHz SPI as the default.

**Tech Stack:** Rust 2021 workspace, tiny-skia 0.12, fontdue, usvg, rppal (SPI), std threads and `mpsc::sync_channel`, tracing.

Spec: `docs/superpowers/specs/2026-11-03-speed-pipeline-design.md`.

Deliberate deviations from the spec: the base cache compares the base drawables by equality instead of a hash (exact, no hashing of `f32`s); the mailbox buffer pool is left out (one allocation of at most 115 KB per pushed frame is small next to the removed 230 KB clone and full orient copy; add a pool only if the Pi stats show `flush_ms` dominated by allocation).

## Global Constraints

- Goldens stay byte-identical: `cargo test -p rackscreen-render --test golden` must pass unchanged in every task (no `UPDATE_GOLDENS`, no re-captures).
- At most 4 dirty rects per frame; bands closer than 8 rows merge; more than 70 % of the frame covered → one full rect.
- `display.fps` default 60; `hz` default `50000000`.
- One screen transitions at a time stays (`one_at_a_time`); the slide transition itself is not changed here (crossfade is the next spec).
- No new crates. No custom rasteriser.
- Deploying to the Pi is push + tag + release workflow + `sudo rackscreen update`; never a local cross build. Pushing and tagging need Silke's go-ahead each time.
- Commit after every task; commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Workspace check before every commit: `cargo test --workspace -q` and `cargo clippy --workspace --all-targets -q -- -D warnings`.

## File Structure

| file | responsibility |
|---|---|
| `crates/app/src/stats.rs` (new) | `Window` (ms samples, avg, p95), `ScreenStats`, `FrameStats` (period, log lines) |
| `crates/app/src/worker.rs` (new) | `Job`, `Done`, `ScreenFrames` (skip, render, darken, diff, pack, push), `ScreenWorker` (thread per screen) |
| `crates/app/src/runloop.rs` | main loop: events, model, scene build, jobs to workers, stats, night, pacing |
| `crates/app/src/run.rs`, `panels.rs` | wiring: brightness to workers, `Gc9a01::open` without brightness |
| `crates/render/src/frame.rs` | `Rect` helpers, `dirty_rects`, `merge_rects`, `Orient::src_to_dst`/`map_rect`, `brightness_lut`, `pack_oriented` |
| `crates/render/src/renderer.rs` | base layer cache |
| `crates/render/tests/bench.rs` | pipeline bench (render + diff + pack) |
| `crates/display/src/lib.rs` | `PackedFrame`, `DisplayCmd::Frame(PackedFrame)`, `Mailbox::put_frame`, `PushStats` |
| `crates/display/src/gc9a01.rs`, `sim.rs` | push packed rects |
| `crates/setup/src/screens/calibrate.rs` | push packed full frames |
| `crates/core/src/scene.rs`, `p2/screen.rs`, `p2/ring.rs` | `Scene::base_len`, ring track/fill split, compose base-first order |
| `crates/app/src/config.rs`, `config.example.yaml`, `README.md`, `Cargo.toml` | defaults, docs, version 0.9.0 |

---

### Task 1: Measure: frame stats in the log and a pipeline bench

**Files:**
- Create: `crates/app/src/stats.rs`
- Modify: `crates/app/src/lib.rs` (add `pub mod stats;`)
- Modify: `crates/display/src/lib.rs` (`PushStats`, record in display thread)
- Modify: `crates/app/src/runloop.rs` (time build / render / flush, log every 60 s)
- Modify: `crates/render/tests/bench.rs` (new ignored test `pipeline_per_frame`)

**Interfaces:**
- Produces: `rackscreen_display::PushStats { frames: u32, total_ms: f64, max_ms: f64, bytes: u64 }`, `Mailbox::record_push(&self, ms: f64, bytes: u64)`, `Mailbox::take_push_stats(&self) -> PushStats`.
- Produces: `rackscreen_app::stats::{Window, ScreenStats, FrameStats}`; `FrameStats::new(n: usize, period: Secs, now: Secs)`, `FrameStats::screen(&mut self, i) -> &mut ScreenStats`, `FrameStats::due(&self, now) -> bool`, `FrameStats::line(&self, i: usize, push: PushStats, secs: f64) -> String`, `FrameStats::reset(&mut self, now)`. `ScreenStats { build: Window, render: Window, flush: Window, skipped: u32 }`.

- [ ] **Step 1: Write the failing tests for `stats.rs`**

Create `crates/app/src/stats.rs` with only the tests first:

```rust
//! Per-screen frame timings, logged once per period so the Pi shows where
//! the frame budget goes.

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_display::PushStats;

    #[test]
    fn window_avg_and_p95() {
        let mut w = Window::default();
        for i in 1..=100 {
            w.push(i as f32);
        }
        assert!((w.avg() - 50.5).abs() < 1e-3);
        assert_eq!(w.p95(), 95.0);
        w.clear();
        assert_eq!(w.avg(), 0.0);
        assert_eq!(w.p95(), 0.0);
    }

    #[test]
    fn due_after_period_and_reset() {
        let mut s = FrameStats::new(2, 60.0, 0.0);
        assert!(!s.due(59.9));
        assert!(s.due(60.0));
        s.screen(1).render.push(2.0);
        s.reset(60.0);
        assert!(!s.due(100.0));
        assert_eq!(s.screen(1).render.avg(), 0.0);
    }

    #[test]
    fn line_reports_fps_and_timings() {
        let mut s = FrameStats::new(1, 60.0, 0.0);
        s.screen(0).build.push(0.2);
        s.screen(0).render.push(1.5);
        s.screen(0).flush.push(0.5);
        s.screen(0).skipped = 3;
        let push = PushStats {
            frames: 1800,
            total_ms: 3600.0,
            max_ms: 9.0,
            bytes: 1800 * 1000,
        };
        assert_eq!(
            s.line(0, push, 60.0),
            "frame stats screen=0 fps=30.0 rendered=1 skipped=3 build_ms=0.20/0.20 \
             render_ms=1.50/1.50 flush_ms=0.50/0.50 push_ms=2.00/9.00 kb_per_frame=1.0"
        );
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-app stats -q`
Expected: compile errors (`Window`, `FrameStats`, `PushStats` not found).

- [ ] **Step 3: Add `PushStats` to the display crate**

In `crates/display/src/lib.rs`, add the type, a second shared field on `Mailbox`, and recording in the display thread:

```rust
/// Push timings recorded by the display thread; read and reset by the stats logger.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PushStats {
    pub frames: u32,
    pub total_ms: f64,
    pub max_ms: f64,
    pub bytes: u64,
}
```

Change `Mailbox` to:

```rust
#[derive(Clone)]
pub struct Mailbox {
    inner: Arc<(Mutex<Slot>, Condvar)>,
    stats: Arc<Mutex<PushStats>>,
}
```

`Mailbox::new` initialises `stats: Arc::new(Mutex::new(PushStats::default()))`. Add:

```rust
    pub fn record_push(&self, ms: f64, bytes: u64) {
        let mut s = self.stats.lock().unwrap();
        s.frames += 1;
        s.total_ms += ms;
        s.max_ms = s.max_ms.max(ms);
        s.bytes += bytes;
    }

    pub fn take_push_stats(&self) -> PushStats {
        std::mem::take(&mut *self.stats.lock().unwrap())
    }
```

In `spawn_display_thread`, time the frame push:

```rust
                DisplayCmd::Frame(px, rect) => {
                    let t0 = std::time::Instant::now();
                    let r = display.push(&px, rect);
                    mailbox.record_push(
                        t0.elapsed().as_secs_f64() * 1000.0,
                        rect.w as u64 * rect.h as u64 * 2,
                    );
                    r
                }
```

Add a test in the display crate's `tests` module:

```rust
    #[test]
    fn push_stats_accumulate_and_reset() {
        let mb = Mailbox::new();
        mb.record_push(2.0, 100);
        mb.record_push(4.0, 300);
        let s = mb.take_push_stats();
        assert_eq!((s.frames, s.total_ms, s.max_ms, s.bytes), (2, 6.0, 4.0, 400));
        assert_eq!(mb.take_push_stats(), PushStats::default());
    }
```

- [ ] **Step 4: Implement `stats.rs`**

Above the tests in `crates/app/src/stats.rs`:

```rust
use rackscreen_core::anim::Secs;
use rackscreen_display::PushStats;

/// Millisecond samples over one logging period.
#[derive(Clone, Debug, Default)]
pub struct Window {
    samples: Vec<f32>,
}

impl Window {
    pub fn push(&mut self, ms: f32) {
        self.samples.push(ms);
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    pub fn avg(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }
    /// Nearest-rank 95th percentile.
    pub fn p95(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut v = self.samples.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        let rank = ((0.95 * v.len() as f32).ceil() as usize).clamp(1, v.len());
        v[rank - 1]
    }
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScreenStats {
    pub build: Window,
    pub render: Window,
    pub flush: Window,
    pub skipped: u32,
}

pub struct FrameStats {
    screens: Vec<ScreenStats>,
    period: Secs,
    since: Secs,
}

impl FrameStats {
    pub fn new(n: usize, period: Secs, now: Secs) -> Self {
        Self {
            screens: vec![ScreenStats::default(); n],
            period,
            since: now,
        }
    }
    pub fn screen(&mut self, i: usize) -> &mut ScreenStats {
        &mut self.screens[i]
    }
    pub fn due(&self, now: Secs) -> bool {
        now - self.since >= self.period
    }
    /// One log line for screen `i`; `secs` is the length of the period.
    pub fn line(&self, i: usize, push: PushStats, secs: f64) -> String {
        let s = &self.screens[i];
        let push_avg = if push.frames > 0 {
            push.total_ms / push.frames as f64
        } else {
            0.0
        };
        let kb = if push.frames > 0 {
            push.bytes as f64 / push.frames as f64 / 1000.0
        } else {
            0.0
        };
        format!(
            "frame stats screen={i} fps={:.1} rendered={} skipped={} build_ms={:.2}/{:.2} \
             render_ms={:.2}/{:.2} flush_ms={:.2}/{:.2} push_ms={push_avg:.2}/{:.2} kb_per_frame={kb:.1}",
            push.frames as f64 / secs.max(1e-9),
            s.render.len(),
            s.skipped,
            s.build.avg(),
            s.build.p95(),
            s.render.avg(),
            s.render.p95(),
            s.flush.avg(),
            s.flush.p95(),
            push.max_ms,
        )
    }
    pub fn reset(&mut self, now: Secs) {
        for s in &mut self.screens {
            *s = ScreenStats::default();
        }
        self.since = now;
    }
}
```

Add `pub mod stats;` to `crates/app/src/lib.rs` (next to `pub mod runloop;`). If `rackscreen-app` does not already depend on `rackscreen-display` directly, it does through `runloop.rs` (`use rackscreen_display::{DisplayCmd, Mailbox}`), so no Cargo change is needed.

- [ ] **Step 5: Run the stats and display tests**

Run: `cargo test -p rackscreen-app stats -q && cargo test -p rackscreen-display -q`
Expected: PASS.

- [ ] **Step 6: Time each stage in the render loop and log**

In `crates/app/src/runloop.rs`, `RenderLoop::run`, create `let mut stats = crate::stats::FrameStats::new(self.screens.len(), 60.0, 0.0);` before the loop. Replace the per-screen block with:

```rust
            for (i, s) in self.screens.iter_mut().enumerate() {
                let t = Instant::now();
                let scene = model.scene(s.index, now, &mut s.ui);
                let t_build = t.elapsed();
                let t = Instant::now();
                renderer.render(&scene, &mut s.cur);
                if let Some(f) = fade {
                    darken(&mut s.cur, 1.0 - f);
                }
                let t_render = t.elapsed();
                let t = Instant::now();
                s.flush();
                let t_flush = t.elapsed();
                let st = stats.screen(i);
                st.build.push(t_build.as_secs_f32() * 1000.0);
                st.render.push(t_render.as_secs_f32() * 1000.0);
                st.flush.push(t_flush.as_secs_f32() * 1000.0);
            }
            if stats.due(now) {
                for (i, s) in self.screens.iter().enumerate() {
                    tracing::info!("{}", stats.line(i, s.mailbox.take_push_stats(), 60.0));
                }
                stats.reset(now);
            }
```

(`stats.line` uses the configured 60 s period; the first period starts at loop start, `now` = 0.)

- [ ] **Step 7: Add the pipeline bench**

Append to `crates/render/tests/bench.rs` (reuse the model setup of `every_role_under_a_millisecond`: move its event block into `fn bench_model() -> Model` and call it from both tests):

```rust
/// Render + orient + diff + pack per frame for a moving screen (cpu, the
/// hero rolls) and a slide frame: the numbers the Pi stats should match.
#[test]
#[ignore]
fn pipeline_per_frame() {
    use rackscreen_render::frame::{dirty_rect, pack_rgb565, Orient};
    let m = bench_model();
    let mut r = Renderer::new().unwrap();
    let orient = Orient::new(270, false);
    let mut cur = new_pixmap();
    let mut oriented = new_pixmap();
    let mut prev = new_pixmap();
    let mut ui = ScreenUi::new();
    let t0 = Instant::now();
    for i in 0..ITERATIONS {
        let now = i as f64 / 60.0;
        r.render(&m.scene_for_role(Role::Cpu, now, &mut ui), &mut cur);
        orient.apply(&cur, &mut oriented);
        if let Some(rect) = dirty_rect(&prev, &oriented) {
            let _ = pack_rgb565(&oriented, rect, 1.0);
        }
        std::mem::swap(&mut prev, &mut oriented);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
    println!("{:<12} {ms:.3} ms", "pipeline");
}
```

Run: `cargo test --release -q -p rackscreen-render --test bench -- --ignored --nocapture`
Expected: both tests print timings; record the `pipeline` number in the commit message body as the desktop baseline.

- [ ] **Step 8: Workspace check and commit**

```bash
cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/app/src/stats.rs crates/app/src/lib.rs crates/app/src/runloop.rs crates/display/src/lib.rs crates/render/tests/bench.rs
git commit -m "feat(app,display): frame stats per screen every 60 s; pipeline bench

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 9: Pi baseline (checkpoint, needs Silke)**

Ask Silke whether to release this as 0.8.2 for a Pi baseline (bump `version` in the root `Cargo.toml` to `0.8.2`, `cargo build -q` to refresh `Cargo.lock`, commit, push, tag `v0.8.2`, wait for the release workflow, `sudo rackscreen update` on the Pi). After 2 minutes, `journalctl -u rackscreen@silke --since -2min | grep "frame stats"` and paste the lines into `.superpowers/sdd/speed-baseline.md`. If Silke says no, skip; Task 7 measures before/after with the same lines.

---

### Task 2: Pure frame functions: several dirty rects, rect mapping, oriented RGB565 pack

**Files:**
- Modify: `crates/render/src/frame.rs`

**Interfaces:**
- Produces (all in `rackscreen_render::frame`):
  - `Rect::area(&self) -> u64`, `Rect::intersects(&self, o: Rect) -> bool`
  - `pub const MAX_RECTS: usize = 4;`
  - `pub fn merge_rects(rects: Vec<Rect>, w: u32, h: u32) -> Vec<Rect>`
  - `pub fn dirty_rects(prev: &Pixmap, next: &Pixmap) -> Vec<Rect>` (empty when identical)
  - `Orient::src_to_dst(&self, sx: u32, sy: u32) -> (u32, u32)`, `Orient::map_rect(&self, r: Rect) -> Rect`
  - `pub fn brightness_lut(brightness: f32) -> [u8; 256]`
  - `pub fn pack_oriented(src: &Pixmap, orient: &Orient, rects: &[Rect], lut: &[u8; 256]) -> Vec<u8>` (rects in panel coordinates, bytes concatenated in rect order)
- Kept unchanged as reference: `Orient::apply`, `dirty_rect`, `pack_rgb565`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `crates/render/src/frame.rs`:

```rust
    fn noise(size: u32, seed: u32) -> Pixmap {
        let mut p = Pixmap::new(size, size).unwrap();
        let mut s = seed;
        for b in p.data_mut().chunks_exact_mut(4) {
            for c in &mut b[..3] {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                *c = (s >> 24) as u8;
            }
            b[3] = 255;
        }
        p
    }

    const ORIENTS: [(u32, bool); 8] = [
        (0, false), (90, false), (180, false), (270, false),
        (0, true), (90, true), (180, true), (270, true),
    ];

    #[test]
    fn src_to_dst_inverts_the_map() {
        for (rot, flip) in ORIENTS {
            let o = Orient::new_sized(5, rot, flip);
            for dy in 0..5 {
                for dx in 0..5 {
                    let src = o.map[(dy * 5 + dx) as usize];
                    assert_eq!(o.src_to_dst(src % 5, src / 5), (dx, dy), "{rot} {flip}");
                }
            }
        }
    }

    #[test]
    fn map_rect_covers_the_same_pixels() {
        for (rot, flip) in ORIENTS {
            let o = Orient::new_sized(6, rot, flip);
            let r = Rect { x: 1, y: 2, w: 3, h: 2 };
            let m = o.map_rect(r);
            assert_eq!(m.area(), r.area());
            for sy in r.y..r.y + r.h {
                for sx in r.x..r.x + r.w {
                    let (dx, dy) = o.src_to_dst(sx, sy);
                    assert!(dx >= m.x && dx < m.x + m.w && dy >= m.y && dy < m.y + m.h);
                }
            }
        }
    }

    #[test]
    fn pack_oriented_equals_orient_then_pack() {
        let src = noise(240, 7);
        for (rot, flip) in ORIENTS {
            let o = Orient::new(rot, flip);
            let mut oriented = Pixmap::new(240, 240).unwrap();
            o.apply(&src, &mut oriented);
            for k in [1.0f32, 0.6] {
                let panel = o.map_rect(Rect { x: 10, y: 30, w: 50, h: 20 });
                let old = pack_rgb565(&oriented, panel, k);
                let new = pack_oriented(&src, &o, &[panel], &brightness_lut(k));
                assert_eq!(old, new, "{rot} {flip} {k}");
            }
        }
    }

    #[test]
    fn dirty_rects_split_far_apart_changes() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        px(&mut b, 100, 5, (255, 255, 255));
        px(&mut b, 120, 200, (255, 255, 255));
        assert_eq!(
            dirty_rects(&a, &b),
            vec![Rect { x: 100, y: 5, w: 1, h: 1 }, Rect { x: 120, y: 200, w: 1, h: 1 }]
        );
        assert!(dirty_rects(&a, &a.clone()).is_empty());
    }

    #[test]
    fn dirty_rects_merge_close_rows() {
        let a = new_pixmap();
        let mut b = new_pixmap();
        px(&mut b, 50, 10, (255, 0, 0));
        px(&mut b, 60, 18, (255, 0, 0)); // 8 rows below: same band
        assert_eq!(dirty_rects(&a, &b), vec![Rect { x: 50, y: 10, w: 11, h: 9 }]);
    }

    #[test]
    fn merge_rects_caps_at_four_and_falls_back_to_full() {
        let row = |y| Rect { x: 0, y, w: 10, h: 1 };
        let out = merge_rects((0..6).map(|i| row(i * 40)).collect(), 240, 240);
        assert_eq!(out.len(), MAX_RECTS);
        let total: u64 = out.iter().map(Rect::area).sum();
        assert!(total >= 60);
        let big = vec![
            Rect { x: 0, y: 0, w: 240, h: 100 },
            Rect { x: 0, y: 140, w: 240, h: 100 },
        ];
        assert_eq!(merge_rects(big, 240, 240), vec![Rect::full()]);
        let overlap = vec![Rect { x: 0, y: 0, w: 10, h: 10 }, Rect { x: 5, y: 5, w: 10, h: 10 }];
        assert_eq!(merge_rects(overlap, 240, 240), vec![Rect { x: 0, y: 0, w: 15, h: 15 }]);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-render --lib frame -q`
Expected: compile errors for the new functions.

- [ ] **Step 3: Implement**

In `crates/render/src/frame.rs`:

Add to `impl Rect`:

```rust
    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }
    pub fn intersects(&self, o: Rect) -> bool {
        !self.is_empty()
            && !o.is_empty()
            && self.x < o.x + o.w
            && o.x < self.x + self.w
            && self.y < o.y + o.h
            && o.y < self.y + self.h
    }
```

Store the orientation parameters in `Orient` (add fields `rotate: u32, hflip: bool`, set in `new_sized` as `rotate_cw % 360` and `hflip`), then add:

```rust
    /// Where source pixel (sx, sy) lands on the panel.
    pub fn src_to_dst(&self, sx: u32, sy: u32) -> (u32, u32) {
        let n = self.size;
        let (rx, ry) = match self.rotate {
            0 => (sx, sy),
            90 => (n - 1 - sy, sx),
            180 => (n - 1 - sx, n - 1 - sy),
            270 => (sy, n - 1 - sx),
            _ => unreachable!("checked in new_sized"),
        };
        if self.hflip {
            (n - 1 - rx, ry)
        } else {
            (rx, ry)
        }
    }

    /// A source-space rectangle in panel coordinates.
    pub fn map_rect(&self, r: Rect) -> Rect {
        if r.is_empty() {
            return r;
        }
        let (ax, ay) = self.src_to_dst(r.x, r.y);
        let (bx, by) = self.src_to_dst(r.x + r.w - 1, r.y + r.h - 1);
        Rect {
            x: ax.min(bx),
            y: ay.min(by),
            w: ax.abs_diff(bx) + 1,
            h: ay.abs_diff(by) + 1,
        }
    }
```

Free functions:

```rust
pub const MAX_RECTS: usize = 4;
/// Changed rows closer than this many rows end up in one rect.
const BAND_GAP_ROWS: u32 = 8;
/// Above this share of the frame, one full window write is cheaper.
const FULL_FRACTION: f64 = 0.70;

fn merge_overlaps(rects: &mut Vec<Rect>) {
    'again: loop {
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if rects[i].intersects(rects[j]) {
                    rects[i] = rects[i].union(rects[j]);
                    rects.remove(j);
                    continue 'again;
                }
            }
        }
        return;
    }
}

/// Overlaps merged, at most `MAX_RECTS` (closest pairs merged first: least
/// added area), one full rect when they cover more than 70 % of `w`×`h`.
pub fn merge_rects(mut rects: Vec<Rect>, w: u32, h: u32) -> Vec<Rect> {
    rects.retain(|r| !r.is_empty());
    merge_overlaps(&mut rects);
    while rects.len() > MAX_RECTS {
        let mut best = (0, 1, u64::MAX);
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let extra = rects[i]
                    .union(rects[j])
                    .area()
                    .saturating_sub(rects[i].area() + rects[j].area());
                if extra < best.2 {
                    best = (i, j, extra);
                }
            }
        }
        rects[best.0] = rects[best.0].union(rects[best.1]);
        rects.remove(best.1);
        merge_overlaps(&mut rects);
    }
    let total: u64 = rects.iter().map(Rect::area).sum();
    if total as f64 > FULL_FRACTION * (w as u64 * h as u64) as f64 {
        return vec![Rect { x: 0, y: 0, w, h }];
    }
    rects
}

/// Changed regions between two frames, grouped into row bands.
pub fn dirty_rects(prev: &Pixmap, next: &Pixmap) -> Vec<Rect> {
    let w = prev.width() as usize;
    let h = prev.height() as usize;
    let (a, b) = (prev.data(), next.data());
    let mut bands: Vec<Rect> = Vec::new();
    for y in 0..h {
        let ra = &a[y * w * 4..(y + 1) * w * 4];
        let rb = &b[y * w * 4..(y + 1) * w * 4];
        if ra == rb {
            continue;
        }
        let differs = |x: usize| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4];
        let x0 = (0..w).find(|&x| differs(x)).unwrap() as u32;
        let x1 = (0..w).rev().find(|&x| differs(x)).unwrap() as u32;
        let row = Rect { x: x0, y: y as u32, w: x1 - x0 + 1, h: 1 };
        match bands.last_mut() {
            Some(last) if (y as u32) <= last.y + last.h - 1 + BAND_GAP_ROWS => *last = last.union(row),
            _ => bands.push(row),
        }
    }
    merge_rects(bands, w as u32, h as u32)
}

/// `out[v] = v × brightness`, truncated like `pack_rgb565`.
pub fn brightness_lut(brightness: f32) -> [u8; 256] {
    let k = brightness.clamp(0.0, 1.0);
    std::array::from_fn(|v| (v as f32 * k) as u8)
}

/// Big-endian RGB565 of panel-space `rects`, read from the unrotated `src`
/// through `orient`; the bytes of each rect follow the previous one.
pub fn pack_oriented(src: &Pixmap, orient: &Orient, rects: &[Rect], lut: &[u8; 256]) -> Vec<u8> {
    let n = orient.size as usize;
    let data = src.data();
    let cap: usize = rects.iter().map(|r| r.area() as usize * 2).sum();
    let mut out = Vec::with_capacity(cap);
    for r in rects {
        for y in r.y..r.y + r.h {
            let row = &orient.map[y as usize * n + r.x as usize..y as usize * n + (r.x + r.w) as usize];
            for &m in row {
                let i = m as usize * 4;
                let red = lut[data[i] as usize] as u16;
                let green = lut[data[i + 1] as usize] as u16;
                let blue = lut[data[i + 2] as usize] as u16;
                let v = ((red & 0xF8) << 8) | ((green & 0xFC) << 3) | (blue >> 3);
                out.extend_from_slice(&v.to_be_bytes());
            }
        }
    }
    out
}
```

In `new_sized`, the existing `panic!` for other rotations keeps `unreachable!` in `src_to_dst` honest.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-render --lib frame -q`
Expected: PASS (all old and new frame tests).

- [ ] **Step 5: Commit**

```bash
cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/render/src/frame.rs
git commit -m "feat(render): several dirty rects, rect mapping and RGB565 packing through the orientation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Packed frames through the mailbox; diff in source orientation

**Files:**
- Modify: `crates/display/src/lib.rs` (`PackedFrame`, `DisplayCmd::Frame`, `Display::push`, `Mailbox::put_frame`, tests)
- Modify: `crates/display/src/gc9a01.rs` (push packed rects; `open` without brightness)
- Modify: `crates/display/src/sim.rs` (blit RGB565)
- Modify: `crates/app/src/panels.rs` (`Gc9a01::open(pins, chunk)`)
- Modify: `crates/app/src/runloop.rs` (`ScreenSlot`: no `oriented`, source-space diff, `put_frame`, brightness)
- Modify: `crates/app/src/run.rs` (pass `cfg.display.brightness` to `ScreenSlot::new`)
- Modify: `crates/setup/src/screens/calibrate.rs` (push packed full frames)

**Interfaces:**
- Consumes: Task 2 `merge_rects`, `dirty_rects`, `Orient::map_rect`, `brightness_lut`, `pack_oriented`, `Rect::area`.
- Produces: `rackscreen_display::PackedFrame { pub rects: Vec<Rect>, pub bytes: Vec<u8> }` with `PackedFrame::slices(&self) -> impl Iterator<Item = (Rect, &[u8])>`; `DisplayCmd::Frame(PackedFrame)`; `trait Display { fn push(&mut self, frame: &PackedFrame) -> Result<()>; … }`; `Mailbox::put_frame(&self, rects: &[Rect], pack: impl FnOnce(&[Rect]) -> Vec<u8>)`; `ScreenSlot::new(index, role, orient, mailbox, brightness: f32)`.

- [ ] **Step 1: Rewrite the mailbox tests for packed frames (failing)**

Replace the frame helpers and frame tests in `crates/display/src/lib.rs` `tests` with:

```rust
    fn bytes_for(rects: &[Rect], tag: u8) -> Vec<u8> {
        vec![tag; rects.iter().map(|r| r.area() as usize * 2).sum()]
    }

    #[test]
    fn put_frame_packs_the_given_rects() {
        let mb = Mailbox::new();
        let r = Rect { x: 1, y: 2, w: 3, h: 4 };
        mb.put_frame(&[r], |rs| bytes_for(rs, 7));
        match mb.take() {
            DisplayCmd::Frame(f) => {
                assert_eq!(f.rects, vec![r]);
                assert_eq!(f.bytes.len(), 24);
            }
            _ => panic!(),
        }
        assert!(mb.try_take().is_none());
    }

    #[test]
    fn untaken_frame_rects_join_the_next_pack() {
        let mb = Mailbox::new();
        let a = Rect { x: 0, y: 0, w: 2, h: 2 };
        let b = Rect { x: 100, y: 100, w: 2, h: 2 };
        mb.put_frame(&[a], |rs| bytes_for(rs, 1));
        let mut seen = Vec::new();
        mb.put_frame(&[b], |rs| {
            seen = rs.to_vec();
            bytes_for(rs, 2)
        });
        assert_eq!(seen, vec![b, a], "pending rects are repacked from the newest pixels");
        match mb.take() {
            DisplayCmd::Frame(f) => {
                assert_eq!(f.rects, vec![b, a]);
                assert!(f.bytes.iter().all(|&x| x == 2));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn slices_split_bytes_per_rect() {
        let rects = vec![Rect { x: 0, y: 0, w: 1, h: 1 }, Rect { x: 5, y: 5, w: 2, h: 1 }];
        let f = PackedFrame { rects: rects.clone(), bytes: vec![1, 1, 2, 2, 2, 2] };
        let s: Vec<(Rect, Vec<u8>)> = f.slices().map(|(r, b)| (r, b.to_vec())).collect();
        assert_eq!(s, vec![(rects[0], vec![1, 1]), (rects[1], vec![2, 2, 2, 2])]);
    }

    #[test]
    fn frame_is_delivered_after_pending_control() {
        let mb = Mailbox::new();
        mb.put(DisplayCmd::Sleep);
        mb.put_frame(&[Rect::full()], |rs| bytes_for(rs, 1));
        assert!(matches!(mb.take(), DisplayCmd::Sleep));
        assert!(matches!(mb.take(), DisplayCmd::Frame(..)));
        assert!(mb.try_take().is_none());
    }
```

Keep the existing control-only tests (`control_is_not_replaced_by_frame` becomes the same shape as `frame_is_delivered_after_pending_control`; delete the duplicate) and `push_stats_accumulate_and_reset`.

Run: `cargo test -p rackscreen-display -q`
Expected: compile errors (`PackedFrame`, `put_frame`).

- [ ] **Step 2: Implement the packed mailbox**

In `crates/display/src/lib.rs`:

```rust
use rackscreen_render::frame::merge_rects;
use rackscreen_core::theme::layout::SIZE;

/// Big-endian RGB565 for one or more panel rectangles; the bytes of each
/// rect follow the previous one.
#[derive(Clone, Debug, PartialEq)]
pub struct PackedFrame {
    pub rects: Vec<Rect>,
    pub bytes: Vec<u8>,
}

impl PackedFrame {
    pub fn slices(&self) -> impl Iterator<Item = (Rect, &[u8])> {
        let mut off = 0;
        self.rects.iter().map(move |r| {
            let n = r.area() as usize * 2;
            let s = &self.bytes[off..off + n];
            off += n;
            (*r, s)
        })
    }
}

#[derive(Debug)]
pub enum DisplayCmd {
    Frame(PackedFrame),
    Sleep,
    Wake,
    Quit,
}

pub trait Display: Send {
    fn push(&mut self, frame: &PackedFrame) -> Result<()>;
    fn sleep(&mut self) -> Result<()>;
    fn wake(&mut self) -> Result<()>;
}
```

`Slot.frame` becomes `Option<PackedFrame>`. `Mailbox::put` keeps handling controls; for `DisplayCmd::Frame(f)` it replaces the pending frame (`slot.frame = Some(f)`), documented as "use `put_frame` for rendered frames". Add:

```rust
    /// Queue a rendered frame. `rects` changed since the previous frame. When
    /// the display thread has not taken the previous frame yet, its rects join
    /// this one so no region is lost, and `pack` is called once for the merged
    /// set: it must read the newest pixels. Runs under the mailbox lock.
    pub fn put_frame(&self, rects: &[Rect], pack: impl FnOnce(&[Rect]) -> Vec<u8>) {
        let (lock, cv) = &*self.inner;
        let mut slot = lock.lock().unwrap();
        let mut all = rects.to_vec();
        if let Some(old) = slot.frame.take() {
            all.extend(old.rects);
        }
        let rects = merge_rects(all, SIZE, SIZE);
        let bytes = pack(&rects);
        slot.frame = Some(PackedFrame { rects, bytes });
        cv.notify_one();
    }
```

`pop` returns `slot.frame.take().map(DisplayCmd::Frame)`. In `spawn_display_thread`:

```rust
                DisplayCmd::Frame(f) => {
                    let t0 = std::time::Instant::now();
                    let r = display.push(&f);
                    mailbox.record_push(t0.elapsed().as_secs_f64() * 1000.0, f.bytes.len() as u64);
                    r
                }
```

If `rackscreen-display` lacks a `rackscreen-core` dependency, use `rackscreen_render::frame::Rect::full()` dimensions instead of `SIZE` (`let full = Rect::full(); merge_rects(all, full.w, full.h)`), which avoids a new dependency.

- [ ] **Step 3: GC9A01 pushes each rect; brightness moves out**

In `crates/display/src/gc9a01.rs`: remove the `brightness` field and parameter (`pub fn open(pins: Pins, chunk: usize) -> Result<Self>`), drop the `pack_rgb565` import, and:

```rust
    fn push(&mut self, frame: &PackedFrame) -> Result<()> {
        for (r, bytes) in frame.slices() {
            if r.is_empty() {
                continue;
            }
            self.set_window(r)?;
            self.data(bytes)?;
        }
        Ok(())
    }
```

In `crates/app/src/panels.rs`, call `Gc9a01::open(pins, cfg.display.spi_chunk)`.

- [ ] **Step 4: Simulator blits RGB565**

In `crates/display/src/sim.rs` replace `blit` and the `Display` impl:

```rust
    fn blit(&self, frame: &PackedFrame) {
        let c = PANEL as f32 / 2.0 - 0.5;
        let mut buf = self.buf.lock().unwrap();
        for (r, bytes) in frame.slices() {
            for (k, px) in bytes.chunks_exact(2).enumerate() {
                let x = r.x + k as u32 % r.w;
                let y = r.y + k as u32 / r.w;
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                if d > 120.0 {
                    continue;
                }
                let v = u16::from_be_bytes([px[0], px[1]]) as u32;
                let (r5, g6, b5) = ((v >> 11) & 0x1F, (v >> 5) & 0x3F, v & 0x1F);
                let rgb = ((r5 << 3) | (r5 >> 2)) << 16 | ((g6 << 2) | (g6 >> 4)) << 8 | ((b5 << 3) | (b5 >> 2));
                buf[(self.oy + y as usize) * self.stride + self.ox + x as usize] = rgb;
            }
        }
    }
```

`push(&mut self, frame: &PackedFrame)` calls `self.blit(frame)` unless asleep; `sleep` blits `PackedFrame { rects: vec![Rect::full()], bytes: vec![0; 240 * 240 * 2] }`. Update `panel_masks_to_circle_and_sleeps` to push a white packed frame (`bytes: vec![0xFF; 240 * 240 * 2]`) and expect `0xFFFFFF` inside the circle.

- [ ] **Step 5: Runloop diffs in source orientation (failing test first)**

Replace `first_flush_is_full_then_dirty_only` in `crates/app/src/runloop.rs` with:

```rust
    #[test]
    fn first_flush_is_full_then_dirty_only_in_panel_space() {
        let mb = Mailbox::new();
        let mut slot = ScreenSlot::new(0, Role::Cpu, Orient::new(90, false), mb.clone(), 1.0);
        assert_eq!(slot.flush(), Some(vec![Rect::full()]));
        assert!(matches!(mb.take(), DisplayCmd::Frame(f) if f.rects == vec![Rect::full()]));
        assert_eq!(slot.flush(), None, "identical frame pushes nothing");
        // source pixel (10, 20) lands at panel (219, 10) for a 90° turn
        slot.cur.data_mut()[(20 * 240 + 10) * 4] = 200;
        assert_eq!(slot.flush(), Some(vec![Rect { x: 219, y: 10, w: 1, h: 1 }]));
    }
```

Note: after the first flush, `prev` holds the first frame and `cur` the buffer swapped out, both black, so the second flush finds no change. The third flush compares the edited `cur` with `prev`.

Run: `cargo test -p rackscreen-app runloop -q`
Expected: FAIL to compile (`ScreenSlot::new` arity, `flush` return type).

- [ ] **Step 6: Implement the new `ScreenSlot`**

```rust
pub struct ScreenSlot {
    pub index: usize,
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    pub ui: ScreenUi,
    lut: [u8; 256],
    cur: Pixmap,
    prev: Pixmap,
    first: bool,
}

impl ScreenSlot {
    pub fn new(index: usize, role: Role, orient: Orient, mailbox: Mailbox, brightness: f32) -> Self {
        Self {
            index,
            role,
            orient,
            mailbox,
            ui: ScreenUi::new(),
            lut: brightness_lut(brightness),
            cur: new_pixmap(),
            prev: new_pixmap(),
            first: true,
        }
    }

    /// Diff the fresh frame against the last pushed one (both unrotated), pack
    /// the changed panel rects, push. Returns the panel rects pushed.
    fn flush(&mut self) -> Option<Vec<Rect>> {
        let src = if self.first {
            vec![Rect::full()]
        } else {
            dirty_rects(&self.prev, &self.cur)
        };
        self.first = false;
        if src.is_empty() {
            return None;
        }
        let panel: Vec<Rect> = src.iter().map(|r| self.orient.map_rect(*r)).collect();
        let (cur, orient, lut) = (&self.cur, &self.orient, &self.lut);
        self.mailbox.put_frame(&panel, |rs| pack_oriented(cur, orient, rs, lut));
        std::mem::swap(&mut self.prev, &mut self.cur);
        Some(panel)
    }
}
```

Imports: `use rackscreen_render::frame::{brightness_lut, dirty_rects, new_pixmap, pack_oriented, Orient, Rect};` (drop `dirty_rect`). Every render starts with `Clear` (or the slide's black fill), so rendering into the swapped-out `cur` overwrites it fully.

In `crates/app/src/run.rs`, pass `cfg.display.brightness` as the fifth argument of `ScreenSlot::new`.

- [ ] **Step 7: Calibrate pushes packed frames**

In `crates/setup/src/screens/calibrate.rs` `push`: remove `shown` (field, initialisation, uses) and replace the orient-and-put lines with:

```rust
        let orient = Orient::new(o.rotate, o.hflip);
        let lut = brightness_lut(self.cfg.display.brightness);
        let frame = &self.frames[i];
        h.mailbox
            .put_frame(&[Rect::full()], |rs| pack_oriented(frame, &orient, rs, &lut));
```

Imports: `use rackscreen_render::frame::{brightness_lut, new_pixmap, pack_oriented, Orient, Rect};`. If the borrow of `self.frames` conflicts with `r: &mut self.renderer`, render first, then take `&self.frames[i]` after the `r.render` call (the `let (Some(p), Some(r))` borrow ends after rendering; re-borrow `self.panels` for `h`).

- [ ] **Step 8: Run everything**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings`
Expected: PASS. Goldens unchanged (render path untouched).

Smoke test in the simulator: `cargo run -q -- run --sim --source fake` for 20 s; all four panels animate, slide transitions complete, no smear. Close with Escape.

- [ ] **Step 9: Commit**

```bash
git add crates/display crates/app/src/runloop.rs crates/app/src/run.rs crates/app/src/panels.rs crates/setup/src/screens/calibrate.rs
git commit -m "perf(display,app): packed RGB565 rects through the mailbox, diff in source orientation, no frame clone

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: One render worker per screen; skip unchanged scenes

**Files:**
- Create: `crates/app/src/worker.rs`
- Modify: `crates/app/src/lib.rs` (`pub mod worker;`)
- Modify: `crates/app/src/runloop.rs` (`ScreenSlot` slims down; loop sends jobs, waits, collects stats)
- Modify: `crates/app/src/run.rs` (unchanged call shape; `ScreenSlot::new` keeps 5 args)

**Interfaces:**
- Consumes: Task 3 `ScreenSlot` fields (`orient`, `mailbox`, brightness) and `flush` body; Task 1 `FrameStats`.
- Produces:
  - `worker::Job { pub scene: Scene, pub fade: Option<f32>, pub force_full: bool }`
  - `worker::Done { pub render_ms: f32, pub flush_ms: f32, pub skipped: bool }`
  - `worker::ScreenFrames::new(orient: Orient, mailbox: Mailbox, brightness: f32)`, `ScreenFrames::run(&mut self, r: &mut Renderer, job: Job) -> Done`, `ScreenFrames::flush(&mut self) -> Option<Vec<Rect>>` (moved from `ScreenSlot`)
  - `worker::ScreenWorker::spawn(index: usize, orient: Orient, mailbox: Mailbox, brightness: f32) -> anyhow::Result<ScreenWorker>`, `send(&self, job: Job) -> anyhow::Result<()>`, `wait(&self) -> anyhow::Result<Done>`
  - `runloop::ScreenSlot { pub index, pub role, pub orient, pub mailbox, pub ui, pub brightness: f32 }`, `ScreenSlot::new(index, role, orient, mailbox, brightness)`
  - `runloop::darken` stays public (used by the worker).

- [ ] **Step 1: Write the failing worker tests**

Create `crates/app/src/worker.rs` with tests first:

```rust
//! One render thread per screen: skip unchanged scenes, render, darken for
//! the night fade, diff, pack and hand the frame to the display thread.

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::scene::{Drawable, Scene};
    use rackscreen_core::theme::Color;
    use rackscreen_display::DisplayCmd;
    use rackscreen_render::frame::Rect;

    fn scene(x: f32) -> Scene {
        let mut s = Scene::new();
        s.push(Drawable::RoundRect { x, y: 100.0, w: 10.0, h: 10.0, r: 2.0, color: Color::hex(0xFFFFFF), alpha: 1.0 });
        s
    }

    fn job(s: Scene) -> Job {
        Job { scene: s, fade: None, force_full: false }
    }

    #[test]
    fn same_scene_twice_is_skipped() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        assert!(!f.run(&mut r, job(scene(50.0))).skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == vec![Rect::full()]));
        assert!(f.run(&mut r, job(scene(50.0))).skipped);
        assert!(mb.try_take().is_none());
        assert!(!f.run(&mut r, job(scene(60.0))).skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects.len() == 1 && fr.rects[0] != Rect::full()));
    }

    #[test]
    fn force_full_and_fade_change_are_not_skipped() {
        let mb = Mailbox::new();
        let mut f = ScreenFrames::new(Orient::identity(), mb.clone(), 1.0);
        let mut r = Renderer::new().unwrap();
        f.run(&mut r, job(scene(50.0)));
        let _ = mb.take();
        let d = f.run(&mut r, Job { scene: scene(50.0), fade: None, force_full: true });
        assert!(!d.skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(fr) if fr.rects == vec![Rect::full()]));
        let d = f.run(&mut r, Job { scene: scene(50.0), fade: Some(0.5), force_full: false });
        assert!(!d.skipped);
    }

    #[test]
    fn worker_thread_renders_and_reports() {
        let mb = Mailbox::new();
        let w = ScreenWorker::spawn(0, Orient::identity(), mb.clone(), 1.0).unwrap();
        w.send(job(scene(50.0))).unwrap();
        let d = w.wait().unwrap();
        assert!(!d.skipped);
        assert!(matches!(mb.take(), DisplayCmd::Frame(_)));
    }
}
```

Run: `cargo test -p rackscreen-app worker -q`
Expected: compile errors.

- [ ] **Step 2: Implement `worker.rs`**

Above the tests:

```rust
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{anyhow, Result};
use rackscreen_core::scene::Scene;
use rackscreen_display::Mailbox;
use rackscreen_render::frame::{brightness_lut, dirty_rects, new_pixmap, pack_oriented, Orient, Rect};
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

use crate::runloop::darken;

pub struct Job {
    pub scene: Scene,
    /// Night fade progress, `Some(1.0)` = black.
    pub fade: Option<f32>,
    /// Push the whole frame (after waking the panels).
    pub force_full: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Done {
    pub render_ms: f32,
    pub flush_ms: f32,
    pub skipped: bool,
}

/// Frame buffers and the last scene of one screen.
pub struct ScreenFrames {
    orient: Orient,
    mailbox: Mailbox,
    lut: [u8; 256],
    cur: Pixmap,
    prev: Pixmap,
    first: bool,
    last: Option<(Scene, Option<f32>)>,
}

impl ScreenFrames {
    pub fn new(orient: Orient, mailbox: Mailbox, brightness: f32) -> Self {
        Self {
            orient,
            mailbox,
            lut: brightness_lut(brightness),
            cur: new_pixmap(),
            prev: new_pixmap(),
            first: true,
            last: None,
        }
    }

    pub fn run(&mut self, r: &mut Renderer, job: Job) -> Done {
        if job.force_full {
            self.first = true;
        }
        let unchanged = !self.first
            && matches!(&self.last, Some((s, f)) if *f == job.fade && *s == job.scene);
        if unchanged {
            return Done { skipped: true, ..Done::default() };
        }
        let t = Instant::now();
        r.render(&job.scene, &mut self.cur);
        if let Some(f) = job.fade {
            darken(&mut self.cur, 1.0 - f);
        }
        let render_ms = t.elapsed().as_secs_f32() * 1000.0;
        let t = Instant::now();
        self.flush();
        let flush_ms = t.elapsed().as_secs_f32() * 1000.0;
        self.last = Some((job.scene, job.fade));
        Done { render_ms, flush_ms, skipped: false }
    }

    /// Body moved unchanged from Task 3's `ScreenSlot::flush`.
    pub fn flush(&mut self) -> Option<Vec<Rect>> {
        let src = if self.first {
            vec![Rect::full()]
        } else {
            dirty_rects(&self.prev, &self.cur)
        };
        self.first = false;
        if src.is_empty() {
            return None;
        }
        let panel: Vec<Rect> = src.iter().map(|r| self.orient.map_rect(*r)).collect();
        let (cur, orient, lut) = (&self.cur, &self.orient, &self.lut);
        self.mailbox.put_frame(&panel, |rs| pack_oriented(cur, orient, rs, lut));
        std::mem::swap(&mut self.prev, &mut self.cur);
        Some(panel)
    }
}

/// A persistent render thread for one screen. The renderer is built on the
/// thread (its caches need not be `Send`).
pub struct ScreenWorker {
    jobs: Option<SyncSender<Job>>,
    done: Receiver<Done>,
    handle: Option<JoinHandle<()>>,
}

impl ScreenWorker {
    pub fn spawn(index: usize, orient: Orient, mailbox: Mailbox, brightness: f32) -> Result<Self> {
        let (jobs, job_rx) = sync_channel::<Job>(1);
        let (done_tx, done) = sync_channel::<Done>(1);
        let (ready_tx, ready_rx) = sync_channel::<Result<()>>(1);
        let handle = std::thread::Builder::new()
            .name(format!("render-{index}"))
            .spawn(move || {
                let mut renderer = match Renderer::new() {
                    Ok(r) => {
                        let _ = ready_tx.send(Ok(()));
                        r
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let mut frames = ScreenFrames::new(orient, mailbox, brightness);
                for job in job_rx {
                    if done_tx.send(frames.run(&mut renderer, job)).is_err() {
                        break;
                    }
                }
            })?;
        ready_rx.recv().map_err(|_| anyhow!("render worker {index} died at start"))??;
        Ok(Self { jobs: Some(jobs), done, handle: Some(handle) })
    }

    pub fn send(&self, job: Job) -> Result<()> {
        self.jobs
            .as_ref()
            .expect("jobs sender lives until drop")
            .send(job)
            .map_err(|_| anyhow!("render worker stopped"))
    }

    pub fn wait(&self) -> Result<Done> {
        self.done.recv().map_err(|_| anyhow!("render worker stopped"))
    }
}

impl Drop for ScreenWorker {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
```

Add `pub mod worker;` to `crates/app/src/lib.rs`.

- [ ] **Step 3: Slim `ScreenSlot` and drive the workers from the loop**

In `crates/app/src/runloop.rs`: `ScreenSlot` keeps `index, role, orient, mailbox, ui` and gains `pub brightness: f32`; remove `lut, cur, prev, first`, `flush`, and the `first_flush…` test (now covered by worker tests; move its 90° panel-space assertion into `worker.rs` tests as `flush_maps_rects_to_panel_space`, constructing `ScreenFrames::new(Orient::new(90, false), …)` and editing `frames.cur` the same way; `cur` is private, so put that test inside `worker.rs`'s own `tests` module).

In `RenderLoop::run`: drop `let mut renderer = Renderer::new()?;` and spawn workers after the guard:

```rust
        let workers: Vec<crate::worker::ScreenWorker> = self
            .screens
            .iter()
            .map(|s| {
                crate::worker::ScreenWorker::spawn(s.index, s.orient.clone(), s.mailbox.clone(), s.brightness)
            })
            .collect::<Result<_>>()?;
        let mut force_full = true;
```

On wake (where the old code set `s.first = true`), set `force_full = true` instead. Replace the per-screen render block with:

```rust
            for (i, s) in self.screens.iter_mut().enumerate() {
                let t = Instant::now();
                let scene = model.scene(s.index, now, &mut s.ui);
                stats.screen(i).build.push(t.elapsed().as_secs_f32() * 1000.0);
                workers[i].send(crate::worker::Job { scene, fade, force_full })?;
            }
            force_full = false;
            for (i, w) in workers.iter().enumerate() {
                let d = w.wait()?;
                let st = stats.screen(i);
                if d.skipped {
                    st.skipped += 1;
                } else {
                    st.render.push(d.render_ms);
                    st.flush.push(d.flush_ms);
                }
            }
```

Declare `workers` before `_guard` is dropped: `_guard` is created first, so on return `workers` drops first (joining render threads), then the guard sends `Quit` to the display threads. Keep that order.

- [ ] **Step 4: Run everything**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings`
Expected: PASS.

Smoke test: `cargo run -q -- run --sim --source fake` for 30 s, then quit; `top -H -p $(pgrep -x rackscreen)` during the run shows four `render-N` threads. Static screens (e.g. sun) stay quiet in the stats line (`skipped` high) after enabling `RUST_LOG=info` and waiting 60 s.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/worker.rs crates/app/src/lib.rs crates/app/src/runloop.rs crates/app/src/run.rs
git commit -m "perf(app): one render worker per screen; skip unchanged scenes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Base layer cache

**Files:**
- Modify: `crates/core/src/scene.rs` (`base_len`)
- Modify: `crates/core/src/p2/ring.rs` (`track`, `fill`)
- Modify: `crates/core/src/p2/screen.rs` (`compose` base-first order, test)
- Modify: `crates/render/src/renderer.rs` (cache)

**Interfaces:**
- Produces: `Scene { pub items: Vec<Drawable>, pub slide: Option<Box<Slide>>, pub base_len: usize }` (`Scene::new()` sets `base_len: 0`); `RingSpec::track(&self) -> Vec<Drawable>`, `RingSpec::fill(&self) -> Vec<Drawable>` with `drawables() == track() ++ fill()`.
- Contract: `items[..base_len]` is the base; the renderer may reuse a cached raster of it when the slice is equal to the last one and `items[0]` is `Drawable::Clear`.

- [ ] **Step 1: Failing renderer tests**

Add to `crates/render/src/renderer.rs` tests:

```rust
    fn based_scene(hero_x: f32) -> Scene {
        let mut s = Scene::new();
        s.push(Drawable::Arc { cx: 120.0, cy: 120.0, r: 106.0, width: 9.0, start_deg: 30.0, sweep_deg: 300.0, color: Color::hex(0x2C2C33), alpha: 1.0 });
        s.push(Drawable::RoundRect { x: 66.0, y: 150.0, w: 108.0, h: 48.0, r: 14.0, color: Color::hex(0x24242A), alpha: 1.0 });
        s.base_len = s.items.len();
        s.push(Drawable::RoundRect { x: hero_x, y: 60.0, w: 30.0, h: 30.0, r: 4.0, color: Color::hex(0xEDEDEF), alpha: 1.0 });
        s
    }

    #[test]
    fn base_cache_draws_the_same_pixels_as_one_pass() {
        let mut cached = Renderer::new().unwrap();
        let mut plain = Renderer::new().unwrap();
        for x in [50.0, 80.0, 80.0, 110.0] {
            let s = based_scene(x);
            let mut flat = s.clone();
            flat.base_len = 0;
            let (mut a, mut b) = (new_pixmap(), new_pixmap());
            cached.render(&s, &mut a);
            plain.render(&flat, &mut b);
            assert_eq!(a.data(), b.data(), "hero at {x}");
        }
    }

    #[test]
    fn base_without_leading_clear_is_not_cached() {
        let mut r = Renderer::new().unwrap();
        let mut s = based_scene(50.0);
        s.items.remove(0);
        s.base_len -= 1;
        let mut px = new_pixmap();
        r.render(&s, &mut px);
        assert!(r.base.is_none());
    }
```

(Adjust `Color` import to what the module already uses, e.g. `rackscreen_core::theme::Color`.)

Run: `cargo test -p rackscreen-render --lib renderer -q`
Expected: compile error (`base_len`, `r.base`).

- [ ] **Step 2: `Scene::base_len`**

In `crates/core/src/scene.rs`, add the field with a doc comment and `base_len: 0` in `Scene::new`:

```rust
    /// `items[..base_len]` rarely change (clear, ring track, title, card
    /// background); the renderer may reuse a cached raster of them.
    pub base_len: usize,
```

- [ ] **Step 3: Renderer cache**

In `crates/render/src/renderer.rs`, add `base: Option<(Vec<Drawable>, Pixmap)>` to `Renderer` (`None` in `new`), and replace the non-slide path of `render`:

```rust
        let n = scene.base_len.min(scene.items.len());
        let cacheable = n > 0 && matches!(scene.items.first(), Some(Drawable::Clear(_)));
        let start = if cacheable {
            let base = &scene.items[..n];
            match &self.base {
                Some((d, raster)) if d.as_slice() == base => {
                    px.data_mut().copy_from_slice(raster.data());
                }
                _ => {
                    for d in base {
                        self.painter.draw_item(px, d);
                    }
                    self.base = Some((base.to_vec(), px.clone()));
                }
            }
            n
        } else {
            0
        };
        for d in &scene.items[start..] {
            self.painter.draw_item(px, d);
        }
```

Run: `cargo test -p rackscreen-render --lib renderer -q`
Expected: PASS.

- [ ] **Step 4: Ring track/fill split**

In `crates/core/src/p2/ring.rs`, split `drawables`:

```rust
    /// The full-length background track.
    pub fn track(&self) -> Vec<Drawable> {
        self.arc(0.0, 1.0, TRACK, 1.0).into_iter().collect()
    }

    /// Everything drawn over the track.
    pub fn fill(&self) -> Vec<Drawable> {
        let mut out = Vec::new();
        match &self.kind {
            // the existing match arms from `drawables`, unchanged
        }
        out
    }

    pub fn drawables(&self) -> Vec<Drawable> {
        let mut out = self.track();
        out.extend(self.fill());
        out
    }
```

Move the existing `match &self.kind { … }` body (all arms, including the early `return out` in `Bands`) into `fill` verbatim.

- [ ] **Step 5: `compose` emits the base first (test first)**

Update `compose_orders_ring_title_hero_unit_pill_card` in `crates/core/src/p2/screen.rs` to the new order:

```rust
        let items = &scene.items;
        assert!(matches!(items[0], Drawable::Clear(_)));
        assert!(matches!(items[1], Drawable::Arc { color, .. } if color == TRACK), "track first");
        assert!(matches!(&items[2], Drawable::CurvedText { text, .. } if text == "CPU"));
        assert!(matches!(items[3], Drawable::RoundRect { color, .. } if color == CARD), "card background in the base");
        assert_eq!(scene.base_len, 4);
        assert!(
            matches!(items[4], Drawable::Arc { sweep_deg, .. } if (sweep_deg - 300.0 * 0.42).abs() < 1e-3),
            "fill right after the base"
        );
```

Keep the rest of the test (runs, card rect, polyline). Import `TRACK` from `crate::theme::p2`.

Run: `cargo test -p rackscreen-core compose -q`
Expected: FAIL (old order).

Then change the top of `compose`:

```rust
pub fn compose(s: &P2Screen, ui: &mut ScreenUi, now: Secs) -> Scene {
    let mut scene = Scene::new();
    let ring = glided_ring(&s.ring, ui, now);
    scene.items.extend(ring.track());
    scene.push(curved_title(&s.title));
    let pill_cy = if s.unit_below {
        PILL_CY_UNIT_BELOW
    } else {
        PILL_CY
    };
    if s.card.is_some() {
        scene.push(Drawable::RoundRect {
            x: CX - CARD_W / 2.0,
            y: card_top(pill_cy),
            w: CARD_W,
            h: CARD_H,
            r: CARD_R,
            color: CARD,
            alpha: 1.0,
        });
    }
    scene.base_len = scene.items.len();
    scene.items.extend(ring.fill());
    if let Some((a, b)) = &s.ends {
        scene.items.extend(ring_ends(a, b));
    }
```

…then the hero, unit and pill code as before (remove the later duplicate `pill_cy` binding), and in the card branch only push `card_drawables` (the background is already in the base):

```rust
    if let Some(card) = &s.card {
        scene
            .items
            .extend(card_drawables(card, ui, chart_rect(pill_cy), now));
    }
```

- [ ] **Step 6: Goldens must stay byte-identical**

Run: `cargo test -p rackscreen-render --test golden -q`
Expected: PASS. Reordering is pixel-safe only where the moved base items (title, card background) do not overlap the items that used to be drawn before them (ring fill caps, hero, pill).

If any golden fails: move the curved title out of the base (push it right after `scene.base_len = scene.items.len();`, before `ring.fill()`), update the compose test (`items[2]` = card background, `base_len` 3, title at `items[3]`, fill at `items[4]`), and rerun. If a golden still fails, also move the card background back to its original place (directly before `card_drawables`), set `base_len` after the track (2), and update the test to match. Record which variant was needed in the commit message.

- [ ] **Step 7: Run everything and bench**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings`
Run: `cargo test --release -q -p rackscreen-render --test bench -- --ignored --nocapture`
Expected: PASS; `pipeline` and per-role numbers at or below Task 1's.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/scene.rs crates/core/src/p2/ring.rs crates/core/src/p2/screen.rs crates/render/src/renderer.rs
git commit -m "perf(core,render): cached base layer per screen (clear, ring track, title, card background)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: 60 Hz deadline pacing, 50 MHz default, docs, version 0.9.0

**Files:**
- Modify: `crates/app/src/runloop.rs` (`advance` and its use)
- Modify: `crates/app/src/config.rs` (`fps: 60`, `default_hz` 50 MHz, tests)
- Modify: `config.example.yaml`, `README.md`
- Modify: `Cargo.toml` (`version = "0.9.0"`), `Cargo.lock` (refreshed by build)
- Modify: the `bench.rs` doc comment (Pi budget at 60 fps)

**Interfaces:**
- Produces: `runloop::advance(deadline: Instant, tick: Duration, now: Instant) -> Instant`.

- [ ] **Step 1: Failing pacing test**

In `crates/app/src/runloop.rs` tests:

```rust
    #[test]
    fn advance_keeps_cadence_and_resyncs_when_far_behind() {
        let t0 = Instant::now();
        let tick = Duration::from_millis(16);
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(5)), t0 + tick);
        // one tick late: keep the cadence, the next sleep is short
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(20)), t0 + tick);
        // more than a tick behind the next deadline: start over from now
        let late = t0 + Duration::from_millis(50);
        assert_eq!(advance(t0, tick, late), late);
    }
```

Run: `cargo test -p rackscreen-app advance -q`
Expected: compile error.

- [ ] **Step 2: Implement and use**

```rust
/// The next frame deadline: one tick after the last, or `now` when the loop
/// fell more than a tick behind it (no burst of catch-up frames).
pub fn advance(deadline: Instant, tick: Duration, now: Instant) -> Instant {
    let next = deadline + tick;
    if next + tick < now {
        now
    } else {
        next
    }
}
```

In `RenderLoop::run`, before the loop: `let mut deadline = Instant::now();`. Replace the end-of-loop `let spent = …; if spent < tick { sleep(tick - spent) }` with:

```rust
            deadline = advance(deadline, tick, Instant::now());
            if let Some(d) = deadline.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
```

`t0` is now unused; remove it. After the night-mode `continue` paths, reset `deadline = Instant::now();` on wake so the first frame after waking is not counted as late.

- [ ] **Step 3: Defaults**

`crates/app/src/config.rs`: `DisplayCfg::default()` → `fps: 60`; `default_hz()` → `50_000_000`. Update the tests that assert the old values (`c.display.fps, 30` → `60`; `c.screens[0].hz, 40_000_000` → `50_000_000`; `c.screens[2].hz, 16_000_000` → `50_000_000` after the example change below). Run `cargo test -p rackscreen-app config -q` and fix any other literal the failures name.

`config.example.yaml`: `fps: 60`; every screen `hz: 50000000`; above the screens list add the comment:

```yaml
# hz: SPI clock. The Pi 3B+ divides a 400 MHz core clock, so the real steps
# are 40, 50 and 66.7 MHz. 50 MHz ran clean on all four panels; 66.7 MHz glitched.
```

`README.md`: the example config block (`fps: 30` and the four `hz:` lines) gets the same values and comment; in the performance/hardware section add one paragraph: "RackScreen renders each screen on its own thread at up to 60 fps, skips frames that did not change and sends only the changed regions over SPI. At 50 MHz a full 240×240 frame takes about 18 ms, so a fully changing screen (a transition) runs at roughly 50 fps while its bus partner is idle."

`crates/render/tests/bench.rs` doc comment: "The Pi 3B+ budget is 4 ms per screen at 60 fps with four render threads".

`Cargo.toml` (root workspace package): `version = "0.9.0"`. Run `cargo build -q` to refresh `Cargo.lock`.

- [ ] **Step 4: Run everything**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings`
Expected: PASS. Simulator smoke test at 60 fps: `cargo run -q -- run --sim --source fake`; motion is smooth, CPU per `render-N` thread low on static screens.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/runloop.rs crates/app/src/config.rs config.example.yaml README.md Cargo.toml Cargo.lock crates/render/tests/bench.rs
git commit -m "perf(app): 60 Hz deadline pacing; 60 fps and 50 MHz defaults; version 0.9.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Verify on the Pi (checkpoint, needs Silke)

**Files:**
- Create: `.superpowers/sdd/speed-results.md` (gitignored notes)

- [ ] **Step 1: Ask Silke to release**

Ask: "Push main and tag v0.9.0 for the release workflow, then `sudo rackscreen update` on the Pi?" Only on yes: `git push origin main && git tag v0.9.0 && git push origin v0.9.0`, wait for the `release` workflow (`gh run watch` is not allowed; check with `gh run list --workflow release -L 1` once it should be done), then on the Pi `sudo rackscreen update` (sudo password via a secret prompt, deleted after).

- [ ] **Step 2: Set fps 60 on the Pi config**

`/etc/rackscreen/config.yaml` on the Pi has `fps: 30` from before; change to `fps: 60` (backup first: `sudo cp /etc/rackscreen/config.yaml /etc/rackscreen/config.yaml.0.8.1.bak`), restart `rackscreen@silke`.

- [ ] **Step 3: Collect and compare**

After 3 minutes: `journalctl -u rackscreen@silke --since -3min | grep "frame stats"` and `top -b -n 2 -d 3 -H -p $(pgrep -x rackscreen) | tail -12`. Write both into `.superpowers/sdd/speed-results.md` next to the Task 1 baseline (or the 66 % / 30 fps measurement from the spec).

Pass criteria from the spec:
- a moving screen (cpu hero rolling, iss countdown) logs `fps` ≥ 55;
- the summed CPU of the `render-*` threads plus `render` main thread at 60 fps is below 66 % of one core;
- Silke sees no glitches or smear on any panel.

If a criterion fails, report the stats lines to Silke with the slowest stage named (build, render, flush or push) before changing anything.

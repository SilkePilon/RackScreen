# Readable surfaces, vertical slide, hero fit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make cards, tracks, captions and pills visible on the real GC9A01 panels, replace the iris with a vertical slide, and keep the hero (plus its unit) clear of the ring.

**Architecture:** Contrast is a token change in `theme::p2` plus a floor test over every role's drawables. The hero fit is a pure geometry function in `p2/text.rs` used by `compose`, with the chosen scale glided per screen. The slide is a two-scene `Scene` (`Scene.slide`) built by `Model::scene` from two role scenes (the outgoing one keeps its widget state in a new `ScreenUi` outgoing slot) and drawn by the renderer through a transparent scratch pixmap with a transform, opacity and a round mask.

**Tech Stack:** Rust workspace (rackscreen-core, -render, -app, -setup), tiny-skia 0.12.

Spec: `docs/superpowers/specs/2026-11-02-contrast-slide-herofit-design.md`.

## Global Constraints

- New tokens: `CARD = #24242A`, `TRACK = #2C2C33`, `TEXT3 = #80808A`; `PILL_FILL_ALPHA = 0.30` (the spec's 0.26 leaves the grey pill at `0x26`, under the `0x2A` floor; 0.30 gives `0x2C`); bars `past_alpha` 0.28 → 0.45.
- Floor: every drawable except `Clear`, pure `BLACK` and `INK` cut-outs has an effective colour (channel × colour alpha × drawable alpha) whose brightest channel is ≥ `0x2A`; text runs (`Run`, `CurvedText`) ≥ `0x78`.
- Slide: `TRANSITION_SECS = 0.75`; `e = InOutCubic(t)`, `s = 1 − 0.08·sin(πt)`; outgoing `dy = −240e`, scale `s`, alpha `1 − 0.6e`; incoming `dy = 240(1 − e)`, scale `s`, alpha `0.4 + 0.6e`.
- Hero fit: inner ring edge `RING_R − RING_W/2 = 101.5`, `HERO_RING_MARGIN = 10`; all four corners of the hero group box within `91.5` of (120,120); scale glides over `HERO_FIT_GLIDE_SECS = 0.4`.
- Gate after every task: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Do not push or tag.

---

### Task 1: Contrast floor

**Files:**
- Modify: `crates/core/src/theme.rs:256-258`, `crates/core/src/p2/layout.rs:37`, `crates/core/src/p2/roles/cluster.rs:248,537`, `crates/core/src/p2/roles/energy.rs:80`
- Test: `crates/render/tests/golden.rs` (new `contrast_floor_on_every_role`), goldens re-captured

**Interfaces:**
- Produces: the new token values; nothing else changes shape.

- [ ] **Step 1: Write the failing floor test**

Append to `crates/render/tests/golden.rs`:

```rust
/// Brightest channel of a colour as it lands on black.
fn lit(c: rackscreen_core::theme::Color, alpha: f32) -> u8 {
    let k = (c.a as f32 / 255.0) * alpha.clamp(0.0, 1.0);
    [c.r, c.g, c.b]
        .iter()
        .map(|v| (*v as f32 * k).round() as u8)
        .max()
        .unwrap()
}

fn floor_violations(s: &rackscreen_core::scene::Scene) -> Vec<String> {
    use rackscreen_core::scene::Drawable as D;
    use rackscreen_core::theme::p2::INK;
    use rackscreen_core::theme::BLACK;
    let cut_out = |c: rackscreen_core::theme::Color| {
        (c.r, c.g, c.b) == (BLACK.r, BLACK.g, BLACK.b) || (c.r, c.g, c.b) == (INK.r, INK.g, INK.b)
    };
    let mut bad = Vec::new();
    for d in &s.items {
        let (what, color, alpha, min) = match d {
            D::Clear(_) => continue,
            D::Arc { color, alpha, .. } => ("arc", *color, *alpha, 0x2A),
            D::RoundRect { color, alpha, .. } => ("rect", *color, *alpha, 0x2A),
            D::Polygon { color, alpha, .. } => ("polygon", *color, *alpha, 0x2A),
            D::Ellipse { color, alpha, .. } => ("ellipse", *color, *alpha, 0x2A),
            D::Polyline { color, alpha, .. } => ("polyline", *color, *alpha, 0x2A),
            D::Icon { color, alpha, .. } => ("icon", *color, *alpha, 0x2A),
            D::Run { color, alpha, text, .. } => {
                if text.trim().is_empty() {
                    continue;
                }
                ("run", *color, *alpha, 0x78)
            }
            D::CurvedText { color, alpha, .. } => ("title", *color, *alpha, 0x78),
        };
        if cut_out(color) || alpha <= 0.0 {
            continue;
        }
        let v = lit(color, alpha);
        if v < min {
            bad.push(format!("{what} {color:?} × {alpha} = {v:#04x} < {min:#04x}"));
        }
    }
    bad
}

#[test]
fn contrast_floor_on_every_role() {
    let full = full_model();
    let mut bare = Model::new(Thresholds::default());
    bare.set_token_present(true);
    bare.set_location_present(true);
    bare.set_github_token_present(true);
    bare.set_jellyfin_configured(true);
    link(&mut bare, LinkTarget::K8sApi);
    link(&mut bare, LinkTarget::Prometheus);
    let mut problems = Vec::new();
    for (label, m) in [("normal", &full), ("no data", &bare)] {
        for role in Role::ALL {
            let s = m.scene_for_role(role, 120.0, &mut ScreenUi::new());
            for p in floor_violations(&s) {
                problems.push(format!("{} ({label}): {p}", role.name()));
            }
        }
    }
    assert!(problems.is_empty(), "too dark for the panel:\n{}", problems.join("\n"));
}
```

- [ ] **Step 2: Run it to see the offenders**

Run: `cargo test -p rackscreen-render --test golden contrast_floor_on_every_role`
Expected: FAIL listing card rects (`#121214`), track arcs (`#17171A`), TEXT3 runs (`#4A4A4F`), pill fills, past bars.

- [ ] **Step 3: Raise the tokens**

`crates/core/src/theme.rs` in `pub mod p2`:

```rust
    pub const TEXT3: Color = Color::hex(0x80808A);
    pub const TRACK: Color = Color::hex(0x2C2C33);
    pub const CARD: Color = Color::hex(0x24242A);
```

`crates/core/src/p2/layout.rs`:

```rust
pub const PILL_FILL_ALPHA: f32 = 0.30;
```

`crates/core/src/p2/roles/cluster.rs:248` and `crates/core/src/p2/roles/energy.rs:80`: `past_alpha: 0.45,`; the assertion at `cluster.rs:537` becomes `*past_alpha == 0.45`.

- [ ] **Step 4: Re-run and fix what remains**

Run: `cargo test -p rackscreen-render --test golden contrast_floor_on_every_role`
Expected: PASS. If a role still reports an element, raise that element's alpha (or move it to `TEXT3`/`TRACK`) at the line the message points to until it clears the floor; do not lower the floor. Record each such change in the commit message body.

- [ ] **Step 5: Re-capture goldens, look, gate, commit**

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden` then view `p2_cpu.png`, `p2_restarts.png`, `p2_ups.png`, `p2_cpu_nodata.png` (card and track now plainly visible, captions readable). Run the gate.

```bash
git add -A crates
git commit -m "fix(core): lift card, track, caption and pill colours above the panel's black floor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Hero fit

**Files:**
- Modify: `crates/core/src/p2/layout.rs`, `crates/core/src/p2/text.rs`, `crates/core/src/p2/screen.rs:75-97`, `crates/core/src/p2/roles/mod.rs:66-97`
- Test: unit tests in `text.rs` and `roles/mod.rs`; goldens re-captured

**Interfaces:**
- Produces: `layout::{HERO_RING_MARGIN, HERO_FIT_GLIDE_SECS}`, `text::{hero_at, hero_group_width, hero_fits, hero_fit_scale}`.

- [ ] **Step 1: Write the failing tests**

Append to the tests in `crates/core/src/p2/text.rs`:

```rust
    #[test]
    fn hero_fit_keeps_short_numbers_and_shrinks_wide_ones() {
        assert_eq!(hero_fit_scale("42", Some("%"), 1.0, HERO_BASELINE), 1.0);
        assert_eq!(hero_fit_scale("100", None, 1.0, HERO_BASELINE), 1.0);
        let k = hero_fit_scale("100", Some("%"), 1.0, HERO_BASELINE);
        assert!(k < 1.0 && k > 0.7, "100 % shrinks a little: {k}");
        assert!(hero_fits("100", Some("%"), k, HERO_BASELINE));
        assert!(!hero_fits("100", Some("%"), k + 0.02, HERO_BASELINE), "largest that fits");
        assert!(hero_fit_scale("16/16", None, 0.82, HERO_BASELINE) <= 0.82, "the role's scale is a ceiling");
        assert!(hero_group_width("42", Some("%"), 1.0) > hero_width("42", 1.0));
    }

    #[test]
    fn hero_at_centres_on_the_given_x() {
        let mut ui = ScreenUi::new();
        let h = hero_at(&mut ui, "42", 1.0, 100.0, HERO_BASELINE, TEXT, 0.0);
        assert!(((h.x0 + h.x1) / 2.0 - 100.0).abs() < 1e-3);
    }
```

Replace the test `worst_case_heroes_fit_180_px` in `crates/core/src/p2/roles/mod.rs` (and its imports `HERO_MAX_W`, `hero_width`) with:

```rust
    use crate::p2::layout::{HERO_BASELINE, HERO_BASELINE_UNIT_BELOW};
    use crate::p2::text::{hero_fit_scale, hero_fits};

    #[test]
    fn worst_case_heroes_fit_the_ring() {
        // (hero, superscript unit, the role's hero_scale, unit below?)
        for (text, unit, scale, below) in [
            ("100", Some("%"), 1.0, false), // cpu, mem, storage, ups, renewable, moon
            ("999", None, 1.0, false),      // pods, deploys, contributions, carbon
            ("16/16", None, 0.82, false),   // cluster
            ("-12", Some("°"), 1.0, false), // weather below zero
            ("59:59", None, 0.82, false),   // iss
            ("-0.123", None, 0.78, true),   // price
            ("1000", None, 1.0, true),      // net Mb/s
            ("120", None, 1.0, true),       // wind, aqi, rain minutes
            ("23:59", None, 0.82, false),   // sun
            ("9999", None, 1.0, false),     // media library, restarts
            ("99.9", None, 1.0, true),      // traffic req/s
            ("6.1k", None, 1.0, true),      // fans
            ("260", Some("V"), 1.0, false), // mains
            ("82", Some("d"), 1.0, false),  // uptime
            ("10.52", None, 0.82, false),   // load from 10 up
            ("12/12", None, 0.82, false),   // ci
            ("1:41:54", None, 0.62, true),  // jellyfin h:mm:ss
            ("59:59", None, 0.82, true),    // jellyfin m:ss
        ] {
            let base = if below { HERO_BASELINE_UNIT_BELOW } else { HERO_BASELINE };
            let k = hero_fit_scale(text, unit, scale, base);
            assert!(k <= scale && k > 0.4, "{text}: {k}");
            assert!(hero_fits(text, unit, k, base), "{text} {unit:?} at {k}");
        }
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core hero`
Expected: compile errors (`hero_fit_scale`, `hero_at` not found).

- [ ] **Step 3: Implement**

`crates/core/src/p2/layout.rs`, after `HERO_MAX_W`:

```rust
/// The hero group keeps this far inside the ring's inner edge.
pub const HERO_RING_MARGIN: f32 = 10.0;
/// A fitted hero size change glides over this long.
pub const HERO_FIT_GLIDE_SECS: f64 = 0.4;
```

`crates/core/src/p2/text.rs`: rename the body of `hero` into `hero_at` with a `cx` parameter, keep `hero` as the centred wrapper, and add the fit functions:

```rust
/// The rolling hero, centred on the screen: one clipped `Run` per glyph.
pub fn hero(
    ui: &mut ScreenUi,
    text: &str,
    scale: f32,
    baseline: f32,
    color: Color,
    now: Secs,
) -> HeroLayout {
    hero_at(ui, text, scale, CX, baseline, color, now)
}

/// The rolling hero centred on `cx`.
#[allow(clippy::too_many_arguments)]
pub fn hero_at(
    ui: &mut ScreenUi,
    text: &str,
    scale: f32,
    cx: f32,
    baseline: f32,
    color: Color,
    now: Secs,
) -> HeroLayout {
    let px = HERO_PX * scale;
    let roll = ui.roll(("hero", 0), text, now);
    let widths = cell_widths(roll.text(), px);
    let total: f32 = widths.iter().sum();
    let x0 = cx - total / 2.0;
    // ... the rest of the former `hero` body unchanged ...
}

/// Width of the hero plus its superscript unit (gap included).
pub fn hero_group_width(text: &str, unit: Option<&str>, scale: f32) -> f32 {
    let px = HERO_PX * scale;
    hero_width(text, scale)
        + unit.map_or(0.0, |u| UNIT_GAP + measure(FontId::Bold, px * UNIT_SCALE, u, 0.0))
}

/// All four corners of the hero group box are at least `HERO_RING_MARGIN`
/// inside the ring's inner edge.
pub fn hero_fits(text: &str, unit: Option<&str>, scale: f32, baseline: f32) -> bool {
    use crate::theme::layout::CY;
    let half = hero_group_width(text, unit, scale) / 2.0;
    let top = baseline - cap_height(FontId::Bold, HERO_PX * scale);
    let limit = RING_R - RING_W / 2.0 - HERO_RING_MARGIN;
    [top, baseline]
        .iter()
        .all(|y| (half * half + (y - CY) * (y - CY)).sqrt() <= limit)
}

/// The largest scale up to `max_scale` at which the hero group fits the ring.
pub fn hero_fit_scale(text: &str, unit: Option<&str>, max_scale: f32, baseline: f32) -> f32 {
    if hero_fits(text, unit, max_scale, baseline) {
        return max_scale;
    }
    let (mut lo, mut hi) = (0.2_f32, max_scale);
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        if hero_fits(text, unit, mid, baseline) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}
```

`crates/core/src/p2/screen.rs` in `compose`, replace the `let h = hero(...)` line with:

```rust
    let unit_sup = s.unit.as_deref().filter(|_| !s.unit_below);
    let fit = hero_fit_scale(&s.hero, unit_sup, s.hero_scale, baseline);
    let scale = ui.glide_with(("hero-scale", 0), fit, now, HERO_FIT_GLIDE_SECS);
    let unit_w = unit_sup.map_or(0.0, |u| {
        UNIT_GAP + measure(FontId::Bold, HERO_PX * scale * UNIT_SCALE, u, 0.0)
    });
    let h = hero_at(ui, &s.hero, scale, CX - unit_w / 2.0, baseline, s.hero_color, now);
```

and import `hero_at, hero_fit_scale` from `p2::text`, `measure, FontId` from `p2::font`, `CX` from `theme::layout`, and `HERO_FIT_GLIDE_SECS, HERO_PX, UNIT_GAP, UNIT_SCALE` from `p2::layout` (drop `hero` from the import if unused). Delete `HERO_MAX_W` from `layout.rs` and the `hero_width(..) < HERO_MAX_W` line in the text test if nothing else uses it.

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green. A screen test that pinned the hero's x at 120 with a superscript unit now sees the group centred; update it to assert the group centre (`(h.x0 + unit end) / 2 ≈ 120`) rather than the digits' centre.

- [ ] **Step 5: Goldens, look, gate, commit**

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden`; view `p2_ups.png` (needs a 100 % state: render `full_model` with `Event::Ups { charge_pct: 100.0, .. }` in a scratch check, or the ups golden if it already shows 100), `p2_mains.png`, `p2_uptime.png`, `p2_cpu.png`: number plus unit centred, clear gap to the ring. Run the gate.

```bash
git add -A crates
git commit -m "feat(core): fit the hero and its unit inside the ring, centred as one group

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Slide model (core)

**Files:**
- Modify: `crates/core/src/scene.rs`, `crates/core/src/screens.rs`, `crates/core/src/p2/ui.rs`, `crates/core/src/scene_model.rs:55-68` and its tests, `crates/core/src/p2/screen.rs:196`
- Test: unit tests in each

**Interfaces:**
- Produces: `scene::{Placement, Slide}`, `Placement::STILL`, `Scene.slide: Option<Box<Slide>>` (the `zoom` field is gone), `screens::slide(t) -> (Placement, Placement)`, `screens::TRANSITION_SECS = 0.75`, `ScreenUi::{outgoing(from) -> &mut ScreenUi, end_slide()}`.

- [ ] **Step 1: Write the failing tests**

`crates/core/src/screens.rs` tests:

```rust
    #[test]
    fn slide_moves_old_up_and_new_in_from_below() {
        let (o, i) = slide(0.0);
        assert_eq!((o.dy, o.scale, o.alpha), (0.0, 1.0, 1.0));
        assert!((i.dy - 240.0).abs() < 1e-3 && (i.alpha - 0.4).abs() < 1e-6);
        let (o, i) = slide(0.5);
        assert!((o.dy + 120.0).abs() < 1e-3 && (i.dy - 120.0).abs() < 1e-3);
        assert!((o.scale - 0.92).abs() < 1e-4 && (i.scale - 0.92).abs() < 1e-4);
        assert!((o.alpha - 0.7).abs() < 1e-4 && (i.alpha - 0.7).abs() < 1e-4);
        let (o, i) = slide(1.0);
        assert!((o.dy + 240.0).abs() < 1e-3);
        assert_eq!((i.dy, i.scale, i.alpha), (0.0, 1.0, 1.0));
    }
```

`crates/core/src/p2/ui.rs` tests (add a `#[cfg(test)] mod tests` if absent):

```rust
    #[test]
    fn outgoing_keeps_the_old_roles_state_while_the_new_one_starts_fresh() {
        let mut ui = ScreenUi::new();
        ui.for_role(Role::Cpu);
        ui.glide(("x", 0), 10.0, 0.0);
        ui.glide(("x", 0), 50.0, 1.0);
        let mid = ui.outgoing(Role::Cpu).glide(("x", 0), 50.0, 1.2);
        assert!(mid > 10.0 && mid < 50.0, "the cpu glide carries on in the outgoing slot");
        assert_eq!(ui.role(), None);
        ui.for_role(Role::Mem);
        assert_eq!(ui.outgoing(Role::Cpu).role(), Some(Role::Cpu), "for_role leaves the slot alone");
        ui.end_slide();
        assert_eq!(ui.outgoing(Role::Cpu).role(), None, "after the slide the slot starts empty");
    }
```

`crates/core/src/scene_model.rs`: replace `cycling_screen_transitions_with_iris` with:

```rust
    #[test]
    fn cycling_screen_slides_between_roles() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        let mut ui = ScreenUi::new();
        let s = m.scene(0, 3.4, &mut ui);
        let sl = s.slide.as_ref().expect("mid slide");
        assert_eq!(title_of(&sl.out).as_deref(), Some("CPU"));
        assert_eq!(title_of(&sl.inc).as_deref(), Some("MEMORY"));
        assert!(sl.out_at.dy < 0.0 && sl.inc_at.dy > 0.0, "old goes up, new comes from below");
        for i in 101..=200 {
            m.tick(i as f64 * 0.033);
        }
        assert_eq!(m.current_role(0), Role::Mem);
        assert!(m.scene(0, 6.6, &mut ui).slide.is_none(), "settled on mem");
        assert_eq!(ui.outgoing(Role::Cpu).role(), None, "the outgoing slot was dropped");
    }
```

and in the other tests: `assert_eq!(s.zoom, 1.0)` → `assert!(s.slide.is_none())`; in `transition_into_a_cluster_role_with_api_down_shows_connecting` replace the two asserts with

```rust
        let sl = s.slide.as_ref().expect("mid transition");
        assert!(connecting(&sl.inc), "incoming cpu half is the connecting scene");
```

In `crates/core/src/p2/screen.rs:196`: `assert!(scene.slide.is_none());`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core`
Expected: compile errors (`slide`, `Slide`, `outgoing` not found; `zoom` still present).

- [ ] **Step 3: Implement**

`crates/core/src/scene.rs`: remove `zoom` from `Scene` and `Scene::new`; add

```rust
/// Where one half of a slide sits: vertical offset in px, scale about the
/// centre, opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub dy: f32,
    pub scale: f32,
    pub alpha: f32,
}

impl Placement {
    pub const STILL: Placement = Placement {
        dy: 0.0,
        scale: 1.0,
        alpha: 1.0,
    };
}

/// A screen change in progress: both role scenes and where each sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub out: Scene,
    pub out_at: Placement,
    pub inc: Scene,
    pub inc_at: Placement,
}
```

and the field `pub slide: Option<Box<Slide>>,` on `Scene` (initialised `None` in `Scene::new`), doc comment: "During a screen change: the two halves to draw instead of `items`."

`crates/core/src/screens.rs`: module doc "…cycling and the slide transition."; `pub const TRANSITION_SECS: Secs = 0.75;`; delete `transition_zoom`; add

```rust
/// Placement of the outgoing and incoming scene at `t ∈ 0..1`: the old one
/// moves up and out, the new one comes in from below, both a touch smaller
/// mid-move.
pub fn slide(t: f32) -> (Placement, Placement) {
    let t = t.clamp(0.0, 1.0);
    let e = Easing::InOutCubic.apply(t);
    let s = 1.0 - 0.08 * (std::f32::consts::PI * t).sin();
    (
        Placement {
            dy: -240.0 * e,
            scale: s,
            alpha: 1.0 - 0.6 * e,
        },
        Placement {
            dy: 240.0 * (1.0 - e),
            scale: s,
            alpha: 0.4 + 0.6 * e,
        },
    )
}
```

with `use crate::scene::Placement;`.

`crates/core/src/p2/ui.rs`: add the field `out: Option<Box<ScreenUi>>,` to `ScreenUi`, and:

```rust
    pub fn for_role(&mut self, role: Role) {
        if self.role != Some(role) {
            let out = self.out.take();
            *self = Self::default();
            self.role = Some(role);
            self.out = out;
        }
    }

    /// State for the outgoing half of a slide away from `from`. The first call
    /// moves this screen's state (which belongs to `from`) into the slot, so
    /// the incoming role starts fresh here and the old one keeps animating.
    pub fn outgoing(&mut self, from: Role) -> &mut ScreenUi {
        if self.out.as_ref().and_then(|o| o.role) != Some(from) {
            let mut prev = std::mem::take(self);
            prev.out = None;
            if prev.role != Some(from) {
                prev = ScreenUi::default();
            }
            self.out = Some(Box::new(prev));
        }
        self.out.as_mut().expect("set above")
    }

    /// The slide is over: drop the outgoing state.
    pub fn end_slide(&mut self) {
        self.out = None;
    }
```

(For the test's last assertion, `outgoing(Role::Cpu)` after `end_slide` on a `Mem` screen creates a default slot, whose role is `None`.)

`crates/core/src/scene_model.rs` `scene`:

```rust
        if let Some(tr) = state.transition(now) {
            let (out_at, inc_at) = crate::screens::slide(tr.t);
            let out = self.scene_for_role(tr.from, now, ui.outgoing(tr.from));
            let inc = self.scene_for_role(tr.to, now, ui);
            let mut s = Scene::new();
            s.slide = Some(Box::new(Slide {
                out,
                out_at,
                inc,
                inc_at,
            }));
            return s;
        }
        ui.end_slide();
        self.scene_for_role(role, now, ui)
```

importing `Slide` from `crate::scene`.

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green. (`rackscreen-render` does not compile yet; Task 4 fixes it — run only core here.)

- [ ] **Step 5: Commit**

```bash
git add -A crates/core
git commit -m "feat(core): slide transition model: two placed role scenes, outgoing widget state kept

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

(The workspace gate runs at the end of Task 4, which restores the renderer build.)

---

### Task 4: Render the slide; goldens, gifs, docs, version

**Files:**
- Modify: `crates/render/src/renderer.rs`, `crates/render/tests/golden.rs` (`iris_mid` → `slide_mid`), `crates/render/tests/bench.rs`, `README.md`, `config.example.yaml`, `crates/setup/src/screens/configure.rs:588`, `crates/setup/src/screens/screens.rs:52`, `crates/app/src/config.rs:97`, `Cargo.toml`, `Cargo.lock`, `.github/media/screens/*.gif`
- Test: renderer unit test, golden, bench

**Interfaces:**
- Consumes: `Scene.slide`, `Slide`, `Placement`, `screens::slide`.

- [ ] **Step 1: Write the failing renderer test**

Replace `zoom_scales_the_scene_about_the_centre` in `crates/render/src/renderer.rs` with:

```rust
    #[test]
    fn slide_draws_each_half_moved_faded_and_round() {
        use rackscreen_core::scene::{Placement, Slide};
        let m = cpu_model(42.0);
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let cpu = m.scene_for_role(Role::Cpu, 5.0, &mut ScreenUi::new());
        let count = |p: &Pixmap, y0: u32, y1: u32| {
            (y0..y1)
                .flat_map(|y| (0..240).map(move |x| (x, y)))
                .filter(|&(x, y)| pixel(p, x, y) != (0, 0, 0))
                .count()
        };
        let brightness = |p: &Pixmap| p.data().chunks_exact(4).map(|c| c[0] as u64 + c[1] as u64 + c[2] as u64).sum::<u64>();
        let placed = |out_at: Placement| {
            let mut s = Scene::new();
            s.slide = Some(Box::new(Slide {
                out: cpu.clone(),
                out_at,
                inc: Scene::new(),
                inc_at: Placement::STILL,
            }));
            s
        };
        r.render(&placed(Placement { dy: -120.0, scale: 1.0, alpha: 1.0 }), &mut px);
        assert!(count(&px, 0, 100) > 500, "the outgoing half moved up");
        assert_eq!(count(&px, 140, 240), 0, "the empty incoming half's Clear does not paint over it");
        assert_eq!(pixel(&px, 2, 2), (0, 0, 0), "round mask keeps the corners black");
        r.render(&placed(Placement::STILL), &mut px);
        let full = brightness(&px);
        r.render(&placed(Placement { dy: 0.0, scale: 1.0, alpha: 0.5 }), &mut px);
        let half = brightness(&px) as f64 / full as f64;
        assert!((0.4..0.6).contains(&half), "alpha 0.5 halves the light: {half}");
        r.render(&placed(Placement { dy: -240.0, scale: 1.0, alpha: 1.0 }), &mut px);
        assert_eq!(count(&px, 0, 240), 0, "fully off-screen draws nothing");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-render --lib`
Expected: compile errors (`scene.zoom` gone, `slide` unknown to the renderer).

- [ ] **Step 3: Implement the renderer**

In `crates/render/src/renderer.rs`: drop `FULL` and the zoom path; imports `use rackscreen_core::scene::{Drawable, Placement, Scene};`, `use rackscreen_core::theme::layout::{CX, CY, SIZE};`, `use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, PixmapPaint, Transform};`. `Renderer` gains `round: Mask`, built in `new`:

```rust
        let mut round = Mask::new(SIZE, SIZE).expect("mask");
        if let Some(p) = PathBuilder::from_circle(CX, CY, CX) {
            round.fill_path(&p, FillRule::Winding, true, Transform::identity());
        }
```

and

```rust
    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        if let Some(sl) = &scene.slide {
            px.fill(tiny_skia::Color::BLACK);
            self.draw_placed(&sl.out, sl.out_at, px);
            self.draw_placed(&sl.inc, sl.inc_at, px);
            return;
        }
        for d in &scene.items {
            self.painter.draw_item(px, d);
        }
    }

    /// One half of a slide: drawn without its `Clear` into the transparent
    /// scratch, then composited moved, scaled about the centre, faded and
    /// clipped to the round panel.
    fn draw_placed(&mut self, s: &Scene, at: Placement, px: &mut Pixmap) {
        if at.alpha <= 0.0 || at.scale <= 0.0 || at.dy.abs() >= SIZE as f32 {
            return;
        }
        self.scratch.fill(tiny_skia::Color::TRANSPARENT);
        for d in s.items.iter().filter(|d| !matches!(d, Drawable::Clear(_))) {
            self.painter.draw_item(&mut self.scratch, d);
        }
        let ts = Transform::from_translate(CX, CY + at.dy)
            .pre_scale(at.scale, at.scale)
            .pre_translate(-CX, -CY);
        let paint = PixmapPaint {
            opacity: at.alpha.min(1.0),
            ..PixmapPaint::default()
        };
        px.draw_pixmap(0, 0, self.scratch.as_ref(), &paint, ts, Some(&self.round));
    }
```

- [ ] **Step 4: Golden and bench**

`crates/render/tests/golden.rs`: rename `iris_mid_transition` to `slide_mid_transition`, sample at `3.4`, assert `s.slide.is_some()`, and `check("slide_mid", &px)`; delete `crates/render/tests/goldens/iris_mid.png`.

`crates/render/tests/bench.rs`, append inside `every_role_under_a_millisecond` before the final assertion:

```rust
    let cpu = m.scene_for_role(Role::Cpu, 1.0, &mut ScreenUi::new());
    let mem = m.scene_for_role(Role::Mem, 1.0, &mut ScreenUi::new());
    let (out_at, inc_at) = rackscreen_core::screens::slide(0.5);
    let mut s = rackscreen_core::scene::Scene::new();
    s.slide = Some(Box::new(rackscreen_core::scene::Slide {
        out: cpu,
        out_at,
        inc: mem,
        inc_at,
    }));
    r.render(&s, &mut px);
    let t0 = Instant::now();
    for _ in 0..ITERATIONS {
        r.render(&s, &mut px);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
    println!("{:<12} {ms:.3} ms", "slide");
    assert!(ms < 2.0 * BUDGET_MS, "mid-slide frame {ms:.3} ms");
```

Run: `cargo test -p rackscreen-render`, then `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden slide_mid_transition`; view `slide_mid.png` (cpu top half moving up, memory rising from below, corners black). Run `cargo test --release -p rackscreen-render --test bench -- --ignored --nocapture`.

- [ ] **Step 5: Wording, gifs, version**

Replace "iris"/"irises" in user-facing text with "slide"/"changes": `README.md` (intro: "with a zoom transition" → "with a vertical slide"; lines 151, 293, 319), `config.example.yaml` (`one_at_a_time` comment: "one screen changes at a time"), `crates/setup/src/screens/configure.rs:588` ("Only one screen changes at a time."), `crates/setup/src/screens/screens.rs:52`, `crates/app/src/config.rs:97`; copy the new `config.example.yaml` into the README `<details>` block. `Cargo.toml` `version = "0.8.1"`, `cargo build`. Regenerate gifs: `cargo run --release -p rackscreen-app --example gifs -- .github/media/screens`; look at a frame of `all.gif` mid-change.

- [ ] **Step 6: Gate and commit**

Run the gate.

```bash
git add -A crates README.md config.example.yaml Cargo.toml Cargo.lock .github/media/screens
git commit -m "feat(render): vertical slide between screens; docs, gifs, version 0.8.1

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review

Spec coverage: tokens, pill, past bars, floor rule and test (Task 1); group-centred hero, corner fit with margin 10, glide 0.4, `hero_scale` as ceiling, worst-case ring test (Task 2); `slide(t)` numbers, 0.75 s, `Placement`/`Slide`, outgoing state, connecting half kept (Task 3); renderer transform, alpha, round mask, off-screen skip, golden, bench under 2× (Task 4). Deviation: pill alpha 0.30 instead of 0.26 to meet the spec's own floor. Types: `Placement { dy, scale, alpha }`, `Slide { out, out_at, inc, inc_at }`, `ScreenUi::outgoing(Role)`, `hero_fit_scale(&str, Option<&str>, f32, f32)` used consistently.

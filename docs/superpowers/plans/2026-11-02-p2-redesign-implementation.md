# P2 Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the tick-ring / icon / badge look of all 22 roles with P2 "Tiles" (Inter typography, a 300° round-capped ring with a curved title, one rolling hero number, a pill and a 108×48 card with a per-role chart), turn the face into a pure vector mood display, drop splashes, sweeps and the act system, and add the history and forecast data the new charts need.

**Architecture:** A new `rackscreen_core::p2` module holds the fonts and metrics (`font.rs`), geometry (`layout.rs`), motion primitives (`roll.rs`, `marquee.rs`, `ui.rs`), the ring and card chart builders (`ring.rs`, `chart.rs`), text helpers (`text.rs`) and the composer (`screen.rs`: plain-data `P2Screen` → `Scene`). Role builders are small `Model → P2Screen` functions under `p2/roles/`. `core::scene::Drawable` gains vector primitives (`Arc`, `RoundRect`, `Polygon`, `Ellipse`, `Polyline`, `Run`, `CurvedText`) that `render` rasterises with tiny-skia; per-screen widget state lives in a `ScreenUi` the run loop owns and passes into `Model::scene`. Legacy drawables coexist with the new ones until Task 15 removes them.

**Tech Stack:** Rust 2021 workspace (`rackscreen-core`, `rackscreen-render`, `rackscreen-sources`, `rackscreen-app`, `rackscreen-setup`), `fontdue 0.9` (now also in core), `tiny-skia 0.12`, `serde_yaml_ng`, `tokio`, golden PNG tests, `gif` example for README media. Inter v4.0 static TTFs (OFL).

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-p2-redesign-design.md` (the contract; its "Decisions added while planning" section wins over earlier sections). Visual/timing reference: the gitignored mockups in `.superpowers/brainstorm/491126-1793618315/content/` (`p2-v4.html`, `p2-rest-v2.html`, `storage-card.html`, `github-card.html`, `power-mix-badge.html`, `face-moods-v2.html`, `marquee.html`, `jellyfin-final.html`).
- Screen is 240 × 240, centre `(120, 120)`. Ring radius `106`, stroke `9`, round caps, sweep `300°` starting `30°` clockwise of 12 o'clock. Hero baseline `y = 100` (`104` with a unit below), Bold `64 px × hero_scale`, must fit `180 px`. Pill height `20`, radius `10`, centre `y = 126` (`134` with a unit below), width = text + `20`, text at most `132 px` then marquee, fill = pill colour at `16 %` alpha. Card `108 × 48`, radius `14`, top = pill centre + `18`, chart inset `10` left/right, `8` top, `14` bottom (chart area `88 × 26`). Curved title Bold `10 px`, letter spacing `1.5`, `TEXT2`, uppercase, radius `106`, centred at 12 o'clock.
- Colour tokens (`theme::p2`): `BG #000000`, `TEXT #EDEDEF`, `TEXT2 #8E8E93`, `TEXT3 #4A4A4F`, `TRACK #17171A`, `CARD #121214`, `OK #5BD18B`, `WARN #F2B84B`, `CRIT #F0605D`, `BLUE #6EA8FE`, `VIOLET #A48BFA`, `TEAL #5CC8C3`, `INDIGO #8C9CFF`, `CYAN #67D1EE`, `AMBER #F2B84B`, `GREEN #4CC38A`. No gradients, blur or glow anywhere; no icons on data screens except the power-mix capsules.
- Motion: value glides `0.65 s` ease-out cubic; rolling digits `0.5 s` ease-out quart, `30 ms` stagger per character, travel `0.82 ×` size, a character keeps its old glyph until its own start; marquee holds `1.5 s`, scrolls `30 px/s`, gap `36 px`, hard clip; segment colour cross-fade `0.5 s`, `20 ms` stagger; power-mix rotation `1.0 s` ease-out cubic; mood tween `0.42 s` ease-in-out cubic; sparkline scroll over the first `30 %` of the poll period, ease-in-out cubic.
- Fonts: Inter 4.0 static `Medium`, `SemiBold`, `Bold` (`extras/ttf/` of `https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip`, sha256 `ff970a5d4561a04f102a7cb781adbd6ac4e9b6c460914c7a101f15acb7f7d1a4`) embedded once, in core; render reads the bytes from core. Digits are laid out in cells as wide as `0`.
- Performance: scene + render per role under `1 ms` on the desktop in release (4 × margin on the Pi 3B+ budget of 4 ms).
- `rackscreen-core` gains exactly one dependency: `fontdue = "0.9"`. No `chrono`, no `rand` in core.
- Workspace version becomes `0.7.0` (Task 16). Config: the `face:` block is removed; old configs that carry it load with one warning.
- After every task: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace` green (goldens re-captured with `UPDATE_GOLDENS=1` where a task changes pixels on purpose, and the PNGs committed with the task).
- Commit messages: `type(scope): summary` as in `git log`, ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## File structure

| File | Responsibility |
|---|---|
| `assets/fonts/Inter-{Medium,SemiBold,Bold}.ttf`, `assets/fonts/Inter-LICENSE.txt` (new) | the three embedded faces |
| `crates/core/src/p2/mod.rs` (new) | module list |
| `crates/core/src/p2/font.rs` (new) | `FontId`, embedded bytes, `OnceLock`'d fontdue fonts, `advance`, `measure`, `digit_width`, `cap_height` |
| `crates/core/src/p2/layout.rs` (new) | geometry constants, `card_top`, `chart_rect` |
| `crates/core/src/p2/roll.rs` (new) | `Roll` (rolling digits with the stagger fix) |
| `crates/core/src/p2/marquee.rs` (new) | `Marquee` (hold, scroll, wrap) |
| `crates/core/src/p2/ui.rs` (new) | `ScreenUi`: glides, rolls, marquees, colour fades, keyed by `(&'static str, u32)` |
| `crates/core/src/p2/ring.rs` (new) | `RingKind`, `RingSpec` → `Drawable::Arc`s |
| `crates/core/src/p2/text.rs` (new) | curved title, hero (+ unit), pill (+ marquee), ring end labels |
| `crates/core/src/p2/chart.rs` (new) | `Card` kinds → drawables |
| `crates/core/src/p2/screen.rs` (new) | `P2Screen`, `compose` |
| `crates/core/src/p2/status.rs` (new, Task 15) | no-data, token, location and connecting screens |
| `crates/core/src/p2/roles/{mod,cluster,system,energy,sky,github,power_mix}.rs` (new) | `Model → P2Screen` per role; the power-mix scene |
| `crates/core/src/history.rs` (new) | `History` (fixed step ring buffer) and `Hourly` (24 hour slots) |
| `crates/core/src/sky.rs` (new) | moon days-to-full/new, next sun event, daylight |
| `crates/core/src/face.rs` (new) | `Face`: mood tween + idle life → `FaceFrame` |
| `crates/core/src/{mood,face_expr,scene_face}.rs` (rewritten) | priority mood table; expression table and idle; vector eyes |
| `crates/core/src/{scene,anim,theme,model,screens,format,electricity,event}.rs` (modified) | drawables, `OutQuart`, tokens/titles, history + deploy log + plumbing, zoom-only transitions, formatters, P2 band colours, new events |
| `crates/render/src/{text,prims,renderer,assets}.rs` (modified) | three Inter faces, `draw_run`, `draw_curved`, arc/polygon/ellipse/polyline paths |
| `crates/sources/src/{prometheus,open_meteo,argocd,fake}.rs` (modified) | range prefill, hourly forecast + AQI history, sync history, volume sizes, fake data |
| `crates/app/src/{runloop,config,run,calibrate}.rs`, `crates/app/examples/gifs.rs` (modified) | `ScreenUi` per screen, `face:` removal with warning, P2 test pattern, new scripts |
| `crates/setup/src/screens/configure.rs` (modified) | face fields removed |
| `crates/render/tests/{golden,bench}.rs` (modified/new) | goldens for every role and state, bench |
| `README.md`, `config.example.yaml`, `Cargo.toml`, `scripts/fetch-assets.sh` (modified) | docs, example config, version, font download |

Shared signatures every task relies on (all `pub`):

```rust
// core::p2::font
pub enum FontId { Medium, SemiBold, Bold }
pub fn bytes(id: FontId) -> &'static [u8];
pub fn advance(id: FontId, px: f32, ch: char) -> f32;
pub fn measure(id: FontId, px: f32, text: &str, letter_spacing: f32) -> f32;
pub fn digit_width(id: FontId, px: f32) -> f32;
pub fn cap_height(id: FontId, px: f32) -> f32;
// core::p2::ui
pub type UiKey = (&'static str, u32);
impl ScreenUi {
    pub fn new() -> Self;
    pub fn for_role(&mut self, role: Role);
    pub fn glide(&mut self, key: UiKey, target: f32, now: Secs) -> f32;              // 0.65 s
    pub fn glide_with(&mut self, key: UiKey, target: f32, now: Secs, duration: Secs) -> f32;
    pub fn glide_angle(&mut self, key: UiKey, target_deg: f32, now: Secs, duration: Secs) -> f32;
    pub fn roll(&mut self, key: UiKey, text: &str, now: Secs) -> &Roll;
    pub fn marquee(&mut self, key: UiKey, text: &str, now: Secs) -> &Marquee;
    pub fn fade(&mut self, key: UiKey, color: Color, now: Secs, delay: Secs) -> Vec<(Color, f32)>;
}
// core::p2::screen
pub struct P2Screen { title, hero, hero_scale, hero_color, unit, unit_below, ring, ends, pill, card, live }
pub fn compose(s: &P2Screen, ui: &mut ScreenUi, now: Secs) -> Scene;
// core::model
impl Model {
    pub fn scene(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> Scene;
    pub fn scene_for_role(&self, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene;
}
```

---

### Task 1: Inter fonts, core font metrics, render multi-font `draw_run`

**Files:**
- Create: `assets/fonts/Inter-Medium.ttf`, `assets/fonts/Inter-SemiBold.ttf`, `assets/fonts/Inter-Bold.ttf`, `assets/fonts/Inter-LICENSE.txt`
- Create: `crates/core/src/p2/mod.rs`, `crates/core/src/p2/font.rs`
- Modify: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/scene.rs` (add `ClipRect`), `scripts/fetch-assets.sh`
- Modify: `crates/render/src/text.rs`
- Test: `crates/core/src/p2/font.rs` (unit), `crates/render/src/text.rs` (unit)

**Interfaces:**
- Produces: `rackscreen_core::p2::font::{FontId, bytes, font, advance, measure, digit_width, cap_height}`; `rackscreen_core::scene::ClipRect { x, y, w, h }`; `rackscreen_render::text::TextRenderer::draw_run(pix, text, font, px, x, baseline, letter_spacing, color, alpha, clip)`. The legacy `draw_centered` (JetBrains Mono) stays until Task 15.

- [ ] **Step 1: Fetch the fonts**

```bash
cd /home/silke/Documents/GitHub/RackScreen
SCRATCH=$(mktemp -d)
ZIP=/tmp/claude-1000/-home-silke-Documents-GitHub-RackScreen/5365eb52-18b9-4e6e-b9b8-84db09f4a9c5/scratchpad/inter/inter.zip
[ -f "$ZIP" ] || curl -sSL https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip -o "$SCRATCH/inter.zip" && ZIP="$SCRATCH/inter.zip"
echo "ff970a5d4561a04f102a7cb781adbd6ac4e9b6c460914c7a101f15acb7f7d1a4  $ZIP" | sha256sum -c -
unzip -q -o "$ZIP" extras/ttf/Inter-Medium.ttf extras/ttf/Inter-SemiBold.ttf extras/ttf/Inter-Bold.ttf LICENSE.txt -d "$SCRATCH"
cp "$SCRATCH"/extras/ttf/Inter-Medium.ttf "$SCRATCH"/extras/ttf/Inter-SemiBold.ttf "$SCRATCH"/extras/ttf/Inter-Bold.ttf assets/fonts/
cp "$SCRATCH/LICENSE.txt" assets/fonts/Inter-LICENSE.txt
ls -la assets/fonts/
```

Expected: `inter.zip: OK` and four new files (each TTF about 410 KiB).

Append to `scripts/fetch-assets.sh` before the final `echo "assets ok"`:

```bash
curl -sSL "https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip" -o "$TMP/inter.zip"
echo "ff970a5d4561a04f102a7cb781adbd6ac4e9b6c460914c7a101f15acb7f7d1a4  $TMP/inter.zip" | sha256sum -c -
unzip -q -o "$TMP/inter.zip" extras/ttf/Inter-Medium.ttf extras/ttf/Inter-SemiBold.ttf extras/ttf/Inter-Bold.ttf LICENSE.txt -d "$TMP"
cp "$TMP"/extras/ttf/Inter-Medium.ttf "$TMP"/extras/ttf/Inter-SemiBold.ttf "$TMP"/extras/ttf/Inter-Bold.ttf assets/fonts/
cp "$TMP/LICENSE.txt" assets/fonts/Inter-LICENSE.txt
```

(The script's `rm -rf "$TMP"` already follows; move it after these lines.)

- [ ] **Step 2: Write the failing font test**

Add `fontdue = "0.9"` under `[dependencies]` in `crates/core/Cargo.toml`. Add `pub mod p2;` to `crates/core/src/lib.rs` (alphabetically after `pub mod night;`). Create `crates/core/src/p2/mod.rs`:

```rust
//! The P2 "Tiles" look: fonts, layout, motion primitives and the screen composer.
pub mod font;
```

Create `crates/core/src/p2/font.rs` with only the test module for now:

```rust
//! The three embedded Inter instances and read-only metrics, so core can lay
//! text out (centre, pill width, marquee overflow, roll cells) without drawing.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_metrics_are_sane() {
        assert!((digit_width(FontId::Bold, 64.0) - advance(FontId::Bold, 64.0, '0')).abs() < 1e-6);
        let w = measure(FontId::Bold, 64.0, "42", 0.0);
        assert!(w > 60.0 && w < 90.0, "{w}");
        let spaced = measure(FontId::Bold, 10.0, "CPU", 1.5) - measure(FontId::Bold, 10.0, "CPU", 0.0);
        assert!((spaced - 3.0).abs() < 1e-4, "two gaps of 1.5: {spaced}");
        let cap = cap_height(FontId::Bold, 64.0);
        assert!(cap > 40.0 && cap < 50.0, "Inter cap height is 0.727 em: {cap}");
        assert!(advance(FontId::Bold, 20.0, 'M') > advance(FontId::Medium, 20.0, 'M'));
        assert_eq!(measure(FontId::Medium, 10.0, "", 1.0), 0.0);
        for id in FontId::ALL {
            for ch in ['€', '°', '↓', '↑', '·', '—', '₂', '%', '/'] {
                assert!(font(id).has_glyph(ch), "{id:?} lacks {ch}");
            }
            assert!(!bytes(id).is_empty());
        }
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::font`
Expected: compile error, `cannot find function digit_width`.

- [ ] **Step 4: Implement the font module**

Replace the top of `crates/core/src/p2/font.rs` (keep the test module) with:

```rust
//! The three embedded Inter instances and read-only metrics, so core can lay
//! text out (centre, pill width, marquee overflow, roll cells) without drawing.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId {
    Medium,
    SemiBold,
    Bold,
}

impl FontId {
    pub const ALL: [FontId; 3] = [FontId::Medium, FontId::SemiBold, FontId::Bold];

    pub fn index(self) -> usize {
        match self {
            FontId::Medium => 0,
            FontId::SemiBold => 1,
            FontId::Bold => 2,
        }
    }
}

pub const INTER_MEDIUM: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Medium.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf");
pub const INTER_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Bold.ttf");

/// The TTF bytes for a face; `render` rasterises from these, nothing is embedded twice.
pub fn bytes(id: FontId) -> &'static [u8] {
    match id {
        FontId::Medium => INTER_MEDIUM,
        FontId::SemiBold => INTER_SEMIBOLD,
        FontId::Bold => INTER_BOLD,
    }
}

fn fonts() -> &'static [Font; 3] {
    static FONTS: OnceLock<[Font; 3]> = OnceLock::new();
    FONTS.get_or_init(|| {
        let load = |b: &'static [u8]| {
            Font::from_bytes(b, FontSettings::default()).expect("embedded Inter parses")
        };
        [load(INTER_MEDIUM), load(INTER_SEMIBOLD), load(INTER_BOLD)]
    })
}

/// The parsed face, for metrics only (drawing is `render`'s job).
pub fn font(id: FontId) -> &'static Font {
    &fonts()[id.index()]
}

/// Advance width of one character in px.
pub fn advance(id: FontId, px: f32, ch: char) -> f32 {
    font(id).metrics(ch, px).advance_width
}

/// Width of a run: advances plus `letter_spacing` between characters.
pub fn measure(id: FontId, px: f32, text: &str, letter_spacing: f32) -> f32 {
    let n = text.chars().count();
    if n == 0 {
        return 0.0;
    }
    let adv: f32 = text.chars().map(|c| advance(id, px, c)).sum();
    adv + letter_spacing * (n as f32 - 1.0)
}

/// The tabular digit cell: as wide as `0`.
pub fn digit_width(id: FontId, px: f32) -> f32 {
    advance(id, px, '0')
}

/// Height of `H` above the baseline in px.
pub fn cap_height(id: FontId, px: f32) -> f32 {
    let m = font(id).metrics('H', px);
    m.height as f32 + m.ymin as f32
}
```

Add to `crates/core/src/scene.rs`, right after the `SegState` enum:

```rust
/// An axis-aligned clip in screen pixels for text runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p rackscreen-core p2::font`
Expected: `test p2::font::tests::inter_metrics_are_sane ... ok`.

- [ ] **Step 6: Write the failing render test**

Append to the `tests` module of `crates/render/src/text.rs`:

```rust
    use rackscreen_core::p2::font::FontId;
    use rackscreen_core::scene::ClipRect;

    #[test]
    fn draw_run_places_left_edge_and_baseline() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(120, 60).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_run(&mut p, "H", FontId::Bold, 20.0, 10.0, 40.0, 0.0, WHITE, 1.0, None);
        let (x0, _, _, y1) = lit_bbox(&p).expect("drawn");
        assert!((10..=13).contains(&x0), "left edge near x: {x0}");
        assert!((38..=40).contains(&y1), "bottom on the baseline: {y1}");
    }

    #[test]
    fn draw_run_letter_spacing_and_clip() {
        let mut t = TextRenderer::new().unwrap();
        let mut a = Pixmap::new(120, 60).unwrap();
        a.fill(tiny_skia::Color::BLACK);
        t.draw_run(&mut a, "HH", FontId::Bold, 20.0, 10.0, 40.0, 0.0, WHITE, 1.0, None);
        let mut b = Pixmap::new(120, 60).unwrap();
        b.fill(tiny_skia::Color::BLACK);
        t.draw_run(&mut b, "HH", FontId::Bold, 20.0, 10.0, 40.0, 4.0, WHITE, 1.0, None);
        let wa = lit_bbox(&a).unwrap().2;
        let wb = lit_bbox(&b).unwrap().2;
        assert_eq!(wb - wa, 4, "one gap of 4 px");
        let mut c = Pixmap::new(120, 60).unwrap();
        c.fill(tiny_skia::Color::BLACK);
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            w: 16.0,
            h: 60.0,
        };
        t.draw_run(&mut c, "HH", FontId::Bold, 20.0, 10.0, 40.0, 0.0, WHITE, 1.0, Some(clip));
        assert!(lit_bbox(&c).unwrap().2 <= 15, "clipped at x = 16");
        t.draw_run(&mut c, "HH", FontId::Medium, 20.0, 10.0, 40.0, 0.0, WHITE, 0.0, None);
    }
```

- [ ] **Step 7: Run to verify it fails**

Run: `cargo test -p rackscreen-render text::tests`
Expected: compile error, `no method named draw_run`.

- [ ] **Step 8: Implement `draw_run`**

In `crates/render/src/text.rs`, change the imports and struct:

```rust
use std::collections::HashMap;

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings, Metrics};
use rackscreen_core::p2::font::{bytes, FontId};
use rackscreen_core::scene::ClipRect;
use rackscreen_core::theme::Color;
use tiny_skia::Pixmap;

use crate::assets::FONT_BOLD;

pub struct TextRenderer {
    /// JetBrains Mono, for the legacy centred badge text (removed in the P2 cleanup).
    font: Font,
    /// Inter Medium, SemiBold, Bold in `FontId` order.
    inter: [Font; 3],
    cache: HashMap<(char, u32), (Metrics, Vec<u8>)>,
    /// Glyphs keyed by face, character and size in half pixels.
    run_cache: HashMap<(FontId, char, u32), (Metrics, Vec<u8>)>,
}
```

Replace `new` and add the glyph helper and `draw_run`:

```rust
impl TextRenderer {
    pub fn new() -> Result<Self> {
        let font = Font::from_bytes(FONT_BOLD, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("font: {e}"))
            .context("load embedded font")?;
        let load = |id: FontId| {
            Font::from_bytes(bytes(id), FontSettings::default())
                .map_err(|e| anyhow::anyhow!("inter {id:?}: {e}"))
        };
        let inter = [load(FontId::Medium)?, load(FontId::SemiBold)?, load(FontId::Bold)?];
        Ok(Self {
            font,
            inter,
            cache: HashMap::new(),
            run_cache: HashMap::new(),
        })
    }

    /// Size quantised to half pixels, so a rolling hero does not fill the cache.
    fn half_px(px: f32) -> u32 {
        (px * 2.0).round().max(1.0) as u32
    }

    fn inter_glyph(&mut self, id: FontId, ch: char, px: f32) -> (Metrics, Vec<u8>) {
        let key = (id, ch, Self::half_px(px));
        if let Some(g) = self.run_cache.get(&key) {
            return g.clone();
        }
        let g = self.inter[id.index()].rasterize(ch, key.2 as f32 / 2.0);
        self.run_cache.insert(key, g.clone());
        g
    }

    /// Draw `text` with its left edge at `x` and its baseline at `baseline`,
    /// `letter_spacing` px between characters, optionally clipped.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_run(
        &mut self,
        pix: &mut Pixmap,
        text: &str,
        font: FontId,
        px: f32,
        x: f32,
        baseline: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
        clip: Option<ClipRect>,
    ) {
        if alpha <= 0.0 || text.is_empty() {
            return;
        }
        let pw = pix.width() as i32;
        let ph = pix.height() as i32;
        let (cx0, cy0, cx1, cy1) = match clip {
            Some(c) => (
                (c.x.floor() as i32).max(0),
                (c.y.floor() as i32).max(0),
                ((c.x + c.w).ceil() as i32).min(pw),
                ((c.y + c.h).ceil() as i32).min(ph),
            ),
            None => (0, 0, pw, ph),
        };
        let base = baseline.round() as i32;
        let mut pen = x;
        for ch in text.chars() {
            let (m, bitmap) = self.inter_glyph(font, ch, px);
            let gx = pen.round() as i32 + m.xmin;
            let gtop = base - (m.height as i32 + m.ymin);
            let data = pix.data_mut();
            for row in 0..m.height as i32 {
                let y = gtop + row;
                if y < cy0 || y >= cy1 {
                    continue;
                }
                for col in 0..m.width as i32 {
                    let xx = gx + col;
                    if xx < cx0 || xx >= cx1 {
                        continue;
                    }
                    let cov = bitmap[row as usize * m.width + col as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = (cov as f32 / 255.0) * alpha * (color.a as f32 / 255.0);
                    let i = ((y * pw + xx) * 4) as usize;
                    blend(&mut data[i..i + 4], color, a);
                }
            }
            pen += m.advance_width + letter_spacing;
        }
    }
```

Keep the existing `glyph`, `layout`, `measure`, `draw_centered` and `blend` as they are.

- [ ] **Step 9: Run to verify it passes**

Run: `cargo test -p rackscreen-render text::tests`
Expected: 5 tests pass (3 old, 2 new).

- [ ] **Step 10: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add assets/fonts scripts/fetch-assets.sh crates/core/Cargo.toml Cargo.lock crates/core/src/lib.rs crates/core/src/p2 crates/core/src/scene.rs crates/render/src/text.rs
git commit -m "feat(core,render): embed Inter in core with fontdue metrics; multi-font draw_run

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Vector drawables and their rasterisation

**Files:**
- Modify: `crates/core/src/scene.rs` (`Drawable` variants, `FontId` re-export)
- Modify: `crates/render/src/prims.rs` (arc/polyline/polygon/ellipse paths, `stroke`)
- Modify: `crates/render/src/text.rs` (`draw_curved`)
- Modify: `crates/render/src/renderer.rs` (new match arms)
- Test: `crates/render/src/renderer.rs` (unit), `crates/render/src/prims.rs` (unit)

**Interfaces:**
- Consumes: `FontId`, `ClipRect`, `TextRenderer::draw_run` from Task 1.
- Produces: `Drawable::{Arc, RoundRect, Polygon, Ellipse, Polyline, Run, CurvedText}` (fields below), `prims::{arc_points, polyline_path, polygon_path, ellipse_path, stroke_round, stroke}`, `TextRenderer::draw_curved`.

- [ ] **Step 1: Write the failing renderer tests**

Append to the `tests` module in `crates/render/src/renderer.rs`:

```rust
    use rackscreen_core::p2::font::FontId;
    use rackscreen_core::theme::WHITE;

    fn render_one(d: Drawable) -> Pixmap {
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = Scene::new();
        s.push(d);
        r.render(&s, &mut px);
        px
    }

    fn lit(p: &Pixmap, x: u32, y: u32) -> bool {
        pixel(p, x, y).0 > 60
    }

    #[test]
    fn arc_runs_clockwise_from_twelve_with_round_caps() {
        let px = render_one(Drawable::Arc {
            cx: CX,
            cy: CY,
            r: 100.0,
            width: 8.0,
            start_deg: 0.0,
            sweep_deg: 90.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&px, 120, 20), "start at 12 o'clock");
        assert!(lit(&px, 191, 49), "half way, 45 degrees");
        assert!(lit(&px, 220, 120), "end at 3 o'clock");
        assert!(!lit(&px, 120, 220), "nothing at 6 o'clock");
        assert!(!lit(&px, 20, 120), "nothing at 9 o'clock");
        assert!(lit(&px, 117, 20), "round cap reaches a little past the start");
    }

    #[test]
    fn filled_shapes_cover_their_interior_only() {
        let rr = render_one(Drawable::RoundRect {
            x: 20.0,
            y: 20.0,
            w: 60.0,
            h: 30.0,
            r: 10.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&rr, 50, 35));
        assert!(!lit(&rr, 20, 20), "corner rounded away");
        let poly = render_one(Drawable::Polygon {
            points: vec![(120.0, 20.0), (220.0, 120.0), (20.0, 120.0)],
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&poly, 120, 100));
        assert!(!lit(&poly, 30, 40));
        let ell = render_one(Drawable::Ellipse {
            cx: 120.0,
            cy: 120.0,
            rx: 40.0,
            ry: 10.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&ell, 150, 120));
        assert!(!lit(&ell, 120, 100), "ten tall, not forty");
        let line = render_one(Drawable::Polyline {
            points: vec![(20.0, 200.0), (120.0, 100.0), (220.0, 200.0)],
            width: 3.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&line, 70, 150));
        assert!(!lit(&line, 120, 200));
    }

    #[test]
    fn run_and_curved_text_land_where_asked() {
        let run = render_one(Drawable::Run {
            x: 100.0,
            baseline: 130.0,
            text: "42".into(),
            font: FontId::Bold,
            px: 30.0,
            letter_spacing: 0.0,
            color: WHITE,
            alpha: 1.0,
            clip: None,
        });
        let n = (100..140)
            .flat_map(|x| (105..131).map(move |y| (x, y)))
            .filter(|&(x, y)| lit(&run, x, y))
            .count();
        assert!(n > 100, "glyph pixels inside the run box: {n}");
        assert!(!(0..240).any(|x| lit(&run, x, 140)), "nothing below the baseline");
        let curved = render_one(Drawable::CurvedText {
            text: "MEMORY".into(),
            radius: 106.0,
            centre_deg: 0.0,
            px: 10.0,
            font: FontId::Bold,
            letter_spacing: 1.5,
            color: WHITE,
            alpha: 1.0,
        });
        let top = (0..240)
            .flat_map(|x| (0..40).map(move |y| (x, y)))
            .filter(|&(x, y)| lit(&curved, x, y))
            .count();
        assert!(top > 60, "title pixels near the top: {top}");
        assert!(
            !(0..240).any(|x| (60..240).any(|y| lit(&curved, x, y))),
            "nothing below y = 60"
        );
        let bottom = render_one(Drawable::CurvedText {
            text: "MEMORY".into(),
            radius: 106.0,
            centre_deg: 180.0,
            px: 10.0,
            font: FontId::Bold,
            letter_spacing: 1.5,
            color: WHITE,
            alpha: 1.0,
        });
        assert!((0..240).any(|x| (200..240).any(|y| lit(&bottom, x, y))));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-render renderer::tests`
Expected: compile error, `no variant named Arc`.

- [ ] **Step 3: Add the drawables**

In `crates/core/src/scene.rs` add `pub use crate::p2::font::FontId;` after the existing `use` lines and append these variants to `Drawable` (after `Text { .. }`):

```rust
    /// A stroked arc with round caps; angles clockwise from 12 o'clock.
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
        start_deg: f32,
        sweep_deg: f32,
        color: Color,
        alpha: f32,
    },
    RoundRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: Color,
        alpha: f32,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        color: Color,
        alpha: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        color: Color,
        alpha: f32,
    },
    /// Open stroked path with round joins and caps.
    Polyline {
        points: Vec<(f32, f32)>,
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// Text placed by core: left edge `x`, baseline `baseline`.
    Run {
        x: f32,
        baseline: f32,
        text: String,
        font: FontId,
        px: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
        clip: Option<ClipRect>,
    },
    /// Text set along a circle around the screen centre, upright at the top,
    /// centred on `centre_deg` (clockwise from 12 o'clock).
    CurvedText {
        text: String,
        radius: f32,
        centre_deg: f32,
        px: f32,
        font: FontId,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
    },
```

- [ ] **Step 4: Add the path helpers**

Append to `crates/render/src/prims.rs` (before the tests), and add `Rect` to the `tiny_skia` import:

```rust
/// Points along an arc at most 2° apart; angles clockwise from 12 o'clock.
pub fn arc_points(cx: f32, cy: f32, r: f32, start_deg: f32, sweep_deg: f32) -> Vec<(f32, f32)> {
    let steps = ((sweep_deg.abs() / 2.0).ceil() as usize).max(1);
    (0..=steps)
        .map(|i| {
            let a = (start_deg + sweep_deg * i as f32 / steps as f32).to_radians();
            (cx + r * a.sin(), cy - r * a.cos())
        })
        .collect()
}

pub fn polyline_path(points: &[(f32, f32)]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    pb.finish()
}

pub fn polygon_path(points: &[(f32, f32)]) -> Option<Path> {
    if points.len() < 3 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for p in &points[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let rect = Rect::from_xywh(cx - rx, cy - ry, (2.0 * rx).max(0.01), (2.0 * ry).max(0.01))?;
    PathBuilder::from_oval(rect)
}

pub fn stroke_round(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

/// Stroke `path` with round caps and joins.
pub fn stroke(px: &mut Pixmap, path: &Path, width: f32, c: Color, alpha: f32) {
    if alpha <= 0.0 || width <= 0.0 {
        return;
    }
    px.stroke_path(
        path,
        &paint(c, alpha),
        &stroke_round(width),
        Transform::identity(),
        None,
    );
}
```

Add a prims test:

```rust
    #[test]
    fn arc_points_are_dense_and_clockwise() {
        let pts = arc_points(120.0, 120.0, 100.0, 0.0, 90.0);
        assert_eq!(pts.len(), 46, "90 degrees in 2 degree steps plus the start");
        assert!((pts[0].0 - 120.0).abs() < 1e-3 && (pts[0].1 - 20.0).abs() < 1e-3);
        let last = pts[45];
        assert!((last.0 - 220.0).abs() < 1e-3 && (last.1 - 120.0).abs() < 1e-3);
        assert!(polyline_path(&pts[..1]).is_none());
        assert!(polygon_path(&pts[..2]).is_none());
        assert!(ellipse_path(10.0, 10.0, 0.0, 0.0).is_some());
    }
```

- [ ] **Step 5: Add `draw_curved`**

In `crates/render/src/text.rs` extend the imports:

```rust
use rackscreen_core::p2::font::{advance, bytes, cap_height, measure, FontId};
use tiny_skia::{BlendMode, FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};
```

Add a field `curved_cache: HashMap<(FontId, char, u32, u32), Option<(Metrics, Pixmap)>>` to `TextRenderer`, initialise it with `HashMap::new()` in `new`, and add:

```rust
    /// A glyph pre-tinted into its own premultiplied pixmap, for rotated drawing.
    fn curved_glyph(&mut self, id: FontId, ch: char, px: f32, color: Color) -> Option<(Metrics, Pixmap)> {
        let rgb = ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32;
        let key = (id, ch, Self::half_px(px), rgb);
        if let Some(g) = self.curved_cache.get(&key) {
            return g.clone();
        }
        let (m, cov) = self.inter_glyph(id, ch, px);
        let built = IntSize::from_wh(m.width as u32, m.height as u32).and_then(|size| {
            let mut data = Vec::with_capacity(cov.len() * 4);
            for c in &cov {
                let a = *c as u32;
                data.push((color.r as u32 * a / 255) as u8);
                data.push((color.g as u32 * a / 255) as u8);
                data.push((color.b as u32 * a / 255) as u8);
                data.push(*c);
            }
            Pixmap::from_vec(data, size).map(|p| (m, p))
        });
        self.curved_cache.insert(key, built.clone());
        built
    }

    /// Text along a circle of `radius` around `(cx, cy)`, each glyph upright
    /// away from the centre, the run centred on `centre_deg` (clockwise from
    /// 12 o'clock) with its cap height centred on the circle.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_curved(
        &mut self,
        pix: &mut Pixmap,
        text: &str,
        font: FontId,
        px: f32,
        cx: f32,
        cy: f32,
        radius: f32,
        centre_deg: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
    ) {
        if alpha <= 0.0 || text.is_empty() || radius <= 0.0 {
            return;
        }
        let total = measure(font, px, text, letter_spacing);
        let cap = cap_height(font, px);
        let mut a = centre_deg.to_radians() - total / 2.0 / radius;
        let paint = PixmapPaint {
            opacity: alpha.clamp(0.0, 1.0),
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Bilinear,
        };
        for ch in text.chars() {
            let adv = advance(font, px, ch);
            let w = adv + letter_spacing;
            let ac = a + w / 2.0 / radius;
            a += w / radius;
            let Some((m, glyph)) = self.curved_glyph(font, ch, px, color) else {
                continue;
            };
            let gx = cx + radius * ac.sin();
            let gy = cy - radius * ac.cos();
            let ts = Transform::from_translate(gx, gy)
                .pre_rotate(ac.to_degrees())
                .pre_translate(
                    m.xmin as f32 - adv / 2.0,
                    -(m.height as f32 + m.ymin as f32) + cap / 2.0,
                );
            pix.draw_pixmap(0, 0, glyph.as_ref(), &paint, ts, None);
        }
    }
```

- [ ] **Step 6: Add the renderer arms**

In `crates/render/src/renderer.rs` extend the prims import:

```rust
use crate::prims::{
    arc_points, circle, circle_stroke, ellipse_path, fill, outline, polygon_path, polyline_path,
    rounded_rect, segment_outline, skia_color, stroke, SegmentCache,
};
```

and append these arms to the `match d` in `draw_item` (after the `Drawable::Text` arm):

```rust
            Drawable::Arc {
                cx,
                cy,
                r,
                width,
                start_deg,
                sweep_deg,
                color,
                alpha,
            } => {
                if let Some(p) = polyline_path(&arc_points(*cx, *cy, *r, *start_deg, *sweep_deg)) {
                    stroke(px, &p, *width, *color, *alpha);
                }
            }
            Drawable::RoundRect {
                x,
                y,
                w,
                h,
                r,
                color,
                alpha,
            } => {
                if *w > 0.0 && *h > 0.0 {
                    fill(px, &rounded_rect(*x, *y, *w, *h, *r), *color, *alpha);
                }
            }
            Drawable::Polygon {
                points,
                color,
                alpha,
            } => {
                if let Some(p) = polygon_path(points) {
                    fill(px, &p, *color, *alpha);
                }
            }
            Drawable::Ellipse {
                cx,
                cy,
                rx,
                ry,
                color,
                alpha,
            } => {
                if let Some(p) = ellipse_path(*cx, *cy, *rx, *ry) {
                    fill(px, &p, *color, *alpha);
                }
            }
            Drawable::Polyline {
                points,
                width,
                color,
                alpha,
            } => {
                if let Some(p) = polyline_path(points) {
                    stroke(px, &p, *width, *color, *alpha);
                }
            }
            Drawable::Run {
                x,
                baseline,
                text,
                font,
                px: size,
                letter_spacing,
                color,
                alpha,
                clip,
            } => {
                self.text.draw_run(
                    px,
                    text,
                    *font,
                    *size,
                    *x,
                    *baseline,
                    *letter_spacing,
                    *color,
                    *alpha,
                    *clip,
                );
            }
            Drawable::CurvedText {
                text,
                radius,
                centre_deg,
                px: size,
                font,
                letter_spacing,
                color,
                alpha,
            } => {
                self.text.draw_curved(
                    px,
                    text,
                    *font,
                    *size,
                    CX,
                    CY,
                    *radius,
                    *centre_deg,
                    *letter_spacing,
                    *color,
                    *alpha,
                );
            }
```

- [ ] **Step 7: Run to verify it passes**

Run: `cargo test -p rackscreen-render`
Expected: all tests pass, including `arc_runs_clockwise_from_twelve_with_round_caps`, `filled_shapes_cover_their_interior_only`, `run_and_curved_text_land_where_asked`, `arc_points_are_dense_and_clockwise`. If `lit(&px, 191, 49)` fails by a pixel, check `arc_points` uses `sin` for x and `-cos` for y (12 o'clock is up).

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src/scene.rs crates/render/src
git commit -m "feat(core,render): vector drawables (arc, round rect, polygon, ellipse, polyline, run, curved text)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Motion primitives: `Roll`, `Marquee`, glides, `ScreenUi`

**Files:**
- Modify: `crates/core/src/anim.rs` (`Easing::OutQuart`)
- Create: `crates/core/src/p2/roll.rs`, `crates/core/src/p2/marquee.rs`, `crates/core/src/p2/ui.rs`
- Modify: `crates/core/src/p2/mod.rs`
- Test: unit tests in each new file

**Interfaces:**
- Consumes: `anim::{Secs, Easing, Smooth}`, `theme::{Color, Role}`.
- Produces: `Roll::{new, set, text, glyphs, rolling}` with `RollGlyph { cell, ch, dy, alpha }` (`dy` in units of the font size); `Marquee::{new, set, text, offset(text_w, slot_w, now) -> Option<f32>}`; `ScreenUi` (signatures in the header) and `UiKey`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/src/anim.rs` tests:

```rust
    #[test]
    fn out_quart_known_value() {
        assert!((Easing::OutQuart.apply(0.5) - 0.9375).abs() < 1e-6);
        assert_eq!(Easing::OutQuart.apply(1.0), 1.0);
    }
```

Create `crates/core/src/p2/roll.rs` with its tests only:

```rust
//! Rolling digits: when the formatted hero changes, only the changed
//! characters roll, staggered from the left, each keeping its old glyph until
//! its own roll starts.

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs_of(r: &Roll, now: f64, cell: usize) -> Vec<RollGlyph> {
        r.glyphs(now).into_iter().filter(|g| g.cell == cell).collect()
    }

    #[test]
    fn a_staggered_cell_shows_its_old_glyph_until_its_start() {
        let mut r = Roll::new("40", 0.0);
        r.set("52", 10.0);
        // cell 1 starts at 10.03: before that it still shows '0', not '2'
        let late = glyphs_of(&r, 10.01, 1);
        assert_eq!(late.len(), 1);
        assert_eq!((late[0].ch, late[0].dy, late[0].alpha), ('0', 0.0, 1.0));
        let early = glyphs_of(&r, 10.01, 0);
        assert_eq!(early.len(), 2, "cell 0 is rolling: old and new glyph");
        assert_eq!(early[0].ch, '4');
        assert_eq!(early[1].ch, '5');
        assert!(r.rolling(10.2));
        assert!(!r.rolling(10.6));
        assert_eq!(glyphs_of(&r, 10.6, 1), vec![RollGlyph { cell: 1, ch: '2', dy: 0.0, alpha: 1.0 }]);
    }

    #[test]
    fn only_changed_characters_roll_and_direction_follows_the_value() {
        let mut r = Roll::new("42", 0.0);
        r.set("47", 1.0);
        assert_eq!(glyphs_of(&r, 1.1, 0).len(), 1, "the 4 stays put");
        let up = glyphs_of(&r, 1.1, 1);
        assert_eq!(up.len(), 2);
        assert!(up[0].dy < 0.0, "old leaves upward when increasing");
        assert!(up[1].dy > 0.0, "new enters from below");
        assert!(up[0].alpha < 1.0 && up[1].alpha > 0.0);
        let mut d = Roll::new("47", 0.0);
        d.set("42", 1.0);
        let down = glyphs_of(&d, 1.1, 1);
        assert!(down[0].dy > 0.0 && down[1].dy < 0.0, "mirrored when decreasing");
        assert!((down[1].dy.abs() - (ROLL_TRAVEL * (1.0 - Easing::OutQuart.apply(0.2)))).abs() < 1e-4);
    }

    #[test]
    fn a_length_change_resets_without_rolling() {
        let mut r = Roll::new("9", 0.0);
        r.set("10", 1.0);
        assert_eq!(r.text(), "10");
        assert_eq!(r.glyphs(1.1).len(), 2);
        assert!(!r.rolling(1.1));
        r.set("10", 2.0);
        assert!(!r.rolling(2.1), "same text is a no-op");
        let mut t = Roll::new("14:32", 0.0);
        t.set("14:31", 5.0);
        assert!(t.rolling(5.2));
        assert_eq!(glyphs_of(&t, 5.2, 2).len(), 1, "the colon never rolls");
    }
}
```

Create `crates/core/src/p2/marquee.rs` with its tests only:

```rust
//! Text wider than its slot holds still, then scrolls left and wraps.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_text_never_moves_and_wide_text_holds_then_scrolls() {
        let m = Marquee::new("ubuntu-24.04.iso", 10.0);
        assert_eq!(m.offset(100.0, 132.0, 11.0), None, "fits");
        assert_eq!(m.offset(200.0, 132.0, 10.0), Some(0.0));
        assert_eq!(m.offset(200.0, 132.0, 11.4), Some(0.0), "still holding at 1.4 s");
        let at = m.offset(200.0, 132.0, 12.5).unwrap();
        assert!((at - 30.0).abs() < 1e-4, "1 s past the hold at 30 px/s: {at}");
        // period = 1.5 + (200 + 36) / 30 = 9.3666 s; just before the wrap the
        // text has scrolled its width plus the gap
        let end = m.offset(200.0, 132.0, 10.0 + 9.36).unwrap();
        assert!((end - (236.0 - 0.2)).abs() < 0.01, "{end}");
        assert_eq!(m.offset(200.0, 132.0, 10.0 + 9.3667), Some(0.0), "wrapped and holding");
    }

    #[test]
    fn new_text_restarts_the_hold() {
        let mut m = Marquee::new("a", 0.0);
        assert!(m.offset(200.0, 132.0, 3.0).unwrap() > 0.0);
        m.set("a", 3.0);
        assert!(m.offset(200.0, 132.0, 3.0).unwrap() > 0.0, "same text keeps scrolling");
        m.set("b", 3.0);
        assert_eq!(m.text(), "b");
        assert_eq!(m.offset(200.0, 132.0, 3.5), Some(0.0));
    }
}
```

Create `crates/core/src/p2/ui.rs` with its tests only:

```rust
//! Per-screen widget state: glides, rolls, marquees and colour fades, keyed by
//! a static name plus an index. The run loop owns one per physical screen and
//! it resets whenever the screen's role changes.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{BLACK, WHITE};

    #[test]
    fn glides_start_at_their_target_and_ease_to_new_ones() {
        let mut ui = ScreenUi::new();
        assert_eq!(ui.glide(("ring", 0), 0.4, 0.0), 0.4, "no glide from zero on first sight");
        assert_eq!(ui.glide(("ring", 0), 0.4, 0.1), 0.4);
        let mid = ui.glide(("ring", 0), 0.8, 1.0);
        assert_eq!(mid, 0.4, "retarget starts where it is");
        let later = ui.glide(("ring", 0), 0.8, 1.3);
        assert!(later > 0.4 && later < 0.8, "{later}");
        assert!((ui.glide(("ring", 0), 0.8, 1.7) - 0.8).abs() < 1e-5, "done after 0.65 s");
        assert_eq!(ui.glide(("ring", 1), 0.1, 1.7), 0.1, "another key is independent");
    }

    #[test]
    fn angle_glides_take_the_short_way_round() {
        let mut ui = ScreenUi::new();
        assert_eq!(ui.glide_angle(("rot", 0), 350.0, 0.0, 1.0), 350.0);
        let v = ui.glide_angle(("rot", 0), 10.0, 1.0, 1.0);
        assert_eq!(v, 350.0);
        let v = ui.glide_angle(("rot", 0), 10.0, 1.5, 1.0);
        assert!(v > 350.0 && v < 370.0, "passes through 360: {v}");
        let v = ui.glide_angle(("rot", 0), 10.0, 2.5, 1.0);
        assert!((v.rem_euclid(360.0) - 10.0).abs() < 1e-3, "{v}");
    }

    #[test]
    fn rolls_marquees_and_fades_are_keyed_and_reset_on_role_change() {
        let mut ui = ScreenUi::new();
        assert_eq!(ui.roll(("hero", 0), "42", 0.0).text(), "42");
        ui.roll(("hero", 0), "43", 1.0);
        assert!(ui.roll(("hero", 0), "43", 1.1).rolling(1.1));
        assert_eq!(ui.marquee(("pill", 0), "abc", 0.0).text(), "abc");
        let first = ui.fade(("seg", 3), WHITE, 0.0, 0.0);
        assert_eq!(first, vec![(WHITE, 1.0)]);
        let layers = ui.fade(("seg", 3), BLACK, 1.0, 0.06);
        assert_eq!(layers, vec![(WHITE, 1.0)], "before its staggered start the old colour shows");
        let layers = ui.fade(("seg", 3), BLACK, 1.3, 0.06);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].0, WHITE);
        assert_eq!(layers[1].0, BLACK);
        assert!((layers[0].1 + layers[1].1 - 1.0).abs() < 1e-5);
        assert_eq!(ui.fade(("seg", 3), BLACK, 2.0, 0.06), vec![(BLACK, 1.0)]);
        ui.for_role(Role::Cpu);
        assert_eq!(ui.role(), Some(Role::Cpu));
        ui.glide(("ring", 0), 0.5, 0.0);
        ui.for_role(Role::Cpu);
        assert_eq!(ui.glide(("ring", 0), 0.9, 0.0), 0.5, "same role keeps state");
        ui.for_role(Role::Mem);
        assert_eq!(ui.glide(("ring", 0), 0.9, 0.0), 0.9, "new role starts fresh");
    }
}
```

Register the modules in `crates/core/src/p2/mod.rs`:

```rust
pub mod font;
pub mod marquee;
pub mod roll;
pub mod ui;
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::`
Expected: compile errors (`Roll`, `Marquee`, `ScreenUi` not found; `OutQuart` not found).

- [ ] **Step 3: Implement**

In `crates/core/src/anim.rs` add the variant and its arm:

```rust
pub enum Easing {
    Linear,
    OutCubic,
    OutQuart,
    InOutCubic,
    Spring,
}
// in apply:
            Easing::OutQuart => 1.0 - (1.0 - t).powi(4),
```

`crates/core/src/p2/roll.rs` (above the tests):

```rust
//! Rolling digits: when the formatted hero changes, only the changed
//! characters roll, staggered from the left, each keeping its old glyph until
//! its own roll starts.

use crate::anim::{Easing, Secs};

pub const ROLL_SECS: Secs = 0.5;
pub const ROLL_STAGGER_SECS: Secs = 0.03;
/// Travel of a glyph during its roll, in units of the font size.
pub const ROLL_TRAVEL: f32 = 0.82;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub cur: char,
    pub prev: Option<char>,
    pub start: Secs,
    pub up: bool,
}

/// One glyph to draw for a cell: `dy` is in units of the font size, positive
/// down; a rolling cell yields two glyphs (old then new).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollGlyph {
    pub cell: usize,
    pub ch: char,
    pub dy: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Roll {
    text: String,
    value: f32,
    cells: Vec<Cell>,
}

fn number_of(text: &str) -> f32 {
    let digits: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    digits.parse().unwrap_or(0.0)
}

impl Roll {
    pub fn new(text: &str, now: Secs) -> Roll {
        Roll {
            text: text.to_string(),
            value: number_of(text),
            cells: text
                .chars()
                .map(|c| Cell {
                    cur: c,
                    prev: None,
                    start: now - ROLL_SECS,
                    up: true,
                })
                .collect(),
        }
    }

    /// Show `text`: a length change resets, otherwise the changed cells roll,
    /// staggered 30 ms per cell from the left, up when the value increased.
    pub fn set(&mut self, text: &str, now: Secs) {
        if text == self.text {
            return;
        }
        if text.chars().count() != self.cells.len() {
            *self = Roll::new(text, now);
            return;
        }
        let value = number_of(text);
        let up = value >= self.value;
        for (i, (cell, ch)) in self.cells.iter_mut().zip(text.chars()).enumerate() {
            if cell.cur != ch {
                cell.prev = Some(cell.cur);
                cell.cur = ch;
                cell.start = now + i as Secs * ROLL_STAGGER_SECS;
                cell.up = up;
            }
        }
        self.text = text.to_string();
        self.value = value;
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn rolling(&self, now: Secs) -> bool {
        self.cells
            .iter()
            .any(|c| c.prev.is_some() && now < c.start + ROLL_SECS)
    }

    pub fn glyphs(&self, now: Secs) -> Vec<RollGlyph> {
        let mut out = Vec::with_capacity(self.cells.len() * 2);
        for (i, c) in self.cells.iter().enumerate() {
            let k = ((now - c.start) / ROLL_SECS) as f32;
            match c.prev {
                Some(p) if k < 0.0 => out.push(RollGlyph {
                    cell: i,
                    ch: p,
                    dy: 0.0,
                    alpha: 1.0,
                }),
                Some(p) if k < 1.0 => {
                    let e = Easing::OutQuart.apply(k);
                    let dir = if c.up { 1.0 } else { -1.0 };
                    out.push(RollGlyph {
                        cell: i,
                        ch: p,
                        dy: -dir * e * ROLL_TRAVEL,
                        alpha: 1.0 - e,
                    });
                    out.push(RollGlyph {
                        cell: i,
                        ch: c.cur,
                        dy: dir * (1.0 - e) * ROLL_TRAVEL,
                        alpha: e,
                    });
                }
                _ => out.push(RollGlyph {
                    cell: i,
                    ch: c.cur,
                    dy: 0.0,
                    alpha: 1.0,
                }),
            }
        }
        out
    }
}
```

`crates/core/src/p2/marquee.rs` (above the tests):

```rust
//! Text wider than its slot holds still, then scrolls left and wraps.

use crate::anim::Secs;

pub const HOLD_SECS: Secs = 1.5;
pub const SPEED_PX_PER_SEC: f32 = 30.0;
pub const GAP_PX: f32 = 36.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Marquee {
    text: String,
    started: Secs,
}

impl Marquee {
    pub fn new(text: &str, now: Secs) -> Marquee {
        Marquee {
            text: text.to_string(),
            started: now,
        }
    }

    /// Changing the text restarts the hold; the same text keeps its phase.
    pub fn set(&mut self, text: &str, now: Secs) {
        if text != self.text {
            self.text = text.to_string();
            self.started = now;
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Pixels the text has scrolled left, or `None` when it fits the slot.
    pub fn offset(&self, text_w: f32, slot_w: f32, now: Secs) -> Option<f32> {
        if text_w <= slot_w {
            return None;
        }
        let period = HOLD_SECS + ((text_w + GAP_PX) / SPEED_PX_PER_SEC) as Secs;
        let p = (now - self.started).max(0.0) % period;
        Some(if p < HOLD_SECS {
            0.0
        } else {
            (p - HOLD_SECS) as f32 * SPEED_PX_PER_SEC
        })
    }
}
```

`crates/core/src/p2/ui.rs` (above the tests):

```rust
//! Per-screen widget state: glides, rolls, marquees and colour fades, keyed by
//! a static name plus an index. The run loop owns one per physical screen and
//! it resets whenever the screen's role changes.

use std::collections::HashMap;

use crate::anim::{Easing, Secs, Smooth};
use crate::p2::marquee::Marquee;
use crate::p2::roll::Roll;
use crate::theme::{Color, Role};

pub type UiKey = (&'static str, u32);

/// Data steps glide over this long, ease-out cubic (`Smooth`'s easing).
pub const GLIDE_SECS: Secs = 0.65;
/// A segment's colour cross-fades over this long.
pub const FADE_SECS: Secs = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorFade {
    cur: Color,
    prev: Option<Color>,
    start: Secs,
}

impl ColorFade {
    /// The layers to draw, bottom first: `(colour, alpha)`.
    pub fn layers(&self, now: Secs) -> Vec<(Color, f32)> {
        match self.prev {
            Some(p) if now < self.start => vec![(p, 1.0)],
            Some(p) if now < self.start + FADE_SECS => {
                let e = Easing::OutCubic.apply(((now - self.start) / FADE_SECS) as f32);
                vec![(p, 1.0 - e), (self.cur, e)]
            }
            _ => vec![(self.cur, 1.0)],
        }
    }
}

#[derive(Debug, Default)]
pub struct ScreenUi {
    role: Option<Role>,
    glides: HashMap<UiKey, Smooth>,
    rolls: HashMap<UiKey, Roll>,
    marquees: HashMap<UiKey, Marquee>,
    fades: HashMap<UiKey, ColorFade>,
}

impl ScreenUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// The role this state belongs to; a different role starts from scratch so
    /// nothing rolls or glides across a role change.
    pub fn for_role(&mut self, role: Role) {
        if self.role != Some(role) {
            *self = Self::default();
            self.role = Some(role);
        }
    }

    pub fn role(&self) -> Option<Role> {
        self.role
    }

    pub fn glide(&mut self, key: UiKey, target: f32, now: Secs) -> f32 {
        self.glide_with(key, target, now, GLIDE_SECS)
    }

    /// First sight of a key shows `target` at once; later changes ease over `duration`.
    pub fn glide_with(&mut self, key: UiKey, target: f32, now: Secs, duration: Secs) -> f32 {
        let s = self
            .glides
            .entry(key)
            .or_insert_with(|| Smooth::new(target, duration));
        s.set(target, now);
        s.value(now)
    }

    /// Like `glide_with` for an angle in degrees: always the short way round.
    /// The returned value is unwrapped (may leave 0..360); callers take `rem_euclid(360)`.
    pub fn glide_angle(&mut self, key: UiKey, target_deg: f32, now: Secs, duration: Secs) -> f32 {
        match self.glides.get_mut(&key) {
            None => {
                self.glides.insert(key, Smooth::new(target_deg, duration));
                target_deg
            }
            Some(s) => {
                let mut delta = (target_deg - s.target()).rem_euclid(360.0);
                if delta > 180.0 {
                    delta -= 360.0;
                }
                s.set(s.target() + delta, now);
                s.value(now)
            }
        }
    }

    pub fn roll(&mut self, key: UiKey, text: &str, now: Secs) -> &Roll {
        let r = self
            .rolls
            .entry(key)
            .or_insert_with(|| Roll::new(text, now));
        r.set(text, now);
        r
    }

    pub fn marquee(&mut self, key: UiKey, text: &str, now: Secs) -> &Marquee {
        let m = self
            .marquees
            .entry(key)
            .or_insert_with(|| Marquee::new(text, now));
        m.set(text, now);
        m
    }

    /// Cross-fade a keyed colour; a change starts `delay` after `now` (the
    /// per-segment stagger). Returns the layers to draw, bottom first.
    pub fn fade(&mut self, key: UiKey, color: Color, now: Secs, delay: Secs) -> Vec<(Color, f32)> {
        let f = self.fades.entry(key).or_insert(ColorFade {
            cur: color,
            prev: None,
            start: now - FADE_SECS,
        });
        if f.cur != color {
            f.prev = Some(f.cur);
            f.cur = color;
            f.start = now + delay;
        }
        f.layers(now)
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p rackscreen-core p2:: anim::`
Expected: all `p2::roll`, `p2::marquee`, `p2::ui` and `anim` tests pass. If `angle_glides_take_the_short_way_round` reports `v` just under 350 at `1.5`, `Smooth::set` was given the wrapped target; it must get `s.target() + delta`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src/anim.rs crates/core/src/p2
git commit -m "feat(core): rolling digits, marquee, glides and ScreenUi

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Colour tokens, titles, P2 layout and ring kinds

**Files:**
- Modify: `crates/core/src/theme.rs` (`theme::p2` tokens, `Role::title`)
- Create: `crates/core/src/p2/layout.rs`, `crates/core/src/p2/ring.rs`
- Modify: `crates/core/src/p2/mod.rs`
- Test: unit tests in `theme.rs` and `ring.rs`

**Interfaces:**
- Consumes: `Drawable::{Arc, Ellipse}` (Task 2).
- Produces: `theme::p2::{BG, TEXT, TEXT2, TEXT3, TRACK, CARD, OK, WARN, CRIT, BLUE, VIOLET, TEAL, INDIGO, CYAN, AMBER, GREEN, NIGHT, MOON, INK}`; `Role::title()`; `p2::layout::*` constants plus `card_top(pill_cy)`, `chart_rect(pill_cy)`; `p2::ring::{RingKind, RingSpec}` with `RingSpec::{new, fill, parts, discrete, bands, empty, gap_frac, arc, point, drawables}`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/src/theme.rs` tests:

```rust
    #[test]
    fn p2_tokens_and_titles() {
        assert_eq!(p2::TEXT, Color::hex(0xEDEDEF));
        assert_eq!(p2::TRACK, Color::hex(0x17171A));
        assert_eq!(p2::WARN, p2::AMBER);
        assert_eq!(Role::Health.title(), "CLUSTER");
        assert_eq!(Role::Deploys.title(), "DEPLOYS TODAY");
        assert_eq!(Role::GhActivity.title(), "CONTRIBUTIONS");
        for r in Role::ALL {
            assert!(!r.title().is_empty());
            assert_eq!(r.title(), r.title().to_uppercase());
        }
    }
```

Create `crates/core/src/p2/ring.rs` with tests only:

```rust
//! Ring kinds (fill, parts, discrete segments, bands with a marker) drawn as
//! round-capped arcs over a `TRACK` arc.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::layout::{EDGE_R, RING_START_DEG, RING_SWEEP_DEG, RING_W};
    use crate::theme::p2::{BLUE, CRIT, OK, TRACK, WARN};

    fn arcs(spec: &RingSpec) -> Vec<(f32, f32, Color, f32)> {
        spec.drawables()
            .into_iter()
            .filter_map(|d| match d {
                Drawable::Arc {
                    start_deg,
                    sweep_deg,
                    color,
                    alpha,
                    r,
                    width,
                    ..
                } => {
                    assert!(r + width / 2.0 <= EDGE_R, "caps stay inside the edge");
                    Some((start_deg, sweep_deg, color, alpha))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fill_is_track_plus_one_arc() {
        let a = arcs(&RingSpec::fill(0.5, BLUE));
        assert_eq!(a.len(), 2);
        assert_eq!(a[0], (RING_START_DEG, RING_SWEEP_DEG, TRACK, 1.0));
        assert_eq!(a[1], (RING_START_DEG, 150.0, BLUE, 1.0));
        assert_eq!(arcs(&RingSpec::fill(7.0, BLUE))[1].1, 300.0, "clamps above one");
        assert!((arcs(&RingSpec::fill(-1.0, BLUE))[1].1 - 300.0 * 0.004).abs() < 1e-4, "a dot at the start, never nothing");
        assert_eq!(arcs(&RingSpec::empty()).len(), 1);
    }

    #[test]
    fn parts_are_consecutive_with_gaps_and_normalise() {
        let spec = RingSpec::parts(vec![(0.5, OK), (0.3, WARN), (0.2, CRIT)]);
        let a = arcs(&spec);
        assert_eq!(a.len(), 4);
        let gap = spec.gap_frac(1.3) * RING_SWEEP_DEG;
        assert!((a[1].0 - RING_START_DEG).abs() < 1e-4, "first run starts at the start");
        assert!((a[1].1 - (150.0 - gap / 2.0)).abs() < 1e-3);
        assert!((a[2].0 - (RING_START_DEG + 150.0 + gap / 2.0)).abs() < 1e-3);
        assert!((a[2].1 - (90.0 - gap)).abs() < 1e-3);
        assert_eq!(a[3].2, CRIT);
        let over = arcs(&RingSpec::parts(vec![(1.0, OK), (1.0, WARN)]));
        assert!((over[1].1 + over[2].1 + 1.5 * gap - 300.0).abs() < 1e-2, "sum over one is normalised");
        assert_eq!(arcs(&RingSpec::parts(vec![(0.0, OK)])).len(), 1, "empty parts draw nothing");
    }

    #[test]
    fn discrete_segments_are_equal_and_coloured() {
        let colors = vec![OK, OK, CRIT, OK, OK, OK, OK];
        let spec = RingSpec::discrete(colors.clone());
        let a = arcs(&spec);
        assert_eq!(a.len(), 8);
        let gap = spec.gap_frac(1.6) * RING_SWEEP_DEG;
        for (i, seg) in a[1..].iter().enumerate() {
            assert!((seg.1 - (300.0 / 7.0 - gap)).abs() < 1e-3);
            assert_eq!(seg.2, colors[i]);
        }
        assert!(gap > 0.0 && gap < 10.0, "{gap}");
    }

    #[test]
    fn bands_dim_all_but_the_current_and_place_a_marker() {
        let colors = vec![OK, WARN, CRIT];
        let spec = RingSpec::bands(colors.clone(), 0.5);
        let a = arcs(&spec);
        // track, 3 bands, then the marker ring (a full 360 arc)
        assert_eq!(a.len(), 5);
        assert_eq!(a[1].3, 0.3);
        assert_eq!(a[2].3, 1.0, "band 1 holds 0.5");
        assert_eq!(a[3].3, 0.3);
        assert_eq!((a[4].1, a[4].2), (360.0, WARN));
        let (mx, my) = spec.point(0.5);
        let ell: Vec<(f32, f32, f32)> = spec
            .drawables()
            .into_iter()
            .filter_map(|d| match d {
                Drawable::Ellipse { cx, cy, rx, .. } => Some((cx, cy, rx)),
                _ => None,
            })
            .collect();
        assert_eq!(ell.len(), 2);
        assert!((ell[0].0 - mx).abs() < 1e-4 && (ell[0].1 - my).abs() < 1e-4);
        assert!((ell[0].2 - 0.95 * RING_W).abs() < 1e-4 && (ell[1].2 - 0.5 * RING_W).abs() < 1e-4);
        assert!(((mx - 120.0).powi(2) + (my - 120.0).powi(2)).sqrt() + 0.95 * RING_W <= EDGE_R);
        let top = arcs(&RingSpec::bands(colors, 1.5));
        assert_eq!(top[3].3, 1.0, "clamps into the last band");
    }
}
```

Register in `crates/core/src/p2/mod.rs`: add `pub mod layout;` and `pub mod ring;`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core theme::tests::p2_tokens_and_titles p2::ring`
Expected: compile errors (`p2` module missing in theme, `RingSpec` not found).

- [ ] **Step 3: Implement the tokens and titles**

Append to `crates/core/src/theme.rs` before `pub mod layout`:

```rust
/// The P2 "Tiles" palette.
pub mod p2 {
    use super::Color;
    pub const BG: Color = Color::hex(0x000000);
    pub const TEXT: Color = Color::hex(0xEDEDEF);
    pub const TEXT2: Color = Color::hex(0x8E8E93);
    pub const TEXT3: Color = Color::hex(0x4A4A4F);
    pub const TRACK: Color = Color::hex(0x17171A);
    pub const CARD: Color = Color::hex(0x121214);
    pub const OK: Color = Color::hex(0x5BD18B);
    pub const WARN: Color = Color::hex(0xF2B84B);
    pub const CRIT: Color = Color::hex(0xF0605D);
    pub const BLUE: Color = Color::hex(0x6EA8FE);
    pub const VIOLET: Color = Color::hex(0xA48BFA);
    pub const TEAL: Color = Color::hex(0x5CC8C3);
    pub const INDIGO: Color = Color::hex(0x8C9CFF);
    pub const CYAN: Color = Color::hex(0x67D1EE);
    pub const AMBER: Color = Color::hex(0xF2B84B);
    pub const GREEN: Color = Color::hex(0x4CC38A);
    /// The dark half of the sun ring.
    pub const NIGHT: Color = Color::hex(0x2A2A33);
    pub const MOON: Color = Color::hex(0xD6D6E0);
    /// Capsule icon and text on the power-mix ring.
    pub const INK: Color = Color::hex(0x0B0B0C);
}
```

Add to `impl Role` (after `name`):

```rust
    /// The curved title in the P2 look (upper case); `weather` and `sun` show
    /// a live title instead and use this one only while waiting for data.
    pub fn title(self) -> &'static str {
        match self {
            Role::Cpu => "CPU",
            Role::Mem => "MEMORY",
            Role::Pods => "PODS",
            Role::Health => "CLUSTER",
            Role::Thermal => "THERMAL",
            Role::Storage => "STORAGE",
            Role::PowerMix => "POWER MIX",
            Role::Price => "ELECTRICITY",
            Role::Carbon => "CARBON",
            Role::Renewable => "RENEWABLE",
            Role::GhActivity => "CONTRIBUTIONS",
            Role::Weather => "WEATHER",
            Role::Wind => "WIND",
            Role::Aqi => "AIR QUALITY",
            Role::Rain => "RAIN",
            Role::Sun => "SUN",
            Role::Moon => "MOON",
            Role::Iss => "ISS PASS IN",
            Role::Ups => "UPS",
            Role::Net => "NETWORK",
            Role::Deploys => "DEPLOYS TODAY",
            Role::Face => "FACE",
        }
    }
```

- [ ] **Step 4: Implement layout and rings**

Create `crates/core/src/p2/layout.rs`:

```rust
//! P2 geometry on the 240 × 240 screen, centre (120, 120).

use crate::theme::layout::{CX, CY};

pub const RING_R: f32 = 106.0;
pub const RING_W: f32 = 9.0;
/// The ring sweeps clockwise from 30° right of 12 o'clock round to 30° left of it.
pub const RING_START_DEG: f32 = 30.0;
pub const RING_SWEEP_DEG: f32 = 300.0;
/// Nothing is drawn past this radius: the panel's visible edge.
pub const EDGE_R: f32 = 116.0;

pub const TITLE_R: f32 = 106.0;
pub const TITLE_PX: f32 = 10.0;
pub const TITLE_LS: f32 = 1.5;
/// A title wider than this (the 60° gap is 111 px of arc) uses its first word.
pub const TITLE_MAX_W: f32 = 104.0;

pub const HERO_PX: f32 = 64.0;
pub const HERO_BASELINE: f32 = 100.0;
pub const HERO_BASELINE_UNIT_BELOW: f32 = 104.0;
pub const HERO_MAX_W: f32 = 180.0;
/// Superscript unit size relative to the hero size.
pub const UNIT_SCALE: f32 = 0.36;
pub const UNIT_GAP: f32 = 2.0;
pub const UNIT_BELOW_PX: f32 = 10.5;
/// Baseline of a unit word below the hero, relative to the hero baseline.
pub const UNIT_BELOW_DY: f32 = 18.0;

pub const PILL_CY: f32 = 126.0;
pub const PILL_CY_UNIT_BELOW: f32 = 134.0;
pub const PILL_H: f32 = 20.0;
pub const PILL_R: f32 = 10.0;
pub const PILL_PAD: f32 = 20.0;
pub const PILL_PX: f32 = 10.5;
pub const PILL_MAX_TEXT_W: f32 = 132.0;
pub const PILL_FILL_ALPHA: f32 = 0.16;

pub const CARD_W: f32 = 108.0;
pub const CARD_H: f32 = 48.0;
pub const CARD_R: f32 = 14.0;
/// Card top relative to the pill centre.
pub const CARD_DY: f32 = 18.0;
pub const CHART_INSET_X: f32 = 10.0;
pub const CHART_INSET_TOP: f32 = 8.0;
pub const CHART_INSET_BOTTOM: f32 = 14.0;
pub const CAPTION_PX: f32 = 8.5;

/// Centre of the health role's live dot, between the card and the ring.
pub const LIVE_CY: f32 = 204.0;
/// Radius and size of the scale-end labels at the two ends of the ring.
pub const ENDS_R: f32 = 90.0;
pub const ENDS_PX: f32 = 8.5;

pub fn card_top(pill_cy: f32) -> f32 {
    pill_cy + CARD_DY
}

/// `(x, y, w, h)` of the chart area inside the card.
pub fn chart_rect(pill_cy: f32) -> (f32, f32, f32, f32) {
    let top = card_top(pill_cy);
    (
        CX - CARD_W / 2.0 + CHART_INSET_X,
        top + CHART_INSET_TOP,
        CARD_W - 2.0 * CHART_INSET_X,
        CARD_H - CHART_INSET_TOP - CHART_INSET_BOTTOM,
    )
}

/// The screen centre, for callers that only import this module.
pub const CENTRE: (f32, f32) = (CX, CY);
```

Create `crates/core/src/p2/ring.rs` (above the tests):

```rust
//! Ring kinds (fill, parts, discrete segments, bands with a marker) drawn as
//! round-capped arcs over a `TRACK` arc.

use crate::p2::layout::{RING_R, RING_START_DEG, RING_SWEEP_DEG, RING_W};
use crate::scene::Drawable;
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{TEXT, TRACK};
use crate::theme::{Color, BLACK};

#[derive(Clone, Debug, PartialEq)]
pub enum RingKind {
    /// Just the track.
    Empty,
    /// A coloured arc from the start to `frac` of the sweep.
    Fill { frac: f32, color: Color },
    /// Consecutive coloured runs with a gap between them; the rest is track.
    Parts(Vec<(f32, Color)>),
    /// `n` equal segments, each its own colour (`TRACK` for off).
    Discrete(Vec<Color>),
    /// `n` equal bands, the one holding `frac` at full alpha, a marker at `frac`.
    Bands { colors: Vec<Color>, frac: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RingSpec {
    pub kind: RingKind,
    pub r: f32,
    pub width: f32,
    pub start_deg: f32,
    pub sweep_deg: f32,
}

pub const PARTS_GAP_K: f32 = 1.3;
pub const DISCRETE_GAP_K: f32 = 1.6;
pub const BANDS_GAP_K: f32 = 1.4;
pub const DIM_BAND_ALPHA: f32 = 0.3;
/// Runs shorter than this fraction of the sweep are not drawn.
pub const MIN_FRAC: f32 = 0.002;
/// A fill never vanishes: at least a dot at the start.
pub const MIN_FILL: f32 = 0.004;

impl RingSpec {
    pub fn new(kind: RingKind) -> RingSpec {
        RingSpec {
            kind,
            r: RING_R,
            width: RING_W,
            start_deg: RING_START_DEG,
            sweep_deg: RING_SWEEP_DEG,
        }
    }
    pub fn empty() -> RingSpec {
        RingSpec::new(RingKind::Empty)
    }
    pub fn fill(frac: f32, color: Color) -> RingSpec {
        RingSpec::new(RingKind::Fill { frac, color })
    }
    pub fn parts(parts: Vec<(f32, Color)>) -> RingSpec {
        RingSpec::new(RingKind::Parts(parts))
    }
    pub fn discrete(colors: Vec<Color>) -> RingSpec {
        RingSpec::new(RingKind::Discrete(colors))
    }
    pub fn bands(colors: Vec<Color>, frac: f32) -> RingSpec {
        RingSpec::new(RingKind::Bands { colors, frac })
    }

    /// `k` stroke widths of gap, as a fraction of the sweep.
    pub fn gap_frac(&self, k: f32) -> f32 {
        self.width * k / self.r / self.sweep_deg.to_radians()
    }

    /// An arc from `f0` to `f1` of the sweep, or `None` when too short to show.
    pub fn arc(&self, f0: f32, f1: f32, color: Color, alpha: f32) -> Option<Drawable> {
        if f1 - f0 < MIN_FRAC {
            return None;
        }
        Some(Drawable::Arc {
            cx: CX,
            cy: CY,
            r: self.r,
            width: self.width,
            start_deg: self.start_deg + self.sweep_deg * f0,
            sweep_deg: self.sweep_deg * (f1 - f0),
            color,
            alpha,
        })
    }

    /// The point on the ring at `f` of the sweep.
    pub fn point(&self, f: f32) -> (f32, f32) {
        let a = (self.start_deg + self.sweep_deg * f).to_radians();
        (CX + self.r * a.sin(), CY - self.r * a.cos())
    }

    pub fn drawables(&self) -> Vec<Drawable> {
        let mut out = Vec::new();
        out.extend(self.arc(0.0, 1.0, TRACK, 1.0));
        match &self.kind {
            RingKind::Empty => {}
            RingKind::Fill { frac, color } => {
                out.extend(self.arc(0.0, frac.clamp(MIN_FILL, 1.0), *color, 1.0));
            }
            RingKind::Parts(parts) => {
                let sum: f32 = parts.iter().map(|(f, _)| f.max(0.0)).sum();
                let scale = if sum > 1.0 { 1.0 / sum } else { 1.0 };
                let gap = self.gap_frac(PARTS_GAP_K);
                let mut f = 0.0;
                for (p, c) in parts {
                    let p = p.max(0.0) * scale;
                    if p > 0.003 {
                        let lead = if f > 0.0 { gap / 2.0 } else { 0.0 };
                        out.extend(self.arc(f + lead, f + p - gap / 2.0, *c, 1.0));
                    }
                    f += p;
                }
            }
            RingKind::Discrete(colors) => {
                let n = colors.len().max(1) as f32;
                let gap = self.gap_frac(DISCRETE_GAP_K);
                for (i, c) in colors.iter().enumerate() {
                    let i = i as f32;
                    out.extend(self.arc(i / n + gap / 2.0, (i + 1.0) / n - gap / 2.0, *c, 1.0));
                }
            }
            RingKind::Bands { colors, frac } => {
                if colors.is_empty() {
                    return out;
                }
                let n = colors.len() as f32;
                let gap = self.gap_frac(BANDS_GAP_K);
                let fr = frac.clamp(0.0, 0.999);
                let cur = (fr * n).floor() as usize;
                for (i, c) in colors.iter().enumerate() {
                    let alpha = if i == cur { 1.0 } else { DIM_BAND_ALPHA };
                    let i = i as f32;
                    out.extend(self.arc(i / n + gap / 2.0, (i + 1.0) / n - gap / 2.0, *c, alpha));
                }
                let (mx, my) = self.point(fr);
                out.push(Drawable::Ellipse {
                    cx: mx,
                    cy: my,
                    rx: 0.95 * self.width,
                    ry: 0.95 * self.width,
                    color: BLACK,
                    alpha: 1.0,
                });
                out.push(Drawable::Ellipse {
                    cx: mx,
                    cy: my,
                    rx: 0.5 * self.width,
                    ry: 0.5 * self.width,
                    color: TEXT,
                    alpha: 1.0,
                });
                out.push(Drawable::Arc {
                    cx: mx,
                    cy: my,
                    r: 0.95 * self.width,
                    width: 2.5,
                    start_deg: 0.0,
                    sweep_deg: 360.0,
                    color: colors[cur],
                    alpha: 1.0,
                });
            }
        }
        out
    }
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p rackscreen-core theme:: p2::ring`
Expected: all pass. (`bands_dim_all_but_the_current_and_place_a_marker` counts 5 arcs: the marker ring is an `Arc` too.)

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src/theme.rs crates/core/src/p2
git commit -m "feat(core): P2 colour tokens, role titles, layout constants and ring kinds

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: P2 text helpers: curved title, hero + unit, pill with marquee, scale ends

**Files:**
- Create: `crates/core/src/p2/text.rs`
- Modify: `crates/core/src/p2/mod.rs`
- Test: unit tests in `text.rs`

**Interfaces:**
- Consumes: `p2::font`, `p2::layout`, `p2::ui::ScreenUi`, `Drawable::{Run, RoundRect, CurvedText}`.
- Produces: `middle_baseline(font, px, cy)`, `run_centred(cx, baseline, text, font, px, letter_spacing, color, alpha)`, `curved_title(title)`, `hero_width(text, scale)`, `hero(ui, text, scale, baseline, color, now) -> HeroLayout { drawables, x0, x1, px, baseline }`, `unit_superscript(&HeroLayout, unit)`, `unit_below(unit)`, `pill(ui, text, color, cy, now) -> Vec<Drawable>`, `ring_ends(start, end) -> Vec<Drawable>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/p2/text.rs` with the tests only:

```rust
//! Text placed by core: the curved title, the tabular rolling hero with its
//! unit, the pill (with a marquee when the text is too wide) and scale ends.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::font::{cap_height, digit_width, measure};
    use crate::p2::layout::*;
    use crate::p2::ui::ScreenUi;
    use crate::theme::p2::{BLUE, TEXT, TEXT2};

    fn runs(ds: &[Drawable]) -> Vec<(f32, f32, String, f32, f32, bool)> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::Run {
                    x,
                    baseline,
                    text,
                    px,
                    alpha,
                    clip,
                    ..
                } => Some((*x, *baseline, text.clone(), *px, *alpha, clip.is_some())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn long_titles_use_their_first_word() {
        match curved_title("partly cloudy forecast") {
            Drawable::CurvedText {
                text,
                radius,
                centre_deg,
                px,
                letter_spacing,
                color,
                ..
            } => {
                assert_eq!(text, "PARTLY");
                assert_eq!((radius, centre_deg, px, letter_spacing, color), (TITLE_R, 0.0, TITLE_PX, TITLE_LS, TEXT2));
            }
            _ => panic!("curved text"),
        }
        assert!(matches!(curved_title("cpu"), Drawable::CurvedText { text, .. } if text == "CPU"));
        assert!(matches!(curved_title("air quality"), Drawable::CurvedText { text, .. } if text == "AIR QUALITY"));
    }

    #[test]
    fn hero_lays_digits_in_tabular_cells_and_rolls_changed_ones() {
        let mut ui = ScreenUi::new();
        let h = hero(&mut ui, "42", 1.0, HERO_BASELINE, TEXT, 0.0);
        let cell = digit_width(FontId::Bold, HERO_PX);
        assert!((h.x1 - h.x0 - 2.0 * cell).abs() < 1e-3);
        assert!(((h.x0 + h.x1) / 2.0 - 120.0).abs() < 1e-3, "centred");
        let r = runs(&h.drawables);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].2, "4");
        assert!(r[0].5, "every glyph is clipped to its cell");
        assert_eq!(r[0].3, HERO_PX);
        assert!((hero_width("14:32", 0.82) - hero_width("14:32", 1.0) * 0.82).abs() < 1e-3);
        assert!(hero_width("100", 1.0) < HERO_MAX_W && hero_width("16/16", 0.82) < HERO_MAX_W);
        let h2 = hero(&mut ui, "43", 1.0, HERO_BASELINE, TEXT, 1.0);
        let r2 = runs(&h2.drawables);
        assert_eq!(r2.len(), 2, "at the start of the roll cell 1 still shows the old 2");
        assert_eq!(r2[1].2, "2");
        let h3 = hero(&mut ui, "43", 1.0, HERO_BASELINE, TEXT, 1.2);
        let r3 = runs(&h3.drawables);
        assert_eq!(r3.len(), 3, "cell 0 holds, cell 1 rolls with two glyphs");
        assert!(r3[1].4 < 1.0 && r3[2].4 < 1.0);
        assert!(r3[1].1 < HERO_BASELINE && r3[2].1 > HERO_BASELINE, "old leaves up, new enters from below");
    }

    #[test]
    fn units_sit_at_cap_height_or_below() {
        let mut ui = ScreenUi::new();
        let h = hero(&mut ui, "42", 1.0, HERO_BASELINE, TEXT, 0.0);
        let r = runs(&[unit_superscript(&h, "%")]);
        let upx = HERO_PX * UNIT_SCALE;
        assert_eq!(r[0].3, upx);
        assert!((r[0].0 - (h.x1 + UNIT_GAP)).abs() < 1e-4);
        let want = HERO_BASELINE - cap_height(FontId::Bold, HERO_PX) + cap_height(FontId::Bold, upx);
        assert!((r[0].1 - want).abs() < 1e-3, "top-aligned at the hero cap height");
        let b = runs(&[unit_below("Mb/s")]);
        assert_eq!(b[0].3, UNIT_BELOW_PX);
        assert!((b[0].1 - (HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY)).abs() < 1e-4);
        let w = measure(FontId::Medium, UNIT_BELOW_PX, "Mb/s", 0.0);
        assert!((b[0].0 - (120.0 - w / 2.0)).abs() < 1e-3);
    }

    #[test]
    fn pill_fits_or_scrolls() {
        let mut ui = ScreenUi::new();
        let short = pill(&mut ui, "On mains", BLUE, PILL_CY, 0.0);
        let tw = measure(FontId::SemiBold, PILL_PX, "On mains", 0.0);
        match &short[0] {
            Drawable::RoundRect { x, y, w, h, r, color, alpha } => {
                assert!((*w - (tw + PILL_PAD)).abs() < 1e-3);
                assert!((*x - (120.0 - w / 2.0)).abs() < 1e-3);
                assert_eq!((*y, *h, *r, *color, *alpha), (PILL_CY - PILL_H / 2.0, PILL_H, PILL_R, BLUE, PILL_FILL_ALPHA));
            }
            _ => panic!("pill background first"),
        }
        let r = runs(&short);
        assert_eq!(r.len(), 1);
        assert!(!r[0].5, "fitting text is not clipped");
        let long = "ubuntu-24.04.2-live-server-amd64.iso";
        let mut ui2 = ScreenUi::new();
        let wide = pill(&mut ui2, long, BLUE, PILL_CY, 0.0);
        assert!(matches!(&wide[0], Drawable::RoundRect { w, .. } if (*w - (PILL_MAX_TEXT_W + PILL_PAD)).abs() < 1e-3));
        let r = runs(&wide);
        assert_eq!(r.len(), 2, "text and its repeat");
        assert!(r[0].5 && r[1].5);
        assert!((r[0].0 - (120.0 - PILL_MAX_TEXT_W / 2.0)).abs() < 1e-3, "holding at the slot's left edge");
        let later = pill(&mut ui2, long, BLUE, PILL_CY, 2.5);
        let r2 = runs(&later);
        assert!((r2[0].0 - (r[0].0 - 30.0)).abs() < 1e-3, "30 px/s after the 1.5 s hold");
    }

    #[test]
    fn ring_ends_sit_inside_the_ring_at_both_ends() {
        let r = runs(&ring_ends("11°", "21°"));
        assert_eq!(r.len(), 2);
        assert!(r[0].0 > 120.0 && r[1].0 < 120.0, "start is upper right, end upper left");
        assert!(r[0].1 < 60.0 && r[1].1 < 60.0);
    }
}
```

Add `pub mod text;` to `crates/core/src/p2/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::text`
Expected: compile errors (`curved_title` not found).

- [ ] **Step 3: Implement**

Put above the tests in `crates/core/src/p2/text.rs`:

```rust
use crate::anim::Secs;
use crate::p2::font::{advance, cap_height, digit_width, measure, FontId};
use crate::p2::layout::*;
use crate::p2::marquee::GAP_PX;
use crate::p2::ui::ScreenUi;
use crate::scene::{ClipRect, Drawable};
use crate::theme::layout::CX;
use crate::theme::p2::{TEXT2, TEXT3};
use crate::theme::Color;

/// Baseline that puts the cap height of `font` at `px` centred on `cy`.
pub fn middle_baseline(font: FontId, px: f32, cy: f32) -> f32 {
    cy + cap_height(font, px) / 2.0
}

#[allow(clippy::too_many_arguments)]
pub fn run_centred(
    cx: f32,
    baseline: f32,
    text: &str,
    font: FontId,
    px: f32,
    letter_spacing: f32,
    color: Color,
    alpha: f32,
) -> Drawable {
    let w = measure(font, px, text, letter_spacing);
    Drawable::Run {
        x: cx - w / 2.0,
        baseline,
        text: text.to_string(),
        font,
        px,
        letter_spacing,
        color,
        alpha,
        clip: None,
    }
}

/// The curved title in the ring gap: upper case, first word only when the
/// whole title would not fit the gap.
pub fn curved_title(title: &str) -> Drawable {
    let upper = title.to_uppercase();
    let text = if measure(FontId::Bold, TITLE_PX, &upper, TITLE_LS) > TITLE_MAX_W {
        upper.split(' ').next().unwrap_or("").to_string()
    } else {
        upper
    };
    Drawable::CurvedText {
        text,
        radius: TITLE_R,
        centre_deg: 0.0,
        px: TITLE_PX,
        font: FontId::Bold,
        letter_spacing: TITLE_LS,
        color: TEXT2,
        alpha: 1.0,
    }
}

/// Cell widths for a hero string: digits take the `0` cell, the rest their advance.
fn cell_widths(text: &str, px: f32) -> Vec<f32> {
    let digit = digit_width(FontId::Bold, px);
    text.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                digit
            } else {
                advance(FontId::Bold, px, c)
            }
        })
        .collect()
}

/// Width of a hero at `scale`, for the fit tests.
pub fn hero_width(text: &str, scale: f32) -> f32 {
    cell_widths(text, HERO_PX * scale).iter().sum()
}

pub struct HeroLayout {
    pub drawables: Vec<Drawable>,
    pub x0: f32,
    pub x1: f32,
    pub px: f32,
    pub baseline: f32,
}

/// The rolling hero, centred on the screen: one clipped `Run` per glyph.
pub fn hero(
    ui: &mut ScreenUi,
    text: &str,
    scale: f32,
    baseline: f32,
    color: Color,
    now: Secs,
) -> HeroLayout {
    let px = HERO_PX * scale;
    let roll = ui.roll(("hero", 0), text, now);
    let widths = cell_widths(roll.text(), px);
    let total: f32 = widths.iter().sum();
    let x0 = CX - total / 2.0;
    let mut xs = Vec::with_capacity(widths.len());
    let mut x = x0;
    for w in &widths {
        xs.push(x);
        x += w;
    }
    let mut drawables = Vec::with_capacity(widths.len() * 2);
    for g in roll.glyphs(now) {
        let w = widths[g.cell];
        let gx = xs[g.cell] + (w - advance(FontId::Bold, px, g.ch)) / 2.0;
        drawables.push(Drawable::Run {
            x: gx,
            baseline: baseline + g.dy * px,
            text: g.ch.to_string(),
            font: FontId::Bold,
            px,
            letter_spacing: 0.0,
            color,
            alpha: g.alpha,
            clip: Some(ClipRect {
                x: xs[g.cell] - 2.0,
                y: baseline - px * 0.95,
                w: w + 4.0,
                h: px * 1.15,
            }),
        });
    }
    HeroLayout {
        drawables,
        x0,
        x1: x0 + total,
        px,
        baseline,
    }
}

/// A unit to the right of the hero, Bold at 0.36 × the hero size, its top on
/// the hero's cap height.
pub fn unit_superscript(h: &HeroLayout, unit: &str) -> Drawable {
    let upx = h.px * UNIT_SCALE;
    let top = h.baseline - cap_height(FontId::Bold, h.px);
    Drawable::Run {
        x: h.x1 + UNIT_GAP,
        baseline: top + cap_height(FontId::Bold, upx),
        text: unit.to_string(),
        font: FontId::Bold,
        px: upx,
        letter_spacing: 0.0,
        color: TEXT2,
        alpha: 1.0,
        clip: None,
    }
}

/// A unit word centred under the hero (`Mb/s`, `km/h`, `€/kWh`).
pub fn unit_below(unit: &str) -> Drawable {
    run_centred(
        CX,
        HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY,
        unit,
        FontId::Medium,
        UNIT_BELOW_PX,
        0.0,
        TEXT2,
        1.0,
    )
}

/// The status pill: fill at 16 % of the colour, SemiBold text; text wider
/// than 132 px scrolls, hard-clipped to the slot.
pub fn pill(ui: &mut ScreenUi, text: &str, color: Color, cy: f32, now: Secs) -> Vec<Drawable> {
    let tw = measure(FontId::SemiBold, PILL_PX, text, 0.0);
    let slot = tw.min(PILL_MAX_TEXT_W);
    let w = slot + PILL_PAD;
    let baseline = middle_baseline(FontId::SemiBold, PILL_PX, cy + 0.5);
    let mut out = vec![Drawable::RoundRect {
        x: CX - w / 2.0,
        y: cy - PILL_H / 2.0,
        w,
        h: PILL_H,
        r: PILL_R,
        color,
        alpha: PILL_FILL_ALPHA,
    }];
    let run = |x: f32, clip: Option<ClipRect>| Drawable::Run {
        x,
        baseline,
        text: text.to_string(),
        font: FontId::SemiBold,
        px: PILL_PX,
        letter_spacing: 0.0,
        color,
        alpha: 1.0,
        clip,
    };
    let mq = ui.marquee(("pill", 0), text, now);
    match mq.offset(tw, PILL_MAX_TEXT_W, now) {
        None => out.push(run(CX - tw / 2.0, None)),
        Some(off) => {
            let x0 = CX - PILL_MAX_TEXT_W / 2.0;
            let clip = Some(ClipRect {
                x: x0,
                y: cy - PILL_H / 2.0,
                w: PILL_MAX_TEXT_W,
                h: PILL_H,
            });
            out.push(run(x0 - off, clip));
            out.push(run(x0 - off + tw + GAP_PX, clip));
        }
    }
    out
}

/// Scale labels just inside the two ends of the ring (start upper right, end
/// upper left), Medium 8.5 px `TEXT3`.
pub fn ring_ends(start: &str, end: &str) -> Vec<Drawable> {
    let at = |deg: f32, text: &str| {
        let a = deg.to_radians();
        let (cx, cy) = (CX + ENDS_R * a.sin(), crate::theme::layout::CY - ENDS_R * a.cos());
        run_centred(
            cx,
            middle_baseline(FontId::Medium, ENDS_PX, cy),
            text,
            FontId::Medium,
            ENDS_PX,
            0.0,
            TEXT3,
            1.0,
        )
    };
    vec![
        at(RING_START_DEG, start),
        at(RING_START_DEG + RING_SWEEP_DEG, end),
    ]
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p rackscreen-core p2::text`
Expected: 5 tests pass. If `long_titles_use_their_first_word` fails on `AIR QUALITY`, print `measure(FontId::Bold, 10.0, "AIR QUALITY", 1.5)`; it must be under `TITLE_MAX_W` (about 90 px). Do not change `TITLE_MAX_W` below the spec's gap width; adjust the test title to a clearly longer one instead.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src/p2
git commit -m "feat(core): P2 text helpers: curved title, tabular rolling hero, pill marquee

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Card charts (Spark, Bars, Week, Stats, StackBar, NodeBars, Compass)

**Files:**
- Create: `crates/core/src/p2/chart.rs`
- Modify: `crates/core/src/p2/mod.rs`
- Test: unit tests in `chart.rs`

**Interfaces:**
- Consumes: `ScreenUi::{glide, glide_angle}`, `text::{run_centred, middle_baseline}`, drawables.
- Produces: `Card` enum and `card_drawables(card, ui, area: (x, y, w, h), now) -> Vec<Drawable>`; `smooth_curve(points) -> Vec<(f32, f32)>`; `compass_name(deg) -> &'static str`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/p2/chart.rs` with tests only:

```rust
//! The small chart inside the card, one kind per role.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::ui::ScreenUi;
    use crate::theme::p2::{BLUE, GREEN, TEXT, TEXT3, TRACK};

    const AREA: (f32, f32, f32, f32) = (76.0, 152.0, 88.0, 26.0);

    fn count<F: Fn(&Drawable) -> bool>(ds: &[Drawable], f: F) -> usize {
        ds.iter().filter(|d| f(d)).count()
    }

    fn rects(ds: &[Drawable]) -> Vec<(f32, f32, f32, f32, Color, f32)> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::RoundRect { x, y, w, h, color, alpha, .. } => Some((*x, *y, *w, *h, *color, *alpha)),
                _ => None,
            })
            .collect()
    }

    fn texts(ds: &[Drawable]) -> Vec<String> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn spark_draws_a_clipped_line_a_dot_and_a_caption() {
        let mut ui = ScreenUi::new();
        let series: Vec<f32> = (0..31).map(|i| 30.0 + i as f32).collect();
        let card = Card::Spark {
            series: series.clone(),
            prev: Some(29.0),
            scroll: 0.5,
            min: 0.0,
            max: 100.0,
            now_idx: None,
            color: BLUE,
            caption: "last 60 s".into(),
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let line = ds
            .iter()
            .find_map(|d| match d {
                Drawable::Polyline { points, width, color, .. } if *width == 1.8 && *color == BLUE => Some(points.clone()),
                _ => None,
            })
            .expect("the line");
        assert!(line.iter().all(|p| p.0 >= AREA.0 - 1e-3), "clipped at the left edge");
        assert!(line.len() > series.len(), "the cubic is sampled");
        let dot = ds.iter().find(|d| matches!(d, Drawable::Ellipse { rx, color, .. } if *rx == 2.8 && *color == TEXT)).unwrap();
        if let Drawable::Ellipse { cx, cy, .. } = dot {
            assert!((cx - (AREA.0 + AREA.2 + (88.0 / 30.0) * 0.5)).abs() < 1e-3, "newest point, shifted by the half-step scroll");
            assert!((cy - (AREA.1 + AREA.3 - 0.6 * AREA.3)).abs() < 1e-3);
        }
        assert_eq!(texts(&ds), vec!["last 60 s".to_string()]);
        let now_card = Card::Spark {
            series,
            prev: None,
            scroll: 1.0,
            min: 0.0,
            max: 100.0,
            now_idx: Some(10),
            color: BLUE,
            caption: "24 h".into(),
        };
        let ds = card_drawables(&now_card, &mut ui, AREA, 0.0);
        assert!(count(&ds, |d| matches!(d, Drawable::Polyline { width, color, .. } if *width == 1.0 && *color == TEXT3)) >= 6, "dashed now line");
        let short = Card::Spark { series: vec![1.0], prev: None, scroll: 1.0, min: 0.0, max: 1.0, now_idx: None, color: BLUE, caption: "24 h".into() };
        assert_eq!(card_drawables(&short, &mut ui, AREA, 0.0).len(), 1, "fewer than 2 points: caption only");
    }

    #[test]
    fn bars_highlight_now_and_dim_past_and_future() {
        let mut ui = ScreenUi::new();
        let card = Card::Bars {
            values: vec![1.0, 2.0, 4.0, 2.0],
            colors: vec![BLUE; 4],
            now: 2,
            past_alpha: 0.28,
            caption: "per hour".into(),
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].5, 0.28);
        assert_eq!(r[2].5, 1.0);
        assert_eq!(r[3].5, 0.6);
        assert!((r[2].3 - AREA.3).abs() < 1e-3, "the max bar fills the height");
        assert!((r[0].3 - AREA.3 / 4.0).abs() < 1e-3);
        let bw = (AREA.2 - 2.0 * 3.0) / 4.0;
        assert!((r[1].0 - (AREA.0 + bw + 2.0)).abs() < 1e-3);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Ellipse { rx, cy, .. } if *rx == 1.6 && (*cy - (AREA.1 + AREA.3 + 5.0)).abs() < 1e-3)));
        assert_eq!(texts(&ds), vec!["per hour".to_string()]);
    }

    #[test]
    fn week_has_seven_slots_letters_and_no_future_bars() {
        let mut ui = ScreenUi::new();
        let card = Card::Week { values: [3.0, 0.0, 5.0, 2.0, 1.0, 0.0, 0.0], today: 4, color: GREEN };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.iter().filter(|x| x.4 == TRACK).count(), 7, "seven track slots");
        let bars: Vec<_> = r.iter().filter(|x| x.4 == GREEN).collect();
        assert_eq!(bars.len(), 4, "days with a count up to today");
        assert_eq!(bars[3].5, 1.0, "today full");
        assert_eq!(bars[0].5, 0.45);
        assert_eq!(texts(&ds), ["M", "T", "W", "T", "F", "S", "S"]);
    }

    #[test]
    fn stats_pick_their_size_and_divide_two_columns() {
        let mut ui = ScreenUi::new();
        let two = Card::Stats(vec![("2".into(), "pending".into(), TEXT), ("0".into(), "failed".into(), TEXT3)]);
        let ds = card_drawables(&two, &mut ui, AREA, 0.0);
        assert_eq!(texts(&ds), ["2", "pending", "0", "failed"]);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { px, .. } if *px == 15.0)));
        assert_eq!(rects(&ds).len(), 1, "one divider");
        let wide = Card::Stats(vec![("21:14".into(), "start".into(), TEXT), ("6 min".into(), "duration".into(), TEXT)]);
        let ds = card_drawables(&wide, &mut ui, AREA, 0.0);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { px, text, .. } if *px == 15.0 && text == "21:14")), "5 chars still 15 px");
        let wider = Card::Stats(vec![("49 min".into(), "runtime".into(), TEXT), ("7%".into(), "load".into(), TEXT)]);
        let ds = card_drawables(&wider, &mut ui, AREA, 0.0);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { px, text, .. } if *px == 12.5 && text == "49 min")));
        let three = Card::Stats(vec![("a".into(), "x".into(), TEXT); 3]);
        let ds = card_drawables(&three, &mut ui, AREA, 0.0);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { px, .. } if *px == 11.5)));
        assert_eq!(rects(&ds).len(), 0);
    }

    #[test]
    fn stackbar_nodebars_and_compass() {
        let mut ui = ScreenUi::new();
        let sb = Card::StackBar { parts: vec![(0.16, BLUE), (0.13, GREEN)], caption: "1.9 TB free".into() };
        let ds = card_drawables(&sb, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 3, "track and two segments");
        assert!((r[1].2 - (0.16 * AREA.2 - 1.5)).abs() < 1e-3);
        assert!((r[2].0 - (AREA.0 + 0.16 * AREA.2)).abs() < 1e-3);
        assert_eq!(texts(&ds), vec!["1.9 TB free".to_string()]);
        let nb = Card::NodeBars {
            values: vec![46.0, 54.0, 70.0],
            min: 30.0,
            max: 85.0,
            labels: vec!["c1".into(), "p1".into(), "p2".into()],
            colors: vec![GREEN, GREEN, BLUE],
            highlight: Some(2),
        };
        let ds = card_drawables(&nb, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 6);
        assert_eq!(r[5].5, 1.0, "highlighted bar full");
        assert_eq!(r[1].5, 0.5);
        assert!((r[5].3 / (AREA.3 - 8.0) - (40.0 / 55.0)).abs() < 1e-3);
        assert_eq!(texts(&ds), ["c1", "p1", "p2"]);
        let c = Card::Compass { deg: 225.0, label: "SW · 225°".into() };
        let ds = card_drawables(&c, &mut ui, AREA, 0.0);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Polygon { points, .. } if points.len() == 3)));
        let t = texts(&ds);
        assert!(t.contains(&"SW".to_string()) && t.contains(&"SW · 225°".to_string()));
        assert!(!t.contains(&"N".to_string()), "north is 225° away, off the strip");
        assert_eq!(compass_name(225.0), "SW");
        assert_eq!(compass_name(359.0), "N");
        assert_eq!(compass_name(100.0), "E");
    }
}
```

Add `pub mod chart;` to `crates/core/src/p2/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::chart`
Expected: compile errors (`Card` not found).

- [ ] **Step 3: Implement**

Put above the tests in `crates/core/src/p2/chart.rs`:

```rust
use crate::anim::Secs;
use crate::p2::font::FontId;
use crate::p2::layout::CAPTION_PX;
use crate::p2::text::{middle_baseline, run_centred};
use crate::p2::ui::ScreenUi;
use crate::scene::Drawable;
use crate::theme::p2::{CYAN, TEXT, TEXT2, TEXT3, TRACK};
use crate::theme::Color;

#[derive(Clone, Debug, PartialEq)]
pub enum Card {
    /// A smooth line; `prev` is the sample that just scrolled off (drawn while
    /// `scroll` < 1 so the line slides left one step), `now_idx` marks a point
    /// other than the newest with a dashed line.
    Spark {
        series: Vec<f32>,
        prev: Option<f32>,
        scroll: f32,
        min: f32,
        max: f32,
        now_idx: Option<usize>,
        color: Color,
        caption: String,
    },
    /// Rounded bars, one colour each; `now` full, earlier at `past_alpha`, later at 0.6.
    Bars {
        values: Vec<f32>,
        colors: Vec<Color>,
        now: usize,
        past_alpha: f32,
        caption: String,
    },
    /// Monday to Sunday on track slots; `today` full, earlier 45 %, later empty.
    Week {
        values: [f32; 7],
        today: usize,
        color: Color,
    },
    /// Two or three `(value, label, value colour)` columns.
    Stats(Vec<(String, String, Color)>),
    /// One rounded bar split into coloured parts; the free part is track.
    StackBar {
        parts: Vec<(f32, Color)>,
        caption: String,
    },
    /// One bar per node on track slots; `highlight` full, the rest at 50 %.
    NodeBars {
        values: Vec<f32>,
        min: f32,
        max: f32,
        labels: Vec<String>,
        colors: Vec<Color>,
        highlight: Option<usize>,
    },
    /// A sliding strip of ticks and letters with `deg` under the centre marker.
    Compass { deg: f32, label: String },
}

/// Sample a horizontal-tangent cubic between consecutive points (no overshoot).
pub fn smooth_curve(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    const STEPS: usize = 6;
    let mut out = Vec::with_capacity(pts.len() * STEPS);
    if let Some(first) = pts.first() {
        out.push(*first);
    }
    for w in pts.windows(2) {
        let (o, q) = (w[0], w[1]);
        let m = (o.0 + q.0) / 2.0;
        for s in 1..=STEPS {
            let t = s as f32 / STEPS as f32;
            let u = 1.0 - t;
            let x = u * u * u * o.0 + 3.0 * u * u * t * m + 3.0 * u * t * t * m + t * t * t * q.0;
            let y = u * u * u * o.1 + 3.0 * u * u * t * o.1 + 3.0 * u * t * t * q.1 + t * t * t * q.1;
            out.push((x, y));
        }
    }
    out
}

/// Drop everything left of `x0`, inserting the crossing point.
fn clip_left(pts: &[(f32, f32)], x0: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        if p.0 >= x0 {
            if i > 0 && pts[i - 1].0 < x0 {
                let q = pts[i - 1];
                let t = (x0 - q.0) / (p.0 - q.0);
                out.push((x0, q.1 + (p.1 - q.1) * t));
            }
            out.push(*p);
        }
    }
    out
}

fn caption(text: &str, x: f32, y: f32, w: f32, h: f32, dy: f32, px: f32, color: Color) -> Drawable {
    run_centred(
        x + w / 2.0,
        middle_baseline(FontId::Medium, px, y + h + dy),
        text,
        FontId::Medium,
        px,
        0.0,
        color,
        1.0,
    )
}

fn bar(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color, alpha: f32) -> Drawable {
    Drawable::RoundRect {
        x,
        y,
        w,
        h,
        r,
        color,
        alpha,
    }
}

/// Eight-point compass name for a direction in degrees clockwise from north.
pub fn compass_name(deg: f32) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    NAMES[((deg.rem_euclid(360.0) + 22.5) / 45.0).floor() as usize % 8]
}

pub fn card_drawables(
    card: &Card,
    ui: &mut ScreenUi,
    area: (f32, f32, f32, f32),
    now: Secs,
) -> Vec<Drawable> {
    let (x, y, w, h) = area;
    let mut out = Vec::new();
    match card {
        Card::Spark {
            series,
            prev,
            scroll,
            min,
            max,
            now_idx,
            color,
            caption: cap,
        } => {
            if series.len() >= 2 {
                let n = series.len();
                let span = (max - min).max(1e-3);
                let norm = |v: f32| y + h - ((v.clamp(*min, *max) - min) / span) * h;
                let dx = w / (n as f32 - 1.0);
                let shift = if prev.is_some() { dx * (1.0 - scroll.clamp(0.0, 1.0)) } else { 0.0 };
                let mut pts: Vec<(f32, f32)> = Vec::with_capacity(n + 1);
                if let Some(p) = prev {
                    pts.push((x - dx + shift, norm(*p)));
                }
                for (i, v) in series.iter().enumerate() {
                    pts.push((x + i as f32 * dx + shift, norm(*v)));
                }
                let curve = clip_left(&smooth_curve(&pts), x);
                out.push(Drawable::Polyline {
                    points: curve,
                    width: 1.8,
                    color: *color,
                    alpha: 1.0,
                });
                let li = now_idx.unwrap_or(n - 1).min(n - 1) + usize::from(prev.is_some());
                let lp = pts[li];
                if now_idx.is_some() {
                    let mut yy = y - 2.0;
                    while yy < y + h + 2.0 {
                        out.push(Drawable::Polyline {
                            points: vec![(lp.0, yy), (lp.0, (yy + 2.0).min(y + h + 2.0))],
                            width: 1.0,
                            color: TEXT3,
                            alpha: 1.0,
                        });
                        yy += 4.0;
                    }
                }
                out.push(Drawable::Ellipse {
                    cx: lp.0,
                    cy: lp.1,
                    rx: 2.8,
                    ry: 2.8,
                    color: TEXT,
                    alpha: 1.0,
                });
            }
            out.push(caption(cap, x, y, w, h, 11.0, CAPTION_PX, TEXT3));
        }
        Card::Bars {
            values,
            colors,
            now: now_i,
            past_alpha,
            caption: cap,
        } => {
            let n = values.len().max(1);
            let gap = if n > 12 { 1.6 } else { 2.0 };
            let bw = (w - gap * (n as f32 - 1.0)) / n as f32;
            let mx = values.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
            for (i, v) in values.iter().enumerate() {
                let hh = (ui.glide(("bar", i as u32), v / mx, now) * h).max(1.5);
                let col = colors.get(i).copied().unwrap_or(TEXT2);
                let alpha = if i == *now_i {
                    1.0
                } else if i < *now_i {
                    *past_alpha
                } else {
                    0.6
                };
                out.push(bar(x + i as f32 * (bw + gap), y + h - hh, bw, hh, (bw / 2.0).min(1.5), col, alpha));
            }
            let cx = x + *now_i as f32 * (bw + gap) + bw / 2.0;
            out.push(Drawable::Ellipse {
                cx,
                cy: y + h + 5.0,
                rx: 1.6,
                ry: 1.6,
                color: TEXT,
                alpha: 1.0,
            });
            out.push(caption(cap, x, y, w, h, 13.0, CAPTION_PX, TEXT3));
        }
        Card::Week {
            values,
            today,
            color,
        } => {
            const LETTERS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
            let gap = 5.0;
            let bw = (w - gap * 6.0) / 7.0;
            let mx = values.iter().cloned().fold(1.0_f32, f32::max);
            let hh0 = h - 9.0;
            for (i, v) in values.iter().enumerate() {
                let x0 = x + i as f32 * (bw + gap);
                out.push(bar(x0, y, bw, hh0, 3.0, TRACK, 1.0));
                if i <= *today && *v > 0.0 {
                    let hh = (ui.glide(("week", i as u32), v / mx, now) * hh0).max(3.0);
                    let alpha = if i == *today { 1.0 } else { 0.45 };
                    out.push(bar(x0, y + hh0 - hh, bw, hh, 3.0, *color, alpha));
                }
                let (col, font) = if i == *today { (TEXT, FontId::Bold) } else { (TEXT3, FontId::Medium) };
                out.push(run_centred(
                    x0 + bw / 2.0,
                    middle_baseline(font, 8.0, y + h + 3.0),
                    LETTERS[i],
                    font,
                    8.0,
                    0.0,
                    col,
                    1.0,
                ));
            }
        }
        Card::Stats(items) => {
            let n = items.len().max(1);
            let longest = items.iter().map(|(v, _, _)| v.chars().count()).max().unwrap_or(0);
            let vpx = if n >= 3 {
                11.5
            } else if longest > 5 {
                12.5
            } else {
                15.0
            };
            for (i, (v, l, c)) in items.iter().enumerate() {
                let cx = x + w * (i as f32 + 0.5) / n as f32;
                let vy = y + h * 0.32;
                out.push(run_centred(cx, middle_baseline(FontId::SemiBold, vpx, vy), v, FontId::SemiBold, vpx, 0.0, *c, 1.0));
                out.push(run_centred(cx, middle_baseline(FontId::Medium, 8.5, vy + 15.0), l, FontId::Medium, 8.5, 0.0, TEXT2, 1.0));
            }
            if n == 2 {
                out.push(bar(x + w / 2.0 - 0.5, y + 2.0, 1.0, h - 4.0, 0.0, TRACK, 1.0));
            }
        }
        Card::StackBar { parts, caption: cap } => {
            let bh = 8.0;
            let by = y + 4.0;
            out.push(bar(x, by, w, bh, bh / 2.0, TRACK, 1.0));
            let mut fx = x;
            for (i, (f, c)) in parts.iter().enumerate() {
                let pw = ui.glide(("stack", i as u32), f.clamp(0.0, 1.0), now) * w;
                if pw > 1.5 {
                    out.push(bar(fx, by, pw - 1.5, bh, bh / 2.0, *c, 1.0));
                }
                fx += pw;
            }
            out.push(run_centred(
                x + w / 2.0,
                middle_baseline(FontId::Medium, 9.0, by + bh + 11.0),
                cap,
                FontId::Medium,
                9.0,
                0.0,
                TEXT2,
                1.0,
            ));
        }
        Card::NodeBars {
            values,
            min,
            max,
            labels,
            colors,
            highlight,
        } => {
            let n = values.len().max(1);
            let gap = if n > 5 { 4.0 } else { 7.0 };
            let bw = (w - gap * (n as f32 - 1.0)) / n as f32;
            let hh0 = h - 8.0;
            let span = (max - min).max(1e-3);
            for (i, v) in values.iter().enumerate() {
                let x0 = x + i as f32 * (bw + gap);
                let f = ((v - min) / span).clamp(0.04, 1.0);
                let hh = ui.glide(("node", i as u32), f, now) * hh0;
                let col = colors.get(i).copied().unwrap_or(TEXT2);
                let alpha = if highlight.is_none() || *highlight == Some(i) { 1.0 } else { 0.5 };
                out.push(bar(x0, y, bw, hh0, 2.5, TRACK, 1.0));
                out.push(bar(x0, y + hh0 - hh, bw, hh, 2.5, col, alpha));
                let lc = if *highlight == Some(i) { TEXT } else { TEXT3 };
                let label = labels.get(i).map(String::as_str).unwrap_or("");
                out.push(run_centred(
                    x0 + bw / 2.0,
                    middle_baseline(FontId::SemiBold, 7.5, y + h + 3.0),
                    label,
                    FontId::SemiBold,
                    7.5,
                    0.0,
                    lc,
                    1.0,
                ));
            }
        }
        Card::Compass { deg, label } => {
            let deg = ui.glide_angle(("compass", 0), *deg, now, 1.2);
            let cx = x + w / 2.0;
            let pxd = w / 180.0;
            let mut d = -180.0;
            while d <= 540.0 {
                let xx = cx + (d - deg) * pxd;
                if xx >= x && xx <= x + w {
                    let dd = (d as i32).rem_euclid(360);
                    let major = dd % 45 == 0;
                    out.push(Drawable::Polyline {
                        points: vec![(xx, y + if major { 2.0 } else { 5.0 }), (xx, y + 9.0)],
                        width: 1.0,
                        color: if major { TEXT2 } else { TEXT3 },
                        alpha: 1.0,
                    });
                    if major {
                        let col = if (xx - cx).abs() < 6.0 { TEXT } else { TEXT2 };
                        out.push(run_centred(
                            xx,
                            middle_baseline(FontId::SemiBold, 8.0, y + 17.0),
                            compass_name(dd as f32),
                            FontId::SemiBold,
                            8.0,
                            0.0,
                            col,
                            1.0,
                        ));
                    }
                }
                d += 15.0;
            }
            out.push(Drawable::Polygon {
                points: vec![(cx - 4.0, y - 3.0), (cx + 4.0, y - 3.0), (cx, y + 3.0)],
                color: CYAN,
                alpha: 1.0,
            });
            out.push(caption(label, x, y, w, h, 3.0, CAPTION_PX, TEXT3));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p rackscreen-core p2::chart`
Expected: 5 tests pass. The spark dot assertion: with `prev` present and `scroll = 0.5` the newest point is at `x + w + dx/2` where `dx = 88 / 30`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src/p2
git commit -m "feat(core): P2 card charts (spark, bars, week, stats, stack bar, node bars, compass)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: `P2Screen`, `compose`, and `ScreenUi` plumbing through `Model::scene`

**Files:**
- Create: `crates/core/src/p2/screen.rs`
- Modify: `crates/core/src/p2/mod.rs`, `crates/core/src/scene.rs` (`role_scene` signature), `crates/core/src/scene_fx.rs` (`scene`, `scene_for_role`), tests in `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`, `crates/core/src/model.rs`
- Modify: `crates/render/src/renderer.rs` (tests), `crates/render/tests/golden.rs`, `crates/app/src/runloop.rs`, `crates/app/examples/gifs.rs`
- Test: unit test in `screen.rs`, golden `p2_sample`

**Interfaces:**
- Consumes: everything from Tasks 3–6.
- Produces: `P2Screen { title: String, hero: String, hero_scale: f32, hero_color: Color, unit: Option<String>, unit_below: bool, ring: RingSpec, ends: Option<(String, String)>, pill: Option<(String, Color)>, card: Option<Card>, live: bool }` with `P2Screen::new(title, hero)`; `compose(&P2Screen, &mut ScreenUi, now) -> Scene`; `role_scene(model, role, now, ui)`; `Model::scene(screen, now, ui)`; `Model::scene_for_role(role, now, ui)`; `ScreenSlot.ui`.

- [ ] **Step 1: Write the failing compose test**

Create `crates/core/src/p2/screen.rs` with tests only:

```rust
//! A P2 screen as plain data, and the composer that turns it into a `Scene`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::chart::Card;
    use crate::p2::layout::*;
    use crate::p2::ring::RingSpec;
    use crate::p2::ui::ScreenUi;
    use crate::theme::p2::{BLUE, CARD, OK, TEXT2};

    fn sample() -> P2Screen {
        let mut s = P2Screen::new("cpu", "42");
        s.unit = Some("%".into());
        s.ring = RingSpec::fill(0.42, BLUE);
        s.pill = Some(("7 nodes · avg".into(), TEXT2));
        s.card = Some(Card::Spark {
            series: (0..31).map(|i| 40.0 + (i % 5) as f32).collect(),
            prev: Some(39.0),
            scroll: 1.0,
            min: 0.0,
            max: 100.0,
            now_idx: None,
            color: BLUE,
            caption: "last 60 s".into(),
        });
        s
    }

    #[test]
    fn compose_orders_ring_title_hero_unit_pill_card() {
        let mut ui = ScreenUi::new();
        let scene = compose(&sample(), &mut ui, 0.0);
        let items = &scene.items;
        assert!(matches!(items[0], Drawable::Clear(_)));
        assert!(matches!(items[1], Drawable::Arc { .. }), "track first");
        assert!(matches!(items[2], Drawable::Arc { sweep_deg, .. } if (sweep_deg - 300.0 * 0.42).abs() < 1e-3));
        assert!(matches!(&items[3], Drawable::CurvedText { text, .. } if text == "CPU"));
        let runs: Vec<&String> = items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(runs[0], "4");
        assert_eq!(runs[1], "2");
        assert_eq!(runs[2], "%");
        assert_eq!(runs[3], "7 nodes · avg");
        assert!(items.iter().any(|d| matches!(d, Drawable::RoundRect { y, w, h, color, .. } if *color == CARD && *w == CARD_W && *h == CARD_H && (*y - card_top(PILL_CY)).abs() < 1e-3)));
        assert!(items.iter().any(|d| matches!(d, Drawable::Polyline { width, .. } if *width == 1.8)));
        assert_eq!(scene.zoom, 1.0);
    }

    #[test]
    fn ring_fill_glides_and_unit_below_moves_everything_down() {
        let mut ui = ScreenUi::new();
        let mut s = sample();
        compose(&s, &mut ui, 0.0);
        s.ring = RingSpec::fill(0.9, BLUE);
        let mid = compose(&s, &mut ui, 0.3);
        let sweep = mid
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Arc { sweep_deg, color, .. } if *color == BLUE => Some(*sweep_deg),
                _ => None,
            })
            .next()
            .unwrap();
        assert!(sweep > 126.0 && sweep < 270.0, "gliding: {sweep}");
        let mut below = sample();
        below.unit = Some("Mb/s".into());
        below.unit_below = true;
        below.live = true;
        let scene = compose(&below, &mut ui, 1.0);
        assert!(scene.items.iter().any(|d| matches!(d, Drawable::Run { text, baseline, .. } if text == "Mb/s" && (*baseline - (HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY)).abs() < 1e-3)));
        assert!(scene.items.iter().any(|d| matches!(d, Drawable::RoundRect { y, color, .. } if *color == CARD && (*y - card_top(PILL_CY_UNIT_BELOW)).abs() < 1e-3)));
        assert!(scene.items.iter().any(|d| matches!(d, Drawable::Ellipse { cy, color, .. } if *color == OK && *cy == LIVE_CY)));
        let empty = P2Screen::new("ups", "—");
        let scene = compose(&empty, &mut ui, 1.0);
        assert_eq!(scene.items.iter().filter(|d| matches!(d, Drawable::Arc { .. })).count(), 1, "track only");
        assert!(!scene.items.iter().any(|d| matches!(d, Drawable::RoundRect { .. })), "no pill, no card");
    }
}
```

Add `pub mod screen;` to `crates/core/src/p2/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::screen`
Expected: compile error (`P2Screen` not found).

- [ ] **Step 3: Implement `P2Screen` and `compose`**

Above the tests in `crates/core/src/p2/screen.rs`:

```rust
use crate::anim::Secs;
use crate::p2::chart::{card_drawables, Card};
use crate::p2::layout::*;
use crate::p2::ring::{RingKind, RingSpec};
use crate::p2::text::{curved_title, hero, pill, ring_ends, unit_below, unit_superscript};
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{CARD, OK, TEXT};
use crate::theme::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct P2Screen {
    pub title: String,
    pub hero: String,
    pub hero_scale: f32,
    pub hero_color: Color,
    pub unit: Option<String>,
    pub unit_below: bool,
    pub ring: RingSpec,
    /// Scale labels at the two ends of the ring (weather: day low and high).
    pub ends: Option<(String, String)>,
    pub pill: Option<(String, Color)>,
    pub card: Option<Card>,
    /// The health role's live indicator.
    pub live: bool,
}

impl P2Screen {
    pub fn new(title: impl Into<String>, hero: impl Into<String>) -> P2Screen {
        P2Screen {
            title: title.into(),
            hero: hero.into(),
            hero_scale: 1.0,
            hero_color: TEXT,
            unit: None,
            unit_below: false,
            ring: RingSpec::empty(),
            ends: None,
            pill: None,
            card: None,
            live: false,
        }
    }
}

/// The ring with its animated fractions glided (0.65 s) by the screen's state.
fn glided_ring(spec: &RingSpec, ui: &mut ScreenUi, now: Secs) -> RingSpec {
    let kind = match &spec.kind {
        RingKind::Fill { frac, color } => RingKind::Fill {
            frac: ui.glide(("ring", 0), *frac, now),
            color: *color,
        },
        RingKind::Parts(parts) => RingKind::Parts(
            parts
                .iter()
                .enumerate()
                .map(|(i, (f, c))| (ui.glide(("part", i as u32), *f, now), *c))
                .collect(),
        ),
        RingKind::Bands { colors, frac } => RingKind::Bands {
            colors: colors.clone(),
            frac: ui.glide(("band", 0), *frac, now),
        },
        other => other.clone(),
    };
    RingSpec {
        kind,
        ..spec.clone()
    }
}

pub fn compose(s: &P2Screen, ui: &mut ScreenUi, now: Secs) -> Scene {
    let mut scene = Scene::new();
    for d in glided_ring(&s.ring, ui, now).drawables() {
        scene.push(d);
    }
    scene.push(curved_title(&s.title));
    if let Some((a, b)) = &s.ends {
        scene.items.extend(ring_ends(a, b));
    }
    let baseline = if s.unit_below {
        HERO_BASELINE_UNIT_BELOW
    } else {
        HERO_BASELINE
    };
    let h = hero(ui, &s.hero, s.hero_scale, baseline, s.hero_color, now);
    scene.items.extend(h.drawables.iter().cloned());
    if let Some(u) = &s.unit {
        scene.push(if s.unit_below {
            unit_below(u)
        } else {
            unit_superscript(&h, u)
        });
    }
    let pill_cy = if s.unit_below {
        PILL_CY_UNIT_BELOW
    } else {
        PILL_CY
    };
    if let Some((text, color)) = &s.pill {
        scene.items.extend(pill(ui, text, *color, pill_cy, now));
    }
    if let Some(card) = &s.card {
        scene.push(Drawable::RoundRect {
            x: CX - CARD_W / 2.0,
            y: card_top(pill_cy),
            w: CARD_W,
            h: CARD_H,
            r: CARD_R,
            color: CARD,
            alpha: 1.0,
        });
        scene
            .items
            .extend(card_drawables(card, ui, chart_rect(pill_cy), now));
    }
    if s.live {
        let p = ((now % 2.0) / 2.0) as f32;
        scene.push(Drawable::Arc {
            cx: CX,
            cy: LIVE_CY,
            r: 2.0 + 5.0 * p,
            width: 1.0,
            start_deg: 0.0,
            sweep_deg: 360.0,
            color: OK,
            alpha: 0.6 * (1.0 - p),
        });
        scene.push(Drawable::Ellipse {
            cx: CX,
            cy: LIVE_CY,
            rx: 2.0,
            ry: 2.0,
            color: OK,
            alpha: 1.0,
        });
    }
    scene
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p rackscreen-core p2::screen`
Expected: 2 tests pass.

- [ ] **Step 5: Thread `ScreenUi` through the scene entry points**

In `crates/core/src/scene.rs`:

```rust
use crate::p2::ui::ScreenUi;
// ...
pub fn role_scene(model: &Model, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
    ui.for_role(role);
    match role {
```

(the body is unchanged for now; `ui` is unused by the legacy arms, which is fine because it is used by `for_role`).

In `crates/core/src/scene_fx.rs` replace the two `impl Model` entry points:

```rust
    /// Scene for one screen at one instant; the only call the render loop makes.
    /// `ui` is that screen's widget state; it resets itself on a role change.
    pub fn scene(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> Scene {
        let screens = self.screen_count();
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene(now),
        };
        let role = state.current();
        if self.wants_connecting(role) {
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
            let mut s = if self.wants_connecting(shown) {
                connecting_scene(now)
            } else {
                self.scene_for_role(shown, now, ui)
            };
            s.zoom = zoom;
            s.ring_reveal = reveal;
            return s;
        }
        let base = self.scene_for_role(role, now, ui);
        if self.fx().sweeps.active().is_some() {
            return base;
        }
        match self.fx().splashes[role.index()].active() {
            Some(sp) => splash_overlay(base, sp, now),
            None => base,
        }
    }

    /// Role scene without transition or splash (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
        if !self.needs_data(role) {
            return role_scene(self, role, now, ui);
        }
```

(the rest of `scene_for_role` is unchanged) and add `use crate::p2::ui::ScreenUi;` to its imports.

Update every test call site mechanically. In `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`, `crates/core/src/model.rs`, `crates/render/src/renderer.rs` and `crates/render/tests/golden.rs`:

```bash
cd /home/silke/Documents/GitHub/RackScreen
for f in crates/core/src/scene.rs crates/core/src/scene_fx.rs crates/core/src/model.rs crates/render/src/renderer.rs crates/render/tests/golden.rs; do
  sed -i -E 's/\.scene_for_role\(([^,]+), ([^)]+)\)/.scene_for_role(\1, \2, \&mut ScreenUi::new())/g' "$f"
  sed -i -E 's/([a-z0-9_]+)\.scene\(([^,]+), ([^)]+)\)/\1.scene(\2, \3, \&mut ScreenUi::new())/g' "$f"
  sed -i -E 's/role_scene\(&m, ([^,]+), ([^)]+)\)/role_scene(\&m, \1, \2, \&mut ScreenUi::new())/g' "$f"
done
grep -n "ScreenUi::new())" crates/core/src/scene_fx.rs | grep -v "#\[test\]" | head
```

The sed also rewrote the two non-test calls inside `Model::scene` (`self.scene_for_role(shown, now, &mut ScreenUi::new())` and `self.scene_for_role(role, now, &mut ScreenUi::new())`): change both back to pass `ui` as shown in the code above. Then add the import to each test module (and to the golden test file's top-level imports):

```rust
use crate::p2::ui::ScreenUi;            // core test modules (scene.rs, scene_fx.rs, model.rs)
use rackscreen_core::p2::ui::ScreenUi;  // render: renderer.rs tests and tests/golden.rs
```

In `crates/core/src/model.rs` tests one assertion compares two scenes for purity:

```rust
        let s = m.scene_for_role(Role::Face, 1.0, &mut ScreenUi::new());
        assert_eq!(s.items.len(), 49);
        assert_eq!(m.scene_for_role(Role::Face, 1.0, &mut ScreenUi::new()), s, "pure in `now`");
```

(the sed produces exactly this).

- [ ] **Step 6: The run loop and the gifs example own one `ScreenUi` per screen**

In `crates/app/src/runloop.rs` add `use rackscreen_core::p2::ui::ScreenUi;`, a field and its initialiser:

```rust
pub struct ScreenSlot {
    pub index: usize,
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    /// Rolls, glides and marquees of whatever role this panel is showing.
    pub ui: ScreenUi,
    cur: Pixmap,
    oriented: Pixmap,
    prev: Pixmap,
    first: bool,
}
// in ScreenSlot::new:
            ui: ScreenUi::new(),
// in the render loop:
            for s in &mut self.screens {
                let scene = model.scene(s.index, now, &mut s.ui);
```

In `crates/app/examples/gifs.rs` add `use rackscreen_core::p2::ui::ScreenUi;`, declare `let mut ui = ScreenUi::new();` next to `let mut px = new_pixmap();` in `record`, and call `renderer.render(&model.scene(0, now, &mut ui), &mut px);`.

- [ ] **Step 7: A golden of the composed sample**

Append to `crates/render/tests/golden.rs`:

```rust
#[test]
fn p2_sample_screen() {
    use rackscreen_core::p2::chart::Card;
    use rackscreen_core::p2::ring::RingSpec;
    use rackscreen_core::p2::screen::{compose, P2Screen};
    use rackscreen_core::theme::p2::{BLUE, TEXT2};
    let mut s = P2Screen::new("cpu", "42");
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(0.42, BLUE);
    s.pill = Some(("7 nodes · avg".into(), TEXT2));
    s.card = Some(Card::Spark {
        series: (0..31).map(|i| 30.0 + 20.0 * ((i as f32) * 0.4).sin()).collect(),
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max: 100.0,
        now_idx: None,
        color: BLUE,
        caption: "last 60 s".into(),
    });
    let mut ui = ScreenUi::new();
    let scene = compose(&s, &mut ui, 5.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&scene, &mut px);
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0));
    let (red, g, b) = pixel(&px, 226, 120);
    assert!(b > 200 && red < 140 && g > 140, "blue fill at 3 o'clock, got {red},{g},{b}");
    let (t, ..) = pixel(&px, 120, 226);
    assert!((15..=35).contains(&t), "track at 6 o'clock, got {t}");
    check("p2_sample", &px);
}
```

- [ ] **Step 8: Run everything**

Run: `cargo test --workspace`
Expected: all green; `p2_sample_screen` writes `crates/render/tests/goldens/p2_sample.png` on its first run. Open it: a blue 300° ring opening at the top with "CPU" curved in the gap, "42" with a small "%" top-right, a grey pill, a dark card with a blue sparkline and "last 60 s". The fill arc starts at the upper right and runs clockwise.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src crates/render crates/app
git commit -m "feat(core,app): P2Screen composer; ScreenUi owned per screen and passed into Model::scene

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: History buffers, hourly buffers and the Prometheus range prefill

**Files:**
- Create: `crates/core/src/history.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/event.rs` (`Event::MetricsHistory`), `crates/core/src/model.rs` (buffers, fold, accessors)
- Modify: `crates/sources/src/prometheus.rs` (`parse_matrix`, `query_range`, `prefill`), `crates/sources/src/fake.rs` (`metrics_history` in `initial`)
- Test: unit tests in `history.rs`, `model.rs`, `prometheus.rs`

**Interfaces:**
- Produces: `History::{new(cap, step), push(v, now), prefill(values, now), series(), prev(), scroll(now), len(), is_empty(), mean()}`; `Hourly::{new(), push(unix_hour, v), values(), values_since(unix_hour), mean(), len(), is_empty()}`; `Event::MetricsHistory { cpu: Vec<f32>, mem: Vec<f32>, net_rx_mbps: Vec<f32> }`; `Model::{cpu_history(), mem_history(), net_history(), carbon_hourly(), renewable_hourly(), unix_hour()}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/history.rs` with tests only:

```rust
//! Fixed-size sample buffers behind the sparklines: a stepped ring buffer that
//! remembers the sample that just scrolled off, and 24 hourly slots.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_cap_plus_the_dropped_sample_and_scrolls_after_a_push() {
        let mut h = History::new(3, 2.0);
        assert!(h.is_empty());
        assert_eq!(h.scroll(0.0), 1.0, "nothing to scroll");
        h.push(1.0, 0.0);
        h.push(2.0, 2.0);
        h.push(3.0, 4.0);
        assert_eq!(h.series(), vec![1.0, 2.0, 3.0]);
        assert_eq!(h.prev(), None, "not full yet");
        assert_eq!(h.scroll(4.0), 1.0);
        h.push(f32::NAN, 5.0);
        assert_eq!(h.len(), 3, "NaN is skipped");
        h.push(4.0, 6.0);
        assert_eq!(h.series(), vec![2.0, 3.0, 4.0]);
        assert_eq!(h.prev(), Some(1.0));
        assert_eq!(h.scroll(6.0), 0.0, "the line starts one step to the right");
        let mid = h.scroll(6.3);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(h.scroll(6.6), 1.0, "settled after 30 % of the 2 s step");
        h.push(5.0, 8.0);
        assert_eq!((h.series(), h.prev()), (vec![3.0, 4.0, 5.0], Some(2.0)));
        assert!((h.mean().unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn prefill_takes_the_newest_and_does_not_scroll() {
        let mut h = History::new(3, 2.0);
        h.prefill(&[9.0, f32::NAN, 1.0, 2.0, 3.0, 4.0], 10.0);
        assert_eq!(h.series(), vec![2.0, 3.0, 4.0]);
        assert_eq!(h.prev(), Some(1.0));
        assert_eq!(h.scroll(10.0), 1.0, "a prefilled line stands still");
        h.push(5.0, 12.0);
        assert_eq!(h.scroll(12.0), 0.0);
    }

    #[test]
    fn hourly_slots_replace_the_same_hour_and_drop_old_ones() {
        let mut h = Hourly::new();
        assert!(h.is_empty() && h.mean().is_none());
        h.push(100, 10.0);
        h.push(100, 11.0);
        assert_eq!(h.values(), vec![11.0]);
        h.push(101, 12.0);
        h.push(99, 5.0);
        assert_eq!(h.values(), vec![11.0, 12.0], "an older hour is ignored");
        for i in 2..30 {
            h.push(100 + i, i as f32);
        }
        assert_eq!(h.len(), 24);
        assert_eq!(h.values()[0], 6.0, "hour 106 is the oldest kept at hour 129");
        assert_eq!(h.values_since(128), vec![28.0, 29.0]);
        assert!((h.mean().unwrap() - 17.5).abs() < 1e-5);
    }
}
```

Add to `crates/core/src/model.rs` tests:

```rust
    #[test]
    fn metrics_and_network_feed_the_histories_and_prefill_fills_them() {
        let mut m = Model::new(Thresholds::default());
        m.apply(metrics(42.0, None), 0.0);
        m.apply(metrics(44.0, None), 2.0);
        assert_eq!(m.cpu_history().series(), vec![42.0, 44.0]);
        assert_eq!(m.mem_history().series(), vec![50.0, 50.0]);
        m.apply(
            Event::Network {
                rx_bps: 40e6,
                tx_bps: 1e6,
            },
            3.0,
        );
        assert_eq!(m.net_history().series(), vec![40.0], "download in Mb/s");
        m.apply(
            Event::MetricsHistory {
                cpu: (0..40).map(|i| i as f32).collect(),
                mem: vec![1.0; 10],
                net_rx_mbps: vec![],
            },
            4.0,
        );
        assert_eq!(m.cpu_history().len(), 31);
        assert_eq!(m.cpu_history().series()[30], 39.0);
        assert_eq!(m.cpu_history().prev(), Some(8.0));
        assert_eq!(m.mem_history().len(), 10);
        assert_eq!(m.net_history().len(), 0, "an empty prefill clears");
    }

    #[test]
    fn electricity_samples_land_in_hourly_slots_by_unix_hour() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 100);
        m.apply(electricity_sample(214.0, 61.0), 0.0);
        m.set_unix_now(3600 * 1000 + 1800);
        m.apply(electricity_sample(220.0, 62.0), 1.0);
        assert_eq!(m.carbon_hourly().values(), vec![220.0], "same hour replaces");
        m.set_unix_now(3600 * 1001);
        m.apply(electricity_sample(180.0, 70.0), 2.0);
        assert_eq!(m.carbon_hourly().values(), vec![220.0, 180.0]);
        assert_eq!(m.renewable_hourly().values(), vec![62.0, 70.0]);
        assert_eq!(m.unix_hour(), 1001);
    }
```

with this helper in the same test module:

```rust
    fn electricity_sample(carbon: f32, renewable: f32) -> Event {
        Event::Electricity {
            zone: "NL".into(),
            mix_mw: vec![(Source::Wind, 50.0), (Source::Gas, 50.0)],
            renewable_pct: renewable,
            fossil_free_pct: renewable + 5.0,
            carbon_gco2: carbon,
            updated_at: String::new(),
        }
    }
```

(`Source` is already imported at the top of `model.rs`.)

Add to `crates/sources/src/prometheus.rs` tests:

```rust
    const MATRIX: &str = r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1,"4.5"],[3,"4.7"],[5,"NaN"],[7,"5.1"]]}]}}"#;

    #[test]
    fn matrix_values_in_order_with_nan_as_nan() {
        let v = parse_matrix(MATRIX);
        assert_eq!(v.len(), 4);
        assert_eq!((v[0], v[1], v[3]), (4.5, 4.7, 5.1));
        assert!(v[2].is_nan());
        assert!(parse_matrix(r#"{"data":{"result":[]}}"#).is_empty());
        assert!(parse_matrix("nope").is_empty());
    }

    #[test]
    fn range_path_covers_the_spark_window() {
        let p = range_path(Q_CPU, 1000, 1060, 2);
        assert!(p.starts_with("/api/v1/query_range?query="));
        assert!(p.ends_with("&start=1000&end=1060&step=2"));
        let ev = history_event(vec![1.0, 2.0], vec![3.0], vec![4e6, 8e6]);
        assert_eq!(
            ev,
            Event::MetricsHistory {
                cpu: vec![1.0, 2.0],
                mem: vec![3.0],
                net_rx_mbps: vec![4.0, 8.0],
            }
        );
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core history model::tests::metrics_and_network && cargo test -p rackscreen-sources prometheus`
Expected: compile errors (`History`, `MetricsHistory`, `parse_matrix` missing).

- [ ] **Step 3: Implement the buffers**

Above the tests in `crates/core/src/history.rs`:

```rust
use std::collections::VecDeque;

use crate::anim::{Easing, Secs};

/// Samples at a nominal `step`, the newest last. Holds one more than `cap`
/// so the sparkline can slide the dropped point off the left edge.
#[derive(Clone, Debug, PartialEq)]
pub struct History {
    cap: usize,
    step: Secs,
    values: VecDeque<f32>,
    pushed_at: Option<Secs>,
}

impl History {
    pub fn new(cap: usize, step: Secs) -> History {
        History {
            cap: cap.max(1),
            step: step.max(0.1),
            values: VecDeque::with_capacity(cap + 1),
            pushed_at: None,
        }
    }

    /// Append a sample; non-finite values are skipped.
    pub fn push(&mut self, v: f32, now: Secs) {
        if !v.is_finite() {
            return;
        }
        self.values.push_back(v);
        while self.values.len() > self.cap + 1 {
            self.values.pop_front();
        }
        self.pushed_at = Some(now);
    }

    /// Replace the contents with the newest `cap + 1` finite values of
    /// `values` (oldest first). The line does not scroll after a prefill.
    pub fn prefill(&mut self, values: &[f32], _now: Secs) {
        let keep: Vec<f32> = values.iter().copied().filter(|v| v.is_finite()).collect();
        let start = keep.len().saturating_sub(self.cap + 1);
        self.values = keep[start..].iter().copied().collect();
        self.pushed_at = None;
    }

    /// The last `cap` samples, oldest first.
    pub fn series(&self) -> Vec<f32> {
        let start = self.values.len().saturating_sub(self.cap);
        self.values.iter().skip(start).copied().collect()
    }

    /// The sample just before `series`, once the buffer is full.
    pub fn prev(&self) -> Option<f32> {
        if self.values.len() > self.cap {
            self.values.front().copied()
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.values.len().min(self.cap)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn step(&self) -> Secs {
        self.step
    }

    pub fn mean(&self) -> Option<f32> {
        let s = self.series();
        if s.is_empty() {
            None
        } else {
            Some(s.iter().sum::<f32>() / s.len() as f32)
        }
    }

    /// Sparkline scroll phase: 0 right after a push (the line sits one step to
    /// the right), 1 once it has slid into place over the first 30 % of the step.
    pub fn scroll(&self, now: Secs) -> f32 {
        match self.pushed_at {
            Some(t) if self.prev().is_some() => {
                Easing::InOutCubic.apply(((now - t) / (0.3 * self.step)).clamp(0.0, 1.0) as f32)
            }
            _ => 1.0,
        }
    }
}

pub const HOURLY_CAP: i64 = 24;

/// One value per unix hour for the last 24 hours; a repeated hour replaces,
/// an older hour than the newest is ignored, gaps are simply absent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hourly {
    slots: VecDeque<(i64, f32)>,
}

impl Hourly {
    pub fn new() -> Hourly {
        Hourly::default()
    }

    pub fn push(&mut self, unix_hour: i64, v: f32) {
        if !v.is_finite() {
            return;
        }
        match self.slots.back_mut() {
            Some(b) if b.0 == unix_hour => b.1 = v,
            Some(b) if b.0 > unix_hour => return,
            _ => self.slots.push_back((unix_hour, v)),
        }
        while self
            .slots
            .front()
            .is_some_and(|f| unix_hour - f.0 >= HOURLY_CAP)
        {
            self.slots.pop_front();
        }
    }

    pub fn values(&self) -> Vec<f32> {
        self.slots.iter().map(|s| s.1).collect()
    }

    /// Values of hours at or after `unix_hour_from` (today's, for instance).
    pub fn values_since(&self, unix_hour_from: i64) -> Vec<f32> {
        self.slots
            .iter()
            .filter(|s| s.0 >= unix_hour_from)
            .map(|s| s.1)
            .collect()
    }

    pub fn mean(&self) -> Option<f32> {
        if self.slots.is_empty() {
            None
        } else {
            Some(self.slots.iter().map(|s| s.1).sum::<f32>() / self.slots.len() as f32)
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}
```

Add `pub mod history;` to `crates/core/src/lib.rs`.

- [ ] **Step 4: Wire the buffers into the model**

In `crates/core/src/event.rs` add to `Event` (after `Metrics { .. }`):

```rust
    /// Recent samples from a Prometheus range query at start, oldest first:
    /// cpu % every 2 s, mem % every 3 s, download Mb/s every 2 s.
    MetricsHistory {
        cpu: Vec<f32>,
        mem: Vec<f32>,
        net_rx_mbps: Vec<f32>,
    },
```

In `crates/core/src/model.rs`:

```rust
use crate::history::{History, Hourly};
// fields in `Model` (after `net_last_tick`):
    /// The sparkline buffers: cpu % at 2 s, mem % at 3 s, download Mb/s at 2 s.
    cpu_hist: History,
    mem_hist: History,
    net_hist: History,
    /// Carbon intensity and renewable share per unix hour, 24 h.
    carbon_hist: Hourly,
    renewable_hist: Hourly,
// in `Model::new`:
            cpu_hist: History::new(31, 2.0),
            mem_hist: History::new(31, 3.0),
            net_hist: History::new(31, 2.0),
            carbon_hist: Hourly::new(),
            renewable_hist: Hourly::new(),
// accessors (after `smooth_storage_pct`):
    pub fn cpu_history(&self) -> &History {
        &self.cpu_hist
    }
    pub fn mem_history(&self) -> &History {
        &self.mem_hist
    }
    pub fn net_history(&self) -> &History {
        &self.net_hist
    }
    pub fn carbon_hourly(&self) -> &Hourly {
        &self.carbon_hist
    }
    pub fn renewable_hourly(&self) -> &Hourly {
        &self.renewable_hist
    }
    /// The current unix hour, the hourly buffers' key.
    pub fn unix_hour(&self) -> i64 {
        self.unix_now.div_euclid(3600)
    }
```

In `apply`: in the `Event::Metrics` arm add after `self.mem.set(mem_pct, now);`:

```rust
                self.cpu_hist.push(cpu_pct, now);
                self.mem_hist.push(mem_pct, now);
```

add a new arm:

```rust
            Event::MetricsHistory {
                cpu,
                mem,
                net_rx_mbps,
            } => {
                self.cpu_hist.prefill(&cpu, now);
                self.mem_hist.prefill(&mem, now);
                self.net_hist.prefill(&net_rx_mbps, now);
            }
```

in the `Event::Network` arm add `self.net_hist.push((rx_bps / 1e6) as f32, now);`, and in the `Event::Electricity` arm (before `self.electricity = ElectricityState { .. }`):

```rust
                let hour = self.unix_hour();
                self.carbon_hist.push(hour, carbon_gco2);
                self.renewable_hist.push(hour, renewable_pct);
```

- [ ] **Step 5: Prefill from Prometheus**

In `crates/sources/src/prometheus.rs` add after `parse_vector`:

```rust
/// Values of the first series of a `query_range` matrix, in time order;
/// Prometheus' `"NaN"` strings come through as NaN.
pub fn parse_matrix(json: &str) -> Vec<f64> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(first) = v
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
    else {
        return Vec::new();
    };
    first
        .get("values")
        .and_then(|vs| vs.as_array())
        .map(|vs| {
            vs.iter()
                .filter_map(|pair| pair.as_array()?.get(1)?.as_str()?.parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn range_path(q: &str, start: i64, end: i64, step: u32) -> String {
    format!(
        "/api/v1/query_range?query={}&start={start}&end={end}&step={step}",
        urlencoding::encode(q)
    )
}

pub fn history_event(cpu: Vec<f64>, mem: Vec<f64>, net_rx_bps: Vec<f64>) -> Event {
    Event::MetricsHistory {
        cpu: cpu.into_iter().map(|v| v as f32).collect(),
        mem: mem.into_iter().map(|v| v as f32).collect(),
        net_rx_mbps: net_rx_bps.into_iter().map(|v| (v / 1e6) as f32).collect(),
    }
}

async fn query_range(t: &mut Tunnel, q: &str, start: i64, end: i64, step: u32) -> Result<String> {
    let r = t.get(&range_path(q, start, end, step), &[]).await?;
    anyhow::ensure!(r.status == 200, "prometheus HTTP {}", r.status);
    Ok(r.body)
}

/// One range query per sparkline so the lines are full at boot: cpu and
/// download over the last 60 s at 2 s, memory over 90 s at 3 s.
async fn prefill(t: &mut Tunnel) -> Result<Event> {
    let end = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cpu = parse_matrix(&query_range(t, Q_CPU, end - 60, end, 2).await?);
    let mem = parse_matrix(&query_range(t, Q_MEM, end - 90, end, 3).await?);
    let net = parse_matrix(&query_range(t, Q_NET_RX, end - 60, end, 2).await?);
    Ok(history_event(cpu, mem, net))
}
```

In `run_prometheus`, declare `let mut prefilled = false;` next to `let mut backoff = 5u64;`, and right after `backoff = 5;` (tunnel open succeeded):

```rust
        if !prefilled {
            match prefill(&mut t).await {
                Ok(ev) => {
                    ctx.emit(ev);
                    prefilled = true;
                }
                Err(e) => tracing::warn!("prometheus prefill: {e:#}"),
            }
        }
```

In `crates/sources/src/fake.rs` add a builder and include it in `initial()` right after `self.metrics()`:

```rust
    /// A plausible last minute, so the simulator's sparklines start full.
    fn metrics_history(&self) -> Event {
        let wave = |i: usize, base: f32, amp: f32, k: f32| base + amp * ((i as f32) * k).sin();
        Event::MetricsHistory {
            cpu: (0..31).map(|i| wave(i, self.cpu, 8.0, 0.45)).collect(),
            mem: (0..31).map(|i| wave(i, self.mem, 1.5, 0.3)).collect(),
            net_rx_mbps: (0..31)
                .map(|i| wave(i, self.net_rx as f32 / 1e6, 12.0, 0.5).max(0.0))
                .collect(),
        }
    }
```

and extend the fake test `initial_has_every_new_role_fed` with `assert!(evs.iter().any(|e| matches!(e, Event::MetricsHistory { cpu, .. } if cpu.len() == 31)));`.

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test --workspace`
Expected: green, including the three new core tests and the two new prometheus tests.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src crates/sources/src
git commit -m "feat(core,sources): history buffers for cpu, mem, net, carbon and renewable; Prometheus range prefill

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Data additions: hourly forecast, AQI history, Argo sync history, Longhorn sizes, moon and sun helpers

**Files:**
- Modify: `crates/core/src/event.rs` (`Forecast`, `AirQualityHistory`, `VolumeSizes`, `AppSyncHistory`), `crates/core/src/model.rs` (state fields, `DeployLog`, folds, local-time helpers)
- Create: `crates/core/src/sky.rs`; modify `crates/core/src/lib.rs`
- Modify: `crates/sources/src/open_meteo.rs`, `crates/sources/src/argocd.rs`, `crates/sources/src/prometheus.rs`, `crates/sources/src/fake.rs`
- Create: `crates/sources/tests/fixtures/open-meteo-forecast-hourly.json`, `crates/sources/tests/fixtures/open-meteo-air-quality-hourly.json`
- Test: unit tests in `sky.rs`, `model.rs`, `open_meteo.rs`, `argocd.rs`, `prometheus.rs`

**Interfaces:**
- Produces: `Event::Forecast { hourly_temp_c, hourly_precip_mm, day_min_c, day_max_c }`, `Event::AirQualityHistory { hourly }`, `Event::VolumeSizes(Vec<(String, u64)>)`, `Event::AppSyncHistory { finished_at: Vec<i64> }`; `WeatherState::{hourly_temp_c, hourly_precip_mm, day_min_c, day_max_c}`; `AirState::hourly`; `ClusterState::volume_bytes`; `DeployLog { day, hours: [u32; 24], last: Option<(String, Secs)> }` via `Model::deploys_log()`; `Model::{local_secs(), local_day(), local_hour(), local_midnight_unix_hour()}`; `sky::{moon_days, SunEvent, next_sun_event, daylight_secs, sun_progress}`; `argocd::Parsed { app, phase, finished_at }`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/sky.rs` with tests only:

```rust
//! Sun and moon derived values for the P2 cards: days to the next full and
//! new moon, the next sun event, daylight and progress through day or night.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_days_follow_the_synodic_month() {
        let (full, new) = moon_days(0.0, true);
        assert!((full - SYNODIC_DAYS / 2.0).abs() < 0.01 && (new - 0.0).abs() < 0.01 || (new - SYNODIC_DAYS).abs() < 0.01);
        let (full, new) = moon_days(1.0, true);
        assert!(full < 0.01 || (full - SYNODIC_DAYS).abs() < 0.01);
        assert!((new - SYNODIC_DAYS / 2.0).abs() < 0.01);
        let (full, new) = moon_days(0.5, true);
        assert!((full - SYNODIC_DAYS / 4.0).abs() < 0.01, "first quarter: a week to full");
        assert!((new - 3.0 * SYNODIC_DAYS / 4.0).abs() < 0.01);
        let (full, new) = moon_days(0.5, false);
        assert!((full - 3.0 * SYNODIC_DAYS / 4.0).abs() < 0.01, "last quarter: three weeks to full");
        assert!((new - SYNODIC_DAYS / 4.0).abs() < 0.01);
    }

    #[test]
    fn next_event_daylight_and_progress() {
        let (rise, set) = (Some(7 * 3600), Some(19 * 3600));
        assert_eq!(next_sun_event(rise, set, 12 * 3600), Some((SunEvent::Sunset, 19 * 3600)));
        assert_eq!(next_sun_event(rise, set, 5 * 3600), Some((SunEvent::Sunrise, 7 * 3600)));
        assert_eq!(next_sun_event(rise, set, 20 * 3600), Some((SunEvent::Sunrise, 31 * 3600)), "tonight: tomorrow's sunrise");
        assert_eq!(next_sun_event(Some(31 * 3600), set, 20 * 3600), Some((SunEvent::Sunrise, 31 * 3600)), "the source already rolled sunrise");
        assert_eq!(next_sun_event(None, set, 0), None);
        assert_eq!(daylight_secs(rise, set), Some(12 * 3600));
        assert_eq!(daylight_secs(Some(31 * 3600), set), Some(12 * 3600));
        assert!((sun_progress(rise, set, 13 * 3600).unwrap() - 0.5).abs() < 1e-6, "midday");
        assert!((sun_progress(rise, set, 22 * 3600).unwrap() - 0.25).abs() < 1e-6, "three hours into a twelve hour night");
        assert!((sun_progress(rise, set, 4 * 3600).unwrap() - 0.75).abs() < 1e-6, "three hours before sunrise");
        assert_eq!(sun_progress(None, None, 0), None);
    }
}
```

Add to `crates/core/src/model.rs` tests:

```rust
    #[test]
    fn forecast_air_history_and_volume_sizes_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Forecast {
                hourly_temp_c: vec![13.4, 13.6],
                hourly_precip_mm: vec![0.0, 1.1],
                day_min_c: 10.3,
                day_max_c: 13.6,
            },
            0.0,
        );
        assert_eq!(m.weather().hourly_temp_c, vec![13.4, 13.6]);
        assert_eq!(m.weather().day_min_c, Some(10.3));
        assert!(!m.weather().have, "a forecast alone is not current weather");
        m.apply(Event::AirQualityHistory { hourly: vec![58.0, 55.0] }, 0.0);
        assert_eq!(m.air().hourly, vec![58.0, 55.0]);
        m.apply(Event::VolumeSizes(vec![("pvc-a".into(), 7)]), 0.0);
        assert_eq!(m.state().volume_bytes, vec![("pvc-a".to_string(), 7)]);
    }

    #[test]
    fn deploys_are_counted_per_local_hour_today() {
        let mut m = Model::new(Thresholds::default());
        // 2026-09-07 12:00 UTC in Amsterdam (+2): local 14:00
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        assert_eq!((m.local_hour(), m.local_secs() % 86_400), (14, 14 * 3600));
        m.apply(
            Event::AppSyncHistory {
                finished_at: vec![
                    1_788_782_400 - 3 * 3600,  // 11:00 today
                    1_788_782_400 - 20 * 3600, // 18:00 yesterday
                    1_788_782_400 + 60,        // 14:01 today
                ],
            },
            0.0,
        );
        let log = m.deploys_log();
        assert_eq!(log.hours[11], 1);
        assert_eq!(log.hours[14], 1);
        assert_eq!(log.hours.iter().sum::<u32>(), 2);
        m.apply(Event::AppSynced { name: "grafana".into() }, 5.0);
        assert_eq!(m.deploys_log().hours[14], 2);
        assert_eq!(m.deploys_log().last, Some(("grafana".to_string(), 5.0)));
        // the next local day empties the buckets but keeps the last sync
        m.set_unix_now(1_788_782_400 + 12 * 3600);
        m.tick(6.0);
        assert_eq!(m.deploys_log().hours.iter().sum::<u32>(), 0);
        assert!(m.deploys_log().last.is_some());
    }
```

Add to `crates/sources/src/open_meteo.rs` tests:

```rust
    const FCH: &str = include_str!("../tests/fixtures/open-meteo-forecast-hourly.json");
    const AQH: &str = include_str!("../tests/fixtures/open-meteo-air-quality-hourly.json");

    #[test]
    fn parses_hourly_forecast_and_daily_range() {
        let ev = parse_forecast_hours(FCH).unwrap();
        assert_eq!(
            ev,
            Event::Forecast {
                hourly_temp_c: vec![13.4, 13.6, 13.3, 12.3, 12.4, 12.0, 11.8, 11.6, 11.2, 11.5, 11.3, 10.8],
                hourly_precip_mm: vec![0.0, 0.0, 0.0, 1.1, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                day_min_c: 10.3,
                day_max_c: 13.6,
            }
        );
        assert!(parse_forecast_hours(FC).is_err(), "the old fixture has no hourly block");
    }

    #[test]
    fn air_history_ends_at_the_current_hour_and_skips_nulls() {
        // fixture: 2026-11-01T00:00 .. 2026-11-02T23:00 UTC, index 20 is null
        let now = 1_793_620_800; // 2026-11-02T12:00:00Z
        let h = parse_air_quality_history(AQH, now).unwrap();
        assert_eq!(h.len(), 23, "24 hours ending at 12:00 minus the null at 20:00 yesterday");
        assert_eq!(h[0], 55.0, "13:00 yesterday");
        assert_eq!(*h.last().unwrap(), 26.0, "12:00 today");
        assert_eq!(parse_air_quality_history(AQH, 1_793_577_600 - 7200).unwrap().len(), 22, "22:00 yesterday: the 23 values up to it minus the null");
    }

    #[test]
    fn urls_ask_for_the_hourly_blocks() {
        let cfg = WeatherConfig { lat: 52.37, lon: 4.89, poll_secs: 600 };
        assert!(forecast_url(&cfg).contains("&hourly=temperature_2m,precipitation&daily=temperature_2m_min,temperature_2m_max&forecast_hours=12&forecast_days=1&timezone=UTC"));
        assert!(air_quality_url(&cfg).ends_with("&current=european_aqi&hourly=european_aqi&past_days=1&forecast_days=1&timezone=UTC"));
    }
```

and replace the old `urls_carry_the_location` assertions with:

```rust
        assert!(forecast_url(&cfg).starts_with("https://api.open-meteo.com/v1/forecast?latitude=52.37&longitude=4.89&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day"));
        assert!(air_quality_url(&cfg).starts_with("https://air-quality-api.open-meteo.com/v1/air-quality?latitude=52.37&longitude=4.89&current=european_aqi"));
```

Create `crates/sources/tests/fixtures/open-meteo-forecast-hourly.json`:

```json
{"latitude":52.366,"longitude":4.901,"utc_offset_seconds":0,"timezone":"GMT",
"current":{"time":"2026-11-02T12:30","interval":900,"temperature_2m":13.5,"weather_code":0,"wind_speed_10m":18.0,"wind_direction_10m":11,"wind_gusts_10m":34.2,"is_day":1},
"hourly":{"time":["2026-11-02T12:00","2026-11-02T13:00","2026-11-02T14:00","2026-11-02T15:00","2026-11-02T16:00","2026-11-02T17:00","2026-11-02T18:00","2026-11-02T19:00","2026-11-02T20:00","2026-11-02T21:00","2026-11-02T22:00","2026-11-02T23:00"],
"temperature_2m":[13.4,13.6,13.3,12.3,12.4,12.0,11.8,11.6,11.2,11.5,11.3,10.8],
"precipitation":[0.00,0.00,0.00,1.10,0.10,0.10,0.00,0.00,0.00,0.00,0.00,0.00]},
"daily":{"time":["2026-11-02"],"temperature_2m_min":[10.3],"temperature_2m_max":[13.6]}}
```

Create `crates/sources/tests/fixtures/open-meteo-air-quality-hourly.json` (48 hourly entries from `2026-11-01T00:00` to `2026-11-02T23:00`; the value at index 20 is `null`; values climb from 58 down to 26 at index 36):

```json
{"latitude":52.35,"longitude":4.9,"utc_offset_seconds":0,"timezone":"GMT",
"current":{"time":"2026-11-02T12:00","interval":3600,"european_aqi":26},
"hourly":{"time":["2026-11-01T00:00","2026-11-01T01:00","2026-11-01T02:00","2026-11-01T03:00","2026-11-01T04:00","2026-11-01T05:00","2026-11-01T06:00","2026-11-01T07:00","2026-11-01T08:00","2026-11-01T09:00","2026-11-01T10:00","2026-11-01T11:00","2026-11-01T12:00","2026-11-01T13:00","2026-11-01T14:00","2026-11-01T15:00","2026-11-01T16:00","2026-11-01T17:00","2026-11-01T18:00","2026-11-01T19:00","2026-11-01T20:00","2026-11-01T21:00","2026-11-01T22:00","2026-11-01T23:00","2026-11-02T00:00","2026-11-02T01:00","2026-11-02T02:00","2026-11-02T03:00","2026-11-02T04:00","2026-11-02T05:00","2026-11-02T06:00","2026-11-02T07:00","2026-11-02T08:00","2026-11-02T09:00","2026-11-02T10:00","2026-11-02T11:00","2026-11-02T12:00","2026-11-02T13:00","2026-11-02T14:00","2026-11-02T15:00","2026-11-02T16:00","2026-11-02T17:00","2026-11-02T18:00","2026-11-02T19:00","2026-11-02T20:00","2026-11-02T21:00","2026-11-02T22:00","2026-11-02T23:00"],
"european_aqi":[58,55,54,52,50,49,48,47,46,45,44,43,42,55,41,40,39,38,37,36,null,35,34,33,32,31,30,29,28,28,27,27,27,26,26,26,26,25,25,24,24,23,23,22,22,21,21,20]}}
```

Replace the argocd tests `parses_the_fixture` and the tracker test's `parse_app` uses with the struct form, and add a history assertion:

```rust
    #[test]
    fn parses_the_fixture() {
        let o: DynamicObject = serde_json::from_str(APP).unwrap();
        let p = parse_app(&o);
        assert_eq!(
            p.app,
            App {
                name: "arr-stack".into(),
                sync: AppSync::Synced,
                health: AppHealth::Healthy,
                operating: false,
            }
        );
        assert_eq!(p.phase.as_deref(), Some("Succeeded"));
        assert_eq!(p.finished_at, Some(1_788_627_225), "2026-09-05T16:53:45Z");
        let running = parse_app(&obj("OutOfSync", "Progressing", Some("Running")));
        assert!(running.app.operating);
        assert_eq!(running.app.sync, AppSync::OutOfSync);
        assert_eq!(running.app.health, AppHealth::Progressing);
        let bare = parse_app(&obj("Weird", "Odd", None));
        assert_eq!(
            (bare.app.sync, bare.app.health, bare.phase, bare.finished_at),
            (AppSync::Unknown, AppHealth::Unknown, None, None)
        );
    }
```

and in `tracker_snapshots_after_init_and_emits_edges` change the init assertions to:

```rust
        let done = t.init_done();
        assert_eq!(done.len(), 2);
        assert_eq!(
            done[0],
            Event::AppSyncHistory {
                finished_at: vec![1_788_627_225]
            }
        );
        assert!(matches!(&done[1], Event::Apps(list) if list.len() == 1));
```

(every other `t.apply(parse_app(..))` call stays as it is: `apply` takes the struct).

Add to the prometheus tests:

```rust
    #[test]
    fn volume_sizes_keep_the_largest_sample_per_volume() {
        let v = parse_vector(
            r#"{"data":{"result":[
            {"metric":{"volume":"pvc-a","node":"n1"},"value":[1,"100"]},
            {"metric":{"volume":"pvc-a","node":"n2"},"value":[1,"120"]},
            {"metric":{"volume":"pvc-b","node":"n1"},"value":[1,"7"]}]}}"#,
        );
        assert_eq!(volume_sizes_from(&v), vec![("pvc-a".into(), 120), ("pvc-b".into(), 7)]);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core sky model::tests::forecast model::tests::deploys && cargo test -p rackscreen-sources`
Expected: compile errors in all four crates' tests.

- [ ] **Step 3: Implement the core side**

Above the tests in `crates/core/src/sky.rs`:

```rust
use std::f32::consts::TAU;

pub const SYNODIC_DAYS: f32 = 29.530_588;

/// Days until the next full and the next new moon, from the illuminated
/// fraction and whether it is growing (a mean synodic month).
pub fn moon_days(illumination: f32, waxing: bool) -> (f32, f32) {
    let p = (1.0 - 2.0 * illumination.clamp(0.0, 1.0)).acos() / TAU;
    let phase = if waxing { p } else { 1.0 - p };
    let to_full = (0.5 - phase).rem_euclid(1.0) * SYNODIC_DAYS;
    let to_new = (1.0 - phase).rem_euclid(1.0) * SYNODIC_DAYS;
    (to_full, to_new)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SunEvent {
    Sunrise,
    Sunset,
}

/// The next event after `unix`: sunset by day, sunrise by night (tomorrow's
/// when the source has not rolled it yet). `None` on polar days and nights.
pub fn next_sun_event(sunrise: Option<i64>, sunset: Option<i64>, unix: i64) -> Option<(SunEvent, i64)> {
    let (r, s) = (sunrise?, sunset?);
    if unix >= r && unix < s {
        Some((SunEvent::Sunset, s))
    } else if unix < r {
        Some((SunEvent::Sunrise, r))
    } else {
        Some((SunEvent::Sunrise, if r > s { r } else { r + 86_400 }))
    }
}

pub fn daylight_secs(sunrise: Option<i64>, sunset: Option<i64>) -> Option<i64> {
    let (r, s) = (sunrise?, sunset?);
    let r = if r > s { r - 86_400 } else { r };
    (s > r).then_some(s - r)
}

/// Fraction of the current span that has passed: of the daylight by day, of
/// the night by night.
pub fn sun_progress(sunrise: Option<i64>, sunset: Option<i64>, unix: i64) -> Option<f32> {
    let (r, s) = (sunrise?, sunset?);
    let day = daylight_secs(sunrise, sunset)? as f32;
    let night = 86_400.0 - day;
    let r_today = if r > s { r - 86_400 } else { r };
    let f = if unix >= r_today && unix < s {
        (unix - r_today) as f32 / day
    } else if unix >= s {
        (unix - s) as f32 / night
    } else {
        (unix - (s - 86_400)) as f32 / night
    };
    Some(f.clamp(0.0, 1.0))
}
```

Add `pub mod sky;` to `crates/core/src/lib.rs`.

In `crates/core/src/event.rs` add to `Event`:

```rust
    /// Hourly forecast from Open-Meteo: the next 12 hours from the current
    /// hour, plus today's low and high.
    Forecast {
        hourly_temp_c: Vec<f32>,
        hourly_precip_mm: Vec<f32>,
        day_min_c: f32,
        day_max_c: f32,
    },
    /// The last 24 hourly EAQI values ending at the current hour, oldest first,
    /// gaps skipped.
    AirQualityHistory {
        hourly: Vec<f32>,
    },
    /// Longhorn actual size per volume in bytes.
    VolumeSizes(Vec<(String, u64)>),
    /// `finishedAt` of every Argo CD application's last operation, unix seconds,
    /// sent once after the initial list so today's sync count survives a restart.
    AppSyncHistory {
        finished_at: Vec<i64>,
    },
```

In `crates/core/src/model.rs`:

```rust
// WeatherState gains:
    pub hourly_temp_c: Vec<f32>,
    pub hourly_precip_mm: Vec<f32>,
    pub day_min_c: Option<f32>,
    pub day_max_c: Option<f32>,
// AirState gains:
    pub hourly: Vec<f32>,
// ClusterState gains:
    pub volume_bytes: Vec<(String, u64)>,

/// Argo syncs per local hour today, and the last one seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeployLog {
    /// Local day index (days since the epoch in local time) the buckets belong to.
    pub day: i64,
    pub hours: [u32; 24],
    /// App name and the monotonic time of the last sync.
    pub last: Option<(String, Secs)>,
}
// Model field + init:
    deploys: DeployLog,
            deploys: DeployLog::default(),
// accessors:
    pub fn deploys_log(&self) -> &DeployLog {
        &self.deploys
    }
    /// Seconds since the epoch on the local clock.
    pub fn local_secs(&self) -> i64 {
        self.unix_now + self.utc_offset_secs as i64
    }
    pub fn local_day(&self) -> i64 {
        self.local_secs().div_euclid(86_400)
    }
    pub fn local_hour(&self) -> usize {
        (self.local_secs().rem_euclid(86_400) / 3600) as usize
    }
    /// The unix hour at local midnight, the first hour "today" in the hourly buffers.
    pub fn local_midnight_unix_hour(&self) -> i64 {
        (self.local_day() * 86_400 - self.utc_offset_secs as i64).div_euclid(3600)
    }
    fn roll_deploy_day(&mut self) {
        let day = self.local_day();
        if self.deploys.day != day {
            self.deploys.day = day;
            self.deploys.hours = [0; 24];
        }
    }
```

In `apply`, replace the `Event::AppSynced { .. } => self.fx.push(FxRequest::AppSynced),` arm with:

```rust
            Event::AppSynced { name } => {
                self.roll_deploy_day();
                let h = self.local_hour();
                self.deploys.hours[h] += 1;
                self.deploys.last = Some((name, now));
                self.fx.push(FxRequest::AppSynced);
            }
            Event::AppSyncHistory { finished_at } => {
                self.roll_deploy_day();
                let offset = self.utc_offset_secs as i64;
                for t in finished_at {
                    let local = t + offset;
                    if local.div_euclid(86_400) == self.deploys.day {
                        self.deploys.hours[(local.rem_euclid(86_400) / 3600) as usize] += 1;
                    }
                }
            }
            Event::Forecast {
                hourly_temp_c,
                hourly_precip_mm,
                day_min_c,
                day_max_c,
            } => {
                self.weather.hourly_temp_c = hourly_temp_c;
                self.weather.hourly_precip_mm = hourly_precip_mm;
                self.weather.day_min_c = Some(day_min_c);
                self.weather.day_max_c = Some(day_max_c);
            }
            Event::AirQualityHistory { hourly } => self.air.hourly = hourly,
            Event::VolumeSizes(sizes) => self.state.volume_bytes = sizes,
```

The `Event::Weather` arm builds a fresh `WeatherState { .. }`: keep the forecast fields across it by writing

```rust
                self.weather = WeatherState {
                    temp_c,
                    code,
                    is_day,
                    wind_kmh,
                    gust_kmh,
                    wind_from_deg,
                    at,
                    have: true,
                    ..std::mem::take(&mut self.weather)
                };
```

and the `Event::AirQuality` arm likewise: `self.air = AirState { eaqi, have: true, ..std::mem::take(&mut self.air) };`. Add `self.roll_deploy_day();` as the first line of `Model::tick`.

- [ ] **Step 4: Implement the sources side**

`crates/sources/src/open_meteo.rs`:

```rust
pub fn forecast_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&hourly=temperature_2m,precipitation&daily=temperature_2m_min,temperature_2m_max&forecast_hours=12&forecast_days=1&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

pub fn air_quality_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=european_aqi&hourly=european_aqi&past_days=1&forecast_days=1&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

fn f32s(v: &Value, path: &[&str]) -> Result<Vec<Option<f32>>> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p).with_context(|| format!("{} missing", path.join(".")))?;
    }
    Ok(cur
        .as_array()
        .with_context(|| format!("{} is not an array", path.join(".")))?
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect())
}

/// The hourly and daily blocks of the same forecast response.
pub fn parse_forecast_hours(json: &str) -> Result<Event> {
    let v: Value = serde_json::from_str(json).context("forecast json")?;
    let temps = f32s(&v, &["hourly", "temperature_2m"])?;
    let precip = f32s(&v, &["hourly", "precipitation"])?;
    let min = f32s(&v, &["daily", "temperature_2m_min"])?;
    let max = f32s(&v, &["daily", "temperature_2m_max"])?;
    Ok(Event::Forecast {
        hourly_temp_c: temps.into_iter().map(|t| t.unwrap_or(f32::NAN)).collect(),
        hourly_precip_mm: precip.into_iter().map(|p| p.unwrap_or(0.0)).collect(),
        day_min_c: min.first().copied().flatten().context("daily min")?,
        day_max_c: max.first().copied().flatten().context("daily max")?,
    })
}

/// Unix seconds of an Open-Meteo `YYYY-MM-DDTHH:MM` stamp (UTC).
fn stamp_unix(s: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
        .ok()
        .map(|d| d.and_utc().timestamp())
}

/// The 24 hourly values ending at the hour of `now_unix`, oldest first, nulls skipped.
pub fn parse_air_quality_history(json: &str, now_unix: i64) -> Result<Vec<f32>> {
    let v: Value = serde_json::from_str(json).context("air quality json")?;
    let times = v
        .get("hourly")
        .and_then(|h| h.get("time"))
        .and_then(Value::as_array)
        .context("hourly.time missing")?;
    let values = f32s(&v, &["hourly", "european_aqi"])?;
    let hour = now_unix.div_euclid(3600) * 3600;
    let end = times
        .iter()
        .rposition(|t| t.as_str().and_then(stamp_unix).is_some_and(|u| u <= hour))
        .context("no hourly value up to now")?;
    let start = (end + 1).saturating_sub(24);
    Ok(values[start..=end].iter().flatten().copied().collect())
}
```

Add `chrono` to the sources imports where needed (it is already a dependency). In `run_weather`, after `ctx.emit(ev);` for the weather event, parse the same body again for the forecast; restructure the `Ok` branch so the body is available:

```rust
        match fetch(&forecast_url(&cfg)).await {
            Ok(body) => match parse_forecast(&body) {
                Ok(ev) => {
                    failures = 0;
                    if let Event::Weather { temp_c, code, .. } = &ev {
                        tracing::info!("weather: poll ok ({temp_c:.1} °C, code {code})");
                    }
                    ctx.emit(Event::Link {
                        target: LinkTarget::Weather,
                        up: true,
                    });
                    ctx.emit(ev);
                    match parse_forecast_hours(&body) {
                        Ok(fc) => ctx.emit(fc),
                        Err(e) => tracing::warn!("forecast hours: {e:#}"),
                    }
                    // air quality rides along; a miss here is a warning, not a link edge
                    match fetch(&air_quality_url(&cfg)).await {
                        Ok(aq) => {
                            match parse_air_quality(&aq) {
                                Ok(eaqi) => ctx.emit(Event::AirQuality { eaqi }),
                                Err(e) => tracing::warn!("air quality: {e:#}"),
                            }
                            let now = chrono::Utc::now().timestamp();
                            match parse_air_quality_history(&aq, now) {
                                Ok(hourly) => ctx.emit(Event::AirQualityHistory { hourly }),
                                Err(e) => tracing::warn!("air quality history: {e:#}"),
                            }
                        }
                        Err(e) => tracing::warn!("air quality: {e:#}"),
                    }
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("weather: {e:#}");
                }
            },
            Err(e) => {
                failures += 1;
                tracing::warn!("weather: {e:#}");
            }
        }
        if failures >= 2 {
            ctx.emit(Event::Link {
                target: LinkTarget::Weather,
                up: false,
            });
        }
```

`crates/sources/src/argocd.rs`: replace `parse_app` and the tracker's `apply`/`init_done`:

```rust
/// The app, the raw operation phase (`Running`, `Succeeded`, ...) the tracker
/// needs for the sync-finished edge, and when that operation finished.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub app: App,
    pub phase: Option<String>,
    pub finished_at: Option<i64>,
}

pub fn parse_app(obj: &DynamicObject) -> Parsed {
    let status = obj.data.get("status").cloned().unwrap_or(Value::Null);
    let sync = match status_str(&status, &["sync", "status"]) {
        Some("Synced") => AppSync::Synced,
        Some("OutOfSync") => AppSync::OutOfSync,
        _ => AppSync::Unknown,
    };
    let health = match status_str(&status, &["health", "status"]) {
        Some("Healthy") => AppHealth::Healthy,
        Some("Progressing") => AppHealth::Progressing,
        Some("Degraded") => AppHealth::Degraded,
        Some("Suspended") => AppHealth::Suspended,
        Some("Missing") => AppHealth::Missing,
        _ => AppHealth::Unknown,
    };
    let phase = status_str(&status, &["operationState", "phase"]).map(str::to_string);
    let finished_at = status_str(&status, &["operationState", "finishedAt"])
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp());
    Parsed {
        app: App {
            name: obj.metadata.name.clone().unwrap_or_default(),
            sync,
            health,
            operating: phase.as_deref() == Some("Running"),
        },
        phase,
        finished_at,
    }
}
```

In `AppTracker` add a field `finished: HashMap<String, i64>`, clear it in `begin_init`, and:

```rust
    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        let mut finished_at: Vec<i64> = self.finished.values().copied().collect();
        finished_at.sort_unstable();
        vec![Event::AppSyncHistory { finished_at }, self.snapshot()]
    }

    pub fn apply(&mut self, parsed: Parsed) -> Vec<Event> {
        let Parsed {
            app,
            phase,
            finished_at,
        } = parsed;
        match finished_at {
            Some(t) => {
                self.finished.insert(app.name.clone(), t);
            }
            None => {
                self.finished.remove(&app.name);
            }
        }
        // ... the existing body from `let mut out = Vec::new();` on, unchanged
```

and in `delete` also `self.finished.remove(name);`.

`crates/sources/src/prometheus.rs`: add `const Q_LH_SIZE: &str = "longhorn_volume_actual_size_bytes";` and

```rust
/// Actual size per volume; Longhorn exports one series per node holding a
/// replica, so the largest sample per name wins.
pub fn volume_sizes_from(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, u64)> {
    let mut map: std::collections::BTreeMap<String, u64> = Default::default();
    for (m, val) in v {
        let Some(name) = m.get("volume") else {
            continue;
        };
        let bytes = val.max(0.0) as u64;
        let e = map.entry(name.clone()).or_insert(bytes);
        *e = (*e).max(bytes);
    }
    map.into_iter().collect()
}
```

and in `poll`, inside `if !vols.is_empty() { .. }` after pushing `Event::Storage`:

```rust
        let sizes = volume_sizes_from(&parse_vector(&query(t, Q_LH_SIZE).await?));
        if !sizes.is_empty() {
            out.push(Event::VolumeSizes(sizes));
        }
```

`crates/sources/src/fake.rs`: add builders and emit them (in `initial()` after `self.weather()`, `self.air()`, `self.storage()` and `self.apps()` respectively; in `tick()` push `self.forecast()` after `self.weather()`, `self.air_history()` after `self.air()`, `self.volume_sizes()` after `self.storage()`):

```rust
    /// Twelve hours that cool off after a shower in the afternoon.
    fn forecast(&self) -> Event {
        let hourly_temp_c: Vec<f32> = (0..12)
            .map(|i| self.temp + 3.0 * ((i as f32 + 2.0) / 12.0 * std::f32::consts::PI).sin() - 1.0)
            .collect();
        let hourly_precip_mm: Vec<f32> = (0..12)
            .map(|i| if (5..8).contains(&i) { 0.8 } else { 0.0 })
            .collect();
        Event::Forecast {
            day_min_c: hourly_temp_c.iter().cloned().fold(f32::MAX, f32::min) - 2.0,
            day_max_c: hourly_temp_c.iter().cloned().fold(f32::MIN, f32::max) + 1.0,
            hourly_temp_c,
            hourly_precip_mm,
        }
    }

    fn air_history(&self) -> Event {
        Event::AirQualityHistory {
            hourly: (0..24)
                .map(|i| (self.eaqi + 10.0 * ((i as f32) / 24.0 * std::f32::consts::TAU * 1.5).sin()).max(1.0))
                .collect(),
        }
    }

    fn volume_sizes(&self) -> Event {
        let n = self.volumes.len().max(1) as u64;
        Event::VolumeSizes(
            self.volumes
                .iter()
                .enumerate()
                .map(|(i, (name, _))| (name.clone(), self.storage_used * (n - i as u64) * 2 / (n * (n + 1))))
                .collect(),
        )
    }

    /// Yesterday evening and this morning, on the fake clock.
    fn app_sync_history(&self) -> Event {
        let now = self.unix_now();
        Event::AppSyncHistory {
            finished_at: vec![now - 5 * 3600, now - 3 * 3600, now - 3 * 3600 + 600, now - 20 * 3600],
        }
    }
```

Extend the fake test `initial_has_every_new_role_fed` with one `assert!` per new event kind (`Forecast`, `AirQualityHistory`, `VolumeSizes`, `AppSyncHistory`).

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test --workspace`
Expected: green. (`Event` gained variants; the match in `Model::apply` is exhaustive once the arms above are in.)

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src crates/sources
git commit -m "feat(core,sources): hourly forecast, AQI history, Argo sync history, Longhorn volume sizes, sun and moon helpers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Screens: cpu, mem, storage, pods, health (+ torrent mode), deploys

**Files:**
- Modify: `crates/core/src/format.rs` (new formatters)
- Create: `crates/core/src/p2/roles/mod.rs`, `crates/core/src/p2/roles/cluster.rs`
- Modify: `crates/core/src/p2/mod.rs`, `crates/core/src/scene.rs` (`role_scene` dispatch + tests), `crates/core/src/scene_fx.rs` (tests)
- Modify: `crates/render/tests/golden.rs` (pixel asserts) and re-capture goldens
- Test: unit tests in `format.rs`, `roles/mod.rs`, `roles/cluster.rs`

**Interfaces:**
- Consumes: `P2Screen`, `compose`, `RingSpec`, `Card`, `Model::{cpu_history, mem_history, deploys_log, local_hour, torrent_mode, torrent_badge_list}`.
- Produces: `format::{fmt_ago, fmt_hhmm, fmt_in, fmt_hm, fmt_mmss, fmt_capacity}`; `p2::roles::screen(model, role, now) -> Option<P2Screen>` (the single dispatch every later screen task extends); `p2::roles::short_node`; `cluster::{cpu, mem, storage, pods, health, torrent, deploys, app_color, teal_shade}`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/src/format.rs` tests:

```rust
    #[test]
    fn p2_formatters() {
        assert_eq!(fmt_ago(2.0), "just now");
        assert_eq!(fmt_ago(42.0), "42 s ago");
        assert_eq!(fmt_ago(150.0), "2 m ago");
        assert_eq!(fmt_ago(7300.0), "2 h ago");
        assert_eq!(fmt_hhmm(16 * 3600 + 48 * 60), "16:48");
        assert_eq!(fmt_hhmm(86_400 + 5 * 60), "00:05", "wraps the day");
        assert_eq!(fmt_in(2 * 3600 + 14 * 60), "in 2h 14m");
        assert_eq!(fmt_in(14 * 60 + 30), "in 15 min");
        assert_eq!(fmt_in(20), "now");
        assert_eq!(fmt_hm(8 * 3600 + 56 * 60), "8h 56m");
        assert_eq!(fmt_mmss(14 * 60 + 32), "14:32");
        assert_eq!(fmt_mmss(5), "00:05");
        assert_eq!(fmt_mmss(100 * 60), "1h40");
        let tb = 1u64 << 40;
        assert_eq!(fmt_capacity(tb + tb / 5, 3 * tb + tb / 10), ("1.2 of 3.1 TB".to_string(), "1.9 TB free".to_string()));
        let gb = 1u64 << 30;
        assert_eq!(fmt_capacity(49 * gb, 128 * gb), ("49 of 128 GB".to_string(), "79 GB free".to_string()));
        assert_eq!(fmt_capacity(0, 0).0, "0 of 0 GB");
    }
```

Create `crates/core/src/p2/roles/mod.rs` with the dispatcher's tests only:

```rust
//! Role builders: `Model` → `P2Screen`, plain data the composer draws.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::layout::HERO_MAX_W;
    use crate::p2::text::hero_width;

    #[test]
    fn worst_case_heroes_fit_180_px() {
        for (text, scale) in [
            ("100", 1.0),   // cpu, mem, storage, ups, renewable, moon
            ("999", 1.0),   // pods, deploys, contributions, carbon
            ("16/16", 0.82), // cluster
            ("-12", 1.0),   // weather below zero
            ("59:59", 0.82), // iss
            ("-0.123", 0.78), // price
            ("1000", 1.0),  // net Mb/s
            ("120", 1.0),   // wind, aqi, rain minutes
            ("23:59", 0.82), // sun
        ] {
            let w = hero_width(text, scale);
            assert!(w <= HERO_MAX_W, "{text} at {scale}: {w} px");
        }
    }

    #[test]
    fn node_labels_are_two_characters() {
        assert_eq!(short_node("raspberrypi-5-8gb-3"), "r3");
        assert_eq!(short_node("hp-elitedesk-800-g5-i7"), "i7");
        assert_eq!(short_node("node"), "no");
        assert_eq!(short_node("k3s-worker-12"), "k12");
    }
}
```

Create `crates/core/src/p2/roles/cluster.rs` with tests only:

```rust
//! Cluster roles: cpu, mem, storage, pods, health (and its torrent mode), deploys.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{App, AppHealth, AppSync, Event, LinkTarget, Robustness, Torrent};
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{BLUE, CRIT, INDIGO, OK, TEAL, TEXT2, TEXT3, VIOLET, WARN};

    fn base() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Metrics {
                cpu_pct: 42.4,
                mem_pct: 67.0,
                mem_used_gb: 10.7,
                mem_total_gb: 16.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 7,
                total: 7,
                not_ready: vec![],
            },
            0.0,
        );
        m
    }

    fn pill(s: &P2Screen) -> (String, Color) {
        s.pill.clone().expect("pill")
    }

    #[test]
    fn cpu_and_mem() {
        let m = base();
        let s = cpu(&m, 1.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("CPU", "42", Some("%")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.424).abs() < 1e-4 && color == BLUE));
        assert_eq!(pill(&s), ("7 nodes · avg".into(), TEXT2));
        assert!(matches!(&s.card, Some(Card::Spark { series, max, caption, .. }) if series == &vec![42.4] && *max == 100.0 && caption == "last 60 s"));
        let mut hot = base();
        hot.apply(
            Event::Metrics {
                cpu_pct: 93.0,
                mem_pct: 67.0,
                mem_used_gb: 10.7,
                mem_total_gb: 16.0,
                hot_cpu: None,
                hot_mem: None,
            },
            1.0,
        );
        let s = cpu(&hot, 1.0);
        assert_eq!(pill(&s), ("High load".into(), WARN));
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        let s = mem(&m, 1.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("MEMORY", "67"));
        assert_eq!(pill(&s), ("10.7 of 16 GB".into(), TEXT2));
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == VIOLET));
        assert!(matches!(&s.card, Some(Card::Spark { min, max, caption, .. }) if *min == 50.0 && *max == 80.0 && caption == "last 90 s"));
    }

    #[test]
    fn storage_parts_follow_volume_sizes_and_flag_degraded_volumes() {
        let mut m = base();
        let tb = 1u64 << 40;
        m.apply(
            Event::Storage {
                volumes: vec![
                    ("graf".into(), Robustness::Healthy),
                    ("prom".into(), Robustness::Healthy),
                    ("qbit".into(), Robustness::Healthy),
                ],
                used_bytes: tb + tb / 5,
                capacity_bytes: 3 * tb + tb / 10,
            },
            0.0,
        );
        m.apply(
            Event::VolumeSizes(vec![("graf".into(), tb / 4), ("prom".into(), tb / 2), ("qbit".into(), 2 * tb / 5)]),
            0.0,
        );
        let s = storage(&m, 1.0);
        assert_eq!((s.hero.as_str(), s.unit.as_deref()), ("39", Some("%")));
        assert_eq!(pill(&s), ("1.2 of 3.1 TB".into(), TEXT2));
        let parts = match &s.ring.kind {
            RingKind::Parts(p) => p.clone(),
            _ => panic!("parts"),
        };
        assert_eq!(parts.len(), 3);
        assert!((parts[0].0 - 0.5 / 3.1).abs() < 1e-3, "largest first: prom");
        assert_eq!(parts[0].1, TEAL);
        assert_ne!(parts[1].1, parts[0].1, "each volume its own shade");
        assert!(matches!(&s.card, Some(Card::StackBar { caption, .. }) if caption == "1.9 TB free"));
        m.apply(
            Event::Storage {
                volumes: vec![
                    ("graf".into(), Robustness::Healthy),
                    ("prom".into(), Robustness::Healthy),
                    ("qbit".into(), Robustness::Degraded),
                ],
                used_bytes: tb + tb / 5,
                capacity_bytes: 3 * tb + tb / 10,
            },
            1.0,
        );
        let s = storage(&m, 2.0);
        assert_eq!(pill(&s), ("qbit degraded".into(), CRIT));
        let parts = match &s.ring.kind {
            RingKind::Parts(p) => p.clone(),
            _ => panic!("parts"),
        };
        assert_eq!(parts[1].1, CRIT, "qbit is the second largest");
        assert!(matches!(&s.card, Some(Card::StackBar { caption, .. }) if caption == "qbit degraded"));
        let mut bare = base();
        bare.apply(
            Event::Storage {
                volumes: vec![("a".into(), Robustness::Healthy)],
                used_bytes: 64 << 30,
                capacity_bytes: 128 << 30,
            },
            0.0,
        );
        assert!(matches!(&storage(&bare, 0.0).ring.kind, RingKind::Parts(p) if p.len() == 1 && (p[0].0 - 0.5).abs() < 1e-4), "no sizes: one used part");
    }

    #[test]
    fn pods_health_and_torrent_mode() {
        let mut m = base();
        m.apply(
            Event::PodSnapshot {
                running: 38,
                pending: 2,
                failed: 1,
                total: 41,
            },
            0.0,
        );
        let s = pods(&m, 0.0);
        assert_eq!(s.hero, "38");
        assert!(matches!(&s.ring.kind, RingKind::Parts(p) if p.len() == 3 && p[0].1 == OK && p[1].1 == WARN && p[2].1 == CRIT && (p[0].0 - 38.0 / 41.0).abs() < 1e-5));
        assert_eq!(pill(&s), ("running".into(), TEXT2));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("2".to_string(), "pending".to_string(), WARN) && items[1].2 == CRIT));
        let h = health(&m, 0.0);
        assert_eq!((h.title.as_str(), h.hero.as_str(), h.live), ("CLUSTER", "7/7", true));
        assert!(matches!(&h.ring.kind, RingKind::Discrete(c) if c.len() == 7 && c.iter().all(|c| *c == OK)));
        assert_eq!(pill(&h), ("All nodes ready".into(), OK));
        assert!(matches!(&h.card, Some(Card::Stats(items)) if items[0].1 == "alerts" && items[0].2 == TEXT3 && items[1] == ("0".to_string(), "not ready".to_string(), TEXT3)));
        m.apply(
            Event::NodeSnapshot {
                ready: 6,
                total: 7,
                not_ready: vec!["pi3".into()],
            },
            1.0,
        );
        m.apply(
            Event::AlertSnapshot {
                firing: vec!["Down".into()],
            },
            1.0,
        );
        let h = health(&m, 1.0);
        assert_eq!(h.hero, "6/7");
        assert_eq!(pill(&h), ("pi3 not ready".into(), CRIT));
        assert!(matches!(&h.ring.kind, RingKind::Discrete(c) if c[5] == OK && c[6] == CRIT));
        assert!(matches!(&h.card, Some(Card::Stats(items)) if items[0] == ("1".to_string(), "alerts".to_string(), CRIT) && items[1].0 == "1"));
        let mut big = base();
        big.apply(
            Event::NodeSnapshot {
                ready: 12,
                total: 12,
                not_ready: vec![],
            },
            0.0,
        );
        assert_eq!(health(&big, 0.0).hero_scale, 0.82, "five characters shrink");
        let mut t = base();
        t.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        t.apply(
            Event::Torrents(vec![
                Torrent {
                    name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
                    progress: 78.0,
                    eta_secs: 900,
                    speed_bps: 9_000_000,
                },
                Torrent {
                    name: "b".into(),
                    progress: 41.0,
                    eta_secs: 3000,
                    speed_bps: 3_000_000,
                },
            ]),
            0.0,
        );
        t.tick(1.0);
        let s = health(&t, 1.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("DOWNLOADS", "78"));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.78).abs() < 1e-5 && color == CYAN));
        assert_eq!(pill(&s).0, "ubuntu-24.04.2-live-server-amd64.iso");
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("11.4M".to_string(), "speed".to_string(), TEXT) && items[1] == ("15m".to_string(), "ETA".to_string(), TEXT)));
    }

    #[test]
    fn deploys_count_today_and_name_the_last_sync() {
        let mut m = base();
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        m.apply(
            Event::Link {
                target: LinkTarget::ArgoCd,
                up: true,
            },
            0.0,
        );
        let app = |name: &str, sync, health, operating| App {
            name: name.into(),
            sync,
            health,
            operating,
        };
        m.apply(
            Event::Apps(vec![
                app("a", AppSync::Synced, AppHealth::Healthy, false),
                app("b", AppSync::OutOfSync, AppHealth::Progressing, true),
                app("c", AppSync::Synced, AppHealth::Degraded, false),
                app("d", AppSync::Unknown, AppHealth::Unknown, false),
            ]),
            0.0,
        );
        let s = deploys(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("DEPLOYS TODAY", "0"));
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, BLUE, CRIT, TEXT3]));
        assert_eq!(pill(&s), ("no syncs yet".into(), TEXT2));
        assert!(matches!(&s.card, Some(Card::Bars { values, now, past_alpha, caption, .. }) if values.len() == 24 && *now == 14 && *past_alpha == 0.28 && caption == "per hour"));
        m.apply(Event::AppSynced { name: "grafana".into() }, 10.0);
        let s = deploys(&m, 12.0);
        assert_eq!(s.hero, "1");
        assert_eq!(pill(&s), ("grafana · just now".into(), INDIGO));
        let s = deploys(&m, 140.0);
        assert_eq!(pill(&s), ("grafana · 2 m ago".into(), TEXT2));
        assert!(matches!(&s.card, Some(Card::Bars { values, .. }) if values[14] == 1.0));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core format p2::roles`
Expected: compile errors.

- [ ] **Step 3: Implement the formatters**

Append to `crates/core/src/format.rs` (before the tests):

```rust
/// Seconds since something happened: `just now`, `42 s ago`, `2 m ago`, `2 h ago`.
pub fn fmt_ago(secs: f64) -> String {
    let s = secs.max(0.0);
    if s < 4.0 {
        "just now".into()
    } else if s < 60.0 {
        format!("{} s ago", s as i64)
    } else if s < 3600.0 {
        format!("{} m ago", (s / 60.0) as i64)
    } else {
        format!("{} h ago", (s / 3600.0) as i64)
    }
}

/// Seconds of the local day (any offset) as `HH:MM`.
pub fn fmt_hhmm(local_secs: i64) -> String {
    let s = local_secs.rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

/// Time until something: `in 2h 14m`, `in 15 min`, `now`.
pub fn fmt_in(secs: i64) -> String {
    let mins = (secs + 30) / 60;
    if mins < 1 {
        "now".into()
    } else if mins < 60 {
        format!("in {mins} min")
    } else {
        format!("in {}h {:02}m", mins / 60, mins % 60)
    }
}

/// A length of time as `8h 56m`.
pub fn fmt_hm(secs: i64) -> String {
    let mins = secs.max(0) / 60;
    format!("{}h {:02}m", mins / 60, mins % 60)
}

/// A countdown as `mm:ss` under 100 minutes, `1h40` above.
pub fn fmt_mmss(secs: i64) -> String {
    let s = secs.max(0);
    if s < 100 * 60 {
        format!("{:02}:{:02}", s / 60, s % 60)
    } else {
        format!("{}h{:02}", s / 3600, (s % 3600) / 60)
    }
}

/// `("1.2 of 3.1 TB", "1.9 TB free")`, in GB when the capacity is under a TiB.
pub fn fmt_capacity(used: u64, cap: u64) -> (String, String) {
    const TB: f64 = 1_099_511_627_776.0;
    const GB: f64 = 1_073_741_824.0;
    let free = cap.saturating_sub(used);
    if cap as f64 >= TB {
        (
            format!("{:.1} of {:.1} TB", used as f64 / TB, cap as f64 / TB),
            format!("{:.1} TB free", free as f64 / TB),
        )
    } else {
        (
            format!("{:.0} of {:.0} GB", used as f64 / GB, cap as f64 / GB),
            format!("{:.0} GB free", free as f64 / GB),
        )
    }
}
```

- [ ] **Step 4: Implement the dispatcher and the cluster builders**

`crates/core/src/p2/roles/mod.rs` above its tests:

```rust
pub mod cluster;

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::screen::P2Screen;
use crate::theme::Role;

/// The P2 screen for a role with live data. `None` for roles that still draw
/// their legacy scene, and for power-mix and face, which draw themselves.
pub fn screen(model: &Model, role: Role, now: Secs) -> Option<P2Screen> {
    Some(match role {
        Role::Cpu => cluster::cpu(model, now),
        Role::Mem => cluster::mem(model, now),
        Role::Storage => cluster::storage(model, now),
        Role::Pods => cluster::pods(model, now),
        Role::Health => cluster::health(model, now),
        Role::Deploys => cluster::deploys(model, now),
        _ => return None,
    })
}

/// A two-character node label for the node bars: the last `-` token, prefixed
/// with the name's first letter when the token is a bare number.
pub fn short_node(name: &str) -> String {
    let last = name.rsplit('-').next().unwrap_or(name);
    if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
        let first = name.chars().next().unwrap_or('n');
        format!("{first}{last}")
    } else {
        last.chars().take(2).collect()
    }
}
```

Add `pub mod roles;` to `crates/core/src/p2/mod.rs`.

`crates/core/src/p2/roles/cluster.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::event::{App, AppHealth, AppSync, Robustness};
use crate::format::{fmt_ago, fmt_capacity, fmt_eta, fmt_speed};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CRIT, CYAN, INDIGO, OK, TEAL, TEXT, TEXT2, TEXT3, VIOLET, WARN};
use crate::theme::Color;

pub fn cpu(model: &Model, now: Secs) -> P2Screen {
    let st = model.state();
    let hot = st.cpu_pct >= model.thresholds().hot_cpu;
    let color = if hot { WARN } else { BLUE };
    let hist = model.cpu_history();
    let mut s = P2Screen::new("CPU", format!("{:.0}", st.cpu_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(st.cpu_pct / 100.0, color);
    s.pill = Some(if hot {
        ("High load".into(), WARN)
    } else if st.nodes_total > 0 {
        (format!("{} nodes · avg", st.nodes_total), TEXT2)
    } else {
        ("cluster avg".into(), TEXT2)
    });
    s.card = Some(Card::Spark {
        series: hist.series(),
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max: 100.0,
        now_idx: None,
        color,
        caption: "last 60 s".into(),
    });
    s
}

pub fn mem(model: &Model, now: Secs) -> P2Screen {
    let st = model.state();
    let hist = model.mem_history();
    let mut s = P2Screen::new("MEMORY", format!("{:.0}", st.mem_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(st.mem_pct / 100.0, VIOLET);
    s.pill = Some((
        format!("{:.1} of {:.0} GB", st.mem_used_gb, st.mem_total_gb),
        TEXT2,
    ));
    s.card = Some(Card::Spark {
        series: hist.series(),
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 50.0,
        max: 80.0,
        now_idx: None,
        color: VIOLET,
        caption: "last 90 s".into(),
    });
    s
}

/// Teal, darker for each further volume.
pub fn teal_shade(i: usize, n: usize) -> Color {
    let t = if n <= 1 {
        0.0
    } else {
        i as f32 / (n as f32 - 1.0) * 0.9
    };
    TEAL.mix(Color::hex(0x2F6866), t)
}

pub fn storage(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let cap = st.storage_capacity.max(1) as f32;
    let used_pct = if st.storage_capacity > 0 {
        st.storage_used as f32 / cap * 100.0
    } else {
        0.0
    };
    let degraded: Vec<&str> = st
        .volumes
        .iter()
        .filter(|(_, r)| matches!(r, Robustness::Degraded | Robustness::Faulted))
        .map(|(n, _)| n.as_str())
        .collect();
    let mut sized: Vec<(&str, u64)> = st
        .volume_bytes
        .iter()
        .map(|(n, b)| (n.as_str(), *b))
        .collect();
    sized.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let n = sized.len();
    let parts: Vec<(f32, Color)> = if sized.is_empty() {
        vec![(used_pct / 100.0, TEAL)]
    } else {
        sized
            .iter()
            .enumerate()
            .map(|(i, (name, bytes))| {
                let c = if degraded.contains(name) {
                    CRIT
                } else {
                    teal_shade(i, n)
                };
                (*bytes as f32 / cap, c)
            })
            .collect()
    };
    let (of, free) = fmt_capacity(st.storage_used, st.storage_capacity);
    let mut s = P2Screen::new("STORAGE", format!("{used_pct:.0}"));
    s.unit = Some("%".into());
    s.ring = RingSpec::parts(parts.clone());
    s.pill = Some(match degraded.first() {
        Some(v) => (format!("{v} degraded"), CRIT),
        None => (of, TEXT2),
    });
    s.card = Some(Card::StackBar {
        parts,
        caption: match degraded.first() {
            Some(v) => format!("{v} degraded"),
            None => free,
        },
    });
    s
}

pub fn pods(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let total = st.pods_total.max(1) as f32;
    let mut s = P2Screen::new("PODS", st.pods_running.to_string());
    s.ring = RingSpec::parts(vec![
        (st.pods_running as f32 / total, OK),
        (st.pods_pending as f32 / total, WARN),
        (st.pods_failed as f32 / total, CRIT),
    ]);
    s.pill = Some(("running".into(), TEXT2));
    s.card = Some(Card::Stats(vec![
        (
            st.pods_pending.to_string(),
            "pending".into(),
            if st.pods_pending > 0 { WARN } else { TEXT3 },
        ),
        (
            st.pods_failed.to_string(),
            "failed".into(),
            if st.pods_failed > 0 { CRIT } else { TEXT3 },
        ),
    ]));
    s
}

pub fn health(model: &Model, now: Secs) -> P2Screen {
    if model.torrent_mode() {
        return torrent(model, now);
    }
    let st = model.state();
    let (ready, total) = (st.nodes_ready as usize, st.nodes_total as usize);
    let not_ready = total.saturating_sub(ready);
    let mut s = P2Screen::new("CLUSTER", format!("{ready}/{total}"));
    if format!("{ready}/{total}").len() >= 5 {
        s.hero_scale = 0.82;
    }
    s.ring = RingSpec::discrete((0..total).map(|i| if i < ready { OK } else { CRIT }).collect());
    s.pill = Some(match st.nodes_not_ready.first() {
        Some(n) => (format!("{n} not ready"), CRIT),
        None if not_ready > 0 => ("node not ready".into(), CRIT),
        None => ("All nodes ready".into(), OK),
    });
    let alerts = st.alerts.len();
    s.card = Some(Card::Stats(vec![
        (alerts.to_string(), "alerts".into(), if alerts > 0 { CRIT } else { TEXT3 }),
        (not_ready.to_string(), "not ready".into(), if not_ready > 0 { CRIT } else { TEXT3 }),
    ]));
    s.live = true;
    s
}

/// HEALTH while qBittorrent is busy: the most complete download.
pub fn torrent(model: &Model, _now: Secs) -> P2Screen {
    let list = model.torrent_badge_list();
    let top = list.iter().max_by(|a, b| a.progress.total_cmp(&b.progress));
    let (name, progress) = top
        .map(|t| (t.name.clone(), t.progress))
        .unwrap_or_default();
    let mut s = P2Screen::new("DOWNLOADS", format!("{progress:.0}"));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(progress / 100.0, CYAN);
    s.pill = Some((name, CYAN));
    let speed: i64 = list.iter().map(|t| t.speed_bps).sum();
    let eta = list
        .iter()
        .map(|t| t.eta_secs)
        .filter(|e| (0..864_000).contains(e))
        .min()
        .unwrap_or(-1);
    s.card = Some(Card::Stats(vec![
        (fmt_speed(speed), "speed".into(), TEXT),
        (fmt_eta(eta), "ETA".into(), TEXT),
    ]));
    s
}

/// Segment colour for an Argo application.
pub fn app_color(a: &App) -> Color {
    if matches!(a.health, AppHealth::Degraded | AppHealth::Missing) {
        CRIT
    } else if matches!(a.health, AppHealth::Suspended | AppHealth::Unknown)
        || a.sync == AppSync::Unknown
    {
        TEXT3
    } else if a.health == AppHealth::Progressing || a.sync == AppSync::OutOfSync || a.operating {
        BLUE
    } else {
        OK
    }
}

pub fn deploys(model: &Model, now: Secs) -> P2Screen {
    let apps = &model.apps().apps;
    let log = model.deploys_log();
    let today: u32 = log.hours.iter().sum();
    let mut s = P2Screen::new("DEPLOYS TODAY", today.to_string());
    s.ring = RingSpec::discrete(apps.iter().map(app_color).collect());
    s.pill = Some(match &log.last {
        Some((name, at)) if now - at < 4.0 => (format!("{name} · just now"), INDIGO),
        Some((name, at)) => (format!("{name} · {}", fmt_ago(now - at)), TEXT2),
        None => ("no syncs yet".into(), TEXT2),
    });
    s.card = Some(Card::Bars {
        values: log.hours.iter().map(|h| *h as f32).collect(),
        colors: vec![INDIGO; 24],
        now: model.local_hour(),
        past_alpha: 0.28,
        caption: "per hour".into(),
    });
    s
}
```

- [ ] **Step 5: Dispatch converted roles in `role_scene`**

In `crates/core/src/scene.rs`:

```rust
use crate::p2::screen::compose;
// ...
pub fn role_scene(model: &Model, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
    ui.for_role(role);
    if let Some(screen) = crate::p2::roles::screen(model, role, now) {
        return compose(&screen, ui, now);
    }
    match role {
```

Leave the legacy arms in the match as they are (they are unreachable for converted roles and go in Task 15).

- [ ] **Step 6: Fix the tests the conversion breaks**

In `crates/core/src/scene.rs` tests, add a helper and replace the three scene-level tests:

```rust
    fn title_of(s: &Scene) -> Option<String> {
        s.items.iter().find_map(|d| match d {
            Drawable::CurvedText { text, .. } => Some(text.clone()),
            _ => None,
        })
    }

    #[test]
    fn cpu_scene_is_a_p2_tile() {
        let m = model_with_cpu(42.0);
        let s = role_scene(&m, Role::Cpu, 5.0, &mut ScreenUi::new());
        assert_eq!(title_of(&s).as_deref(), Some("CPU"));
        assert!(matches!(s.items[0], Drawable::Clear(_)));
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Ring { .. } | Drawable::Icon { .. } | Drawable::Badge { .. })));
    }

    #[test]
    fn health_scene_switches_to_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 2,
                total: 2,
                not_ready: vec![],
            },
            0.0,
        );
        assert_eq!(title_of(&role_scene(&m, Role::Health, 0.0, &mut ScreenUi::new())).as_deref(), Some("CLUSTER"));
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![Torrent {
                name: "a".into(),
                progress: 50.0,
                eta_secs: 60,
                speed_bps: 2_097_152,
            }]),
            0.0,
        );
        assert_eq!(title_of(&role_scene(&m, Role::Health, 1.0, &mut ScreenUi::new())).as_deref(), Some("DOWNLOADS"));
    }
```

Delete `health_not_ready_shows_red_tail` (its P2 equivalent is `pods_health_and_torrent_mode` in `cluster.rs`). Keep `ring_states_lights_rounded_share`, `pod_segments_*`, `connecting_has_comet_and_big_icon`, `no_data_is_dim_full_ring`, the badge tests (legacy helpers still exist).

In `crates/core/src/scene_fx.rs` tests:

- delete `splash_counter_in_badge` (the pods badge no longer exists; splashes go in Task 15);
- in `cycling_screen_transitions_with_iris` replace the two icon assertions with title checks:

```rust
        let s = m.scene(0, 3.15, &mut ui);
        assert!(s.zoom < 1.0 && s.zoom > 0.0);
        assert_eq!(title_of(&s).as_deref(), Some("CPU"));
        let s = m.scene(0, 3.4, &mut ui);
        assert_eq!(title_of(&s).as_deref(), Some("MEMORY"), "second half shows the incoming role");
```

with `let mut ui = ScreenUi::new();` declared at the top of that test and the same `title_of` helper added to this test module;
- in `thermal_and_storage_gate_on_the_prometheus_link` replace `assert!(has_icon(&m.scene_for_role(Role::Storage, 1.0, ..), "database"));` with `assert_eq!(title_of(&m.scene_for_role(Role::Storage, 1.0, &mut ScreenUi::new())).as_deref(), Some("STORAGE"));` (the `cloud-off` assertion after the link drop stays).

- [ ] **Step 7: Goldens**

In `crates/render/tests/golden.rs` delete the three pixel assertions in `cpu_idle` (keep `assert_eq!(pixel(&px, 4, 4), (0, 0, 0));` and `check`), delete the violet assertion in `storage_idle_and_degraded`, then:

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden
cargo test --workspace
git status --short crates/render/tests/goldens
```

Expected: the second run is green; changed PNGs: `cpu_idle`, `idle_cpu`, `idle_memory-stick`, `idle_box`, `idle_heart-pulse`, `pods_splash`, `torrent`, `torrent_unwind`, `storage`, `storage_degraded`, `deploys`, `iris_mid`. Open `idle_cpu.png`, `idle_heart-pulse.png`, `storage_degraded.png`, `deploys.png` and compare with `p2-v4.html`: ring opening at the top with the curved title, hero with a superscript unit, pill, card. `pods_splash.png` shows the legacy event icon over the P2 tile; that is expected until Task 15.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src crates/render/tests
git commit -m "feat(core): P2 screens for cpu, mem, storage, pods, health, downloads and deploys

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Screens: weather, iss, price, carbon, github, thermal

**Files:**
- Create: `crates/core/src/p2/roles/sky.rs`, `crates/core/src/p2/roles/energy.rs`, `crates/core/src/p2/roles/github.rs`, `crates/core/src/p2/roles/system.rs`
- Modify: `crates/core/src/p2/roles/mod.rs` (dispatch), `crates/core/src/electricity.rs` (`PriceLevel::{name, p2_color}`), `crates/core/src/scene_fx.rs` (tests), `crates/render/tests/golden.rs` (pixel asserts) and re-capture goldens
- Test: unit tests in the four new files and `electricity.rs`

**Interfaces:**
- Consumes: `Model::{weather, iss, prices, current_price, local_slot, electricity, carbon_hourly, github, local_secs, state, thresholds, unix_now, utc_offset_secs}`, `format::{fmt_hhmm, fmt_mmss}`, `price_level`, `short_node`.
- Produces: `sky::{condition, weather, iss}`, `energy::{band_colors, price, carbon, CARBON_BANDS}`, `github::{weekday_index, streak, github}`, `system::{temp_color, thermal}`; `PriceLevel::name() -> &'static str`, `PriceLevel::p2_color() -> Color`. The dispatcher gains `Weather`, `Iss`, `Price`, `Carbon`, `GhActivity`, `Thermal`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/src/electricity.rs` tests:

```rust
    #[test]
    fn p2_level_names_and_colours() {
        assert_eq!(PriceLevel::VeryCheap.name(), "Very cheap");
        assert_eq!(PriceLevel::VeryPricey.name(), "Very pricey");
        assert_eq!(PriceLevel::Normal.p2_color(), Color::hex(0xD9CB6C));
        let colours: std::collections::HashSet<u32> = PriceLevel::ALL
            .iter()
            .map(|l| {
                let c = l.p2_color();
                ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32
            })
            .collect();
        assert_eq!(colours.len(), 5);
    }
```

Create `crates/core/src/p2/roles/sky.rs` with tests only:

```rust
//! Sky roles with a forecast: weather and the ISS pass.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, IssPass};
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{AMBER, CYAN, TEXT2, TEXT3};

    fn weather_model(code: u16, temp: f32, precip: Vec<f32>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400); // 12:00 UTC
        m.set_utc_offset_secs(7200); // 14:00 local
        m.apply(
            Event::Weather {
                temp_c: temp,
                code,
                is_day: true,
                wind_kmh: 8.0,
                gust_kmh: 15.0,
                wind_from_deg: 225.0,
                at: String::new(),
            },
            0.0,
        );
        let n = precip.len();
        m.apply(
            Event::Forecast {
                hourly_temp_c: (0..n).map(|i| temp + i as f32 * 0.5).collect(),
                hourly_precip_mm: precip,
                day_min_c: 11.0,
                day_max_c: 21.0,
            },
            0.0,
        );
        m
    }

    #[test]
    fn weather_titles_the_condition_scales_the_day_and_hints_rain() {
        let m = weather_model(2, 17.0, vec![0.0, 0.0, 0.0, 0.0, 1.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let s = weather(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("PARTLY CLOUDY", "17", Some("°")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.6).abs() < 1e-5 && color == AMBER));
        assert_eq!(s.ends, Some(("11°".into(), "21°".into())));
        assert_eq!(s.pill, Some(("Rain from 18:00".into(), CYAN)));
        assert!(matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series.len() == 12 && *now_idx == Some(0) && *min == 9.0 && *max == 23.0 && caption == "next 12 h"));
        let dry = weather(&weather_model(0, 24.0, vec![0.0; 12]), 0.0);
        assert_eq!(dry.title, "CLEAR");
        assert_eq!(dry.pill, Some(("Dry next 12 h".into(), TEXT2)));
        let wet = weather(&weather_model(61, 9.0, vec![2.0; 12]), 0.0);
        assert_eq!(wet.pill, Some(("Raining now".into(), CYAN)));
        assert_eq!(condition(95), "THUNDER");
        assert_eq!(condition(48), "FOG");
        assert_eq!(condition(123), "WEATHER");
        let mut bare = Model::new(Thresholds::default());
        bare.apply(
            Event::Weather {
                temp_c: 5.0,
                code: 3,
                is_day: false,
                wind_kmh: 0.0,
                gust_kmh: 0.0,
                wind_from_deg: 0.0,
                at: String::new(),
            },
            0.0,
        );
        let s = weather(&bare, 0.0);
        assert_eq!(s.ends, Some(("0°".into(), "10°".into())), "no daily range: ±5 around now");
        assert!(matches!(&s.card, Some(Card::Spark { series, .. }) if series.is_empty()));
    }

    fn iss_model(unix: i64, pass: Option<IssPass>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(7200);
        m.apply(Event::IssPass(pass), 0.0);
        m
    }

    #[test]
    fn iss_counts_down_shows_the_pass_and_rests() {
        let pass = IssPass {
            start: 10_000,
            end: 10_372,
            max_elevation_deg: 62.0,
            visible: true,
        };
        let s = iss(&iss_model(10_000 - 14 * 60 - 32, Some(pass)), 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale), ("ISS PASS IN", "14:32", 0.82));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - (1.0 - 872.0 / 5400.0)).abs() < 1e-4 && color == CYAN));
        assert_eq!(s.pill, Some(("Visible · max 62°".into(), CYAN)));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("04:46".to_string(), "start".to_string(), TEXT) && items[1] == ("6 min".to_string(), "duration".to_string(), TEXT)));
        let during = iss(&iss_model(10_100, Some(pass)), 0.0);
        assert_eq!((during.title.as_str(), during.hero.as_str(), during.unit.as_deref()), ("ISS OVERHEAD", "62", Some("°")));
        assert!(matches!(during.ring.kind, RingKind::Fill { frac, .. } if frac == 1.0));
        assert_eq!(during.pill, Some(("Visible now".into(), CYAN)));
        let dim = iss(&iss_model(5_000, Some(IssPass { visible: false, ..pass })), 0.0);
        assert_eq!(dim.pill, Some(("Not visible · max 62°".into(), TEXT2)));
        let none = iss(&iss_model(0, None), 0.0);
        assert_eq!((none.hero.as_str(), none.hero_color), ("—", TEXT3));
        assert!(matches!(none.ring.kind, RingKind::Empty));
        assert_eq!(none.pill, Some(("No pass in 24 h".into(), TEXT2)));
        assert!(none.card.is_none());
    }
}
```

Create `crates/core/src/p2/roles/energy.rs` with tests only:

```rust
//! Electricity roles with a hero: price and carbon (power-mix draws itself,
//! renewable lives in `system.rs` with the other fill rings).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electricity::{PriceLevel, Source};
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{TEXT2, TEXT3};

    fn price_model(at_now: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let mut eur: Vec<f32> = (0..96).map(|i| (6.2 + (i / 4) as f32) / 100.0).collect();
        eur[56] = at_now;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.18,
                currency: "EUR".into(),
            },
            0.0,
        );
        m
    }

    #[test]
    fn price_hero_bands_and_hourly_bars() {
        let m = price_model(0.221);
        let s = price(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale, s.unit.as_deref(), s.unit_below), ("ELECTRICITY", "0.221", 0.78, Some("€/kWh"), true));
        let (lvl, _) = crate::electricity::price_level(0.221, 0.18);
        assert_eq!(s.pill, Some((lvl.name().into(), lvl.p2_color())));
        match &s.ring.kind {
            RingKind::Bands { colors, frac } => {
                assert_eq!(colors, &band_colors());
                // today runs 0.062 .. 0.292 (plus the 0.221 at 14:00)
                assert!((frac - (0.221 - 0.062) / (0.292 - 0.062)).abs() < 1e-3, "{frac}");
            }
            _ => panic!("bands"),
        }
        match &s.card {
            Some(Card::Bars { values, colors, now, caption, .. }) => {
                assert_eq!((values.len(), colors.len(), *now, caption.as_str()), (24, 24, 14, "today"));
                assert!((values[0] - 0.062).abs() < 1e-4);
                assert_eq!(colors[0], PriceLevel::VeryCheap.p2_color());
            }
            _ => panic!("bars"),
        }
        let neg = price(&price_model(-0.006), 0.0);
        assert_eq!(neg.hero, "-0.006");
        assert!(matches!(&neg.card, Some(Card::Bars { values, .. }) if values.iter().all(|v| *v >= 0.0)), "bars shift up for negative prices");
        let mut hole = Model::new(Thresholds::default());
        hole.set_local_slot(56);
        hole.apply(
            Event::Prices {
                date: "x".into(),
                eur_per_kwh: vec![f32::NAN; 96],
                avg_eur_per_kwh: 0.2,
                currency: "EUR".into(),
            },
            0.0,
        );
        let s = price(&hole, 0.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert_eq!(s.pill, Some(("no price".into(), TEXT2)));
    }

    fn carbon_model(samples: &[(i64, f32)]) -> Model {
        let mut m = Model::new(Thresholds::default());
        for (hour, g) in samples {
            m.set_unix_now(hour * 3600);
            m.apply(
                Event::Electricity {
                    zone: "NL".into(),
                    mix_mw: vec![(Source::Gas, 100.0)],
                    renewable_pct: 10.0,
                    fossil_free_pct: 10.0,
                    carbon_gco2: *g,
                    updated_at: String::new(),
                },
                0.0,
            );
        }
        m
    }

    #[test]
    fn carbon_compares_with_the_day_and_sparks_24_hours() {
        let m = carbon_model(&[(1, 300.0), (2, 280.0), (3, 200.0)]);
        let s = carbon(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below), ("CARBON", "200", Some("gCO₂/kWh"), true));
        assert!(matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors.len() == 5 && (frac - 0.4).abs() < 1e-5));
        assert_eq!(s.pill.as_ref().unwrap().0, "Cleaner than usual");
        assert!(matches!(&s.card, Some(Card::Spark { series, now_idx, caption, .. }) if series == &vec![300.0, 280.0, 200.0] && *now_idx == Some(2) && caption == "24 h"));
        assert_eq!(carbon(&carbon_model(&[(1, 200.0), (2, 300.0)]), 0.0).pill.unwrap().0, "Dirtier than usual");
        assert_eq!(carbon(&carbon_model(&[(1, 250.0), (2, 255.0)]), 0.0).pill.unwrap().0, "About usual");
        assert_eq!(carbon(&carbon_model(&[(1, 250.0)]), 0.0).pill, Some(("NL grid".into(), TEXT2)));
    }
}
```

Create `crates/core/src/p2/roles/github.rs` with tests only:

```rust
//! The GitHub contributions role.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{GREEN, TEXT2};

    #[test]
    fn weekday_and_streak() {
        assert_eq!(weekday_index(0), 3, "1970-01-01 was a Thursday");
        assert_eq!(weekday_index(4 * 86_400), 0, "Monday");
        assert_eq!(weekday_index(1_788_782_400 + 7200), 0, "2026-09-07 is a Monday");
        let days = |v: &[u32]| v.iter().enumerate().map(|(i, c)| (format!("d{i}"), *c)).collect::<Vec<_>>();
        assert_eq!(streak(&days(&[0, 3, 1, 2])), 3);
        assert_eq!(streak(&days(&[1, 0])), 0);
    }

    #[test]
    fn github_fills_by_best_day_and_lays_the_week_out() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400 + 2 * 86_400); // Wednesday 2026-09-09 12:00Z
        m.set_utc_offset_secs(7200);
        let days = (0..30).map(|i| (format!("d{i}"), [5u32, 12, 30, 0, 8, 25, 43, 28][i % 8])).collect();
        m.apply(Event::GithubActivity { days }, 0.0);
        let s = github(&m, 0.0);
        // today is index 29 -> 29 % 8 = 5 -> 25; best is 43
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("CONTRIBUTIONS", "25"));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 25.0 / 43.0).abs() < 1e-5 && color == GREEN));
        assert_eq!(s.pill, Some(("best day 43 · 2 streak".into(), TEXT2)));
        match &s.card {
            Some(Card::Week { values, today, color }) => {
                assert_eq!((*today, *color), (2, GREEN));
                assert_eq!(values[2], 25.0, "today");
                assert_eq!(values[1], 8.0, "yesterday: index 28 -> 4 -> 8");
                assert_eq!(values[0], 0.0, "Monday: index 27 -> 3 -> 0");
                assert_eq!(&values[3..], &[0.0; 4], "the rest of the week is empty");
            }
            _ => panic!("week"),
        }
    }
}
```

Create `crates/core/src/p2/roles/system.rs` with tests only (the other builders join in Task 12):

```rust
//! Rack hardware and the plain fill-ring roles: thermal, net, ups, wind,
//! rain, aqi, sun, moon, renewable.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{CRIT, OK, TEXT2, WARN};

    #[test]
    fn thermal_names_the_hottest_node_and_bars_the_rest() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeTemps(vec![
                ("hp-elitedesk-800-g5-i7".into(), 46.0),
                ("raspberrypi-5-8gb-1".into(), 54.0),
                ("raspberrypi-5-8gb-2".into(), 68.0),
            ]),
            0.0,
        );
        let s = thermal(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("THERMAL", "68", Some("°")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 38.0 / 55.0).abs() < 1e-5 && color == WARN));
        assert_eq!(s.pill, Some(("raspberrypi-5-8gb-2 hottest".into(), TEXT2)));
        match &s.card {
            Some(Card::NodeBars { values, min, max, labels, colors, highlight }) => {
                assert_eq!(values, &vec![46.0, 54.0, 68.0]);
                assert_eq!((*min, *max, *highlight), (30.0, 85.0, Some(2)));
                assert_eq!(labels, &vec!["i7".to_string(), "r1".into(), "r2".into()]);
                assert_eq!(colors, &vec![OK, OK, WARN]);
            }
            _ => panic!("node bars"),
        }
        assert_eq!(temp_color(76.0, 70.0), CRIT);
        assert_eq!(temp_color(64.9, 70.0), OK);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::roles electricity`
Expected: compile errors.

- [ ] **Step 3: Implement**

`crates/core/src/electricity.rs`, in `impl PriceLevel`:

```rust
    /// The P2 pill text.
    pub fn name(self) -> &'static str {
        match self {
            PriceLevel::VeryCheap => "Very cheap",
            PriceLevel::Cheap => "Cheap",
            PriceLevel::Normal => "Normal",
            PriceLevel::Pricey => "Pricey",
            PriceLevel::VeryPricey => "Very pricey",
        }
    }

    /// The P2 band colour, green through yellow to red.
    pub fn p2_color(self) -> Color {
        Color::hex(match self {
            PriceLevel::VeryCheap => 0x45D6A4,
            PriceLevel::Cheap => 0x7CCF8E,
            PriceLevel::Normal => 0xD9CB6C,
            PriceLevel::Pricey => 0xF2A65A,
            PriceLevel::VeryPricey => 0xF0605D,
        })
    }
```

`crates/core/src/p2/roles/sky.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::format::{fmt_hhmm, fmt_mmss};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{AMBER, CYAN, TEXT, TEXT2, TEXT3};

/// The curved title for a WMO weather code.
pub fn condition(code: u16) -> &'static str {
    match code {
        0 => "CLEAR",
        1 => "MAINLY CLEAR",
        2 => "PARTLY CLOUDY",
        3 => "OVERCAST",
        45 | 48 => "FOG",
        51..=57 => "DRIZZLE",
        61..=67 => "RAIN",
        71..=77 => "SNOW",
        80..=82 => "SHOWERS",
        85 | 86 => "SNOW SHOWERS",
        95..=99 => "THUNDER",
        _ => "WEATHER",
    }
}

pub fn weather(model: &Model, _now: Secs) -> P2Screen {
    let w = model.weather();
    let (lo, hi) = match (w.day_min_c, w.day_max_c) {
        (Some(a), Some(b)) if b > a => (a, b),
        _ => (w.temp_c - 5.0, w.temp_c + 5.0),
    };
    let mut s = P2Screen::new(condition(w.code), format!("{:.0}", w.temp_c));
    s.unit = Some("°".into());
    s.ring = RingSpec::fill((w.temp_c - lo) / (hi - lo), AMBER);
    s.ends = Some((format!("{lo:.0}°"), format!("{hi:.0}°")));
    let hour = model.local_hour();
    s.pill = Some(match w.hourly_precip_mm.iter().position(|p| *p >= 0.1) {
        Some(0) => ("Raining now".into(), CYAN),
        Some(i) => (format!("Rain from {:02}:00", (hour + i) % 24), CYAN),
        None => ("Dry next 12 h".into(), TEXT2),
    });
    s.card = Some(Card::Spark {
        series: w.hourly_temp_c.iter().copied().filter(|t| t.is_finite()).collect(),
        prev: None,
        scroll: 1.0,
        min: lo - 2.0,
        max: hi + 2.0,
        now_idx: Some(0),
        color: AMBER,
        caption: "next 12 h".into(),
    });
    s
}

pub fn iss(model: &Model, _now: Secs) -> P2Screen {
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let visible = |v: bool, el: f32| {
        if v {
            (format!("Visible · max {el:.0}°"), CYAN)
        } else {
            (format!("Not visible · max {el:.0}°"), TEXT2)
        }
    };
    match model.iss().pass {
        Some(p) if unix < p.start => {
            let until = p.start - unix;
            let mut s = P2Screen::new("ISS PASS IN", fmt_mmss(until));
            s.hero_scale = 0.82;
            s.ring = RingSpec::fill(1.0 - (until as f32 / 5400.0).clamp(0.0, 1.0), CYAN);
            s.pill = Some(visible(p.visible, p.max_elevation_deg));
            s.card = Some(Card::Stats(vec![
                (fmt_hhmm(p.start + offset), "start".into(), TEXT),
                (format!("{} min", (p.end - p.start + 30) / 60), "duration".into(), TEXT),
            ]));
            s
        }
        Some(p) if unix < p.end => {
            let mut s = P2Screen::new("ISS OVERHEAD", format!("{:.0}", p.max_elevation_deg));
            s.unit = Some("°".into());
            s.ring = RingSpec::fill(1.0, CYAN);
            s.pill = Some(if p.visible {
                ("Visible now".into(), CYAN)
            } else {
                ("Not visible".into(), TEXT2)
            });
            s.card = Some(Card::Stats(vec![
                (fmt_mmss(p.end - unix), "ends in".into(), TEXT),
                (format!("{} min", (p.end - p.start + 30) / 60), "duration".into(), TEXT),
            ]));
            s
        }
        _ => {
            let mut s = P2Screen::new("ISS PASS IN", "—");
            s.hero_color = TEXT3;
            s.pill = Some(("No pass in 24 h".into(), TEXT2));
            s
        }
    }
}
```

`crates/core/src/p2/roles/energy.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::electricity::{price_level, PriceLevel};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{TEXT2, TEXT3};
use crate::theme::Color;

/// The five price bands, cheapest first.
pub fn band_colors() -> Vec<Color> {
    PriceLevel::ALL.iter().map(|l| l.p2_color()).collect()
}

pub fn price(model: &Model, _now: Secs) -> P2Screen {
    let p = model.prices();
    let hourly: Vec<f32> = (0..24)
        .map(|h| {
            let slots: Vec<f32> = p
                .eur
                .iter()
                .skip(h * 4)
                .take(4)
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            if slots.is_empty() {
                f32::NAN
            } else {
                slots.iter().sum::<f32>() / slots.len() as f32
            }
        })
        .collect();
    let finite: Vec<f32> = p.eur.iter().copied().filter(|v| v.is_finite()).collect();
    let min = finite.iter().cloned().fold(f32::MAX, f32::min);
    let max = finite.iter().cloned().fold(f32::MIN, f32::max);
    let cur = model.current_price();
    let mut s = P2Screen::new(
        "ELECTRICITY",
        cur.map(|v| format!("{v:.3}")).unwrap_or_else(|| "—".into()),
    );
    s.hero_scale = 0.78;
    s.unit = Some("€/kWh".into());
    s.unit_below = true;
    if cur.is_none() {
        s.hero_color = TEXT3;
    }
    let frac = match cur {
        Some(v) if max > min => (v - min) / (max - min),
        _ => 0.5,
    };
    s.ring = RingSpec::bands(band_colors(), frac);
    s.pill = Some(match cur {
        Some(v) => {
            let (lvl, _) = price_level(v, p.avg);
            (lvl.name().into(), lvl.p2_color())
        }
        None => ("no price".into(), TEXT2),
    });
    let floor = if finite.is_empty() { 0.0 } else { min.min(0.0) };
    s.card = Some(Card::Bars {
        values: hourly
            .iter()
            .map(|v| if v.is_finite() { v - floor } else { 0.0 })
            .collect(),
        colors: hourly
            .iter()
            .map(|v| {
                if v.is_finite() {
                    price_level(*v, p.avg).0.p2_color()
                } else {
                    TEXT3
                }
            })
            .collect(),
        now: (model.local_slot() / 4) as usize,
        past_alpha: 0.28,
        caption: "today".into(),
    });
    s
}

/// Carbon intensity bands over 0..500 gCO₂/kWh, green to red.
pub const CARBON_BANDS: [u32; 5] = [0x45D6A4, 0x9BD07A, 0xE8C860, 0xF2A65A, 0xF0605D];

pub fn carbon(model: &Model, _now: Secs) -> P2Screen {
    let e = model.electricity();
    let g = e.carbon_gco2;
    let hist = model.carbon_hourly();
    let colors: Vec<Color> = CARBON_BANDS.iter().map(|c| Color::hex(*c)).collect();
    let band = ((g / 500.0).clamp(0.0, 0.999) * 5.0) as usize;
    let mut s = P2Screen::new("CARBON", format!("{g:.0}"));
    s.unit = Some("gCO₂/kWh".into());
    s.unit_below = true;
    s.ring = RingSpec::bands(colors.clone(), g / 500.0);
    s.pill = Some(match hist.mean() {
        Some(mean) if hist.len() >= 2 && g < mean * 0.95 => {
            ("Cleaner than usual".into(), Color::hex(0x7CCF8E))
        }
        Some(mean) if hist.len() >= 2 && g > mean * 1.05 => {
            ("Dirtier than usual".into(), Color::hex(0xF2A65A))
        }
        Some(_) if hist.len() >= 2 => ("About usual".into(), TEXT2),
        _ => (format!("{} grid", e.zone), TEXT2),
    });
    let series = hist.values();
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(f32::MIN, f32::max);
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: if lo.is_finite() { lo * 0.9 } else { 0.0 },
        max: if hi.is_finite() { (hi * 1.1).max(lo * 0.9 + 1.0) } else { 500.0 },
        color: colors[band],
        caption: "24 h".into(),
    });
    s
}
```

`crates/core/src/p2/roles/github.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{GREEN, TEXT2};

/// Monday = 0 .. Sunday = 6 for a local timestamp.
pub fn weekday_index(local_secs: i64) -> usize {
    (local_secs.div_euclid(86_400) + 3).rem_euclid(7) as usize
}

/// Consecutive days with a contribution, counted back from today.
pub fn streak(days: &[(String, u32)]) -> u32 {
    days.iter().rev().take_while(|d| d.1 > 0).count() as u32
}

pub fn github(model: &Model, _now: Secs) -> P2Screen {
    let g = model.github();
    let best = g.best();
    let today = g.today();
    let mut s = P2Screen::new("CONTRIBUTIONS", today.to_string());
    s.ring = RingSpec::fill(today as f32 / best as f32, GREEN);
    s.pill = Some((format!("best day {best} · {} streak", streak(&g.days)), TEXT2));
    let wd = weekday_index(model.local_secs());
    let mut values = [0.0f32; 7];
    for k in 0..=wd {
        if let Some(d) = g.days.len().checked_sub(1 + k).and_then(|i| g.days.get(i)) {
            values[wd - k] = d.1 as f32;
        }
    }
    s.card = Some(Card::Week {
        values,
        today: wd,
        color: GREEN,
    });
    s
}
```

`crates/core/src/p2/roles/system.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::roles::short_node;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{CRIT, OK, TEXT2, WARN};
use crate::theme::Color;

/// `OK` under `hot - 5`, `WARN` from there, `CRIT` from `hot + 5` (65 / 75 with
/// the default `hot_temp` of 70).
pub fn temp_color(c: f32, hot: f32) -> Color {
    if c >= hot + 5.0 {
        CRIT
    } else if c >= hot - 5.0 {
        WARN
    } else {
        OK
    }
}

pub fn thermal(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let hot = model.thresholds().hot_temp;
    let hottest = st
        .temps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
    let (idx, name, max) = match hottest {
        Some((i, (n, c))) => (Some(i), n.as_str(), *c),
        None => (None, "", 0.0),
    };
    let mut s = P2Screen::new("THERMAL", format!("{max:.0}"));
    s.unit = Some("°".into());
    s.ring = RingSpec::fill(((max - 30.0) / 55.0).clamp(0.0, 1.0), temp_color(max, hot));
    s.pill = Some((format!("{name} hottest"), TEXT2));
    s.card = Some(Card::NodeBars {
        values: st.temps.iter().map(|(_, c)| *c).collect(),
        min: 30.0,
        max: 85.0,
        labels: st.temps.iter().map(|(n, _)| short_node(n)).collect(),
        colors: st.temps.iter().map(|(_, c)| temp_color(*c, hot)).collect(),
        highlight: idx,
    });
    s
}
```

`crates/core/src/p2/roles/mod.rs`: add `pub mod energy; pub mod github; pub mod sky; pub mod system;` and the arms

```rust
        Role::Weather => sky::weather(model, now),
        Role::Iss => sky::iss(model, now),
        Role::Price => energy::price(model, now),
        Role::Carbon => energy::carbon(model, now),
        Role::GhActivity => github::github(model, now),
        Role::Thermal => system::thermal(model, now),
```

- [ ] **Step 4: Tests the conversion breaks, and goldens**

- `crates/core/src/scene_fx.rs`: `price_role_gates_on_the_prices_link` asserts a `Drawable::Text` reading `€/kWh`; change it to look for `Drawable::Run { text, .. } if text == "€/kWh"`. `thermal_and_storage_gate_on_the_prometheus_link`: replace the `thermometer` icon assertion with `assert_eq!(title_of(&m.scene_for_role(Role::Thermal, 1.0, &mut ScreenUi::new())).as_deref(), Some("THERMAL"));`. Delete `splash_counter_leaves_textless_badges_alone` (thermal badges are gone).
- `crates/core/src/scene_electricity.rs`: delete `price_gauge_has_five_bands_with_gaps_and_lights_the_needle_band`, `price_gauge_words_follow_the_bands`, `needle_over_a_gap_breathes_the_segment_before_it`, `negative_price_is_blue_and_very_cheap`, `price_gauge_without_data_hides_the_needle`, `carbon_scene_colours_ring_icon_and_badge` (their P2 equivalents are in `energy.rs`); the legacy functions stay until Task 15.
- `crates/core/src/scene_thermal.rs`, `scene_github.rs`, `scene_weather.rs` (weather only), `scene_sky.rs` (iss only): delete the scene-level tests that assert badges/icons for these roles (`hottest_node_drives_ring_and_text`, `github_scene_*`, `weather_ring_and_badge_follow_the_outdoor_scale`, `iss_ring_empties_toward_the_pass_and_fills_during_it`); keep the pure helper tests (`wmo_codes_map_to_icons`, `levels_are_quartiles_of_the_best_day`, `sun_*`, `moon_*`, `wind_*`).
- `crates/render/tests/golden.rs`: delete the pixel assertions in `thermal_idle` and `price_carbon_and_renewable` stays as it is; in `ups_on_battery_and_iss_during_pass` delete the violet-ring assertion.

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden
cargo test --workspace
```

Expected: green; changed PNGs `thermal`, `price`, `carbon`, `gh_activity`, `weather`, `iss`, `iss_pass`. Compare `price.png` with the price tile in `p2-v4.html` (bands with a marker, `0.221` with `€/kWh` under it, a coloured pill, hourly bars).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src crates/render/tests
git commit -m "feat(core): P2 screens for weather, iss, price, carbon, contributions and thermal

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: Screens: net, ups, wind, rain, aqi, sun, moon, renewable

**Files:**
- Modify: `crates/core/src/p2/roles/system.rs`, `crates/core/src/p2/roles/mod.rs`, `crates/core/src/event.rs` (`MoonPhase::name`), `crates/render/tests/golden.rs`; re-capture goldens
- Test: unit tests in `system.rs`

**Interfaces:**
- Consumes: `model::net_fill`, `scene_rain::{current_slots, first_wet_minutes, WET_MM}`, `sky::{next_sun_event, sun_progress, daylight_secs, moon_days, SunEvent}`, `theme::eaqi_band`, `chart::compass_name`, `format::{fmt_runtime, fmt_hhmm, fmt_in, fmt_hm}`.
- Produces: `system::{net, ups, wind, rain, aqi, sun, moon, renewable, EAQI_BANDS, eaqi_name}`, `MoonPhase::name()`. The dispatcher covers every role except `PowerMix` and `Face`.

- [ ] **Step 1: Write the failing tests**

Append to the tests in `crates/core/src/p2/roles/system.rs`:

```rust
    use crate::event::MoonPhase;
    use crate::theme::p2::{AMBER, BLUE, CYAN, GREEN, MOON, NIGHT, TEXT, TEXT3};

    #[test]
    fn net_and_ups() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Network {
                rx_bps: 68e6,
                tx_bps: 6e6,
            },
            0.0,
        );
        let s = net(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below), ("NETWORK", "74", Some("Mb/s"), true));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - crate::model::net_fill(74e6)).abs() < 1e-6 && color == BLUE));
        assert_eq!(s.pill, Some(("↓ 68  ↑ 6".into(), TEXT2)));
        assert!(matches!(&s.card, Some(Card::Spark { series, min, max, caption, .. }) if series == &vec![68.0] && *min == 0.0 && (*max - 74.8).abs() < 1e-3 && caption == "download · 60 s"));
        m.apply(
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 7.0,
                runtime_secs: 49 * 60,
            },
            0.0,
        );
        let s = ups(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("UPS", "100", Some("%")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if frac == 1.0 && color == OK));
        assert_eq!(s.pill, Some(("On mains".into(), OK)));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("49 min".to_string(), "runtime".to_string(), TEXT) && items[1] == ("7%".to_string(), "load".to_string(), TEXT)));
        m.apply(
            Event::Ups {
                on_battery: true,
                low_battery: false,
                charge_pct: 80.0,
                load_pct: 7.0,
                runtime_secs: 38 * 60,
            },
            1.0,
        );
        let s = ups(&m, 1.0);
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        assert_eq!(s.pill, Some(("On battery".into(), WARN)));
    }

    fn sky_model(unix: i64) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(7200);
        m.apply(
            Event::Weather {
                temp_c: 18.0,
                code: 2,
                is_day: true,
                wind_kmh: 23.0,
                gust_kmh: 39.0,
                wind_from_deg: 225.0,
                at: String::new(),
            },
            0.0,
        );
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        m.apply(Event::AirQualityHistory { hourly: vec![30.0, 35.0, 32.0] }, 0.0);
        m.apply(
            Event::Sky {
                sunrise: Some(1_788_782_400 - 7 * 3600),
                sunset: Some(1_788_782_400 + 6 * 3600),
                sun_elevation_deg: 45.0,
                moon_illumination: 0.64,
                moon_waxing: true,
                moon_phase: MoonPhase::WaxingGibbous,
            },
            0.0,
        );
        m
    }

    #[test]
    fn wind_and_aqi() {
        let m = sky_model(1_788_782_400);
        let s = wind(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below), ("WIND", "23", Some("km/h"), true));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 23.0 / 60.0).abs() < 1e-5 && color == CYAN));
        assert_eq!(s.pill, Some(("Gusts 39 km/h".into(), TEXT2)));
        assert!(matches!(&s.card, Some(Card::Compass { deg, label }) if *deg == 225.0 && label == "SW · 225°"));
        let s = aqi(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("AIR QUALITY", "32"));
        assert!(matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors.len() == 6 && (frac - 32.0 / 120.0).abs() < 1e-5));
        assert_eq!(s.pill, Some(("Fair".into(), Color::hex(EAQI_BANDS[1]))));
        assert!(matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series == &vec![30.0, 35.0, 32.0] && *now_idx == Some(2) && *min == 0.0 && *max == 60.0 && caption == "24 h"));
        assert_eq!(eaqi_name(5), "Extremely poor");
    }

    #[test]
    fn rain_counts_down_or_reports_dry() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1000);
        let mut slots = vec![0.0f32; 24];
        slots[3] = 0.4;
        slots[4] = 1.8;
        m.apply(
            Event::Rain {
                from: 1000,
                mm_per_h: slots,
            },
            0.0,
        );
        let s = rain(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below), ("RAIN", "15", Some("min"), true));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 1.8 / 5.0).abs() < 1e-5 && color == BLUE));
        assert_eq!(s.pill, Some(("until rain · 1.8 mm/h".into(), BLUE)));
        assert!(matches!(&s.card, Some(Card::Bars { values, now, past_alpha, caption, .. }) if values.len() == 24 && values[4] == 1.8 && *now == 0 && *past_alpha == 1.0 && caption == "next 2 h"));
        let mut wet = Model::new(Thresholds::default());
        wet.apply(
            Event::Rain {
                from: 0,
                mm_per_h: vec![2.3; 24],
            },
            0.0,
        );
        let s = rain(&wet, 0.0);
        assert_eq!((s.hero.as_str(), s.unit.as_deref()), ("2.3", Some("mm/h")));
        assert_eq!(s.pill, Some(("Raining now".into(), BLUE)));
        let mut dry = Model::new(Thresholds::default());
        dry.apply(
            Event::Rain {
                from: 0,
                mm_per_h: vec![0.0; 24],
            },
            0.0,
        );
        let s = rain(&dry, 0.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert_eq!(s.pill, Some(("Dry for 2 h".into(), TEXT2)));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, .. } if frac == 0.0));
    }

    #[test]
    fn sun_moon_and_renewable() {
        // 12:00 UTC = 14:00 local; sunrise 07:00 local, sunset 20:00 local
        let m = sky_model(1_788_782_400);
        let s = sun(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale), ("SUNSET", "20:00", 0.82));
        assert!(matches!(&s.ring.kind, RingKind::Parts(p) if p.len() == 2 && (p[0].0 - 7.0 / 13.0).abs() < 1e-5 && p[0].1 == AMBER && p[1].1 == NIGHT));
        assert_eq!(s.pill, Some(("in 6h 00m".into(), AMBER)));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("07:00".to_string(), "sunrise".to_string(), TEXT) && items[1] == ("13h 00m".to_string(), "daylight".to_string(), TEXT)));
        let night = sun(&sky_model(1_788_782_400 + 9 * 3600), 0.0);
        assert_eq!((night.title.as_str(), night.hero.as_str()), ("SUNRISE", "07:00"));
        assert!(matches!(&night.card, Some(Card::Stats(items)) if items[0].1 == "sunset"));
        let s = moon(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("MOON", "64", Some("%")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.64).abs() < 1e-5 && color == MOON));
        assert_eq!(s.pill, Some(("Waxing gibbous".into(), TEXT2)));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("6 d".to_string(), "to full".to_string(), TEXT) && items[1] == ("21 d".to_string(), "to new".to_string(), TEXT)));
        assert_eq!(MoonPhase::LastQuarter.name(), "Last quarter");
        let mut e = sky_model(1_788_782_400);
        for (h, ren) in [(-3, 55.0), (-2, 60.0), (-1, 70.0), (0, 72.0)] {
            e.set_unix_now(1_788_782_400 + h * 3600);
            e.apply(
                Event::Electricity {
                    zone: "NL".into(),
                    mix_mw: vec![(crate::electricity::Source::Wind, 72.0), (crate::electricity::Source::Gas, 28.0)],
                    renewable_pct: ren,
                    fossil_free_pct: 82.0,
                    carbon_gco2: 100.0,
                    updated_at: String::new(),
                },
                0.0,
            );
        }
        let s = renewable(&e, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.unit.as_deref()), ("RENEWABLE", "72", Some("%")));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.72).abs() < 1e-5 && color == GREEN));
        assert_eq!(s.pill, Some(("Fossil-free 82 %".into(), TEXT2)));
        assert!(matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series == &vec![55.0, 60.0, 70.0, 72.0] && *now_idx == Some(3) && *min == 0.0 && *max == 100.0 && caption == "today"));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::roles::system`
Expected: compile errors (`net`, `ups`, ... not found).

- [ ] **Step 3: Implement**

In `crates/core/src/event.rs`, `impl MoonPhase`:

```rust
    /// The P2 pill text.
    pub fn name(self) -> &'static str {
        match self {
            MoonPhase::New => "New moon",
            MoonPhase::WaxingCrescent => "Waxing crescent",
            MoonPhase::FirstQuarter => "First quarter",
            MoonPhase::WaxingGibbous => "Waxing gibbous",
            MoonPhase::Full => "Full moon",
            MoonPhase::WaningGibbous => "Waning gibbous",
            MoonPhase::LastQuarter => "Last quarter",
            MoonPhase::WaningCrescent => "Waning crescent",
        }
    }
```

Extend the imports of `crates/core/src/p2/roles/system.rs`:

```rust
use crate::format::{fmt_hhmm, fmt_hm, fmt_in, fmt_runtime};
use crate::model::net_fill;
use crate::p2::chart::compass_name;
use crate::scene_rain::{current_slots, first_wet_minutes, WET_MM};
use crate::sky::{daylight_secs, moon_days, next_sun_event, sun_progress, SunEvent};
use crate::theme::eaqi_band;
use crate::theme::p2::{AMBER, BLUE, CYAN, GREEN, MOON, NIGHT, TEXT, TEXT3};
```

and append the builders:

```rust
pub fn net(model: &Model, now: Secs) -> P2Screen {
    let n = model.net();
    let total = n.rx_bps + n.tx_bps;
    let hist = model.net_history();
    let mut s = P2Screen::new("NETWORK", format!("{:.0}", total / 1e6));
    s.unit = Some("Mb/s".into());
    s.unit_below = true;
    s.ring = RingSpec::fill(net_fill(total), BLUE);
    s.pill = Some((format!("↓ {:.0}  ↑ {:.0}", n.rx_bps / 1e6, n.tx_bps / 1e6), TEXT2));
    let series = hist.series();
    let max = (series.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(1.0);
    s.card = Some(Card::Spark {
        series,
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max,
        now_idx: None,
        color: BLUE,
        caption: "download · 60 s".into(),
    });
    s
}

pub fn ups(model: &Model, _now: Secs) -> P2Screen {
    let u = model.ups();
    let mut s = P2Screen::new("UPS", format!("{:.0}", u.charge_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(u.charge_pct / 100.0, if u.on_battery { WARN } else { OK });
    s.pill = Some(if u.on_battery {
        ("On battery".into(), WARN)
    } else {
        ("On mains".into(), OK)
    });
    s.card = Some(Card::Stats(vec![
        (fmt_runtime(u.runtime_secs), "runtime".into(), TEXT),
        (format!("{:.0}%", u.load_pct), "load".into(), TEXT),
    ]));
    s
}

pub fn wind(model: &Model, _now: Secs) -> P2Screen {
    let w = model.weather();
    let mut s = P2Screen::new("WIND", format!("{:.0}", w.wind_kmh));
    s.unit = Some("km/h".into());
    s.unit_below = true;
    s.ring = RingSpec::fill(w.wind_kmh / 60.0, CYAN);
    s.pill = Some((format!("Gusts {:.0} km/h", w.gust_kmh), TEXT2));
    s.card = Some(Card::Compass {
        deg: w.wind_from_deg,
        label: format!("{} · {:.0}°", compass_name(w.wind_from_deg), w.wind_from_deg),
    });
    s
}

pub fn rain(model: &Model, _now: Secs) -> P2Screen {
    let r = model.rain();
    let slots = current_slots(r.from, &r.mm_per_h, model.unix_now());
    let peak = slots.iter().take(24).cloned().fold(0.0_f32, f32::max);
    let now_mm = slots.first().copied().unwrap_or(0.0);
    let mut s = match (now_mm >= WET_MM, first_wet_minutes(&slots)) {
        (true, _) => {
            let mut s = P2Screen::new("RAIN", format!("{now_mm:.1}"));
            s.unit = Some("mm/h".into());
            s.pill = Some(("Raining now".into(), BLUE));
            s
        }
        (false, Some(mins)) => {
            let mut s = P2Screen::new("RAIN", mins.to_string());
            s.unit = Some("min".into());
            s.pill = Some((format!("until rain · {peak:.1} mm/h"), BLUE));
            s
        }
        (false, None) => {
            let mut s = P2Screen::new("RAIN", "—");
            s.hero_color = TEXT3;
            s.unit = Some("min".into());
            s.pill = Some(("Dry for 2 h".into(), TEXT2));
            s
        }
    };
    s.unit_below = true;
    s.ring = RingSpec::fill((peak / 5.0).clamp(0.0, 1.0), BLUE);
    let mut values: Vec<f32> = slots.iter().take(24).copied().collect();
    values.resize(24, 0.0);
    s.card = Some(Card::Bars {
        values,
        colors: vec![BLUE; 24],
        now: 0,
        past_alpha: 1.0,
        caption: "next 2 h".into(),
    });
    s
}

/// The six EAQI levels, good to extremely poor.
pub const EAQI_BANDS: [u32; 6] = [0x45D6A4, 0x9BD07A, 0xE8C860, 0xF2A65A, 0xF0605D, 0xB07CF0];

pub fn eaqi_name(band: usize) -> &'static str {
    ["Good", "Fair", "Moderate", "Poor", "Very poor", "Extremely poor"][band.min(5)]
}

pub fn aqi(model: &Model, _now: Secs) -> P2Screen {
    let a = model.air();
    let band = eaqi_band(a.eaqi);
    let color = Color::hex(EAQI_BANDS[band]);
    let mut s = P2Screen::new("AIR QUALITY", format!("{:.0}", a.eaqi));
    s.ring = RingSpec::bands(EAQI_BANDS.iter().map(|c| Color::hex(*c)).collect(), a.eaqi / 120.0);
    s.pill = Some((eaqi_name(band).into(), color));
    let max = (a.hourly.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(60.0);
    s.card = Some(Card::Spark {
        now_idx: Some(a.hourly.len().saturating_sub(1)),
        series: a.hourly.clone(),
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max,
        color,
        caption: "24 h".into(),
    });
    s
}

pub fn sun(model: &Model, _now: Secs) -> P2Screen {
    let sky = model.sky();
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let Some((event, at)) = next_sun_event(sky.sunrise, sky.sunset, unix) else {
        let mut s = P2Screen::new("SUN", "—");
        s.hero_color = TEXT3;
        s.pill = Some(("Polar day or night".into(), TEXT2));
        return s;
    };
    let title = match event {
        SunEvent::Sunset => "SUNSET",
        SunEvent::Sunrise => "SUNRISE",
    };
    let mut s = P2Screen::new(title, fmt_hhmm(at + offset));
    s.hero_scale = 0.82;
    let progress = sun_progress(sky.sunrise, sky.sunset, unix).unwrap_or(0.0);
    s.ring = RingSpec::parts(vec![(progress, AMBER), (1.0 - progress, NIGHT)]);
    s.pill = Some((fmt_in(at - unix), AMBER));
    let (other_label, other) = match event {
        SunEvent::Sunset => ("sunrise", sky.sunrise),
        SunEvent::Sunrise => ("sunset", sky.sunset),
    };
    s.card = Some(Card::Stats(vec![
        (
            other.map(|t| fmt_hhmm(t + offset)).unwrap_or_else(|| "—".into()),
            other_label.into(),
            TEXT,
        ),
        (
            daylight_secs(sky.sunrise, sky.sunset)
                .map(fmt_hm)
                .unwrap_or_else(|| "—".into()),
            "daylight".into(),
            TEXT,
        ),
    ]));
    s
}

pub fn moon(model: &Model, _now: Secs) -> P2Screen {
    let sky = model.sky();
    let illum = sky.moon_illumination.clamp(0.0, 1.0);
    let mut s = P2Screen::new("MOON", format!("{:.0}", illum * 100.0));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(illum, MOON);
    s.pill = Some((sky.moon_phase.name().into(), TEXT2));
    let (full, new) = moon_days(illum, sky.moon_waxing);
    s.card = Some(Card::Stats(vec![
        (format!("{full:.0} d"), "to full".into(), TEXT),
        (format!("{new:.0} d"), "to new".into(), TEXT),
    ]));
    s
}

pub fn renewable(model: &Model, _now: Secs) -> P2Screen {
    let e = model.electricity();
    let mut s = P2Screen::new("RENEWABLE", format!("{:.0}", e.renewable_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(e.renewable_pct / 100.0, GREEN);
    s.pill = Some((format!("Fossil-free {:.0} %", e.fossil_free_pct), TEXT2));
    let series = model
        .renewable_hourly()
        .values_since(model.local_midnight_unix_hour());
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max: 100.0,
        color: GREEN,
        caption: "today".into(),
    });
    s
}
```

In `crates/core/src/p2/roles/mod.rs` the dispatcher becomes exhaustive except for the two custom roles:

```rust
        Role::Net => system::net(model, now),
        Role::Ups => system::ups(model, now),
        Role::Wind => system::wind(model, now),
        Role::Rain => system::rain(model, now),
        Role::Aqi => system::aqi(model, now),
        Role::Sun => system::sun(model, now),
        Role::Moon => system::moon(model, now),
        Role::Renewable => system::renewable(model, now),
        Role::PowerMix | Role::Face => return None,
```

(remove the `_ => return None` arm).

- [ ] **Step 4: Tests the conversion breaks, and goldens**

- `crates/core/src/scene_net.rs`, `scene_ups.rs`, `scene_weather.rs` (wind/aqi), `scene_rain.rs`, `scene_sky.rs` (sun/moon), `scene_electricity.rs` (`renewable_shows_two_rings_and_alternates_the_badge`): delete the scene-level tests that assert legacy badges/icons (`net_scene_*`, `ups_scene_*`/`badge_*` in ups, `wind_and_aqi_badges`, `badge_says_dry_minutes_or_millimetres`, `sun_scene_*`, `moon_ring_direction_and_badge_alternation`'s badge half); keep pure helper tests.
- `crates/core/src/scene_fx.rs`: in `electricity_roles_go_back_to_no_data_when_the_link_drops` the `Renewable` assertion still uses `has_icon(.., "cloud-off")` (fine); nothing else changes.
- `crates/render/tests/golden.rs`: in `ups_on_battery_and_iss_during_pass` nothing further; in `new_roles_idle` nothing changes.

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden
cargo test --workspace
```

Expected: green; changed PNGs `net`, `ups`, `ups_on_battery`, `wind`, `rain`, `aqi`, `sun`, `moon`, `renewable`. Open `wind.png` (compass strip with SW under the marker) and `sun.png` (amber progress arc, `20:00` hero, `in 6h 00m` pill).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src crates/render/tests
git commit -m "feat(core): P2 screens for net, ups, wind, rain, air quality, sun, moon and renewable

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 13: power-mix: 20 segments, leader at 12 o'clock, capsules

**Files:**
- Create: `crates/core/src/p2/roles/power_mix.rs`
- Modify: `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene.rs` (`Role::PowerMix` arm), `crates/core/src/model.rs` (remove `mix_start`, `mix_leader`, `mix_sections`, `track_mix_leader`, `smooth_mix_start`), `crates/core/src/scene_electricity.rs` (delete `power_mix_scene` and its tests), `crates/core/src/electricity.rs` (delete `partition`, `Section`, `MIX_ICON_MIN_SEGS` and their tests), `crates/core/src/scene_fx.rs` (tests), `crates/render/tests/golden.rs`
- Test: unit tests in `power_mix.rs`

**Interfaces:**
- Consumes: `Model::electricity().mix`, `ScreenUi::{glide, glide_angle, fade}`, `RingSpec::arc`, `Drawable::{Arc, RoundRect, Icon, Run}`, `font::measure`, `text::middle_baseline`.
- Produces: `power_mix::{SEGMENTS, MIX_R, MIX_W, source_color, shares_pct, segment_counts, leader, rotation_for, capsule_centre, power_mix_scene(model, now, ui) -> Scene}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/p2/roles/power_mix.rs` with tests only:

```rust
//! The power-mix ring: twenty 5 % segments coloured by source, the leading
//! source's run centred at 12 o'clock, and a capsule (icon + share) on the
//! ring for every source with at least two segments.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electricity::Source;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ui::ScreenUi;
    use crate::scene::Drawable;

    fn shares() -> Vec<(Source, f32)> {
        vec![
            (Source::Wind, 38.0),
            (Source::Solar, 24.0),
            (Source::Gas, 14.0),
            (Source::Nuclear, 12.0),
            (Source::Coal, 7.0),
            (Source::Biomass, 5.0),
        ]
    }

    #[test]
    fn largest_remainder_fills_twenty_segments_and_the_leader_is_centred() {
        let counts = segment_counts(&shares());
        assert_eq!(counts.iter().map(|c| c.1).sum::<usize>(), SEGMENTS);
        assert_eq!(counts[0], (Source::Wind, 8), "38 % -> 7.6 -> 8 with its remainder");
        assert_eq!(counts[1], (Source::Solar, 5));
        assert_eq!(counts[2], (Source::Gas, 3));
        assert_eq!(counts[5], (Source::Biomass, 1));
        assert_eq!(leader(&counts, &shares()), Some(0));
        let rot = rotation_for(&counts, 0);
        assert!((rot - (-(8.0 / 2.0) / 20.0 * 360.0)).abs() < 1e-4, "wind's run centred on 12 o'clock: {rot}");
        let tie = vec![(Source::Wind, 25.0), (Source::Solar, 25.0), (Source::Gas, 50.0)];
        let c = segment_counts(&tie);
        assert_eq!(leader(&c, &tie), Some(2));
        assert!(segment_counts(&[]).is_empty());
        assert_eq!(segment_counts(&[(Source::Gas, 100.0)]), vec![(Source::Gas, 20)]);
    }

    #[test]
    fn capsules_move_inward_until_they_fit() {
        let (x, y) = capsule_centre(90.0, 60.0, 22.0);
        assert!((y - 120.0).abs() < 1e-3, "3 o'clock stays on the horizontal");
        assert!(x < 120.0 + MIX_R && x > 160.0, "shifted inward from the ring: {x}");
        let (_, y0) = capsule_centre(0.0, 60.0, 22.0);
        assert!(y0 > 120.0 - MIX_R, "12 o'clock moves down: {y0}");
        let (x45, y45) = capsule_centre(45.0, 40.0, 22.0);
        let d = ((x45 - 120.0).powi(2) + (y45 - 120.0).powi(2)).sqrt();
        assert!(d + 22.0 / 2.0 + 3.0 <= 116.0 + 1e-3);
    }

    #[test]
    fn scene_has_twenty_segments_and_a_capsule_per_big_source() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: shares().into_iter().map(|(s, p)| (s, p * 100.0)).collect(),
                renewable_pct: 67.0,
                fossil_free_pct: 79.0,
                carbon_gco2: 150.0,
                updated_at: String::new(),
            },
            0.0,
        );
        let mut ui = ScreenUi::new();
        let s = power_mix_scene(&m, 1.0, &mut ui);
        let arcs = s.items.iter().filter(|d| matches!(d, Drawable::Arc { .. })).count();
        assert_eq!(arcs, SEGMENTS, "one arc per segment, no fades yet");
        let icons: Vec<&str> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .collect();
        assert_eq!(icons, vec!["em-nuclear", "em-wind", "em-solar", "em-gas"], "Source::ALL order; coal (1) and biomass (1) get no capsule");
        let runs: Vec<String> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(runs, vec!["12%", "38%", "24%", "14%"]);
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::CurvedText { .. })), "no title");
        // a leader change turns the ring and fades the segments
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: vec![(Source::Solar, 7000.0), (Source::Wind, 3000.0)],
                renewable_pct: 100.0,
                fossil_free_pct: 100.0,
                carbon_gco2: 20.0,
                updated_at: String::new(),
            },
            2.0,
        );
        let s2 = power_mix_scene(&m, 2.2, &mut ui);
        let arcs2 = s2.items.iter().filter(|d| matches!(d, Drawable::Arc { .. })).count();
        assert!(arcs2 > SEGMENTS, "segments that changed colour draw two layers while fading");
        let start = |s: &crate::scene::Scene| match &s.items[1] {
            Drawable::Arc { start_deg, .. } => *start_deg,
            _ => panic!("arc"),
        };
        assert_ne!(start(&s), start(&s2), "the ring is turning");
        let empty = power_mix_scene(&Model::new(Thresholds::default()), 0.0, &mut ScreenUi::new());
        assert_eq!(empty.items.len(), 1 + SEGMENTS, "track segments only");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::roles::power_mix`
Expected: compile errors.

- [ ] **Step 3: Implement**

Above the tests in `crates/core/src/p2/roles/power_mix.rs`:

```rust
use crate::anim::Secs;
use crate::electricity::Source;
use crate::model::Model;
use crate::p2::font::{measure, FontId};
use crate::p2::layout::EDGE_R;
use crate::p2::ring::{RingKind, RingSpec};
use crate::p2::text::middle_baseline;
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{AMBER, BLUE, CYAN, INK, TRACK, VIOLET};
use crate::theme::{Color, BLACK};

pub const SEGMENTS: usize = 20;
pub const MIX_R: f32 = 104.0;
pub const MIX_W: f32 = 9.0;
pub const MIX_GAP_K: f32 = 1.5;
pub const CAPSULE_H: f32 = 22.0;
pub const CAPSULE_PX: f32 = 10.0;
pub const ICON_PX: f32 = 14.0;
/// Rotation to a new leader.
pub const ROTATE_SECS: Secs = 1.0;
/// Stagger of the segment colour fades.
pub const FADE_STAGGER: Secs = 0.02;

pub fn source_color(s: Source) -> Color {
    match s {
        Source::Wind => CYAN,
        Source::Solar => AMBER,
        Source::Gas => Color::hex(0xF2A65A),
        Source::Nuclear => VIOLET,
        Source::Coal => Color::hex(0x9A8F85),
        Source::Biomass => Color::hex(0x7CCF8E),
        Source::Hydro => BLUE,
        _ => Color::hex(0x5A5A60),
    }
}

/// Percent of total production per source, in `Source::ALL` order, zero shares dropped.
pub fn shares_pct(mix_mw: &[(Source, f32)]) -> Vec<(Source, f32)> {
    let total: f32 = mix_mw.iter().map(|(_, mw)| mw.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut out: Vec<(Source, f32)> = mix_mw
        .iter()
        .map(|(s, mw)| (*s, mw.max(0.0) / total * 100.0))
        .filter(|(_, p)| *p > 0.0)
        .collect();
    out.sort_by_key(|(s, _)| s.index());
    out
}

/// Largest-remainder apportionment of the 20 segments, input order kept.
pub fn segment_counts(shares: &[(Source, f32)]) -> Vec<(Source, usize)> {
    let total: f32 = shares.iter().map(|(_, p)| p.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let raw: Vec<f32> = shares.iter().map(|(_, p)| p.max(0.0) / total * SEGMENTS as f32).collect();
    let mut counts: Vec<usize> = raw.iter().map(|r| r.floor() as usize).collect();
    let mut left = SEGMENTS - counts.iter().sum::<usize>();
    let mut by_rem: Vec<(usize, f32)> = raw.iter().enumerate().map(|(i, r)| (i, r - r.floor())).collect();
    by_rem.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (i, _) in by_rem {
        if left == 0 {
            break;
        }
        counts[i] += 1;
        left -= 1;
    }
    shares.iter().zip(counts).map(|((s, _), c)| (*s, c)).collect()
}

/// Index of the leader: most segments, then the larger share, then `Source::ALL` order.
pub fn leader(counts: &[(Source, usize)], shares: &[(Source, f32)]) -> Option<usize> {
    let share = |i: usize| shares.get(i).map(|s| s.1).unwrap_or(0.0);
    (0..counts.len()).max_by(|&a, &b| {
        counts[a]
            .1
            .cmp(&counts[b].1)
            .then_with(|| share(a).total_cmp(&share(b)))
            .then_with(|| counts[b].0.index().cmp(&counts[a].0.index()))
    })
}

/// Start angle of segment 0 (clockwise from 12 o'clock) that centres the
/// leader's run on 12 o'clock.
pub fn rotation_for(counts: &[(Source, usize)], lead: usize) -> f32 {
    let before: usize = counts.iter().take(lead).map(|c| c.1).sum();
    let mid = before as f32 + counts.get(lead).map(|c| c.1).unwrap_or(0) as f32 / 2.0;
    -mid / SEGMENTS as f32 * 360.0
}

/// Centre of a capsule `w × h` on the ring at `angle_deg`, moved inward along
/// its angle until the whole capsule (3 px outline included) sits inside the edge.
pub fn capsule_centre(angle_deg: f32, w: f32, h: f32) -> (f32, f32) {
    let a = angle_deg.to_radians();
    let mut rr = MIX_R;
    let mut p = (CX + rr * a.sin(), CY - rr * a.cos());
    while rr > 60.0 {
        p = (CX + rr * a.sin(), CY - rr * a.cos());
        let fx = (p.0 - CX).abs() + w / 2.0 + 3.0;
        let fy = (p.1 - CY).abs() + h / 2.0 + 3.0;
        if fx.hypot((p.1 - CY).abs()) < EDGE_R && (p.0 - CX).abs().hypot(fy) < EDGE_R {
            break;
        }
        rr -= 1.0;
    }
    p
}

pub fn power_mix_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    let mut scene = Scene::new();
    // shares glide per source so the counts move one segment at a time
    let shares: Vec<(Source, f32)> = shares_pct(&model.electricity().mix)
        .into_iter()
        .map(|(s, p)| (s, ui.glide(("share", s.index() as u32), p, now)))
        .collect();
    let counts = segment_counts(&shares);
    let rot = match leader(&counts, &shares) {
        Some(lead) => ui.glide_angle(("rot", 0), rotation_for(&counts, lead), now, ROTATE_SECS),
        None => 0.0,
    };
    let spec = RingSpec {
        kind: RingKind::Empty,
        r: MIX_R,
        width: MIX_W,
        start_deg: rot,
        sweep_deg: 360.0,
    };
    let gap = spec.gap_frac(MIX_GAP_K);
    let mut owner: Vec<Color> = Vec::with_capacity(SEGMENTS);
    for (s, c) in &counts {
        for _ in 0..*c {
            owner.push(source_color(*s));
        }
    }
    owner.resize(SEGMENTS, TRACK);
    for (j, color) in owner.iter().enumerate() {
        let f0 = j as f32 / SEGMENTS as f32 + gap / 2.0;
        let f1 = (j + 1) as f32 / SEGMENTS as f32 - gap / 2.0;
        for (c, alpha) in ui.fade(("seg", j as u32), *color, now, j as Secs * FADE_STAGGER) {
            scene.items.extend(spec.arc(f0, f1, c, alpha));
        }
    }
    let mut acc = 0usize;
    for (i, (s, c)) in counts.iter().enumerate() {
        let mid = rot + (acc as f32 + *c as f32 / 2.0) / SEGMENTS as f32 * 360.0;
        acc += c;
        if *c < 2 {
            continue;
        }
        let text = format!("{:.0}%", shares[i].1);
        let tw = measure(FontId::Bold, CAPSULE_PX, &text, 0.0);
        let cw = 16.0 + 3.0 + tw + 14.0;
        let (x, y) = capsule_centre(mid, cw, CAPSULE_H);
        let color = source_color(*s);
        scene.push(Drawable::RoundRect {
            x: x - cw / 2.0 - 3.0,
            y: y - CAPSULE_H / 2.0 - 3.0,
            w: cw + 6.0,
            h: CAPSULE_H + 6.0,
            r: (CAPSULE_H + 6.0) / 2.0,
            color: BLACK,
            alpha: 1.0,
        });
        scene.push(Drawable::RoundRect {
            x: x - cw / 2.0,
            y: y - CAPSULE_H / 2.0,
            w: cw,
            h: CAPSULE_H,
            r: CAPSULE_H / 2.0,
            color,
            alpha: 1.0,
        });
        scene.push(Drawable::Icon {
            name: s.icon(),
            cx: x - cw / 2.0 + 6.0 + ICON_PX / 2.0,
            cy: y,
            size: ICON_PX,
            color: INK,
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        });
        scene.push(Drawable::Run {
            x: x - cw / 2.0 + 23.0,
            baseline: middle_baseline(FontId::Bold, CAPSULE_PX, y + 0.5),
            text,
            font: FontId::Bold,
            px: CAPSULE_PX,
            letter_spacing: 0.0,
            color: INK,
            alpha: 1.0,
            clip: None,
        });
    }
    scene
}
```

Register `pub mod power_mix;` in `crates/core/src/p2/roles/mod.rs`. In `crates/core/src/scene.rs` change the arm `Role::PowerMix => crate::scene_electricity::power_mix_scene(model, now),` to `Role::PowerMix => crate::p2::roles::power_mix::power_mix_scene(model, now, ui),`.

- [ ] **Step 4: Remove the legacy power-mix machinery**

- `crates/core/src/model.rs`: delete the fields `mix_start`, `mix_leader`, `mix_sections` (and their initialisers), `smooth_mix_start`, `track_mix_leader` and its call in `tick`; drop `partition` and `Section` from the `use crate::electricity::{..}` line and `SEG_N` from the `use crate::theme::layout::{..}` line.
- `crates/core/src/scene_electricity.rs`: delete `power_mix_scene`, the constants `MIX_ICON_R`, `MIX_ICON_SIZE`, `MIX_TICK_R0`, `MIX_TICK_R1`, the tests `power_mix_draws_a_section_per_source_and_icons_for_the_big_ones`, `a_new_leader_turns_the_ring_up_to_twelve_oclock`, `power_mix_without_data_is_an_empty_ring` and the helpers only they used (`ring_start`, `icons` if unused), and prune the imports (`partition`, `MIX_ICON_MIN_SEGS`, `breathe` if unused).
- `crates/core/src/electricity.rs`: delete `Section`, `partition`, `MIX_ICON_MIN_SEGS` and the three `partition_*` tests.
- `crates/core/src/scene_fx.rs` tests: in `electricity_roles_go_back_to_no_data_when_the_link_drops` and `electricity_roles_do_not_need_the_api_link` replace `s.lit_count() > 0` with `s.items.iter().filter(|d| matches!(d, Drawable::Arc { .. })).count() >= 20`.
- `crates/render/tests/golden.rs`: delete the solar-yellow pixel assertion in `power_mix_ring` (the leader's run is centred on 12 o'clock; with an even count a gap sits exactly there).

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden power_mix
cargo test --workspace
```

Expected: green; `power_mix.png` shows twenty separated segments and four capsules (solar leads in the golden's mix) with dark icons and shares.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/core/src crates/render/tests
git commit -m "feat(core): P2 power-mix ring with twenty segments, leader at twelve and share capsules

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 14: Face: vector eyes, priority moods, no acts; config and TUI cleanup

**Files:**
- Rewrite: `crates/core/src/mood.rs`, `crates/core/src/face_expr.rs`, `crates/core/src/scene_face.rs`
- Create: `crates/core/src/face.rs`
- Delete: `crates/core/src/face_acts.rs`, `crates/core/src/face_player.rs`, `crates/core/src/face_sprites.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/model.rs` (`Face`, `mood_inputs`, `face_frame`, `set_bedtime_near`, tests), `crates/core/src/scene_fx.rs` (`wants_connecting`, tests), `crates/core/src/scene.rs` (`Role::Face` arm)
- Modify: `crates/app/src/config.rs`, `crates/app/src/run.rs`, `crates/app/src/runloop.rs`, `config.example.yaml`, `crates/setup/src/screens/configure.rs`
- Modify: `crates/render/tests/golden.rs` (face tests)
- Test: unit tests in `mood.rs`, `face_expr.rs`, `face.rs`, `scene_face.rs`, `model.rs`, `config.rs`, `configure.rs`

**Interfaces:**
- Produces: `mood::{Mood, MoodInputs, mood(&MoodInputs) -> Mood}`; `face_expr::{Expr { open, tilt, lower, scale, lift }, Expr::of(Mood), ExprTween, Idle, IdleOut { blink, gaze, wobble }}`; `face::{Face::{new, set_seed, tick(mood, now), frame(now) -> FaceFrame}, FaceFrame { e, blink, gaze, wobble, mood }}`; `scene_face::{face_scene(model, now), render_face(&FaceFrame) -> Scene}`; `Model::{mood_inputs(), face_mood(), face_frame(now), set_bedtime_near(bool)}`. Removed: `FaceCfg`, `FaceTuning`, `Model::{face, set_face_tuning, face_inputs}`, `RenderLoop::face`, setup `Field::Face*`.

- [ ] **Step 1: Write the failing tests**

Replace `crates/core/src/mood.rs` entirely with its tests:

```rust
//! The face's mood: the first matching rack state wins.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_table_first_match_wins() {
        let all = MoodInputs {
            on_battery: true,
            nodes_not_ready: true,
            alerts: true,
            app_degraded: true,
            volume_degraded: true,
            hot: true,
            pods_failed: true,
            recent_restart: true,
            bedtime: true,
            ci_passed_recently: true,
            cheap_price: true,
            clear_day: true,
        };
        let mut i = all;
        assert_eq!(mood(&i), Mood::Scared);
        i.on_battery = false;
        assert_eq!(mood(&i), Mood::Sad);
        i.nodes_not_ready = false;
        assert_eq!(mood(&i), Mood::Angry);
        i.alerts = false;
        assert_eq!(mood(&i), Mood::Angry, "a degraded app is enough");
        i.app_degraded = false;
        assert_eq!(mood(&i), Mood::Angry, "so is a degraded volume");
        i.volume_degraded = false;
        assert_eq!(mood(&i), Mood::Hot);
        i.hot = false;
        assert_eq!(mood(&i), Mood::Worried);
        i.pods_failed = false;
        assert_eq!(mood(&i), Mood::Worried, "a recent restart worries too");
        i.recent_restart = false;
        assert_eq!(mood(&i), Mood::Sleepy);
        i.bedtime = false;
        assert_eq!(mood(&i), Mood::Happy);
        i.ci_passed_recently = false;
        i.cheap_price = false;
        assert_eq!(mood(&i), Mood::Happy, "a clear day alone is happy");
        i.clear_day = false;
        assert_eq!(mood(&i), Mood::Content);
        assert_eq!(mood(&MoodInputs::default()), Mood::Content);
        assert_eq!(Mood::ALL.len(), 8);
        assert_eq!(Mood::Sleepy.name(), "sleepy");
    }
}
```

Replace the tests of `crates/core/src/face_expr.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mood_table_matches_the_spec() {
        let check = |m: Mood, open: f32, tilt: f32, lower: f32, scale: f32, lift: f32| {
            let e = Expr::of(m);
            assert_eq!((e.open, e.tilt, e.lower, e.scale, e.lift), (open, tilt, lower, scale, lift), "{m:?}");
        };
        check(Mood::Scared, 1.30, -0.40, 0.0, 0.85, -2.0);
        check(Mood::Sad, 0.65, -1.00, 0.0, 1.0, 8.0);
        check(Mood::Angry, 0.60, 1.00, 0.15, 1.0, 0.0);
        check(Mood::Hot, 0.70, 0.30, 0.20, 1.0, 3.0);
        check(Mood::Worried, 0.85, -0.60, 0.10, 1.0, 2.0);
        check(Mood::Sleepy, 0.25, 0.0, 0.0, 1.0, 6.0);
        check(Mood::Happy, 1.00, 0.0, 0.70, 1.0, -4.0);
        check(Mood::Content, 1.00, 0.0, 0.0, 1.0, 0.0);
    }

    #[test]
    fn expr_tween_retargets_without_a_jump() {
        let mut t = ExprTween::new(Expr::of(Mood::Content));
        assert_eq!(t.value(5.0), Expr::of(Mood::Content));
        t.retarget(Expr::of(Mood::Sad), 10.0);
        assert!(t.value(10.0).close_to(&Expr::of(Mood::Content), 1e-4));
        let mid = t.value(10.21);
        assert!(mid.lift > 0.5 && mid.lift < 7.5, "half way: {}", mid.lift);
        assert!(t.value(10.42).close_to(&Expr::of(Mood::Sad), 1e-4));
        t.retarget(Expr::of(Mood::Content), 10.21);
        assert!(t.value(10.21).close_to(&mid, 1e-4));
    }

    #[test]
    fn idle_blinks_wanders_and_is_deterministic() {
        let mut rng = 99u64;
        let mut idle = Idle::new();
        let mut blinked = false;
        let mut gazes = std::collections::HashSet::new();
        let mut t = 0.0;
        while t < 30.0 {
            let o = idle.tick(Mood::Content, t, &mut rng);
            if o.blink > 0.9 {
                blinked = true;
            }
            assert!((0.0..=1.0).contains(&o.blink));
            assert!(o.gaze.0.abs() <= 1.0 && o.gaze.1.abs() <= 1.0);
            gazes.insert(((o.gaze.0 * 100.0) as i32, (o.gaze.1 * 100.0) as i32));
            assert!(o.wobble.1.abs() <= 1.5 + 1e-6, "content only breathes");
            t += 1.0 / 30.0;
        }
        assert!(blinked, "no full blink in 30 s");
        assert!(gazes.len() > 20, "gaze never wandered");
        let mut a = (Idle::new(), 7u64);
        let mut b = (Idle::new(), 7u64);
        for i in 0..900 {
            let t = i as f64 / 30.0;
            assert_eq!(a.0.tick(Mood::Worried, t, &mut a.1), b.0.tick(Mood::Worried, t, &mut b.1));
        }
    }

    #[test]
    fn moods_shape_the_idle_life() {
        let mut rng = 3u64;
        let mut idle = Idle::new();
        let mut max_blink: f32 = 0.0;
        let mut t = 0.0;
        while t < 20.0 {
            max_blink = max_blink.max(idle.tick(Mood::Scared, t, &mut rng).blink);
            t += 1.0 / 30.0;
        }
        assert!(max_blink > 0.5 && max_blink <= 0.6 + 1e-3, "scared blinks 60 %: {max_blink}");
        let mut idle = Idle::new();
        let mut down = true;
        let mut t = 0.0;
        while t < 20.0 {
            let g = idle.tick(Mood::Sad, t, &mut rng).gaze;
            if t > 1.0 && g.1 < 0.3 {
                down = false;
            }
            t += 1.0 / 30.0;
        }
        assert!(down, "sad looks down");
        let mut idle = Idle::new();
        let mut blinks = 0;
        let mut was = false;
        let mut t = 0.0;
        while t < 60.0 {
            let b = idle.tick(Mood::Sleepy, t, &mut rng).blink > 0.5;
            if b && !was {
                blinks += 1;
            }
            was = b;
            t += 1.0 / 30.0;
        }
        assert!((8..=16).contains(&blinks), "sleepy blinks about every 5 s: {blinks}");
        let mut idle = Idle::new();
        let w = idle.tick(Mood::Scared, 1.0, &mut rng).wobble;
        assert!(w.0.abs() > 0.0, "scared trembles sideways");
        let s = idle.tick(Mood::Sleepy, 1.0, &mut rng).wobble;
        assert!(s.1 > 0.0, "sleepy nods down");
    }
}
```

Create `crates/core/src/face.rs` with tests only:

```rust
//! The face: a mood tween plus the idle life, producing one `FaceFrame` per tick.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mood_changes_tween_and_frames_are_pure_in_now() {
        let mut f = Face::new();
        f.set_seed(42);
        f.tick(Mood::Content, 0.0);
        f.tick(Mood::Sad, 1.0);
        let at = f.frame(1.21);
        assert!(at.e.lift > 0.5 && at.e.lift < 7.5, "half way to sad: {}", at.e.lift);
        assert_eq!(at.mood, Mood::Sad);
        assert_eq!(f.frame(1.21), at, "the same instant gives the same frame");
        f.tick(Mood::Sad, 2.0);
        assert!((f.frame(2.0).e.lift - 8.0).abs() < 1e-4);
        f.tick(Mood::Content, 3.0);
        assert!((f.frame(3.42).e.lift).abs() < 1e-4, "back after 0.42 s");
    }
}
```

Replace the tests of `crates/core/src/scene_face.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::FaceFrame;
    use crate::face_expr::Expr;
    use crate::mood::Mood;
    use crate::theme::p2::TEXT;

    fn frame(m: Mood) -> FaceFrame {
        FaceFrame {
            e: Expr::of(m),
            blink: 0.0,
            gaze: (0.0, 0.0),
            wobble: (0.0, 0.0),
            mood: m,
        }
    }

    fn eyes(s: &Scene) -> Vec<(f32, f32, f32, f32, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::RoundRect { x, y, w, h, r, color, .. } if *color == TEXT => Some((*x, *y, *w, *h, *r)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn content_is_two_open_eyes_and_nothing_else() {
        let s = render_face(&frame(Mood::Content));
        let e = eyes(&s);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].2, e[0].3, e[0].4), (56.0, 68.0, 18.0));
        assert!((e[0].0 + 28.0 - 72.0).abs() < 1e-3, "left eye centred at 120 - 48");
        assert!((e[1].0 + 28.0 - 168.0).abs() < 1e-3);
        assert!((e[0].1 + 34.0 - 118.0).abs() < 1e-3);
        assert_eq!(s.items.len(), 3, "clear and two eyes");
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Arc { .. } | Drawable::CurvedText { .. } | Drawable::Icon { .. })));
    }

    #[test]
    fn moods_add_lids_blinks_squash_and_gaze_shifts() {
        let sad = render_face(&frame(Mood::Sad));
        let lids: Vec<&Drawable> = sad.items.iter().filter(|d| matches!(d, Drawable::Polygon { .. })).collect();
        assert_eq!(lids.len(), 2, "an upper lid per eye");
        if let Drawable::Polygon { points, .. } = lids[0] {
            // left eye, side -1: inner corner (x = cx + w/2) at the top, outer corner lowered by k = 0.55 * 56 * 1.0
            assert!((points[3].1 - points[2].1 + 30.8).abs() < 1e-3, "tilt -1 lowers the outer corner: {points:?}");
        }
        let happy = render_face(&frame(Mood::Happy));
        let lower: Vec<&Drawable> = happy.items.iter().filter(|d| matches!(d, Drawable::Ellipse { .. })).collect();
        assert_eq!(lower.len(), 2, "a lower lid per eye");
        if let Drawable::Ellipse { rx, ry, .. } = lower[0] {
            let rise = 0.75 * 0.7 * 68.0;
            assert!((*rx - 42.0).abs() < 1e-3 && (*ry - 1.4 * rise).abs() < 1e-3);
        }
        let mut blink = frame(Mood::Content);
        blink.blink = 1.0;
        let shut = eyes(&render_face(&blink));
        assert!((shut[0].3 - 68.0 * 0.04).abs() < 1e-3, "a full blink leaves a sliver");
        let mut look = frame(Mood::Content);
        look.gaze = (1.0, -1.0);
        look.wobble = (2.0, 3.0);
        let moved = eyes(&render_face(&look));
        assert!((moved[0].0 - (eyes(&render_face(&frame(Mood::Content)))[0].0 + 12.0)).abs() < 1e-3);
        assert!((moved[0].1 - (eyes(&render_face(&frame(Mood::Content)))[0].1 - 5.0)).abs() < 1e-3);
        let scared = eyes(&render_face(&frame(Mood::Scared)));
        assert!((scared[0].2 - 56.0 * 0.85).abs() < 1e-3 && (scared[0].3 - 68.0 * 0.85 * 1.3).abs() < 1e-3);
    }
}
```

Replace the four face tests at the end of `crates/core/src/model.rs` (`fx_requests_reach_the_face`, `link_down_reaches_the_face_and_link_up_after_boot_does_too`, `face_inputs_reduce_cluster_state`, `face_scene_is_dot_rows`) with:

```rust
    #[test]
    fn mood_inputs_reduce_rack_state_and_the_face_follows() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 4,
                not_ready: vec!["pi-4".into()],
            },
            0.0,
        );
        let i = m.mood_inputs();
        assert!(i.nodes_not_ready && !i.on_battery && !i.hot);
        m.tick(1.0);
        assert_eq!(m.face_mood(), crate::mood::Mood::Sad);
        m.apply(metrics(95.0, None), 2.0);
        assert!(m.mood_inputs().hot, "cpu over hot_cpu");
        m.apply(
            Event::NodeSnapshot {
                ready: 4,
                total: 4,
                not_ready: vec![],
            },
            3.0,
        );
        m.apply(metrics(10.0, None), 3.0);
        m.set_bedtime_near(true);
        m.tick(3.0);
        assert_eq!(m.face_mood(), crate::mood::Mood::Sleepy);
        m.set_bedtime_near(false);
        m.apply(
            Event::Weather {
                temp_c: 21.0,
                code: 0,
                is_day: true,
                wind_kmh: 1.0,
                gust_kmh: 1.0,
                wind_from_deg: 0.0,
                at: String::new(),
            },
            4.0,
        );
        m.tick(4.0);
        assert_eq!(m.face_mood(), crate::mood::Mood::Happy, "a clear day");
        let f = m.face_frame(4.2);
        assert!(f.e.lower > 0.0 && f.e.lower < 0.7, "tweening toward happy: {}", f.e.lower);
        assert_eq!(m.face_frame(4.2), f, "pure in `now`");
        let s = m.scene_for_role(Role::Face, 4.2, &mut ScreenUi::new());
        assert!(s.items.iter().any(|d| matches!(d, Drawable::RoundRect { .. })));
    }
```

(add `use crate::scene::Drawable;` to that test module if it is not there.)

Replace the test `face_defaults_and_validation` in `crates/app/src/config.rs` with:

```rust
    #[test]
    fn a_legacy_face_block_is_ignored() {
        let c = Config::from_yaml("face:\n  bored_after_mins: 4\n  idle_habits: false\nnight:\n  enabled: false\n").unwrap();
        assert!(!c.night.enabled);
        c.validate().unwrap();
        assert!(!c.to_yaml().unwrap().contains("face:"), "never written back");
    }
```

In `crates/setup/src/screens/configure.rs` delete the test `face_fields_round_trip_and_clamp` and change `assert_eq!(group_indices(Group::Thresholds).len(), 7);` to `3`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core mood face`
Expected: compile errors.

- [ ] **Step 3: Implement the core face**

`crates/core/src/mood.rs` above its tests:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Scared,
    Sad,
    Angry,
    Hot,
    Worried,
    Sleepy,
    Happy,
    Content,
}

impl Mood {
    pub const ALL: [Mood; 8] = [
        Mood::Scared,
        Mood::Sad,
        Mood::Angry,
        Mood::Hot,
        Mood::Worried,
        Mood::Sleepy,
        Mood::Happy,
        Mood::Content,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mood::Scared => "scared",
            Mood::Sad => "sad",
            Mood::Angry => "angry",
            Mood::Hot => "hot",
            Mood::Worried => "worried",
            Mood::Sleepy => "sleepy",
            Mood::Happy => "happy",
            Mood::Content => "content",
        }
    }
}

/// The rack facts the mood is read from, already reduced to booleans. The CI
/// and restart inputs come from later sources; without them they stay false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoodInputs {
    pub on_battery: bool,
    pub nodes_not_ready: bool,
    pub alerts: bool,
    pub app_degraded: bool,
    pub volume_degraded: bool,
    pub hot: bool,
    pub pods_failed: bool,
    pub recent_restart: bool,
    pub bedtime: bool,
    pub ci_passed_recently: bool,
    pub cheap_price: bool,
    pub clear_day: bool,
}

/// First match wins, in the spec's order.
pub fn mood(i: &MoodInputs) -> Mood {
    if i.on_battery {
        Mood::Scared
    } else if i.nodes_not_ready {
        Mood::Sad
    } else if i.alerts || i.app_degraded || i.volume_degraded {
        Mood::Angry
    } else if i.hot {
        Mood::Hot
    } else if i.pods_failed || i.recent_restart {
        Mood::Worried
    } else if i.bedtime {
        Mood::Sleepy
    } else if i.ci_passed_recently || i.cheap_price || i.clear_day {
        Mood::Happy
    } else {
        Mood::Content
    }
}
```

`crates/core/src/face_expr.rs` above its tests (replacing everything: the act helpers `seg`, `inseg`, `bump`, `out_back` go; `lerp`, `ease`, `xorshift`, `unit` stay):

```rust
//! The face's expression: five knobs per mood, tweened, plus the idle life
//! (blinks, gaze, breathing and mood wobble) that keeps it alive.

use crate::anim::{Easing, Secs};
use crate::mood::Mood;

pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}
pub fn ease(k: f32) -> f32 {
    Easing::InOutCubic.apply(k)
}
/// xorshift64, the same generator `Model` uses for its screen order.
pub fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}
/// Uniform in `0..1`.
pub fn unit(state: &mut u64) -> f32 {
    (xorshift(state) >> 11) as f32 / (1u64 << 53) as f32
}

/// Lid openness, upper-lid slant (+ inner corners down = angry, − = sad),
/// lower-lid rise (0.7 is the `^ ^` shape), eye size and vertical lift in
/// pixels (negative is up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expr {
    pub open: f32,
    pub tilt: f32,
    pub lower: f32,
    pub scale: f32,
    pub lift: f32,
}

const fn expr(open: f32, tilt: f32, lower: f32, scale: f32, lift: f32) -> Expr {
    Expr {
        open,
        tilt,
        lower,
        scale,
        lift,
    }
}

impl Expr {
    pub fn of(m: Mood) -> Expr {
        match m {
            Mood::Scared => expr(1.30, -0.40, 0.00, 0.85, -2.0),
            Mood::Sad => expr(0.65, -1.00, 0.00, 1.00, 8.0),
            Mood::Angry => expr(0.60, 1.00, 0.15, 1.00, 0.0),
            Mood::Hot => expr(0.70, 0.30, 0.20, 1.00, 3.0),
            Mood::Worried => expr(0.85, -0.60, 0.10, 1.00, 2.0),
            Mood::Sleepy => expr(0.25, 0.00, 0.00, 1.00, 6.0),
            Mood::Happy => expr(1.00, 0.00, 0.70, 1.00, -4.0),
            Mood::Content => expr(1.00, 0.00, 0.00, 1.00, 0.0),
        }
    }
    pub fn lerp(a: Expr, b: Expr, k: f32) -> Expr {
        expr(
            lerp(a.open, b.open, k),
            lerp(a.tilt, b.tilt, k),
            lerp(a.lower, b.lower, k),
            lerp(a.scale, b.scale, k),
            lerp(a.lift, b.lift, k),
        )
    }
    pub fn close_to(&self, o: &Expr, eps: f32) -> bool {
        (self.open - o.open).abs() <= eps
            && (self.tilt - o.tilt).abs() <= eps
            && (self.lower - o.lower).abs() <= eps
            && (self.scale - o.scale).abs() <= eps
            && (self.lift - o.lift).abs() <= eps
    }
}

pub const EXPR_TWEEN_SECS: Secs = 0.42;

/// An expression easing toward a retargetable goal, never jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExprTween {
    from: Expr,
    to: Expr,
    start: Secs,
}

impl ExprTween {
    pub fn new(still: Expr) -> Self {
        Self {
            from: still,
            to: still,
            start: -1.0e9,
        }
    }
    pub fn retarget(&mut self, to: Expr, now: Secs) {
        if to == self.to {
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.start = now;
    }
    pub fn value(&self, now: Secs) -> Expr {
        let k = ((now - self.start) / EXPR_TWEEN_SECS).clamp(0.0, 1.0) as f32;
        Expr::lerp(self.from, self.to, ease(k))
    }
    pub fn target(&self) -> Expr {
        self.to
    }
}

/// What the idle layer contributes this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdleOut {
    /// 0 open .. 1 shut, already scaled by the mood's blink depth.
    pub blink: f32,
    /// Where the eyes look, each axis in −1..1.
    pub gaze: (f32, f32),
    /// Breathing bob plus the mood wobble, in pixels.
    pub wobble: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Idle {
    next_blink: Secs,
    blink_start: Option<Secs>,
    gaze_from: (f32, f32),
    gaze_to: (f32, f32),
    gaze_start: Secs,
    gaze_dur: Secs,
    next_gaze: Secs,
}

impl Default for Idle {
    fn default() -> Self {
        Self::new()
    }
}

impl Idle {
    pub fn new() -> Self {
        Self {
            next_blink: 1.5,
            blink_start: None,
            gaze_from: (0.0, 0.0),
            gaze_to: (0.0, 0.0),
            gaze_start: 0.0,
            gaze_dur: 0.26,
            next_gaze: 0.8,
        }
    }

    pub fn tick(&mut self, mood: Mood, now: Secs, rng: &mut u64) -> IdleOut {
        // blink: 150 ms every 1.5..4 s, about every 5 s when sleepy
        if self.blink_start.is_none() && now >= self.next_blink {
            self.blink_start = Some(now);
        }
        let mut blink = 0.0;
        if let Some(start) = self.blink_start {
            let p = ((now - start) / 0.15) as f32;
            if p < 1.0 {
                blink = (p * std::f32::consts::PI).sin();
            } else {
                self.blink_start = None;
                self.next_blink = now
                    + if mood == Mood::Sleepy {
                        4.0 + 2.0 * unit(rng) as Secs
                    } else {
                        1.5 + 2.5 * unit(rng) as Secs
                    };
            }
        }
        if matches!(mood, Mood::Scared | Mood::Angry) {
            blink *= 0.6;
        }
        // gaze: retarget every 0.7..2 s, 0.35..0.85 s when jittery; sad and sleepy look down
        let fast = matches!(mood, Mood::Worried | Mood::Scared);
        if now >= self.next_gaze {
            self.gaze_from = self.gaze_value(now);
            self.gaze_to = match mood {
                Mood::Sad | Mood::Sleepy => ((unit(rng) - 0.5) * 0.6, 0.7),
                _ => ((unit(rng) - 0.5) * 1.4, (unit(rng) - 0.5) * 0.9),
            };
            self.gaze_start = now;
            self.gaze_dur = if fast { 0.12 } else { 0.26 };
            self.next_gaze = now
                + if fast {
                    0.35 + 0.5 * unit(rng) as Secs
                } else {
                    0.7 + 1.3 * unit(rng) as Secs
                };
        }
        let gaze = self.gaze_value(now);
        // f64 sines: an f32 clock would stutter after a few days of uptime
        let s = |w: f64| (now * w).sin() as f32;
        let mut wobble = (0.0, 1.5 * s(0.9));
        match mood {
            Mood::Angry => {
                if s(1.3) > 0.2 {
                    wobble.0 = s(46.0) * 2.2;
                }
            }
            Mood::Scared => wobble.0 = s(70.0) * 1.4,
            Mood::Sleepy => wobble.1 += s(1.4) * 4.0 + 3.0,
            Mood::Hot => wobble.1 += s(2.2) * 1.2,
            _ => {}
        }
        IdleOut {
            blink,
            gaze,
            wobble,
        }
    }

    fn gaze_value(&self, now: Secs) -> (f32, f32) {
        let k = ease(((now - self.gaze_start) / self.gaze_dur).clamp(0.0, 1.0) as f32);
        (
            lerp(self.gaze_from.0, self.gaze_to.0, k),
            lerp(self.gaze_from.1, self.gaze_to.1, k),
        )
    }
}
```

`crates/core/src/face.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::face_expr::{Expr, ExprTween, Idle, IdleOut};
use crate::mood::Mood;

/// What `scene_face` draws: the tweened expression and the idle life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceFrame {
    pub e: Expr,
    pub blink: f32,
    pub gaze: (f32, f32),
    pub wobble: (f32, f32),
    pub mood: Mood,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    tween: ExprTween,
    idle: Idle,
    rng: u64,
    mood: Mood,
    last: IdleOut,
}

impl Default for Face {
    fn default() -> Self {
        Self::new()
    }
}

impl Face {
    pub fn new() -> Face {
        Face {
            tween: ExprTween::new(Expr::of(Mood::Content)),
            idle: Idle::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            mood: Mood::Content,
            last: IdleOut {
                blink: 0.0,
                gaze: (0.0, 0.0),
                wobble: (0.0, 0.0),
            },
        }
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.rng = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
    }

    pub fn mood(&self) -> Mood {
        self.mood
    }

    /// Advance once per frame with the mood the rack state gives.
    pub fn tick(&mut self, mood: Mood, now: Secs) {
        if mood != self.mood {
            self.mood = mood;
            self.tween.retarget(Expr::of(mood), now);
        }
        self.last = self.idle.tick(mood, now, &mut self.rng);
    }

    /// The frame at `now`: the expression is re-evaluated, the idle life is
    /// the last tick's, so two reads at the same instant agree.
    pub fn frame(&self, now: Secs) -> FaceFrame {
        FaceFrame {
            e: self.tween.value(now),
            blink: self.last.blink,
            gaze: self.last.gaze,
            wobble: self.last.wobble,
            mood: self.mood,
        }
    }
}
```

`crates/core/src/scene_face.rs` above its tests (replacing the whole dot-matrix file):

```rust
//! The face role: two solid vector eyes with mood-shaped lids. No ring, title,
//! pill or card.

use crate::anim::Secs;
use crate::face::FaceFrame;
use crate::model::Model;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::TEXT;
use crate::theme::BLACK;

pub const EYE_W: f32 = 56.0;
pub const EYE_H: f32 = 68.0;
pub const EYE_CY: f32 = 118.0;
pub const EYE_DX: f32 = 48.0;
pub const EYE_RADIUS: f32 = 18.0;
pub const GAZE_X: f32 = 10.0;
pub const GAZE_Y: f32 = 8.0;
/// A full blink still leaves this much of the eye.
pub const MIN_OPEN: f32 = 0.04;

pub fn face_scene(model: &Model, now: Secs) -> Scene {
    render_face(&model.face_frame(now))
}

pub fn render_face(f: &FaceFrame) -> Scene {
    let e = &f.e;
    let open = (e.open * (1.0 - f.blink)).max(MIN_OPEN);
    let mut s = Scene::new();
    for side in [-1.0f32, 1.0] {
        let w = EYE_W * e.scale;
        let h = EYE_H * e.scale * open;
        let cx = CX + side * EYE_DX + f.gaze.0 * GAZE_X + f.wobble.0;
        let cy = EYE_CY + e.lift + f.gaze.1 * GAZE_Y + f.wobble.1;
        s.push(Drawable::RoundRect {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
            r: (h / 2.0).min(EYE_RADIUS),
            color: TEXT,
            alpha: 1.0,
        });
        if e.tilt.abs() > 0.01 {
            let k = 0.55 * w * e.tilt.abs();
            let top = cy - h / 2.0;
            let inner = cx - side * w / 2.0;
            let outer = cx + side * w / 2.0;
            let d_inner = if e.tilt > 0.0 { k } else { 0.0 };
            let d_outer = if e.tilt < 0.0 { k } else { 0.0 };
            s.push(Drawable::Polygon {
                points: vec![
                    (inner, top - 60.0),
                    (outer, top - 60.0),
                    (outer, top + d_outer),
                    (inner, top + d_inner),
                ],
                color: BLACK,
                alpha: 1.0,
            });
        }
        if e.lower > 0.01 {
            let rise = 0.75 * e.lower * EYE_H * e.scale;
            s.push(Drawable::Ellipse {
                cx,
                cy: cy + h / 2.0 + rise * 0.4,
                rx: 0.75 * w,
                ry: 1.4 * rise,
                color: BLACK,
                alpha: 1.0,
            });
        }
    }
    s
}
```

`crates/core/src/lib.rs`: remove `pub mod face_acts;`, `pub mod face_player;`, `pub mod face_sprites;`; add `pub mod face;`. Delete the three files:

```bash
git rm crates/core/src/face_acts.rs crates/core/src/face_player.rs crates/core/src/face_sprites.rs
```

`crates/core/src/model.rs`:

```rust
// imports: replace the face_player and mood lines with
use crate::face::{Face, FaceFrame};
use crate::mood::{mood, Mood, MoodInputs};
// field: replace `face: FacePlayer` with
    face: Face,
    /// Half an hour before night mode and ten minutes after: the face is sleepy.
    bedtime_near: bool,
// init: replace `face: FacePlayer::new(0.0)` with
            face: Face::new(),
            bedtime_near: false,
// replace `face()`, `set_face_tuning`, `set_bedtime_near` and `face_inputs` with:
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.bedtime_near = near;
    }

    /// The rack facts the face reads each frame.
    pub fn mood_inputs(&self) -> MoodInputs {
        let st = &self.state;
        let th = self.thresholds;
        let hottest = st.temps.iter().map(|(_, c)| *c).fold(0.0_f32, f32::max);
        MoodInputs {
            on_battery: self.ups.have && self.ups.on_battery,
            nodes_not_ready: st.have_nodes && st.nodes_ready < st.nodes_total,
            alerts: !st.alerts.is_empty(),
            app_degraded: self
                .apps
                .apps
                .iter()
                .any(|a| a.health == AppHealth::Degraded),
            volume_degraded: st
                .volumes
                .iter()
                .any(|(_, r)| matches!(r, Robustness::Degraded | Robustness::Faulted)),
            hot: (st.have_temps && hottest >= th.hot_temp)
                || (st.have_metrics && (st.cpu_pct >= th.hot_cpu || st.mem_pct >= th.hot_mem)),
            pods_failed: st.pods_failed > 0,
            recent_restart: false,
            bedtime: self.bedtime_near,
            ci_passed_recently: false,
            cheap_price: self
                .current_price()
                .is_some_and(|p| price_level(p, self.prices.avg).0.index() <= PriceLevel::Cheap.index()),
            clear_day: self.weather.have && self.weather.code == 0 && self.weather.is_day,
        }
    }

    pub fn face_mood(&self) -> Mood {
        self.face.mood()
    }

    pub fn face_frame(&self, now: Secs) -> FaceFrame {
        self.face.frame(now)
    }
```

(add `PriceLevel` to the `use crate::electricity::{..}` import.) In `tick`, replace

```rust
        for req in std::mem::take(&mut self.fx) {
            self.face.on_fx(&req, now);
            self.fx_state.apply(req, now);
        }
        let inputs = self.face_inputs(now);
        self.face.tick(&inputs, now);
```

with

```rust
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
        let mood = mood(&self.mood_inputs());
        self.face.tick(mood, now);
```

In the `LinkTarget::K8sApi` arm delete the `if was && !up { self.face.on_link_down(now); }` lines. In `set_rng_seed` keep `self.face.set_seed(seed);`.

`crates/core/src/scene_fx.rs`: `wants_connecting` becomes `role.is_cluster() && !self.link().api`; remove the `use crate::face_acts::ActKind;` import. The `Role::Face` arm in `role_scene` stays `crate::scene_face::face_scene(model, now)`.

- [ ] **Step 4: Config, run loop and TUI**

`crates/app/src/config.rs`: delete `FaceCfg`, its `impl`, its `Default`, the `pub face: FaceCfg` field and the two `face.*` validations; make `from_yaml` warn once:

```rust
    pub fn from_yaml(text: &str) -> Result<Config> {
        let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).context("parse yaml")?;
        if raw.get("face").is_some() {
            tracing::warn!("config: the `face:` block is no longer used and is ignored");
        }
        let mut cfg: Config = serde_yaml_ng::from_str(text).context("parse yaml")?;
        cfg.normalize();
        Ok(cfg)
    }
```

`crates/app/src/run.rs`: delete `face: cfg.face.tuning(),`. `crates/app/src/runloop.rs`: delete `use rackscreen_core::face_player::FaceTuning;`, the `pub face: FaceTuning` field and `model.set_face_tuning(self.face);`. `config.example.yaml`: delete the `face:` block (five lines).

`crates/setup/src/screens/configure.rs`: delete `Field::FaceBored`, `Field::FaceReaction`, `Field::FaceIdle`, `Field::FaceWeather` from the enum, their four `spec(..)` entries in `FIELDS`, their arms in `get` and `set`.

`crates/render/tests/golden.rs`: delete `face_mid_act_frames` and `face_sunny_shades_and_sleepy`; keep `face_content_and_sad`, change its pixel checks to the new eyes (off-white at `(72, 118)`, dark at `(120, 118)`):

```rust
    let (red, g, b) = pixel(&px, 72, 118);
    assert!(red > 200 && g > 200 && b > 200, "left eye off-white, got {red},{g},{b}");
```

and add a sleepy golden to it:

```rust
    let mut m = ready_model();
    m.set_bedtime_near(true);
    tick_until(&mut m, 0.0, 5.0);
    let px = render_face(&m, 5.0);
    let (red, ..) = pixel(&px, 72, 95);
    assert!(red < 60, "sleepy: the top of the eye is closed, got {red}");
    check("face_sleepy", &px);
```

(`render_face` there already calls `scene_for_role(Role::Face, now, &mut ScreenUi::new())`.) Then:

```bash
git rm crates/render/tests/goldens/face_ouch_mid.png crates/render/tests/goldens/face_launch_mid.png crates/render/tests/goldens/face_sunny_shades.png
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden face
cargo test --workspace
```

Expected: green; `face_content.png` shows two off-white rounded eyes, `face_sad.png` droopy outer corners, `face_sleepy.png` thin slits.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A crates/core/src crates/app crates/setup crates/render/tests config.example.yaml
git commit -m "feat(core,app,setup): vector face with priority moods; drop the act system and the face config

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 15: P2 status screens; remove splashes, sweeps, `ring_reveal`, the legacy drawables and dead code

**Files:**
- Create: `crates/core/src/p2/status.rs`
- Delete: `crates/core/src/fx.rs`, `crates/core/src/scene_deploys.rs`, `crates/core/src/scene_electricity.rs`, `crates/core/src/scene_github.rs`, `crates/core/src/scene_net.rs`, `crates/core/src/scene_storage.rs`, `crates/core/src/scene_thermal.rs`, `crates/core/src/scene_ups.rs`, `crates/core/src/scene_weather.rs`, `crates/core/src/scene_sky.rs`, `assets/fonts/JetBrainsMono-Bold.ttf`, `assets/fonts/OFL.txt`, unused icon SVGs
- Rename: `crates/core/src/scene_fx.rs` → `crates/core/src/scene_model.rs`
- Modify: `crates/core/src/{lib,scene,scene_rain,screens,model,event,theme,electricity}.rs`, `crates/render/src/{renderer,prims,text,assets,icons}.rs`, `crates/app/src/{runloop,calibrate}.rs`, `crates/app/examples/gifs.rs`, `crates/sources/src/fake.rs`, `crates/display/src/sim.rs`, `crates/render/tests/golden.rs`
- Test: unit tests in `status.rs`, `scene_model.rs`, `screens.rs`

**Interfaces:**
- Produces: `p2::status::{no_data_screen(role, pill) -> P2Screen, connecting_scene(now) -> Scene, NoData { Waiting, Token, Location }}`; `screens::transition_zoom(t) -> f32`; `Scene { items, zoom }` (no `ring_reveal`); `Drawable` without `Ring`, `Badge`, `Ripple`, `Dots`, `Tick`, `Text`; `SegState` gone; `Model` without `fx`, `FxRequest`, `pending_fx`, `take_fx`, `fx()`, the `smooth_*` accessors, `torrent_rings`; `Event::Boot` gone; `FakeCmd::Boot` and the `b` key gone.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/p2/status.rs` with tests only:

```rust
//! What a screen shows without live data: waiting, missing token or
//! location, and the connecting scene while the cluster link is down.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::ring::RingKind;
    use crate::p2::ui::ScreenUi;
    use crate::scene::Drawable;
    use crate::theme::p2::{TEXT2, TEXT3, WARN};
    use crate::theme::Role;

    #[test]
    fn no_data_screens_show_a_dash_and_a_pill() {
        let s = no_data_screen(Role::Cpu, NoData::Waiting);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_color), ("CPU", "—", TEXT3));
        assert!(matches!(s.ring.kind, RingKind::Empty));
        assert_eq!(s.pill, Some(("Waiting for data".into(), TEXT2)));
        assert!(s.card.is_none() && s.unit.is_none());
        assert_eq!(no_data_screen(Role::Price, NoData::Token).pill, Some(("Token needed".into(), WARN)));
        assert_eq!(no_data_screen(Role::Sun, NoData::Location).pill, Some(("Location needed".into(), WARN)));
        assert_eq!(no_data_screen(Role::Weather, NoData::Waiting).title, "WEATHER");
    }

    #[test]
    fn connecting_runs_an_arc_round_the_track() {
        let a = connecting_scene(0.0);
        let arcs: Vec<(f32, f32)> = a
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Arc { start_deg, sweep_deg, .. } => Some((*start_deg, *sweep_deg)),
                _ => None,
            })
            .collect();
        assert_eq!(arcs.len(), 2, "track and the runner");
        assert!((arcs[1].1 - 0.12 * 300.0).abs() < 1e-3, "12 % of the sweep");
        assert!(a.items.iter().any(|d| matches!(d, Drawable::CurvedText { text, .. } if text == "CONNECTING")));
        assert!(a.items.iter().any(|d| matches!(d, Drawable::Run { text, .. } if text == "Kubernetes API")));
        let b = connecting_scene(0.8);
        let half = match &b.items[2] {
            Drawable::Arc { start_deg, .. } => *start_deg,
            _ => panic!("arc"),
        };
        assert!((half - (arcs[1].0 + 150.0)).abs() < 1e-3, "half a lap in 0.8 s");
        assert_eq!(connecting_scene(1.6), connecting_scene(0.0), "one lap every 1.6 s");
        let mut ui = ScreenUi::new();
        let composed = crate::p2::screen::compose(&no_data_screen(Role::Mem, NoData::Waiting), &mut ui, 0.0);
        assert!(composed.items.iter().any(|d| matches!(d, Drawable::Run { text, .. } if text == "—")));
    }
}
```

Add to `crates/core/src/screens.rs` tests (replacing `cycle_floor_and_transform_shape`'s transform half):

```rust
    #[test]
    fn cycle_floor_and_zoom_shape() {
        let s = ScreenState::new(vec![Role::Cpu, Role::Mem], 1.0);
        assert_eq!(s.cycle_secs, MIN_CYCLE_SECS);
        assert_eq!(transition_zoom(0.0), 1.0);
        assert!(transition_zoom(0.49) < 0.1);
        assert_eq!(transition_zoom(0.5), 0.0);
        assert_eq!(transition_zoom(1.0), 1.0);
    }
```

In the `scene_model.rs` test module (the renamed `scene_fx.rs`), the surviving tests are: `connecting_when_api_down`, `no_data_until_metrics_arrive`, `cycling_screen_transitions_with_iris`, `new_roles_show_no_data_and_out_of_range_screen_connects`, `electricity_roles_go_back_to_no_data_when_the_link_drops`, `price_role_gates_on_the_prices_link`, `thermal_and_storage_gate_on_the_prometheus_link`, `unconfigured_roles_show_pin_or_key`, `electricity_roles_do_not_need_the_api_link`, `transition_into_a_cluster_role_with_api_down_shows_connecting`. Rewrite their icon checks with these helpers (replace `icons`, `has_icon`):

```rust
    fn title_of(s: &Scene) -> Option<String> {
        s.items.iter().find_map(|d| match d {
            Drawable::CurvedText { text, .. } => Some(text.clone()),
            _ => None,
        })
    }

    fn pill_of(s: &Scene) -> Option<String> {
        // the pill text is the first run with more than one character after the hero glyphs
        let mut runs = s.items.iter().filter_map(|d| match d {
            Drawable::Run { text, .. } => Some(text.clone()),
            _ => None,
        });
        runs.find(|t| t.chars().count() > 1 && t != "—")
    }

    fn connecting(s: &Scene) -> bool {
        title_of(s).as_deref() == Some("CONNECTING")
    }
```

and these assertions: `connecting_when_api_down` → `assert!(connecting(&s));`; `no_data_until_metrics_arrive` → `assert_eq!(pill_of(&s).as_deref(), Some("Waiting for data"));` for both roles; `new_roles_show_no_data_and_out_of_range_screen_connects` → `Waiting for data` and `connecting(&s)`; `electricity_roles_go_back_to_no_data_when_the_link_drops` → `pill_of(..) == Some("Waiting for data")` after the link drops and `Some("Token needed")` without a token; `price_role_gates_on_the_prices_link` → the live scene has `title_of == Some("ELECTRICITY")` and a `Run` reading `€/kWh`, after the drop `pill_of == Some("Waiting for data")`; `thermal_and_storage_gate_on_the_prometheus_link` → titles `THERMAL` / `STORAGE` then `Waiting for data`; `unconfigured_roles_show_pin_or_key` → `Location needed` for `Weather`, `Token needed` for `GhActivity`, `Waiting for data` for `Ups`, then after `set_location_present` / `set_github_token_present` both `Waiting for data`; `electricity_roles_do_not_need_the_api_link` → `!connecting(&s)` and `arcs >= 20` for power-mix, `connecting(&m.scene(1, 5.0, &mut ui))` for cpu; `transition_into_a_cluster_role_with_api_down_shows_connecting` → `connecting(&s)`. Delete `splash_swaps_icon_with_overshoot_and_ripple`, `sweep_takes_over_all_screens_and_suppresses_splash`, `two_screens_with_the_same_role_both_get_the_splash`, `boot_uses_role_colours`, `splash_kinds_have_icons`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::status screens`
Expected: compile errors (`no_data_screen`, `transition_zoom` not found).

- [ ] **Step 3: The status screens**

`crates/core/src/p2/status.rs` above its tests:

```rust
use crate::anim::Secs;
use crate::p2::layout::*;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::p2::text::{curved_title, middle_baseline, run_centred};
use crate::p2::font::FontId;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{TEXT2, TEXT3, TRACK, WARN};
use crate::theme::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoData {
    /// The source is configured but has not delivered yet, or went quiet.
    Waiting,
    /// An API token is missing from the config.
    Token,
    /// `location.lat` / `lon` are missing from the config.
    Location,
}

pub fn no_data_screen(role: Role, why: NoData) -> P2Screen {
    let mut s = P2Screen::new(role.title(), "—");
    s.hero_color = TEXT3;
    s.ring = RingSpec::empty();
    s.pill = Some(match why {
        NoData::Waiting => ("Waiting for data".into(), TEXT2),
        NoData::Token => ("Token needed".into(), WARN),
        NoData::Location => ("Location needed".into(), WARN),
    });
    s
}

/// One lap of the runner around the track.
pub const CONNECT_LAP_SECS: Secs = 1.6;
pub const CONNECT_RUNNER_FRAC: f32 = 0.12;

/// The cluster link is down: a `TRACK` ring with a short `TEXT2` arc running
/// round it, the title `CONNECTING` and the link's name in the pill.
pub fn connecting_scene(now: Secs) -> Scene {
    let mut s = Scene::new();
    let spec = RingSpec::empty();
    s.items.extend(spec.drawables());
    let phase = (now / CONNECT_LAP_SECS).rem_euclid(1.0) as f32;
    let f1 = phase + CONNECT_RUNNER_FRAC;
    s.items.extend(spec.arc(phase, f1.min(1.0), TEXT2, 1.0));
    if f1 > 1.0 {
        s.items.extend(spec.arc(0.0, f1 - 1.0, TEXT2, 1.0));
    }
    s.push(curved_title("connecting"));
    let text = "Kubernetes API";
    let w = crate::p2::font::measure(FontId::SemiBold, PILL_PX, text, 0.0) + PILL_PAD;
    s.push(Drawable::RoundRect {
        x: CX - w / 2.0,
        y: PILL_CY - PILL_H / 2.0,
        w,
        h: PILL_H,
        r: PILL_R,
        color: TRACK,
        alpha: 1.0,
    });
    s.push(run_centred(
        CX,
        middle_baseline(FontId::SemiBold, PILL_PX, PILL_CY + 0.5),
        text,
        FontId::SemiBold,
        PILL_PX,
        0.0,
        TEXT2,
        1.0,
    ));
    s
}
```

The runner is a constant 12 % arc that wraps at the end of the sweep (two arcs while it straddles the start); the pill is a plain `TRACK` round rect with the link's name, no marquee.

Add `pub mod status;` to `crates/core/src/p2/mod.rs`.

- [ ] **Step 4: Rewire `Model::scene` and drop the fx machinery**

```bash
git mv crates/core/src/scene_fx.rs crates/core/src/scene_model.rs
git rm crates/core/src/fx.rs
```

`crates/core/src/scene_model.rs` becomes:

```rust
//! `Model::scene`: which scene a physical screen shows at one instant.

use crate::anim::Secs;
use crate::p2::screen::compose;
use crate::p2::status::{connecting_scene, no_data_screen, NoData};
use crate::p2::ui::ScreenUi;
use crate::scene::{role_scene, Scene};
use crate::theme::Role;

impl crate::model::Model {
    /// A role has nothing live to show: its data never arrived, or the link that
    /// feeds it is down (stale data is not shown as live).
    fn needs_data(&self, role: Role) -> bool {
        // unchanged body
    }

    /// Cluster roles show the connecting scene while the API link is down.
    fn wants_connecting(&self, role: Role) -> bool {
        role.is_cluster() && !self.link().api
    }

    /// Scene for one screen at one instant; the only call the render loop makes.
    /// `ui` is that screen's widget state; it resets itself on a role change.
    pub fn scene(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> Scene {
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene(now),
        };
        let role = state.current();
        if let Some(tr) = state.transition(now) {
            let shown = if tr.t < 0.5 { tr.from } else { tr.to };
            let mut s = self.scene_for_role(shown, now, ui);
            s.zoom = crate::screens::transition_zoom(tr.t);
            return s;
        }
        self.scene_for_role(role, now, ui)
    }

    /// Role scene without a transition (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
        if self.wants_connecting(role) {
            return connecting_scene(now);
        }
        if !self.needs_data(role) {
            return role_scene(self, role, now, ui);
        }
        let electricity = matches!(
            role,
            Role::PowerMix | Role::Price | Role::Carbon | Role::Renewable
        );
        let why = if role.is_sky() && !self.location_present() {
            NoData::Location
        } else if (role == Role::GhActivity && !self.github_token_present())
            || (electricity && !self.token_present())
        {
            NoData::Token
        } else {
            NoData::Waiting
        };
        ui.for_role(role);
        compose(&no_data_screen(role, why), ui, now)
    }
}
```

(`wants_connecting` moved into `scene_for_role` so goldens and tests see the connecting scene through one path.) Update `crates/core/src/lib.rs`: `pub mod scene_model;` replaces `pub mod scene_fx;`, remove `pub mod fx;`.

`crates/core/src/screens.rs`: replace `transition_transform` with

```rust
/// Scale about the centre for a transition instant: the outgoing scene
/// shrinks with ease-in over the first half, the incoming grows with ease-out.
pub fn transition_zoom(t: f32) -> f32 {
    if t < 0.5 {
        1.0 - (1.0 - Easing::OutCubic.apply(1.0 - t / 0.5))
    } else {
        Easing::OutCubic.apply((t - 0.5) / 0.5)
    }
}
```

and drop the `busy` parameter: `tick(&mut self, now: Secs)` and `ready_to_transition(&self, now: Secs)` (update the tests `cycles_after_dwell_and_waits_while_busy` → rename `cycles_after_dwell`, remove the busy step; `first_transition_ticks` and the others call `tick(now)`).

`crates/core/src/model.rs`:
- remove `FxRequest`, the `fx`, `fx_state`, `hot_last`, `hot_temp_last`, `iss_swept`, `rain_prev_wet`, `seen_api_up`, `boot_pending` fields; `pending_fx`, `take_fx`, `fx`, `check_hot`; `HOT_DEBOUNCE_SECS`;
- remove the `Smooth` fields and accessors that no screen reads any more: `cpu`, `mem`, `pods`, `hot_temp`, `storage_pct`, `temp`, `eaqi`, `wind`, `ups_charge`, `ups_load`, `gh_today`, `net_rx`, `net_tx`, `net_rx_phase`, `net_tx_phase`, `net_last_tick`, `shares`, `renewable`, `fossil_free`, `carbon`, `price_needle`, `SMOOTH_SECS`, `NET_LAPS_PER_SEC`, `SHARE_EPS`, `smooth_*`, `net_phases`, `retarget_price_needle`; keep `net_fill` (the net ring uses it);
- torrents: keep `TorrentAnim` only as a plain list: replace the `torrent_anims` machinery with `torrent_mode()` = `self.all_healthy() && self.link.qbit && !self.state.torrents.is_empty()`, `torrent_badge_list()` = `self.state.torrents.iter().collect()`, `fold_torrents` = `self.state.torrents = list`; delete `torrent_rings`, `reslot_torrents`, `torrents_by_progress`, `TORRENT_GONE_PCT`, `TORRENT_SLOT_SECS`, `TORRENT_RADII` use;
- `apply`: the reaction events (`PodStarted`, `PodCrashed`, `PodGone`, `NodeReady`, `AlertChanged`, `HotTemp`, `VolumeDegraded`, `VolumeHealthy`, `UpsOnBattery`, `UpsOnline`, `TorrentAdded`, `TorrentDone`, `GithubPush`, `GithubStar`, `GithubMerge`, `GithubRelease`, `GithubRun`, `AppDegraded`, `AppHealthy`) become `=> {}` arms (the sources keep emitting them; the model has nothing to animate); `Event::Boot` is removed from `event.rs` and its arm deleted; `NodeTemps` keeps the list only; `Weather`, `AirQuality` and `Rain` drop their edge detection; `Link { K8sApi }` just sets `self.link.api = up`;
- `tick` keeps: `roll_deploy_day`, the face tick, the screen cycling (`s.tick(now)` / `s.advance(now)` + `ready_to_transition(now)`), `start_one_transition`;
- tests: delete every test that asserted `take_fx`/`pending_fx`/sweeps/boot/`torrent_rings`/`smooth_*` (`pod_events_request_fx`, `node_flip_requests_sweep`, `hot_node_debounced_five_minutes`, `node_temps_track_the_hottest_and_debounce_the_splash` → keep only its `have_temps`/`temps.len()` asserts as `node_temps_are_kept`, `volume_and_temp_events_request_splashes`, `link_up_sweep_only_after_a_previous_up`, `boot_*`, the torrent ring tests, the `smooth_*` tests, the mix leader tests, the price needle tests, the thunder/air/rain edge tests), and keep `metrics_update_state_and_smooth` as `metrics_update_state` (drop the smooth asserts). `storage_fold_computes_used_percentage` asserts `smooth_storage_pct`: assert `state().storage_used` / `storage_capacity` instead.

`crates/core/src/event.rs`: delete `Boot`. `crates/sources/src/fake.rs`: delete `FakeCmd::Boot`, its `'b'` key and arm, and `Event::Boot` uses; `crates/display/src/sim.rs`: delete the two `Key::B` entries; `crates/app/src/runloop.rs`: delete both `model.apply(Event::Boot, ..)` lines (and the `Event` import if unused).

- [ ] **Step 5: Delete the legacy drawables, scene helpers and files**

`crates/core/src/scene.rs`: keep `ClipRect`, `Drawable` with only `Clear`, `Icon`, `Arc`, `RoundRect`, `Polygon`, `Ellipse`, `Polyline`, `Run`, `CurvedText`; `Scene { items, zoom }` with `new` and `push`; and

```rust
pub fn role_scene(model: &Model, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
    ui.for_role(role);
    match role {
        Role::PowerMix => crate::p2::roles::power_mix::power_mix_scene(model, now, ui),
        Role::Face => crate::scene_face::face_scene(model, now),
        _ => {
            let screen = crate::p2::roles::screen(model, role, now).expect("every other role has a P2 builder");
            compose(&screen, ui, now)
        }
    }
}
```

Delete `SegState`, `ring_mut`, `main_icon_mut`, `lit_count`, `icon_at`, `badge`, `badge_w`, `ring`, `node_dots_badge`, `badge_text_alternate`, `seg_count`, `ring_states`, `pod_segments`, `heartbeat`, `torrent_scene`, `connecting_scene`, `no_data_scene`, `no_data_scene_with` and their tests; keep `cpu_scene_is_a_p2_tile` and `health_scene_switches_to_torrent_mode`.

```bash
git rm crates/core/src/scene_deploys.rs crates/core/src/scene_electricity.rs crates/core/src/scene_github.rs \
       crates/core/src/scene_net.rs crates/core/src/scene_storage.rs crates/core/src/scene_thermal.rs \
       crates/core/src/scene_ups.rs crates/core/src/scene_weather.rs crates/core/src/scene_sky.rs
```

and remove their `pub mod` lines. `crates/core/src/scene_rain.rs` keeps only `RAIN_SLOTS`, `SLOT_SECS`, `WET_MM`, `SOON_MINUTES`, `current_slots`, `first_wet_minutes` and their tests (delete `RAIN_SEGS`, `rain_color`, `rain_states`, `rain_scene`, `badge_says_dry_minutes_or_millimetres`, `ring_has_two_segments_per_slot_and_the_current_slot_breathes`). `crates/core/src/theme.rs`: delete `temp_color`, `carbon_color`, `GH_GREENS`, `outdoor_color`, `lerp`, `eaqi_color`, `MOON`, `OFF`, `BADGE_FILL`, `GREY`, `DIM_GREY`, `AMBER`, `VIOLET`, `BLUE`, `GREEN`, `RED` (keep `WHITE`, `BLACK`, `Color`, `Role`, `eaqi_band`, `theme::p2`), `Role::accent`, `Role::icon`, and in `theme::layout` keep only `SIZE`, `CX`, `CY`; fix the tests accordingly (`twenty_two_roles_with_unique_names` drops the icon/accent asserts, `temp_color_ramps_blue_amber_red`, `carbon_scale`, `outdoor_and_air_quality_scales` keep only the `eaqi_band` asserts). `crates/core/src/electricity.rs`: delete `PriceLevel::{word, color}` and `Source::color` (keep `Source::icon`, `index`, `from_api_key`, `price_level`, `LEVEL_EDGES`, `energy_charts_zone_for`, `PriceLevel::{name, p2_color, index}`), fix their tests.

`crates/render`:
- `renderer.rs`: delete the `reveal` logic (`FULL` stays for zoom), the `Ring`, `Badge`, `Ripple`, `Dots`, `Tick`, `Text` arms, the `SegmentCache` field, and the tests `ring_reveal_truncates_lit_segments`, `tick_spans_r0_to_r1`, `text_draws_centred_glyphs`; rewrite `zoom_scales_the_scene_about_the_centre` on a `Drawable::Arc` at radius 100 (full scale lit at `(120, 20)`, half scale at `(120, 70)`);
- `prims.rs`: delete `segment_outline`, `SegmentCache`, `circle_stroke`, `circle` (the `Dots` helper) and their tests;
- `text.rs`: delete `font`, `cache`, `glyph`, `layout`, `measure`, `draw_centered` and the tests `text_is_centred`, `measure_grows_with_text`, `zero_alpha_draws_nothing`;
- `assets.rs`: delete `FONT_BOLD` and the `font_loads` test; keep in the `icons!` list only `arrow-up` and the twelve `em-*` icons; `git rm` every other `assets/icons/*.svg`, plus `assets/fonts/JetBrainsMono-Bold.ttf` and `assets/fonts/OFL.txt`; update `scripts/fetch-assets.sh` (`ICONS="arrow-up"`, drop the JetBrains Mono block); `icons.rs` tests: replace `cpu`/`box` with `arrow-up`.

`crates/app/src/calibrate.rs`: the test pattern on the new drawables:

```rust
use rackscreen_core::p2::font::FontId;
use rackscreen_core::p2::text::{middle_baseline, run_centred};
use rackscreen_core::scene::{Drawable, Scene};
use rackscreen_core::theme::layout::{CX, CY};
use rackscreen_core::theme::p2::{AMBER, TEXT};
use rackscreen_core::theme::Role;

pub fn test_pattern(index: usize, _role: Role) -> Scene {
    let mut s = Scene::new();
    s.push(Drawable::Arc {
        cx: CX,
        cy: CY,
        r: 106.0,
        width: 9.0,
        start_deg: 0.0,
        sweep_deg: 360.0,
        color: AMBER,
        alpha: 1.0,
    });
    s.push(Drawable::Icon {
        name: "arrow-up",
        cx: CX,
        cy: CY - 14.0,
        size: 120.0,
        color: TEXT,
        alpha: 1.0,
        scale: 1.0,
        dy: 0.0,
    });
    s.push(run_centred(
        CX,
        middle_baseline(FontId::Bold, 24.0, 182.0),
        &format!("{}", index + 1),
        FontId::Bold,
        24.0,
        0.0,
        TEXT,
        1.0,
    ));
    // top-right marker: visible mirror indicator
    s.push(Drawable::Ellipse {
        cx: 186.0,
        cy: 54.0,
        rx: 7.0,
        ry: 7.0,
        color: AMBER,
        alpha: 1.0,
    });
    s
}
```

with its test checking an `Icon { name: "arrow-up" }`, a `Run` reading `3` and an `Ellipse` at `cx > 120 && cy < 120`.

`crates/app/examples/gifs.rs`: delete the `Role::Face` script's `PodCrashed`/`GithubStar` steps (replace with `(1.5, Step::Cmd(FakeCmd::NodeDown)), (5.0, Step::Cmd(FakeCmd::NodeUp))`), keep the others (they still change state the P2 tiles show).

`crates/render/tests/golden.rs`: delete `pod_crash_splash_mid_swap`, `sweep_hold`, `torrent_unwind`; in `iris_mid_transition` drop the `ring_reveal` assertion; `connecting_and_no_data` stays (the scenes changed); `git rm` the PNGs `pods_splash.png`, `sweep_hold.png`, `torrent_unwind.png`.

- [ ] **Step 6: Run everything and re-capture**

```bash
cargo build --workspace --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c
```

Expected: no errors; fix every `unused import` / dead-code warning the deletions expose (clippy with `-D warnings` is the gate). Then:

```bash
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: green; changed PNGs `connecting`, `no_data`, `no_token`, `no_location`, `iris_mid`. Open `connecting.png`: a track ring with a short grey runner, `CONNECTING` curved on top, `Kubernetes API` in a dark pill.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add -A
git commit -m "refactor: P2 status screens; remove splashes, sweeps, ring reveal, legacy drawables, JetBrains Mono and unused icons

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 16: Goldens for every role and state, bench, README gallery, gifs, version 0.7.0

**Files:**
- Rewrite: `crates/render/tests/golden.rs`; delete the stale PNGs in `crates/render/tests/goldens/` and capture the new set
- Create: `crates/render/tests/bench.rs`
- Modify: `README.md`, `Cargo.toml` (version), `.github/media/screens/*.gif` (regenerated)
- Test: `golden.rs`, `bench.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: goldens `p2_<role>.png` (normal), `p2_<role>_warn.png` (the warning or alternate state where a role has one), `p2_<role>_nodata.png`, `face_<mood>.png` for the eight moods, `connecting.png`; `bench.rs` (`--ignored`, release).

- [ ] **Step 1: Rewrite the golden suite**

Replace `crates/render/tests/golden.rs` with:

```rust
use std::path::PathBuf;

use rackscreen_core::electricity::Source;
use rackscreen_core::event::{App, AppHealth, AppSync, Event, IssPass, LinkTarget, MoonPhase, Robustness, Torrent};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::mood::Mood;
use rackscreen_core::p2::ui::ScreenUi;
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;
use tiny_skia::Pixmap;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(format!("{name}.png"))
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
    assert!(
        bad * 200 < total,
        "{name}: {bad} of {total} pixels differ (run with UPDATE_GOLDENS=1 to accept)"
    );
}

fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * p.width() + x) * 4) as usize;
    (p.data()[i], p.data()[i + 1], p.data()[i + 2])
}

fn render(m: &Model, role: Role, now: f64) -> Pixmap {
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(role, now, &mut ScreenUi::new()), &mut px);
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0), "{role:?}: the corner is black");
    px
}

const UNIX: i64 = 1_788_782_400; // 2026-09-07 12:00 UTC, Amsterdam (+2): a Monday, 14:00 local
const OFFSET: i32 = 7200;

fn link(m: &mut Model, target: LinkTarget) {
    m.apply(Event::Link { target, up: true }, 0.0);
}

/// Every source healthy and busy, the clock fixed.
fn full_model() -> Model {
    let mut m = Model::new(Thresholds::default());
    m.set_unix_now(UNIX);
    m.set_utc_offset_secs(OFFSET);
    m.set_local_slot(56);
    m.set_token_present(true);
    m.set_location_present(true);
    m.set_github_token_present(true);
    for t in [
        LinkTarget::K8sApi,
        LinkTarget::Prometheus,
        LinkTarget::Electricity,
        LinkTarget::Prices,
        LinkTarget::Weather,
        LinkTarget::Rain,
        LinkTarget::Github,
        LinkTarget::ArgoCd,
    ] {
        link(&mut m, t);
    }
    m.apply(
        Event::Metrics {
            cpu_pct: 42.0,
            mem_pct: 67.0,
            mem_used_gb: 10.7,
            mem_total_gb: 16.0,
            hot_cpu: None,
            hot_mem: None,
        },
        0.0,
    );
    m.apply(
        Event::MetricsHistory {
            cpu: (0..31).map(|i| 34.0 + 16.0 * ((i as f32) * 0.45).sin()).collect(),
            mem: (0..31).map(|i| 66.0 + 3.0 * ((i as f32) * 0.3).sin()).collect(),
            net_rx_mbps: (0..31).map(|i| 40.0 + 25.0 * ((i as f32) * 0.5).sin()).collect(),
        },
        0.0,
    );
    m.apply(
        Event::PodSnapshot {
            running: 38,
            pending: 2,
            failed: 0,
            total: 40,
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 7,
            total: 7,
            not_ready: vec![],
        },
        0.0,
    );
    m.apply(
        Event::NodeTemps(
            [("hp-elitedesk-800-g5-i7", 46.0), ("hp-elitedesk-800-g6-i5", 44.0), ("hp-elitedesk-800-g6-i7", 47.0), ("raspberrypi-5-16gb-1", 52.0), ("raspberrypi-5-8gb-1", 49.0), ("raspberrypi-5-8gb-2", 54.0), ("raspberrypi-5-8gb-3", 51.0)]
                .iter()
                .map(|(n, c)| (n.to_string(), *c))
                .collect(),
        ),
        0.0,
    );
    let tb = 1u64 << 40;
    m.apply(
        Event::Storage {
            volumes: vec![("prom".into(), Robustness::Healthy), ("qbit".into(), Robustness::Healthy), ("graf".into(), Robustness::Healthy)],
            used_bytes: tb + tb / 5,
            capacity_bytes: 3 * tb + tb / 10,
        },
        0.0,
    );
    m.apply(Event::VolumeSizes(vec![("prom".into(), tb / 2), ("qbit".into(), 2 * tb / 5), ("graf".into(), 3 * tb / 10)]), 0.0);
    for h in 0..24 {
        m.set_unix_now(UNIX - (23 - h) * 3600);
        m.apply(
            Event::Electricity {
                zone: "NL".into(),
                mix_mw: vec![(Source::Wind, 3800.0), (Source::Solar, 2400.0), (Source::Gas, 1400.0), (Source::Nuclear, 1200.0), (Source::Coal, 700.0), (Source::Biomass, 500.0)],
                renewable_pct: 55.0 + 20.0 * (((h as f32) - 6.0) / 24.0 * std::f32::consts::TAU).sin(),
                fossil_free_pct: 82.0,
                carbon_gco2: 230.0 + 80.0 * (-((h as f32 - 19.0).powi(2)) / 8.0).exp() - 90.0 * (-((h as f32 - 13.0).powi(2)) / 10.0).exp(),
                updated_at: String::new(),
            },
            0.0,
        );
    }
    m.set_unix_now(UNIX);
    let eur: Vec<f32> = (0..96)
        .map(|i| {
            let h = (i / 4) as f32;
            0.11 + 0.06 * (-((h - 19.0).powi(2)) / 5.0).exp() + 0.035 * (-((h - 8.0).powi(2)) / 3.0).exp() - 0.05 * (-((h - 13.0).powi(2)) / 7.0).exp()
        })
        .collect();
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            avg_eur_per_kwh: eur.iter().sum::<f32>() / 96.0,
            eur_per_kwh: eur,
            currency: "EUR".into(),
        },
        0.0,
    );
    m.apply(
        Event::Weather {
            temp_c: 17.0,
            code: 2,
            is_day: true,
            wind_kmh: 8.0,
            gust_kmh: 15.0,
            wind_from_deg: 225.0,
            at: String::new(),
        },
        0.0,
    );
    m.apply(
        Event::Forecast {
            hourly_temp_c: (0..12).map(|i| 13.0 + 7.0 * (((i as f32) + 4.0) / 24.0 * std::f32::consts::TAU).sin()).collect(),
            hourly_precip_mm: vec![0.0, 0.0, 0.0, 0.0, 1.1, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            day_min_c: 11.0,
            day_max_c: 21.0,
        },
        0.0,
    );
    m.apply(Event::AirQuality { eaqi: 27.0 }, 0.0);
    m.apply(
        Event::AirQualityHistory {
            hourly: (0..24).map(|i| 22.0 + 10.0 * ((i as f32) / 24.0 * std::f32::consts::TAU * 1.5).sin()).collect(),
        },
        0.0,
    );
    let mut rain = vec![0.0f32; 24];
    for (i, v) in [(1, 0.4), (2, 1.0), (3, 1.4), (4, 1.8), (5, 1.6), (6, 1.2), (7, 0.8), (8, 0.3)] {
        rain[i] = v;
    }
    m.apply(Event::Rain { from: UNIX, mm_per_h: rain }, 0.0);
    m.apply(
        Event::Sky {
            sunrise: Some(UNIX - 7 * 3600),
            sunset: Some(UNIX + 6 * 3600),
            sun_elevation_deg: 45.0,
            moon_illumination: 0.64,
            moon_waxing: true,
            moon_phase: MoonPhase::WaxingGibbous,
        },
        0.0,
    );
    m.apply(
        Event::IssPass(Some(IssPass {
            start: UNIX + 14 * 60 + 32,
            end: UNIX + 20 * 60 + 32,
            max_elevation_deg: 62.0,
            visible: true,
        })),
        0.0,
    );
    let days = (0..30).map(|i| (format!("d{i}"), [5u32, 12, 30, 0, 8, 25, 43, 28][i % 8])).collect();
    m.apply(Event::GithubActivity { days }, 0.0);
    m.apply(
        Event::Ups {
            on_battery: false,
            low_battery: false,
            charge_pct: 100.0,
            load_pct: 7.0,
            runtime_secs: 49 * 60,
        },
        0.0,
    );
    m.apply(
        Event::Network {
            rx_bps: 68e6,
            tx_bps: 6e6,
        },
        0.0,
    );
    let apps: Vec<App> = ["arr-stack", "cloudflared", "grafana", "homeassistant", "longhorn", "monitoring", "n8n", "pihole", "tailscale"]
        .iter()
        .map(|n| App {
            name: n.to_string(),
            sync: AppSync::Synced,
            health: AppHealth::Healthy,
            operating: false,
        })
        .collect();
    m.apply(Event::Apps(apps), 0.0);
    m.apply(
        Event::AppSyncHistory {
            finished_at: vec![UNIX - 6 * 3600, UNIX - 5 * 3600, UNIX - 5 * 3600 + 900, UNIX - 3 * 3600, UNIX - 1800],
        },
        0.0,
    );
    m.apply(Event::AppSynced { name: "grafana".into() }, 0.0);
    m.tick(0.0);
    m
}

#[test]
fn every_role_normal() {
    let m = full_model();
    for role in Role::ALL {
        let px = render(&m, role, 120.0);
        check(&format!("p2_{}", role.name()), &px);
    }
}

#[test]
fn warning_and_alternate_states() {
    let mut m = full_model();
    m.apply(
        Event::Metrics {
            cpu_pct: 93.0,
            mem_pct: 67.0,
            mem_used_gb: 10.7,
            mem_total_gb: 16.0,
            hot_cpu: None,
            hot_mem: None,
        },
        0.0,
    );
    m.apply(
        Event::PodSnapshot {
            running: 38,
            pending: 1,
            failed: 2,
            total: 41,
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 6,
            total: 7,
            not_ready: vec!["raspberrypi-5-8gb-3".into()],
        },
        0.0,
    );
    m.apply(Event::AlertSnapshot { firing: vec!["KubeNodeNotReady".into()] }, 0.0);
    let tb = 1u64 << 40;
    m.apply(
        Event::Storage {
            volumes: vec![("prom".into(), Robustness::Healthy), ("qbit".into(), Robustness::Degraded), ("graf".into(), Robustness::Healthy)],
            used_bytes: tb + tb / 5,
            capacity_bytes: 3 * tb + tb / 10,
        },
        0.0,
    );
    m.apply(
        Event::NodeTemps(vec![("hp-elitedesk-800-g5-i7".into(), 46.0), ("raspberrypi-5-8gb-1".into(), 72.0), ("raspberrypi-5-8gb-2".into(), 66.0)]),
        0.0,
    );
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 7.0,
            runtime_secs: 38 * 60,
        },
        0.0,
    );
    let mut apps = m.apps().apps.clone();
    apps[2].health = AppHealth::Degraded;
    apps[5].sync = AppSync::OutOfSync;
    apps[5].operating = true;
    m.apply(Event::Apps(apps), 0.0);
    m.apply(Event::Rain { from: UNIX, mm_per_h: vec![2.3; 24] }, 0.0);
    m.apply(Event::AirQuality { eaqi: 72.0 }, 0.0);
    m.tick(0.0);
    for role in [Role::Cpu, Role::Pods, Role::Health, Role::Storage, Role::Thermal, Role::Ups, Role::Deploys, Role::Rain, Role::Aqi] {
        check(&format!("p2_{}_warn", role.name()), &render(&m, role, 120.0));
    }
    // health in torrent mode
    let mut t = full_model();
    link(&mut t, LinkTarget::QBittorrent);
    t.apply(
        Event::Torrents(vec![
            Torrent {
                name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
                progress: 78.0,
                eta_secs: 900,
                speed_bps: 9_000_000,
            },
            Torrent {
                name: "debian-13.iso".into(),
                progress: 41.0,
                eta_secs: 3000,
                speed_bps: 3_000_000,
            },
        ]),
        0.0,
    );
    t.tick(0.0);
    check("p2_health_torrent", &render(&t, Role::Health, 120.0));
    // the ISS overhead and a dry nowcast
    let mut i = full_model();
    i.set_unix_now(UNIX + 16 * 60);
    check("p2_iss_pass", &render(&i, Role::Iss, 120.0));
    let mut d = full_model();
    d.apply(Event::Rain { from: UNIX, mm_per_h: vec![0.0; 24] }, 0.0);
    check("p2_rain_dry", &render(&d, Role::Rain, 120.0));
    // sunrise side of the sun role
    let mut n = full_model();
    n.set_unix_now(UNIX + 9 * 3600);
    check("p2_sun_night", &render(&n, Role::Sun, 120.0));
}

#[test]
fn no_data_token_location_and_connecting() {
    let mut m = Model::new(Thresholds::default());
    link(&mut m, LinkTarget::K8sApi);
    for role in Role::ALL {
        if role == Role::Face {
            continue;
        }
        check(&format!("p2_{}_nodata", role.name()), &render(&m, role, 0.0));
    }
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    let bare = Model::new(Thresholds::default());
    r.render(&bare.scene(0, 0.4, &mut ScreenUi::new()), &mut px);
    check("connecting", &px);
}

#[test]
fn face_moods() {
    let mut m = full_model();
    // walk the priority table from the bottom up by adding one condition at a time
    let capture = |m: &mut Model, mood: Mood| {
        let mut t = 0.0;
        while t < 2.0 {
            m.tick(t);
            t += 1.0 / 30.0;
        }
        assert_eq!(m.face_mood(), mood);
        let px = render(m, Role::Face, 2.0);
        check(&format!("face_{}", mood.name()), &px);
    };
    // a flat price day: the afternoon dip in `full_model` would otherwise read as cheap and happy
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            eur_per_kwh: vec![0.15; 96],
            avg_eur_per_kwh: 0.15,
            currency: "EUR".into(),
        },
        0.0,
    );
    capture(&mut m, Mood::Content);
    m.apply(
        Event::Weather {
            temp_c: 21.0,
            code: 0,
            is_day: true,
            wind_kmh: 1.0,
            gust_kmh: 1.0,
            wind_from_deg: 0.0,
            at: String::new(),
        },
        0.0,
    );
    capture(&mut m, Mood::Happy);
    m.set_bedtime_near(true);
    capture(&mut m, Mood::Sleepy);
    m.apply(
        Event::PodSnapshot {
            running: 38,
            pending: 0,
            failed: 1,
            total: 39,
        },
        0.0,
    );
    capture(&mut m, Mood::Worried);
    m.apply(
        Event::NodeTemps(vec![("raspberrypi-5-8gb-1".into(), 80.0)]),
        0.0,
    );
    capture(&mut m, Mood::Hot);
    m.apply(Event::AlertSnapshot { firing: vec!["Down".into()] }, 0.0);
    capture(&mut m, Mood::Angry);
    m.apply(
        Event::NodeSnapshot {
            ready: 6,
            total: 7,
            not_ready: vec!["pi".into()],
        },
        0.0,
    );
    capture(&mut m, Mood::Sad);
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 7.0,
            runtime_secs: 38 * 60,
        },
        0.0,
    );
    capture(&mut m, Mood::Scared);
}

#[test]
fn iris_mid_transition() {
    let mut m = full_model();
    m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
    let mut t = 0.0;
    while t < 3.05 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    let mut ui = ScreenUi::new();
    let s = m.scene(0, 3.25, &mut ui);
    assert!(s.zoom < 0.5 && s.zoom > 0.1, "zoom {}", s.zoom);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&s, &mut px);
    check("iris_mid", &px);
}

#[test]
fn p2_sample_screen() {
    // kept from Task 7: the composer on hand-written data
    use rackscreen_core::p2::chart::Card;
    use rackscreen_core::p2::ring::RingSpec;
    use rackscreen_core::p2::screen::{compose, P2Screen};
    use rackscreen_core::theme::p2::{BLUE, TEXT2};
    let mut s = P2Screen::new("cpu", "42");
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(0.42, BLUE);
    s.pill = Some(("7 nodes · avg".into(), TEXT2));
    s.card = Some(Card::Spark {
        series: (0..31).map(|i| 30.0 + 20.0 * ((i as f32) * 0.4).sin()).collect(),
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max: 100.0,
        now_idx: None,
        color: BLUE,
        caption: "last 60 s".into(),
    });
    let mut ui = ScreenUi::new();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&compose(&s, &mut ui, 5.0), &mut px);
    check("p2_sample", &px);
}
```

Then replace the whole golden set:

```bash
git rm -q crates/render/tests/goldens/*.png
UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden
cargo test -p rackscreen-render --test golden
ls crates/render/tests/goldens | wc -l
```

Expected: 22 `p2_<role>.png`, 9 `p2_<role>_warn.png`, `p2_health_torrent`, `p2_iss_pass`, `p2_rain_dry`, `p2_sun_night`, 21 `p2_<role>_nodata.png`, 8 `face_<mood>.png`, `connecting`, `iris_mid`, `p2_sample` (66 files). Open every `p2_*.png` once against the mockups; in particular check that no hero touches the ring, that long pills (`raspberrypi-5-8gb-3 not ready`, the torrent name) are clipped at 132 px, and that `p2_power-mix.png` has its leader centred at the top.

- [ ] **Step 2: The bench**

Create `crates/render/tests/bench.rs`:

```rust
//! Scene + render time per role. Ignored by default: run in release with
//!     cargo test --release -p rackscreen-render --test bench -- --ignored --nocapture
//! The Pi 3B+ budget is 4 ms per screen; the desktop must stay under 1 ms (4 × margin).

use std::time::Instant;

use rackscreen_core::event::{Event, LinkTarget};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::p2::ui::ScreenUi;
use rackscreen_core::theme::Role;
use rackscreen_render::frame::new_pixmap;
use rackscreen_render::renderer::Renderer;

const BUDGET_MS: f64 = 1.0;
const ITERATIONS: u32 = 200;

#[test]
#[ignore]
fn every_role_under_a_millisecond() {
    let mut m = Model::new(Thresholds::default());
    m.apply(
        Event::Link {
            target: LinkTarget::K8sApi,
            up: true,
        },
        0.0,
    );
    m.apply(
        Event::Link {
            target: LinkTarget::Prometheus,
            up: true,
        },
        0.0,
    );
    m.apply(
        Event::Metrics {
            cpu_pct: 42.0,
            mem_pct: 67.0,
            mem_used_gb: 10.7,
            mem_total_gb: 16.0,
            hot_cpu: None,
            hot_mem: None,
        },
        0.0,
    );
    m.apply(
        Event::MetricsHistory {
            cpu: (0..31).map(|i| 34.0 + 16.0 * ((i as f32) * 0.45).sin()).collect(),
            mem: vec![66.0; 31],
            net_rx_mbps: vec![40.0; 31],
        },
        0.0,
    );
    m.apply(
        Event::NodeSnapshot {
            ready: 7,
            total: 7,
            not_ready: vec![],
        },
        0.0,
    );
    m.tick(0.0);
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    let mut worst = (Role::Cpu, 0.0f64);
    for role in Role::ALL {
        let mut ui = ScreenUi::new();
        // warm the glyph caches
        r.render(&m.scene_for_role(role, 0.0, &mut ui), &mut px);
        let t0 = Instant::now();
        for i in 0..ITERATIONS {
            let now = 1.0 + i as f64 / 30.0;
            r.render(&m.scene_for_role(role, now, &mut ui), &mut px);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64;
        println!("{:<12} {ms:.3} ms", role.name());
        if ms > worst.1 {
            worst = (role, ms);
        }
    }
    assert!(worst.1 < BUDGET_MS, "{:?} takes {:.3} ms", worst.0, worst.1);
}
```

Run: `cargo test --release -p rackscreen-render --test bench -- --ignored --nocapture`
Expected: one line per role, all under 1 ms; the slowest are power-mix (capsule icons) and the face. If a role is over budget, profile it: the usual culprits are the curved title (cache the tinted glyph pixmaps, Task 2 does) and `Arc` sampling (`arc_points` every ≤ 2°; a 300° arc is 151 points, fine).

- [ ] **Step 3: README, example config, gifs, version**

`README.md`:
- the intro paragraph: replace "Each panel is one icon, one segmented ring and a small outlined badge on black, and none of it is ever static: values ease into place, the last segment breathes, icons run micro-loops. Cluster events splash on the screen that owns them, big events sweep the whole rack top to bottom, and a screen can cycle through several roles with an iris transition." with "Each panel is one tile: a curved title in the gap of a round-capped ring, one big number, a status pill and a small card with the chart that suits the data (a sparkline, bars, a stacked bar, a compass). Values glide into place and only the digits that change roll; nothing wobbles while idle. A screen can cycle through several roles with a zoom transition."
- the gallery table: keep the layout, rewrite each caption to the P2 content, for example `**\`cpu\`** — cluster CPU load with the last minute as a sparkline; amber when a node is over the threshold`, `**\`health\`** — one segment per node, alerts and not-ready counts; a live dot while the link is up`, `**\`price\`** — today's price on its five bands, the marker where it sits between today's low and high, the hours as bars`, `**\`power-mix\`** — twenty 5 % segments by source, the leader at the top, a capsule with icon and share for every source worth two segments`, `**\`face\`** — two eyes whose mood comes from the rack: scared on battery, sad with a node down, angry on an alert, hot, worried, sleepy before night, happy on a clear day or cheap power`;
- the "Screens and roles" table: rename the third column to "Tile" and describe ring, pill and card per role in one line each (take the wording from the spec's Screens table);
- "Electricity mode": replace the gauge paragraph with the band description (`Very cheap` under 60 % of the three-day mean, `Cheap` under 90 %, `Normal` up to 115 %, `Pricey` up to 140 %, `Very pricey` above; the marker shows where today's price sits between today's low and high; the card has the hours as bars);
- "Sky roles": drop the umbrella splash and the violet sweep sentences; mention the 12 h forecast sparkline, the day's low and high at the ring ends, the compass strip, the 24 h air-quality line, days to full and new moon;
- "GitHub role": replace the splash/sweep sentences with "the ring is today against your best day of the last 30, the pill the best day and your streak, the card this week as seven bars";
- "Face role": replace both paragraphs with one: "`face` shows two off-white eyes that blink, glance around and breathe. Their mood is read from the rack every frame, first match wins: scared on battery, sad with a node down, angry on a firing alert or a degraded app or volume, hot over the thresholds, worried with a failed pod, sleepy half an hour before night mode, happy on a clear day or a cheap electricity hour, content otherwise. There is nothing to configure.";
- "Configuration": delete the `face:` block from the YAML listing;
- "Development": delete `b` boot from the simulator keys; change the font line in "Licence" to "Font: Inter (OFL), `assets/fonts/Inter-LICENSE.txt`."

`Cargo.toml`: `version = "0.7.0"` under `[workspace.package]`. Then:

```bash
cargo build --workspace
cargo run -p rackscreen-app --example gifs -- .github/media/screens
ls -la .github/media/screens
```

Expected: one GIF per role plus `health-torrent.gif` and `all.gif`; every file rewritten; open `cpu.gif` (digits roll when the value changes, the sparkline slides), `power-mix.gif` and `face.gif` (node down at 1.5 s: sad; node up at 5 s: content).

- [ ] **Step 4: Final checks and commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --features sim,pi
grep -rn "JetBrains\|splash\|sweep\|ring_reveal\|face_acts\|FaceCfg\|bored_after" --include=*.rs --include=*.md --include=*.yaml --include=*.toml . | grep -v "docs/superpowers\|target/" 
```

Expected: tests green, the grep prints nothing (the design docs under `docs/superpowers` may still mention the old words).

```bash
git add -A
git commit -m "chore: P2 goldens for every role and state, render bench, README gallery and gifs, version 0.7.0

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Silke tags and pushes `v0.7.0`, as before.

---

## Self-review

**Spec coverage.** Design system: tokens (Task 4), Inter static faces embedded once in core (Task 1), tabular digits, hero/unit/title/pill sizes (Tasks 4–5), ring geometry and the four ring kinds (Task 4), the seven card kinds (Task 6). Motion: glides 0.65 s (Task 3/7), rolling digits with the stagger fix (Task 3; test `a_staggered_cell_shows_its_old_glyph_until_its_start`), sparkline scroll (Task 8 `History::scroll`), marquee (Task 3/5), segment cross-fade and 1.0 s rotation (Task 13), live indicator (Task 7). Screens: every row of the spec table has a builder (Tasks 10–13), the no-data / link-down / token rules (Task 15), power-mix capsules and inward shift (Task 13), the face shapes, mood table, tween and idle life (Task 14). Architecture: `p2/` modules, drawables (Task 2), `ScreenUi` through `Model::scene` (Task 7), removal of the act system and the legacy drawables (Tasks 14–15), splashes and sweeps removed (Task 15). Data: history buffers with range prefill, Open-Meteo hourly/daily, AQI past day, Argo syncs per hour with seeding, Longhorn per-volume size, moon and sun helpers (Tasks 8–9). Config: `face:` removed with a warning, TUI fields dropped (Task 14). Tests: the unit list in the spec maps to Tasks 3, 4, 5, 10, 13, 14; goldens and the bench to Task 16. Docs and 0.7.0: Task 16.

**Deviations recorded for the reviewer.** (1) The boot reveal in the spec's Tests section conflicts with the later "Decisions added while planning" (Boot sweep and `ring_reveal` removed); the plan follows the decisions and removes `Event::Boot`. (2) Sparkline scroll phase lives in `History` (`pushed_at`), not in `ScreenUi`: it needs the push instant, which only the buffer knows, and it must not restart on a role change. (3) `StackBar` segments are rounded pills (there is no rounded-rect clip primitive). (4) The sun ring's `Parts` show progress through the current day or night span (amber elapsed, night the rest) as the reading of "glide with time". (5) Pill texts the spec leaves open: `Dry next 12 h`, `Raining now`, `Dry for 2 h`, `no syncs yet`, `No pass in 24 h`, `Location needed`, `Polar day or night`, `ISS OVERHEAD`. (6) The reaction `Event` variants stay (the sources' contract and tests) but the model ignores them. (7) Thermal's `WARN`/`CRIT` thresholds are `hot_temp ± 5` (the config has only `hot_temp`). (8) The cpu/net sparklines push once per Prometheus poll (`prometheus.poll_secs`, default 5 s), so "last 60 s" is nominal at the spec's 2 s step.

**Placeholder scan.** No "TBD", "similar to Task N" or "add error handling"; every code step carries its code; every test step its command and expected result.

**Type consistency.** `ScreenUi::{glide, glide_with, glide_angle, roll, marquee, fade, for_role}` (Task 3) are what Tasks 5–7, 13 call; `RingSpec::{fill, parts, discrete, bands, empty, arc, point, gap_frac, drawables}` (Task 4) match Tasks 7, 10–13, 15; `Card` variants and field names (Task 6) match every builder; `P2Screen` fields (Task 7) match the builders and `status.rs`; `History::{series, prev, scroll}` and `Hourly::{values, values_since, mean, len}` (Task 8) match Tasks 10–12; `Model::{cpu_history, mem_history, net_history, carbon_hourly, renewable_hourly, deploys_log, local_hour, local_secs, local_midnight_unix_hour, mood_inputs, face_mood, face_frame}` match their uses; `scene_for_role(role, now, ui)` is the one signature used in core, render, app and the goldens.

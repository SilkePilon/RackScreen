# Readable cards, vertical slide transition, hero that always fits

Date: 2026-11-02. Status: approved in brainstorm (contrast variant B, transition "slide" turned vertical, hero fit).

## Why

On the real GC9A01 panels the card under the hero, the ring track, captions and pill fills are nearly invisible. The panel is RGB565 (5 bits red/blue) and the Waveshare gamma table in `crates/display/src/gc9a01.rs` crushes everything below roughly `#202020` to black, so the P2 surface colours (`#121214`, `#17171A`) and the caption grey (`#4A4A4F`) vanish. Separately, the hero number plus its superscript unit can run into the ring (`100 %` on `ups`), because only the number is centred and its width limit ignores the unit and the ring's curvature. And Silke wants the iris transition replaced by a vertical slide.

## 1. Contrast floor (variant B)

Token changes in `crates/core/src/theme.rs` (`theme::p2`):

| token | old | new |
|---|---|---|
| `CARD` | `#121214` | `#24242A` |
| `TRACK` | `#17171A` | `#2C2C33` |
| `TEXT3` | `#4A4A4F` | `#80808A` |
| `PILL_FILL_ALPHA` (`p2/layout.rs`) | 0.16 | 0.26 |

Other dim elements get the same floor:

- `past_alpha: 0.28` in the bars cards (`cluster.rs` pods/deploys, `energy.rs` price) becomes `0.45`.
- The paused Jellyfin ring (`color.with_alpha(0.45)`) stays; it is above the floor.
- `NIGHT` (`#2A2A33`, sun ring night part) stays; it equals the floor.
- `INK` (power-mix capsule cutout) stays deliberately near-black: it is a cut-out against the coloured ring, not a surface to read.

Rule, enforced by a test: every drawable a P2 builder emits for every role in normal, warning and no-data states, other than `Clear` and `INK` cut-outs, has an effective colour (`color × alpha` over black) whose brightest channel is at least `0x2A`. Text runs must reach at least `0x78`.

Status screens (`p2/status.rs`, no-data `—`, "connecting") use the same tokens and inherit the change.

## 2. Vertical slide replaces the iris

- `screens.rs`: `TRANSITION_SECS` 0.5 → 0.75. `transition_zoom` is removed; a `slide(t)` function returns the placement of the outgoing and incoming scene for `t ∈ 0..1`:
  - `e = ease_in_out_cubic(t)`, `s = 1 − 0.08·sin(πt)`.
  - Outgoing: `dy = −240·e`, scale `s`, alpha `1 − 0.6·e`.
  - Incoming: `dy = +240·(1 − e)`, scale `s`, alpha `0.4 + 0.6·e`.
  - Old moves up and out, new comes in from the bottom.
- `scene.rs`: the `zoom` field is removed. New types `Placement { dy: f32, scale: f32, alpha: f32 }` and `Slide { out: Scene, out_at: Placement, inc: Scene, inc_at: Placement }`; `Scene` gains `slide: Option<Box<Slide>>` (default `None`). In the steady state `Model::scene` returns a normal scene; during a transition it returns a scene with only `Clear(BLACK)` in `items` and `slide` set. The renderer draws each half with its own transform (scale about (120,120), then translate by `dy`) and alpha, and clips to the round mask.
- `crates/render/src/renderer.rs`: the zoom path becomes the placement path: draw a scene into the shared pixmap with `Transform::from_translate(0, dy)` pre-scaled about (120,120) and the alpha applied per drawable (multiply into each paint's alpha), outgoing first, incoming second. A placement fully off-screen (`|dy| ≥ 240` or `alpha ≤ 0`) is skipped.
- Serialisation is unchanged: one screen transitions at a time with the existing gap, so the SPI bus never carries four full-frame streams.
- The "connecting" logic for the incoming half (`scene_model.rs`, `transition_into_a_cluster_role_with_api_down_shows_connecting`) keeps working: each half is built with the same role/gate logic as today.
- Golden/gifs: `gifs.rs` clips show the slide; the `all.gif` cycle uses it too.

## 3. Hero fit

- The hero and its superscript unit are laid out as one group, centred on `CX`. `unit_superscript` keeps its size (`UNIT_SCALE` of the hero px) and gap.
- Fit: take the inner ring edge `R_in = RING_R − RING_W/2 = 101.5`, margin `HERO_RING_MARGIN = 10`. The group's box spans `x0..x1` horizontally and `baseline − cap_height .. baseline` vertically (unit superscript included in `x1`). The scale is the largest `k ≤ hero_scale` such that all four corners of the box lie within `R_in − margin` of the centre; it is solved by bisection on `k` (the box width and cap height scale linearly with `k`, so 12 iterations are plenty) and is a pure function `hero_fit_scale(text, unit, max_scale, baseline) -> f32` in `p2/text.rs`.
- The applied scale glides (`ui.glide(("hero-scale",0), k, now, 0.4)`) so `99 → 100` shrinks smoothly rather than jumping. The roll of the digits stays as it is.
- `unit_below` screens fit the hero alone (the unit word sits below and is narrow).
- Explicit `hero_scale` values in the role builders stay as ceilings; the `worst_case_heroes_fit_180_px` test becomes `worst_case_heroes_fit_the_ring`, asserting the four corners for each worst case, including the unit (`100 %`, `82 d`, `260 V`, `-12 °`, `1000`, `16/16`, `1:41:54`, `-0.123`).

## Testing

- Unit: `slide(t)` at 0, 0.5, 1 (outgoing at 0 / mid / gone, incoming mirrored); `hero_fit_scale` keeps `42 %` at 1.0 and shrinks `100 %` until the corners clear the margin; contrast-floor test over all 31 roles × states.
- Renderer: a two-half slide scene draws the outgoing above the incoming at `t = 0.5` (lit pixels in the top and bottom halves), nothing off-screen at `t = 0` / `1`.
- Goldens: all re-captured (token change touches every screen). Visual check of `ups` at 100 %, `uptime`, `mains`, `cpu` and a mid-slide frame.
- Bench: a mid-slide frame (two scenes) on the bench list; it must stay under 2× a steady frame.

## Out of scope

The other 18 gallery transitions, per-role transition choice, gamma register changes on the panel.

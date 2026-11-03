# Speed pipeline: 60 fps on four panels

Date: 2026-11-03. Status: approved in brainstorm. First of three specs (speed, then crossfade, then GIF role); each gets its own plan and build.

## Why

Measured on the Pi 3B+ (2026-11-03, v0.8.1, 30 fps): the single `render` thread uses 66 % of one core in steady state, about 22 ms of the 33 ms budget, for four screens. During a slide it renders two scenes plus a scaled, faded composite and runs over budget, so every screen stutters. The slide also changes every pixel, so each frame is a full 115 KB push; the two `spi1` screens ran at 16 MHz on one bus (~8 fps each while sliding).

The SPI clock was tested live with Silke watching the panels: 33, 40 and 50 MHz clean on all four, 66.7 MHz glitched. The Pi config now runs 50 MHz on all four screens (backup `/etc/rackscreen/config.yaml.hz-test.bak`). At 50 MHz a full frame takes ~18 ms.

## Goal

60 fps on any screen whose content moves, all four screens at once; render CPU at 60 fps below today's 66 % of one core at 30 fps. Pixels unchanged: every golden stays byte-identical.

## 1. Measure

- `FrameStats` per screen in `crates/app`: build ms, render ms, pack ms, push ms (from the display thread), frames rendered, frames skipped, achieved fps. Logged at `info` every 60 s as one line per screen (avg and p95).
- `crates/render/tests/bench.rs` gains the steady and mid-fade cases for the roles used on the Pi, run on the Pi with `--ignored --nocapture`. Each step below is checked against these numbers; the plan records before and after.

## 2. Skip unchanged frames

`Scene` already derives `PartialEq`. Each screen keeps its last scene; when the new scene equals it (and no night fade is running), the screen skips render, diff and push for that tick. Static screens then cost one scene build and one compare.

## 3. Base layer cache

- `Scene` gains `base: Vec<Drawable>` next to `items`. `base` holds what rarely changes: `Clear`, card, ring track, captions, static labels. `items` holds what moves: hero digits, needles, arcs that glide, marquees, pills that blink.
- Role builders put their drawables in the right list. Rule: anything whose position, colour, alpha or text depends on `now` or on a glide goes in `items`.
- The renderer keeps one cached base pixmap per screen with a hash of the `base` list (hash over the drawables' bit patterns). Same hash: copy the cached pixmap into the frame (memcpy). Different: redraw the base, store it, then draw `items`.
- Correctness guard: a test renders every role in every state both ways (base + items vs everything drawn in one pass) and asserts identical pixels. Since `items` are drawn after `base`, a role may only put a drawable in `base` if nothing in `base` draws over an earlier `items` drawable; the builders keep base drawables first.

## 4. One render worker per screen

- The main loop keeps the model, events, clock and night logic. Each tick it builds the four scenes (cheap) and hands each to that screen's persistent worker thread.
- A worker owns a `Renderer` (its own glyph caches), the screen's frame buffers and `Orient`; it renders, diffs, packs and puts the result in the screen's mailbox, then signals done.
- The main loop waits for all four workers before the next tick (a barrier per tick), so model state and timing stay deterministic. Pi 3B+ has four cores; the display threads mostly sleep in SPI writes.
- `ScreenUi` stays with the main thread: it is touched only while building scenes.

## 5. Less copying

- Render in source orientation and diff in source orientation (`cur` vs `prev`), swapping buffers instead of copying.
- Dirty rects are rotated/flipped into panel coordinates by `Orient::map_rect`.
- `pack_rgb565` moves into `rackscreen-render` as `pack_oriented(src, orient, panel_rect, brightness, out)`: it reads source pixels through the orientation map and writes big-endian RGB565 straight into a byte buffer. The full-frame `Orient::apply` copy goes away. A test compares it against the old orient-then-pack path for all eight orientations.
- The mailbox carries packed bytes (`Vec<u8>` per rect) from a small buffer pool returned by the display thread, not a cloned 230 KB `Pixmap`. Coalescing keeps working: when a frame is still pending, the newer one is packed for the union of both rect sets (the worker keeps the pending rects until the display thread takes them).

## 6. Several dirty rects

`dirty_rects(prev, next) -> SmallVec<[Rect; 4]>`: changed rows are grouped into bands, each band gets its column span; bands closer than 8 rows are merged; at most 4 rects (merge the closest pair until 4 remain); when the rects cover more than 70 % of the frame, one full rect. Each rect is a separate `set_window` + data write. A marquee at the top and a rolling digit at the bottom then push two small strips.

## 7. 60 Hz loop

- `display.fps` default 60 (config example and setup TUI default). The loop paces with an absolute deadline (`next += tick`) instead of sleeping `tick - spent`, so jitter does not accumulate; when a tick overruns it resyncs instead of bursting.
- Animation code already uses `now` in seconds, so speeds do not change with fps.

## 8. SPI clock default

`hz` default becomes 50000000 in `config.example.yaml`, the setup TUI defaults and the README. The README notes that 66.7 MHz glitched on Silke's panels and that the Pi 3B+ divides a 400 MHz core clock, so real steps are 40, 50, 66.7 MHz.

## Testing

- Unit: scene skip (same scene twice pushes nothing), base hash hit/miss, `dirty_rects` grouping/merge/full fallback, `Orient::map_rect` for all orientations, `pack_oriented` equals the old path, mailbox coalescing with rect sets, deadline pacing.
- Goldens: unchanged, byte-identical.
- Base-vs-single-pass equality over all roles × states.
- Pi: FrameStats before and after each step; target 60 fps achieved on a moving screen, render CPU below 66 % of one core.

## Out of scope

Transition change (spec 2), GIF role (spec 3), a custom RGB565 rasteriser.

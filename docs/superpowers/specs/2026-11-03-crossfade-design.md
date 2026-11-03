# Crossfade via black replaces the vertical slide

Date: 2026-11-03. Status: approved in brainstorm. Second of three specs; built after the speed pipeline.

## Why

The vertical slide (0.8.1) lags: it draws two scenes plus a scaled, faded composite per frame and changes every pixel. Silke chose a crossfade via black.

## Design

- `screens.rs`: `TRANSITION_SECS` 0.75 → 0.4. `slide(t)` is replaced by `fade(t) -> (Half, f32)`: for `t < 0.5` the outgoing scene with brightness `1 − ease_in_out_cubic(2t)`, else the incoming scene with brightness `ease_in_out_cubic(2t − 1)`. Brightness quantised to 1/64 so equal frames are skipped by the pipeline.
- `scene.rs`: `Slide` and `Placement` are removed. `Scene` gains `fade: f32` (1.0 = normal). During a transition `Model::scene` builds only one half (outgoing or incoming, with the same role/gate logic as today, outgoing widget state kept) and sets `fade`.
- Renderer: draw the scene normally (base cache included), then one darken pass when `fade < 1.0`, using a 256-entry lookup table built per fade level. The scratch pixmap and round mask are removed. Night fade uses the same darken function.
- One screen transitions at a time, as today (`one_at_a_time`).
- Gifs/README: `gifs.rs` clips and `all.gif` show the fade.

## Testing

- Unit: `fade(0)` = outgoing at 1, `fade(0.5)` = black, `fade(1)` = incoming at 1; monotonic halves.
- Renderer: a fade of 0.5 halves every channel (±1); fade 0 is black.
- `transition_into_a_cluster_role_with_api_down_shows_connecting` still passes.
- Bench: mid-fade frame costs at most a steady frame plus the darken pass.

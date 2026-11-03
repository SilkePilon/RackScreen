# Pixel shows across all four screens

Date: 2026-11-03. Status: approved in brainstorm (companion session `.superpowers/brainstorm/1130412-1793723449/`). Reference animations, the source of truth for each show's look and timing: `docs/superpowers/specs/assets/2026-11-03-pixel-shows-round1.html` and `…-round2.html` (JavaScript canvas mockups).

## What

Original pixel-art animations that play across the four round panels, which sit side by side left to right, as one wide scene. Silke picked 11:

| id | name | from mockup | length |
|---|---|---|---|
| freight | Dusk freight | round 1 `freight` | 9 s |
| deep-sea | Deep sea | round 1 `deep` | 10 s |
| paper-plane | Paper plane | round 1 `plane` | 8 s |
| storm | Thunderstorm | round 1 `storm` | 8 s |
| day | Day in a minute | round 1 `daycycle` | 12 s |
| space | Space cruise | round 1 `space` | 8 s |
| birds | Birds at sunset | round 1 `birds` | 9 s |
| fireworks | Fireworks | round 1 `fireworks` | 8 s |
| alpine | Mountains: alpine dawn | round 2 `alpine` | 10 s |
| caravan | Desert: caravan | round 2 `caravan` | 10 s |
| desert-night | Desert: night tumbleweed | round 2 `desertNight` | 9 s |

Dropped: night drive, rack runner, packet relay, cat & laser, jungle (both), cable car.

## Pixel world

- Each show draws into a world raster that is `H = 60` px high and `W = 4·60 + 3·gap` px wide. Panel `i` shows the 60×60 slice starting at `x = i·(60 + gap)`, scaled ×4 with nearest-neighbour to 240×240. The round panel hides the corners.
- The gap comes from `shows.gap_mm` (default 25, from Silke's "2–3 cm"). The GC9A01's 240 px span about 32.4 mm, so 1 logical px = 4 panel px ≈ 0.54 mm, and `gap = round(gap_mm / 0.54)` (25 mm → 46).
- Objects crossing a gap disappear behind the bezel for the right time. In the mockups, positions given as `px0(i) + offset` keep their panel. Absolute x positions are rewritten as panel + offset, so each element lands on the same screen it did in the mockup (e.g. the round-1 moon at x 178 is panel 2 + 42).
- Frames run at 12 fps ("on twos"): the frame index is `floor(t·12)`, and every show is a pure function of `(frame, t)`. Equal frames produce equal scenes, so the render workers skip repeats.
- Drawing primitives are ported from the mockup helpers: `P` (rect), `spr` (string sprite + palette), `dith` (checker dither), `disc`, `ring`, `bands` (colour bands with a one-row dither seam). `rnd(i)` is the mockup's hash: `fract(sin(i·127.1 + 311.7)·43758.5453)`, in `f64` to match.
- Palettes are the mockups' hex colours, exactly.

## Playing

- `shows: { enabled: true, every_mins: 15, gap_mm: 25, list: [<ids>] }`. The default list is all 11. `every_mins` is at least 2.
- The model starts a show when `every_mins` has passed since the last one, provided that:
  - it is not night (`NightPhase::Day`);
  - no screen is mid-transition;
  - there are at least 2 screens.

  Each start picks at random from `list`, never the same show twice in a row when the list has more than one entry.
- Sequence on all screens together:
  1. Roles fade to black over 0.2 s, using the crossfade's out-half brightness curve.
  2. The show plays for its length.
  3. The show fades to black over 0.2 s.
  4. Roles fade back in over 0.2 s.
- Role cycling pauses during a show. Each screen's dwell timer is extended by the show's total length, so no role change fires right after it.
- Screen order: the order of `screens` in the config is left to right. `shows.order: [2, 0, 1, 3]` overrides it, as the config index of each position from the left.
- With fewer than 4 screens, the world is `n` panels wide.
- Play now: the run loop checks a request file (`$XDG_STATE_HOME/rackscreen/play` of the service user, default `~/.local/state/rackscreen/play`) once a second. The file holds a show id or `random`. The loop starts that show at once, even outside the schedule, but still not at night, and deletes the file. `rackscreen play <id|random>` and the setup TUI write it.

## Rendering

- `Drawable::World { frame: Arc<WorldFrame>, x0: u32 }` asks the renderer to blit `frame` (RGBA, `W×60`) columns `x0..x0+60`, scaled ×4 nearest-neighbour, into the target. It is compared by `Arc::ptr_eq` + `x0`. `WorldFrame` is shared by all four screens' scenes for one tick.
- The model renders the world once per new 12 fps frame and caches it until the frame index changes.
- Cost: each changed show frame is a full-panel push on all four panels: 12 fps × 2 panels per bus × 18 ms ≈ 43 % of each bus at 50 MHz.

## Setup TUI

New Configure group "Shows":
- enabled on/off
- every N minutes
- gap in mm
- a checklist of the 11 shows
- "Play now" (writes the request file, then prints "requested <id>")

## Testing

- **World layout:** `W` and the panel `x0` values for gap 46 and for n = 2, 3, 4; `gap_mm` to px conversion.
- **Primitives:** pixel-exact unit tests for `P`, `spr`, `dith`, `disc` and `bands`.
- **Every show:**
  - draws without panicking for every frame of its length;
  - is deterministic (same frame, same pixels);
  - changes between frames 0 and 6;
  - a golden of panel 0..3 at one mid frame.
- **Scheduler:**
  - starts after `every_mins`, not at night, not mid-transition;
  - never repeats;
  - follows the fade sequence timing;
  - extends dwells;
  - a play-now request works and is deleted after use.
- **Config:** defaults, `every_mins` floor, unknown show id error, `order` must be a permutation.
- **Gifs:** README gets a "Shows" section with one wide 4-circle gif per show (the gifs example renders the strip).

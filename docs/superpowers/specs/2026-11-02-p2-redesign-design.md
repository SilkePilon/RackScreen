# P2 redesign: one look for every screen, each screen its own chart

## Purpose

Every role today uses the same tick ring, a centre icon and a stroked badge,
so the 22 screens look alike and say little beyond one number. This spec
replaces that look with **P2 ("Tiles")** for all existing roles: a calm,
professional look based on Wear OS Material 3 Expressive and Apple watch
complications. Each screen keeps one hero number but gets a ring and a small
chart chosen for its data. The `face` role drops its acts and becomes a pure
mood display in the same look.

New roles (media, traffic, restarts, uptime, fans, mains, load, GitHub CI,
Jellyfin) are a separate spec: `2026-11-02-new-screens-design.md`, built on
this one.

## Decisions taken during brainstorming (2026-11-02)

- First pass ("fun" styles: liquid, speedometer, aquarium, rockets) was
  rejected as unprofessional. Research (Wear OS round-screen guidance, Apple
  HIG gauges and complications, Stephen Few/NetBeez on gauges, Tufte
  data-ink and sparklines, dark-UI colour guidance, NumberFlow-style digit
  rolling) led to two systems; **P2 Tiles** won over P1 Instrument.
- Rules from the research, all binding: one hero number per screen; no
  needles or decorative gauges; a ring shows a bounded quantity, counts use
  discrete segments, trends use sparklines; label the scale (unit, ends where
  relevant); colour only carries meaning (neutral text, one muted accent per
  screen, amber/red only for warnings); calm motion (values glide, only
  changed digits roll, no constant wobble).
- Changes after review: **no icons** anywhere on data screens (power-mix is
  the one exception, see below); **no gradients** anywhere (rings use solid
  bands, sparklines are a plain line, no fades); card narrower (108 px);
  storage card = stacked bar (st1); github card = this week as 7 bars with
  weekday letters (gh2); media card = active download (md3, spec B); Jellyfin
  = jp-card on plain black (spec B, no cover art).
- **Marquee rule**: any text wider than its slot scrolls (see Motion).
- **Rolling-digit fix**: a staggered digit keeps showing its old value until
  its own roll starts (the mockups first drew the new digit early, so the
  number jumped and then rolled).
- **power-mix** keeps its original idea (ring split per source, leader at 12
  o'clock, source icons) restyled: 20 separated segments of 5 % each and
  solid **capsules** on the ring holding icon + share.
- **face**: style F1 (smooth solid off-white eyes, no dots), **no reactions
  at all**: no acts, no icon slot, no badges. Mood comes only from rack state.
- Mockups (gitignored) in `.superpowers/brainstorm/491126-1793618315/content/`:
  `pro-systems.html` (P1 vs P2), `p2-v4.html` (11 picked screens),
  `p2-rest-v2.html` (other 10 + new), `github-card.html`,
  `storage-card.html`, `power-mix-badge.html` (capsule), `face-moods-v2.html`,
  `marquee.html`, `jellyfin-final.html`. The JavaScript there is the
  reference for layout and timing; this spec is the contract.

## Design system

All coordinates are on the 240 × 240 px round screen, centre `(120, 120)`.

### Colour tokens (`core/src/theme.rs`)

| Token | Hex | Use |
|---|---|---|
| `BG` | `#000000` | background |
| `TEXT` | `#EDEDEF` | hero, primary text |
| `TEXT2` | `#8E8E93` | labels, secondary text |
| `TEXT3` | `#4A4A4F` | captions, tertiary |
| `TRACK` | `#17171A` | ring track, empty segments |
| `CARD` | `#121214` | card fill |
| `OK` | `#5BD18B` | healthy state |
| `WARN` | `#F2B84B` | warning |
| `CRIT` | `#F0605D` | critical |
| accents | blue `#6EA8FE`, violet `#A48BFA`, teal `#5CC8C3`, indigo `#8C9CFF`, cyan `#67D1EE`, amber `#F2B84B`, green `#4CC38A` | one per screen |

Pill fill is the pill's colour at 16 % alpha over black. No gradient,
blur or glow anywhere.

### Typography

- **Inter** (OFL, same licence family as the current JetBrains Mono),
  embedded as static instances **Medium 500, SemiBold 600, Bold 700**
  (fontdue cannot read variable axes). JetBrains Mono stays for the setup TUI
  only.
- **Tabular digits**: every digit is laid out in a cell as wide as Inter's
  `0` at that size, centred in the cell, so numbers never shift sideways.
- Hero: Bold 64 px (scaled by `hero_scale`, e.g. 0.82 for `14:32` or
  `0.060`, so the hero always fits within 180 px; heroes never scroll).
- Unit: either superscript right of the hero (Bold, 0.36 × hero size,
  `TEXT2`, top-aligned at hero cap height) or below the hero (Medium 10.5 px,
  `TEXT2`) when the unit is a word (`Mb/s`, `km/h`, `€/kWh`, `rpm`).
- Curved title: Bold 10 px, letter spacing 1.5 px, `TEXT2`, uppercase, set on
  radius 106 centred at 12 o'clock inside the ring gap. Titles longer than the
  gap use their first word.
- Pill text SemiBold 10.5 px; card text Medium/SemiBold 8.5 to 11 px.

### Layout

| Element | Geometry |
|---|---|
| Ring | radius 106, stroke 9, round caps, sweep 300° starting 30° clockwise of 12 o'clock (gap of 60° centred at the top for the title) |
| Hero | baseline-centred at y = 100 (y = 104 when the hero carries a unit below) |
| Pill | height 20, radius 10, centred at y = 126 (y = 134 with a unit below), width = text + 20, max text width 132 then marquee |
| Card | 108 × 48, radius 14, `CARD`, top at pill centre + 18; chart area inset 10 px horizontally, 8 px top, 14 px bottom |

### Ring kinds

| Kind | Look |
|---|---|
| `Fill { frac, color }` | `TRACK` arc over the full sweep, coloured arc from the start to `frac` |
| `Parts { [(frac, color)] }` | consecutive coloured runs with a gap of 1.3 × stroke between runs; rest `TRACK` |
| `Discrete { colors }` | `n` equal segments, gap 1.6 × stroke, each its own colour (`TRACK` = off) |
| `Bands { colors, frac }` | `n` equal solid bands, gap 1.4 × stroke; the band holding `frac` at full alpha, others at 30 %; a marker at `frac`: black disc r = 0.95 × stroke, white dot r = 0.5 × stroke, ring 2.5 px in the band colour |

### Card chart kinds

| Kind | Look |
|---|---|
| `Spark { series, min, max, now_idx?, scroll }` | 1.8 px smooth line (monotone cubic), no fill; white dot r 2.8 at the newest point (or at `now_idx` with a dashed `TEXT3` vertical); caption below in `TEXT3` 8.5 px |
| `Bars { values, now, color(v), past_alpha }` | rounded bars, the `now` bar full colour, past at `past_alpha` (default 0.28), future at 0.6; white dot under `now`; caption |
| `Week { values[7], today }` | 7 rounded bars on `TRACK` slots, today full colour, past 45 %, future empty; weekday letters below, today's white |
| `Stats { [(value, label, color)] }` | 2 or 3 columns; value SemiBold 15 px (12.5 px if any value > 5 chars, 11.5 px for 3 columns), label 8.5 px `TEXT2`; 1 px `TRACK` divider between two columns |
| `StackBar { [(frac, color)], caption }` | one 8 px rounded bar, segments separated by 1.5 px, free part `TRACK`; caption below |
| `NodeBars { values, min, max, labels, color(v), highlight }` | one bar per node on `TRACK` slots, highlighted bar full colour, others 50 %; 7.5 px labels |
| `Compass { deg, label }` | horizontal strip of ticks every 15°, letters every 45°, sliding so `deg` sits under a small cyan triangle at the centre |

## Motion

- **Data steps**: sources update in steps (polls). A new value glides from the
  shown value over **0.65 s, ease-out cubic** (`Tween`). Ring fills, bars and
  markers use the glide; nothing wobbles while idle.
- **Rolling digits** (`Roll`): when the formatted hero changes, only changed
  characters roll. Duration **0.5 s, ease-out quart**, stagger **30 ms** per
  character from the left. Direction follows the value (up when increasing:
  new digit enters from below, old leaves upward; mirrored when decreasing).
  The old glyph fades out as it moves 0.82 × size, the new fades in; both are
  clipped to the glyph cell. **Until a character's own start time it keeps
  showing its old glyph.** A length change resets without rolling.
- **Sparkline scroll**: when a new sample arrives the line shifts left one
  step over the first 30 % of the poll period (ease-in-out cubic).
- **Marquee**: text wider than its slot holds still 1.5 s, then scrolls left
  at 30 px/s with a 36 px gap before the repeat; hard clip at the slot edges
  (no fade). Changing text restarts the hold. Fitting text never moves.
- **Segment colour change** (discrete rings, power-mix): each segment
  cross-fades over 0.5 s, staggered 20 ms.
- **Ring rotation** (power-mix leader change): 1.0 s ease-out cubic.
- Live indicator (health only): a 2 px green dot under the pill whose ring
  expands to 7 px and fades every 2 s.

## Screens

Each row: curved title · hero (unit) · ring · pill · card. "Warn/crit"
gives the state colours. Data marked **new** is listed under Data changes.

| Role | Title | Hero | Ring | Pill | Card |
|---|---|---|---|---|---|
| cpu | CPU | cluster cpu % | Fill, blue; `WARN` over `hot_cpu` | "7 nodes · avg" or "High load" (warn) | Spark, last 60 s (**new** history) |
| mem | MEMORY | mem % | Fill, violet | "10.6 of 16 GB" | Spark, last 90 s, 50–80 % scale (**new**) |
| storage | STORAGE | used % | Parts per Longhorn volume (teal shades; degraded volume `CRIT`) | "1.2 of 3.1 TB" or "<vol> degraded" (crit) | StackBar per volume + "1.9 TB free" (**new** per-volume size) |
| pods | PODS | running | Parts running `OK` / pending `WARN` / failed `CRIT` | "running" | Stats pending · failed |
| health | CLUSTER | ready/total nodes (`7/7`) | Discrete, one per node, `OK`/`CRIT` | "All nodes ready" (ok) or "<node> not ready" (crit) | Stats alerts · not ready; live indicator |
| health (torrent mode) | DOWNLOADS | top torrent % | Fill, cyan | torrent name (marquee) | Stats speed · ETA |
| deploys | DEPLOYS TODAY | syncs today | Discrete, one per Argo app (ok / progressing blue / degraded crit) | "<app> · just now" (indigo, < 4 s) or "<app> · 2 m ago" | Bars per hour today (**new**) |
| weather | condition (e.g. PARTLY CLOUDY) | temp ° | Fill amber, day low → high, ends labelled | rain hint ("Rain from 18:00", cyan) | Spark next 12 h, dot at now (**new** hourly) |
| iss | ISS PASS IN | `mm:ss` countdown (`hero_scale` 0.82) | Fill cyan, time until pass within 90 min | "Visible · max 62°" | Stats start · duration |
| price | ELECTRICITY | `0.060` (0.78), unit `€/kWh` below | Bands, the 5 price bands, marker = today min→max position | band name, band colour | Bars, today per hour, coloured by band, now highlighted |
| carbon | CARBON | gCO₂/kWh, unit below | Bands green→red over 0–500 | "Cleaner than usual" / "Dirtier than usual" vs 24 h mean | Spark 24 h, `now_idx` (**new** history) |
| github | CONTRIBUTIONS | today's count | Fill green, today ÷ best day in 30 | "best day 13 · 23 streak" | Week (gh2) |
| thermal | THERMAL | hottest node ° | Fill, `OK`/`WARN` ≥ 65/`CRIT` ≥ 75 (thresholds from config) | "<node> hottest" | NodeBars per node, hottest highlighted |
| net | NETWORK | total Mb/s, unit below | Fill blue on the existing log scale | "↓ 68  ↑ 6" | Spark download 60 s (**new** history) |
| ups | UPS | charge % | Fill `OK`; `WARN` on battery | "On mains" (ok) / "On battery" (warn) | Stats runtime · load |
| wind | WIND | km/h, unit below | Fill cyan over 0–60 km/h | "Gusts 15 km/h" | Compass with "SW · 225°" |
| rain | RAIN | minutes until rain (or "Dry" via hero "—") | Fill blue, peak mm/h of the next 2 h ÷ 5 | "until rain · 1.8 mm/h" | Bars, 5-min steps, next 2 h |
| aqi | AIR QUALITY | EAQI | Bands, 6 EAQI levels | level name in its colour | Spark 24 h (**new** hourly) |
| sun | SUNSET or SUNRISE (the next one) | `16:48` (0.82) | Parts: daylight amber / night `#2A2A33`, glide with time | "in 2h 14m" | Stats sunrise/sunset · daylight |
| moon | MOON | illumination % | Fill `#D6D6E0` | phase name | Stats days to full · days to new (**new**, computed) |
| power-mix | — | — | see below | — | — |
| renewable | RENEWABLE | % | Fill green | "Fossil-free 82 %" | Spark today, `now_idx` (**new** history) |
| face | — | — | see below | — | — |

Rules for every screen:

- No data yet: hero `—` in `TEXT3`, pill "Waiting for data", empty `TRACK`
  ring, no card.
- Link down: the existing connecting scene, restyled: `TRACK` ring with a
  12 % `TEXT2` arc running round it once every 1.6 s, curved title
  "CONNECTING", pill with the link name.
- Missing token (electricity roles): pill "Token needed" (warn), no card.

### power-mix

Full circle (no title gap). 20 segments of 5 %, radius 104, stroke 9, gap
1.5 × stroke; counts per source by largest remainder; the leading source's
run centred at 12 o'clock (rotation glides 1.0 s on a leader change).
Each source with ≥ 2 segments gets a **capsule** at its run's middle angle on
the ring: height 22, width = 16 (icon) + 3 + text + 14, fully rounded, filled
with the source colour, a 3 px black outline separating it from the ring,
the `em-*` icon (14 px) and share (`38%`, Bold 10 px) in `#0B0B0C`. A capsule
that would cross the screen edge (radius 116) moves inward along its angle
until it fits. Source colours: wind cyan, solar amber, gas `#F2A65A`,
nuclear violet, coal `#9A8F85`, biomass `#7CCF8E`, hydro blue, others grey.
No centre content.

### face

Rendered as vector shapes, not dots. Two solid `TEXT` rounded rectangles,
centres `120 ∓ 48 + 10·gx`, `118 + lift + 8·gy + bob`, size `56·scale ×
68·scale·open_eff`, corner radius `min(18, h/2)`. Upper lid: a black polygon
over the eye from the inner corner at `top + k·max(tilt,0)` to the outer
corner at `top + k·max(−tilt,0)`, `k = 0.55·w·|tilt|`. Lower lid: a black
ellipse centred `bottom + 0.4·rise`, radii `0.75·w × 1.4·rise`,
`rise = 0.75·lower·68·scale` (gives `^ ^`). No ring, title, pill or card.

**Mood** is recomputed every frame from state, first match wins:

| # | Mood | When | open | tilt | lower | scale | lift |
|---|---|---|---|---|---|---|---|
| 1 | scared | UPS on battery | 1.30 | −0.40 | 0 | 0.85 | −2 |
| 2 | sad | any node not ready | 0.65 | −1.00 | 0 | 1 | 8 |
| 3 | angry | alert firing, Argo app degraded, or Longhorn volume degraded | 0.60 | 1.00 | 0.15 | 1 | 0 |
| 4 | hot | hottest node ≥ `hot_temp`, or cpu/mem over their hot thresholds | 0.70 | 0.30 | 0.20 | 1 | 3 |
| 5 | worried | pods failed > 0 or a container restarted in the last 300 s | 0.85 | −0.60 | 0.10 | 1 | 2 |
| 6 | sleepy | bedtime window (existing `night` maths) | 0.25 | 0 | 0 | 1 | 6 |
| 7 | happy | all of 1–6 false **and** one of: last CI run passed < 30 min ago, current price band ≤ cheap, weather code clear and day | 1.00 | 0 | 0.70 | 1 | −4 |
| 8 | content | otherwise | 1.00 | 0 | 0 | 1 | 0 |

A mood change tweens all fields over 0.42 s (ease-in-out cubic). Idle life
stays: blinks every 1.5–4 s (60 % depth when scared or angry, every ~5 s when
sleepy), gaze retargets (0.35–0.85 s when worried or scared, else 0.7–2 s;
sad and sleepy look down), breathing bob `1.5·sin(0.9t)`, mood wobble (angry
shake bursts, scared tremble, sleepy nod, hot slow sway). **Removed**: the
act catalogue, act player and queue, icon slot, sprites (X, star, spiral,
shades…), weather and idle habits, the 60 s reaction hold, the bored mood,
and the `face:` config block. The CI and "restart in the last 300 s" inputs come
from spec B's sources when present; without them those clauses are false.

## Architecture

### Core

- `core/src/p2/` (new module): `layout.rs` (geometry constants above),
  `ring.rs` (ring kinds → drawables), `chart.rs` (card kinds → drawables),
  `text.rs` (hero/unit/pill/curved-title helpers, marquee), `roll.rs`
  (`Roll`), `tween.rs` (`Tween`, reusing `anim::Smooth` where it fits).
- `ScreenUi` (new, `core/src/p2/ui.rs`): per-screen widget state (rolls,
  marquees, tweens, segment colour fades, sparkline scroll phase), keyed by
  `&'static str`. The runloop owns one `ScreenUi` per physical screen and
  passes `&mut ScreenUi` into `role_scene`; it resets when the screen's role
  changes. Scene building stays deterministic for a given (model, ui, now).
- Each role's scene builder moves to the P2 helpers; per-role files
  (`scene_weather.rs`, …) keep their data logic and drop tick-ring code.
- `face_acts.rs`, `face_player.rs`, `face_sprites.rs` and their tests are
  deleted; `face_expr.rs` keeps the expression table; `mood.rs` becomes the
  priority function above; `scene_face.rs` emits vector drawables.

### Drawables (core `Drawable`, rasterised in `render`)

Added:

- `Arc { cx, cy, r, width, start_deg, sweep_deg, color, alpha }` round caps.
- `RoundRect { x, y, w, h, r, color, alpha }` fill.
- `Polygon { points, color, alpha }` fill (eye lids, compass marker).
- `Ellipse { cx, cy, rx, ry, color, alpha }` fill.
- `Polyline { points, width, color, alpha }` round joins (sparklines; core
  samples the cubic into points).
- `Text` gains `font: Font` (Medium/SemiBold/Bold), `align` (left, centre,
  right), `letter_spacing`, `clip: Option<Rect>` and `dy` (roll offset).
- `CurvedText { text, radius, centre_deg, px, font, letter_spacing, color }`.
- `Image { name, cx, cy, size, color }` tinted alpha mask (power-mix icons;
  reuses the existing icon rasteriser).

Removed once no role uses them: `Ring`/`SegState` tick ring, `Badge`,
`Ripple`, `Tick`, `Dots` (the face no longer uses dots), and `Scene::zoom`/
`ring_reveal` if the boot animation is rewritten on `Arc` (it is: the boot
reveal sweeps the ring arc in over 0.8 s).

### Render

- Three embedded Inter instances; glyph cache keyed by (font, char, px
  rounded to 0.5).
- tiny-skia paths for arcs, round rects, polygons, ellipses and polylines,
  anti-aliased; clip rects through a mask.
- Performance budget: 30 fps on four screens on the Pi 3B+. The dirty-rect
  SPI path already limits transfers; scenes must stay under 4 ms render per
  screen on the Pi (bench test on the desktop with a 4× margin: < 1 ms).

### Data changes

- **History buffers** (`core/src/history.rs`): fixed-size ring buffers per
  metric with a step; `cpu` 31 × 2 s, `mem` 31 × 3 s, `net` rx 31 × 2 s,
  `carbon` 24 × 1 h, `renewable` 24 × 1 h. Prometheus-backed ones are
  prefilled at start with one `query_range` call each so they are full at
  boot; electricity ones fill from polls (empty points are skipped; the spark
  draws once it has 2 points).
- **Open-Meteo**: forecast adds `hourly=temperature_2m,precipitation` (next
  12 h) and `daily=temperature_2m_min,temperature_2m_max`; air quality adds
  `hourly=european_aqi&past_days=1` for the 24 h spark.
- **Argo**: count `AppSynced` events since local midnight per hour; on start,
  seed from each app's `status.operationState.finishedAt` when it is today.
- **Longhorn**: per-volume `longhorn_volume_actual_size_bytes` for the
  storage stacked bar.
- **Moon**: days to next full and new moon from the existing astro code.
- **Sun**: next event and daylight length from the existing sunrise/sunset.

### Config

- Removed: the whole `face:` block (`bored_after_mins`, `reaction_secs`,
  `idle_habits`, `weather_habits`) and `FaceCfg::tuning`. Old configs that
  still carry the block load with a single warning; the keys are ignored. The
  setup TUI drops the face fields from the Configure thresholds group.
- No new required fields. Thresholds keep their current names.

## Errors and edge cases

- Text that does not fit uses the marquee; heroes are sized by `hero_scale`
  so the longest expected value fits 180 px (unit test per role with worst-case
  values).
- A ring `frac` outside 0..1 clamps; `Parts` that sum over 1 are normalised.
- Fewer than 2 history points: the card shows only its caption.
- Role change on a screen resets its `ScreenUi` (no cross-role roll).
- Night mode dimming (existing) applies unchanged to all drawables.

## Tests

- Unit: `Roll` (holds the old glyph before its stagger start; rolls only
  changed characters; direction by value; length change resets), marquee
  timing (hold, speed, wrap, restart on new text), tween, ring geometry
  (sweep, gaps, caps stay inside radius 116), largest-remainder segment counts
  and leader centring for power-mix, capsule inward shift, mood priority
  table, hero fit per role.
- Render goldens re-captured for every role (normal, warning and no-data
  states) and the face moods; the boot reveal golden.
- Bench test for scene + render time per role.

## Docs and release

- README role gallery and `all.gif` regenerated with the gifs example.
- Version 0.7.0 (Silke tags and pushes, as before).

# PortWatch screen: design

Date: 2026-11-27. Status: approved in brainstorming, awaiting spec review.

A new role, `portwatch`, that shows how close the community is to running the
leaked Xbox build of GTA VI on PC, scraped from [portwatch-vi.xyz](https://portwatch-vi.xyz).
The leader's overall percentage is the main focus; around it the screen shows
the milestone reached, who leads, whether that is a private build, whether
PortWatch verified it, the leader's fps, its GPU vendors and when the site last
updated.

Mockups (open in a browser):
- `assets/2026-11-27-portwatch-mockup.html`: final layout (design C, "Milestone
  nodes on the ring", with the GPU tile). The other two rows on that page are
  the rejected icon placements.
- `assets/2026-11-27-portwatch-complete-icon.html`: Complete-marker icon
  options; option F (`check_circle`) was picked.

## 1. Data source

- `GET https://portwatch-vi.xyz/data/tracker.json`: public, no key, JSON,
  ~320 KB, served by Cloudflare with an `ETag`. The site's scout rewrites it
  about every 3 hours.
- Poll every `poll_secs` (default 900, minimum 300). Send `If-None-Match` with
  the last `ETag`; a `304` keeps the current data without downloading or
  parsing the body.
- Use the shared HTTP client (`crates/sources/src/http.rs`) and its user agent.
  No personal details in requests.

Fields read (everything else is ignored, unknown fields must not fail parsing):

| JSON | Use |
|---|---|
| `updated` (ISO 8601) | footer "Xh ago" |
| `efforts[].id`, `.name` | leader identity and pill text |
| `efforts[].repo` | private build when not a string containing `/` (null for Doomsday) |
| `efforts[].verification` | `"portwatch"` = verified; anything else (`"self-reported"`, missing) = not |
| `efforts[].gpu` | list of vendor strings or null |
| `efforts[].gta.{loads,boots,menu,world,playable}` | ladder: `true`/`false`/null or 0..1 |
| `efforts[].gta.{story,render,fps,stability}` | quality tracks; `stability` is a number 0..1 or a label |
| `efforts[].groundwork.{loader,shaders,graphics,proven}` | first 10 % (may be missing or null) |
| `efforts[].manualScore` | owner override (optional) |
| `efforts[].nuked` | taken offline: skipped |
| `efforts[].stats.stars` | tie-break |

## 2. Scoring (port of the site's `assets/js/scoring.js`)

The site computes the percentage in the browser, so RackScreen ports the
formula to Rust as a pure function in `rackscreen-core` (new module
`crates/core/src/portwatch.rs`), unit tested.

Per effort:

1. `groundwork` = 0.20·loader + 0.28·shaders + 0.32·graphics + 0.20·proven
   (each clamped 0..1, missing = 0). Score starts at `10 · groundwork`,
   reached = Groundwork.
2. Ladder in order `loads`(10) `boots`(20) `menu`(30) `world`(40)
   `playable`(50). Value: `true` = 1, `false`/null = 0, number clamped 0..1.
   A value ≥ 1 sets score to that tick and reached to that step, then
   continue. A value between 0 and 1 sets score to
   `max(score, prevTick + v · (tick − prevTick))` and stops. 0 stops.
3. Tracks: story (0.35), render (0.25), perf = fps / 30 (0.20), stability
   (0.20). Stability is a number clamped 0..1, or a label: unknown 0,
   constant 0, often 0.25, sometimes 0.55, rarely 0.8, stable 1 (anything else 0).
   Only when reached = Playable: score = `50 + quality · 50`; quality ≥ 1
   means reached = Complete.
4. `manualScore`, when a finite number, replaces the score (clamped 0..100).
5. Round to one decimal, half up (`round(score · 10) / 10`).

Overall: drop `nuked` efforts, sort by score descending, ties by
`stats.stars` descending; the first is the leader. Private builds count (the
site's headline does the same).

Check: with the 2026-11-27 16:18 UTC snapshot the port gives Belobug 91.3 and
top-3 average 90.3, matching the site's `history` entry for that day.

## 3. Model and events

- New event `Event::PortWatch(PortWatchSnapshot)` from the poller with
  everything the screen needs, already scored (the poller calls the core
  scoring function, so the render side does no JSON work):
  `score: f32`, `milestone: Milestone` (Groundwork, ExeRuns, FirstFrame,
  MainMenu, InTheWorld, Playable, Complete), `name: String`,
  `private: bool`, `verified: bool`, `fps: Option<f32>` (None when 0 or
  missing), `gpus: Vec<Gpu>` (Amd, Nvidia, Intel; unknown strings dropped,
  order kept, duplicates removed), `updated_unix: i64`.
- `Model` keeps the last snapshot. `Model::role_available(Role::PortWatch)`
  is false until the first snapshot arrives, so the role is skipped in the
  cycle like `sen` without live video (`crates/core/src/scene_model.rs`).

## 4. The screen

Role `portwatch` in `Role::ALL` (`crates/core/src/theme.rs`): title
`PORTWATCH`, top icon Material `sports_esports`. Renderer in
`crates/core/src/p2/roles/` (new `portwatch.rs`, wired in `roles/mod.rs`).

Milestone colours (all existing theme constants, no gradients; Groundwork uses `TEXT2` because `TEXT3` at the pill's 30 % fill falls under the panel contrast floor):

| Milestone | From | Colour | Marker icon |
|---|---|---|---|
| Groundwork | 0 % | `TEXT2` #8E8E93 | `construction` |
| Exe runs | 10 % | `VIOLET` #A48BFA | `terminal` |
| First frame | 20 % | `INDIGO` #8C9CFF | `image` |
| Main menu | 30 % | `BLUE` #6EA8FE | `list` |
| In the world | 40 % | `TEAL` #5CC8C3 | `public` |
| Playable | 50 % | `GREEN` #4CC38A | `directions_run` |
| Complete | 100 % | `AMBER` #F2B84B | `check_circle` |

### Ring: "route" kind

New `RingKind::Route { frac, bands: Vec<Color>, markers: Vec<(usize, &'static str)> }`
in `crates/core/src/p2/ring.rs`. Radius 104, width 9 (normal ring geometry,
`RING_START_DEG`/`RING_SWEEP_DEG`).

- Ten equal 10 % bands with the band gap of `Bands`. Band colour = colour of
  the milestone that band belongs to (bands 0–4 the five ladder colours,
  5–8 Playable green, 9 Complete amber).
- Bands fully behind `frac` at alpha 1, the band holding `frac` lit up to
  `frac` and dimmed (`DIM_BAND_ALPHA` 0.3) after it, bands ahead dimmed.
- Markers on bands 0–5 and 9, centred on the band's middle: black disc
  r 9.5 (cuts the ring), circle r 8 with a 1.6 px stroke in the band colour,
  icon 10.5 px.
  - Reached (band fully lit): circle filled with the band colour, icon `INK`
    at 0.9.
  - Current (band partly lit): black fill, full-alpha stroke, icon in band
    colour at full alpha.
  - Ahead: black fill, stroke at 0.45, icon in band colour at 0.55.

### Centre

- Hero: score, one decimal, `.0` dropped (`100`, `34`), `%` unit as on other
  screens, rolling digits as usual. Hero size 60.
- Pill (milestone colour, `PILL_FILL_ALPHA`): `verified` icon when verified,
  leader name, `lock` icon when private. Icons centred on the text's cap
  height; the group centred as one. Long names use the existing pill marquee.
- Two tiles below the pill (each 52 × 44, r 12, `CARD`, 4 px apart):
  - fps: number at 18 px (`—` in `TEXT3` when None), label `fps`.
  - gpu: vendor logos in brand colours (AMD #ED1C24, NVIDIA #76B900, Intel
    #0071C5); one logo at 1.35× its base height, two side by side scaled to
    fit 42 px, more than two shows the first two; `—` when the list is
    empty. Label `gpu`.
  - Top lines share one cap-height centre, labels share one baseline.
- Footer: `schedule` icon + relative age of `updated` ("5m ago", "2h ago",
  "3d ago") in `TEXT3`, 9 px.

Not shown (decided): project avatar/logo, stability, approach, top-3 average.

### Logos

AMD, NVIDIA and Intel shapes from Simple Icons (CC0-1.0,
`simple-icons@13.21.0`) added as `assets/icons/brand-amd.svg`,
`brand-nvidia.svg`, `brand-intel.svg` and drawn through the existing usvg icon
path (`crates/render/src/icons.rs`) tinted with the brand colour. Record the
source and licence next to the other icon licence notes. They mark which GPU
vendor an effort runs on; no other use of the marks.

## 5. Poller

`crates/sources/src/portwatch.rs`, spawned from `crates/app/src/run.rs` only
when at least one screen lists `portwatch`.

- Loop: fetch, parse with serde into structs holding only the fields above,
  score, send the snapshot, sleep `poll_secs`.
- `304`: nothing sent, sleep `poll_secs`.
- Failure (network, non-2xx, JSON error, empty efforts): warn once per
  failure streak, keep the last snapshot, retry after 1, 5, 15, then 60 min
  (reuse `gif::retry_delay` or move it to a shared helper). The footer's age
  shows staleness on screen.

## 6. Config and web UI

- `portwatch: { poll_secs: 900 }` in `crates/app/src/config.rs`
  (`#[serde(default)]`, clamp to ≥ 300), documented in `config.example.yaml`.
- `crates/web/src/roles.rs`: description "How close the GTA VI PC port is:
  the leading effort's score from PortWatch VI, milestone, fps and GPU."
- Preview `web/public/roles/portwatch.webp` via `scripts/role-previews.sh`
  from the golden.
- No Integrations page entry (nothing to configure besides the poll interval).

## 7. Tests

- Core scoring: fixture `crates/core/tests/data/portwatch-tracker.json`
  (the 2026-11-27 snapshot, trimmed to the fields read) → leader `belobug`
  91.3, top-3 90.3; unit cases for partial ladder (`loads`/`boots` true,
  `menu: 0.5` → 25 %, reached First frame), groundwork only, stability label
  vs number, `manualScore`, `nuked` skipped, tie on stars, private detection,
  GPU parsing (unknown vendor dropped, duplicates removed).
- Ring: `RingKind::Route` lit/dim/marker states per band.
- Goldens (`crates/render/tests/golden.rs`): public verified leader at
  91.3 (AMD), private leader at 78.7 (AMD + NVIDIA), early effort at 34
  (Main menu, NVIDIA + Intel, no fps), complete at 100.
- Model: role unavailable before the first snapshot, available after.
- Ignored live test: fetch the real URL, parse, score, leader present.

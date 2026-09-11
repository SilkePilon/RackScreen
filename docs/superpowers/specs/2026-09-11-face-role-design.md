# Face role: a dot-matrix pet that reacts to the cluster

Date: 2026-09-11
Status: approved by Silke in brainstorming session (style E of 6, variant E3 of 6)
Builds on: `2026-09-07-sky-github-cluster-roles-design.md` (events, fx), `2026-09-06-screen-roles-electricity-design.md` (roles, config)

## Purpose

A new `face` role: a Pwnagotchi-like face on a 24×24 amber LED matrix that is
always alive (blinks, looks around, breathes) and whose **mood is the
information**. Nothing else on the screen. A node down makes it sad, a
crashlooping pod worried, a synced deploy happy, a pushed commit excited, a UPS
on battery scared, a hot node sweaty, a long quiet evening bored, bedtime sleepy.

## Decisions taken during brainstorming

- Six face styles were mocked live (Vector slabs, glossy orbs, kawaii, cyclops,
  LED matrix, ink strokes). **LED matrix** was chosen; of its six variants
  **E3** won: 24×24 grid, round dots, amber, slab eyes, a one-dot mouth.
  Mockups: `.superpowers/brainstorm/190877-1789158051/content/face-styles.html`
  and `led-variants.html` (gitignored; the expression table below is the one
  they run).
- One role on one screen. Not two screens as two eyes, no pet mode over all
  four, no barging in on other screens for big events (sweeps already do that).
- Face only: no name, no status line, no HUD.
- The displays have no touch. "Interactive" means: reacts to the cluster, with
  lively idle behaviour. No button, no speech bubbles, no kubectl detection.
- Mood inputs: cluster health, load and heat, events, time and idle. All four.
- Reactions hold **60 s** (configurable). Idle energy: **livelier** than the
  mockup (double blinks, head tilts, more frequent glances).
- Rendering: shapes rasterised **in core** to a 24×24 brightness grid and
  emitted as 24 existing `Drawable::Dots` rows. Rejected: a new `Grid`
  drawable (touches the renderer for no need) and hand-drawn sprite frames
  (lose gaze, lid angles and the tweened transitions).

## Role

- `Role::Face`, config id `face`, accent `AMBER`, icon `smile` (new Lucide SVG
  `assets/icons/smile.svg`, registered in `render/assets.rs`).
- `is_cluster()` is true: with the Kubernetes link down the screen shows the
  existing connecting scene, as the other cluster roles do.
- `Role::ALL` grows to 22 (`Face` last). The TUI Screens editor, `Role::parse`
  and the README role table follow.

## Mood engine

New module `crates/core/src/mood.rs`, one `MoodEngine` owned by `Model`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood { Content, Happy, Excited, Worried, Sad, Angry, Hot, Scared, Bored, Sleepy }

pub struct MoodTuning { pub bored_after: Secs, pub reaction: Secs }   // from config

pub struct MoodEngine {
    reaction: Option<(Mood, Secs)>,   // mood and when it was set
    last_event: Secs,                 // resets the bored timer
    crashed_at: Option<Secs>,         // last PodCrashed
    bedtime_near: bool,               // set by the runloop each frame
    tuning: MoodTuning,
}
```

### State mood

`MoodEngine::state_mood(&Model-derived inputs, now)` is recomputed every frame.
First match wins:

| Mood | Condition |
|---|---|
| scared | `ups.on_battery` |
| sad | `nodes_not_ready > 0` |
| angry | any Argo app degraded, any Longhorn volume degraded, or `alerts > 0` |
| hot | smoothed hottest node ≥ `thresholds.hot_temp`, or `cpu_pct` ≥ `hot_cpu`, or `mem_pct` ≥ `hot_mem` |
| worried | `pods_failed > 0`, or a `PodCrashed` in the last 300 s |
| sleepy | `bedtime_near` |
| bored | `now - last_event > bored_after` |
| content | otherwise |

Only cluster state counts. Weather, prices and sky never move the face.

### Reactions

`Model::apply` calls `mood.react(mood, now)` next to the existing `fx.push`:

| Event | Reaction |
|---|---|
| `GithubPush`, `GithubStar`, `GithubMerge`, `GithubRelease`, `TorrentDone` | excited |
| `AppSynced`, `AppHealthy`, `NodeReady { ready: true }`, `UpsOnline`, `AlertChanged { firing: false }`, `VolumeHealthy`, `GithubRun { ok: true }` | happy |
| `PodCrashed`, `GithubRun { ok: false }` | worried (also sets `crashed_at` for `PodCrashed`) |
| `Boot` | sleepy, held 4 s regardless of `reaction` (waking up) |

A new reaction replaces the current one. Every event above plus `PodStarted`,
`PodGone`, `TorrentAdded`, `AppDegraded`, `NodeReady { ready: false }`,
`UpsOnBattery`, `AlertChanged { firing: true }`, `VolumeDegraded`, `HotTemp`
touches `last_event`. Snapshots and polls (`Metrics`, `PodSnapshot`,
`NodeSnapshot`, `Apps`, `Ups`, ...) do not: a bored face means nothing
*happened*, not that nothing was polled.

### Resolution

```
severe = state ∈ {scared, sad, angry, hot}
mood   = if severe { state } else if reaction unexpired { reaction } else { state }
```

Expiry: `now - set_at > tuning.reaction` (4 s for the boot yawn). A pushed
commit while a node is down does not make the face grin; a node coming back
does (happy, 60 s), then content.

`MoodEngine::current(now) -> Mood` is what the scene reads. `last_event`
starts at construction time so a fresh boot is not bored.

### Bedtime

Night mode puts the displays to sleep, so "sleepy" is shown around it: from
**30 min before `night.start`** until night begins, and for **10 min after
`night.end`**. The runloop already computes local minutes and the night window;
it calls `model.set_bedtime_near(bool)` every frame (false when night is
disabled or overridden off). `Model` gains that setter; the window maths lives
in `night.rs` as `bedtime_near(minutes, start_min, end_min) -> bool` with the
same midnight wrap as `is_night`.

## Expression

`crates/core/src/scene_face.rs`. Each mood maps to an `Expr`:

```rust
pub struct Expr {
    open: f32,     // upper lid openness, 1.0 = eye fully open, may exceed 1 (wide)
    tilt: f32,     // upper lid slant, +1 inner corners down (angry), -1 inner corners up (sad)
    lower: f32,    // lower lid rising as a convex arc, 1 = happy squint
    scale: f32,    // eye width scale (excited widens)
    mouth: f32,    // -1 frown .. +1 smile
    mouth_open: f32, // 0 line .. 1 open oval (a dark hole inside when smiling)
}
```

| Mood | open | tilt | lower | scale | mouth | mouth_open | extras |
|---|---|---|---|---|---|---|---|
| content | 1.00 | 0.00 | 0.00 | 1.00 | 0.25 | 0.00 | |
| happy | 1.00 | 0.00 | 0.60 | 1.00 | 0.90 | 0.20 | |
| excited | 1.15 | -0.10 | 0.00 | 1.08 | 1.00 | 0.80 | four sparkles orbiting the rim |
| worried | 0.85 | -0.60 | 0.10 | 1.00 | -0.40 | 0.15 | sweat drop sliding down the right temple, slow |
| sad | 0.70 | -1.00 | 0.00 | 1.00 | -0.90 | 0.00 | tear falling from the left eye |
| angry | 0.70 | 1.00 | 0.15 | 1.00 | -0.60 | 0.10 | |
| hot | 0.80 | 0.30 | 0.20 | 1.00 | -0.20 | 0.60 | sweat drop, fast, plus three rising heat waves above the eyes |
| scared | 1.25 | -0.40 | 0.00 | 1.00 | -0.30 | 0.50 | three tremor strokes at the upper left |
| bored | 0.55 | 0.00 | 0.00 | 1.00 | -0.05 | 0.00 | |
| sleepy | 0.30 | 0.00 | 0.00 | 1.00 | 0.10 | 0.10 | three z's drifting up and right, fading |

A mood change tweens every `Expr` field over **0.42 s** with
`Easing::InOutCubic` (existing `Tween`). Extras fade in with the tween.

### Idle behaviour (livelier)

All random draws come from the model's xorshift (`next_rand`), so frames are
deterministic given the seed and `now`.

- **Blink**: every 1.5 to 4 s, 150 ms, `open` multiplied by `1 - sin(π·p)`.
  20 % of blinks are double. Scared and angry blink at 60 % depth.
- **Gaze**: a target in `[-1, 1]²` every 0.7 to 2 s, eased over 0.26 s; in
  worried, excited and scared every 0.35 to 0.85 s, eased over 0.12 s. Bored
  looks hard left or right; sleepy and sad look down. Gaze shifts the eye pair
  by up to ±9 px horizontally and ±7 px vertically.
- **Head tilt**: every 10 to 25 s the eye pair rotates ±6° about the face
  centre for 1.5 s (out and back, `InOutCubic`).
- **Breathing**: the face bobs ±1.5 px at 0.9 rad/s.
- **Mood wobble**: excited bounces (`-|sin 9t|·7 px`), angry shakes in bursts
  (`sin 46t·2.2 px` while `sin 1.3t > 0.2`), scared trembles (`sin 70t·1.4 px`),
  sleepy nods (`sin 1.4t·4 px + 3`), sad and bored sag 4 and 2 px.

## Raster

Shapes are defined in the 240 px screen space, then sampled to the grid.

- **Eyes**: two rounded rectangles, centres at x = 72 and 168, y = 108, width
  62·`scale`, height 72·`open_eff`, corner radius 18, where
  `open_eff = max(0.04, open · blink)`. The upper lid is a half-plane cut with
  slope `0.55·tilt` (mirrored per eye so "inner corner" means towards the
  nose); the lower lid is a quadratic arc rising `0.75·lower·72` px at the
  centre.
- **Mouth**: centre (120, 176). Closed: a 9 px stroke from x = 100 to 140 with
  a quadratic control point 26·`mouth` px below the ends. Open
  (`mouth_open > 0.35`): a filled oval, half-height `6 + 18·mouth_open`, with a
  dark inner oval when `mouth > 0.5`.
- **Extras** as small filled shapes (drop, tear oval, z glyphs as three short
  strokes, eight-point sparkles, tremor strokes, heat-wave curves).
- **Sampling**: 24×24 cells of 10 px; each cell takes a 3×3 grid of
  sub-samples at ±2.5 px; brightness = fraction inside any shape. Lit when
  brightness > 0.2.
- **Dots**: one `Drawable::Dots` per row (`cx: 120`, `cy: 5 + 10·row`,
  `spacing: 10`, 24 colours). Lit: radius 3.6, colour
  `(255, 150 + 60·b, 40)` at alpha `0.45 + 0.55·b`. Unlit: radius 2.3, amber at
  alpha 0.08. Cells whose centre is more than 116 px from the screen centre
  are fully transparent. Radius is per drawable, so lit and unlit cells are
  split into two `Dots` per row where both occur (48 drawables worst case).
- Cost: 576 cells × 9 samples of a handful of shape tests per frame, then at
  most 576 small circle fills. Negligible on the Pi 3B+ at 30 fps.

The face scene ignores splash overlays (no ring to flash) and takes part in
sweeps and role transitions like any other role.

## Config

```yaml
face:
  bored_after_mins: 120   # no cluster event for this long -> bored
  reaction_secs: 60       # how long excited / happy / worried hold after an event
```

`FaceCfg` with `#[serde(default)]`, in `config.example.yaml` and the README.
Validation: `bored_after_mins` ≥ 5, `reaction_secs` in 1..=600. The runloop
passes both to `Model::set_mood_tuning`. The TUI Configure screen shows both
fields in the **Thresholds** group with a help line each.

## Errors and edge cases

- No data yet (`have_nodes`, `have_pods` false): the connecting scene, as for
  the other cluster roles.
- Prometheus down but Kubernetes up: hot never triggers, UPS is unknown
  (`ups.have` false means not on battery); the face still works from pods
  and nodes.
- Argo CD absent: `apps.have` false, never angry from apps.
- Night disabled: never sleepy except the 4 s boot yawn.
- Clock jump (NTP): `last_event` and reaction timestamps are monotonic `now`
  seconds, unaffected.

## Tests

- `mood.rs`: state priority (scared over sad over angry over hot over worried
  over sleepy over bored), reaction expiry at `reaction` seconds, severe state
  hides a reaction, node back after node down is happy then content, bored
  after `bored_after` with no events and not bored after a snapshot-only
  period, boot yawn lasts 4 s.
- `night.rs`: `bedtime_near` at 22:29, 22:30, 23:00, 07:05, 07:10, 07:11 for
  a 23:00 to 07:00 window; window across midnight.
- `scene_face.rs`: scene is 24 rows with 24 colours; corner cells transparent;
  sad places the top lit row lower than content; a mid-blink frame leaves at
  most two lit rows per eye; identical `now` and seed give identical scenes;
  every mood at `t = 1.0` lights between 40 and 200 cells.
- Config: defaults, `bored_after_mins: 4` and `reaction_secs: 0` rejected.
- Golden frames in `render/tests/golden.rs`: `face_content`, `face_sad`,
  `face_excited`, `face_sleepy` at a fixed `now` past the boot yawn.
- Runloop: `bedtime_near` false when night is disabled.

## Docs and release

- README: role table row (`face` | Kubernetes API + Prometheus + Argo CD | a
  dot-matrix face whose mood follows the cluster), a "Face role" section with
  the mood table, config fields.
- `crates/app/examples/gifs.rs`: a `face.gif` cycling all ten moods and the regenerated
  `all.gif`.
- Workspace version 0.6.0.

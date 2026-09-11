# Face role: a dot-matrix pet with an act for everything

Date: 2026-09-11 (revised 2026-09-12)
Status: approved by Silke in brainstorming session (style E3 of six styles and six variants; 56 acts, eyes only, tinted icons, bottom slot)
Builds on: `2026-09-07-sky-github-cluster-roles-design.md` (events, fx), `2026-09-06-screen-roles-electricity-design.md` (roles, config)

## Purpose

A new `face` role: two big eyes on a 24×24 amber LED matrix, always alive
(blinks, glances, breathes, hums, sneezes) and reacting to **every single thing**
the cluster and its surroundings do with its own short **act**: a box hits it
when a pod crashes, a rocket launches when Argo syncs, the lights flicker when
the UPS goes on battery, it puts on shades when the sun is out. Each act shows
the source's icon in a slot at the bottom of the screen in that source's colour,
and the eyes look down at it. Between acts a **mood** derived from cluster
health holds the expression.

## Decisions taken during brainstorming

- Six face styles mocked (Vector slabs, glossy orbs, kawaii, cyclops, LED
  matrix, ink strokes); LED matrix won, variant **E3**: 24×24, round dots,
  amber, slab eyes. Later refined to **eyes only, no mouth** (Vector-style: the
  two eyes are the whole face) with eyes enlarged to 7×8 dots.
- **Icons are tinted per source** (GitHub off-white, Argo orange, qBittorrent
  light blue, Kubernetes blue, UPS yellow, Longhorn green, Prometheus red,
  alerts red, sky gold, prices teal, weather light blue, snow white, wind
  grey). The face itself stays amber.
- Generic reactions were rejected: **one act per event** (56 acts, catalogue
  below), plus **weather habits** for ongoing conditions and **idle habits**
  that fire with no event at all.
- **Icon slot at the bottom** (rows 17 to 23, centred). Side icons blocked the
  face. The face rises 30 px during an act and looks down at the icon.
- **One envelope for every act** so any act can follow any other: 0.3 s in,
  body, 0.3 s out; every act starts from the held mood and ends at rest.
- One role on one screen; no barging into other screens; no name or HUD; no
  touch, buttons or speech bubbles.
- Reactions hold **60 s** (configurable). Idle energy: livelier than the first
  mockup (double blinks, head tilts, frequent glances).
- Rendering in **core** to a 24×24 brightness-and-colour grid emitted as
  existing `Drawable::Dots` rows. Rejected: a new `Grid` drawable, hand-drawn
  sprite frames.
- Mockups (gitignored): `.superpowers/brainstorm/190877-1789158051/content/`
  (`face-styles.html`, `led-variants.html`) and
  `.superpowers/brainstorm/16625-1789164266/content/acts-4.html` (the approved
  56 acts; `acts-3.html` is the same with a mouth). The act bodies in the spec
  are prose; the JavaScript in `acts-4.html` is the reference timing.

## Role

- `Role::Face`, config id `face`, accent `AMBER`, icon `smile` (new Lucide SVG
  `assets/icons/smile.svg`, registered in `render/assets.rs`).
- `is_cluster()` is true: with the Kubernetes link down the screen shows the
  existing connecting scene, as the other cluster roles do. (The `link-down`
  act plays first, then the screen switches to the connecting scene when the
  link stays down beyond the act.)
- `Role::ALL` grows to 22 (`Face` last). The TUI Screens editor, `Role::parse`
  and the README role table follow.

## Screen geometry

240 px round display, 24×24 cells of 10 px. Cell `(i, j)` has its centre at
`(10i + 5, 10j + 5)`. Cells whose centre lies more than 116 px from the screen
centre are never lit.

- **Eyes**: two rounded rectangles, centres `x = 120 ∓ 50·sep`, `y = 120 +
  lift`, width `70·scale`, height `84·scale·open_eff`, corner radius 20,
  `open_eff = max(0.04, open · (1 − blink))`. Upper lid: a half-plane cut with
  slope `0.55·tilt` mirrored per eye (positive tilt = inner corners down =
  angry). Lower lid: a quadratic arc rising `0.75·lower·84` px at the centre
  (`lower ≈ 0.7` gives the `^ ^` happy shape).
- **Eye sprites** replace the rectangles for some acts: `xeye`, `star`,
  `spiral` (two frames), `euroeye`, `shade` (7×3 bar), `sqz` (`>` and its
  mirror `<`), 7 cells wide, centred on each eye.
- **Icon slot**: 7×7 cells at columns 9 to 15, rows 17 to 23 when fully in.
- **Rest pose**: eyes centred, no offset, gaze wandering.

Face-wide transforms: offset `(off_x, off_y)`, rotation about the face centre
(head tilt), gaze `(gx, gy)` in `[−1, 1]²` shifting the eye pair by `10·gx`,
`8·gy` px, breathing bob `1.5·sin(0.9t)` px.

## Expression

```rust
pub struct Expr { open: f32, tilt: f32, lower: f32, scale: f32, sep: f32, lift: f32 }
```

| Mood | open | tilt | lower | scale | sep | lift |
|---|---|---|---|---|---|---|
| content | 1.00 | 0.00 | 0.00 | 1.00 | 1.00 | 0 |
| happy | 1.00 | 0.00 | 0.70 | 1.00 | 1.00 | −4 |
| excited | 1.20 | −0.10 | 0.10 | 1.10 | 1.02 | −6 |
| worried | 0.85 | −0.60 | 0.10 | 1.00 | 1.00 | 2 |
| sad | 0.65 | −1.00 | 0.00 | 1.00 | 1.00 | 8 |
| angry | 0.60 | 1.00 | 0.15 | 1.00 | 0.96 | 0 |
| hot | 0.70 | 0.30 | 0.20 | 1.00 | 1.00 | 3 |
| scared | 1.30 | −0.40 | 0.00 | 0.85 | 1.00 | −2 |
| sleepy | 0.25 | 0.00 | 0.00 | 1.00 | 1.00 | 6 |
| bored | 0.50 | 0.00 | 0.00 | 1.00 | 1.00 | 2 |

Transient poses used inside acts only: relief (0.12, 0, 0.40, 1, 1, 0),
grimace (0.55, 0.50, 0.35, 1, 0.97, 0), wow (1.35, 0, 0, 1.05, 1, −3),
meh (0.50, 0, 0, 1, 1, 2), focus (0.75, 0.20, 0.10, 1, 0.94, 0).

Any change of target tweens every field over **0.42 s**, `Easing::InOutCubic`.

### Idle behaviour

All randomness from the model's xorshift (`next_rand`).

- **Blink** every 1.5 to 4 s, 150 ms, `open × (1 − sin πp)`; 20 % double
  blinks; scared and angry blink at 60 % depth.
- **Gaze** retarget every 0.7 to 2 s eased over 0.26 s; every 0.35 to 0.85 s
  eased over 0.12 s in worried, excited, scared. Bored looks hard left or
  right; sleepy and sad look down.
- **Head tilt** ±6° for 1.5 s every 10 to 25 s.
- **Mood wobble**: excited bounces `−|sin 9t|·7`, angry shakes in bursts
  (`sin 46t·2.2` while `sin 1.3t > 0.2`), scared trembles `sin 70t·1.4`, sleepy
  nods `sin 1.4t·4 + 3`, sad and bored sag 4 and 2 px.
- **Idle habits** (below) every 45 to 120 s while the mood is content or bored.

## Mood engine

`crates/core/src/mood.rs`, one `MoodEngine` owned by `Model`.

**State mood**, recomputed every frame, first match wins: scared
(`ups.on_battery`), sad (`nodes_not_ready > 0`), angry (any Argo app degraded,
any Longhorn volume degraded, or `alerts > 0`), hot (smoothed hottest node ≥
`thresholds.hot_temp`, or CPU or memory over their hot thresholds), worried
(`pods_failed > 0`, or a `PodCrashed` in the last 300 s), sleepy (bedtime
near), bored (no event for `bored_after`), else content. Only cluster state
counts.

**Reaction**: each act carries a `mood` (table below). When the act ends, that
mood holds for `reaction_secs` (60 s), unless the state mood is severe
(scared, sad, angry, hot), which always wins. `MoodEngine::current(now)` is
the expression target between acts.

**Bedtime**: sleepy from 30 min before `night.start` until night begins and
for 10 min after `night.end`. The runloop calls `model.set_bedtime_near(bool)`
every frame; the window maths is `night::bedtime_near(minutes, start_min,
end_min)` with the same midnight wrap as `is_night`. Night disabled: never
sleepy.

`last_event` starts at construction so a fresh boot is not bored. Every event
that triggers an act touches it; snapshots and polls do not.

## Acts

`crates/core/src/acts.rs`: the catalogue and the player. `scene_face.rs`
turns the player's output into dots.

### Envelope

Every act has a duration `dur` (1.4 to 3.6 s) and follows the same shape:

- **In** (first 0.3 s): `env` rises 0 → 1. The face rises `30·env` px (acts
  without an icon do not rise), the icon slides up from below the bottom edge
  into the slot (`row = 17 + 8·(1 − env)`), the gaze eases to `(0, 0.9)` (down
  at the icon), the expression eases from the held mood towards content.
- **Body**: `q = 0 → 1`. The act's own choreography (sprites, offsets, eye
  overrides, matrix post-effects).
- **Out** (last 0.3 s): `env` falls 1 → 0. Icon slides down, face returns to
  centre, rotation and offsets decay to zero, eye sprites and per-eye
  overrides clear, the expression tweens into the act's `mood`.

So at `p = 0` and `p = 1` every act is at rest: no offset, no rotation, no
sprite, no icon. That is the invariant that makes acts chain.

### Queue and priority

- Acts never overlap. An event during an act is queued (FIFO, at most 3; a
  fourth is dropped). Severe acts (`ups-battery`, `node-down`, `thunder`,
  `link-down`, `pod-crashed`) go to the front of the queue.
- `pod-started` and `pod-gone` are rate-limited: at most one of each per 20 s,
  extra ones dropped, so a rolling deploy does not queue twenty boxes.
- Idle and weather habits are never queued: they only start when nothing is
  playing and the queue is empty, and any real event interrupts them at the
  next frame by jumping to their out phase (0.3 s), then the event's act
  starts. That is the one case an act is cut short; it still exits at rest.
- Boot plays first at startup, before anything else, and is not interruptible.

### Post-effects on the matrix

Acts may set per frame: `flicker` (brightness multiplier for all lit cells,
`> 1` allowed for lightning), `rim` (minimum brightness for cells more than
104 px from the centre), and a `cell(i, j)` overlay (confetti, fog band).

### Sprites

7-wide icon bitmaps in `acts.rs` (`box`, `server`, `branch`, `rocket`,
`download`, `bolt`, `batlow`, `bell`, `bell2`, `check`, `cross`, `tri`, `cpu`,
`mem`, `disk`, `flame`, `cloud`, `sun`, `moon`, `euro`, `note`, `wind`, `sat`,
`trophy`, `flake`, `fog`, `therm`, `star`, `coin`, `z`) plus the eye sprites.
A sprite is drawn as 10 px squares at a fractional cell position with a tint
and alpha; the sampler quantises it like everything else. Sprites are drawn
after the eyes and add to them.

### Catalogue

`mood` is the mood held after the act (blank = content). Weather and idle acts
carry no icon unless listed. The reference for timing is `acts-4.html`.

**Pods** (icon `box`, Kubernetes blue)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| oh-hi | `PodStarted` (rate-limited) | 1.8 | | lids lift for half the body, eyes flick down to the box and back |
| ouch | `PodCrashed` | 2.8 | worried | box drops from the top, face jolts down-left on impact, X eyes, box tumbles into the slot with a blinking cross |
| bye | `PodGone` (rate-limited) | 2.4 | | box fades out while watched, one slow blink, shrug (face lifts 6 px and drops) |

**Nodes** (icon `server`)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| lost-one | `NodeReady { ready: false }` | 3.0 | sad | cross blinks over the rack, expression sinks to sad, tear from the left eye |
| its-back | `NodeReady { ready: true }` | 2.6 | happy | check pops over the rack, `^ ^`, two hops of 10 px |

**GitHub** (icon `branch`, off-white)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| catch | `GithubPush` | 2.8 | excited | commit dot flies in from the right, eyes track it, lands between the eyes and pops into a four-dot sparkle, `^ ^` |
| starry-eyes | `GithubStar` | 2.6 | excited | star eyes, six sparkle dots twinkle around the rim |
| merge | `GithubMerge` | 2.6 | happy | two dots slide in from both sides between the eyes, fuse into one, two nods |
| party | `GithubRelease` | 3.2 | excited | confetti overlay over the whole grid, wow eyes, bounce |
| ding | `GithubRun { ok: true }` (icon `check`) | 2.0 | happy | rim flash, wink (left closed, right squints) |
| eye-roll | `GithubRun { ok: false }` (icon `cross`) | 2.6 | worried | eyes narrow and roll up to the ceiling, sink back down |
| level-up | today's contributions cross 10 (icon `trophy`, gold) | 3.0 | excited | three stars light above the eyes one by one, proud `^ ^`, face lifts |

**Argo CD** (icon `rocket`, orange)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| launch | `AppSynced` | 2.8 | happy | rocket lifts out of the slot and off the top with exhaust dots, wide eyes follow it |
| grump | `AppDegraded` (icon `tri`, red) | 2.6 | angry | triangle blinks, lids slam to angry, one hard head shake |
| wink | `AppHealthy` | 2.2 | happy | rocket pulses, right eye winks, left squints |

**qBittorrent** (icon `download`, light blue)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| incoming | `TorrentAdded` | 2.8 | | progress bar of 12 dots fills above the slot, eyes follow it, focus pose, lean in 3 px |
| got-it | `TorrentDone` | 3.2 | happy | package drops from the top and lands between the eyes, `> <` squeeze with a 4 px wiggle, then `^ ^` |

**UPS** (icon `bolt` or `batlow`, yellow)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| lights-flicker | `UpsOnBattery` | 3.0 | scared | matrix flickers off/on twice (0.08 / 1.0), bolt slides in, scared pose, tremble |
| phew | `UpsOnline` | 2.8 | | check over the bolt, relief pose, face sinks 6 px (sigh), back to content |
| on-fumes | `ups.charge_pct` crosses below 20 (re-arms above 30) | 3.0 | worried | one bar of the battery blinks red, lids droop, face sinks, one slow heavy blink |

**Alerts** (icon `bell`, red)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| alarm | `AlertChanged { firing: true }` | 3.0 | angry | bell alternates two frames (swing), rim pulses, scared pose with darting gaze |
| all-clear | `AlertChanged { firing: false }` | 2.6 | | check pops over the bell, soft rim glow, relief, one slow nod |

**Heat and load** (Prometheus red; `mem` in violet)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| too-hot | `HotTemp` (icon `flame`) | 3.0 | hot | lids sag, sweat drop slides on the right, panting bob (3 px at 12 rad/s), head shake |
| working-hard | `HotNode(Cpu)` (icon `cpu`) | 2.8 | hot | grimace, effort shake (1.5 px at 40 rad/s), sweat drop on the left |
| stuffed | `HotNode(Mem)` (icon `mem`) | 2.8 | hot | eyes squeeze shut, swell 18 % and spread 12 %, deflate with three puff dots |

**Longhorn** (icon `disk`, green)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| hmm | `VolumeDegraded` | 2.8 | angry | cross blinks over the disk, left eye narrows and right widens, face leans 4 px and tilts −7° toward it |
| disks-fine | `VolumeHealthy` | 2.2 | | check pops, `^ ^`, small nod |
| so-full | smoothed storage crosses 90 % (re-arms below 85) | 3.0 | worried | the disk icon fills bar by bar, eyes get rounder and bigger with each bar |

**Link** (icon `cloud`, Kubernetes blue)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| hello | `Link { K8sApi, up: false }` | 3.2 | worried | cloud and cross blink, eyes narrow and scan left-right-left |
| found-you | `Link { K8sApi, up: true }` | 2.2 | happy | cloud brightens, double blink, wide, `^ ^` |

**Weather habits** (Open-Meteo, replayed every 4 to 8 min while the condition
holds and once when it changes; category from the WMO code and temperature)

| sunny | code 0 or 1, daytime (icon `sun`, gold) | 3.2 | | squint, then shades slide over both eyes |
| raining | codes 51 to 67 and 80 to 82 (icon `cloud`) | 3.2 | | drops fall across the grid, eyes up, hard blinks |
| windy | gust ≥ 50 km/h (icon `wind`, grey) | 3.2 | | dot streaks blow right to left, face leans 8° into it, eyes to slits |
| snowing | codes 71 to 77, 85, 86 (icon `flake`, white) | 3.6 | | fat flakes drift, one lands on the right eye, cross-eyed look up, head shake |
| foggy | codes 45, 48 (icon `fog`) | 3.4 | | matrix dims to half, a dim band drifts down the face, squint and peer |
| meh-clouds | codes 2, 3 (icon `cloud`, grey) | 3.2 | | a cloud crosses the top, meh pose follows it, shrug |
| heatwave | `temp_c ≥ 28` (icon `therm`, red) | 3.2 | | half lids, panting bob, sweat, three heat waves rising |
| freezing | `temp_c ≤ 0` (icon `flake`) | 3.0 | | hard shiver (2.2 px at 60 rad/s), eyes squeezed, 15 % smaller and closer |

Priority when several apply: snowing, raining, windy, foggy, freezing,
heatwave, meh-clouds, sunny. Night-time clear skies get no habit.

**Sky and air events**

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| lightning | `Thunder` (icon `bolt`, light blue) | 3.0 | scared | matrix flashes ×3 twice, face jumps 10 px with huge eyes, then eyes shut and shiver |
| uh-oh-rain | `RainSoon` (icon `cloud`) | 3.0 | | first drops fall, eyes up and squint |
| cough | `AirWorse` (icon `wind`) | 2.6 | worried | two coughs: eyes shut, head jerks down 8 px, puff dots below |
| morning | local time passes `sky.sunrise` (icon `sun`) | 3.2 | | sun rises out of the slot to mid-screen, squint, `^ ^` |
| evening | local time passes `sky.sunset`, moon not full (icon `moon`) | 3.0 | | slow yawn (eyes close, head back 6°), settle half-lidded |
| awoo | sunset with `moon_illumination ≥ 0.97` (icon `moon`) | 3.2 | | head back 9°, eyes shut, three notes rise from between the eyes |
| look-up | `IssPass` (icon `sat`) | 3.4 | | a dot arcs across the top, wide eyes track it |

**Prices** (icon `euro`, teal; Energy-Charts)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| ka-ching | `PriceLevel` becomes `VeryCheap` or `Cheap` from a higher band | 2.8 | happy | euro eyes, a coin bounces in along the bottom and settles beside the icon |
| expensive | `PriceLevel` becomes `Pricey` or `VeryPricey` from a lower band | 2.8 | | a coin rolls away off the left edge, eyes follow, one lid raised |

**Boot and idle habits** (no icon, face stays centred)

| Act | Trigger | dur | mood | Body |
|---|---|---|---|---|
| wake-up | `Boot` | 3.6 | | asleep with rising z's, big yawn (eyes shut, spread 15 %, head back), two blinks, awake |
| sneeze | idle, weight 1 | 2.4 | | eyes narrow with the head tipping back, snap forward with eyes shut and four puff dots, blink |
| humming | idle, weight 3 | 3.0 | | `^ ^`, sway ±3 px and ±3°, a note glyph bobs beside the head |
| peek | idle, weight 3 | 2.6 | | face leans 26 px to one edge, eyes squished 30 % narrower, snaps back |
| stretch | idle, weight 3 | 2.6 | | eyes squeeze shut, grow 20 % and spread 15 %, face lifts 5 px, relax with a blink |
| scanning | idle, weight 3 | 3.4 | | slits, gaze sweeps left-right-left, one blue dot travels the rim with it |
| hic | idle, weight 1 | 1.4 | | face jumps 12 px, eyes pop wide and 8 % smaller, settle |
| dozing-off | only while bored, weight 4 | 3.6 | sleepy | lids droop, head nods down 12 px and 6°, snaps back with wide eyes |
| dizzy | three `PodCrashed` within 10 min (event, front of queue after `ouch`) | 3.2 | worried | spiral eyes alternate frames, face circles ±5 px and tilts ±6° |

Idle habits pick by weight; `dozing-off` only when the mood is bored,
`sneeze` and `hic` never twice in a row.

## Raster

Each frame `scene_face.rs` builds the 240 px description (two eye shapes or
sprites, act sprites, post-effects) and samples it: per cell a 3×3 grid of
sub-samples at ±3 px; brightness `b` = fraction inside any white shape; tint =
average colour of tinted sub-samples if any hit a tinted sprite.

- Lit when `b > 0.2`: radius 3.6. Colour amber `(255, 150 + 60b, 40)` or the
  tint; alpha `clamp((0.45 + 0.55b) · flicker, 0, 1)`. A 1.3 px white
  highlight at 28 % alpha is not reproduced (one colour per dot); instead the
  lit colour is the colour above at full value.
- Unlit: radius 2.3, amber at 8 % alpha.
- `rim` and `cell` overlays raise `b` before the threshold.
- Emitted as `Drawable::Dots` rows (`cx: 120`, `cy: 10j + 5`, `spacing: 10`,
  24 colours). Lit and unlit radii differ, so each row is two `Dots` (one per
  radius, the other cells transparent): 48 drawables per frame.

Cost: 5184 sub-samples against a handful of shapes plus up to 576 small
circle fills per frame. Negligible on the Pi 3B+ at 30 fps.

Splash overlays do not apply to the face (no ring). Sweeps and role
transitions apply as to any role.

## Config

```yaml
face:
  bored_after_mins: 120   # no event for this long -> bored
  reaction_secs: 60       # how long an act's mood holds afterwards
  idle_habits: true       # hum, peek, stretch, scan, sneeze, hiccup, doze
  weather_habits: true    # sunny, rain, wind, snow, fog, clouds, heat, cold (needs weather.enabled)
```

`FaceCfg` with `#[serde(default)]`, in `config.example.yaml` and the README.
Validation: `bored_after_mins ≥ 5`, `reaction_secs` in 1..=600. The runloop
passes the four values to `Model::set_face_cfg`. TUI Configure shows them in
the **Thresholds** group with help lines.

## Errors and edge cases

- No cluster data yet: connecting scene, as for the other cluster roles.
- Prometheus down: no hot, UPS, alert or storage acts; pods and nodes still
  work. Argo CD absent: no Argo acts. Weather disabled: no weather habits,
  no thunder, rain or air acts. Prices disabled: no price acts. GitHub
  disabled: no GitHub acts. Nothing configured beyond Kubernetes still gives
  pods, nodes, link, boot and idle habits.
- Edge triggers (`on-fumes`, `so-full`, `level-up`, `ka-ching`, `expensive`,
  `morning`, `evening`, `awoo`) fire once per crossing and re-arm as listed;
  `morning`/`evening` fire when `unix_now` passes the timestamp within the
  same minute, so a Pi that boots at noon does not replay sunrise.
- Night disabled: never sleepy except `wake-up`.
- Screen not currently showing the face: the act player still runs (mood,
  queue, timers advance) so switching to the face mid-act shows the act.

## Tests

- `mood.rs`: state priority, reaction expiry, severe state hides a reaction,
  bored timer ignores snapshots, boot not bored.
- `night.rs`: `bedtime_near` around a 23:00 to 07:00 window and across
  midnight.
- `acts.rs`: envelope invariant for every act in the catalogue (offset,
  rotation, sprite and icon are all at rest at `p = 0` and `p = 1`; icon row
  is 17 at `env = 1`); queue cap and severe-first ordering; pod rate limit;
  habit interruption jumps to out phase; boot uninterruptible; weather
  category mapping and priority; every edge trigger fires once and re-arms;
  idle weights never repeat sneeze or hiccup back to back.
- `scene_face.rs`: 48 drawables of 24 colours; corner cells transparent; sad
  places the top lit row lower than content; a mid-blink frame leaves at most
  two lit rows per eye; tinted sprite cells carry the tint, eye cells amber;
  identical `now` and seed give identical scenes.
- Config: defaults, `bored_after_mins: 4` and `reaction_secs: 0` rejected.
- Golden frames in `render/tests/golden.rs`: `face_content`, `face_sad`,
  `face_ouch_mid` (X eyes, box falling), `face_launch_mid` (rocket half way),
  `face_sunny_shades`, `face_sleepy`.
- Runloop: `bedtime_near` false when night is disabled.

## Docs and release

- README: role table row (`face` | everything | two eyes on a dot matrix that
  act out every event and hold the cluster's mood), a "Face role" section with
  the mood table, the act catalogue in short, config fields.
- `crates/app/examples/gifs.rs`: `face.gif` cycling boot, content, one act per
  source and the weather habits; regenerated `all.gif`.
- Workspace version 0.6.0.

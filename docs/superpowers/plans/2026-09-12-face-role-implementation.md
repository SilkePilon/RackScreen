# Face Role Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `face` role: two big eyes on a 24×24 amber dot matrix that play one short act per cluster event (with the source's tinted icon in a bottom slot), weather habits, idle habits, and hold a cluster-derived mood in between.

**Architecture:** Everything lives in `rackscreen-core` (no I/O): `mood.rs` derives a mood from cluster state, `face_expr.rs` holds the expression table and idle behaviour, `face_sprites.rs` the pixel bitmaps, `face_acts.rs` the 56 act bodies plus the shared envelope, `face_player.rs` the queue, edge triggers, habit timers and per-frame output, and `scene_face.rs` samples that output into `Drawable::Dots` rows. `Model` owns one `FacePlayer`, feeds it the existing `FxRequest` stream (the same one the splashes and sweeps use) and per-frame inputs. The app crate adds `face:` config, the runloop passes tuning and the bedtime flag, the setup TUI edits the four fields.

**Tech Stack:** Rust 2021 workspace (`rackscreen-core`, `rackscreen-render`, `rackscreen-app`, `rackscreen-setup`), `serde`/`serde_yaml` config, `tiny-skia` renderer with golden PNG tests, `gif` example for README media. No new dependencies.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-09-11-face-role-design.md`. Reference timing for every act: `.superpowers/brainstorm/16625-1789164266/content/acts-4.html` (gitignored; the bodies below are its port).
- `rackscreen-core` stays dependency-free (its `Cargo.toml` has no `[dependencies]` entries). No `chrono`, no `rand`.
- Grid is 24×24 cells of 10 px on the 240 px screen; cell centre `(10i + 5, 10j + 5)`; cells farther than 116 px from `(120, 120)` are never lit.
- Icon slot: 7×7 cells at columns 9..=15, rows 17..=23 (`SLOT_X = 9.0`, `SLOT_Y = 17.0`). Face rises `30` px during acts with an icon. Envelope in/out `0.3` s.
- Expression tween `0.42` s `Easing::InOutCubic`. Reaction hold default `60` s. Bored default `120` min.
- Amber lit dot: `(255, 150 + 60·b, 40)`, alpha `clamp((0.45 + 0.55·b)·flicker, 0, 1)`, radius `3.6`. Unlit: amber `0xffb020` at alpha `0.08`, radius `2.3`. Lit threshold `b > 0.2`. Tints: GitHub `0xe6e6ff`, Argo `0xff9a3c`, qBittorrent `0x5ac8fa`, Kubernetes `0x4c8dff`, UPS `0xffe66d`, Longhorn `0x8be9a5`, Prometheus `0xff6b57`, alert `0xff5a5a`, ISS `0xc9d6ff`, sky `0xffd36b`, price `0x7ee0c3`, memory `0xb48cff`, weather `0x9ad4ff`, snow `0xe8f4ff`, wind `0xb8c4cc`.
- Role id `face`, accent `AMBER`, icon `smile`, `Role::ALL` length 22, `is_cluster()` true.
- Config keys: `face.bored_after_mins` (default 120, ≥ 5), `face.reaction_secs` (default 60, 1..=600), `face.idle_habits` (true), `face.weather_habits` (true).
- Workspace version becomes `0.6.0`.
- Run tests with `cargo test -p rackscreen-core` (fast) and `cargo test --workspace` before each commit; `cargo clippy --workspace --all-targets -- -D warnings` must stay clean; `cargo fmt --all` before committing.
- Commit messages: `type(scope): summary` as in `git log`, ending with the `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` line.

## File structure

| File | Responsibility |
|---|---|
| `crates/core/src/mood.rs` (new) | `Mood` enum, `MoodTuning`, `MoodInputs`, `MoodEngine` (state mood, reaction hold, bored timer, bedtime) |
| `crates/core/src/night.rs` (modify) | `bedtime_near(minutes, start_min, end_min)` |
| `crates/core/src/face_expr.rs` (new) | `Expr` (six knobs), mood table, transient poses, `ExprTween`, `Idle` (blink, gaze, head tilt, wobble), helpers, xorshift |
| `crates/core/src/face_sprites.rs` (new) | `Sprite` bitmaps: icons, eye sprites, disk fill levels, tint constants |
| `crates/core/src/face_acts.rs` (new) | `ActKind` (56), `ActDef`, `Frame` API, `envelope`, `run_act`, all act bodies, `weather_cat`, `IDLE_HABITS` |
| `crates/core/src/face_player.rs` (new) | `FacePlayer`: queue, priorities, rate limits, fx mapping, edge triggers, weather/idle habit timers, rest expression tween, `FaceFrame` output |
| `crates/core/src/scene_face.rs` (new) | `face_scene(model, now)` and `render_frame(&FaceFrame, now) -> Scene`: shape tests, 3×3 sampling, `Dots` rows |
| `crates/core/src/theme.rs` (modify) | `Role::Face` |
| `crates/core/src/model.rs` (modify) | owns `FacePlayer`, feeds fx and inputs, setters |
| `crates/core/src/scene.rs`, `scene_fx.rs` (modify) | dispatch and `needs_data` arms |
| `crates/core/src/lib.rs` (modify) | module list |
| `crates/render/src/assets.rs`, `assets/icons/smile.svg`, `scripts/fetch-assets.sh` (modify/new) | role icon |
| `crates/render/tests/golden.rs` (modify) | six face goldens |
| `crates/app/src/config.rs`, `config.example.yaml`, `crates/app/src/run.rs`, `crates/app/src/runloop.rs` (modify) | `FaceCfg`, validation, tuning and bedtime plumbing |
| `crates/setup/src/screens/configure.rs` (modify) | four fields in the Thresholds group |
| `README.md`, `crates/app/examples/gifs.rs`, `Cargo.toml` (modify) | docs, `face.gif`, version |

Shared helper signatures used across tasks (all `pub` in `face_expr.rs`):

```rust
pub fn lerp(a: f32, b: f32, k: f32) -> f32
pub fn seg(p: f32, a: f32, b: f32) -> f32      // clamp((p - a) / (b - a), 0, 1)
pub fn inseg(p: f32, a: f32, b: f32) -> bool   // a <= p < b
pub fn bump(p: f32, a: f32, b: f32) -> f32     // sin(seg · π): 0..1..0
pub fn ease(k: f32) -> f32                     // Easing::InOutCubic
pub fn out_back(k: f32) -> f32                 // Easing::Spring
pub fn xorshift(state: &mut u64) -> u64
pub fn unit(state: &mut u64) -> f32            // 0..1
```

---

### Task 1: `Role::Face` and the role icon

**Files:**
- Modify: `crates/core/src/theme.rs:124-290` (enum, `ALL`, `name`, `is_cluster`, `accent`, `icon`)
- Modify: `crates/core/src/theme.rs:370-412` (tests `cluster_roles_are_the_kubernetes_ones`, `twenty_one_roles_with_unique_names_and_icons`)
- Create: `assets/icons/smile.svg`
- Modify: `crates/render/src/assets.rs` (icon list), `scripts/fetch-assets.sh:5` (`ICONS=` list)
- Create: `crates/core/src/scene_face.rs` (temporary stub; Task 10 replaces it)
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs:346-470` (`role_scene` arm), `crates/core/src/scene_fx.rs:154-176` (`needs_data` arm)

**Interfaces:**
- Produces: `Role::Face` with `name() == "face"`, `icon() == "smile"`, `accent() == AMBER`, `is_cluster() == true`, `Role::ALL.len() == 22`; `rackscreen_core::scene_face::face_scene(model: &Model, now: Secs) -> Scene`.

- [ ] **Step 1: Extend the role test**

In `crates/core/src/theme.rs` rename and extend the test at line 390:

```rust
    #[test]
    fn twenty_two_roles_with_unique_names_and_icons() {
        assert_eq!(Role::ALL.len(), 22);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 22);
        assert_eq!(Role::parse("face"), Some(Role::Face));
        assert_eq!(Role::Face.icon(), "smile");
        assert_eq!(Role::Face.accent(), AMBER);
        assert!(Role::Face.is_cluster());
        assert!(!Role::Face.is_sky());
        assert_eq!(Role::ALL[21], Role::Face);
```

Keep the rest of the existing test body (the icon uniqueness assertions) as it is. Also update `cluster_roles_are_the_kubernetes_ones` (line 370) so its expected list ends with `Role::Deploys, Role::Face`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core theme::tests -- --nocapture`
Expected: compile error `no variant named Face`.

- [ ] **Step 3: Add the variant**

In `crates/core/src/theme.rs`:

```rust
pub enum Role {
    // ... existing variants ...
    Deploys,
    Face,
}

impl Role {
    pub const ALL: [Role; 22] = [
        // ... existing 21 in order ...
        Role::Deploys,
        Role::Face,
    ];
```

Add to each match: `name`: `Role::Face => "face",`; `is_cluster`: add `| Role::Face` after `Role::Deploys`; `accent`: `Role::Face => AMBER,`; `icon`: `Role::Face => "smile",`.

- [ ] **Step 4: Add the icon asset**

Create `assets/icons/smile.svg` (Lucide `smile`, ISC, same shape as the other files):

```svg
<!-- @license lucide-static v0.544.0 - ISC -->
<svg
  class="lucide lucide-smile"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <circle cx="12" cy="12" r="10" />
  <path d="M8 14s1.5 2 4 2 4-2 4-2" />
  <line x1="9" x2="9.01" y1="9" y2="9" />
  <line x1="15" x2="15.01" y1="9" y2="9" />
</svg>
```

Add `"smile",` to the `icons!( ... )` list in `crates/render/src/assets.rs` (at the end of the list) and append ` smile` to the `ICONS=` string in `scripts/fetch-assets.sh`.

- [ ] **Step 5: Stub the scene and wire the dispatch**

Create `crates/core/src/scene_face.rs`:

```rust
//! The `face` role: two eyes on a 24×24 dot matrix. Filled in by the face tasks.

use crate::anim::Secs;
use crate::model::Model;
use crate::scene::Scene;

pub fn face_scene(_model: &Model, _now: Secs) -> Scene {
    Scene::new()
}
```

Add `pub mod scene_face;` to `crates/core/src/lib.rs` (alphabetically after `scene_electricity`). In `crates/core/src/scene.rs` `role_scene`, add after the `Role::Deploys` arm:

```rust
        Role::Face => crate::scene_face::face_scene(model, now),
```

In `crates/core/src/scene_fx.rs` `needs_data`, add:

```rust
            Role::Face => !st.have_nodes,
```

- [ ] **Step 6: Run the whole workspace**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all green. If `crates/render/tests/golden.rs` or the setup crate has an exhaustive match or a `21` that fails, fix it to 22 (the setup tests count `Role::ALL` dynamically; the render golden list is explicit and needs no entry yet).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/theme.rs crates/core/src/scene_face.rs crates/core/src/lib.rs crates/core/src/scene.rs crates/core/src/scene_fx.rs crates/render/src/assets.rs assets/icons/smile.svg scripts/fetch-assets.sh
git commit -m "feat(core): add the face role with a stub scene

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Mood engine and bedtime window

**Files:**
- Create: `crates/core/src/mood.rs`
- Modify: `crates/core/src/night.rs` (add `bedtime_near`), `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum Mood { Content, Happy, Excited, Worried, Sad, Angry, Hot, Scared, Bored, Sleepy }
  impl Mood { pub fn is_severe(self) -> bool }
  pub struct MoodTuning { pub bored_after: Secs, pub reaction: Secs }   // Default: 7200, 60
  pub struct MoodInputs { pub on_battery: bool, pub nodes_not_ready: bool, pub app_degraded: bool, pub volume_degraded: bool, pub alerts: bool, pub hot: bool, pub pods_failed: bool }
  pub struct MoodEngine
  impl MoodEngine {
      pub fn new(now: Secs) -> Self
      pub fn set_tuning(&mut self, t: MoodTuning)
      pub fn tuning(&self) -> MoodTuning
      pub fn touch(&mut self, now: Secs)                 // an event happened (bored timer)
      pub fn crashed(&mut self, now: Secs)               // PodCrashed (also touches)
      pub fn react(&mut self, mood: Mood, now: Secs)     // hold `mood` for `reaction` secs
      pub fn set_bedtime_near(&mut self, near: bool)
      pub fn bedtime_near(&self) -> bool
      pub fn state_mood(&self, i: &MoodInputs, now: Secs) -> Mood
      pub fn current(&self, i: &MoodInputs, now: Secs) -> Mood
  }
  pub fn night::bedtime_near(now_min: u32, start_min: u32, end_min: u32) -> bool
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/mood.rs` with the enum and the tests first:

```rust
//! Which mood the face holds between acts: derived from cluster state, with a
//! short reaction hold after an act.

use crate::anim::Secs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Content,
    Happy,
    Excited,
    Worried,
    Sad,
    Angry,
    Hot,
    Scared,
    Bored,
    Sleepy,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> MoodInputs {
        MoodInputs::default()
    }

    #[test]
    fn state_priority_scared_over_sad_over_angry_over_hot_over_worried() {
        let e = MoodEngine::new(0.0);
        let all = MoodInputs {
            on_battery: true,
            nodes_not_ready: true,
            app_degraded: true,
            volume_degraded: true,
            alerts: true,
            hot: true,
            pods_failed: true,
        };
        assert_eq!(e.state_mood(&all, 1.0), Mood::Scared);
        let mut i = all;
        i.on_battery = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Sad);
        i.nodes_not_ready = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Angry);
        i.app_degraded = false;
        i.volume_degraded = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Angry, "alerts alone are angry");
        i.alerts = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Hot);
        i.hot = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Worried);
        i.pods_failed = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Content);
    }

    #[test]
    fn a_crash_keeps_worried_for_five_minutes() {
        let mut e = MoodEngine::new(0.0);
        e.crashed(10.0);
        assert_eq!(e.state_mood(&quiet(), 100.0), Mood::Worried);
        assert_eq!(e.state_mood(&quiet(), 309.0), Mood::Worried);
        assert_eq!(e.state_mood(&quiet(), 311.0), Mood::Content);
    }

    #[test]
    fn reaction_holds_then_expires() {
        let mut e = MoodEngine::new(0.0);
        e.react(Mood::Excited, 100.0);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Excited);
        assert_eq!(e.current(&quiet(), 159.0), Mood::Excited);
        assert_eq!(e.current(&quiet(), 161.0), Mood::Content);
    }

    #[test]
    fn severe_state_hides_a_reaction() {
        let mut e = MoodEngine::new(0.0);
        e.react(Mood::Excited, 100.0);
        let down = MoodInputs {
            nodes_not_ready: true,
            ..MoodInputs::default()
        };
        assert_eq!(e.current(&down, 101.0), Mood::Sad);
        // node back: happy reaction shows, then content
        e.react(Mood::Happy, 200.0);
        assert_eq!(e.current(&quiet(), 201.0), Mood::Happy);
        assert_eq!(e.current(&quiet(), 300.0), Mood::Content);
    }

    #[test]
    fn bored_after_quiet_and_not_after_boot() {
        let mut e = MoodEngine::new(0.0);
        e.set_tuning(MoodTuning {
            bored_after: 600.0,
            reaction: 60.0,
        });
        assert_eq!(e.current(&quiet(), 599.0), Mood::Content);
        assert_eq!(e.current(&quiet(), 601.0), Mood::Bored);
        e.touch(700.0);
        assert_eq!(e.current(&quiet(), 800.0), Mood::Content);
    }

    #[test]
    fn sleepy_beats_bored_and_loses_to_worried() {
        let mut e = MoodEngine::new(0.0);
        e.set_tuning(MoodTuning {
            bored_after: 10.0,
            reaction: 60.0,
        });
        e.set_bedtime_near(true);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Sleepy);
        let failed = MoodInputs {
            pods_failed: true,
            ..MoodInputs::default()
        };
        assert_eq!(e.current(&failed, 100.0), Mood::Worried);
        e.set_bedtime_near(false);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Bored);
    }

    #[test]
    fn reaction_beats_bored_and_sleepy() {
        let mut e = MoodEngine::new(0.0);
        e.set_bedtime_near(true);
        e.react(Mood::Worried, 5.0);
        assert_eq!(e.current(&quiet(), 6.0), Mood::Worried);
    }
}
```

Add `pub mod mood;` to `crates/core/src/lib.rs` (after `model`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core mood:: 2>&1 | head -20`
Expected: compile errors (`MoodEngine` not found).

- [ ] **Step 3: Implement the engine**

Insert between the enum and the tests in `crates/core/src/mood.rs`:

```rust
impl Mood {
    /// Moods the cluster state forces regardless of any reaction.
    pub fn is_severe(self) -> bool {
        matches!(self, Mood::Scared | Mood::Sad | Mood::Angry | Mood::Hot)
    }
}

/// How long a crash keeps the face worried.
pub const CRASH_WORRY_SECS: Secs = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodTuning {
    pub bored_after: Secs,
    pub reaction: Secs,
}

impl Default for MoodTuning {
    fn default() -> Self {
        Self {
            bored_after: 120.0 * 60.0,
            reaction: 60.0,
        }
    }
}

/// The cluster facts the state mood is derived from, already reduced to bools.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoodInputs {
    pub on_battery: bool,
    pub nodes_not_ready: bool,
    pub app_degraded: bool,
    pub volume_degraded: bool,
    pub alerts: bool,
    pub hot: bool,
    pub pods_failed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoodEngine {
    reaction: Option<(Mood, Secs)>,
    last_event: Secs,
    crashed_at: Option<Secs>,
    bedtime_near: bool,
    tuning: MoodTuning,
}

impl MoodEngine {
    pub fn new(now: Secs) -> Self {
        Self {
            reaction: None,
            last_event: now,
            crashed_at: None,
            bedtime_near: false,
            tuning: MoodTuning::default(),
        }
    }
    pub fn set_tuning(&mut self, t: MoodTuning) {
        self.tuning = t;
    }
    pub fn tuning(&self) -> MoodTuning {
        self.tuning
    }
    /// Something happened: the bored timer restarts.
    pub fn touch(&mut self, now: Secs) {
        self.last_event = now;
    }
    pub fn crashed(&mut self, now: Secs) {
        self.crashed_at = Some(now);
        self.touch(now);
    }
    /// Hold `mood` for `tuning.reaction` seconds (unless the state is severe).
    pub fn react(&mut self, mood: Mood, now: Secs) {
        self.touch(now);
        self.reaction = if mood == Mood::Content {
            None
        } else {
            Some((mood, now))
        };
    }
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.bedtime_near = near;
    }
    pub fn bedtime_near(&self) -> bool {
        self.bedtime_near
    }

    pub fn state_mood(&self, i: &MoodInputs, now: Secs) -> Mood {
        if i.on_battery {
            return Mood::Scared;
        }
        if i.nodes_not_ready {
            return Mood::Sad;
        }
        if i.app_degraded || i.volume_degraded || i.alerts {
            return Mood::Angry;
        }
        if i.hot {
            return Mood::Hot;
        }
        let recent_crash = self
            .crashed_at
            .is_some_and(|t| now - t < CRASH_WORRY_SECS);
        if i.pods_failed || recent_crash {
            return Mood::Worried;
        }
        if self.bedtime_near {
            return Mood::Sleepy;
        }
        if now - self.last_event > self.tuning.bored_after {
            return Mood::Bored;
        }
        Mood::Content
    }

    /// The mood the face holds right now.
    pub fn current(&self, i: &MoodInputs, now: Secs) -> Mood {
        let state = self.state_mood(i, now);
        if state.is_severe() {
            return state;
        }
        match self.reaction {
            Some((m, at)) if now - at <= self.tuning.reaction => m,
            _ => state,
        }
    }
}
```

- [ ] **Step 4: Run the mood tests**

Run: `cargo test -p rackscreen-core mood::`
Expected: 7 passed.

- [ ] **Step 5: Bedtime window, test first**

Append to the tests in `crates/core/src/night.rs`:

```rust
    #[test]
    fn bedtime_is_half_an_hour_before_and_ten_minutes_after() {
        let (s, e) = (1380, 420); // 23:00 .. 07:00
        assert!(!bedtime_near(1349, s, e)); // 22:29
        assert!(bedtime_near(1350, s, e)); // 22:30
        assert!(bedtime_near(1379, s, e)); // 22:59
        assert!(!bedtime_near(1380, s, e)); // 23:00 is night, displays off
        assert!(!bedtime_near(0, s, e));
        assert!(bedtime_near(420, s, e)); // 07:00
        assert!(bedtime_near(429, s, e)); // 07:09
        assert!(!bedtime_near(430, s, e)); // 07:10
        assert!(!bedtime_near(720, s, e));
    }

    #[test]
    fn bedtime_wraps_midnight_and_ignores_a_disabled_window() {
        // 00:10 .. 06:00: the half hour before starts at 23:40
        assert!(bedtime_near(1420, 10, 360));
        assert!(bedtime_near(5, 10, 360));
        assert!(!bedtime_near(10, 10, 360));
        assert!(!bedtime_near(100, 300, 300));
    }
```

Run: `cargo test -p rackscreen-core night::`
Expected: compile error, `bedtime_near` not found.

- [ ] **Step 6: Implement `bedtime_near`**

Add to `crates/core/src/night.rs` after `is_night`:

```rust
/// Minutes before `start` the face turns sleepy.
pub const BEDTIME_BEFORE_MIN: u32 = 30;
/// Minutes after `end` the face is still waking up.
pub const BEDTIME_AFTER_MIN: u32 = 10;

/// True in the half hour before the night window opens and the ten minutes
/// after it closes: the displays are off during the night itself.
pub fn bedtime_near(now_min: u32, start_min: u32, end_min: u32) -> bool {
    if start_min == end_min {
        return false;
    }
    let day = 24 * 60;
    let before_start = (start_min + day - BEDTIME_BEFORE_MIN) % day;
    let after_end = (end_min + BEDTIME_AFTER_MIN) % day;
    is_night(now_min, before_start, start_min) || is_night(now_min, end_min, after_end)
}
```

(`is_night` already implements "inside `[a, b)` with midnight wrap", which is exactly the two windows needed.)

- [ ] **Step 7: Run and commit**

Run: `cargo test -p rackscreen-core night:: mood::` then `cargo clippy -p rackscreen-core --all-targets -- -D warnings`
Expected: green, clean.

```bash
git add crates/core/src/mood.rs crates/core/src/night.rs crates/core/src/lib.rs
git commit -m "feat(core): mood engine and bedtime window for the face

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Expression table and idle behaviour

**Files:**
- Create: `crates/core/src/face_expr.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn lerp / seg / inseg / bump / ease / out_back / xorshift / unit   // see "Shared helper signatures"
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Expr { pub open: f32, pub tilt: f32, pub lower: f32, pub scale: f32, pub sep: f32, pub lift: f32 }
  impl Expr {
      pub const CONTENT/HAPPY/EXCITED/WORRIED/SAD/ANGRY/HOT/SCARED/SLEEPY/BORED: Expr
      pub const RELIEF/GRIMACE/WOW/MEH/FOCUS: Expr
      pub fn of(m: Mood) -> Expr
      pub fn lerp(a: Expr, b: Expr, k: f32) -> Expr
      pub fn close_to(&self, other: &Expr, eps: f32) -> bool
  }
  pub const EXPR_TWEEN_SECS: Secs = 0.42;
  pub struct ExprTween { .. }  // new(still: Expr), retarget(&mut self, to: Expr, now), value(&self, now) -> Expr, target(&self) -> Expr
  pub struct IdleOut { pub blink: f32, pub gaze: (f32, f32), pub tilt: f32, pub wobble: (f32, f32) }
  pub struct Idle { .. }   // Clone, Debug, PartialEq, Default
  impl Idle { pub fn new() -> Self; pub fn tick(&mut self, mood: Mood, now: Secs, rng: &mut u64) -> IdleOut }
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/face_expr.rs` starting with the module doc, the helper functions and the tests (the tests reference items implemented in Step 3):

```rust
//! The face's expression: six knobs per mood, tweened, plus the idle behaviour
//! (blinks, gaze, head tilt, mood wobble) that keeps it alive between acts.

use crate::anim::{Easing, Secs};
use crate::mood::Mood;

pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}
/// 0 at `p <= a`, 1 at `p >= b`, linear between.
pub fn seg(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a)).clamp(0.0, 1.0)
}
pub fn inseg(p: f32, a: f32, b: f32) -> bool {
    p >= a && p < b
}
/// Rises 0..1..0 over `[a, b]`.
pub fn bump(p: f32, a: f32, b: f32) -> f32 {
    (seg(p, a, b) * std::f32::consts::PI).sin()
}
pub fn ease(k: f32) -> f32 {
    Easing::InOutCubic.apply(k)
}
pub fn out_back(k: f32) -> f32 {
    Easing::Spring.apply(k)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(seg(0.5, 0.0, 1.0), 0.5);
        assert_eq!(seg(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(seg(2.0, 0.0, 1.0), 1.0);
        assert!(inseg(0.2, 0.2, 0.5) && !inseg(0.5, 0.2, 0.5));
        assert!((bump(0.5, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(bump(0.0, 0.0, 1.0).abs() < 1e-6);
        let mut s = 7u64;
        let a = unit(&mut s);
        assert!((0.0..1.0).contains(&a));
        assert_ne!(a, unit(&mut s));
    }

    #[test]
    fn mood_table_matches_the_spec() {
        assert_eq!(Expr::of(Mood::Content), Expr::CONTENT);
        assert_eq!(Expr::HAPPY.lower, 0.70);
        assert_eq!(Expr::HAPPY.lift, -4.0);
        assert_eq!(Expr::SAD.tilt, -1.0);
        assert_eq!(Expr::SAD.lift, 8.0);
        assert_eq!(Expr::ANGRY.tilt, 1.0);
        assert_eq!(Expr::SCARED.scale, 0.85);
        assert_eq!(Expr::SLEEPY.open, 0.25);
        assert_eq!(Expr::of(Mood::Bored).open, 0.50);
        let mid = Expr::lerp(Expr::CONTENT, Expr::SAD, 0.5);
        assert!((mid.lift - 4.0).abs() < 1e-6);
        assert!((mid.tilt + 0.5).abs() < 1e-6);
    }

    #[test]
    fn expr_tween_retargets_without_a_jump() {
        let mut t = ExprTween::new(Expr::CONTENT);
        assert_eq!(t.value(5.0), Expr::CONTENT);
        t.retarget(Expr::SAD, 10.0);
        let before = t.value(10.0);
        assert!(before.close_to(&Expr::CONTENT, 1e-4));
        let mid = t.value(10.21);
        assert!(mid.lift > 0.5 && mid.lift < 7.5, "half way: {}", mid.lift);
        assert!(t.value(10.42).close_to(&Expr::SAD, 1e-4));
        // retarget mid-flight starts from the current value
        t.retarget(Expr::CONTENT, 10.21);
        assert!(t.value(10.21).close_to(&mid, 1e-4));
        assert_eq!(t.target(), Expr::CONTENT);
    }

    #[test]
    fn idle_blinks_and_wanders_deterministically() {
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
            t += 1.0 / 30.0;
        }
        assert!(blinked, "no full blink in 30 s");
        assert!(gazes.len() > 20, "gaze never wandered");
        // same seed, same story
        let mut rng2 = 99u64;
        let mut idle2 = Idle::new();
        let mut rng3 = 99u64;
        let mut idle3 = Idle::new();
        for i in 0..900 {
            let t = i as f64 / 30.0;
            assert_eq!(
                idle2.tick(Mood::Content, t, &mut rng2),
                idle3.tick(Mood::Content, t, &mut rng3)
            );
        }
    }

    #[test]
    fn scared_and_angry_blink_shallow_and_bored_looks_sideways() {
        let mut rng = 3u64;
        let mut idle = Idle::new();
        let mut max_blink: f32 = 0.0;
        let mut t = 0.0;
        while t < 20.0 {
            max_blink = max_blink.max(idle.tick(Mood::Scared, t, &mut rng).blink);
            t += 1.0 / 30.0;
        }
        assert!(max_blink > 0.5 && max_blink <= 0.6 + 1e-3, "{max_blink}");
        let mut idle = Idle::new();
        let mut t = 0.0;
        let mut sideways = false;
        while t < 20.0 {
            let g = idle.tick(Mood::Bored, t, &mut rng).gaze;
            if g.0.abs() > 0.95 {
                sideways = true;
            }
            t += 1.0 / 30.0;
        }
        assert!(sideways);
    }

    #[test]
    fn wobble_is_zero_for_content_and_sags_for_sad() {
        let mut rng = 1u64;
        let mut idle = Idle::new();
        let c = idle.tick(Mood::Content, 1.0, &mut rng).wobble;
        assert_eq!(c, (0.0, 0.0));
        let s = idle.tick(Mood::Sad, 1.0, &mut rng).wobble;
        assert_eq!(s, (0.0, 4.0));
    }
}
```

Add `pub mod face_expr;` to `crates/core/src/lib.rs` (after `event`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core face_expr:: 2>&1 | head`
Expected: compile errors for `Expr`, `ExprTween`, `Idle`.

- [ ] **Step 3: Implement `Expr`, `ExprTween` and `Idle`**

Insert before the tests:

```rust
/// One expression: lid openness, upper-lid slant (+ inner corners down =
/// angry, − = sad), lower-lid squint (0.7 is the `^ ^` shape), eye size,
/// separation and vertical lift in pixels (negative is up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expr {
    pub open: f32,
    pub tilt: f32,
    pub lower: f32,
    pub scale: f32,
    pub sep: f32,
    pub lift: f32,
}

const fn expr(open: f32, tilt: f32, lower: f32, scale: f32, sep: f32, lift: f32) -> Expr {
    Expr {
        open,
        tilt,
        lower,
        scale,
        sep,
        lift,
    }
}

impl Expr {
    pub const CONTENT: Expr = expr(1.00, 0.00, 0.00, 1.00, 1.00, 0.0);
    pub const HAPPY: Expr = expr(1.00, 0.00, 0.70, 1.00, 1.00, -4.0);
    pub const EXCITED: Expr = expr(1.20, -0.10, 0.10, 1.10, 1.02, -6.0);
    pub const WORRIED: Expr = expr(0.85, -0.60, 0.10, 1.00, 1.00, 2.0);
    pub const SAD: Expr = expr(0.65, -1.00, 0.00, 1.00, 1.00, 8.0);
    pub const ANGRY: Expr = expr(0.60, 1.00, 0.15, 1.00, 0.96, 0.0);
    pub const HOT: Expr = expr(0.70, 0.30, 0.20, 1.00, 1.00, 3.0);
    pub const SCARED: Expr = expr(1.30, -0.40, 0.00, 0.85, 1.00, -2.0);
    pub const SLEEPY: Expr = expr(0.25, 0.00, 0.00, 1.00, 1.00, 6.0);
    pub const BORED: Expr = expr(0.50, 0.00, 0.00, 1.00, 1.00, 2.0);
    // transient poses used inside acts only
    pub const RELIEF: Expr = expr(0.12, 0.00, 0.40, 1.00, 1.00, 0.0);
    pub const GRIMACE: Expr = expr(0.55, 0.50, 0.35, 1.00, 0.97, 0.0);
    pub const WOW: Expr = expr(1.35, 0.00, 0.00, 1.05, 1.00, -3.0);
    pub const MEH: Expr = expr(0.50, 0.00, 0.00, 1.00, 1.00, 2.0);
    pub const FOCUS: Expr = expr(0.75, 0.20, 0.10, 1.00, 0.94, 0.0);

    pub fn of(m: Mood) -> Expr {
        match m {
            Mood::Content => Expr::CONTENT,
            Mood::Happy => Expr::HAPPY,
            Mood::Excited => Expr::EXCITED,
            Mood::Worried => Expr::WORRIED,
            Mood::Sad => Expr::SAD,
            Mood::Angry => Expr::ANGRY,
            Mood::Hot => Expr::HOT,
            Mood::Scared => Expr::SCARED,
            Mood::Sleepy => Expr::SLEEPY,
            Mood::Bored => Expr::BORED,
        }
    }
    pub fn lerp(a: Expr, b: Expr, k: f32) -> Expr {
        expr(
            lerp(a.open, b.open, k),
            lerp(a.tilt, b.tilt, k),
            lerp(a.lower, b.lower, k),
            lerp(a.scale, b.scale, k),
            lerp(a.sep, b.sep, k),
            lerp(a.lift, b.lift, k),
        )
    }
    pub fn close_to(&self, o: &Expr, eps: f32) -> bool {
        (self.open - o.open).abs() <= eps
            && (self.tilt - o.tilt).abs() <= eps
            && (self.lower - o.lower).abs() <= eps
            && (self.scale - o.scale).abs() <= eps
            && (self.sep - o.sep).abs() <= eps
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
    /// Head tilt in radians.
    pub tilt: f32,
    /// Mood wobble offset in pixels.
    pub wobble: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Idle {
    next_blink: Secs,
    blink_start: Option<Secs>,
    double: bool,
    gaze_from: (f32, f32),
    gaze_to: (f32, f32),
    gaze_start: Secs,
    gaze_dur: Secs,
    next_gaze: Secs,
    tilt_start: Option<(Secs, f32)>,
    next_tilt: Secs,
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
            double: false,
            gaze_from: (0.0, 0.0),
            gaze_to: (0.0, 0.0),
            gaze_start: 0.0,
            gaze_dur: 0.26,
            next_gaze: 0.8,
            tilt_start: None,
            next_tilt: 10.0,
        }
    }

    pub fn tick(&mut self, mood: Mood, now: Secs, rng: &mut u64) -> IdleOut {
        // blink: every 1.5..4 s, 150 ms, 20 % double
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
                if !self.double && unit(rng) < 0.2 {
                    self.double = true;
                    self.next_blink = now + 0.12;
                } else {
                    self.double = false;
                    self.next_blink = now + 1.5 + 2.5 * unit(rng) as Secs;
                }
            }
        }
        if matches!(mood, Mood::Scared | Mood::Angry) {
            blink *= 0.6;
        }
        // gaze: retarget every 0.7..2 s (0.35..0.85 s when jittery)
        let fast = matches!(mood, Mood::Worried | Mood::Excited | Mood::Scared);
        if now >= self.next_gaze {
            self.gaze_from = self.gaze_value(now);
            self.gaze_to = match mood {
                Mood::Bored => (if unit(rng) < 0.5 { -1.0 } else { 1.0 }, 0.15),
                Mood::Sleepy => ((unit(rng) - 0.5) * 0.3, 0.6),
                Mood::Sad => ((unit(rng) - 0.5) * 0.6, 0.5),
                _ => ((unit(rng) - 0.5) * 1.6, (unit(rng) - 0.5) * 1.0),
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
        // head tilt: ±6° for 1.5 s every 10..25 s
        if self.tilt_start.is_none() && now >= self.next_tilt {
            let dir = if unit(rng) < 0.5 { -1.0 } else { 1.0 };
            self.tilt_start = Some((now, dir));
        }
        let mut tilt = 0.0;
        if let Some((start, dir)) = self.tilt_start {
            let p = ((now - start) / 1.5) as f32;
            if p < 1.0 {
                tilt = dir * 6.0_f32.to_radians() * bump(p, 0.0, 1.0);
            } else {
                self.tilt_start = None;
                self.next_tilt = now + 10.0 + 15.0 * unit(rng) as Secs;
            }
        }
        let t = now as f32;
        let wobble = match mood {
            Mood::Excited => (0.0, -(t * 9.0).sin().abs() * 7.0),
            Mood::Angry => {
                let burst = if (t * 1.3).sin() > 0.2 { 1.0 } else { 0.0 };
                ((t * 46.0).sin() * 2.2 * burst, 0.0)
            }
            Mood::Scared => ((t * 70.0).sin() * 1.4, 0.0),
            Mood::Sleepy => (0.0, (t * 1.4).sin() * 4.0 + 3.0),
            Mood::Sad => (0.0, 4.0),
            Mood::Bored => (0.0, 2.0),
            _ => (0.0, 0.0),
        };
        IdleOut {
            blink,
            gaze,
            tilt,
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

- [ ] **Step 4: Run and commit**

Run: `cargo test -p rackscreen-core face_expr::` then clippy.
Expected: 6 passed, clean.

```bash
git add crates/core/src/face_expr.rs crates/core/src/lib.rs
git commit -m "feat(core): face expression table, tween and idle behaviour

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Sprites and tints

**Files:**
- Create: `crates/core/src/face_sprites.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, PartialEq)] pub struct Sprite { pub rows: &'static [&'static str] }
  impl Sprite { pub fn w(&self) -> usize; pub fn h(&self) -> usize; pub fn on(&self, i: usize, j: usize) -> bool }
  pub static BOX, SERVER, BRANCH, ROCKET, DOWNLOAD, BOLT, BATLOW, BELL, BELL2, CHECK, CROSS, TRI, CPU, MEM, DISK, FLAME, CLOUD, SUN, MOON, EURO, NOTE, WIND, SAT, TROPHY, FLAKE, FOG, THERM: Sprite   // icons, 7 wide (SERVER, MEM, CLOUD 5 tall; BATLOW 6; CROSS 5×5)
  pub static ICONS: &[(&str, &Sprite)]
  pub static XEYE, STAR, SPIRAL1, SPIRAL2, EUROEYE, SHADE, SQZ_L, SQZ_R, ARC: Sprite       // eye sprites, 7 wide
  pub static DOT, COIN, Z, STARLET: Sprite                                                  // 1×1, 2×2, 2×3, 5×5
  pub static DISK_FILL: [Sprite; 8]                                                          // disk with the bottom n rows filled
  pub const T_GITHUB, T_ARGO, T_TORRENT, T_K8S, T_UPS, T_LONGHORN, T_PROM, T_ALERT, T_ISS, T_SKY, T_PRICE, T_MEM, T_WX, T_SNOW, T_WIND: Color
  ```

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/face_sprites.rs`:

```rust
//! Pixel bitmaps for the face: source icons (7 wide), eye sprites (7 wide) and
//! the odd dot. `#` is lit, anything else dark.

use crate::theme::Color;

#[derive(Debug, PartialEq)]
pub struct Sprite {
    pub rows: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_is_rectangular_and_icons_are_seven_wide() {
        for (name, s) in ICONS {
            assert_eq!(s.w(), 7, "{name} is not 7 wide");
            assert!(s.h() >= 5 && s.h() <= 7, "{name} height");
            for r in s.rows {
                assert_eq!(r.len(), 7, "{name} has a ragged row");
            }
        }
        for s in [&XEYE, &STAR, &SPIRAL1, &SPIRAL2, &EUROEYE, &SHADE, &SQZ_L, &SQZ_R, &ARC] {
            assert_eq!(s.w(), 7);
            for r in s.rows {
                assert_eq!(r.len(), 7);
            }
        }
        assert_eq!((DOT.w(), DOT.h()), (1, 1));
        assert_eq!((COIN.w(), COIN.h()), (2, 2));
        assert_eq!((Z.w(), Z.h()), (2, 3));
        assert_eq!((STARLET.w(), STARLET.h()), (5, 5));
        assert!(DOT.on(0, 0));
        assert!(!BOX.on(0, 0) && BOX.on(2, 0));
        assert!(!BOX.on(9, 9), "out of range is dark");
    }

    #[test]
    fn sqz_is_mirrored_and_spiral2_is_spiral1_upside_down() {
        for j in 0..7 {
            for i in 0..7 {
                assert_eq!(SQZ_L.on(i, j), SQZ_R.on(6 - i, j));
                assert_eq!(SPIRAL1.on(i, j), SPIRAL2.on(i, 6 - j));
            }
        }
    }

    #[test]
    fn disk_fill_levels_fill_from_the_bottom() {
        assert_eq!(DISK_FILL[0].rows, DISK.rows);
        assert!(DISK_FILL[7].rows.iter().all(|r| r == &"#######"));
        assert!(DISK_FILL[3].on(3, 6) && DISK_FILL[3].on(3, 4));
        assert!(!DISK_FILL[3].on(3, 3));
    }
}
```

Add `pub mod face_sprites;` to `crates/core/src/lib.rs` (after `face_expr`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core face_sprites::`
Expected: compile errors (`ICONS`, `XEYE` ... missing).

- [ ] **Step 3: Implement the bitmaps**

Insert between `Sprite` and the tests:

```rust
impl Sprite {
    pub fn w(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }
    pub fn h(&self) -> usize {
        self.rows.len()
    }
    /// Cell `(i, j)` lit? Out of range is dark.
    pub fn on(&self, i: usize, j: usize) -> bool {
        self.rows
            .get(j)
            .and_then(|r| r.as_bytes().get(i))
            .is_some_and(|b| *b == b'#')
    }
}

macro_rules! sprite {
    ($name:ident, $($row:literal),+ $(,)?) => {
        pub static $name: Sprite = Sprite { rows: &[$($row),+] };
    };
}

// ---- source icons, 7 wide ----
sprite!(BOX, "..###..", ".#...#.", "#..#..#", "#.###.#", "#..#..#", ".#.#.#.", "..###..");
sprite!(SERVER, "#######", "#.#...#", "#######", "#.#...#", "#######");
sprite!(BRANCH, ".##....", ".##..##", "..#..##", "..#.#..", "..##...", "..#....", ".##....");
sprite!(ROCKET, "...#...", "..###..", "..###..", "..###..", ".#####.", "#..#..#", "...#...");
sprite!(DOWNLOAD, "...#...", "...#...", ".#####.", "..###..", "...#...", "#.....#", "#######");
sprite!(BOLT, "...##..", "..##...", ".###...", "..####.", "...##..", "..##...", ".##....");
sprite!(BATLOW, "######.", "#.....#", "##....#", "##....#", "#.....#", "######.");
sprite!(BELL, "...#...", "..###..", ".#####.", ".#####.", ".#####.", "#######", "...#...");
sprite!(BELL2, "....#..", "...###.", "..####.", ".#####.", ".#####.", "######.", "...#...");
sprite!(CHECK, "......#", ".....##", "....##.", "#..##..", "##.#...", ".###...", "..#....");
sprite!(CROSS, "#...#", ".#.#.", "..#..", ".#.#.", "#...#");
sprite!(TRI, "...#...", "..#.#..", "..#.#..", ".#...#.", ".#.#.#.", "#.....#", "#######");
sprite!(CPU, ".#.#.#.", "#######", "#.....#", "#..#..#", "#.....#", "#######", ".#.#.#.");
sprite!(MEM, "#######", "#.#.#.#", "#.#.#.#", "#######", ".#.#.#.");
sprite!(DISK, ".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", ".#####.");
sprite!(FLAME, "...#...", "..##...", "..###..", ".#####.", ".#####.", ".##.##.", "..###..");
sprite!(CLOUD, "..###..", ".#...##", "#.....#", "#.....#", ".#####.");
sprite!(SUN, "#..#..#", ".#.#.#.", "..###..", "#.###.#", "..###..", ".#.#.#.", "#..#..#");
sprite!(MOON, "..###..", ".##....", "##.....", "##.....", "##.....", ".##....", "..###..");
sprite!(EURO, "..####.", ".#....#", "####...", ".#.....", "####...", ".#....#", "..####.");
sprite!(NOTE, "...##..", "...#.#.", "...#...", "...#...", ".###...", "####...", ".##....");
sprite!(WIND, ".###...", "....#..", "######.", ".......", "..####.", "......#", ".#####.");
sprite!(SAT, "#.....#", "##...##", ".#####.", "..###..", ".#####.", "##...##", "#.....#");
sprite!(TROPHY, "#######", ".#####.", ".#####.", "..###..", "...#...", "..###..", ".#####.");
sprite!(FLAKE, "#..#..#", ".#.#.#.", "..###..", "#######", "..###..", ".#.#.#.", "#..#..#");
sprite!(FOG, ".......", "#####..", ".......", "..#####", ".......", "#####..", ".......");
sprite!(THERM, "...#...", "..#.#..", "..#.#..", "..#.#..", ".##.##.", ".#####.", "..###..");

/// Every icon with its name, for tests and the catalogue.
pub static ICONS: &[(&str, &Sprite)] = &[
    ("box", &BOX), ("server", &SERVER), ("branch", &BRANCH), ("rocket", &ROCKET),
    ("download", &DOWNLOAD), ("bolt", &BOLT), ("batlow", &BATLOW), ("bell", &BELL),
    ("bell2", &BELL2), ("check", &CHECK), ("tri", &TRI), ("cpu", &CPU), ("mem", &MEM),
    ("disk", &DISK), ("flame", &FLAME), ("cloud", &CLOUD), ("sun", &SUN), ("moon", &MOON),
    ("euro", &EURO), ("note", &NOTE), ("wind", &WIND), ("sat", &SAT), ("trophy", &TROPHY),
    ("flake", &FLAKE), ("fog", &FOG), ("therm", &THERM),
];

// ---- eye sprites, 7 wide, replace an eye ----
sprite!(XEYE, "##...##", ".##.##.", "..###..", "...#...", "..###..", ".##.##.", "##...##");
sprite!(STAR, "...#...", "...#...", "..###..", "#######", ".#####.", "..#.#..", ".#...#.");
sprite!(SPIRAL1, ".#####.", "#.....#", "#.###.#", "#.#.#.#", "#.#.###", "#.#....", ".####..");
sprite!(SPIRAL2, ".####..", "#.#....", "#.#.###", "#.#.#.#", "#.###.#", "#.....#", ".#####.");
sprite!(EUROEYE, "..####.", ".#....#", "####...", ".#.....", "####...", ".#....#", "..####.");
sprite!(SHADE, "#######", "#######", "#######");
sprite!(SQZ_L, "#......", ".#.....", "..#....", "...#...", "..#....", ".#.....", "#......");
sprite!(SQZ_R, "......#", ".....#.", "....#..", "...#...", "....#..", ".....#.", "......#");
sprite!(ARC, "..###..", ".#...#.", "#.....#");

// ---- small things ----
sprite!(DOT, "#");
sprite!(COIN, "##", "##");
sprite!(Z, "##", ".#", "##");
sprite!(STARLET, "..#..", "..#..", "#####", ".###.", "#...#");

/// The disk icon with its bottom `n` rows filled solid, `n = 0..=7`.
pub static DISK_FILL: [Sprite; 8] = [
    Sprite { rows: &[".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", ".#####."] },
    Sprite { rows: &[".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", "#######"] },
    Sprite { rows: &[".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#######", "#######"] },
    Sprite { rows: &[".#####.", "#.....#", ".#####.", "#.....#", "#######", "#######", "#######"] },
    Sprite { rows: &[".#####.", "#.....#", ".#####.", "#######", "#######", "#######", "#######"] },
    Sprite { rows: &[".#####.", "#.....#", "#######", "#######", "#######", "#######", "#######"] },
    Sprite { rows: &[".#####.", "#######", "#######", "#######", "#######", "#######", "#######"] },
    Sprite { rows: &["#######", "#######", "#######", "#######", "#######", "#######", "#######"] },
];

// ---- tints per source ----
pub const T_GITHUB: Color = Color::hex(0xe6e6ff);
pub const T_ARGO: Color = Color::hex(0xff9a3c);
pub const T_TORRENT: Color = Color::hex(0x5ac8fa);
pub const T_K8S: Color = Color::hex(0x4c8dff);
pub const T_UPS: Color = Color::hex(0xffe66d);
pub const T_LONGHORN: Color = Color::hex(0x8be9a5);
pub const T_PROM: Color = Color::hex(0xff6b57);
pub const T_ALERT: Color = Color::hex(0xff5a5a);
pub const T_ISS: Color = Color::hex(0xc9d6ff);
pub const T_SKY: Color = Color::hex(0xffd36b);
pub const T_PRICE: Color = Color::hex(0x7ee0c3);
pub const T_MEM: Color = Color::hex(0xb48cff);
pub const T_WX: Color = Color::hex(0x9ad4ff);
pub const T_SNOW: Color = Color::hex(0xe8f4ff);
pub const T_WIND: Color = Color::hex(0xb8c4cc);
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p rackscreen-core face_sprites::` then `cargo fmt --all` and clippy (rustfmt will reflow the `ICONS` list; that is fine).
Expected: 3 passed, clean.

```bash
git add crates/core/src/face_sprites.rs crates/core/src/lib.rs
git commit -m "feat(core): face sprites and source tints

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Act engine, envelope, and the pod, node and GitHub acts

**Files:**
- Create: `crates/core/src/face_acts.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `face_expr::{Expr, lerp, seg, inseg, bump, ease, out_back}`, `face_sprites::*`, `mood::Mood`.
- Produces:
  ```rust
  pub const IN_S: f32 = 0.3; pub const SLOT_X: f32 = 9.0; pub const SLOT_Y: f32 = 17.0; pub const RISE_PX: f32 = 30.0;
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum ActKind { OhHi, Ouch, Bye, LostOne, ItsBack, Catch, StarryEyes, Merge, Party, Ding, EyeRoll, LevelUp, /* Tasks 6..8 add more */ }
  impl ActKind { pub const ALL: &'static [ActKind]; pub fn def(self) -> ActDef; pub fn is_severe(self) -> bool; pub fn is_habit(self) -> bool; pub fn is_rate_limited(self) -> bool }
  pub type EyePair = [&'static Sprite; 2];
  pub fn both(s: &'static Sprite) -> EyePair
  #[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct EyeOv { pub open: Option<f32>, pub scale: Option<f32>, pub tilt: Option<f32> }
  #[derive(Clone, Copy, Debug, PartialEq)] pub enum Overlay { None, Confetti(f32), FogBand(f32) }
  #[derive(Clone, Copy, Debug, PartialEq)] pub struct Post { pub flicker: f32, pub rim: f32, pub overlay: Overlay }   // Default: 1.0, 0.0, None
  #[derive(Clone, Debug, PartialEq)] pub struct Placed { pub sprite: &'static Sprite, pub x: f32, pub y: f32, pub tint: Option<Color>, pub alpha: f32 }
  #[derive(Clone, Debug, PartialEq)] pub struct Frame { pub q: f32, pub t: f32, pub e: Expr, pub off: (f32, f32), pub rot: f32, pub gaze: (f32, f32), pub eyes: [EyeOv; 2], pub sprite: Option<EyePair>, pub icon_alpha: f32, pub icon_frame: Option<&'static Sprite>, pub placed: Vec<Placed>, pub post: Post, pub slot_y: f32 }
  impl Frame { pub fn at(&mut self, s: &'static Sprite, x: f32, y: f32, tint: Option<Color>, alpha: f32); pub fn over(&mut self, s, dx, dy, tint, alpha); pub fn dot(&mut self, x, y, tint: Option<Color>, alpha); pub fn set(&mut self, e: &Expr, k: f32); pub fn hold(&mut self, e: &Expr, a: f32, b: f32) }
  pub struct ActDef { pub kind: ActKind, pub dur: f32, pub mood: Mood, pub icon: Option<&'static Sprite>, pub tint: Option<Color>, pub rise: bool, pub body: fn(&mut Frame) }
  pub struct Env { pub env: f32, pub q: f32, pub inn: f32, pub out: f32 }
  pub fn envelope(p: f32, dur: f32) -> Env
  pub fn run_act(def: &ActDef, p: f32, held: Expr, idle_gaze: (f32, f32)) -> Frame
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/face_acts.rs`:

```rust
//! The acts: one short choreography per event, all sharing one envelope so any
//! act can follow any other. Bodies are a port of the approved mockup
//! (`acts-4.html`); `q` is body progress, `t` seconds since the act started.

use std::f32::consts::PI;

use crate::face_expr::{bump, ease, inseg, lerp, out_back, seg, Expr};
use crate::face_sprites::*;
use crate::mood::Mood;
use crate::theme::Color;

/// Envelope in and out, seconds.
pub const IN_S: f32 = 0.3;
/// Icon slot: 7×7 cells, left column and top row when fully in.
pub const SLOT_X: f32 = 9.0;
pub const SLOT_Y: f32 = 17.0;
/// How far the face rises while an icon is in the slot.
pub const RISE_PX: f32 = 30.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: ActKind, p: f32) -> Frame {
        run_act(&kind.def(), p, Expr::SAD, (0.4, -0.3))
    }

    #[test]
    fn every_act_starts_and_ends_at_rest() {
        for kind in ActKind::ALL {
            let d = kind.def();
            for p in [0.0, 1.0] {
                let f = frame(*kind, p);
                assert!(f.off.0.abs() < 1e-3 && f.off.1.abs() < 1e-3, "{kind:?} offset at p={p}: {:?}", f.off);
                assert!(f.rot.abs() < 1e-3, "{kind:?} rot at p={p}");
                assert!(f.sprite.is_none(), "{kind:?} eye sprite at p={p}");
                assert_eq!(f.eyes, [EyeOv::default(); 2], "{kind:?} eye override at p={p}");
                for pl in &f.placed {
                    assert!(pl.alpha < 1e-3 || pl.y >= 24.0, "{kind:?} visible sprite at p={p}: {:?}", (pl.x, pl.y, pl.alpha));
                }
                assert!((f.gaze.0 - 0.4).abs() < 1e-3 && (f.gaze.1 + 0.3).abs() < 1e-3, "{kind:?} gaze at p={p}");
                assert!((f.post.flicker - 1.0).abs() < 1e-3 && f.post.rim.abs() < 1e-3, "{kind:?} post at p={p}");
                assert_eq!(f.post.overlay, Overlay::None, "{kind:?} overlay at p={p}");
            }
            assert!(frame(*kind, 0.0).e.close_to(&Expr::SAD, 1e-3), "{kind:?} does not start from the held mood");
            assert!(frame(*kind, 1.0).e.close_to(&Expr::of(d.mood), 1e-3), "{kind:?} does not end in its mood");
            assert!(d.dur >= 1.4 && d.dur <= 3.6, "{kind:?} duration");
        }
    }

    #[test]
    fn icon_sits_in_the_slot_and_the_face_rises_mid_act() {
        let f = frame(ActKind::LostOne, 0.5);
        let icon = &f.placed[0];
        assert!(std::ptr::eq(icon.sprite, &SERVER));
        assert_eq!((icon.x, icon.y), (SLOT_X, SLOT_Y));
        assert_eq!(icon.tint, Some(T_K8S));
        assert!((f.off.1 + RISE_PX).abs() < 1e-3, "rise: {}", f.off.1);
        assert!(f.gaze.1 > 0.8, "looking down at the icon");
        // ouch has no icon but still rises (rise: true)
        let f = frame(ActKind::Ouch, 0.5);
        assert!(f.off.1 < 0.0);
    }

    #[test]
    fn envelope_shape() {
        let e = envelope(0.0, 3.0);
        assert_eq!((e.env, e.q, e.inn, e.out), (0.0, 0.0, 0.0, 0.0));
        let e = envelope(0.05, 3.0);
        assert!((e.env - 0.5).abs() < 1e-6 && e.q == 0.0);
        let e = envelope(0.5, 3.0);
        assert!((e.env - 1.0).abs() < 1e-6 && (e.q - 0.5).abs() < 1e-6);
        let e = envelope(0.95, 3.0);
        assert!((e.env - 0.5).abs() < 1e-6 && e.q == 1.0 && (e.out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ouch_shows_x_eyes_and_lands_the_box() {
        let d = ActKind::Ouch.def();
        let mid = run_act(&d, 0.55, Expr::CONTENT, (0.0, 0.0));
        assert!(mid.sprite.is_some_and(|s| std::ptr::eq(s[0], &XEYE)));
        let boxes: Vec<&Placed> = mid.placed.iter().filter(|p| std::ptr::eq(p.sprite, &BOX)).collect();
        assert_eq!(boxes.len(), 1);
        assert!((boxes[0].y - SLOT_Y).abs() < 1e-3);
        assert_eq!(d.mood, Mood::Worried);
    }

    #[test]
    fn kinds_have_the_right_flags() {
        assert!(ActKind::Ouch.is_severe() && ActKind::LostOne.is_severe());
        assert!(!ActKind::Catch.is_severe());
        assert!(ActKind::OhHi.is_rate_limited() && ActKind::Bye.is_rate_limited());
        assert!(!ActKind::OhHi.is_habit());
        let kinds: std::collections::HashSet<ActKind> = ActKind::ALL.iter().copied().collect();
        assert_eq!(kinds.len(), ActKind::ALL.len(), "ALL has duplicates");
        for k in ActKind::ALL {
            assert_eq!(k.def().kind, *k);
        }
    }
}
```

Add `pub mod face_acts;` to `crates/core/src/lib.rs` (after `event`, before `face_expr`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core face_acts:: 2>&1 | head`
Expected: compile errors.

- [ ] **Step 3: Implement the engine**

Insert after the constants:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActKind {
    // pods
    OhHi,
    Ouch,
    Bye,
    // nodes
    LostOne,
    ItsBack,
    // github
    Catch,
    StarryEyes,
    Merge,
    Party,
    Ding,
    EyeRoll,
    LevelUp,
}

impl ActKind {
    pub const ALL: &'static [ActKind] = &[
        ActKind::OhHi,
        ActKind::Ouch,
        ActKind::Bye,
        ActKind::LostOne,
        ActKind::ItsBack,
        ActKind::Catch,
        ActKind::StarryEyes,
        ActKind::Merge,
        ActKind::Party,
        ActKind::Ding,
        ActKind::EyeRoll,
        ActKind::LevelUp,
    ];

    /// Jumps the queue.
    pub fn is_severe(self) -> bool {
        matches!(self, ActKind::Ouch | ActKind::LostOne)
    }
    /// Habits never queue and any event interrupts them.
    pub fn is_habit(self) -> bool {
        false
    }
    /// At most one per 20 s.
    pub fn is_rate_limited(self) -> bool {
        matches!(self, ActKind::OhHi | ActKind::Bye)
    }

    pub fn def(self) -> ActDef {
        let d = |dur: f32, mood: Mood, icon: Option<&'static Sprite>, tint: Option<Color>, body: fn(&mut Frame)| ActDef {
            kind: self,
            dur,
            mood,
            icon,
            tint,
            rise: icon.is_some(),
            body,
        };
        match self {
            ActKind::OhHi => d(1.8, Mood::Content, Some(&BOX), Some(T_K8S), oh_hi),
            ActKind::Ouch => ActDef { rise: true, ..d(2.8, Mood::Worried, None, None, ouch) },
            ActKind::Bye => d(2.4, Mood::Content, Some(&BOX), Some(T_K8S), bye),
            ActKind::LostOne => d(3.0, Mood::Sad, Some(&SERVER), Some(T_K8S), lost_one),
            ActKind::ItsBack => d(2.6, Mood::Happy, Some(&SERVER), Some(T_K8S), its_back),
            ActKind::Catch => d(2.8, Mood::Excited, Some(&BRANCH), Some(T_GITHUB), catch),
            ActKind::StarryEyes => d(2.6, Mood::Excited, Some(&BRANCH), Some(T_GITHUB), starry_eyes),
            ActKind::Merge => d(2.6, Mood::Happy, Some(&BRANCH), Some(T_GITHUB), merge),
            ActKind::Party => d(3.2, Mood::Excited, Some(&BRANCH), Some(T_GITHUB), party),
            ActKind::Ding => d(2.0, Mood::Happy, Some(&CHECK), Some(T_LONGHORN), ding),
            ActKind::EyeRoll => d(2.6, Mood::Worried, Some(&CROSS), Some(T_ALERT), eye_roll),
            ActKind::LevelUp => d(3.0, Mood::Excited, Some(&TROPHY), Some(T_SKY), level_up),
        }
    }
}

pub type EyePair = [&'static Sprite; 2];
pub fn both(s: &'static Sprite) -> EyePair {
    [s, s]
}

/// Per-eye override an act may set; `[left, right]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EyeOv {
    pub open: Option<f32>,
    pub scale: Option<f32>,
    pub tilt: Option<f32>,
}

/// Whole-matrix overlays the sampler applies; the `f32` is the act clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overlay {
    None,
    Confetti(f32),
    FogBand(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    /// Brightness multiplier for every lit cell (3.0 is lightning).
    pub flicker: f32,
    /// Minimum brightness for cells more than 104 px from the centre.
    pub rim: f32,
    pub overlay: Overlay,
}

impl Default for Post {
    fn default() -> Self {
        Post {
            flicker: 1.0,
            rim: 0.0,
            overlay: Overlay::None,
        }
    }
}

/// A sprite at a (fractional) cell position, in grid space, not rotated with the face.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub sprite: &'static Sprite,
    pub x: f32,
    pub y: f32,
    pub tint: Option<Color>,
    pub alpha: f32,
}

/// What one act frame asks the face to be. `e`, `off`, `rot`, `gaze` and
/// `eyes` shape the eyes; `placed` are sprites on top; `post` is applied to
/// the whole matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub q: f32,
    pub t: f32,
    pub e: Expr,
    pub off: (f32, f32),
    pub rot: f32,
    pub gaze: (f32, f32),
    pub eyes: [EyeOv; 2],
    pub sprite: Option<EyePair>,
    pub icon_alpha: f32,
    pub icon_frame: Option<&'static Sprite>,
    pub placed: Vec<Placed>,
    pub post: Post,
    pub slot_y: f32,
}

impl Frame {
    pub fn at(&mut self, s: &'static Sprite, x: f32, y: f32, tint: Option<Color>, alpha: f32) {
        if alpha > 0.0 {
            self.placed.push(Placed {
                sprite: s,
                x,
                y,
                tint,
                alpha,
            });
        }
    }
    /// Relative to the icon slot.
    pub fn over(&mut self, s: &'static Sprite, dx: f32, dy: f32, tint: Option<Color>, alpha: f32) {
        let y = self.slot_y + dy;
        self.at(s, SLOT_X + dx, y, tint, alpha);
    }
    pub fn dot(&mut self, x: f32, y: f32, tint: Option<Color>, alpha: f32) {
        self.at(&DOT, x, y, tint, alpha);
    }
    /// Blend the expression toward `e` by `k`.
    pub fn set(&mut self, e: &Expr, k: f32) {
        self.e = Expr::lerp(self.e, *e, k);
    }
    /// Blend toward `e` as `q` runs from `a` to `b`.
    pub fn hold(&mut self, e: &Expr, a: f32, b: f32) {
        let k = seg(self.q, a, b);
        self.e = Expr::lerp(self.e, *e, k);
    }
}

pub struct ActDef {
    pub kind: ActKind,
    pub dur: f32,
    pub mood: Mood,
    pub icon: Option<&'static Sprite>,
    pub tint: Option<Color>,
    pub rise: bool,
    pub body: fn(&mut Frame),
}

/// Where in the envelope `p` (0..1 of `dur`) sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// 0 → 1 over the in phase, 1 → 0 over the out phase.
    pub env: f32,
    /// Body progress between the two.
    pub q: f32,
    pub inn: f32,
    pub out: f32,
}

pub fn envelope(p: f32, dur: f32) -> Env {
    let a = IN_S / dur;
    let inn = seg(p, 0.0, a);
    let out = seg(p, 1.0 - a, 1.0);
    Env {
        env: inn.min(1.0 - out),
        q: seg(p, a, 1.0 - a),
        inn,
        out,
    }
}

/// One frame of `def` at progress `p`, starting from the expression `held`
/// (the rest pose before the act) and the idle gaze. At `p = 0` and `p = 1`
/// the result is at rest: no offset, rotation, sprite or visible icon.
pub fn run_act(def: &ActDef, p: f32, held: Expr, idle_gaze: (f32, f32)) -> Frame {
    let env = envelope(p, def.dur);
    let slot_y = SLOT_Y + (1.0 - env.env) * 8.0;
    let base_e = Expr::lerp(held, Expr::CONTENT, env.inn);
    let base_off = (0.0, if def.rise { -RISE_PX * env.env } else { 0.0 });
    let base_gaze = (
        idle_gaze.0 * (1.0 - env.env),
        lerp(idle_gaze.1, 0.9, env.env),
    );
    let mut f = Frame {
        q: env.q,
        t: p * def.dur,
        e: base_e,
        off: base_off,
        rot: 0.0,
        gaze: base_gaze,
        eyes: [EyeOv::default(); 2],
        sprite: None,
        icon_alpha: 1.0,
        icon_frame: None,
        placed: Vec::new(),
        post: Post::default(),
        slot_y,
    };
    (def.body)(&mut f);
    // in phase: fade the body's contribution in, so p = 0 is the rest pose
    let k = env.inn;
    f.e = Expr::lerp(base_e, f.e, k);
    f.off = (lerp(base_off.0, f.off.0, k), lerp(base_off.1, f.off.1, k));
    f.rot *= k;
    f.gaze = (lerp(base_gaze.0, f.gaze.0, k), lerp(base_gaze.1, f.gaze.1, k));
    f.post.flicker = lerp(1.0, f.post.flicker, k);
    f.post.rim *= k;
    for pl in &mut f.placed {
        pl.alpha *= k;
    }
    if k < 0.5 {
        f.sprite = None;
        f.eyes = [EyeOv::default(); 2];
        f.post.overlay = Overlay::None;
    }
    if let Some(icon) = def.icon {
        f.placed.insert(
            0,
            Placed {
                sprite: f.icon_frame.unwrap_or(icon),
                x: SLOT_X,
                y: slot_y,
                tint: def.tint,
                alpha: f.icon_alpha,
            },
        );
    }
    // out phase: everything settles into the act's mood
    let o = env.out;
    f.e = Expr::lerp(f.e, Expr::of(def.mood), o);
    if o > 0.0 {
        f.sprite = None;
        f.eyes = [EyeOv::default(); 2];
        f.rot *= 1.0 - o;
        // decay the body's own offset and gaze; the envelope baseline (rise,
        // look-down) already follows `env`, so the face and the icon settle together
        f.off = (
            base_off.0 + (f.off.0 - base_off.0) * (1.0 - o),
            base_off.1 + (f.off.1 - base_off.1) * (1.0 - o),
        );
        f.gaze = (
            base_gaze.0 + (f.gaze.0 - base_gaze.0) * (1.0 - o),
            base_gaze.1 + (f.gaze.1 - base_gaze.1) * (1.0 - o),
        );
        f.post.flicker = lerp(f.post.flicker, 1.0, o);
        f.post.rim *= 1.0 - o;
        let first_is_icon = def.icon.is_some();
        for (i, pl) in f.placed.iter_mut().enumerate() {
            if !(first_is_icon && i == 0) {
                pl.alpha *= 1.0 - o;
            }
        }
        if o >= 0.5 {
            f.post.overlay = Overlay::None;
        }
    }
    f
}

fn blink(t: f32, hz: f32, low: f32) -> f32 {
    if (t * hz).sin() > 0.0 {
        1.0
    } else {
        low
    }
}

// ---------------- pods ----------------

fn oh_hi(f: &mut Frame) {
    if f.q < 0.5 {
        f.e.open = 1.25;
    }
}

fn ouch(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let fall = seg(q, 0.0, 0.3);
    if q < 0.3 {
        f.at(&BOX, SLOT_X, -7.0 + fall * 12.0, Some(T_K8S), 1.0);
    }
    if inseg(q, 0.3, 0.42) {
        let b = bump(q, 0.3, 0.42);
        f.off.0 -= 6.0 * b;
        f.off.1 += 8.0 * b;
    }
    if inseg(q, 0.3, 0.85) {
        f.sprite = Some(both(&XEYE));
    }
    if q >= 0.3 {
        let s = seg(q, 0.3, 0.55);
        f.at(&BOX, SLOT_X, 5.0 + s * (SLOT_Y - 5.0), Some(T_K8S), 1.0);
        if s >= 1.0 {
            f.at(&CROSS, SLOT_X + 1.0, SLOT_Y + 1.0, Some(T_ALERT), blink(t, 12.0, 0.2));
        }
    }
    f.hold(&Expr::WORRIED, 0.85, 1.0);
}

fn bye(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 1.0 - seg(q, 0.3, 0.8);
    if inseg(q, 0.4, 0.55) {
        f.e.open = 0.08;
    }
    if inseg(q, 0.7, 0.95) {
        f.off.1 -= 6.0 * bump(q, 0.7, 0.95);
    }
}

// ---------------- nodes ----------------

fn lost_one(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.over(&CROSS, 1.0, 0.0, Some(T_ALERT), blink(t, 12.0, 0.15));
    f.hold(&Expr::SAD, 0.1, 0.4);
    if q > 0.4 {
        let d = (t * 0.7).fract();
        f.dot(5.0, 13.0 + d * 4.0, Some(T_TORRENT), 1.0 - d * 0.5);
    }
}

fn its_back(f: &mut Frame) {
    let q = f.q;
    f.over(&CHECK, 0.0, -1.0, Some(T_LONGHORN), out_back(seg(q, 0.05, 0.3)));
    f.hold(&Expr::HAPPY, 0.1, 0.3);
    if inseg(q, 0.3, 0.8) {
        f.off.1 -= (seg(q, 0.3, 0.8) * PI * 2.0).sin().abs() * 10.0;
    }
}

// ---------------- github ----------------

fn catch(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.05, 0.55);
    let gx = 26.0 - s * 14.5;
    let gy = 6.0 + (s * PI).sin() * -4.0 + s * 4.0;
    if s < 1.0 {
        f.dot(gx, gy, Some(T_GITHUB), 1.0);
        f.gaze = ((gx - 12.0) / 12.0, (gy - 9.0) / 10.0);
    }
    if inseg(q, 0.55, 0.7) {
        let b = bump(q, 0.55, 0.7);
        let fade = 1.0 - seg(q, 0.6, 0.7);
        for k in 0..4 {
            let a = k as f32 * 1.57;
            f.dot(11.5 + a.cos() * b * 2.0, 10.0 + a.sin() * b * 2.0, Some(T_UPS), fade);
        }
        f.e.open = 1.2;
    }
    f.hold(&Expr::HAPPY, 0.6, 0.75);
    f.hold(&Expr::EXCITED, 0.8, 1.0);
}

fn starry_eyes(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.05, 0.85) {
        f.sprite = Some(both(&STAR));
        for i in 0..6 {
            let a = i as f32 * 1.05 + q * 2.0;
            if (t * 9.0 + i as f32 * 2.0).sin() > 0.3 {
                f.dot(11.5 + a.cos() * 10.0, 11.5 + a.sin() * 10.0, Some(T_UPS), 1.0);
            }
        }
    }
    f.hold(&Expr::EXCITED, 0.85, 1.0);
}

fn merge(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.0, 0.5));
    if q < 0.5 {
        f.dot(1.0 + s * 10.0, 9.0, Some(T_GITHUB), 1.0);
        f.dot(22.0 - s * 10.0, 9.0, Some(T_GITHUB), 1.0);
    } else {
        f.at(&COIN, 11.0, 8.5, Some(T_GITHUB), 1.0 - seg(q, 0.8, 1.0));
    }
    f.gaze = (0.0, 0.2);
    if inseg(q, 0.5, 0.85) {
        f.off.1 += (seg(q, 0.5, 0.85) * PI * 4.0).sin() * 4.0;
    }
    f.hold(&Expr::HAPPY, 0.6, 0.85);
}

fn party(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.set(&Expr::WOW, seg(q, 0.0, 0.15));
    f.post.overlay = Overlay::Confetti(t);
    f.off.1 -= (t * 8.0).sin().abs() * 7.0 * (1.0 - seg(q, 0.8, 1.0));
    f.hold(&Expr::EXCITED, 0.8, 1.0);
}

fn ding(f: &mut Frame) {
    let q = f.q;
    f.post.rim = 0.6 * bump(q, 0.0, 0.3);
    if inseg(q, 0.2, 0.7) {
        f.eyes[0].open = Some(0.06);
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 1.0);
}

fn eye_roll(f: &mut Frame) {
    let q = f.q;
    f.gaze = (0.0, lerp(0.9, -1.2, seg(q, 0.1, 0.35)));
    if inseg(q, 0.1, 0.75) {
        f.e.open = 0.4;
        f.e.tilt = 0.4;
        f.e.lower = 0.3;
    }
    f.hold(&Expr::WORRIED, 0.75, 1.0);
}

fn level_up(f: &mut Frame) {
    let q = f.q;
    for k in 0..3 {
        let kk = k as f32;
        f.at(&STARLET, 4.0 + kk * 6.0, 0.0, Some(T_UPS), seg(q, 0.05 + kk * 0.12, 0.15 + kk * 0.12));
    }
    f.gaze = (0.0, lerp(0.9, -0.8, seg(q, 0.05, 0.2)));
    f.hold(&Expr::HAPPY, 0.3, 0.45);
    f.hold(&Expr::EXCITED, 0.85, 1.0);
}
```

- [ ] **Step 4: Run, fix, commit**

Run: `cargo test -p rackscreen-core face_acts::` then `cargo fmt --all` and clippy.
Expected: 5 passed. If `every_act_starts_and_ends_at_rest` fails for one act, the fix belongs in `run_act` (the envelope), not in the body, unless the body writes `f.off.1 = ...` (assignment instead of `+=`), which is the one thing bodies must not do.

```bash
git add crates/core/src/face_acts.rs crates/core/src/lib.rs
git commit -m "feat(core): act engine with the pod, node and github acts

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Argo, torrent, UPS, alert, heat, Longhorn and link acts

**Files:**
- Modify: `crates/core/src/face_acts.rs`

**Interfaces:**
- Produces: `ActKind::{Launch, Grump, Wink, Incoming, GotIt, LightsFlicker, Phew, OnFumes, Alarm, AllClear, TooHot, WorkingHard, Stuffed, Hmm, DisksFine, SoFull, Hello, FoundYou}` in `ALL` (30 total); `is_severe` adds `LightsFlicker`, `Hello`.

- [ ] **Step 1: Extend the tests**

Add to `face_acts::tests`:

```rust
    #[test]
    fn task_six_acts_exist_with_their_sources() {
        assert_eq!(ActKind::ALL.len(), 30);
        let d = ActKind::Launch.def();
        assert!(std::ptr::eq(d.icon.unwrap(), &ROCKET) && d.tint == Some(T_ARGO) && d.mood == Mood::Happy);
        let d = ActKind::SoFull.def();
        assert!(std::ptr::eq(d.icon.unwrap(), &DISK));
        let f = run_act(&d, 0.6, Expr::CONTENT, (0.0, 0.0));
        assert!(f.placed[0].sprite.rows[6] == "#######", "disk fills from the bottom");
        assert!(ActKind::LightsFlicker.is_severe() && ActKind::Hello.is_severe());
        let f = run_act(&ActKind::LightsFlicker.def(), 0.15, Expr::CONTENT, (0.0, 0.0));
        assert!(f.post.flicker < 0.5 || f.post.flicker > 0.9, "flicker toggles");
        let f = run_act(&ActKind::Alarm.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.post.rim > 0.4);
        let f = run_act(&ActKind::GotIt.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &SQZ_L) && std::ptr::eq(s[1], &SQZ_R)));
        let f = run_act(&ActKind::Hmm.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.eyes[0].open.unwrap() < f.eyes[1].open.unwrap(), "suspicious: left narrow, right wide");
        assert!(f.rot < 0.0);
    }
```

Run: `cargo test -p rackscreen-core face_acts::task_six` → compile error.

- [ ] **Step 2: Add the kinds and bodies**

Extend the enum (after `LevelUp`):

```rust
    // argo cd
    Launch,
    Grump,
    Wink,
    // qbittorrent
    Incoming,
    GotIt,
    // ups
    LightsFlicker,
    Phew,
    OnFumes,
    // alerts
    Alarm,
    AllClear,
    // heat and load
    TooHot,
    WorkingHard,
    Stuffed,
    // longhorn
    Hmm,
    DisksFine,
    SoFull,
    // link
    Hello,
    FoundYou,
```

Extend `ALL` with the same 18 in the same order after `ActKind::LevelUp`. Change `is_severe` to:

```rust
        matches!(
            self,
            ActKind::Ouch | ActKind::LostOne | ActKind::LightsFlicker | ActKind::Hello
        )
```

Add to the `def` match:

```rust
            ActKind::Launch => d(2.8, Mood::Happy, Some(&ROCKET), Some(T_ARGO), launch),
            ActKind::Grump => d(2.6, Mood::Angry, Some(&TRI), Some(T_ALERT), grump),
            ActKind::Wink => d(2.2, Mood::Happy, Some(&ROCKET), Some(T_ARGO), wink),
            ActKind::Incoming => d(2.8, Mood::Content, Some(&DOWNLOAD), Some(T_TORRENT), incoming),
            ActKind::GotIt => d(3.2, Mood::Happy, Some(&DOWNLOAD), Some(T_TORRENT), got_it),
            ActKind::LightsFlicker => d(3.0, Mood::Scared, Some(&BOLT), Some(T_UPS), lights_flicker),
            ActKind::Phew => d(2.8, Mood::Content, Some(&BOLT), Some(T_UPS), phew),
            ActKind::OnFumes => d(3.0, Mood::Worried, Some(&BATLOW), Some(T_UPS), on_fumes),
            ActKind::Alarm => d(3.0, Mood::Angry, Some(&BELL), Some(T_ALERT), alarm),
            ActKind::AllClear => d(2.6, Mood::Content, Some(&BELL), Some(T_ALERT), all_clear),
            ActKind::TooHot => d(3.0, Mood::Hot, Some(&FLAME), Some(T_PROM), too_hot),
            ActKind::WorkingHard => d(2.8, Mood::Hot, Some(&CPU), Some(T_PROM), working_hard),
            ActKind::Stuffed => d(2.8, Mood::Hot, Some(&MEM), Some(T_MEM), stuffed),
            ActKind::Hmm => d(2.8, Mood::Angry, Some(&DISK), Some(T_LONGHORN), hmm),
            ActKind::DisksFine => d(2.2, Mood::Content, Some(&DISK), Some(T_LONGHORN), disks_fine),
            ActKind::SoFull => d(3.0, Mood::Worried, Some(&DISK), Some(T_LONGHORN), so_full),
            ActKind::Hello => d(3.2, Mood::Worried, Some(&CLOUD), Some(T_K8S), hello),
            ActKind::FoundYou => d(2.2, Mood::Happy, Some(&CLOUD), Some(T_K8S), found_you),
```

Add the bodies at the end of the file (before the tests):

```rust
fn sweat(f: &mut Frame, gx: f32, speed: f32) {
    let d = (f.t * speed).fract();
    f.dot(gx, 1.0 + d * 6.0, Some(T_TORRENT), 1.0 - d * 0.3);
}

// ---------------- argo cd ----------------

fn launch(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let s = ease(seg(q, 0.1, 0.7));
    f.icon_alpha = 0.0;
    let gy = f.slot_y - s * 26.0;
    f.at(&ROCKET, SLOT_X, gy, Some(T_ARGO), 1.0);
    for k in 1..4 {
        if (t * 25.0 + k as f32).sin() > 0.0 {
            let side = if k % 2 == 1 { -0.5 } else { 0.5 };
            f.dot(SLOT_X + 3.0 + side * (k as f32 - 1.0), gy + 7.0 + k as f32, Some(T_UPS), 1.0);
        }
    }
    f.gaze = (0.0, lerp(0.9, -1.0, s));
    if inseg(q, 0.1, 0.6) {
        f.e.open = 1.25;
    }
    f.hold(&Expr::HAPPY, 0.65, 0.9);
}

fn grump(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_alpha = blink(t, 10.0, 0.15);
    f.hold(&Expr::ANGRY, 0.0, 0.2);
    if inseg(q, 0.3, 0.55) {
        f.off.0 += (seg(q, 0.3, 0.55) * PI * 3.0).sin() * 6.0;
    }
}

fn wink(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_alpha = 0.7 + 0.3 * (t * 8.0).sin();
    if inseg(q, 0.1, 0.7) {
        f.eyes[1].open = Some(0.06);
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 1.0);
}

// ---------------- qbittorrent ----------------

fn incoming(f: &mut Frame) {
    let q = f.q;
    let n = (seg(q, 0.1, 0.9) * 12.0).floor() as usize;
    for k in 0..12 {
        f.dot(6.0 + k as f32, 15.5, Some(T_TORRENT), if k < n { 1.0 } else { 0.25 });
    }
    f.gaze = (-0.6 + seg(q, 0.1, 0.9) * 1.2, 0.9);
    f.set(&Expr::FOCUS, 1.0);
    f.off.1 += 3.0;
}

fn got_it(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.0, 0.35);
    f.at(&COIN, 11.0, -2.0 + s * 11.0, Some(T_TORRENT), 1.0 - seg(q, 0.75, 0.85));
    f.gaze = if s < 1.0 { (0.0, -1.0 + s * 2.0) } else { (0.0, 0.1) };
    if inseg(q, 0.35, 0.7) {
        f.sprite = Some([&SQZ_L, &SQZ_R]);
        f.off.0 += (seg(q, 0.35, 0.7) * PI * 4.0).sin() * 4.0;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.9);
}

// ---------------- ups ----------------

fn lights_flicker(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if q < 0.3 {
        f.post.flicker = if (q * 60.0).sin() > -0.2 { 1.0 } else { 0.08 };
    }
    f.hold(&Expr::SCARED, 0.3, 0.5);
    if q > 0.45 {
        f.off.0 += (t * 70.0).sin() * 1.6;
    }
}

fn phew(f: &mut Frame) {
    let q = f.q;
    f.over(&CHECK, 1.0, 0.0, Some(T_LONGHORN), seg(q, 0.05, 0.2));
    f.hold(&Expr::RELIEF, 0.1, 0.3);
    if inseg(q, 0.3, 0.8) {
        f.off.1 += 6.0 * ease(seg(q, 0.3, 0.8));
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

fn on_fumes(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let on = blink(t, 8.0, 0.2);
    f.over(&DOT, 1.0, 2.0, Some(T_ALERT), on);
    f.over(&DOT, 1.0, 3.0, Some(T_ALERT), on);
    let s = seg(q, 0.1, 0.7);
    f.e.open = 1.0 - s * 0.7;
    f.off.1 += s * 6.0;
    if inseg(q, 0.6, 0.85) {
        f.e.open = 0.05 + 0.5 * (1.0 - bump(q, 0.6, 0.85));
    }
    f.hold(&Expr::WORRIED, 0.85, 1.0);
}

// ---------------- alerts ----------------

fn alarm(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_frame = Some(if (t * 14.0).sin() > 0.0 { &BELL } else { &BELL2 });
    f.post.rim = 0.45 + 0.45 * (t * 10.0).sin().max(0.0);
    f.set(&Expr::SCARED, seg(q, 0.0, 0.15));
    f.gaze = ((t * 5.0).sin() * 0.9, 0.3);
    f.hold(&Expr::ANGRY, 0.8, 1.0);
}

fn all_clear(f: &mut Frame) {
    let q = f.q;
    f.over(&CHECK, 1.0, -1.0, Some(T_LONGHORN), out_back(seg(q, 0.05, 0.3)));
    f.post.rim = 0.35 * bump(q, 0.1, 0.5);
    f.hold(&Expr::RELIEF, 0.2, 0.45);
    if inseg(q, 0.45, 0.8) {
        f.off.1 += 5.0 * bump(q, 0.45, 0.8);
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

// ---------------- heat and load ----------------

fn too_hot(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.hold(&Expr::HOT, 0.0, 0.2);
    f.off.1 += (t * 12.0).sin().abs() * 3.0;
    if inseg(q, 0.4, 0.7) {
        f.off.0 += (seg(q, 0.4, 0.7) * PI * 4.0).sin() * 5.0;
    }
    sweat(f, 19.0, 0.8);
}

fn working_hard(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.hold(&Expr::GRIMACE, 0.0, 0.2);
    if inseg(q, 0.3, 0.8) {
        f.off.0 += (t * 40.0).sin() * 1.5;
    }
    sweat(f, 5.0, 0.9);
    f.hold(&Expr::HOT, 0.8, 1.0);
}

fn stuffed(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.1, 0.5);
    if q < 0.7 {
        f.e.open = 0.2;
        f.e.scale = 1.0 + 0.18 * s;
        f.e.sep = 1.0 + 0.12 * s;
        f.e.lower = 0.5;
    }
    if inseg(q, 0.5, 0.7) {
        let r = seg(q, 0.5, 0.7);
        for k in 0..3 {
            let kk = k as f32;
            f.dot(11.0 + kk * 0.5, 15.0 + r * 2.0 + kk, None, 1.0 - r);
        }
    }
    f.hold(&Expr::HOT, 0.75, 1.0);
}

// ---------------- longhorn ----------------

fn hmm(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.over(&CROSS, 1.0, 1.0, Some(T_ALERT), blink(t, 10.0, 0.15));
    f.hold(&Expr::GRIMACE, 0.0, 0.2);
    f.eyes[0].open = Some(0.4);
    f.eyes[1].open = Some(1.15);
    let s = ease(seg(q, 0.05, 0.35)) * (1.0 - seg(q, 0.8, 1.0));
    f.rot = -0.12 * s;
    f.off.0 -= 4.0 * s;
    f.hold(&Expr::ANGRY, 0.8, 1.0);
}

fn disks_fine(f: &mut Frame) {
    let q = f.q;
    f.over(&CHECK, 1.0, 0.0, Some(T_LONGHORN), out_back(seg(q, 0.05, 0.3)));
    f.hold(&Expr::HAPPY, 0.1, 0.3);
    if inseg(q, 0.4, 0.65) {
        f.off.1 += 4.0 * bump(q, 0.4, 0.65);
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

fn so_full(f: &mut Frame) {
    let q = f.q;
    let n = (seg(q, 0.05, 0.7) * 7.0).floor() as usize;
    f.icon_frame = Some(&DISK_FILL[n.min(7)]);
    let k = n as f32 / 7.0;
    f.e.scale = 1.0 + 0.16 * k;
    f.e.open = 1.1 + 0.1 * k;
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

// ---------------- link ----------------

fn hello(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let on = (t * 8.0).sin() > 0.0;
    f.icon_alpha = if on { 0.9 } else { 0.3 };
    f.over(&CROSS, 1.0, 0.0, Some(T_ALERT), if on { 1.0 } else { 0.25 });
    f.e.open = 0.5;
    f.e.lower = 0.2;
    f.gaze = ((q * 9.0).sin() * 1.2, -0.2);
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

fn found_you(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 0.3 + 0.7 * seg(q, 0.0, 0.2);
    let b = (seg(q, 0.1, 0.45) * PI * 2.0).sin();
    if b > 0.7 {
        f.e.open = 0.05;
    }
    if inseg(q, 0.45, 0.7) {
        f.e.open = 1.3;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.95);
}
```

- [ ] **Step 3: Run, fix, commit**

Run: `cargo test -p rackscreen-core face_acts::` then fmt and clippy.
Expected: 6 passed (the rest invariant now covers 30 acts).

```bash
git add crates/core/src/face_acts.rs
git commit -m "feat(core): argo, torrent, ups, alert, heat, longhorn and link acts

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Weather habits, sky and air events, price acts, weather category

**Files:**
- Modify: `crates/core/src/face_acts.rs`

**Interfaces:**
- Produces: `ActKind::{Sunny, Raining, Windy, Snowing, Foggy, MehClouds, Heatwave, Freezing, Lightning, UhOhRain, Cough, Morning, Evening, Awoo, LookUp, KaChing, Expensive}` (47 total); `is_severe` adds `Lightning`; `is_habit` true for the eight weather acts;
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum WeatherCat { Snow, Rain, Wind, Fog, Cold, Hot, Clouds, Sunny }
  pub fn weather_cat(code: u16, temp_c: f32, gust_kmh: f32, is_day: bool) -> Option<WeatherCat>
  impl WeatherCat { pub fn act(self) -> ActKind }
  ```

- [ ] **Step 1: Extend the tests**

Add to `face_acts::tests`:

```rust
    #[test]
    fn weather_category_priority_and_codes() {
        use WeatherCat::*;
        assert_eq!(weather_cat(0, 20.0, 10.0, true), Some(Sunny));
        assert_eq!(weather_cat(1, 20.0, 10.0, true), Some(Sunny));
        assert_eq!(weather_cat(0, 20.0, 10.0, false), None, "clear night: nothing");
        assert_eq!(weather_cat(2, 20.0, 10.0, true), Some(Clouds));
        assert_eq!(weather_cat(3, 20.0, 10.0, false), Some(Clouds));
        assert_eq!(weather_cat(45, 20.0, 10.0, true), Some(Fog));
        assert_eq!(weather_cat(48, 20.0, 10.0, true), Some(Fog));
        assert_eq!(weather_cat(61, 20.0, 10.0, true), Some(Rain));
        assert_eq!(weather_cat(80, 20.0, 10.0, true), Some(Rain));
        assert_eq!(weather_cat(71, -3.0, 10.0, true), Some(Snow));
        assert_eq!(weather_cat(85, -3.0, 60.0, true), Some(Snow), "snow beats wind and cold");
        assert_eq!(weather_cat(61, 5.0, 60.0, true), Some(Rain), "rain beats wind");
        assert_eq!(weather_cat(0, 30.0, 60.0, true), Some(Wind), "wind beats heat");
        assert_eq!(weather_cat(0, 30.0, 10.0, true), Some(Hot));
        assert_eq!(weather_cat(0, 28.0, 10.0, true), Some(Hot));
        assert_eq!(weather_cat(0, 0.0, 10.0, true), Some(Cold));
        assert_eq!(weather_cat(2, -1.0, 10.0, true), Some(Cold), "cold beats clouds");
        assert_eq!(weather_cat(45, -1.0, 10.0, true), Some(Fog), "fog beats cold");
        assert_eq!(weather_cat(95, 20.0, 10.0, true), None, "thunder is an event");
        assert_eq!(Snow.act(), ActKind::Snowing);
        assert_eq!(Sunny.act(), ActKind::Sunny);
        for c in [Snow, Rain, Wind, Fog, Cold, Hot, Clouds, Sunny] {
            assert!(c.act().is_habit(), "{c:?}");
        }
    }

    #[test]
    fn task_seven_acts_exist() {
        assert_eq!(ActKind::ALL.len(), 47);
        assert!(ActKind::Lightning.is_severe());
        assert!(!ActKind::Lightning.is_habit());
        let f = run_act(&ActKind::Sunny.def(), 0.6, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &SHADE)));
        let f = run_act(&ActKind::Foggy.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!((f.post.flicker - 0.5).abs() < 1e-3);
        assert!(matches!(f.post.overlay, Overlay::FogBand(_)));
        let f = run_act(&ActKind::Lightning.def(), 0.13, Expr::CONTENT, (0.0, 0.0));
        assert!(f.post.flicker > 1.5 || f.post.flicker < 1.2, "flash or not, never nan");
        let f = run_act(&ActKind::KaChing.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &EUROEYE)));
        let f = run_act(&ActKind::Morning.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.placed.iter().any(|p| std::ptr::eq(p.sprite, &SUN) && p.y < SLOT_Y - 2.0), "sun rises out of the slot");
    }
```

Run: `cargo test -p rackscreen-core face_acts::` → compile error.

- [ ] **Step 2: Add kinds, category, bodies**

Extend the enum after `FoundYou`:

```rust
    // weather habits
    Sunny,
    Raining,
    Windy,
    Snowing,
    Foggy,
    MehClouds,
    Heatwave,
    Freezing,
    // sky and air events
    Lightning,
    UhOhRain,
    Cough,
    Morning,
    Evening,
    Awoo,
    LookUp,
    // prices
    KaChing,
    Expensive,
```

Extend `ALL` with the same 17 in order. `is_severe`: add `| ActKind::Lightning`. Replace `is_habit`:

```rust
    pub fn is_habit(self) -> bool {
        matches!(
            self,
            ActKind::Sunny
                | ActKind::Raining
                | ActKind::Windy
                | ActKind::Snowing
                | ActKind::Foggy
                | ActKind::MehClouds
                | ActKind::Heatwave
                | ActKind::Freezing
        )
    }
```

Add to the `def` match:

```rust
            ActKind::Sunny => d(3.2, Mood::Content, Some(&SUN), Some(T_SKY), sunny),
            ActKind::Raining => d(3.2, Mood::Content, Some(&CLOUD), Some(T_WX), raining),
            ActKind::Windy => d(3.2, Mood::Content, Some(&WIND), Some(T_WIND), windy),
            ActKind::Snowing => d(3.6, Mood::Content, Some(&FLAKE), Some(T_SNOW), snowing),
            ActKind::Foggy => d(3.4, Mood::Content, Some(&FOG), Some(T_WIND), foggy),
            ActKind::MehClouds => d(3.2, Mood::Content, Some(&CLOUD), Some(T_WIND), meh_clouds),
            ActKind::Heatwave => d(3.2, Mood::Content, Some(&THERM), Some(T_PROM), heatwave),
            ActKind::Freezing => d(3.0, Mood::Content, Some(&FLAKE), Some(T_SNOW), freezing),
            ActKind::Lightning => d(3.0, Mood::Scared, Some(&BOLT), Some(T_WX), lightning),
            ActKind::UhOhRain => d(3.0, Mood::Content, Some(&CLOUD), Some(T_WX), uh_oh_rain),
            ActKind::Cough => d(2.6, Mood::Worried, Some(&WIND), Some(T_WIND), cough),
            ActKind::Morning => d(3.2, Mood::Content, Some(&SUN), Some(T_SKY), morning),
            ActKind::Evening => d(3.0, Mood::Content, Some(&MOON), Some(T_ISS), evening),
            ActKind::Awoo => d(3.2, Mood::Content, Some(&MOON), Some(T_ISS), awoo),
            ActKind::LookUp => d(3.4, Mood::Content, Some(&SAT), Some(T_ISS), look_up),
            ActKind::KaChing => d(2.8, Mood::Happy, Some(&EURO), Some(T_PRICE), ka_ching),
            ActKind::Expensive => d(2.8, Mood::Content, Some(&EURO), Some(T_PRICE), expensive),
```

Add the category after the `ActKind` impl:

```rust
/// Current-conditions category from the WMO code, temperature and gusts.
/// Thunder (95..=99) is an event, so it returns `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherCat {
    Snow,
    Rain,
    Wind,
    Fog,
    Cold,
    Hot,
    Clouds,
    Sunny,
}

pub fn weather_cat(code: u16, temp_c: f32, gust_kmh: f32, is_day: bool) -> Option<WeatherCat> {
    if matches!(code, 71..=77 | 85 | 86) {
        return Some(WeatherCat::Snow);
    }
    if matches!(code, 51..=67 | 80..=82) {
        return Some(WeatherCat::Rain);
    }
    if gust_kmh >= 50.0 {
        return Some(WeatherCat::Wind);
    }
    if matches!(code, 45 | 48) {
        return Some(WeatherCat::Fog);
    }
    if temp_c <= 0.0 {
        return Some(WeatherCat::Cold);
    }
    if temp_c >= 28.0 {
        return Some(WeatherCat::Hot);
    }
    if matches!(code, 2 | 3) {
        return Some(WeatherCat::Clouds);
    }
    if matches!(code, 0 | 1) && is_day {
        return Some(WeatherCat::Sunny);
    }
    None
}

impl WeatherCat {
    pub fn act(self) -> ActKind {
        match self {
            WeatherCat::Snow => ActKind::Snowing,
            WeatherCat::Rain => ActKind::Raining,
            WeatherCat::Wind => ActKind::Windy,
            WeatherCat::Fog => ActKind::Foggy,
            WeatherCat::Cold => ActKind::Freezing,
            WeatherCat::Hot => ActKind::Heatwave,
            WeatherCat::Clouds => ActKind::MehClouds,
            WeatherCat::Sunny => ActKind::Sunny,
        }
    }
}
```

Bodies (append before the tests):

```rust
// ---------------- weather habits ----------------

fn sunny(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.0, 0.35) {
        f.e.open = 0.3;
        f.e.lower = 0.3;
    }
    if inseg(q, 0.35, 0.9) {
        f.sprite = Some(both(&SHADE));
    }
    f.gaze = (0.0, lerp(0.9, 0.0, seg(q, 0.3, 0.4)));
    f.hold(&Expr::HAPPY, 0.9, 1.0);
}

fn raining(f: &mut Frame) {
    let t = f.t;
    for k in 0..7 {
        let kk = k as f32;
        let fr = (t * 1.2 + kk * 0.14).fract();
        f.dot(2.0 + kk * 3.2 + (k % 2) as f32, -1.0 + fr * 18.0, Some(T_WX), 0.9 * (1.0 - fr * 0.3));
    }
    f.gaze = (0.2, -1.0);
    f.e.open = 0.55 + if (t * 6.0).sin() > 0.6 { -0.45 } else { 0.0 };
}

fn windy(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..8 {
        let kk = k as f32;
        let fr = (t * 1.6 + kk * 0.125).fract();
        f.dot(24.0 - fr * 26.0, 1.0 + ((k * 5) % 15) as f32 + (fr * 6.0).sin() * 0.6, Some(T_WIND), 0.8);
    }
    let s = seg(q, 0.0, 0.3) * (1.0 - seg(q, 0.85, 1.0));
    f.rot = 0.14 * s;
    f.off.0 -= 6.0 * s;
    f.e.open = 1.0 - 0.7 * s;
}

fn snowing(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..5 {
        let kk = k as f32;
        let fr = (t * 0.35 + kk / 5.0).fract();
        f.at(&COIN, 1.0 + ((k * 7) % 20) as f32 + (fr * 7.0 + kk).sin() * 1.2, -2.0 + fr * 19.0, Some(T_SNOW), 0.9);
    }
    f.off.0 += (t * 45.0).sin() * 0.6;
    let fr = seg(q, 0.05, 0.4);
    if q < 0.8 {
        f.at(&COIN, 16.0, -2.0 + fr * 7.0, Some(T_SNOW), 1.0);
    }
    if inseg(q, 0.4, 0.75) {
        f.gaze = (0.5, -1.0);
        f.eyes[0] = EyeOv {
            open: Some(1.1),
            tilt: Some(-0.3),
            scale: None,
        };
        f.eyes[1].open = Some(0.8);
    }
    if inseg(q, 0.75, 0.9) {
        f.off.0 += (seg(q, 0.75, 0.9) * PI * 3.0).sin() * 6.0;
    }
}

fn foggy(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.post.flicker = 1.0 - 0.5 * seg(q, 0.0, 0.2).min(1.0 - seg(q, 0.85, 1.0));
    f.post.overlay = Overlay::FogBand(t);
    f.e.open = 0.4;
    f.e.lower = 0.2;
    f.gaze = ((t * 2.0).sin() * 0.8, -0.2);
}

fn meh_clouds(f: &mut Frame) {
    let q = f.q;
    f.at(&CLOUD, -8.0 + q * 26.0, 0.0, Some(T_WIND), 1.0);
    f.set(&Expr::MEH, seg(q, 0.0, 0.2));
    f.gaze = (lerp(-1.0, 1.0, q), -0.9);
    if inseg(q, 0.7, 0.9) {
        f.off.1 -= 5.0 * bump(q, 0.7, 0.9);
    }
}

fn heatwave(f: &mut Frame) {
    let t = f.t;
    f.e.open = 0.55;
    f.e.lift = 3.0;
    f.off.1 += (t * 12.0).sin().abs() * 3.0;
    sweat(f, 19.0, 0.8);
    for k in 0..3 {
        let kk = k as f32;
        let fr = (t * 0.7 + kk / 3.0).fract();
        f.dot(8.0 + kk * 4.0 + (fr * 12.0).sin() * 0.7, 3.0 - fr * 4.0, Some(T_PROM), (1.0 - fr) * 0.7);
    }
}

fn freezing(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let s = seg(q, 0.0, 0.2) * (1.0 - seg(q, 0.85, 1.0));
    f.off.0 += (t * 60.0).sin() * 2.2 * s;
    f.e.open = 1.0 - 0.85 * s;
    f.e.scale = 1.0 - 0.15 * s;
    f.e.sep = 1.0 - 0.15 * s;
    f.e.lower = 0.3 * s;
}

// ---------------- sky and air events ----------------

fn lightning(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.02, 0.08) || inseg(q, 0.14, 0.19) {
        f.post.flicker = 3.0;
    }
    if inseg(q, 0.02, 0.35) {
        f.off.1 -= 10.0 * bump(q, 0.02, 0.35);
        f.e.open = 1.4;
    }
    if inseg(q, 0.35, 0.85) {
        f.e.open = 0.05;
        f.off.0 += (t * 60.0).sin() * 1.5;
    }
    f.hold(&Expr::SCARED, 0.85, 1.0);
}

fn uh_oh_rain(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..4 {
        let kk = k as f32;
        let fr = (t + kk * 0.25).fract();
        f.dot(4.0 + kk * 5.0, -1.0 + fr * 12.0, Some(T_WX), 0.9 * (1.0 - fr * 0.3) * seg(q, 0.2, 0.4));
    }
    f.gaze = (0.2, lerp(0.9, -1.0, seg(q, 0.1, 0.3)));
    if inseg(q, 0.3, 0.9) {
        f.e.open = 0.5;
    }
}

fn cough(f: &mut Frame) {
    let q = f.q;
    for (a, b) in [(0.1, 0.35), (0.45, 0.7)] {
        if inseg(q, a, b) {
            let s = seg(q, a, b);
            f.off.1 += 8.0 * (s * PI).sin();
            f.e.open = 0.08;
            for k in 0..3 {
                let kk = k as f32;
                f.dot(11.0 + kk + s * 3.0, 15.0 + kk * 0.5 + s * 2.0, None, 1.0 - s);
            }
        }
    }
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

fn morning(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 0.0;
    let s = ease(seg(q, 0.0, 0.6));
    let y = f.slot_y - s * 8.0;
    f.at(&SUN, SLOT_X, y, Some(T_SKY), 1.0);
    f.gaze = (0.0, 0.9 - s * 0.6);
    if inseg(q, 0.2, 0.7) {
        f.e.open = 0.3;
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.9);
}

fn evening(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.3, 0.65) {
        let r = bump(q, 0.3, 0.65);
        f.e.open = 1.0 - 0.95 * r;
        f.rot = -0.1 * r;
        f.off.1 -= 4.0 * r;
    }
    if q >= 0.65 {
        f.e.open = 0.5;
        f.e.lower = 0.25;
    }
}

fn awoo(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.15, 0.85) {
        f.off.1 -= 6.0;
        f.rot = -0.15;
        f.e.open = 0.08;
        for i in 0..3 {
            let ii = i as f32;
            let r = (t * 0.6 + ii / 3.0).fract();
            f.at(&NOTE, 9.0 + r * 3.0 + ii, 8.0 - r * 8.0 - ii, Some(T_SKY), 1.0 - r);
        }
    }
}

fn look_up(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.1, 0.9);
    let gx = -1.0 + s * 25.0;
    let gy = 3.0 - (s * PI).sin() * 3.0;
    f.dot(gx, gy, Some(T_ISS), 1.0);
    f.gaze = ((gx - 12.0) / 10.0, -1.0);
    if inseg(q, 0.15, 0.85) {
        f.e.open = 1.2;
    }
}

// ---------------- prices ----------------

fn ka_ching(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.05, 0.75) {
        f.sprite = Some(both(&EUROEYE));
    }
    let s = seg(q, 0.05, 0.6);
    f.at(&COIN, -2.0 + s * 8.0, 19.0 - (s * PI * 3.0).sin().abs() * (5.0 - s * 4.0), Some(T_PRICE), 1.0);
    f.hold(&Expr::HAPPY, 0.75, 1.0);
}

fn expensive(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.1, 0.7));
    if s < 1.0 {
        f.at(&COIN, SLOT_X - 1.0 - s * 10.0, 19.0, Some(T_PRICE), 1.0);
    }
    f.gaze = (lerp(0.0, -1.4, s), 0.9);
    f.eyes[0].open = Some(0.5);
    f.eyes[1].open = Some(1.1);
}
```

- [ ] **Step 3: Run, fix, commit**

Run: `cargo test -p rackscreen-core face_acts::` then fmt and clippy. Expected: 8 passed.

```bash
git add crates/core/src/face_acts.rs
git commit -m "feat(core): weather habits, sky, air and price acts

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Boot and idle habits

**Files:**
- Modify: `crates/core/src/face_acts.rs`

**Interfaces:**
- Produces: `ActKind::{WakeUp, Sneeze, Humming, Peek, Stretch, Scanning, Hic, DozingOff, Dizzy}` (56 total); `is_habit` true for the seven idle habits (`Sneeze, Humming, Peek, Stretch, Scanning, Hic, DozingOff`), false for `WakeUp` and `Dizzy`; `pub const IDLE_HABITS: &[(ActKind, u32)]` with weights; `ActKind::is_idle_habit(self) -> bool`.

- [ ] **Step 1: Extend the tests**

```rust
    #[test]
    fn task_eight_acts_complete_the_catalogue() {
        assert_eq!(ActKind::ALL.len(), 56);
        assert!(!ActKind::WakeUp.is_habit() && !ActKind::Dizzy.is_habit());
        assert!(ActKind::Sneeze.is_habit() && ActKind::DozingOff.is_habit());
        assert_eq!(IDLE_HABITS.len(), 7);
        assert_eq!(IDLE_HABITS.iter().map(|(_, w)| w).sum::<u32>(), 18);
        for (k, _) in IDLE_HABITS {
            assert!(k.is_idle_habit() && k.def().icon.is_none() && !k.def().rise, "{k:?}");
        }
        assert!(!ActKind::Sunny.is_idle_habit());
        let d = ActKind::WakeUp.def();
        assert!(d.icon.is_none() && !d.rise && (d.dur - 3.6).abs() < 1e-6);
        let f = run_act(&d, 0.2, Expr::CONTENT, (0.0, 0.0));
        assert!(f.e.open < 0.4, "asleep at the start: {}", f.e.open);
        assert!(f.placed.iter().any(|p| std::ptr::eq(p.sprite, &Z)));
        let f = run_act(&ActKind::Dizzy.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &SPIRAL1) || std::ptr::eq(s[0], &SPIRAL2)));
        let f = run_act(&ActKind::Peek.def(), 0.45, Expr::CONTENT, (0.0, 0.0));
        assert!(f.off.0 > 15.0, "leans to the edge: {}", f.off.0);
    }
```

Run → compile error.

- [ ] **Step 2: Add the kinds and bodies**

Extend the enum after `Expensive`:

```rust
    // boot and idle habits
    WakeUp,
    Sneeze,
    Humming,
    Peek,
    Stretch,
    Scanning,
    Hic,
    DozingOff,
    Dizzy,
```

Extend `ALL` in the same order. Update `is_habit` to also match `ActKind::Sneeze | ActKind::Humming | ActKind::Peek | ActKind::Stretch | ActKind::Scanning | ActKind::Hic | ActKind::DozingOff`. Add:

```rust
    pub fn is_idle_habit(self) -> bool {
        IDLE_HABITS.iter().any(|(k, _)| *k == self)
    }
```

and, after the `impl ActKind` block:

```rust
/// Idle habits with their pick weights. `DozingOff` is only eligible while bored.
pub const IDLE_HABITS: &[(ActKind, u32)] = &[
    (ActKind::Sneeze, 1),
    (ActKind::Humming, 3),
    (ActKind::Peek, 3),
    (ActKind::Stretch, 3),
    (ActKind::Scanning, 3),
    (ActKind::Hic, 1),
    (ActKind::DozingOff, 4),
];
```

`def` arms:

```rust
            ActKind::WakeUp => d(3.6, Mood::Content, None, None, wake_up),
            ActKind::Sneeze => d(2.4, Mood::Content, None, None, sneeze),
            ActKind::Humming => d(3.0, Mood::Content, None, None, humming),
            ActKind::Peek => d(2.6, Mood::Content, None, None, peek),
            ActKind::Stretch => d(2.6, Mood::Content, None, None, stretch),
            ActKind::Scanning => d(3.4, Mood::Content, None, None, scanning),
            ActKind::Hic => d(1.4, Mood::Content, None, None, hic),
            ActKind::DozingOff => d(3.6, Mood::Sleepy, None, None, dozing_off),
            ActKind::Dizzy => d(3.2, Mood::Worried, None, None, dizzy),
```

Bodies:

```rust
// ---------------- boot and idle habits ----------------

fn wake_up(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.set(&Expr::SLEEPY, 1.0);
    if q < 0.35 {
        let fade = 1.0 - seg(q, 0.25, 0.35);
        for i in 0..3 {
            let ii = i as f32;
            let r = (t * 0.5 + ii / 3.0).fract();
            f.at(&Z, 17.0 + r * 3.0 + ii * 1.5, 7.0 - r * 4.0 - ii, None, (1.0 - r) * fade);
        }
    }
    if inseg(q, 0.35, 0.7) {
        let r = bump(q, 0.35, 0.7);
        f.e.open = 0.06;
        f.e.sep = 1.0 + 0.15 * r;
        f.e.scale = 1.0 + 0.1 * r;
        f.rot = -0.1 * r;
        f.off.1 -= r * 4.0;
    }
    if q >= 0.7 {
        let r = seg(q, 0.7, 1.0);
        f.set(&Expr::CONTENT, r);
        if (r * PI * 4.0).sin() > 0.8 {
            f.e.open = 0.05;
        }
    }
}

fn sneeze(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.05, 0.5) {
        let s = seg(q, 0.05, 0.5);
        f.e.open = 0.6 - s * 0.5;
        f.e.tilt = -0.4 * s;
        f.off.1 -= s * 8.0;
        f.rot = -s * 0.12;
    }
    if inseg(q, 0.5, 0.8) {
        let s = seg(q, 0.5, 0.8);
        f.off.1 += 10.0 * (1.0 - s);
        f.rot = 0.15 * (1.0 - s);
        f.e.open = 0.06;
        for k in 0..4 {
            let kk = k as f32;
            f.dot(9.0 + kk * 1.5 + s * 5.0, 17.0 + kk * 0.5 + s * 3.0, None, 1.0 - s);
        }
    }
}

fn humming(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.e.open = 0.1;
    f.e.lower = 0.5;
    f.off.0 += (t * 3.5).sin() * 3.0;
    f.rot = (t * 3.5).sin() * 0.06;
    let a = seg(q, 0.0, 0.15) * (1.0 - seg(q, 0.85, 1.0));
    f.at(&NOTE, 17.0, 3.0 + (t * 7.0).sin() * 0.6, Some(T_SKY), a);
}

fn peek(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.05, 0.4)) * (1.0 - ease(seg(q, 0.7, 0.95)));
    f.off.0 += 26.0 * s;
    f.gaze = (1.4 * s, 0.0);
    f.eyes[0].scale = Some(1.0 - 0.3 * s);
    f.eyes[1].scale = Some(1.0 - 0.3 * s);
    f.e.open = 1.0 - 0.2 * s;
}

fn stretch(f: &mut Frame) {
    let q = f.q;
    let s = (ease(seg(q, 0.05, 0.85)) * PI).sin();
    f.e.open = 1.0 - 0.92 * s;
    f.e.scale = 1.0 + 0.2 * s;
    f.e.sep = 1.0 + 0.15 * s;
    f.off.1 -= 5.0 * s;
}

fn scanning(f: &mut Frame) {
    let q = f.q;
    f.e.open = 0.3;
    let s = (seg(q, 0.05, 0.95) * PI * 2.0).sin();
    f.gaze = (s * 1.3, 0.0);
    let a = PI + s * 1.2;
    f.dot(11.5 + (a + PI / 2.0).cos() * 10.5, 11.5 + (a + PI / 2.0).sin() * 10.5, Some(T_K8S), 1.0);
}

fn hic(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.2, 0.5) {
        let s = bump(q, 0.2, 0.5);
        f.off.1 -= 12.0 * s;
        f.e.open = 1.35;
        f.e.scale = 0.92;
    }
}

fn dozing_off(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.0, 0.7);
    f.e.open = 1.0 - s * 0.85;
    f.off.1 += s * s * 12.0;
    f.rot = s * 0.1;
    if inseg(q, 0.7, 0.8) {
        f.e.open = 1.3;
        f.off.1 -= s * s * 12.0;
    }
    if q >= 0.8 {
        f.e.open = 0.7;
    }
}

fn dizzy(f: &mut Frame) {
    let t = f.t;
    let odd = ((t * 6.0).floor() as i64) % 2 == 1;
    f.sprite = Some(both(if odd { &SPIRAL1 } else { &SPIRAL2 }));
    f.off.0 += (t * 3.0).cos() * 5.0;
    f.off.1 += (t * 3.0).sin() * 4.0;
    f.rot = (t * 3.0).sin() * 0.1;
    f.hold(&Expr::WORRIED, 0.85, 1.0);
}
```

- [ ] **Step 3: Run, fix, commit**

Run: `cargo test -p rackscreen-core face_acts::` then fmt and clippy. Expected: 9 passed, the rest invariant across all 56.

```bash
git add crates/core/src/face_acts.rs
git commit -m "feat(core): boot and idle habit acts complete the catalogue

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: The player: queue, triggers, habits, rest expression, frame output

**Files:**
- Create: `crates/core/src/face_player.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `face_acts::{ActKind, ActDef, Frame, run_act, weather_cat, WeatherCat, IDLE_HABITS, Placed, EyeOv, EyePair, Post}`, `face_expr::{Expr, ExprTween, Idle, IdleOut, unit}`, `mood::{Mood, MoodEngine, MoodInputs, MoodTuning}`, `model::FxRequest`, `electricity::PriceLevel`.
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct FaceTuning { pub bored_after: Secs, pub reaction: Secs, pub idle_habits: bool, pub weather_habits: bool }   // Default 7200, 60, true, true
  /// Everything the player reads from the model each frame.
  #[derive(Clone, Copy, Debug, Default, PartialEq)]
  pub struct FaceInputs {
      pub mood: MoodInputs,
      pub ups_have: bool, pub ups_charge_pct: f32,
      pub storage_pct: f32, pub storage_have: bool,
      pub gh_today: u32, pub gh_have: bool,
      pub price_level: Option<PriceLevel>,
      pub weather_have: bool, pub weather_code: u16, pub weather_temp_c: f32, pub weather_gust_kmh: f32, pub weather_is_day: bool,
      pub sky_have: bool, pub sunrise: Option<i64>, pub sunset: Option<i64>, pub moon_illumination: f32,
      pub unix_now: i64,
  }
  #[derive(Clone, Debug, PartialEq)]
  pub struct FaceFrame { pub e: Expr, pub blink: f32, pub off: (f32, f32), pub rot: f32, pub gaze: (f32, f32), pub eyes: [EyeOv; 2], pub sprite: Option<EyePair>, pub placed: Vec<Placed>, pub post: Post }
  pub struct FacePlayer { .. }
  impl FacePlayer {
      pub fn new(now: Secs) -> Self
      pub fn set_tuning(&mut self, t: FaceTuning)
      pub fn set_seed(&mut self, seed: u64)
      pub fn set_bedtime_near(&mut self, near: bool)
      pub fn on_fx(&mut self, req: &FxRequest, now: Secs)
      pub fn on_link_down(&mut self, now: Secs)
      pub fn tick(&mut self, i: &FaceInputs, now: Secs)
      pub fn frame(&self, now: Secs) -> FaceFrame
      pub fn mood(&self) -> Mood                      // as of the last tick
      pub fn current_act(&self) -> Option<ActKind>
      pub fn queued(&self) -> Vec<ActKind>
  }
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/face_player.rs`:

```rust
//! Plays the acts: one at a time from a short queue, habits when nothing is
//! happening, edge triggers from cluster state, and the rest expression that
//! tweens toward the mood in between.

use std::collections::VecDeque;

use crate::anim::Secs;
use crate::electricity::PriceLevel;
use crate::face_acts::{run_act, weather_cat, ActKind, EyeOv, EyePair, Placed, Post, WeatherCat, IDLE_HABITS};
use crate::face_expr::{unit, Expr, ExprTween, Idle};
use crate::model::FxRequest;
use crate::mood::{Mood, MoodEngine, MoodInputs, MoodTuning};
use crate::theme::Role;

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> FaceInputs {
        FaceInputs::default()
    }

    /// Advance at 30 fps from `from` to `to`, ticking with `inputs`.
    fn run(p: &mut FacePlayer, i: &FaceInputs, from: Secs, to: Secs) {
        let mut t = from;
        while t < to {
            p.tick(i, t);
            t += 1.0 / 30.0;
        }
    }

    fn player() -> FacePlayer {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            idle_habits: false,
            weather_habits: false,
            ..FaceTuning::default()
        });
        p
    }

    #[test]
    fn fx_requests_map_to_acts_and_play_one_at_a_time() {
        let mut p = player();
        p.on_fx(&FxRequest::PodCrashed, 1.0);
        p.on_fx(&FxRequest::GithubStar, 1.0);
        p.tick(&quiet(), 1.0);
        assert_eq!(p.current_act(), Some(ActKind::Ouch));
        assert_eq!(p.queued(), vec![ActKind::StarryEyes]);
        run(&mut p, &quiet(), 1.0, 3.9);
        assert_eq!(p.current_act(), Some(ActKind::StarryEyes), "next act starts when ouch (2.8 s) ends");
        run(&mut p, &quiet(), 3.9, 7.0);
        assert_eq!(p.current_act(), None);
        assert_eq!(p.mood(), Mood::Excited, "the last act's mood holds");
        run(&mut p, &quiet(), 7.0, 68.0);
        assert_eq!(p.mood(), Mood::Worried, "reaction expired after 60 s; the crash still worries");
        run(&mut p, &quiet(), 68.0, 320.0);
        assert_eq!(p.mood(), Mood::Content, "crash worry gone after 300 s");
    }

    #[test]
    fn severe_acts_jump_the_queue_and_the_queue_caps_at_three_without_duplicates() {
        let mut p = player();
        p.on_fx(&FxRequest::GithubPush, 0.0);
        p.on_fx(&FxRequest::GithubPush, 0.0); // one request per commit: collapses
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::Catch));
        assert!(p.queued().is_empty(), "the second push collapsed into the first");
        p.on_fx(&FxRequest::AppSynced, 0.0);
        p.on_fx(&FxRequest::TorrentDone, 0.0);
        p.on_fx(&FxRequest::GithubMerge, 0.0);
        p.on_fx(&FxRequest::AppHealthy, 0.0); // fourth in the queue: dropped
        assert_eq!(p.queued(), vec![ActKind::Launch, ActKind::GotIt, ActKind::Merge]);
        p.on_fx(&FxRequest::UpsOnBattery, 0.1);
        assert_eq!(p.queued()[0], ActKind::LightsFlicker, "severe goes first");
        assert_eq!(p.queued().len(), 3, "and the cap still holds");
    }

    #[test]
    fn pod_starts_are_rate_limited() {
        let mut p = player();
        p.on_fx(&FxRequest::PodStarted, 0.0);
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::OhHi));
        p.on_fx(&FxRequest::PodStarted, 5.0);
        assert!(p.queued().is_empty(), "5 s later: dropped");
        p.on_fx(&FxRequest::PodStarted, 21.0);
        assert_eq!(p.queued(), vec![ActKind::OhHi], "21 s later: allowed again");
    }

    #[test]
    fn boot_plays_first_and_is_not_interrupted() {
        let mut p = player();
        p.on_fx(&FxRequest::GithubStar, 0.0);
        p.on_fx(&FxRequest::Boot, 0.0);
        p.tick(&quiet(), 0.0);
        assert_eq!(p.current_act(), Some(ActKind::WakeUp));
        p.on_fx(&FxRequest::PodCrashed, 0.5);
        run(&mut p, &quiet(), 0.5, 3.0);
        assert_eq!(p.current_act(), Some(ActKind::WakeUp), "still waking up at 3 s");
        run(&mut p, &quiet(), 3.0, 3.7);
        assert_eq!(p.current_act(), Some(ActKind::Ouch), "then the severe one");
    }

    #[test]
    fn three_crashes_in_ten_minutes_make_it_dizzy() {
        let mut p = player();
        for t in [0.0, 100.0, 200.0] {
            p.on_fx(&FxRequest::PodCrashed, t);
            run(&mut p, &quiet(), t, t + 4.0);
        }
        p.tick(&quiet(), 204.0);
        assert_eq!(p.current_act(), Some(ActKind::Dizzy));
    }

    #[test]
    fn link_down_and_up() {
        let mut p = player();
        p.on_link_down(1.0);
        p.tick(&quiet(), 1.0);
        assert_eq!(p.current_act(), Some(ActKind::Hello));
        run(&mut p, &quiet(), 1.0, 5.0);
        p.on_fx(&FxRequest::LinkUp, 5.0);
        p.tick(&quiet(), 5.0);
        assert_eq!(p.current_act(), Some(ActKind::FoundYou));
    }

    #[test]
    fn edge_triggers_fire_once_and_rearm() {
        let mut p = player();
        let mut i = quiet();
        i.ups_have = true;
        i.ups_charge_pct = 50.0;
        run(&mut p, &i, 0.0, 1.0);
        i.ups_charge_pct = 19.0;
        p.tick(&i, 1.0);
        assert_eq!(p.current_act(), Some(ActKind::OnFumes));
        run(&mut p, &i, 1.0, 5.0);
        i.ups_charge_pct = 25.0;
        run(&mut p, &i, 5.0, 6.0);
        i.ups_charge_pct = 15.0;
        run(&mut p, &i, 6.0, 7.0);
        assert_eq!(p.current_act(), None, "not re-armed until above 30");
        i.ups_charge_pct = 35.0;
        run(&mut p, &i, 7.0, 8.0);
        i.ups_charge_pct = 10.0;
        p.tick(&i, 8.0);
        assert_eq!(p.current_act(), Some(ActKind::OnFumes));

        // storage
        let mut p = player();
        let mut i = quiet();
        i.storage_have = true;
        i.storage_pct = 91.0;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), Some(ActKind::SoFull));

        // contributions
        let mut p = player();
        let mut i = quiet();
        i.gh_have = true;
        i.gh_today = 9;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.gh_today = 10;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::LevelUp));

        // prices: first observation only arms, then a band change plays
        let mut p = player();
        let mut i = quiet();
        i.price_level = Some(PriceLevel::Normal);
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.price_level = Some(PriceLevel::Cheap);
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::KaChing));
        run(&mut p, &i, 0.1, 4.0);
        i.price_level = Some(PriceLevel::VeryPricey);
        p.tick(&i, 4.0);
        assert_eq!(p.current_act(), Some(ActKind::Expensive));

        // sunrise, sunset, full moon
        let mut p = player();
        let mut i = quiet();
        i.sky_have = true;
        i.sunrise = Some(1000);
        i.sunset = Some(2000);
        i.unix_now = 999;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
        i.unix_now = 1010;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::Morning));
        run(&mut p, &i, 0.1, 4.0);
        i.unix_now = 1020;
        p.tick(&i, 4.0);
        assert_eq!(p.current_act(), None, "not twice for the same sunrise");
        i.unix_now = 2005;
        i.moon_illumination = 0.5;
        p.tick(&i, 4.1);
        assert_eq!(p.current_act(), Some(ActKind::Evening));
        let mut p = player();
        i.unix_now = 1999;
        p.tick(&i, 0.0);
        i.unix_now = 2005;
        i.moon_illumination = 0.98;
        p.tick(&i, 0.1);
        assert_eq!(p.current_act(), Some(ActKind::Awoo));
        // a Pi booting at noon does not replay sunrise
        let mut p = player();
        i.unix_now = 5000;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), None);
    }

    #[test]
    fn weather_habit_plays_on_change_and_then_every_few_minutes() {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            idle_habits: false,
            ..FaceTuning::default()
        });
        let mut i = quiet();
        i.weather_have = true;
        i.weather_code = 61;
        i.weather_temp_c = 12.0;
        i.weather_is_day = true;
        p.tick(&i, 0.0);
        assert_eq!(p.current_act(), Some(ActKind::Raining), "first sight of rain");
        run(&mut p, &i, 0.0, 5.0);
        assert_eq!(p.current_act(), None);
        let mut replayed_at = None;
        let mut t = 5.0;
        while t < 600.0 {
            p.tick(&i, t);
            if p.current_act() == Some(ActKind::Raining) {
                replayed_at = Some(t);
                break;
            }
            t += 1.0 / 30.0;
        }
        let at = replayed_at.expect("replayed within ten minutes");
        assert!((240.0..=480.0).contains(&at), "replay at {at}");
        // a real event interrupts the habit: it jumps to its out phase
        p.on_fx(&FxRequest::PodCrashed, at + 0.5);
        run(&mut p, &i, at + 0.5, at + 0.95);
        assert_eq!(p.current_act(), Some(ActKind::Ouch), "habit cut short within 0.3 s");
    }

    #[test]
    fn idle_habits_fire_when_content_and_never_sneeze_twice() {
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            weather_habits: false,
            ..FaceTuning::default()
        });
        let mut seen = Vec::new();
        let mut last = None;
        let mut t = 0.0;
        while t < 1800.0 {
            p.tick(&quiet(), t);
            let cur = p.current_act();
            if cur.is_some() && cur != last {
                seen.push(cur.unwrap());
            }
            last = cur;
            t += 1.0 / 30.0;
        }
        assert!(seen.len() >= 10 && seen.len() <= 40, "{} habits in 30 min", seen.len());
        assert!(seen.iter().all(|k| k.is_idle_habit()));
        assert!(!seen.contains(&ActKind::DozingOff), "not bored yet");
        for w in seen.windows(2) {
            assert!(!(matches!(w[0], ActKind::Sneeze | ActKind::Hic) && w[0] == w[1]), "{w:?} twice");
        }
        // bored: dozing off joins in
        let mut p = FacePlayer::new(0.0);
        p.set_tuning(FaceTuning {
            bored_after: 10.0,
            weather_habits: false,
            ..FaceTuning::default()
        });
        let mut dozed = false;
        let mut t = 0.0;
        while t < 1800.0 {
            p.tick(&quiet(), t);
            if p.current_act() == Some(ActKind::DozingOff) {
                dozed = true;
                break;
            }
            t += 1.0 / 30.0;
        }
        assert!(dozed);
    }

    #[test]
    fn frames_are_continuous_and_deterministic() {
        let mut p = player();
        p.on_fx(&FxRequest::NodeNotReady, 1.0);
        let mut prev = p.frame(0.0);
        let mut t = 0.0;
        while t < 8.0 {
            p.tick(&quiet(), t);
            let f = p.frame(t);
            let d = (f.e.open - prev.e.open).abs() + (f.e.lift - prev.e.lift).abs() / 10.0;
            assert!(d < 0.25, "jump at {t}: {d}");
            assert!((f.off.1 - prev.off.1).abs() < 12.0, "offset jump at {t}");
            prev = f;
            t += 1.0 / 30.0;
        }
        assert!(p.frame(8.0).e.close_to(&Expr::SAD, 0.05), "held sad after lost-one");
        let mut a = player();
        let mut b = player();
        for k in 0..300 {
            let t = k as f64 / 30.0;
            a.tick(&quiet(), t);
            b.tick(&quiet(), t);
            assert_eq!(a.frame(t), b.frame(t));
        }
    }
}
```

Add `pub mod face_player;` to `crates/core/src/lib.rs` (after `face_expr`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core face_player:: 2>&1 | head`
Expected: compile errors.

- [ ] **Step 3: Implement the player**

Insert before the tests:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceTuning {
    pub bored_after: Secs,
    pub reaction: Secs,
    pub idle_habits: bool,
    pub weather_habits: bool,
}

impl Default for FaceTuning {
    fn default() -> Self {
        Self {
            bored_after: 120.0 * 60.0,
            reaction: 60.0,
            idle_habits: true,
            weather_habits: true,
        }
    }
}

/// Everything the player reads from the model each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceInputs {
    pub mood: MoodInputs,
    pub ups_have: bool,
    pub ups_charge_pct: f32,
    pub storage_have: bool,
    pub storage_pct: f32,
    pub gh_have: bool,
    pub gh_today: u32,
    pub price_level: Option<PriceLevel>,
    pub weather_have: bool,
    pub weather_code: u16,
    pub weather_temp_c: f32,
    pub weather_gust_kmh: f32,
    pub weather_is_day: bool,
    pub sky_have: bool,
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub moon_illumination: f32,
    pub unix_now: i64,
}

/// What the rasteriser draws this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFrame {
    pub e: Expr,
    pub blink: f32,
    pub off: (f32, f32),
    pub rot: f32,
    pub gaze: (f32, f32),
    pub eyes: [EyeOv; 2],
    pub sprite: Option<EyePair>,
    pub placed: Vec<Placed>,
    pub post: Post,
}

/// Queue depth; a fourth waiting act is dropped.
pub const QUEUE_CAP: usize = 3;
/// Pod start and gone acts: at most one per this many seconds.
pub const POD_RATE_SECS: Secs = 20.0;
/// Crashes within this window make the face dizzy.
pub const DIZZY_WINDOW_SECS: Secs = 600.0;
pub const DIZZY_CRASHES: usize = 3;
/// Edge thresholds with their re-arm levels.
pub const UPS_LOW_PCT: f32 = 20.0;
pub const UPS_LOW_REARM_PCT: f32 = 30.0;
pub const STORAGE_FULL_PCT: f32 = 90.0;
pub const STORAGE_FULL_REARM_PCT: f32 = 85.0;
pub const GH_MILESTONE: u32 = 10;
pub const FULL_MOON: f32 = 0.97;
/// A sunrise or sunset counts while `unix_now` is within this many seconds after it.
pub const SUN_EDGE_SECS: i64 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    kind: ActKind,
    started: Secs,
    dur: Secs,
}

impl Playing {
    fn progress(&self, now: Secs) -> f32 {
        (((now - self.started) / self.dur).clamp(0.0, 1.0)) as f32
    }
    fn done(&self, now: Secs) -> bool {
        now - self.started >= self.dur
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacePlayer {
    tuning: FaceTuning,
    mood: MoodEngine,
    idle: Idle,
    rng: u64,
    queue: VecDeque<ActKind>,
    current: Option<Playing>,
    /// Expression at rest, tweening toward the mood.
    rest: ExprTween,
    last_mood: Mood,
    last_pod_start: Option<Secs>,
    last_pod_gone: Option<Secs>,
    crashes: Vec<Secs>,
    // habits
    next_idle: Secs,
    last_idle: Option<ActKind>,
    next_weather: Secs,
    weather_seen: Option<WeatherCat>,
    // edges
    ups_low_armed: bool,
    storage_full_armed: bool,
    gh_milestone_armed: bool,
    price_prev: Option<PriceLevel>,
    sunrise_done: Option<i64>,
    sunset_done: Option<i64>,
    last_tick: Secs,
}

impl FacePlayer {
    pub fn new(now: Secs) -> Self {
        let mut rng = 0x9E37_79B9_7F4A_7C15;
        let first_idle = now + 45.0 + 75.0 * unit(&mut rng) as Secs;
        Self {
            tuning: FaceTuning::default(),
            mood: MoodEngine::new(now),
            idle: Idle::new(),
            rng,
            queue: VecDeque::new(),
            current: None,
            rest: ExprTween::new(Expr::CONTENT),
            last_mood: Mood::Content,
            last_pod_start: None,
            last_pod_gone: None,
            crashes: Vec::new(),
            next_idle: first_idle,
            last_idle: None,
            next_weather: now,
            weather_seen: None,
            ups_low_armed: true,
            storage_full_armed: true,
            gh_milestone_armed: true,
            price_prev: None,
            sunrise_done: None,
            sunset_done: None,
            last_tick: now,
        }
    }

    pub fn set_tuning(&mut self, t: FaceTuning) {
        self.tuning = t;
        self.mood.set_tuning(MoodTuning {
            bored_after: t.bored_after,
            reaction: t.reaction,
        });
    }
    pub fn tuning(&self) -> FaceTuning {
        self.tuning
    }
    pub fn set_seed(&mut self, seed: u64) {
        self.rng = seed | 1;
    }
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.mood.set_bedtime_near(near);
    }
    pub fn mood(&self) -> Mood {
        self.last_mood
    }
    pub fn current_act(&self) -> Option<ActKind> {
        self.current.map(|p| p.kind)
    }
    pub fn queued(&self) -> Vec<ActKind> {
        self.queue.iter().copied().collect()
    }

    /// The same request stream the splashes and sweeps get.
    pub fn on_fx(&mut self, req: &FxRequest, now: Secs) {
        let kind = match req {
            FxRequest::PodStarted => ActKind::OhHi,
            FxRequest::PodCrashed => ActKind::Ouch,
            FxRequest::PodGone => ActKind::Bye,
            FxRequest::HotNode(Role::Mem) => ActKind::Stuffed,
            FxRequest::HotNode(_) => ActKind::WorkingHard,
            FxRequest::HotTemp => ActKind::TooHot,
            FxRequest::VolumeDegraded => ActKind::Hmm,
            FxRequest::VolumeHealthy => ActKind::DisksFine,
            FxRequest::TorrentAdded => ActKind::Incoming,
            FxRequest::TorrentDone => ActKind::GotIt,
            FxRequest::NodeNotReady => ActKind::LostOne,
            FxRequest::NodeReady => ActKind::ItsBack,
            FxRequest::AlertFiring => ActKind::Alarm,
            FxRequest::AlertResolved => ActKind::AllClear,
            FxRequest::LinkUp => ActKind::FoundYou,
            FxRequest::Boot => ActKind::WakeUp,
            FxRequest::GithubPush => ActKind::Catch,
            FxRequest::GithubStar => ActKind::StarryEyes,
            FxRequest::GithubMerge => ActKind::Merge,
            FxRequest::GithubRelease => ActKind::Party,
            FxRequest::GithubRunFailed => ActKind::EyeRoll,
            FxRequest::GithubRunPassed => ActKind::Ding,
            FxRequest::Thunder => ActKind::Lightning,
            FxRequest::RainSoon => ActKind::UhOhRain,
            FxRequest::AirWorse => ActKind::Cough,
            FxRequest::AppSynced => ActKind::Launch,
            FxRequest::AppDegraded => ActKind::Grump,
            FxRequest::AppHealthy => ActKind::Wink,
            FxRequest::UpsOnBattery => ActKind::LightsFlicker,
            FxRequest::UpsOnline => ActKind::Phew,
            FxRequest::IssPass => ActKind::LookUp,
        };
        if kind == ActKind::Ouch {
            self.mood.crashed(now);
            self.crashes.retain(|t| now - t <= DIZZY_WINDOW_SECS);
            self.crashes.push(now);
        }
        self.request(kind, now);
        if kind == ActKind::Ouch && self.crashes.len() >= DIZZY_CRASHES {
            self.crashes.clear();
            self.request(ActKind::Dizzy, now);
        }
    }

    pub fn on_link_down(&mut self, now: Secs) {
        self.request(ActKind::Hello, now);
    }

    /// Queue an event act (not a habit), honouring rate limits, the cap,
    /// severity, and boot.
    fn request(&mut self, kind: ActKind, now: Secs) {
        if kind == ActKind::WakeUp {
            self.queue.clear();
            self.current = Some(Playing {
                kind,
                started: now,
                dur: kind.def().dur as Secs,
            });
            self.mood.touch(now);
            return;
        }
        if kind.is_rate_limited() {
            let last = if kind == ActKind::OhHi {
                &mut self.last_pod_start
            } else {
                &mut self.last_pod_gone
            };
            if last.is_some_and(|t| now - t < POD_RATE_SECS) {
                return;
            }
            *last = Some(now);
        }
        self.mood.touch(now);
        if self.queue.contains(&kind) || self.current.is_some_and(|p| p.kind == kind && !p.done(now)) {
            return;
        }
        // an interruptible habit gives way: jump to its out phase
        if let Some(cur) = self.current {
            if cur.kind.is_habit() {
                let out_start = cur.dur - crate::face_acts::IN_S as Secs;
                if now - cur.started < out_start {
                    self.current = Some(Playing {
                        started: now - out_start,
                        ..cur
                    });
                }
            }
        }
        if kind.is_severe() {
            self.queue.push_front(kind);
            self.queue.truncate(QUEUE_CAP);
        } else if self.queue.len() < QUEUE_CAP {
            self.queue.push_back(kind);
        }
    }

    /// Advance: finish or start acts, run edge triggers and habits, and move
    /// the rest expression toward the mood.
    pub fn tick(&mut self, i: &FaceInputs, now: Secs) {
        self.last_tick = now;
        self.edges(i, now);
        if let Some(cur) = self.current {
            if cur.done(now) {
                let def = cur.kind.def();
                let end = Expr::of(def.mood);
                self.rest = ExprTween::new(end);
                if !cur.kind.is_habit() {
                    self.mood.react(def.mood, now);
                }
                self.current = None;
            }
        }
        if self.current.is_none() {
            if let Some(kind) = self.queue.pop_front() {
                self.start(kind, now);
            } else {
                self.habits(i, now);
            }
        }
        self.last_mood = self.mood.current(&i.mood, now);
        self.rest.retarget(Expr::of(self.last_mood), now);
    }

    fn start(&mut self, kind: ActKind, now: Secs) {
        self.current = Some(Playing {
            kind,
            started: now,
            dur: kind.def().dur as Secs,
        });
    }

    fn edges(&mut self, i: &FaceInputs, now: Secs) {
        if i.ups_have {
            if self.ups_low_armed && i.ups_charge_pct < UPS_LOW_PCT {
                self.ups_low_armed = false;
                self.request(ActKind::OnFumes, now);
            } else if !self.ups_low_armed && i.ups_charge_pct > UPS_LOW_REARM_PCT {
                self.ups_low_armed = true;
            }
        }
        if i.storage_have {
            if self.storage_full_armed && i.storage_pct >= STORAGE_FULL_PCT {
                self.storage_full_armed = false;
                self.request(ActKind::SoFull, now);
            } else if !self.storage_full_armed && i.storage_pct < STORAGE_FULL_REARM_PCT {
                self.storage_full_armed = true;
            }
        }
        if i.gh_have {
            if self.gh_milestone_armed && i.gh_today >= GH_MILESTONE {
                self.gh_milestone_armed = false;
                self.request(ActKind::LevelUp, now);
            } else if !self.gh_milestone_armed && i.gh_today < GH_MILESTONE {
                self.gh_milestone_armed = true;
            }
        }
        if let Some(level) = i.price_level {
            let cheap = |l: PriceLevel| matches!(l, PriceLevel::VeryCheap | PriceLevel::Cheap);
            let pricey = |l: PriceLevel| matches!(l, PriceLevel::Pricey | PriceLevel::VeryPricey);
            if let Some(prev) = self.price_prev {
                if prev != level {
                    if cheap(level) && !cheap(prev) {
                        self.request(ActKind::KaChing, now);
                    } else if pricey(level) && !pricey(prev) {
                        self.request(ActKind::Expensive, now);
                    }
                }
            }
            self.price_prev = Some(level);
        }
        if i.sky_have {
            let passing = |edge: Option<i64>| edge.filter(|e| (*e..*e + SUN_EDGE_SECS).contains(&i.unix_now));
            if let Some(e) = passing(i.sunrise) {
                if self.sunrise_done != Some(e) {
                    self.sunrise_done = Some(e);
                    self.request(ActKind::Morning, now);
                }
            }
            if let Some(e) = passing(i.sunset) {
                if self.sunset_done != Some(e) {
                    self.sunset_done = Some(e);
                    let kind = if i.moon_illumination >= FULL_MOON {
                        ActKind::Awoo
                    } else {
                        ActKind::Evening
                    };
                    self.request(kind, now);
                }
            }
        }
    }

    /// Nothing is playing and nothing is queued: maybe a weather or idle habit.
    fn habits(&mut self, i: &FaceInputs, now: Secs) {
        if self.tuning.weather_habits && i.weather_have {
            let cat = weather_cat(i.weather_code, i.weather_temp_c, i.weather_gust_kmh, i.weather_is_day);
            let changed = cat != self.weather_seen;
            if changed {
                self.weather_seen = cat;
            }
            if let Some(cat) = cat {
                if changed || now >= self.next_weather {
                    self.next_weather = now + 240.0 + 240.0 * unit(&mut self.rng) as Secs;
                    self.start(cat.act(), now);
                    return;
                }
            }
        }
        if self.tuning.idle_habits
            && matches!(self.last_mood, Mood::Content | Mood::Bored)
            && now >= self.next_idle
        {
            self.next_idle = now + 45.0 + 75.0 * unit(&mut self.rng) as Secs;
            let bored = self.last_mood == Mood::Bored;
            let pool: Vec<(ActKind, u32)> = IDLE_HABITS
                .iter()
                .copied()
                .filter(|(k, _)| bored || *k != ActKind::DozingOff)
                .collect();
            let total: u32 = pool.iter().map(|(_, w)| w).sum();
            let mut pick = (unit(&mut self.rng) * total as f32) as u32;
            let mut kind = pool[0].0;
            for (k, w) in &pool {
                if pick < *w {
                    kind = *k;
                    break;
                }
                pick -= w;
            }
            if matches!(kind, ActKind::Sneeze | ActKind::Hic) && self.last_idle == Some(kind) {
                kind = ActKind::Humming;
            }
            self.last_idle = Some(kind);
            self.start(kind, now);
        }
    }

    /// The face this instant. Idle blink, gaze and tilt are computed here from
    /// the seeded generator, so equal seeds and clocks give equal frames.
    pub fn frame(&self, now: Secs) -> FaceFrame {
        let mut idle = self.idle.clone();
        let mut rng = self.rng;
        let io = idle.tick(self.last_mood, now, &mut rng);
        match self.current {
            Some(cur) => {
                let def = cur.kind.def();
                let held = self.rest.value(now);
                let f = run_act(&def, cur.progress(now), held, io.gaze);
                FaceFrame {
                    e: f.e,
                    blink: io.blink,
                    off: f.off,
                    rot: f.rot + io.tilt,
                    gaze: f.gaze,
                    eyes: f.eyes,
                    sprite: f.sprite,
                    placed: f.placed,
                    post: f.post,
                }
            }
            None => FaceFrame {
                e: self.rest.value(now),
                blink: io.blink,
                off: io.wobble,
                rot: io.tilt,
                gaze: io.gaze,
                eyes: [EyeOv::default(); 2],
                sprite: None,
                placed: Vec::new(),
                post: Post::default(),
            },
        }
    }
}
```

The idle state must also advance, or blinks would restart every frame. `frame(&self)` cannot mutate, so make `tick` advance it: add to the end of `tick`, before `last_mood` is computed:

```rust
        let _ = self.idle.tick(self.last_mood, now, &mut self.rng);
```

and change `frame` to use `self.idle.clone()` / `self.rng` as written (the clone replays the same step `tick` just took, since `tick` and `frame` are called with the same `now`; a `frame` between ticks reuses the last state, which is what the simulator's paused mode needs).

- [ ] **Step 4: Run, fix, commit**

Run: `cargo test -p rackscreen-core face_player::` then fmt and clippy.
Expected: 10 passed. Common failures and their fixes:
- `PriceLevel` or `WeatherCat` missing a trait for the `FacePlayer` derives: add `#[derive(Clone, Copy, Debug, PartialEq, Eq)]` to `PriceLevel` in `crates/core/src/electricity.rs` if it lacks any of them.
- `frames_are_continuous`: the jump is at act end; make sure `tick` sets `self.rest = ExprTween::new(end)` before `mood.react`, and that `retarget` is a no-op when the target is unchanged.
- `weather_habit...` interruption: `request` must rewrite `started` so that `now - started == dur - IN_S`.
- `idle_habits` count: 30 min at 45..120 s spacing gives roughly 15..40 habits.

```bash
git add crates/core/src/face_player.rs crates/core/src/lib.rs
git commit -m "feat(core): face player with queue, edges, habits and rest expression

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Rasteriser: from `FaceFrame` to dots

**Files:**
- Modify: `crates/core/src/scene_face.rs` (replace the stub body; `face_scene` stays a stub until Task 11)

**Interfaces:**
- Consumes: `face_player::FaceFrame`, `face_acts::{Overlay, Placed, EyeOv, EyePair}`, `face_expr::Expr`, `face_sprites::Sprite`, `scene::{Scene, Drawable}`, `theme::{Color, AMBER}`.
- Produces:
  ```rust
  pub const CELL: f32 = 10.0; pub const N: usize = 24; pub const VISIBLE_R: f32 = 116.0; pub const RIM_R: f32 = 104.0;
  pub const LIT_R: f32 = 3.6; pub const UNLIT_R: f32 = 2.3; pub const LIT_MIN: f32 = 0.2;
  pub fn render_frame(f: &FaceFrame, now: Secs) -> Scene           // Clear + 48 Dots
  pub fn cell_brightness(f: &FaceFrame, now: Secs, i: usize, j: usize) -> (f32, Option<Color>)   // for tests
  pub fn amber(b: f32) -> Color
  ```

- [ ] **Step 1: Write the failing tests**

Replace `crates/core/src/scene_face.rs` with:

```rust
//! The `face` role: two eyes on a 24×24 dot matrix, sampled from the shapes
//! the player describes and emitted as `Drawable::Dots` rows.

use crate::anim::Secs;
use crate::face_acts::{EyeOv, Overlay, Placed};
use crate::face_player::FaceFrame;
use crate::face_sprites::Sprite;
use crate::model::Model;
use crate::scene::{Drawable, Scene};
use crate::theme::{Color, AMBER};

pub const CELL: f32 = 10.0;
pub const N: usize = 24;
/// Cells whose centre is farther from the middle than this are never lit.
pub const VISIBLE_R: f32 = 116.0;
/// The `rim` post-effect lights cells beyond this radius.
pub const RIM_R: f32 = 104.0;
pub const LIT_R: f32 = 3.6;
pub const UNLIT_R: f32 = 2.3;
pub const LIT_MIN: f32 = 0.2;
/// Eye geometry in the 240 px space.
pub const EYE_W: f32 = 70.0;
pub const EYE_H: f32 = 84.0;
pub const EYE_CY: f32 = 120.0;
pub const EYE_DX: f32 = 50.0;
pub const EYE_RADIUS: f32 = 20.0;
/// Half width of the lid cuts.
pub const LID_W: f32 = 48.0;

pub fn face_scene(_model: &Model, _now: Secs) -> Scene {
    Scene::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face_acts::{Post, SLOT_X, SLOT_Y};
    use crate::face_expr::Expr;
    use crate::face_sprites::{BOX, T_K8S, XEYE};

    fn frame(e: Expr) -> FaceFrame {
        FaceFrame {
            e,
            blink: 0.0,
            off: (0.0, 0.0),
            rot: 0.0,
            gaze: (0.0, 0.0),
            eyes: [EyeOv::default(); 2],
            sprite: None,
            placed: Vec::new(),
            post: Post::default(),
        }
    }

    fn dots(s: &Scene) -> Vec<(f32, f32, &Vec<Color>)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Dots { cy, r, colors, .. } => Some((*cy, *r, colors)),
                _ => None,
            })
            .collect()
    }

    fn lit_cells(f: &FaceFrame) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for j in 0..N {
            for i in 0..N {
                if cell_brightness(f, 0.0, i, j).0 > LIT_MIN {
                    v.push((i, j));
                }
            }
        }
        v
    }

    #[test]
    fn scene_is_a_clear_and_forty_eight_rows_of_twenty_four() {
        let s = render_frame(&frame(Expr::CONTENT), 0.0);
        assert_eq!(s.items.len(), 49);
        let d = dots(&s);
        assert_eq!(d.len(), 48);
        for (k, (cy, r, colors)) in d.iter().enumerate() {
            assert_eq!(colors.len(), 24);
            assert_eq!(*cy, (k / 2) as f32 * CELL + 5.0);
            assert_eq!(*r, if k % 2 == 0 { LIT_R } else { UNLIT_R });
        }
        if let Drawable::Dots { cx, spacing, .. } = &s.items[1] {
            assert_eq!((*cx, *spacing), (120.0, CELL));
        } else {
            panic!("second item is not Dots");
        }
        // corners are outside the panel: transparent in both rows
        assert_eq!(d[0].2[0].a, 0);
        assert_eq!(d[1].2[0].a, 0);
        assert_eq!(d[46].2[23].a, 0);
    }

    #[test]
    fn content_eyes_are_two_amber_blobs_with_a_dark_gap() {
        let f = frame(Expr::CONTENT);
        let lit = lit_cells(&f);
        assert!(lit.len() > 40 && lit.len() < 200, "{} lit", lit.len());
        let (b, tint) = cell_brightness(&f, 0.0, 7, 12);
        assert!(b > 0.9 && tint.is_none(), "left eye centre lit amber");
        let (b, _) = cell_brightness(&f, 0.0, 16, 12);
        assert!(b > 0.9, "right eye centre lit");
        let (b, _) = cell_brightness(&f, 0.0, 12, 12);
        assert!(b < 0.1, "between the eyes is dark");
        let c = amber(1.0);
        assert_eq!((c.r, c.g, c.b), (255, 210, 40));
        assert_eq!(amber(0.0).g, 150);
        // the lit row carries colour, the unlit row is transparent there
        let s = render_frame(&f, 0.0);
        let d = dots(&s);
        assert!(d[24].2[7].a > 200 && d[25].2[7].a == 0);
        // and vice versa in the gap
        assert_eq!(d[24].2[12].a, 0);
        assert!((d[25].2[12].a as i32 - 20).abs() <= 2, "unlit at 8 %: {}", d[25].2[12].a);
    }

    #[test]
    fn every_mood_lights_a_sane_number_of_cells() {
        for m in [
            Expr::CONTENT, Expr::HAPPY, Expr::EXCITED, Expr::WORRIED, Expr::SAD,
            Expr::ANGRY, Expr::HOT, Expr::SCARED, Expr::SLEEPY, Expr::BORED,
        ] {
            let n = lit_cells(&frame(m)).len();
            assert!(n >= 20 && n <= 200, "{m:?}: {n}");
        }
    }

    #[test]
    fn sad_sits_lower_and_happy_is_two_arcs() {
        let top = |e: Expr| lit_cells(&frame(e)).iter().map(|c| c.1).min().unwrap();
        assert!(top(Expr::SAD) > top(Expr::CONTENT));
        assert!(top(Expr::HAPPY) <= top(Expr::CONTENT), "happy lifts (by less than a row)");
        // happy: the lower half of each eye is cut away
        let (b, _) = cell_brightness(&frame(Expr::HAPPY), 0.0, 7, 14);
        assert!(b < 0.3, "below the arc is dark: {b}");
    }

    #[test]
    fn a_blink_leaves_at_most_two_rows_per_eye() {
        let mut f = frame(Expr::CONTENT);
        f.blink = 1.0;
        let rows: std::collections::HashSet<usize> =
            lit_cells(&f).iter().filter(|c| c.0 < 12).map(|c| c.1).collect();
        assert!(rows.len() <= 2, "{rows:?}");
    }

    #[test]
    fn tilt_and_per_eye_override_change_the_shape() {
        let angry = lit_cells(&frame(Expr::ANGRY));
        // inner corners down: the topmost lit cell of the left eye is on its outer side
        let top_row = angry.iter().filter(|c| c.0 < 12).map(|c| c.1).min().unwrap();
        let cols: Vec<usize> = angry.iter().filter(|c| c.0 < 12 && c.1 == top_row).map(|c| c.0).collect();
        assert!(cols.iter().all(|c| *c <= 7), "angry left eye top is outer: {cols:?}");
        let mut f = frame(Expr::CONTENT);
        f.eyes[0].open = Some(0.06);
        let left: Vec<_> = lit_cells(&f).into_iter().filter(|c| c.0 < 12).collect();
        let right: Vec<_> = lit_cells(&f).into_iter().filter(|c| c.0 >= 12).collect();
        assert!(left.len() * 3 < right.len(), "winking left eye is small");
    }

    #[test]
    fn sprites_replace_eyes_and_icons_carry_their_tint() {
        let mut f = frame(Expr::CONTENT);
        f.sprite = Some([&XEYE, &XEYE]);
        let (b, _) = cell_brightness(&f, 0.0, 7, 12);
        assert!(b > 0.9, "X centre");
        let (b, _) = cell_brightness(&f, 0.0, 6, 9);
        assert!(b < 0.1, "X corner gap");
        let mut f = frame(Expr::CONTENT);
        f.placed.push(Placed {
            sprite: &BOX,
            x: SLOT_X,
            y: SLOT_Y,
            tint: Some(T_K8S),
            alpha: 1.0,
        });
        let (b, tint) = cell_brightness(&f, 0.0, 11, 17);
        assert!(b > 0.9 && tint == Some(T_K8S), "box top row is blue");
        let s = render_frame(&f, 0.0);
        let d = dots(&s);
        let c = d[34].2[11];
        assert_eq!((c.r, c.g, c.b), (T_K8S.r, T_K8S.g, T_K8S.b));
        // half alpha sprite: dim but lit
        f.placed[0].alpha = 0.5;
        let (b, _) = cell_brightness(&f, 0.0, 11, 17);
        assert!((b - 0.5).abs() < 0.05);
    }

    #[test]
    fn post_effects_apply() {
        let mut f = frame(Expr::CONTENT);
        f.post.rim = 0.5;
        let (b, _) = cell_brightness(&f, 0.0, 12, 1);
        assert!((b - 0.5).abs() < 1e-6, "rim lights the top edge");
        let (b, _) = cell_brightness(&f, 0.0, 12, 12);
        assert!(b < 0.1, "not the middle");
        f.post.rim = 0.0;
        f.post.flicker = 0.08;
        let s = render_frame(&f, 0.0);
        let c = dots(&s)[24].2[7];
        assert!(c.a < 30, "flicker dims the eye: {}", c.a);
        f.post.flicker = 3.0;
        let s = render_frame(&f, 0.0);
        assert_eq!(dots(&s)[24].2[7].a, 255, "clamped");
        f.post.flicker = 1.0;
        f.post.overlay = Overlay::FogBand(0.0);
        let band: Vec<usize> = (0..N).filter(|i| cell_brightness(&f, 0.0, *i, 6).0 > 0.3 && (i + 6) % 2 == 0).collect();
        assert!(band.len() >= 8, "fog band on row 6: {band:?}");
        f.post.overlay = Overlay::Confetti(0.0);
        let n = lit_cells(&f).len();
        assert!(n > lit_cells(&frame(Expr::CONTENT)).len(), "confetti adds cells");
    }

    #[test]
    fn transforms_move_the_eyes() {
        let mut f = frame(Expr::CONTENT);
        f.off = (0.0, -30.0);
        let top = lit_cells(&f).iter().map(|c| c.1).min().unwrap();
        assert!(top < lit_cells(&frame(Expr::CONTENT)).iter().map(|c| c.1).min().unwrap());
        let mut f = frame(Expr::CONTENT);
        f.gaze = (1.0, 0.0);
        let left = lit_cells(&f).iter().map(|c| c.0).min().unwrap();
        assert!(left > lit_cells(&frame(Expr::CONTENT)).iter().map(|c| c.0).min().unwrap());
        let mut f = frame(Expr::CONTENT);
        f.rot = 0.3;
        let cells = lit_cells(&f);
        let l = cells.iter().filter(|c| c.0 < 12).map(|c| c.1).min().unwrap();
        let r = cells.iter().filter(|c| c.0 >= 12).map(|c| c.1).min().unwrap();
        assert_ne!(l, r, "tilted: the eyes are at different heights");
        assert_eq!(render_frame(&f, 1.0), render_frame(&f, 1.0));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core scene_face:: 2>&1 | head`
Expected: compile errors (`render_frame`, `cell_brightness`, `amber` missing).

- [ ] **Step 3: Implement the rasteriser**

Insert between `face_scene` and the tests:

```rust
/// Amber for brightness `b` in 0..1, before alpha.
pub fn amber(b: f32) -> Color {
    Color::rgb(255, (150.0 + 60.0 * b.clamp(0.0, 1.0)).round() as u8, 40)
}

/// One eye's shape in the face's local space.
struct Eye {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    /// Upper-lid slope, already mirrored for this side.
    slope: f32,
    /// Lower-lid rise in px (0 = none).
    rise: f32,
    sprite: Option<&'static Sprite>,
}

impl Eye {
    fn inside(&self, x: f32, y: f32) -> bool {
        if let Some(sp) = self.sprite {
            let gx = ((self.cx - 35.0) / CELL).round();
            let gy = ((self.cy - sp.h() as f32 * 5.0) / CELL).round();
            let i = ((x / CELL) - gx).floor();
            let j = ((y / CELL) - gy).floor();
            return i >= 0.0 && j >= 0.0 && sp.on(i as usize, j as usize);
        }
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        let r = EYE_RADIUS.min(hw).min(hh);
        let dx = (x - self.cx).abs() - (hw - r);
        let dy = (y - self.cy).abs() - (hh - r);
        let outside = if dx > 0.0 && dy > 0.0 {
            dx * dx + dy * dy > r * r
        } else {
            dx > r || dy > r
        };
        if outside {
            return false;
        }
        // upper lid: black above the slanted line
        let top = self.cy - hh + 2.0 + self.slope * (x - self.cx);
        if y < top {
            return false;
        }
        // lower lid: black below the parabola
        if self.rise > 0.0 {
            let u = (x - self.cx) / LID_W;
            let bottom = self.cy + hh - 1.3 * self.rise + 0.95 * self.rise * u * u;
            if y > bottom {
                return false;
            }
        }
        true
    }
}

fn eyes_of(f: &FaceFrame) -> [Eye; 2] {
    let e = &f.e;
    let build = |side: f32, ov: &EyeOv, sprite: Option<&'static Sprite>| {
        let open = (e.open * ov.open.unwrap_or(1.0) * (1.0 - f.blink)).max(0.04);
        let scale = e.scale * ov.scale.unwrap_or(1.0);
        Eye {
            cx: 120.0 - side * EYE_DX * e.sep,
            cy: EYE_CY,
            w: EYE_W * scale,
            h: EYE_H * scale * open,
            slope: (e.tilt + ov.tilt.unwrap_or(0.0)) * 0.55 * side,
            rise: if e.lower >= 0.02 { 0.75 * e.lower * EYE_H } else { 0.0 },
            sprite,
        }
    };
    [
        build(1.0, &f.eyes[0], f.sprite.map(|s| s[0])),
        build(-1.0, &f.eyes[1], f.sprite.map(|s| s[1])),
    ]
}

/// Screen point → face-local point. The forward map is `S = T + C + R(rot)·(L − C)`
/// with `C = (120, 120)` the face centre in local space: translate first, then
/// tilt the whole translated face about its own centre.
fn to_local(f: &FaceFrame, now: Secs, x: f32, y: f32) -> (f32, f32) {
    let bob = ((now * 0.9).sin() * 1.5) as f32;
    let tx = f.off.0 + f.gaze.0 * 10.0;
    let ty = f.off.1 + f.e.lift + bob + f.gaze.1 * 8.0;
    let (vx, vy) = (x - tx, y - ty);
    if f.rot == 0.0 {
        return (vx, vy);
    }
    let (cx, cy) = (120.0, 120.0);
    let (s, c) = (-f.rot).sin_cos();
    let (dx, dy) = (vx - cx, vy - cy);
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

fn sprite_hit(p: &Placed, x: f32, y: f32) -> bool {
    let i = (x / CELL - p.x).floor();
    let j = (y / CELL - p.y).floor();
    i >= 0.0 && j >= 0.0 && p.sprite.on(i as usize, j as usize)
}

fn overlay(f: &FaceFrame, i: usize, j: usize) -> f32 {
    match f.post.overlay {
        Overlay::None => 0.0,
        Overlay::Confetti(t) => {
            let a = ((t * 9.0).floor() as i64 + i as i64 * 3) % 6 == 0;
            let b = (j as i64 + ((i * 7 + 3) % 11) as i64 + (t * 7.0).floor() as i64) % 4 == 0;
            if a && b {
                0.9
            } else {
                0.0
            }
        }
        Overlay::FogBand(t) => {
            let band = 6.0 + (t * 2.0) % 12.0;
            if (j as f32 - band).abs() < 1.5 && (i + j) % 2 == 0 {
                0.35
            } else {
                0.0
            }
        }
    }
}

/// Brightness 0..1 of cell `(i, j)` and the tint of the sprite covering it,
/// before the flicker multiplier.
pub fn cell_brightness(f: &FaceFrame, now: Secs, i: usize, j: usize) -> (f32, Option<Color>) {
    let (cx, cy) = (i as f32 * CELL + 5.0, j as f32 * CELL + 5.0);
    if ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() > VISIBLE_R {
        return (0.0, None);
    }
    let eyes = eyes_of(f);
    let mut sum = 0.0;
    let mut tint = None;
    for dy in [-3.0, 0.0, 3.0] {
        for dx in [-3.0, 0.0, 3.0] {
            let (sx, sy) = (cx + dx, cy + dy);
            let (lx, ly) = to_local(f, now, sx, sy);
            let mut v: f32 = if eyes.iter().any(|e| e.inside(lx, ly)) { 1.0 } else { 0.0 };
            for p in &f.placed {
                if p.alpha > 0.0 && sprite_hit(p, sx, sy) {
                    v = v.max(p.alpha);
                    if p.tint.is_some() {
                        tint = p.tint;
                    }
                }
            }
            sum += v;
        }
    }
    let mut b: f32 = sum / 9.0;
    if f.post.rim > 0.0 && ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() > RIM_R {
        b = b.max(f.post.rim);
    }
    b = b.max(overlay(f, i, j));
    (b, tint)
}

/// The whole matrix: a clear, then for each row a `Dots` of lit cells
/// (radius 3.6) and one of unlit cells (radius 2.3), the other cells
/// transparent in each.
pub fn render_frame(f: &FaceFrame, now: Secs) -> Scene {
    let mut s = Scene::new();
    let clear = Color { a: 0, ..AMBER };
    for j in 0..N {
        let mut lit = Vec::with_capacity(N);
        let mut unlit = Vec::with_capacity(N);
        for i in 0..N {
            let (cx, cy) = (i as f32 * CELL + 5.0, j as f32 * CELL + 5.0);
            let visible = ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() <= VISIBLE_R;
            let (b, tint) = cell_brightness(f, now, i, j);
            if !visible {
                lit.push(clear);
                unlit.push(clear);
            } else if b > LIT_MIN {
                let a = ((0.45 + 0.55 * b) * f.post.flicker).clamp(0.0, 1.0);
                lit.push(tint.unwrap_or_else(|| amber(b)).with_alpha(a));
                unlit.push(clear);
            } else {
                lit.push(clear);
                unlit.push(AMBER.with_alpha(0.08));
            }
        }
        let cy = j as f32 * CELL + 5.0;
        s.push(Drawable::Dots {
            cx: 120.0,
            cy,
            spacing: CELL,
            r: LIT_R,
            colors: lit,
        });
        s.push(Drawable::Dots {
            cx: 120.0,
            cy,
            spacing: CELL,
            r: UNLIT_R,
            colors: unlit,
        });
    }
    s
}
```

- [ ] **Step 4: Run, fix, commit**

Run: `cargo test -p rackscreen-core scene_face::` then fmt and clippy.
Expected: 9 passed. If `content_eyes...` complains about `(7, 12)`: the left eye centre is at x = 70 → cell 6.5, so cells 6 and 7 both cover it; the test uses 7 because its 3×3 samples at 72..78 are all inside. If `a_blink...` finds three rows, check `open.max(0.04)` is applied before the height, not after.

```bash
git add crates/core/src/scene_face.rs
git commit -m "feat(core): rasterise the face into dot rows

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Model integration

**Files:**
- Modify: `crates/core/src/model.rs` (struct, `new`, `set_rng_seed`, `tick`, `apply` link arm, new setters, `face_inputs`, tests)
- Modify: `crates/core/src/scene_face.rs` (`face_scene`)

**Interfaces:**
- Consumes: `face_player::{FacePlayer, FaceInputs, FaceTuning}`, `mood::MoodInputs`, `electricity::price_level`, `event::AppHealth`, `scene_face::render_frame`.
- Produces: `Model::face(&self) -> &FacePlayer`, `Model::set_face_tuning(&mut self, t: FaceTuning)`, `Model::set_bedtime_near(&mut self, near: bool)`, `Model::face_inputs(&self, now: Secs) -> FaceInputs`; `scene_face::face_scene(model, now)` renders the player's frame.

- [ ] **Step 1: Write the failing tests**

Append to `model::tests`:

```rust
    #[test]
    fn fx_requests_reach_the_face() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::PodCrashed {
                ns: "a".into(),
                name: "b".into(),
            },
            1.0,
        );
        m.tick(1.0);
        assert_eq!(m.face().current_act(), Some(crate::face_acts::ActKind::Ouch));
        assert!(m.fx().splashes[Role::Pods.index()].active().is_some(), "the splash still plays too");
    }

    #[test]
    fn link_down_reaches_the_face_and_link_up_after_boot_does_too() {
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
                target: LinkTarget::K8sApi,
                up: false,
            },
            1.0,
        );
        m.tick(1.0);
        assert_eq!(m.face().current_act(), Some(crate::face_acts::ActKind::Hello));
        let mut t = 1.0;
        while t < 5.0 {
            m.tick(t);
            t += 0.1;
        }
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            5.0,
        );
        m.tick(5.0);
        assert_eq!(m.face().current_act(), Some(crate::face_acts::ActKind::FoundYou));
    }

    #[test]
    fn face_inputs_reduce_cluster_state() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 4,
                not_ready: vec!["pi-4".into()],
            },
            0.0,
        );
        m.apply(
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 18.0,
                load_pct: 10.0,
                runtime_secs: 600,
            },
            0.0,
        );
        let i = m.face_inputs(0.0);
        assert!(i.mood.nodes_not_ready && !i.mood.on_battery);
        assert!(i.ups_have && i.ups_charge_pct == 18.0);
        m.tick(1.0);
        assert_eq!(m.face().mood(), crate::mood::Mood::Sad);
        assert_eq!(m.face().current_act(), Some(crate::face_acts::ActKind::OnFumes));
        m.apply(metrics(95.0, None), 2.0);
        assert!(m.face_inputs(2.0).mood.hot, "cpu over hot_cpu");
        m.set_bedtime_near(true);
        m.set_face_tuning(crate::face_player::FaceTuning {
            reaction: 5.0,
            ..Default::default()
        });
        assert_eq!(m.face().tuning().reaction, 5.0);
    }

    #[test]
    fn face_scene_is_dot_rows() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 4,
                total: 4,
                not_ready: vec![],
            },
            0.0,
        );
        m.tick(1.0);
        let s = m.scene_for_role(Role::Face, 1.0);
        assert_eq!(s.items.len(), 49);
        assert_eq!(m.scene_for_role(Role::Face, 1.0), s, "pure in `now`");
    }
```

Run: `cargo test -p rackscreen-core model::tests::f` → compile errors.

- [ ] **Step 2: Wire the player into the model**

In `crates/core/src/model.rs`:

Imports: extend the `use crate::event::{...}` line with `AppHealth`, and add

```rust
use crate::face_player::{FaceInputs, FacePlayer, FaceTuning};
use crate::mood::MoodInputs;
```

Struct field (after `boot_pending: bool,`):

```rust
    /// The `face` role's act player; fed the same fx requests as the splashes.
    face: FacePlayer,
```

`Model::new`: add `face: FacePlayer::new(0.0),` after `boot_pending: false,`.

`set_rng_seed` becomes:

```rust
    pub fn set_rng_seed(&mut self, seed: u64) {
        self.rng = seed | 1;
        self.face.set_seed(seed);
    }
```

(Check the existing body first; keep whatever it does to `self.rng` and add the `face` line.)

New accessors next to `night_override()`:

```rust
    pub fn face(&self) -> &FacePlayer {
        &self.face
    }
    pub fn set_face_tuning(&mut self, t: FaceTuning) {
        self.face.set_tuning(t);
    }
    /// Half an hour before night and ten minutes after: the face is sleepy.
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.face.set_bedtime_near(near);
    }

    /// The cluster facts the face reads each frame.
    pub fn face_inputs(&self, now: Secs) -> FaceInputs {
        let st = &self.state;
        let th = self.thresholds;
        let hot = (st.have_temps && self.hot_temp.value(now) >= th.hot_temp)
            || (st.have_metrics && (st.cpu_pct >= th.hot_cpu || st.mem_pct >= th.hot_mem));
        FaceInputs {
            mood: MoodInputs {
                on_battery: self.ups.have && self.ups.on_battery,
                nodes_not_ready: !st.nodes_not_ready.is_empty(),
                app_degraded: self.apps.apps.iter().any(|a| a.health == AppHealth::Degraded),
                volume_degraded: st
                    .volumes
                    .iter()
                    .any(|(_, r)| matches!(r, Robustness::Degraded | Robustness::Faulted)),
                alerts: !st.alerts.is_empty(),
                hot,
                pods_failed: st.pods_failed > 0,
            },
            ups_have: self.ups.have,
            ups_charge_pct: self.ups.charge_pct,
            storage_have: st.have_storage,
            storage_pct: self.storage_pct.value(now),
            gh_have: self.github.have,
            gh_today: self.github.today(),
            price_level: if self.prices.have {
                self.current_price().map(|p| price_level(p, self.prices.avg).0)
            } else {
                None
            },
            weather_have: self.weather.have,
            weather_code: self.weather.code,
            weather_temp_c: self.weather.temp_c,
            weather_gust_kmh: self.weather.gust_kmh,
            weather_is_day: self.weather.is_day,
            sky_have: self.sky.have,
            sunrise: self.sky.sunrise,
            sunset: self.sky.sunset,
            moon_illumination: self.sky.moon_illumination,
            unix_now: self.unix_now,
        }
    }
```

(`self.prices.avg` is the `PriceState` field named `avg`; if `current_price` already folds `have` in, the outer `if` is harmless.)

In `tick`, replace

```rust
        for req in std::mem::take(&mut self.fx) {
            self.fx_state.apply(req, now);
        }
```

with

```rust
        for req in std::mem::take(&mut self.fx) {
            self.face.on_fx(&req, now);
            self.fx_state.apply(req, now);
        }
        let inputs = self.face_inputs(now);
        self.face.tick(&inputs, now);
```

In `apply`, the `LinkTarget::K8sApi` arm: after `let was = self.link.api; self.link.api = up;` add

```rust
                    if was && !up {
                        self.face.on_link_down(now);
                    }
```

In `crates/core/src/scene_face.rs` replace the stub:

```rust
pub fn face_scene(model: &Model, now: Secs) -> Scene {
    render_frame(&model.face().frame(now), now)
}
```

- [ ] **Step 3: Run the whole core crate, fix, commit**

Run: `cargo test -p rackscreen-core` then fmt and clippy.
Expected: green. `pod_events_request_fx` and friends keep passing: `on_fx` only reads the request.

```bash
git add crates/core/src/model.rs crates/core/src/scene_face.rs
git commit -m "feat(core): the model feeds the face player and renders its scene

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: Golden frames

**Files:**
- Modify: `crates/render/tests/golden.rs`
- Create: `crates/render/tests/goldens/face_content.png`, `face_sad.png`, `face_ouch_mid.png`, `face_launch_mid.png`, `face_sunny_shades.png`, `face_sleepy.png` (written by the first run)

**Interfaces:**
- Consumes: `Model::{apply, tick, set_bedtime_near, scene_for_role}`, `Renderer::render`, `check`, `pixel`, `ready_model`.

- [ ] **Step 1: Write the tests**

Append to `crates/render/tests/golden.rs`:

```rust
fn tick_until(m: &mut Model, from: f64, to: f64) {
    let mut t = from;
    while t < to {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    m.tick(to);
}

fn render_face(m: &Model, now: f64) -> Pixmap {
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Face, now), &mut px);
    px
}

#[test]
fn face_content_and_sad() {
    let mut m = ready_model();
    tick_until(&mut m, 0.0, 5.0);
    let px = render_face(&m, 5.0);
    let (red, g, b) = pixel(&px, 75, 125);
    assert!(red > 200 && g > 140 && b < 80, "left eye amber, got {red},{g},{b}");
    let (r2, ..) = pixel(&px, 120, 125);
    assert!(r2 < 60, "gap between the eyes is dark, got {r2}");
    assert_eq!(pixel(&px, 4, 4), (0, 0, 0));
    check("face_content", &px);

    m.apply(
        Event::NodeSnapshot {
            ready: 3,
            total: 4,
            not_ready: vec!["pi-4".into()],
        },
        5.0,
    );
    tick_until(&mut m, 5.0, 8.0);
    check("face_sad", &render_face(&m, 8.0));
}

#[test]
fn face_mid_act_frames() {
    let mut m = ready_model();
    tick_until(&mut m, 0.0, 2.0);
    m.apply(
        Event::PodCrashed {
            ns: "media".into(),
            name: "sonarr-0".into(),
        },
        2.0,
    );
    // 55 % into the 2.8 s act: X eyes, box in the slot
    tick_until(&mut m, 2.0, 2.0 + 0.55 * 2.8);
    let px = render_face(&m, 2.0 + 0.55 * 2.8);
    let (_, _, b) = pixel(&px, 125, 175);
    assert!(b > 150, "the box is kubernetes blue, got b={b}");
    check("face_ouch_mid", &px);

    let mut m = ready_model();
    tick_until(&mut m, 0.0, 2.0);
    m.apply(Event::AppSynced { name: "media".into() }, 2.0);
    // 34 % in: the rocket is mid-screen (at 50 % it has already left the top)
    tick_until(&mut m, 2.0, 2.0 + 0.34 * 2.8);
    check("face_launch_mid", &render_face(&m, 2.0 + 0.34 * 2.8));
}

#[test]
fn face_sunny_shades_and_sleepy() {
    let mut m = ready_model();
    tick_until(&mut m, 0.0, 2.0);
    m.apply(
        Event::Weather {
            temp_c: 21.0,
            code: 0,
            is_day: true,
            wind_kmh: 8.0,
            gust_kmh: 12.0,
            wind_from_deg: 180.0,
            at: "2026-09-07T12:00".into(),
        },
        2.0,
    );
    // the weather habit starts on the next tick; 60 % in the shades are on
    m.tick(2.0);
    tick_until(&mut m, 2.0, 2.0 + 0.6 * 3.2);
    check("face_sunny_shades", &render_face(&m, 2.0 + 0.6 * 3.2));

    let mut m = ready_model();
    m.set_bedtime_near(true);
    tick_until(&mut m, 0.0, 5.0);
    let px = render_face(&m, 5.0);
    let (red, ..) = pixel(&px, 75, 95);
    assert!(red < 60, "sleepy: the top of the eye is closed, got {red}");
    check("face_sleepy", &px);
}
```

- [ ] **Step 2: Run to write the goldens, inspect, run again**

Run: `cargo test -p rackscreen-render --test golden face_ 2>&1 | tail -12`
Expected: `wrote golden .../face_content.png` and five more, tests pass. Open the six PNGs (`crates/render/tests/goldens/face_*.png`) and check: two amber eyes; sad with the inner corners up; X eyes and a blue box for ouch; an orange rocket above the slot for launch; two amber bars for shades with a gold sun in the slot; slits for sleepy. If any pixel assertion fails, adjust the coordinate to the visible feature, not the threshold.

Run again: `cargo test -p rackscreen-render --test golden`
Expected: all pass against the stored files.

- [ ] **Step 3: Commit**

```bash
git add crates/render/tests/golden.rs crates/render/tests/goldens/face_*.png
git commit -m "test(render): golden frames for the face role

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 13: Config, validation and runloop plumbing

**Files:**
- Modify: `crates/app/src/config.rs` (struct, default, validate, tests), `config.example.yaml`, `crates/app/src/runloop.rs`, `crates/app/src/run.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct FaceCfg { pub bored_after_mins: u32, pub reaction_secs: u32, pub idle_habits: bool, pub weather_habits: bool }   // Default 120, 60, true, true
  impl FaceCfg { pub fn tuning(&self) -> rackscreen_core::face_player::FaceTuning }
  Config { pub face: FaceCfg, .. }
  RenderLoop { pub face: FaceTuning, .. }
  ```

- [ ] **Step 1: Write the failing tests**

Append to the tests in `crates/app/src/config.rs`:

```rust
    #[test]
    fn face_defaults_and_validation() {
        let c = Config::default();
        assert_eq!(c.face.bored_after_mins, 120);
        assert_eq!(c.face.reaction_secs, 60);
        assert!(c.face.idle_habits && c.face.weather_habits);
        let t = c.face.tuning();
        assert_eq!(t.bored_after, 7200.0);
        assert_eq!(t.reaction, 60.0);
        let mut bad = Config::default();
        bad.face.bored_after_mins = 4;
        assert!(bad.validate().unwrap_err().to_string().contains("face.bored_after_mins"));
        let mut bad = Config::default();
        bad.face.reaction_secs = 0;
        assert!(bad.validate().unwrap_err().to_string().contains("face.reaction_secs"));
        let mut bad = Config::default();
        bad.face.reaction_secs = 601;
        assert!(bad.validate().is_err());
        let c: Config = serde_yaml::from_str("face:\n  idle_habits: false\n").unwrap();
        assert!(!c.face.idle_habits && c.face.weather_habits);
    }
```

Run: `cargo test -p rackscreen-app face_defaults` → compile error.

- [ ] **Step 2: Add the config**

In `crates/app/src/config.rs`, add the field to `Config` after `argocd`:

```rust
    #[serde(default)]
    pub face: FaceCfg,
```

Add the struct after `ArgocdCfg` and its `Default` after `impl Default for ArgocdCfg`:

```rust
/// The `face` role: how long it stays in a mood and which habits it has.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct FaceCfg {
    /// Minutes without a cluster event before the face is bored.
    pub bored_after_mins: u32,
    /// Seconds an act's mood holds afterwards.
    pub reaction_secs: u32,
    /// Hum, peek, stretch, scan, sneeze, hiccup, doze off on a random timer.
    pub idle_habits: bool,
    /// Replay the current weather as an act every few minutes.
    pub weather_habits: bool,
}

impl FaceCfg {
    pub fn tuning(&self) -> rackscreen_core::face_player::FaceTuning {
        rackscreen_core::face_player::FaceTuning {
            bored_after: self.bored_after_mins as f64 * 60.0,
            reaction: self.reaction_secs as f64,
            idle_habits: self.idle_habits,
            weather_habits: self.weather_habits,
        }
    }
}

impl Default for FaceCfg {
    fn default() -> Self {
        Self {
            bored_after_mins: 120,
            reaction_secs: 60,
            idle_habits: true,
            weather_habits: true,
        }
    }
}
```

In `validate`, after the `price.poll_secs` check:

```rust
        anyhow::ensure!(
            self.face.bored_after_mins >= 5,
            "face.bored_after_mins must be at least 5 (got {})",
            self.face.bored_after_mins
        );
        anyhow::ensure!(
            (1..=600).contains(&self.face.reaction_secs),
            "face.reaction_secs must be between 1 and 600 (got {})",
            self.face.reaction_secs
        );
```

In `config.example.yaml`, after the `argocd:` block (at the end of the sections, before `screens:`):

```yaml
face:
  bored_after_mins: 120    # no cluster event for this long and the face role looks bored
  reaction_secs: 60        # how long an act's mood (excited, sad, ...) holds afterwards
  idle_habits: true        # hum, peek, stretch, scan, sneeze, hiccup, doze off now and then
  weather_habits: true     # replay the current weather as an act every few minutes (needs weather.enabled)
```

Run: `cargo test -p rackscreen-app config::` → green (`example_parses_with_four_screens` reads the example file and still validates).

- [ ] **Step 3: Runloop and run.rs**

In `crates/app/src/runloop.rs`: add `use rackscreen_core::face_player::FaceTuning;` and `use rackscreen_core::night::{bedtime_near, is_night};` (replacing the existing `is_night` import). Add a pure helper next to `NightWindow` with its test:

```rust
/// Sleepy around the night window; never when night is disabled or forced.
pub fn face_bedtime(night: &NightWindow, minutes: u32, override_: Option<bool>) -> bool {
    night.enabled && override_.is_none() && bedtime_near(minutes, night.start_min, night.end_min)
}

#[cfg(test)]
mod bedtime_tests {
    use super::*;

    #[test]
    fn bedtime_needs_night_enabled_and_no_override() {
        let on = NightWindow { enabled: true, start_min: 1380, end_min: 420 };
        let off = NightWindow { enabled: false, ..on };
        assert!(face_bedtime(&on, 1360, None));
        assert!(!face_bedtime(&off, 1360, None));
        assert!(!face_bedtime(&on, 1360, Some(false)));
        assert!(!face_bedtime(&on, 720, None));
    }
}
```

(If `NightWindow` does not derive `Clone`/`Copy`, add `#[derive(Clone, Copy, Debug)]` to it.) Add to `RenderLoop`:

```rust
    /// `face:` config, passed to the model once.
    pub face: FaceTuning,
```

After `model.set_github_token_present(self.github_token_present);` add `model.set_face_tuning(self.face);`. In the frame loop, after `model.set_utc_offset_secs(offset);` and before `model.tick(now);` add:

```rust
            model.set_bedtime_near(face_bedtime(&self.night, minutes, model.night_override()));
```

In `crates/app/src/run.rs`, in the `RenderLoop { ... }` literal add `face: cfg.face.tuning(),` after `one_at_a_time: cfg.display.one_at_a_time,`.

If `runloop.rs` has unit tests that build a `RenderLoop` literal, add `face: FaceTuning::default(),` there too (grep `RenderLoop {` in `crates/app/src`).

- [ ] **Step 4: Run, commit**

Run: `cargo test --workspace` then fmt and `cargo clippy --workspace --all-targets -- -D warnings`.
Expected: green, clean.

```bash
git add crates/app/src/config.rs config.example.yaml crates/app/src/runloop.rs crates/app/src/run.rs
git commit -m "feat(app): face config and bedtime plumbing

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 14: TUI Configure fields

**Files:**
- Modify: `crates/setup/src/screens/configure.rs` (`Field` enum, `FIELDS`, `get`, `set`, tests)

**Interfaces:**
- Produces: `Field::{FaceBored, FaceReaction, FaceIdle, FaceWeather}` in the `Thresholds` group, section `"Face"`; `FIELDS.len() == 47`; `group_indices(Group::Thresholds).len() == 7`.

- [ ] **Step 1: Update the tests**

In `configure.rs` tests: change both `assert_eq!(FIELDS.len(), 43);` to `47` and `assert_eq!(group_indices(Group::Thresholds).len(), 3);` to `7`. Add a test:

```rust
    #[test]
    fn face_fields_round_trip_and_clamp() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::FaceBored), "120");
        assert_eq!(get(&c, Field::FaceReaction), "60");
        assert_eq!(get(&c, Field::FaceIdle), "true");
        assert_eq!(get(&c, Field::FaceWeather), "true");
        set(&mut c, Field::FaceBored, "3").unwrap();
        assert_eq!(c.face.bored_after_mins, 5, "clamped to the minimum");
        set(&mut c, Field::FaceReaction, "900").unwrap();
        assert_eq!(c.face.reaction_secs, 600);
        set(&mut c, Field::FaceReaction, "0").unwrap();
        assert_eq!(c.face.reaction_secs, 1);
        assert!(set(&mut c, Field::FaceBored, "soon").is_err());
        set(&mut c, Field::FaceIdle, "false").unwrap();
        assert!(!c.face.idle_habits);
        assert_eq!(section_enabled(&c, "Face"), None, "no `enabled` toggle for the face");
        let face: Vec<Field> = FIELDS
            .iter()
            .filter(|s| s.section == "Face")
            .map(|s| s.field)
            .collect();
        assert_eq!(
            face,
            vec![Field::FaceBored, Field::FaceReaction, Field::FaceIdle, Field::FaceWeather]
        );
        assert!(FIELDS.iter().filter(|s| s.section == "Face").all(|s| s.group == Group::Thresholds));
    }
```

Run: `cargo test -p rackscreen-setup configure::` → compile error.

- [ ] **Step 2: Add the fields**

`Field` enum: append after `ArgoNamespace`:

```rust
    FaceBored,
    FaceReaction,
    FaceIdle,
    FaceWeather,
```

`FIELDS`: append after the `Field::HotTemp` spec (the Thresholds group is last, so these stay contiguous):

```rust
    spec(
        Field::FaceBored,
        Thresholds,
        "Face",
        "bored after min",
        Number,
        "Minutes without a cluster event before it looks bored.",
    ),
    spec(
        Field::FaceReaction,
        Thresholds,
        "Face",
        "reaction secs",
        Number,
        "How long an act's mood holds afterwards, 1 to 600.",
    ),
    spec(
        Field::FaceIdle,
        Thresholds,
        "Face",
        "idle habits",
        Bool,
        "Hum, peek, stretch, scan, sneeze, doze off now and then.",
    ),
    spec(
        Field::FaceWeather,
        Thresholds,
        "Face",
        "weather habits",
        Bool,
        "Replay the current weather every few minutes.",
    ),
```

(Every help string must be 60 characters or fewer; these are.)

`get`: after the `Field::HotTemp` arm:

```rust
        Field::FaceBored => cfg.face.bored_after_mins.to_string(),
        Field::FaceReaction => cfg.face.reaction_secs.to_string(),
        Field::FaceIdle => cfg.face.idle_habits.to_string(),
        Field::FaceWeather => cfg.face.weather_habits.to_string(),
```

`set`: after the `Field::HotTemp` arm:

```rust
        Field::FaceBored => cfg.face.bored_after_mins = num::<u32>(t, "minutes")?.max(5),
        Field::FaceReaction => cfg.face.reaction_secs = num::<u32>(t, "seconds")?.clamp(1, 600),
        Field::FaceIdle => cfg.face.idle_habits = t == "true",
        Field::FaceWeather => cfg.face.weather_habits = t == "true",
```

- [ ] **Step 3: Run, commit**

Run: `cargo test -p rackscreen-setup` then fmt and clippy.
Expected: green. If a Configure layout test asserts a scroll position or row count that shifted by four, update the number and note it in the commit body.

```bash
git add crates/setup/src/screens/configure.rs
git commit -m "feat(setup): face fields in the Configure thresholds group

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 15: README, gifs, version

**Files:**
- Modify: `README.md`, `crates/app/examples/gifs.rs`, `Cargo.toml`
- Regenerate: `.github/media/screens/face.gif`, `.github/media/screens/all.gif`

- [ ] **Step 1: Version**

In `Cargo.toml` change `version = "0.5.0"` under `[workspace.package]` to `version = "0.6.0"`. Run `cargo build --workspace` so `Cargo.lock` follows, and check `git diff --stat Cargo.lock` only touches the workspace crates.

- [ ] **Step 2: Gif script**

In `crates/app/examples/gifs.rs` `clips()`, add an arm before `_ => Vec::new(),`:

```rust
                Role::Face => vec![
                    (1.0, Step::Cmd(FakeCmd::PodCrashed)),
                    (4.5, Step::Cmd(FakeCmd::GithubStar)),
                ],
```

Every role clip is 8 s, so the face GIF shows the ouch act, the starry-eyes act, and the excited mood after. `all.gif` picks the new role up from `Role::ALL` (its length is derived).

Run: `cargo run -p rackscreen-app --example gifs -- .github/media/screens 2>&1 | tail -4`
Expected: 25 GIFs listed, `face.gif` among them, `all.gif` rewritten. View `.github/media/screens/face.gif` and confirm the box hits the face and the stars appear.

- [ ] **Step 3: README**

`README.md`:

1. Screens GIF table (line ~52): replace the last row

   ```markdown
   | <img src=".github/media/screens/deploys.gif" alt="DEPLOYS role" width="200"> | |
   | **`deploys`** — one arc per Argo CD application; a sync splashes the rocket | |
   ```

   with

   ```markdown
   | <img src=".github/media/screens/deploys.gif" alt="DEPLOYS role" width="200"> | <img src=".github/media/screens/face.gif" alt="FACE role" width="200"> |
   | **`deploys`** — one arc per Argo CD application; a sync splashes the rocket | **`face`** — two eyes on a dot matrix; every event gets its own act, the cluster's health its mood |
   ```

2. "Screens and roles": `Twenty-one roles exist:` → `Twenty-two roles exist:` and append to the role table:

   ```markdown
   | `face` | everything above | two eyes on a 24×24 dot matrix; one short act per event with the source's icon, the cluster's health as its mood between acts |
   ```

3. New section after "GitHub role" (before "## Configuration"):

   ```markdown
   ## Face role

   `face` is a pet: two amber eyes on a 24×24 dot matrix that blink, glance around, tilt their head, and play a short **act** for every event the other roles only splash. A crashed pod drops a box on its head (X eyes); a node going down slides a rack in with a cross and a tear; a GitHub push is a commit it catches between the eyes; a release is confetti; an Argo sync launches a rocket; a finished torrent is a package it hugs; mains loss flickers the whole matrix; a hot node makes it sweat; thunder makes it jump; a cheap electricity hour turns its eyes into euro signs. Fifty-six acts in all, each with the source's icon in that source's colour in a slot at the bottom of the screen. While the weather is on, it replays the current conditions every few minutes (shades in the sun, shivering below zero, catching snowflakes), and when nothing happens it hums, peeks off the edge, stretches, scans, sneezes, or dozes off when it has been bored for hours.

   Between acts the eyes hold a **mood** from the cluster: scared on battery, sad with a node down, angry with a degraded app, volume or firing alert, hot over the thresholds, worried after a crash, sleepy half an hour before night mode, bored after `face bored after min` without an event, content otherwise. An act's mood (excited, happy, worried) holds `face reaction secs` afterwards unless the cluster state is worse. `face idle habits` and `face weather habits` turn the no-event acts off.
   ```

4. Configuration `<details>` block: add the `face:` YAML (same four lines and comments as `config.example.yaml`) after the `argocd:` block.

- [ ] **Step 4: Final checks and commit**

Run: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
Expected: all green and clean.

```bash
git add README.md crates/app/examples/gifs.rs Cargo.toml Cargo.lock .github/media/screens/face.gif .github/media/screens/all.gif
git commit -m "docs: face role in the README and gifs, version 0.6.0

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

# Look Refresh Implementation Plan (remove face, Outfit, top icons)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the face feature completely, switch every screen from Inter to Outfit, and replace the curved title at the top of every screen with a small Material Symbols icon, closing the ring gap from 60° to 32°.

**Architecture:** Task 1 deletes the face role, the vector face, the mood engine and its inputs, with a config shim so a `face` entry in an old roles list is dropped with a warning. Task 2 swaps the three embedded font files. Task 3 vendors Material Symbols Rounded (weight 700) SVGs, maps each role to one, draws it as `Drawable::Icon` at 12 o'clock and moves the ring ends to 16° / 344°.

**Tech Stack:** Rust 2021 workspace, fontdue, usvg, tiny-skia.

Spec: `docs/superpowers/specs/2026-11-03-face-removal-outfit-icons-design.md`.

## Global Constraints

- Face removal: `Role::Face`, `face.rs`, `face_expr.rs`, `scene_face.rs`, `mood.rs` and every mood input are deleted. A `face` entry in a screen's `roles` list is dropped with one `warn` line ("face role was removed; ignoring"); a screen whose list becomes empty is a config error naming the screen.
- Font: Outfit static TTFs Medium 500, SemiBold 600, Bold 700 plus `Outfit-LICENSE.txt` (OFL 1.1) in `assets/fonts/`; Inter files and licence removed; `FontId` keeps `Medium`, `SemiBold`, `Bold`.
- Icons: Material Symbols Rounded, weight 700, 18 px, colour `TEXT2`, centred at `(CX, CY − RING_R)`; SVGs in `assets/icons/material/` (or named `ms-<name>.svg` beside the existing icons, see Task 3) with `MATERIAL-LICENSE.txt` (Apache 2.0).
- Ring: `RING_START_DEG` 30 → 16, `RING_SWEEP_DEG` 300 → 328.
- Icon per role, exactly:

| role | icon | role | icon |
|---|---|---|---|
| cpu | memory | wind | air |
| mem | memory_alt | aqi | aq |
| pods | deployed_code | rain | rainy |
| health | monitor_heart | sun | wb_sunny |
| thermal | thermostat | moon | dark_mode |
| storage | hard_drive | iss | satellite_alt |
| power-mix | bolt | ups | battery_charging_full |
| price | euro | net | lan |
| carbon | co2 | deploys | rocket_launch |
| renewable | eco | media | video_library |
| gh-activity | commit | traffic | swap_vert |
| weather | partly_cloudy_day | restarts | restart_alt |
| uptime | timer | fans | mode_fan |
| mains | electrical_services | load | speed |
| ci | build | jellyfin | movie |

  (`music` → `music_note` and `gif` → `gif_box` are added by the music and GIF plans.) The connecting screen of an out-of-range screen index uses `sync`.
- Goldens: Task 1 only deletes face goldens (others byte-identical; compare exactly, the golden test has a tolerance). Tasks 2 and 3 intentionally re-capture all goldens with `UPDATE_GOLDENS=1` and regenerate the README gifs; the implementer looks at a sample of the new PNGs.
- Contrast floor test keeps passing.
- No new crates. Version 0.10.0 at the end (Task 3).
- Commit after every task; messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Before every commit: `cargo test --workspace -q`, `cargo clippy --workspace --all-targets -q -- -D warnings`, `cargo fmt --check`.

---

### Task 1: Remove the face feature

**Files (expected; follow the compiler):**
- Delete: `crates/core/src/face.rs`, `crates/core/src/face_expr.rs`, `crates/core/src/scene_face.rs`, `crates/core/src/mood.rs`
- Modify: `crates/core/src/lib.rs`, `theme.rs` (`Role::Face` and its arms/tests; `Role::ALL` length 31 → 30), `model.rs` (face/mood fields, `set_bedtime_near`, `mood_inputs`, `face_mood`, `face_frame`, the CI-pass and restart "mood" state and constants if nothing else reads them, tests), `scene.rs` (`role_scene` face arm), `scene_model.rs` (face gate), `p2/roles/mod.rs`, `night.rs` (`bedtime_near` and its test if unused)
- Modify: `crates/app/src/runloop.rs` (`face_bedtime`, `NightWindow` use for bedtime, `set_bedtime_near` call, `bedtime_tests`), `crates/app/src/config.rs` (roles shim + tests), `crates/app/examples/gifs.rs` (face clip)
- Modify: `crates/sources/src/fake.rs` only if it emits face-only events
- Modify: `crates/render/tests/golden.rs` (face cases) and delete their PNGs under `crates/render/tests/goldens/`
- Delete: `.github/media/screens/face.gif`; Modify: `README.md` (face rows/sections)
- Modify: setup TUI files only if they name the face role explicitly (`grep -rn "Face\|face" crates/setup/src`)

**Interfaces:**
- Produces: `ScreenCfg::roles()` drops `face` (case-insensitive) with `tracing::warn!("face role was removed; ignoring")` once per screen that had it, and errors with `"screen {i} has no roles left after dropping the removed face role"` (use whatever screen identifier the existing error messages use) when nothing remains. `Role::ALL: [Role; 30]`.

- [ ] **Step 1: Failing config tests**

In `crates/app/src/config.rs` tests (adapt the YAML to the existing test helpers' style):

```rust
    #[test]
    fn a_face_role_is_dropped_from_a_roles_list() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [cpu, face, mem], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!(c.screens[0].roles().unwrap(), vec![Role::Cpu, Role::Mem]);
    }

    #[test]
    fn a_roles_list_with_only_face_is_an_error() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [face], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        let e = c.screens[0].roles().unwrap_err().to_string();
        assert!(e.contains("face"), "{e}");
    }
```

If `Config::from_yaml` validates roles itself, assert on its error instead. Run: `cargo test -p rackscreen-app config -q` — expected FAIL (face still parses as a role).

- [ ] **Step 2: Delete the face and mood code**

Remove the four files and their `mod` lines, then fix every compile error by deleting the face/mood code paths (not by stubbing). Rules:
- `Role::Face` goes from the enum, `ALL`, `name`, `parse`, `is_cluster`, title/colour tables and their tests; tests asserting `Role::ALL[21] == Face` or a count of 31 change to the new list/count.
- Model state used only by the mood engine goes (the CI-pass "happy" window and restart "worried" window constants at `model.rs` ~282–285 and whatever fields feed only them). State also used by a screen (e.g. restart counts on the restarts screen, CI runs on the ci screen) stays.
- `night::bedtime_near` and `runloop::face_bedtime` go if nothing else uses them.
- `ScreenCfg::roles()` filters `face` before `Role::parse` and implements the warn/error from Interfaces.

- [ ] **Step 3: Goldens, gifs, README**

Delete the face cases in `crates/render/tests/golden.rs` and their PNG files. Remove the face clip in `crates/app/examples/gifs.rs` and delete `.github/media/screens/face.gif`; regenerate only `all.gif` (`cargo run --release -q -p rackscreen-app --example gifs -- .github/media/screens`, then `git checkout` any per-role gif that changed only in encoding — none should). Remove face from `README.md` (gallery row, role list, config docs).

Exact-compare every remaining golden against its PNG (throwaway check, not committed): all identical.

- [ ] **Step 4: Run and commit**

Run: `cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo fmt --check`
Check: `grep -rni "face\|mood\|bedtime" crates --include=*.rs` shows only unrelated words (e.g. "surface", "interface", "typeface").

```bash
git add -A crates assets .github README.md config.example.yaml
git commit -m "refactor!: remove the face role, the vector face and the mood engine; old face entries are ignored

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Outfit replaces Inter

**Files:**
- Add: `assets/fonts/Outfit-Medium.ttf`, `Outfit-SemiBold.ttf`, `Outfit-Bold.ttf`, `Outfit-LICENSE.txt`
- Delete: `assets/fonts/Inter-*.ttf`, `assets/fonts/Inter-LICENSE.txt`
- Modify: `crates/core/src/p2/font.rs` (constants and names), any other `include_bytes!` of Inter (`grep -rn "Inter" crates`)
- Modify: tests whose expected widths/positions are font-dependent (hero fit, marquee, pill widths); goldens; gifs; README font credit

**Interfaces:**
- Produces: `p2::font::OUTFIT_MEDIUM`, `OUTFIT_SEMIBOLD`, `OUTFIT_BOLD` (`&[u8]`), replacing `INTER_*`; `FontId` unchanged.

- [ ] **Step 1: Vendor the fonts**

```bash
for f in Outfit-Medium Outfit-SemiBold Outfit-Bold; do
  curl -fsSL -o assets/fonts/$f.ttf https://raw.githubusercontent.com/Outfitio/Outfit-Fonts/main/fonts/ttf/$f.ttf
done
curl -fsSL -o assets/fonts/Outfit-LICENSE.txt https://raw.githubusercontent.com/Outfitio/Outfit-Fonts/main/OFL.txt
git rm -q assets/fonts/Inter-*.ttf assets/fonts/Inter-LICENSE.txt
```

Check each TTF is 70–80 KB and parses (`file assets/fonts/Outfit-*.ttf` says TrueType).

- [ ] **Step 2: Switch the constants**

In `crates/core/src/p2/font.rs` rename the three constants to `OUTFIT_*` pointing at the new files and update their users (`grep -rn "INTER_" crates`). Add a test:

```rust
    #[test]
    fn outfit_digits_share_one_width_per_size() {
        for id in FontId::ALL {
            let w0 = measure(id, 64.0, "0", 0.0);
            for d in '1'..='9' {
                let w = measure(id, 64.0, &d.to_string(), 0.0);
                assert!(w <= w0 + 0.5, "{id:?} digit {d} ({w}) wider than 0 ({w0})");
            }
        }
    }
```

If Outfit's `0` is not the widest digit, change `digit_width` in `p2/text.rs` to the maximum advance over `0..=9` (and keep this test asserting `w <= digit_width(id, 64.0)`), so the rolling hero's fixed cells never clip a digit.

- [ ] **Step 3: Fix font-dependent tests, not behaviour**

Run `cargo test --workspace -q`; tests that fail only because a measured width moved (hero fit worst cases `worst_case_heroes_fit_the_ring`, title fit, marquee thresholds, pill text widths) get their expectations updated if the behaviour still holds (e.g. still fits the ring, still scrolls when too wide). If a worst-case hero no longer fits the ring, that is a real failure: report it, do not loosen the ring rule.

- [ ] **Step 4: Re-capture goldens and gifs, look at them**

```bash
UPDATE_GOLDENS=1 cargo test -q -p rackscreen-render --test golden
cargo test -q -p rackscreen-render --test golden
cargo run --release -q -p rackscreen-app --example gifs -- .github/media/screens
```

Open at least `p2_cpu.png`, `p2_jellyfin.png`, `p2_ups.png` (100 %), `p2_uptime.png` and a status screen with the Read tool and check: digits are not clipped, the hero and unit sit inside the ring, the pill text is centred. Describe what you saw in the report.

README: replace the Inter credit with Outfit (OFL).

- [ ] **Step 5: Commit**

```bash
cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo fmt --check
git add -A assets crates .github README.md
git commit -m "feat: Outfit replaces Inter on every screen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Top icon instead of the curved title; 32° ring gap

**Files:**
- Add: one SVG per icon in the table plus `sync` (see Step 1), `assets/icons/MATERIAL-LICENSE.txt`
- Modify: `crates/render/src/assets.rs` (icon list), `crates/render/src/icons.rs` (only if the 24-unit check or viewBox handling needs it)
- Modify: `crates/core/src/theme.rs` (`Role::icon`)
- Modify: `crates/core/src/p2/layout.rs` (`RING_START_DEG`, `RING_SWEEP_DEG`, add `ICON_PX = 18.0`, `ICON_CY`)
- Modify: `crates/core/src/p2/text.rs` (`top_icon`; remove `curved_title` and `TITLE_*` if unused)
- Modify: `crates/core/src/p2/screen.rs` (`P2Screen::icon`, compose draws the icon)
- Modify: `crates/core/src/p2/roles/mod.rs` (set `icon` from the role), `p2/status.rs` (`connecting_scene(icon, now)`, no-data screens), `p2/roles/power_mix.rs`, `scene_model.rs` (connecting call sites, tests' `title_of` → `icon_of`)
- Modify: tests that look for `CurvedText` titles; goldens; gifs; README; `Cargo.toml` version 0.10.0

**Interfaces:**
- Produces: `Role::icon(self) -> &'static str` (asset name, e.g. `"ms-memory"`); `p2::text::top_icon(name: &'static str) -> Drawable` (`Drawable::Icon { name, cx: CX, cy: ICON_CY, size: ICON_PX, color: TEXT2, alpha: 1.0, scale: 1.0, dy: 0.0 }`); `P2Screen { …, pub icon: &'static str }`; `p2::status::connecting_scene(icon: &'static str, now: Secs) -> Scene`.

- [ ] **Step 1: Vendor the icons**

Icons go beside the existing ones as `assets/icons/ms-<name>.svg` (the `assets.rs` macro includes `assets/icons/<name>.svg`). Material SVGs are `width="48" height="48" viewBox="0 -960 960 960"`; normalise the size to 24 so the existing size check (8 | 16 | 24) and the per-icon scale (`units = tree.size().width()`) work:

```bash
names="memory memory_alt deployed_code monitor_heart thermostat hard_drive bolt euro co2 eco commit partly_cloudy_day air aq rainy wb_sunny dark_mode satellite_alt battery_charging_full lan rocket_launch video_library swap_vert restart_alt timer mode_fan electrical_services speed build movie sync"
for n in $names; do
  curl -fsSL https://unpkg.com/@material-symbols/svg-700@0.23.0/rounded/$n.svg \
    | sed 's/width="48" height="48"/width="24" height="24"/' > assets/icons/ms-$n.svg
done
curl -fsSL -o assets/icons/MATERIAL-LICENSE.txt https://raw.githubusercontent.com/google/material-design-icons/master/LICENSE
grep -L 'width="24"' assets/icons/ms-*.svg   # must print nothing
```

Add the 31 names (`ms-memory`, …, `ms-sync`) to the icon list macro in `crates/render/src/assets.rs`.

- [ ] **Step 2: Failing tests**

`crates/core/src/theme.rs`:

```rust
    #[test]
    fn every_role_has_a_material_icon() {
        for r in Role::ALL {
            assert!(r.icon().starts_with("ms-"), "{r:?}");
        }
        assert_eq!(Role::Cpu.icon(), "ms-memory");
        assert_eq!(Role::Jellyfin.icon(), "ms-movie");
    }
```

`crates/render/src/icons.rs` (or its tests module):

```rust
    #[test]
    fn every_role_icon_is_bundled_and_draws_inside_its_box() {
        let mut cache = IconCache::new().unwrap();
        for r in rackscreen_core::theme::Role::ALL {
            assert!(cache.has(r.icon()), "{r:?} -> {}", r.icon());
            let mut px = tiny_skia::Pixmap::new(40, 40).unwrap();
            cache.draw(&mut px, r.icon(), 20.0, 20.0, 18.0, rackscreen_core::theme::Color::hex(0xFFFFFF), 1.0, 1.0, 0.0);
            let lit: Vec<(u32, u32)> = (0..40u32)
                .flat_map(|y| (0..40u32).map(move |x| (x, y)))
                .filter(|&(x, y)| px.data()[((y * 40 + x) * 4 + 3) as usize] > 0)
                .collect();
            assert!(!lit.is_empty(), "{r:?} draws nothing");
            assert!(lit.iter().all(|&(x, y)| (10..=30).contains(&x) && (10..=30).contains(&y)), "{r:?} spills out of its 18 px box");
        }
    }
```

(Match `draw`'s real parameter order; it is `draw(px, name, cx, cy, size, color, alpha, scale, dy)` in the renderer call.) If the Material viewBox offset (`0 -960`) makes the paths land outside the box, fix it in `load` by applying the tree's root transform / view box (usvg gives `abs_transform` per path; check it includes the view box) and keep this test as the guard.

`crates/core/src/p2/screen.rs`: update `compose_orders_ring_title_hero_unit_pill_card` so `items` contain, after the ring overlay, a `Drawable::Icon { name, cx, cy, size, .. }` with `name == "ms-memory"`, `cx == CX`, `cy == ICON_CY`, `size == ICON_PX`, and no `CurvedText`.

Run: `cargo test --workspace -q` — expected FAIL.

- [ ] **Step 3: Implement**

- `layout.rs`: `RING_START_DEG = 16.0`, `RING_SWEEP_DEG = 328.0`, `pub const ICON_PX: f32 = 18.0;`, `pub const ICON_CY: f32 = CY - RING_R;`.
- `theme.rs`: `Role::icon` with the exact table (prefix `ms-`).
- `text.rs`: `top_icon` as in Interfaces; delete `curved_title`, `TITLE_R`, `TITLE_PX`, `TITLE_LS`, `TITLE_MAX_W` and their tests when no caller remains (`CurvedText` itself stays only if another screen still uses it; otherwise remove the variant and its renderer arm too).
- `screen.rs`: `P2Screen` gains `pub icon: &'static str` (default `""` in `P2Screen::new`); `compose` pushes `top_icon(s.icon)` where it pushed the curved title (after the ring overlay, same position in the order) when `s.icon` is not empty.
- `roles/mod.rs`: the dispatcher sets `screen.icon = role.icon()` on every builder result; `status.rs` `no_data_screen(role, …)` sets it too; `connecting_scene(icon, now)` draws `top_icon(icon)` instead of the "connecting" title (the status text stays). `scene_model.rs` passes `role.icon()` for cluster roles and `"ms-sync"` for an out-of-range screen. `power_mix.rs` draws `top_icon(Role::PowerMix.icon())`.
- Tests that identified a screen by its curved title (`title_of` in `scene_model.rs`, `power_mix.rs`, `status.rs`, `screen.rs`) now identify it by icon name.
- Geometry check test in `screen.rs` or `ring.rs`:

```rust
    #[test]
    fn icon_sits_in_the_ring_gap_clear_of_the_caps() {
        // the ring ends 16° either side of 12 o'clock; its caps are RING_W/2 round
        let half_gap = (360.0 - RING_SWEEP_DEG) / 2.0;
        let a = half_gap.to_radians();
        let cap = (CX + RING_R * a.sin(), CY - RING_R * a.cos());
        let dx = (cap.0 - CX).abs() - ICON_PX / 2.0;
        assert!(dx > RING_W / 2.0, "icon box edge {dx} px from a cap centre");
    }
```

- [ ] **Step 4: Re-capture goldens and gifs, look at them**

```bash
UPDATE_GOLDENS=1 cargo test -q -p rackscreen-render --test golden
cargo test -q -p rackscreen-render --test golden
cargo run --release -q -p rackscreen-app --example gifs -- .github/media/screens
```

Open `p2_cpu.png`, `p2_jellyfin.png`, `p2_power-mix.png`, `p2_sun.png` and one status screen with the Read tool: icon centred in the gap, ring ends clear of it, nothing else moved unexpectedly. Describe in the report.

README: mention the icons (Material Symbols, Apache 2.0) next to the font credit; `Cargo.toml` version `0.10.0`, `cargo build -q` to refresh `Cargo.lock`.

- [ ] **Step 5: Commit**

```bash
cargo test --workspace -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo fmt --check
git add -A assets crates .github README.md Cargo.toml Cargo.lock
git commit -m "feat: Material Symbols icon at the top of every screen, 32° ring gap; version 0.10.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

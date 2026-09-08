# Setup TUI redesign: the Shell (v0.5.0)

Date: 2026-09-08
Status: approved by Silke in brainstorming session (five directions shown in the browser, "Shell" picked; overflow and calibrate variants picked in a second round)
Builds on: `2026-09-06-setup-tui-design.md` (v0.2.0), `2026-09-07-sky-github-cluster-roles-design.md` (v0.4.0)

## Purpose

The setup TUI (`crates/setup`) does everything it needs to, but it looks like a list of lists. Configure is a flat 44-row form with no grouping, Calibrate draws square panels for round displays, and every screen is a bare menu. This redesign keeps every feature and key that exists today and changes how the screens look, how Configure is organised, and how the TUI moves.

Three goals:

1. **Orientation.** A permanent sidebar shows where you are and lets you jump anywhere. Home is a live overview, not a menu.
2. **Simplicity.** Configure is six named groups with a one-line explanation of the focused field. Calibrate shows the actual frame the panel is displaying.
3. **Motion.** Eased highlight bar, rows that settle in when a screen opens, a breathing header glyph, a preview that slides. All time based on the monotonic clock already used by `anim::Slide`.

## Decisions taken during brainstorming

- Five directions were drawn as terminal-faithful mockups (`.superpowers/brainstorm/73814-1788891327/content/tui-designs.html`): Shell (sidebar), Tiles and Wizard, Tabs and Dashboard, Quiet (no borders), Rack (live discs). Silke picked **Shell**.
- Home's Screens block with many roles per screen: **truncate to width with a `+N` count**, one row per screen. Rejected: wrapping (squeezes the log block on 24 rows) and a sliding marquee (idle motion, unreadable at a glance). The dedicated Screens editor always wraps.
- Calibrate panels must be round. Picked the **hybrid**: the selected panel is rendered large from its real pixmap with half-block cells, the other panels are a one-line strip. Rejected: four small live renders (too coarse) and drawn circles only (a symbol, not the panel). Drawn circles remain as the fallback for terminals without UTF-8 or true colour.
- Configure keeps text editing for numbers; no bars or steppers. Bools toggle in place. Fields of disabled modules stay visible (dimmed), they do not fold away. No search. These were offered and not picked.
- Implementation **evolves in place**: the `Screen` trait, `ops`, and their tests stay; the shell lives in `App::draw`, each screen's `draw()` is rewritten, and Configure gains a group model. Rejected: rebuilding the UI layer on a new component model.
- The horizontal slide transition is replaced by a settle-in fade. A whole-pane slide fights a fixed sidebar.

## Layout

Terminal is split into header (1 row), body, footer (1 row). Body is sidebar (16 columns) plus a one-column separator plus the content pane.

```
 RackScreen › Configure                                     ● unsaved
─────────────────────────────────────────────────────────────────────────
    Cluster       │  Prometheus
  ▸ Services      │
    Sky           │    namespace        monitoring
    …             │  ▸ port             9090▌
                  │  ⓘ Port of the Prometheus service inside the cluster.
─────────────────────────────────────────────────────────────────────────
 ↑↓ field   ←→ group   ⏎ edit   s save   Esc back
```

### Header

One row, then a full-width rule. Left: `RackScreen` in the title style, then ` › <screen>` muted when not on Home. Right, in priority order: the status word for the current screen (`● unsaved` amber when a screen is dirty, `● panels live` green in Calibrate when the panels opened), otherwise `v<version>` muted. The breathing ring glyph stays and precedes `RackScreen`.

`Screen` gains `fn status(&self, shared: &Shared) -> Option<(String, StatusTone)>` with `StatusTone::{Ok, Warn}`. `subtitle()` stays as the breadcrumb text.

### Sidebar

16 columns. Two contents:

- **Main menu** (Home and every other screen except Configure): the eight items from `menu::ITEMS` with the eased highlight bar (`Slide`, 0.15 s). Item titles shorten to fit: Install, Calibrate, Screens, Configure, Status, Run here, Update, Uninstall. Update shows an amber `↑` at the right edge when `shared.update` says a newer release exists. The item matching the current screen is drawn in the selected style when focus is in the content; the bar shows the hovered item when focus is in the sidebar.
- **Configure groups** (only while the Configure screen is open): the six group names, same bar. Esc leaves Configure and the main menu returns.

### Focus

Two focus areas: sidebar and content. `Shared` gains `pub focus: Focus` with `Focus::{Sidebar, Content}`.

- On Home, focus is Sidebar. `↑↓`/`jk`/Tab move the bar, Enter opens the hovered screen and sets focus to Content. `q` quits.
- In any other screen, focus is Content and keys go to the screen as today. Esc (and `q` where a screen already treats `q` as back) returns to Home with focus Sidebar. `←` also returns to Home when the screen reports `fn consumes_left(&self) -> bool` false. Configure and Calibrate use `←`, so they return true; everything else returns false.
- In Configure, `←→` change group. The sidebar bar follows the group. The sidebar itself never takes key focus inside Configure.

A screen never has to know about the sidebar. `App::handle` intercepts Esc/`←` only after the screen returned `Action::None` for them, so screens that use Esc for "cancel edit" keep working.

### Footer

One rule then one row of keys from `Screen::keys()`, unchanged. On Home the keys are `↑↓ move   ⏎ open   q quit`.

### Narrow terminals

Below 72 columns the sidebar is hidden and the content pane takes the full width. The header breadcrumb is the only navigation cue; Esc still returns to Home, and Home shows the main menu as a plain list in the content pane (the sidebar's content, drawn in the pane). Below 40 columns or 12 rows the pane shows a one-line `terminal too small` notice. Every widget must survive a 1×1 area (existing test `narrow_terminal_does_not_panic` is extended to the new widgets).

## Home

Replaces the Menu screen's content. The Menu type stays as the screen for `ScreenId::Menu` but draws the overview:

```
  Service      ● active  up 3d 4h                       ↑ v0.4.1
  Boot         ● enabled
  Links        ● k8s  ● prometheus  ● qbittorrent  ○ argocd
               ● weather  ● rain  ● github  ○ electricity  ○ prices

  Screens      1 cpu › mem › pods › health +5    15 s
               2 health                          static
               3 weather › rain › sun › moon +2  20 s
               4 price › carbon                  30 s

  Recent       12:01:04 prom  7 nodes, 3 hot
               12:00:49 k8s   142 pods running
```

- Data is the existing `status::Snapshot`, collected by `status::collect` on a background thread when Home is created and again every 10 s while Home is shown. Until the first snapshot arrives the rows show `collecting…` muted. In `sim` mode the service rows read `not available (sim)`.
- Screen numbers use the panel accent per index: amber, green, blue, violet (`theme::AMBER, GREEN, BLUE, VIOLET`), matching the discs' order on the rack.
- Role list: joined with ` › `, then truncated by whole role names to fit the width left after the number column and the timing column. When at least one role is dropped, ` +N` (faint) is appended with N the number of dropped roles. Timing is `every N s` shortened to `N s`, or `static` for a single role. A pure function `fit_roles(names: &[&str], width: usize) -> String` does the fitting and is unit tested.
- Recent: the last 3 journal lines from the snapshot, faint, timestamps in faint too. The banner (`shared.banner`) replaces the Recent header row while set.
- Update hint sits on the Service row's right edge. `up to date` is not shown on Home (Status keeps it).

## Configure

### Groups

`configure::FIELDS` gains a group and a section per field. A section is the module header shown above its fields.

| Group | Sections (fields) |
|---|---|
| Cluster | Kubernetes (kubeconfig path); Prometheus (namespace, service, port, poll) |
| Services | qBittorrent (enabled, namespace, service, port, user, password, poll); Argo CD (enabled, namespace); GitHub (enabled, token, poll) |
| Energy | Electricity Maps (enabled, zone, token, poll); Prices (source, ENTSO-E token, ENTSO-E zone, incl. VAT, poll) |
| Sky | Location (lat, lon); Weather (enabled, poll); Rain (enabled, poll); ISS (enabled, min elevation) |
| Display | Panels (brightness, fps, spi chunk, one at a time); Night (enabled, start, end) |
| Thresholds | Hot node (cpu %, mem %, temp °C) |

That is all 44 fields; the test `new_fields_round_trip` asserts every `Field` appears in exactly one group.

New types in `configure.rs`:

```rust
pub enum Group { Cluster, Services, Energy, Sky, Display, Thresholds }
pub struct FieldSpec { field: Field, group: Group, section: &'static str, label: &'static str, kind: FieldKind, help: &'static str }
pub const FIELDS: [FieldSpec; 44]
```

Labels drop the module prefix (`namespace`, not `prometheus namespace`). `help` is one sentence, under 60 characters, for the help line. Poll fields display as `every 15 s` (or `every 10 min` when a whole number of minutes) in browse mode and as the raw number while editing.

### Pane

```
  Prometheus
    namespace        monitoring
    service          prometheus-server
  ▸ port             9090▌
    poll             every 15 s

  qBittorrent                                        ● on
    namespace        arr-stack
    …
  ⓘ Port of the Prometheus service inside the cluster.
```

- Section header in the normal (white) style; if the section has an `enabled` field, its state is drawn at the right edge as `● on` (green) or `○ off` (muted). Fields of a section whose `enabled` is off are drawn faint, still editable.
- Field rows: two-space indent, pointer `▸` on the focused row, label padded to 16, value muted. Focused row has the highlight background. Secrets show `•` per character. Bools show `● on` / `○ off`. Choice shows the current value with `‹ ›` around it to hint that Enter cycles.
- The help line is the last row of the pane, `ⓘ` in blue then the help text muted. When `error` is set it replaces the help line in the error style; when a note is set (restart result) it shows green. The config path is no longer shown here (it is in Status).
- The pane scrolls so the focused row stays visible; the section header of the focused section stays visible when it fits.

### Keys

`↑↓`/`jk`/Tab move between fields inside the group, wrapping within the group. `←→` (and `h`/`l`) switch group; the focused field becomes the first field of the new group. Enter or space toggles a bool or cycles a choice; Enter on text or number opens inline edit. Inline edit: type, Backspace, Enter applies with the existing `set` validation, Esc cancels. `s` saves all groups at once through the existing `save`, restart dialog unchanged. Esc in browse mode returns to Home; the dirty flag is not lost (a `● unsaved` confirm dialog asks `Discard changes? y / Esc`).

The header shows `● unsaved` while `dirty`. The dot pulses (see Motion).

## Calibrate

### Hybrid preview

```
  ┃▀▄▀▄…  (30×15 cells)  ┃    1 ok     2 rot 90     3 ok     4 ok
  ┃                      ┃             ▔▔▔▔▔▔▔▔
  ┃      live frame      ┃    Screen 2   rotate 90°   flip off
  ┃                      ┃
  ┃                      ┃    r rotate +90   R rotate −90   f flip
  ┃                      ┃    a apply to all s save         Esc discard
                              Match the preview to the glass:
                              arrow up, amber dot top-right.
```

- The selected panel's oriented frame is drawn with `widgets::pixels(f, area, &pixmap)`: each cell shows two vertical pixels using `▀` with the top pixel as foreground colour and the bottom pixel as background colour (`Color::Rgb`). The 240×240 pixmap is sampled with nearest neighbour into `w × 2h` pixels where `w` and `2h` are chosen to keep a 2:1 cell aspect (a 30×15 area shows a 30×30 sample). Black outside the disc is left as the terminal background so the preview reads as round.
- `Calibrate` keeps one oriented `Pixmap` per panel (`shown: Vec<Pixmap>`) instead of reusing `scratch`, so the preview can be drawn on every frame without re-rendering. `push(i)` renders into `frames[i]`, orients into `shown[i]`, and sends a clone to the mailbox as today.
- The strip lists every panel: number, `ok` (green) when rotate is 0 and hflip false, otherwise the change in amber (`rot 90`, `flip`, `rot 180 flip`). The selected entry is underlined with `▔` on the next row.
- The preview slides in from the direction of travel when the selection changes (`Slide`, 0.2 s, horizontal offset within the preview area). This reuses `anim::Slide`.
- With `screens: []` or when panels failed to open, the preview area shows the error text and the strip is empty; keys behave as today.

### Fallback

`Theme` gains `pub truecolor: bool`, detected from `COLORTERM` containing `truecolor` or `24bit`. When `!unicode || !truecolor`, the preview area shows the drawn circle instead:

```
      ╭─────╮
    ╭─╯     ╰─╮
   ╭╯    ↑    ╰╮
   │     2    ●│
   ╰╮         ╭╯
    ╰─╮     ╭─╯
      ╰─────╯
```

`mini_panel` is replaced by `circle_panel(rotate, hflip, number, unicode) -> Vec<String>` (7 rows, 13 columns; ASCII uses `/ \ | -`), with the same arrow and corner-dot logic and the same tests. The strip and keys are identical in both modes.

### Keys

Unchanged: `1-4` select, `←→↑↓`/Tab move, `r`/`R` rotate, `f` flip, `a` all, `s` save, Esc discard. Calibrate reports `consumes_left() == true`.

## Screens editor

Same behaviour and keys. Redrawn in the pane:

```
  ▸ 1  cpu › mem › pods › health › thermal ›          every 15 s
       storage › net › ups › deploys
    2  health                                          static
    3  weather › rain › sun › moon › wind › aqi        every 20 s
    4  price › carbon                                  every 30 s

    one screen at a time   ● on                        o

  Roles                                Presets  c cluster  e electricity  m mixed  w sky
```

- Roles always wrap onto continuation rows indented under the first role (`wrap_roles(names, width) -> Vec<String>`, unit tested). The timing sits on the first row.
- Screen numbers use the per-index panel colour, as on Home.
- The role picker replaces the Roles block while open: the same `✓n` marks and cursor, plus the role's `Role::name()` in normal style and, at the right edge, the config hint that `role_hint` already produces (for example `needs a GitHub token`) in the warning style.
- Header shows `● unsaved` while dirty. Esc with unsaved changes asks `Discard changes?` like Configure.

## Status, Install, Update, Uninstall, Run here

No behaviour change. Their `draw()` is adjusted to the pane:

- **Status** keeps the full detail: service, boot, binary and config paths, boot files, version and update state, links (two rows), then logs. Rows get the two-space indent and the dot styles already in use. `l` full-screen logs, `r` restart, unchanged.
- **Install, Update, Uninstall** use `step_list` unchanged; the gauge sits under the steps as today. Dialogs (`confirm_dialog`) are unchanged.
- **Run here** unchanged apart from the indent.

## Motion

All animations are time based and only force 60 Hz while something moves (`Screen::animating`, `App::animating`).

| What | Where | How |
|---|---|---|
| Highlight bar | sidebar, Screens rows, Configure fields | `Slide` 0.15 s ease-out on the row index (exists in Menu; generalised into `widgets::bar_row(now)`) |
| Settle in | every content pane on screen enter | Each row's foreground is mixed from `faint` toward its final colour over 0.25 s, row `i` starting `i × 40 ms` late, capped at 12 rows so long lists finish within 0.75 s. Implemented as `anim::Settle { start: Secs }` in `App`, applied by a `fn settle(style, row, now) -> Style` helper the screens call. Replaces the horizontal slide. |
| Header ring | header | `ring_glyph` 2.4 s loop, exists |
| Unsaved dot | header | opacity pulse: colour mixes amber toward dim on a 1.2 s sine, `anim::pulse(now) -> f32` |
| Calibrate preview | Calibrate | `Slide` 0.2 s horizontal offset on selection change |
| Toggle flash | Configure, Screens | a toggled bool draws its `●` in white for 0.15 s before settling to green or muted |
| Progress gauge | Install, Update, Uninstall | `Slide` on ratio, exists |
| Spinner | step list, restart | `spinner_frame`, exists |

Colour mixing uses `rackscreen_core::theme::Color::mix`, converted with the existing `rgb()` helper; `Theme` gains `fn mix(&self, from: Color, to: Color, t: f32) -> Color` for ratatui colours.

## Code structure

`crates/setup/src/`

- `lib.rs`: `App` gains `focus`, `settle`, and the shell layout in `draw()` (header, sidebar or none by width, pane, footer); `handle()` routes sidebar keys on Home and intercepts Esc/`←` after the screen. `Screen` trait gains `status()`, `consumes_left()` with defaults.
- `anim.rs`: `Settle`, `pulse`, unchanged `Slide`, `spinner_frame`, `ring_glyph`.
- `theme.rs`: `truecolor`, `mix`, and a `highlight` background colour (`#1b170e`) for the focused row.
- `widgets.rs`: `header` (new signature with status), `sidebar`, `footer`, `bar_row`, `field_row`, `help_line`, `pixels`, `circle_panel`, `step_list`, `confirm_dialog`. Each widget gets a `TestBackend` test at 1×1, 40×10 and 80×24.
- `screens/menu.rs`: Home drawing, snapshot thread, `fit_roles`.
- `screens/configure.rs`: `Group`, `FieldSpec`, group navigation, help line, humanised poll, discard dialog.
- `screens/calibrate.rs`: `shown` pixmaps, hybrid draw, `circle_panel`, preview slide.
- `screens/screens.rs`: `wrap_roles`, pane draw, discard dialog.
- Other screens: draw adjustments only.

Nothing changes in `ops/`, `crates/app`, `crates/core`, `crates/render` or `crates/display`. The `Color::mix` helper in `core::theme` is reused, not modified.

## Testing

- Every existing test in `crates/setup` keeps passing; tests that assert on drawn text (`menu_renders_items_and_pointer`, calibrate `mini_panel` tests) are updated to the new output.
- New pure-function tests: `fit_roles` (fits exactly, drops whole names, `+N` count, width smaller than one name), `wrap_roles`, `circle_panel` arrow and dot for all 8 orientations, `pixels` sampling (a pixmap with a known top-left pixel colour lands in the first cell's foreground), poll humanising, group membership covers all 44 fields, `Settle` and `pulse` bounds.
- Focus tests in `lib.rs` with `TestBackend`: Enter on Home opens the hovered screen with Content focus; Esc from Configure with no changes returns to Home with Sidebar focus; Esc with changes shows the discard dialog; `←` in Calibrate moves selection and does not leave.
- Width tests: the shell at 100×30 shows the sidebar, at 60×24 hides it, at 30×8 shows the too-small notice; none panic.
- Manual check on the Pi: `rackscreen setup` over SSH in a 24-bit terminal and in `TERM=xterm` without `COLORTERM` (fallback circles), plus `LANG=C` for ASCII.

## Out of scope

- Mouse support.
- Changing any config field, default, validation rule or YAML shape.
- New roles, presets or panel behaviour.
- Reworking `install.sh` or the non-TUI CLI.

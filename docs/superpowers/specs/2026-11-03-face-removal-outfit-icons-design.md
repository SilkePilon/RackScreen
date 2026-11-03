# Remove face, Outfit font, top icon instead of the curved title

Date: 2026-11-03. Status: approved in brainstorm (companion session `.superpowers/brainstorm/1130412-1793723449/`, picks in `fonts.html` and `icons-v2.html`).

## 1. Remove the face feature completely

- Delete `Role::Face`, `face.rs`, `face_expr.rs`, `scene_face.rs`, `mood.rs` and every mood input (CI mood, restart mood, bedtime: `face_bedtime`, `set_bedtime_near`, `bedtime_near` when nothing else uses it).
- Remove face config keys, the setup TUI face entries, face goldens, `face.gif`, README sections and the face fake data.
- Old configs keep working: a `face` entry in a screen's `roles` list is dropped with one `warn` log line ("face role was removed; ignoring"); a screen whose list becomes empty is a config error naming the screen. Silke's Pi config lists `face` on screen 2.
- Tests that only covered face or mood are deleted; tests that used face as a sample role switch to another role.

## 2. Outfit replaces Inter on every screen

- `assets/fonts/` ships Outfit static TTFs for the weights in use (Medium 500, SemiBold 600, Bold 700) and `Outfit-LICENSE.txt` (OFL 1.1). Inter files and licence are removed. Setup TUI is unaffected (terminal font).
- Font IDs keep their roles (medium/semibold/bold); only the files change.
- The rolling hero keeps fixed digit cells: cell width = widest digit advance of Outfit at that size, so digits do not jitter.
- `hero_fit_scale` and the ring-fit test still hold with Outfit's metrics (Outfit's cap height and widths differ; the worst-case heroes are re-checked).
- All goldens and gifs are re-captured.

## 3. Top icon instead of the curved title

- Every role and status screen draws a small icon centred at `(CX, CY − RING_R)` instead of `curved_title`: Material Symbols Rounded, weight 700, 18 px, colour `TEXT2`.
- Ring gap shrinks from 60° to 32°: `RING_START_DEG` 30 → 16, `RING_SWEEP_DEG` 300 → 328. Anything placed relative to the ring ends (`ENDS_R` labels, power-mix leader, sun ring) follows the new ends.
- SVGs from `@material-symbols/svg-700` (rounded) go into `assets/icons/material/` with `MATERIAL-LICENSE.txt` (Apache 2.0). They render through the existing `usvg` `IconCache` (`Drawable::Icon`).
- `curved_title`, `TITLE_R`, `TITLE_PX`, `TITLE_LS`, `TITLE_MAX_W` are removed if no other screen uses curved text; otherwise `CurvedText` stays for those uses only.
- Status screens (`connecting`, no data, check config, no token) show the role's icon; the status word stays in the existing status text.

Icon per role (`Role::icon() -> &'static str`):

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
| music | music_note | gif | gif_box |

## Testing

- Config: `face` in a roles list is dropped with a warning; a list with only `face` errors.
- Every role has an icon file that parses (`Role::ALL` loop).
- Ring ends at 16° and 344°; the icon sits in the gap without touching the ring caps (icon box vs cap circles).
- Contrast-floor test still passes (icon is `TEXT2`).
- Goldens and gifs re-captured; visual check of cpu, jellyfin, power-mix, sun and a status screen.

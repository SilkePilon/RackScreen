# GIF role implementation plan

Spec: `docs/superpowers/specs/2026-11-03-gif-role-design.md`. Branch `gif-role`.

Decisions taken after the spec (Silke): links are pasted, no API key, no search;
dwell is the screen's `cycle_secs`; fit is cover (centre crop); timing is the
native frame delays with 0 and 10 ms read as 100 ms. Top icon `ms-gif_box`.
The playing GIF is drawn full-circle with no ring and no icon; "Loading" and
"GIF failed" use the normal status screen with the `ms-gif_box` icon.

Every task is TDD: write the test, watch it fail, implement, watch it pass.
Before each commit: `cargo test --workspace -q`,
`cargo clippy --workspace --all-targets -q -- -D warnings`, `cargo fmt --check`.

## Design choice: `Role::Gif(u16)`

`Role` stays `Copy + Hash + Eq` and gains one data-carrying variant,
`Gif(u16)`, the index into the config's `gifs:` list.

- `Role::ALL` stays the fixed `[Role; 28]` of the built-in roles. GIF roles are
  not "all roles": they only exist relative to a config. Code that iterates
  `ALL` (goldens, icon tests, the README gif example, `Role::parse`) keeps its
  meaning unchanged and never meets a `Gif`.
- Places that must offer GIF roles build the list themselves:
  `rackscreen_app::config::Config::pickable_roles()` = `ALL` followed by
  `Gif(0..gifs.len())`. The Screens editor's picker walks that list.
- `Role::name()` stays `&'static str` and returns `"gif"` for every GIF; the
  full `gif:<name>` needs the config, so `Config::role_name(role) -> String`
  produces it (logs, TUI, saving roles back to YAML).
- `Role::parse` only knows built-in names. `gif:<name>` is resolved against
  `gifs:` by `ScreenCfg::roles_in(&gifs)` / `Config::screen_roles(&screen)`.
  `validate` resolves every screen, so an unknown name is a config error that
  names the entry. `ScreenCfg::roles()` stays for callers without the list and
  rejects `gif:` entries; all in-tree callers move to `Config::screen_roles`.
- `Role::index()` keeps the position in `ALL` for built-in roles and returns
  `ALL.len() + i` for `Gif(i)`, so it stays injective; `from_index` only maps
  back built-in roles.
- Exhaustive matches gain an arm: `title` "GIF", `icon` `ms-gif_box`,
  `is_cluster`/`is_sky` false, `needs_data` = not decoded yet,
  `p2::roles::screen` = `None` (the GIF has its own scene).

## Task 1: icon

- Vendor `assets/icons/ms-gif_box.svg` (unpkg material-symbols svg-700 0.23.0,
  48 -> 24). Add `gif_box` to `MS_ICONS` in `scripts/fetch-assets.sh` and
  `"ms-gif_box"` to `crates/render/src/assets.rs`.
- Test: `every_icon_parses_as_svg` covers it once listed.

## Task 2: core types (`crates/core/src/gif.rs`)

```rust
pub const GIF_SIDE: u32 = 240;
pub const MAX_FRAMES: usize = 150;
pub fn delay_ms(centis: u16) -> u32;              // 0 and 1 cs -> 100 ms
pub fn cap_frames<T>(Vec<(T, u32)>, max) -> Vec<(T, u32)>;  // every n-th, skipped delays added
pub fn keep_mask(n: usize, max: usize) -> Vec<bool>; // which source frames survive
pub struct GifFrames { frames: Vec<Vec<u8>>, delays_ms: Vec<u32>, total_ms: u64 }
impl GifFrames { new(Vec<(Vec<u8>, u32)>) -> Result<Self, String>; len; frame(i); delay_ms(i);
                 frame_at(elapsed: Secs) -> usize /* loops */ }
#[derive(Clone)] pub struct GifRef(pub Arc<GifFrames>);  // PartialEq = Arc::ptr_eq
pub enum GifLink { Direct(String), TenorPage(String) }
pub fn classify_link(url: &str) -> Result<GifLink, String>;  // GIPHY page/media, Tenor page/media, *.gif
pub fn valid_gif_name(name: &str) -> bool;            // [a-z0-9-]+
```

Tests: zero-delay rule, cap keeps timing and every n-th frame, `frame_at`
over time and looping, `GifRef` identity, each link pattern.

## Task 3: `Role::Gif(u16)` (`crates/core/src/theme.rs`)

Arms as above. Tests: icon `ms-gif_box`, title, name, index injective, not
cluster/sky, `ALL` unchanged (existing tests).

## Task 4: model, event, scene

- `Event::Gif { index: u16, frames: GifRef }`, `Event::GifFailed { index: u16, reason: String }`.
- `Model`: `gifs: Vec<GifState>` (`Loading | Ready(GifRef) | Failed`),
  `gif_state(i)`.
- `Drawable::Image { frames: GifRef, frame: u16 }`.
- `ScreenUi::since(key, now) -> Secs`: the first `now` a key was asked for,
  reset on a role change; the GIF's clock starts when it is first shown.
- `NoData::Loading` ("Loading", TEXT2), `NoData::GifFailed` ("GIF failed", WARN);
  both fit `PILL_MAX_TEXT_W` (test).
- `Model::scene_for_role(Gif(i))`: `Ready` -> `Scene { Clear, Image }` with the
  frame from `ui.since`; else the status screen.
- Tests: loading/failed pills and icon, playing scene has only Clear + Image,
  frame advances with time and loops, equal instants give equal scenes,
  a failure after ready keeps playing? (no: `GifFailed` only arrives before a
  successful decode).

## Task 5: renderer

`Drawable::Image` memcpy into the pixmap when sizes match. Test: rendered
pixels equal the frame bytes inside the circle.

## Task 6: sources (`crates/sources/src/gif.rs`)

- Dependencies: `gif = "0.14"`, `sha2 = "0.11"` in `crates/sources`.
- `tenor_gif_url(html) -> Option<String>`: `og:image` content, else JSON-LD
  `contentUrl`; first that ends in `.gif`; unescapes `/` and `&amp;`.
- `cache_path(dir, url) -> PathBuf`: `<sha256 hex of url>.gif`.
- `decode(bytes) -> Result<GifFrames>`: two passes (count frames, then compose
  with disposal and keep every n-th), cover-scale each kept canvas to
  240x240 (box filter down, nearest up), composite over black, opaque.
  Canvas size capped at 4 Mpx.
- `fetch_capped(url, 20 MB)`; `load(link, cache_dir)`: cached file, else
  resolve (Tenor page fetch) and download, then write the cache.
- `run_gifs(Vec<(u16, String)>, cache_dir, ctx)`: one task per GIF; decode on
  `spawn_blocking`; emit `Event::Gif` or `Event::GifFailed`; retry a failure
  every hour.
- Fixtures: `crates/sources/tests/fixtures/gif-rgb.gif` (tiny, 3 frames,
  generated), `tenor-view.html` (hand-written, no real page content).
- Tests: Tenor HTML both forms, cache path, decode of the fixture (frame count,
  delays, cover crop colours), disposal, cap applied while decoding, cache hit
  avoids the network (`load` with a cached file and an unreachable URL).

## Task 7: config (`crates/app/src/config.rs`)

- `GifCfg { name, url }`, `Config.gifs: Vec<GifCfg>` (skipped when empty).
- `ScreenCfg::roles_in(&[GifCfg])`, `Config::screen_roles`, `Config::role_name`,
  `Config::pickable_roles`, `Config::used_gifs() -> Vec<(u16, String)>`.
- `validate`: names valid and unique, links classify, every `gif:` entry known.
- `gif_cache_dir()`: `$XDG_CACHE_HOME/rackscreen/gifs`, default
  `~/.cache/rackscreen/gifs`, the sudo user's home under sudo.
- `panels.rs` and `run.rs`: use `screen_roles`, spawn `run_gifs` for the GIFs
  the screens use (any source mode).
- Tests: parse, unknown name error names the entry, bad name, duplicate, bad
  link, role names round trip, cache dir with and without XDG.

## Task 8: goldens

`crates/render/tests/golden.rs` (dev-dependency on `rackscreen-sources` for
`decode`): `p2_gif_loading`, `p2_gif_playing`, `p2_gif_failed` from the
fixture. Check the other goldens are byte-identical.

## Task 9: setup TUI

- Configure: `Group::Gifs` ("GIFs") appended to `Group::ALL` (jump key 8). A
  list pane: rows per GIF plus "+ add GIF"; `⏎` edits (name, then link),
  `a` adds, `d` removes. A link is checked with `classify_link`. Renaming
  rewrites `gif:<old>` in screen roles; removing a GIF a screen uses asks to
  remove it from those screens too (refused if a screen would be left empty).
- Screens editor: `Editor` gets the GIF names; the picker walks
  `pickable_roles`; names shown via `role_name`; saving writes `gif:<name>`.
- Home (menu): role names via `role_name`.
- Tests: add/edit/remove GIF, remove-used asks and cleans screens, pick
  `gif:<name>` in the Screens editor and save.

## Task 10: docs

README GIF section (config, link kinds, memory ≈ 35 MB per GIF, cache path);
`config.example.yaml` commented `gifs:` example. No version bump.

## Task 11: self-review and report

`docs/superpowers/plans/2026-11-03-gif-role-report.md`.

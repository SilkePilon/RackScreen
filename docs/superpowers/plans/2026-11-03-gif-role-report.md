# GIF role: implementation report

Branch `gif-role`, from `3deaa3a`. Spec: `docs/superpowers/specs/2026-11-03-gif-role-design.md`.
Plan: `docs/superpowers/plans/2026-11-03-gif-role-implementation.md`. Nothing is pushed or tagged, and the version is not bumped.

## What was built

- **Core** (`crates/core/src/gif.rs`)
  - `GifFrames` holds 240×240 opaque premultiplied RGBA frames and their delays. `frame_at(elapsed)` picks the frame and loops.
  - `GifRef` is an `Arc` newtype compared by pointer, the same pattern as `WorldRef`.
  - `delay_ms` turns 0 and 1 cs into 100 ms. `keep_mask` and `cap_frames` keep every n-th frame (at most 150) and add the skipped delays to the kept frame.
  - `classify_link` handles GIPHY pages, GIPHY media links, `i.giphy.com`, Tenor pages, Tenor media links and any other https `.gif` link.
  - `valid_gif_name` checks names. `GifState` is `Loading | Ready | Failed`.
- **Role**: `Role::Gif(u16)`, with title "GIF" and icon `ms-gif_box` (vendored at 24 px and listed in `assets.rs` and `fetch-assets.sh`).
- **Model and scene**
  - New events: `Event::Gif { index, frames }` and `Event::GifFailed { index, reason }`. A ready GIF draws as `Scene { Clear, Drawable::Image { frames, frame } }`: full circle, no ring, no icon.
  - The frame clock starts the first time the decoded GIF is shown under the current role (`ScreenUi::since`), and it resets on a role change. The outgoing half of a crossfade keeps its clock.
  - "Loading" (TEXT2) and "GIF failed" (WARN) use the normal no-data screen. Both pills fit `PILL_MAX_TEXT_W`, and a test checks this.
- **Renderer**: `Drawable::Image` copies the frame straight into the pixmap with a memcpy. The base cache is not involved. The fade still applies afterwards.
- **Sources** (`crates/sources/src/gif.rs`, adds `gif 0.14` and `sha2 0.11`)
  - Tenor pages: the `.gif` URL is read from `og:image`, falling back to JSON-LD `contentUrl`.
  - Cache: the file is `<sha256 of the pasted link>.gif`. A cached file is used without any network.
  - Downloads use the shared client, capped at 20 MB and 120 s, with `Accept: image/gif`. The `GIF8` magic is checked.
  - Decoding takes two passes. The first reads only the delays, so a long GIF never holds frames it drops. The second composes frames with disposal Keep/Background/Previous, then cover-scales each kept frame: box filter when shrinking, nearest when growing, centre crop, over black.
  - `run_gifs` runs one task per used GIF. Decoding runs one GIF at a time behind a semaphore. A failure is retried every hour, and a broken cached file is deleted so it is downloaded again.
- **Config** (`crates/app/src/config.rs`)
  - `gifs: [{ name, url }]` is not written to the file while empty.
  - `ScreenCfg::roles_in(&gifs)` and `Config::screen_roles` resolve `gif:<name>`. `Config::role_name` gives `gif:<name>`. `Config::pickable_roles` and `Config::used_gifs` support the TUI and the source.
  - `validate` checks names, duplicates and links, and resolves every screen. Each error names the bad entry.
  - `gif_cache_dir()` returns `$XDG_CACHE_HOME/rackscreen/gifs`, else `~/.cache/rackscreen/gifs`, using the sudo user's home under sudo.
  - `panels.rs` and `run.rs` use `screen_roles`. Only GIFs that some screen uses are fetched, in every source mode, `--sim` and fake included.
- **Setup TUI**
  - A new Configure group "GIFs", the eighth (jump key `8`), in `screens/gifs.rs`. It is a list of name, link and which screens use it, plus "+ add GIF".
  - Keys: `⏎` edits (name, then link; `Tab` switches between them), `a` adds, `d` removes.
  - A link is checked with `classify_link`. Renaming rewrites `gif:<old>` in the screens. Removing a GIF that a screen uses asks first, then removes it from those screens too. Removal is refused if a screen would be left with no roles.
  - The help line gives the 35 MB memory note.
  - The Screens editor picker lists GIFs after the built-in roles, and rows, picker and Home show `gif:<name>`. Saving writes `gif:<name>`.
- **Docs**: a README "GIF role" section covering config, link kinds, cache, timing, frame cap and memory (~35 MB per GIF). The Configure bullet now says eight groups and `1`–`8`. There are pointers from the roles section and the config list, and a commented `gifs:` example in `config.example.yaml` and in the README copy of it.

## Test results

`cargo test --workspace -q`: 529 passed, 0 failed. `cargo clippy --workspace --all-targets -q -- -D warnings` is clean, and so is `cargo fmt --check`. All three were run before every commit.

No test uses the network. The fixtures are:
- `crates/sources/tests/fixtures/gif-fixture.gif`: 80×60, 3 frames with delays 100 / 0 / 500 ms, and red edge bars that the cover crop must remove. It was generated with Pillow.
- `tenor-view.html`: hand-written, with no real page content.
- Disposal and frame-cap tests build their GIFs in the test with the `gif` encoder.
- The cache test points at an unreachable `.invalid` URL to show that no network is used.

## Golden and gif changes

- New goldens: `p2_gif_loading.png`, `p2_gif_failed.png` and `p2_gif_playing.png` (fixture frame 1, green with a white dot, red bars cropped). I looked at all three.
- No existing golden changed: `git diff --name-status 3deaa3a..HEAD -- '*.png'` lists only the three new files.
- `crates/sources/src/fake.rs` is untouched, so the RNG draws are unchanged. `.github/media` is untouched: I did not add a README gif for the GIF role.

## Deviations from the spec

1. **Failure event.** I added `Event::GifFailed { index, reason }` next to `Event::Gif`, so the model can show "GIF failed". The spec names only `Event::Gif`.
2. **Image drawable.** `Drawable::Image` holds `frames: GifRef` (an `Arc<GifFrames>` newtype) rather than a bare `Arc`. This reuses the pointer-compared `WorldRef` pattern; equality is still the pointer plus the frame index.
3. **Frame clock.** Playback starts at frame 0 when the decoded GIF is first shown, not when the role came on screen. If a GIF finishes loading while its screen shows "Loading", it starts from the beginning.
4. **Only used GIFs are fetched.** GIFs that no screen references are validated but never downloaded or decoded, which saves memory.
5. **Extra limits** not in the spec:
   - A GIF canvas over 4 Mpx is refused; the canvas alone would be 16 MB.
   - A Tenor page is read up to 4 MB.
   - A download may take up to 120 s; the shared client's own 10 s timeout is overridden.
   - A truncated GIF plays the frames before the damage.
6. **Scaling and transparency.** Box filter when shrinking, nearest when growing (pixel-art GIFs stay sharp). Transparent pixels show black.
7. **Cache key.** The key is the sha256 of the pasted link (trimmed), not of the resolved `.gif` URL, so a Tenor page is not fetched again after a restart.
8. **The GIF source runs in fake/sim mode too.** GIFs are real links, so `rackscreen run --sim` shows them.
9. **Configure layout.** It now has eight groups, so `1`–`8` jump. Existing tests were updated for this: the wrap target, the sidebar count of 8, "1-8 jump", and the GIFs group having no form fields.

## `Role::Gif` design choice

- `Role` stays `Copy + Eq + Hash` and gains `Gif(u16)`, an index into `gifs:`.
- **`Role::ALL` stays the fixed list of 28 built-in roles.** GIF roles exist only relative to a config. Code that iterates `ALL` keeps its meaning and never meets a `Gif`: goldens, the icon tests, the README gif example, `Role::parse`.
- Code that must offer GIFs builds its own list: `Config::pickable_roles()` in the app, and `Editor::pickable()` in the Screens editor, which holds the GIF names.
- **Names.** `Role::name()` stays `&'static str` and returns `"gif"`. The real `gif:<name>` comes from `Config::role_name` and is used for logs, display-thread names, the TUI and saving.
- **Parsing.** `Role::parse` knows only built-in names. `gif:<name>` is resolved against `gifs:` in `ScreenCfg::roles_in` / `Config::screen_roles`, and `validate` resolves every screen.
- **Compatibility.** `ScreenCfg::roles()` is kept as `roles_in(&[])` for any caller without the list; it rejects `gif:` entries. All in-tree callers now use `screen_roles`.
- **Indices.** `Role::index()` returns `ALL.len() + i` for `Gif(i)`, so indices stay unique. `from_index` maps back only built-in roles.
- **Exhaustive matches** gained one arm each: `title`, `icon`, `name`, `needs_data`, `p2::roles::screen` (returns `None`) and `role_scene`.

## Open questions for Silke

1. **SPI load.** A playing GIF pushes a full frame (115 KB) on every frame change, often 10–25 times a second. Several GIF screens at once may contend with the other panels on the bus, as transitions did. Should GIF playback be capped (for example at 15 fps), or should GIFs be kept to one screen at a time?
2. **Memory budget.** Nothing stops a config with many GIFs (35 MB each, worst case). Should there be a limit on GIFs in use, or a total budget, rather than the README warning alone?
3. **Failed GIFs in the cycle.** A failed GIF shows "GIF failed" for its whole dwell. Should the cycle skip it until a retry succeeds?
4. **Cache cleanup.** Removing or changing a link leaves the old file in `~/.cache/rackscreen/gifs`. Should the service prune files that no configured link maps to?
5. **Running under sudo.** `sudo rackscreen run` caches into the invoking user's `~/.cache` but writes as root, so those files end up owned by root. The systemd service runs as the user and is not affected. Should these files be `chown`ed, or is this fine as it is?
6. **README media.** Do you want a `gif:` role clip in `.github/media/screens`, using the fixture or a GIF you choose?
7. **Retry timing.** Retry is fixed at one hour. A shorter first retry, say 1 minute, would help when the Pi boots before the network is up and the GIF is not cached yet.

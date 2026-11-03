# Music role: implementation plan

Spec: `docs/superpowers/specs/2026-11-03-music-role-design.md` (look N1, no lyrics).
Branch: `music-role`. Every task is TDD: write the test, watch it fail, implement,
then `cargo test --workspace -q`, `cargo clippy --workspace --all-targets -q -- -D warnings`
and `cargo fmt --check` before the commit.

Decisions taken after the spec: icon `ms-music_note`; Spotify `GREEN`, Jellyfin
`VIOLET`; the `jellyfin` role becomes video only; Spotify is built and tested
with fixtures only (no client ID exists yet).

## Shape

- **core** gains plain data (`event::Track`, `event::Cover`, `event::CoverRef`,
  `Event::Music`), pure rules (`core::music`: source pick, position, `since`
  stamping, demo cover), model state (`MusicState`, `music_configured`), a new
  drawable (`Drawable::Image`), and the role scene (`p2::roles::music`).
- **render** draws `Drawable::Image` (premultiplied pixmap, rounded clip).
- **sources** decodes and caches covers (`cover.rs`), polls Spotify
  (`spotify.rs`: PKCE, tokens, player parsing, back-off) and routes Jellyfin
  audio sessions to `Event::Music`.
- **app** has the `spotify:` config block, the token path helpers and spawns
  the Spotify source; **setup** gets a "Spotify" Configure group with a login
  dialog (URL, half-block QR, paste-back).

## Tasks

### 1. Icon asset
- Download `music_note.svg` (svg-700@0.23.0, rounded) to `assets/icons/ms-music_note.svg`,
  rewrite `width="48" height="48"` → `width="24" height="24"`.
- Add `"ms-music_note"` to `crates/render/src/assets.rs` and the name list in
  `scripts/fetch-assets.sh`. Test: the existing every-asset-loads test in render covers it.

### 2. Core data: `Track`, `Cover`, `CoverRef`, `Event::Music`
File `crates/core/src/event.rs`:
```rust
pub enum MusicSource { Spotify, Jellyfin }
pub struct Cover { pub key: String, pub size: u32, pub rgba: Vec<u8> } // straight RGBA, size²
pub struct CoverRef(pub Arc<Cover>);            // PartialEq: same key and size
pub struct Track {
    pub source: MusicSource, pub title: String, pub artist: String, pub device: String,
    pub duration_secs: f64, pub position_secs: f64, pub playing: bool,
    /// unix secs of the last start, pause or track change
    pub since: i64,
    pub cover: Option<CoverRef>,
}
Event::Music { source: MusicSource, track: Option<Track> }
```
Tests: `CoverRef` equality by key, not by pointer.

### 3. Core rules: `crates/core/src/music.rs`
```rust
pub fn pick<'a>(a: Option<&'a Track>, b: Option<&'a Track>) -> Option<&'a Track>;
pub fn position(t: &Track, polled_at: Secs, now: Secs) -> f64;
pub fn stamp_since(prev: Option<&Track>, next: &mut Track, unix_now: i64);
pub fn demo_cover(key: &str, a: Color, b: Color) -> Cover; // 84², used by fake + goldens
pub fn fmt_played_ago(secs: i64) -> String; // "just now", "12 min ago", "2 h ago", "3 d ago"
pub const COVER_PX: u32 = 84;
```
Tests: playing beats paused; two playing → later `since`; two paused → later
`since`; position extrapolates only while playing and clamps to duration;
`stamp_since` keeps `since` for the same track and play state, resets otherwise.

### 4. Model: `MusicState`
File `crates/core/src/model.rs`: `MusicState { spotify, jellyfin: Option<Track>,
polled_at: [Secs; 2], have: bool, last: Option<(Track, i64)> }`, `music()`,
`set_music_configured()` / `music_configured()`. `Event::Music` stores the
track per source; `last` follows the picked track (playing: stamped now; paused:
keeps the stamp of the same track). Tests: per-source storage, last played kept
after the source goes empty.

### 5. Role + theme
`Role::Music` (`music`, title `MUSIC`, icon `ms-music_note`), appended to
`Role::ALL` (29). Not cluster, not sky. `scene_model.rs`: `needs_data` →
`!music().have`; not configured → `NoData::Config`. Tests in `theme.rs`
and `scene_model.rs`.

### 6. `Drawable::Image` + renderer
`Drawable::Image { cover: CoverRef, x, y, size, r, alpha }` in `scene.rs`.
Renderer: cache the premultiplied pixmap of the last few covers by key, fill a
rounded rect with a pattern shader scaled to `size`. Contrast-floor test skips
images (like worlds). Tests: a red cover renders red inside, black at the
rounded corner, half alpha halves it.

### 7. Music scene: `crates/core/src/p2/roles/music.rs`
`pub fn music_scene(model, now, ui) -> Scene` (own scene like power-mix):
base = clear, ring track, top icon, cover (or placeholder), card background;
then ring fill (source colour; paused alpha 0.45), pill at cy 136, card
lines (song SemiBold `TEXT`, artist `TEXT2`, device `TEXT2`) at card top 152.
States: playing (`2:14 left` in source colour), paused (`paused`, `TEXT2`),
idle with last track (cover alpha 0.45, ring empty, `played 12 min ago`),
nothing ever (`nothing playing`, no cover, empty card). Missing cover: `CARD`
square with a dim note icon. Tests per state, plus base/overlay order.

### 8. Cover decode + cache: `crates/sources/src/cover.rs`
`decode(bytes) -> Result<(u32, u32, Vec<u8>)>` (JPEG via `zune-jpeg`, PNG via
`png`), `square(w, h, rgba, out_px) -> Vec<u8>` (centre crop, box filter),
`CoverCache` (8 entries, LRU by key), `async fetch(...)`. Tests with tiny
PNG/JPEG fixtures generated in the test (png encoder) and a checked-in JPEG.

### 9. Spotify: `crates/sources/src/spotify.rs`
- PKCE: `verifier_from(bytes)`, `challenge(verifier)` (RFC 7636 vector),
  `new_login(client_id) -> Login { verifier, state, url }`,
  `parse_redirect(url, state) -> Result<String, AuthError>`.
- Tokens: `TokenFile { access_token, refresh_token, expires_at, display_name, error }`,
  `code_form`, `refresh_form`, `apply_token_response(json, now, prev)` (rotated
  refresh token kept), `read_token`, `write_token(path, tok, owner)` (0600, atomic,
  chown), `login_status(Option<&TokenFile>, now) -> String`.
- Player: `parse_player(status, body) -> Result<Option<SpotifyItem>>` (track,
  episode → show name as artist, 204 → None, missing images), `cover_url(images)`
  (smallest ≥ 168), `next_poll(playing, retry_after)` (3 s / 15 s / Retry-After).
- `run_spotify(SpotifyConfig { client_id, token_path, owner }, ctx)`.
Fixtures in `crates/sources/tests/fixtures/spotify-*.json`.

### 10. Jellyfin audio routing
`parse_sessions` skips `Type == "Audio"`. New `parse_audio(json) -> Option<JfAudio>`
(title, artist = `Artists[0]` or `AlbumArtist`, runtime, position, paused,
`user · device`, cover item id `AlbumId` or `Id`, last activity). The poller
emits `Event::Music { Jellyfin, .. }` too, fetching the cover once per album.
Fixture `jellyfin-sessions-audio.json`.

### 11. Fake source
`FakeState` gets `music_state` (Spotify playing → paused → Jellyfin playing →
idle) and a position; no RNG draws (positions from ticks). Initial events and
every second tick emit `Event::Music` for both sources; `FakeCmd::MusicToggle`
on key `p`. Tests: draw count unchanged (same metrics as before for a seed),
toggle cycles the states.

### 12. Config + app wiring
`spotify: { enabled: false, client_id: "" }` (`SpotifyCfg`) in `config.rs` and
`config.example.yaml`; validate client ID charset when set. `crates/app/src/spotify.rs`:
`token_path_in(state)`, `token_path()`, `token_path_for_service_user() ->
(PathBuf, Option<(u32, u32)>)`. `run.rs` spawns the source and sets
`music_configured`; `runloop.rs` passes it to the model. Tests for config
defaults/round trip and path helpers.

### 13. Setup TUI
`Group::Spotify` with fields `SpotifyEnabled` (Bool), `SpotifyClientId` (Text),
`SpotifyLogin` (new `FieldKind::Action`, value = login status). Enter on the
action opens `Mode::Login` (URL, half-block QR from the `qrcode` crate, paste
line); Enter exchanges the code on a worker thread through an injectable
exchanger and writes the token file for the service user. Screens editor:
`music` hint "no source". Tests: fields round trip, QR lines, login flow with
a fake exchanger, status text.

### 14. Goldens, gifs, README
Goldens: `p2_music` (Spotify playing, via `every_role_normal`), `p2_music_jellyfin`,
`p2_music_paused`, `p2_music_idle`, `p2_music_nothing`, `p2_music_config`,
`p2_music_no_cover`. Every other golden byte-identical. Gifs: add `music.gif`
(script toggles states); only `all.gif` changes besides. README: role table,
gif grid, a "Music role" section, Configure groups (eight), config example.

### 15. Self-review and report
Review against the spec; write `docs/superpowers/plans/2026-11-03-music-role-report.md`.

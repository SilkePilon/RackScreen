# Music role: implementation report

Branch `music-role` (from `3deaa3a`). Spec: `docs/superpowers/specs/2026-11-03-music-role-design.md`,
plan: `docs/superpowers/plans/2026-11-03-music-role-implementation.md`. Not pushed, not tagged,
version not bumped, Pi not touched.

## What was built

- **Role `music`** (title `MUSIC`, icon `ms-music_note`, vendored from svg-700@0.23.0 at 24 px and
  listed in `assets.rs` and `fetch-assets.sh`), appended to `Role::ALL` (29 roles). It draws its own
  scene (`crates/core/src/p2/roles/music.rs`), like power-mix:
  - Cover 84 × 84 at (78, 36), radius 14, from a new `Drawable::Image` (renderer: premultiplied
    pixmap cached per cover key, bilinear pattern fill clipped to a rounded rect).
  - Ring = track progress: Spotify `GREEN`, Jellyfin `VIOLET`; paused at alpha 0.45.
  - Pill at cy 136: `3:31 left` in the source colour, `paused` in `TEXT2`, idle
    `played 12 min ago` in `TEXT2`, `nothing playing`.
  - Card at y 152: song (SemiBold, `TEXT`), artist and device (`TEXT2`), marquee for long lines.
  - Idle: last track with the cover at 0.45 and an empty ring. Nothing ever played: no cover,
    empty card. Not configured: the standard "Check config" status screen.
  - Base layer = track, icon, cover, card background (cached); the progress ticks locally between
    polls and moves in quarter-degree steps so frame skipping still works between steps.
- **Core data and rules**: `event::{MusicSource, Track, Cover, CoverRef}`, `Event::Music`;
  `core::music::{pick, position, stamp_since, fmt_played_ago, demo_cover}`. `since` is the last
  start, pause or track change, so one field covers both rules (playing beats paused; later
  `since` wins). `Model::music()` keeps one track per source and the last track heard.
- **Spotify source** (`crates/sources/src/spotify.rs`): PKCE (verifier/challenge, authorize URL,
  redirect parsing), token exchange and refresh (rotated refresh tokens written back, `invalid_grant`
  marks the file), token file at `$XDG_STATE_HOME/rackscreen/spotify.json` (0600, atomic, optional
  chown), `/v1/me/player?additional_types=track,episode` parsing (track, episode with the show as
  artist, 204, ads, missing images), smallest image ≥ 168 px, polls 3 s / 15 s, `Retry-After` on 429
  (keeps the last track). Network errors keep the track on screen; only a missing or refused login
  clears it.
- **Covers** (`crates/sources/src/cover.rs`): JPEG via `zune-jpeg`, PNG via `png`, centre crop and
  box filter to 84 px, last 8 albums in memory, one download per album.
- **Jellyfin**: the `jellyfin` role is video only (`Type: Audio` skipped). The same poller emits the
  playing (else most recently active paused) audio session as `Event::Music` with
  `Artists[0]`/`AlbumArtist`, `user · device`, and the cover from
  `/Items/{AlbumId or Id}/Images/Primary?maxWidth=168` with the API key.
- **Config**: `spotify: { enabled: false, client_id: "" }` (client id letters and digits only),
  in `config.rs`, `config.example.yaml` and the README copy. `crates/app/src/spotify.rs` has the token
  path helpers (reusing `play::state_dir` and `config::sudo_user`). The source starts when Spotify is
  on with a client id; the run loop checks for a login every 5 s, so a login made in setup shows
  without a restart.
- **Setup TUI**: an eighth Configure group, Spotify (`enabled`, `client id`, `log in`). The `log in`
  row shows "logged in as <name>" / "not logged in" / "token expired, log in again". Enter opens a
  dialog with the URL, a half-block QR code (`qrcode` crate, no image features) and a paste line;
  the exchange runs on a worker thread and writes the token file for the service user, chowned to
  them under sudo. The Screens editor flags `music` with "no source".
- **Fake source**: Spotify playing → paused → Jellyfin playing → idle, key `s` in the simulator
  (added `S` to the sim key map). No RNG draws; a test pins the RNG state after 120 ticks.
- **README**: gif grid, role table, a "Music role" section with the Spotify app and login steps,
  eight Configure groups, simulator key.

## Test results

All gates pass on `625ca80`:

- `cargo test --workspace -q`: 536 tests, 0 failed (app 59, core 193, display 15, render lib 36,
  render goldens 17, setup 113, sources 103).
- `cargo clippy --workspace --all-targets -q -- -D warnings`: clean.
- `cargo fmt --check`: clean.

Tests added: source pick and position rules, `since` stamping, model per-source storage and last
track, music view and scene for every state, cover/card inside the ring, Image drawing and cache,
scene_model Check config → Waiting → nothing playing, Spotify fixtures (playing, paused, 204,
episode, ad, missing images, token, refresh, rotated, revoked, `/v1/me`), RFC 7636 test vector,
redirect parsing (code, state mismatch, `error=`, no code, garbage), token file mode and status
text, 429 back-off, login vs network errors, JPEG/PNG decode and square, LRU cache, Jellyfin audio
routing, fake music cycle and RNG pin, config defaults and validation, token path helpers,
`music_ready`, Configure fields and the login flow with a fake exchanger (incl. the QR rendering),
Screens hint. No test makes a network call.

TDD note: most tasks were run test-first (failing run, then implementation). For the music scene,
the renderer's image cache test and the setup login module, tests and code were written in the
same step and run together.

## Golden and gif changes

- New goldens: `p2_music` (Spotify playing), `p2_music_jellyfin`, `p2_music_paused`,
  `p2_music_idle`, `p2_music_nothing`, `p2_music_config`, `p2_music_no_cover`, and
  `p2_music_nodata` (from the existing every-role no-data test). All looked at.
- Every other golden is byte-identical to the branch base (re-captured with `UPDATE_GOLDENS=1`,
  `git status` clean, `git diff --stat 3deaa3a` lists only music PNGs).
- Gifs: new `music.gif`; `all.gif` changed because it cycles through every role and now includes
  music. Every other gif is byte-identical.

## Deviations from the spec

- **Cover scaling** happens once at decode time in the sources (box filter to 84 px), not at
  render time. Cheaper on the Pi and better quality than a per-frame bilinear downscale. The
  renderer still scales any cover size to the slot.
- **`started_at` is `since`**: it also resets on pause. That gives the spec's rule for paused
  sources ("later activity wins") without a second field.
- **Missing cover**: a `CARD`-coloured rounded square with a `TEXT3` note icon (the spec did not
  say what to draw).
- **Status line** is the value of the `log in` row in Configure, not a separate line.
- **Spotify group** is appended as group 8 (after Thresholds), not next to Media & Jellyfin. That
  keeps the existing group numbers and `FIELDS` order unchanged for merging.
- **Simulator key** is `s`, not `p`: an existing test keeps `p` (the old price key) unmapped.
- **`played N min ago`** uses its own formatter (`just now`, `12 min ago`, `2 h ago`, `3 d ago`) to
  match the spec wording; the existing `fmt_ago` says `12 m ago`.
- **"Check config"** with only Spotify on needs a login, not just a client id, as the spec says.
  The Screens editor hint only checks the client id, since it cannot see the service user's token.

## Open questions for Silke

1. **Spotify client ID**: create an app at developer.spotify.com (Web API), then put its client ID
   in Configure → Spotify → `client id`. No client secret is needed (PKCE).
2. **Redirect URI to register**: `http://127.0.0.1:8888/callback`, exactly. Spotify allows
   loopback `http` redirects. Nothing listens on that port: the browser shows an error page and
   you paste the address from the address bar.
3. **Development mode**: a new Spotify app starts in development mode. If the login is refused,
   add your Spotify account under the app's user management in the dashboard.
4. **Non-Latin titles**: Outfit lacks CJK and many other scripts; such titles show missing glyphs
   (the jellyfin role has the same limit). Is a fallback font wanted?
5. **Group placement**: should Spotify move next to "Media & Jellyfin" (renumbering the jump keys)?
6. **Existing simulator gap** (not changed here): `sim.rs` maps no key for `m`, `k`, `b`, `j` or `c`,
   so those fake commands from the README cannot be triggered in the window.
7. A broken album image URL is fetched again on every poll (3 s while playing). It was left as is
   because Spotify's image CDN is reliable; a short negative cache would fix it if needed.

## Merge notes

`main` moved ahead of the branch base (`910da91`, `37791ca`, `497d386`: play-request hardening,
show-world blit and a fireworks golden). The changes here to `theme.rs`, `config.rs`,
`configure.rs`, `golden.rs`, `fake.rs` and `README.md` are additive: new enum variants and match
arms at the end, new tests, new fields and blocks. Expect small textual conflicts in the role
lists, Configure tests and README tables.

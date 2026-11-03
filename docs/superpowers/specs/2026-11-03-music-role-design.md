# Music screen: Spotify and Jellyfin now playing

Date: 2026-11-03. Status: approved in brainstorm (look N1 in `.superpowers/brainstorm/1130412-1793723449/content/spotify-nowplaying.html`). Lyrics were considered and dropped. Builds on spec "remove face, Outfit, top icon".

## Look (N1)

- Top icon `music_note`; ring = track progress, in the source colour: Spotify `GREEN` (`#4CC38A`), Jellyfin `VIOLET` (`#A48BFA`).
- Cover: rounded square 84 × 84 at (78, 36), corner radius 14 (`CARD_R`).
- Pill at `PILL_CY`-like position under the cover (cy 136): `2:14 left` in the source colour; paused: `paused` in `TEXT2` and the ring at alpha 0.45.
- Card (`CARD_W` × `CARD_H`, at y 152): song (semibold, `TEXT`), artist (`TEXT2`), device (`TEXT2`, Spotify device name / Jellyfin `user · device`). Long lines use the existing marquee rule.
- Progress and time left tick locally between polls (position + elapsed since the poll while playing).
- Idle (nothing playing): last played track, cover at alpha 0.45, ring empty, pill `played 12 min ago` in `TEXT2`. Nothing ever played: pill `nothing playing`, no cover, card empty.
- Not configured (no Spotify login and no Jellyfin URL): "Check config" status screen.

## Which source shows

Each source reports at most one track with `playing`, `started_at` (when playback last went from stopped/paused to playing, or the track changed). Rule: a playing source beats a paused one; between two playing sources the later `started_at` wins; between two paused ones the later activity wins.

## Roles

- New `Role::Music` (`music`), added to `Role::ALL`, setup TUI Screens editor, fake data, goldens, gifs, README.
- `Role::Jellyfin` becomes video only: sessions whose item `Type` is `Audio` are ignored there and go to the music source instead.

## Spotify source (`sources/src/spotify.rs`)

- Endpoint `GET https://api.spotify.com/v1/me/player` (gives `item`, `is_playing`, `progress_ms`, `device.name`); `204` = nothing playing. Scopes `user-read-playback-state user-read-currently-playing`.
- Poll every 3 s while playing, 15 s otherwise; back off on `429` using `Retry-After`.
- Cover: smallest album image ≥ 168 px, downloaded once per album id, decoded (JPEG/PNG via `zune-jpeg`/`png`, or the `image` crate with only those features), scaled to 84 × 84 with rounded corners at render time; cached in memory (last 8 albums).
- Auth: Authorization Code with PKCE, own client ID (Silke registers a free Spotify developer app; redirect URI `http://127.0.0.1:8888/callback`). Access token refreshed with the refresh token; a rotated refresh token is written back.
- Token file: `$XDG_STATE_HOME/rackscreen/spotify.json` (default `~/.local/state/...`) of the service user, mode 0600. Config holds only `spotify: { enabled, client_id }`.

## Jellyfin audio

- The existing Jellyfin poller also emits audio sessions to the music source: title `Name`, artist `Artists[0]` or `AlbumArtist`, `RunTimeTicks`, `PositionTicks`, `IsPaused`, `UserName · DeviceName`.
- Cover: `GET {url}/Items/{AlbumId or Id}/Images/Primary?maxWidth=168` with the existing API key, same decode and cache.

## Setup TUI

- New Configure group "Spotify": client ID field, "Log in" action. The action builds the PKCE authorize URL, shows it as text and as a half-block QR code, and asks to paste the full redirect URL back (the browser shows an error page on `127.0.0.1`; the code is in the address bar). It exchanges the code and writes the token file for the service user, with ownership set to that user.
- Status line: "logged in as <display name>" / "not logged in" / "token expired, log in again".

## Testing

- Source choice rule (playing vs paused, later start wins).
- Spotify JSON fixtures: playing, paused, 204, podcast episode (show name as artist), missing images; token refresh incl. rotated token; 429 back-off.
- Jellyfin audio fixture routed to music, not to the jellyfin role.
- PKCE: verifier/challenge per RFC 7636 test vector; redirect URL parsing (code, state mismatch error).
- Goldens: playing Spotify, playing Jellyfin, paused, idle last played, nothing played, check config.

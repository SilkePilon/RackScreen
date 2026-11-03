# GIF role: GIPHY and Tenor links as screens

Date: 2026-11-03. Status: approved in brainstorm. Third of three specs; built after the speed pipeline and the crossfade.

## Config

```yaml
gifs:
  - { name: party, url: "https://giphy.com/gifs/some-slug-AbCdEf123" }
  - { name: nod,   url: "https://tenor.com/view/some-slug-12345678" }
screens:
  - { roles: [cpu, gif:party, mem], cycle_secs: 10, ... }
```

- `Role` gains `Gif(u16)`, the index into `gifs` (Role stays `Copy`). `gif:<name>` in a roles list parses to it; an unknown name is a config error naming the entry. Role name for logs and the TUI: `gif:<name>`.
- Dwell is the screen's `cycle_secs`, like any role. The crossfade applies on the way in and out.
- Names: lowercase letters, digits and `-`, unique.

## Link resolution (no API keys)

- GIPHY page `giphy.com/gifs/<slug>-<id>` or `giphy.com/gifs/<id>`, or media link `media*.giphy.com/media/<id>/...`: becomes `https://i.giphy.com/<id>.gif`.
- Tenor page `tenor.com/view/...`: fetched once, the `.gif` URL read from `og:image` or JSON-LD `contentUrl` (first one ending in `.gif`). Direct `media*.tenor.com/....gif` links are used as is.
- Any other `https` link ending in `.gif` is used as is.
- Resolution and download run in a `sources` task (`sources/src/gif.rs`) using the shared HTTP client; download capped at 20 MB.

## Cache and decode

- Raw file cached at `$XDG_CACHE_HOME/rackscreen/gifs/<sha256 of url>.gif` (default `~/.cache/...`), so a restart works offline. A cached file is used without re-download.
- Decoded with the `gif` crate (already a dependency of `crates/app`, moved or added to `sources`): frames composed with their disposal modes onto a full canvas, then scaled once to cover 240×240 (scale to fill, centre crop), premultiplied RGBA.
- Frame cap 150 per GIF: longer GIFs keep every n-th frame and add the skipped delays to the kept frame, so timing holds. Memory ≈ 150 × 230 KB ≈ 35 MB per GIF worst case; the setup TUI and README say so.
- Delays: per-frame native delay; 0 or 10 ms is treated as 100 ms, as browsers do.
- Decoded GIFs reach the model as an `Event::Gif { index, frames: Arc<GifFrames> }`.

## Rendering

- `Drawable::Image { frames: Arc<GifFrames>, frame: u16 }` (compared by `Arc::ptr_eq` + frame index, so the speed pipeline skips repeats). The renderer copies the frame into the target (memcpy); the round panel hides the corners.
- The model picks the frame from time since the role came on screen, looping.
- States: before the GIF is ready, the role shows the status screen "Loading"; after a failure, "GIF failed" with the reason in the log; retry every hour.

## Setup TUI

- New Configure group "GIFs": list of name + link, add, edit, remove; a link is checked for the patterns above (full resolution happens on the Pi).
- Screens editor: `gif:<name>` entries are pickable like any role; removing a GIF that a screen uses asks to remove it from those screens too.

## Testing

- Unit: link resolution for each pattern (Tenor with a saved HTML fixture), cache path, frame cap with timing kept, zero-delay rule, cover scaling, `gif:` parsing and unknown-name error, frame choice over time and looping.
- Render: an `Image` drawable equals the frame bytes inside the circle.
- Golden: one fixture GIF (in `crates/sources/tests/fixtures`) in loading, playing and failed states.
- TUI: add/remove GIF, pick `gif:<name>` in the Screens editor.

# SEN live role (ISS 4K Earth stream)

Brainstormed 2026-11-10. Depends on per-role durations and role skipping
(`2026-11-10-role-durations-design.md`) and on the ISS globe
(`2026-11-10-iss-globe-design.md`) as fallback.

## What it shows

New role `sen`: the live Earth view from Sen's cameras on the ISS, full
panel, 15 fps. Only true live Earth footage is shown. Night side, the
camera-switch slate (teal "sen" card), replay footage (orange badge) and
promo clips are never shown: the role is then unavailable and the panel
skips it (leaving right away if it is on screen). A panel with no other
showable role draws the ISS globe screen instead.

## Stream source (research 2026-11-10)

- sen.com/live embeds YouTube; the channel live URL is
  `https://www.youtube.com/@Sen/live` (video id today `fO9e9jnhYK8`).
- Built-in resolver only (no yt-dlp):
  1. GET the channel live page; read the canonical `watch?v=` id and
     `VISITOR_DATA`.
  2. POST `https://www.youtube.com/youtubei/v1/player` with the VISIONOS
     innertube client (no JS player needed) and the visitor data; read
     `streamingData.hlsManifestUrl`.
  3. GET the master playlist, pick the 640×360 H.264 variant (itag 230,
     ~1 Mbit/s); its media playlist lists 5 s MPEG-TS segments.
  URLs expire after ~6 h: re-resolve at 5 h or on HTTP 403/404.
- On failure: back off 1/5/15/60 min; the role is unavailable meanwhile.

## Pipeline (only while needed)

One task in `sources` (async fetch) feeding one decode thread:

- Fetch the media playlist every ~4 s, download new segments (live edge, keep
  only the newest 2 queued).
- Demux MPEG-TS by hand (PAT → PMT → H.264 PID → PES → Annex B).
- Decode with OpenH264 (`openh264` crate, statically built from source).
  Every frame is decoded (P-frames need it); every 2nd frame (15 fps) is
  converted straight from I420 to the panel's frame: the 280×280 square at
  (180, 15) of the 640×360 picture (above the HUD bar, clear of the LIVE badge
  and logo) scaled to 240×240 with a fixed bilinear map, pixels outside the
  circle skipped. Output paced to presentation timestamps.
- Frames go to the model as `Arc<VideoFrame>` (RGBA 240×240, 230 KB, two
  buffers recycled); the scene is `Drawable::Video(frame)` compared by
  pointer identity.
- Budget: < 12 MB RSS while running, ~0 when stopped; Pi 3B+ target
  < 15 % of one core while shown.

## Live detection

Per decoded frame (cheap, on a few dozen pixels of the full 640×360 picture,
YUV planes directly):

- **HUD present**: the bottom info bar (y ≥ 300) is a dark translucent band
  with the round mini-globe at x ≈ 20..90; live and replay have it, slate and
  promos do not.
- **Badge colour** at (18..62, 15..32): red = LIVE, orange = REPLAY.
- **Slate**: teal gradient + large white "sen" in the middle.
- State = Live only when HUD present and badge red, debounced (3 consecutive
  checks at 1/s to change state).
- **Night**: predicted from the ISS ephemeris and sun (ISS in Earth's
  shadow), with a 45 s margin for the stream delay; no fetching at night.

## When it runs

- Demand-driven: the stream runs while any panel shows `sen` or will show it
  within ~8 s (`ScreenState::upcoming`), and it is day for the ISS.
- At slot start the role is shown only if the state is Live; otherwise
  skipped. A non-live state while on screen calls `request_leave`.
- On a panel where `sen` is the only showable role, the stream keeps running
  on the day side (to notice live footage returning) and the panel shows the
  ISS globe until then.
- Stops (task + decoder dropped, buffers freed) 10 s after demand ends.

## UI

- Role `sen`, title "SEN LIVE", icon Material `videocam` in role lists and
  the web gallery; preview image from a real frame.
- No overlays on the video (no pill, no ring).
- Errors only reach the web UI/logs; the panel shows the globe fallback.

## Tests

- Resolver JSON parsing (fixture responses), master playlist variant pick,
  media playlist parsing.
- TS demux on a fixture segment (small, committed) → NAL units.
- Detector on fixture frames: live, replay, slate, promo, night-black.
- Scheduler integration: unavailable sen is skipped; leave request.
- Pi: RSS and CPU while shown, while warming, while stopped.

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
     ~1 Mbit/s); its media playlist lists 5 s MPEG-TS segments (video + AAC audio muxed;
     the audio PID is dropped in the demuxer).
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

Findings from the stream (2026-11-11): the camera-switch slate keeps the HUD
bar and the red LIVE badge (the teal "sen / SWITCHING CAMERA" card replaces
only the picture), so HUD and badge alone cannot spot it. Night frames are
near-black but also carry the HUD and a "LIVE · Night" label.

Per decoded frame (cheap, a few dozen to a few hundred samples of the full
640×360 picture, YUV planes directly, at most once per second):

- **Slate**: compare a coarse signature of the picture area above the HUD
  (e.g. 16×9 cell means of Y/Cb/Cr) with the slate signature taken from the
  `sen_slate.ts` fixture; the slate is a static teal gradient with a large
  white "sen" in the middle. Close = slate.
- **Badge colour** at (18..62, 15..32): red (Cr high, Cb ≈ 105+) = LIVE;
  orange (Cr high, Cb low) or missing = not live (replay, promos).
- **HUD present**: the bottom info bar (y ≥ 300) with the round mini-globe at
  x ≈ 20..90; missing = promo/other content.
- **Dark**: mean luma of the crop area very low = night (backup for the
  orbit prediction).
- State = Live only when not slate, badge red, HUD present and not dark;
  debounced (2 consecutive checks to become Live, 1 to become not-live so
  a slate is left quickly).
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

## As built (2026-11-11)

Where the implementation differs from the plan above, and why:

- **Segments without the playlist.** The media playlist lists the whole hour
  (720 segments, 870 KB; 10 KB gzipped)
  and is read once per start, keeping only its last 8 KB. After that the next
  segment's URL is the newest one with `/sq/<n>/` counted up: the URLs differ
  in nothing else, and the server holds a request for a segment that does not
  exist yet until it does (measured: `sq+1` answered after 2.6 s, `sq+2` after
  5 s, `sq+5` a 404). A 404 reads the playlist again, a 403 resolves again.
- **Every shown frame is classified**, not one a second: a check is 15 µs on
  the desktop, and this way no slate or night frame is ever handed out. Live
  takes 15 live pictures in a row (one second), leaving takes one.
- **Leaving live.** The slate and a missing badge end it on the first
  picture; a dark picture or a missing info bar only after four looked-at
  pictures in a row (0.25 s), and those pictures are held back meanwhile.
- **Info bar.** Found by its three cyan labels at the right (their summed Cr
  is 30 to 55 under the digits next to them, day, night and slate alike). A
  brightness step at the bar's top edge misread 102 live pictures in 22 real
  segments over dark ocean with clouds.
- **HTTP.** The stream has its own client, bound to IPv4 (the URLs are bound
  to the resolving address, and a dual-stack network could split the
  requests over two families), with redirects held to the stream's hosts.
  It asks for gzip itself; the shared client is unchanged.
- **Probing.** While the picture is not live the rest of a segment is not
  decoded (one keyframe per segment is looked at) and only every third
  segment is fetched.
- **Frame hold.** The model shows the last picture for 1.5 s after it
  arrived, so a screen leaving the role fades out from the picture and not
  from the globe; a stalled stream makes the role unavailable the same way.
- **Night** is computed in core from the ISS ephemeris and the subsolar point
  with a fixed 420 km altitude (`IssTrack::in_shadow`); without an ephemeris
  the stream runs and the dark check decides.
- **Frame buffers**: a pool of up to four (the model, the render workers'
  last scene, one in flight), reused once nobody else holds them.
- **Live page**: the visitor data is about 550 characters; the page is
  scanned while it downloads and dropped once both values are found.
- Measured on the desktop (release, 360p fixture): 0.28 ms demux + decode per
  picture, 0.10 ms crop + convert and 0.015 ms classify per shown frame;
  about 8 MB RSS while running, back to the start value when stopped.
  `cargo run --release -p rackscreen-sources --example sen -- bench|live|run`.

## Hardware decoding (2026-11-11)

On the Pi 3B+ OpenH264 took 26 % of a core for the 360p stream (every
picture is a reference, so none can be skipped). The decode thread now uses
the board's decoder where there is one:

- **Device.** `/dev/video10` (`bcm2835-codec`), a V4L2 stateful
  memory-to-memory decoder, driven by raw ioctls through `libc`
  (`sen/v4l2/device.rs`; no new dependency). 64-bit little-endian Linux
  with glibc only; elsewhere the module is a stub that never opens.
- **Flow.** One access unit per bitstream buffer (3 buffers, up to 512 KB
  each), numbered in the buffer's timestamp. The picture queue (4 buffers of
  350 KB, single-plane `YU12`) is built when the decoder reports the
  stream's format, and again on every source change. Nothing waits: each
  unit's call takes back what the decoder is done with and hands out the
  pictures of earlier units, which is the same one-picture delay OpenH264
  has. Only a full bitstream queue waits, a second at most.
- **No copy of pictures.** Detector and crop read the mapped buffer in
  place, through the padded layout the driver reports (640x368 rows for
  360 visible ones); the buffer goes back to the decoder right after.
- **Breaks.** After a skipped segment the pictures of units queued before
  it are recognised by their number and dropped; the decoder itself is not
  flushed.
- **Falling back.** No device, no H.264, a failed ioctl, a decoder that
  takes no unit for a second or gives no picture for 90 units: OpenH264
  takes over from the next keyframe, with one line in the log. Three
  failures in a row leave the hardware alone until the service restarts
  (a missing device counts as three). `RACKSCREEN_SEN_DECODER=software`
  turns it off, `=v4l2` ignores the count; `RACKSCREEN_SEN_V4L2_DEVICE`
  names another device.
- **Log.** `sen: decoder v4l2 (/dev/video10, bcm2835-codec)` or
  `sen: decoder software (openh264)` at every stream start, and after 30 s
  and then every 5 minutes
  `sen: decoder v4l2: 900 pictures in 30 s, 0.20 ms each, thread cpu 1.5 %`
  (time in the decoder per picture; CPU of the whole decode thread).
- **Not done.** The decoder is not drained with `V4L2_DEC_CMD_STOP` when
  the stream stops: the pictures left in it are not wanted, so the queues
  are stopped and the device is closed.

## Measured on the Pi 3B+ (2026-11-11, v0.20.2)

- Hardware decoding (V4L2, bcm2835-codec): `sen-decode` thread 6.8 % of one
  core at 30 decoded / 15 shown pictures per second, 0.27 ms per picture.
  OpenH264 in software was 26 %; reading the decoder's uncached buffers in
  place was 61 % before shown pictures were copied out first.
- RSS 59 MB with the stream running (58 MB without).
- The larger cost is pushing 15 full frames a second over SPI: about 28 % of
  one core on the SPI1 (aux, polled) bus that panel 4 uses; a `sen` panel on
  SPI0 costs much less.
- ISS globe: one raster about 21 ms, every 2 s.

### Long run (v0.20.4, 2026-11-11 03:57-04:07, panel 4 on SPI1)

| | 15 fps (5 min) | 10 fps (3 min) |
|---|---|---|
| shown rate | 15.0 steady | 10.0 steady |
| `sen-decode` thread | 6.4-7.0 % | 4.3-5.3 % |
| SPI1 bus thread | 26.8 % | 19.9 % |
| render of the panel | 0.3 ms/frame | 0.35 ms/frame |
| RSS | 60 MB | 60 MB |

The SPI1 cost is the wire time of ~95 KB a frame at 50 MHz on a bus the CPU
polls (no DMA on the aux controller): round-panel clipping in 30-row bands
is already the cheapest split under the window cost, and every picture of
moving Earth changes nearly every pixel. `sen.fps` (10/15/30) is the lever.
During the 10 fps run a real loss of signal switched the stream to REPLAY
(04:05:23): the panel fell back to the globe and returned to live at
04:06:30. Detector calibration on an hour of real stream (360 segments):
292 live, 38 night, 15 REPLAY, 5 Trailer, 7 with the slate; no replay or
trailer picture accepted, no sampled live one refused.

### Video push (v0.20.5/v0.20.6)

Video frames are diffed against what the panel shows, ignoring changes of
at most one RGB565 step, in fine row bands (up to 16 rects, only inside the
circle); `prev` mirrors the panel by copying only what was sent. Replaying
real segments through the real code (`cargo run --release -p
rackscreen-render --example pushsim -- seg.ts...`): 41-77 % of a full push
against 92-98 % for the exact diff. On the Pi (05:19-05:27, 15 fps, SPI1):
a fully live minute pushed 63.9 KB a frame (86-97 KB before), a dark
sunrise minute 3.8 KB; slates and a replay in the same window were left
and rejoined correctly.

### SPI0 vs SPI1 (controlled, 2026-11-11 05:41-05:46, other panels static)

| `sen` on | process CPU | that bus's thread |
|---|---|---|
| panel 2, SPI0 (DMA) | 30 % | < 3 % |
| panel 4, SPI1 (aux, polled) | 40 % | 28.8 % |

Pushing video costs almost nothing on SPI0, where the controller's DMA does
the transfer; on SPI1 the CPU polls for the whole wire time. Put `sen` on a
panel of SPI0 (the Pi config now has it on panel 2).

### Production run (v0.20.6, 2026-11-11 06:58-07:40, real config, untouched)

`sen` 60 s in panel 2's cycle of eleven roles, the other panels cycling as
usual. 15 slots came up in the ISS day pass:
- 07:00 live, then REPLAY at 07:00:24: left, stream stopped 10 s later.
- 07:02-07:09: the replay went on; each slot warmed the stream up for 18 s,
  saw no live footage and was skipped (never shown).
- 07:10-07:36: nine slots live within 5-6 s of the warm-up start, ~45 s of
  15 fps video each, decode thread 6.4-8.1 %, 0.20-0.26 ms a picture.
- 07:31:21 camera-switch slate: left at once. 07:36:22 dusk (Dark): left.
- 07:37:58 last warm-up near the predicted night: not live, skipped.
- The stream URL was resolved once (07:00) and reused for every slot.
Whole process over the 42 min: 48.5 % of one core with all four panels
cycling (GIFs included), RSS 64.7 MB (peak 68 MB), 15 threads.

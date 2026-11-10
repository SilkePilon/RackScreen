# ISS globe screen

Brainstormed 2026-11-10 with live browser mockups
(`.superpowers/brainstorm/2157170-1794347913/content/`, final look
`globe-b-v3.html`: style G2 "illustrated", B layout, satellite icon). Replaces
the countdown-only `iss` role look.

## Look (240 × 240 round panel)

- **Globe**: orthographic, radius 95 px, centre (120, 120), drawn per pixel.
  Five flat colours: ocean `#163A5C`, forest `#2C5C3E`, grass `#608248`,
  desert `#C4A66C`, ice `#E8EEF2`. Lit by the real sun as a two-step band:
  day ×1.0, twilight (sun dot product in -0.06..0.06) ×0.62, night ×0.30
  with blue boosted ×1.25 in the darker steps; steps blend over ~1.5 px.
  Night side shows city lights as amber `#F2B84B` where the lights map is
  bright. Slight limb darkening (×0.88 at the edge). Anti-aliased rim.
- **Stability** (no flicker): the colour map is cleaned with a majority
  filter (removes single-texel specks), sampled bilinearly, 4 samples per
  pixel.
- **Camera** follows the ISS: centre = (ISS lon − 18°, ISS lat × 0.55),
  eased.
- **Track**: past 50 min solid white 2 px over a 4 px 40 % black shadow;
  next orbit dashed white 70 % (3 on, 4 off, 1.5 px). Both only the
  continuous stretch on the near side, walked outward from the ISS and cut
  at the horizon (no part re-appearing from the back side).
- **ISS marker**: Material `satellite_alt` 20 px white with a 1.5 px dark
  (`#0B0B0C` 85 %) outline, upright.
- **Home dot**: white 2.6 px radius with a 1 px dark ring, drawn above the
  ISS marker; only during the by-eye visible window of a pass (ISS ≥ 10°,
  observer sun < −6°, ISS sunlit); fades in/out.
- **Ring**: standard P2 ring (r 106, 9 px, 32° gap, satellite icon in the
  gap), fill white (`TEXT`) = 1 − (time to next pass / 90 min); full during
  a pass.
- **Pill**: standard P2 pill. Before: `"12:40 · max 22°"` in `TEXT`
  (h:mm:ss when ≥ 1 h). During: `"Overhead · max 22°"` in `CYAN`. None:
  `"No pass in 24 h"` in `TEXT2`. Pill centre y = 190 (below the globe
  centre, standard size).

## Data

- `sources::iss` already fetches the TLE daily and runs SGP4. It adds:
  - **Ephemeris event** `Event::IssTrack { t0, step_secs: 20, points:
    Vec<(lat, lon)> }` covering now − 60 min .. now + 100 min, re-emitted
    every 5 min (and right after a TLE refresh).
  - `IssPass` gains `vis: Option<(i64, i64)>`, the by-eye visible window of
    the pass (first/last instant ISS ≥ 10°, observer dark, ISS sunlit).
- `core` interpolates the ISS position between ephemeris points (slerp of
  unit vectors) and computes the subsolar point itself (small formula; no
  `sgp4` dependency in core).

## Rendering and cost

- New `Drawable::Globe { cam_lat, cam_lon, sun_lat, sun_lon, r }`; the
  renderer keeps the last globe raster and reuses it when the parameters are
  equal. The scene quantises the globe parameters to whole seconds of time,
  so the globe re-rasterises at most once per second (the ISS moves
  ~0.1 px/s on screen, so this is visually continuous). Precomputed per-pixel
  sphere normals; per frame only a 3×3 rotation, fast `asin`/`atan2`
  approximations and bilinear lookups.
- Earth maps are generated once by `scripts/earth-maps.py` from NASA Blue
  Marble (land/ice colours), the water mask and Black Marble city lights
  (public domain) into `assets/earth/classes.rle` (1024×512 class indices,
  run-length coded) and `assets/earth/lights.bin` (512×256 u8). Embedded;
  decoded lazily on first use (~0.6 MB RAM).
- Track, marker, dot, ring and pill are ordinary drawables.

## Tests

- core: ephemeris interpolation, near-side track cut, pill/ring states,
  home dot only inside the visible window.
- render: globe golden (`p2_iss`, `p2_iss_pass`, `p2_iss_nodata`), raster
  reuse when params equal.
- sources: visible-window computation against the fixture TLE.
- Pi: render ms and CPU with the iss role on screen.

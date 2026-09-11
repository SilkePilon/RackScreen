# Price role: level gauge on Energy-Charts

Date: 2026-09-11
Status: approved by Silke in brainstorming session (mockup 04 of 10)
Builds on: `2026-09-06-screen-roles-electricity-design.md` (the `price` role and its sources)

## Purpose

The `price` role shows a ring of 48 green-to-red stubs, a euro glyph that carries no
information and a badge that flips between the current price and the daily minimum.
It is hard to read from across the room. This spec replaces it with a **level gauge**:
a five-band arc, a needle at today's level, the current price in euro as the centre
piece and a one-word verdict (`CHEAP`, `PRICEY`, ...) in the badge.

At the same time the two price sources (EnergyZero, ENTSO-E) are replaced by one:
**Energy-Charts** by Fraunhofer ISE.

## Decisions taken during brainstorming

- Ten alternatives were mocked (hero number, radial bars, clock face, level gauge,
  cheapest window, today+tomorrow, sparkline, quarter-hour ring, price+clean signal,
  deviation from average). The level gauge was chosen.
- Price is shown in **euro per kWh** (`0.171`), not cents.
- The level is the **ratio of the current price to a rolling three-day average**,
  Tibber's method, not the rank within today and not fixed thresholds. Rank within
  today mislabels flat or extreme days; fixed thresholds need hand tuning.
- One API only: **Energy-Charts** (`api.energy-charts.info`). No token, 40+ European
  bidding zones, 15-minute resolution, history and today in one call, more endpoints
  for later roles (`signal`, `ren_share_forecast`, `frequency`). Licence CC BY 4.0,
  which needs an attribution line. EnergyZero (NL only) and ENTSO-E (token by mail)
  are dropped.
- Energy-Charts reports the raw exchange price without VAT; a local `vat_pct`
  multiplier (default 21) replaces EnergyZero's `include_vat`.
- Energy-Charts answers `429 Too Many Requests` on bursts. One request per poll,
  poll interval never below 15 minutes.

## Screen

Everything is drawn with the existing primitives plus one new `Drawable::Text`.

**Arc.** One `Drawable::Ring` at `RING_R` with 45 segments at a 6° pitch, starting at
225° (7:30 o'clock) and sweeping clockwise to 495° (4:30 o'clock). Five bands of nine
segments. Colours, first to last: `GREEN`, `GREEN.mix(AMBER, 0.5)`, `AMBER`,
`AMBER.mix(RED, 0.5)`, `RED`. The last segment of bands one to four is `Off`, so a gap
separates the bands; band five keeps all nine. The band under the needle is drawn at
alpha 1.0, the other bands at 0.45. The segment directly under the needle breathes
(`breathe(now, 2.4)`), the house rule that the live segment is never static.

**Needle.** One `Drawable::Tick`, white, `r0 = 88`, `r1 = 116`, width 4, at angle
`225 + 270 * t`. `t` is the needle position in `0..=1` and is eased with a `Smooth`
in the model (same `SMOOTH_SECS` as the other rings), so a new slot swings the needle
instead of jumping it. With no data the needle is not drawn.

**Needle position from the ratio.** `ratio = price / avg`. Band boundaries at ratio
0.6, 0.9, 1.15 and 1.4 (Tibber's cut-offs). Each band spans one fifth of the arc and
the ratio is linear inside its band:

| band | word       | ratio range   | t range     |
|------|------------|---------------|-------------|
| 1    | `V.CHEAP`  | 0.30 to 0.60  | 0.0 to 0.2  |
| 2    | `CHEAP`    | 0.60 to 0.90  | 0.2 to 0.4  |
| 3    | `NORMAL`   | 0.90 to 1.15  | 0.4 to 0.6  |
| 4    | `PRICEY`   | 1.15 to 1.40  | 0.6 to 0.8  |
| 5    | `V.PRICEY` | 1.40 to 1.70  | 0.8 to 1.0  |

Ratios below 0.30 clamp to `t = 0`, above 1.70 to `t = 1`. An average at or below zero
(or non-finite) gives `NORMAL` and `t = 0.5`. A negative price with a positive average
gives a ratio below zero and lands in `V.CHEAP` at `t = 0`.

**Centre.** `Drawable::Text` with the current slot price formatted `{:.3}` (three
decimals, so cheap slots keep digits: `0.019`, `-0.006`), 40 px, at `cy = 100`.
White, `BLUE` when the price is negative. Under it a `Drawable::Text` caption
`€/kWh`, 11 px, `GREY`, at `cy = 130`.

**Badge.** The band word, in the usual `badge()` at `BADGE_CY` (165) but 84 px wide,
13 px text, stroke in the band colour.

**No data** (`!prices.have`, or no finite value for the current slot): centre text
`--` in `GREY`, no needle, arc bands all at alpha 0.45, badge `no data` in `GREY`.
The simulator key `p` (price outage) keeps producing this state.

**Current slot.** `slot = local_hour * 4 + local_minute / 15`, `0..96`. The runloop
already computes local minutes; `Model` gets `set_local_slot(u32)` next to
`set_local_hour`, and the gifs example and the tests set it directly.

## Data

### Event and model

```rust
Event::Prices {
    date: String,               // YYYY-MM-DD, local
    eur_per_kwh: Vec<f32>,      // 96 quarter-hour slots for `date`, NaN where unknown
    avg_eur_per_kwh: f32,       // mean of every finite slot of today and the two days before
    currency: String,           // "EUR"
}
```

`PriceState` in the model mirrors it (`eur: Vec<f32>`, `avg: f32`, `have: bool`).
The model owns the needle `Smooth`; on every `tick` it recomputes the target `t` from
the current slot and `apply(Prices)` resets `avg`. `ct_per_kwh` disappears; every
consumer (scene, fake source, sim, golden tests, gifs) moves to euro and 96 slots.

### Source

`crates/sources/src/prices.rs` keeps its name and loop shape (poll, `Link` up/down
after two failures, backoff) and loses both old branches. Per poll, one request:

```
GET https://api.energy-charts.info/v2/price?bzn={zone}&start={today-2}&end={today}
```

`start`/`end` are local calendar days (`YYYY-MM-DD`); Energy-Charts interprets them in
the zone's own time zone and returns every quarter-hour of those days.

Response (schema 2.0):

```json
{ "resolution": "PT15M", "unit": "EUR / MWh",
  "data": [ { "timestamp": "2026-09-09T00:00:00+02:00",
              "values": { "day_ahead_price": 150.31 } }, ... ] }
```

`parse_energy_charts(json) -> Vec<(DateTime<Utc>, f32)>` reads `data[].timestamp`
(RFC 3339 with offset) and `values.day_ahead_price`, skipping entries where either is
missing or null. Conversion: `eur_per_kwh = eur_per_mwh / 1000 * (1 + vat_pct / 100)`.

`slots_for_day(samples, day, tz) -> Vec<f32>` (96 entries) buckets samples into the
local quarter-hours of `day` using the Pi's zone, exactly as `hourly_ct_for_day` does
today, so a DST day still works (23 or 25 local hours, the extra or missing hour is
simply missing or averaged). The three-day average is the mean of every finite sample
whose local date is one of the three requested days; it is computed from the samples,
not from the bucketed slots, so no bucketing artefact leaks in.

A `429` is an ordinary failure: it goes through the existing failure counter and
backoff. Every request carries the existing `client()` user agent.

### Zone

`PriceConfig { zone: String, vat_pct: f32, poll_secs: u64, tz: Local }`.

`energy_charts_zone_for(country: &str) -> Option<&'static str>` replaces
`entsoe_zone_for` and maps Electricity Maps zones to Energy-Charts bidding zones:

| Electricity Maps | Energy-Charts |
|---|---|
| `NL` `BE` `FR` `AT` `CH` `ES` `PT` `PL` `FI` `CZ` | same |
| `DE` `DE-LU` | `DE-LU` |
| `DK-DK1` `DK-DK2` | `DK1` `DK2` |
| `NO-NO1` .. `NO-NO5` | `NO1` .. `NO5` |
| `SE-SE1` .. `SE-SE4` | `SE1` .. `SE4` |
| `IT-NO` | `IT-North` |

`NL`, `BE`, `DE-LU`, `DK1`, `NO1` and `CH` were verified live on 2026-09-11; the rest
follow the published zone list. `run.rs` resolves the zone: `price.zone` when set,
otherwise the table on `electricity.zone`; when neither gives a zone, prices stay off
and the log says `price zone unknown for electricity zone X; set price.zone`.

### Config

```yaml
price:
  enabled: true            # Energy-Charts day-ahead prices, no key
  zone: ""                 # Energy-Charts bidding zone, e.g. NL, DE-LU, DK1; derived from electricity.zone when empty
  vat_pct: 21              # added to the raw exchange price
  poll_secs: 900           # minimum 900; Energy-Charts rate-limits bursts
```

`PriceCfg { enabled: bool, zone: String, vat_pct: f32, poll_secs: u64 }` with
defaults `true`, `""`, `21.0`, `900`. Validation: `vat_pct` in `0..=100`,
`poll_secs >= 900`. The struct is `#[serde(default)]` and not `deny_unknown_fields`,
so an old file with `source`, `entsoe_token`, `entsoe_zone` or `include_vat` still
loads; those keys are dropped on the next save. An old `source: none` is no longer
honoured; that user sets `enabled: false` once.

## Renderer

New `Drawable::Text { cx, cy, text: String, px: f32, color: Color, alpha: f32 }`,
drawn with the existing `draw_centered`. It is what `Badge` already does minus the
rectangle. Nothing else changes in `rackscreen-render`.

## Setup TUI

**Configure** replaces the three price fields (`price source`, `entsoe token`,
`entsoe zone`) and `price incl. VAT` with:

- `price enabled` (toggle, Enter flips it)
- `price zone` (text, upper-cased, empty allowed; hint: "Energy-Charts bidding zone, empty derives from the electricity zone")
- `price vat %` (number, 0 to 100)

`price poll secs` keeps its field, minimum 900.

**Screens** role hint for `price`: `prices off` when `enabled` is false; `no price zone`
when `enabled` and neither `price.zone` nor the table on `electricity.zone` yields a
zone; otherwise none. **Status** keeps its `prices` link dot unchanged.

## README

- Screens table: `price` — the day-ahead price as a level gauge, the needle at today's level against the three-day average.
- Electricity mode: the price paragraphs describe Energy-Charts, `price zone`, `price vat %`, the 15-minute slots and the time zone note; the EnergyZero and ENTSO-E text goes.
- Configuration example: the new `price:` block.
- Licence: "Day-ahead prices come from [Energy-Charts](https://www.energy-charts.info) by Fraunhofer ISE under CC BY 4.0."
- `.github/media/screens/price.gif` regenerated with `cargo run --example gifs`.

## Fake source and simulator

`PRICE_CURVE` (24 hourly values, ct) becomes 96 quarter-hour euro values by linear
interpolation of the same shape divided by 100, and the fake emits
`avg_eur_per_kwh` as the mean of the curve so the demo needle sits around `NORMAL`
and travels the bands as the curve rotates. The `p` key still drops `have`.

## Tests

- `scene_electricity`: a `Prices` event with a known curve and average produces 45
  segments, the right band alpha pattern, the needle at the expected angle for each
  band boundary ratio (0.6, 0.9, 1.15, 1.4), the words for each band, `BLUE` centre
  text for a negative slot, the clamped ends, and the no-data state.
- `prices`: `parse_energy_charts` on a saved fixture (`crates/sources/tests/fixtures/energy-charts-price.json`,
  four days of NL as fetched on 2026-09-11, trimmed), `slots_for_day` on the
  spring-forward and fall-back days, the three-day average, the VAT multiplier, and
  `energy_charts_zone_for` on every table row plus an unknown zone.
- `config`: defaults, `vat_pct` and `poll_secs` bounds, an old file with the dropped
  keys still loads.
- `setup`: the new Configure fields get and set, the role hints for `enabled: false`
  and an unknown zone.
- `render` golden: `price` regenerated and reviewed by eye.
- `model`: `set_local_slot`, the needle `Smooth` easing toward the new target.

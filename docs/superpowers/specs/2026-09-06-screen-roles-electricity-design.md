# Screen roles, cycling, Electricity mode, Thermal and Storage screens

Date: 2026-09-06
Status: approved by Silke in brainstorming session (mockups v6)
Builds on: `2026-09-05-rackscreen-design.md` (v0.1.0) and `2026-09-06-setup-tui-design.md` (v0.2.0)

## Purpose

1. Screens become configurable: every physical screen has an ordered list of roles and cycles through them. The TUI gives full control over which screen shows what and the timing.
2. Two new cluster roles: **Thermal** (node temperatures) and **Storage** (Longhorn).
3. **Electricity mode**: four roles fed by the Electricity Maps API and a day-ahead price source, using the site's colours and icons: **Power mix**, **Price**, **Carbon intensity**, **Renewable / carbon-free**.

## Decisions taken during brainstorming

- Electricity data from the official Electricity Maps API with a free personal token. Scraping the map is impossible (Cloudflare challenge) and against their terms.
- Price from EnergyZero (Netherlands, no token) or ENTSO-E (any EU zone, free token); config selects.
- Colours and icons are the ones the site renders today (captured from its markup); the web app is AGPL-3.0, so the icon files ship under that licence with attribution.
- Power mix is one ring split by share, icons in source colour on an inner circle, short dark tick between icon and section, no centre icon, unused sources hidden.
- Thermal uses the Health-screen grammar (ring, thermometer icon, temperature-coloured node dots in a wide badge). The heat-grid variant was rejected.
- Badges that hold node dots grow with the node count (fixes the 8-node overflow on Health).

## Roles and cycling

Role identifiers (config strings): `cpu`, `mem`, `pods`, `health`, `thermal`, `storage`, `power-mix`, `price`, `carbon`, `renewable`.

Config:

```yaml
screens:
  - { roles: [cpu, thermal], cycle_secs: 15, spi: 0, cs: 0, dc: 6,  rst: 5,  rotate: 270, hflip: false, hz: 40000000 }
  - { roles: [mem],          cycle_secs: 15, spi: 0, cs: 1, dc: 13, rst: 26, rotate: 270, hflip: true,  hz: 40000000 }
  - { roles: [pods, storage], cycle_secs: 20, ... }
  - { roles: [health],       ... }
```

- `roles` replaces `role`. Loading a config that still has `role: cpu` is accepted and upgraded in memory to `roles: [cpu]` (and written back that way on the next save). At least one role per screen; unknown role names fail validation.
- `cycle_secs` (default 15, minimum 3) is the dwell time per role; one role = static, the field is ignored.
- Cycling is per screen, independent phases. A splash or sweep never interrupts mid-transition: transitions wait until no fx is active on that screen; a splash targeting a role that is not currently shown is dropped (the persistent marker still applies when the role comes up).
- Transition "iris", 500 ms total: the current scene's ring segments turn off from the last lit segment backwards and the icon/badge shrink to nothing (250 ms, ease-in); the next scene grows from the centre and its ring lights clockwise (250 ms, ease-out). Implemented as a scene-level transform: `Scene` gets an optional `zoom: f32` and `ring_reveal: f32` that the renderer applies (scale about the centre; only the first `ring_reveal` fraction of segments drawn).
- Presets in the TUI: `cluster` = `[cpu] [mem] [pods] [health]`, `electricity` = `[power-mix] [price] [carbon] [renewable]`, `mixed` = `[cpu, power-mix] [mem, price] [pods, carbon] [health, renewable]` all at 15 s.

Splash/sweep routing: events keep their target role. The model maps a role to the screen(s) currently listing it; local splashes play on any screen whose current role matches; rack sweeps are unchanged.

## Events

```rust
pub enum Source { Nuclear, Geothermal, Biomass, Coal, Wind, Solar, Hydro, Gas, Oil, Unknown, HydroStorage, BatteryStorage }

Event::Electricity { zone: String, mix_mw: Vec<(Source, f32)>, renewable_pct: f32, fossil_free_pct: f32, carbon_gco2: f32, updated_at: String }
Event::Prices { date: String /* YYYY-MM-DD local */, ct_per_kwh: Vec<f32> /* 24, may be shorter before publication */, currency: String }
Event::NodeTemps(Vec<(String /* node name */, f32 /* °C */)>)   // sorted by name
Event::Storage { volumes: Vec<(String, Robustness)>, used_bytes: u64, capacity_bytes: u64 }
Event::HotTemp { node: String, celsius: f32 }
Event::VolumeDegraded { name: String, robustness: Robustness }
Event::VolumeHealthy { name: String }
Event::Link { target: LinkTarget::Electricity | LinkTarget::Prices, up }

pub enum Robustness { Healthy, Degraded, Faulted, Unknown }
```

`Source` order above is the display order for ties; `mix_mw` is production (`powerProductionBreakdown`) with `hydro discharge` mapped to `HydroStorage` and `battery discharge` to `BatteryStorage`; sources with `null` or `0` MW are omitted.

## Sources

### Electricity Maps

- Config:
```yaml
electricity:
  enabled: true
  zone: NL
  token: ""            # free personal token from app.electricitymaps.com
  poll_secs: 300
```
- Endpoints: `GET https://api.electricitymap.org/v3/power-breakdown/latest?zone={zone}` and `GET .../carbon-intensity/latest?zone={zone}`, header `auth-token: {token}`. Fields used: `powerProductionBreakdown`, `renewablePercentage`, `fossilFreePercentage`, `datetime`; `carbonIntensity`.
- One `Event::Electricity` per successful poll pair. Two consecutive failures emit `Link { Electricity, false }`; success emits `up`. Missing token or `enabled: false`: source not started, link stays down, electricity roles show No-data with a `key` icon instead of `cloud-off` when the token is empty.
- HTTP via `reqwest` (rustls) with a 10 s timeout; no port-forward involved.

### Prices

- Config:
```yaml
price:
  source: energyzero      # energyzero | entsoe | none
  entsoe_token: ""
  entsoe_zone: 10YNL----------L   # EIC code; default derived from `electricity.zone` for NL, BE, DE, FR, AT
  include_vat: true       # energyzero only; entsoe prices are ex VAT
  poll_secs: 900
```
- EnergyZero: `GET https://api.energyzero.nl/v1/energyprices?fromDate={day}T00:00:00.000Z&tillDate={day}T23:59:59.999Z&interval=4&usageType=1&inclBtw={true|false}` → `Prices[].{readingDate, price}` in €/kWh; converted to ct/kWh. Fetched for today in local time (Europe/Amsterdam); after 15:00 also tomorrow so the ring can show the evening.
- ENTSO-E: `GET https://web-api.tp.entsoe.eu/api?securityToken=..&documentType=A44&in_Domain=..&out_Domain=..&periodStart=..&periodEnd=..` XML (`Publication_MarketDocument`) parsed with `quick-xml`; €/MWh → ct/kWh ex VAT.
- Emits `Event::Prices` with 24 hourly values for the local day (missing hours as `NaN`, rendered unlit). Failures follow the same two-strike link rule with `LinkTarget::Prices`.

### Prometheus additions

- Node temperatures every poll: `max by (nodename) (node_hwmon_temp_celsius * on(instance) group_left(nodename) node_uname_info)`; if empty, fall back to `node_thermal_zone_temp` with the same join. Emits `NodeTemps` sorted by node name. `HotTemp` when a node exceeds `thresholds.hot_temp` (default 70 °C), debounced 5 minutes per node like HotNode.
- Longhorn every poll: `longhorn_volume_robustness` (0 unknown, 1 healthy, 2 degraded, 3 faulted) per `volume`, `sum(longhorn_volume_actual_size_bytes)`, `sum(longhorn_volume_capacity_bytes)`. Emits `Storage`; robustness transitions healthy→degraded/faulted emit `VolumeDegraded`, back to healthy emit `VolumeHealthy`. If no Longhorn metrics exist the storage role shows No-data.

### Fake source

Adds deterministic electricity mix (drifting shares), prices (a plausible daily curve), temperatures (7 nodes named like the real cluster) and storage (21 volumes). Keys: `9` degrade a volume, `0` heal it, `h` hot node temperature, `p` toggle price source outage.

## Screens

Geometry in the 240 px space, shared constants unchanged (ring radius 102, 60 segments, badge 64x26 at y 165, icon 72 px at y 98).

### Power mix

- Ring: 60 segments partitioned by production share, biggest source first starting at 12 o'clock, clockwise, in source colour. Between two sources one segment is left unlit as a gap (taken from the larger neighbour). A source gets at least one segment if it has any production. Sources with zero production are absent.
- Icons: for every source with at least 3 segments, its icon in the source colour, 26 px, centred on a circle of radius 60 at the angular centre of its section. A 3 px round-capped tick in `OFF` colour (`#1c1c1c`) from radius 80 to 88 on the same angle. No centre icon, no badge.
- Motion: shares are `Smooth`ed; section boundaries and icon angles move with them. The lead source's last segment breathes. When the lead source changes, the ring rotates so the new leader starts at 12 o'clock (Smooth on the start angle over 800 ms).
- Colours: biomass `#008043`, geothermal `#A73C15`, hydro `#1878EA`, solar `#FFC700`, wind `#69D6F8`, nuclear `#9D71F7`, battery storage `#1DA484`, hydro storage `#2B3CD8`, coal `#ac8c35`, gas `#AAA189`, oil `#584745`, unknown `#ACACAC`.

### Price

- Ring: 48 segments at 7.5° pitch, two per hour, so each hour owns a 15° slot; midnight at 12 o'clock, clockwise. Colour per hour from cheap green `#3ddc97` to expensive red `#ff4d4d`, linear between the day's min and max; hours without data unlit. Past hours at 35 % alpha, current hour breathing, future hours full.
- Icon: Lucide `euro` (72 px). Badge: current hour price `22.1 ct` (one decimal, `ct` suffix; `>= 100` shown as `1.02 €`), stroke in the current hour's colour. Negative prices show `-1.3 ct` in blue `#4f8dff`.
- Every 5 s the badge flips to the day's minimum with its hour (`min 6.2`), then back.

### Carbon intensity

- Ring fill = `carbon_gco2 / 800` clamped, colour from the site's scale: 0 `#2AA364`, 150 `#F5EB4D`, 600 `#9E4229`, 800 `#381D02` (linear between stops). Lucide `cloud` icon tinted the same colour. Badge `214 g`, stroke in the same colour.

### Renewable / carbon-free

- Outer ring (r 102) renewable %, green `#3ddc97`; inner ring (r 84) carbon-free %, blue `#69D6F8`. Lucide `leaf` icon (60 px at y 92). Badge alternates `61%` (green stroke) and `73%` (blue stroke) every 5 s with the same fade used by the torrent badge; a 6 px dot left of the number in the ring's colour tells which is which.

### Thermal

- Ring = hottest node temperature on 0–100 °C, colour by temperature: 35 °C `#4f8dff` → 55 °C `#ffb020` → 70 °C `#ff4d4d`. Lucide `thermometer` icon. Wide badge with one dot per node (sorted by name) in its temperature colour, hottest dot breathing; badge text alternates every 5 s between the hottest temp `54°` and the average `46°`.
- `HotTemp` splash: red, `flame` icon, on the thermal role.

### Storage

- Outer ring: one equal section per volume (with unlit gap segments when there are ≤ 20 volumes; otherwise contiguous), green healthy, amber degraded, red faulted, grey unknown. When every volume is healthy the ring is drawn as one solid violet `#a78bfa` ring (no gaps) so the calm state matches the other screens.
- Inner ring (r 80): used / capacity %, blue.
- Lucide `database` icon. Badge cycles every 4 s: `21/21` (violet stroke) → `49 GB` (blue) → `38%` (blue).
- `VolumeDegraded` splash: amber, `database-zap` icon (Lucide), persistent amber marker while any volume is not healthy. `VolumeHealthy` splash: green `database` icon.

### Wide badge

`Drawable::Badge` gets an optional `w` override; scenes with node dots compute `w = max(64, 12·n + 16)`, dots 12 px apart, radius 4.5. Applies to Health and Thermal.

### No-data variants

Electricity roles without a token show the No-data ring with Lucide `key-round`; without link (API down) `cloud-off` as today. Thermal and Storage show `cloud-off` when their metrics are absent.

## Rendering

- `Drawable::Ring` gains `pitch_deg: Option<f32>` (default 360/n) and `start_deg: f32` so the price ring (48 segments at 7.5°) and the rotating power-mix ring work with the existing segment cache (cache key includes pitch and start).
- `Drawable::Icon` accepts any name from the embedded icon set; the Electricity Maps icons live in `assets/icons/em/{biomass,geothermal,hydro,solar,wind,nuclear,battery-storage,hydro-storage,coal,gas,oil,unknown}.svg` (8×8 or 16×16 viewBox as captured) with `assets/icons/em/LICENSE` (AGPL-3.0 notice and source URL). The icon loader learns per-icon `units` from the SVG's `viewBox` instead of assuming 24.
- Filled icons: the loader already collects fill paths; the coal icon's nested groups with matrix transforms and a stroked path are handled by usvg's absolute transforms.
- New Lucide icons: `euro`, `cloud`, `leaf`, `thermometer`, `database`, `database-zap`, `key-round`.
- Scene transitions: renderer applies `zoom` (scale about centre) and `ring_reveal` (draw only the first fraction of every ring's segments) when set on the `Scene`.

## Model

- `Model` keeps electricity, prices, temps and storage state plus `Smooth` values for mix shares and percentages.
- `ScreenState` per physical screen: `roles`, `current: usize`, `since: Secs`, `transition: Option<Transition { from, to, started }>`; advanced in `Model::tick`. `Model::scene(screen_index, now)` replaces `scene(role, now)`: it resolves the current role, builds the scene, applies the transition transform, and layers splashes whose role matches.
- Health scene uses the wide badge for its node dots.

## TUI

### Screens editor (new menu item between Calibrate and Configure)

```
▸ 1  cpu › thermal            every 15 s
  2  mem                      static
  3  pods › storage           every 20 s
  4  health                   static
Presets:  c cluster   e electricity   m mixed   s save
```
- `↑↓` pick screen, `⏎` opens the role picker (list of all roles; `space` toggles, `↑↓` moves the cursor, `J/K` move the selected role up/down in the screen's order, `⏎` done), `+`/`-` change `cycle_secs` by 5 (min 3, max 300), presets overwrite all rows, `s` validates and saves (restart prompt like Configure), `Esc` discards.
- Roles that lack their data source in the config are shown with a `!` (e.g. electricity roles with no token) but allowed.

### Configure additions

Fields: `electricity enabled`, `zone`, `token` (secret), `poll secs`, `price source` (cycles energyzero / entsoe / none), `entsoe token` (secret), `entsoe zone`, `include vat`, `hot temp °C`.

### Status additions

Link dots for `electricity` and `prices` from the journal (`electricity: ok` / warn lines).

## Config summary (new keys)

```yaml
screens: [{ roles: [...], cycle_secs: 15, ... }]
thresholds: { hot_cpu: 90, hot_mem: 90, hot_temp: 70 }
electricity: { enabled: false, zone: NL, token: "", poll_secs: 300 }
price: { source: energyzero, entsoe_token: "", entsoe_zone: "", include_vat: true, poll_secs: 900 }
```
`electricity.enabled` defaults to false so a fresh install keeps working without a token; the Screens editor `e` preset turns it on and reminds to set the token.

## Testing

- core: role parsing and upgrade from `role:`; cycle timing and transition phases; splash routing to screens by current role; power-mix partition (gaps, minimum one segment, leader at 12 o'clock, icons only ≥ 3 segments, zero sources absent); price ring colouring and past/current/future alpha; carbon colour scale stops; thermal colour and wide badge width; storage ring sections and solid-when-healthy.
- sources: Electricity Maps JSON parsing (fixtures with nulls and storage keys), EnergyZero JSON and ENTSO-E XML parsing (fixtures), temperature join query parsing, Longhorn robustness transitions, fake source determinism.
- render: golden images for the six new scenes plus a mid-transition frame; icon loader test that all 12 EM icons parse and render non-empty.
- setup: Screens editor model (pure state: toggle, reorder, presets, interval clamp) and a TestBackend snapshot; Configure round-trip for the new fields.
- Manual: simulator with fake data, then Pi.

## Out of scope

- Forecasts, history graphs, per-node CPU rings, Pi-hole and Home Assistant (no exporters).

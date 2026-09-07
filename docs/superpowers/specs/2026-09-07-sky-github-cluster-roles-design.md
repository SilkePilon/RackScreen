# Sky, GitHub and more cluster roles (v0.4.0)

Date: 2026-09-07
Status: approved by Silke in brainstorming session (visual catalog, 11 of 22 panels picked)
Builds on: `2026-09-06-screen-roles-electricity-design.md` (v0.3.0), `2026-09-06-setup-tui-design.md` (v0.2.0)

## Purpose

Eleven new roles so a screen can show more than the cluster and the grid:

| Role | Data | Ring and badge |
|---|---|---|
| `gh-activity` | GitHub GraphQL + events | today's contributions outside, last 7 days inside, badge today's count |
| `weather` | Open-Meteo | temperature ring, icon from the WMO code, badge °C |
| `wind` | Open-Meteo | compass arc where the wind comes from, badge km/h |
| `aqi` | Open-Meteo air quality | European AQI ring on the EEA colour bands, badge the index |
| `rain` | Buienradar raintext | next two hours in 5-minute slots, badge minutes to rain |
| `sun` | computed | 24-hour dial with daylight, badge countdown to sunset or sunrise |
| `moon` | computed | illuminated fraction, badge percent alternating with the phase |
| `iss` | Celestrak TLE + SGP4 | countdown to the next ISS pass, badge minutes |
| `ups` | Prometheus (nut-exporter) | battery charge outside, load inside, badge runtime |
| `net` | Prometheus (node-exporter) | download outside, upload inside, badge Mbit/s |
| `deploys` | Argo CD Applications | one arc per app by sync and health, badge healthy over total |

All 11 ship together as v0.4.0 (Silke chose one milestone over three).

## Decisions taken during brainstorming

- One source module per API, each a copy of the `electricity.rs` poll loop (backoff, two-strike link edge, pure parse functions with fixtures). Rejected: a generic config-driven gauge role (loses the picked designs) and one fat poller for everything (one slow API stalls the rest).
- Open Notify no longer predicts passes, so `iss` fetches the TLE from Celestrak and computes passes on the Pi with the `sgp4` crate.
- `github` needs only a token; the login comes from the GraphQL `viewer`.
- `rain` is Buienradar only (Netherlands and Belgium). Elsewhere the ring stays dry; a non-NL nowcast is out of scope.
- Rain intensity is shown by colour and alpha, not by segment length: `Drawable::Ring` keeps one length per ring so the segment cache stays as it is.
- Sun and moon maths live in `sources/astro.rs` (pure functions, `chrono` available there); `core` stays dependency-free and receives results as events.
- UPS and network need no config: their PromQL runs on every Prometheus poll and the roles show no-data when the metrics are absent.

## Config

New sections, all `#[serde(default)]`, added to `config.example.yaml`:

```yaml
location:
  lat: null                # decimal degrees; needed by weather, wind, aqi, rain, sun, moon, iss
  lon: null

weather:
  enabled: false           # Open-Meteo forecast + air quality, no key
  poll_secs: 600

rain:
  enabled: false           # Buienradar nowcast (NL/BE), no key
  poll_secs: 300

iss:
  enabled: false           # Celestrak TLE, passes computed on the Pi
  min_elevation: 10        # degrees above the horizon that count as a pass

github:
  enabled: false           # fine-grained token, read access to contributions, events and Actions
  token: ""
  poll_secs: 60

argocd:
  enabled: true            # watch applications.argoproj.io; no Argo CD, no deploys role
  namespace: argocd
```

Validation (`Config::validate`): `lat` in −90..90 and `lon` in −180..180 when set; both or neither; `weather`, `rain` or `iss` enabled without a location is an error; `iss.min_elevation` 0..90; `poll_secs` at least 60 for the public APIs; `github.enabled` with an empty token is allowed (the role shows the key icon, as electricity does).

Role identifiers: `gh-activity`, `weather`, `wind`, `aqi`, `rain`, `sun`, `moon`, `iss`, `ups`, `net`, `deploys`. `Role::ALL` grows to 21; `is_cluster()` is true for `ups`, `net` and `deploys`.

Workspace version becomes 0.4.0.

## Events

```rust
pub enum LinkTarget { K8sApi, Prometheus, QBittorrent, Electricity, Prices, Weather, Rain, Github, ArgoCd }

Event::Weather { temp_c: f32, code: u16, is_day: bool, wind_kmh: f32, gust_kmh: f32, wind_from_deg: f32, at: String }
Event::AirQuality { eaqi: f32 }
Event::Rain { from: i64 /* unix secs of slot 0 */, mm_per_h: Vec<f32> /* 24 slots of 5 min */ }
Event::Sky { sunrise: Option<i64>, sunset: Option<i64>, sun_elevation_deg: f32,
             moon_illumination: f32 /* 0..1 */, moon_waxing: bool, moon_phase: MoonPhase }
Event::IssPass(Option<IssPass>)   // None = no pass above min_elevation in the next 24 h
pub struct IssPass { start: i64, end: i64, max_elevation_deg: f32, visible: bool }

Event::GithubActivity { days: Vec<(String /* YYYY-MM-DD local */, u32)> }   // 30 entries, oldest first, last = today
Event::GithubPush { repo: String, commits: u32 }
Event::GithubStar { repo: String }
Event::GithubMerge { repo: String }
Event::GithubRelease { repo: String, tag: String }
Event::GithubRun { repo: String, ok: bool }

Event::Ups { on_battery: bool, low_battery: bool, charge_pct: f32, load_pct: f32, runtime_secs: u32 }
Event::UpsOnBattery
Event::UpsOnline
Event::Network { rx_bps: f64, tx_bps: f64 }

Event::Apps(Vec<(String, AppSync, AppHealth)>)   // sorted by name
Event::AppSynced { name: String }
Event::AppDegraded { name: String }
Event::AppHealthy { name: String }

pub enum MoonPhase { New, WaxingCrescent, FirstQuarter, WaxingGibbous, Full, WaningGibbous, LastQuarter, WaningCrescent }
pub enum AppSync { Synced, OutOfSync, Unknown }
pub enum AppHealth { Healthy, Progressing, Degraded, Suspended, Missing, Unknown }
```

Timestamps are unix seconds (i64) so `core` needs no time crate; `Model` already receives `now` from the run loop and gets the local hour and the UTC offset pushed in (`set_local_hour` gains a `set_utc_offset_secs` sibling for the sun dial).

## Sources

### Open-Meteo (`sources/open_meteo.rs`)

- `GET https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC` → `Event::Weather`.
- `GET https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=european_aqi` → `Event::AirQuality`. A failed air-quality call after a good forecast call still emits `Weather` and logs a warning; the link goes down only when the forecast call fails twice in a row.
- Every `weather.poll_secs` (minimum 60). Both through `http::client()`. Fixtures: `open-meteo-forecast.json`, `open-meteo-air-quality.json`.

### Buienradar (`sources/buienradar.rs`)

- `GET https://gpsgadget.buienradar.nl/data/raintext?lat={lat:.2}&lon={lon:.2}`: text lines `VVV|HH:MM`, 24 lines, five minutes apart, `VVV` 0..255. mm/h = `10^((v − 109) / 32)`, with `v = 0` meaning 0 mm/h. A slot counts as rain at 0.1 mm/h or more (v ≥ 77).
- `from` is the unix time of the first line, built from its `HH:MM` in the Pi's local zone (rolling over midnight when the first slot is earlier than the current clock minus an hour).
- Every `rain.poll_secs` (minimum 60). Fewer than 12 lines or unparsable lines are an error. Fixture: `buienradar-raintext.txt`.

### Astro (`sources/astro.rs`)

Pure functions, unit-tested against known dates, no network:

- `sun_times(lat, lon, day_utc) -> (Option<i64>, Option<i64>)` sunrise and sunset with the NOAA solar calculator at zenith 90.833°; `None` for polar day or night.
- `sun_elevation(lat, lon, t) -> f32` degrees.
- `moon(t) -> (illumination, waxing, MoonPhase)` from the synodic month (29.530588853 d) counted from the reference new moon 2000-01-06 18:14 UTC; illumination = (1 − cos(2π·age/synodic)) / 2; phase names at the usual eighths.
- `run_astro(location, ctx)` emits `Event::Sky` once a minute, sunrise and sunset for the current local day (tomorrow's sunrise once today's sunset has passed).

### ISS (`sources/iss.rs`)

- TLE: `GET https://celestrak.org/NORAD/elements/gp.php?CATNR=25544&FORMAT=TLE`, refreshed every 24 h, kept in memory between refreshes; a failed refresh keeps the old TLE for up to 7 days before the role goes no-data.
- Propagation with the `sgp4` crate at 10 s steps over the next 24 h: TEME position → ECEF via GMST → topocentric elevation from the observer. A pass is a run of steps above `iss.min_elevation`; the first pass whose end is in the future is emitted as `Event::IssPass(Some(..))`, or `None`.
- `visible` when, at the pass maximum, the observer's sun elevation is below −6° and the satellite is sunlit (its position has a positive component along the sun direction, or its distance from the Earth–sun axis exceeds the Earth radius).
- Recomputed every 10 minutes and right after a pass ends. No link target: the role shows no-data only when the TLE is stale.

### GitHub (`sources/github.rs`)

Header `Authorization: Bearer {token}`, `User-Agent` from `http::client()`.

- Calendar: `POST https://api.github.com/graphql` with `viewer { login contributionsCollection(from, to) { contributionCalendar { weeks { contributionDays { date contributionCount } } } } }`, `from` = 29 days before today at 00:00 local, `to` = tomorrow 00:00 local; every 300 s. Emits `Event::GithubActivity` with exactly 30 days (missing days as 0). The `login` is remembered for the events feed.
- Events: `GET https://api.github.com/users/{login}/events?per_page=30` with `If-None-Match` from the previous ETag; 304 means nothing new. Poll every `max(github.poll_secs, X-Poll-Interval)`. A tracker keyed by event id is primed on the first fetch so a restart does not replay old pushes. Mapped: `PushEvent` → `GithubPush` (size of `payload.commits`), `WatchEvent` action `started` → `GithubStar`, `PullRequestEvent` action `closed` with `payload.pull_request.merged` → `GithubMerge`, `ReleaseEvent` action `published` → `GithubRelease`.
- Actions: for at most 5 repos with a `PushEvent` in the last 24 h, `GET https://api.github.com/repos/{full_name}/actions/runs?per_page=5` on every events poll; a tracker keyed by run id emits `GithubRun` when a run first appears as `completed` (conclusion `success` → `ok: true`, `failure`/`timed_out`/`cancelled` → `ok: false`, others ignored), primed on first sight per repo.
- Budget: about 1 + 5 calls a minute plus 12 GraphQL calls an hour, far below 5000 an hour.
- 401 or 403 with a token message is `token rejected`: link down, warning logged once, retry after `poll_secs`; other failures follow the two-strike rule with `LinkTarget::Github`.
- Fixtures: `github-calendar.json`, `github-events.json`, `github-runs.json`.

### Argo CD (`sources/argocd.rs`)

- `Api::<DynamicObject>::namespaced(client, &cfg.namespace)` with `ApiResource::from_gvk(argoproj.io/v1alpha1, Application)`, `watcher(..).default_backoff()`, the same `select!` loop and `LinkEdge` as the pod watch, `LinkTarget::ArgoCd`.
- `AppTracker` (pure, tested) reads `status.sync.status`, `status.health.status` and `status.operationState.phase` from the object JSON. On every change it emits `Event::Apps` (sorted by name). Edges after the initial list: `phase` `Running` → `Succeeded` emits `AppSynced`; `health` entering `Degraded` or `Missing` emits `AppDegraded`; `health` returning to `Healthy` from either emits `AppHealthy`.
- If the CRD does not exist (404 on the first list) the source logs once at info, emits `Link { ArgoCd, false }` and retries every 10 minutes. `argocd.enabled: false` does not start it.

### Prometheus additions (`sources/prometheus.rs`)

- `Q_UPS_STATUS = nut_ups_status` (vector, label `status`, value 1 for set flags: `OL`, `OB`, `LB`, `CHRG`, ...), `Q_UPS_CHARGE = nut_battery_charge`, `Q_UPS_LOAD = nut_load`, `Q_UPS_RUNTIME = nut_battery_runtime_seconds`. Values at or below 1 are fractions, above 1 percentages (the two common nut exporters differ). Emits `Event::Ups` when `nut_ups_status` returns at least one series; `UpsTracker` (primed) emits `UpsOnBattery` on `OB` rising and `UpsOnline` on `OB` falling. Absent metrics: no event, role shows no-data.
- `Q_NET_RX = sum(rate(node_network_receive_bytes_total{device=~"eth.*|end.*|enp.*|eno.*|wlan.*"}[1m])) * 8`, `Q_NET_TX` likewise. Emits `Event::Network` every poll; missing series is no event.

### Fake source

Deterministic additions: weather cycling through clear, partly cloudy, rain and thunder every 30 ticks with the temperature drifting; a rain curve rotating one slot every 5 ticks; wind direction drifting with a gust every 20 ticks; AQI random walk; sky computed from the real clock at Amsterdam through `astro`; an ISS pass 42 minutes out, visible; UPS online at 100 % and 6 % load; network random walks between 2 and 400 Mbit/s; 16 apps named like the real cluster, all synced and healthy; a 30-day contribution curve with today at 28 and best 43.

New keys: `u` UPS outage toggle, `d` degrade or heal one app, `g` GitHub push, `r` GitHub star, `f` CI failure, `l` thunder, `w` rain in 10 minutes, `i` ISS pass now.

## Screens

Shared geometry unchanged: ring radius 102, 60 segments, icon 72 px at y 98, badge 64x26 at y 165, inner rings at radius 84 with `seg_count(84)` segments.

### `gh-activity`

- Outer ring: `today / max(best of last 30 days, 1)` in `#39d353`, last segment breathing.
- Inner ring: 7 sections (one per day, oldest first from 12 o'clock, one unlit gap between sections, partition as `power-mix`), colour by the day's count relative to the 30-day best: 0 unlit, up to 25 % `#0e4429`, 50 % `#006d32`, 75 % `#26a641`, above `#39d353`. Today's section breathes.
- Icon `github` (Lucide 0.263, the last version shipping brand icons, ISC). Badge today's count, stroke `#39d353`.
- Splashes on this role: `GithubPush` green `git-commit-horizontal` (the `+N` counter shows the commit count), `GithubStar` amber `star`, `GithubMerge` violet `git-merge`, `GithubRun` red `circle-x` or green `circle-check`. `GithubRelease` is a rack sweep, green, upwards, icon `tag`.
- No token: no-data ring with `key-round`; token rejected: same.

### `weather`

- Ring fill `(temp + 10) / 50` clamped, colour on the outdoor scale: 0 °C and below `#4f8dff`, 15 °C `#3ddc97`, 25 °C `#ffb020`, 35 °C and above `#ff4d4d`, linear between.
- Icon by WMO code: 0 `sun` (night: `moon`); 1–2 `cloud-sun` (night: `cloud-moon`); 3 `cloud`; 45, 48 `cloud-fog`; 51–57 `cloud-drizzle`; 61–67, 80–82 `cloud-rain`; 71–77, 85–86 `snowflake`; 95–99 `cloud-lightning`. Micro-loops: `sun` scale pulse 3.5 s, clouds bob 2.6 s, rain and drizzle bob 1.4 s, snow bob 3 s, fog alpha breathe 3 s, lightning alpha flicker 0.9 s.
- Badge `18°C`, stroke in the temperature colour.
- Splash: the code entering 95–99 splashes amber `cloud-lightning`.

### `wind`

- Compass: 12 o'clock is north, clockwise. An arc centred on the from-direction with half-width `3 + speed_kmh / 5` segments (at most 29), brightness fading from `#4f8dff` at the centre to `OFF` at the edge; the rest of the ring unlit. Every 6 s, for 0.8 s, the half-width uses the gust speed when the gust exceeds the speed by 30 %.
- Icon `wind`. Badge `23 km/h`, width 84. Below 3 km/h the ring is all dim and the badge says `calm`.

### `aqi`

- Ring fill `eaqi / 100` clamped, colour by EEA band: 0–20 `#50F0E6`, 20–40 `#50CCAA`, 40–60 `#F0E641`, 60–80 `#FF5050`, 80–100 `#960032`, above `#7D2181`.
- Icon `haze` in the band colour. Badge `AQI 32`, width 84. Splash amber `haze` when the band index rises.

### `rain`

- 48 segments at 7.5° pitch, two per slot, slot 0 (now) at 12 o'clock, clockwise. Dry slots unlit; below 1 mm/h `#4f8dff` at 50 % alpha; 1–5 mm/h `#4f8dff` to `#a78bfa` linear; above 5 mm/h `#a78bfa`. The current slot breathes; slots already in the past (when the poll is older than a slot) are dropped from the front so slot 0 is always the current five minutes.
- Icon `cloud-rain`, blue when any slot is wet, grey when the whole two hours are dry. Badge: `DRY` (grey) when nothing is coming, `25 min` (blue) until the first wet slot, `2.3 mm` (blue) when it is raining now.
- Splash: blue `umbrella` when the first wet slot moves inside 15 minutes while it is dry now.

### `sun`

- 60 segments as 24 hours (2.5 segments an hour), midnight at 12 o'clock, local time. Segments between sunrise and sunset `#ffb020`; night `#1b2a4a`; the 30 minutes either side of sunrise and sunset mixed. The segment holding the current time is `#fff2b0` and breathes.
- Icon `sunset` by day, `sunrise` by night; badge the countdown, `-5h48` or `-42m`, amber. Polar day or night: the ring is all one colour and the badge shows `--`.

### `moon`

- Ring fill = illumination, colour `#e8e8f0`; waxing lights clockwise from the top, waning is drawn from the top counter-clockwise (`start_deg` −6·k) so the direction hints at the phase.
- Icon `moon`. Badge alternates every 5 s between `63%` and the phase (`new`, `waxing`, `first q`, `full`, `last q`, `waning`), width 84.

### `iss`

- Before a pass: ring fill = `min(seconds to start, 12 h) / 12 h`, so it empties as the pass approaches; violet `#a78bfa`, at 40 % alpha when the pass is not visible. Badge the countdown `-42m` or `-3h10`.
- During a pass: whole ring lit and breathing, badge the maximum elevation `62°`. A visible pass starting fires a rack sweep, violet, upwards, icon `satellite`, hold 1.5 s.
- No pass in 24 h: ring unlit, badge `--`. Icon `satellite`.

### `ups`

- Outer ring charge: green above 50 %, amber above 20 %, red at or below or when `low_battery`. Inner ring load in amber. Badge runtime `42 min` or `1h12`, stroke green online, red on battery.
- Icon `battery-charging` online, `battery-warning` on battery (alpha breathe). `UpsOnBattery` is a rack sweep, red, downwards, icon `battery-warning`, hold 1.5 s; `UpsOnline` a green sweep upwards with `battery-charging`.

### `net`

- Outer ring download, inner ring upload. Fill = `log10(bps / 1e5) / 4` clamped to 0.03..1 (100 kbit/s empty, 1 Gbit/s full). Within the lit part a 3-segment bright pulse travels clockwise on the outer ring and counter-clockwise on the inner one, one lap every `6 / fill` seconds (min 6 s, max 60 s), the rest of the lit segments at 45 % alpha. Outer `#4f8dff`, inner `#a78bfa`.
- Icon `arrow-down-up`. Badge `41 Mb` (`kb` below 1 Mbit/s, `Gb` at 1000 and above), stroke blue.

### `deploys`

- One section per app in name order, gaps when 20 apps or fewer (as `storage`): green synced and healthy; amber (breathing) out of sync, progressing or with an operation running; red degraded or missing; grey suspended or unknown.
- Icon `rocket`. Badge `15/16` (apps that are synced and healthy over total), stroke green when equal, amber otherwise.
- Splashes: `AppSynced` green `rocket`, `AppDegraded` red `triangle-alert` with a persistent red marker while any app is degraded, `AppHealthy` green `circle-check`.

### No-data variants

Sky roles (`weather`, `wind`, `aqi`, `rain`, `sun`, `moon`, `iss`) without a location: no-data ring with `map-pin`. `gh-activity` without a token: `key-round`. Everything else: `cloud-off` while the link is down or the first event has not arrived.

## Rendering

- New Lucide icons: `github` (from 0.263), `git-commit-horizontal`, `git-merge`, `star`, `circle-x`, `tag`, `sun`, `moon`, `cloud-sun`, `cloud-moon`, `cloud-fog`, `cloud-drizzle`, `cloud-rain`, `cloud-lightning`, `snowflake`, `wind`, `haze`, `umbrella`, `sunset`, `sunrise`, `satellite`, `battery-charging`, `battery-warning`, `arrow-down-up`, `rocket`, `triangle-alert`, `map-pin`, added to `assets/icons` and the `icons!` list.
- No primitive changes: dimming (the non-visible ISS ring, the rain intensity classes, the net pulse) uses the per-segment alpha already in `SegState::On`, and wide badges use the existing `w` override.
- `SweepKind` gains `GithubRelease`, `UpsOnBattery`, `UpsOnline`, `IssPass`; `SplashKind` gains the GitHub, weather, rain, AQI and Argo CD kinds above. `FxRequest` gains one variant per new splash or sweep.

## Model

- `LinkState` gains `weather`, `rain`, `github`, `argocd`.
- New state: `WeatherState`, `AirState`, `RainState`, `SkyState`, `IssState`, `GithubState` (30 days, best, today), `UpsState`, `NetState` (two `Smooth` on the log fill and two phase accumulators for the pulses), `AppsState`, each with `have`.
- `Model::apply` folds the events; edge events push `FxRequest`s. `needs_data` gets an arm per role; `wants_connecting` covers `ups`, `net`, `deploys`.
- The rain scene drops elapsed slots using `now` and `from`; the ISS and sun scenes use `now` and the UTC offset.

## TUI

### Configure

New fields: `location lat`, `location lon` (Number, blank allowed), `weather enabled`, `weather poll secs`, `rain enabled`, `rain poll secs`, `iss enabled`, `iss min elevation`, `github enabled`, `github token` (Secret), `github poll secs`, `argocd enabled`, `argocd namespace`. `FieldKind::Choice` becomes `Choice(&'static [&'static str])` so `next_choice` is no longer wired to the price sources.

### Screens

Fourth preset `s` sky: `[weather, aqi] [rain, wind] [sun, moon] [iss, gh-activity]` at 15 s. Role hints: sky roles without a location `!`, `gh-activity` without a token `!`, `deploys` with `argocd.enabled: false` `!`.

### Status

Four more dots: `weather`, `rain`, `github`, `argocd`, from journal lines starting `weather:`, `rain:`, `github:`, `argocd:` (info up, warn or error down), same rule as `electricity:`.

## Docs and tooling

- README: the screens table gains the 11 GIFs, the roles table the 11 rows, a "Sky roles" section (location, Open-Meteo, Buienradar, ISS) and a "GitHub role" section (token scopes: read-only on contents, metadata and Actions for the runs call); the `all.gif` caption and `examples/gifs.rs` stop saying ten roles (`ALL_SECS` derived from `Role::ALL.len()`).
- GIF generator scripts: `ups` outage at 3 s, `gh-activity` push at 2 s and CI failure at 5 s, `deploys` degrade at 3 s, `iss` pass start at 4 s, `rain` umbrella at 3 s.
- `sgp4` is the only new dependency (sources crate).

## Testing

- core: role parsing for the 11 names and `ALL` length; each new scene's ring counts and colours at fixed inputs (gh levels, outdoor temperature scale, EAQI bands, rain intensity classes and slot dropping, sun dial segments for a known sunrise and sunset, moon direction, ISS fill and dim alpha, UPS colour thresholds, net log fill and pulse position, deploys sections and badge); fx routing for every new splash and sweep; `needs_data` per role; the no-data icon selection.
- sources: parse fixtures for Open-Meteo (both endpoints), Buienradar (dry, wet, short file), GitHub calendar, events and runs (tracker priming, ETag 304, merged detection); `astro` against known sunrise, sunset and moon values (Amsterdam 2026-06-21 and 2026-12-21, a polar case); ISS pass finder against a fixture TLE and a fixed time (a pass exists, elevation monotone, visibility flags); `UpsTracker` and `AppTracker` edges; nut fraction versus percent normalisation.
- render: golden images for the 11 idle scenes plus the UPS on-battery ring and the ISS pass ring; icon loader test covers the 27 new SVGs.
- setup: Configure round-trip for the new fields, generalised choice cycling, the `s` preset, `links_from_logs` for the four new dots.
- app: config validation cases (location bounds, sky role without location, github without token allowed), `Default` still parses `config.example.yaml`.
- Manual: simulator with all keys, then the Pi with real location, token and cluster.

## Out of scope

- Non-NL rain nowcasts, hourly temperature dial, historic charts, per-repo GitHub screens, GitHub inbox and stars roles, Home Assistant, Pi-hole, Traefik, uptime and clock roles (all in the catalog, not picked).

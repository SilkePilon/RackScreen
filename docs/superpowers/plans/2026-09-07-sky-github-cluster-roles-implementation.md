# Sky, GitHub and more cluster roles (v0.4.0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eleven new screen roles: `gh-activity`, `weather`, `wind`, `aqi`, `rain`, `sun`, `moon`, `iss`, `ups`, `net` and `deploys`, each with its own ring, icon, badge, splashes and data source, selectable in the Screens editor and configurable in the setup TUI.

**Architecture:** `Role` grows from 10 to 21 variants. Every role gets a scene function in `core` fed by new `Event`s folded into new state structs on `Model`; edge events push `FxRequest`s that become splashes or rack sweeps. Data comes from one source module per API in the `sources` crate (`open_meteo`, `buienradar`, `astro`, `iss`, `github`, `argocd`), each a copy of the `electricity.rs` poll loop (backoff, two-strike link edge, pure parse functions tested against fixtures), plus four PromQL queries and two trackers in the existing Prometheus poller. `Model` learns the wall clock (`unix_now`, `utc_offset_secs`) from the run loop because the sun dial, rain ring and ISS countdown are clock-based. The setup TUI gains Configure fields, a Sky preset, role hints and four Status dots.

**Tech Stack:** existing workspace (Rust 2021, tiny-skia, usvg, ratatui 0.30, reqwest 0.13 rustls, kube 4.2, chrono 0.4) plus `sgp4 = "2"` in the sources crate for ISS propagation.

Spec: `docs/superpowers/specs/2026-09-07-sky-github-cluster-roles-design.md`

## Global Constraints

- Role ids and `Role::ALL` order after `renewable`: `gh-activity`, `weather`, `wind`, `aqi`, `rain`, `sun`, `moon`, `iss`, `ups`, `net`, `deploys` (21 roles). `is_cluster()` is true for `ups`, `net`, `deploys` (and the six existing cluster roles). Sky roles (`weather`, `wind`, `aqi`, `rain`, `sun`, `moon`, `iss`) show the no-data ring with `map-pin` when no location is set; `gh-activity` shows `key-round` when no token is set.
- Shared geometry unchanged: ring radius 102, 60 segments, icon 72 px at y 98, badge 64x26 at y 165, inner rings at radius 84 with `seg_count(84.0)` (= 49) segments, marker dot at y 188.
- Colours: GitHub greens `#0e4429`, `#006d32`, `#26a641`, `#39d353`. Outdoor temperature scale: 0 °C and below `#4f8dff`, 15 °C `#3ddc97`, 25 °C `#ffb020`, 35 °C and above `#ff4d4d`, linear between. EAQI bands 0–20 `#50F0E6`, 20–40 `#50CCAA`, 40–60 `#F0E641`, 60–80 `#FF5050`, 80–100 `#960032`, above `#7D2181`. Rain: below 1 mm/h `#4f8dff` at 50 % alpha, 1–5 mm/h `#4f8dff` to `#a78bfa`, above 5 `#a78bfa`. Sun dial: day `#ffb020`, night `#1b2a4a`, now `#fff2b0`. Moon `#e8e8f0`. ISS `#a78bfa`, 40 % alpha when not visible. Net outer `#4f8dff`, inner `#a78bfa`. UPS charge green above 50 %, amber above 20 %, red at or below or on low battery; load inner ring amber.
- Net fill = `log10(bps / 1e5) / 4` clamped to 0.03..1; a 3-segment bright pulse laps the lit part every `6 / fill` seconds (6..60 s), the rest at 45 % alpha; outer clockwise, inner counter-clockwise.
- Config (all `#[serde(default)]`): `location { lat: null, lon: null }`, `weather { enabled: false, poll_secs: 600 }`, `rain { enabled: false, poll_secs: 300 }`, `iss { enabled: false, min_elevation: 10 }`, `github { enabled: false, token: "", poll_secs: 60 }`, `argocd { enabled: true, namespace: argocd }`. Validation: lat −90..90, lon −180..180, both or neither; `weather`, `rain` or `iss` enabled without a location is an error; `min_elevation` 0..90; public-API `poll_secs` at least 60; `github.enabled` with an empty token is allowed.
- APIs: Open-Meteo `https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC` and `https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=european_aqi`; Buienradar `https://gpsgadget.buienradar.nl/data/raintext?lat={lat:.2}&lon={lon:.2}` (`VVV|HH:MM` lines, mm/h = `10^((v-109)/32)`, 0 means 0, wet at 0.1 mm/h = v ≥ 77); Celestrak `https://celestrak.org/NORAD/elements/gp.php?CATNR=25544&FORMAT=TLE`; GitHub `https://api.github.com/graphql`, `https://api.github.com/users/{login}/events?per_page=30`, `https://api.github.com/repos/{full_name}/actions/runs?per_page=5`; Argo CD `applications.argoproj.io/v1alpha1`.
- Splash icons and colours: `GithubPush` green `git-commit-horizontal`, `GithubStar` amber `star`, `GithubMerge` violet `git-merge`, `GithubRunFailed` red `circle-x`, `GithubRunPassed` green `circle-check`, `Thunder` amber `cloud-lightning`, `RainSoon` blue `umbrella`, `AirWorse` amber `haze`, `AppSynced` green `rocket`, `AppDegraded` red `triangle-alert`, `AppHealthy` green `circle-check`. Sweeps: `GithubRelease` green up `tag`; `UpsOnBattery` red down `battery-warning` hold 1.5 s; `UpsOnline` green up `battery-charging` hold 1.5 s; `IssPass` violet up `satellite` hold 1.5 s.
- New Lucide icons (all from `lucide-static@0.544.0` except `github` from `lucide-static@0.263.0`): `github`, `git-commit-horizontal`, `git-merge`, `star`, `circle-x`, `tag`, `sun`, `moon`, `cloud-sun`, `cloud-moon`, `cloud-fog`, `cloud-drizzle`, `cloud-rain`, `cloud-lightning`, `snowflake`, `wind`, `haze`, `umbrella`, `sunset`, `sunrise`, `satellite`, `battery-charging`, `battery-warning`, `arrow-down-up`, `rocket`, `map-pin` (26 files; `triangle-alert` already exists).
- Fake source keys: `u` UPS outage toggle, `d` degrade or heal one app, `g` GitHub push, `r` GitHub star, `f` CI failure, `l` thunder, `w` rain in 10 minutes, `i` ISS pass now.
- Screens editor preset `w` sky: `[weather, aqi] [rain, wind] [sun, moon] [iss, gh-activity]` at 15 s.
- All existing tests keep passing; `cargo clippy --workspace --features sim,pi -- -D warnings`, `--no-default-features --features pi` and `--features sim` all clean; `cargo fmt --all` before every commit; commit bodies end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- `core` stays dependency-free: all clock and astronomy maths that need `chrono` live in `sources`; `core` receives unix seconds as `i64`.

## File map

```
crates/core/src/theme.rs            Role (21 variants), outdoor_color, eaqi_band/eaqi_color, GitHub greens
crates/core/src/event.rs            new Event variants, LinkTarget additions, MoonPhase, AppSync, AppHealth, IssPass
crates/core/src/format.rs           fmt_until, fmt_runtime, fmt_mbit
crates/core/src/model.rs            LinkState fields, new state structs, folds, clock setters, net pulse phases, ISS sweep
crates/core/src/fx.rs               new SplashKind / SweepKind arms and FxRequest routing
crates/core/src/scene_fx.rs         needs_data arms, no-data icon selection
crates/core/src/scene.rs            role_scene arms
crates/core/src/scene_ups.rs        ups (new)
crates/core/src/scene_net.rs        net (new)
crates/core/src/scene_deploys.rs    deploys (new)
crates/core/src/scene_github.rs     gh-activity (new)
crates/core/src/scene_weather.rs    weather, wind, aqi (new)
crates/core/src/scene_rain.rs       rain (new)
crates/core/src/scene_sky.rs        sun, moon, iss (new)
crates/render/src/assets.rs         26 new icons
assets/icons/*.svg                  the icon files
crates/sources/src/prometheus.rs    UPS + network queries, UpsTracker
crates/sources/src/argocd.rs        Application watcher (new)
crates/sources/src/open_meteo.rs    forecast + air quality poller (new)
crates/sources/src/buienradar.rs    raintext poller (new)
crates/sources/src/astro.rs         sun/moon maths + run_astro (new)
crates/sources/src/iss.rs           TLE fetch, SGP4 pass finder + run_iss (new)
crates/sources/src/github.rs        calendar, events, runs poller (new)
crates/sources/src/fake.rs          new data + keys
crates/sources/tests/fixtures/      open-meteo-forecast.json, open-meteo-air-quality.json, buienradar-raintext.txt, iss.tle, github-calendar.json, github-events.json, github-runs.json, argocd-application.json
crates/display/src/sim.rs           keys u d g r f l w i
crates/app/src/config.rs            location, weather, rain, iss, github, argocd sections + validation
crates/app/src/run.rs               spawn_external_sources, argocd in spawn_k8s_sources, flags into RenderLoop
crates/app/src/runloop.rs           clock setters per frame, location/token flags, clippy fix
crates/app/examples/gifs.rs         clips for the new roles, fixed clock
crates/setup/src/screens/configure.rs   13 new fields
crates/setup/src/screens/screens.rs     sky preset key, role hints
crates/setup/src/ops/config_file.rs     Preset::Sky
crates/setup/src/ops/systemd.rs         LinkDots + links_from_logs
crates/setup/src/screens/status.rs      four more dots
crates/render/tests/golden.rs           goldens for the new scenes
config.example.yaml, README.md, Cargo.toml (0.4.0)
```

---

### Task 1: CI clippy fix, 26 icons, roles and events

Everything later builds on the 21-variant `Role`, the new events and the icons, so they land first. The new `role_scene` arms point at the no-data scene until each scene task replaces them; `needs_data` returns `true` for the new roles for the same reason.

**Files:**
- Modify: `crates/app/src/runloop.rs:79`
- Create: `assets/icons/{github,git-commit-horizontal,git-merge,star,circle-x,tag,sun,moon,cloud-sun,cloud-moon,cloud-fog,cloud-drizzle,cloud-rain,cloud-lightning,snowflake,wind,haze,umbrella,sunset,sunrise,satellite,battery-charging,battery-warning,arrow-down-up,rocket,map-pin}.svg`
- Modify: `crates/render/src/assets.rs:17-56`
- Modify: `crates/core/src/theme.rs` (Role enum, `ALL`, `name`, `is_cluster`, `accent`, `icon`, new colour helpers)
- Modify: `crates/core/src/event.rs` (LinkTarget, Event, new enums)
- Modify: `crates/core/src/model.rs` (LinkState fields, Link fold)
- Modify: `crates/core/src/scene.rs:437-440` (role_scene arms)
- Modify: `crates/core/src/scene_fx.rs:156-171` (needs_data arms)
- Modify: `crates/core/src/fx.rs` (nothing yet; `Fx::default` already sizes by `Role::ALL.len()`)
- Modify: `crates/sources/src/fake.rs:245-260` and `crates/render/tests/golden.rs` only if they enumerate `LinkTarget` (they do not).

**Interfaces:**
- Produces: `Role::{GhActivity, Weather, Wind, Aqi, Rain, Sun, Moon, Iss, Ups, Net, Deploys}`, `Role::is_sky()`, `theme::{GH_GREENS, outdoor_color, eaqi_band, eaqi_color}`, `LinkTarget::{Weather, Rain, Github, ArgoCd}`, `Event::{Weather, AirQuality, Rain, Sky, IssPass, GithubActivity, GithubPush, GithubStar, GithubMerge, GithubRelease, GithubRun, Ups, UpsOnBattery, UpsOnline, Network, Apps, AppSynced, AppDegraded, AppHealthy}`, `event::{MoonPhase, AppSync, AppHealth, IssPass}`, `LinkState::{weather, rain, github, argocd}`.

- [ ] **Step 1: Fix the clippy error CI fails on**

`crates/app/src/runloop.rs:79`, replace the loop body of `darken`:

```rust
pub fn darken(px: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    for p in px.data_mut().as_chunks_mut::<4>().0 {
        p[0] = (p[0] as f32 * k) as u8;
        p[1] = (p[1] as f32 * k) as u8;
        p[2] = (p[2] as f32 * k) as u8;
    }
}
```

Run: `cargo clippy --workspace --features sim,pi -- -D warnings`
Expected: clean (the last three CI runs on `main` failed on `using chunks_exact_mut with a constant chunk size`; the local toolchain may not raise it, CI's stable does).

- [ ] **Step 2: Download the 26 icons**

```bash
cd assets/icons
for i in git-commit-horizontal git-merge star circle-x tag sun moon cloud-sun cloud-moon cloud-fog cloud-drizzle cloud-rain cloud-lightning snowflake wind haze umbrella sunset sunrise satellite battery-charging battery-warning arrow-down-up rocket map-pin; do
  curl -sfL -o "$i.svg" "https://unpkg.com/lucide-static@0.544.0/icons/$i.svg" || echo "MISSING $i"
done
curl -sfL -o github.svg "https://unpkg.com/lucide-static@0.263.0/icons/github.svg" || echo "MISSING github"
grep -L "<svg" *.svg
```

Expected: no `MISSING` lines and `grep -L` prints nothing (every file is an SVG). Lucide is ISC, already covered by `assets/icons/LICENSE`.

- [ ] **Step 3: Register the icons**

`crates/render/src/assets.rs`, inside the `icons!(` list after `"zap",`:

```rust
    "github",
    "git-commit-horizontal",
    "git-merge",
    "star",
    "circle-x",
    "tag",
    "sun",
    "moon",
    "cloud-sun",
    "cloud-moon",
    "cloud-fog",
    "cloud-drizzle",
    "cloud-rain",
    "cloud-lightning",
    "snowflake",
    "wind",
    "haze",
    "umbrella",
    "sunset",
    "sunrise",
    "satellite",
    "battery-charging",
    "battery-warning",
    "arrow-down-up",
    "rocket",
    "map-pin",
```

Run: `cargo test -p rackscreen-render assets`
Expected: the existing "every icon parses with viewBox 8/16/24" test passes with 64 icons.

- [ ] **Step 4: Write the failing role tests**

`crates/core/src/theme.rs`, in `mod tests`, replace `cluster_roles_are_the_six_kubernetes_ones` and add:

```rust
    #[test]
    fn cluster_roles_are_the_kubernetes_ones() {
        let cluster: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_cluster()).collect();
        assert_eq!(
            cluster,
            vec![
                Role::Cpu,
                Role::Mem,
                Role::Pods,
                Role::Health,
                Role::Thermal,
                Role::Storage,
                Role::Ups,
                Role::Net,
                Role::Deploys,
            ]
        );
        assert!(!Role::PowerMix.is_cluster() && !Role::Price.is_cluster());
        assert!(!Role::Weather.is_cluster() && !Role::GhActivity.is_cluster());
    }

    #[test]
    fn twenty_one_roles_with_unique_names_and_icons() {
        assert_eq!(Role::ALL.len(), 21);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 21);
        assert_eq!(Role::parse("gh-activity"), Some(Role::GhActivity));
        assert_eq!(Role::parse("deploys"), Some(Role::Deploys));
        assert_eq!(Role::Iss.icon(), "satellite");
        assert_eq!(Role::Ups.icon(), "battery-charging");
        let sky: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_sky()).collect();
        assert_eq!(
            sky,
            vec![Role::Weather, Role::Wind, Role::Aqi, Role::Rain, Role::Sun, Role::Moon, Role::Iss]
        );
    }

    #[test]
    fn outdoor_and_air_quality_scales() {
        assert_eq!(outdoor_color(-5.0), BLUE);
        assert_eq!(outdoor_color(0.0), BLUE);
        assert_eq!(outdoor_color(15.0), GREEN);
        assert_eq!(outdoor_color(25.0), AMBER);
        assert_eq!(outdoor_color(35.0), RED);
        assert_eq!(outdoor_color(40.0), RED);
        assert_eq!(outdoor_color(20.0), GREEN.mix(AMBER, 0.5));
        assert_eq!(eaqi_band(0.0), 0);
        assert_eq!(eaqi_band(19.9), 0);
        assert_eq!(eaqi_band(20.0), 1);
        assert_eq!(eaqi_band(45.0), 2);
        assert_eq!(eaqi_band(79.0), 3);
        assert_eq!(eaqi_band(99.0), 4);
        assert_eq!(eaqi_band(150.0), 5);
        assert_eq!(eaqi_color(32.0), Color::hex(0x50CCAA));
        assert_eq!(eaqi_color(500.0), Color::hex(0x7D2181));
    }
```

- [ ] **Step 5: Run the tests to see them fail**

Run: `cargo test -p rackscreen-core theme`
Expected: compile errors, `Role::Ups` and `outdoor_color` do not exist.

- [ ] **Step 6: Extend `Role` and add the colour helpers**

`crates/core/src/theme.rs`. After `price_color`:

```rust
/// GitHub contribution calendar greens, lightest level first.
pub const GH_GREENS: [Color; 4] = [
    Color::hex(0x0e4429),
    Color::hex(0x006d32),
    Color::hex(0x26a641),
    Color::hex(0x39d353),
];

/// Outdoor temperature: 0 °C and below blue, 15 green, 25 amber, 35 and above red.
pub fn outdoor_color(c: f32) -> Color {
    if c <= 15.0 {
        BLUE.mix(GREEN, (c / 15.0).clamp(0.0, 1.0))
    } else if c <= 25.0 {
        GREEN.mix(AMBER, (c - 15.0) / 10.0)
    } else {
        AMBER.mix(RED, ((c - 25.0) / 10.0).clamp(0.0, 1.0))
    }
}

/// European Air Quality Index band, 0 (good) to 5 (extremely poor).
pub fn eaqi_band(v: f32) -> usize {
    match v {
        v if v < 20.0 => 0,
        v if v < 40.0 => 1,
        v if v < 60.0 => 2,
        v if v < 80.0 => 3,
        v if v < 100.0 => 4,
        _ => 5,
    }
}

/// The EEA colour for an EAQI value.
pub fn eaqi_color(v: f32) -> Color {
    const BANDS: [u32; 6] = [0x50F0E6, 0x50CCAA, 0xF0E641, 0xFF5050, 0x960032, 0x7D2181];
    Color::hex(BANDS[eaqi_band(v)])
}
```

Replace the `Role` enum and its `impl` up to and including `icon`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Cpu,
    Mem,
    Pods,
    Health,
    Thermal,
    Storage,
    PowerMix,
    Price,
    Carbon,
    Renewable,
    GhActivity,
    Weather,
    Wind,
    Aqi,
    Rain,
    Sun,
    Moon,
    Iss,
    Ups,
    Net,
    Deploys,
}

impl Role {
    pub const ALL: [Role; 21] = [
        Role::Cpu,
        Role::Mem,
        Role::Pods,
        Role::Health,
        Role::Thermal,
        Role::Storage,
        Role::PowerMix,
        Role::Price,
        Role::Carbon,
        Role::Renewable,
        Role::GhActivity,
        Role::Weather,
        Role::Wind,
        Role::Aqi,
        Role::Rain,
        Role::Sun,
        Role::Moon,
        Role::Iss,
        Role::Ups,
        Role::Net,
        Role::Deploys,
    ];

    pub fn index(self) -> usize {
        Role::ALL
            .iter()
            .position(|r| *r == self)
            .expect("role in ALL")
    }
    pub fn from_index(i: usize) -> Option<Role> {
        Role::ALL.get(i).copied()
    }
    /// Config identifier.
    pub fn name(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "mem",
            Role::Pods => "pods",
            Role::Health => "health",
            Role::Thermal => "thermal",
            Role::Storage => "storage",
            Role::PowerMix => "power-mix",
            Role::Price => "price",
            Role::Carbon => "carbon",
            Role::Renewable => "renewable",
            Role::GhActivity => "gh-activity",
            Role::Weather => "weather",
            Role::Wind => "wind",
            Role::Aqi => "aqi",
            Role::Rain => "rain",
            Role::Sun => "sun",
            Role::Moon => "moon",
            Role::Iss => "iss",
            Role::Ups => "ups",
            Role::Net => "net",
            Role::Deploys => "deploys",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|r| r.name() == s)
    }
    /// Cluster roles need the Kubernetes API link; the electricity, sky and
    /// GitHub roles are fed by public APIs and work without a cluster.
    pub fn is_cluster(self) -> bool {
        matches!(
            self,
            Role::Cpu
                | Role::Mem
                | Role::Pods
                | Role::Health
                | Role::Thermal
                | Role::Storage
                | Role::Ups
                | Role::Net
                | Role::Deploys
        )
    }
    /// Roles that need `location.lat` / `location.lon` in the config.
    pub fn is_sky(self) -> bool {
        matches!(
            self,
            Role::Weather | Role::Wind | Role::Aqi | Role::Rain | Role::Sun | Role::Moon | Role::Iss
        )
    }
    pub fn accent(self) -> Color {
        match self {
            Role::Cpu => AMBER,
            Role::Mem => VIOLET,
            Role::Pods => BLUE,
            Role::Health => GREEN,
            Role::Thermal => AMBER,
            Role::Storage => VIOLET,
            Role::PowerMix => Color::hex(0xFFC700),
            Role::Price => GREEN,
            Role::Carbon => Color::hex(0x2AA364),
            Role::Renewable => GREEN,
            Role::GhActivity => GH_GREENS[3],
            Role::Weather => AMBER,
            Role::Wind => BLUE,
            Role::Aqi => Color::hex(0x50CCAA),
            Role::Rain => BLUE,
            Role::Sun => AMBER,
            Role::Moon => Color::hex(0xe8e8f0),
            Role::Iss => VIOLET,
            Role::Ups => GREEN,
            Role::Net => BLUE,
            Role::Deploys => GREEN,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "memory-stick",
            Role::Pods => "box",
            Role::Health => "heart-pulse",
            Role::Thermal => "thermometer",
            Role::Storage => "database",
            Role::PowerMix => "zap",
            Role::Price => "euro",
            Role::Carbon => "cloud",
            Role::Renewable => "leaf",
            Role::GhActivity => "github",
            Role::Weather => "cloud-sun",
            Role::Wind => "wind",
            Role::Aqi => "haze",
            Role::Rain => "cloud-rain",
            Role::Sun => "sunset",
            Role::Moon => "moon",
            Role::Iss => "satellite",
            Role::Ups => "battery-charging",
            Role::Net => "arrow-down-up",
            Role::Deploys => "rocket",
        }
    }
}
```

- [ ] **Step 7: Add the events**

`crates/core/src/event.rs`. Replace `LinkTarget`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    K8sApi,
    Prometheus,
    QBittorrent,
    Electricity,
    Prices,
    Weather,
    Rain,
    Github,
    ArgoCd,
}
```

After `Robustness`'s impl add:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoonPhase {
    New,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    Full,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl MoonPhase {
    /// Short badge text.
    pub fn label(self) -> &'static str {
        match self {
            MoonPhase::New => "new",
            MoonPhase::WaxingCrescent | MoonPhase::WaxingGibbous => "waxing",
            MoonPhase::FirstQuarter => "first q",
            MoonPhase::Full => "full",
            MoonPhase::WaningGibbous | MoonPhase::WaningCrescent => "waning",
            MoonPhase::LastQuarter => "last q",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppSync {
    Synced,
    OutOfSync,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppHealth {
    Healthy,
    Progressing,
    Degraded,
    Suspended,
    Missing,
    Unknown,
}

/// One Argo CD application: name, sync state, health, and whether an operation is running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub sync: AppSync,
    pub health: AppHealth,
    pub operating: bool,
}

/// The next ISS pass over the observer, unix seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IssPass {
    pub start: i64,
    pub end: i64,
    pub max_elevation_deg: f32,
    pub visible: bool,
}
```

In `Event`, before `Torrents(Vec<Torrent>)` add:

```rust
    /// Current conditions from Open-Meteo.
    Weather {
        temp_c: f32,
        /// WMO weather interpretation code.
        code: u16,
        is_day: bool,
        wind_kmh: f32,
        gust_kmh: f32,
        /// Direction the wind comes from, degrees clockwise from north.
        wind_from_deg: f32,
        at: String,
    },
    AirQuality {
        eaqi: f32,
    },
    /// Buienradar nowcast: 24 five-minute slots from `from`.
    Rain {
        from: i64,
        mm_per_h: Vec<f32>,
    },
    /// Sun and moon, computed on the Pi once a minute.
    Sky {
        sunrise: Option<i64>,
        sunset: Option<i64>,
        sun_elevation_deg: f32,
        /// 0..1
        moon_illumination: f32,
        moon_waxing: bool,
        moon_phase: MoonPhase,
    },
    /// `None` when no pass clears `iss.min_elevation` in the next 24 h.
    IssPass(Option<IssPass>),
    /// Thirty days of contribution counts, oldest first, last entry today.
    GithubActivity {
        days: Vec<(String, u32)>,
    },
    GithubPush {
        repo: String,
        commits: u32,
    },
    GithubStar {
        repo: String,
    },
    GithubMerge {
        repo: String,
    },
    GithubRelease {
        repo: String,
        tag: String,
    },
    GithubRun {
        repo: String,
        ok: bool,
    },
    Ups {
        on_battery: bool,
        low_battery: bool,
        charge_pct: f32,
        load_pct: f32,
        runtime_secs: u32,
    },
    UpsOnBattery,
    UpsOnline,
    Network {
        rx_bps: f64,
        tx_bps: f64,
    },
    /// Every Argo CD application, sorted by name.
    Apps(Vec<App>),
    AppSynced {
        name: String,
    },
    AppDegraded {
        name: String,
    },
    AppHealthy {
        name: String,
    },
```

Note: the spec wrote `Apps(Vec<(String, AppSync, AppHealth)>)`; the tuple grew a fourth field (an operation running colours the arc amber), so it is a named struct `App`.

- [ ] **Step 8: Link state and the folds that keep it compiling**

`crates/core/src/model.rs`. `LinkState`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkState {
    pub api: bool,
    pub prom: bool,
    pub qbit: bool,
    pub electricity: bool,
    pub prices: bool,
    pub weather: bool,
    pub rain: bool,
    pub github: bool,
    pub argocd: bool,
}
```

In `Model::new` the `link:` initialiser gains `weather: false, rain: false, github: false, argocd: false,`. In `apply`, the `Event::Link` match gains:

```rust
                LinkTarget::Weather => self.link.weather = up,
                LinkTarget::Rain => self.link.rain = up,
                LinkTarget::Github => self.link.github = up,
                LinkTarget::ArgoCd => self.link.argocd = up,
```

and, at the end of the `match ev` in `apply`, a temporary catch-all that the scene tasks remove one by one:

```rust
            Event::Weather { .. }
            | Event::AirQuality { .. }
            | Event::Rain { .. }
            | Event::Sky { .. }
            | Event::IssPass(_)
            | Event::GithubActivity { .. }
            | Event::GithubPush { .. }
            | Event::GithubStar { .. }
            | Event::GithubMerge { .. }
            | Event::GithubRelease { .. }
            | Event::GithubRun { .. }
            | Event::Ups { .. }
            | Event::UpsOnBattery
            | Event::UpsOnline
            | Event::Network { .. }
            | Event::Apps(_)
            | Event::AppSynced { .. }
            | Event::AppDegraded { .. }
            | Event::AppHealthy { .. } => {}
```

`crates/core/src/scene.rs`, `role_scene`, after the `Role::Renewable` arm:

```rust
        Role::GhActivity
        | Role::Weather
        | Role::Wind
        | Role::Aqi
        | Role::Rain
        | Role::Sun
        | Role::Moon
        | Role::Iss
        | Role::Ups
        | Role::Net
        | Role::Deploys => no_data_scene(now),
```

`crates/core/src/scene_fx.rs`, `needs_data`, after the `Role::Price` arm:

```rust
            Role::GhActivity
            | Role::Weather
            | Role::Wind
            | Role::Aqi
            | Role::Rain
            | Role::Sun
            | Role::Moon
            | Role::Iss
            | Role::Ups
            | Role::Net
            | Role::Deploys => true,
```

- [ ] **Step 9: Run the whole workspace**

Run: `cargo test --workspace --features sim,pi && cargo clippy --workspace --features sim,pi -- -D warnings`
Expected: all green. If `crates/setup` or `crates/app` has an exhaustive `match` on `Role` or `LinkTarget` the compiler names it; add the same catch-all arm there.

- [ ] **Step 10: Commit**

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): eleven new roles, their events and icons

Role grows to 21 variants with names, accents and icons; LinkTarget and
Event gain the weather, rain, sky, ISS, GitHub, UPS, network and Argo CD
variants the new sources will emit. The new roles show the no-data ring
until their scenes land. Also fixes the chunks_exact_mut clippy error CI
fails on.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Clock in the model, formatting helpers, fx kinds

The sun dial, rain ring and ISS countdown need the wall clock; every new splash and sweep needs a kind. This task adds the plumbing all scene tasks share.

**Files:**
- Modify: `crates/core/src/model.rs` (clock fields + setters, FxRequest variants)
- Modify: `crates/core/src/fx.rs` (SplashKind, SweepKind, Fx::apply)
- Modify: `crates/core/src/format.rs`

**Interfaces:**
- Produces: `Model::{set_unix_now, unix_now, set_utc_offset_secs, utc_offset_secs, set_location_present, location_present, set_github_token_present, github_token_present}`; `FxRequest::{GithubPush, GithubStar, GithubMerge, GithubRelease, GithubRunFailed, GithubRunPassed, Thunder, RainSoon, AirWorse, AppSynced, AppDegraded, AppHealthy, UpsOnBattery, UpsOnline, IssPass}`; `SplashKind::{GithubPush, GithubStar, GithubMerge, GithubRunFailed, GithubRunPassed, Thunder, RainSoon, AirWorse, AppSynced, AppDegraded, AppHealthy}`; `SweepKind::{GithubRelease, UpsOnBattery, UpsOnline, IssPass}`; `format::{fmt_until, fmt_runtime, fmt_mbit}`.

- [ ] **Step 1: Failing tests for the formatting helpers**

`crates/core/src/format.rs`, in `mod tests`:

```rust
    #[test]
    fn until_runtime_and_mbit() {
        assert_eq!(fmt_until(42 * 60), "-42m");
        assert_eq!(fmt_until(5 * 3600 + 48 * 60), "-5h48");
        assert_eq!(fmt_until(30), "-1m");
        assert_eq!(fmt_until(-5), "--");
        assert_eq!(fmt_runtime(42 * 60), "42 min");
        assert_eq!(fmt_runtime(72 * 60), "1h12");
        assert_eq!(fmt_runtime(30), "0 min");
        assert_eq!(fmt_mbit(41_300_000.0), "41 Mb");
        assert_eq!(fmt_mbit(512_000.0), "512 kb");
        assert_eq!(fmt_mbit(2_400_000_000.0), "2.4 Gb");
        assert_eq!(fmt_mbit(0.0), "0 kb");
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-core format`
Expected: compile error, `fmt_until` not found.

- [ ] **Step 3: Implement**

`crates/core/src/format.rs`, after `fmt_eta`:

```rust
/// Seconds until something, as a countdown: `-42m`, `-5h48`. Past or unknown is `--`.
pub fn fmt_until(secs: i64) -> String {
    if secs < 0 {
        return "--".into();
    }
    let mins = (secs + 59) / 60;
    if mins < 60 {
        format!("-{}m", mins.max(1))
    } else {
        format!("-{}h{:02}", mins / 60, mins % 60)
    }
}

/// UPS runtime: `42 min` under an hour, `1h12` above.
pub fn fmt_runtime(secs: u32) -> String {
    let mins = secs / 60;
    if mins < 60 {
        format!("{mins} min")
    } else {
        format!("{}h{:02}", mins / 60, mins % 60)
    }
}

/// Bits per second as `512 kb`, `41 Mb` or `2.4 Gb`.
pub fn fmt_mbit(bps: f64) -> String {
    let b = bps.max(0.0);
    if b >= 1e9 {
        format!("{:.1} Gb", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} Mb", b / 1e6)
    } else {
        format!("{:.0} kb", b / 1e3)
    }
}
```

- [ ] **Step 4: Failing test for the clock and flags on the model**

`crates/core/src/model.rs`, in `mod tests`:

```rust
    #[test]
    fn clock_and_presence_flags() {
        let mut m = Model::new(Thresholds::default());
        assert_eq!(m.unix_now(), 0);
        assert_eq!(m.utc_offset_secs(), 0);
        assert!(!m.location_present() && !m.github_token_present());
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        m.set_location_present(true);
        m.set_github_token_present(true);
        assert_eq!(m.unix_now(), 1_788_782_400);
        assert_eq!(m.utc_offset_secs(), 7200);
        assert!(m.location_present() && m.github_token_present());
    }
```

- [ ] **Step 5: Add the fields, setters and the new `FxRequest` variants**

`crates/core/src/model.rs`. In `pub struct Model`, after `local_hour: u32,`:

```rust
    /// Unix seconds, set by the render loop every frame; the sun dial, rain
    /// ring and ISS countdown are clock-based.
    unix_now: i64,
    /// Seconds east of UTC for the Pi's local zone.
    utc_offset_secs: i32,
    /// `location.lat`/`lon` are set; without them the sky roles show a pin.
    location_present: bool,
    /// A GitHub token is set; without one `gh-activity` shows a key.
    github_token_present: bool,
```

In `Model::new`, after `local_hour: 12,`:

```rust
            unix_now: 0,
            utc_offset_secs: 0,
            location_present: false,
            github_token_present: false,
```

After `local_hour()`:

```rust
    pub fn set_unix_now(&mut self, secs: i64) {
        self.unix_now = secs;
    }
    pub fn unix_now(&self) -> i64 {
        self.unix_now
    }
    pub fn set_utc_offset_secs(&mut self, secs: i32) {
        self.utc_offset_secs = secs;
    }
    pub fn utc_offset_secs(&self) -> i32 {
        self.utc_offset_secs
    }
    pub fn set_location_present(&mut self, present: bool) {
        self.location_present = present;
    }
    pub fn location_present(&self) -> bool {
        self.location_present
    }
    pub fn set_github_token_present(&mut self, present: bool) {
        self.github_token_present = present;
    }
    pub fn github_token_present(&self) -> bool {
        self.github_token_present
    }
```

`FxRequest` gains, after `Boot`:

```rust
    GithubPush,
    GithubStar,
    GithubMerge,
    GithubRelease,
    GithubRunFailed,
    GithubRunPassed,
    Thunder,
    RainSoon,
    AirWorse,
    AppSynced,
    AppDegraded,
    AppHealthy,
    UpsOnBattery,
    UpsOnline,
    IssPass,
```

- [ ] **Step 6: Failing fx tests**

`crates/core/src/fx.rs`, in `mod tests` (create the module if the file has none; it has one, append):

```rust
    #[test]
    fn new_splash_kinds_route_to_their_roles() {
        let mut fx = Fx::default();
        fx.apply(FxRequest::GithubPush, 0.0);
        fx.apply(FxRequest::Thunder, 0.0);
        fx.apply(FxRequest::RainSoon, 0.0);
        fx.apply(FxRequest::AirWorse, 0.0);
        fx.apply(FxRequest::AppDegraded, 0.0);
        assert_eq!(
            fx.splashes[Role::GhActivity.index()].active().map(|s| s.kind),
            Some(SplashKind::GithubPush)
        );
        assert_eq!(
            fx.splashes[Role::Weather.index()].active().map(|s| s.kind),
            Some(SplashKind::Thunder)
        );
        assert_eq!(
            fx.splashes[Role::Rain.index()].active().map(|s| s.kind),
            Some(SplashKind::RainSoon)
        );
        assert_eq!(
            fx.splashes[Role::Aqi.index()].active().map(|s| s.kind),
            Some(SplashKind::AirWorse)
        );
        assert_eq!(
            fx.splashes[Role::Deploys.index()].active().map(|s| s.kind),
            Some(SplashKind::AppDegraded)
        );
        assert_eq!(SplashKind::GithubPush.icon(), "git-commit-horizontal");
        assert_eq!(SplashKind::GithubRunFailed.color(), RED);
        assert_eq!(SplashKind::AppSynced.icon(), "rocket");
    }

    #[test]
    fn new_sweep_kinds_have_direction_colour_icon_and_hold() {
        let mut fx = Fx::default();
        fx.apply(FxRequest::UpsOnBattery, 0.0);
        fx.tick(0.0, 4);
        assert_eq!(fx.sweeps.active().map(|s| s.kind), Some(SweepKind::UpsOnBattery));
        assert_eq!(SweepKind::UpsOnBattery.direction(), Direction::Down);
        assert_eq!(SweepKind::UpsOnBattery.color(Role::Cpu), RED);
        assert_eq!(SweepKind::UpsOnBattery.icon(Role::Cpu), "battery-warning");
        assert_eq!(SweepKind::UpsOnBattery.hold(), 1.5);
        assert_eq!(SweepKind::UpsOnline.direction(), Direction::Up);
        assert_eq!(SweepKind::UpsOnline.color(Role::Cpu), GREEN);
        assert_eq!(SweepKind::GithubRelease.icon(Role::Cpu), "tag");
        assert_eq!(SweepKind::GithubRelease.hold(), SWEEP_HOLD_SECS);
        assert_eq!(SweepKind::IssPass.color(Role::Cpu), crate::theme::VIOLET);
        assert_eq!(SweepKind::IssPass.hold(), 1.5);
    }
```

- [ ] **Step 7: Run to see them fail**

Run: `cargo test -p rackscreen-core fx`
Expected: compile errors on the new variants.

- [ ] **Step 8: Add the kinds**

`crates/core/src/fx.rs`. Add `pub const SHORT_HOLD_SECS: Secs = 1.5;` after `BOOT_HOLD_SECS`. Import `VIOLET` in the `use crate::theme::...` line. `SplashKind`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplashKind {
    PodStarted,
    PodCrashed,
    PodGone,
    HotNode,
    HotTemp,
    VolumeDegraded,
    VolumeHealthy,
    TorrentAdded,
    GithubPush,
    GithubStar,
    GithubMerge,
    GithubRunFailed,
    GithubRunPassed,
    Thunder,
    RainSoon,
    AirWorse,
    AppSynced,
    AppDegraded,
    AppHealthy,
}

impl SplashKind {
    pub fn icon(self) -> &'static str {
        match self {
            SplashKind::PodStarted => "package-plus",
            SplashKind::PodCrashed => "package-x",
            SplashKind::PodGone => "package-minus",
            SplashKind::HotNode => "flame",
            SplashKind::HotTemp => "flame",
            SplashKind::VolumeDegraded => "database-zap",
            SplashKind::VolumeHealthy => "database",
            SplashKind::TorrentAdded => "download",
            SplashKind::GithubPush => "git-commit-horizontal",
            SplashKind::GithubStar => "star",
            SplashKind::GithubMerge => "git-merge",
            SplashKind::GithubRunFailed => "circle-x",
            SplashKind::GithubRunPassed => "circle-check",
            SplashKind::Thunder => "cloud-lightning",
            SplashKind::RainSoon => "umbrella",
            SplashKind::AirWorse => "haze",
            SplashKind::AppSynced => "rocket",
            SplashKind::AppDegraded => "triangle-alert",
            SplashKind::AppHealthy => "circle-check",
        }
    }
    pub fn color(self) -> Color {
        match self {
            SplashKind::PodStarted => BLUE,
            SplashKind::PodCrashed => RED,
            SplashKind::PodGone => BLUE.with_alpha(0.6),
            SplashKind::HotNode => AMBER,
            SplashKind::HotTemp => RED,
            SplashKind::VolumeDegraded => AMBER,
            SplashKind::VolumeHealthy => GREEN,
            SplashKind::TorrentAdded => BLUE,
            SplashKind::GithubPush => GREEN,
            SplashKind::GithubStar => AMBER,
            SplashKind::GithubMerge => VIOLET,
            SplashKind::GithubRunFailed => RED,
            SplashKind::GithubRunPassed => GREEN,
            SplashKind::Thunder => AMBER,
            SplashKind::RainSoon => BLUE,
            SplashKind::AirWorse => AMBER,
            SplashKind::AppSynced => GREEN,
            SplashKind::AppDegraded => RED,
            SplashKind::AppHealthy => GREEN,
        }
    }
}
```

`SweepKind` and its impl:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SweepKind {
    TorrentDone,
    NodeNotReady,
    NodeReady,
    AlertFiring,
    AlertResolved,
    LinkUp,
    Boot,
    GithubRelease,
    UpsOnBattery,
    UpsOnline,
    IssPass,
}

impl SweepKind {
    /// Bad news runs bottom to top (as the README says); good news and the
    /// mains outage run top to bottom.
    pub fn direction(self) -> Direction {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring => Direction::Up,
            SweepKind::UpsOnBattery => Direction::Down,
            SweepKind::GithubRelease | SweepKind::UpsOnline | SweepKind::IssPass => Direction::Up,
            _ => Direction::Down,
        }
    }
    pub fn color(self, role: Role) -> Color {
        match self {
            SweepKind::NodeNotReady | SweepKind::AlertFiring | SweepKind::UpsOnBattery => RED,
            SweepKind::Boot => role.accent(),
            SweepKind::IssPass => VIOLET,
            _ => GREEN,
        }
    }
    pub fn icon(self, role: Role) -> &'static str {
        match self {
            SweepKind::TorrentDone => "circle-check",
            SweepKind::NodeNotReady => "server-off",
            SweepKind::NodeReady => "server",
            SweepKind::AlertFiring => "triangle-alert",
            SweepKind::AlertResolved => "shield-check",
            SweepKind::LinkUp => "plug",
            SweepKind::Boot => role.icon(),
            SweepKind::GithubRelease => "tag",
            SweepKind::UpsOnBattery => "battery-warning",
            SweepKind::UpsOnline => "battery-charging",
            SweepKind::IssPass => "satellite",
        }
    }
    pub fn hold(self) -> Secs {
        match self {
            SweepKind::Boot => BOOT_HOLD_SECS,
            SweepKind::UpsOnBattery | SweepKind::UpsOnline | SweepKind::IssPass => SHORT_HOLD_SECS,
            _ => SWEEP_HOLD_SECS,
        }
    }
}
```

`Fx::apply`, after `FxRequest::Boot => ...`:

```rust
            FxRequest::GithubPush => splash(SplashKind::GithubPush, Role::GhActivity),
            FxRequest::GithubStar => splash(SplashKind::GithubStar, Role::GhActivity),
            FxRequest::GithubMerge => splash(SplashKind::GithubMerge, Role::GhActivity),
            FxRequest::GithubRunFailed => splash(SplashKind::GithubRunFailed, Role::GhActivity),
            FxRequest::GithubRunPassed => splash(SplashKind::GithubRunPassed, Role::GhActivity),
            FxRequest::Thunder => splash(SplashKind::Thunder, Role::Weather),
            FxRequest::RainSoon => splash(SplashKind::RainSoon, Role::Rain),
            FxRequest::AirWorse => splash(SplashKind::AirWorse, Role::Aqi),
            FxRequest::AppSynced => splash(SplashKind::AppSynced, Role::Deploys),
            FxRequest::AppDegraded => splash(SplashKind::AppDegraded, Role::Deploys),
            FxRequest::AppHealthy => splash(SplashKind::AppHealthy, Role::Deploys),
            FxRequest::GithubRelease => self.sweeps.push(SweepKind::GithubRelease),
            FxRequest::UpsOnBattery => self.sweeps.push(SweepKind::UpsOnBattery),
            FxRequest::UpsOnline => self.sweeps.push(SweepKind::UpsOnline),
            FxRequest::IssPass => self.sweeps.push(SweepKind::IssPass),
```

- [ ] **Step 9: Run tests, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`
Expected: green.

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): wall clock on the model, new splash and sweep kinds, countdown formats

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: `ups` and `net` roles in core

**Files:**
- Modify: `crates/core/src/model.rs` (UpsState, NetState, smooths, pulse phases, folds, accessors)
- Create: `crates/core/src/scene_ups.rs`, `crates/core/src/scene_net.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs` (role_scene arms + `badge_w`), `crates/core/src/scene_fx.rs` (needs_data arms)

**Interfaces:**
- Consumes: `Event::{Ups, UpsOnBattery, UpsOnline, Network}`, `FxRequest::{UpsOnBattery, UpsOnline}`, `format::{fmt_runtime, fmt_mbit}`.
- Produces: `Model::{ups() -> &UpsState, net() -> &NetState, smooth_ups_charge, smooth_ups_load, smooth_net_rx, smooth_net_tx, net_phases() -> (f32, f32)}`, `model::net_fill(bps: f64) -> f32`, `scene::badge_w(cy, stroke, text, w) -> Drawable`, `scene_ups::ups_scene`, `scene_net::{net_scene, flow_states}`.

- [ ] **Step 1: Failing model tests**

`crates/core/src/model.rs`, `mod tests`:

```rust
    #[test]
    fn ups_and_network_fold_into_state_and_smooths() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 6.0,
                runtime_secs: 3014,
            },
            0.0,
        );
        assert!(m.ups().have);
        assert_eq!(m.ups().runtime_secs, 3014);
        assert_eq!(m.smooth_ups_charge(5.0), 100.0);
        assert_eq!(m.smooth_ups_load(5.0), 6.0);
        m.apply(Event::UpsOnBattery, 1.0);
        assert_eq!(m.pending_fx(), &[FxRequest::UpsOnBattery]);
        m.apply(Event::UpsOnline, 2.0);
        assert_eq!(m.pending_fx().last(), Some(&FxRequest::UpsOnline));

        m.apply(
            Event::Network {
                rx_bps: 1e7,
                tx_bps: 1e5,
            },
            0.0,
        );
        assert!(m.net().have);
        assert!((m.smooth_net_rx(5.0) - 0.5).abs() < 1e-6, "10 Mbit is half the log scale");
        assert!((m.smooth_net_tx(5.0) - 0.03).abs() < 1e-6, "idle floor");
        // the pulse phase advances with fill: one lap per 6 s at full fill
        m.tick(10.0);
        let (a, _) = m.net_phases();
        m.tick(13.0);
        let (b, _) = m.net_phases();
        assert!(((b - a).rem_euclid(1.0) - 0.25).abs() < 0.01, "half fill, 3 s = quarter lap");
    }

    #[test]
    fn net_fill_is_a_clamped_log_scale() {
        assert_eq!(net_fill(0.0), 0.03);
        assert_eq!(net_fill(1e5), 0.03);
        assert!((net_fill(1e7) - 0.5).abs() < 1e-6);
        assert_eq!(net_fill(1e9), 1.0);
        assert_eq!(net_fill(5e9), 1.0);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-core model::tests::ups`
Expected: compile error, `Model::ups` not found.

- [ ] **Step 3: State, smooths and folds**

`crates/core/src/model.rs`. After `PriceState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpsState {
    pub on_battery: bool,
    pub low_battery: bool,
    pub charge_pct: f32,
    pub load_pct: f32,
    pub runtime_secs: u32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetState {
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub have: bool,
}

/// Ring fill for a bit rate: 100 kbit/s is (almost) empty, 1 Gbit/s is full,
/// four decades in between, and a floor so idle still shows a little.
pub fn net_fill(bps: f64) -> f32 {
    if bps <= 0.0 {
        return 0.03;
    }
    (((bps / 1e5).log10() / 4.0) as f32).clamp(0.03, 1.0)
}

/// Laps per second of the net pulse at full fill (one lap every 6 s).
const NET_LAPS_PER_SEC: f64 = 1.0 / 6.0;
```

Fields on `Model`, after `prices: PriceState,`:

```rust
    ups: UpsState,
    net: NetState,
    ups_charge: Smooth,
    ups_load: Smooth,
    /// Ring fill (0..1) for download and upload, eased.
    net_rx: Smooth,
    net_tx: Smooth,
    /// Position of the travelling pulse on each net ring, in laps (fractional).
    net_rx_phase: f64,
    net_tx_phase: f64,
    net_last_tick: Option<Secs>,
```

In `Model::new`, after `prices: PriceState::default(),`:

```rust
            ups: UpsState::default(),
            net: NetState::default(),
            ups_charge: Smooth::new(0.0, SMOOTH_SECS),
            ups_load: Smooth::new(0.0, SMOOTH_SECS),
            net_rx: Smooth::new(0.03, SMOOTH_SECS),
            net_tx: Smooth::new(0.03, SMOOTH_SECS),
            net_rx_phase: 0.0,
            net_tx_phase: 0.0,
            net_last_tick: None,
```

Accessors, after `prices()`:

```rust
    pub fn ups(&self) -> &UpsState {
        &self.ups
    }
    pub fn net(&self) -> &NetState {
        &self.net
    }
    pub fn smooth_ups_charge(&self, now: Secs) -> f32 {
        self.ups_charge.value(now)
    }
    pub fn smooth_ups_load(&self, now: Secs) -> f32 {
        self.ups_load.value(now)
    }
    pub fn smooth_net_rx(&self, now: Secs) -> f32 {
        self.net_rx.value(now)
    }
    pub fn smooth_net_tx(&self, now: Secs) -> f32 {
        self.net_tx.value(now)
    }
    /// Pulse positions (download, upload) in laps, 0..1.
    pub fn net_phases(&self) -> (f32, f32) {
        (self.net_rx_phase as f32, self.net_tx_phase as f32)
    }
```

In `tick`, right after `self.track_mix_leader(now);`:

```rust
        let dt = self.net_last_tick.map_or(0.0, |l| (now - l).max(0.0));
        self.net_last_tick = Some(now);
        self.net_rx_phase =
            (self.net_rx_phase + dt * self.net_rx.value(now) as f64 * NET_LAPS_PER_SEC).fract();
        self.net_tx_phase =
            (self.net_tx_phase + dt * self.net_tx.value(now) as f64 * NET_LAPS_PER_SEC).fract();
```

In `apply`, remove `Event::Ups { .. } | Event::UpsOnBattery | Event::UpsOnline | Event::Network { .. }` from the catch-all and add:

```rust
            Event::Ups {
                on_battery,
                low_battery,
                charge_pct,
                load_pct,
                runtime_secs,
            } => {
                self.ups_charge.set(charge_pct, now);
                self.ups_load.set(load_pct, now);
                self.ups = UpsState {
                    on_battery,
                    low_battery,
                    charge_pct,
                    load_pct,
                    runtime_secs,
                    have: true,
                };
            }
            Event::UpsOnBattery => self.fx.push(FxRequest::UpsOnBattery),
            Event::UpsOnline => self.fx.push(FxRequest::UpsOnline),
            Event::Network { rx_bps, tx_bps } => {
                self.net_rx.set(net_fill(rx_bps), now);
                self.net_tx.set(net_fill(tx_bps), now);
                self.net = NetState {
                    rx_bps,
                    tx_bps,
                    have: true,
                };
            }
```

- [ ] **Step 4: Run the model tests**

Run: `cargo test -p rackscreen-core model::tests`
Expected: PASS.

- [ ] **Step 5: `badge_w` helper**

`crates/core/src/scene.rs`, after `badge`:

```rust
/// The standard badge with another width (for `23 km/h`, `AQI 32`, phase names).
pub fn badge_w(cy: f32, stroke: Color, text: String, w: f32) -> Drawable {
    let mut b = badge(cy, stroke, text);
    if let Drawable::Badge { w: bw, .. } = &mut b {
        *bw = w;
    }
    b
}
```

- [ ] **Step 6: The UPS scene with its tests**

Create `crates/core/src/scene_ups.rs`:

```rust
//! UPS role: battery charge outside, load inside, runtime in the badge.

use crate::anim::{breathe, Secs};
use crate::format::fmt_runtime;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREEN, RED, WHITE};

/// Charge ring colour: green above 50 %, amber above 20 %, red at or below or on low battery.
pub fn charge_color(charge_pct: f32, low_battery: bool) -> Color {
    if low_battery || charge_pct <= 20.0 {
        RED
    } else if charge_pct <= 50.0 {
        AMBER
    } else {
        GREEN
    }
}

pub fn ups_scene(model: &Model, now: Secs) -> Scene {
    let u = model.ups();
    let charge = model.smooth_ups_charge(now);
    let load = model.smooth_ups_load(now);
    let color = charge_color(charge, u.low_battery);
    let mut states = ring_states(charge, color, SEG_N, now);
    if u.low_battery {
        let a = breathe(now, 1.2);
        for st in states.iter_mut() {
            if let SegState::On(c, _) = *st {
                *st = SegState::On(c, a);
            }
        }
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(ring(84.0, ring_states(load, AMBER, seg_count(84.0), now)));
    let (icon, icon_color, alpha) = if u.on_battery {
        ("battery-warning", RED, breathe(now, 1.6))
    } else {
        ("battery-charging", WHITE, 1.0)
    };
    s.push(icon_at(icon, 92.0, 60.0, icon_color, alpha));
    let stroke = if u.on_battery { RED } else { GREEN };
    s.push(badge(BADGE_CY, stroke, fmt_runtime(u.runtime_secs)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    fn model(on_battery: bool, low: bool, charge: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Ups {
                on_battery,
                low_battery: low,
                charge_pct: charge,
                load_pct: 6.0,
                runtime_secs: 42 * 60,
            },
            0.0,
        );
        m
    }

    fn badge_text(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .expect("badge")
    }

    fn icon(s: &Scene) -> &'static str {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .expect("icon")
    }

    #[test]
    fn charge_colour_thresholds() {
        assert_eq!(charge_color(100.0, false), GREEN);
        assert_eq!(charge_color(50.0, false), AMBER);
        assert_eq!(charge_color(20.0, false), RED);
        assert_eq!(charge_color(90.0, true), RED);
    }

    #[test]
    fn online_is_a_full_green_ring_with_load_inside() {
        let s = ups_scene(&model(false, false, 100.0), 5.0);
        assert_eq!(s.lit_count(), 60);
        let radii: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii, vec![RING_R, 84.0]);
        assert_eq!(icon(&s), "battery-charging");
        assert_eq!(badge_text(&s), ("42 min".into(), GREEN));
    }

    #[test]
    fn on_battery_swaps_icon_and_reddens_the_badge() {
        let s = ups_scene(&model(true, false, 80.0), 5.0);
        assert_eq!(icon(&s), "battery-warning");
        assert_eq!(badge_text(&s).1, RED);
        let low = ups_scene(&model(true, true, 15.0), 0.3);
        // low battery: every lit segment breathes together
        let alphas: Vec<f32> = low
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(
                    states
                        .iter()
                        .filter_map(|s| match s {
                            SegState::On(_, a) => Some(*a),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap();
        assert!(alphas.iter().all(|a| (a - alphas[0]).abs() < 1e-6 && *a < 1.0));
    }
}
```

- [ ] **Step 7: The network scene with its tests**

Create `crates/core/src/scene_net.rs`:

```rust
//! Network role: download outside, upload inside, a pulse crawling with the flow.

use crate::anim::Secs;
use crate::format::fmt_mbit;
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, BLUE, VIOLET, WHITE};

/// Lit segments for `fill`, with a three-segment bright pulse at `phase` laps
/// travelling clockwise (or the other way), the rest of the lit part dimmed.
pub fn flow_states(fill: f32, phase: f32, n: usize, color: Color, clockwise: bool) -> Vec<SegState> {
    let lit = ((fill.clamp(0.0, 1.0) * n as f32).round() as usize).clamp(1, n);
    let head = ((phase.rem_euclid(1.0) * lit as f32).floor() as usize).min(lit - 1);
    (0..n)
        .map(|i| {
            if i >= lit {
                return SegState::Off;
            }
            let pos = if clockwise { i } else { lit - 1 - i };
            let behind = (pos + lit - head) % lit;
            let alpha = match behind {
                0 => 1.0,
                1 => 0.85,
                2 => 0.7,
                _ => 0.45,
            };
            SegState::On(color, alpha)
        })
        .collect()
}

pub fn net_scene(model: &Model, now: Secs) -> Scene {
    let n = model.net();
    let (rx, tx) = (model.smooth_net_rx(now), model.smooth_net_tx(now));
    let (prx, ptx) = model.net_phases();
    let mut s = Scene::new();
    s.push(ring(RING_R, flow_states(rx, prx, SEG_N, BLUE, true)));
    s.push(ring(
        84.0,
        flow_states(tx, ptx, seg_count(84.0), VIOLET, false),
    ));
    s.push(icon_at("arrow-down-up", 92.0, 60.0, WHITE, 1.0));
    s.push(badge(BADGE_CY, BLUE, fmt_mbit(n.rx_bps + n.tx_bps)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    #[test]
    fn pulse_sits_at_the_head_and_trails_behind() {
        let v = flow_states(0.5, 0.0, 60, BLUE, true);
        assert_eq!(v.iter().filter(|s| matches!(s, SegState::On(..))).count(), 30);
        assert!(matches!(v[0], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[1], SegState::On(_, a) if a == 0.85));
        assert!(matches!(v[2], SegState::On(_, a) if a == 0.7));
        assert!(matches!(v[3], SegState::On(_, a) if a == 0.45));
        assert!(matches!(v[29], SegState::On(_, a) if a == 0.45));
        assert_eq!(v[30], SegState::Off);
        // a third of a lap in: head at segment 10
        let v = flow_states(0.5, 1.0 / 3.0, 60, BLUE, true);
        assert!(matches!(v[10], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[9], SegState::On(_, a) if a == 0.45));
        // counter-clockwise: the head starts at the last lit segment
        let v = flow_states(0.5, 0.0, 60, VIOLET, false);
        assert!(matches!(v[29], SegState::On(_, a) if a == 1.0));
        assert!(matches!(v[28], SegState::On(_, a) if a == 0.85));
        // the floor still lights something
        assert_eq!(
            flow_states(0.0, 0.0, 60, BLUE, true)
                .iter()
                .filter(|s| matches!(s, SegState::On(..)))
                .count(),
            1
        );
    }

    #[test]
    fn scene_has_two_rings_and_a_total_badge() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Network {
                rx_bps: 40e6,
                tx_bps: 1.3e6,
            },
            0.0,
        );
        let s = net_scene(&m, 5.0);
        let radii: Vec<f32> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ring { radius, .. } => Some(*radius),
                _ => None,
            })
            .collect();
        assert_eq!(radii, vec![RING_R, 84.0]);
        let text = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "41 Mb");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "arrow-down-up", .. })));
    }
}
```

- [ ] **Step 8: Wire the scenes in**

`crates/core/src/lib.rs`: add `pub mod scene_net;` and `pub mod scene_ups;` (alphabetical). `crates/core/src/scene.rs` `role_scene`: remove `Role::Ups | Role::Net` from the catch-all and add:

```rust
        Role::Ups => crate::scene_ups::ups_scene(model, now),
        Role::Net => crate::scene_net::net_scene(model, now),
```

`crates/core/src/scene_fx.rs` `needs_data`: remove them from the `=> true` arm and add:

```rust
            Role::Ups => !(link.prom && self.ups().have),
            Role::Net => !(link.prom && self.net().have),
```

- [ ] **Step 9: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`
Expected: green.

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): ups and net roles

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: `deploys` role in core

**Files:**
- Modify: `crates/core/src/model.rs` (AppsState, fold, accessor)
- Create: `crates/core/src/scene_deploys.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`

**Interfaces:**
- Consumes: `Event::{Apps, AppSynced, AppDegraded, AppHealthy}`, `event::{App, AppSync, AppHealth}`, `FxRequest::{AppSynced, AppDegraded, AppHealthy}`.
- Produces: `Model::apps() -> &AppsState`, `scene_deploys::{deploys_scene, app_states, app_ok}`.

- [ ] **Step 1: Failing model test**

`crates/core/src/model.rs`, `mod tests`:

```rust
    #[test]
    fn apps_fold_and_edge_events_splash() {
        use crate::event::{App, AppHealth, AppSync};
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Apps(vec![App {
                name: "argocd".into(),
                sync: AppSync::Synced,
                health: AppHealth::Healthy,
                operating: false,
            }]),
            0.0,
        );
        assert!(m.apps().have);
        assert_eq!(m.apps().apps.len(), 1);
        m.apply(Event::AppDegraded { name: "x".into() }, 1.0);
        m.apply(Event::AppSynced { name: "x".into() }, 1.0);
        m.apply(Event::AppHealthy { name: "x".into() }, 1.0);
        assert_eq!(
            m.pending_fx(),
            &[FxRequest::AppDegraded, FxRequest::AppSynced, FxRequest::AppHealthy]
        );
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p rackscreen-core apps_fold`
Expected: compile error, `Model::apps` not found.

- [ ] **Step 3: State and fold**

`crates/core/src/model.rs`. Add `use crate::event::App;` to the imports (extend the existing `use crate::event::{...}` line). After `NetState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppsState {
    pub apps: Vec<App>,
    pub have: bool,
}
```

Field `apps: AppsState,` after `net: NetState,`; initialiser `apps: AppsState::default(),`; accessor:

```rust
    pub fn apps(&self) -> &AppsState {
        &self.apps
    }
```

In `apply`, remove the four `App*` variants from the catch-all and add:

```rust
            Event::Apps(list) => {
                self.apps = AppsState {
                    apps: list,
                    have: true,
                };
            }
            Event::AppSynced { .. } => self.fx.push(FxRequest::AppSynced),
            Event::AppDegraded { .. } => self.fx.push(FxRequest::AppDegraded),
            Event::AppHealthy { .. } => self.fx.push(FxRequest::AppHealthy),
```

- [ ] **Step 4: The scene with its tests**

Create `crates/core/src/scene_deploys.rs`:

```rust
//! Deploys role: one arc per Argo CD application, coloured by sync and health.

use crate::anim::{breathe, Secs};
use crate::event::{App, AppHealth, AppSync};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREEN, GREY, RED, WHITE};

/// Synced, healthy and idle: the calm state.
pub fn app_ok(a: &App) -> bool {
    a.health == AppHealth::Healthy && a.sync == AppSync::Synced && !a.operating
}

/// Arc colour and whether it breathes.
fn app_color(a: &App) -> (Color, bool) {
    if matches!(a.health, AppHealth::Degraded | AppHealth::Missing) {
        (RED, false)
    } else if matches!(a.health, AppHealth::Suspended | AppHealth::Unknown)
        || a.sync == AppSync::Unknown
    {
        (GREY, false)
    } else if a.health == AppHealth::Progressing || a.sync == AppSync::OutOfSync || a.operating {
        (AMBER, true)
    } else {
        (GREEN, false)
    }
}

/// Equal sections in name order, an unlit gap between them when there is room.
pub fn app_states(apps: &[App], now: Secs) -> Vec<SegState> {
    let n = SEG_N;
    if apps.is_empty() {
        return vec![SegState::Off; n];
    }
    let count = apps.len();
    let per = (n / count).max(1);
    let gap = if per >= 3 { 1 } else { 0 };
    let mut out = vec![SegState::Off; n];
    for (k, app) in apps.iter().enumerate().take(n) {
        let start = k * per;
        let end = if k == count - 1 { n } else { (start + per).min(n) };
        let lit_end = end.saturating_sub(gap);
        let (color, breathing) = app_color(app);
        let a = if breathing { breathe(now, 1.2) } else { 1.0 };
        for st in out.iter_mut().take(lit_end).skip(start) {
            *st = SegState::On(color, a);
        }
    }
    out
}

pub fn deploys_scene(model: &Model, now: Secs) -> Scene {
    let apps = &model.apps().apps;
    let ok = apps.iter().filter(|a| app_ok(a)).count();
    let degraded = apps
        .iter()
        .any(|a| matches!(a.health, AppHealth::Degraded | AppHealth::Missing));
    let mut s = Scene::new();
    s.push(ring(RING_R, app_states(apps, now)));
    s.push(icon_at("rocket", ICON_CY, ICON_SIZE, WHITE, 1.0));
    let stroke = if ok == apps.len() { GREEN } else { AMBER };
    s.push(badge(BADGE_CY, stroke, format!("{ok}/{}", apps.len())));
    if degraded {
        s.push(Drawable::Dots {
            cx: CX,
            cy: MARKER_CY,
            spacing: 0.0,
            r: 3.0,
            colors: vec![RED.with_alpha(breathe(now, 2.4))],
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn app(name: &str, sync: AppSync, health: AppHealth, operating: bool) -> App {
        App {
            name: name.into(),
            sync,
            health,
            operating,
        }
    }

    fn sixteen(bad: Option<usize>) -> Vec<App> {
        (0..16)
            .map(|i| {
                if Some(i) == bad {
                    app(&format!("app-{i:02}"), AppSync::OutOfSync, AppHealth::Healthy, false)
                } else {
                    app(&format!("app-{i:02}"), AppSync::Synced, AppHealth::Healthy, false)
                }
            })
            .collect()
    }

    #[test]
    fn sixteen_apps_are_sixteen_arcs_with_gaps() {
        let v = app_states(&sixteen(Some(5)), 0.0);
        // 60 / 16 = 3 per app, one of them a gap
        assert!(matches!(v[0], SegState::On(c, _) if c == GREEN));
        assert!(matches!(v[1], SegState::On(c, _) if c == GREEN));
        assert_eq!(v[2], SegState::Off);
        assert!(matches!(v[15], SegState::On(c, a) if c == AMBER && a < 1.0));
        assert_eq!(v[17], SegState::Off);
        // the last app takes the leftover segments up to the end
        assert!(matches!(v[59], SegState::On(c, _) if c == GREEN));
        assert_eq!(app_states(&[], 0.0), vec![SegState::Off; 60]);
    }

    #[test]
    fn colours_follow_health_then_sync_then_operation() {
        let red = app("a", AppSync::Synced, AppHealth::Degraded, false);
        let grey = app("a", AppSync::Synced, AppHealth::Suspended, false);
        let amber = app("a", AppSync::Synced, AppHealth::Healthy, true);
        let green = app("a", AppSync::Synced, AppHealth::Healthy, false);
        assert_eq!(app_color(&red), (RED, false));
        assert_eq!(app_color(&grey), (GREY, false));
        assert_eq!(app_color(&amber), (AMBER, true));
        assert_eq!(app_color(&green), (GREEN, false));
        assert!(app_ok(&green) && !app_ok(&amber));
    }

    #[test]
    fn badge_counts_ok_apps_and_a_degraded_one_shows_the_marker() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Apps(sixteen(Some(3))), 0.0);
        let s = deploys_scene(&m, 0.5);
        let (text, stroke) = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "15/16");
        assert_eq!(stroke, AMBER);
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
        let mut apps = sixteen(None);
        apps[0].health = AppHealth::Degraded;
        m.apply(Event::Apps(apps), 1.0);
        let s = deploys_scene(&m, 1.5);
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
    }
}
```

- [ ] **Step 5: Wire in**

`crates/core/src/lib.rs`: `pub mod scene_deploys;`. `role_scene`: `Role::Deploys => crate::scene_deploys::deploys_scene(model, now),`. `needs_data`: `Role::Deploys => !(link.argocd && self.apps().have),`. Remove `Role::Deploys` from both catch-alls.

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): deploys role, one arc per Argo CD application

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: `gh-activity` role in core

**Files:**
- Modify: `crates/core/src/model.rs` (GithubState, smooth, folds, accessor)
- Create: `crates/core/src/scene_github.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs` (needs_data + key icon)

**Interfaces:**
- Consumes: `Event::{GithubActivity, GithubPush, GithubStar, GithubMerge, GithubRelease, GithubRun}`, `FxRequest::{GithubPush, GithubStar, GithubMerge, GithubRelease, GithubRunFailed, GithubRunPassed}`, `theme::GH_GREENS`.
- Produces: `Model::{github() -> &GithubState, smooth_gh_today(now)}`, `GithubState::{today(), best()}`, `scene_github::{github_scene, level_color, week_states}`.

- [ ] **Step 1: Failing model test**

```rust
    #[test]
    fn github_activity_folds_and_pushes_splash_per_commit() {
        let mut m = Model::new(Thresholds::default());
        let days: Vec<(String, u32)> = (0..30)
            .map(|i| (format!("2026-08-{:02}", i + 1), if i == 29 { 28 } else if i == 28 { 43 } else { 5 }))
            .collect();
        m.apply(Event::GithubActivity { days: days.clone() }, 0.0);
        assert!(m.github().have);
        assert_eq!(m.github().today(), 28);
        assert_eq!(m.github().best(), 43);
        assert_eq!(m.smooth_gh_today(5.0), 28.0);
        m.apply(
            Event::GithubPush {
                repo: "r".into(),
                commits: 3,
            },
            1.0,
        );
        assert_eq!(m.pending_fx(), &vec![FxRequest::GithubPush; 3]);
        m.take_fx();
        m.apply(Event::GithubStar { repo: "r".into() }, 1.0);
        m.apply(Event::GithubMerge { repo: "r".into() }, 1.0);
        m.apply(
            Event::GithubRelease {
                repo: "r".into(),
                tag: "v1".into(),
            },
            1.0,
        );
        m.apply(
            Event::GithubRun {
                repo: "r".into(),
                ok: false,
            },
            1.0,
        );
        m.apply(
            Event::GithubRun {
                repo: "r".into(),
                ok: true,
            },
            1.0,
        );
        assert_eq!(
            m.pending_fx(),
            &[
                FxRequest::GithubStar,
                FxRequest::GithubMerge,
                FxRequest::GithubRelease,
                FxRequest::GithubRunFailed,
                FxRequest::GithubRunPassed,
            ]
        );
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p rackscreen-core github_activity_folds`
Expected: compile error.

- [ ] **Step 3: State and folds**

`crates/core/src/model.rs`, after `AppsState`:

```rust
/// Thirty days of contribution counts, oldest first, last entry today.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GithubState {
    pub days: Vec<(String, u32)>,
    pub have: bool,
}

impl GithubState {
    pub fn today(&self) -> u32 {
        self.days.last().map(|d| d.1).unwrap_or(0)
    }
    /// Best day in the window, at least 1 so ratios stay finite.
    pub fn best(&self) -> u32 {
        self.days.iter().map(|d| d.1).max().unwrap_or(0).max(1)
    }
}
```

Fields: `github: GithubState,` and `gh_today: Smooth,`; initialisers `github: GithubState::default(),` and `gh_today: Smooth::new(0.0, SMOOTH_SECS),`. Accessors:

```rust
    pub fn github(&self) -> &GithubState {
        &self.github
    }
    pub fn smooth_gh_today(&self, now: Secs) -> f32 {
        self.gh_today.value(now)
    }
```

Folds (remove the six variants from the catch-all):

```rust
            Event::GithubActivity { days } => {
                self.github = GithubState { days, have: true };
                self.gh_today.set(self.github.today() as f32, now);
            }
            Event::GithubPush { commits, .. } => {
                // one request per commit: the splash queue collapses them into `+N`
                for _ in 0..commits.max(1) {
                    self.fx.push(FxRequest::GithubPush);
                }
            }
            Event::GithubStar { .. } => self.fx.push(FxRequest::GithubStar),
            Event::GithubMerge { .. } => self.fx.push(FxRequest::GithubMerge),
            Event::GithubRelease { .. } => self.fx.push(FxRequest::GithubRelease),
            Event::GithubRun { ok, .. } => self.fx.push(if ok {
                FxRequest::GithubRunPassed
            } else {
                FxRequest::GithubRunFailed
            }),
```

- [ ] **Step 4: The scene with its tests**

Create `crates/core/src/scene_github.rs`:

```rust
//! GitHub activity role: today against the month's best outside, the last week inside.

use crate::anim::{breathe, Secs};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, ring_states, seg_count, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, GH_GREENS, WHITE};

/// Calendar green for a day's count relative to the best day; `None` for zero.
pub fn level_color(count: u32, best: u32) -> Option<Color> {
    if count == 0 {
        return None;
    }
    let q = count as f32 / best.max(1) as f32;
    Some(if q <= 0.25 {
        GH_GREENS[0]
    } else if q <= 0.5 {
        GH_GREENS[1]
    } else if q <= 0.75 {
        GH_GREENS[2]
    } else {
        GH_GREENS[3]
    })
}

/// Seven sections for the last seven days (oldest first from 12 o'clock), one
/// unlit gap between them, today's section breathing.
pub fn week_states(days: &[(String, u32)], best: u32, n: usize, now: Secs) -> Vec<SegState> {
    let start = days.len().saturating_sub(7);
    let week = &days[start..];
    let per = (n / 7).max(1);
    let mut out = vec![SegState::Off; n];
    for (k, (_, count)) in week.iter().enumerate() {
        let Some(color) = level_color(*count, best) else {
            continue;
        };
        let first = k * per;
        let last = (first + per).min(n).saturating_sub(1); // the gap
        let alpha = if k + 1 == week.len() { breathe(now, 2.4) } else { 1.0 };
        for st in out.iter_mut().take(last).skip(first) {
            *st = SegState::On(color, alpha);
        }
    }
    out
}

pub fn github_scene(model: &Model, now: Secs) -> Scene {
    let g = model.github();
    let best = g.best();
    let today = model.smooth_gh_today(now);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states((today / best as f32 * 100.0).clamp(0.0, 100.0), GH_GREENS[3], SEG_N, now),
    ));
    s.push(ring(84.0, week_states(&g.days, best, seg_count(84.0), now)));
    s.push(icon_at("github", 92.0, 60.0, WHITE, 1.0));
    s.push(badge(BADGE_CY, GH_GREENS[3], format!("{}", g.today())));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::scene::Drawable;

    fn days(counts: &[u32]) -> Vec<(String, u32)> {
        counts
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("d{i}"), *c))
            .collect()
    }

    #[test]
    fn levels_are_quartiles_of_the_best_day() {
        assert_eq!(level_color(0, 40), None);
        assert_eq!(level_color(10, 40), Some(GH_GREENS[0]));
        assert_eq!(level_color(20, 40), Some(GH_GREENS[1]));
        assert_eq!(level_color(30, 40), Some(GH_GREENS[2]));
        assert_eq!(level_color(40, 40), Some(GH_GREENS[3]));
        assert_eq!(level_color(3, 0), Some(GH_GREENS[3]), "best is never zero");
    }

    #[test]
    fn week_ring_has_seven_sections_with_gaps_and_today_breathing() {
        let d = days(&[9, 9, 9, 12, 30, 0, 8, 25, 43, 28]);
        let v = week_states(&d, 43, 49, 0.6);
        // 49 / 7 = 7 per day: 6 lit + 1 gap
        assert!(matches!(v[0], SegState::On(c, _) if c == GH_GREENS[1])); // 12 of 43
        assert!(matches!(v[5], SegState::On(..)));
        assert_eq!(v[6], SegState::Off, "gap");
        assert!(matches!(v[7], SegState::On(c, _) if c == GH_GREENS[2])); // 30 of 43
        assert!(v[14..21].iter().all(|s| *s == SegState::Off), "a zero day is dark");
        assert!(matches!(v[42], SegState::On(c, a) if c == GH_GREENS[2] && a < 1.0)); // today, 28 of 43
        assert!(matches!(v[35], SegState::On(c, a) if c == GH_GREENS[3] && a == 1.0)); // 43 of 43
        // fewer than seven days still works
        let v = week_states(&days(&[1, 2]), 2, 49, 0.0);
        assert!(matches!(v[0], SegState::On(..)) && matches!(v[7], SegState::On(..)));
        assert!(v[14..].iter().all(|s| *s == SegState::Off));
    }

    #[test]
    fn scene_fills_outer_ring_by_today_over_best() {
        let mut m = Model::new(Thresholds::default());
        let mut counts = vec![5u32; 30];
        counts[28] = 43;
        counts[29] = 28;
        m.apply(Event::GithubActivity { days: days(&counts) }, 0.0);
        let s = github_scene(&m, 5.0);
        // 28 / 43 of 60 segments = 39
        assert_eq!(s.lit_count(), 39);
        let text = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "28");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "github", .. })));
    }
}
```

- [ ] **Step 5: Wire in, with the key icon when there is no token**

`crates/core/src/lib.rs`: `pub mod scene_github;`. `role_scene`: `Role::GhActivity => crate::scene_github::github_scene(model, now),`. `needs_data`: `Role::GhActivity => !(link.github && self.github().have),`. Replace the body of `scene_for_role` in `crates/core/src/scene_fx.rs`:

```rust
    pub fn scene_for_role(&self, role: Role, now: Secs) -> Scene {
        if !self.needs_data(role) {
            return role_scene(self, role, now);
        }
        let electricity = matches!(
            role,
            Role::PowerMix | Role::Price | Role::Carbon | Role::Renewable
        );
        // never configured, rather than a source that went quiet
        let icon = if role.is_sky() && !self.location_present() {
            "map-pin"
        } else if role == Role::GhActivity && !self.github_token_present() {
            "key-round"
        } else if electricity && !self.token_present() {
            "key-round"
        } else {
            "cloud-off"
        };
        no_data_scene_with(now, icon)
    }
```

and drop the now-unused `no_data_scene` import if clippy flags it. Add a test in `scene_fx.rs` `mod tests`:

```rust
    #[test]
    fn unconfigured_roles_show_pin_or_key() {
        let m = ready_model();
        let icon = |s: &Scene| icons(s)[0].0;
        assert_eq!(icon(&m.scene_for_role(Role::Weather, 1.0)), "map-pin");
        assert_eq!(icon(&m.scene_for_role(Role::GhActivity, 1.0)), "key-round");
        assert_eq!(icon(&m.scene_for_role(Role::Ups, 1.0)), "cloud-off");
        let mut m = ready_model();
        m.set_location_present(true);
        m.set_github_token_present(true);
        assert_eq!(icon(&m.scene_for_role(Role::Weather, 1.0)), "cloud-off");
        assert_eq!(icon(&m.scene_for_role(Role::GhActivity, 1.0)), "cloud-off");
    }
```

(`icons` in that test module returns `Vec<(&str, f32, f32)>`; the no-data scene has exactly one icon.)

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): gh-activity role with the week ring and GitHub splashes

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: `weather`, `wind` and `aqi` roles in core

**Files:**
- Modify: `crates/core/src/model.rs` (WeatherState, AirState, smooths, folds with Thunder and AirWorse edges)
- Create: `crates/core/src/scene_weather.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`

**Interfaces:**
- Consumes: `Event::{Weather, AirQuality}`, `FxRequest::{Thunder, AirWorse}`, `theme::{outdoor_color, eaqi_band, eaqi_color}`, `scene::badge_w`.
- Produces: `Model::{weather() -> &WeatherState, air() -> &AirState, smooth_temp, smooth_eaqi, smooth_wind}`, `scene_weather::{weather_scene, wind_scene, aqi_scene, weather_icon, wind_states}`.

- [ ] **Step 1: Failing model tests**

```rust
    fn weather(code: u16, temp: f32) -> Event {
        Event::Weather {
            temp_c: temp,
            code,
            is_day: true,
            wind_kmh: 19.0,
            gust_kmh: 39.0,
            wind_from_deg: 232.0,
            at: "2026-09-07T21:45".into(),
        }
    }

    #[test]
    fn weather_folds_and_thunder_splashes_on_the_edge() {
        let mut m = Model::new(Thresholds::default());
        m.apply(weather(95, 18.0), 0.0);
        assert!(m.weather().have);
        assert_eq!(m.weather().code, 95);
        assert_eq!(m.smooth_temp(5.0), 18.0);
        assert_eq!(m.smooth_wind(5.0), 19.0);
        assert!(m.pending_fx().is_empty(), "the first sample never splashes");
        m.apply(weather(3, 18.0), 1.0);
        m.apply(weather(96, 18.0), 2.0);
        assert_eq!(m.pending_fx(), &[FxRequest::Thunder]);
        m.apply(weather(99, 18.0), 3.0);
        assert_eq!(m.pending_fx().len(), 1, "staying thundery is not a new edge");
    }

    #[test]
    fn air_quality_splashes_when_the_band_worsens() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        assert!(m.air().have);
        assert_eq!(m.smooth_eaqi(5.0), 32.0);
        assert!(m.pending_fx().is_empty());
        m.apply(Event::AirQuality { eaqi: 38.0 }, 1.0);
        assert!(m.pending_fx().is_empty(), "same band");
        m.apply(Event::AirQuality { eaqi: 41.0 }, 2.0);
        assert_eq!(m.pending_fx(), &[FxRequest::AirWorse]);
        m.apply(Event::AirQuality { eaqi: 10.0 }, 3.0);
        assert_eq!(m.pending_fx().len(), 1, "improving is quiet");
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-core weather_folds`
Expected: compile error.

- [ ] **Step 3: State and folds**

`crates/core/src/model.rs`, after `GithubState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherState {
    pub temp_c: f32,
    pub code: u16,
    pub is_day: bool,
    pub wind_kmh: f32,
    pub gust_kmh: f32,
    pub wind_from_deg: f32,
    pub at: String,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AirState {
    pub eaqi: f32,
    pub have: bool,
}

/// WMO codes 95, 96 and 99 are thunderstorms.
fn is_thunder(code: u16) -> bool {
    (95..=99).contains(&code)
}
```

Fields: `weather: WeatherState, air: AirState, temp: Smooth, eaqi: Smooth, wind: Smooth,`; initialisers `weather: WeatherState::default(), air: AirState::default(), temp: Smooth::new(0.0, SMOOTH_SECS), eaqi: Smooth::new(0.0, SMOOTH_SECS), wind: Smooth::new(0.0, SMOOTH_SECS),`. Accessors:

```rust
    pub fn weather(&self) -> &WeatherState {
        &self.weather
    }
    pub fn air(&self) -> &AirState {
        &self.air
    }
    pub fn smooth_temp(&self, now: Secs) -> f32 {
        self.temp.value(now)
    }
    pub fn smooth_eaqi(&self, now: Secs) -> f32 {
        self.eaqi.value(now)
    }
    pub fn smooth_wind(&self, now: Secs) -> f32 {
        self.wind.value(now)
    }
```

Folds (add `use crate::theme::eaqi_band;`):

```rust
            Event::Weather {
                temp_c,
                code,
                is_day,
                wind_kmh,
                gust_kmh,
                wind_from_deg,
                at,
            } => {
                if self.weather.have && is_thunder(code) && !is_thunder(self.weather.code) {
                    self.fx.push(FxRequest::Thunder);
                }
                self.temp.set(temp_c, now);
                self.wind.set(wind_kmh, now);
                self.weather = WeatherState {
                    temp_c,
                    code,
                    is_day,
                    wind_kmh,
                    gust_kmh,
                    wind_from_deg,
                    at,
                    have: true,
                };
            }
            Event::AirQuality { eaqi } => {
                if self.air.have && eaqi_band(eaqi) > eaqi_band(self.air.eaqi) {
                    self.fx.push(FxRequest::AirWorse);
                }
                self.eaqi.set(eaqi, now);
                self.air = AirState { eaqi, have: true };
            }
```

- [ ] **Step 4: The scenes with their tests**

Create `crates/core/src/scene_weather.rs`:

```rust
//! Open-Meteo roles: weather now, wind compass, air quality.

use crate::anim::{pulse, Secs};
use crate::model::Model;
use crate::scene::{badge, badge_w, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{eaqi_color, outdoor_color, Color, AMBER, BLUE, DIM_GREY, OFF, WHITE};

const MOON: Color = Color::hex(0xe8e8f0);

/// Lucide icon for a WMO weather interpretation code.
pub fn weather_icon(code: u16, is_day: bool) -> &'static str {
    match code {
        0 => {
            if is_day {
                "sun"
            } else {
                "moon"
            }
        }
        1 | 2 => {
            if is_day {
                "cloud-sun"
            } else {
                "cloud-moon"
            }
        }
        3 => "cloud",
        45 | 48 => "cloud-fog",
        51..=57 => "cloud-drizzle",
        61..=67 | 80..=82 => "cloud-rain",
        71..=77 | 85 | 86 => "snowflake",
        95..=99 => "cloud-lightning",
        _ => "cloud",
    }
}

/// Colour and micro-loop `(scale, dy, alpha)` per icon.
fn icon_motion(icon: &str, now: Secs) -> (Color, f32, f32, f32) {
    match icon {
        "sun" => (AMBER, 1.0 + 0.06 * pulse(now, 3.5), 0.0, 1.0),
        "moon" => (MOON, 1.0 + 0.04 * pulse(now, 3.5), 0.0, 1.0),
        "cloud-sun" => (AMBER, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud-moon" => (MOON, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud" => (WHITE, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud-fog" => (WHITE, 1.0, 0.0, 0.6 + 0.4 * pulse(now, 3.0)),
        "cloud-drizzle" | "cloud-rain" => (BLUE, 1.0, -3.0 * pulse(now, 1.4), 1.0),
        "snowflake" => (WHITE, 1.0, -3.0 * pulse(now, 3.0), 1.0),
        "cloud-lightning" => (AMBER, 1.0, 0.0, 0.5 + 0.5 * pulse(now, 0.9)),
        _ => (WHITE, 1.0, 0.0, 1.0),
    }
}

pub fn weather_scene(model: &Model, now: Secs) -> Scene {
    let w = model.weather();
    let t = model.smooth_temp(now);
    let color = outdoor_color(t);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states(((t + 10.0) / 50.0 * 100.0).clamp(0.0, 100.0), color, SEG_N, now),
    ));
    let name = weather_icon(w.code, w.is_day);
    let (icon_color, scale, dy, alpha) = icon_motion(name, now);
    let mut ic = icon_at(name, ICON_CY, ICON_SIZE, icon_color, alpha);
    if let Drawable::Icon {
        scale: sc, dy: d, ..
    } = &mut ic
    {
        *sc = scale;
        *d = dy;
    }
    s.push(ic);
    s.push(badge(BADGE_CY, color, format!("{t:.0}°C")));
    s
}

/// Below this the compass is all dim and the badge says `calm`.
pub const CALM_KMH: f32 = 3.0;

/// A blue arc centred on the from-direction (12 o'clock is north), half-width
/// `3 + speed / 5` segments, fading to the edge; a gust widens it for 0.8 s
/// every 6 s when it beats the mean speed by 30 %.
pub fn wind_states(from_deg: f32, speed_kmh: f32, gust_kmh: f32, now: Secs) -> Vec<SegState> {
    let n = SEG_N as i32;
    if speed_kmh < CALM_KMH {
        return vec![SegState::On(DIM_GREY, 1.0); SEG_N];
    }
    let gusting = gust_kmh > speed_kmh * 1.3 && now.rem_euclid(6.0) < 0.8;
    let speed = if gusting { gust_kmh } else { speed_kmh };
    let half = ((3.0 + speed / 5.0).round() as i32).min(29);
    let centre = ((from_deg / 6.0).round() as i32).rem_euclid(n);
    (0..n)
        .map(|i| {
            let d = ((i - centre).rem_euclid(n)).min((centre - i).rem_euclid(n));
            if d <= half {
                SegState::On(BLUE.mix(OFF, d as f32 / (half as f32 + 1.0)), 1.0)
            } else {
                SegState::Off
            }
        })
        .collect()
}

pub fn wind_scene(model: &Model, now: Secs) -> Scene {
    let w = model.weather();
    let speed = model.smooth_wind(now);
    let mut s = Scene::new();
    s.push(ring(RING_R, wind_states(w.wind_from_deg, speed, w.gust_kmh, now)));
    s.push(icon_at("wind", ICON_CY, ICON_SIZE, BLUE, 1.0));
    let text = if speed < CALM_KMH {
        "calm".to_string()
    } else {
        format!("{speed:.0} km/h")
    };
    s.push(badge_w(BADGE_CY, BLUE, text, 84.0));
    s
}

pub fn aqi_scene(model: &Model, now: Secs) -> Scene {
    let v = model.smooth_eaqi(now);
    let color = eaqi_color(v);
    let mut s = Scene::new();
    s.push(ring(RING_R, ring_states(v.clamp(0.0, 100.0), color, SEG_N, now)));
    s.push(icon_at("haze", ICON_CY, ICON_SIZE, color, 1.0));
    s.push(badge_w(BADGE_CY, color, format!("AQI {v:.0}"), 84.0));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::theme::{GREEN, RED};

    fn model(code: u16, is_day: bool, temp: f32, wind: f32, gust: f32, from: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Weather {
                temp_c: temp,
                code,
                is_day,
                wind_kmh: wind,
                gust_kmh: gust,
                wind_from_deg: from,
                at: String::new(),
            },
            0.0,
        );
        m
    }

    fn badge_of(s: &Scene) -> (String, Color, f32) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge {
                    text, stroke, w, ..
                } => Some((text.clone(), *stroke, *w)),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn wmo_codes_map_to_icons() {
        assert_eq!(weather_icon(0, true), "sun");
        assert_eq!(weather_icon(0, false), "moon");
        assert_eq!(weather_icon(2, false), "cloud-moon");
        assert_eq!(weather_icon(3, true), "cloud");
        assert_eq!(weather_icon(48, true), "cloud-fog");
        assert_eq!(weather_icon(55, true), "cloud-drizzle");
        assert_eq!(weather_icon(81, true), "cloud-rain");
        assert_eq!(weather_icon(75, true), "snowflake");
        assert_eq!(weather_icon(99, true), "cloud-lightning");
        assert_eq!(weather_icon(123, true), "cloud");
    }

    #[test]
    fn weather_ring_and_badge_follow_the_outdoor_scale() {
        let s = weather_scene(&model(2, true, 18.0, 0.0, 0.0, 0.0), 5.0);
        // (18 + 10) / 50 of 60 = 33.6 -> 34
        assert_eq!(s.lit_count(), 34);
        let (text, stroke, _) = badge_of(&s);
        assert_eq!(text, "18°C");
        assert_eq!(stroke, outdoor_color(18.0));
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "cloud-sun", color, .. } if *color == AMBER)));
        let cold = weather_scene(&model(71, true, -12.0, 0.0, 0.0, 0.0), 5.0);
        assert_eq!(cold.lit_count(), 0);
        assert_eq!(badge_of(&cold).1, BLUE);
        let hot = weather_scene(&model(0, true, 40.0, 0.0, 0.0, 0.0), 5.0);
        assert_eq!(hot.lit_count(), 60);
        assert_eq!(badge_of(&hot).1, RED);
    }

    #[test]
    fn wind_arc_is_centred_on_the_from_direction() {
        // from 90° (east): centre segment 15, 23 km/h -> half width 8
        let v = wind_states(90.0, 23.0, 23.0, 1.0);
        assert!(matches!(v[15], SegState::On(c, _) if c == BLUE));
        assert!(matches!(v[7], SegState::On(..)) && matches!(v[23], SegState::On(..)));
        assert_eq!(v[6], SegState::Off);
        assert_eq!(v[24], SegState::Off);
        assert_eq!(v[45], SegState::Off, "the far side is dark");
        // wrap-around near north
        let v = wind_states(357.0, 10.0, 10.0, 1.0);
        assert!(matches!(v[0], SegState::On(c, _) if c == BLUE));
        assert!(matches!(v[59], SegState::On(..)) && matches!(v[1], SegState::On(..)));
        // a gust widens the arc briefly
        let calm_phase = wind_states(90.0, 20.0, 40.0, 3.0);
        let gust_phase = wind_states(90.0, 20.0, 40.0, 6.2);
        let lit = |v: &[SegState]| v.iter().filter(|s| matches!(s, SegState::On(..))).count();
        assert!(lit(&gust_phase) > lit(&calm_phase));
        // calm: everything dim, nothing off
        assert!(wind_states(90.0, 1.0, 1.0, 0.0)
            .iter()
            .all(|s| matches!(s, SegState::On(c, _) if *c == DIM_GREY)));
    }

    #[test]
    fn wind_and_aqi_badges() {
        let s = wind_scene(&model(0, true, 18.0, 23.0, 39.0, 232.0), 5.0);
        assert_eq!(badge_of(&s), ("23 km/h".into(), BLUE, 84.0));
        let calm = wind_scene(&model(0, true, 18.0, 1.0, 2.0, 0.0), 5.0);
        assert_eq!(badge_of(&calm).0, "calm");
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        let s = aqi_scene(&m, 5.0);
        let (text, stroke, w) = badge_of(&s);
        assert_eq!((text.as_str(), w), ("AQI 32", 84.0));
        assert_eq!(stroke, Color::hex(0x50CCAA));
        assert_eq!(s.lit_count(), 19);
        assert_ne!(stroke, GREEN);
    }
}
```

- [ ] **Step 5: Wire in**

`lib.rs`: `pub mod scene_weather;`. `role_scene`:

```rust
        Role::Weather => crate::scene_weather::weather_scene(model, now),
        Role::Wind => crate::scene_weather::wind_scene(model, now),
        Role::Aqi => crate::scene_weather::aqi_scene(model, now),
```

`needs_data`:

```rust
            Role::Weather | Role::Wind => !(link.weather && self.weather().have),
            Role::Aqi => !(link.weather && self.air().have),
```

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): weather, wind and aqi roles

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: `rain` role in core

**Files:**
- Modify: `crates/core/src/model.rs` (RainState, fold with the RainSoon edge)
- Create: `crates/core/src/scene_rain.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`

**Interfaces:**
- Consumes: `Event::Rain`, `FxRequest::RainSoon`, `Model::unix_now()`.
- Produces: `Model::rain() -> &RainState`, `scene_rain::{rain_scene, current_slots, first_wet_minutes, rain_color, rain_states, RAIN_SLOTS, RAIN_SEGS, SLOT_SECS, WET_MM}`.

- [ ] **Step 1: Failing tests**

Model test:

```rust
    #[test]
    fn rain_folds_and_splashes_when_rain_moves_inside_fifteen_minutes() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_000_000);
        let mut far = vec![0.0f32; 24];
        far[6] = 2.0; // 30 minutes out
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: far.clone(),
            },
            0.0,
        );
        assert!(m.rain().have);
        assert!(m.pending_fx().is_empty(), "first sample is quiet");
        let mut soon = vec![0.0f32; 24];
        soon[2] = 2.0; // 10 minutes out
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: soon.clone(),
            },
            1.0,
        );
        assert_eq!(m.pending_fx(), &[FxRequest::RainSoon]);
        m.apply(
            Event::Rain {
                from: 1_000_300,
                mm_per_h: soon,
            },
            2.0,
        );
        assert_eq!(m.pending_fx().len(), 1, "already inside the window: no repeat");
        // raining now: no umbrella, it is too late for one
        let mut wet_now = vec![0.0f32; 24];
        wet_now[0] = 1.0;
        m.take_fx();
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: far,
            },
            3.0,
        );
        m.apply(
            Event::Rain {
                from: 1_000_000,
                mm_per_h: wet_now,
            },
            4.0,
        );
        assert!(m.pending_fx().is_empty());
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p rackscreen-core rain_folds`
Expected: compile error.

- [ ] **Step 3: The scene module (pure functions first, the model uses them)**

Create `crates/core/src/scene_rain.rs`:

```rust
//! Rain role: the Buienradar nowcast, two hours in five-minute slots.

use crate::anim::{breathe, Secs};
use crate::model::Model;
use crate::scene::{badge, icon_at, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, BLUE, GREY, VIOLET};

pub const RAIN_SLOTS: usize = 24;
/// Two segments per slot.
pub const RAIN_SEGS: usize = 48;
pub const SLOT_SECS: i64 = 300;
/// A slot at or above this counts as rain.
pub const WET_MM: f32 = 0.1;
/// Rain moving inside this many minutes while it is dry splashes the umbrella.
pub const SOON_MINUTES: u32 = 15;

/// The slots still ahead: elapsed ones are dropped from the front so slot 0
/// is always the current five minutes.
pub fn current_slots(from: i64, mm_per_h: &[f32], unix_now: i64) -> Vec<f32> {
    let elapsed = ((unix_now - from).max(0) / SLOT_SECS) as usize;
    mm_per_h.iter().copied().skip(elapsed).collect()
}

/// Minutes until the first wet slot, `None` when the window is dry.
pub fn first_wet_minutes(slots: &[f32]) -> Option<u32> {
    slots
        .iter()
        .position(|v| *v >= WET_MM)
        .map(|i| (i as i64 * SLOT_SECS / 60) as u32)
}

/// Colour and alpha for an intensity; `None` for dry.
pub fn rain_color(mm: f32) -> Option<(Color, f32)> {
    if mm < WET_MM {
        None
    } else if mm < 1.0 {
        Some((BLUE, 0.5))
    } else if mm < 5.0 {
        Some((BLUE.mix(VIOLET, (mm - 1.0) / 4.0), 1.0))
    } else {
        Some((VIOLET, 1.0))
    }
}

/// Two segments per slot, slot 0 at 12 o'clock breathing.
pub fn rain_states(slots: &[f32], now: Secs) -> Vec<SegState> {
    let mut out = vec![SegState::Off; RAIN_SEGS];
    for (i, mm) in slots.iter().enumerate().take(RAIN_SLOTS) {
        let Some((color, alpha)) = rain_color(*mm) else {
            continue;
        };
        let alpha = if i == 0 { alpha * breathe(now, 2.4) } else { alpha };
        out[i * 2] = SegState::On(color, alpha);
        out[i * 2 + 1] = SegState::On(color, alpha);
    }
    out
}

pub fn rain_scene(model: &Model, now: Secs) -> Scene {
    let r = model.rain();
    let slots = current_slots(r.from, &r.mm_per_h, model.unix_now());
    let wet_ahead = first_wet_minutes(&slots);
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: RAIN_SEGS,
        states: rain_states(&slots, now),
        pitch_deg: 360.0 / RAIN_SEGS as f32,
        start_deg: 0.0,
    });
    let icon_color = if wet_ahead.is_some() { BLUE } else { GREY };
    s.push(icon_at("cloud-rain", ICON_CY, ICON_SIZE, icon_color, 1.0));
    let (text, stroke) = match (slots.first().copied(), wet_ahead) {
        (Some(mm), _) if mm >= WET_MM => (format!("{mm:.1} mm"), BLUE),
        (_, Some(mins)) => (format!("{mins} min"), BLUE),
        _ => ("DRY".to_string(), GREY),
    };
    s.push(badge(BADGE_CY, stroke, text));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn slots(wet: &[(usize, f32)]) -> Vec<f32> {
        let mut v = vec![0.0f32; 24];
        for (i, mm) in wet {
            v[*i] = *mm;
        }
        v
    }

    #[test]
    fn elapsed_slots_drop_off_the_front() {
        let s = slots(&[(3, 2.0)]);
        assert_eq!(current_slots(1000, &s, 1000).len(), 24);
        let later = current_slots(1000, &s, 1000 + 2 * SLOT_SECS + 10);
        assert_eq!(later.len(), 22);
        assert_eq!(later[1], 2.0);
        assert!(current_slots(1000, &s, 1000 + 30 * SLOT_SECS).is_empty());
        assert_eq!(current_slots(1000, &s, 900).len(), 24, "clock behind: nothing dropped");
    }

    #[test]
    fn first_wet_and_colours() {
        assert_eq!(first_wet_minutes(&slots(&[(5, 0.3)])), Some(25));
        assert_eq!(first_wet_minutes(&slots(&[(0, 1.0)])), Some(0));
        assert_eq!(first_wet_minutes(&slots(&[])), None);
        assert_eq!(first_wet_minutes(&slots(&[(2, 0.05)])), None, "below the wet threshold");
        assert_eq!(rain_color(0.0), None);
        assert_eq!(rain_color(0.5), Some((BLUE, 0.5)));
        assert_eq!(rain_color(1.0), Some((BLUE, 1.0)));
        assert_eq!(rain_color(3.0), Some((BLUE.mix(VIOLET, 0.5), 1.0)));
        assert_eq!(rain_color(7.0), Some((VIOLET, 1.0)));
    }

    #[test]
    fn ring_has_two_segments_per_slot_and_the_current_slot_breathes() {
        let v = rain_states(&slots(&[(0, 2.0), (1, 6.0)]), 0.6);
        assert_eq!(v.len(), RAIN_SEGS);
        assert!(matches!(v[0], SegState::On(_, a) if a < 1.0));
        assert!(matches!(v[1], SegState::On(_, a) if a < 1.0));
        assert!(matches!(v[2], SegState::On(c, a) if c == VIOLET && a == 1.0));
        assert!(matches!(v[3], SegState::On(c, _) if c == VIOLET));
        assert_eq!(v[4], SegState::Off);
    }

    fn model(from: i64, now: i64, s: Vec<f32>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(now);
        m.apply(Event::Rain { from, mm_per_h: s }, 0.0);
        m
    }

    fn badge_text(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn badge_says_dry_minutes_or_millimetres() {
        assert_eq!(badge_text(&rain_scene(&model(0, 0, slots(&[])), 1.0)), ("DRY".into(), GREY));
        assert_eq!(
            badge_text(&rain_scene(&model(0, 0, slots(&[(5, 1.0)])), 1.0)),
            ("25 min".into(), BLUE)
        );
        assert_eq!(
            badge_text(&rain_scene(&model(0, 0, slots(&[(0, 2.3)])), 1.0)),
            ("2.3 mm".into(), BLUE)
        );
        // ten minutes later the same nowcast says 15 min
        assert_eq!(
            badge_text(&rain_scene(&model(0, 600, slots(&[(5, 1.0)])), 1.0)).0,
            "15 min"
        );
        let dry = rain_scene(&model(0, 0, slots(&[])), 1.0);
        assert!(dry
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name: "cloud-rain", color, .. } if *color == GREY)));
    }
}
```

- [ ] **Step 4: State and fold**

`crates/core/src/model.rs`, after `AirState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RainState {
    pub from: i64,
    pub mm_per_h: Vec<f32>,
    pub have: bool,
}
```

Fields `rain: RainState,` and `/// Minutes to rain as of the previous nowcast, for the umbrella edge.\n rain_prev_wet: Option<u32>,`; initialisers `rain: RainState::default(), rain_prev_wet: None,`; accessor `pub fn rain(&self) -> &RainState { &self.rain }`. Fold (add `use crate::scene_rain::{current_slots, first_wet_minutes, SOON_MINUTES, WET_MM};`):

```rust
            Event::Rain { from, mm_per_h } => {
                let slots = current_slots(from, &mm_per_h, self.unix_now);
                let wet = first_wet_minutes(&slots);
                let dry_now = slots.first().is_none_or(|v| *v < WET_MM);
                let inside = |w: Option<u32>| w.is_some_and(|m| m <= SOON_MINUTES);
                if self.rain.have && dry_now && inside(wet) && !inside(self.rain_prev_wet) {
                    self.fx.push(FxRequest::RainSoon);
                }
                self.rain_prev_wet = wet;
                self.rain = RainState {
                    from,
                    mm_per_h,
                    have: true,
                };
            }
```

(`Option::is_none_or` is stable since Rust 1.82; the toolchain floor is 1.85.)

- [ ] **Step 5: Wire in**

`lib.rs`: `pub mod scene_rain;`. `role_scene`: `Role::Rain => crate::scene_rain::rain_scene(model, now),`. `needs_data`: `Role::Rain => !(link.rain && self.rain().have),`.

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): rain role from the Buienradar nowcast

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: `sun`, `moon` and `iss` roles in core

**Files:**
- Modify: `crates/core/src/model.rs` (SkyState, IssState, folds, ISS sweep in `tick`)
- Create: `crates/core/src/scene_sky.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scene.rs`, `crates/core/src/scene_fx.rs`

**Interfaces:**
- Consumes: `Event::{Sky, IssPass}`, `event::{IssPass, MoonPhase}`, `FxRequest::IssPass`, `Model::{unix_now, utc_offset_secs}`, `format::fmt_until`, `scene::badge_w`.
- Produces: `Model::{sky() -> &SkyState, iss() -> &IssState}`, `scene_sky::{sun_scene, moon_scene, iss_scene, sun_states, moon_states, iss_fill}`.

- [ ] **Step 1: Failing model tests**

```rust
    #[test]
    fn sky_and_iss_fold_and_a_visible_pass_sweeps_once() {
        use crate::event::{IssPass, MoonPhase};
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Sky {
                sunrise: Some(100),
                sunset: Some(200),
                sun_elevation_deg: 30.0,
                moon_illumination: 0.63,
                moon_waxing: true,
                moon_phase: MoonPhase::WaxingGibbous,
            },
            0.0,
        );
        assert!(m.sky().have);
        assert_eq!(m.sky().sunset, Some(200));
        let pass = IssPass {
            start: 1_000,
            end: 1_400,
            max_elevation_deg: 62.0,
            visible: true,
        };
        m.apply(Event::IssPass(Some(pass)), 0.0);
        assert!(m.iss().have);
        m.set_unix_now(900);
        m.tick(1.0);
        assert!(m.fx().sweeps.active().is_none(), "not yet");
        m.set_unix_now(1_000);
        m.tick(2.0);
        assert_eq!(
            m.fx().sweeps.active().map(|s| s.kind),
            Some(crate::fx::SweepKind::IssPass)
        );
        m.set_unix_now(1_010);
        m.tick(3.0);
        assert!(m.pending_fx().is_empty(), "one sweep per pass");
        // a pass that is not visible stays quiet
        let mut m2 = Model::new(Thresholds::default());
        m2.apply(
            Event::IssPass(Some(IssPass {
                visible: false,
                ..pass
            })),
            0.0,
        );
        m2.set_unix_now(1_000);
        m2.tick(1.0);
        assert!(m2.fx().sweeps.active().is_none());
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p rackscreen-core sky_and_iss`
Expected: compile error.

- [ ] **Step 3: State, folds and the sweep in `tick`**

`crates/core/src/model.rs`, after `RainState` (extend the `use crate::event::...` import with `IssPass, MoonPhase`):

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct SkyState {
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub sun_elevation_deg: f32,
    pub moon_illumination: f32,
    pub moon_waxing: bool,
    pub moon_phase: MoonPhase,
    pub have: bool,
}

impl Default for SkyState {
    fn default() -> Self {
        Self {
            sunrise: None,
            sunset: None,
            sun_elevation_deg: 0.0,
            moon_illumination: 0.0,
            moon_waxing: true,
            moon_phase: MoonPhase::New,
            have: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssState {
    pub pass: Option<IssPass>,
    pub have: bool,
}
```

Fields `sky: SkyState, iss: IssState, /// Start time of the pass whose sweep has played.\n iss_swept: Option<i64>,`; initialisers `sky: SkyState::default(), iss: IssState::default(), iss_swept: None,`; accessors:

```rust
    pub fn sky(&self) -> &SkyState {
        &self.sky
    }
    pub fn iss(&self) -> &IssState {
        &self.iss
    }
```

Folds (remove `Sky` and `IssPass` from the catch-all, which should now be empty and deleted):

```rust
            Event::Sky {
                sunrise,
                sunset,
                sun_elevation_deg,
                moon_illumination,
                moon_waxing,
                moon_phase,
            } => {
                self.sky = SkyState {
                    sunrise,
                    sunset,
                    sun_elevation_deg,
                    moon_illumination,
                    moon_waxing,
                    moon_phase,
                    have: true,
                };
            }
            Event::IssPass(pass) => {
                self.iss = IssState { pass, have: true };
            }
```

In `tick`, after the net phase block:

```rust
        if let Some(p) = self.iss.pass {
            let inside = (p.start..p.end).contains(&self.unix_now);
            if p.visible && inside && self.iss_swept != Some(p.start) {
                self.iss_swept = Some(p.start);
                self.fx.push(FxRequest::IssPass);
            }
        }
```

(The push happens before the drain loop `for req in std::mem::take(&mut self.fx)` further down in `tick`, so the sweep starts on the same frame. Check the order: put the block before `for req in ...`.)

- [ ] **Step 4: The scenes with their tests**

Create `crates/core/src/scene_sky.rs`:

```rust
//! Computed sky roles: the sun dial, the moon phase and the ISS pass countdown.

use crate::anim::{breathe, Secs};
use crate::format::fmt_until;
use crate::model::Model;
use crate::scene::{badge, badge_w, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREY, VIOLET};

pub const DAY: Color = AMBER;
pub const NIGHT: Color = Color::hex(0x1b2a4a);
pub const NOW: Color = Color::hex(0xfff2b0);
pub const MOON: Color = Color::hex(0xe8e8f0);
/// Seconds of dial per segment: 24 h over 60 segments.
const SEG_SECS: i64 = 1440;
/// Half-width of the twilight blend around sunrise and sunset.
const TWILIGHT_SECS: i64 = 1800;
/// The ISS ring is full this long before a pass and empties toward it.
pub const ISS_HORIZON_SECS: i64 = 12 * 3600;

/// 24-hour dial, midnight at 12 o'clock. `rise` and `set` are seconds of the
/// local day; `polar_day` decides a day without either.
pub fn sun_states(
    local_secs: i64,
    rise: Option<i64>,
    set: Option<i64>,
    polar_day: bool,
    now: Secs,
) -> Vec<SegState> {
    let blend = |centre: i64| -> Color {
        match (rise, set) {
            (Some(r), Some(s)) => {
                let day = centre >= r && centre <= s;
                let base = if day { DAY } else { NIGHT };
                let near_rise = (centre - r).abs() < TWILIGHT_SECS;
                let near_set = (centre - s).abs() < TWILIGHT_SECS;
                if near_rise {
                    NIGHT.mix(DAY, ((centre - r) as f32 / TWILIGHT_SECS as f32 + 1.0) / 2.0)
                } else if near_set {
                    NIGHT.mix(DAY, ((s - centre) as f32 / TWILIGHT_SECS as f32 + 1.0) / 2.0)
                } else {
                    base
                }
            }
            _ => {
                if polar_day {
                    DAY
                } else {
                    NIGHT
                }
            }
        }
    };
    let now_seg = (local_secs.rem_euclid(86_400) / SEG_SECS) as usize;
    (0..SEG_N)
        .map(|i| {
            if i == now_seg {
                SegState::On(NOW, breathe(now, 2.4))
            } else {
                SegState::On(blend(i as i64 * SEG_SECS + SEG_SECS / 2), 1.0)
            }
        })
        .collect()
}

pub fn sun_scene(model: &Model, now: Secs) -> Scene {
    let sky = model.sky();
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let local = unix + offset;
    let local_secs = local.rem_euclid(86_400);
    let day_start = local - local_secs;
    let on_dial = |t: Option<i64>| t.map(|t| (t + offset - day_start).rem_euclid(86_400));
    let daytime = sky.sun_elevation_deg > 0.0;
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        sun_states(local_secs, on_dial(sky.sunrise), on_dial(sky.sunset), daytime, now),
    ));
    let (icon, target) = if daytime {
        ("sunset", sky.sunset)
    } else {
        ("sunrise", sky.sunrise)
    };
    s.push(icon_at(icon, ICON_CY, ICON_SIZE, AMBER, 1.0));
    let text = match target {
        Some(t) if t >= unix => fmt_until(t - unix),
        _ => "--".to_string(),
    };
    s.push(badge(BADGE_CY, AMBER, text));
    s
}

/// Illuminated fraction as lit segments: clockwise from the top while waxing,
/// counter-clockwise while waning.
pub fn moon_states(illumination: f32, waxing: bool, now: Secs) -> Vec<SegState> {
    let v = ring_states(illumination.clamp(0.0, 1.0) * 100.0, MOON, SEG_N, now);
    if waxing {
        return v;
    }
    let n = v.len();
    (0..n).map(|i| v[(n - i) % n]).collect()
}

pub fn moon_scene(model: &Model, now: Secs) -> Scene {
    let sky = model.sky();
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        moon_states(sky.moon_illumination, sky.moon_waxing, now),
    ));
    s.push(icon_at("moon", ICON_CY, ICON_SIZE, MOON, 1.0));
    let odd = ((now / 5.0).floor() as i64).rem_euclid(2) == 1;
    let text = if odd {
        sky.moon_phase.label().to_string()
    } else {
        format!("{:.0}%", sky.moon_illumination * 100.0)
    };
    let mut b = badge_w(BADGE_CY, MOON, text, 84.0);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s
}

/// Ring fill before a pass: full twelve hours out, empty at the start.
pub fn iss_fill(start: i64, unix_now: i64) -> f32 {
    ((start - unix_now).clamp(0, ISS_HORIZON_SECS) as f32 / ISS_HORIZON_SECS as f32) * 100.0
}

pub fn iss_scene(model: &Model, now: Secs) -> Scene {
    let unix = model.unix_now();
    let mut s = Scene::new();
    let (states, text, stroke) = match model.iss().pass {
        Some(p) if unix < p.start => {
            let mut v = ring_states(iss_fill(p.start, unix), VIOLET, SEG_N, now);
            if !p.visible {
                for st in v.iter_mut() {
                    if let SegState::On(c, a) = *st {
                        *st = SegState::On(c, a * 0.4);
                    }
                }
            }
            (v, fmt_until(p.start - unix), VIOLET)
        }
        Some(p) if unix < p.end => {
            let a = breathe(now, 1.2) * if p.visible { 1.0 } else { 0.4 };
            (
                vec![SegState::On(VIOLET, a); SEG_N],
                format!("{:.0}°", p.max_elevation_deg),
                VIOLET,
            )
        }
        _ => (vec![SegState::Off; SEG_N], "--".to_string(), GREY),
    };
    s.push(ring(RING_R, states));
    s.push(icon_at("satellite", ICON_CY, ICON_SIZE, VIOLET, 1.0));
    s.push(badge(BADGE_CY, stroke, text));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, IssPass, MoonPhase};
    use crate::model::Thresholds;

    fn lit(v: &[SegState]) -> usize {
        v.iter().filter(|s| matches!(s, SegState::On(..))).count()
    }

    #[test]
    fn sun_dial_lights_day_amber_night_blue_and_now_bright() {
        // sunrise 07:00, sunset 20:00, now 14:07
        let v = sun_states(14 * 3600 + 7 * 60, Some(7 * 3600), Some(20 * 3600), true, 0.6);
        assert_eq!(v.len(), 60);
        assert!(matches!(v[0], SegState::On(c, _) if c == NIGHT), "midnight");
        assert!(matches!(v[30], SegState::On(c, _) if c == DAY), "noon");
        assert!(matches!(v[35], SegState::On(c, a) if c == NOW && a < 1.0), "14:07 breathes");
        assert!(matches!(v[57], SegState::On(c, _) if c == NIGHT), "23:00");
        // twilight blends around sunrise: segment 17 is 06:48..07:12
        assert!(matches!(v[17], SegState::On(c, _) if c != DAY && c != NIGHT));
        // polar cases
        assert!(sun_states(0, None, None, true, 0.0)[30].eq(&SegState::On(DAY, 1.0)));
        assert!(sun_states(0, None, None, false, 0.0)[30].eq(&SegState::On(NIGHT, 1.0)));
    }

    fn sky_model(unix: i64, offset: i32, rise: i64, set: i64, elevation: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(offset);
        m.apply(
            Event::Sky {
                sunrise: Some(rise),
                sunset: Some(set),
                sun_elevation_deg: elevation,
                moon_illumination: 0.63,
                moon_waxing: false,
                moon_phase: MoonPhase::WaningGibbous,
            },
            0.0,
        );
        m
    }

    fn badge_of(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap()
    }

    fn icon_of(s: &Scene) -> &'static str {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn sun_scene_counts_down_to_sunset_by_day_and_sunrise_by_night() {
        // 2026-09-07 12:00 UTC, Amsterdam (+2): sunset 20:00 local = 18:00 UTC
        let day = 1_788_782_400;
        let m = sky_model(day, 7200, day - 5 * 3600, day + 6 * 3600, 40.0);
        let s = sun_scene(&m, 1.0);
        assert_eq!(icon_of(&s), "sunset");
        assert_eq!(badge_of(&s).0, "-6h00");
        let night = sky_model(day + 12 * 3600, 7200, day + 19 * 3600, day + 6 * 3600, -20.0);
        let s = sun_scene(&night, 1.0);
        assert_eq!(icon_of(&s), "sunrise");
        assert_eq!(badge_of(&s).0, "-7h00");
        let mut none = sky_model(day, 7200, 0, 0, -20.0);
        none.apply(
            Event::Sky {
                sunrise: None,
                sunset: None,
                sun_elevation_deg: -20.0,
                moon_illumination: 0.0,
                moon_waxing: true,
                moon_phase: MoonPhase::New,
            },
            0.0,
        );
        assert_eq!(badge_of(&sun_scene(&none, 1.0)).0, "--");
    }

    #[test]
    fn moon_ring_direction_and_badge_alternation() {
        let waxing = moon_states(0.63, true, 5.0);
        assert_eq!(lit(&waxing), 38);
        assert!(matches!(waxing[0], SegState::On(..)) && matches!(waxing[37], SegState::On(..)));
        assert_eq!(waxing[38], SegState::Off);
        let waning = moon_states(0.63, false, 5.0);
        assert_eq!(lit(&waning), 38);
        assert!(matches!(waning[0], SegState::On(..)), "the top stays lit");
        assert!(matches!(waning[59], SegState::On(..)) && matches!(waning[23], SegState::On(..)));
        assert_eq!(waning[22], SegState::Off);
        assert_eq!(waning[1], SegState::Off, "nothing clockwise of the top");
        let m = sky_model(0, 0, 0, 0, 0.0);
        assert_eq!(badge_of(&moon_scene(&m, 2.5)).0, "63%");
        assert_eq!(badge_of(&moon_scene(&m, 7.5)).0, "waning");
    }

    fn iss_model(unix: i64, pass: Option<IssPass>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.apply(Event::IssPass(pass), 0.0);
        m
    }

    #[test]
    fn iss_ring_empties_toward_the_pass_and_fills_during_it() {
        assert_eq!(iss_fill(1_000, 1_000), 0.0);
        assert_eq!(iss_fill(1_000 + 6 * 3600, 1_000), 50.0);
        assert_eq!(iss_fill(1_000 + 48 * 3600, 1_000), 100.0);
        let pass = IssPass {
            start: 10_000,
            end: 10_400,
            max_elevation_deg: 62.0,
            visible: true,
        };
        let before = iss_scene(&iss_model(10_000 - 42 * 60, Some(pass)), 5.0);
        assert_eq!(badge_of(&before), ("-42m".into(), VIOLET));
        assert_eq!(before.lit_count(), 4, "42 min of 12 h");
        let during = iss_scene(&iss_model(10_100, Some(pass)), 5.0);
        assert_eq!(badge_of(&during).0, "62°");
        assert_eq!(during.lit_count(), 60);
        let after = iss_scene(&iss_model(11_000, Some(pass)), 5.0);
        assert_eq!(badge_of(&after), ("--".into(), GREY));
        assert_eq!(after.lit_count(), 0);
        assert_eq!(badge_of(&iss_scene(&iss_model(0, None), 5.0)).0, "--");
        // a pass that will not be visible is drawn dim
        let dim = iss_scene(
            &iss_model(
                10_000 - 6 * 3600,
                Some(IssPass {
                    visible: false,
                    ..pass
                }),
            ),
            5.0,
        );
        let alphas: Vec<f32> = dim
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(
                    states
                        .iter()
                        .filter_map(|s| match s {
                            SegState::On(_, a) => Some(*a),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap();
        assert!(alphas.iter().all(|a| *a <= 0.4));
        assert_eq!(icon_of(&dim), "satellite");
    }
}
```

- [ ] **Step 5: Wire in**

`lib.rs`: `pub mod scene_sky;`. `role_scene`:

```rust
        Role::Sun => crate::scene_sky::sun_scene(model, now),
        Role::Moon => crate::scene_sky::moon_scene(model, now),
        Role::Iss => crate::scene_sky::iss_scene(model, now),
```

`needs_data`:

```rust
            Role::Sun | Role::Moon => !self.sky().have,
            Role::Iss => !self.iss().have,
```

Both catch-all arms (in `role_scene` and `needs_data`) and the catch-all in `Model::apply` are now empty: delete them.

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-core && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(core): sun dial, moon phase and ISS pass roles

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Prometheus: UPS and network queries

**Files:**
- Modify: `crates/sources/src/prometheus.rs`

**Interfaces:**
- Consumes: `Event::{Ups, UpsOnBattery, UpsOnline, Network}`, existing `parse_scalar`, `parse_vector`, `query`.
- Produces: `prometheus::{ups_from, as_percent, UpsTracker}`; `poll` emits `Ups`/`Network` when the metrics exist.

- [ ] **Step 1: Failing tests**

`crates/sources/src/prometheus.rs`, `mod tests`:

```rust
    fn status(flags: &[(&str, f64)]) -> Vec<(HashMap<String, String>, f64)> {
        flags
            .iter()
            .map(|(f, v)| {
                let mut m = HashMap::new();
                m.insert("status".to_string(), f.to_string());
                m.insert("ups".to_string(), "apc".to_string());
                (m, *v)
            })
            .collect()
    }

    #[test]
    fn ups_event_reads_flags_and_normalises_fractions() {
        let ev = ups_from(
            &status(&[("OL", 1.0), ("OB", 0.0), ("LB", 0.0)]),
            Some(1.0),
            Some(0.06),
            Some(3014.0),
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 6.0,
                runtime_secs: 3014,
            }
        );
        let ev = ups_from(
            &status(&[("OL", 0.0), ("OB", 1.0), ("LB", 1.0)]),
            Some(18.0),
            Some(42.0),
            None,
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Ups {
                on_battery: true,
                low_battery: true,
                charge_pct: 18.0,
                load_pct: 42.0,
                runtime_secs: 0,
            }
        );
        assert!(ups_from(&[], Some(1.0), None, None).is_none(), "no UPS, no event");
        assert_eq!(as_percent(0.5), 50.0);
        assert_eq!(as_percent(1.0), 100.0);
        assert_eq!(as_percent(75.0), 75.0);
    }

    #[test]
    fn ups_tracker_edges_after_priming() {
        let mut t = UpsTracker::default();
        assert!(t.diff(true).is_empty(), "first poll only primes");
        assert!(t.diff(true).is_empty());
        assert_eq!(t.diff(false), vec![Event::UpsOnline]);
        assert_eq!(t.diff(true), vec![Event::UpsOnBattery]);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-sources prometheus::tests::ups`
Expected: compile error.

- [ ] **Step 3: Queries, helpers, tracker and the poll additions**

After the existing `const Q_LH_CAP` line:

```rust
const Q_UPS_STATUS: &str = "nut_ups_status";
const Q_UPS_CHARGE: &str = "nut_battery_charge";
const Q_UPS_LOAD: &str = "nut_load";
const Q_UPS_RUNTIME: &str = "nut_battery_runtime_seconds";
/// Physical interfaces only: no veth, cni, flannel or bridge traffic, which
/// would count every pod packet twice. Two minutes so a 60 s scrape interval
/// still yields two samples for `rate`.
const Q_NET_RX: &str = "sum(rate(node_network_receive_bytes_total{device=~\"eth.*|end.*|enp.*|eno.*|wlan.*\"}[2m]))*8";
const Q_NET_TX: &str = "sum(rate(node_network_transmit_bytes_total{device=~\"eth.*|end.*|enp.*|eno.*|wlan.*\"}[2m]))*8";

/// The two common nut exporters disagree: one reports charge and load as
/// 0..1, the other as 0..100. At or below 1 is a fraction.
pub fn as_percent(v: f64) -> f32 {
    if v <= 1.0 {
        (v * 100.0) as f32
    } else {
        v as f32
    }
}

/// One `Event::Ups` from the status vector and the three gauges; `None`
/// when no UPS is exported at all.
pub fn ups_from(
    status: &[(HashMap<String, String>, f64)],
    charge: Option<f64>,
    load: Option<f64>,
    runtime: Option<f64>,
) -> Option<Event> {
    if status.is_empty() {
        return None;
    }
    let flag = |f: &str| {
        status
            .iter()
            .any(|(m, v)| m.get("status").map(String::as_str) == Some(f) && *v > 0.0)
    };
    Some(Event::Ups {
        on_battery: flag("OB"),
        low_battery: flag("LB"),
        charge_pct: charge.map(as_percent).unwrap_or(0.0),
        load_pct: load.map(as_percent).unwrap_or(0.0),
        runtime_secs: runtime.unwrap_or(0.0).max(0.0) as u32,
    })
}

/// On-battery edges between polls; the first poll only primes so a restart
/// during an outage does not replay the red sweep.
#[derive(Debug, Default)]
pub struct UpsTracker {
    primed: bool,
    on_battery: bool,
}

impl UpsTracker {
    pub fn diff(&mut self, on_battery: bool) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed && on_battery != self.on_battery {
            out.push(if on_battery {
                Event::UpsOnBattery
            } else {
                Event::UpsOnline
            });
        }
        self.on_battery = on_battery;
        self.primed = true;
        out
    }
}
```

`poll` gains a fourth parameter `ups: &mut UpsTracker` and, after the Longhorn block:

```rust
    let status = parse_vector(&query(t, Q_UPS_STATUS).await?);
    if !status.is_empty() {
        let charge = parse_scalar(&query(t, Q_UPS_CHARGE).await?);
        let load = parse_scalar(&query(t, Q_UPS_LOAD).await?);
        let runtime = parse_scalar(&query(t, Q_UPS_RUNTIME).await?);
        if let Some(ev) = ups_from(&status, charge, load, runtime) {
            if let Event::Ups { on_battery, .. } = &ev {
                out.extend(ups.diff(*on_battery));
            }
            out.push(ev);
        }
    }
    let rx = parse_scalar(&query(t, Q_NET_RX).await?);
    let tx = parse_scalar(&query(t, Q_NET_TX).await?);
    if let (Some(rx_bps), Some(tx_bps)) = (rx, tx) {
        out.push(Event::Network { rx_bps, tx_bps });
    }
```

In `run_prometheus`: `let mut ups = UpsTracker::default();` next to the other trackers and `poll(&mut t, &mut alerts, &mut storage, &mut ups)`.

- [ ] **Step 4: Test, clippy, commit**

Run: `cargo test -p rackscreen-sources && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(sources): UPS and network metrics from Prometheus

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Argo CD Application watcher

**Files:**
- Modify: `crates/sources/src/k8s.rs:255-297` (`LinkEdge` gets a target)
- Create: `crates/sources/src/argocd.rs`, `crates/sources/tests/fixtures/argocd-application.json`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Consumes: `Event::{Apps, AppSynced, AppDegraded, AppHealthy, Link}`, `event::{App, AppSync, AppHealth}`, `LinkTarget::ArgoCd`, `k8s::{LinkEdge, LINK_GRACE}`.
- Produces: `k8s::LinkEdge::new_for(target: LinkTarget, grace: Duration)`, `argocd::{ArgoConfig { namespace }, parse_app(&DynamicObject) -> (App, Option<String>), AppTracker, run_argocd(client, cfg, ctx)}`.

- [ ] **Step 1: Give `LinkEdge` a target**

`crates/sources/src/k8s.rs`. Add `target: LinkTarget` to the struct; `new(grace)` keeps `LinkTarget::K8sApi`:

```rust
#[derive(Debug)]
pub struct LinkEdge {
    target: LinkTarget,
    grace: Duration,
    up: bool,
    first_err: Option<Instant>,
}

impl LinkEdge {
    pub fn new(grace: Duration) -> Self {
        Self::new_for(LinkTarget::K8sApi, grace)
    }

    pub fn new_for(target: LinkTarget, grace: Duration) -> Self {
        Self {
            target,
            grace,
            up: false,
            first_err: None,
        }
    }
```

and in `on_error` / `on_ok` replace `target: LinkTarget::K8sApi` with `target: self.target`.

Run: `cargo test -p rackscreen-sources k8s`
Expected: PASS (the existing LinkEdge tests still see `K8sApi`).

- [ ] **Step 2: Fixture**

Create `crates/sources/tests/fixtures/argocd-application.json`:

```json
{
  "apiVersion": "argoproj.io/v1alpha1",
  "kind": "Application",
  "metadata": { "name": "arr-stack", "namespace": "argocd" },
  "spec": { "project": "default" },
  "status": {
    "sync": { "status": "Synced", "revision": "abc123" },
    "health": { "status": "Healthy" },
    "operationState": { "phase": "Succeeded", "finishedAt": "2026-09-05T16:53:45Z" }
  }
}
```

- [ ] **Step 3: Failing tests**

Create `crates/sources/src/argocd.rs` with the tests first (the module body follows in Step 5):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = include_str!("../tests/fixtures/argocd-application.json");

    fn obj(sync: &str, health: &str, phase: Option<&str>) -> DynamicObject {
        let mut v: serde_json::Value = serde_json::from_str(APP).unwrap();
        v["status"]["sync"]["status"] = sync.into();
        v["status"]["health"]["status"] = health.into();
        match phase {
            Some(p) => v["status"]["operationState"]["phase"] = p.into(),
            None => {
                v["status"].as_object_mut().unwrap().remove("operationState");
            }
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn parses_the_fixture() {
        let o: DynamicObject = serde_json::from_str(APP).unwrap();
        let (app, phase) = parse_app(&o);
        assert_eq!(
            app,
            App {
                name: "arr-stack".into(),
                sync: AppSync::Synced,
                health: AppHealth::Healthy,
                operating: false,
            }
        );
        assert_eq!(phase.as_deref(), Some("Succeeded"));
        let (running, _) = parse_app(&obj("OutOfSync", "Progressing", Some("Running")));
        assert!(running.operating);
        assert_eq!(running.sync, AppSync::OutOfSync);
        assert_eq!(running.health, AppHealth::Progressing);
        let (bare, phase) = parse_app(&obj("Weird", "Odd", None));
        assert_eq!((bare.sync, bare.health, phase), (AppSync::Unknown, AppHealth::Unknown, None));
    }

    #[test]
    fn tracker_snapshots_after_init_and_emits_edges() {
        let mut t = AppTracker::default();
        t.begin_init();
        assert!(t.apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded")))).is_empty());
        let done = t.init_done();
        assert_eq!(done.len(), 1);
        assert!(matches!(&done[0], Event::Apps(list) if list.len() == 1));
        // an operation starting then finishing: a sync splash on the finish
        let evs = t.apply(parse_app(&obj("OutOfSync", "Healthy", Some("Running"))));
        assert!(matches!(evs.as_slice(), [Event::Apps(_)]));
        let evs = t.apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded"))));
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0], Event::AppSynced { name: "arr-stack".into() });
        // degraded, then back
        let evs = t.apply(parse_app(&obj("Synced", "Degraded", Some("Succeeded"))));
        assert_eq!(evs[0], Event::AppDegraded { name: "arr-stack".into() });
        let evs = t.apply(parse_app(&obj("Synced", "Degraded", Some("Succeeded"))));
        assert!(evs.is_empty(), "unchanged: nothing");
        let evs = t.apply(parse_app(&obj("Synced", "Healthy", Some("Succeeded"))));
        assert_eq!(evs[0], Event::AppHealthy { name: "arr-stack".into() });
        // deletion re-snapshots
        let evs = t.delete("arr-stack");
        assert!(matches!(evs.as_slice(), [Event::Apps(list)] if list.is_empty()));
        assert!(t.delete("arr-stack").is_empty());
    }
}
```

- [ ] **Step 4: Run to see them fail**

Add `pub mod argocd;` to `crates/sources/src/lib.rs`. Run: `cargo test -p rackscreen-sources argocd`
Expected: compile errors (`parse_app`, `AppTracker` missing).

- [ ] **Step 5: The module**

Top of `crates/sources/src/argocd.rs`, above the tests:

```rust
//! Argo CD Application watcher: sync and health per app, with edges when an
//! operation finishes, an app degrades, or it recovers.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use futures::StreamExt;
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};
use kube::runtime::watcher;
use kube::Client;
use rackscreen_core::event::{App, AppHealth, AppSync, Event, LinkTarget};
use serde_json::Value;

use crate::k8s::{LinkEdge, LINK_GRACE};
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct ArgoConfig {
    pub namespace: String,
}

/// How long to wait before looking for the CRD again when it is absent.
const CRD_RETRY: Duration = Duration::from_secs(600);

fn status_str<'a>(status: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut v = status;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_str()
}

/// The app plus the raw operation phase (`Running`, `Succeeded`, ...), which
/// the tracker needs for the sync-finished edge.
pub fn parse_app(obj: &DynamicObject) -> (App, Option<String>) {
    let status = obj.data.get("status").cloned().unwrap_or(Value::Null);
    let sync = match status_str(&status, &["sync", "status"]) {
        Some("Synced") => AppSync::Synced,
        Some("OutOfSync") => AppSync::OutOfSync,
        _ => AppSync::Unknown,
    };
    let health = match status_str(&status, &["health", "status"]) {
        Some("Healthy") => AppHealth::Healthy,
        Some("Progressing") => AppHealth::Progressing,
        Some("Degraded") => AppHealth::Degraded,
        Some("Suspended") => AppHealth::Suspended,
        Some("Missing") => AppHealth::Missing,
        _ => AppHealth::Unknown,
    };
    let phase = status_str(&status, &["operationState", "phase"]).map(str::to_string);
    let app = App {
        name: obj.metadata.name.clone().unwrap_or_default(),
        sync,
        health,
        operating: phase.as_deref() == Some("Running"),
    };
    (app, phase)
}

fn is_bad(h: AppHealth) -> bool {
    matches!(h, AppHealth::Degraded | AppHealth::Missing)
}

/// Pure state behind the watch: sorted apps, last phases, and whether the
/// initial list has completed (before that, nothing is emitted).
#[derive(Debug, Default)]
pub struct AppTracker {
    apps: BTreeMap<String, App>,
    phases: HashMap<String, String>,
    synced: bool,
}

impl AppTracker {
    pub fn begin_init(&mut self) {
        self.synced = false;
        self.apps.clear();
        self.phases.clear();
    }

    pub fn init_done(&mut self) -> Vec<Event> {
        self.synced = true;
        vec![self.snapshot()]
    }

    pub fn snapshot(&self) -> Event {
        Event::Apps(self.apps.values().cloned().collect())
    }

    pub fn apply(&mut self, (app, phase): (App, Option<String>)) -> Vec<Event> {
        let mut out = Vec::new();
        let prev = self.apps.get(&app.name).cloned();
        if self.synced {
            let prev_phase = self.phases.get(&app.name).map(String::as_str);
            if prev_phase == Some("Running") && phase.as_deref() == Some("Succeeded") {
                out.push(Event::AppSynced {
                    name: app.name.clone(),
                });
            }
            let was_bad = prev.as_ref().is_some_and(|p| is_bad(p.health));
            if is_bad(app.health) && !was_bad {
                out.push(Event::AppDegraded {
                    name: app.name.clone(),
                });
            } else if was_bad && app.health == AppHealth::Healthy {
                out.push(Event::AppHealthy {
                    name: app.name.clone(),
                });
            }
        }
        let changed = prev.as_ref() != Some(&app);
        match phase {
            Some(p) => {
                self.phases.insert(app.name.clone(), p);
            }
            None => {
                self.phases.remove(&app.name);
            }
        }
        self.apps.insert(app.name.clone(), app);
        if self.synced && changed {
            out.push(self.snapshot());
        }
        out
    }

    pub fn delete(&mut self, name: &str) -> Vec<Event> {
        self.phases.remove(name);
        if self.apps.remove(name).is_none() || !self.synced {
            return Vec::new();
        }
        vec![self.snapshot()]
    }
}

pub async fn run_argocd(client: Client, cfg: ArgoConfig, ctx: SourceCtx) {
    let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client, &cfg.namespace, &ar);
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        // The CRD may simply not be installed: say so once, retry later.
        let wait = match api.list(&ListParams::default().limit(1)).await {
            Ok(_) => None,
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                tracing::info!(
                    "argocd: no applications.argoproj.io in {}; deploys stays on no-data, retry in {}s",
                    cfg.namespace,
                    CRD_RETRY.as_secs()
                );
                Some(CRD_RETRY)
            }
            Err(e) => {
                tracing::warn!("argocd: {e}; retry in 30s");
                Some(Duration::from_secs(30))
            }
        };
        if let Some(w) = wait {
            ctx.emit(Event::Link {
                target: LinkTarget::ArgoCd,
                up: false,
            });
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(w) => {}
            }
            continue;
        }
        tracing::info!("argocd: watching applications in {}", cfg.namespace);
        let mut stream = watcher(api.clone(), watcher::Config::default().any_semantic())
            .default_backoff()
            .boxed();
        let mut tracker = AppTracker::default();
        let mut link = LinkEdge::new_for(LinkTarget::ArgoCd, LINK_GRACE);
        loop {
            let item = tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                item = stream.next() => item,
            };
            let Some(item) = item else { break };
            match item {
                Ok(ev) => {
                    let synced = matches!(ev, watcher::Event::InitDone);
                    let evs = match ev {
                        watcher::Event::Init => {
                            tracker.begin_init();
                            Vec::new()
                        }
                        watcher::Event::InitApply(o) => tracker.apply(parse_app(&o)),
                        watcher::Event::InitDone => tracker.init_done(),
                        watcher::Event::Apply(o) => tracker.apply(parse_app(&o)),
                        watcher::Event::Delete(o) => {
                            tracker.delete(o.metadata.name.as_deref().unwrap_or(""))
                        }
                    };
                    if let Some(edge) = link.on_ok(Instant::now(), synced) {
                        ctx.emit(edge);
                    }
                    ctx.emit_all(evs);
                }
                Err(e) => {
                    tracing::warn!("argocd: watch: {e}");
                    if let Some(edge) = link.on_error(Instant::now()) {
                        ctx.emit(edge);
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test -p rackscreen-sources argocd && cargo clippy --workspace --features sim,pi -- -D warnings`
Expected: PASS. If `watcher` complains about `DynamicObject`'s `DynamicType`, the `Api` already carries the `ApiResource`; the kube 4 `watcher` has no `Default` bound on it.

```bash
cargo fmt --all
git add -A
git commit -m "feat(sources): Argo CD application watcher

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Open-Meteo and Buienradar pollers

**Files:**
- Create: `crates/sources/src/open_meteo.rs`, `crates/sources/src/buienradar.rs`
- Create: `crates/sources/tests/fixtures/open-meteo-forecast.json`, `open-meteo-air-quality.json`, `buienradar-raintext.txt`
- Modify: `crates/sources/src/lib.rs`, `crates/sources/src/electricity.rs` (`token_rejected` becomes `pub`)

**Interfaces:**
- Consumes: `Event::{Weather, AirQuality, Rain, Link}`, `LinkTarget::{Weather, Rain}`, `http::client`, `electricity::{HttpStatus, backoff_secs}` (make `backoff_secs` `pub` too).
- Produces: `open_meteo::{WeatherConfig { lat, lon, poll_secs }, parse_forecast(&str) -> Result<Event>, parse_air_quality(&str) -> Result<f32>, run_weather(cfg, ctx)}`, `buienradar::{RainConfig { lat, lon, poll_secs }, mm_per_hour(u32) -> f32, parse_raintext<Tz>(&str, DateTime<Tz>) -> Result<Event>, run_rain(cfg, ctx)}`.

- [ ] **Step 1: Fixtures**

`crates/sources/tests/fixtures/open-meteo-forecast.json`:

```json
{
  "latitude": 52.366, "longitude": 4.901, "utc_offset_seconds": 0, "timezone": "UTC",
  "current_units": {"time": "iso8601", "temperature_2m": "°C", "weather_code": "wmo code", "wind_speed_10m": "km/h", "wind_direction_10m": "°", "wind_gusts_10m": "km/h", "is_day": ""},
  "current": {"time": "2026-09-07T19:45", "interval": 900, "temperature_2m": 21.2, "weather_code": 3, "wind_speed_10m": 19.1, "wind_direction_10m": 232, "wind_gusts_10m": 38.9, "is_day": 0}
}
```

`crates/sources/tests/fixtures/open-meteo-air-quality.json`:

```json
{
  "latitude": 52.4, "longitude": 4.9, "utc_offset_seconds": 0, "timezone": "GMT",
  "current_units": {"time": "iso8601", "european_aqi": "EAQI"},
  "current": {"time": "2026-09-07T19:00", "interval": 3600, "european_aqi": 40}
}
```

`crates/sources/tests/fixtures/buienradar-raintext.txt` (24 lines, `VVV|HH:MM`, five minutes apart; dry for 25 minutes, then a shower peaking at 168 = 6.9 mm/h):

```
000|21:50
000|21:55
000|22:00
000|22:05
000|22:10
077|22:15
110|22:20
141|22:25
168|22:30
150|22:35
120|22:40
090|22:45
000|22:50
000|22:55
000|23:00
000|23:05
000|23:10
000|23:15
000|23:20
000|23:25
000|23:30
000|23:35
000|23:40
000|23:45
```

- [ ] **Step 2: Failing tests for Open-Meteo**

Create `crates/sources/src/open_meteo.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = include_str!("../tests/fixtures/open-meteo-forecast.json");
    const AQ: &str = include_str!("../tests/fixtures/open-meteo-air-quality.json");

    #[test]
    fn parses_current_conditions() {
        let ev = parse_forecast(FC).unwrap();
        assert_eq!(
            ev,
            Event::Weather {
                temp_c: 21.2,
                code: 3,
                is_day: false,
                wind_kmh: 19.1,
                gust_kmh: 38.9,
                wind_from_deg: 232.0,
                at: "2026-09-07T19:45".into(),
            }
        );
        assert!(parse_forecast("{}").is_err());
        assert!(parse_forecast("{\"current\": {\"temperature_2m\": 1}}").is_err(), "code missing");
    }

    #[test]
    fn parses_air_quality() {
        assert_eq!(parse_air_quality(AQ).unwrap(), 40.0);
        assert!(parse_air_quality("{\"current\": {}}").is_err());
    }

    #[test]
    fn urls_carry_the_location() {
        let cfg = WeatherConfig {
            lat: 52.37,
            lon: 4.89,
            poll_secs: 600,
        };
        assert_eq!(
            forecast_url(&cfg),
            "https://api.open-meteo.com/v1/forecast?latitude=52.37&longitude=4.89&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC"
        );
        assert_eq!(
            air_quality_url(&cfg),
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude=52.37&longitude=4.89&current=european_aqi"
        );
    }
}
```

- [ ] **Step 3: Run to see them fail**

Add `pub mod buienradar;` and `pub mod open_meteo;` to `crates/sources/src/lib.rs`. Run: `cargo test -p rackscreen-sources open_meteo`
Expected: compile errors.

- [ ] **Step 4: The Open-Meteo module**

Above the tests in `crates/sources/src/open_meteo.rs`:

```rust
//! Open-Meteo poller: current conditions and the European air quality index.
//! No key; the free tier allows 10 000 calls a day.

use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::electricity::{backoff_secs, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct WeatherConfig {
    pub lat: f64,
    pub lon: f64,
    pub poll_secs: u64,
}

pub fn forecast_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

pub fn air_quality_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=european_aqi",
        cfg.lat, cfg.lon
    )
}

fn num(cur: &Value, key: &str) -> Result<f64> {
    cur.get(key)
        .and_then(Value::as_f64)
        .with_context(|| format!("current.{key} missing"))
}

pub fn parse_forecast(json: &str) -> Result<Event> {
    let v: Value = serde_json::from_str(json).context("forecast json")?;
    let cur = v.get("current").context("current missing")?;
    Ok(Event::Weather {
        temp_c: num(cur, "temperature_2m")? as f32,
        code: num(cur, "weather_code")? as u16,
        is_day: num(cur, "is_day").unwrap_or(1.0) > 0.0,
        wind_kmh: num(cur, "wind_speed_10m").unwrap_or(0.0) as f32,
        gust_kmh: num(cur, "wind_gusts_10m").unwrap_or(0.0) as f32,
        wind_from_deg: num(cur, "wind_direction_10m").unwrap_or(0.0) as f32,
        at: cur
            .get("time")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub fn parse_air_quality(json: &str) -> Result<f32> {
    let v: Value = serde_json::from_str(json).context("air quality json")?;
    let cur = v.get("current").context("current missing")?;
    Ok(num(cur, "european_aqi")? as f32)
}

async fn fetch(url: &str) -> Result<String> {
    let resp = client().get(url).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    Ok(body)
}

pub async fn run_weather(cfg: WeatherConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match fetch(&forecast_url(&cfg)).await.and_then(|b| parse_forecast(&b)) {
            Ok(ev) => {
                failures = 0;
                if let Event::Weather { temp_c, code, .. } = &ev {
                    tracing::info!("weather: poll ok ({temp_c:.1} °C, code {code})");
                }
                ctx.emit(Event::Link {
                    target: LinkTarget::Weather,
                    up: true,
                });
                ctx.emit(ev);
                // air quality rides along; a miss here is a warning, not a link edge
                match fetch(&air_quality_url(&cfg))
                    .await
                    .and_then(|b| parse_air_quality(&b))
                {
                    Ok(eaqi) => ctx.emit(Event::AirQuality { eaqi }),
                    Err(e) => tracing::warn!("weather: air quality: {e:#}"),
                }
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("weather: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Weather,
                        up: false,
                    });
                }
            }
        }
        let wait = backoff_secs(cfg.poll_secs, failures, false);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}
```

In `crates/sources/src/electricity.rs` make `backoff_secs` and `token_rejected` `pub` (they are reused here and in Task 13).

- [ ] **Step 5: Failing tests for Buienradar**

Create `crates/sources/src/buienradar.rs` with its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Europe::Amsterdam;

    const TXT: &str = include_str!("../tests/fixtures/buienradar-raintext.txt");

    #[test]
    fn intensity_scale() {
        assert_eq!(mm_per_hour(0), 0.0);
        assert!((mm_per_hour(77) - 0.1).abs() < 0.01);
        assert!((mm_per_hour(109) - 1.0).abs() < 0.01);
        assert!((mm_per_hour(141) - 10.0).abs() < 0.1);
        assert!((mm_per_hour(168) - 69.8).abs() < 1.0);
    }

    #[test]
    fn parses_the_fixture_into_slots_from_the_first_line() {
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 7, 21, 52, 0).unwrap();
        let Event::Rain { from, mm_per_h } = parse_raintext(TXT, now).unwrap() else {
            panic!("rain event");
        };
        assert_eq!(mm_per_h.len(), 24);
        assert_eq!(from, Amsterdam.with_ymd_and_hms(2026, 9, 7, 21, 50, 0).unwrap().timestamp());
        assert_eq!(mm_per_h[0], 0.0);
        assert!((mm_per_h[5] - 0.1).abs() < 0.01);
        assert!((mm_per_h[8] - 69.8).abs() < 1.0);
    }

    #[test]
    fn a_first_line_before_midnight_seen_after_midnight_is_yesterday() {
        let text = "000|23:55\n000|00:00\n000|00:05\n000|00:10\n000|00:15\n000|00:20\n000|00:25\n000|00:30\n000|00:35\n000|00:40\n000|00:45\n000|00:50\n";
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 8, 0, 2, 0).unwrap();
        let Event::Rain { from, .. } = parse_raintext(text, now).unwrap() else {
            panic!()
        };
        assert_eq!(from, Amsterdam.with_ymd_and_hms(2026, 9, 7, 23, 55, 0).unwrap().timestamp());
    }

    #[test]
    fn short_or_broken_input_is_an_error() {
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 7, 21, 52, 0).unwrap();
        assert!(parse_raintext("000|21:50\n000|21:55\n", now).is_err(), "fewer than 12 lines");
        assert!(parse_raintext("abc\n".repeat(24).as_str(), now).is_err());
        assert!(parse_raintext("", now).is_err());
    }

    #[test]
    fn url_rounds_the_location_to_two_decimals() {
        let cfg = RainConfig {
            lat: 52.3702,
            lon: 4.8952,
            poll_secs: 300,
        };
        assert_eq!(
            raintext_url(&cfg),
            "https://gpsgadget.buienradar.nl/data/raintext?lat=52.37&lon=4.90"
        );
    }
}
```

- [ ] **Step 6: Run to see them fail**

Run: `cargo test -p rackscreen-sources buienradar`
Expected: compile errors.

- [ ] **Step 7: The Buienradar module**

Above the tests in `crates/sources/src/buienradar.rs`:

```rust
//! Buienradar rain nowcast: 24 five-minute slots, two hours ahead, NL and BE.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, TimeZone};
use rackscreen_core::event::{Event, LinkTarget};

use crate::electricity::{backoff_secs, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct RainConfig {
    pub lat: f64,
    pub lon: f64,
    pub poll_secs: u64,
}

/// Fewer lines than this and the feed is broken rather than short.
const MIN_LINES: usize = 12;

pub fn raintext_url(cfg: &RainConfig) -> String {
    format!(
        "https://gpsgadget.buienradar.nl/data/raintext?lat={:.2}&lon={:.2}",
        cfg.lat, cfg.lon
    )
}

/// Buienradar's 0..255 scale: `10^((v - 109) / 32)` mm/h, and 0 is dry.
pub fn mm_per_hour(v: u32) -> f32 {
    if v == 0 {
        0.0
    } else {
        10f32.powf((v as f32 - 109.0) / 32.0)
    }
}

/// Lines `VVV|HH:MM` into `Event::Rain`. The first line's clock time is
/// placed on `now`'s local day, or the day before when it reads later than
/// an hour past `now` (the feed straddling midnight).
pub fn parse_raintext<Tz: TimeZone>(text: &str, now: DateTime<Tz>) -> Result<Event> {
    let mut mm = Vec::new();
    let mut first: Option<NaiveTime> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (v, t) = line
            .split_once('|')
            .ok_or_else(|| anyhow!("raintext line without '|': {line:?}"))?;
        let v: u32 = v.trim().parse().with_context(|| format!("raintext value {v:?}"))?;
        let t = NaiveTime::parse_from_str(t.trim(), "%H:%M")
            .with_context(|| format!("raintext time {t:?}"))?;
        first.get_or_insert(t);
        mm.push(mm_per_hour(v));
    }
    let first = first.ok_or_else(|| anyhow!("raintext is empty"))?;
    anyhow::ensure!(
        mm.len() >= MIN_LINES,
        "raintext has {} lines, expected at least {MIN_LINES}",
        mm.len()
    );
    let today = now.date_naive();
    let mut from = now
        .timezone()
        .from_local_datetime(&today.and_time(first))
        .single()
        .ok_or_else(|| anyhow!("ambiguous local time"))?;
    if from > now.clone() + ChronoDuration::hours(1) {
        from -= ChronoDuration::days(1);
    }
    Ok(Event::Rain {
        from: from.timestamp(),
        mm_per_h: mm,
    })
}

async fn fetch(url: &str) -> Result<String> {
    let resp = client().get(url).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    Ok(body)
}

pub async fn run_rain(cfg: RainConfig, ctx: SourceCtx) {
    let url = raintext_url(&cfg);
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match fetch(&url)
            .await
            .and_then(|b| parse_raintext(&b, chrono::Local::now()))
        {
            Ok(ev) => {
                failures = 0;
                if let Event::Rain { mm_per_h, .. } = &ev {
                    let wet = mm_per_h.iter().filter(|v| **v >= 0.1).count();
                    tracing::info!("rain: poll ok ({wet} wet slots of {})", mm_per_h.len());
                }
                ctx.emit(Event::Link {
                    target: LinkTarget::Rain,
                    up: true,
                });
                ctx.emit(ev);
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("rain: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Rain,
                        up: false,
                    });
                }
            }
        }
        let wait = backoff_secs(cfg.poll_secs, failures, false);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}
```

- [ ] **Step 8: Test, clippy, commit**

Run: `cargo test -p rackscreen-sources && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(sources): Open-Meteo weather and air quality, Buienradar rain nowcast

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: Sun, moon and ISS computed on the Pi

**Files:**
- Create: `crates/sources/src/astro.rs`, `crates/sources/src/iss.rs`, `crates/sources/tests/fixtures/iss.tle`
- Modify: `crates/sources/Cargo.toml` (`sgp4 = "2"`), `crates/sources/src/lib.rs`

**Interfaces:**
- Consumes: `Event::{Sky, IssPass}`, `event::{IssPass, MoonPhase}`.
- Produces: `astro::{sun_times(lat, lon, local_midnight_unix) -> (Option<i64>, Option<i64>), sun_elevation(lat, lon, unix) -> f64, sun_direction(unix) -> [f64; 3], moon(unix) -> (f32, bool, MoonPhase), sky_event(lat, lon, unix, utc_offset_secs) -> Event, run_astro(lat, lon, ctx)}`, `iss::{IssConfig { lat, lon, min_elevation }, parse_tle(&str) -> Result<(String, String)>, next_pass(line1, line2, lat, lon, min_elevation, from_unix) -> Result<Option<IssPass>>, run_iss(cfg, ctx)}`.

- [ ] **Step 1: Failing astro tests**

Create `crates/sources/src/astro.rs` with the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const AMS: (f64, f64) = (52.37, 4.89);
    const JUN21_UTC: i64 = 1_782_000_000; // 2026-06-21T00:00Z
    const DEC21_UTC: i64 = 1_797_811_200; // 2026-12-21T00:00Z

    fn close(a: i64, b: i64, tol: i64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn amsterdam_sunrise_and_sunset_on_the_solstices() {
        // local midnight CEST is 22:00Z the day before
        let (rise, set) = sun_times(AMS.0, AMS.1, JUN21_UTC - 7200);
        // 05:18 CEST = 03:18Z, 22:05 CEST = 20:05Z, within 15 minutes
        assert!(close(rise.unwrap(), JUN21_UTC + 3 * 3600 + 18 * 60, 900), "{rise:?}");
        assert!(close(set.unwrap(), JUN21_UTC + 20 * 3600 + 5 * 60, 900), "{set:?}");
        // CET: local midnight is 23:00Z the day before
        let (rise, set) = sun_times(AMS.0, AMS.1, DEC21_UTC - 3600);
        // 08:47 CET = 07:47Z, 16:31 CET = 15:31Z
        assert!(close(rise.unwrap(), DEC21_UTC + 7 * 3600 + 47 * 60, 900), "{rise:?}");
        assert!(close(set.unwrap(), DEC21_UTC + 15 * 3600 + 31 * 60, 900), "{set:?}");
    }

    #[test]
    fn polar_day_and_night_have_no_events() {
        assert_eq!(sun_times(80.0, 20.0, JUN21_UTC), (None, None));
        assert_eq!(sun_times(80.0, 20.0, DEC21_UTC), (None, None));
    }

    #[test]
    fn sun_elevation_is_high_at_noon_and_negative_at_midnight() {
        let noon = sun_elevation(AMS.0, AMS.1, JUN21_UTC + 11 * 3600 + 40 * 60);
        assert!(noon > 60.0 && noon < 62.5, "{noon}");
        let night = sun_elevation(AMS.0, AMS.1, JUN21_UTC);
        assert!(night < -10.0, "{night}");
        let d = sun_direction(JUN21_UTC);
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
        assert!(d[2] > 0.38, "the sun is far north of the equator in June: {d:?}");
    }

    #[test]
    fn moon_phases_on_known_dates() {
        // full moon 2026-01-03 10:03Z, new moon 2026-01-18 19:52Z
        let (illum, waxing, phase) = moon(1_767_434_400);
        assert!(illum > 0.9, "{illum}");
        assert_eq!(phase, MoonPhase::Full);
        let (illum, _, phase) = moon(1_768_765_920);
        assert!(illum < 0.1, "{illum}");
        assert_eq!(phase, MoonPhase::New);
        // a week after new: waxing, about half
        let (illum, waxing2, phase) = moon(1_768_765_920 + 7 * 86_400 + 12 * 3600);
        assert!(illum > 0.4 && illum < 0.6, "{illum}");
        assert!(waxing2);
        assert_eq!(phase, MoonPhase::FirstQuarter);
        let _ = waxing;
    }

    #[test]
    fn sky_event_uses_tomorrows_sunrise_after_sunset() {
        // 2026-06-21 22:30 local (20:30Z): sunset has passed
        let now = JUN21_UTC + 20 * 3600 + 30 * 60;
        let Event::Sky {
            sunrise, sunset, ..
        } = sky_event(AMS.0, AMS.1, now, 7200)
        else {
            panic!()
        };
        assert!(sunset.unwrap() < now, "today's sunset is in the past");
        assert!(sunrise.unwrap() > now, "so the sunrise offered is tomorrow's");
        assert!(sunrise.unwrap() - now < 8 * 3600);
    }
}
```

- [ ] **Step 2: Run to see them fail**

Add `pub mod astro;` and `pub mod iss;` to `lib.rs` (create an empty `iss.rs` for now). Run: `cargo test -p rackscreen-sources astro`
Expected: compile errors.

- [ ] **Step 3: The astro module**

Above the tests in `crates/sources/src/astro.rs`:

```rust
//! Sun and moon without a network: NOAA's solar position algorithm and a
//! mean-synodic moon, both good to a minute or two, which is all a ring needs.

use std::f64::consts::PI;
use std::time::Duration;

use rackscreen_core::event::{Event, MoonPhase};

use crate::SourceCtx;

const SYNODIC_DAYS: f64 = 29.530_588_853;
/// A reference new moon: 2000-01-06 18:14 UTC.
const NEW_MOON_REF: i64 = 947_182_440;
/// Sunrise and sunset are the sun's centre at this zenith (refraction included).
const RISE_SET_ZENITH_DEG: f64 = 90.833;

fn julian_day(unix: f64) -> f64 {
    unix / 86_400.0 + 2_440_587.5
}

/// Solar declination (deg), equation of time (minutes) and the apparent
/// longitude / obliquity (deg) at a unix time. NOAA general solar position.
fn solar(unix: f64) -> (f64, f64, f64, f64) {
    let jc = (julian_day(unix) - 2_451_545.0) / 36_525.0;
    let l0 = (280.466_46 + jc * (36_000.769_83 + jc * 0.000_303_2)).rem_euclid(360.0);
    let m = 357.529_11 + jc * (35_999.050_29 - 0.000_153_7 * jc);
    let e = 0.016_708_634 - jc * (0.000_042_037 + 0.000_000_126_7 * jc);
    let mr = m.to_radians();
    let c = mr.sin() * (1.914_602 - jc * (0.004_817 + 0.000_014 * jc))
        + (2.0 * mr).sin() * (0.019_993 - 0.000_101 * jc)
        + (3.0 * mr).sin() * 0.000_289;
    let true_long = l0 + c;
    let omega = (125.04 - 1_934.136 * jc).to_radians();
    let lambda = true_long - 0.005_69 - 0.004_78 * omega.sin();
    let eps0 = 23.0 + (26.0 + (21.448 - jc * (46.815 + jc * (0.000_59 - jc * 0.001_813))) / 60.0) / 60.0;
    let eps = eps0 + 0.002_56 * omega.cos();
    let decl = (eps.to_radians().sin() * lambda.to_radians().sin()).asin().to_degrees();
    let y = (eps.to_radians() / 2.0).tan().powi(2);
    let l0r = l0.to_radians();
    let eot = 4.0
        * (y * (2.0 * l0r).sin() - 2.0 * e * mr.sin() + 4.0 * e * y * mr.sin() * (2.0 * l0r).cos()
            - 0.5 * y * y * (4.0 * l0r).sin()
            - 1.25 * e * e * (2.0 * mr).sin())
        .to_degrees();
    (decl, eot, lambda, eps)
}

/// Sunrise and sunset (unix seconds) for the local day starting at
/// `local_midnight` (unix seconds). `None` for polar day or night.
pub fn sun_times(lat: f64, lon: f64, local_midnight: i64) -> (Option<i64>, Option<i64>) {
    let local_noon = local_midnight + 43_200;
    let utc_day = local_noon.div_euclid(86_400) * 86_400;
    let (decl, eot, _, _) = solar(local_noon as f64);
    let (latr, dr) = (lat.to_radians(), decl.to_radians());
    let cos_ha = RISE_SET_ZENITH_DEG.to_radians().cos() / (latr.cos() * dr.cos()) - latr.tan() * dr.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return (None, None);
    }
    let ha_min = cos_ha.acos().to_degrees() * 4.0;
    let noon_min = 720.0 - 4.0 * lon - eot;
    let at = |min: f64| utc_day + (min * 60.0).round() as i64;
    (Some(at(noon_min - ha_min)), Some(at(noon_min + ha_min)))
}

/// Sun elevation above the horizon in degrees, no refraction.
pub fn sun_elevation(lat: f64, lon: f64, unix: i64) -> f64 {
    let (decl, eot, _, _) = solar(unix as f64);
    let minutes = (unix.rem_euclid(86_400)) as f64 / 60.0;
    let tst = (minutes + eot + 4.0 * lon).rem_euclid(1_440.0);
    let ha = (tst / 4.0 - 180.0).to_radians();
    let (latr, dr) = (lat.to_radians(), decl.to_radians());
    (latr.sin() * dr.sin() + latr.cos() * dr.cos() * ha.cos())
        .asin()
        .to_degrees()
}

/// Unit vector toward the sun in equatorial (TEME-compatible) coordinates.
pub fn sun_direction(unix: i64) -> [f64; 3] {
    let (_, _, lambda, eps) = solar(unix as f64);
    let (l, e) = (lambda.to_radians(), eps.to_radians());
    [l.cos(), e.cos() * l.sin(), e.sin() * l.sin()]
}

/// Illuminated fraction, whether the moon is waxing, and the phase name.
pub fn moon(unix: i64) -> (f32, bool, MoonPhase) {
    let age = ((unix - NEW_MOON_REF) as f64 / 86_400.0).rem_euclid(SYNODIC_DAYS);
    let p = age / SYNODIC_DAYS;
    let illumination = (1.0 - (2.0 * PI * p).cos()) / 2.0;
    let phase = match p {
        p if p < 0.0625 => MoonPhase::New,
        p if p < 0.1875 => MoonPhase::WaxingCrescent,
        p if p < 0.3125 => MoonPhase::FirstQuarter,
        p if p < 0.4375 => MoonPhase::WaxingGibbous,
        p if p < 0.5625 => MoonPhase::Full,
        p if p < 0.6875 => MoonPhase::WaningGibbous,
        p if p < 0.8125 => MoonPhase::LastQuarter,
        p if p < 0.9375 => MoonPhase::WaningCrescent,
        _ => MoonPhase::New,
    };
    (illumination as f32, p < 0.5, phase)
}

/// The `Sky` event for `unix` at an observer: today's sunrise and sunset in
/// the local day, or tomorrow's sunrise once today's sunset has passed.
pub fn sky_event(lat: f64, lon: f64, unix: i64, utc_offset_secs: i32) -> Event {
    let offset = utc_offset_secs as i64;
    let local_midnight = (unix + offset).div_euclid(86_400) * 86_400 - offset;
    let (mut sunrise, sunset) = sun_times(lat, lon, local_midnight);
    if sunset.is_some_and(|s| s < unix) {
        sunrise = sun_times(lat, lon, local_midnight + 86_400).0;
    }
    let (moon_illumination, moon_waxing, moon_phase) = moon(unix);
    Event::Sky {
        sunrise,
        sunset,
        sun_elevation_deg: sun_elevation(lat, lon, unix) as f32,
        moon_illumination,
        moon_waxing,
        moon_phase,
    }
}

pub async fn run_astro(lat: f64, lon: f64, ctx: SourceCtx) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = ticker.tick() => {
                let now = chrono::Local::now();
                let offset = now.offset().local_minus_utc();
                ctx.emit(sky_event(lat, lon, now.timestamp(), offset));
            }
        }
    }
}
```

`chrono::Offset::local_minus_utc` needs `use chrono::Offset;` at the top.

Run: `cargo test -p rackscreen-sources astro`
Expected: PASS. If a solstice assertion misses by more than 15 minutes, the sign of `4.0 * lon` is the usual culprit (east longitudes are positive here, and the NOAA sheet subtracts `4 * lon` from 720 for solar noon in UTC).

- [ ] **Step 4: Fixture and failing ISS tests**

`crates/sources/tests/fixtures/iss.tle` (Celestrak, epoch 2026-09-07 ~12:00Z):

```
ISS (ZARYA)
1 25544U 98067A   26250.49846501  .00005306  00000+0  10435-3 0  9991
2 25544  51.6306 252.7093 0004983 115.8922 244.2580 15.49018229584512
```

`crates/sources/src/iss.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TLE: &str = include_str!("../tests/fixtures/iss.tle");
    /// 2026-09-07T12:00Z, the TLE epoch.
    const EPOCH: i64 = 1_788_782_400;

    #[test]
    fn parses_the_two_lines() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        assert!(l1.starts_with("1 25544U"));
        assert!(l2.starts_with("2 25544 "));
        assert!(parse_tle("ISS\n1 nope\n").is_err());
        assert!(parse_tle("").is_err());
    }

    #[test]
    fn iss_is_at_orbital_altitude() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let r = position_teme(&l1, &l2, EPOCH + 600).unwrap();
        let alt = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() - EARTH_RADIUS_KM;
        assert!(alt > 380.0 && alt < 460.0, "altitude {alt} km");
    }

    #[test]
    fn a_pass_over_amsterdam_exists_within_a_day() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let pass = next_pass(&l1, &l2, 52.37, 4.89, 10.0, EPOCH)
            .unwrap()
            .expect("the ISS passes Amsterdam several times a day");
        assert!(pass.start >= EPOCH && pass.end > pass.start);
        assert!(pass.end - pass.start < 15 * 60, "passes are minutes long");
        assert!(pass.max_elevation_deg >= 10.0 && pass.max_elevation_deg <= 90.0);
        // a pass is where the elevation really clears the threshold
        let mid = (pass.start + pass.end) / 2;
        let el = elevation_deg(&l1, &l2, 52.37, 4.89, mid).unwrap();
        assert!(el > 5.0, "mid-pass elevation {el}");
        // the next pass after this one starts after it ends
        let later = next_pass(&l1, &l2, 52.37, 4.89, 10.0, pass.end + 1)
            .unwrap()
            .unwrap();
        assert!(later.start > pass.end);
        // an impossible threshold finds nothing
        assert!(next_pass(&l1, &l2, 52.37, 4.89, 89.9, EPOCH).unwrap().is_none());
    }

    #[test]
    fn observer_and_frames() {
        let o = observer_ecef(0.0, 0.0);
        assert!((o[0] - 6378.137).abs() < 0.01 && o[1].abs() < 1e-6 && o[2].abs() < 1e-6);
        let p = observer_ecef(90.0, 0.0);
        assert!((p[2] - 6356.75).abs() < 0.1, "{p:?}");
        // GMST at J2000 noon is about 18.697 h = 280.46°
        let g = gmst_rad(946_728_000.0).to_degrees();
        assert!((g - 280.46).abs() < 0.05, "{g}");
        // a point straight above the observer has elevation 90
        let obs = observer_ecef(52.37, 4.89);
        let up = [obs[0] * 1.06, obs[1] * 1.06, obs[2] * 1.06];
        assert!((topocentric_elevation(up, obs, 52.37, 4.89) - 90.0).abs() < 0.5);
    }
}
```

- [ ] **Step 5: Run to see them fail**

Add to `crates/sources/Cargo.toml` `[dependencies]`: `sgp4 = "2"`. Run: `cargo test -p rackscreen-sources iss`
Expected: compile errors.

- [ ] **Step 6: The ISS module**

Above the tests in `crates/sources/src/iss.rs`:

```rust
//! ISS pass prediction on the Pi: the TLE from Celestrak once a day, SGP4
//! propagation, a topocentric elevation search for the next pass.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use rackscreen_core::event::{Event, IssPass};

use crate::astro::{sun_direction, sun_elevation};
use crate::electricity::HttpStatus;
use crate::http::client;
use crate::SourceCtx;

const TLE_URL: &str = "https://celestrak.org/NORAD/elements/gp.php?CATNR=25544&FORMAT=TLE";
pub const EARTH_RADIUS_KM: f64 = 6_371.0;
const WGS84_A: f64 = 6_378.137;
const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// Propagation step of the pass search.
const STEP_SECS: i64 = 10;
/// How far ahead to look for a pass.
const HORIZON_SECS: i64 = 24 * 3600;
/// The TLE is fetched again after this long, and used for up to a week when
/// the fetch keeps failing.
const TLE_REFRESH: Duration = Duration::from_secs(24 * 3600);
const TLE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// Passes are recomputed this often, and right after one ends.
const RECOMPUTE: Duration = Duration::from_secs(600);
/// The observer counts as dark below this sun elevation (civil twilight).
const DARK_DEG: f64 = -6.0;

#[derive(Clone, Debug)]
pub struct IssConfig {
    pub lat: f64,
    pub lon: f64,
    pub min_elevation: f32,
}

/// The two element lines out of a Celestrak TLE response.
pub fn parse_tle(text: &str) -> Result<(String, String)> {
    let l1 = text
        .lines()
        .map(str::trim_end)
        .find(|l| l.starts_with("1 ") && l.len() >= 69)
        .ok_or_else(|| anyhow!("TLE line 1 missing"))?;
    let l2 = text
        .lines()
        .map(str::trim_end)
        .find(|l| l.starts_with("2 ") && l.len() >= 69)
        .ok_or_else(|| anyhow!("TLE line 2 missing"))?;
    Ok((l1.to_string(), l2.to_string()))
}

fn elements(l1: &str, l2: &str) -> Result<(sgp4::Elements, sgp4::Constants)> {
    let el = sgp4::Elements::from_tle(Some("ISS".into()), l1.as_bytes(), l2.as_bytes())
        .map_err(|e| anyhow!("tle: {e}"))?;
    let k = sgp4::Constants::from_elements(&el).map_err(|e| anyhow!("sgp4: {e}"))?;
    Ok((el, k))
}

/// Satellite position in TEME kilometres at a unix time.
pub fn position_teme(l1: &str, l2: &str, unix: i64) -> Result<[f64; 3]> {
    let (el, k) = elements(l1, l2)?;
    propagate(&el, &k, unix)
}

fn propagate(el: &sgp4::Elements, k: &sgp4::Constants, unix: i64) -> Result<[f64; 3]> {
    let dt = chrono::DateTime::from_timestamp(unix, 0)
        .context("timestamp")?
        .naive_utc();
    let t = el
        .datetime_to_minutes_since_epoch(&dt)
        .map_err(|e| anyhow!("epoch: {e}"))?;
    let p = k.propagate(t).map_err(|e| anyhow!("propagate: {e}"))?;
    Ok(p.position)
}

/// Greenwich mean sidereal time in radians.
pub fn gmst_rad(unix: f64) -> f64 {
    let jd = unix / 86_400.0 + 2_440_587.5;
    let d = jd - 2_451_545.0;
    let t = d / 36_525.0;
    let g = 280.460_618_37 + 360.985_647_366_29 * d + 0.000_387_933 * t * t - t * t * t / 38_710_000.0;
    g.rem_euclid(360.0).to_radians()
}

fn teme_to_ecef(r: [f64; 3], gmst: f64) -> [f64; 3] {
    let (s, c) = gmst.sin_cos();
    [r[0] * c + r[1] * s, -r[0] * s + r[1] * c, r[2]]
}

/// WGS84 observer position at sea level, kilometres.
pub fn observer_ecef(lat: f64, lon: f64) -> [f64; 3] {
    let (latr, lonr) = (lat.to_radians(), lon.to_radians());
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let n = WGS84_A / (1.0 - e2 * latr.sin().powi(2)).sqrt();
    [
        n * latr.cos() * lonr.cos(),
        n * latr.cos() * lonr.sin(),
        n * (1.0 - e2) * latr.sin(),
    ]
}

/// Elevation of an ECEF point above the observer's horizon, degrees.
pub fn topocentric_elevation(sat: [f64; 3], obs: [f64; 3], lat: f64, lon: f64) -> f64 {
    let (latr, lonr) = (lat.to_radians(), lon.to_radians());
    let d = [sat[0] - obs[0], sat[1] - obs[1], sat[2] - obs[2]];
    let e = -lonr.sin() * d[0] + lonr.cos() * d[1];
    let n = -latr.sin() * lonr.cos() * d[0] - latr.sin() * lonr.sin() * d[1] + latr.cos() * d[2];
    let u = latr.cos() * lonr.cos() * d[0] + latr.cos() * lonr.sin() * d[1] + latr.sin() * d[2];
    u.atan2((e * e + n * n).sqrt()).to_degrees()
}

/// Elevation of the ISS from the observer at a unix time.
pub fn elevation_deg(l1: &str, l2: &str, lat: f64, lon: f64, unix: i64) -> Result<f64> {
    let r = position_teme(l1, l2, unix)?;
    let sat = teme_to_ecef(r, gmst_rad(unix as f64));
    Ok(topocentric_elevation(sat, observer_ecef(lat, lon), lat, lon))
}

/// The satellite is lit when it is on the sun's side of the Earth's centre or
/// outside the Earth's shadow cylinder.
fn sunlit(r_teme: [f64; 3], unix: i64) -> bool {
    let s = sun_direction(unix);
    let along = r_teme[0] * s[0] + r_teme[1] * s[1] + r_teme[2] * s[2];
    if along > 0.0 {
        return true;
    }
    let perp = [
        r_teme[0] - along * s[0],
        r_teme[1] - along * s[1],
        r_teme[2] - along * s[2],
    ];
    (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt() > EARTH_RADIUS_KM
}

/// The first pass above `min_elevation` that ends after `from`, searched in
/// ten-second steps over the next 24 hours.
pub fn next_pass(
    l1: &str,
    l2: &str,
    lat: f64,
    lon: f64,
    min_elevation: f32,
    from: i64,
) -> Result<Option<IssPass>> {
    let (el, k) = elements(l1, l2)?;
    let obs = observer_ecef(lat, lon);
    let min = min_elevation as f64;
    let mut t = from;
    let mut start: Option<i64> = None;
    let mut max_el = f64::MIN;
    let mut max_at = from;
    let mut max_r = [0.0; 3];
    while t <= from + HORIZON_SECS {
        let r = propagate(&el, &k, t)?;
        let elv = topocentric_elevation(teme_to_ecef(r, gmst_rad(t as f64)), obs, lat, lon);
        if elv >= min {
            if start.is_none() {
                start = Some(t);
                max_el = f64::MIN;
            }
            if elv > max_el {
                max_el = elv;
                max_at = t;
                max_r = r;
            }
        } else if let Some(s) = start {
            let visible = sun_elevation(lat, lon, max_at) < DARK_DEG && sunlit(max_r, max_at);
            return Ok(Some(IssPass {
                start: s,
                end: t,
                max_elevation_deg: max_el as f32,
                visible,
            }));
        }
        t += STEP_SECS;
    }
    Ok(None)
}

async fn fetch_tle() -> Result<(String, String)> {
    let resp = client().get(TLE_URL).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    parse_tle(&body)
}

pub async fn run_iss(cfg: IssConfig, ctx: SourceCtx) {
    let mut tle: Option<(String, String)> = None;
    let mut fetched = std::time::Instant::now();
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let stale = tle.is_none() || fetched.elapsed() >= TLE_REFRESH;
        if stale {
            match fetch_tle().await {
                Ok(t) => {
                    tracing::info!("iss: TLE refreshed");
                    tle = Some(t);
                    fetched = std::time::Instant::now();
                }
                Err(e) => {
                    tracing::warn!("iss: TLE fetch: {e:#}");
                    if fetched.elapsed() >= TLE_MAX_AGE {
                        tle = None;
                    }
                }
            }
        }
        let mut wait = RECOMPUTE;
        match &tle {
            None => ctx.emit(Event::IssPass(None)),
            Some((l1, l2)) => {
                let now = chrono::Utc::now().timestamp();
                match next_pass(l1, l2, cfg.lat, cfg.lon, cfg.min_elevation, now) {
                    Ok(pass) => {
                        if let Some(p) = pass {
                            tracing::info!(
                                "iss: next pass in {} min, max {:.0}°{}",
                                (p.start - now) / 60,
                                p.max_elevation_deg,
                                if p.visible { ", visible" } else { "" }
                            );
                            // recompute right after the pass ends
                            let until_end = Duration::from_secs((p.end - now).max(1) as u64 + 5);
                            wait = wait.min(until_end);
                        }
                        ctx.emit(Event::IssPass(pass));
                    }
                    Err(e) => tracing::warn!("iss: {e:#}"),
                }
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
        }
    }
}
```

Run: `cargo test -p rackscreen-sources iss`
Expected: PASS. The `sgp4` 2.x API used: `Elements::from_tle(Option<String>, &[u8], &[u8])`, `Elements::datetime_to_minutes_since_epoch(&NaiveDateTime)`, `Constants::from_elements(&Elements)`, `Constants::propagate(MinutesSinceEpoch) -> Prediction { position: [f64; 3], velocity }`.

- [ ] **Step 7: Clippy for `pi`-only and `sim`-only too, commit**

Run: `cargo test -p rackscreen-sources && cargo clippy --workspace --features sim,pi -- -D warnings && cargo clippy --no-default-features --features pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(sources): sun, moon and ISS passes computed on the Pi

NOAA solar position for sunrise, sunset and elevation, a mean-synodic
moon, and SGP4 propagation of the Celestrak TLE for the next ISS pass.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 13: GitHub poller

**Files:**
- Create: `crates/sources/src/github.rs`, `crates/sources/tests/fixtures/github-calendar.json`, `github-events.json`, `github-runs.json`
- Modify: `crates/sources/src/lib.rs`

**Interfaces:**
- Consumes: `Event::{GithubActivity, GithubPush, GithubStar, GithubMerge, GithubRelease, GithubRun, Link}`, `LinkTarget::Github`, `electricity::{HttpStatus, token_rejected, backoff_secs}`.
- Produces: `github::{GithubConfig { token, poll_secs }, window_days(today: NaiveDate) -> Vec<NaiveDate>, parse_calendar(&str, &[NaiveDate]) -> Result<(String, Vec<(String, u32)>)>, GhEvent, GhKind, parse_events(&str) -> Result<Vec<GhEvent>>, EventTracker, Run, parse_runs(&str) -> Result<Vec<Run>>, RunTracker, pushed_repos(&[GhEvent]) -> Vec<String>, run_github(cfg, ctx)}`.

- [ ] **Step 1: Fixtures**

`crates/sources/tests/fixtures/github-calendar.json` (two weeks is enough to test the mapping; missing days are zero):

```json
{
  "data": {
    "viewer": {
      "login": "SilkePilon",
      "contributionsCollection": {
        "contributionCalendar": {
          "weeks": [
            { "contributionDays": [
              { "date": "2026-08-30", "contributionCount": 0 },
              { "date": "2026-08-31", "contributionCount": 0 },
              { "date": "2026-09-01", "contributionCount": 0 },
              { "date": "2026-09-02", "contributionCount": 0 },
              { "date": "2026-09-03", "contributionCount": 0 },
              { "date": "2026-09-04", "contributionCount": 0 },
              { "date": "2026-09-05", "contributionCount": 25 }
            ] },
            { "contributionDays": [
              { "date": "2026-09-06", "contributionCount": 43 },
              { "date": "2026-09-07", "contributionCount": 28 }
            ] }
          ]
        }
      }
    }
  }
}
```

`crates/sources/tests/fixtures/github-events.json`:

```json
[
  { "id": "1005", "type": "ReleaseEvent", "repo": { "name": "silkepilon/RackScreen" },
    "payload": { "action": "published", "release": { "tag_name": "v0.3.0" } }, "created_at": "2026-09-07T07:03:54Z" },
  { "id": "1004", "type": "PullRequestEvent", "repo": { "name": "silkepilon/homelab" },
    "payload": { "action": "closed", "pull_request": { "merged": true } }, "created_at": "2026-09-07T06:50:00Z" },
  { "id": "1003", "type": "PullRequestEvent", "repo": { "name": "silkepilon/homelab" },
    "payload": { "action": "closed", "pull_request": { "merged": false } }, "created_at": "2026-09-07T06:40:00Z" },
  { "id": "1002", "type": "WatchEvent", "repo": { "name": "silkepilon/mineflayer-baritone" },
    "payload": { "action": "started" }, "created_at": "2026-09-07T06:30:00Z" },
  { "id": "1001", "type": "PushEvent", "repo": { "name": "silkepilon/RackScreen" },
    "payload": { "commits": [ { "sha": "a" }, { "sha": "b" }, { "sha": "c" } ] }, "created_at": "2026-09-07T06:19:14Z" },
  { "id": "1000", "type": "CreateEvent", "repo": { "name": "silkepilon/resend-docs" },
    "payload": { "ref_type": "branch" }, "created_at": "2026-09-07T06:00:00Z" }
]
```

`crates/sources/tests/fixtures/github-runs.json`:

```json
{
  "total_count": 3,
  "workflow_runs": [
    { "id": 301, "name": "ci", "status": "in_progress", "conclusion": null },
    { "id": 300, "name": "ci", "status": "completed", "conclusion": "failure" },
    { "id": 299, "name": "release", "status": "completed", "conclusion": "success" }
  ]
}
```

- [ ] **Step 2: Failing tests**

`crates/sources/src/github.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const CAL: &str = include_str!("../tests/fixtures/github-calendar.json");
    const EVENTS: &str = include_str!("../tests/fixtures/github-events.json");
    const RUNS: &str = include_str!("../tests/fixtures/github-runs.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn window_is_thirty_days_ending_today() {
        let w = window_days(d("2026-09-07"));
        assert_eq!(w.len(), 30);
        assert_eq!(w[0], d("2026-08-09"));
        assert_eq!(w[29], d("2026-09-07"));
    }

    #[test]
    fn calendar_maps_onto_the_window_with_zeros_for_missing_days() {
        let (login, days) = parse_calendar(CAL, &window_days(d("2026-09-07"))).unwrap();
        assert_eq!(login, "SilkePilon");
        assert_eq!(days.len(), 30);
        assert_eq!(days[29], ("2026-09-07".to_string(), 28));
        assert_eq!(days[28].1, 43);
        assert_eq!(days[27].1, 25);
        assert_eq!(days[0], ("2026-08-09".to_string(), 0));
        assert!(parse_calendar("{\"errors\":[{\"message\":\"Bad credentials\"}]}", &[]).is_err());
    }

    #[test]
    fn events_map_to_kinds_and_skip_the_rest() {
        let evs = parse_events(EVENTS).unwrap();
        assert_eq!(evs.len(), 4, "CreateEvent and the unmerged PR are ignored");
        assert_eq!(evs[0].kind, GhKind::Release("v0.3.0".into()));
        assert_eq!(evs[1].kind, GhKind::Merge);
        assert_eq!(evs[2].kind, GhKind::Star);
        assert_eq!(evs[3], GhEvent {
            id: "1001".into(),
            repo: "silkepilon/RackScreen".into(),
            kind: GhKind::Push(3),
        });
        assert_eq!(pushed_repos(&evs), vec!["silkepilon/RackScreen".to_string()]);
    }

    #[test]
    fn event_tracker_primes_then_emits_only_new_ids() {
        let evs = parse_events(EVENTS).unwrap();
        let mut t = EventTracker::default();
        assert!(t.diff(&evs).is_empty(), "first page only primes");
        assert!(t.diff(&evs).is_empty());
        let mut newer = vec![GhEvent {
            id: "1006".into(),
            repo: "silkepilon/RackScreen".into(),
            kind: GhKind::Push(1),
        }];
        newer.extend(evs.iter().cloned());
        let out = t.diff(&newer);
        assert_eq!(
            out,
            vec![Event::GithubPush {
                repo: "silkepilon/RackScreen".into(),
                commits: 1,
            }]
        );
        // several new ones arrive oldest first
        let two = vec![
            GhEvent { id: "1008".into(), repo: "r".into(), kind: GhKind::Star },
            GhEvent { id: "1007".into(), repo: "r".into(), kind: GhKind::Merge },
        ];
        let out = t.diff(&two);
        assert_eq!(out, vec![Event::GithubMerge { repo: "r".into() }, Event::GithubStar { repo: "r".into() }]);
    }

    #[test]
    fn runs_parse_and_the_tracker_reports_completions_once() {
        let runs = parse_runs(RUNS).unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0], Run { id: 301, status: "in_progress".into(), conclusion: None });
        let mut t = RunTracker::default();
        assert!(t.diff("r", &runs).is_empty(), "a repo's first page primes");
        let mut done = runs.clone();
        done[0].status = "completed".into();
        done[0].conclusion = Some("success".into());
        assert_eq!(t.diff("r", &done), vec![Event::GithubRun { repo: "r".into(), ok: true }]);
        assert!(t.diff("r", &done).is_empty(), "reported once");
        let mut cancelled = done.clone();
        cancelled.insert(0, Run { id: 302, status: "completed".into(), conclusion: Some("cancelled".into()) });
        assert_eq!(t.diff("r", &cancelled), vec![Event::GithubRun { repo: "r".into(), ok: false }]);
        let mut skipped = cancelled.clone();
        skipped.insert(0, Run { id: 303, status: "completed".into(), conclusion: Some("skipped".into()) });
        assert!(t.diff("r", &skipped).is_empty(), "skipped is neither pass nor fail");
    }
}
```

- [ ] **Step 3: Run to see them fail**

Add `pub mod github;` to `lib.rs`. Run: `cargo test -p rackscreen-sources github`
Expected: compile errors.

- [ ] **Step 4: The module**

Above the tests in `crates/sources/src/github.rs`:

```rust
//! GitHub poller: the contribution calendar (GraphQL), the public events feed
//! for splashes, and Actions runs for the repos pushed to lately.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::{json, Value};

use crate::electricity::{backoff_secs, token_rejected, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

const GRAPHQL: &str = "https://api.github.com/graphql";
const API: &str = "https://api.github.com";
const CALENDAR_QUERY: &str = "query($from: DateTime!, $to: DateTime!) { viewer { login contributionsCollection(from: $from, to: $to) { contributionCalendar { weeks { contributionDays { date contributionCount } } } } } }";
/// Days in the activity window (the 30-day best sets the outer ring).
const WINDOW_DAYS: i64 = 30;
/// The calendar is refreshed this often; events every `poll_secs`.
const CALENDAR_SECS: u64 = 300;
/// Actions runs are checked for at most this many recently pushed repos.
const MAX_RUN_REPOS: usize = 5;

#[derive(Clone, Debug)]
pub struct GithubConfig {
    pub token: String,
    pub poll_secs: u64,
}

/// Thirty dates ending on `today`, oldest first.
pub fn window_days(today: NaiveDate) -> Vec<NaiveDate> {
    (0..WINDOW_DAYS)
        .rev()
        .map(|back| today - chrono::Duration::days(back))
        .collect()
}

/// `(login, days)` from the GraphQL response; every date in `window` gets a
/// count, zero when the calendar has no entry.
pub fn parse_calendar(json: &str, window: &[NaiveDate]) -> Result<(String, Vec<(String, u32)>)> {
    let v: Value = serde_json::from_str(json).context("calendar json")?;
    if let Some(errs) = v.get("errors").and_then(Value::as_array) {
        let msg = errs
            .first()
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(anyhow!("graphql: {msg}"));
    }
    let viewer = v
        .get("data")
        .and_then(|d| d.get("viewer"))
        .context("data.viewer missing")?;
    let login = viewer
        .get("login")
        .and_then(Value::as_str)
        .context("viewer.login missing")?
        .to_string();
    let mut counts: HashMap<String, u32> = HashMap::new();
    let weeks = viewer
        .pointer("/contributionsCollection/contributionCalendar/weeks")
        .and_then(Value::as_array)
        .context("weeks missing")?;
    for w in weeks {
        for day in w
            .get("contributionDays")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(date), Some(n)) = (
                day.get("date").and_then(Value::as_str),
                day.get("contributionCount").and_then(Value::as_u64),
            ) {
                counts.insert(date.to_string(), n as u32);
            }
        }
    }
    let days = window
        .iter()
        .map(|d| {
            let key = d.format("%Y-%m-%d").to_string();
            let n = counts.get(&key).copied().unwrap_or(0);
            (key, n)
        })
        .collect();
    Ok((login, days))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GhKind {
    Push(u32),
    Star,
    Merge,
    Release(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhEvent {
    pub id: String,
    pub repo: String,
    pub kind: GhKind,
}

/// The events we splash on, newest first as GitHub returns them; everything
/// else in the feed is dropped.
pub fn parse_events(json: &str) -> Result<Vec<GhEvent>> {
    let v: Value = serde_json::from_str(json).context("events json")?;
    let arr = v.as_array().context("events is not an array")?;
    let mut out = Vec::new();
    for e in arr {
        let id = e.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let repo = e
            .pointer("/repo/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let action = e.pointer("/payload/action").and_then(Value::as_str);
        let kind = match e.get("type").and_then(Value::as_str) {
            Some("PushEvent") => GhKind::Push(
                e.pointer("/payload/commits")
                    .and_then(Value::as_array)
                    .map_or(1, |c| c.len().max(1) as u32),
            ),
            Some("WatchEvent") if action == Some("started") => GhKind::Star,
            Some("PullRequestEvent")
                if action == Some("closed")
                    && e.pointer("/payload/pull_request/merged")
                        .and_then(Value::as_bool)
                        .unwrap_or(false) =>
            {
                GhKind::Merge
            }
            Some("ReleaseEvent") if action == Some("published") => GhKind::Release(
                e.pointer("/payload/release/tag_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ),
            _ => continue,
        };
        out.push(GhEvent { id, repo, kind });
    }
    Ok(out)
}

/// Repos with a push in the page, most recent first, at most `MAX_RUN_REPOS`.
pub fn pushed_repos(events: &[GhEvent]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in events {
        if matches!(e.kind, GhKind::Push(_)) && !out.contains(&e.repo) {
            out.push(e.repo.clone());
            if out.len() == MAX_RUN_REPOS {
                break;
            }
        }
    }
    out
}

/// Emits one model event per feed event not seen before; the first page only
/// primes so a restart never replays yesterday's pushes.
#[derive(Debug, Default)]
pub struct EventTracker {
    primed: bool,
    seen: HashSet<String>,
}

impl EventTracker {
    pub fn diff(&mut self, events: &[GhEvent]) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed {
            // the feed is newest first; emit oldest first so splashes queue in order
            for e in events.iter().rev() {
                if self.seen.contains(&e.id) {
                    continue;
                }
                out.push(match &e.kind {
                    GhKind::Push(n) => Event::GithubPush {
                        repo: e.repo.clone(),
                        commits: *n,
                    },
                    GhKind::Star => Event::GithubStar {
                        repo: e.repo.clone(),
                    },
                    GhKind::Merge => Event::GithubMerge {
                        repo: e.repo.clone(),
                    },
                    GhKind::Release(tag) => Event::GithubRelease {
                        repo: e.repo.clone(),
                        tag: tag.clone(),
                    },
                });
            }
        }
        self.seen.extend(events.iter().map(|e| e.id.clone()));
        self.primed = true;
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub id: u64,
    pub status: String,
    pub conclusion: Option<String>,
}

pub fn parse_runs(json: &str) -> Result<Vec<Run>> {
    let v: Value = serde_json::from_str(json).context("runs json")?;
    let arr = v
        .get("workflow_runs")
        .and_then(Value::as_array)
        .context("workflow_runs missing")?;
    Ok(arr
        .iter()
        .filter_map(|r| {
            Some(Run {
                id: r.get("id")?.as_u64()?,
                status: r.get("status")?.as_str()?.to_string(),
                conclusion: r
                    .get("conclusion")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect())
}

/// Reports each run once, the first time it is seen completed; a repo's first
/// page only primes.
#[derive(Debug, Default)]
pub struct RunTracker {
    primed: HashSet<String>,
    reported: HashSet<u64>,
}

impl RunTracker {
    pub fn diff(&mut self, repo: &str, runs: &[Run]) -> Vec<Event> {
        let completed = runs.iter().filter(|r| r.status == "completed");
        if !self.primed.contains(repo) {
            self.primed.insert(repo.to_string());
            self.reported.extend(completed.map(|r| r.id));
            return Vec::new();
        }
        let mut out = Vec::new();
        for r in completed {
            if self.reported.contains(&r.id) {
                continue;
            }
            self.reported.insert(r.id);
            let ok = match r.conclusion.as_deref() {
                Some("success") => true,
                Some("failure") | Some("timed_out") | Some("cancelled") => false,
                _ => continue,
            };
            out.push(Event::GithubRun {
                repo: repo.to_string(),
                ok,
            });
        }
        out
    }
}

struct Http {
    token: String,
}

impl Http {
    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    async fn calendar(&self, window: &[NaiveDate]) -> Result<(String, Vec<(String, u32)>)> {
        let from = format!("{}T00:00:00Z", window[0].format("%Y-%m-%d"));
        let to = format!(
            "{}T00:00:00Z",
            (window[window.len() - 1] + chrono::Duration::days(1)).format("%Y-%m-%d")
        );
        let body = json!({ "query": CALENDAR_QUERY, "variables": { "from": from, "to": to } });
        let resp = self
            .auth(client().post(GRAPHQL))
            .json(&body)
            .send()
            .await
            .context("graphql request")?;
        let status = resp.status();
        let text = resp.text().await.context("graphql body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_calendar(&text, window)
    }

    /// `Ok(None)` on 304 (nothing new); otherwise the page and the new ETag.
    async fn events(
        &self,
        login: &str,
        etag: Option<&str>,
    ) -> Result<Option<(Vec<GhEvent>, Option<String>, Option<u64>)>> {
        let mut req = self.auth(client().get(format!("{API}/users/{login}/events?per_page=30")));
        if let Some(tag) = etag {
            req = req.header("If-None-Match", tag);
        }
        let resp = req.send().await.context("events request")?;
        let status = resp.status();
        let poll = resp
            .headers()
            .get("X-Poll-Interval")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let new_etag = resp
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        if status.as_u16() == 304 {
            return Ok(None);
        }
        let text = resp.text().await.context("events body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        Ok(Some((parse_events(&text)?, new_etag, poll)))
    }

    async fn runs(&self, repo: &str) -> Result<Vec<Run>> {
        let resp = self
            .auth(client().get(format!("{API}/repos/{repo}/actions/runs?per_page=5")))
            .send()
            .await
            .context("runs request")?;
        let status = resp.status();
        let text = resp.text().await.context("runs body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_runs(&text)
    }
}

pub async fn run_github(cfg: GithubConfig, ctx: SourceCtx) {
    let http = Http {
        token: cfg.token.clone(),
    };
    let mut login: Option<String> = None;
    let mut etag: Option<String> = None;
    let mut events = EventTracker::default();
    let mut runs = RunTracker::default();
    let mut repos: Vec<String> = Vec::new();
    let mut failures = 0u32;
    let mut warned_about_token = false;
    let mut last_calendar: Option<std::time::Instant> = None;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let mut wait = cfg.poll_secs.max(60);
        let step: Result<()> = async {
            let calendar_due =
                last_calendar.is_none_or(|t| t.elapsed() >= Duration::from_secs(CALENDAR_SECS));
            if calendar_due {
                let today = Local::now().date_naive();
                let window = window_days(today);
                let (who, days) = http.calendar(&window).await?;
                tracing::info!(
                    "github: calendar ok ({} today, {} this window)",
                    days.last().map_or(0, |d| d.1),
                    days.iter().map(|d| d.1).sum::<u32>()
                );
                login = Some(who);
                last_calendar = Some(std::time::Instant::now());
                ctx.emit(Event::GithubActivity { days });
            }
            let who = login.as_deref().context("login unknown")?;
            if let Some((page, new_etag, poll)) = http.events(who, etag.as_deref()).await? {
                etag = new_etag;
                if let Some(p) = poll {
                    wait = wait.max(p);
                }
                repos = pushed_repos(&page);
                ctx.emit_all(events.diff(&page));
            }
            for repo in repos.clone() {
                match http.runs(&repo).await {
                    Ok(page) => ctx.emit_all(runs.diff(&repo, &page)),
                    Err(e) => tracing::warn!("github: runs for {repo}: {e:#}"),
                }
            }
            Ok(())
        }
        .await;
        match step {
            Ok(()) => {
                failures = 0;
                warned_about_token = false;
                ctx.emit(Event::Link {
                    target: LinkTarget::Github,
                    up: true,
                });
            }
            Err(e) => {
                failures += 1;
                let rejected = token_rejected(&e).is_some();
                match token_rejected(&e) {
                    Some(status) if !warned_about_token => {
                        warned_about_token = true;
                        tracing::warn!(
                            "github: token rejected (HTTP {status}); check the token in Configure"
                        );
                    }
                    Some(_) => tracing::debug!("github: {e:#}"),
                    None => tracing::warn!("github: {e:#}"),
                }
                if failures >= 2 || rejected {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Github,
                        up: false,
                    });
                }
                wait = backoff_secs(cfg.poll_secs, failures, rejected);
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}
```

`Datelike` is imported for `NaiveDate` arithmetic helpers; drop it if clippy reports it unused.

- [ ] **Step 5: Test, clippy, commit**

Run: `cargo test -p rackscreen-sources && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(sources): GitHub calendar, events and Actions poller

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 14: Fake source data and simulator keys

**Files:**
- Modify: `crates/sources/src/fake.rs`
- Modify: `crates/display/src/sim.rs:55-75` and its `keys_forward_every_fake_command` test

**Interfaces:**
- Consumes: every new `Event`, `astro::sky_event`.
- Produces: `FakeCmd::{UpsToggle, AppToggle, GithubPush, GithubStar, CiFailure, Thunder, RainSoon, IssPassNow}` on keys `u d g r f l w i`; `FakeState::set_clock(unix: i64, utc_offset_secs: i32)`; `FakeState::unix_now()`.

- [ ] **Step 1: Failing tests**

`crates/sources/src/fake.rs`, `mod tests`: extend `keys_map` with

```rust
        assert_eq!(FakeCmd::from_key('u'), Some(FakeCmd::UpsToggle));
        assert_eq!(FakeCmd::from_key('d'), Some(FakeCmd::AppToggle));
        assert_eq!(FakeCmd::from_key('g'), Some(FakeCmd::GithubPush));
        assert_eq!(FakeCmd::from_key('r'), Some(FakeCmd::GithubStar));
        assert_eq!(FakeCmd::from_key('f'), Some(FakeCmd::CiFailure));
        assert_eq!(FakeCmd::from_key('l'), Some(FakeCmd::Thunder));
        assert_eq!(FakeCmd::from_key('w'), Some(FakeCmd::RainSoon));
        assert_eq!(FakeCmd::from_key('i'), Some(FakeCmd::IssPassNow));
```

and add:

```rust
    #[test]
    fn initial_has_every_new_role_fed() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.initial();
        for target in [
            LinkTarget::Weather,
            LinkTarget::Rain,
            LinkTarget::Github,
            LinkTarget::ArgoCd,
        ] {
            assert!(
                evs.iter()
                    .any(|e| matches!(e, Event::Link { target: t, up: true } if *t == target)),
                "{target:?} link up missing"
            );
        }
        assert!(evs.iter().any(|e| matches!(e, Event::Weather { .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::AirQuality { .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::Rain { mm_per_h, .. } if mm_per_h.len() == 24)));
        assert!(evs.iter().any(|e| matches!(e, Event::Sky { sunrise: Some(_), .. })));
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::IssPass(Some(p)) if p.start == 1_788_782_400 + 42 * 60 && p.visible
        )));
        assert!(evs.iter().any(|e| matches!(e, Event::GithubActivity { days } if days.len() == 30 && days[29].1 == 28)));
        assert!(evs.iter().any(|e| matches!(e, Event::Ups { on_battery: false, .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::Network { .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::Apps(a) if a.len() == 16)));
    }

    #[test]
    fn new_commands_emit_their_edges() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.command(FakeCmd::UpsToggle);
        assert!(evs.contains(&Event::UpsOnBattery));
        assert!(evs.iter().any(|e| matches!(e, Event::Ups { on_battery: true, .. })));
        assert!(s.command(FakeCmd::UpsToggle).contains(&Event::UpsOnline));
        let evs = s.command(FakeCmd::AppToggle);
        assert!(evs.iter().any(|e| matches!(e, Event::AppDegraded { .. })));
        assert!(s.command(FakeCmd::AppToggle).iter().any(|e| matches!(e, Event::AppHealthy { .. })));
        assert!(matches!(
            s.command(FakeCmd::GithubPush).as_slice(),
            [Event::GithubPush { commits: 3, .. }]
        ));
        assert!(matches!(s.command(FakeCmd::GithubStar).as_slice(), [Event::GithubStar { .. }]));
        assert!(matches!(s.command(FakeCmd::CiFailure).as_slice(), [Event::GithubRun { ok: false, .. }]));
        assert!(matches!(s.command(FakeCmd::Thunder).as_slice(), [Event::Weather { code: 95, .. }]));
        assert!(matches!(
            s.command(FakeCmd::RainSoon).as_slice(),
            [Event::Rain { mm_per_h, .. }] if mm_per_h[2] > 0.1 && mm_per_h[0] == 0.0
        ));
        assert!(matches!(
            s.command(FakeCmd::IssPassNow).as_slice(),
            [Event::IssPass(Some(p))] if p.start == s.unix_now()
        ));
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-sources fake`
Expected: compile errors.

- [ ] **Step 3: State, builders, initial, tick, commands**

`crates/sources/src/fake.rs`. Imports: extend the `rackscreen_core::event` import with `App, AppHealth, AppSync, IssPass` and add `use crate::astro::sky_event;`. Constants after `PRICE_DATE`:

```rust
/// Simulated observer: Amsterdam.
const FAKE_LAT: f64 = 52.37;
const FAKE_LON: f64 = 4.89;
/// WMO codes the demo weather cycles through: clear, partly cloudy, rain, thunder.
const WEATHER_CYCLE: [u16; 4] = [0, 2, 61, 95];
/// Rain nowcast seed, mm/h per five-minute slot: dry, a shower, dry.
const RAIN_SEED: [f32; 24] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.3, 1.0, 2.5, 5.5, 6.5, 6.0, 4.0, 3.0, 2.0, 1.5, 0.8, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0,
];
/// Contribution counts for the last 30 days, today last.
const GH_SEED: [u32; 30] = [
    3, 7, 0, 12, 5, 9, 2, 0, 14, 6, 8, 1, 4, 11, 3, 0, 9, 17, 6, 2, 5, 8, 0, 3, 13, 7, 9, 4, 43, 28,
];
/// Argo CD applications, named like the real cluster.
const APP_NAMES: [&str; 16] = [
    "arr-stack", "cloudflared", "hermes", "homeassistant", "longhorn", "monitoring", "n8n",
    "node-maintenance", "open-webui", "pihole", "plane", "portfolio", "root", "stirling-pdf",
    "tailscale", "twenty",
];
/// The app the `d` key degrades.
const DEGRADED_APP: usize = 5;
/// Minutes until the demo ISS pass, and its length.
const ISS_PASS_IN_SECS: i64 = 42 * 60;
const ISS_PASS_SECS: i64 = 6 * 60;
```

`FakeCmd` gains `UpsToggle, AppToggle, GithubPush, GithubStar, CiFailure, Thunder, RainSoon, IssPassNow` and `from_key` the arms `'u' => FakeCmd::UpsToggle, 'd' => FakeCmd::AppToggle, 'g' => FakeCmd::GithubPush, 'r' => FakeCmd::GithubStar, 'f' => FakeCmd::CiFailure, 'l' => FakeCmd::Thunder, 'w' => FakeCmd::RainSoon, 'i' => FakeCmd::IssPassNow,`.

`FakeState` fields, after `price_rot: usize,`:

```rust
    /// Wall clock the sky, rain and ISS events are built against; advances with ticks.
    unix_start: i64,
    utc_offset: i32,
    weather_idx: usize,
    temp: f32,
    wind_from: f32,
    wind: f32,
    eaqi: f32,
    rain: Vec<f32>,
    ups_on_battery: bool,
    ups_charge: f32,
    net_rx: f64,
    net_tx: f64,
    apps: Vec<App>,
    gh_days: Vec<u32>,
    iss_start: i64,
```

In `new`, after `price_rot: 0,`:

```rust
            unix_start: chrono::Local::now().timestamp(),
            utc_offset: chrono::Local::now().offset().local_minus_utc(),
            weather_idx: 1,
            temp: 18.0,
            wind_from: 232.0,
            wind: 19.0,
            eaqi: 32.0,
            rain: RAIN_SEED.to_vec(),
            ups_on_battery: false,
            ups_charge: 100.0,
            net_rx: 41e6,
            net_tx: 4e6,
            apps: APP_NAMES
                .iter()
                .map(|n| App {
                    name: (*n).to_string(),
                    sync: AppSync::Synced,
                    health: AppHealth::Healthy,
                    operating: false,
                })
                .collect(),
            gh_days: GH_SEED.to_vec(),
            iss_start: 0,
```

and right after the struct literal in `new`, before returning, set the pass: turn `Self { ... }` into `let mut s = Self { ... }; s.iss_start = s.unix_start + ISS_PASS_IN_SECS; s`. `chrono::Offset` needs importing for `local_minus_utc` (`use chrono::Offset;`).

New methods on `FakeState`:

```rust
    /// Pin the clock (the GIF generator and tests want reproducible skies).
    pub fn set_clock(&mut self, unix: i64, utc_offset_secs: i32) {
        self.unix_start = unix;
        self.utc_offset = utc_offset_secs;
        self.iss_start = unix + ISS_PASS_IN_SECS;
    }
    pub fn unix_now(&self) -> i64 {
        self.unix_start + self.ticks as i64
    }

    fn weather(&self) -> Event {
        Event::Weather {
            temp_c: self.temp,
            code: WEATHER_CYCLE[self.weather_idx % WEATHER_CYCLE.len()],
            is_day: true,
            wind_kmh: self.wind,
            gust_kmh: self.wind * 1.8,
            wind_from_deg: self.wind_from,
            at: self.updated_at(),
        }
    }
    fn air(&self) -> Event {
        Event::AirQuality { eaqi: self.eaqi }
    }
    fn rain(&self) -> Event {
        Event::Rain {
            from: self.unix_now(),
            mm_per_h: self.rain.clone(),
        }
    }
    fn sky(&self) -> Event {
        sky_event(FAKE_LAT, FAKE_LON, self.unix_now(), self.utc_offset)
    }
    fn iss(&self) -> Event {
        Event::IssPass(Some(IssPass {
            start: self.iss_start,
            end: self.iss_start + ISS_PASS_SECS,
            max_elevation_deg: 62.0,
            visible: true,
        }))
    }
    fn github(&self) -> Event {
        let today = chrono::DateTime::from_timestamp(self.unix_now() + self.utc_offset as i64, 0)
            .map(|d| d.date_naive())
            .unwrap_or_default();
        let days = self
            .gh_days
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let back = (self.gh_days.len() - 1 - i) as i64;
                ((today - chrono::Duration::days(back)).format("%Y-%m-%d").to_string(), *n)
            })
            .collect();
        Event::GithubActivity { days }
    }
    fn ups(&self) -> Event {
        Event::Ups {
            on_battery: self.ups_on_battery,
            low_battery: self.ups_charge <= 20.0,
            charge_pct: self.ups_charge,
            load_pct: 6.0,
            runtime_secs: (self.ups_charge / 100.0 * 3014.0) as u32,
        }
    }
    fn net(&self) -> Event {
        Event::Network {
            rx_bps: self.net_rx,
            tx_bps: self.net_tx,
        }
    }
    fn apps(&self) -> Event {
        Event::Apps(self.apps.clone())
    }
```

`initial()`: add links `Weather`, `Rain`, `Github`, `ArgoCd` (all `up: true`) after `Prices`, and after `self.prices(),` the events `self.weather(), self.air(), self.rain(), self.sky(), self.iss(), self.github(), self.ups(), self.net(), self.apps(),`.

`tick()`, after the `carbon` random walk and before `let mut out`:

```rust
        self.net_rx = (self.net_rx * (1.0 + self.rng.f64() * 0.4 - 0.2)).clamp(2e6, 400e6);
        self.net_tx = (self.net_tx * (1.0 + self.rng.f64() * 0.4 - 0.2)).clamp(2e5, 40e6);
        self.temp = (self.temp + self.rng.f32() * 0.4 - 0.2).clamp(-5.0, 35.0);
        self.wind = (self.wind + self.rng.f32() * 2.0 - 1.0).clamp(3.0, 60.0);
        self.wind_from = (self.wind_from + self.rng.f32() * 6.0 - 3.0).rem_euclid(360.0);
        self.eaqi = (self.eaqi + self.rng.f32() * 2.0 - 1.0).clamp(5.0, 95.0);
        if self.ups_on_battery {
            self.ups_charge = (self.ups_charge - 0.5).max(0.0);
        } else {
            self.ups_charge = (self.ups_charge + 1.0).min(100.0);
        }
```

and in the emission part, after `let mut out = vec![self.metrics()];`: `out.push(self.net());`. In the `is_multiple_of(5)` block add `out.push(self.ups());` and

```rust
            self.rain.rotate_left(1);
            out.push(self.rain());
```

In the `is_multiple_of(10)` block add `out.push(self.air());`. Add a new block:

```rust
        if self.ticks.is_multiple_of(30) {
            self.weather_idx += 1;
            out.push(self.weather());
        }
        if self.ticks.is_multiple_of(60) {
            out.push(self.sky());
            out.push(self.iss());
            out.push(self.github());
        }
```

`command()` arms:

```rust
            FakeCmd::UpsToggle => {
                self.ups_on_battery = !self.ups_on_battery;
                vec![
                    if self.ups_on_battery {
                        Event::UpsOnBattery
                    } else {
                        Event::UpsOnline
                    },
                    self.ups(),
                ]
            }
            FakeCmd::AppToggle => {
                let app = &mut self.apps[DEGRADED_APP];
                let name = app.name.clone();
                let edge = if app.health == AppHealth::Healthy {
                    app.health = AppHealth::Degraded;
                    Event::AppDegraded { name }
                } else {
                    app.health = AppHealth::Healthy;
                    Event::AppHealthy { name }
                };
                vec![edge, self.apps()]
            }
            FakeCmd::GithubPush => vec![Event::GithubPush {
                repo: "silkepilon/RackScreen".into(),
                commits: 3,
            }],
            FakeCmd::GithubStar => vec![Event::GithubStar {
                repo: "silkepilon/RackScreen".into(),
            }],
            FakeCmd::CiFailure => vec![Event::GithubRun {
                repo: "silkepilon/RackScreen".into(),
                ok: false,
            }],
            FakeCmd::Thunder => {
                self.weather_idx = 3; // WEATHER_CYCLE[3] is thunder
                vec![self.weather()]
            }
            FakeCmd::RainSoon => {
                self.rain = vec![0.0; 24];
                for (i, v) in [(2, 1.5), (3, 3.0), (4, 4.0), (5, 2.0), (6, 0.5)] {
                    self.rain[i] = v;
                }
                vec![self.rain()]
            }
            FakeCmd::IssPassNow => {
                self.iss_start = self.unix_now();
                vec![self.iss()]
            }
```

- [ ] **Step 4: Simulator keys**

`crates/display/src/sim.rs`, in the `key_char` match after `Key::P => 'p',`:

```rust
        Key::U => 'u',
        Key::D => 'd',
        Key::G => 'g',
        Key::R => 'r',
        Key::F => 'f',
        Key::L => 'l',
        Key::W => 'w',
        Key::I => 'i',
```

and in `keys_forward_every_fake_command` add `(Key::U, 'u'), (Key::D, 'd'), (Key::G, 'g'), (Key::R, 'r'), (Key::F, 'f'), (Key::L, 'l'), (Key::W, 'w'), (Key::I, 'i'),`. If the simulator window's key help text lists keys, add `u d g r f l w i` there too.

- [ ] **Step 5: Test, clippy (sim feature), commit**

Run: `cargo test --workspace --features sim,pi && cargo clippy --workspace --features sim,pi -- -D warnings && cargo clippy --no-default-features --features sim -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(sim): fake data and keys for the eleven new roles

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 15: App config and wiring

**Files:**
- Modify: `crates/app/src/config.rs`, `config.example.yaml`
- Modify: `crates/app/src/run.rs`, `crates/app/src/runloop.rs`

**Interfaces:**
- Consumes: `open_meteo::run_weather`, `buienradar::run_rain`, `astro::run_astro`, `iss::run_iss`, `github::run_github`, `argocd::run_argocd`, `Model::{set_unix_now, set_utc_offset_secs, set_location_present, set_github_token_present}`.
- Produces: `config::{LocationCfg, WeatherCfg, RainCfg, IssCfg, GithubCfg, ArgocdCfg}`, `Config::location() -> Option<(f64, f64)>`, `RenderLoop::{location_present, github_token_present}`.

- [ ] **Step 1: Failing config tests**

`crates/app/src/config.rs`, `mod tests`:

```rust
    #[test]
    fn new_sections_default_off_and_location_unset() {
        let c = Config::default();
        assert_eq!(c.location(), None);
        assert!(!c.weather.enabled && c.weather.poll_secs == 600);
        assert!(!c.rain.enabled && c.rain.poll_secs == 300);
        assert!(!c.iss.enabled && c.iss.min_elevation == 10.0);
        assert!(!c.github.enabled && c.github.token.is_empty() && c.github.poll_secs == 60);
        assert!(c.argocd.enabled && c.argocd.namespace == "argocd");
        c.validate().unwrap();
    }

    #[test]
    fn location_and_sky_validation() {
        let mut c = Config::default();
        c.weather.enabled = true;
        assert!(c.validate().unwrap_err().to_string().contains("location"));
        c.location.lat = Some(52.37);
        assert!(c.validate().unwrap_err().to_string().contains("both"));
        c.location.lon = Some(4.89);
        assert_eq!(c.location(), Some((52.37, 4.89)));
        c.validate().unwrap();
        c.location.lat = Some(95.0);
        assert!(c.validate().is_err());
        c.location.lat = Some(52.37);
        c.iss.min_elevation = 91.0;
        assert!(c.validate().is_err());
        c.iss.min_elevation = 10.0;
        c.weather.poll_secs = 5;
        assert!(c.validate().unwrap_err().to_string().contains("poll_secs"));
        c.weather.poll_secs = 60;
        // github on without a token is allowed: the role shows the key icon
        c.github.enabled = true;
        c.validate().unwrap();
        let yaml = "location: { lat: 1.5, lon: -2.25 }\nweather: { enabled: true }\nscreens:\n  - { roles: [weather], spi: 0, cs: 0, dc: 6, rst: 5 }\n";
        let c = Config::from_yaml(yaml).unwrap();
        assert_eq!(c.location(), Some((1.5, -2.25)));
        assert!(c.weather.enabled);
        assert_eq!(c.weather.poll_secs, 600);
        c.validate().unwrap();
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-app config`
Expected: compile errors.

- [ ] **Step 3: Sections, defaults, example, validation**

`crates/app/src/config.rs`. In `Config`, after `pub price: PriceCfg,`:

```rust
    #[serde(default)]
    pub location: LocationCfg,
    #[serde(default)]
    pub weather: WeatherCfg,
    #[serde(default)]
    pub rain: RainCfg,
    #[serde(default)]
    pub iss: IssCfg,
    #[serde(default)]
    pub github: GithubCfg,
    #[serde(default)]
    pub argocd: ArgocdCfg,
```

After `PriceCfg`:

```rust
/// Where the rack lives; needed by the sky roles.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default)]
pub struct LocationCfg {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

/// Open-Meteo current conditions and air quality, no key.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WeatherCfg {
    pub enabled: bool,
    pub poll_secs: u64,
}

/// Buienradar rain nowcast (Netherlands and Belgium), no key.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RainCfg {
    pub enabled: bool,
    pub poll_secs: u64,
}

/// ISS passes from the Celestrak TLE, computed on the Pi.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct IssCfg {
    pub enabled: bool,
    /// Degrees above the horizon that count as a pass.
    pub min_elevation: f32,
}

/// GitHub activity: a fine-grained token with read access to contributions,
/// events and Actions.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct GithubCfg {
    pub enabled: bool,
    pub token: String,
    pub poll_secs: u64,
}

/// Argo CD applications for the deploys role.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ArgocdCfg {
    pub enabled: bool,
    pub namespace: String,
}
```

Defaults, after `impl Default for PriceCfg`:

```rust
impl Default for WeatherCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_secs: 600,
        }
    }
}
impl Default for RainCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_secs: 300,
        }
    }
}
impl Default for IssCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            min_elevation: 10.0,
        }
    }
}
impl Default for GithubCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            poll_secs: 60,
        }
    }
}
impl Default for ArgocdCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            namespace: "argocd".into(),
        }
    }
}
```

On `impl Config`, next to `validate`:

```rust
    /// `(lat, lon)` when both are set.
    pub fn location(&self) -> Option<(f64, f64)> {
        match (self.location.lat, self.location.lon) {
            (Some(lat), Some(lon)) => Some((lat, lon)),
            _ => None,
        }
    }
```

In `validate`, before `Ok(())`:

```rust
        anyhow::ensure!(
            self.location.lat.is_some() == self.location.lon.is_some(),
            "location needs both lat and lon, or neither"
        );
        if let Some((lat, lon)) = self.location() {
            anyhow::ensure!(
                (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
                "location.lat must be -90..90 and location.lon -180..180 (got {lat}, {lon})"
            );
        }
        let sky_on = self.weather.enabled || self.rain.enabled || self.iss.enabled;
        anyhow::ensure!(
            !sky_on || self.location().is_some(),
            "weather, rain and iss need location.lat and location.lon"
        );
        anyhow::ensure!(
            (0.0..=90.0).contains(&self.iss.min_elevation),
            "iss.min_elevation must be 0..90 (got {})",
            self.iss.min_elevation
        );
        for (name, secs) in [
            ("weather", self.weather.poll_secs),
            ("rain", self.rain.poll_secs),
            ("github", self.github.poll_secs),
        ] {
            anyhow::ensure!(secs >= 60, "{name}.poll_secs must be at least 60 (got {secs})");
        }
```

`config.example.yaml`, after the `price:` block and before `display:`:

```yaml
location:
  lat: null                # decimal degrees; needed by weather, wind, aqi, rain, sun, moon and iss
  lon: null

weather:
  enabled: false           # Open-Meteo current conditions + air quality, no key
  poll_secs: 600

rain:
  enabled: false           # Buienradar two-hour nowcast (NL/BE), no key
  poll_secs: 300

iss:
  enabled: false           # next ISS pass, computed from the Celestrak TLE
  min_elevation: 10        # degrees above the horizon that count as a pass

github:
  enabled: false           # fine-grained token: read access to contributions, events and Actions
  token: ""
  poll_secs: 60

argocd:
  enabled: true            # watch applications.argoproj.io for the deploys role
  namespace: argocd
```

and change the roles comment line to:

```yaml
# Roles: cpu, mem, pods, health, thermal, storage, power-mix, price, carbon, renewable,
#        gh-activity, weather, wind, aqi, rain, sun, moon, iss, ups, net, deploys
```

Run: `cargo test -p rackscreen-app config`
Expected: PASS (including the existing round-trip test, since every new key serialises).

- [ ] **Step 4: Spawn the sources and feed the model**

`crates/app/src/run.rs`. After `spawn_energy_sources(&runtime, cfg, &ctx);` inside the `if !matches!(source, SourceKind::Fake)` block add `spawn_external_sources(&runtime, cfg, &ctx);`. New function after `spawn_energy_sources`:

```rust
/// Sky and GitHub sources: public APIs and local maths, independent of the cluster.
fn spawn_external_sources(runtime: &tokio::runtime::Runtime, cfg: &Config, ctx: &SourceCtx) {
    let location = cfg.location();
    let needs_location = cfg.weather.enabled || cfg.rain.enabled || cfg.iss.enabled;
    let Some((lat, lon)) = location else {
        if needs_location {
            tracing::warn!("weather/rain/iss enabled but no location set; sky screens show a pin");
        }
        if cfg.github.enabled {
            spawn_github(runtime, cfg, ctx);
        }
        return;
    };
    // sun and moon cost nothing and every sky role wants them
    runtime.spawn(rackscreen_sources::astro::run_astro(lat, lon, ctx.clone()));
    if cfg.weather.enabled {
        runtime.spawn(rackscreen_sources::open_meteo::run_weather(
            rackscreen_sources::open_meteo::WeatherConfig {
                lat,
                lon,
                poll_secs: cfg.weather.poll_secs,
            },
            ctx.clone(),
        ));
    }
    if cfg.rain.enabled {
        runtime.spawn(rackscreen_sources::buienradar::run_rain(
            rackscreen_sources::buienradar::RainConfig {
                lat,
                lon,
                poll_secs: cfg.rain.poll_secs,
            },
            ctx.clone(),
        ));
    }
    if cfg.iss.enabled {
        runtime.spawn(rackscreen_sources::iss::run_iss(
            rackscreen_sources::iss::IssConfig {
                lat,
                lon,
                min_elevation: cfg.iss.min_elevation,
            },
            ctx.clone(),
        ));
    }
    if cfg.github.enabled {
        spawn_github(runtime, cfg, ctx);
    }
}

fn spawn_github(runtime: &tokio::runtime::Runtime, cfg: &Config, ctx: &SourceCtx) {
    if cfg.github.token.is_empty() {
        tracing::warn!("github enabled but no token set; gh-activity stays on the key icon");
        return;
    }
    runtime.spawn(rackscreen_sources::github::run_github(
        rackscreen_sources::github::GithubConfig {
            token: cfg.github.token.clone(),
            poll_secs: cfg.github.poll_secs,
        },
        ctx.clone(),
    ));
}
```

In `spawn_k8s_sources`, before the `qbittorrent` block:

```rust
        if cfg2.argocd.enabled {
            tokio::spawn(rackscreen_sources::argocd::run_argocd(
                client.clone(),
                rackscreen_sources::argocd::ArgoConfig {
                    namespace: cfg2.argocd.namespace.clone(),
                },
                ctx.clone(),
            ));
        }
```

In `Monitor::start`, the `RenderLoop` literal gains:

```rust
            location_present: matches!(source, SourceKind::Fake) || cfg.location().is_some(),
            github_token_present: matches!(source, SourceKind::Fake)
                || (cfg.github.enabled && !cfg.github.token.is_empty()),
```

`crates/app/src/runloop.rs`. `RenderLoop` gains:

```rust
    /// `location.lat`/`lon` are set; without them the sky screens show a pin.
    pub location_present: bool,
    /// A GitHub token is configured; without one `gh-activity` shows a key.
    pub github_token_present: bool,
```

After `model.set_token_present(self.token_present);`:

```rust
        model.set_location_present(self.location_present);
        model.set_github_token_present(self.github_token_present);
```

Replace `local_hour_and_minutes` with a clock reader that also returns the unix time and offset, and use it in the loop:

```rust
/// Local wall clock as `(hour, minutes since midnight, unix seconds, seconds east of UTC)`;
/// one clock read per tick.
fn clock() -> (u32, u32, i64, i32) {
    let t = chrono::Local::now();
    (
        t.hour(),
        t.hour() * 60 + t.minute(),
        t.timestamp(),
        t.offset().local_minus_utc(),
    )
}
```

(`use chrono::Offset;` alongside the existing `Timelike` import.) In the loop:

```rust
            let (hour, minutes, unix, offset) = clock();
            model.set_local_hour(hour);
            model.set_unix_now(unix);
            model.set_utc_offset_secs(offset);
```

Any other constructor of `RenderLoop` (search `RenderLoop {` under `crates/app` and `crates/setup`) gets the two new fields as `true`.

- [ ] **Step 5: Test, clippy, commit**

Run: `cargo test --workspace --features sim,pi && cargo clippy --workspace --features sim,pi -- -D warnings && cargo clippy --no-default-features --features pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(app): config sections and wiring for the sky, GitHub and Argo CD sources

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 16: Setup TUI: Configure fields, Sky preset, role hints, Status dots

**Files:**
- Modify: `crates/setup/src/screens/configure.rs`
- Modify: `crates/setup/src/ops/config_file.rs`, `crates/setup/src/screens/screens.rs`
- Modify: `crates/setup/src/ops/systemd.rs`, `crates/setup/src/screens/status.rs`

**Interfaces:**
- Consumes: the config sections from Task 15, `Role::is_sky`.
- Produces: `Field::{LocLat, LocLon, WeatherEnabled, WeatherPoll, RainEnabled, RainPoll, IssEnabled, IssMinElevation, GithubEnabled, GithubToken, GithubPoll, ArgoEnabled, ArgoNamespace}`, `Preset::Sky`, `LinkDots::{weather, rain, github, argocd}`.

- [ ] **Step 1: Failing tests**

`crates/setup/src/screens/configure.rs`, `mod tests` (there is one; add):

```rust
    #[test]
    fn new_fields_round_trip() {
        let mut cfg = Config::default();
        assert_eq!(get(&cfg, Field::LocLat), "");
        set(&mut cfg, Field::LocLat, "52.37").unwrap();
        set(&mut cfg, Field::LocLon, "4.89").unwrap();
        assert_eq!(cfg.location(), Some((52.37, 4.89)));
        assert_eq!(get(&cfg, Field::LocLat), "52.37");
        set(&mut cfg, Field::LocLat, "").unwrap();
        assert_eq!(cfg.location.lat, None, "blank clears it");
        assert!(set(&mut cfg, Field::LocLon, "east").is_err());
        assert!(set(&mut cfg, Field::LocLat, "95").is_err());
        set(&mut cfg, Field::WeatherEnabled, "true").unwrap();
        set(&mut cfg, Field::WeatherPoll, "30").unwrap();
        assert_eq!(cfg.weather.poll_secs, 60, "clamped to the API floor");
        set(&mut cfg, Field::IssMinElevation, "25").unwrap();
        assert_eq!(cfg.iss.min_elevation, 25.0);
        assert!(set(&mut cfg, Field::IssMinElevation, "100").is_err());
        set(&mut cfg, Field::GithubToken, "ghp_x").unwrap();
        assert_eq!(get(&cfg, Field::GithubToken), "ghp_x");
        set(&mut cfg, Field::ArgoNamespace, "argo").unwrap();
        assert_eq!(cfg.argocd.namespace, "argo");
        assert_eq!(FIELDS.len(), 44);
        assert!(FIELDS
            .iter()
            .any(|(f, _, k)| *f == Field::GithubToken && *k == FieldKind::Secret));
    }
```

`crates/setup/src/ops/config_file.rs` tests:

```rust
    #[test]
    fn sky_preset_fills_four_screens() {
        use rackscreen_core::theme::Role;
        let rows = Preset::Sky.rows();
        assert_eq!(rows[0], vec![Role::Weather, Role::Aqi]);
        assert_eq!(rows[1], vec![Role::Rain, Role::Wind]);
        assert_eq!(rows[2], vec![Role::Sun, Role::Moon]);
        assert_eq!(rows[3], vec![Role::Iss, Role::GhActivity]);
        let mut c = Config::default();
        apply_preset(&mut c, Preset::Sky);
        assert_eq!(c.screens[3].roles, vec!["iss", "gh-activity"]);
    }
```

`crates/setup/src/screens/screens.rs` tests, next to the existing `role_hint` assertions (look at how those tests build a `Screens` with a config; reuse that helper):

```rust
    #[test]
    fn sky_and_github_hints() {
        let dir = tempfile::tempdir().unwrap();
        let sh = test_shared(dir.path());
        let mut s = Screens::new(&sh);
        // defaults: no location, no GitHub token, Argo CD on
        assert_eq!(s.role_hint(Role::Weather), Some("no location"));
        assert_eq!(s.role_hint(Role::Moon), Some("no location"));
        assert_eq!(s.role_hint(Role::GhActivity), Some("no token"));
        assert_eq!(s.role_hint(Role::Deploys), None);
        {
            let cfg = s.cfg.as_mut().unwrap();
            cfg.location.lat = Some(52.37);
            cfg.location.lon = Some(4.89);
            cfg.github.token = "t".into();
            cfg.argocd.enabled = false;
        }
        assert_eq!(s.role_hint(Role::Weather), None);
        assert_eq!(s.role_hint(Role::GhActivity), None);
        assert_eq!(s.role_hint(Role::Deploys), Some("argocd off"));
    }
```

`crates/setup/src/ops/systemd.rs`, extend `journal_and_links` (the `LinkDots` literal there gains `weather: Dot::Unknown, rain: Dot::Unknown, github: Dot::Unknown, argocd: Dot::Unknown`) and add:

```rust
    #[test]
    fn new_link_dots_follow_their_log_prefixes() {
        let d = links_from_logs(&[
            "INFO weather: poll ok (21.2 °C, code 3)".to_string(),
            "WARN rain: request: timed out".to_string(),
            "INFO github: calendar ok (28 today, 230 this window)".to_string(),
            "INFO argocd: watching applications in argocd".to_string(),
        ]);
        assert_eq!(d.weather, Dot::Up);
        assert_eq!(d.rain, Dot::Down);
        assert_eq!(d.github, Dot::Up);
        assert_eq!(d.argocd, Dot::Up);
        let d = links_from_logs(&["WARN argocd: watch: 410 Gone".to_string()]);
        assert_eq!(d.argocd, Dot::Down);
        assert_eq!(d.weather, Dot::Unknown);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p rackscreen-setup`
Expected: compile errors.

- [ ] **Step 3: Configure fields**

`crates/setup/src/screens/configure.rs`. `Field` gains, after `HotTemp`:

```rust
    LocLat,
    LocLon,
    WeatherEnabled,
    WeatherPoll,
    RainEnabled,
    RainPoll,
    IssEnabled,
    IssMinElevation,
    GithubEnabled,
    GithubToken,
    GithubPoll,
    ArgoEnabled,
    ArgoNamespace,
```

`FIELDS` becomes `[(Field, &str, FieldKind); 44]` with, after the `HotTemp` entry:

```rust
    (Field::LocLat, "location lat", FieldKind::Number),
    (Field::LocLon, "location lon", FieldKind::Number),
    (Field::WeatherEnabled, "weather enabled", FieldKind::Bool),
    (Field::WeatherPoll, "weather poll secs", FieldKind::Number),
    (Field::RainEnabled, "rain enabled (NL/BE)", FieldKind::Bool),
    (Field::RainPoll, "rain poll secs", FieldKind::Number),
    (Field::IssEnabled, "iss enabled", FieldKind::Bool),
    (Field::IssMinElevation, "iss min elevation °", FieldKind::Number),
    (Field::GithubEnabled, "github enabled", FieldKind::Bool),
    (Field::GithubToken, "github token", FieldKind::Secret),
    (Field::GithubPoll, "github poll secs", FieldKind::Number),
    (Field::ArgoEnabled, "argocd enabled", FieldKind::Bool),
    (Field::ArgoNamespace, "argocd namespace", FieldKind::Text),
```

`get` arms:

```rust
        Field::LocLat => cfg.location.lat.map(|v| v.to_string()).unwrap_or_default(),
        Field::LocLon => cfg.location.lon.map(|v| v.to_string()).unwrap_or_default(),
        Field::WeatherEnabled => cfg.weather.enabled.to_string(),
        Field::WeatherPoll => cfg.weather.poll_secs.to_string(),
        Field::RainEnabled => cfg.rain.enabled.to_string(),
        Field::RainPoll => cfg.rain.poll_secs.to_string(),
        Field::IssEnabled => cfg.iss.enabled.to_string(),
        Field::IssMinElevation => format!("{}", cfg.iss.min_elevation),
        Field::GithubEnabled => cfg.github.enabled.to_string(),
        Field::GithubToken => cfg.github.token.clone(),
        Field::GithubPoll => cfg.github.poll_secs.to_string(),
        Field::ArgoEnabled => cfg.argocd.enabled.to_string(),
        Field::ArgoNamespace => cfg.argocd.namespace.clone(),
```

`set` arms (a helper for the optional coordinates first):

```rust
fn coord(t: &str, what: &str, limit: f64) -> Result<Option<f64>, String> {
    if t.is_empty() {
        return Ok(None);
    }
    let v: f64 = num(t, what)?;
    if v.abs() > limit {
        return Err(format!("{what} must be between -{limit} and {limit}"));
    }
    Ok(Some(v))
}
```

```rust
        Field::LocLat => cfg.location.lat = coord(t, "lat", 90.0)?,
        Field::LocLon => cfg.location.lon = coord(t, "lon", 180.0)?,
        Field::WeatherEnabled => cfg.weather.enabled = t == "true",
        Field::WeatherPoll => cfg.weather.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::RainEnabled => cfg.rain.enabled = t == "true",
        Field::RainPoll => cfg.rain.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::IssEnabled => cfg.iss.enabled = t == "true",
        Field::IssMinElevation => {
            let e: f32 = num(t, "elevation")?;
            if !(0.0..=90.0).contains(&e) {
                return Err("elevation must be between 0 and 90".into());
            }
            cfg.iss.min_elevation = e;
        }
        Field::GithubEnabled => cfg.github.enabled = t == "true",
        Field::GithubToken => cfg.github.token = text.into(),
        Field::GithubPoll => cfg.github.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::ArgoEnabled => cfg.argocd.enabled = t == "true",
        Field::ArgoNamespace => cfg.argocd.namespace = t.into(),
```

The spec planned to generalise `FieldKind::Choice`; no new field is a choice, so `PRICE_SOURCES` and `next_choice` stay as they are.

- [ ] **Step 4: Sky preset and role hints**

`crates/setup/src/ops/config_file.rs`: `Preset` gains `Sky`, and `rows`:

```rust
            Preset::Sky => vec![
                vec![Weather, Aqi],
                vec![Rain, Wind],
                vec![Sun, Moon],
                vec![Iss, GhActivity],
            ],
```

`crates/setup/src/screens/screens.rs`: in `handle`, after the `'m'` arm:

```rust
            KeyCode::Char('w') => {
                self.editor.apply_preset(Preset::Sky);
                self.dirty = true;
                if let Ok(cfg) = &self.cfg {
                    if cfg.location().is_none() {
                        self.error =
                            Some("sky preset needs a location: set lat/lon under Configure".into());
                    }
                }
            }
```

`role_hint` gains, before `_ => None`:

```rust
            r if r.is_sky() => c.location().is_none().then_some("no location"),
            Role::GhActivity => c.github.token.is_empty().then_some("no token"),
            Role::Deploys => (!c.argocd.enabled).then_some("argocd off"),
```

In `draw`, the presets line gains `Span::styled("w", th.selected()), Span::styled(" sky", th.muted()),` after `" mixed"` (change the `" mixed"` span to `" mixed   "`).

- [ ] **Step 5: Status dots**

`crates/setup/src/ops/systemd.rs`. `LinkDots` gains `pub weather: Dot, pub rain: Dot, pub github: Dot, pub argocd: Dot,`; the literal in `links_from_logs` initialises them `Unknown`; inside the loop after the `prices:` rule:

```rust
        for (needle, dot) in [
            ("weather:", &mut d.weather),
            ("rain:", &mut d.rain),
            ("github:", &mut d.github),
            ("argocd:", &mut d.argocd),
        ] {
            if lower.contains(needle) {
                *dot = if warn { Dot::Down } else { Dot::Up };
            }
        }
```

`crates/setup/src/screens/status.rs`: every `LinkDots { .. }` literal (lines ~409 and ~463 in tests) gains the four fields as `Dot::Unknown`; the dots line becomes two lines:

```rust
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(g.dot, dot_style(s.links.api, th)),
            Span::styled(" kubernetes   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.prometheus, th)),
            Span::styled(" prometheus   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.qbittorrent, th)),
            Span::styled(" qbittorrent   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.argocd, th)),
            Span::styled(" argocd", th.normal()),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(g.dot, dot_style(s.links.electricity, th)),
            Span::styled(" electricity   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.prices, th)),
            Span::styled(" prices   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.weather, th)),
            Span::styled(" weather   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.rain, th)),
            Span::styled(" rain   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.github, th)),
            Span::styled(" github", th.normal()),
        ]));
```

If a Status snapshot test asserts the exact dots line, update its expectation to the two lines.

- [ ] **Step 6: Test, clippy, commit**

Run: `cargo test --workspace --features sim,pi && cargo clippy --workspace --features sim,pi -- -D warnings`

```bash
cargo fmt --all
git add -A
git commit -m "feat(setup): Configure fields, sky preset, role hints and Status dots for the new roles

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 17: Goldens, GIFs, README, version 0.4.0

**Files:**
- Modify: `crates/render/tests/golden.rs`
- Modify: `crates/app/examples/gifs.rs`
- Modify: `README.md`, `Cargo.toml`, the `0.3.0` literals in `crates/setup/src/screens/{screens,update}.rs` tests only if they assert the workspace version (they are fixtures; leave them)

- [ ] **Step 1: Golden tests for the eleven scenes**

`crates/render/tests/golden.rs`. A model with every new source fed, after `electricity_model`:

```rust
fn new_roles_model() -> Model {
    use rackscreen_core::event::{App, AppHealth, AppSync, IssPass, MoonPhase};
    let mut m = ready_model();
    m.set_location_present(true);
    m.set_github_token_present(true);
    m.set_unix_now(1_788_782_400); // 2026-09-07T12:00Z
    m.set_utc_offset_secs(7200);
    for target in [
        LinkTarget::Weather,
        LinkTarget::Rain,
        LinkTarget::Github,
        LinkTarget::ArgoCd,
    ] {
        m.apply(Event::Link { target, up: true }, 0.0);
    }
    m.apply(
        Event::Weather {
            temp_c: 18.0,
            code: 2,
            is_day: true,
            wind_kmh: 23.0,
            gust_kmh: 39.0,
            wind_from_deg: 40.0,
            at: String::new(),
        },
        0.0,
    );
    m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
    let mut rain = vec![0.0f32; 24];
    for (i, v) in [(5, 0.3), (6, 1.0), (7, 2.5), (8, 5.5), (9, 6.5), (10, 4.0), (11, 2.0)] {
        rain[i] = v;
    }
    m.apply(
        Event::Rain {
            from: 1_788_782_400,
            mm_per_h: rain,
        },
        0.0,
    );
    m.apply(
        Event::Sky {
            sunrise: Some(1_788_782_400 - 7 * 3600),
            sunset: Some(1_788_782_400 + 6 * 3600),
            sun_elevation_deg: 45.0,
            moon_illumination: 0.63,
            moon_waxing: true,
            moon_phase: MoonPhase::WaxingGibbous,
        },
        0.0,
    );
    m.apply(
        Event::IssPass(Some(IssPass {
            start: 1_788_782_400 + 42 * 60,
            end: 1_788_782_400 + 48 * 60,
            max_elevation_deg: 62.0,
            visible: true,
        })),
        0.0,
    );
    let days = (0..30)
        .map(|i| (format!("d{i}"), [5u32, 12, 30, 0, 8, 25, 43, 28][i % 8]))
        .collect();
    m.apply(Event::GithubActivity { days }, 0.0);
    m.apply(
        Event::Ups {
            on_battery: false,
            low_battery: false,
            charge_pct: 100.0,
            load_pct: 18.0,
            runtime_secs: 42 * 60,
        },
        0.0,
    );
    m.apply(
        Event::Network {
            rx_bps: 40e6,
            tx_bps: 1.3e6,
        },
        0.0,
    );
    let apps = (0..16)
        .map(|i| App {
            name: format!("app-{i:02}"),
            sync: if i == 5 { AppSync::OutOfSync } else { AppSync::Synced },
            health: AppHealth::Healthy,
            operating: false,
        })
        .collect();
    m.apply(Event::Apps(apps), 0.0);
    // let the smooths settle
    let mut t = 0.0;
    while t < 2.0 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    m
}

#[test]
fn new_roles_idle() {
    let m = new_roles_model();
    let mut r = Renderer::new().unwrap();
    for (role, name) in [
        (Role::GhActivity, "gh_activity"),
        (Role::Weather, "weather"),
        (Role::Wind, "wind"),
        (Role::Aqi, "aqi"),
        (Role::Rain, "rain"),
        (Role::Sun, "sun"),
        (Role::Moon, "moon"),
        (Role::Iss, "iss"),
        (Role::Ups, "ups"),
        (Role::Net, "net"),
        (Role::Deploys, "deploys"),
    ] {
        let mut px = new_pixmap();
        // 2.5 s: the alternating badges are in their first window and faded in
        r.render(&m.scene_for_role(role, 2.5), &mut px);
        check(name, &px);
    }
}

#[test]
fn ups_on_battery_and_iss_during_pass() {
    let mut m = new_roles_model();
    m.apply(
        Event::Ups {
            on_battery: true,
            low_battery: false,
            charge_pct: 80.0,
            load_pct: 18.0,
            runtime_secs: 38 * 60,
        },
        2.0,
    );
    m.set_unix_now(1_788_782_400 + 44 * 60);
    let mut t = 2.0;
    while t < 4.0 {
        m.tick(t);
        t += 1.0 / 30.0;
    }
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Ups, 4.0), &mut px);
    check("ups_on_battery", &px);
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Iss, 4.0), &mut px);
    check("iss_pass", &px);
    let (red, g, b) = pixel(&px, 120, 18);
    assert!(red > 120 && b > 200 && g < 200, "violet ring during the pass, got {red},{g},{b}");
}

#[test]
fn sky_without_location_shows_a_pin() {
    let m = ready_model();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&m.scene_for_role(Role::Sun, 2.0), &mut px);
    check("no_location", &px);
}
```

Run: `cargo test -p rackscreen-render golden`
Expected: the first run writes the new goldens (`wrote golden ...`) and passes; run it again to confirm they are stable. Open a few of `crates/render/tests/goldens/{gh_activity,rain,sun,ups,net,deploys}.png` and compare with the catalog mockups in `.superpowers/brainstorm/64784-1788809704/content/role-catalog-v2.html` before committing them.

- [ ] **Step 2: GIF generator**

`crates/app/examples/gifs.rs`. Doc comment: "one of a screen cycling through all ten roles" becomes "through every role". Replace the `ALL_SECS` constant:

```rust
/// Every role, each dwelling `CYCLE` and then irising for half a second: one
/// whole round of the cycling screen, so its GIF loops seamlessly.
const ALL_SECS: Secs = Role::ALL.len() as Secs * (CYCLE + TRANSITION_SECS);
/// Fixed clock: 2026-09-07 12:00 UTC in Amsterdam, so the sun dial, rain ring
/// and ISS countdown render the same every time.
const UNIX_START: i64 = 1_788_782_400;
const UTC_OFFSET: i32 = 7200;
```

In `clips()`, add arms to the `match role`:

```rust
                Role::GhActivity => vec![
                    (2.0, Step::Cmd(FakeCmd::GithubPush)),
                    (5.0, Step::Cmd(FakeCmd::CiFailure)),
                ],
                Role::Weather => vec![(3.0, Step::Cmd(FakeCmd::Thunder))],
                Role::Rain => vec![(3.0, Step::Cmd(FakeCmd::RainSoon))],
                Role::Iss => vec![(4.0, Step::Cmd(FakeCmd::IssPassNow))],
                Role::Ups => vec![
                    (3.0, Step::Cmd(FakeCmd::UpsToggle)),
                    (7.0, Step::Cmd(FakeCmd::UpsToggle)),
                ],
                Role::Deploys => vec![
                    (3.0, Step::Cmd(FakeCmd::AppToggle)),
                    (6.0, Step::Cmd(FakeCmd::AppToggle)),
                ],
```

In `record`, after `model.set_local_hour(HOUR);`:

```rust
    model.set_location_present(true);
    model.set_github_token_present(true);
    model.set_utc_offset_secs(UTC_OFFSET);
```

after `let mut fake = FakeState::new(SEED);`: `fake.set_clock(UNIX_START, UTC_OFFSET);` and inside the frame loop, right before `model.tick(now);`: `model.set_unix_now(UNIX_START + now as i64);`.

Run: `cargo run -p rackscreen-app --example gifs -- .github/media/screens`
Expected: 24 GIFs (21 roles, `health-torrent`, `all`), the new ones showing the same scenes as the goldens with their scripted moments. `git status` shows the changed `all.gif` and the 11 new files.

- [ ] **Step 3: README**

`README.md`:

1. Intro paragraph: "Six roles come from the cluster, four from Electricity Maps and the day-ahead price feeds" becomes "Nine roles come from the cluster, four from [Electricity Maps](https://app.electricitymaps.com) and the day-ahead price feeds, seven from the sky over your house and one from GitHub".
2. Screens table: append rows in the same two-column format:

```markdown
| <img src=".github/media/screens/gh-activity.gif" alt="GH ACTIVITY role" width="200"> | <img src=".github/media/screens/weather.gif" alt="WEATHER role" width="200"> |
| **`gh-activity`** — today's contributions outside, the last week inside; pushes, stars and CI results splash | **`weather`** — the temperature ring and an icon for the sky; thunder splashes |
| <img src=".github/media/screens/wind.gif" alt="WIND role" width="200"> | <img src=".github/media/screens/aqi.gif" alt="AQI role" width="200"> |
| **`wind`** — a compass arc where the wind comes from, gusts flick it wider | **`aqi`** — the European air quality index on its colour bands |
| <img src=".github/media/screens/rain.gif" alt="RAIN role" width="200"> | <img src=".github/media/screens/sun.gif" alt="SUN role" width="200"> |
| **`rain`** — the next two hours in five-minute slots; rain within 15 min splashes an umbrella | **`sun`** — a 24-hour dial, daylight amber, counting down to sunset or sunrise |
| <img src=".github/media/screens/moon.gif" alt="MOON role" width="200"> | <img src=".github/media/screens/iss.gif" alt="ISS role" width="200"> |
| **`moon`** — the illuminated fraction, lit the way the phase is going | **`iss`** — the ring empties toward the next ISS pass; a visible pass sweeps the rack |
| <img src=".github/media/screens/ups.gif" alt="UPS role" width="200"> | <img src=".github/media/screens/net.gif" alt="NET role" width="200"> |
| **`ups`** — battery outside, load inside; mains loss sweeps the rack red | **`net`** — download outside, upload inside, crawling with the flow |
| <img src=".github/media/screens/deploys.gif" alt="DEPLOYS role" width="200"> | |
| **`deploys`** — one arc per Argo CD application; a sync splashes the rocket | |
```

3. `all.gif` caption: "One screen, all ten roles." becomes "One screen, every role."
4. Roles table: append

```markdown
| `gh-activity` | GitHub | today's contributions against the month's best outside, the last 7 days inside, badge today's count |
| `weather` | Open-Meteo | temperature ring, icon from the WMO weather code, badge °C |
| `wind` | Open-Meteo | compass arc where the wind comes from, badge km/h |
| `aqi` | Open-Meteo | European AQI ring on the EEA colour bands, badge the index |
| `rain` | Buienradar | the next two hours in 5-minute slots, badge minutes to rain |
| `sun` | computed | 24-hour dial with daylight, badge countdown to sunset or sunrise |
| `moon` | computed | illuminated fraction, badge percent alternating with the phase |
| `iss` | Celestrak + SGP4 | countdown to the next ISS pass, badge minutes |
| `ups` | Prometheus (nut-exporter) | battery charge outside, load inside, badge runtime |
| `net` | Prometheus (node-exporter) | download outside, upload inside, badge Mbit/s |
| `deploys` | Argo CD | one arc per application by sync and health, badge healthy over total |
```

5. In the Screens paragraph: "Three presets fill all four screens at once: ... and `m` mixed (...)" becomes "Four presets fill all four screens at once: `c` cluster (`cpu`, `mem`, `pods`, `health`), `e` electricity (`power-mix`, `price`, `carbon`, `renewable`), `m` mixed (each screen alternates a cluster role with an electricity one) and `w` sky (`weather`+`aqi`, `rain`+`wind`, `sun`+`moon`, `iss`+`gh-activity`)."
6. New section after "Electricity mode":

```markdown
## Sky roles

`weather`, `wind`, `aqi`, `rain`, `sun`, `moon` and `iss` all need to know where the rack is: set `location lat` and `location lon` in **Configure** (decimal degrees). Without them these roles show a pin icon.

- **Weather, wind and air quality** come from [Open-Meteo](https://open-meteo.com), free and without a key; turn on `weather enabled`. One forecast call and one air-quality call every `weather poll secs` (600 by default).
- **Rain** is the [Buienradar](https://www.buienradar.nl) two-hour nowcast, five-minute slots, free and without a key, for the Netherlands and Belgium; turn on `rain enabled`. The ring starts at "now" at the top and runs two hours clockwise; the badge counts down to the first wet slot, and rain arriving within fifteen minutes splashes an umbrella.
- **Sun and moon** are computed on the Pi from the location and the clock, nothing to configure.
- **ISS** fetches the station's orbital elements from [Celestrak](https://celestrak.org) once a day and predicts the next pass above `iss min elevation °` (10 by default); turn on `iss enabled`. A pass is marked visible when the sky is dark and the station is still sunlit, and a visible pass sweeps the whole rack violet when it starts.

The sun dial and the rain ring use the Pi's local clock, so set the time zone once with `sudo timedatectl set-timezone Europe/Amsterdam`.

## GitHub role

`gh-activity` shows your contribution calendar: the outer ring is today against your best day of the last 30, the inner ring the last seven days in the calendar greens. Pushes, new stars, merged pull requests and finished CI runs splash on it, and a published release sweeps the rack green.

Create a fine-grained personal access token at github.com with read access to contents, metadata and Actions on the repositories you care about (contributions and the events feed need no extra permission), turn on `github enabled` and paste it into `github token`. The calendar is refreshed every five minutes and the events feed every `github poll secs` (60, the minimum GitHub allows); together with the Actions checks for repositories pushed to recently that is a few hundred requests an hour, far below the limit. Without a token the role shows a key icon.
```

7. Setup TUI Status bullet: "a dot per link (`k8s`, `prometheus`, `qbittorrent`, `electricity`, `prices`)" becomes "a dot per link (`k8s`, `prometheus`, `qbittorrent`, `argocd`, `electricity`, `prices`, `weather`, `rain`, `github`)".
8. Configuration `<details>` block: paste the new sections from `config.example.yaml` in the same place, and add bullets:

```markdown
- **`location`**, **`weather`**, **`rain`**, **`iss`** — see [Sky roles](#sky-roles).
- **`github`** — see [GitHub role](#github-role).
- **`argocd`** — the namespace whose Argo CD applications feed `deploys`; off, and the role shows no data.
```

9. Nav line at the top: add `<a href="#sky-roles">Sky roles</a> &nbsp;·&nbsp;` after Electricity mode.

- [ ] **Step 4: Version**

`Cargo.toml` workspace `version = "0.4.0"`. Run `cargo build` so `Cargo.lock` follows.

- [ ] **Step 5: Full verification and commit**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --features sim,pi
cargo clippy --workspace --features sim,pi -- -D warnings
cargo clippy --no-default-features --features pi -- -D warnings
cargo clippy --no-default-features --features sim -- -D warnings
```

Expected: all clean. Then the simulator once by hand: `cargo run -- run --sim --config config.example.yaml` with a config whose four screens are the sky preset plus `[ups, net, deploys, gh-activity]`, pressing `u d g r f l w i` in the window and watching each splash and sweep land on the right screen.

```bash
git add -A
git commit -m "docs: sky and GitHub roles in the README, goldens and GIFs, v0.4.0

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review notes

- Spec coverage: config (Task 15), events (1), sources (9–13), scenes (3–8), fx (2), model clock (2), TUI (16), docs and GIFs (17), tests in every task. The spec's "generalised `FieldKind::Choice`" is dropped in Task 16 because no new field is a choice; the spec's `Apps(Vec<(String, AppSync, AppHealth)>)` became the `App` struct with an `operating` flag (Task 1); the net rate window is two minutes rather than one (Task 9) so a 60 s scrape still yields two samples.
- Names used across tasks: `Model::{ups, net, apps, github, weather, air, rain, sky, iss}`, `smooth_{ups_charge, ups_load, net_rx, net_tx, gh_today, temp, eaqi, wind}`, `net_phases`, `set_unix_now`, `set_utc_offset_secs`, `set_location_present`, `set_github_token_present`; `FakeState::{set_clock, unix_now}`; `FakeCmd::{UpsToggle, AppToggle, GithubPush, GithubStar, CiFailure, Thunder, RainSoon, IssPassNow}`; `LinkEdge::new_for`; `electricity::{backoff_secs, token_rejected, HttpStatus}` public.

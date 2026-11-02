# New Screens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add nine P2 roles (media, traffic, restarts, uptime, fans, mains, load, ci, jellyfin) with their data sources (seven new Prometheus query groups, the arr queue over the kube tunnel, GitHub Actions runs, a Jellyfin server), two new card kinds, the two missing face inputs, fake-source coverage, goldens, gifs and docs, and ship as 0.8.0.

**Architecture:** Every new role is a `Model → P2Screen` builder under `crates/core/src/p2/roles/` (`rack.rs` for the six Prometheus-backed hardware roles, `media.rs` for media and Jellyfin, `github.rs` gains `ci`) that the existing dispatcher `p2::roles::screen` routes to and `compose` draws. New data arrives as new `Event` variants folded into small per-role state structs on `Model` (`MediaState`, `TrafficState`, `RestartsState`, `UptimeState`, `FansState`, `MainsState`, `LoadState`, `CiState`, `JellyfinState`) plus one `History` (traffic) and two `Hourly` buffers (restarts, mains). Sources: `prometheus.rs` grows new instant and range queries; three new pollers `arr.rs`, `jellyfin.rs` and a CI block in `github.rs`. Two new `Card` kinds (`Download`, `Lines`) and a shared `marquee_run` text helper carry the media and Jellyfin cards.

**Tech Stack:** Rust 2021 workspace (`rackscreen-core`, `rackscreen-render`, `rackscreen-sources`, `rackscreen-app`, `rackscreen-setup`), `serde_json`, `reqwest` (Jellyfin, GitHub), `hyper` over the kube port-forward `Tunnel` (Prometheus, arr), `chrono` (sources only), golden PNG tests, `gif` example. No new crate dependencies.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-new-screens-design.md` (the contract). Spec A (`docs/superpowers/specs/2026-11-02-p2-redesign-design.md`) is implemented on `main`; its layout, ring kinds, card kinds, motion and marquee rules apply unchanged. Mockups (gitignored): `.superpowers/brainstorm/491126-1793618315/content/p2-rest-v2.html` (`SPEC.media/traffic/restarts/uptime/fans/mains/load/ghci`), `media-card-v2.html` (`kind:'download'` card, `md3`), `jellyfin-final.html` (`jfP2`, variant `'card'`).
- `Role::ALL` grows from 22 to **31**, the nine appended at the end in this order: `Media, Traffic, Restarts, Uptime, Fans, Mains, Load, Ci, Jellyfin`; config ids `media, traffic, restarts, uptime, fans, mains, load, ci, jellyfin`; titles `MEDIA`, `WEB TRAFFIC`, `RESTARTS · 24H`, `UPTIME`, `FANS`, `MAINS`, `LOAD`, `CI`, `NOW PLAYING`. The seven Prometheus-backed roles (media, traffic, restarts, uptime, fans, mains, load) are `is_cluster()`; `ci` and `jellyfin` are not. No role is `is_sky()`.
- Colours: `theme::p2` tokens only, plus one new token `theme::p2::ORANGE = #F2A65A` (movies on the media ring, the Jellyfin movie ring). Traffic shares use `INDIGO`, `#6E7BD0`, `#525C9E`. No gradients, blur, glow or icons.
- Geometry and motion are spec A's: hero baseline 100 (104 with a unit below), pill centre 126 (134), card 108 × 48 at pill centre + 18, chart area inset 10/8/14, marquee hold 1.5 s / 30 px/s / gap 36 px, glides 0.65 s. Heroes must fit 180 px: load uses `hero_scale 0.82` from 10.00 up, ci `0.82`, Jellyfin `0.62` for `h:mm:ss` and `0.82` for `m:ss`. Measured with the embedded Inter: `RESTARTS · 24H` is 101.2 px (fits the 104 px title gap), `10.52` at 0.82 is 159 px, `1:41:54` at 0.62 is 160 px, `11/12` at 0.82 is 162 px.
- Prometheus instant queries (every poll) and range queries exactly as listed in Task 3; range windows traffic 90 s / step 3 s, restarts 24 h / 1 h, mains 24 h / 1 h; the hourly ranges are re-run every 300 s, the traffic range only at start. Node names: the `node` label when present (the live cluster carries it on every node-exporter series), else `node_tag(instance)`.
- Config additions: `media: { enabled: false, namespace: arr-stack, radarr: { service: auto, port: 7878, api_key: "" }, sonarr: { service: auto, port: 8989, api_key: "" }, lidarr: { service: auto, port: 8686, api_key: "" } }`, `github.ci_repos: []`, `jellyfin: { enabled: false, url: "", api_key: "", poll_secs: 5 }`. Validation: `jellyfin.poll_secs >= 2`; `jellyfin.url` must start with `http://` or `https://` when set. The TUI gets a seventh Configure group **Media & Jellyfin** (after Sky, before Display); API keys are `Secret` fields like the GitHub token.
- Arr queue: poll every 10 s, `GET /api/v3/queue?pageSize=5&includeMovie=true` (radarr), `/api/v3/queue?pageSize=5&includeSeries=true&includeEpisode=true` (sonarr), `/api/v1/queue?pageSize=5&includeArtist=true&includeAlbum=true` (lidarr), header `X-Api-Key`, `service: auto` picks the first service in the namespace whose name contains `radarr`/`sonarr`/`lidarr` exposing the port. GitHub: `GET /repos/{repo}/actions/runs?per_page=12` every 60 s; empty `ci_repos` = the token owner's repos pushed in the last 7 days, at most 10 (`GET /user/repos?sort=pushed&per_page=30`). Jellyfin: `GET {url}/Sessions?activeWithinSeconds=60` with `Authorization: MediaBrowser Token="{api_key}"`, ticks are 100 ns.
- Face inputs: `recent_restart` = the 24 h restart counter rose between two polls less than 300 s ago; `ci_passed_recently` = the newest completed CI run passed less than 1800 s ago (unix clock).
- Workspace version becomes `0.8.0` (Task 8).
- After every task: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace` green (goldens re-captured with `UPDATE_GOLDENS=1` where a task changes pixels on purpose, and the PNGs committed with the task).
- Commit messages: `type(scope): summary` as in `git log`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## File structure

| File | Responsibility |
|---|---|
| `crates/core/src/theme.rs` (modified) | nine `Role` variants, `ALL: [Role; 31]`, titles, names, `is_cluster`, `p2::ORANGE` |
| `crates/core/src/event.rs` (modified) | `Download`, `NodeLoad`, `CiStatus`, `CiRun`, `JfKind`, `JfSession`, `JfStatus`; the new `Event` variants |
| `crates/core/src/model.rs` (modified) | per-role state structs, buffers, `apply` arms, accessors, `exporter_missing`, configured flags, the two mood inputs |
| `crates/core/src/history.rs` (modified) | `Hourly::last_24` (24 slots ending at the current hour, gaps as `None`) |
| `crates/core/src/format.rs` (modified) | `fmt_left` (`19 min left`), `fmt_mins_secs` (`2m 14s`), `fmt_hms` (`1:41:54` / `5:02`), `fmt_days` |
| `crates/core/src/p2/status.rs` (modified) | `NoData::{Exporter, Config}` |
| `crates/core/src/p2/text.rs` (modified) | `marquee_run`: a clipped, scrolling text run for cards |
| `crates/core/src/p2/chart.rs` (modified) | `Card::Download`, `Card::Lines`, `CardLine` |
| `crates/core/src/p2/roles/rack.rs` (new) | `traffic`, `restarts`, `uptime`, `fans`, `mains`, `load` builders |
| `crates/core/src/p2/roles/media.rs` (new) | `media`, `jellyfin` builders, session pick and position extrapolation |
| `crates/core/src/p2/roles/github.rs` (modified) | `ci` builder |
| `crates/core/src/p2/roles/mod.rs` (modified) | dispatcher arms, hero fit test |
| `crates/core/src/scene_model.rs` (modified) | `needs_data` arms, the `Check config` and `No exporter` gates |
| `crates/sources/src/prometheus.rs` (modified) | new queries, parsers (`parse_matrix_ts`, `node_name`, `short_service`, `short_pod`), events, range prefill |
| `crates/sources/src/arr.rs` (new) | radarr/sonarr/lidarr queue poller over the kube tunnel |
| `crates/sources/src/github.rs` (modified) | CI runs: `parse_ci_runs`, `CiTracker`, repo discovery, per-hour warnings |
| `crates/sources/src/jellyfin.rs` (new) | sessions poller, `parse_sessions` |
| `crates/sources/src/fake.rs` (modified) | state and commands for the nine roles |
| `crates/sources/tests/fixtures/{arr-radarr-queue,arr-sonarr-queue,arr-lidarr-queue,github-ci-runs,github-user-repos,jellyfin-sessions,jellyfin-sessions-empty}.json` (new) | parser fixtures |
| `crates/app/src/config.rs` (modified) | `MediaCfg`, `ArrCfg`, `JellyfinCfg`, `GithubCfg::ci_repos`, validation |
| `crates/app/src/run.rs`, `crates/app/src/runloop.rs` (modified) | spawn arr, CI and Jellyfin; `arr_configured` / `jellyfin_configured` flags |
| `crates/setup/src/screens/configure.rs` (modified) | `Group::Media`, 16 new fields, `FIELDS: [FieldSpec; 59]`, jump keys `1-7` |
| `crates/setup/src/screens/screens.rs` (modified) | hints for `ci` and `jellyfin`; the picker test's last role |
| `crates/render/tests/golden.rs` (modified) | data for the nine roles in `full_model`, warn/alternate goldens |
| `crates/app/examples/gifs.rs` (modified) | clips for the nine roles |
| `README.md`, `config.example.yaml`, `Cargo.toml` (modified) | gallery, role table, new sections, example blocks, 0.8.0 |

Shared signatures every task relies on (all `pub`):

```rust
// core::event (Task 3, 5, 6, 7)
pub struct Download { pub title: String, pub progress: f32 /* 0..=1 */, pub eta_secs: i64 /* <0 unknown */ }
pub struct NodeLoad { pub node: String, pub load1: f32, pub load5: f32, pub load15: f32, pub cores: u32 }
pub enum CiStatus { Success, Failure, InProgress, Other }
pub struct CiRun { pub id: u64, pub repo: String, pub status: CiStatus, pub created_at: i64, pub updated_at: i64, pub duration_secs: Option<i64> }
pub enum JfKind { Movie, Episode, Other }
pub struct JfSession { pub title: String, pub kind: JfKind, pub series: Option<String>, pub season: Option<u32>, pub episode: Option<u32>, pub year: Option<u32>, pub runtime_secs: f64, pub position_secs: f64, pub paused: bool, pub transcoding: bool, pub user: String, pub device: String, pub last_activity: String }
pub enum JfStatus { Ok, Offline, Unauthorized }
Event::NoExporter { role: Role }                       // Task 1
Event::MediaLibrary { movies: u32, series: u32, artists: u32 }
Event::Traffic { req_per_sec: f32, top: Vec<(String, f32)>, error_share: f32 }
Event::TrafficHistory { req_per_sec: Vec<f32> }
Event::Restarts { total_24h: u32, worst: Option<(String, u32)> }
Event::RestartsHistory { hourly: Vec<(i64, f32)> }     // (unix hour, restarts in that hour)
Event::Uptime(Vec<(String, f64)>)                      // (node, seconds up)
Event::Fans(Vec<(String, f32)>)                        // (node, rpm)
Event::Mains { volts: f32, ups_load_pct: f32 }
Event::MainsHistory { hourly: Vec<(i64, f32)> }
Event::Load(Vec<NodeLoad>)
Event::Download(Option<Download>)                      // Task 5
Event::MediaAdded { title: String }
Event::CiRuns(Vec<CiRun>)                              // Task 6, newest first, at most 12
Event::CiRun { repo: String, ok: bool, at: i64 }
Event::Jellyfin { status: JfStatus, sessions: Vec<JfSession> }   // Task 7
// core::model accessors
impl Model {
    pub fn media(&self) -> &MediaState;        pub fn traffic(&self) -> &TrafficState;  pub fn traffic_history(&self) -> &History;
    pub fn restarts(&self) -> &RestartsState;  pub fn restarts_hourly(&self) -> &Hourly;
    pub fn uptime(&self) -> &UptimeState;      pub fn fans(&self) -> &FansState;
    pub fn mains(&self) -> &MainsState;        pub fn mains_hourly(&self) -> &Hourly;
    pub fn load(&self) -> &LoadState;          pub fn ci(&self) -> &CiState;            pub fn jellyfin(&self) -> &JellyfinState;
    pub fn exporter_missing(&self, role: Role) -> bool;
    pub fn set_arr_configured(&mut self, on: bool);       pub fn arr_configured(&self) -> bool;
    pub fn set_jellyfin_configured(&mut self, on: bool);  pub fn jellyfin_configured(&self) -> bool;
}
// core::p2::chart (Task 2)
Card::Download { title: String, progress: f32, label: String, done: bool }
Card::Lines(Vec<CardLine>)   // CardLine { text, font: FontId, px, color, dy }  dy from the chart-area top
// core::p2::text (Task 2)
pub fn marquee_run(ui: &mut ScreenUi, key: UiKey, text: &str, cx: f32, baseline: f32, slot_w: f32, font: FontId, px: f32, color: Color, clip_y: f32, clip_h: f32) -> Vec<Drawable>;
// core::p2::roles (Tasks 4–7)
rack::{traffic, restarts, uptime, fans, mains, load}(model: &Model, now: Secs) -> P2Screen
media::{media, jellyfin}(model: &Model, now: Secs) -> P2Screen
github::ci(model: &Model, now: Secs) -> P2Screen
```

---

### Task 1: Nine roles, status variants, config, TUI and run-loop plumbing

**Files:**
- Modify: `crates/core/src/theme.rs`, `crates/core/src/p2/status.rs`, `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/scene_model.rs`, `crates/core/src/p2/roles/mod.rs`
- Modify: `crates/app/src/config.rs`, `crates/app/src/run.rs`, `crates/app/src/runloop.rs`, `config.example.yaml`
- Modify: `crates/setup/src/screens/configure.rs`, `crates/setup/src/screens/screens.rs`
- Test: unit tests in each of the above; `crates/render/tests/golden.rs` captures `p2_<role>_nodata.png` for the nine (final) and `p2_<role>.png` (replaced in Tasks 4–7)

**Interfaces:**
- Consumes: `Role`, `NoData`, `no_data_screen`, `Config`, `FieldSpec`.
- Produces: `Role::{Media, Traffic, Restarts, Uptime, Fans, Mains, Load, Ci, Jellyfin}`, `theme::p2::ORANGE`, `NoData::{Exporter, Config}`, `Event::NoExporter { role }`, `Model::{exporter_missing, set_arr_configured, arr_configured, set_jellyfin_configured, jellyfin_configured}`, `Model::clear_exporter(role)` (crate-private, called by the `apply` arms of Task 3), `config::{MediaCfg, ArrCfg, JellyfinCfg}`, `GithubCfg::ci_repos`, `MediaCfg::keyed()`, `Group::Media`, `RenderLoop::{arr_configured, jellyfin_configured}`.

- [ ] **Step 1: Write the failing role tests**

In `crates/core/src/theme.rs` replace the test `twenty_two_roles_with_unique_names` with:

```rust
    #[test]
    fn thirty_one_roles_with_unique_names() {
        assert_eq!(Role::ALL.len(), 31);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 31);
        assert_eq!(Role::parse("face"), Some(Role::Face));
        assert!(Role::Face.is_cluster());
        assert!(!Role::Face.is_sky());
        assert_eq!(Role::ALL[21], Role::Face);
        assert_eq!(
            &Role::ALL[22..],
            &[
                Role::Media,
                Role::Traffic,
                Role::Restarts,
                Role::Uptime,
                Role::Fans,
                Role::Mains,
                Role::Load,
                Role::Ci,
                Role::Jellyfin
            ]
        );
        for (name, role) in [
            ("media", Role::Media),
            ("traffic", Role::Traffic),
            ("restarts", Role::Restarts),
            ("uptime", Role::Uptime),
            ("fans", Role::Fans),
            ("mains", Role::Mains),
            ("load", Role::Load),
            ("ci", Role::Ci),
            ("jellyfin", Role::Jellyfin),
        ] {
            assert_eq!(Role::parse(name), Some(role));
            assert!(!role.is_sky());
        }
        assert_eq!(Role::Restarts.title(), "RESTARTS · 24H");
        assert_eq!(Role::Traffic.title(), "WEB TRAFFIC");
        assert_eq!(Role::Jellyfin.title(), "NOW PLAYING");
        assert!(!Role::Ci.is_cluster() && !Role::Jellyfin.is_cluster());
        assert_eq!(Role::parse("gh-activity"), Some(Role::GhActivity));
        assert_eq!(Role::parse("deploys"), Some(Role::Deploys));
        let sky: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_sky()).collect();
        assert_eq!(
            sky,
            vec![
                Role::Weather,
                Role::Wind,
                Role::Aqi,
                Role::Rain,
                Role::Sun,
                Role::Moon,
                Role::Iss
            ]
        );
        assert_eq!(p2::ORANGE, Color::hex(0xF2A65A));
    }
```

and in `cluster_roles_are_the_kubernetes_ones` extend the expected vector:

```rust
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
                Role::Face,
                Role::Media,
                Role::Traffic,
                Role::Restarts,
                Role::Uptime,
                Role::Fans,
                Role::Mains,
                Role::Load,
            ]
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rackscreen-core theme::`
Expected: compile error, `no variant named Media`.

- [ ] **Step 3: Add the roles and the token**

In `crates/core/src/theme.rs`:

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
    Face,
    Media,
    Traffic,
    Restarts,
    Uptime,
    Fans,
    Mains,
    Load,
    Ci,
    Jellyfin,
}

impl Role {
    pub const ALL: [Role; 31] = [
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
        Role::Face,
        Role::Media,
        Role::Traffic,
        Role::Restarts,
        Role::Uptime,
        Role::Fans,
        Role::Mains,
        Role::Load,
        Role::Ci,
        Role::Jellyfin,
    ];
```

Add arms to `title()` (after `Role::Face => "FACE",`):

```rust
            Role::Media => "MEDIA",
            Role::Traffic => "WEB TRAFFIC",
            Role::Restarts => "RESTARTS · 24H",
            Role::Uptime => "UPTIME",
            Role::Fans => "FANS",
            Role::Mains => "MAINS",
            Role::Load => "LOAD",
            Role::Ci => "CI",
            Role::Jellyfin => "NOW PLAYING",
```

to `name()` (after `Role::Face => "face",`):

```rust
            Role::Media => "media",
            Role::Traffic => "traffic",
            Role::Restarts => "restarts",
            Role::Uptime => "uptime",
            Role::Fans => "fans",
            Role::Mains => "mains",
            Role::Load => "load",
            Role::Ci => "ci",
            Role::Jellyfin => "jellyfin",
```

and to `is_cluster()` (after `| Role::Face`):

```rust
                | Role::Media
                | Role::Traffic
                | Role::Restarts
                | Role::Uptime
                | Role::Fans
                | Role::Mains
                | Role::Load
```

In `pub mod p2` add after `GREEN`:

```rust
    /// Movies on the media ring and the Jellyfin movie ring.
    pub const ORANGE: Color = Color::hex(0xF2A65A);
```

- [ ] **Step 4: Status variants, the no-exporter event and the model flags**

`crates/core/src/p2/status.rs`: extend the enum and `no_data_screen`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoData {
    /// The source is configured but has not delivered yet, or went quiet.
    Waiting,
    /// An API token is missing from the config.
    Token,
    /// `location.lat` / `lon` are missing from the config.
    Location,
    /// Prometheus answers but has no series for this role.
    Exporter,
    /// A URL or similar the role cannot work without is empty in the config.
    Config,
}

pub fn no_data_screen(role: Role, why: NoData) -> P2Screen {
    let mut s = P2Screen::new(role.title(), "—");
    s.hero_color = TEXT3;
    s.ring = RingSpec::empty();
    s.pill = Some(match why {
        NoData::Waiting => ("Waiting for data".into(), TEXT2),
        NoData::Token => ("Token needed".into(), WARN),
        NoData::Location => ("Location needed".into(), WARN),
        NoData::Exporter => ("No exporter".into(), WARN),
        NoData::Config => ("Check config".into(), WARN),
    });
    s
}
```

Append to its test `no_data_screens_show_a_dash_and_a_pill`:

```rust
        assert_eq!(
            no_data_screen(Role::Fans, NoData::Exporter).pill,
            Some(("No exporter".into(), WARN))
        );
        assert_eq!(
            no_data_screen(Role::Jellyfin, NoData::Config),
            P2Screen {
                pill: Some(("Check config".into(), WARN)),
                ..no_data_screen(Role::Jellyfin, NoData::Waiting)
            }
        );
```

`crates/core/src/event.rs`: add `use crate::theme::Role;` at the top and the variant at the end of `Event` (before `ForceNight`):

```rust
    /// Prometheus answered but has no series for the role (exporter not
    /// installed); the next data event for the role clears it.
    NoExporter {
        role: Role,
    },
```

`crates/core/src/model.rs`: three fields on `Model` (after `bedtime_near: bool,`):

```rust
    /// Roles whose Prometheus query came back empty on the last poll.
    exporter_missing: Vec<Role>,
    /// An arr API key is configured: the media card never falls back to the
    /// qBittorrent torrent.
    arr_configured: bool,
    /// `jellyfin.url` is set; without it the role says "Check config".
    jellyfin_configured: bool,
```

initialise them in `Model::new` (after `bedtime_near: false,`):

```rust
            exporter_missing: Vec::new(),
            arr_configured: false,
            jellyfin_configured: false,
```

add the accessors next to `set_github_token_present`:

```rust
    pub fn exporter_missing(&self, role: Role) -> bool {
        self.exporter_missing.contains(&role)
    }
    pub(crate) fn clear_exporter(&mut self, role: Role) {
        self.exporter_missing.retain(|r| *r != role);
    }
    pub fn set_arr_configured(&mut self, on: bool) {
        self.arr_configured = on;
    }
    pub fn arr_configured(&self) -> bool {
        self.arr_configured
    }
    pub fn set_jellyfin_configured(&mut self, on: bool) {
        self.jellyfin_configured = on;
    }
    pub fn jellyfin_configured(&self) -> bool {
        self.jellyfin_configured
    }
```

and the `apply` arm (before `Event::ForceNight(v) => ...`):

```rust
            Event::NoExporter { role } => {
                if !self.exporter_missing.contains(&role) {
                    self.exporter_missing.push(role);
                }
            }
```

Add a model test:

```rust
    #[test]
    fn no_exporter_flags_a_role_until_cleared() {
        let mut m = Model::new(Thresholds::default());
        assert!(!m.exporter_missing(Role::Fans));
        m.apply(Event::NoExporter { role: Role::Fans }, 0.0);
        m.apply(Event::NoExporter { role: Role::Fans }, 1.0);
        assert!(m.exporter_missing(Role::Fans));
        assert!(!m.exporter_missing(Role::Mains));
        m.clear_exporter(Role::Fans);
        assert!(!m.exporter_missing(Role::Fans));
        assert!(!m.arr_configured() && !m.jellyfin_configured());
        m.set_arr_configured(true);
        m.set_jellyfin_configured(true);
        assert!(m.arr_configured() && m.jellyfin_configured());
    }
```

`crates/core/src/scene_model.rs`: add the nine arms to `needs_data` (placeholder `true` until each role's data lands; Tasks 4–7 replace them):

```rust
            Role::Media
            | Role::Traffic
            | Role::Restarts
            | Role::Uptime
            | Role::Fans
            | Role::Mains
            | Role::Load
            | Role::Ci
            | Role::Jellyfin => true,
```

and the two gates in `scene_for_role`, right after the `wants_connecting` return:

```rust
        if role == Role::Jellyfin && !self.jellyfin_configured() {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Config), ui, now);
        }
        if self.link().prom && self.exporter_missing(role) {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Exporter), ui, now);
        }
```

Add to its tests:

```rust
    #[test]
    fn new_roles_wait_jellyfin_checks_config_and_exporters_can_be_missing() {
        let mut m = ready_model();
        for role in [Role::Media, Role::Traffic, Role::Load, Role::Ci] {
            assert_eq!(pill_of(&for_role(&m, role, 1.0)).as_deref(), Some("Waiting for data"), "{role:?}");
        }
        assert_eq!(pill_of(&for_role(&m, Role::Jellyfin, 1.0)).as_deref(), Some("Check config"));
        m.set_jellyfin_configured(true);
        assert_eq!(pill_of(&for_role(&m, Role::Jellyfin, 1.0)).as_deref(), Some("Waiting for data"));
        m.apply(Event::NoExporter { role: Role::Fans }, 1.0);
        assert_eq!(pill_of(&for_role(&m, Role::Fans, 1.0)).as_deref(), Some("No exporter"));
        m.apply(link(LinkTarget::Prometheus, false), 2.0);
        assert_eq!(
            pill_of(&for_role(&m, Role::Fans, 2.0)).as_deref(),
            Some("Waiting for data"),
            "a missing exporter only means something while Prometheus answers"
        );
        assert!(connecting(&m.scene_for_role(Role::Uptime, 2.0, &mut ScreenUi::new())) == false);
        m.apply(link(LinkTarget::K8sApi, false), 3.0);
        assert!(connecting(&for_role(&m, Role::Uptime, 3.0)), "cluster role");
        assert!(!connecting(&for_role(&m, Role::Ci, 3.0)), "ci is not a cluster role");
    }
```

`crates/core/src/p2/roles/mod.rs`: make the dispatcher exhaustive:

```rust
        Role::Renewable => system::renewable(model, now),
        Role::Media
        | Role::Traffic
        | Role::Restarts
        | Role::Uptime
        | Role::Fans
        | Role::Mains
        | Role::Load
        | Role::Ci
        | Role::Jellyfin => return None, // builders arrive with their data (Tasks 4–7)
        Role::PowerMix | Role::Face => return None,
```

and extend `worst_case_heroes_fit_180_px`:

```rust
            ("9999", 1.0),    // media library, restarts
            ("99.9", 1.0),    // traffic req/s
            ("6.1k", 1.0),    // fans
            ("260", 1.0),     // mains
            ("10.52", 0.82),  // load from 10 up
            ("12/12", 0.82),  // ci
            ("1:41:54", 0.62), // jellyfin h:mm:ss
            ("59:59", 0.82),  // jellyfin m:ss
```

- [ ] **Step 5: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: all pass (including `thirty_one_roles_with_unique_names`, `no_exporter_flags_a_role_until_cleared`, `new_roles_wait_jellyfin_checks_config_and_exporters_can_be_missing`).

- [ ] **Step 6: Write the failing config tests**

Append to the tests in `crates/app/src/config.rs`:

```rust
    #[test]
    fn media_github_ci_and_jellyfin_defaults_and_validation() {
        let c = Config::default();
        assert!(!c.media.enabled);
        assert_eq!(c.media.namespace, "arr-stack");
        assert_eq!((c.media.radarr.service.as_str(), c.media.radarr.port), ("auto", 7878));
        assert_eq!((c.media.sonarr.service.as_str(), c.media.sonarr.port), ("auto", 8989));
        assert_eq!((c.media.lidarr.service.as_str(), c.media.lidarr.port), ("auto", 8686));
        assert!(c.media.radarr.api_key.is_empty() && !c.media.keyed());
        assert!(c.github.ci_repos.is_empty());
        assert!(!c.jellyfin.enabled && c.jellyfin.url.is_empty() && c.jellyfin.api_key.is_empty());
        assert_eq!(c.jellyfin.poll_secs, 5);
        c.validate().unwrap();
        let mut c = c;
        c.media.enabled = true;
        c.media.sonarr.api_key = "k".into();
        assert!(c.media.keyed());
        c.media.enabled = false;
        assert!(!c.media.keyed(), "keys only count while media is on");
        c.jellyfin.poll_secs = 1;
        assert!(c.validate().unwrap_err().to_string().contains("jellyfin.poll_secs"));
        c.jellyfin.poll_secs = 2;
        c.jellyfin.url = "192.168.0.10:8096".into();
        assert!(c.validate().unwrap_err().to_string().contains("jellyfin.url"));
        c.jellyfin.url = "http://192.168.0.10:8096".into();
        c.validate().unwrap();
        c.jellyfin.url = "https://jf.example".into();
        c.validate().unwrap();
        let yaml = "github: { ci_repos: [a/b, c/d] }\njellyfin: { enabled: true, url: \"http://x:8096\", api_key: k }\nmedia: { enabled: true, radarr: { api_key: r } }\n";
        let c = Config::from_yaml(yaml).unwrap();
        assert_eq!(c.github.ci_repos, vec!["a/b".to_string(), "c/d".to_string()]);
        assert!(c.jellyfin.enabled && c.jellyfin.api_key == "k");
        assert_eq!(c.media.radarr.port, 7878, "partial arr block keeps the defaults");
        assert_eq!(c.media.radarr.api_key, "r");
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
    }
```

- [ ] **Step 7: Run to verify it fails**

Run: `cargo test -p rackscreen-app config::tests::media_github`
Expected: compile error, `no field media`.

- [ ] **Step 8: Add the config structs, defaults, validation and the example blocks**

In `crates/app/src/config.rs` add fields to `Config` (after `argocd`):

```rust
    #[serde(default)]
    pub media: MediaCfg,
    #[serde(default)]
    pub jellyfin: JellyfinCfg,
```

add `ci_repos` to `GithubCfg`:

```rust
pub struct GithubCfg {
    pub enabled: bool,
    pub token: String,
    pub poll_secs: u64,
    /// `owner/repo` list for the `ci` role; empty = your repos pushed in the last 7 days.
    pub ci_repos: Vec<String>,
}
```

add the new structs after `ArgocdCfg`:

```rust
/// One *arr app reached over the kube tunnel for the media download card.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ArrCfg {
    /// Service name, or `auto` = the first service containing the app name on `port`.
    pub service: String,
    pub port: u16,
    pub api_key: String,
}

/// Radarr, Sonarr and Lidarr queues for the media role's download card.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct MediaCfg {
    pub enabled: bool,
    pub namespace: String,
    pub radarr: ArrCfg,
    pub sonarr: ArrCfg,
    pub lidarr: ArrCfg,
}

impl MediaCfg {
    /// Media is on and at least one app has an API key: the arr queue is polled
    /// and the card never falls back to the qBittorrent torrent.
    pub fn keyed(&self) -> bool {
        self.enabled
            && [&self.radarr, &self.sonarr, &self.lidarr]
                .iter()
                .any(|a| !a.api_key.trim().is_empty())
    }
}

/// Jellyfin now-playing sessions; the server lives outside the cluster.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct JellyfinCfg {
    pub enabled: bool,
    /// `http://host:8096`; empty until filled in, the role then says "Check config".
    pub url: String,
    pub api_key: String,
    pub poll_secs: u64,
}
```

defaults (after `impl Default for ArgocdCfg`):

```rust
impl Default for ArrCfg {
    fn default() -> Self {
        Self {
            service: "auto".into(),
            port: 0,
            api_key: String::new(),
        }
    }
}
impl Default for MediaCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            namespace: "arr-stack".into(),
            radarr: ArrCfg {
                port: 7878,
                ..ArrCfg::default()
            },
            sonarr: ArrCfg {
                port: 8989,
                ..ArrCfg::default()
            },
            lidarr: ArrCfg {
                port: 8686,
                ..ArrCfg::default()
            },
        }
    }
}
impl Default for JellyfinCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            api_key: String::new(),
            poll_secs: 5,
        }
    }
}
```

Note the per-app port: `ArrCfg::default()` has port 0 so a bare `radarr: { api_key: r }` would lose 7878. Fix that in `normalize`:

```rust
    /// Upgrade legacy fields in place and fill the per-app arr ports.
    pub fn normalize(&mut self) {
        for s in &mut self.screens {
            if let Some(r) = s.role.take() {
                if s.roles.is_empty() {
                    s.roles = vec![r];
                }
            }
        }
        let defaults = MediaCfg::default();
        for (app, d) in [
            (&mut self.media.radarr, &defaults.radarr),
            (&mut self.media.sonarr, &defaults.sonarr),
            (&mut self.media.lidarr, &defaults.lidarr),
        ] {
            if app.port == 0 {
                app.port = d.port;
            }
            if app.service.trim().is_empty() {
                app.service = d.service.clone();
            }
        }
    }
```

`GithubCfg::default()` gains `ci_repos: Vec::new(),`. Validation, appended to `validate()` before `Ok(())`:

```rust
        anyhow::ensure!(
            self.jellyfin.poll_secs >= 2,
            "jellyfin.poll_secs must be at least 2 (got {})",
            self.jellyfin.poll_secs
        );
        let url = self.jellyfin.url.trim();
        anyhow::ensure!(
            url.is_empty() || url.starts_with("http://") || url.starts_with("https://"),
            "jellyfin.url must start with http:// or https:// (got {url})"
        );
```

`config.example.yaml`: replace the `github:` block and add two blocks after `argocd:`:

```yaml
github:
  enabled: false           # fine-grained token: read access to contributions, events and Actions
  token: ""
  poll_secs: 60
  ci_repos: []             # owner/repo for the ci role; empty = your repos with runs in the last 7 days

argocd:
  enabled: true            # watch applications.argoproj.io for the deploys role
  namespace: argocd

media:
  enabled: false           # arr queues for the media role's download card (library counts come from Prometheus)
  namespace: arr-stack
  radarr: { service: auto, port: 7878, api_key: "" }
  sonarr: { service: auto, port: 8989, api_key: "" }
  lidarr: { service: auto, port: 8686, api_key: "" }

jellyfin:
  enabled: false           # now playing from a Jellyfin server, outside the cluster
  url: ""                  # e.g. http://192.168.0.x:8096
  api_key: ""              # Jellyfin Dashboard → API Keys
  poll_secs: 5             # >= 2
```

and the role comment:

```yaml
# Roles: cpu, mem, pods, health, thermal, storage, power-mix, price, carbon, renewable,
#        gh-activity, weather, wind, aqi, rain, sun, moon, iss, ups, net, deploys, face,
#        media, traffic, restarts, uptime, fans, mains, load, ci, jellyfin
```

- [ ] **Step 9: Run the app tests**

Run: `cargo test -p rackscreen-app`
Expected: all pass (the example YAML still parses with four screens; the new test passes).

- [ ] **Step 10: Plumb the two flags through the run loop**

`crates/app/src/runloop.rs`: two fields on `RenderLoop` after `github_token_present`:

```rust
    /// An arr API key is configured: the media card never shows the torrent.
    pub arr_configured: bool,
    /// `jellyfin.url` is set; without it the role says "Check config".
    pub jellyfin_configured: bool,
```

and after `model.set_github_token_present(self.github_token_present);` in `run`:

```rust
        model.set_arr_configured(self.arr_configured);
        model.set_jellyfin_configured(self.jellyfin_configured);
```

`crates/app/src/run.rs`, in `Monitor::start` where `RenderLoop` is built, after `github_token_present: ...,`:

```rust
            arr_configured: matches!(source, SourceKind::Fake) || cfg.media.keyed(),
            jellyfin_configured: matches!(source, SourceKind::Fake)
                || (cfg.jellyfin.enabled && !cfg.jellyfin.url.trim().is_empty()),
```

(The sources themselves are spawned in Tasks 5–7.) Search the workspace for other `RenderLoop {` literals (`grep -rn "RenderLoop {" crates`) and add the two fields with `false` where one is constructed in a test.

- [ ] **Step 11: Write the failing TUI tests**

In `crates/setup/src/screens/configure.rs` tests: change both `assert_eq!(FIELDS.len(), 43);` to `59`, `assert_eq!(screen.sidebar().unwrap().items.len(), 6);` to `7`, and in `tab_and_digits_switch_groups` replace the `'5'`/`'1'`/`'6'` block with:

```rust
        screen.handle(KeyEvent::from(KeyCode::Char('5')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Media);
        assert_eq!(FIELDS[screen.row].field, Field::MediaEnabled);
        screen.handle(KeyEvent::from(KeyCode::Char('6')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Display);
        assert_eq!(FIELDS[screen.row].field, Field::Brightness);
        screen.handle(KeyEvent::from(KeyCode::Char('1')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Cluster);
        screen.handle(KeyEvent::from(KeyCode::Char('7')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Thresholds);
        assert!(!screen.dirty, "navigation never dirties the form");
        assert!(screen.keys().contains("1-7 jump"));
```

Add a new test:

```rust
    #[test]
    fn media_and_jellyfin_fields() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::MediaEnabled), "false");
        assert_eq!(get(&c, Field::RadarrPort), "7878");
        assert_eq!(get(&c, Field::LidarrService), "auto");
        assert_eq!(get(&c, Field::JellyfinPoll), "5");
        assert_eq!(get(&c, Field::GithubCiRepos), "");
        set(&mut c, Field::GithubCiRepos, " a/b, c/d ,,").unwrap();
        assert_eq!(c.github.ci_repos, vec!["a/b".to_string(), "c/d".to_string()]);
        assert_eq!(get(&c, Field::GithubCiRepos), "a/b, c/d");
        set(&mut c, Field::RadarrKey, "rk").unwrap();
        assert_eq!(c.media.radarr.api_key, "rk");
        set(&mut c, Field::SonarrPort, "8990").unwrap();
        assert_eq!(c.media.sonarr.port, 8990);
        assert!(set(&mut c, Field::SonarrPort, "x").is_err());
        set(&mut c, Field::JellyfinUrl, "http://jf:8096").unwrap();
        assert!(set(&mut c, Field::JellyfinUrl, "jf:8096")
            .unwrap_err()
            .contains("http"));
        set(&mut c, Field::JellyfinUrl, "").unwrap();
        set(&mut c, Field::JellyfinPoll, "1").unwrap();
        assert_eq!(c.jellyfin.poll_secs, 2, "floored at 2");
        set(&mut c, Field::JellyfinKey, "jk").unwrap();
        assert_eq!(c.jellyfin.api_key, "jk");
        for f in [Field::RadarrKey, Field::SonarrKey, Field::LidarrKey, Field::JellyfinKey] {
            assert!(
                FIELDS.iter().any(|s| s.field == f && s.kind == FieldKind::Secret),
                "{f:?} is masked"
            );
        }
        assert_eq!(Group::Media.name(), "Media & Jellyfin");
        assert_eq!(Group::ALL[4], Group::Media);
        assert_eq!(group_indices(Group::Media).len(), 15);
        assert!(c.validate().is_ok());
    }
```

In `crates/setup/src/screens/screens.rs` tests: in `picker_scrolls_to_keep_the_cursor_visible` change `"▸ ○  face"` to `"▸ ○  jellyfin"`, and append to `sky_and_github_hints`:

```rust
        assert_eq!(s.role_hint(Role::Jellyfin), Some("no url"));
        assert_eq!(s.role_hint(Role::Ci), None, "token set above");
        assert_eq!(s.role_hint(Role::Media), None);
        {
            let cfg = s.cfg.as_mut().unwrap();
            cfg.github.token.clear();
            cfg.jellyfin.enabled = true;
            cfg.jellyfin.url = "http://jf:8096".into();
        }
        assert_eq!(s.role_hint(Role::Ci), Some("no token"));
        assert_eq!(s.role_hint(Role::Jellyfin), None);
```

- [ ] **Step 12: Run to verify they fail**

Run: `cargo test -p rackscreen-setup`
Expected: compile errors (`Field::MediaEnabled`, `Group::Media` missing).

- [ ] **Step 13: Add the group, fields, get/set and hints**

`crates/setup/src/screens/configure.rs`: append to `enum Field`:

```rust
    GithubCiRepos,
    MediaEnabled,
    MediaNamespace,
    RadarrService,
    RadarrPort,
    RadarrKey,
    SonarrService,
    SonarrPort,
    SonarrKey,
    LidarrService,
    LidarrPort,
    LidarrKey,
    JellyfinEnabled,
    JellyfinUrl,
    JellyfinKey,
    JellyfinPoll,
```

`Group` and its impl:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cluster,
    Services,
    Energy,
    Sky,
    Media,
    Display,
    Thresholds,
}

impl Group {
    pub const ALL: [Group; 7] = [
        Group::Cluster,
        Group::Services,
        Group::Energy,
        Group::Sky,
        Group::Media,
        Group::Display,
        Group::Thresholds,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Group::Cluster => "Cluster",
            Group::Services => "Services",
            Group::Energy => "Energy",
            Group::Sky => "Sky",
            Group::Media => "Media & Jellyfin",
            Group::Display => "Display",
            Group::Thresholds => "Thresholds",
        }
    }
```

`use Group::{Cluster, Display, Energy, Media, Services, Sky, Thresholds};` and `pub const FIELDS: [FieldSpec; 59]`. Insert after the `GithubPoll` spec (still in `Services`):

```rust
    spec(
        Field::GithubCiRepos,
        Services,
        "GitHub",
        "ci repos",
        Text,
        "owner/repo, comma separated; empty: your recent repos.",
    ),
```

and insert after the `IssMinElevation` spec, before `Brightness`:

```rust
    spec(
        Field::MediaEnabled,
        Media,
        "Media",
        "enabled",
        Bool,
        "Poll the arr queues for the media download card.",
    ),
    spec(
        Field::MediaNamespace,
        Media,
        "Media",
        "namespace",
        Text,
        "Namespace the arr services run in.",
    ),
    spec(
        Field::RadarrService,
        Media,
        "Radarr",
        "service",
        Text,
        "Service name, or auto.",
    ),
    spec(
        Field::RadarrPort,
        Media,
        "Radarr",
        "port",
        Number,
        "Radarr web port, usually 7878.",
    ),
    spec(
        Field::RadarrKey,
        Media,
        "Radarr",
        "api key",
        Secret,
        "Radarr Settings → General → API Key.",
    ),
    spec(
        Field::SonarrService,
        Media,
        "Sonarr",
        "service",
        Text,
        "Service name, or auto.",
    ),
    spec(
        Field::SonarrPort,
        Media,
        "Sonarr",
        "port",
        Number,
        "Sonarr web port, usually 8989.",
    ),
    spec(
        Field::SonarrKey,
        Media,
        "Sonarr",
        "api key",
        Secret,
        "Sonarr Settings → General → API Key.",
    ),
    spec(
        Field::LidarrService,
        Media,
        "Lidarr",
        "service",
        Text,
        "Service name, or auto.",
    ),
    spec(
        Field::LidarrPort,
        Media,
        "Lidarr",
        "port",
        Number,
        "Lidarr web port, usually 8686.",
    ),
    spec(
        Field::LidarrKey,
        Media,
        "Lidarr",
        "api key",
        Secret,
        "Lidarr Settings → General → API Key.",
    ),
    spec(
        Field::JellyfinEnabled,
        Media,
        "Jellyfin",
        "enabled",
        Bool,
        "Now playing from your Jellyfin server.",
    ),
    spec(
        Field::JellyfinUrl,
        Media,
        "Jellyfin",
        "url",
        Text,
        "http://host:8096; the server is outside the cluster.",
    ),
    spec(
        Field::JellyfinKey,
        Media,
        "Jellyfin",
        "api key",
        Secret,
        "Jellyfin Dashboard → API Keys.",
    ),
    spec(
        Field::JellyfinPoll,
        Media,
        "Jellyfin",
        "poll",
        Number,
        "Seconds between session polls, at least 2.",
    ),
```

`get` arms (after `Field::ArgoNamespace => ...`):

```rust
        Field::GithubCiRepos => cfg.github.ci_repos.join(", "),
        Field::MediaEnabled => cfg.media.enabled.to_string(),
        Field::MediaNamespace => cfg.media.namespace.clone(),
        Field::RadarrService => cfg.media.radarr.service.clone(),
        Field::RadarrPort => cfg.media.radarr.port.to_string(),
        Field::RadarrKey => cfg.media.radarr.api_key.clone(),
        Field::SonarrService => cfg.media.sonarr.service.clone(),
        Field::SonarrPort => cfg.media.sonarr.port.to_string(),
        Field::SonarrKey => cfg.media.sonarr.api_key.clone(),
        Field::LidarrService => cfg.media.lidarr.service.clone(),
        Field::LidarrPort => cfg.media.lidarr.port.to_string(),
        Field::LidarrKey => cfg.media.lidarr.api_key.clone(),
        Field::JellyfinEnabled => cfg.jellyfin.enabled.to_string(),
        Field::JellyfinUrl => cfg.jellyfin.url.clone(),
        Field::JellyfinKey => cfg.jellyfin.api_key.clone(),
        Field::JellyfinPoll => cfg.jellyfin.poll_secs.to_string(),
```

`set` arms:

```rust
        Field::GithubCiRepos => {
            cfg.github.ci_repos = t
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect();
        }
        Field::MediaEnabled => cfg.media.enabled = t == "true",
        Field::MediaNamespace => cfg.media.namespace = t.into(),
        Field::RadarrService => cfg.media.radarr.service = t.into(),
        Field::RadarrPort => cfg.media.radarr.port = num(t, "port")?,
        Field::RadarrKey => cfg.media.radarr.api_key = text.into(),
        Field::SonarrService => cfg.media.sonarr.service = t.into(),
        Field::SonarrPort => cfg.media.sonarr.port = num(t, "port")?,
        Field::SonarrKey => cfg.media.sonarr.api_key = text.into(),
        Field::LidarrService => cfg.media.lidarr.service = t.into(),
        Field::LidarrPort => cfg.media.lidarr.port = num(t, "port")?,
        Field::LidarrKey => cfg.media.lidarr.api_key = text.into(),
        Field::JellyfinEnabled => cfg.jellyfin.enabled = t == "true",
        Field::JellyfinUrl => {
            if !t.is_empty() && !t.starts_with("http://") && !t.starts_with("https://") {
                return Err("url must start with http:// or https://".into());
            }
            cfg.jellyfin.url = t.into();
        }
        Field::JellyfinKey => cfg.jellyfin.api_key = text.into(),
        Field::JellyfinPoll => cfg.jellyfin.poll_secs = num::<u64>(t, "poll secs")?.max(2),
```

Jump keys: `KeyCode::Char(c @ '1'..='7') => {` and the help string `"↑↓ field  Tab/←→ group  1-7 jump  ⏎ edit  s save  Esc back"`; the sidebar comment `// Numbered, so the `1-7` jump keys explain themselves.`

`crates/setup/src/screens/screens.rs` `role_hint`, before `_ => None,`:

```rust
            Role::Ci => c.github.token.is_empty().then_some("no token"),
            Role::Jellyfin => {
                (!c.jellyfin.enabled || c.jellyfin.url.trim().is_empty()).then_some("no url")
            }
```

- [ ] **Step 14: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green. `every_role_normal` and `no_data_token_location_and_connecting` write 18 new PNGs under `crates/render/tests/goldens/` (`p2_media.png` … `p2_jellyfin_nodata.png`); the nine `p2_<role>.png` are "Waiting for data" tiles for now and are replaced in Tasks 4–7. Open two of them to confirm the title renders (`RESTARTS · 24H` must be on one line).

- [ ] **Step 15: Commit**

```bash
git add -A
git commit -m "feat(core,app,setup): nine new roles, media/jellyfin/ci config and TUI group, no-exporter and check-config states

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Card kinds `Download` and `Lines`, `marquee_run`, formatters

**Files:**
- Modify: `crates/core/src/p2/text.rs`, `crates/core/src/p2/chart.rs`, `crates/core/src/format.rs`
- Test: unit tests in each; `crates/render/tests/golden.rs` gains `p2_sample_download` and `p2_sample_lines`

**Interfaces:**
- Consumes: `ScreenUi::{marquee, glide}`, `Marquee::offset`, `measure`, `middle_baseline`, `run_centred`, `ClipRect`.
- Produces: `text::marquee_run(ui, key, text, cx, baseline, slot_w, font, px, color, clip_y, clip_h) -> Vec<Drawable>`; `chart::Card::Download { title, progress, label, done }`, `chart::Card::Lines(Vec<CardLine>)`, `chart::CardLine { text, font, px, color, dy }`; `format::{fmt_left, fmt_mins_secs, fmt_hms, fmt_days}`.

- [ ] **Step 1: Write the failing tests**

Append to the tests in `crates/core/src/p2/text.rs`:

```rust
    #[test]
    fn marquee_run_centres_short_text_and_scrolls_long_text() {
        let mut ui = ScreenUi::new();
        let short = marquee_run(
            &mut ui,
            ("line", 0),
            "Dune",
            120.0,
            160.0,
            88.0,
            FontId::SemiBold,
            10.0,
            TEXT,
            150.0,
            20.0,
            0.0,
        );
        let r = runs(&short);
        assert_eq!(r.len(), 1);
        let w = measure(FontId::SemiBold, 10.0, "Dune", 0.0);
        assert!((r[0].0 - (120.0 - w / 2.0)).abs() < 1e-3, "centred");
        assert!(!r[0].5, "no clip when it fits");
        let long = "The Lord of the Rings: The Return of the King";
        let at = |ui: &mut ScreenUi, now: f64| {
            marquee_run(
                ui,
                ("line", 1),
                long,
                120.0,
                160.0,
                88.0,
                FontId::SemiBold,
                10.0,
                TEXT,
                150.0,
                20.0,
                now,
            )
        };
        let wide = at(&mut ui, 0.0);
        let r = runs(&wide);
        assert_eq!(r.len(), 2, "text and its repeat");
        assert!(r[0].5 && r[1].5);
        assert!(
            (r[0].0 - (120.0 - 44.0)).abs() < 1e-3,
            "holding at the slot's left edge"
        );
        match &wide[0] {
            Drawable::Run { clip: Some(c), .. } => {
                assert_eq!((c.x, c.y, c.w, c.h), (76.0, 150.0, 88.0, 20.0));
            }
            _ => panic!("clipped run"),
        }
        let moved = runs(&at(&mut ui, 2.5));
        assert!(
            (moved[0].0 - (r[0].0 - 30.0)).abs() < 1e-3,
            "30 px/s after the 1.5 s hold"
        );
    }
```

(`runs` is the existing helper in that test module.)

Append to the tests in `crates/core/src/p2/chart.rs`:

```rust
    #[test]
    fn download_card_has_title_bar_and_caption() {
        let mut ui = ScreenUi::new();
        let card = Card::Download {
            title: "Dune: Part Two".into(),
            progress: 0.35,
            label: "35 % · 19 min left".into(),
            done: false,
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 2, "track and fill");
        assert_eq!((r[0].0, r[0].1, r[0].2, r[0].3, r[0].4), (AREA.0, AREA.1 + 13.0, AREA.2, 5.0, TRACK));
        assert_eq!(r[1].4, BLUE);
        assert!((r[1].2 - 0.35 * AREA.2).abs() < 1e-3);
        let t = texts(&ds);
        assert_eq!(t, ["Dune: Part Two", "35 % · 19 min left"]);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, px, font, color, .. } if text == "Dune: Part Two" && *px == 10.0 && *font == FontId::SemiBold && *color == TEXT)));
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, px, color, .. } if text == "35 % · 19 min left" && *px == 8.5 && *color == TEXT2)));
        let done = Card::Download {
            title: "Dune: Part Two".into(),
            progress: 1.0,
            label: "added just now".into(),
            done: true,
        };
        let ds = card_drawables(&done, &mut ui, AREA, 5.0);
        let r = rects(&ds);
        assert_eq!(r[1].4, OK);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, color, .. } if text == "added just now" && *color == OK)));
        let tiny = Card::Download {
            title: "x".into(),
            progress: 0.0,
            label: "0 % · --".into(),
            done: false,
        };
        let ds = card_drawables(&tiny, &mut ui, AREA, 6.0);
        assert!((rects(&ds)[1].2 - 5.0).abs() < 1e-3, "never thinner than its height");
    }

    #[test]
    fn lines_card_places_each_line_and_marquees_long_ones() {
        let mut ui = ScreenUi::new();
        let card = Card::Lines(vec![
            CardLine {
                text: "Blade Runner 2049".into(),
                font: FontId::Bold,
                px: 11.0,
                color: TEXT,
                dy: 7.0,
            },
            CardLine {
                text: "2017 · 2h 44m".into(),
                font: FontId::Medium,
                px: 8.5,
                color: TEXT2,
                dy: 21.0,
            },
            CardLine {
                text: "silke · Living room TV  ·  +1 more".into(),
                font: FontId::Medium,
                px: 8.5,
                color: TEXT3,
                dy: 32.0,
            },
        ]);
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let t = texts(&ds);
        assert_eq!(t.len(), 4, "the long third line is drawn twice");
        assert_eq!(&t[..3], ["Blade Runner 2049", "2017 · 2h 44m", "silke · Living room TV  ·  +1 more"]);
        let base: Vec<f32> = ds
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { baseline, .. } => Some(*baseline),
                _ => None,
            })
            .collect();
        assert!((base[0] - middle_baseline(FontId::Bold, 11.0, AREA.1 + 7.0)).abs() < 1e-3);
        assert!((base[1] - middle_baseline(FontId::Medium, 8.5, AREA.1 + 21.0)).abs() < 1e-3);
        assert!(base[2] > base[1] && base[1] > base[0]);
        assert_eq!(rects(&ds).len(), 0, "no bars, no divider");
    }
```

Add `use crate::p2::font::FontId;` and `use crate::theme::p2::{OK, TEXT2};` to that test module's imports as needed.

Append to the tests in `crates/core/src/format.rs`:

```rust
    #[test]
    fn new_screen_formatters() {
        assert_eq!(fmt_left(19 * 60 + 12), "19 min left");
        assert_eq!(fmt_left(45), "<1 min left");
        assert_eq!(fmt_left(3 * 3600 + 5 * 60), "3h 05m left");
        assert_eq!(fmt_left(-1), "-- left");
        assert_eq!(fmt_mins_secs(134), "2m 14s");
        assert_eq!(fmt_mins_secs(48), "48s");
        assert_eq!(fmt_mins_secs(3 * 3600 + 5 * 60 + 9), "3h 05m");
        assert_eq!(fmt_hms(6114.0), "1:41:54");
        assert_eq!(fmt_hms(302.0), "5:02");
        assert_eq!(fmt_hms(0.4), "0:00");
        assert_eq!(fmt_hms(-3.0), "0:00");
        assert_eq!(fmt_days(82.1 * 86_400.0), "82");
        assert_eq!(fmt_days(0.4 * 86_400.0), "0");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::text p2::chart format`
Expected: compile errors (`marquee_run`, `Card::Download`, `fmt_left` … not found).

- [ ] **Step 3: Implement the formatters**

Append to `crates/core/src/format.rs` (before the tests):

```rust
/// Time left on a download: `19 min left`, `3h 05m left`, `<1 min left`; unknown is `-- left`.
pub fn fmt_left(secs: i64) -> String {
    if !(0..=864_000).contains(&secs) {
        return "-- left".into();
    }
    let mins = secs / 60;
    if mins < 1 {
        "<1 min left".into()
    } else if mins < 60 {
        format!("{mins} min left")
    } else {
        format!("{}h {:02}m left", mins / 60, mins % 60)
    }
}

/// A short duration: `48s`, `2m 14s`, `3h 05m`.
pub fn fmt_mins_secs(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    }
}

/// Playback time as `h:mm:ss` from an hour up, `m:ss` below.
pub fn fmt_hms(secs: f64) -> String {
    let s = secs.max(0.0).floor() as i64;
    let (h, m, x) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{x:02}")
    } else {
        format!("{m}:{x:02}")
    }
}

/// Whole days of uptime.
pub fn fmt_days(secs: f64) -> String {
    format!("{}", (secs.max(0.0) / 86_400.0).floor() as i64)
}
```

- [ ] **Step 4: Implement `marquee_run`**

Append to `crates/core/src/p2/text.rs` (before the tests), importing `use crate::p2::ui::UiKey;`:

```rust
/// A text run centred on `cx` that scrolls (hold, slide, wrap) when wider than
/// `slot_w`, hard-clipped to `slot_w × clip_h` at `clip_y`. Cards use it for
/// titles and captions; the pill has its own copy of the same rule.
#[allow(clippy::too_many_arguments)]
pub fn marquee_run(
    ui: &mut ScreenUi,
    key: UiKey,
    text: &str,
    cx: f32,
    baseline: f32,
    slot_w: f32,
    font: FontId,
    px: f32,
    color: Color,
    clip_y: f32,
    clip_h: f32,
    now: Secs,
) -> Vec<Drawable> {
    let tw = measure(font, px, text, 0.0);
    let run = |x: f32, clip: Option<ClipRect>| Drawable::Run {
        x,
        baseline,
        text: text.to_string(),
        font,
        px,
        letter_spacing: 0.0,
        color,
        alpha: 1.0,
        clip,
    };
    let mq = ui.marquee(key, text, now);
    match mq.offset(tw, slot_w, now) {
        None => vec![run(cx - tw / 2.0, None)],
        Some(off) => {
            let x0 = cx - slot_w / 2.0;
            let clip = Some(ClipRect {
                x: x0,
                y: clip_y,
                w: slot_w,
                h: clip_h,
            });
            vec![run(x0 - off, clip), run(x0 - off + tw + GAP_PX, clip)]
        }
    }
}
```

- [ ] **Step 5: Implement the two card kinds**

In `crates/core/src/p2/chart.rs` add to the imports `use crate::p2::text::marquee_run;` and `use crate::theme::p2::{BLUE, OK};`, then add the variants and the line struct:

```rust
/// One text line of a `Card::Lines`, `dy` below the chart-area top.
#[derive(Clone, Debug, PartialEq)]
pub struct CardLine {
    pub text: String,
    pub font: FontId,
    pub px: f32,
    pub color: Color,
    pub dy: f32,
}
```

and in `enum Card` after `Compass`:

```rust
    /// The media download: a marqueed title, a 5 px progress bar (`BLUE`,
    /// `OK` when done) and a caption such as `35 % · 19 min left`.
    Download {
        title: String,
        progress: f32,
        label: String,
        done: bool,
    },
    /// Up to three marqueed text lines (the Jellyfin card).
    Lines(Vec<CardLine>),
```

Arms in `card_drawables` before the closing of the `match`:

```rust
        Card::Download {
            title,
            progress,
            label,
            done,
        } => {
            out.extend(marquee_run(
                ui,
                ("dl-title", 0),
                title,
                x + w / 2.0,
                middle_baseline(FontId::SemiBold, 10.0, y + 3.0),
                w,
                FontId::SemiBold,
                10.0,
                TEXT,
                y - 5.0,
                16.0,
                now,
            ));
            let bh = 5.0;
            let by = y + 13.0;
            out.push(bar(x, by, w, bh, bh / 2.0, TRACK, 1.0));
            let p = ui.glide_with(("dl", 0), progress.clamp(0.0, 1.0), now, 0.6);
            let col = if *done { OK } else { BLUE };
            out.push(bar(x, by, (p * w).max(bh), bh, bh / 2.0, col, 1.0));
            out.push(caption(
                label,
                x,
                by,
                w,
                bh,
                10.0,
                CAPTION_PX,
                if *done { OK } else { TEXT2 },
            ));
        }
        Card::Lines(lines) => {
            for (i, l) in lines.iter().enumerate() {
                out.extend(marquee_run(
                    ui,
                    ("line", i as u32),
                    &l.text,
                    x + w / 2.0,
                    middle_baseline(l.font, l.px, y + l.dy),
                    w,
                    l.font,
                    l.px,
                    l.color,
                    y + l.dy - l.px * 0.8,
                    l.px * 1.6,
                    now,
                ));
            }
        }
```

- [ ] **Step 6: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green.

- [ ] **Step 7: Golden samples**

Append to `crates/render/tests/golden.rs`:

```rust
#[test]
fn p2_sample_download_and_lines() {
    use rackscreen_core::p2::chart::{Card, CardLine};
    use rackscreen_core::p2::ring::RingSpec;
    use rackscreen_core::p2::screen::{compose, P2Screen};
    use rackscreen_core::scene::FontId;
    use rackscreen_core::theme::p2::{BLUE, CYAN, ORANGE, TEXT, TEXT2, TEXT3, VIOLET};
    let mut s = P2Screen::new("media", "121");
    s.ring = RingSpec::parts(vec![(37.0 / 121.0, ORANGE), (30.0 / 121.0, CYAN), (54.0 / 121.0, VIOLET)]);
    s.pill = Some(("1 downloading".into(), BLUE));
    s.card = Some(Card::Download {
        title: "The Lord of the Rings: The Return of the King".into(),
        progress: 0.35,
        label: "35 % · 19 min left".into(),
        done: false,
    });
    let mut ui = ScreenUi::new();
    let mut r = Renderer::new().unwrap();
    let mut px = new_pixmap();
    r.render(&compose(&s, &mut ui, 3.0), &mut px);
    check("p2_sample_download", &px);
    let mut j = P2Screen::new("now playing", "1:41:54");
    j.hero_scale = 0.62;
    j.unit = Some("left".into());
    j.unit_below = true;
    j.ring = RingSpec::fill(0.38, ORANGE);
    j.pill = Some(("Direct play".into(), rackscreen_core::theme::p2::OK));
    j.card = Some(Card::Lines(vec![
        CardLine { text: "Blade Runner 2049".into(), font: FontId::Bold, px: 11.0, color: TEXT, dy: 7.0 },
        CardLine { text: "2017 · 2h 44m".into(), font: FontId::Medium, px: 8.5, color: TEXT2, dy: 21.0 },
        CardLine { text: "silke · Living room TV  ·  +1 more".into(), font: FontId::Medium, px: 8.5, color: TEXT3, dy: 32.0 },
    ]));
    let mut ui = ScreenUi::new();
    r.render(&compose(&j, &mut ui, 3.0), &mut px);
    check("p2_sample_lines", &px);
}
```

Run: `cargo test -p rackscreen-render --test golden p2_sample_download_and_lines`
Expected: writes `p2_sample_download.png` and `p2_sample_lines.png`. Open both: the download title is scrolling (3 s in: 1.5 s hold then 45 px), the bar is 35 % blue, the lines card shows three lines with the third scrolling.

- [ ] **Step 8: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(core): download and lines cards, marquee_run, formatters for the new screens

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Prometheus data for media, traffic, restarts, uptime, fans, mains and load; model state; the restart face input

**Files:**
- Modify: `crates/core/src/event.rs`, `crates/core/src/history.rs`, `crates/core/src/model.rs`
- Modify: `crates/sources/src/prometheus.rs`
- Test: unit tests in each

**Interfaces:**
- Consumes: `Model::clear_exporter`, `History`, `Hourly`, `Tunnel::get`, `parse_scalar`, `parse_vector`, `parse_matrix`, `node_tag`, `as_percent`, `Q_UPS_LOAD`.
- Produces: `event::NodeLoad`; `Event::{MediaLibrary, Traffic, TrafficHistory, Restarts, RestartsHistory, Uptime, Fans, Mains, MainsHistory, Load}`; `Hourly::last_24(unix_hour_now) -> Vec<Option<f32>>`; `model::{MediaState, TrafficState, RestartsState, UptimeState, FansState, MainsState, LoadState}` and the accessors `media, traffic, traffic_history, restarts, restarts_hourly, uptime, fans, mains, mains_hourly, load`; `Model::mood_inputs(now)` with `recent_restart` wired; `prometheus::{node_name, short_service, short_pod, parse_matrix_ts, hourly_from, per_node, traffic_from, restarts_from, load_from, HOURLY_REFRESH_SECS}`.

- [ ] **Step 1: Write the failing core tests**

Append to the tests in `crates/core/src/history.rs`:

```rust
    #[test]
    fn last_24_lines_the_slots_up_with_gaps() {
        let mut h = Hourly::new();
        h.push(100, 1.0);
        h.push(101, 2.0);
        h.push(103, 4.0);
        let v = h.last_24(103);
        assert_eq!(v.len(), 24);
        assert_eq!(v[23], Some(4.0));
        assert_eq!(v[22], None, "hour 102 was never pushed");
        assert_eq!(v[21], Some(2.0));
        assert_eq!(v[20], Some(1.0));
        assert!(v[..20].iter().all(|x| x.is_none()));
        assert_eq!(h.last_24(104)[23], None, "a newer current hour shifts everything left");
        assert_eq!(h.last_24(104)[22], Some(4.0));
        assert!(Hourly::new().last_24(5).iter().all(|x| x.is_none()));
    }
```

Append to the tests in `crates/core/src/model.rs`:

```rust
    fn node_load(node: &str, l1: f32, cores: u32) -> NodeLoad {
        NodeLoad {
            node: node.into(),
            load1: l1,
            load5: l1 * 0.9,
            load15: l1 * 0.8,
            cores,
        }
    }

    #[test]
    fn new_prometheus_events_fill_their_states_and_clear_the_exporter_flag() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 100);
        for role in [Role::Media, Role::Traffic, Role::Restarts, Role::Uptime, Role::Fans, Role::Mains, Role::Load] {
            m.apply(Event::NoExporter { role }, 0.0);
        }
        m.apply(
            Event::MediaLibrary {
                movies: 37,
                series: 30,
                artists: 54,
            },
            0.0,
        );
        assert_eq!((m.media().movies, m.media().series, m.media().artists, m.media().have), (37, 30, 54, true));
        assert!(!m.exporter_missing(Role::Media));
        m.apply(
            Event::TrafficHistory {
                req_per_sec: (0..40).map(|i| i as f32).collect(),
            },
            0.0,
        );
        assert_eq!(m.traffic_history().len(), 31);
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![("n8n".into(), 2.0), ("grafana".into(), 1.0)],
                service_rps: 4.0,
                error_share: 0.02,
            },
            1.0,
        );
        assert!(m.traffic().have && m.traffic().req_per_sec == 4.2);
        assert_eq!(m.traffic().top[0].0, "n8n");
        assert_eq!(m.traffic_history().series()[30], 4.2);
        assert!(!m.exporter_missing(Role::Traffic));
        m.apply(
            Event::RestartsHistory {
                hourly: (0..24).map(|i| (1000 - 23 + i, if i == 20 { 7.0 } else { 0.0 })).collect(),
            },
            1.0,
        );
        assert_eq!(m.restarts_hourly().len(), 24);
        assert_eq!(m.restarts_hourly().last_24(m.unix_hour())[20], Some(7.0));
        m.apply(
            Event::Restarts {
                total_24h: 56,
                worst: Some(("signal-cli".into(), 36)),
            },
            2.0,
        );
        assert_eq!(m.restarts().total_24h, 56);
        assert_eq!(m.restarts().worst, Some(("signal-cli".to_string(), 36)));
        assert_eq!(m.restarts().last_rise_at, None, "the first sample is not a rise");
        m.apply(Event::Uptime(vec![("pi-2".into(), 500.0), ("pi-1".into(), 6_884_104.0)]), 2.0);
        assert_eq!(m.uptime().nodes[0].0, "pi-1", "sorted by name");
        m.apply(Event::Fans(vec![("pi-1".into(), 4838.0)]), 2.0);
        assert!(m.fans().have && m.fans().fans.len() == 1);
        m.apply(
            Event::MainsHistory {
                hourly: (0..24).map(|i| (1000 - 23 + i, 230.0 + i as f32 * 0.1)).collect(),
            },
            2.0,
        );
        m.apply(
            Event::Mains {
                volts: 234.0,
                ups_load_pct: 7.0,
            },
            2.0,
        );
        assert_eq!((m.mains().volts, m.mains().ups_load_pct, m.mains().have), (234.0, 7.0, true));
        assert_eq!(m.mains_hourly().values().last(), Some(&234.0), "the live sample replaces the current hour");
        m.apply(Event::Load(vec![node_load("pi-1", 0.81, 4), node_load("hp-1", 0.73, 8)]), 2.0);
        assert_eq!(m.load().nodes.len(), 2);
        assert_eq!(m.load().nodes[0].node, "hp-1", "sorted by name");
        for role in [Role::Media, Role::Traffic, Role::Restarts, Role::Uptime, Role::Fans, Role::Mains, Role::Load] {
            assert!(!m.exporter_missing(role), "{role:?} cleared by its data");
        }
    }

    #[test]
    fn a_rising_restart_counter_worries_the_face_for_five_minutes() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Restarts {
                total_24h: 10,
                worst: None,
            },
            0.0,
        );
        assert!(!m.mood_inputs(1.0).recent_restart);
        m.apply(
            Event::Restarts {
                total_24h: 10,
                worst: None,
            },
            5.0,
        );
        assert!(!m.mood_inputs(6.0).recent_restart, "no rise");
        m.apply(
            Event::Restarts {
                total_24h: 11,
                worst: Some(("x".into(), 1)),
            },
            10.0,
        );
        assert!(m.mood_inputs(10.0).recent_restart);
        assert!(m.mood_inputs(309.0).recent_restart);
        assert!(!m.mood_inputs(310.0).recent_restart, "300 s later it is over");
        m.apply(
            Event::Restarts {
                total_24h: 9,
                worst: None,
            },
            400.0,
        );
        assert!(!m.mood_inputs(400.0).recent_restart, "a falling 24 h window is not a restart");
        m.tick(10.5);
    }
```

Add `use crate::event::NodeLoad;` to that test module. In the existing `mood_inputs_reduce_rack_state_and_the_face_follows` test replace `m.mood_inputs()` with `m.mood_inputs(0.0)` (both calls).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core history model`
Expected: compile errors (`last_24`, `Event::MediaLibrary`, `NodeLoad` not found).

- [ ] **Step 3: Events and `Hourly::last_24`**

`crates/core/src/event.rs`: add the struct after `IssPass`:

```rust
/// One node's load averages and core count, for the load role.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeLoad {
    pub node: String,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub cores: u32,
}
```

and the variants before `NoExporter`:

```rust
    /// Library counts from the scraparr exporter.
    MediaLibrary {
        movies: u32,
        series: u32,
        artists: u32,
    },
    /// Traefik: requests per second at the entrypoints, the three busiest
    /// services (short name, req/s), the summed service rate and the 5xx share.
    Traffic {
        req_per_sec: f32,
        top: Vec<(String, f32)>,
        service_rps: f32,
        error_share: f32,
    },
    /// The last 90 s of entrypoint req/s every 3 s, oldest first.
    TrafficHistory {
        req_per_sec: Vec<f32>,
    },
    /// Container restarts over 24 h and the worst pod (short name, count).
    Restarts {
        total_24h: u32,
        worst: Option<(String, u32)>,
    },
    /// Restarts per hour, `(unix hour, count)`, oldest first.
    RestartsHistory {
        hourly: Vec<(i64, f32)>,
    },
    /// Seconds since boot per node.
    Uptime(Vec<(String, f64)>),
    /// Fan speed per node in rpm; nodes without a fan are absent.
    Fans(Vec<(String, f32)>),
    /// UPS input voltage and load.
    Mains {
        volts: f32,
        ups_load_pct: f32,
    },
    /// Hourly mean input voltage, `(unix hour, volts)`, oldest first.
    MainsHistory {
        hourly: Vec<(i64, f32)>,
    },
    /// Load averages per node.
    Load(Vec<NodeLoad>),
```

`crates/core/src/history.rs`, in `impl Hourly`:

```rust
    /// The 24 hours ending at `unix_hour_now`, oldest first; an hour without a
    /// sample is `None`.
    pub fn last_24(&self, unix_hour_now: i64) -> Vec<Option<f32>> {
        (0..HOURLY_CAP)
            .map(|i| {
                let h = unix_hour_now - (HOURLY_CAP - 1) + i;
                self.slots.iter().find(|s| s.0 == h).map(|s| s.1)
            })
            .collect()
    }

    /// Replace everything with `(unix hour, value)` pairs, oldest first.
    pub fn replace(&mut self, hourly: &[(i64, f32)]) {
        self.slots.clear();
        for (h, v) in hourly {
            self.push(*h, *v);
        }
    }
```

- [ ] **Step 4: Model state**

`crates/core/src/model.rs`: import `NodeLoad` in the `use crate::event::{...}` line, then add the structs after `IssState`:

```rust
/// Library counts from the arr exporter; the download card state arrives in Task 5.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub movies: u32,
    pub series: u32,
    pub artists: u32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrafficState {
    pub req_per_sec: f32,
    /// The three busiest services, `(short name, req/s)`, busiest first.
    pub top: Vec<(String, f32)>,
    pub service_rps: f32,
    /// 0..=1 share of 5xx responses over the last 5 min.
    pub error_share: f32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestartsState {
    pub total_24h: u32,
    pub worst: Option<(String, u32)>,
    pub have: bool,
    /// When the 24 h counter last rose between two polls.
    pub last_rise_at: Option<Secs>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UptimeState {
    /// `(node, seconds up)`, sorted by node name.
    pub nodes: Vec<(String, f64)>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FansState {
    /// `(node, rpm)`, sorted by node name.
    pub fans: Vec<(String, f32)>,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainsState {
    pub volts: f32,
    pub ups_load_pct: f32,
    pub have: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadState {
    /// Sorted by node name.
    pub nodes: Vec<NodeLoad>,
    pub have: bool,
}

/// A rising restart counter worries the face for this long.
pub const RESTART_RECENT_SECS: Secs = 300.0;
```

Fields on `Model` (after `iss: IssState,`):

```rust
    media: MediaState,
    traffic: TrafficState,
    restarts: RestartsState,
    uptime: UptimeState,
    fans: FansState,
    mains: MainsState,
    load: LoadState,
    /// Entrypoint req/s every 3 s, 90 s.
    traffic_hist: History,
    /// Restarts and mean input voltage per unix hour, 24 h.
    restarts_hist: Hourly,
    mains_hist: Hourly,
```

in `Model::new` (after `iss: IssState::default(),`):

```rust
            media: MediaState::default(),
            traffic: TrafficState::default(),
            restarts: RestartsState::default(),
            uptime: UptimeState::default(),
            fans: FansState::default(),
            mains: MainsState::default(),
            load: LoadState::default(),
            traffic_hist: History::new(31, 3.0),
            restarts_hist: Hourly::new(),
            mains_hist: Hourly::new(),
```

accessors (after `iss()`):

```rust
    pub fn media(&self) -> &MediaState {
        &self.media
    }
    pub fn traffic(&self) -> &TrafficState {
        &self.traffic
    }
    pub fn traffic_history(&self) -> &History {
        &self.traffic_hist
    }
    pub fn restarts(&self) -> &RestartsState {
        &self.restarts
    }
    pub fn restarts_hourly(&self) -> &Hourly {
        &self.restarts_hist
    }
    pub fn uptime(&self) -> &UptimeState {
        &self.uptime
    }
    pub fn fans(&self) -> &FansState {
        &self.fans
    }
    pub fn mains(&self) -> &MainsState {
        &self.mains
    }
    pub fn mains_hourly(&self) -> &Hourly {
        &self.mains_hist
    }
    pub fn load(&self) -> &LoadState {
        &self.load
    }
```

`mood_inputs` takes `now` and reads the restart state; `tick` passes it:

```rust
    /// The rack facts the face reads each frame.
    pub fn mood_inputs(&self, now: Secs) -> MoodInputs {
        let st = &self.state;
        let th = self.thresholds;
        let hottest = st.temps.iter().map(|(_, c)| *c).fold(0.0_f32, f32::max);
        MoodInputs {
            on_battery: self.ups.have && self.ups.on_battery,
            nodes_not_ready: st.have_nodes && st.nodes_ready < st.nodes_total,
            alerts: !st.alerts.is_empty(),
            app_degraded: self
                .apps
                .apps
                .iter()
                .any(|a| a.health == AppHealth::Degraded),
            volume_degraded: st
                .volumes
                .iter()
                .any(|(_, r)| matches!(r, Robustness::Degraded | Robustness::Faulted)),
            hot: (st.have_temps && hottest >= th.hot_temp)
                || (st.have_metrics && (st.cpu_pct >= th.hot_cpu || st.mem_pct >= th.hot_mem)),
            pods_failed: st.pods_failed > 0,
            recent_restart: self
                .restarts
                .last_rise_at
                .is_some_and(|t| now - t < RESTART_RECENT_SECS),
            bedtime: self.bedtime_near,
            ci_passed_recently: false,
            cheap_price: self.current_price().is_some_and(|p| {
                price_level(p, self.prices.avg).0.index() <= PriceLevel::Cheap.index()
            }),
            clear_day: self.weather.have && self.weather.code == 0 && self.weather.is_day,
        }
    }
```

and in `tick`: `let mood = mood(&self.mood_inputs(now));`.

`apply` arms (before `Event::NoExporter`):

```rust
            Event::MediaLibrary {
                movies,
                series,
                artists,
            } => {
                self.clear_exporter(Role::Media);
                self.media.movies = movies;
                self.media.series = series;
                self.media.artists = artists;
                self.media.have = true;
            }
            Event::Traffic {
                req_per_sec,
                top,
                service_rps,
                error_share,
            } => {
                self.clear_exporter(Role::Traffic);
                self.traffic_hist.push(req_per_sec, now);
                self.traffic = TrafficState {
                    req_per_sec,
                    top,
                    service_rps,
                    error_share,
                    have: true,
                };
            }
            Event::TrafficHistory { req_per_sec } => self.traffic_hist.prefill(&req_per_sec, now),
            Event::Restarts { total_24h, worst } => {
                self.clear_exporter(Role::Restarts);
                if self.restarts.have && total_24h > self.restarts.total_24h {
                    self.restarts.last_rise_at = Some(now);
                }
                self.restarts.total_24h = total_24h;
                self.restarts.worst = worst;
                self.restarts.have = true;
            }
            Event::RestartsHistory { hourly } => self.restarts_hist.replace(&hourly),
            Event::Uptime(mut nodes) => {
                self.clear_exporter(Role::Uptime);
                nodes.sort_by(|a, b| a.0.cmp(&b.0));
                self.uptime = UptimeState { nodes, have: true };
            }
            Event::Fans(mut fans) => {
                self.clear_exporter(Role::Fans);
                fans.sort_by(|a, b| a.0.cmp(&b.0));
                self.fans = FansState { fans, have: true };
            }
            Event::Mains {
                volts,
                ups_load_pct,
            } => {
                self.clear_exporter(Role::Mains);
                let hour = self.unix_hour();
                self.mains_hist.push(hour, volts);
                self.mains = MainsState {
                    volts,
                    ups_load_pct,
                    have: true,
                };
            }
            Event::MainsHistory { hourly } => self.mains_hist.replace(&hourly),
            Event::Load(mut nodes) => {
                self.clear_exporter(Role::Load);
                nodes.sort_by(|a, b| a.node.cmp(&b.node));
                self.load = LoadState { nodes, have: true };
            }
```

- [ ] **Step 5: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green.

- [ ] **Step 6: Write the failing Prometheus tests**

Append to the tests in `crates/sources/src/prometheus.rs`:

```rust
    #[test]
    fn node_name_prefers_the_node_label() {
        let mut m = HashMap::new();
        m.insert("instance".to_string(), "192.168.0.98:9100".to_string());
        assert_eq!(node_name(&m), ".98");
        m.insert("node".to_string(), "raspberrypi-5-16gb-1".to_string());
        assert_eq!(node_name(&m), "raspberrypi-5-16gb-1");
        let mut n = HashMap::new();
        n.insert("nodename".to_string(), "hp-1".to_string());
        assert_eq!(node_name(&n), "hp-1");
        assert_eq!(node_name(&HashMap::new()), "");
    }

    #[test]
    fn service_and_pod_names_are_shortened() {
        assert_eq!(short_service("portfolio-portfolio-http@kubernetes"), "portfolio");
        assert_eq!(short_service("n8n-n8n-http@kubernetes"), "n8n");
        assert_eq!(short_service("windesheim-readers-windesheim-readers-http@kubernetes"), "windesheim-readers");
        assert_eq!(short_service("arr-stack-sonarr-webui@kubernetes"), "arr-stack-sonarr");
        assert_eq!(short_service("jotihunt-client-http@kubernetes"), "jotihunt-client");
        assert_eq!(short_service("grafana-3000@kubernetes"), "grafana");
        assert_eq!(short_service("api@file"), "api");
        assert_eq!(short_pod("signal-cli-65cd44c9db-tf9t6"), "signal-cli");
        assert_eq!(short_pod("csi-smb-node-jvfhl"), "csi-smb-node");
        assert_eq!(short_pod("longhorn-csi-plugin-f9zw2"), "longhorn-csi-plugin");
        assert_eq!(short_pod("prometheus-0"), "prometheus-0");
        assert_eq!(short_pod("redis"), "redis");
    }

    #[test]
    fn matrix_with_timestamps_and_hourly_keys() {
        let v = parse_matrix_ts(MATRIX);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0], (1, 4.5));
        assert!(v[2].1.is_nan());
        let h = hourly_from(&[(3_600_000, 1.0), (3_603_600, 2.5), (3_607_200, f64::NAN)]);
        assert_eq!(h, vec![(1000, 1.0), (1001, 2.5)], "NaN hours are dropped");
        assert!(parse_matrix_ts("nope").is_empty());
    }

    fn labelled(pairs: &[(&[(&str, &str)], f64)]) -> Vec<(HashMap<String, String>, f64)> {
        pairs
            .iter()
            .map(|(ls, v)| {
                (
                    ls.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect(),
                    *v,
                )
            })
            .collect()
    }

    #[test]
    fn traffic_restarts_and_load_events() {
        let top = labelled(&[
            (&[("service", "n8n-n8n-http@kubernetes")], 0.022),
            (&[("service", "portfolio-portfolio-http@kubernetes")], 0.015),
            (&[("service", "jotihunt-client-http@kubernetes")], 0.0),
        ]);
        let ev = traffic_from(Some(0.0333), &top, Some(0.037), Some(0.002)).unwrap();
        assert_eq!(
            ev,
            Event::Traffic {
                req_per_sec: 0.0333,
                top: vec![("n8n".into(), 0.022), ("portfolio".into(), 0.015), ("jotihunt-client".into(), 0.0)],
                service_rps: 0.037,
                error_share: 0.002 / 0.037,
            }
        );
        assert!(matches!(traffic_from(Some(1.0), &[], Some(0.0), None).unwrap(), Event::Traffic { error_share, .. } if error_share == 0.0));
        assert!(traffic_from(None, &top, None, None).is_none(), "no entrypoint series: no exporter");
        let worst = labelled(&[(&[("namespace", "hermes"), ("pod", "signal-cli-65cd44c9db-tf9t6")], 36.14)]);
        assert_eq!(
            restarts_from(Some(56.148), &worst),
            Some(Event::Restarts {
                total_24h: 56,
                worst: Some(("signal-cli".into(), 36)),
            })
        );
        assert_eq!(
            restarts_from(Some(0.0), &[]),
            Some(Event::Restarts {
                total_24h: 0,
                worst: None,
            })
        );
        assert_eq!(restarts_from(None, &[]), None);
        let l1 = labelled(&[(&[("node", "pi-1"), ("instance", "a")], 0.81), (&[("node", "hp-1"), ("instance", "b")], 0.19)]);
        let l5 = labelled(&[(&[("node", "pi-1")], 0.7), (&[("node", "hp-1")], 0.2)]);
        let l15 = labelled(&[(&[("node", "pi-1")], 0.6)]);
        let cores = labelled(&[(&[("node", "pi-1")], 4.0), (&[("node", "hp-1")], 12.0)]);
        let nodes = load_from(&l1, &l5, &l15, &cores);
        assert_eq!(nodes.len(), 2);
        let pi = nodes.iter().find(|n| n.node == "pi-1").unwrap();
        assert_eq!((pi.load1, pi.load5, pi.load15, pi.cores), (0.81, 0.7, 0.6, 4));
        let hp = nodes.iter().find(|n| n.node == "hp-1").unwrap();
        assert_eq!((hp.load15, hp.cores), (0.0, 12), "a missing series reads 0");
        assert!(load_from(&[], &l5, &l15, &cores).is_empty());
        let fans = labelled(&[
            (&[("node", "pi-1"), ("sensor", "fan1")], 4838.0),
            (&[("node", "pi-1"), ("sensor", "fan2")], 5000.0),
            (&[("node", "pi-2"), ("sensor", "fan1")], 4516.0),
        ]);
        assert_eq!(per_node(&fans), vec![("pi-1".to_string(), 5000.0), ("pi-2".to_string(), 4516.0)], "max per node, sorted");
    }
```

- [ ] **Step 7: Run to verify they fail**

Run: `cargo test -p rackscreen-sources prometheus`
Expected: compile errors (`node_name`, `short_service`, … not found).

- [ ] **Step 8: Implement queries, parsers and the poll additions**

In `crates/sources/src/prometheus.rs` add to the imports `use rackscreen_core::event::NodeLoad;` and `use rackscreen_core::theme::Role;`, and after the existing query constants:

```rust
const Q_RADARR: &str = "sum(radarr_movies_total)";
const Q_SONARR: &str = "sum(sonarr_series_total)";
const Q_LIDARR: &str = "sum(lidarr_artists_total)";
const Q_TRAFFIC_RPS: &str = "sum(rate(traefik_entrypoint_requests_total[1m]))";
const Q_TRAFFIC_TOP: &str = "topk(3, sum by (service)(rate(traefik_service_requests_total[5m])))";
/// The 5xx share is divided here, not in PromQL: a quiet site makes the
/// quotient an empty result, which would read as "no exporter".
const Q_TRAFFIC_ALL: &str = "sum(rate(traefik_service_requests_total[5m]))";
const Q_TRAFFIC_5XX: &str = "sum(rate(traefik_service_requests_total{code=~\"5..\"}[5m]))";
const Q_RESTARTS: &str = "sum(increase(kube_pod_container_status_restarts_total[24h]))";
const Q_RESTARTS_TOP: &str =
    "topk(1, sum by (namespace, pod)(increase(kube_pod_container_status_restarts_total[24h])))";
const Q_RESTARTS_HOURLY: &str = "sum(increase(kube_pod_container_status_restarts_total[1h]))";
const Q_UPTIME: &str = "time() - node_boot_time_seconds";
const Q_FANS: &str = "node_hwmon_fan_rpm";
const Q_MAINS_V: &str = "nut_input_voltage_volts";
const Q_MAINS_HOURLY: &str = "avg(nut_input_voltage_volts)";
const Q_LOAD1: &str = "node_load1";
const Q_LOAD5: &str = "node_load5";
const Q_LOAD15: &str = "node_load15";
const Q_CORES: &str = "count by (node, instance)(node_cpu_seconds_total{mode=\"idle\"})";
/// The hourly range queries (restarts, mains) are refreshed this often.
pub const HOURLY_REFRESH_SECS: u64 = 300;
```

Helpers (after `node_tag`):

```rust
/// The node a series belongs to: the `node` label kube-prometheus adds, the
/// `nodename` of a `node_uname_info` join, else the instance's last octet.
pub fn node_name(m: &HashMap<String, String>) -> String {
    m.get("node")
        .or_else(|| m.get("nodename"))
        .cloned()
        .unwrap_or_else(|| node_tag(m.get("instance").map(String::as_str).unwrap_or("")))
}

/// `portfolio-portfolio-http@kubernetes` → `portfolio`: drop the provider,
/// a trailing port name or number, and a repeated `namespace-name` half.
pub fn short_service(s: &str) -> String {
    let base = s.split('@').next().unwrap_or(s);
    let mut toks: Vec<&str> = base.split('-').collect();
    if toks.len() > 1 {
        let last = toks[toks.len() - 1];
        let port_name = matches!(last, "http" | "https" | "web" | "webui" | "tcp" | "udp" | "svc");
        if port_name || last.chars().all(|c| c.is_ascii_digit()) {
            toks.pop();
        }
    }
    let n = toks.len();
    if n >= 2 && n % 2 == 0 && toks[..n / 2] == toks[n / 2..] {
        toks.truncate(n / 2);
    }
    toks.join("-")
}

/// `signal-cli-65cd44c9db-tf9t6` → `signal-cli`: drop the pod suffix and the
/// replica-set hash.
pub fn short_pod(pod: &str) -> String {
    let alnum = |t: &str| t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let mut toks: Vec<&str> = pod.split('-').collect();
    if toks.len() > 1 && toks[toks.len() - 1].len() == 5 && alnum(toks[toks.len() - 1]) {
        toks.pop();
    }
    if toks.len() > 1 {
        let last = toks[toks.len() - 1];
        if (8..=10).contains(&last.len()) && alnum(last) && last.chars().any(|c| c.is_ascii_digit()) {
            toks.pop();
        }
    }
    toks.join("-")
}

/// `(timestamp, value)` pairs of the first series of a `query_range` matrix.
pub fn parse_matrix_ts(json: &str) -> Vec<(i64, f64)> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(first) = v
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
    else {
        return Vec::new();
    };
    first
        .get("values")
        .and_then(|vs| vs.as_array())
        .map(|vs| {
            vs.iter()
                .filter_map(|pair| {
                    let p = pair.as_array()?;
                    let ts = p.first()?.as_f64()? as i64;
                    let val = p.get(1)?.as_str()?.parse::<f64>().ok()?;
                    Some((ts, val))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `(unix hour, value)` per matrix sample, NaN samples dropped.
pub fn hourly_from(m: &[(i64, f64)]) -> Vec<(i64, f32)> {
    m.iter()
        .filter(|(_, v)| v.is_finite())
        .map(|(ts, v)| (ts.div_euclid(3600), *v as f32))
        .collect()
}

/// The largest sample per node, sorted by node name.
pub fn per_node(v: &[(HashMap<String, String>, f64)]) -> Vec<(String, f32)> {
    let mut map: std::collections::BTreeMap<String, f32> = Default::default();
    for (m, val) in v {
        if !val.is_finite() {
            continue;
        }
        let e = map.entry(node_name(m)).or_insert(*val as f32);
        *e = e.max(*val as f32);
    }
    map.into_iter().collect()
}

/// `None` when Traefik exports nothing at all.
pub fn traffic_from(
    rps: Option<f64>,
    top: &[(HashMap<String, String>, f64)],
    all: Option<f64>,
    err: Option<f64>,
) -> Option<Event> {
    let rps = rps?;
    let mut top: Vec<(String, f32)> = top
        .iter()
        .map(|(m, v)| {
            (
                short_service(m.get("service").map(String::as_str).unwrap_or("?")),
                *v as f32,
            )
        })
        .collect();
    top.sort_by(|a, b| b.1.total_cmp(&a.1));
    top.truncate(3);
    let all = all.unwrap_or(0.0);
    let error_share = if all > 0.0 {
        (err.unwrap_or(0.0) / all) as f32
    } else {
        0.0
    };
    Some(Event::Traffic {
        req_per_sec: rps as f32,
        top,
        service_rps: all as f32,
        error_share,
    })
}

/// `None` when kube-state-metrics exports nothing.
pub fn restarts_from(
    total: Option<f64>,
    top: &[(HashMap<String, String>, f64)],
) -> Option<Event> {
    let total = total?;
    let worst = top
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, v)| v.round() >= 1.0)
        .map(|(m, v)| {
            (
                short_pod(m.get("pod").map(String::as_str).unwrap_or("?")),
                v.round() as u32,
            )
        });
    Some(Event::Restarts {
        total_24h: total.round().max(0.0) as u32,
        worst,
    })
}

/// Load averages joined by node name; a node missing from a series reads 0.
pub fn load_from(
    l1: &[(HashMap<String, String>, f64)],
    l5: &[(HashMap<String, String>, f64)],
    l15: &[(HashMap<String, String>, f64)],
    cores: &[(HashMap<String, String>, f64)],
) -> Vec<NodeLoad> {
    let by = |v: &[(HashMap<String, String>, f64)]| -> HashMap<String, f32> {
        per_node(v).into_iter().collect()
    };
    let (m5, m15, mc) = (by(l5), by(l15), by(cores));
    per_node(l1)
        .into_iter()
        .map(|(node, load1)| NodeLoad {
            load5: m5.get(&node).copied().unwrap_or(0.0),
            load15: m15.get(&node).copied().unwrap_or(0.0),
            cores: mc.get(&node).copied().unwrap_or(1.0).max(1.0) as u32,
            node,
            load1,
        })
        .collect()
}
```

Replace `prefill` with a version that also covers traffic, and add the hourly one:

```rust
/// One range query per sparkline so the lines are full at boot: cpu and
/// download over the last 60 s at 2 s, memory over 90 s at 3 s, entrypoint
/// req/s over 90 s at 3 s.
async fn prefill(t: &mut Tunnel) -> Result<Vec<Event>> {
    let end = unix_now();
    let cpu = parse_matrix(&query_range(t, Q_CPU, end - 60, end, 2).await?);
    let mem = parse_matrix(&query_range(t, Q_MEM, end - 90, end, 3).await?);
    let net = parse_matrix(&query_range(t, Q_NET_RX, end - 60, end, 2).await?);
    let mut out = vec![history_event(cpu, mem, net)];
    let rps = parse_matrix(&query_range(t, Q_TRAFFIC_RPS, end - 90, end, 3).await?);
    if !rps.is_empty() {
        out.push(Event::TrafficHistory {
            req_per_sec: rps.into_iter().map(|v| v as f32).collect(),
        });
    }
    Ok(out)
}

/// Restarts per hour and the hourly mean mains voltage over the last 24 h,
/// one sample per hour ending now; re-run every `HOURLY_REFRESH_SECS`.
async fn hourly_prefill(t: &mut Tunnel) -> Result<Vec<Event>> {
    let end = unix_now();
    let mut out = Vec::new();
    let restarts = hourly_from(&parse_matrix_ts(
        &query_range(t, Q_RESTARTS_HOURLY, end - 23 * 3600, end, 3600).await?,
    ));
    if !restarts.is_empty() {
        out.push(Event::RestartsHistory { hourly: restarts });
    }
    let mains = hourly_from(&parse_matrix_ts(
        &query_range(t, Q_MAINS_HOURLY, end - 23 * 3600, end, 3600).await?,
    ));
    if !mains.is_empty() {
        out.push(Event::MainsHistory { hourly: mains });
    }
    Ok(out)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
```

Append to the end of `poll`, before `Ok(out)`:

```rust
    // media library
    let movies = parse_scalar(&query(t, Q_RADARR).await?);
    let series = parse_scalar(&query(t, Q_SONARR).await?);
    let artists = parse_scalar(&query(t, Q_LIDARR).await?);
    out.push(if movies.is_none() && series.is_none() && artists.is_none() {
        Event::NoExporter { role: Role::Media }
    } else {
        Event::MediaLibrary {
            movies: movies.unwrap_or(0.0).max(0.0) as u32,
            series: series.unwrap_or(0.0).max(0.0) as u32,
            artists: artists.unwrap_or(0.0).max(0.0) as u32,
        }
    });
    // traffic
    let rps = parse_scalar(&query(t, Q_TRAFFIC_RPS).await?);
    let top = parse_vector(&query(t, Q_TRAFFIC_TOP).await?);
    let all = parse_scalar(&query(t, Q_TRAFFIC_ALL).await?);
    let err = parse_scalar(&query(t, Q_TRAFFIC_5XX).await?);
    out.push(traffic_from(rps, &top, all, err).unwrap_or(Event::NoExporter {
        role: Role::Traffic,
    }));
    // restarts
    let total = parse_scalar(&query(t, Q_RESTARTS).await?);
    let worst = parse_vector(&query(t, Q_RESTARTS_TOP).await?);
    out.push(restarts_from(total, &worst).unwrap_or(Event::NoExporter {
        role: Role::Restarts,
    }));
    // uptime
    let up = per_node(&parse_vector(&query(t, Q_UPTIME).await?));
    out.push(if up.is_empty() {
        Event::NoExporter { role: Role::Uptime }
    } else {
        Event::Uptime(up.into_iter().map(|(n, s)| (n, s as f64)).collect())
    });
    // fans
    let fans = per_node(&parse_vector(&query(t, Q_FANS).await?));
    out.push(if fans.is_empty() {
        Event::NoExporter { role: Role::Fans }
    } else {
        Event::Fans(fans)
    });
    // mains
    out.push(match parse_scalar(&query(t, Q_MAINS_V).await?) {
        Some(volts) => Event::Mains {
            volts: volts as f32,
            ups_load_pct: parse_scalar(&query(t, Q_UPS_LOAD).await?)
                .map(as_percent)
                .unwrap_or(0.0),
        },
        None => Event::NoExporter { role: Role::Mains },
    });
    // load
    let l1 = parse_vector(&query(t, Q_LOAD1).await?);
    let l5 = parse_vector(&query(t, Q_LOAD5).await?);
    let l15 = parse_vector(&query(t, Q_LOAD15).await?);
    let cores = parse_vector(&query(t, Q_CORES).await?);
    let nodes = load_from(&l1, &l5, &l15, &cores);
    out.push(if nodes.is_empty() {
        Event::NoExporter { role: Role::Load }
    } else {
        Event::Load(nodes)
    });
```

In `run_prometheus`: the prefill now emits a list, and the hourly refresh runs inside the poll loop. Replace the `if !prefilled { ... }` block and the inner loop head with:

```rust
        if !prefilled {
            match prefill(&mut t).await {
                Ok(evs) => {
                    ctx.emit_all(evs);
                    prefilled = true;
                }
                Err(e) => tracing::warn!("prometheus prefill: {e:#}"),
            }
        }
        let mut hourly_at: Option<std::time::Instant> = None;
        let mut failures = 0;
        loop {
            if hourly_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(HOURLY_REFRESH_SECS)) {
                match hourly_prefill(&mut t).await {
                    Ok(evs) => {
                        ctx.emit_all(evs);
                        hourly_at = Some(std::time::Instant::now());
                    }
                    Err(e) => tracing::warn!("prometheus hourly ranges: {e:#}"),
                }
            }
            match poll(&mut t, &mut alerts, &mut storage, &mut ups).await {
```

(the rest of the loop is unchanged). Update the existing test `range_path_covers_the_spark_window` only if it referenced `prefill` directly (it does not).

- [ ] **Step 9: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(core,sources): prometheus data for media, traffic, restarts, uptime, fans, mains and load; restart mood input

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Screens: traffic, restarts, uptime, fans, mains, load

**Files:**
- Create: `crates/core/src/p2/roles/rack.rs`
- Modify: `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene_model.rs`, `crates/render/tests/golden.rs`
- Test: unit tests in `rack.rs`; goldens `p2_{traffic,restarts,uptime,fans,mains,load}.png` (re-captured), `p2_{traffic,restarts,uptime,fans,mains,load}_warn.png`, `p2_traffic_noexporter.png` (new)

**Interfaces:**
- Consumes: `Model::{traffic, traffic_history, restarts, restarts_hourly, uptime, fans, mains, mains_hourly, load, unix_hour}`, `Hourly::last_24`, `short_node`, `format::{fmt_ago, fmt_days}`, `RingSpec::{parts, discrete, fill, bands}`, `Card::{Spark, Bars, NodeBars, Stats}`.
- Produces: `rack::{traffic, restarts, uptime, fans, mains, load, restart_color, load_color, mains_pill, INDIGO_SHADES, FAN_MAX_RPM, FAN_WARN_RPM}`; dispatcher and `needs_data` arms for the six roles.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/p2/roles/rack.rs` with only the test module:

```rust
//! The six hardware roles read from Prometheus: traffic, restarts, uptime,
//! fans, mains and load.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, NodeLoad};
    use crate::model::Thresholds;
    use crate::p2::chart::Card;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{CRIT, CYAN, INDIGO, OK, TEXT, TEXT2, TEXT3, TRACK, WARN};

    fn pill(s: &P2Screen) -> (String, Color) {
        s.pill.clone().expect("pill")
    }

    #[test]
    fn traffic_shares_the_top_three_and_flags_errors() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::TrafficHistory {
                req_per_sec: vec![3.0; 31],
            },
            0.0,
        );
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![("n8n".into(), 2.0), ("grafana".into(), 1.0), ("pihole".into(), 0.5)],
                service_rps: 4.0,
                error_share: 0.0,
            },
            1.0,
        );
        let s = traffic(&m, 1.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below),
            ("WEB TRAFFIC", "4.2", Some("req/s"), true)
        );
        match &s.ring.kind {
            RingKind::Parts(p) => {
                assert_eq!(p.len(), 3);
                assert!((p[0].0 - 0.5).abs() < 1e-5 && p[0].1 == INDIGO);
                assert!((p[1].0 - 0.25).abs() < 1e-5 && p[1].1 == INDIGO_SHADES[1]);
                assert!((p[2].0 - 0.125).abs() < 1e-5 && p[2].1 == INDIGO_SHADES[2]);
            }
            _ => panic!("parts"),
        }
        assert_eq!(pill(&s), ("n8n · 0 % errors".into(), TEXT2));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, min, max, color, caption, .. }) if series.len() == 31 && series[30] == 4.2 && *min == 0.0 && (*max - 4.62).abs() < 1e-3 && *color == INDIGO && caption == "last 90 s")
        );
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![("n8n".into(), 2.0)],
                service_rps: 4.0,
                error_share: 0.012,
            },
            2.0,
        );
        assert_eq!(pill(&traffic(&m, 2.0)), ("n8n · 1 % errors".into(), WARN));
        m.apply(
            Event::Traffic {
                req_per_sec: 0.0,
                top: vec![],
                service_rps: 0.0,
                error_share: 0.07,
            },
            3.0,
        );
        let quiet = traffic(&m, 3.0);
        assert_eq!(pill(&quiet), ("quiet · 7 % errors".into(), CRIT));
        assert!(matches!(&quiet.ring.kind, RingKind::Parts(p) if p.is_empty()));
    }

    #[test]
    fn restarts_colour_hours_by_absolute_thresholds() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 10);
        let mut hourly: Vec<(i64, f32)> = (0..24).map(|i| (1000 - 23 + i, 0.0)).collect();
        hourly[5].1 = 1.0;
        hourly[9].1 = 3.0;
        hourly[20].1 = 41.0;
        m.apply(Event::RestartsHistory { hourly }, 0.0);
        m.apply(
            Event::Restarts {
                total_24h: 45,
                worst: Some(("n8n-worker".into(), 41)),
            },
            0.0,
        );
        let s = restarts(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("RESTARTS · 24H", "45"));
        match &s.ring.kind {
            RingKind::Discrete(c) => {
                assert_eq!(c.len(), 24);
                assert_eq!(c[0], TRACK);
                assert_eq!(c[5], OK.with_alpha(0.8));
                assert_eq!(c[9], WARN);
                assert_eq!(c[20], CRIT);
                assert_eq!(c[23], TRACK, "the current hour is quiet");
            }
            _ => panic!("discrete"),
        }
        assert_eq!(pill(&s), ("n8n-worker ×41".into(), CRIT));
        match &s.card {
            Some(Card::Bars { values, colors, now, past_alpha, caption }) => {
                assert_eq!(values.len(), 24);
                assert_eq!(values[20], 41.0);
                assert_eq!(colors[0], TEXT3);
                assert_eq!(colors[5], OK);
                assert_eq!(colors[9], WARN);
                assert_eq!(colors[20], CRIT);
                assert_eq!((*now, *past_alpha, caption.as_str()), (23, 0.9, "per hour"));
            }
            _ => panic!("bars"),
        }
        m.apply(
            Event::Restarts {
                total_24h: 2,
                worst: Some(("x".into(), 2)),
            },
            1.0,
        );
        assert_eq!(pill(&restarts(&m, 1.0)), ("x ×2".into(), TEXT2));
        m.apply(
            Event::Restarts {
                total_24h: 0,
                worst: None,
            },
            2.0,
        );
        assert_eq!(pill(&restarts(&m, 2.0)), ("No restarts".into(), OK));
        assert_eq!(restart_color(0.0), TRACK);
        assert_eq!(restart_color(5.9), WARN);
        assert_eq!(restart_color(6.0), CRIT);
    }

    #[test]
    fn uptime_names_the_latest_reboot_or_the_shortest_uptime() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Uptime(vec![
                ("hp-elitedesk-800-g5-i7".into(), 82.1 * 86_400.0),
                ("raspberrypi-5-8gb-2".into(), 9.0 * 3600.0),
                ("raspberrypi-5-8gb-3".into(), 3.4 * 86_400.0),
            ]),
            0.0,
        );
        let s = uptime(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below),
            ("UPTIME", "82", Some("d"), false)
        );
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, WARN, OK]));
        assert_eq!(pill(&s), ("raspberrypi-5-8gb-2 rebooted 9 h ago".into(), WARN));
        match &s.card {
            Some(Card::NodeBars { values, min, max, labels, colors, highlight }) => {
                assert!((values[0] - 82.1).abs() < 1e-3 && (values[1] - 0.375).abs() < 1e-3);
                assert_eq!((*min, *max, *highlight), (0.0, 82.1, None));
                assert_eq!(labels, &vec!["i7".to_string(), "r2".into(), "r3".into()]);
                assert_eq!(colors, &vec![OK, WARN, OK]);
            }
            _ => panic!("node bars"),
        }
        m.apply(
            Event::Uptime(vec![
                ("a".into(), 82.1 * 86_400.0),
                ("b".into(), 3.4 * 86_400.0),
            ]),
            1.0,
        );
        assert_eq!(pill(&uptime(&m, 1.0)), ("All up 3 d+".into(), OK));
    }

    #[test]
    fn fans_show_the_loudest_in_kilo_rpm() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Fans(vec![
                ("raspberrypi-5-16gb-1".into(), 4885.0),
                ("raspberrypi-5-8gb-1".into(), 4538.0),
                ("raspberrypi-5-8gb-3".into(), 6059.0),
            ]),
            0.0,
        );
        let s = fans(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below),
            ("FANS", "6.1k", Some("rpm"), true)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 6059.0 / 8000.0).abs() < 1e-5 && color == CYAN)
        );
        assert_eq!(pill(&s), ("raspberrypi-5-8gb-3 loudest".into(), TEXT2));
        match &s.card {
            Some(Card::NodeBars { values, min, max, labels, colors, highlight }) => {
                assert_eq!(values, &vec![4885.0, 4538.0, 6059.0]);
                assert_eq!((*min, *max, *highlight), (0.0, FAN_MAX_RPM, Some(2)));
                assert_eq!(labels, &vec!["r1".to_string(), "r1".into(), "r3".into()]);
                assert_eq!(colors, &vec![CYAN; 3]);
            }
            _ => panic!("node bars"),
        }
        m.apply(Event::Fans(vec![("pi".into(), 7400.0)]), 1.0);
        let loud = fans(&m, 1.0);
        assert_eq!(loud.hero, "7.4k");
        assert!(matches!(loud.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        m.apply(Event::Fans(vec![("pi".into(), 850.0)]), 2.0);
        assert_eq!(fans(&m, 2.0).hero, "850");
    }

    #[test]
    fn mains_bands_the_voltage_and_charts_the_day() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 10);
        m.apply(
            Event::MainsHistory {
                hourly: (0..24).map(|i| (1000 - 23 + i, 232.0 + (i % 3) as f32)).collect(),
            },
            0.0,
        );
        m.apply(
            Event::Mains {
                volts: 234.0,
                ups_load_pct: 7.0,
            },
            0.0,
        );
        let s = mains(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref(), s.unit_below),
            ("MAINS", "234", Some("V"), false)
        );
        assert!(
            matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors == &vec![CRIT, WARN, OK, OK, WARN, CRIT] && (frac - 34.0 / 60.0).abs() < 1e-5)
        );
        assert_eq!(s.ends, Some(("200".into(), "260".into())));
        assert_eq!(pill(&s), ("Stable · UPS load 7 %".into(), OK));
        match &s.card {
            Some(Card::Spark { series, now_idx, min, max, color, caption, .. }) => {
                assert_eq!(series.len(), 24);
                assert_eq!(series[23], 234.0, "the live sample is the current hour");
                assert_eq!(*now_idx, Some(23));
                assert!(*min <= 229.0 && *max >= 237.0, "padded: {min}..{max}");
                assert_eq!((*color, caption.as_str()), (OK, "24 h"));
            }
            _ => panic!("spark"),
        }
        m.apply(
            Event::Mains {
                volts: 212.0,
                ups_load_pct: 7.0,
            },
            1.0,
        );
        let low = mains(&m, 1.0);
        assert_eq!(pill(&low), ("Low voltage".into(), WARN));
        assert!(matches!(&low.card, Some(Card::Spark { color, .. }) if *color == WARN));
        m.apply(
            Event::Mains {
                volts: 247.0,
                ups_load_pct: 7.0,
            },
            2.0,
        );
        assert_eq!(pill(&mains(&m, 2.0)), ("High voltage".into(), WARN));
        assert_eq!(mains_pill(240.0, 7.0), ("Stable · UPS load 7 %".to_string(), OK));
        assert_eq!(mains_pill(219.9, 7.0).1, WARN);
    }

    #[test]
    fn load_per_core_colours_each_node() {
        let mut m = Model::new(Thresholds::default());
        let nl = |node: &str, l1: f32, l5: f32, l15: f32, cores: u32| NodeLoad {
            node: node.into(),
            load1: l1,
            load5: l5,
            load15: l15,
            cores,
        };
        m.apply(
            Event::Load(vec![
                nl("hp-elitedesk-800-g5-i7", 0.73, 0.6, 0.5, 8),
                nl("raspberrypi-5-8gb-2", 3.8, 3.0, 2.2, 4),
                nl("raspberrypi-5-8gb-3", 2.9, 2.5, 2.0, 4),
            ]),
            0.0,
        );
        let s = load(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale), ("LOAD", "3.80", 1.0));
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, CRIT, WARN]));
        assert_eq!(pill(&s), ("raspberrypi-5-8gb-2 busiest".into(), TEXT2));
        match &s.card {
            Some(Card::Stats(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], ("2.48".to_string(), "1 min".to_string(), TEXT));
                assert_eq!(items[1], ("2.03".to_string(), "5 min".to_string(), TEXT));
                assert_eq!(items[2], ("1.57".to_string(), "15 min".to_string(), TEXT));
            }
            _ => panic!("stats"),
        }
        m.apply(Event::Load(vec![nl("big", 12.5, 1.0, 1.0, 16)]), 1.0);
        let big = load(&m, 1.0);
        assert_eq!((big.hero.as_str(), big.hero_scale), ("12.50", 0.82));
        assert_eq!(load_color(0.69), OK);
        assert_eq!(load_color(0.7), WARN);
        assert_eq!(load_color(0.9), CRIT);
    }
}
```

Register the module: in `crates/core/src/p2/roles/mod.rs` add `pub mod rack;` (alphabetically, after `power_mix`).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::roles::rack`
Expected: compile errors (`traffic`, `restarts`, … not found).

- [ ] **Step 3: Implement the builders**

Replace the top of `crates/core/src/p2/roles/rack.rs` (keep the test module) with:

```rust
//! The six hardware roles read from Prometheus: traffic, restarts, uptime,
//! fans, mains and load.

use crate::anim::Secs;
use crate::format::{fmt_ago, fmt_days};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::roles::short_node;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{CRIT, CYAN, INDIGO, OK, TEXT, TEXT2, TEXT3, TRACK, WARN};
use crate::theme::Color;

/// The three busiest services on the traffic ring, busiest first.
pub const INDIGO_SHADES: [Color; 3] = [INDIGO, Color::hex(0x6E7BD0), Color::hex(0x525C9E)];
/// The fan ring is full at this speed, and amber above `FAN_WARN_RPM`.
pub const FAN_MAX_RPM: f32 = 8000.0;
pub const FAN_WARN_RPM: f32 = 7000.0;
/// Mains is "stable" inside this window.
pub const MAINS_OK_V: (f32, f32) = (220.0, 240.0);

pub fn traffic(model: &Model, now: Secs) -> P2Screen {
    let t = model.traffic();
    let hist = model.traffic_history();
    let mut s = P2Screen::new("WEB TRAFFIC", format!("{:.1}", t.req_per_sec));
    s.unit = Some("req/s".into());
    s.unit_below = true;
    let total = t.service_rps.max(1e-6);
    s.ring = RingSpec::parts(
        t.top
            .iter()
            .enumerate()
            .map(|(i, (_, r))| (r / total, INDIGO_SHADES[i.min(2)]))
            .collect(),
    );
    let busiest = t.top.first().map(|(n, _)| n.as_str()).unwrap_or("quiet");
    let err_pct = t.error_share * 100.0;
    let color = if t.error_share >= 0.05 {
        CRIT
    } else if t.error_share >= 0.01 {
        WARN
    } else {
        TEXT2
    };
    s.pill = Some((format!("{busiest} · {err_pct:.0} % errors"), color));
    let series = hist.series();
    let max = (series.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(1.0);
    s.card = Some(Card::Spark {
        series,
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max,
        now_idx: None,
        color: INDIGO,
        caption: "last 90 s".into(),
    });
    s
}

/// Ring segment colour for one hour's restarts: absolute thresholds.
pub fn restart_color(n: f32) -> Color {
    if n >= 6.0 {
        CRIT
    } else if n >= 2.0 {
        WARN
    } else if n >= 1.0 {
        OK.with_alpha(0.8)
    } else {
        TRACK
    }
}

pub fn restarts(model: &Model, _now: Secs) -> P2Screen {
    let r = model.restarts();
    let values: Vec<f32> = model
        .restarts_hourly()
        .last_24(model.unix_hour())
        .iter()
        .map(|v| v.unwrap_or(0.0))
        .collect();
    let mut s = P2Screen::new("RESTARTS · 24H", r.total_24h.to_string());
    s.ring = RingSpec::discrete(values.iter().map(|v| restart_color(*v)).collect());
    s.pill = Some(match &r.worst {
        Some((pod, n)) if r.total_24h > 0 => {
            (format!("{pod} ×{n}"), if *n >= 6 { CRIT } else { TEXT2 })
        }
        _ => ("No restarts".into(), OK),
    });
    s.card = Some(Card::Bars {
        colors: values
            .iter()
            .map(|v| match restart_color(*v) {
                c if c == TRACK => TEXT3,
                c if c == OK.with_alpha(0.8) => OK,
                c => c,
            })
            .collect(),
        values,
        now: 23,
        past_alpha: 0.9,
        caption: "per hour".into(),
    });
    s
}

pub fn uptime(model: &Model, _now: Secs) -> P2Screen {
    let u = model.uptime();
    let max = u.nodes.iter().map(|n| n.1).fold(0.0_f64, f64::max);
    let mut s = P2Screen::new("UPTIME", fmt_days(max));
    s.unit = Some("d".into());
    let color = |secs: f64| if secs < 86_400.0 { WARN } else { OK };
    s.ring = RingSpec::discrete(u.nodes.iter().map(|(_, secs)| color(*secs)).collect());
    let recent = u
        .nodes
        .iter()
        .filter(|(_, secs)| *secs < 86_400.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    s.pill = Some(match recent {
        Some((name, secs)) => (format!("{name} rebooted {}", fmt_ago(*secs)), WARN),
        None => {
            let min = u.nodes.iter().map(|n| n.1).fold(f64::MAX, f64::min);
            (format!("All up {} d+", fmt_days(min.min(max))), OK)
        }
    });
    s.card = Some(Card::NodeBars {
        values: u.nodes.iter().map(|(_, secs)| (*secs / 86_400.0) as f32).collect(),
        min: 0.0,
        max: ((max / 86_400.0) as f32).max(1.0),
        labels: u.nodes.iter().map(|(n, _)| short_node(n)).collect(),
        colors: u.nodes.iter().map(|(_, secs)| color(*secs)).collect(),
        highlight: None,
    });
    s
}

pub fn fans(model: &Model, _now: Secs) -> P2Screen {
    let f = model.fans();
    let loudest = f
        .fans
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
    let (idx, name, max) = match loudest {
        Some((i, (n, rpm))) => (Some(i), n.as_str(), *rpm),
        None => (None, "", 0.0),
    };
    let hero = if max >= 1000.0 {
        format!("{:.1}k", max / 1000.0)
    } else {
        format!("{max:.0}")
    };
    let mut s = P2Screen::new("FANS", hero);
    s.unit = Some("rpm".into());
    s.unit_below = true;
    let color = |rpm: f32| if rpm > FAN_WARN_RPM { WARN } else { CYAN };
    s.ring = RingSpec::fill((max / FAN_MAX_RPM).clamp(0.0, 1.0), color(max));
    s.pill = Some((format!("{name} loudest"), TEXT2));
    s.card = Some(Card::NodeBars {
        values: f.fans.iter().map(|(_, rpm)| *rpm).collect(),
        min: 0.0,
        max: FAN_MAX_RPM,
        labels: f.fans.iter().map(|(n, _)| short_node(n)).collect(),
        colors: f.fans.iter().map(|(_, rpm)| color(*rpm)).collect(),
        highlight: idx,
    });
    s
}

/// `Stable · UPS load 7 %` inside 220–240 V, else low or high voltage.
pub fn mains_pill(volts: f32, ups_load_pct: f32) -> (String, Color) {
    if volts < MAINS_OK_V.0 {
        ("Low voltage".into(), WARN)
    } else if volts > MAINS_OK_V.1 {
        ("High voltage".into(), WARN)
    } else {
        (format!("Stable · UPS load {ups_load_pct:.0} %"), OK)
    }
}

pub fn mains(model: &Model, _now: Secs) -> P2Screen {
    let m = model.mains();
    let mut s = P2Screen::new("MAINS", format!("{:.0}", m.volts));
    s.unit = Some("V".into());
    s.ring = RingSpec::bands(
        vec![CRIT, WARN, OK, OK, WARN, CRIT],
        (m.volts - 200.0) / 60.0,
    );
    s.ends = Some(("200".into(), "260".into()));
    let (text, color) = mains_pill(m.volts, m.ups_load_pct);
    s.pill = Some((text, color));
    let series = model.mains_hourly().values();
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(f32::MIN, f32::max);
    let pad = ((hi - lo) * 0.2).max(3.0);
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: lo - pad,
        max: hi + pad,
        color,
        caption: "24 h".into(),
    });
    s
}

/// Segment colour for a node's 1-minute load per core.
pub fn load_color(per_core: f32) -> Color {
    if per_core >= 0.9 {
        CRIT
    } else if per_core >= 0.7 {
        WARN
    } else {
        OK
    }
}

pub fn load(model: &Model, _now: Secs) -> P2Screen {
    let l = model.load();
    let busiest = l.nodes.iter().max_by(|a, b| a.load1.total_cmp(&b.load1));
    let max = busiest.map(|n| n.load1).unwrap_or(0.0);
    let mut s = P2Screen::new("LOAD", format!("{max:.2}"));
    if max >= 10.0 {
        s.hero_scale = 0.82;
    }
    s.ring = RingSpec::discrete(
        l.nodes
            .iter()
            .map(|n| load_color(n.load1 / n.cores.max(1) as f32))
            .collect(),
    );
    s.pill = Some((
        format!("{} busiest", busiest.map(|n| n.node.as_str()).unwrap_or("")),
        TEXT2,
    ));
    let n = l.nodes.len().max(1) as f32;
    let mean = |f: fn(&crate::event::NodeLoad) -> f32| l.nodes.iter().map(f).sum::<f32>() / n;
    s.card = Some(Card::Stats(vec![
        (format!("{:.2}", mean(|x| x.load1)), "1 min".into(), TEXT),
        (format!("{:.2}", mean(|x| x.load5)), "5 min".into(), TEXT),
        (format!("{:.2}", mean(|x| x.load15)), "15 min".into(), TEXT),
    ]));
    s
}
```

Dispatcher arms in `crates/core/src/p2/roles/mod.rs` (replace the placeholder arm from Task 1):

```rust
        Role::Traffic => rack::traffic(model, now),
        Role::Restarts => rack::restarts(model, now),
        Role::Uptime => rack::uptime(model, now),
        Role::Fans => rack::fans(model, now),
        Role::Mains => rack::mains(model, now),
        Role::Load => rack::load(model, now),
        Role::Media | Role::Ci | Role::Jellyfin => return None, // Tasks 5–7
        Role::PowerMix | Role::Face => return None,
```

`needs_data` arms in `crates/core/src/scene_model.rs` (replace the placeholder):

```rust
            Role::Traffic => !(link.prom && self.traffic().have),
            Role::Restarts => !(link.prom && self.restarts().have),
            Role::Uptime => !(link.prom && self.uptime().have),
            Role::Fans => !(link.prom && self.fans().have),
            Role::Mains => !(link.prom && self.mains().have),
            Role::Load => !(link.prom && self.load().have),
            Role::Media | Role::Ci | Role::Jellyfin => true,
```

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green.

- [ ] **Step 5: Goldens**

In `crates/render/tests/golden.rs` add `NodeLoad` to the `rackscreen_core::event` import, and in `full_model()` before `m.tick(0.0);`:

```rust
    let nodes = [
        "hp-elitedesk-800-g5-i7",
        "hp-elitedesk-800-g6-i5",
        "hp-elitedesk-800-g6-i7",
        "raspberrypi-5-16gb-1",
        "raspberrypi-5-8gb-1",
        "raspberrypi-5-8gb-2",
        "raspberrypi-5-8gb-3",
    ];
    m.apply(
        Event::TrafficHistory {
            req_per_sec: (0..31)
                .map(|i| (3.2 + 1.6 * ((i as f32) * 0.4).sin()).max(0.0))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Traffic {
            req_per_sec: 4.1,
            top: vec![("n8n".into(), 1.9), ("portfolio".into(), 1.1), ("grafana".into(), 0.6)],
            service_rps: 4.0,
            error_share: 0.0,
        },
        0.0,
    );
    let hour = UNIX.div_euclid(3600);
    m.apply(
        Event::RestartsHistory {
            hourly: (0..24)
                .map(|i| (hour - 23 + i, [0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 7.0, 9.0, 11.0, 6.0, 3.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0][i as usize]))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Restarts {
            total_24h: 41,
            worst: Some(("n8n-worker".into(), 33)),
        },
        0.0,
    );
    m.apply(
        Event::Uptime(
            [79.6, 79.6, 79.6, 82.1, 41.0, 3.4, 4.4]
                .iter()
                .zip(nodes)
                .map(|(d, n)| (n.to_string(), d * 86_400.0))
                .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::Fans(
            [4885.0, 4538.0, 6059.0, 5104.0]
                .iter()
                .zip(&nodes[3..])
                .map(|(r, n)| (n.to_string(), *r))
                .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::MainsHistory {
            hourly: (0..24)
                .map(|i| (hour - 23 + i, 233.0 + 2.0 * ((i as f32) / 3.0).sin()))
                .collect(),
        },
        0.0,
    );
    m.apply(
        Event::Mains {
            volts: 234.0,
            ups_load_pct: 7.0,
        },
        0.0,
    );
    m.apply(
        Event::Load(
            [(0.81, 8), (0.02, 12), (0.12, 16), (0.22, 4), (0.13, 4), (3.1, 4), (1.02, 4)]
                .iter()
                .zip(nodes)
                .map(|((l1, cores), n)| NodeLoad {
                    node: n.to_string(),
                    load1: *l1,
                    load5: l1 * 0.85,
                    load15: l1 * 0.7,
                    cores: *cores,
                })
                .collect(),
        ),
        0.0,
    );
    m.apply(
        Event::MediaLibrary {
            movies: 37,
            series: 30,
            artists: 54,
        },
        0.0,
    );
```

Append a test:

```rust
#[test]
fn rack_roles_warning_states() {
    let mut m = full_model();
    let hour = UNIX.div_euclid(3600);
    m.apply(
        Event::Traffic {
            req_per_sec: 12.4,
            top: vec![("n8n".into(), 9.0), ("portfolio".into(), 2.0), ("grafana".into(), 1.0)],
            service_rps: 12.0,
            error_share: 0.06,
        },
        0.0,
    );
    m.apply(
        Event::RestartsHistory {
            hourly: (0..24).map(|i| (hour - 23 + i, if i >= 20 { 8.0 } else { 0.0 })).collect(),
        },
        0.0,
    );
    m.apply(
        Event::Restarts {
            total_24h: 32,
            worst: Some(("signal-cli".into(), 32)),
        },
        0.0,
    );
    m.apply(
        Event::Uptime(vec![
            ("hp-elitedesk-800-g5-i7".into(), 79.6 * 86_400.0),
            ("raspberrypi-5-8gb-2".into(), 9.0 * 3600.0),
            ("raspberrypi-5-8gb-3".into(), 4.4 * 86_400.0),
        ]),
        0.0,
    );
    m.apply(Event::Fans(vec![("raspberrypi-5-8gb-3".into(), 7420.0), ("raspberrypi-5-8gb-1".into(), 5100.0)]), 0.0);
    m.apply(
        Event::Mains {
            volts: 212.0,
            ups_load_pct: 7.0,
        },
        0.0,
    );
    let mut load = m.load().nodes.clone();
    load[5].load1 = 3.9;
    m.apply(Event::Load(load), 0.0);
    m.tick(0.0);
    for role in [Role::Traffic, Role::Restarts, Role::Uptime, Role::Fans, Role::Mains, Role::Load] {
        check(&format!("p2_{}_warn", role.name()), &render(&m, role, 120.0));
    }
    let mut e = full_model();
    e.apply(Event::NoExporter { role: Role::Traffic }, 0.0);
    check("p2_traffic_noexporter", &render(&e, Role::Traffic, 120.0));
}
```

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden every_role_normal rack_roles_warning_states`
Expected: six re-captured normal goldens and seven new ones. Open `p2_restarts.png` (the ring runs from track through green/amber/red and the pill reads `n8n-worker ×33` in red), `p2_mains.png` (marker on the third band, `200`/`260` at the ring ends), `p2_load.png` (three stats fit), `p2_uptime.png` (`82` with a superscript `d`), `p2_fans.png` (`6.1k`, `rpm` below), `p2_traffic_noexporter.png` (`No exporter` in amber).

- [ ] **Step 6: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(core): P2 screens for traffic, restarts, uptime, fans, mains and load

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Arr queue source and the media screen

**Files:**
- Create: `crates/sources/src/arr.rs`, `crates/sources/tests/fixtures/arr-radarr-queue.json`, `crates/sources/tests/fixtures/arr-sonarr-queue.json`, `crates/sources/tests/fixtures/arr-lidarr-queue.json`, `crates/core/src/p2/roles/media.rs`
- Modify: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene_model.rs`, `crates/sources/src/lib.rs`, `crates/app/src/run.rs`, `crates/render/tests/golden.rs`
- Test: unit tests in `arr.rs`, `media.rs`, `model.rs`; goldens `p2_media.png` (re-captured), `p2_media_added.png`, `p2_media_torrent.png`, `p2_media_idle.png`

**Interfaces:**
- Consumes: `Tunnel`, `find_pod_for_service`, `Model::{media, arr_configured, torrent_badge_list, link}`, `Card::{Download, Stats}`, `format::{fmt_left, fmt_ago}`, `theme::p2::ORANGE`.
- Produces: `event::Download`, `Event::Download(Option<Download>)`, `Event::MediaAdded { title }`, `MediaState::{download, last_added}`, `arr::{ArrKind, ArrApp, ArrConfig, QueueItem, parse_queue, parse_timeleft, pick_download, QueueTracker, run_arr}`, `media::media`.

- [ ] **Step 1: Fixtures**

`crates/sources/tests/fixtures/arr-radarr-queue.json`:

```json
{
  "page": 1,
  "pageSize": 5,
  "totalRecords": 2,
  "records": [
    {
      "id": 101,
      "movieId": 12,
      "movie": { "title": "Dune: Part Two", "year": 2024 },
      "title": "Dune.Part.Two.2024.2160p.WEB.H265",
      "size": 12000000000,
      "sizeleft": 7800000000,
      "timeleft": "00:19:12",
      "status": "downloading"
    },
    {
      "id": 102,
      "movieId": 13,
      "movie": { "title": "Blade Runner 2049", "year": 2017 },
      "title": "Blade.Runner.2049.2017.1080p",
      "size": 8000000000,
      "sizeleft": 0,
      "timeleft": "00:00:00",
      "status": "completed"
    }
  ]
}
```

`crates/sources/tests/fixtures/arr-sonarr-queue.json`:

```json
{
  "page": 1,
  "pageSize": 5,
  "totalRecords": 1,
  "records": [
    {
      "id": 7,
      "seriesId": 3,
      "episodeId": 44,
      "series": { "title": "The Expanse" },
      "episode": { "seasonNumber": 2, "episodeNumber": 5, "title": "Home" },
      "title": "The.Expanse.S02E05.1080p",
      "size": 2400000000,
      "sizeleft": 600000000,
      "timeleft": "1.02:03:04",
      "status": "downloading"
    }
  ]
}
```

`crates/sources/tests/fixtures/arr-lidarr-queue.json`:

```json
{
  "page": 1,
  "pageSize": 5,
  "totalRecords": 1,
  "records": [
    {
      "id": 9,
      "artistId": 4,
      "albumId": 21,
      "artist": { "artistName": "Daft Punk" },
      "album": { "title": "Discovery" },
      "title": "Daft Punk - Discovery (2001) FLAC",
      "size": 500000000,
      "sizeleft": 250000000,
      "timeleft": "00:04:30",
      "status": "downloading"
    }
  ]
}
```

- [ ] **Step 2: Write the failing source tests**

Create `crates/sources/src/arr.rs` with only the test module:

```rust
//! Radarr, Sonarr and Lidarr queues over the kube tunnel: the active download
//! and the items that finished, for the media role's card.

#[cfg(test)]
mod tests {
    use super::*;

    const RADARR: &str = include_str!("../tests/fixtures/arr-radarr-queue.json");
    const SONARR: &str = include_str!("../tests/fixtures/arr-sonarr-queue.json");
    const LIDARR: &str = include_str!("../tests/fixtures/arr-lidarr-queue.json");

    #[test]
    fn timeleft_parses_hms_and_days() {
        assert_eq!(parse_timeleft("00:19:12"), 19 * 60 + 12);
        assert_eq!(parse_timeleft("1.02:03:04"), 86_400 + 2 * 3600 + 3 * 60 + 4);
        assert_eq!(parse_timeleft("00:00:00"), 0);
        assert_eq!(parse_timeleft(""), -1);
        assert_eq!(parse_timeleft("soon"), -1);
    }

    #[test]
    fn queues_parse_per_app_with_their_titles() {
        let r = parse_queue(ArrKind::Radarr, RADARR);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].key, "radarr/101");
        assert_eq!(r[0].title, "Dune: Part Two");
        assert_eq!((r[0].size, r[0].sizeleft, r[0].timeleft_secs), (12e9, 7.8e9, 19 * 60 + 12));
        assert_eq!(r[0].status, "downloading");
        assert_eq!(r[1].title, "Blade Runner 2049");
        assert!(r[1].done(), "completed, nothing left");
        let s = parse_queue(ArrKind::Sonarr, SONARR);
        assert_eq!(s[0].title, "The Expanse · S2E5");
        assert_eq!(s[0].timeleft_secs, 86_400 + 2 * 3600 + 3 * 60 + 4);
        let l = parse_queue(ArrKind::Lidarr, LIDARR);
        assert_eq!(l[0].title, "Daft Punk · Discovery");
        assert_eq!(l[0].key, "lidarr/9");
        assert!(parse_queue(ArrKind::Radarr, "garbage").is_empty());
        let bare = parse_queue(ArrKind::Radarr, r#"{"records":[{"id":5,"title":"raw.release","size":10,"sizeleft":5}]}"#);
        assert_eq!(bare[0].title, "raw.release", "falls back to the release title");
        assert_eq!(bare[0].timeleft_secs, -1);
    }

    #[test]
    fn the_download_with_the_least_time_left_wins() {
        let mut items = parse_queue(ArrKind::Radarr, RADARR);
        items.extend(parse_queue(ArrKind::Sonarr, SONARR));
        items.extend(parse_queue(ArrKind::Lidarr, LIDARR));
        let d = pick_download(&items).unwrap();
        assert_eq!(d.title, "Daft Punk · Discovery");
        assert!((d.progress - 0.5).abs() < 1e-6);
        assert_eq!(d.eta_secs, 270);
        assert!(pick_download(&items[1..2]).is_none(), "a completed item is not a download");
        let unknown = vec![QueueItem {
            key: "radarr/1".into(),
            title: "x".into(),
            size: 10.0,
            sizeleft: 2.0,
            timeleft_secs: -1,
            status: "downloading".into(),
        }];
        assert_eq!(pick_download(&unknown).unwrap().eta_secs, -1, "unknown ETA still shows");
        let mut both = unknown.clone();
        both.extend(parse_queue(ArrKind::Lidarr, LIDARR));
        assert_eq!(pick_download(&both).unwrap().title, "Daft Punk · Discovery", "a known ETA beats an unknown one");
    }

    #[test]
    fn tracker_reports_items_that_left_the_queue_completed() {
        let mut t = QueueTracker::default();
        let r = parse_queue(ArrKind::Radarr, RADARR);
        assert!(t.diff(&r).is_empty(), "first poll primes");
        assert!(t.diff(&r).is_empty());
        let only_first = vec![r[0].clone()];
        assert_eq!(
            t.diff(&only_first),
            vec![Event::MediaAdded {
                title: "Blade Runner 2049".into()
            }]
        );
        assert!(t.diff(&[]).is_empty(), "a downloading item that vanished was removed, not added");
        let mut done = r[0].clone();
        done.sizeleft = 0.0;
        done.status = "completed".into();
        assert!(t.diff(&[done]).is_empty());
        assert_eq!(
            t.diff(&[]),
            vec![Event::MediaAdded {
                title: "Dune: Part Two".into()
            }]
        );
    }
}
```

Add `pub mod arr;` to `crates/sources/src/lib.rs` (alphabetically first).

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p rackscreen-sources arr`
Expected: compile errors (`parse_timeleft`, `Event::MediaAdded` … not found).

- [ ] **Step 4: Core: the event, the model fields**

`crates/core/src/event.rs`, after `NodeLoad`:

```rust
/// The arr download shown on the media card.
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub title: String,
    /// 0.0 ..= 1.0
    pub progress: f32,
    /// seconds; negative means unknown
    pub eta_secs: i64,
}
```

and two variants after `Load(...)`:

```rust
    /// The arr download with the least time left, `None` when the queues are empty.
    Download(Option<Download>),
    /// An item left the arr queue completed.
    MediaAdded {
        title: String,
    },
```

`crates/core/src/model.rs`: import `Download`, extend `MediaState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub movies: u32,
    pub series: u32,
    pub artists: u32,
    pub have: bool,
    /// The arr download with the least time left.
    pub download: Option<Download>,
    /// The last item that left the queue completed, and when (monotonic).
    pub last_added: Option<(String, Secs)>,
}
```

`apply` arms:

```rust
            Event::Download(d) => self.media.download = d,
            Event::MediaAdded { title } => self.media.last_added = Some((title, now)),
```

Model test:

```rust
    #[test]
    fn download_and_added_items_are_kept() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Download(Some(Download {
                title: "Dune".into(),
                progress: 0.35,
                eta_secs: 1152,
            })),
            0.0,
        );
        assert_eq!(m.media().download.as_ref().unwrap().title, "Dune");
        m.apply(Event::Download(None), 1.0);
        assert!(m.media().download.is_none());
        m.apply(Event::MediaAdded { title: "Dune".into() }, 2.0);
        assert_eq!(m.media().last_added, Some(("Dune".to_string(), 2.0)));
        assert!(!m.media().have, "the card alone is not library data");
    }
```

(import `Download` in the test module).

- [ ] **Step 5: Implement the source**

Replace the top of `crates/sources/src/arr.rs` (keep the tests) with:

```rust
//! Radarr, Sonarr and Lidarr queues over the kube tunnel: the active download
//! and the items that finished, for the media role's card.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use kube::Client;
use rackscreen_core::event::{Download, Event};
use serde_json::Value;

use crate::tunnel::{find_pod_for_service, Tunnel};
use crate::SourceCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArrKind {
    Radarr,
    Sonarr,
    Lidarr,
}

impl ArrKind {
    pub fn name(self) -> &'static str {
        match self {
            ArrKind::Radarr => "radarr",
            ArrKind::Sonarr => "sonarr",
            ArrKind::Lidarr => "lidarr",
        }
    }
    /// The queue page, with the joins that carry the human titles.
    pub fn queue_path(self) -> &'static str {
        match self {
            ArrKind::Radarr => "/api/v3/queue?pageSize=5&includeMovie=true",
            ArrKind::Sonarr => "/api/v3/queue?pageSize=5&includeSeries=true&includeEpisode=true",
            ArrKind::Lidarr => "/api/v1/queue?pageSize=5&includeArtist=true&includeAlbum=true",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArrApp {
    pub kind: ArrKind,
    pub service: String,
    pub port: u16,
    pub api_key: String,
}

#[derive(Clone, Debug)]
pub struct ArrConfig {
    pub namespace: String,
    /// Only apps with an API key.
    pub apps: Vec<ArrApp>,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    /// `app/id`, unique across the three apps.
    pub key: String,
    pub title: String,
    pub size: f64,
    pub sizeleft: f64,
    /// seconds; negative means unknown
    pub timeleft_secs: i64,
    pub status: String,
}

impl QueueItem {
    /// Finished downloading (imported or waiting to be).
    pub fn done(&self) -> bool {
        self.status == "completed" || (self.size > 0.0 && self.sizeleft <= 0.0)
    }
    pub fn progress(&self) -> f32 {
        if self.size <= 0.0 {
            return 0.0;
        }
        (1.0 - self.sizeleft / self.size).clamp(0.0, 1.0) as f32
    }
}

/// `hh:mm:ss`, or `d.hh:mm:ss`; anything else is unknown (`-1`).
pub fn parse_timeleft(s: &str) -> i64 {
    let (days, rest) = match s.split_once('.') {
        Some((d, r)) => (d.parse::<i64>().unwrap_or(-1), r),
        None => (0, s),
    };
    let parts: Vec<i64> = rest.split(':').filter_map(|p| p.parse().ok()).collect();
    if days < 0 || parts.len() != 3 {
        return -1;
    }
    days * 86_400 + parts[0] * 3600 + parts[1] * 60 + parts[2]
}

fn title_of(kind: ArrKind, r: &Value) -> String {
    let s = |path: &str| r.pointer(path).and_then(Value::as_str).map(str::to_string);
    let joined = match kind {
        ArrKind::Radarr => s("/movie/title"),
        ArrKind::Sonarr => match (
            s("/series/title"),
            r.pointer("/episode/seasonNumber").and_then(Value::as_u64),
            r.pointer("/episode/episodeNumber").and_then(Value::as_u64),
        ) {
            (Some(series), Some(sn), Some(ep)) => Some(format!("{series} · S{sn}E{ep}")),
            (Some(series), _, _) => Some(series),
            _ => None,
        },
        ArrKind::Lidarr => match (s("/artist/artistName"), s("/album/title")) {
            (Some(a), Some(b)) => Some(format!("{a} · {b}")),
            (Some(a), None) => Some(a),
            _ => None,
        },
    };
    joined
        .or_else(|| s("/title"))
        .unwrap_or_else(|| "download".into())
}

pub fn parse_queue(kind: ArrKind, json: &str) -> Vec<QueueItem> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    v.get("records")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| {
                    let id = r.get("id")?.as_u64()?;
                    Some(QueueItem {
                        key: format!("{}/{id}", kind.name()),
                        title: title_of(kind, r),
                        size: r.get("size").and_then(Value::as_f64).unwrap_or(0.0),
                        sizeleft: r.get("sizeleft").and_then(Value::as_f64).unwrap_or(0.0),
                        timeleft_secs: r
                            .get("timeleft")
                            .and_then(Value::as_str)
                            .map(parse_timeleft)
                            .unwrap_or(-1),
                        status: r
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The unfinished item with the least time left; an unknown ETA sorts last.
pub fn pick_download(items: &[QueueItem]) -> Option<Download> {
    items
        .iter()
        .filter(|i| !i.done())
        .min_by_key(|i| {
            if i.timeleft_secs < 0 {
                i64::MAX
            } else {
                i.timeleft_secs
            }
        })
        .map(|i| Download {
            title: i.title.clone(),
            progress: i.progress(),
            eta_secs: i.timeleft_secs,
        })
}

/// Emits `MediaAdded` for items last seen completed that are gone now. The
/// first poll only primes.
#[derive(Debug, Default)]
pub struct QueueTracker {
    seen: HashMap<String, (String, bool)>,
    primed: bool,
}

impl QueueTracker {
    pub fn diff(&mut self, items: &[QueueItem]) -> Vec<Event> {
        let now: HashMap<String, (String, bool)> = items
            .iter()
            .map(|i| (i.key.clone(), (i.title.clone(), i.done())))
            .collect();
        let mut out = Vec::new();
        if self.primed {
            let mut gone: Vec<(&String, &(String, bool))> = self
                .seen
                .iter()
                .filter(|(k, (_, done))| *done && !now.contains_key(*k))
                .collect();
            gone.sort_by(|a, b| a.0.cmp(b.0));
            for (_, (title, _)) in gone {
                out.push(Event::MediaAdded {
                    title: title.clone(),
                });
            }
        }
        self.seen = now;
        self.primed = true;
        out
    }
}

struct AppLink {
    app: ArrApp,
    tunnel: Option<Tunnel>,
}

impl AppLink {
    async fn queue(&mut self, client: &Client, ns: &str) -> Result<Vec<QueueItem>> {
        if self.tunnel.is_none() {
            let (pod, svc) = find_pod_for_service(
                client,
                ns,
                &self.app.service,
                self.app.port,
                self.app.kind.name(),
            )
            .await?;
            tracing::info!(
                "{}: forwarding to {ns}/{pod} (service {svc})",
                self.app.kind.name()
            );
            self.tunnel = Some(Tunnel::open(client, ns, &pod, self.app.port).await?);
        }
        let t = self.tunnel.as_mut().expect("opened above");
        let r = t
            .get(
                self.app.kind.queue_path(),
                &[("x-api-key", self.app.api_key.as_str())],
            )
            .await;
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                self.tunnel = None;
                return Err(e);
            }
        };
        anyhow::ensure!(
            r.status == 200,
            "{} queue HTTP {}",
            self.app.kind.name(),
            r.status
        );
        Ok(parse_queue(self.app.kind, &r.body))
    }
}

pub async fn run_arr(client: Client, cfg: ArrConfig, ctx: SourceCtx) {
    let mut links: Vec<AppLink> = cfg
        .apps
        .into_iter()
        .map(|app| AppLink { app, tunnel: None })
        .collect();
    let mut tracker = QueueTracker::default();
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let mut items: Vec<QueueItem> = Vec::new();
        let mut any_ok = false;
        for link in &mut links {
            match link.queue(&client, &cfg.namespace).await {
                Ok(q) => {
                    any_ok = true;
                    items.extend(q);
                }
                Err(e) => tracing::warn!("{}: {e:#}", link.app.kind.name()),
            }
        }
        if any_ok {
            ctx.emit_all(tracker.diff(&items));
            ctx.emit(Event::Download(pick_download(&items)));
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(2))) => {}
        }
    }
}
```

- [ ] **Step 6: Spawn it**

`crates/app/src/run.rs`, inside `spawn_k8s_sources` after the qbittorrent block (note the earlier arm moves `client` into `run_qbittorrent`; change that call to `client.clone()` so the arr block can use it afterwards):

```rust
        if cfg2.media.keyed() {
            let app = |kind, a: &crate::config::ArrCfg| rackscreen_sources::arr::ArrApp {
                kind,
                service: a.service.clone(),
                port: a.port,
                api_key: a.api_key.trim().to_string(),
            };
            let apps: Vec<rackscreen_sources::arr::ArrApp> = [
                app(rackscreen_sources::arr::ArrKind::Radarr, &cfg2.media.radarr),
                app(rackscreen_sources::arr::ArrKind::Sonarr, &cfg2.media.sonarr),
                app(rackscreen_sources::arr::ArrKind::Lidarr, &cfg2.media.lidarr),
            ]
            .into_iter()
            .filter(|a| !a.api_key.is_empty())
            .collect();
            tokio::spawn(rackscreen_sources::arr::run_arr(
                client,
                rackscreen_sources::arr::ArrConfig {
                    namespace: cfg2.media.namespace.clone(),
                    apps,
                    poll_secs: 10,
                },
                ctx,
            ));
        } else if cfg2.media.enabled {
            tracing::warn!("media enabled but no arr api key set; the media card shows the qBittorrent torrent");
        }
```

- [ ] **Step 7: Write the failing builder tests**

Create `crates/core/src/p2/roles/media.rs` with only the tests (the Jellyfin test arrives in Task 7):

```rust
//! The media library and download card, and the Jellyfin now-playing tile.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Download, Event, LinkTarget, Torrent};
    use crate::model::Thresholds;
    use crate::p2::chart::Card;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{BLUE, CYAN, OK, ORANGE, TEXT2, VIOLET};

    fn library() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::MediaLibrary {
                movies: 37,
                series: 30,
                artists: 54,
            },
            0.0,
        );
        m
    }

    #[test]
    fn media_splits_the_library_and_shows_the_download() {
        let mut m = library();
        let s = media(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("MEDIA", "121"));
        match &s.ring.kind {
            RingKind::Parts(p) => {
                assert!((p[0].0 - 37.0 / 121.0).abs() < 1e-5 && p[0].1 == ORANGE);
                assert!((p[1].0 - 30.0 / 121.0).abs() < 1e-5 && p[1].1 == CYAN);
                assert!((p[2].0 - 54.0 / 121.0).abs() < 1e-5 && p[2].1 == VIOLET);
            }
            _ => panic!("parts"),
        }
        assert_eq!(s.pill, Some(("library 121".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items.len() == 3 && items[0] == ("37".to_string(), "movies".to_string(), ORANGE) && items[2].1 == "artists"),
            "nothing queued, nothing added: the counts"
        );
        m.apply(
            Event::Download(Some(Download {
                title: "Dune: Part Two".into(),
                progress: 0.35,
                eta_secs: 19 * 60 + 12,
            })),
            1.0,
        );
        let s = media(&m, 1.0);
        assert_eq!(s.pill, Some(("1 downloading".into(), BLUE)));
        assert_eq!(
            s.card,
            Some(Card::Download {
                title: "Dune: Part Two".into(),
                progress: 0.35,
                label: "35 % · 19 min left".into(),
                done: false,
            })
        );
        m.apply(Event::Download(None), 2.0);
        m.apply(
            Event::MediaAdded {
                title: "Dune: Part Two".into(),
            },
            2.0,
        );
        let s = media(&m, 30.0);
        assert_eq!(s.pill, Some(("library 121".into(), TEXT2)));
        assert_eq!(
            s.card,
            Some(Card::Download {
                title: "Dune: Part Two".into(),
                progress: 1.0,
                label: "added just now".into(),
                done: true,
            })
        );
        let s = media(&m, 2.0 + 2.0 * 3600.0);
        assert!(matches!(&s.card, Some(Card::Download { label, done: true, .. }) if label == "added 2 h ago"));
    }

    #[test]
    fn media_falls_back_to_the_torrent_without_arr_keys() {
        let mut m = library();
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![
                Torrent {
                    name: "ubuntu-24.04.iso".into(),
                    progress: 78.0,
                    eta_secs: 900,
                    speed_bps: 9_000_000,
                },
                Torrent {
                    name: "debian-13.iso".into(),
                    progress: 41.0,
                    eta_secs: 3000,
                    speed_bps: 3_000_000,
                },
            ]),
            0.0,
        );
        let s = media(&m, 0.0);
        assert_eq!(s.pill, Some(("2 downloading".into(), BLUE)));
        assert!(
            matches!(&s.card, Some(Card::Download { title, progress, label, done: false }) if title == "ubuntu-24.04.iso" && (*progress - 0.78).abs() < 1e-6 && label == "78 % · 15 min left")
        );
        m.set_arr_configured(true);
        let s = media(&m, 1.0);
        assert_eq!(s.pill, Some(("library 121".into(), TEXT2)), "with arr keys the torrent is not the media card");
        assert!(matches!(&s.card, Some(Card::Stats(_))));
        assert_eq!(OK, crate::theme::p2::OK);
    }
}
```

Register `pub mod media;` in `crates/core/src/p2/roles/mod.rs`.

- [ ] **Step 8: Run to verify they fail**

Run: `cargo test -p rackscreen-core p2::roles::media`
Expected: compile error, `media` not found.

- [ ] **Step 9: Implement the media builder**

Replace the top of `crates/core/src/p2/roles/media.rs` (keep the tests) with:

```rust
//! The media library and download card, and the Jellyfin now-playing tile.

use crate::anim::Secs;
use crate::event::Download;
use crate::format::{fmt_ago, fmt_left};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CYAN, ORANGE, TEXT2, VIOLET};

/// "added just now" shows for this long after an item leaves the queue.
pub const ADDED_JUST_NOW_SECS: Secs = 60.0;

/// The download to show and how many are running: the arr pick, or without
/// arr keys the most complete qBittorrent torrent.
fn current_download(model: &Model) -> Option<(Download, usize)> {
    let m = model.media();
    if let Some(d) = &m.download {
        return Some((d.clone(), 1));
    }
    if model.arr_configured() || !model.link().qbit {
        return None;
    }
    let list = model.torrent_badge_list();
    let top = list.iter().max_by(|a, b| a.progress.total_cmp(&b.progress))?;
    Some((
        Download {
            title: top.name.clone(),
            progress: top.progress / 100.0,
            eta_secs: top.eta_secs,
        },
        list.len(),
    ))
}

pub fn media(model: &Model, now: Secs) -> P2Screen {
    let m = model.media();
    let total = m.movies + m.series + m.artists;
    let denom = total.max(1) as f32;
    let mut s = P2Screen::new("MEDIA", total.to_string());
    s.ring = RingSpec::parts(vec![
        (m.movies as f32 / denom, ORANGE),
        (m.series as f32 / denom, CYAN),
        (m.artists as f32 / denom, VIOLET),
    ]);
    let download = current_download(model);
    s.pill = Some(match &download {
        Some((_, n)) => (format!("{n} downloading"), BLUE),
        None => (format!("library {total}"), TEXT2),
    });
    s.card = Some(match (download, &m.last_added) {
        (Some((d, _)), _) => Card::Download {
            label: format!("{:.0} % · {}", d.progress * 100.0, fmt_left(d.eta_secs)),
            title: d.title,
            progress: d.progress,
            done: false,
        },
        (None, Some((title, at))) => {
            let age = now - at;
            Card::Download {
                title: title.clone(),
                progress: 1.0,
                label: if age < ADDED_JUST_NOW_SECS {
                    "added just now".into()
                } else {
                    format!("added {}", fmt_ago(age))
                },
                done: true,
            }
        }
        (None, None) => Card::Stats(vec![
            (m.movies.to_string(), "movies".into(), ORANGE),
            (m.series.to_string(), "series".into(), CYAN),
            (m.artists.to_string(), "artists".into(), VIOLET),
        ]),
    });
    s
}
```

Dispatcher arm in `roles/mod.rs`: `Role::Media => media::media(model, now),` (and drop `Media` from the `return None` arm). `needs_data` in `scene_model.rs`: `Role::Media => !(link.prom && self.media().have),` (drop it from the `true` arm).

- [ ] **Step 10: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green.

- [ ] **Step 11: Goldens**

In `crates/render/tests/golden.rs`, add `Download` to the event import, and in `full_model()` after the `MediaLibrary` event:

```rust
    m.apply(
        Event::Download(Some(Download {
            title: "The Lord of the Rings: The Return of the King".into(),
            progress: 0.35,
            eta_secs: 19 * 60 + 12,
        })),
        0.0,
    );
```

Append a test:

```rust
#[test]
fn media_alternate_states() {
    let mut a = full_model();
    a.apply(Event::Download(None), 0.0);
    a.apply(
        Event::MediaAdded {
            title: "Dune: Part Two".into(),
        },
        100.0,
    );
    check("p2_media_added", &render(&a, Role::Media, 120.0));
    let mut t = full_model();
    t.apply(Event::Download(None), 0.0);
    link(&mut t, LinkTarget::QBittorrent);
    t.apply(
        Event::Torrents(vec![Torrent {
            name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
            progress: 78.0,
            eta_secs: 900,
            speed_bps: 9_000_000,
        }]),
        0.0,
    );
    check("p2_media_torrent", &render(&t, Role::Media, 120.0));
    let mut i = full_model();
    i.apply(Event::Download(None), 0.0);
    check("p2_media_idle", &render(&i, Role::Media, 120.0));
}
```

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden every_role_normal media_alternate_states`
Expected: `p2_media.png` re-captured with the marqueed title and a 35 % bar, plus three new PNGs: `_added` (green bar, `added just now`), `_torrent` (`1 downloading`, 78 %), `_idle` (three counts in the card).

- [ ] **Step 12: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(sources,core,app): arr queue poller and the media screen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: GitHub Actions runs, the ci screen and the CI face input

**Files:**
- Create: `crates/sources/tests/fixtures/github-ci-runs.json`, `crates/sources/tests/fixtures/github-user-repos.json`
- Modify: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/p2/roles/github.rs`, `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene_model.rs`, `crates/sources/src/github.rs`, `crates/app/src/run.rs`, `crates/render/tests/golden.rs`
- Test: unit tests in each; goldens `p2_ci.png` (re-captured), `p2_ci_warn.png`, `p2_ci_building.png`

**Interfaces:**
- Consumes: `github::Http` (`auth`, `client()`), `HttpStatus`, `Model::{unix_now, github_token_present}`, `Card::Stats`, `format::fmt_mins_secs`.
- Produces: `event::{CiStatus, CiRun}`, `Event::CiRuns(Vec<CiRun>)`, `Event::CiRun { repo, ok, at }`, `model::CiState`, `Model::ci()`, `CI_RECENT_SECS`, `github::{parse_ci_runs, merge_runs, parse_user_repos, CiTracker, CI_PER_PAGE, CI_MAX_REPOS}`, `GithubConfig::ci_repos`, `p2::roles::github::{ci, ci_color}`.

- [ ] **Step 1: Fixtures**

`crates/sources/tests/fixtures/github-ci-runs.json`:

```json
{
  "total_count": 4,
  "workflow_runs": [
    { "id": 401, "name": "ci", "status": "in_progress", "conclusion": null, "created_at": "2026-09-07T11:58:00Z", "updated_at": "2026-09-07T11:58:30Z", "run_started_at": "2026-09-07T11:58:05Z", "repository": { "full_name": "silkepilon/RackScreen" } },
    { "id": 400, "name": "ci", "status": "completed", "conclusion": "failure", "created_at": "2026-09-07T11:40:00Z", "updated_at": "2026-09-07T11:42:14Z", "run_started_at": "2026-09-07T11:40:00Z", "repository": { "full_name": "silkepilon/RackScreen" } },
    { "id": 399, "name": "release", "status": "completed", "conclusion": "success", "created_at": "2026-09-07T10:00:00Z", "updated_at": "2026-09-07T10:01:48Z", "run_started_at": "2026-09-07T10:00:00Z", "repository": { "full_name": "silkepilon/RackScreen" } },
    { "id": 398, "name": "ci", "status": "completed", "conclusion": "cancelled", "created_at": "2026-09-07T09:00:00Z", "updated_at": "2026-09-07T09:00:20Z", "run_started_at": null, "repository": { "full_name": "silkepilon/RackScreen" } }
  ]
}
```

`crates/sources/tests/fixtures/github-user-repos.json`:

```json
[
  { "full_name": "silkepilon/RackScreen", "pushed_at": "2026-09-07T11:40:00Z", "fork": false },
  { "full_name": "silkepilon/homelab-infrastructure", "pushed_at": "2026-09-05T18:00:00Z", "fork": false },
  { "full_name": "silkepilon/old-thing", "pushed_at": "2026-07-01T00:00:00Z", "fork": false },
  { "full_name": "someone/forked", "pushed_at": "2026-09-06T00:00:00Z", "fork": true }
]
```

- [ ] **Step 2: Write the failing tests**

`crates/core/src/event.rs` gets the types in Step 4; first the model test, appended to `crates/core/src/model.rs` tests (import `CiRun, CiStatus`):

```rust
    fn run(id: u64, status: CiStatus, at: i64) -> CiRun {
        CiRun {
            id,
            repo: "silkepilon/RackScreen".into(),
            status,
            created_at: at,
            updated_at: at + 120,
            duration_secs: Some(120),
        }
    }

    #[test]
    fn ci_runs_are_kept_and_a_fresh_pass_makes_the_face_happy() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        m.apply(Event::CiRuns(vec![run(2, CiStatus::InProgress, 100), run(1, CiStatus::Success, 50)]), 0.0);
        assert!(m.ci().have);
        assert_eq!(m.ci().runs.len(), 2);
        assert_eq!(m.ci().last_pass_at, None, "the list alone does not time a pass");
        assert!(!m.mood_inputs(0.0).ci_passed_recently);
        m.apply(
            Event::CiRun {
                repo: "silkepilon/RackScreen".into(),
                ok: false,
                at: 1_788_782_400 - 60,
            },
            1.0,
        );
        assert!(!m.mood_inputs(1.0).ci_passed_recently, "a failure is not a pass");
        m.apply(
            Event::CiRun {
                repo: "silkepilon/RackScreen".into(),
                ok: true,
                at: 1_788_782_400 - 60,
            },
            2.0,
        );
        assert_eq!(m.ci().last_pass_at, Some(1_788_782_400 - 60));
        assert!(m.mood_inputs(2.0).ci_passed_recently);
        m.set_unix_now(1_788_782_400 - 60 + CI_RECENT_SECS);
        assert!(!m.mood_inputs(3.0).ci_passed_recently, "30 min later it is over");
        m.apply(
            Event::CiRun {
                repo: "x".into(),
                ok: true,
                at: 10,
            },
            4.0,
        );
        assert_eq!(m.ci().last_pass_at, Some(1_788_782_400 - 60), "an older pass does not move it back");
    }
```

Append to the tests in `crates/sources/src/github.rs`:

```rust
    const CI_RUNS: &str = include_str!("../tests/fixtures/github-ci-runs.json");
    const USER_REPOS: &str = include_str!("../tests/fixtures/github-user-repos.json");

    #[test]
    fn ci_runs_parse_with_timestamps_and_durations() {
        let runs = parse_ci_runs(CI_RUNS, "fallback/repo").unwrap();
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[0].id, 401);
        assert_eq!(runs[0].repo, "silkepilon/RackScreen");
        assert_eq!(runs[0].status, CiStatus::InProgress);
        assert_eq!(runs[0].duration_secs, None, "still running");
        assert_eq!(runs[1].status, CiStatus::Failure);
        assert_eq!(runs[1].duration_secs, Some(134));
        assert_eq!(runs[2].status, CiStatus::Success);
        assert_eq!(runs[2].duration_secs, Some(108));
        assert_eq!(runs[2].created_at, 1_788_775_200);
        assert_eq!(runs[2].updated_at, 1_788_775_308);
        assert_eq!(runs[3].status, CiStatus::Other, "cancelled");
        assert_eq!(runs[3].duration_secs, None, "no run_started_at");
        let bare = parse_ci_runs(r#"{"workflow_runs":[{"id":1,"status":"queued","conclusion":null,"created_at":"2026-09-07T12:00:00Z","updated_at":"2026-09-07T12:00:00Z"}]}"#, "a/b").unwrap();
        assert_eq!((bare[0].repo.as_str(), bare[0].status), ("a/b", CiStatus::InProgress));
        assert!(parse_ci_runs("{}", "a/b").is_err());
    }

    #[test]
    fn runs_merge_newest_first_and_keep_twelve() {
        let mut all: Vec<CiRun> = (0..15)
            .map(|i| CiRun {
                id: i,
                repo: if i % 2 == 0 { "a/b".into() } else { "c/d".into() },
                status: CiStatus::Success,
                created_at: 1000 + i as i64 * 10,
                updated_at: 1000 + i as i64 * 10 + 5,
                duration_secs: Some(5),
            })
            .collect();
        all.rotate_left(4);
        let merged = merge_runs(all);
        assert_eq!(merged.len(), CI_PER_PAGE);
        assert_eq!(merged[0].id, 14, "newest first");
        assert_eq!(merged[11].id, 3);
    }

    #[test]
    fn user_repos_pushed_this_week_at_most_ten_no_forks() {
        let now = 1_788_782_400; // 2026-09-07 12:00Z
        assert_eq!(
            parse_user_repos(USER_REPOS, now),
            vec!["silkepilon/RackScreen".to_string(), "silkepilon/homelab-infrastructure".into()]
        );
        let many: Vec<String> = (0..14)
            .map(|i| format!("{{\"full_name\":\"u/r{i}\",\"pushed_at\":\"2026-09-07T11:00:00Z\",\"fork\":false}}"))
            .collect();
        let json = format!("[{}]", many.join(","));
        assert_eq!(parse_user_repos(&json, now).len(), CI_MAX_REPOS);
        assert!(parse_user_repos("nope", now).is_empty());
    }

    #[test]
    fn ci_tracker_reports_new_completions_with_their_time() {
        let runs = parse_ci_runs(CI_RUNS, "r").unwrap();
        let mut t = CiTracker::default();
        assert!(t.diff("silkepilon/RackScreen", &runs).is_empty(), "primes");
        let mut later = runs.clone();
        later[0].status = CiStatus::Success;
        later[0].updated_at = 1_788_782_000;
        later[0].duration_secs = Some(95);
        assert_eq!(
            t.diff("silkepilon/RackScreen", &later),
            vec![Event::CiRun {
                repo: "silkepilon/RackScreen".into(),
                ok: true,
                at: 1_788_782_000,
            }]
        );
        assert!(t.diff("silkepilon/RackScreen", &later).is_empty(), "once");
        let mut cancelled = later.clone();
        cancelled.insert(
            0,
            CiRun {
                id: 402,
                repo: "silkepilon/RackScreen".into(),
                status: CiStatus::Other,
                created_at: 1_788_782_100,
                updated_at: 1_788_782_110,
                duration_secs: None,
            },
        );
        assert!(t.diff("silkepilon/RackScreen", &cancelled).is_empty(), "cancelled is neither");
        let mut failed = cancelled.clone();
        failed[0].status = CiStatus::Failure;
        assert_eq!(
            t.diff("silkepilon/RackScreen", &failed),
            vec![Event::CiRun {
                repo: "silkepilon/RackScreen".into(),
                ok: false,
                at: 1_788_782_110,
            }]
        );
    }
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p rackscreen-core model::tests::ci_runs && cargo test -p rackscreen-sources github`
Expected: compile errors (`CiRun`, `parse_ci_runs` … not found).

- [ ] **Step 4: Core types, state and the mood input**

`crates/core/src/event.rs`, after `Download`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiStatus {
    Success,
    Failure,
    InProgress,
    /// cancelled, skipped, neutral, …
    Other,
}

/// One GitHub Actions workflow run.
#[derive(Clone, Debug, PartialEq)]
pub struct CiRun {
    pub id: u64,
    pub repo: String,
    pub status: CiStatus,
    /// unix seconds
    pub created_at: i64,
    pub updated_at: i64,
    /// `updated_at − run_started_at` for completed runs with a start.
    pub duration_secs: Option<i64>,
}
```

and variants after `MediaAdded`:

```rust
    /// The last 12 Actions runs across the watched repos, newest first.
    CiRuns(Vec<CiRun>),
    /// A run completed since the last poll; `at` is unix seconds.
    CiRun {
        repo: String,
        ok: bool,
        at: i64,
    },
```

`crates/core/src/model.rs`: import `CiRun`; add after `LoadState`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CiState {
    /// Newest first, at most 12.
    pub runs: Vec<CiRun>,
    pub have: bool,
    /// Unix time of the newest successful completion seen this run.
    pub last_pass_at: Option<i64>,
}

/// A CI pass keeps the face happy for this long (unix clock).
pub const CI_RECENT_SECS: i64 = 1800;
```

field `ci: CiState,` on `Model` (init `ci: CiState::default(),`), accessor:

```rust
    pub fn ci(&self) -> &CiState {
        &self.ci
    }
```

`mood_inputs`: replace `ci_passed_recently: false,` with

```rust
            ci_passed_recently: self
                .ci
                .last_pass_at
                .is_some_and(|t| self.unix_now - t < CI_RECENT_SECS),
```

`apply` arms:

```rust
            Event::CiRuns(runs) => {
                self.ci.runs = runs;
                self.ci.have = true;
            }
            Event::CiRun { ok, at, .. } => {
                if ok {
                    self.ci.last_pass_at = Some(self.ci.last_pass_at.map_or(at, |t| t.max(at)));
                }
            }
```

- [ ] **Step 5: Implement the source side**

In `crates/sources/src/github.rs`: import `CiRun, CiStatus` from `rackscreen_core::event`, add `ci_repos: Vec<String>` to `GithubConfig`, and constants after `MAX_RUN_REPOS`:

```rust
/// Runs kept for the ci role, and repos discovered when `ci_repos` is empty.
pub const CI_PER_PAGE: usize = 12;
pub const CI_MAX_REPOS: usize = 10;
const CI_RECENT_PUSH_SECS: i64 = 7 * 86_400;
/// A repo that answers 404/403 is warned about this often.
const CI_WARN_SECS: u64 = 3600;
```

Parsers and the tracker (after `RunTracker`):

```rust
fn rfc3339(s: Option<&str>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s?)
        .ok()
        .map(|d| d.timestamp())
}

/// Runs of one `GET /repos/{repo}/actions/runs` page; `repo` stands in when
/// the payload carries no `repository.full_name`.
pub fn parse_ci_runs(json: &str, repo: &str) -> Result<Vec<CiRun>> {
    let v: Value = serde_json::from_str(json).context("runs json")?;
    let arr = v
        .get("workflow_runs")
        .and_then(Value::as_array)
        .context("workflow_runs missing")?;
    Ok(arr
        .iter()
        .filter_map(|r| {
            let status = match (
                r.get("status").and_then(Value::as_str)?,
                r.get("conclusion").and_then(Value::as_str),
            ) {
                ("completed", Some("success")) => CiStatus::Success,
                ("completed", Some("failure") | Some("timed_out")) => CiStatus::Failure,
                ("completed", _) => CiStatus::Other,
                _ => CiStatus::InProgress,
            };
            let created_at = rfc3339(r.get("created_at").and_then(Value::as_str))?;
            let updated_at = rfc3339(r.get("updated_at").and_then(Value::as_str)).unwrap_or(created_at);
            let started = rfc3339(r.get("run_started_at").and_then(Value::as_str));
            let duration_secs = match (status, started) {
                (CiStatus::InProgress, _) | (_, None) => None,
                (_, Some(s)) => Some((updated_at - s).max(0)),
            };
            Some(CiRun {
                id: r.get("id")?.as_u64()?,
                repo: r
                    .pointer("/repository/full_name")
                    .and_then(Value::as_str)
                    .unwrap_or(repo)
                    .to_string(),
                status,
                created_at,
                updated_at,
                duration_secs,
            })
        })
        .collect())
}

/// All repos' runs merged, newest first, the last `CI_PER_PAGE` kept.
pub fn merge_runs(mut all: Vec<CiRun>) -> Vec<CiRun> {
    all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    all.truncate(CI_PER_PAGE);
    all
}

/// Non-fork repos pushed in the last 7 days, most recent first, at most
/// `CI_MAX_REPOS`, from `GET /user/repos?sort=pushed`.
pub fn parse_user_repos(json: &str, now_unix: i64) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    let mut repos: Vec<(i64, String)> = arr
        .iter()
        .filter(|r| !r.get("fork").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|r| {
            let pushed = rfc3339(r.get("pushed_at").and_then(Value::as_str))?;
            (now_unix - pushed <= CI_RECENT_PUSH_SECS)
                .then(|| (pushed, r.get("full_name")?.as_str()?.to_string()))
        })
        .collect();
    repos.sort_by(|a, b| b.0.cmp(&a.0));
    repos.truncate(CI_MAX_REPOS);
    repos.into_iter().map(|(_, n)| n).collect()
}

/// Reports each run once, the first time it is seen completed as a pass or a
/// failure, with its completion time; a repo's first page only primes.
#[derive(Debug, Default)]
pub struct CiTracker {
    primed: HashSet<String>,
    reported: HashSet<u64>,
}

impl CiTracker {
    pub fn diff(&mut self, repo: &str, runs: &[CiRun]) -> Vec<Event> {
        let completed = runs
            .iter()
            .filter(|r| matches!(r.status, CiStatus::Success | CiStatus::Failure));
        if !self.primed.contains(repo) {
            self.primed.insert(repo.to_string());
            self.reported.extend(completed.map(|r| r.id));
            return Vec::new();
        }
        let mut out = Vec::new();
        for r in completed {
            if self.reported.insert(r.id) {
                out.push(Event::CiRun {
                    repo: repo.to_string(),
                    ok: r.status == CiStatus::Success,
                    at: r.updated_at,
                });
            }
        }
        out
    }
}
```

Two `Http` methods (next to `runs`):

```rust
    async fn ci_runs(&self, repo: &str) -> Result<Vec<CiRun>> {
        let resp = self
            .auth(client().get(format!(
                "{API}/repos/{repo}/actions/runs?per_page={CI_PER_PAGE}"
            )))
            .send()
            .await
            .context("ci runs request")?;
        let status = resp.status();
        let text = resp.text().await.context("ci runs body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_ci_runs(&text, repo)
    }

    async fn user_repos(&self) -> Result<String> {
        let resp = self
            .auth(client().get(format!("{API}/user/repos?sort=pushed&per_page=30")))
            .send()
            .await
            .context("user repos request")?;
        let status = resp.status();
        let text = resp.text().await.context("user repos body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        Ok(text)
    }
```

In `run_github`, new state before the loop:

```rust
    let mut ci = CiTracker::default();
    let mut ci_repos: Vec<String> = cfg.ci_repos.clone();
    let mut ci_discovered_at: Option<std::time::Instant> = None;
    let mut ci_warned: HashMap<String, std::time::Instant> = HashMap::new();
```

and inside the `step` async block, after the `for repo in repos.clone()` loop and before `Ok(())`:

```rust
            if cfg.ci_repos.is_empty()
                && ci_discovered_at
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(CALENDAR_SECS))
            {
                let json = http.user_repos().await?;
                ci_repos = parse_user_repos(&json, chrono::Utc::now().timestamp());
                ci_discovered_at = Some(std::time::Instant::now());
                tracing::info!("github: ci watches {} repos", ci_repos.len());
            }
            let mut all_runs: Vec<CiRun> = Vec::new();
            for repo in &ci_repos {
                match http.ci_runs(repo).await {
                    Ok(runs) => {
                        ctx.emit_all(ci.diff(repo, &runs));
                        all_runs.extend(runs);
                    }
                    // a per-repo miss (404 private, 403 no Actions scope) is
                    // logged once an hour and never takes the link down
                    Err(e) => {
                        let due = ci_warned
                            .get(repo)
                            .is_none_or(|t| t.elapsed() >= Duration::from_secs(CI_WARN_SECS));
                        if due {
                            tracing::warn!("github ci runs for {repo}: {e:#}");
                            ci_warned.insert(repo.clone(), std::time::Instant::now());
                        } else {
                            tracing::debug!("github ci runs for {repo}: {e:#}");
                        }
                    }
                }
            }
            if !ci_repos.is_empty() {
                ctx.emit(Event::CiRuns(merge_runs(all_runs)));
            }
```

`crates/app/src/run.rs` `spawn_github`: pass `ci_repos: cfg.github.ci_repos.clone(),` in the `GithubConfig` literal.

- [ ] **Step 6: Run the source and core tests**

Run: `cargo test -p rackscreen-core && cargo test -p rackscreen-sources`
Expected: green.

- [ ] **Step 7: Write the failing builder test**

Append to the tests in `crates/core/src/p2/roles/github.rs`:

```rust
    use crate::event::{CiRun, CiStatus};
    use crate::theme::p2::{BLUE, CRIT, OK, TEXT, TEXT3, TRACK};

    fn run(id: u64, status: CiStatus, duration: Option<i64>) -> CiRun {
        CiRun {
            id,
            repo: "silkepilon/RackScreen".into(),
            status,
            created_at: 1000 + id as i64,
            updated_at: 1000 + id as i64 + duration.unwrap_or(0),
            duration_secs: duration,
        }
    }

    #[test]
    fn ci_counts_passes_and_names_the_newest_run() {
        let mut m = Model::new(Thresholds::default());
        let mut runs: Vec<CiRun> = (1..=12)
            .map(|i| run(i, if i == 5 { CiStatus::Failure } else { CiStatus::Success }, Some(100 + i as i64)))
            .collect();
        runs.reverse();
        runs[0].status = CiStatus::InProgress;
        runs[0].duration_secs = None;
        m.apply(Event::CiRuns(runs.clone()), 0.0);
        let s = ci(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale), ("CI", "10/12", 0.82));
        match &s.ring.kind {
            RingKind::Discrete(c) => {
                assert_eq!(c.len(), 12);
                assert_eq!(c[11], BLUE, "newest last");
                assert_eq!(c[4], CRIT, "run 5 failed");
                assert_eq!(c[0], OK);
            }
            _ => panic!("discrete"),
        }
        assert_eq!(s.pill, Some(("RackScreen · building".into(), BLUE)));
        match &s.card {
            Some(Card::Stats(items)) => {
                // completed runs 1..=11 took 101..=111 s: mean 106
                assert_eq!(items[0], ("1m 46s".to_string(), "avg run".to_string(), TEXT));
                assert_eq!(items[1], ("1".to_string(), "failed".to_string(), CRIT));
            }
            _ => panic!("stats"),
        }
        runs[0].status = CiStatus::Failure;
        m.apply(Event::CiRuns(runs.clone()), 1.0);
        let s = ci(&m, 1.0);
        assert_eq!(s.hero, "10/12");
        assert_eq!(s.pill, Some(("RackScreen · failed".into(), CRIT)));
        runs[0].status = CiStatus::Success;
        m.apply(Event::CiRuns(runs[..3].to_vec()), 2.0);
        let s = ci(&m, 2.0);
        assert_eq!(s.hero, "3/3");
        assert_eq!(s.pill, Some(("RackScreen · passed".into(), OK)));
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c.len() == 12 && c[0] == TRACK && c[8] == TRACK && c[9] == OK), "padded with track at the old end");
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[1] == ("0".to_string(), "failed".to_string(), TEXT3)));
        m.apply(Event::CiRuns(vec![]), 3.0);
        let s = ci(&m, 3.0);
        assert_eq!((s.hero.as_str(), s.pill.clone().unwrap().0.as_str()), ("0/0", "no runs"));
        assert!(matches!(&s.card, Some(Card::Stats(items)) if items[0].0 == "—"));
        assert_eq!(ci_color(CiStatus::Other), TRACK);
    }
```

- [ ] **Step 8: Run to verify it fails**

Run: `cargo test -p rackscreen-core p2::roles::github`
Expected: compile error, `ci` not found.

- [ ] **Step 9: Implement the builder**

In `crates/core/src/p2/roles/github.rs` extend the imports:

```rust
use crate::event::CiStatus;
use crate::format::fmt_mins_secs;
use crate::theme::p2::{BLUE, CRIT, OK, TEXT, TEXT3, TRACK};
use crate::theme::Color;
```

and append:

```rust
/// Ring segment colour for one run.
pub fn ci_color(s: CiStatus) -> Color {
    match s {
        CiStatus::Success => OK,
        CiStatus::Failure => CRIT,
        CiStatus::InProgress => BLUE,
        CiStatus::Other => TRACK,
    }
}

pub fn ci(model: &Model, _now: Secs) -> P2Screen {
    let runs = &model.ci().runs;
    let total = runs.len();
    let passed = runs.iter().filter(|r| r.status == CiStatus::Success).count();
    let failed = runs.iter().filter(|r| r.status == CiStatus::Failure).count();
    let mut s = P2Screen::new("CI", format!("{passed}/{total}"));
    s.hero_scale = 0.82;
    let mut colors: Vec<Color> = runs.iter().rev().map(|r| ci_color(r.status)).collect();
    while colors.len() < 12 {
        colors.insert(0, TRACK);
    }
    s.ring = RingSpec::discrete(colors);
    s.pill = Some(match runs.first() {
        Some(r) => {
            let repo = r.repo.rsplit('/').next().unwrap_or(&r.repo);
            match r.status {
                CiStatus::InProgress => (format!("{repo} · building"), BLUE),
                CiStatus::Success => (format!("{repo} · passed"), OK),
                CiStatus::Failure => (format!("{repo} · failed"), CRIT),
                CiStatus::Other => (format!("{repo} · cancelled"), TEXT2),
            }
        }
        None => ("no runs".into(), TEXT2),
    });
    let durations: Vec<i64> = runs.iter().filter_map(|r| r.duration_secs).collect();
    let avg = if durations.is_empty() {
        "—".to_string()
    } else {
        fmt_mins_secs(durations.iter().sum::<i64>() / durations.len() as i64)
    };
    s.card = Some(Card::Stats(vec![
        (avg, "avg run".into(), TEXT),
        (
            failed.to_string(),
            "failed".into(),
            if failed > 0 { CRIT } else { TEXT3 },
        ),
    ]));
    s
}
```

Dispatcher arm in `roles/mod.rs`: `Role::Ci => github::ci(model, now),`. `needs_data`: `Role::Ci => !(link.github && self.ci().have),`. In `scene_for_role` the token gate becomes

```rust
        } else if (matches!(role, Role::GhActivity | Role::Ci) && !self.github_token_present())
            || (electricity && !self.token_present())
        {
```

Add to the `scene_model` test `unconfigured_roles_show_pin_or_key`: `assert_eq!(pill_of(&for_role(&m, Role::Ci, 1.0)).as_deref(), Some("Token needed"));` before `set_github_token_present(true)`, and `Some("Waiting for data")` for `Role::Ci` after it.

- [ ] **Step 10: Run the core tests**

Run: `cargo test -p rackscreen-core`
Expected: green.

- [ ] **Step 11: Goldens**

In `crates/render/tests/golden.rs` import `CiRun, CiStatus` and add a helper plus data in `full_model()` (after the `Download` event):

```rust
    m.apply(Event::CiRuns(ci_runs(CiStatus::InProgress)), 0.0);
```

with, at file scope:

```rust
/// Twelve runs, newest first: the newest in `newest`, the fifth-oldest failed, the rest passed.
fn ci_runs(newest: CiStatus) -> Vec<CiRun> {
    (1..=12)
        .rev()
        .map(|i| {
            let status = if i == 12 {
                newest
            } else if i == 5 {
                CiStatus::Failure
            } else {
                CiStatus::Success
            };
            let duration = (status != CiStatus::InProgress).then_some(120 + i as i64 * 3);
            CiRun {
                id: i,
                repo: "silkepilon/RackScreen".into(),
                status,
                created_at: UNIX - (13 - i as i64) * 1800,
                updated_at: UNIX - (13 - i as i64) * 1800 + duration.unwrap_or(0),
                duration_secs: duration,
            }
        })
        .collect()
}
```

Append a test:

```rust
#[test]
fn ci_alternate_states() {
    let mut f = full_model();
    f.apply(Event::CiRuns(ci_runs(CiStatus::Failure)), 0.0);
    check("p2_ci_warn", &render(&f, Role::Ci, 120.0));
    let mut p = full_model();
    p.apply(Event::CiRuns(ci_runs(CiStatus::Success)), 0.0);
    check("p2_ci_passed", &render(&p, Role::Ci, 120.0));
}
```

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden every_role_normal ci_alternate_states`
Expected: `p2_ci.png` re-captured (`10/12`, blue last segment, `RackScreen · building`), `p2_ci_warn.png` (red last segment, `· failed`, failed `2`), `p2_ci_passed.png`.

- [ ] **Step 12: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(sources,core): GitHub Actions runs, the ci screen and the CI mood input

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Jellyfin source and the now-playing screen

**Files:**
- Create: `crates/sources/src/jellyfin.rs`, `crates/sources/tests/fixtures/jellyfin-sessions.json`, `crates/sources/tests/fixtures/jellyfin-sessions-empty.json`
- Modify: `crates/core/src/event.rs`, `crates/core/src/model.rs`, `crates/core/src/p2/roles/media.rs`, `crates/core/src/p2/roles/mod.rs`, `crates/core/src/scene_model.rs`, `crates/sources/src/lib.rs`, `crates/app/src/run.rs`, `crates/render/tests/golden.rs`
- Test: unit tests in each; goldens `p2_jellyfin.png` (re-captured), `p2_jellyfin_episode.png`, `p2_jellyfin_paused.png`, `p2_jellyfin_idle.png`, `p2_jellyfin_offline.png`, `p2_jellyfin_key.png`, `p2_jellyfin_config.png`

**Interfaces:**
- Consumes: `http::client()`, `Model::{jellyfin, unix_now, jellyfin_configured}`, `Card::Lines`, `CardLine`, `format::{fmt_hms, fmt_hm, fmt_ago}`, `theme::p2::{ORANGE, CYAN}`.
- Produces: `event::{JfKind, JfSession, JfStatus}`, `Event::Jellyfin { status, sessions }`, `model::JellyfinState`, `Model::jellyfin()`, `jellyfin::{JellyfinConfig, parse_sessions, run_jellyfin}`, `media::{jellyfin, pick_session, position}`.

- [ ] **Step 1: Fixtures**

`crates/sources/tests/fixtures/jellyfin-sessions.json` (the fields the parser reads; real payloads carry many more):

```json
[
  {
    "Id": "s1",
    "UserName": "silke",
    "DeviceName": "Living room TV",
    "LastActivityDate": "2026-09-07T12:00:05.000Z",
    "NowPlayingItem": {
      "Name": "Blade Runner 2049",
      "Type": "Movie",
      "ProductionYear": 2017,
      "RunTimeTicks": 98400000000
    },
    "PlayState": { "PositionTicks": 37200000000, "IsPaused": false }
  },
  {
    "Id": "s2",
    "UserName": "sam",
    "DeviceName": "iPad",
    "LastActivityDate": "2026-09-07T12:00:02.000Z",
    "NowPlayingItem": {
      "Name": "Home",
      "Type": "Episode",
      "SeriesName": "The Expanse",
      "ParentIndexNumber": 2,
      "IndexNumber": 5,
      "RunTimeTicks": 26400000000
    },
    "PlayState": { "PositionTicks": 14800000000, "IsPaused": false },
    "TranscodingInfo": { "Container": "ts", "IsVideoDirect": false }
  },
  {
    "Id": "s3",
    "UserName": "guest",
    "DeviceName": "Firefox",
    "LastActivityDate": "2026-09-07T11:59:00.000Z",
    "NowPlayingItem": {
      "Name": "Dune: Part Two",
      "Type": "Movie",
      "ProductionYear": 2024,
      "RunTimeTicks": 99600000000
    },
    "PlayState": { "PositionTicks": 6000000000, "IsPaused": true }
  },
  {
    "Id": "s4",
    "UserName": "idle",
    "DeviceName": "Phone",
    "LastActivityDate": "2026-09-07T11:30:00.000Z",
    "PlayState": { "CanSeek": false }
  }
]
```

`crates/sources/tests/fixtures/jellyfin-sessions-empty.json`:

```json
[]
```

- [ ] **Step 2: Write the failing tests**

Create `crates/sources/src/jellyfin.rs` with only the tests:

```rust
//! Jellyfin now-playing sessions from a server outside the cluster.

#[cfg(test)]
mod tests {
    use super::*;

    const SESSIONS: &str = include_str!("../tests/fixtures/jellyfin-sessions.json");
    const EMPTY: &str = include_str!("../tests/fixtures/jellyfin-sessions-empty.json");

    #[test]
    fn sessions_parse_movies_episodes_pause_and_transcoding() {
        let s = parse_sessions(SESSIONS);
        assert_eq!(s.len(), 3, "the session without an item is skipped");
        let movie = &s[0];
        assert_eq!((movie.title.as_str(), movie.kind, movie.year), ("Blade Runner 2049", JfKind::Movie, Some(2017)));
        assert!((movie.runtime_secs - 9840.0).abs() < 1e-6, "ticks are 100 ns");
        assert!((movie.position_secs - 3720.0).abs() < 1e-6);
        assert!(!movie.paused && !movie.transcoding);
        assert_eq!((movie.user.as_str(), movie.device.as_str()), ("silke", "Living room TV"));
        assert_eq!(movie.last_activity, "2026-09-07T12:00:05.000Z");
        let ep = &s[1];
        assert_eq!((ep.title.as_str(), ep.kind), ("Home", JfKind::Episode));
        assert_eq!((ep.series.as_deref(), ep.season, ep.episode), (Some("The Expanse"), Some(2), Some(5)));
        assert!(ep.transcoding && !ep.paused);
        assert!(s[2].paused);
        assert!(parse_sessions(EMPTY).is_empty());
        assert!(parse_sessions("nope").is_empty());
        assert_eq!(status_of(200, true), JfStatus::Ok);
        assert_eq!(status_of(401, true), JfStatus::Unauthorized);
        assert_eq!(status_of(403, true), JfStatus::Unauthorized);
        assert_eq!(status_of(502, true), JfStatus::Offline);
        assert_eq!(status_of(200, false), JfStatus::Offline);
    }
}
```

Add `pub mod jellyfin;` to `crates/sources/src/lib.rs`.

Append to the tests in `crates/core/src/model.rs` (import `JfKind, JfSession, JfStatus`):

```rust
    fn session(title: &str, pos: f64, paused: bool) -> JfSession {
        JfSession {
            title: title.into(),
            kind: JfKind::Movie,
            series: None,
            season: None,
            episode: None,
            year: Some(2017),
            runtime_secs: 9840.0,
            position_secs: pos,
            paused,
            transcoding: false,
            user: "silke".into(),
            device: "TV".into(),
            last_activity: "2026-09-07T12:00:05.000Z".into(),
        }
    }

    #[test]
    fn jellyfin_polls_are_stamped_and_remember_the_last_item() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        assert!(!m.jellyfin().have);
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Ok,
                sessions: vec![session("Blade Runner 2049", 3720.0, false)],
            },
            5.0,
        );
        let j = m.jellyfin();
        assert!(j.have && j.status == JfStatus::Ok);
        assert_eq!((j.sessions.len(), j.polled_at), (1, 5.0));
        assert_eq!(j.last_seen, Some(("Blade Runner 2049".to_string(), 1_788_782_400)));
        m.set_unix_now(1_788_782_400 + 7200);
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Ok,
                sessions: vec![],
            },
            10.0,
        );
        assert_eq!(m.jellyfin().last_seen, Some(("Blade Runner 2049".to_string(), 1_788_782_400)), "kept while idle");
        m.apply(
            Event::Jellyfin {
                status: JfStatus::Offline,
                sessions: vec![],
            },
            11.0,
        );
        assert_eq!(m.jellyfin().status, JfStatus::Offline);
    }
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p rackscreen-sources jellyfin && cargo test -p rackscreen-core model::tests::jellyfin`
Expected: compile errors.

- [ ] **Step 4: Core types and state**

`crates/core/src/event.rs`, after `CiRun`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JfKind {
    Movie,
    Episode,
    Other,
}

/// One Jellyfin session with something playing.
#[derive(Clone, Debug, PartialEq)]
pub struct JfSession {
    pub title: String,
    pub kind: JfKind,
    pub series: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub year: Option<u32>,
    pub runtime_secs: f64,
    /// Where playback was at the poll.
    pub position_secs: f64,
    pub paused: bool,
    pub transcoding: bool,
    pub user: String,
    pub device: String,
    /// `LastActivityDate` as sent, ISO 8601 UTC; sorts lexically.
    pub last_activity: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JfStatus {
    Ok,
    Offline,
    Unauthorized,
}
```

variant after `CiRun { .. }`:

```rust
    /// One poll of `/Sessions`: the sessions with an item, or why there are none.
    Jellyfin {
        status: JfStatus,
        sessions: Vec<JfSession>,
    },
```

`crates/core/src/model.rs`: import `JfSession, JfStatus`; after `CiState`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct JellyfinState {
    pub status: JfStatus,
    pub sessions: Vec<JfSession>,
    /// Monotonic time of the last poll; positions extrapolate from it.
    pub polled_at: Secs,
    pub have: bool,
    /// The last item seen playing this run and when (unix seconds).
    pub last_seen: Option<(String, i64)>,
}

impl Default for JellyfinState {
    fn default() -> Self {
        Self {
            status: JfStatus::Offline,
            sessions: Vec::new(),
            polled_at: 0.0,
            have: false,
            last_seen: None,
        }
    }
}
```

field `jellyfin: JellyfinState,` (init with `JellyfinState::default()`), accessor `pub fn jellyfin(&self) -> &JellyfinState { &self.jellyfin }`, and the `apply` arm:

```rust
            Event::Jellyfin { status, sessions } => {
                if status == JfStatus::Ok {
                    if let Some(s) = crate::p2::roles::media::pick_session(&sessions) {
                        self.jellyfin.last_seen = Some((s.title.clone(), self.unix_now));
                    }
                }
                self.jellyfin.status = status;
                self.jellyfin.sessions = sessions;
                self.jellyfin.polled_at = now;
                self.jellyfin.have = true;
            }
```

(`pick_session` is written in Step 7; add it there before compiling the model test, or compile both steps together.) For an episode `last_seen` uses the series name: `pick_session` returns the session; use `media::display_title(s)` (Step 7) instead of `s.title.clone()`.

- [ ] **Step 5: Implement the source**

Replace the top of `crates/sources/src/jellyfin.rs` (keep the tests) with:

```rust
//! Jellyfin now-playing sessions from a server outside the cluster.

use std::time::Duration;

use rackscreen_core::event::{Event, JfKind, JfSession, JfStatus};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct JellyfinConfig {
    pub url: String,
    pub api_key: String,
    pub poll_secs: u64,
}

/// 100 ns ticks to seconds.
fn ticks_secs(v: Option<&Value>) -> f64 {
    v.and_then(Value::as_f64).unwrap_or(0.0) / 10_000_000.0
}

/// Sessions with a `NowPlayingItem`, in the server's order.
pub fn parse_sessions(json: &str) -> Vec<JfSession> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let item = s.get("NowPlayingItem")?;
            let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
            let u32_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
            let kind = match item.get("Type").and_then(Value::as_str) {
                Some("Movie") => JfKind::Movie,
                Some("Episode") => JfKind::Episode,
                _ => JfKind::Other,
            };
            let play = s.get("PlayState");
            Some(JfSession {
                title: str_of(item, "Name").unwrap_or_else(|| "Unknown".into()),
                kind,
                series: str_of(item, "SeriesName"),
                season: u32_of(item, "ParentIndexNumber"),
                episode: u32_of(item, "IndexNumber"),
                year: u32_of(item, "ProductionYear"),
                runtime_secs: ticks_secs(item.get("RunTimeTicks")),
                position_secs: ticks_secs(play.and_then(|p| p.get("PositionTicks"))),
                paused: play
                    .and_then(|p| p.get("IsPaused"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                transcoding: s.get("TranscodingInfo").is_some_and(|t| !t.is_null()),
                user: str_of(s, "UserName").unwrap_or_default(),
                device: str_of(s, "DeviceName").unwrap_or_default(),
                last_activity: str_of(s, "LastActivityDate").unwrap_or_default(),
            })
        })
        .collect()
}

/// What an HTTP outcome means for the tile.
pub fn status_of(status: u16, reached: bool) -> JfStatus {
    match (reached, status) {
        (false, _) => JfStatus::Offline,
        (true, 401 | 403) => JfStatus::Unauthorized,
        (true, 200..=299) => JfStatus::Ok,
        _ => JfStatus::Offline,
    }
}

async fn poll(cfg: &JellyfinConfig) -> Event {
    let url = format!(
        "{}/Sessions?activeWithinSeconds=60",
        cfg.url.trim_end_matches('/')
    );
    let resp = client()
        .get(&url)
        .header(
            "Authorization",
            format!("MediaBrowser Token=\"{}\"", cfg.api_key),
        )
        .send()
        .await;
    match resp {
        Err(e) => {
            tracing::debug!("jellyfin: {e}");
            Event::Jellyfin {
                status: JfStatus::Offline,
                sessions: Vec::new(),
            }
        }
        Ok(r) => {
            let status = status_of(r.status().as_u16(), true);
            let sessions = if status == JfStatus::Ok {
                parse_sessions(&r.text().await.unwrap_or_default())
            } else {
                Vec::new()
            };
            Event::Jellyfin { status, sessions }
        }
    }
}

pub async fn run_jellyfin(cfg: JellyfinConfig, ctx: SourceCtx) {
    let mut last: Option<JfStatus> = None;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let ev = poll(&cfg).await;
        if let Event::Jellyfin { status, .. } = &ev {
            if last != Some(*status) {
                match status {
                    JfStatus::Ok => tracing::info!("jellyfin: sessions ok"),
                    JfStatus::Offline => tracing::warn!("jellyfin: unreachable at {}", cfg.url),
                    JfStatus::Unauthorized => {
                        tracing::warn!("jellyfin: api key rejected; check the key in Configure")
                    }
                }
                last = Some(*status);
            }
        }
        ctx.emit(ev);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(2))) => {}
        }
    }
}
```

`crates/app/src/run.rs`: in `spawn_external_sources`, at the top (before the location check, so it runs with or without a location):

```rust
    if cfg.jellyfin.enabled && !cfg.jellyfin.url.trim().is_empty() {
        runtime.spawn(rackscreen_sources::jellyfin::run_jellyfin(
            rackscreen_sources::jellyfin::JellyfinConfig {
                url: cfg.jellyfin.url.trim().to_string(),
                api_key: cfg.jellyfin.api_key.clone(),
                poll_secs: cfg.jellyfin.poll_secs,
            },
            ctx.clone(),
        ));
    } else if cfg.jellyfin.enabled {
        tracing::warn!("jellyfin enabled but no url set; the role says Check config");
    }
```

- [ ] **Step 6: Write the failing builder tests**

Append to the tests in `crates/core/src/p2/roles/media.rs`:

```rust
    use crate::event::{JfKind, JfSession, JfStatus};
    use crate::p2::chart::CardLine;
    use crate::p2::font::FontId;
    use crate::theme::p2::{TEXT, TEXT3, WARN};

    fn movie(pos: f64, paused: bool) -> JfSession {
        JfSession {
            title: "Blade Runner 2049".into(),
            kind: JfKind::Movie,
            series: None,
            season: None,
            episode: None,
            year: Some(2017),
            runtime_secs: 9840.0,
            position_secs: pos,
            paused,
            transcoding: false,
            user: "silke".into(),
            device: "Living room TV".into(),
            last_activity: "2026-09-07T12:00:05.000Z".into(),
        }
    }

    fn episode() -> JfSession {
        JfSession {
            title: "Home".into(),
            kind: JfKind::Episode,
            series: Some("The Expanse".into()),
            season: Some(2),
            episode: Some(5),
            year: None,
            runtime_secs: 2640.0,
            position_secs: 1480.0,
            paused: false,
            transcoding: true,
            user: "sam".into(),
            device: "iPad".into(),
            last_activity: "2026-09-07T12:00:02.000Z".into(),
        }
    }

    fn jf(status: JfStatus, sessions: Vec<JfSession>, polled_at: f64) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        m.apply(Event::Jellyfin { status, sessions }, polled_at);
        m
    }

    #[test]
    fn session_pick_and_position_extrapolation() {
        let playing_late = movie(3720.0, false);
        let mut playing_fresh = episode();
        playing_fresh.position_secs = 10.0;
        let paused = movie(6000.0, true);
        assert_eq!(pick_session(&[paused.clone(), playing_late.clone(), playing_fresh.clone()]).unwrap().title, "Home", "the playing session that started most recently");
        let mut paused_old = paused.clone();
        paused_old.last_activity = "2026-09-07T11:00:00.000Z".into();
        assert_eq!(pick_session(&[paused_old, paused.clone()]).unwrap().position_secs, 6000.0, "all paused: the most recently active");
        assert!(pick_session(&[]).is_none());
        assert!((position(&playing_late, 10.0, 25.5) - 3735.5).abs() < 1e-6);
        assert!((position(&paused, 10.0, 25.5) - 6000.0).abs() < 1e-6, "paused does not move");
        assert!((position(&playing_late, 10.0, 5.0) - 3720.0).abs() < 1e-6, "never backwards");
        let mut near_end = playing_late.clone();
        near_end.position_secs = 9835.0;
        assert!((position(&near_end, 0.0, 100.0) - 9840.0).abs() < 1e-6, "clamped to the runtime");
        assert_eq!(display_title(&episode()), "The Expanse");
        assert_eq!(display_title(&movie(0.0, false)), "Blade Runner 2049");
    }

    #[test]
    fn jellyfin_tile_for_a_movie_an_episode_pause_idle_and_errors() {
        let m = jf(JfStatus::Ok, vec![movie(3720.0, false), episode()], 10.0);
        let s = jellyfin(&m, 16.0);
        // the episode started most recently (position 1480 < 3720): 2640 - (1480 + 6) = 1154 s
        assert_eq!((s.title.as_str(), s.hero.as_str(), s.hero_scale, s.unit.as_deref(), s.unit_below), ("NOW PLAYING", "19:14", 0.82, Some("left"), true));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 1486.0 / 2640.0).abs() < 1e-5 && color == CYAN));
        assert_eq!(s.pill, Some(("Transcoding".into(), WARN)));
        assert_eq!(
            s.card,
            Some(Card::Lines(vec![
                CardLine { text: "The Expanse".into(), font: FontId::Bold, px: 11.0, color: TEXT, dy: 7.0 },
                CardLine { text: "S2 · E5 · Home".into(), font: FontId::Medium, px: 8.5, color: TEXT2, dy: 21.0 },
                CardLine { text: "sam · iPad  ·  +1 more".into(), font: FontId::Medium, px: 8.5, color: TEXT3, dy: 32.0 },
            ]))
        );
        let m = jf(JfStatus::Ok, vec![movie(3720.0, false)], 10.0);
        let s = jellyfin(&m, 16.0);
        assert_eq!((s.hero.as_str(), s.hero_scale), ("1:41:54", 0.62));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 3726.0 / 9840.0).abs() < 1e-5 && color == ORANGE));
        assert_eq!(s.pill, Some(("Direct play".into(), OK)));
        assert!(matches!(&s.card, Some(Card::Lines(l)) if l[0].text == "Blade Runner 2049" && l[1].text == "2017 · 2h 44m" && l[2].text == "silke · Living room TV"));
        let m = jf(JfStatus::Ok, vec![movie(3720.0, true)], 10.0);
        let s = jellyfin(&m, 16.0);
        assert_eq!(s.hero, "1:42:00", "paused: no extrapolation");
        assert_eq!(s.hero_color, TEXT2);
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == ORANGE.with_alpha(0.45)));
        assert_eq!(s.pill, Some(("Paused".into(), WARN)));
        let mut idle = jf(JfStatus::Ok, vec![movie(3720.0, false)], 10.0);
        idle.set_unix_now(1_788_782_400 + 7200);
        idle.apply(Event::Jellyfin { status: JfStatus::Ok, sessions: vec![] }, 20.0);
        let s = jellyfin(&idle, 21.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert!(matches!(s.ring.kind, RingKind::Empty));
        assert_eq!(s.pill, Some(("Nothing playing".into(), TEXT2)));
        assert!(matches!(&s.card, Some(Card::Lines(l)) if l[0].text == "last watched" && l[1].text == "Blade Runner 2049 · 2 h ago"));
        let fresh = jf(JfStatus::Ok, vec![], 1.0);
        assert!(jellyfin(&fresh, 2.0).card.is_none(), "no card before any item was seen");
        let off = jf(JfStatus::Offline, vec![], 1.0);
        let s = jellyfin(&off, 2.0);
        assert_eq!((s.hero.as_str(), s.pill.clone().unwrap()), ("—", ("Jellyfin offline".to_string(), WARN)));
        let key = jf(JfStatus::Unauthorized, vec![], 1.0);
        assert_eq!(jellyfin(&key, 2.0).pill, Some(("Check API key".into(), WARN)));
    }
```

- [ ] **Step 7: Implement the builder**

In `crates/core/src/p2/roles/media.rs` extend the imports:

```rust
use crate::event::{JfKind, JfSession, JfStatus};
use crate::format::{fmt_hm, fmt_hms};
use crate::p2::chart::CardLine;
use crate::p2::font::FontId;
use crate::theme::p2::{OK, TEXT, TEXT3, WARN};
```

and append:

```rust
/// The session to show: of the playing ones the one that started most
/// recently (the smallest position); with everything paused, the most
/// recently active one.
pub fn pick_session(sessions: &[JfSession]) -> Option<&JfSession> {
    sessions
        .iter()
        .filter(|s| !s.paused)
        .min_by(|a, b| a.position_secs.total_cmp(&b.position_secs))
        .or_else(|| sessions.iter().max_by(|a, b| a.last_activity.cmp(&b.last_activity)))
}

/// Playback position now: the polled position plus the time since the poll
/// while playing, never past the end.
pub fn position(s: &JfSession, polled_at: Secs, now: Secs) -> f64 {
    let elapsed = if s.paused { 0.0 } else { (now - polled_at).max(0.0) };
    (s.position_secs + elapsed).min(s.runtime_secs.max(0.0))
}

/// The title line: the series for an episode, the item name otherwise.
pub fn display_title(s: &JfSession) -> String {
    match (&s.kind, &s.series) {
        (JfKind::Episode, Some(series)) => series.clone(),
        _ => s.title.clone(),
    }
}

fn subtitle(s: &JfSession) -> String {
    match s.kind {
        JfKind::Episode => {
            let mut parts = Vec::new();
            if let Some(n) = s.season {
                parts.push(format!("S{n}"));
            }
            if let Some(n) = s.episode {
                parts.push(format!("E{n}"));
            }
            parts.push(s.title.clone());
            parts.join(" · ")
        }
        _ => match s.year {
            Some(y) => format!("{y} · {}", fmt_hm(s.runtime_secs as i64)),
            None => fmt_hm(s.runtime_secs as i64),
        },
    }
}

fn empty_tile(pill: (String, crate::theme::Color)) -> P2Screen {
    let mut s = P2Screen::new("NOW PLAYING", "—");
    s.hero_color = TEXT3;
    s.pill = Some(pill);
    s
}

pub fn jellyfin(model: &Model, now: Secs) -> P2Screen {
    let j = model.jellyfin();
    match j.status {
        JfStatus::Offline => return empty_tile(("Jellyfin offline".into(), WARN)),
        JfStatus::Unauthorized => return empty_tile(("Check API key".into(), WARN)),
        JfStatus::Ok => {}
    }
    let Some(s) = pick_session(&j.sessions) else {
        let mut t = empty_tile(("Nothing playing".into(), TEXT2));
        if let Some((title, at)) = &j.last_seen {
            let ago = fmt_ago((model.unix_now() - at) as f64);
            t.card = Some(Card::Lines(vec![
                CardLine {
                    text: "last watched".into(),
                    font: FontId::Medium,
                    px: 8.5,
                    color: TEXT3,
                    dy: 4.0,
                },
                CardLine {
                    text: format!("{title} · {ago}"),
                    font: FontId::SemiBold,
                    px: 10.0,
                    color: TEXT2,
                    dy: 18.0,
                },
            ]));
        }
        return t;
    };
    let pos = position(s, j.polled_at, now);
    let left = (s.runtime_secs - pos).max(0.0);
    let mut t = P2Screen::new("NOW PLAYING", fmt_hms(left));
    t.hero_scale = if left >= 3600.0 { 0.62 } else { 0.82 };
    t.unit = Some("left".into());
    t.unit_below = true;
    if s.paused {
        t.hero_color = TEXT2;
    }
    let color = if s.kind == JfKind::Movie { ORANGE } else { CYAN };
    t.ring = RingSpec::fill(
        (pos / s.runtime_secs.max(1.0)) as f32,
        if s.paused { color.with_alpha(0.45) } else { color },
    );
    t.pill = Some(if s.paused {
        ("Paused".into(), WARN)
    } else if s.transcoding {
        ("Transcoding".into(), WARN)
    } else {
        ("Direct play".into(), OK)
    });
    let others = j.sessions.len().saturating_sub(1);
    let mut foot = format!("{} · {}", s.user, s.device);
    if others > 0 {
        foot.push_str(&format!("  ·  +{others} more"));
    }
    t.card = Some(Card::Lines(vec![
        CardLine {
            text: display_title(s),
            font: FontId::Bold,
            px: 11.0,
            color: TEXT,
            dy: 7.0,
        },
        CardLine {
            text: subtitle(s),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT2,
            dy: 21.0,
        },
        CardLine {
            text: foot,
            font: FontId::Medium,
            px: 8.5,
            color: TEXT3,
            dy: 32.0,
        },
    ]));
    t
}
```

In the model `apply` arm from Step 4 use `crate::p2::roles::media::display_title(s)` for `last_seen`. Dispatcher arm: `Role::Jellyfin => media::jellyfin(model, now),` (the `return None` arm is now only `Role::PowerMix | Role::Face`). `needs_data`: `Role::Jellyfin => !self.jellyfin().have,` (the placeholder `true` arm is gone).

- [ ] **Step 8: Run the core and source tests**

Run: `cargo test -p rackscreen-core && cargo test -p rackscreen-sources`
Expected: green.

- [ ] **Step 9: Goldens**

In `crates/render/tests/golden.rs` import `JfKind, JfSession, JfStatus`; in `full_model()` add `m.set_jellyfin_configured(true);` next to the other presence flags, and after the `CiRuns` event:

```rust
    m.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(false), jf_episode()],
        },
        0.0,
    );
```

with, at file scope:

```rust
fn jf_movie(paused: bool) -> JfSession {
    JfSession {
        title: "Blade Runner 2049".into(),
        kind: JfKind::Movie,
        series: None,
        season: None,
        episode: None,
        year: Some(2017),
        runtime_secs: 9840.0,
        position_secs: 3720.0,
        paused,
        transcoding: false,
        user: "silke".into(),
        device: "Living room TV".into(),
        last_activity: "2026-09-07T12:00:05.000Z".into(),
    }
}

fn jf_episode() -> JfSession {
    JfSession {
        title: "Home".into(),
        kind: JfKind::Episode,
        series: Some("The Expanse".into()),
        season: Some(2),
        episode: Some(5),
        year: None,
        runtime_secs: 2640.0,
        position_secs: 1480.0,
        paused: false,
        transcoding: true,
        user: "sam".into(),
        device: "iPad".into(),
        last_activity: "2026-09-07T12:00:02.000Z".into(),
    }
}
```

The normal golden shows the episode (it started most recently). Append a test:

```rust
#[test]
fn jellyfin_alternate_states() {
    let mut e = full_model();
    e.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(false)],
        },
        0.0,
    );
    check("p2_jellyfin_movie", &render(&e, Role::Jellyfin, 120.0));
    let mut p = full_model();
    p.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![jf_movie(true)],
        },
        0.0,
    );
    check("p2_jellyfin_paused", &render(&p, Role::Jellyfin, 120.0));
    let mut i = full_model();
    i.set_unix_now(UNIX + 2 * 3600);
    i.apply(
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_idle", &render(&i, Role::Jellyfin, 120.0));
    let mut o = full_model();
    o.apply(
        Event::Jellyfin {
            status: JfStatus::Offline,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_offline", &render(&o, Role::Jellyfin, 120.0));
    let mut k = full_model();
    k.apply(
        Event::Jellyfin {
            status: JfStatus::Unauthorized,
            sessions: vec![],
        },
        0.0,
    );
    check("p2_jellyfin_key", &render(&k, Role::Jellyfin, 120.0));
    let mut c = full_model();
    c.set_jellyfin_configured(false);
    check("p2_jellyfin_config", &render(&c, Role::Jellyfin, 120.0));
}
```

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden every_role_normal jellyfin_alternate_states no_data_token_location_and_connecting`
Expected: `p2_jellyfin.png` re-captured (cyan ring, `Transcoding`, `The Expanse` card), five new PNGs, and `p2_jellyfin_nodata.png` now reads `Check config` (the bare model is not configured). Note the `nodata` golden for jellyfin is the config state; `p2_jellyfin_config.png` renders the same tile from the full model and documents that.

- [ ] **Step 10: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

```bash
git add -A
git commit -m "feat(sources,core,app): Jellyfin sessions and the now-playing screen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Fake source, gifs, README, version 0.8.0

**Files:**
- Modify: `crates/sources/src/fake.rs`, `crates/app/examples/gifs.rs`, `README.md`, `Cargo.toml`, `Cargo.lock`
- Test: `fake.rs` unit tests; `cargo run -p rackscreen-app --example gifs -- .github/media/screens`

**Interfaces:**
- Consumes: every event from Tasks 3–7, `FakeCmd::from_key`.
- Produces: `FakeCmd::{MediaDone, KickRestart, Brownout, JellyfinToggle, CiPass}` on keys `m`, `k`, `b`, `j`, `c`; gifs `media.gif … jellyfin.gif`; version `0.8.0`.

- [ ] **Step 1: Write the failing fake-source tests**

In `crates/sources/src/fake.rs` tests, extend `keys_map`:

```rust
        assert_eq!(FakeCmd::from_key('m'), Some(FakeCmd::MediaDone));
        assert_eq!(FakeCmd::from_key('k'), Some(FakeCmd::KickRestart));
        assert_eq!(FakeCmd::from_key('b'), Some(FakeCmd::Brownout));
        assert_eq!(FakeCmd::from_key('j'), Some(FakeCmd::JellyfinToggle));
        assert_eq!(FakeCmd::from_key('c'), Some(FakeCmd::CiPass));
```

append to `initial_has_every_new_role_fed`:

```rust
        assert!(evs.iter().any(|e| matches!(e, Event::MediaLibrary { movies: 37, series: 30, artists: 54 })));
        assert!(evs.iter().any(|e| matches!(e, Event::Download(Some(d)) if d.progress > 0.3)));
        assert!(evs.iter().any(|e| matches!(e, Event::TrafficHistory { req_per_sec } if req_per_sec.len() == 31)));
        assert!(evs.iter().any(|e| matches!(e, Event::Traffic { top, .. } if top.len() == 3)));
        assert!(evs.iter().any(|e| matches!(e, Event::RestartsHistory { hourly } if hourly.len() == 24 && hourly[23].0 == 1_788_782_400 / 3600)));
        assert!(evs.iter().any(|e| matches!(e, Event::Restarts { total_24h, worst: Some(_) } if *total_24h > 0)));
        assert!(evs.iter().any(|e| matches!(e, Event::Uptime(n) if n.len() == 7)));
        assert!(evs.iter().any(|e| matches!(e, Event::Fans(f) if f.len() == 4)));
        assert!(evs.iter().any(|e| matches!(e, Event::MainsHistory { hourly } if hourly.len() == 24)));
        assert!(evs.iter().any(|e| matches!(e, Event::Mains { volts, .. } if (*volts - 234.0).abs() < 1e-3)));
        assert!(evs.iter().any(|e| matches!(e, Event::Load(n) if n.len() == 7)));
        assert!(evs.iter().any(|e| matches!(e, Event::CiRuns(r) if r.len() == 12 && r[0].status == CiStatus::InProgress)));
        assert!(evs.iter().any(|e| matches!(e, Event::Jellyfin { status: JfStatus::Ok, sessions } if sessions.len() == 2)));
```

and a new test:

```rust
    #[test]
    fn new_screen_commands_drive_their_tiles() {
        let mut s = FakeState::new(1);
        s.set_clock(1_788_782_400, 7200);
        let evs = s.command(FakeCmd::MediaDone);
        assert_eq!(evs[0], Event::MediaAdded { title: "The Lord of the Rings: The Return of the King".into() });
        assert!(matches!(evs[1], Event::Download(None)));
        let later: Vec<Event> = (0..60).flat_map(|_| s.tick()).collect();
        assert!(later.iter().any(|e| matches!(e, Event::Download(Some(_)))), "a new download starts within a minute");
        let before = s.restart_total;
        let evs = s.command(FakeCmd::KickRestart);
        assert!(matches!(&evs[0], Event::Restarts { total_24h, worst: Some((_, n)) } if *total_24h == before + 6 && *n >= 6));
        assert!(matches!(&evs[1], Event::RestartsHistory { hourly } if hourly[23].1 >= 6.0));
        assert!(matches!(s.command(FakeCmd::Brownout).as_slice(), [Event::Mains { volts, .. }] if *volts < 220.0));
        assert!(matches!(s.command(FakeCmd::Brownout).as_slice(), [Event::Mains { volts, .. }] if *volts > 230.0));
        assert!(matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if sessions[0].paused));
        assert!(matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if sessions.is_empty()));
        assert!(matches!(s.command(FakeCmd::JellyfinToggle).as_slice(), [Event::Jellyfin { sessions, .. }] if !sessions.is_empty() && !sessions[0].paused));
        let evs = s.command(FakeCmd::CiFailure);
        assert!(evs.iter().any(|e| matches!(e, Event::GithubRun { ok: false, .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::CiRun { ok: false, .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::CiRuns(r) if r[0].status == CiStatus::Failure)));
        let evs = s.command(FakeCmd::CiPass);
        assert!(evs.iter().any(|e| matches!(e, Event::CiRun { ok: true, .. })));
        assert!(evs.iter().any(|e| matches!(e, Event::CiRuns(r) if r[0].status == CiStatus::Success)));
        let ten: Vec<Event> = (0..10).flat_map(|_| s.tick()).collect();
        assert!(ten.iter().any(|e| matches!(e, Event::Traffic { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Jellyfin { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Load(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Fans(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Mains { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::Uptime(_))));
        assert!(ten.iter().any(|e| matches!(e, Event::Restarts { .. })));
        assert!(ten.iter().any(|e| matches!(e, Event::MediaLibrary { .. })));
    }
```

Update the existing `new_commands_emit_their_edges` assertion on `CiFailure` (it now returns three events): replace the `matches!(s.command(FakeCmd::CiFailure).as_slice(), [Event::GithubRun { ok: false, .. }])` with `assert!(s.command(FakeCmd::CiFailure).iter().any(|e| matches!(e, Event::GithubRun { ok: false, .. })));`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rackscreen-sources fake`
Expected: compile errors (`FakeCmd::MediaDone` …).

- [ ] **Step 3: Implement the fake data**

In `crates/sources/src/fake.rs` extend the event import to

```rust
use rackscreen_core::event::{
    App, AppHealth, AppSync, CiRun, CiStatus, Download, Event, IssPass, JfKind, JfSession,
    JfStatus, LinkTarget, NodeLoad, Robustness, Torrent,
};
```

constants after `ISS_PASS_SECS`:

```rust
/// Media library counts and the demo download.
const MEDIA_LIBRARY: (u32, u32, u32) = (37, 30, 54);
const DOWNLOAD_TITLE: &str = "The Lord of the Rings: The Return of the King";
/// Restarts per hour over the last day, oldest first.
const RESTART_SEED: [f32; 24] = [
    0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 7.0, 9.0, 11.0, 6.0, 3.0, 1.0, 0.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];
const WORST_POD: &str = "n8n-worker";
/// Days up per node (same order as `NODES`); the two short ones rebooted lately.
const UPTIME_DAYS: [f64; 7] = [79.6, 79.6, 79.6, 82.1, 41.0, 0.375, 4.4];
/// Fan rpm for the four Pis (`NODES[3..]`).
const FAN_SEED: [f32; 4] = [4885.0, 4538.0, 6059.0, 5104.0];
const MAINS_V: f32 = 234.0;
const BROWNOUT_V: f32 = 212.0;
/// 1-minute load and cores per node.
const LOAD_SEED: [(f32, u32); 7] = [(0.81, 8), (0.02, 12), (0.12, 16), (0.22, 4), (0.13, 4), (3.1, 4), (1.02, 4)];
const CI_REPO: &str = "silkepilon/RackScreen";
```

`FakeCmd` variants and keys:

```rust
    MediaDone,
    KickRestart,
    Brownout,
    JellyfinToggle,
    CiPass,
```

```rust
            'm' => FakeCmd::MediaDone,
            'k' => FakeCmd::KickRestart,
            'b' => FakeCmd::Brownout,
            'j' => FakeCmd::JellyfinToggle,
            'c' => FakeCmd::CiPass,
```

`FakeState` fields (after `ticks: u64,`):

```rust
    /// Download progress 0..1, `None` between downloads; `dl_restart_at` is the tick a new one starts.
    dl_progress: Option<f32>,
    dl_restart_at: u64,
    traffic_rps: f32,
    restart_hours: Vec<f32>,
    pub(crate) restart_total: u32,
    uptime_days: Vec<f64>,
    fans: Vec<f32>,
    brownout: bool,
    loads: Vec<f32>,
    /// Newest first, at most 12.
    ci_runs: Vec<CiStatus>,
    /// 0 playing, 1 paused, 2 idle.
    jf_state: u8,
    jf_pos: f64,
```

initialised in `new`:

```rust
            dl_progress: Some(0.35),
            dl_restart_at: 0,
            traffic_rps: 3.4,
            restart_hours: RESTART_SEED.to_vec(),
            restart_total: RESTART_SEED.iter().sum::<f32>() as u32,
            uptime_days: UPTIME_DAYS.to_vec(),
            fans: FAN_SEED.to_vec(),
            brownout: false,
            loads: LOAD_SEED.iter().map(|(l, _)| *l).collect(),
            ci_runs: (0..12)
                .map(|i| match i {
                    0 => CiStatus::InProgress,
                    7 => CiStatus::Failure,
                    _ => CiStatus::Success,
                })
                .collect(),
            jf_state: 0,
            jf_pos: 3720.0,
```

Event builders (after `app_sync_history`):

```rust
    fn media_library(&self) -> Event {
        Event::MediaLibrary {
            movies: MEDIA_LIBRARY.0,
            series: MEDIA_LIBRARY.1,
            artists: MEDIA_LIBRARY.2,
        }
    }

    fn download(&self) -> Event {
        Event::Download(self.dl_progress.map(|p| Download {
            title: DOWNLOAD_TITLE.into(),
            progress: p,
            eta_secs: ((1.0 - p) * 1800.0) as i64,
        }))
    }

    fn traffic(&self) -> Event {
        let total = self.traffic_rps.max(0.1);
        Event::Traffic {
            req_per_sec: self.traffic_rps,
            top: vec![
                ("n8n".into(), total * 0.48),
                ("portfolio".into(), total * 0.27),
                ("grafana".into(), total * 0.15),
            ],
            service_rps: total,
            error_share: 0.0,
        }
    }

    fn traffic_history(&self) -> Event {
        Event::TrafficHistory {
            req_per_sec: (0..31)
                .map(|i| (self.traffic_rps + 1.6 * ((i as f32) * 0.4).sin()).max(0.0))
                .collect(),
        }
    }

    fn restarts(&self) -> Event {
        let worst_n = self.restart_hours.iter().cloned().fold(0.0_f32, f32::max) as u32 + 22;
        Event::Restarts {
            total_24h: self.restart_total,
            worst: (self.restart_total > 0).then(|| (WORST_POD.to_string(), worst_n.min(self.restart_total))),
        }
    }

    fn restarts_history(&self) -> Event {
        let hour = self.unix_now().div_euclid(3600);
        Event::RestartsHistory {
            hourly: self
                .restart_hours
                .iter()
                .enumerate()
                .map(|(i, v)| (hour - 23 + i as i64, *v))
                .collect(),
        }
    }

    fn uptime(&self) -> Event {
        Event::Uptime(
            NODES
                .iter()
                .zip(&self.uptime_days)
                .map(|((n, _), d)| ((*n).to_string(), d * 86_400.0 + self.ticks as f64))
                .collect(),
        )
    }

    fn fans(&self) -> Event {
        Event::Fans(
            NODES[3..]
                .iter()
                .zip(&self.fans)
                .map(|((n, _), rpm)| ((*n).to_string(), *rpm))
                .collect(),
        )
    }

    fn mains(&self) -> Event {
        Event::Mains {
            volts: if self.brownout { BROWNOUT_V } else { MAINS_V },
            ups_load_pct: 7.0,
        }
    }

    fn mains_history(&self) -> Event {
        let hour = self.unix_now().div_euclid(3600);
        Event::MainsHistory {
            hourly: (0..24)
                .map(|i| (hour - 23 + i, MAINS_V - 1.0 + 2.0 * ((i as f32) / 3.0).sin()))
                .collect(),
        }
    }

    fn load(&self) -> Event {
        Event::Load(
            NODES
                .iter()
                .zip(LOAD_SEED.iter().zip(&self.loads))
                .map(|((n, _), ((_, cores), l1))| NodeLoad {
                    node: (*n).to_string(),
                    load1: *l1,
                    load5: l1 * 0.85,
                    load15: l1 * 0.7,
                    cores: *cores,
                })
                .collect(),
        )
    }

    fn ci_runs(&self) -> Event {
        let now = self.unix_now();
        Event::CiRuns(
            self.ci_runs
                .iter()
                .enumerate()
                .map(|(i, status)| {
                    let created_at = now - (i as i64 + 1) * 1800;
                    let duration = (*status != CiStatus::InProgress).then_some(120 + i as i64 * 3);
                    CiRun {
                        id: 500 - i as u64,
                        repo: CI_REPO.into(),
                        status: *status,
                        created_at,
                        updated_at: created_at + duration.unwrap_or(0),
                        duration_secs: duration,
                    }
                })
                .collect(),
        )
    }

    fn jellyfin(&self) -> Event {
        let sessions = match self.jf_state {
            2 => Vec::new(),
            st => vec![
                JfSession {
                    title: "Blade Runner 2049".into(),
                    kind: JfKind::Movie,
                    series: None,
                    season: None,
                    episode: None,
                    year: Some(2017),
                    runtime_secs: 9840.0,
                    position_secs: self.jf_pos,
                    paused: st == 1,
                    transcoding: false,
                    user: "silke".into(),
                    device: "Living room TV".into(),
                    last_activity: "2026-09-07T12:00:05.000Z".into(),
                },
                JfSession {
                    title: "Home".into(),
                    kind: JfKind::Episode,
                    series: Some("The Expanse".into()),
                    season: Some(2),
                    episode: Some(5),
                    year: None,
                    runtime_secs: 2640.0,
                    position_secs: self.jf_pos + 2000.0,
                    paused: true,
                    transcoding: true,
                    user: "sam".into(),
                    device: "iPad".into(),
                    last_activity: "2026-09-07T11:50:02.000Z".into(),
                },
            ],
        };
        Event::Jellyfin {
            status: JfStatus::Ok,
            sessions,
        }
    }
```

Append to `initial()`'s vector:

```rust
            self.media_library(),
            self.download(),
            self.traffic_history(),
            self.traffic(),
            self.restarts_history(),
            self.restarts(),
            self.uptime(),
            self.fans(),
            self.mains_history(),
            self.mains(),
            self.load(),
            self.ci_runs(),
            self.jellyfin(),
```

In `tick()`, after the `out.push(self.net());` line:

```rust
        self.traffic_rps = (self.traffic_rps + self.rng.f32() * 0.8 - 0.4).clamp(0.5, 9.0);
        out.push(self.traffic());
        if let Some(p) = self.dl_progress.as_mut() {
            *p = (*p + 0.004).min(0.99);
        } else if self.ticks >= self.dl_restart_at {
            self.dl_progress = Some(0.05);
        }
        out.push(self.download());
        if self.jf_state == 0 {
            self.jf_pos = (self.jf_pos + 1.0).min(9839.0);
        }
        if self.ticks.is_multiple_of(2) {
            out.push(self.jellyfin());
        }
```

and in the `is_multiple_of(5)` block add `out.push(self.load()); out.push(self.fans()); out.push(self.mains()); out.push(self.uptime());` with a little drift before them:

```rust
            for l in self.loads.iter_mut() {
                *l = (*l + self.rng.f32() * 0.1 - 0.05).max(0.01);
            }
            for f in self.fans.iter_mut() {
                *f = (*f + self.rng.f32() * 120.0 - 60.0).clamp(3000.0, 7900.0);
            }
```

in the `is_multiple_of(10)` block add `out.push(self.media_library()); out.push(self.restarts());`, and in the `is_multiple_of(30)` block `out.push(self.ci_runs());`.

Commands (new arms in `command`):

```rust
            FakeCmd::MediaDone => {
                self.dl_progress = None;
                self.dl_restart_at = self.ticks + 45;
                vec![
                    Event::MediaAdded {
                        title: DOWNLOAD_TITLE.into(),
                    },
                    self.download(),
                ]
            }
            FakeCmd::KickRestart => {
                self.restart_hours[23] += 6.0;
                self.restart_total += 6;
                vec![self.restarts(), self.restarts_history()]
            }
            FakeCmd::Brownout => {
                self.brownout = !self.brownout;
                vec![self.mains()]
            }
            FakeCmd::JellyfinToggle => {
                self.jf_state = (self.jf_state + 1) % 3;
                vec![self.jellyfin()]
            }
            FakeCmd::CiPass => {
                self.ci_runs.insert(0, CiStatus::Success);
                self.ci_runs.truncate(12);
                vec![
                    Event::CiRun {
                        repo: CI_REPO.into(),
                        ok: true,
                        at: self.unix_now(),
                    },
                    self.ci_runs(),
                ]
            }
```

and `CiFailure` becomes:

```rust
            FakeCmd::CiFailure => {
                self.ci_runs.insert(0, CiStatus::Failure);
                self.ci_runs.truncate(12);
                vec![
                    Event::GithubRun {
                        repo: CI_REPO.into(),
                        ok: false,
                    },
                    Event::CiRun {
                        repo: CI_REPO.into(),
                        ok: false,
                        at: self.unix_now(),
                    },
                    self.ci_runs(),
                ]
            }
```

Add `use rackscreen_core::event::{CiStatus, JfStatus};` to the fake test module imports if the test file does not already see them through `super::*` (it does, via the extended import).

- [ ] **Step 4: Run the source tests**

Run: `cargo test -p rackscreen-sources`
Expected: green.

- [ ] **Step 5: Gifs**

In `crates/app/examples/gifs.rs` `clips()`, add arms to the `match role` before `_ => Vec::new()`:

```rust
                Role::Media => vec![(3.0, Step::Cmd(FakeCmd::MediaDone))],
                Role::Restarts => vec![(2.5, Step::Cmd(FakeCmd::KickRestart))],
                Role::Mains => vec![
                    (2.5, Step::Cmd(FakeCmd::Brownout)),
                    (6.0, Step::Cmd(FakeCmd::Brownout)),
                ],
                Role::Ci => vec![
                    (2.0, Step::Cmd(FakeCmd::CiFailure)),
                    (5.0, Step::Cmd(FakeCmd::CiPass)),
                ],
                Role::Jellyfin => vec![
                    (3.0, Step::Cmd(FakeCmd::JellyfinToggle)),
                    (6.0, Step::Cmd(FakeCmd::JellyfinToggle)),
                ],
```

(traffic, uptime, fans and load drift on their own.) The `record` function sets the model's presence flags; add `model.set_jellyfin_configured(true);` and `model.set_arr_configured(true);` next to `model.set_github_token_present(true);`.

Run: `cargo run --release -p rackscreen-app --example gifs -- .github/media/screens`
Expected: 33 GIFs listed (31 roles, `health-torrent`, `all`); `media.gif` shows the title scrolling, the bar reaching 100 % green with `added just now`, `jellyfin.gif` goes playing → paused (dim ring) → `Nothing playing`. `all.gif` now runs 31 × 3.5 s.

- [ ] **Step 6: README**

In `README.md`:

1. Intro paragraph: replace "Nine roles come from the cluster, four from … and one from GitHub" with "Sixteen roles come from the cluster, four from [Electricity Maps](https://app.electricitymaps.com) and the day-ahead price feeds, seven from the sky over your house, two from GitHub and one from a Jellyfin server".
2. Gallery table: after the `deploys`/`face` row append:

```markdown
| <img src=".github/media/screens/media.gif" alt="MEDIA role" width="200"> | <img src=".github/media/screens/traffic.gif" alt="WEB TRAFFIC role" width="200"> |
| **`media`** — movies, series and artists on the ring; the active arr download in the card, or the last item added | **`traffic`** — Traefik requests per second, the three busiest services on the ring, the error share in the pill |
| <img src=".github/media/screens/restarts.gif" alt="RESTARTS role" width="200"> | <img src=".github/media/screens/uptime.gif" alt="UPTIME role" width="200"> |
| **`restarts`** — container restarts in 24 h, one segment per hour coloured by count, the worst pod in the pill | **`uptime`** — the longest node uptime, a segment per node, amber for a reboot in the last day |
| <img src=".github/media/screens/fans.gif" alt="FANS role" width="200"> | <img src=".github/media/screens/mains.gif" alt="MAINS role" width="200"> |
| **`fans`** — the loudest fan in rpm, a bar per node with a fan | **`mains`** — UPS input voltage on six bands over 200–260 V, the last 24 h as a sparkline |
| <img src=".github/media/screens/load.gif" alt="LOAD role" width="200"> | <img src=".github/media/screens/ci.gif" alt="CI role" width="200"> |
| **`load`** — the highest 1-minute load, a segment per node by load per core, cluster means in the card | **`ci`** — the last twelve GitHub Actions runs as segments, passes over total, the newest run in the pill |
| <img src=".github/media/screens/jellyfin.gif" alt="NOW PLAYING role" width="200"> | |
| **`jellyfin`** — time left on what is playing, the ring is the position, direct play or transcoding, title and viewer in the card | |
```

3. "Screens and roles": `Twenty-two roles exist` → `Thirty-one roles exist`, and append rows:

```markdown
| `media` | Prometheus (scraparr) + Radarr/Sonarr/Lidarr | ring movies, series and artists, pill the download count or the library size, card the active download or the last item added |
| `traffic` | Prometheus (Traefik) | req/s, ring the three busiest services, pill the busiest and the 5xx share, card the last 90 s |
| `restarts` | Prometheus (kube-state-metrics) | restarts in 24 h, a segment per hour (green 1, amber 2–5, red 6+), pill the worst pod, card restarts per hour |
| `uptime` | Prometheus (node-exporter) | longest uptime in days, a segment per node, pill the latest reboot, card days per node |
| `fans` | Prometheus (node-exporter hwmon) | loudest fan in rpm, ring over 8000, pill the node, card a bar per node |
| `mains` | Prometheus (nut-exporter) | input voltage on six bands, pill stable or low/high voltage with the UPS load, card the last 24 h |
| `load` | Prometheus (node-exporter) | highest 1-minute load, a segment per node by load per core, card the cluster means |
| `ci` | GitHub Actions | passes over the last twelve runs, a segment per run, pill the newest run, card average run time and failures |
| `jellyfin` | Jellyfin | time left, ring the position (amber movie, cyan episode, dim when paused), pill direct play / transcoding / paused, card title, episode or year and viewer |
```

4. Setup TUI bullet for **Configure**: "every setting in seven groups (Cluster, Services, Energy, Sky, Media & Jellyfin, Display, Thresholds) … `1`–`7` jump straight to one".
5. After "## Face role" add three sections:

```markdown
## Media role

`media` reads the library counts the [scraparr](https://github.com/thecfu/scraparr) exporter puts in Prometheus (`radarr_movies_total`, `sonarr_series_total`, `lidarr_artists_total`). The card shows the download with the least time left from the Radarr, Sonarr and Lidarr queues: turn on `media enabled` in **Configure** and paste each app's API key (Settings → General) into `radarr api key`, `sonarr api key` and `lidarr api key`; an app without a key is skipped. The services are reached over the same kube tunnel as Prometheus, in `media namespace` (`arr-stack`), `service: auto` picking the first service whose name contains the app's. Without any key the card falls back to the most complete qBittorrent torrent. When an item leaves a queue completed the card says "added just now" for a minute, then shows the last item added and its age.

## CI role

`ci` shows the last twelve GitHub Actions runs across your repositories: passes over total as the number, one segment per run (green passed, red failed, blue running, dark cancelled), the newest run in the pill and the average run time in the card. It uses the `github token`; set `github ci repos` to a comma-separated `owner/repo` list, or leave it empty to watch your non-fork repositories pushed in the last seven days (at most ten). A repository the token cannot read is skipped with one warning an hour. A run that passes keeps the face happy for half an hour.

## Jellyfin role

`jellyfin` shows what is playing on your Jellyfin server: the time left as the number, the position on the ring (amber for a movie, cyan for an episode, dimmed while paused), "Direct play", "Transcoding" or "Paused" in the pill, and the title, episode or year and the viewer in the card. With several sessions it picks the one that started most recently. Turn on `jellyfin enabled`, set `jellyfin url` to the server (`http://192.168.0.x:8096`; it lives outside the cluster, so the Pi must reach it directly) and paste a key from Dashboard → API Keys into `jellyfin api key`; `jellyfin poll` is 5 s by default. While the URL is empty the tile says "Check config"; an unreachable server says "Jellyfin offline" and a rejected key "Check API key". With nothing playing it shows the last thing watched this run.
```

6. Configuration bullets: `- **`github`** — see [GitHub role](#github-role) and [CI role](#ci-role).`, and add `- **`media`** — see [Media role](#media-role).` and `- **`jellyfin`** — see [Jellyfin role](#jellyfin-role).` after the `argocd` bullet. Replace the `config.example.yaml` block inside `<details>` with the current file contents (`sed -n '/^```yaml$/,/^```$/p' config.example.yaml` is not needed: copy the file verbatim).
7. Simulator keys: append `, `m` the download finishes, `k` a restart burst, `b` a mains brownout and back, `j` Jellyfin playing → paused → idle, `c` a passing CI run` before `, `Esc` quit`.

- [ ] **Step 7: Version**

`Cargo.toml`: `version = "0.8.0"` under `[workspace.package]`. Run `cargo build` so `Cargo.lock` follows.

- [ ] **Step 8: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green, no golden changes (the goldens render the model directly, not the fake source).

```bash
git add -A
git commit -m "chore: fake data, gifs and README for the nine new roles, version 0.8.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review

**Spec coverage.** Roles, ids, titles, `is_cluster`, TUI picker, `Role::parse`, README table and config comment: Task 1 (+ Task 8 README). Screens table: traffic, restarts, uptime, fans, mains, load in Task 4; media in Task 5; ci in Task 6; jellyfin in Task 7, each with the ring kind, pill texts and colours and the card of the spec table, including the Jellyfin session pick rule, position extrapolation, "Nothing playing" with the last-watched card, offline / 401 pills and "Check config". Shared rules: no data → `—` and "Waiting for data" (the Task 1 `needs_data` arms, replaced per role), link down → connecting (`is_cluster`), long text → marquee (pill, `Card::Download` title, every `Card::Lines` line). Data sources: Prometheus instant and range queries per the table (Task 3; the 5xx share is two queries divided locally), node names from the `node` label with `node_tag` as fallback; arr queue with `X-Api-Key`, `service: auto`, 10 s, least time left, titles per app, "added just now", skip without key, torrent fallback (Task 5); GitHub runs `per_page=12`, `ci_repos` or discovery (7 days, max 10), merged newest first, average run time, `CiRun { repo, ok, at }` (Task 6); Jellyfin `/Sessions?activeWithinSeconds=60`, the `MediaBrowser Token` header, the listed fields, 100 ns ticks (Task 7). Face inputs: `recent_restart` (Task 3), `ci_passed_recently` (Task 6). Config blocks, validation and the "Media & Jellyfin" group with masked keys: Task 1. Errors and edge cases: "No exporter" (Task 1 + Task 3 `NoExporter` events), nodes without a fan absent (`per_node` only lists exported series), absolute ring colours with bars scaled to the max hour (Task 4), CI 404/403 warned once an hour (Task 6), Jellyfin position taken from the server each poll (Task 7). Tests: parsers for the three arr queues, CI runs, user repos, Jellyfin sessions (movie, episode, paused, transcoding, none), `parse_matrix_ts`; scene builders per role in normal, warning and no-data states; pick rule and extrapolation; goldens for every new role. Docs and release: Task 8.

**Deviations from the spec, on purpose.** (1) `media.namespace` is a config key (default `arr-stack`) rather than hard-coded, so the TUI can change it. (2) The 5xx share is `sum(5xx) / sum(all)` computed in Rust instead of the PromQL quotient, which is an empty vector on a quiet site and would read as "no exporter". (3) `node` label first, `node_tag(instance)` second: the live cluster carries `node` on every node-exporter series, which gives the same names the thermal screen shows. (4) Jellyfin "started most recently" is read as the smallest playback position among playing sessions (sessions carry no start time); with everything paused, `LastActivityDate` decides. (5) The Jellyfin tile uses the standard unit-below layout (`left` under the hero, pill at 134, card at 152) rather than the mockup's hand-placed 110/128/146, so `compose` stays untouched. (6) With nothing queued and nothing ever added, the media card shows the three library counts rather than nothing.

**Placeholder scan.** Task 1's `needs_data` and dispatcher arms for the nine roles are explicit intermediate code (`true` / `return None`) that Tasks 4–7 replace; every other step carries its full code. No "TBD", "similar to", or "add validation" remains.

**Type consistency.** `Event::Traffic` has the four fields `req_per_sec, top, service_rps, error_share` in Tasks 3, 4 and 8. `CiRun` carries `updated_at` everywhere (Tasks 6, 8, goldens). `JfSession` has the thirteen fields in Tasks 7 and 8 and the goldens. `marquee_run` takes `now` last in Task 2 and its callers in `chart.rs`. `Card::Download { title, progress, label, done }` and `Card::Lines(Vec<CardLine>)` match between Task 2, Task 5 and Task 7. `Model::mood_inputs(now)` is changed in Task 3 and used with `now` in Tasks 3 and 6.

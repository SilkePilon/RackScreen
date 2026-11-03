//! Configure: a form over the YAML fields with inline editing and validation.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use rackscreen_app::config::Config;
use rackscreen_app::play;
use rackscreen_core::anim::Secs;
use rackscreen_core::night::parse_hhmm;
use rackscreen_core::shows::SHOWS;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::config_file::save_config;
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::screens::gifs::GifPane;
use crate::widgets::{confirm_dialog, help_line, SidebarView, StatusTone};
use crate::{Action, Screen, Shared};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Secret,
    Number,
    Bool,
    /// Not stored in the config: ⏎ opens an editor and applying runs it.
    Run,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Kubeconfig,
    PromNamespace,
    PromService,
    PromPort,
    PromPoll,
    QbitEnabled,
    QbitNamespace,
    QbitService,
    QbitPort,
    QbitUser,
    QbitPass,
    QbitPoll,
    NightEnabled,
    NightStart,
    NightEnd,
    HotCpu,
    HotMem,
    Brightness,
    Fps,
    SpiChunk,
    OneAtATime,
    ElecEnabled,
    ElecZone,
    ElecToken,
    ElecPoll,
    HotTemp,
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
    ShowsEnabled,
    ShowsEvery,
    ShowsGap,
    /// One checklist row per show, by index into `SHOWS`.
    Show(u8),
    ShowsPlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cluster,
    Services,
    Energy,
    Sky,
    Media,
    Display,
    Thresholds,
    Shows,
    /// The `gifs:` list; a list pane rather than form fields.
    Gifs,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::Cluster,
        Group::Services,
        Group::Energy,
        Group::Sky,
        Group::Media,
        Group::Display,
        Group::Thresholds,
        Group::Shows,
        Group::Gifs,
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
            Group::Shows => "Shows",
            Group::Gifs => "GIFs",
        }
    }
    pub fn index(self) -> usize {
        Group::ALL
            .iter()
            .position(|g| *g == self)
            .expect("group in ALL")
    }
}

/// One row of the Configure form.
pub struct FieldSpec {
    pub field: Field,
    pub group: Group,
    /// Module header the field sits under (`Prometheus`, `Night`).
    pub section: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    /// One sentence for the help line, under 60 characters.
    pub help: &'static str,
}

const fn spec(
    field: Field,
    group: Group,
    section: &'static str,
    label: &'static str,
    kind: FieldKind,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        field,
        group,
        section,
        label,
        kind,
        help,
    }
}

use FieldKind::{Bool, Number, Run, Secret, Text};
use Group::{Cluster, Display, Energy, Media, Services, Shows, Sky, Thresholds};

/// Checklist row for show `i`, labelled with its id.
const fn show(i: u8) -> FieldSpec {
    spec(
        Field::Show(i),
        Shows,
        "Rotation",
        SHOWS[i as usize].id,
        Bool,
        "In the random rotation; at least one stays on.",
    )
}

pub const FIELDS: [FieldSpec; 70] = [
    spec(
        Field::Kubeconfig,
        Cluster,
        "Kubernetes",
        "kubeconfig",
        Text,
        "Path to the kubeconfig; blank uses in-cluster access.",
    ),
    spec(
        Field::PromNamespace,
        Cluster,
        "Prometheus",
        "namespace",
        Text,
        "Namespace the Prometheus service runs in.",
    ),
    spec(
        Field::PromService,
        Cluster,
        "Prometheus",
        "service",
        Text,
        "Name of the Prometheus service.",
    ),
    spec(
        Field::PromPort,
        Cluster,
        "Prometheus",
        "port",
        Number,
        "Port of the Prometheus service inside the cluster.",
    ),
    spec(
        Field::PromPoll,
        Cluster,
        "Prometheus",
        "poll",
        Number,
        "Seconds between Prometheus queries, at least 1.",
    ),
    spec(
        Field::QbitEnabled,
        Services,
        "qBittorrent",
        "enabled",
        Bool,
        "Show torrent traffic on the ring.",
    ),
    spec(
        Field::QbitNamespace,
        Services,
        "qBittorrent",
        "namespace",
        Text,
        "Namespace of the qBittorrent service.",
    ),
    spec(
        Field::QbitService,
        Services,
        "qBittorrent",
        "service",
        Text,
        "Name of the qBittorrent service.",
    ),
    spec(
        Field::QbitPort,
        Services,
        "qBittorrent",
        "port",
        Number,
        "Web UI port of qBittorrent.",
    ),
    spec(
        Field::QbitUser,
        Services,
        "qBittorrent",
        "user",
        Text,
        "Web UI user name.",
    ),
    spec(
        Field::QbitPass,
        Services,
        "qBittorrent",
        "password",
        Secret,
        "Web UI password, stored in the config file.",
    ),
    spec(
        Field::QbitPoll,
        Services,
        "qBittorrent",
        "poll",
        Number,
        "Seconds between qBittorrent polls, at least 1.",
    ),
    spec(
        Field::ArgoEnabled,
        Services,
        "Argo CD",
        "enabled",
        Bool,
        "Read Applications for the deploys role.",
    ),
    spec(
        Field::ArgoNamespace,
        Services,
        "Argo CD",
        "namespace",
        Text,
        "Namespace where Argo CD runs.",
    ),
    spec(
        Field::GithubEnabled,
        Services,
        "GitHub",
        "enabled",
        Bool,
        "Fetch your contribution graph.",
    ),
    spec(
        Field::GithubToken,
        Services,
        "GitHub",
        "token",
        Secret,
        "Personal access token with read:user.",
    ),
    spec(
        Field::GithubPoll,
        Services,
        "GitHub",
        "poll",
        Number,
        "Seconds between GitHub polls, at least 60.",
    ),
    spec(
        Field::GithubCiRepos,
        Services,
        "GitHub",
        "ci repos",
        Text,
        "owner/repo, comma separated; empty: your recent repos.",
    ),
    spec(
        Field::ElecEnabled,
        Energy,
        "Electricity Maps",
        "enabled",
        Bool,
        "Power mix and renewable roles.",
    ),
    spec(
        Field::ElecZone,
        Energy,
        "Electricity Maps",
        "zone",
        Text,
        "Electricity Maps zone, for example NL.",
    ),
    spec(
        Field::ElecToken,
        Energy,
        "Electricity Maps",
        "api token",
        Secret,
        "Electricity Maps API token.",
    ),
    spec(
        Field::ElecPoll,
        Energy,
        "Electricity Maps",
        "poll",
        Number,
        "Seconds between polls, at least 60.",
    ),
    spec(
        Field::LocLat,
        Sky,
        "Location",
        "latitude",
        Number,
        "Decimal degrees, -90 to 90; blank clears.",
    ),
    spec(
        Field::LocLon,
        Sky,
        "Location",
        "longitude",
        Number,
        "Decimal degrees, -180 to 180; blank clears.",
    ),
    spec(
        Field::WeatherEnabled,
        Sky,
        "Weather",
        "enabled",
        Bool,
        "Weather, wind and air quality roles.",
    ),
    spec(
        Field::WeatherPoll,
        Sky,
        "Weather",
        "poll",
        Number,
        "Seconds between Open-Meteo polls, at least 60.",
    ),
    spec(
        Field::RainEnabled,
        Sky,
        "Rain",
        "enabled",
        Bool,
        "Buienradar nowcast; Netherlands and Belgium.",
    ),
    spec(
        Field::RainPoll,
        Sky,
        "Rain",
        "poll",
        Number,
        "Seconds between rain polls, at least 60.",
    ),
    spec(
        Field::IssEnabled,
        Sky,
        "ISS",
        "enabled",
        Bool,
        "Countdown to the next ISS pass.",
    ),
    spec(
        Field::IssMinElevation,
        Sky,
        "ISS",
        "min elevation",
        Number,
        "Lowest pass elevation to count, 0 to 90°.",
    ),
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
    spec(
        Field::Brightness,
        Display,
        "Panels",
        "brightness",
        Number,
        "0.1 to 1.0; night mode dims further.",
    ),
    spec(
        Field::Fps,
        Display,
        "Panels",
        "fps",
        Number,
        "Frames per second, 1 to 60.",
    ),
    spec(
        Field::SpiChunk,
        Display,
        "Panels",
        "spi chunk",
        Number,
        "Bytes per SPI transfer, at least 64.",
    ),
    spec(
        Field::OneAtATime,
        Display,
        "Panels",
        "one at a time",
        Bool,
        "Only one screen changes at a time.",
    ),
    spec(
        Field::NightEnabled,
        Display,
        "Night",
        "enabled",
        Bool,
        "Fade the panels to a dim moon at night.",
    ),
    spec(
        Field::NightStart,
        Display,
        "Night",
        "start",
        Text,
        "HH:MM when night begins.",
    ),
    spec(
        Field::NightEnd,
        Display,
        "Night",
        "end",
        Text,
        "HH:MM when night ends.",
    ),
    spec(
        Field::HotCpu,
        Thresholds,
        "Hot node",
        "cpu %",
        Number,
        "CPU percent that marks a node hot.",
    ),
    spec(
        Field::HotMem,
        Thresholds,
        "Hot node",
        "mem %",
        Number,
        "Memory percent that marks a node hot.",
    ),
    spec(
        Field::HotTemp,
        Thresholds,
        "Hot node",
        "temp °C",
        Number,
        "Temperature that marks a node hot.",
    ),
    spec(
        Field::ShowsEnabled,
        Shows,
        "Shows",
        "enabled",
        Bool,
        "Pixel-art shows across all screens, never at night.",
    ),
    spec(
        Field::ShowsEvery,
        Shows,
        "Shows",
        "every",
        Number,
        "Minutes between shows, 2 to 240.",
    ),
    spec(
        Field::ShowsGap,
        Shows,
        "Shows",
        "gap mm",
        Number,
        "Bezel gap between two panels in mm, 5 to 80.",
    ),
    show(0),
    show(1),
    show(2),
    show(3),
    show(4),
    show(5),
    show(6),
    show(7),
    show(8),
    show(9),
    show(10),
    spec(
        Field::ShowsPlay,
        Shows,
        "Play now",
        "show",
        Run,
        "A show id or random; plays at once, not at night.",
    ),
];

/// Indices into `FIELDS` for one group, in display order.
pub fn group_indices(g: Group) -> Vec<usize> {
    FIELDS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.group == g)
        .map(|(i, _)| i)
        .collect()
}

/// The `enabled` toggle of a section, if it has one.
pub fn section_enabled(cfg: &Config, section: &str) -> Option<bool> {
    FIELDS
        .iter()
        .find(|s| s.section == section && s.kind == FieldKind::Bool && s.label == "enabled")
        .map(|s| get(cfg, s.field) == "true")
}

/// `every 15 s`, or `every 10 min` for whole minutes.
pub fn humanise_poll(secs: u64) -> String {
    if secs >= 60 && secs.is_multiple_of(60) {
        format!("every {} min", secs / 60)
    } else {
        format!("every {secs} s")
    }
}

pub fn get(cfg: &Config, f: Field) -> String {
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig.clone(),
        Field::PromNamespace => cfg.prometheus.namespace.clone(),
        Field::PromService => cfg.prometheus.service.clone(),
        Field::PromPort => cfg.prometheus.port.to_string(),
        Field::PromPoll => cfg.prometheus.poll_secs.to_string(),
        Field::QbitEnabled => cfg.qbittorrent.enabled.to_string(),
        Field::QbitNamespace => cfg.qbittorrent.namespace.clone(),
        Field::QbitService => cfg.qbittorrent.service.clone(),
        Field::QbitPort => cfg.qbittorrent.port.to_string(),
        Field::QbitUser => cfg.qbittorrent.user.clone(),
        Field::QbitPass => cfg.qbittorrent.pass.clone(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs.to_string(),
        Field::NightEnabled => cfg.night.enabled.to_string(),
        Field::NightStart => cfg.night.start.clone(),
        Field::NightEnd => cfg.night.end.clone(),
        Field::HotCpu => format!("{}", cfg.thresholds.hot_cpu),
        Field::HotMem => format!("{}", cfg.thresholds.hot_mem),
        Field::Brightness => format!("{}", cfg.display.brightness),
        Field::Fps => cfg.display.fps.to_string(),
        Field::SpiChunk => cfg.display.spi_chunk.to_string(),
        Field::OneAtATime => cfg.display.one_at_a_time.to_string(),
        Field::ElecEnabled => cfg.electricity.enabled.to_string(),
        Field::ElecZone => cfg.electricity.zone.clone(),
        Field::ElecToken => cfg.electricity.token.clone(),
        Field::ElecPoll => cfg.electricity.poll_secs.to_string(),
        Field::HotTemp => format!("{}", cfg.thresholds.hot_temp),
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
        Field::ShowsEnabled => cfg.shows.enabled.to_string(),
        Field::ShowsEvery => cfg.shows.every_mins.to_string(),
        Field::ShowsGap => format!("{}", cfg.shows.gap_mm),
        Field::Show(i) => {
            let id = SHOWS[i as usize].id;
            cfg.shows.list.iter().any(|s| s == id).to_string()
        }
        Field::ShowsPlay => "random".into(),
    }
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    s.trim()
        .parse::<T>()
        .map_err(|_| format!("{what}: not a number"))
}

/// An optional coordinate: blank clears it, anything outside ±`limit` is rejected.
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

pub fn set(cfg: &mut Config, f: Field, text: &str) -> Result<(), String> {
    let t = text.trim();
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig = t.into(),
        Field::PromNamespace => cfg.prometheus.namespace = t.into(),
        Field::PromService => cfg.prometheus.service = t.into(),
        Field::PromPort => cfg.prometheus.port = num(t, "port")?,
        Field::PromPoll => cfg.prometheus.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::QbitEnabled => cfg.qbittorrent.enabled = t == "true",
        Field::QbitNamespace => cfg.qbittorrent.namespace = t.into(),
        Field::QbitService => cfg.qbittorrent.service = t.into(),
        Field::QbitPort => cfg.qbittorrent.port = num(t, "port")?,
        Field::QbitUser => cfg.qbittorrent.user = t.into(),
        Field::QbitPass => cfg.qbittorrent.pass = text.into(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::NightEnabled => cfg.night.enabled = t == "true",
        Field::NightStart => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.start = t.into();
        }
        Field::NightEnd => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.end = t.into();
        }
        Field::HotCpu => cfg.thresholds.hot_cpu = num(t, "cpu %")?,
        Field::HotMem => cfg.thresholds.hot_mem = num(t, "mem %")?,
        Field::Brightness => {
            let b: f32 = num(t, "brightness")?;
            if !(0.1..=1.0).contains(&b) {
                return Err("brightness must be between 0.1 and 1.0".into());
            }
            cfg.display.brightness = b;
        }
        Field::Fps => cfg.display.fps = num::<u32>(t, "fps")?.clamp(1, 60),
        Field::SpiChunk => cfg.display.spi_chunk = num::<usize>(t, "spi chunk")?.max(64),
        Field::OneAtATime => cfg.display.one_at_a_time = t == "true",
        Field::ElecEnabled => cfg.electricity.enabled = t == "true",
        Field::ElecZone => cfg.electricity.zone = t.into(),
        Field::ElecToken => cfg.electricity.token = text.into(),
        Field::ElecPoll => cfg.electricity.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::HotTemp => cfg.thresholds.hot_temp = num(t, "temp °C")?,
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
        Field::ShowsEnabled => cfg.shows.enabled = t == "true",
        Field::ShowsEvery => {
            let m: u32 = num(t, "minutes")?;
            if !(2..=240).contains(&m) {
                return Err("minutes must be between 2 and 240".into());
            }
            cfg.shows.every_mins = m;
        }
        Field::ShowsGap => {
            let g: f32 = num(t, "gap mm")?;
            if !(5.0..=80.0).contains(&g) {
                return Err("gap must be between 5 and 80 mm".into());
            }
            cfg.shows.gap_mm = g;
        }
        Field::Show(i) => {
            let id = SHOWS[i as usize].id;
            let on = t == "true";
            let list = &cfg.shows.list;
            if !on && list.iter().all(|s| s == id) {
                return Err("at least one show must stay on".into());
            }
            // Rebuilt in registry order, so turning every show back on gives the
            // default list again (which is left out of the saved file).
            cfg.shows.list = SHOWS
                .iter()
                .filter(|s| {
                    if s.id == id {
                        on
                    } else {
                        list.iter().any(|l| l == s.id)
                    }
                })
                .map(|s| s.id.to_string())
                .collect();
        }
        // Runs from the editor (`Configure::play`); nothing to store.
        Field::ShowsPlay => {}
    }
    Ok(())
}

enum Mode {
    Browse,
    Edit(String),
    AskRestart,
    /// Esc with unsaved changes: confirm before leaving.
    AskDiscard,
    /// `systemctl restart` is running on a worker thread; keys wait for its result.
    Restarting,
}

pub struct Configure {
    cfg: Config,
    row: usize,
    mode: Mode,
    error: Option<String>,
    /// Non-error status shown in the footer (e.g. the restart result).
    note: Option<String>,
    dirty: bool,
    /// The file on disk could not be loaded: `cfg` holds defaults for display only and
    /// `s` must never overwrite the user's file with them.
    unreadable: bool,
    restart_rx: Option<Receiver<String>>,
    /// Where "Play now" writes its request: `None` is the service user's state
    /// dir (as `rackscreen play`); tests point it at a temp dir.
    play_state: Option<PathBuf>,
    /// The GIFs group is open: its list pane has the focus, `row` waits.
    gifs: Option<GifPane>,
}

impl Configure {
    pub fn new(shared: &Shared) -> Configure {
        Configure::from_load(Config::load_or_default(&shared.ctx.config_path))
    }

    /// Build from the result of loading the config file.
    pub fn from_load(loaded: anyhow::Result<Config>) -> Configure {
        let (cfg, error, unreadable) = match loaded {
            Ok(cfg) => (cfg, None, false),
            Err(e) => (
                Config::default(),
                Some(format!("config unreadable: {e:#}; fix the file by hand")),
                true,
            ),
        };
        Configure {
            cfg,
            row: 0,
            mode: Mode::Browse,
            error,
            note: None,
            dirty: false,
            unreadable,
            restart_rx: None,
            play_state: None,
            gifs: None,
        }
    }

    pub fn group(&self) -> Group {
        if self.gifs.is_some() {
            return Group::Gifs;
        }
        FIELDS[self.row].group
    }

    /// `↑↓` flow straight through group boundaries: `FIELDS` is contiguous per group,
    /// so the next row after a group's last field is the next group's first.
    fn move_row(&mut self, delta: i32) {
        let n = FIELDS.len() as i32;
        self.row = (self.row as i32 + delta).rem_euclid(n) as usize;
    }

    fn move_group(&mut self, delta: i32) {
        let n = Group::ALL.len() as i32;
        let g = Group::ALL[(self.group().index() as i32 + delta).rem_euclid(n) as usize];
        self.jump_to(g);
    }

    fn jump_to(&mut self, g: Group) {
        if g == Group::Gifs {
            self.gifs.get_or_insert_with(GifPane::default);
            return;
        }
        self.gifs = None;
        self.row = group_indices(g)[0];
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if self.unreadable {
            self.error =
                Some("not saved: the existing config is unreadable; fix the file by hand".into());
            return Action::None;
        }
        if let Err(e) = self.cfg.validate() {
            self.error = Some(format!("{e:#}"));
            return Action::None;
        }
        match save_config(&self.cfg, &shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                shared.banner = Some("config saved".into());
                if shared.ctx.sim {
                    return Action::Back;
                }
                // Ask systemd now rather than trusting a cached value: on a fresh run
                // nothing has populated `shared.service_active` yet.
                let sh = RealShell;
                let active = Systemd::new(&sh, &service_user())
                    .is_active()
                    .unwrap_or(false);
                shared.service_active = Some(active);
                if active {
                    self.mode = Mode::AskRestart;
                    Action::None
                } else {
                    Action::Back
                }
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                Action::None
            }
        }
    }

    /// Write a play-now request, as `rackscreen play <arg>` does.
    fn play(&mut self, arg: &str) -> Result<(), String> {
        let res = match &self.play_state {
            Some(dir) => play::write_in(dir, arg).map(|p| (p, None)),
            None => play::write_for_service_user(arg),
        };
        let (_, warning) = res.map_err(|e| format!("{e:#}"))?;
        self.error = None;
        self.note = Some(match warning {
            Some(w) => format!("requested {}; {w}", arg.trim()),
            None => format!("requested {}", arg.trim()),
        });
        Ok(())
    }

    fn spawn_restart(&mut self) {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("restart".into())
            .spawn(move || {
                let sh = RealShell;
                let msg = match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "config saved; service restarted".to_string(),
                    Err(e) => format!("config saved; restart failed: {e:#}"),
                };
                let _ = tx.send(msg);
            })
            .expect("spawn restart");
        self.restart_rx = Some(rx);
        self.mode = Mode::Restarting;
    }

    fn draw_dialogs(&self, f: &mut Frame, area: Rect, th: &crate::theme::Theme) {
        if matches!(self.mode, Mode::AskRestart) {
            confirm_dialog(
                f,
                area,
                th,
                "Restart service?",
                &["Apply the new config to the running service now?".to_string()],
                "y/⏎ restart   Esc later",
                false,
            );
        }
        if matches!(self.mode, Mode::AskDiscard) {
            confirm_dialog(
                f,
                area,
                th,
                "Discard changes?",
                &["You have unsaved changes. Leave without saving?".to_string()],
                "y/⏎ discard   Esc keep editing",
                true,
            );
        }
    }
}

impl Screen for Configure {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let (field, kind) = (FIELDS[self.row].field, FIELDS[self.row].kind);
        if let (Some(pane), Mode::Browse) = (&mut self.gifs, &self.mode) {
            let busy = pane.busy();
            let r = pane.handle(key.code, &mut self.cfg);
            if r.consumed {
                self.error = r.error;
                self.dirty |= r.changed;
                return Action::None;
            }
            if busy {
                return Action::None;
            }
        }
        // Take the mode out so the arms can replace it without a live borrow.
        let mode = std::mem::replace(&mut self.mode, Mode::Browse);
        match mode {
            Mode::Browse => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_row(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_row(1),
                KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.move_group(-1),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.move_group(1),
                KeyCode::Char(c @ '1'..='9') if ((c as u8 - b'1') as usize) < Group::ALL.len() => {
                    self.jump_to(Group::ALL[(c as u8 - b'1') as usize]);
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if kind == FieldKind::Bool {
                        let cur = get(&self.cfg, field) == "true";
                        match set(&mut self.cfg, field, if cur { "false" } else { "true" }) {
                            Ok(()) => {
                                self.dirty = true;
                                if !self.unreadable {
                                    self.error = None;
                                }
                            }
                            Err(e) => self.error = Some(e),
                        }
                    } else {
                        self.mode = Mode::Edit(get(&self.cfg, field));
                    }
                }
                KeyCode::Char('s') => return self.save(shared),
                KeyCode::Esc | KeyCode::Char('q') => {
                    if self.dirty {
                        self.mode = Mode::AskDiscard;
                    } else {
                        return Action::Back;
                    }
                }
                _ => {}
            },
            Mode::Edit(buf) if kind == FieldKind::Run && key.code == KeyCode::Enter => {
                if let Err(e) = self.play(&buf) {
                    self.error = Some(e);
                    self.mode = Mode::Edit(buf);
                }
            }
            Mode::Edit(mut buf) => match key.code {
                KeyCode::Enter => match set(&mut self.cfg, field, &buf) {
                    Ok(()) => {
                        self.error = None;
                        self.dirty = true;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.mode = Mode::Edit(buf);
                    }
                },
                KeyCode::Esc => self.error = None,
                KeyCode::Backspace => {
                    buf.pop();
                    self.mode = Mode::Edit(buf);
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.mode = Mode::Edit(buf);
                }
                _ => self.mode = Mode::Edit(buf),
            },
            Mode::AskRestart => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                    self.spawn_restart();
                    return Action::None;
                }
                return Action::Back;
            }
            Mode::AskDiscard => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                    return Action::Back;
                }
                // any other key keeps the changes and returns to browsing
            }
            Mode::Restarting => {
                // Leaving early is allowed; the restart finishes on its own.
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    return Action::Back;
                }
                self.mode = Mode::Restarting;
            }
        }
        Action::None
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        let Some(rx) = &self.restart_rx else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(TryRecvError::Disconnected) => "restart thread died".to_string(),
            Err(TryRecvError::Empty) => return,
        };
        self.restart_rx = None;
        shared.banner = Some(msg.clone());
        self.note = Some(msg);
        if matches!(self.mode, Mode::Restarting) {
            self.mode = Mode::Browse;
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(area);
        if let Some(pane) = &self.gifs {
            pane.draw(f, list, area, th, &self.cfg);
            match (&self.error, &self.note) {
                (Some(e), _) => f.render_widget(
                    Paragraph::new(Line::from(Span::styled(format!("  {e}"), th.bad()))),
                    help,
                ),
                (None, Some(n)) if !pane.editing() => f.render_widget(
                    Paragraph::new(Line::from(Span::styled(format!("  {n}"), th.good()))),
                    help,
                ),
                _ => help_line(f, help, th, pane.help()),
            }
            self.draw_dialogs(f, area, th);
            return;
        }

        enum Row {
            Section(&'static str, Option<bool>),
            Field(usize),
            Blank,
        }
        let mut rows: Vec<Row> = Vec::new();
        let mut section = "";
        for i in group_indices(self.group()) {
            let s = &FIELDS[i];
            if s.section != section {
                if !rows.is_empty() {
                    rows.push(Row::Blank);
                }
                rows.push(Row::Section(
                    s.section,
                    section_enabled(&self.cfg, s.section),
                ));
                section = s.section;
            }
            rows.push(Row::Field(i));
        }
        let focus_pos = rows
            .iter()
            .position(|r| matches!(r, Row::Field(i) if *i == self.row))
            .unwrap_or(0);
        let visible = list.height as usize;
        let scroll = if focus_pos >= visible {
            focus_pos + 1 - visible
        } else {
            0
        };
        let width = list.width as usize;
        let mut lines = Vec::new();
        for row in rows.iter().skip(scroll).take(visible) {
            match row {
                Row::Blank => lines.push(Line::from("")),
                Row::Section(name, enabled) => {
                    let mut spans = vec![Span::styled(format!("  {name}"), th.normal())];
                    if let Some(on) = enabled {
                        let badge = if *on {
                            format!("{} on", g.dot)
                        } else {
                            format!("{} off", g.pending)
                        };
                        let used = 2 + name.chars().count();
                        let bw = badge.chars().count() + 2;
                        if used + bw <= width {
                            spans.push(Span::raw(" ".repeat(width - used - bw)));
                        }
                        spans.push(Span::styled(
                            badge,
                            if *on { th.good() } else { th.muted() },
                        ));
                    }
                    lines.push(Line::from(spans));
                }
                Row::Field(i) => {
                    let s = &FIELDS[*i];
                    let selected = *i == self.row;
                    let raw = get(&self.cfg, s.field);
                    // Read before `shown`'s match, which consumes `raw` in some arms.
                    let on = raw == "true";
                    let off = section_enabled(&self.cfg, s.section) == Some(false)
                        && s.label != "enabled";
                    let shown = match (&self.mode, selected, s.kind) {
                        (Mode::Edit(buf), true, FieldKind::Secret) => {
                            format!("{}_", "•".repeat(buf.chars().count()))
                        }
                        (Mode::Edit(buf), true, _) => format!("{buf}_"),
                        (_, _, FieldKind::Secret) => "•".repeat(raw.chars().count()),
                        (_, _, FieldKind::Bool) => {
                            if raw == "true" {
                                format!("{} on", g.done)
                            } else {
                                format!("{} off", g.pending)
                            }
                        }
                        (_, _, FieldKind::Number) if s.label == "poll" => {
                            raw.parse::<u64>().map(humanise_poll).unwrap_or(raw)
                        }
                        (_, _, _) if s.field == Field::ShowsEvery => format!("every {raw} min"),
                        (_, _, FieldKind::Run) => format!("⏎ {raw} or a show id"),
                        _ => raw,
                    };
                    let editing = matches!(self.mode, Mode::Edit(_)) && selected;
                    let label_style = if selected {
                        th.selected()
                    } else if off {
                        th.faint_style()
                    } else {
                        th.normal()
                    };
                    let value_style = if editing {
                        th.title()
                    } else if s.kind == FieldKind::Bool && on && !off {
                        th.good()
                    } else if off {
                        th.faint_style()
                    } else {
                        th.muted()
                    };
                    let pointer = if selected { g.pointer } else { " " };
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(format!("{pointer} "), th.selected()),
                        Span::styled(format!("{:<16}", s.label), label_style),
                        Span::styled(shown, value_style),
                    ]));
                }
            }
        }
        f.render_widget(Paragraph::new(lines), list);
        // The highlight spans the whole list width, not just the drawn glyphs.
        let focus_row = (focus_pos - scroll) as u16;
        if focus_row < list.height {
            f.buffer_mut().set_style(
                Rect {
                    y: list.y + focus_row,
                    height: 1,
                    ..list
                },
                th.highlighted(),
            );
        }

        match (&self.mode, &self.error, &self.note) {
            (Mode::Restarting, _, _) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  restarting service...",
                    th.warning(),
                ))),
                help,
            ),
            (_, Some(e), _) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("  {e}"), th.bad()))),
                help,
            ),
            (_, None, Some(n)) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("  {n}"), th.good()))),
                help,
            ),
            (_, None, None) => match FIELDS[self.row].field {
                Field::Show(i) => help_line(
                    f,
                    help,
                    th,
                    &format!("{}: {}", SHOWS[i as usize].name, FIELDS[self.row].help),
                ),
                _ => help_line(f, help, th, FIELDS[self.row].help),
            },
        }
        self.draw_dialogs(f, area, th);
    }

    fn keys(&self) -> String {
        if let (Some(pane), Mode::Browse) = (&self.gifs, &self.mode) {
            return pane.keys();
        }
        match self.mode {
            Mode::Browse => "↑↓ field  Tab/←→ group  1-9 jump  ⏎ edit  s save  Esc back".into(),
            Mode::Edit(_) => "type  ⏎ apply  Esc cancel".into(),
            Mode::AskRestart => "y restart  Esc later".into(),
            Mode::AskDiscard => "y discard  Esc keep".into(),
            Mode::Restarting => "restarting...  Esc back".into(),
        }
    }
    fn subtitle(&self) -> String {
        // The group is named here too, so narrow terminals without the sidebar show it.
        format!("Configure › {}", self.group().name())
    }
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        self.dirty
            .then(|| ("unsaved".to_string(), StatusTone::Warn))
    }
    fn consumes_left(&self) -> bool {
        true
    }
    fn sidebar(&self) -> Option<SidebarView> {
        Some(SidebarView {
            title: "Configure".into(),
            // Numbered, so the `1-9` jump keys explain themselves.
            items: Group::ALL
                .iter()
                .enumerate()
                .map(|(i, g)| format!("{} {}", i + 1, g.name()))
                .collect(),
            selected: self.group().index(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use rackscreen_app::logs::LogSink;
    use rackscreen_core::shows::SHOWS;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::Path;

    fn shared_at(path: &Path) -> Shared {
        Shared {
            ctx: Ctx {
                config_path: path.to_path_buf(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        }
    }

    #[test]
    fn unreadable_config_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = shared_at(&path);
        let mut screen = Configure::from_load(Err(anyhow::anyhow!("parse config.yaml: bad")));
        assert!(screen.error.as_deref().unwrap().contains("unreadable"));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::None
        ));
        assert!(!path.exists(), "defaults must not replace the user's file");
        assert!(screen.error.as_deref().unwrap().contains("not saved"));
        assert!(sh.banner.is_none());
    }

    #[test]
    fn readable_config_saves_and_goes_back_in_sim() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = shared_at(&path);
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::Back
        ));
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
        assert_eq!(sh.banner.as_deref(), Some("config saved"));
    }

    #[test]
    fn restart_result_reaches_banner_on_tick() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        let (tx, rx) = mpsc::channel();
        screen.restart_rx = Some(rx);
        screen.mode = Mode::Restarting;
        screen.tick(&mut sh, 0.0);
        assert!(sh.banner.is_none(), "nothing yet");
        tx.send("config saved; service restarted".to_string())
            .unwrap();
        screen.tick(&mut sh, 0.1);
        assert_eq!(
            sh.banner.as_deref(),
            Some("config saved; service restarted")
        );
        assert!(matches!(screen.mode, Mode::Browse));
        assert!(screen.restart_rx.is_none());
    }

    #[test]
    fn get_set_round_trip_and_validation() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::PromPort), "9090");
        set(&mut c, Field::PromPort, " 9091 ").unwrap();
        assert_eq!(c.prometheus.port, 9091);
        assert!(set(&mut c, Field::PromPort, "x")
            .unwrap_err()
            .contains("not a number"));
        assert!(set(&mut c, Field::NightStart, "25:00").is_err());
        set(&mut c, Field::NightStart, "22:30").unwrap();
        assert_eq!(c.night.start, "22:30");
        assert!(set(&mut c, Field::Brightness, "1.5").is_err());
        set(&mut c, Field::Brightness, "0.7").unwrap();
        set(&mut c, Field::QbitEnabled, "false").unwrap();
        assert!(!c.qbittorrent.enabled);
        set(&mut c, Field::Fps, "500").unwrap();
        assert_eq!(c.display.fps, 60);
        assert_eq!(get(&c, Field::OneAtATime), "true");
        set(&mut c, Field::OneAtATime, "false").unwrap();
        assert!(!c.display.one_at_a_time);
        set(&mut c, Field::OneAtATime, "true").unwrap();
        assert!(c.display.one_at_a_time);
        for s in FIELDS.iter() {
            let v = get(&c, s.field);
            assert!(
                set(&mut c, s.field, &v).is_ok(),
                "{:?} round trip with {v:?}",
                s.field
            );
        }
    }

    #[test]
    fn electricity_and_temp_fields() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::ElecZone), "NL");
        assert_eq!(get(&c, Field::HotTemp), "70");
        set(&mut c, Field::ElecEnabled, "true").unwrap();
        assert!(c.electricity.enabled);
        set(&mut c, Field::ElecZone, " DE ").unwrap();
        assert_eq!(c.electricity.zone, "DE");
        set(&mut c, Field::ElecToken, "tok").unwrap();
        assert_eq!(c.electricity.token, "tok");
        set(&mut c, Field::ElecPoll, "10").unwrap();
        assert_eq!(c.electricity.poll_secs, 60, "poll secs are floored at 60");
        set(&mut c, Field::HotTemp, "82.5").unwrap();
        assert_eq!(c.thresholds.hot_temp, 82.5);
        assert!(set(&mut c, Field::HotTemp, "warm").is_err());
        assert!(c.validate().is_ok());
    }

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
        assert_eq!(FIELDS.len(), 70);
        assert!(FIELDS
            .iter()
            .any(|s| s.field == Field::GithubToken && s.kind == FieldKind::Secret));
    }

    #[test]
    fn every_field_is_in_exactly_one_group_in_order() {
        assert_eq!(FIELDS.len(), 70);
        let mut seen: Vec<Field> = Vec::new();
        for s in FIELDS.iter() {
            assert!(!seen.contains(&s.field), "{:?} listed twice", s.field);
            seen.push(s.field);
            assert!(!s.help.is_empty(), "{:?} has no help", s.field);
            assert!(s.help.chars().count() <= 60, "{:?} help too long", s.field);
        }
        // groups are contiguous in FIELDS, in Group::ALL order
        let mut last = 0usize;
        for s in FIELDS.iter() {
            assert!(s.group.index() >= last, "{:?} out of group order", s.field);
            last = s.group.index();
        }
        for g in Group::ALL {
            // GIFs is a list, not form fields
            assert_eq!(group_indices(g).is_empty(), g == Group::Gifs, "{g:?}");
        }
        assert_eq!(group_indices(Group::Cluster), vec![0, 1, 2, 3, 4]);
        assert_eq!(group_indices(Group::Thresholds).len(), 3);
    }

    #[test]
    fn section_enabled_reads_the_bool_of_that_section() {
        let mut c = Config::default();
        assert_eq!(
            section_enabled(&c, "Prometheus"),
            None,
            "always on, no toggle"
        );
        assert_eq!(section_enabled(&c, "Weather"), Some(false));
        c.weather.enabled = true;
        assert_eq!(section_enabled(&c, "Weather"), Some(true));
        assert_eq!(section_enabled(&c, "Nope"), None);
    }

    #[test]
    fn poll_is_humanised() {
        assert_eq!(humanise_poll(15), "every 15 s");
        assert_eq!(humanise_poll(60), "every 1 min");
        assert_eq!(humanise_poll(600), "every 10 min");
        assert_eq!(humanise_poll(90), "every 90 s");
    }

    #[test]
    fn up_down_flow_across_groups_and_left_right_switch_groups() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert_eq!(screen.group(), Group::Cluster);
        screen.handle(KeyEvent::from(KeyCode::Up), &mut sh, 0.0);
        assert_eq!(
            screen.group(),
            Group::Shows,
            "up from the first field wraps to the end"
        );
        assert_eq!(FIELDS[screen.row].field, Field::ShowsPlay);
        screen.handle(KeyEvent::from(KeyCode::Down), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Cluster);
        for _ in 0..5 {
            screen.handle(KeyEvent::from(KeyCode::Down), &mut sh, 0.0);
        }
        assert_eq!(
            screen.group(),
            Group::Services,
            "down past the last field enters the next group"
        );
        assert_eq!(FIELDS[screen.row].field, Field::QbitEnabled);
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Cluster);
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Services);
        assert_eq!(
            FIELDS[screen.row].field,
            Field::QbitEnabled,
            "first field of the group"
        );
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Gifs, "groups wrap");
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Shows);
        assert!(screen.consumes_left());
        assert_eq!(screen.sidebar().unwrap().selected, Group::Shows.index());
        assert_eq!(screen.sidebar().unwrap().items.len(), 9);
    }

    #[test]
    fn gifs_group_adds_shows_and_saves_a_gif() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = shared_at(&path);
        let mut screen = Configure::from_load(Ok(Config::default()));
        screen.handle(KeyEvent::from(KeyCode::Char('9')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Gifs);
        assert_eq!(screen.subtitle(), "Configure › GIFs");
        assert!(screen.keys().contains("a add"), "{}", screen.keys());
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        for c in "party".chars() {
            screen.handle(KeyEvent::from(KeyCode::Char(c)), &mut sh, 0.0);
        }
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        // `s` and digits are typed into the link, not save or jump
        for c in "https://giphy.com/gifs/s1-AbCdEf123".chars() {
            screen.handle(KeyEvent::from(KeyCode::Char(c)), &mut sh, 0.0);
        }
        assert_eq!(screen.group(), Group::Gifs);
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert!(screen.dirty);
        assert_eq!(screen.cfg.gifs[0].name, "party");
        let mut term = Terminal::new(TestBackend::new(90, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(
            t.contains("party") && t.contains("giphy.com/gifs/s1-AbCdEf123"),
            "{t}"
        );
        assert!(t.contains("+ add GIF") && t.contains("unused"), "{t}");
        assert!(t.contains("35 MB"), "the memory note: {t}");
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Cluster, "groups wrap past GIFs");
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Gifs);
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::Back
        ));
        let saved = Config::load_or_default(&path).unwrap();
        assert_eq!(saved.gifs.len(), 1);
        assert_eq!(saved.gifs[0].url, "https://giphy.com/gifs/s1-AbCdEf123");
    }

    #[test]
    fn tab_and_digits_switch_groups() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        screen.handle(KeyEvent::from(KeyCode::Tab), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Services);
        screen.handle(KeyEvent::from(KeyCode::Tab), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Energy);
        screen.handle(KeyEvent::from(KeyCode::BackTab), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Services);
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
        screen.handle(KeyEvent::from(KeyCode::Char('8')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Shows);
        assert!(!screen.dirty, "navigation never dirties the form");
        assert!(screen.keys().contains("1-9 jump"));
    }

    #[test]
    fn esc_with_changes_asks_before_discarding() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::Back
        ));
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        screen.handle(KeyEvent::from(KeyCode::Char(' ')), &mut sh, 0.0);
        assert!(screen.dirty);
        assert_eq!(screen.status(&sh).unwrap().0, "unsaved");
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::None
        ));
        assert!(matches!(screen.mode, Mode::AskDiscard));
        // anything but y/Enter keeps editing
        screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0);
        assert!(matches!(screen.mode, Mode::Browse));
        screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0);
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('y')), &mut sh, 0.0),
            Action::Back
        ));
    }

    #[test]
    fn pane_shows_sections_badges_help_and_humanised_poll() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        let mut term = Terminal::new(TestBackend::new(70, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("Kubernetes"), "{t}");
        assert!(t.contains("Prometheus"));
        assert!(
            t.contains("every 5 s"),
            "default Prometheus poll, humanised: {t}"
        );
        assert!(
            t.contains("ⓘ Path to the kubeconfig"),
            "help for the focused field: {t}"
        );
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("qBittorrent"));
        assert!(t.contains("● on"), "qBittorrent is on by default: {t}");
        assert!(t.contains("○ off"), "GitHub is off by default: {t}");
        assert!(!t.contains("Kubernetes"), "other groups are not drawn");
    }
    #[test]
    fn media_and_jellyfin_fields() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::MediaEnabled), "false");
        assert_eq!(get(&c, Field::RadarrPort), "7878");
        assert_eq!(get(&c, Field::LidarrService), "auto");
        assert_eq!(get(&c, Field::JellyfinPoll), "5");
        assert_eq!(get(&c, Field::GithubCiRepos), "");
        set(&mut c, Field::GithubCiRepos, " a/b, c/d ,,").unwrap();
        assert_eq!(
            c.github.ci_repos,
            vec!["a/b".to_string(), "c/d".to_string()]
        );
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
        for f in [
            Field::RadarrKey,
            Field::SonarrKey,
            Field::LidarrKey,
            Field::JellyfinKey,
        ] {
            assert!(
                FIELDS
                    .iter()
                    .any(|s| s.field == f && s.kind == FieldKind::Secret),
                "{f:?} is masked"
            );
        }
        assert_eq!(Group::Media.name(), "Media & Jellyfin");
        assert_eq!(Group::ALL[4], Group::Media);
        assert_eq!(group_indices(Group::Media).len(), 15);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn shows_fields_round_trip_and_validation() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::ShowsEnabled), "true");
        assert_eq!(get(&c, Field::ShowsEvery), "15");
        assert_eq!(get(&c, Field::ShowsGap), "25");
        set(&mut c, Field::ShowsEnabled, "false").unwrap();
        assert!(!c.shows.enabled);
        set(&mut c, Field::ShowsEvery, " 30 ").unwrap();
        assert_eq!(c.shows.every_mins, 30);
        assert!(set(&mut c, Field::ShowsEvery, "1")
            .unwrap_err()
            .contains("2"));
        assert!(set(&mut c, Field::ShowsEvery, "241").is_err());
        set(&mut c, Field::ShowsEvery, "240").unwrap();
        assert!(set(&mut c, Field::ShowsEvery, "soon").is_err());
        set(&mut c, Field::ShowsGap, "12.5").unwrap();
        assert_eq!(c.shows.gap_mm, 12.5);
        assert!(set(&mut c, Field::ShowsGap, "4").unwrap_err().contains("5"));
        assert!(set(&mut c, Field::ShowsGap, "81").is_err());
        assert_eq!(group_indices(Group::Shows).len(), 4 + SHOWS.len());
        assert_eq!(Group::ALL[7], Group::Shows);
        // the checklist labels are the ids, in registry order
        let labels: Vec<&str> = group_indices(Group::Shows)
            .into_iter()
            .filter(|&i| matches!(FIELDS[i].field, Field::Show(_)))
            .map(|i| FIELDS[i].label)
            .collect();
        let ids: Vec<&str> = SHOWS.iter().map(|s| s.id).collect();
        assert_eq!(labels, ids);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn show_checklist_keeps_at_least_one_on() {
        let mut c = Config::default();
        let all = c.shows.list.clone();
        for i in 0..SHOWS.len() {
            assert_eq!(get(&c, Field::Show(i as u8)), "true");
        }
        set(&mut c, Field::Show(1), "false").unwrap();
        assert!(!c.shows.list.contains(&"deep-sea".to_string()));
        assert_eq!(get(&c, Field::Show(1)), "false");
        set(&mut c, Field::Show(1), "true").unwrap();
        assert_eq!(c.shows.list, all, "back in registry order");
        for i in 1..SHOWS.len() {
            set(&mut c, Field::Show(i as u8), "false").unwrap();
        }
        assert_eq!(c.shows.list, vec!["freight".to_string()]);
        let e = set(&mut c, Field::Show(0), "false").unwrap_err();
        assert!(e.contains("at least one"), "{e}");
        assert_eq!(c.shows.list, vec!["freight".to_string()]);
        set(&mut c, Field::Show(0), "true").unwrap();
        assert_eq!(c.shows.list, vec!["freight".to_string()], "no duplicates");
    }

    fn row_of(f: Field) -> usize {
        FIELDS.iter().position(|s| s.field == f).unwrap()
    }

    #[test]
    fn toggling_the_last_show_off_is_refused_with_a_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut cfg = Config::default();
        cfg.shows.list = vec!["storm".into()];
        let mut screen = Configure::from_load(Ok(cfg));
        screen.handle(KeyEvent::from(KeyCode::Char('8')), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Shows);
        assert_eq!(FIELDS[screen.row].field, Field::ShowsEnabled);
        screen.row = row_of(Field::Show(3));
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert_eq!(screen.cfg.shows.list, vec!["storm".to_string()]);
        assert!(screen.error.as_deref().unwrap().contains("at least one"));
        assert!(!screen.dirty, "a refused toggle changes nothing");
        screen.row = row_of(Field::Show(0));
        screen.handle(KeyEvent::from(KeyCode::Char(' ')), &mut sh, 0.0);
        assert_eq!(
            screen.cfg.shows.list,
            vec!["freight".to_string(), "storm".to_string()]
        );
        assert!(screen.dirty);
        assert!(screen.error.is_none(), "a good toggle clears the message");
        let mut term = Terminal::new(TestBackend::new(70, 24)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("every 15 min"), "{t}");
        assert!(t.contains("desert-night"), "{t}");
        assert!(t.contains("ⓘ Dusk freight"), "help names the show: {t}");
    }

    #[test]
    fn play_now_writes_the_request_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        screen.play_state = Some(dir.path().join("state"));
        screen.row = row_of(Field::ShowsPlay);
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert!(matches!(&screen.mode, Mode::Edit(b) if b == "random"));
        for _ in 0..6 {
            screen.handle(KeyEvent::from(KeyCode::Backspace), &mut sh, 0.0);
        }
        for c in "bogus".chars() {
            screen.handle(KeyEvent::from(KeyCode::Char(c)), &mut sh, 0.0);
        }
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert!(screen.error.as_deref().unwrap().contains("unknown show"));
        assert!(matches!(screen.mode, Mode::Edit(_)), "stays in the editor");
        let req = rackscreen_app::play::request_path(&dir.path().join("state"));
        assert!(!req.exists());
        for _ in 0..5 {
            screen.handle(KeyEvent::from(KeyCode::Backspace), &mut sh, 0.0);
        }
        for c in "storm".chars() {
            screen.handle(KeyEvent::from(KeyCode::Char(c)), &mut sh, 0.0);
        }
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert!(matches!(screen.mode, Mode::Browse));
        assert!(screen.error.is_none());
        assert_eq!(screen.note.as_deref(), Some("requested storm"));
        assert_eq!(std::fs::read_to_string(&req).unwrap(), "storm\n");
        assert!(!screen.dirty, "playing is not a config change");
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
        assert_eq!(screen.note.as_deref(), Some("requested random"));
        assert_eq!(std::fs::read_to_string(&req).unwrap(), "random\n");
    }
}

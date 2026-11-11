//! YAML configuration with defaults matching config.example.yaml.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rackscreen_core::anim::Secs;
use rackscreen_core::screens::Entry;
use rackscreen_core::theme::Role;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub k8s: K8sCfg,
    #[serde(default)]
    pub prometheus: PromCfg,
    #[serde(default)]
    pub qbittorrent: QbitCfg,
    #[serde(default)]
    pub night: NightCfg,
    #[serde(default)]
    pub thresholds: ThresholdsCfg,
    #[serde(default)]
    pub display: DisplayCfg,
    #[serde(default)]
    pub electricity: ElectricityCfg,
    #[serde(default)]
    pub location: LocationCfg,
    #[serde(default)]
    pub weather: WeatherCfg,
    #[serde(default)]
    pub rain: RainCfg,
    #[serde(default)]
    pub iss: IssCfg,
    #[serde(default, skip_serializing_if = "SenCfg::is_default")]
    pub sen: SenCfg,
    #[serde(default)]
    pub github: GithubCfg,
    #[serde(default)]
    pub argocd: ArgocdCfg,
    #[serde(default)]
    pub media: MediaCfg,
    #[serde(default)]
    pub jellyfin: JellyfinCfg,
    #[serde(default)]
    pub spotify: SpotifyCfg,
    /// A Bambu Lab printer on the LAN for the `printer` role.
    #[serde(default)]
    pub bambu: BambuCfg,
    /// Home Assistant entities a screen can show as `ha:<name>`.
    #[serde(default)]
    pub homeassistant: HomeAssistantCfg,
    #[serde(default)]
    pub shows: ShowsCfg,
    /// How one role gives way to the next, and the roles to a show.
    #[serde(default)]
    pub transitions: TransitionsCfg,
    /// The web UI served by the running service.
    #[serde(default)]
    pub web: WebCfg,
    /// GIPHY/Tenor/`.gif` links a screen can show as `gif:<name>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gifs: Vec<GifCfg>,
    /// API keys for the GIF search on the web UI's GIFs page.
    #[serde(default)]
    pub gif_search: GifSearchCfg,
    #[serde(default)]
    pub screens: Vec<ScreenCfg>,
}

/// One pasted GIF link, shown by a screen role `gif:<name>`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GifCfg {
    /// Lowercase letters, digits and `-`; unique.
    pub name: String,
    /// A GIPHY or Tenor page or media link, or any https link ending in `.gif`.
    pub url: String,
}

/// GIPHY and Tenor keys for searching GIFs in the web UI. Only the web
/// server uses them; the panels never search.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct GifSearchCfg {
    /// From developers.giphy.com (free); empty: no GIPHY search.
    pub giphy_key: String,
    /// A Google Cloud key with the Tenor API enabled; empty: no Tenor search.
    pub tenor_key: String,
    /// The most a result may be rated: g, pg, pg-13 or r.
    pub rating: String,
}

impl Default for GifSearchCfg {
    fn default() -> Self {
        Self {
            giphy_key: String::new(),
            tenor_key: String::new(),
            rating: "g".into(),
        }
    }
}

/// Ratings `gif_search.rating` accepts, mildest first.
pub const GIF_RATINGS: [&str; 4] = ["g", "pg", "pg-13", "r"];

/// Four-screen pixel shows.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ShowsCfg {
    pub enabled: bool,
    /// Minutes between the end of one show and the start of the next; >= 2.
    pub every_mins: u32,
    /// Bezel gap between two panels, in mm.
    pub gap_mm: f32,
    /// Show ids to pick from. Left out of the file while it is every show, so
    /// shows added in a later release join an existing config.
    #[serde(skip_serializing_if = "is_every_show")]
    pub list: Vec<String>,
    /// Config index of each screen from the left; default config order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<usize>>,
}

fn every_show() -> Vec<String> {
    rackscreen_core::shows::SHOWS
        .iter()
        .map(|s| s.id.to_string())
        .collect()
}

fn is_every_show(list: &[String]) -> bool {
    list == every_show().as_slice()
}

impl Default for ShowsCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            every_mins: 15,
            gap_mm: 25.0,
            list: every_show(),
            order: None,
        }
    }
}

impl ShowsCfg {
    /// The listed shows; unknown ids (rejected by `validate`) are skipped.
    pub fn defs(&self) -> Vec<&'static rackscreen_core::shows::ShowDef> {
        self.list
            .iter()
            .filter_map(|id| rackscreen_core::shows::by_id(id))
            .collect()
    }

    /// Left-to-right config indices for `screens` screens.
    pub fn screen_order(&self, screens: usize) -> Vec<usize> {
        match &self.order {
            Some(o) if o.len() == screens => o.clone(),
            _ => (0..screens).collect(),
        }
    }
}

/// The transition between roles, and (optionally its own) around shows.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct TransitionsCfg {
    /// A style name: fade-black, crossfade, cut, slide, cover, wipe, iris,
    /// clock, dissolve, zoom or blinds.
    pub style: String,
    /// The style's direction; empty is the style's default, and styles
    /// without one ignore it.
    pub direction: String,
    /// Length in seconds, 0.15 to 2.
    pub secs: f64,
    /// A random style (and direction) for every transition.
    pub random: bool,
    /// The shows' own transition; `None` = the same as the screens'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shows: Option<ShowTransitionCfg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ShowTransitionCfg {
    pub style: String,
    pub direction: String,
    pub secs: f64,
}

impl Default for TransitionsCfg {
    fn default() -> Self {
        let spec = rackscreen_core::transition::Spec::default();
        Self {
            style: spec.kind.style.name().to_string(),
            direction: String::new(),
            secs: spec.secs,
            random: false,
            shows: None,
        }
    }
}

impl Default for ShowTransitionCfg {
    fn default() -> Self {
        let t = TransitionsCfg::default();
        Self {
            style: t.style,
            direction: t.direction,
            secs: t.secs,
        }
    }
}

/// `style`, `direction` and `secs` as a core spec, or why not.
fn transition_spec(
    what: &str,
    style: &str,
    direction: &str,
    secs: f64,
) -> Result<rackscreen_core::transition::Spec> {
    use rackscreen_core::transition::{Direction, Kind, Style, MAX_SECS, MIN_SECS};
    let st = Style::parse(style).ok_or_else(|| {
        let known: Vec<&str> = Style::ALL.iter().map(|s| s.name()).collect();
        anyhow::anyhow!(
            "{what}.style: unknown transition '{style}' (known: {})",
            known.join(", ")
        )
    })?;
    let dir = if st.directions().is_empty() || direction.is_empty() {
        st.default_direction()
    } else {
        Direction::parse(direction)
            .ok_or_else(|| anyhow::anyhow!("{what}.direction: unknown direction '{direction}'"))?
    };
    let kind = Kind::new(st, dir).map_err(|e| anyhow::anyhow!("{what}.direction: {e}"))?;
    anyhow::ensure!(
        secs.is_finite() && (MIN_SECS..=MAX_SECS).contains(&secs),
        "{what}.secs must be between {MIN_SECS} and {MAX_SECS} (got {secs})"
    );
    Ok(rackscreen_core::transition::Spec { kind, secs })
}

impl TransitionsCfg {
    /// The core settings; errors are what `validate` reports.
    pub fn to_core(&self) -> Result<rackscreen_core::transition::Transitions> {
        let screens = transition_spec("transitions", &self.style, &self.direction, self.secs)?;
        let shows = match &self.shows {
            Some(s) => Some(transition_spec(
                "transitions.shows",
                &s.style,
                &s.direction,
                s.secs,
            )?),
            None => None,
        };
        Ok(rackscreen_core::transition::Transitions {
            screens,
            shows,
            random: self.random,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct K8sCfg {
    pub kubeconfig: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PromCfg {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub poll_secs: u64,
    pub ignore_alerts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct QbitCfg {
    pub enabled: bool,
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub poll_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct NightCfg {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ThresholdsCfg {
    pub hot_cpu: f32,
    pub hot_mem: f32,
    pub hot_temp: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct DisplayCfg {
    pub brightness: f32,
    pub fps: u32,
    pub spi_chunk: usize,
    /// Let only one screen change to its next role at a time, in random order.
    pub one_at_a_time: bool,
}

/// Electricity Maps: grid mix and renewable share.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ElectricityCfg {
    pub enabled: bool,
    pub zone: String,
    pub token: String,
    pub poll_secs: u64,
}

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

/// The SEN live view.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SenCfg {
    /// Pictures shown per second: 10, 15 or 30 (the stream has 30). Each
    /// shown picture is a full-panel push: on the SPI1 bus about 2 % of a
    /// core per frame per second.
    pub fps: u32,
}

impl Default for SenCfg {
    fn default() -> Self {
        Self { fps: 15 }
    }
}

impl SenCfg {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// GitHub activity: a fine-grained token with read access to contributions,
/// events and Actions.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct GithubCfg {
    pub enabled: bool,
    pub token: String,
    pub poll_secs: u64,
    /// `owner/repo` list for the `ci` role; empty = your repos pushed in the last 7 days.
    pub ci_repos: Vec<String>,
}

/// Argo CD applications for the deploys role.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ArgocdCfg {
    pub enabled: bool,
    pub namespace: String,
}

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

/// A Bambu Lab printer's MQTT report. On the LAN the printer needs LAN only
/// mode with Developer Mode on; the access code and serial are on its
/// screen. In cloud mode the web UI logs in to the Bambu Lab account
/// instead (the token lives in its own file, never here).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BambuCfg {
    pub enabled: bool,
    /// `lan` (the printer's own broker) or `cloud` (a Bambu Lab login).
    pub mode: BambuMode,
    /// Cloud mode: `global` or `china`, where the account lives.
    pub region: rackscreen_sources::bambu_cloud::Region,
    /// LAN mode: the printer's IP address or host name.
    pub host: String,
    /// The serial number, `01P00A000000000`.
    pub serial: String,
    /// LAN mode: the 8-character access code.
    pub access_code: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BambuMode {
    #[default]
    Lan,
    Cloud,
}

impl BambuCfg {
    /// On, with a serial, and on the LAN a host and access code. Cloud mode
    /// does not wait for the login here: the source does, so a login in the
    /// web UI takes effect without a restart.
    pub fn ready(&self) -> bool {
        self.enabled
            && !self.serial.trim().is_empty()
            && match self.mode {
                BambuMode::Lan => {
                    !self.host.trim().is_empty() && !self.access_code.trim().is_empty()
                }
                BambuMode::Cloud => true,
            }
    }
}

/// Home Assistant's REST API with a long-lived access token.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct HomeAssistantCfg {
    pub enabled: bool,
    /// `http://homeassistant.local:8123`.
    pub url: String,
    /// A long-lived access token (profile → Security).
    pub token: String,
    pub poll_secs: u64,
    /// Each shows on a screen as `ha:<name>`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<HaEntityCfg>,
}

impl HomeAssistantCfg {
    /// On, with a URL and a token.
    pub fn ready(&self) -> bool {
        self.enabled && !self.url.trim().is_empty() && !self.token.trim().is_empty()
    }
}

/// One Home Assistant entity, shown by a screen role `ha:<name>`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct HaEntityCfg {
    /// Lowercase letters, digits and `-`; unique.
    pub name: String,
    /// The entity id, `sensor.living_room_temperature`.
    pub entity: String,
    /// The pill text; empty uses the entity's friendly name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Show this attribute instead of the state (`current_temperature`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribute: String,
    /// Use this unit instead of the entity's own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// The ring's range for a number; both or neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

/// `sensor.living_room_temperature`: a domain and an object id of lowercase
/// letters, digits and `_`.
pub fn valid_entity_id(id: &str) -> bool {
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    matches!(id.split_once('.'), Some((d, o)) if ok(d) && ok(o))
}

impl Default for HomeAssistantCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            token: String::new(),
            poll_secs: 15,
            entities: Vec::new(),
        }
    }
}

/// Spotify now playing for the music role. The login itself (tokens) lives
/// in the service user's state dir, not here; `rackscreen setup` writes it.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SpotifyCfg {
    pub enabled: bool,
    /// Client ID of your own Spotify developer app.
    pub client_id: String,
}

/// The web UI the service serves on the LAN.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WebCfg {
    pub enabled: bool,
    /// Address to bind; `0.0.0.0` is every interface.
    pub bind: String,
    pub port: u16,
    /// Empty: no login. Otherwise the UI asks for it once per browser.
    pub password: String,
}

impl Default for WebCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "0.0.0.0".into(),
            port: 8740,
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ScreenCfg {
    /// Legacy single role; upgraded into `roles` by `Config::normalize`.
    #[serde(default, skip_serializing)]
    pub role: Option<String>,
    #[serde(default)]
    pub roles: Vec<RoleEntry>,
    /// Seconds on screen for each entry without its own `secs`.
    #[serde(default = "default_cycle")]
    pub cycle_secs: u64,
    pub spi: u8,
    pub cs: u8,
    pub dc: u8,
    pub rst: u8,
    #[serde(default)]
    pub rotate: u32,
    #[serde(default)]
    pub hflip: bool,
    #[serde(default = "default_hz")]
    pub hz: u32,
}

/// One `roles:` entry: a bare name, or `{ role, secs }` for a role with its
/// own time on screen. Written back bare when it has no `secs`, so older
/// configs round-trip unchanged.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(from = "RawRoleEntry", into = "RawRoleEntry")]
pub struct RoleEntry {
    pub role: String,
    /// Seconds on screen; `None` is the screen's `cycle_secs`.
    pub secs: Option<u64>,
}

impl RoleEntry {
    /// This entry's dwell on a screen whose default is `cycle_secs`.
    pub fn secs_or(&self, cycle_secs: u64) -> u64 {
        self.secs.unwrap_or(cycle_secs)
    }
}

impl From<&str> for RoleEntry {
    fn from(role: &str) -> Self {
        role.to_string().into()
    }
}

impl From<String> for RoleEntry {
    fn from(role: String) -> Self {
        RoleEntry { role, secs: None }
    }
}

/// The YAML and JSON shape of a `RoleEntry`: a name, or a map with `role`
/// and maybe `secs`. Read by hand (not as an untagged enum) so a bad
/// `secs` is reported as such, with the role, instead of "did not match
/// any variant".
#[derive(Serialize)]
#[serde(untagged)]
enum RawRoleEntry {
    Name(String),
    Timed { role: String, secs: Option<u64> },
}

/// A `secs` value as written: whole seconds, or what was there instead.
enum SecsIn {
    Whole(u64),
    Bad(String),
}

impl<'de> Deserialize<'de> for SecsIn {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = SecsIn;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a whole number of seconds")
            }
            fn visit_u64<E>(self, v: u64) -> Result<SecsIn, E> {
                Ok(SecsIn::Whole(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<SecsIn, E> {
                Ok(u64::try_from(v).map_or_else(|_| SecsIn::Bad(v.to_string()), SecsIn::Whole))
            }
            fn visit_f64<E>(self, v: f64) -> Result<SecsIn, E> {
                Ok(if v >= 0.0 && v.fract() == 0.0 && v <= 1e9 {
                    SecsIn::Whole(v as u64)
                } else {
                    SecsIn::Bad(v.to_string())
                })
            }
            fn visit_str<E>(self, v: &str) -> Result<SecsIn, E> {
                Ok(SecsIn::Bad(format!("\"{v}\"")))
            }
            fn visit_bool<E>(self, v: bool) -> Result<SecsIn, E> {
                Ok(SecsIn::Bad(v.to_string()))
            }
        }
        d.deserialize_any(V)
    }
}

impl<'de> Deserialize<'de> for RawRoleEntry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = RawRoleEntry;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a role name, or a map like { role: iss, secs: 30 }")
            }
            fn visit_str<E>(self, v: &str) -> Result<RawRoleEntry, E> {
                Ok(RawRoleEntry::Name(v.to_string()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<RawRoleEntry, A::Error> {
                let (mut role, mut secs) = (None::<String>, None::<SecsIn>);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "role" => role = Some(map.next_value()?),
                        "secs" => secs = map.next_value()?,
                        _ => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                let role = role.ok_or_else(|| A::Error::missing_field("role"))?;
                match secs {
                    Some(SecsIn::Bad(got)) => Err(A::Error::custom(bad_secs(&role, &got))),
                    Some(SecsIn::Whole(n)) => Ok(RawRoleEntry::Timed {
                        role,
                        secs: Some(n),
                    }),
                    None => Ok(RawRoleEntry::Timed { role, secs: None }),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// What is wrong with a `secs` that is not a whole number.
fn bad_secs(role: &str, got: &str) -> String {
    format!("{role} secs must be a whole number between 3 and 300 (got {got})")
}

/// Name the screen and the role of a `secs` that is not a whole number,
/// before the typed parse meets it (which knows the role but not the screen).
fn check_role_secs(raw: &serde_yaml_ng::Value) -> Result<()> {
    let screens = raw.get("screens").and_then(|s| s.as_sequence());
    for (i, screen) in screens.into_iter().flatten().enumerate() {
        let roles = screen.get("roles").and_then(|r| r.as_sequence());
        for entry in roles.into_iter().flatten() {
            let Some(secs) = entry.get("secs").filter(|s| !s.is_null()) else {
                continue;
            };
            let whole = secs.as_u64().is_some()
                || secs.as_f64().is_some_and(|f| f >= 0.0 && f.fract() == 0.0);
            if !whole {
                let role = entry.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                let got = serde_yaml_ng::to_string(secs).unwrap_or_default();
                anyhow::bail!("screen {}: {}", i + 1, bad_secs(role, got.trim()));
            }
        }
    }
    Ok(())
}

impl From<RawRoleEntry> for RoleEntry {
    fn from(raw: RawRoleEntry) -> Self {
        match raw {
            RawRoleEntry::Name(role) => RoleEntry { role, secs: None },
            RawRoleEntry::Timed { role, secs } => RoleEntry { role, secs },
        }
    }
}

impl From<RoleEntry> for RawRoleEntry {
    fn from(e: RoleEntry) -> Self {
        match e.secs {
            None => RawRoleEntry::Name(e.role),
            secs => RawRoleEntry::Timed { role: e.role, secs },
        }
    }
}

fn default_hz() -> u32 {
    50_000_000
}

fn default_cycle() -> u64 {
    15
}

/// The prefix of a GIF role in a roles list: `gif:<name>`.
pub const GIF_ROLE_PREFIX: &str = "gif:";
/// The prefix of a Home Assistant role in a roles list: `ha:<name>`.
pub const HA_ROLE_PREFIX: &str = "ha:";

impl ScreenCfg {
    /// Every configured role, in cycling order. At least one. A `gif:` entry
    /// needs the `gifs:` list and is an error here; use `Config::screen_roles`.
    pub fn roles(&self) -> Result<Vec<Role>> {
        self.roles_in(&[], &[])
    }
    /// Every configured role, `gif:<name>` resolved against `gifs` and
    /// `ha:<name>` against `ha`.
    pub fn roles_in(&self, gifs: &[GifCfg], ha: &[HaEntityCfg]) -> Result<Vec<Role>> {
        anyhow::ensure!(!self.roles.is_empty(), "screen has no roles");
        let index = |i: usize| u16::try_from(i).ok();
        self.roles
            .iter()
            .map(|e| {
                let r = &e.role;
                if let Some(name) = r.strip_prefix(GIF_ROLE_PREFIX) {
                    gifs.iter()
                        .position(|g| g.name == name)
                        .and_then(index)
                        .map(Role::Gif)
                        .with_context(|| format!("unknown GIF '{r}': add '{name}' under gifs:"))
                } else if let Some(name) = r.strip_prefix(HA_ROLE_PREFIX) {
                    ha.iter()
                        .position(|e| e.name == name)
                        .and_then(index)
                        .map(Role::Ha)
                        .with_context(|| {
                            format!(
                                "unknown Home Assistant entity '{r}': add '{name}' under homeassistant.entities:"
                            )
                        })
                } else {
                    Role::parse(r).with_context(|| format!("unknown screen role '{r}'"))
                }
            })
            .collect()
    }
    /// The role shown first, used where a single role identifies the screen.
    pub fn first_role(&self) -> Result<Role> {
        Ok(self.roles()?[0])
    }
}

impl Default for K8sCfg {
    fn default() -> Self {
        Self {
            kubeconfig: "~/k8s-monitor.yaml".into(),
        }
    }
}
impl Default for PromCfg {
    fn default() -> Self {
        Self {
            namespace: "monitoring".into(),
            service: "auto".into(),
            port: 9090,
            poll_secs: 5,
            ignore_alerts: vec!["Watchdog".into(), "InfoInhibitor".into()],
        }
    }
}
impl Default for QbitCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            namespace: "arr-stack".into(),
            service: "qbittorrent".into(),
            port: 8080,
            user: String::new(),
            pass: String::new(),
            poll_secs: 3,
        }
    }
}
impl Default for NightCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            start: "23:00".into(),
            end: "07:00".into(),
        }
    }
}
impl Default for ThresholdsCfg {
    fn default() -> Self {
        Self {
            hot_cpu: 90.0,
            hot_mem: 90.0,
            hot_temp: 70.0,
        }
    }
}
impl Default for ElectricityCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            zone: "NL".into(),
            token: String::new(),
            poll_secs: 300,
        }
    }
}
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
            ci_repos: Vec::new(),
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
impl Default for DisplayCfg {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            fps: 60,
            spi_chunk: 4096,
            one_at_a_time: true,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Config::from_yaml(include_str!("../../../config.example.yaml"))
            .expect("config.example.yaml is valid")
    }
}

/// `~/x` relative to `home`; anything else unchanged.
pub fn expand_home_with(p: &str, home: &Path) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(p),
    }
}

/// The user who ran `sudo`, when we are root because of it.
pub(crate) fn sudo_user() -> Option<nix::unistd::User> {
    if !nix::unistd::geteuid().is_root() {
        return None;
    }
    let name = std::env::var("SUDO_USER").ok().filter(|n| !n.is_empty())?;
    nix::unistd::User::from_name(&name).ok().flatten()
}

/// Where downloaded GIFs are cached: `$XDG_CACHE_HOME/rackscreen/gifs`, by
/// default `~/.cache/rackscreen/gifs` of the user `~` stands for.
pub fn gif_cache_dir() -> Option<PathBuf> {
    let xdg = if sudo_user().is_some() {
        None
    } else {
        std::env::var("XDG_CACHE_HOME").ok()
    };
    resolved_home().map(|home| gif_cache_dir_with(xdg.as_deref(), &home))
}

/// `gif_cache_dir` for a given `XDG_CACHE_HOME` (used only when absolute) and home.
pub fn gif_cache_dir_with(xdg_cache_home: Option<&str>, home: &Path) -> PathBuf {
    let base = match xdg_cache_home {
        Some(x) if Path::new(x).is_absolute() => PathBuf::from(x),
        _ => home.join(".cache"),
    };
    base.join("rackscreen").join("gifs")
}

/// The home directory `~` stands for: under sudo the invoking user's, else ours.
fn resolved_home() -> Option<PathBuf> {
    sudo_user().map(|u| u.dir).or_else(dirs::home_dir)
}

pub fn expand_home(p: &str) -> PathBuf {
    match resolved_home() {
        Some(home) => expand_home_with(p, &home),
        None => PathBuf::from(p),
    }
}

pub const DEFAULT_PATH: &str = "/etc/rackscreen/config.yaml";

/// Roles that no longer exist; an old config's roles lists drop them at load.
const REMOVED_ROLES: [&str; 3] = ["face", "price", "carbon"];

/// Top-level blocks of removed features; an old config's copy is ignored.
const REMOVED_BLOCKS: [&str; 2] = ["face", "price"];

impl Config {
    pub fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_PATH)
    }

    pub fn from_yaml(text: &str) -> Result<Config> {
        let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).context("parse yaml")?;
        for block in REMOVED_BLOCKS {
            if raw.get(block).is_some() {
                tracing::warn!("config: the `{block}:` block is no longer used and is ignored");
            }
        }
        check_role_secs(&raw)?;
        let mut cfg: Config = serde_yaml_ng::from_str(text).context("parse yaml")?;
        cfg.normalize()?;
        Ok(cfg)
    }

    /// Upgrade legacy fields in place and fill the per-app arr ports. Removed
    /// roles (`REMOVED_ROLES`) are dropped from old configs, with one warning
    /// per screen and role; a screen left with no roles is an error naming
    /// what was dropped.
    pub fn normalize(&mut self) -> Result<()> {
        for (i, s) in self.screens.iter_mut().enumerate() {
            if let Some(r) = s.role.take() {
                if s.roles.is_empty() {
                    s.roles = vec![r.into()];
                }
            }
            let mut dropped: Vec<&str> = Vec::new();
            s.roles.retain(|e| {
                let Some(gone) = REMOVED_ROLES
                    .iter()
                    .find(|g| e.role.eq_ignore_ascii_case(g))
                else {
                    return true;
                };
                if !dropped.contains(gone) {
                    tracing::warn!("screen {}: the {gone} role was removed; ignoring", i + 1);
                    dropped.push(gone);
                }
                false
            });
            anyhow::ensure!(
                dropped.is_empty() || !s.roles.is_empty(),
                "screen {} has no roles left after dropping removed roles ({})",
                i + 1,
                dropped.join(", ")
            );
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
        Ok(())
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml_ng::to_string(self).context("serialise yaml")
    }

    /// Load from `path` (or the default path). Missing file: warn and use defaults.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let path = path
            .map(Path::to_path_buf)
            .unwrap_or_else(Config::default_path);
        if !path.exists() {
            tracing::warn!(
                "config {} not found, using built-in defaults",
                path.display()
            );
            return Ok(Config::default());
        }
        Config::load_or_default(&path)
    }

    /// Load and validate from `path`; a missing file silently yields defaults (used by
    /// setup tools). A file that parses but fails `validate` is an error too, so an
    /// out-of-range `rotate` never reaches `Orient::new`.
    pub fn load_or_default(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let cfg = Config::from_yaml(&text).with_context(|| format!("parse {}", path.display()))?;
        cfg.validate()
            .with_context(|| format!("invalid {}", path.display()))?;
        Ok(cfg)
    }

    /// Write atomically (temp file + rename) with mode 0640: the file may hold the
    /// qBittorrent password, so it is not world-readable.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let text = format!(
            "# RackScreen configuration (written by rackscreen)\n{}",
            self.to_yaml()?
        );
        let tmp = path.with_extension("yaml.tmp");
        std::fs::write(&tmp, text).with_context(|| format!("write {}", tmp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o640))
                .with_context(|| format!("chmod {}", tmp.display()))?;
        }
        std::fs::rename(&tmp, path)
            .with_context(|| format!("rename {} to {}", tmp.display(), path.display()))
    }

    /// A screen's roles, `gif:<name>` resolved against `gifs:`.
    pub fn screen_roles(&self, screen: &ScreenCfg) -> Result<Vec<Role>> {
        screen.roles_in(&self.gifs, &self.homeassistant.entities)
    }

    /// A screen's cycle: each role with its seconds on screen.
    pub fn screen_entries(&self, screen: &ScreenCfg) -> Result<Vec<Entry>> {
        let roles = self.screen_roles(screen)?;
        Ok(roles
            .into_iter()
            .zip(&screen.roles)
            .map(|(role, e)| Entry::new(role, e.secs_or(screen.cycle_secs) as Secs))
            .collect())
    }

    /// The name a role has in this config: `gif:<name>` for a GIF.
    pub fn role_name(&self, role: Role) -> String {
        match role {
            Role::Gif(i) => format!(
                "{GIF_ROLE_PREFIX}{}",
                self.gifs.get(i as usize).map_or("?", |g| g.name.as_str())
            ),
            Role::Ha(i) => format!(
                "{HA_ROLE_PREFIX}{}",
                self.homeassistant
                    .entities
                    .get(i as usize)
                    .map_or("?", |e| e.name.as_str())
            ),
            r => r.name().to_string(),
        }
    }

    /// Every role a screen can show here: the built-in ones, then each GIF.
    pub fn pickable_roles(&self) -> Vec<Role> {
        let gifs = (0..self.gifs.len().min(u16::MAX as usize)).map(|i| Role::Gif(i as u16));
        let ha = (0..self.homeassistant.entities.len().min(u16::MAX as usize))
            .map(|i| Role::Ha(i as u16));
        Role::ALL.iter().copied().chain(gifs).chain(ha).collect()
    }

    /// What the screens need to know about each Home Assistant entity.
    pub fn ha_slots(&self) -> Vec<rackscreen_core::model::HaSlot> {
        self.homeassistant
            .entities
            .iter()
            .map(|e| rackscreen_core::model::HaSlot {
                label: Some(e.label.trim().to_string()).filter(|l| !l.is_empty()),
                min: e.min,
                max: e.max,
            })
            .collect()
    }

    /// The Home Assistant entities some screen shows, for the poller.
    pub fn used_ha(&self) -> Vec<(u16, &HaEntityCfg)> {
        let used: std::collections::HashSet<u16> = self
            .screens
            .iter()
            .filter_map(|s| self.screen_roles(s).ok())
            .flatten()
            .filter_map(|r| match r {
                Role::Ha(i) => Some(i),
                _ => None,
            })
            .collect();
        self.homeassistant
            .entities
            .iter()
            .enumerate()
            .filter(|(i, _)| used.contains(&(*i as u16)))
            .map(|(i, e)| (i as u16, e))
            .collect()
    }

    /// `(index, name, url)` of each GIF some screen shows, in `gifs:` order;
    /// only these are downloaded and decoded.
    pub fn used_gifs(&self) -> Vec<(u16, String, String)> {
        let used: std::collections::HashSet<u16> = self
            .screens
            .iter()
            .filter_map(|s| self.screen_roles(s).ok())
            .flatten()
            .filter_map(|r| match r {
                Role::Gif(i) => Some(i),
                _ => None,
            })
            .collect();
        self.gifs
            .iter()
            .enumerate()
            .filter(|(i, _)| used.contains(&(*i as u16)))
            .map(|(i, g)| (i as u16, g.name.clone(), g.url.trim().to_string()))
            .collect()
    }

    /// Some screen has `role` in its list.
    pub fn uses_role(&self, role: Role) -> bool {
        self.screens
            .iter()
            .filter_map(|s| self.screen_roles(s).ok())
            .flatten()
            .any(|r| r == role)
    }

    /// `(lat, lon)` when both are set.
    pub fn location(&self) -> Option<(f64, f64)> {
        match (self.location.lat, self.location.lon) {
            (Some(lat), Some(lon)) => Some((lat, lon)),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.screens.is_empty(),
            "config needs at least one [[screens]] entry"
        );
        for (i, g) in self.gifs.iter().enumerate() {
            anyhow::ensure!(
                rackscreen_core::gif::valid_gif_name(&g.name),
                "gifs: '{}' is not a valid name (lowercase letters, digits and -)",
                g.name
            );
            anyhow::ensure!(
                !self.gifs[..i].iter().any(|o| o.name == g.name),
                "gifs: '{}' is listed twice",
                g.name
            );
            if let Err(e) = rackscreen_core::gif::classify_link(&g.url) {
                anyhow::bail!("gifs: '{}': {e} (got {})", g.name, g.url.trim());
            }
            if let Some(o) = self.gifs[..i].iter().find(|o| o.url.trim() == g.url.trim()) {
                anyhow::bail!(
                    "gifs: '{}' and '{}' have the same link ({}); keep one",
                    o.name,
                    g.name,
                    g.url.trim()
                );
            }
        }
        anyhow::ensure!(
            GIF_RATINGS.contains(&self.gif_search.rating.as_str()),
            "gif_search.rating must be g, pg, pg-13 or r (got {})",
            self.gif_search.rating
        );
        for (i, s) in self.screens.iter().enumerate() {
            let roles = self.screen_roles(s)?;
            let name = self.role_name(roles[0]);
            anyhow::ensure!(
                matches!(s.rotate, 0 | 90 | 180 | 270),
                "screen '{}': rotate must be 0, 90, 180 or 270 (got {})",
                name,
                s.rotate
            );
            anyhow::ensure!(
                (3..=300).contains(&s.cycle_secs),
                "screen '{}': cycle_secs must be between 3 and 300 (got {})",
                name,
                s.cycle_secs
            );
            for e in &s.roles {
                if let Some(secs) = e.secs {
                    anyhow::ensure!(
                        (3..=300).contains(&secs),
                        "screen {}: {} secs must be between 3 and 300 (got {secs})",
                        i + 1,
                        e.role
                    );
                }
            }
        }
        anyhow::ensure!(
            !self.electricity.enabled || !self.electricity.zone.trim().is_empty(),
            "electricity.zone must be set when electricity is enabled"
        );
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
            matches!(self.sen.fps, 10 | 15 | 30),
            "sen.fps must be 10, 15 or 30 (got {})",
            self.sen.fps
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
            anyhow::ensure!(
                secs >= 60,
                "{name}.poll_secs must be at least 60 (got {secs})"
            );
        }
        anyhow::ensure!(
            self.homeassistant.poll_secs >= 5,
            "homeassistant.poll_secs must be at least 5 (got {})",
            self.homeassistant.poll_secs
        );
        let url = self.homeassistant.url.trim();
        anyhow::ensure!(
            url.is_empty() || url.starts_with("http://") || url.starts_with("https://"),
            "homeassistant.url must start with http:// or https:// (got {url})"
        );
        let ents = &self.homeassistant.entities;
        for (i, e) in ents.iter().enumerate() {
            anyhow::ensure!(
                rackscreen_core::gif::valid_gif_name(&e.name),
                "homeassistant.entities: '{}' is not a valid name (lowercase letters, digits and -)",
                e.name
            );
            anyhow::ensure!(
                !ents[..i].iter().any(|o| o.name == e.name),
                "homeassistant.entities: '{}' is listed twice",
                e.name
            );
            anyhow::ensure!(
                valid_entity_id(e.entity.trim()),
                "homeassistant.entities: '{}' needs an entity id like sensor.living_room (got {})",
                e.name,
                e.entity.trim()
            );
            match (e.min, e.max) {
                (Some(lo), Some(hi)) => anyhow::ensure!(
                    lo.is_finite() && hi.is_finite() && hi > lo,
                    "homeassistant.entities: '{}' needs max above min",
                    e.name
                ),
                (None, None) => {}
                _ => anyhow::bail!(
                    "homeassistant.entities: '{}' needs both min and max, or neither",
                    e.name
                ),
            }
        }
        let host = self.bambu.host.trim();
        anyhow::ensure!(
            !host.contains("://") && !host.contains('/') && !host.contains(' '),
            "bambu.host is the printer's IP address or host name, without http:// (got {host})"
        );
        anyhow::ensure!(
            self.bambu
                .serial
                .trim()
                .chars()
                .all(|c| c.is_ascii_alphanumeric()),
            "bambu.serial must be letters and digits only (got {})",
            self.bambu.serial.trim()
        );
        anyhow::ensure!(
            self.jellyfin.poll_secs >= 2,
            "jellyfin.poll_secs must be at least 2 (got {})",
            self.jellyfin.poll_secs
        );
        anyhow::ensure!(
            self.shows.every_mins >= 2,
            "shows.every_mins must be at least 2 (got {})",
            self.shows.every_mins
        );
        let gap_mm = self.shows.gap_mm;
        anyhow::ensure!(
            gap_mm.is_finite() && (0.0..=200.0).contains(&gap_mm),
            "shows.gap_mm must be between 0 and 200 (got {gap_mm})"
        );
        if let Some(id) = self
            .shows
            .list
            .iter()
            .find(|id| rackscreen_core::shows::by_id(id).is_none())
        {
            let known: Vec<&str> = rackscreen_core::shows::SHOWS.iter().map(|s| s.id).collect();
            anyhow::bail!(
                "shows.list: unknown show '{id}' (known: {})",
                known.join(", ")
            );
        }
        if let Some(order) = &self.shows.order {
            let mut sorted = order.clone();
            sorted.sort_unstable();
            anyhow::ensure!(
                sorted == (0..self.screens.len()).collect::<Vec<_>>(),
                "shows.order must list each screen index 0..{} once (got {order:?})",
                self.screens.len()
            );
        }
        self.transitions.to_core()?;
        let url = self.jellyfin.url.trim();
        anyhow::ensure!(
            url.is_empty() || url.starts_with("http://") || url.starts_with("https://"),
            "jellyfin.url must start with http:// or https:// (got {url})"
        );
        let id = self.spotify.client_id.trim();
        anyhow::ensure!(
            id.chars().all(|c| c.is_ascii_alphanumeric()),
            "spotify.client_id must be letters and digits only (got {id})"
        );
        anyhow::ensure!(
            self.web.bind.trim().parse::<std::net::IpAddr>().is_ok(),
            "web.bind must be an IP address (got {})",
            self.web.bind
        );
        anyhow::ensure!(self.web.port != 0, "web.port must be 1..65535");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses_with_four_screens() {
        let c = Config::default();
        assert_eq!(c.screens.len(), 4);
        assert_eq!(c.screens[3].first_role().unwrap(), Role::Health);
        assert_eq!(c.screens[0].cycle_secs, 15);
        assert_eq!(c.screens[2].hz, 50_000_000);
        assert_eq!(c.prometheus.port, 9090);
        assert_eq!(
            c.prometheus.ignore_alerts,
            vec!["Watchdog", "InfoInhibitor"]
        );
        assert!(c.qbittorrent.enabled);
        assert_eq!(c.display.spi_chunk, 4096);
        assert!(c.display.one_at_a_time);
        c.validate().unwrap();
    }

    #[test]
    fn partial_yaml_fills_defaults() {
        let c = Config::from_yaml(
            "night:\n  enabled: false\nscreens:\n  - { role: cpu, spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert!(!c.night.enabled);
        assert_eq!(c.night.start, "23:00");
        assert_eq!(c.screens[0].hz, 50_000_000);
        assert_eq!(c.screens[0].rotate, 0);
        assert_eq!(c.display.fps, 60);
        assert!(c.display.one_at_a_time, "on unless the file says otherwise");
        // no transitions block: the fade via black of before, 0.4 s
        assert_eq!(c.transitions, TransitionsCfg::default());
        assert_eq!(
            c.transitions.to_core().unwrap(),
            rackscreen_core::transition::Transitions::default()
        );
        assert_eq!(c.transitions.style, "fade-black");
        assert_eq!(c.transitions.direction, "", "the style's default");
        assert_eq!(c.transitions.secs, 0.4);
        assert!(!c.transitions.random);
        assert!(c.transitions.shows.is_none());
    }

    #[test]
    fn transitions_parse_and_validate() {
        use rackscreen_core::transition::{Direction, Kind, Style};
        let base = "screens:\n  - { role: cpu, spi: 0, cs: 0, dc: 6, rst: 5 }\n";
        let c = Config::from_yaml(&format!(
            "{base}transitions:\n  style: slide\n  direction: up\n  secs: 0.6\n  random: true\n  shows: {{ style: wipe, direction: right, secs: 1 }}\n"
        ))
        .unwrap();
        c.validate().unwrap();
        let t = c.transitions.to_core().unwrap();
        assert_eq!(
            t.screens.kind,
            Kind::new(Style::Slide, Direction::Up).unwrap()
        );
        assert_eq!(t.screens.secs, 0.6);
        assert!(t.random);
        let shows = t.shows.unwrap();
        assert_eq!(
            shows.kind,
            Kind::new(Style::Wipe, Direction::Right).unwrap()
        );
        assert_eq!(shows.secs, 1.0);
        // a style without a direction ignores whatever is there
        let c = Config::from_yaml(&format!(
            "{base}transitions:\n  style: crossfade\n  direction: cw\n"
        ))
        .unwrap();
        c.validate().unwrap();
        assert_eq!(
            c.transitions.to_core().unwrap().screens.kind,
            Kind::of(Style::Crossfade)
        );
        // the shows block fills its own defaults; an empty direction is the
        // style's own
        let c = Config::from_yaml(&format!("{base}transitions:\n  shows: {{ style: iris }}\n"))
            .unwrap();
        c.validate().unwrap();
        let shows = c.transitions.to_core().unwrap().shows.unwrap();
        assert_eq!(shows.kind, Kind::of(Style::Iris));
        assert_eq!(shows.secs, 0.4);
        let c = Config::from_yaml(&format!("{base}transitions:\n  style: zoom\n")).unwrap();
        assert_eq!(
            c.transitions.to_core().unwrap().screens.kind,
            Kind::new(Style::Zoom, Direction::In).unwrap()
        );
        // the round trip keeps the block (and leaves `shows` out while unset)
        let y = Config::default().to_yaml().unwrap();
        assert!(y.contains("transitions:"), "{y}");
        assert!(y.contains("style: fade-black"), "{y}");
        assert!(!y.contains("shows: null"), "{y}");
        // bad values
        for (yaml, want) in [
            (
                "transitions:\n  style: swoosh\n",
                "unknown transition 'swoosh'",
            ),
            (
                "transitions:\n  style: slide\n  direction: cw\n",
                "'slide' takes direction left, right, up, down (got cw)",
            ),
            (
                "transitions:\n  style: slide\n  direction: sideways\n",
                "unknown direction 'sideways'",
            ),
            (
                "transitions:\n  secs: 0.1\n",
                "transitions.secs must be between",
            ),
            (
                "transitions:\n  secs: 3\n",
                "transitions.secs must be between",
            ),
            (
                "transitions:\n  shows: { style: iris, direction: left }\n",
                "transitions.shows.direction",
            ),
            (
                "transitions:\n  shows: { style: nope }\n",
                "transitions.shows.style: unknown",
            ),
        ] {
            let c = Config::from_yaml(&format!("{base}{yaml}")).unwrap();
            let err = c.validate().unwrap_err().to_string();
            assert!(err.contains(want), "{yaml:?}: {err}");
        }
    }

    #[test]
    fn yaml_round_trip_preserves_everything() {
        let mut c = Config::default();
        c.screens[1].rotate = 90;
        c.screens[1].hflip = false;
        c.qbittorrent.pass = "s3cret".into();
        let back = Config::from_yaml(&c.to_yaml().unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn save_and_load_or_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.yaml");
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
        let mut c = Config::default();
        c.display.spi_chunk = 65536;
        c.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# RackScreen configuration"));
        assert_eq!(
            Config::load_or_default(&path).unwrap().display.spi_chunk,
            65536
        );
    }

    #[test]
    fn save_is_atomic_and_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        Config::default().save(&path).unwrap();
        assert!(
            !path.with_extension("yaml.tmp").exists(),
            "temp file removed"
        );
        assert!(std::fs::read_to_string(&path).unwrap().contains("screens:"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o640, "mode {mode:o}");
        }
    }

    #[test]
    fn load_or_default_rejects_invalid_rotate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(
            &path,
            "screens: [{ role: cpu, spi: 0, cs: 0, dc: 6, rst: 5, rotate: 45 }]\n",
        )
        .unwrap();
        let err = format!("{:#}", Config::load_or_default(&path).unwrap_err());
        assert!(err.contains("rotate"), "{err}");
    }

    #[test]
    fn bad_role_and_bad_rotate_rejected() {
        let s = ScreenCfg {
            role: None,
            roles: vec!["nope".into()],
            cycle_secs: 15,
            spi: 0,
            cs: 0,
            dc: 0,
            rst: 0,
            rotate: 0,
            hflip: false,
            hz: 1,
        };
        assert!(s.roles().is_err());
        assert!(s.first_role().is_err());
        let mut c = Config::default();
        c.screens[0].rotate = 45;
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("rotate") && err.contains("45"));
    }

    #[test]
    fn legacy_role_key_upgrades_to_roles() {
        let c = Config::from_yaml("screens:\n  - { role: cpu, spi: 0, cs: 0, dc: 6, rst: 5 }\n")
            .unwrap();
        assert_eq!(c.screens[0].roles, vec![RoleEntry::from("cpu")]);
        assert_eq!(c.screens[0].role, None);
        assert_eq!(c.screens[0].cycle_secs, 15);
        c.validate().unwrap();
        let text = c.to_yaml().unwrap();
        assert!(
            !text.contains("role:"),
            "legacy key not written back: {text}"
        );
        assert!(text.contains("roles:"), "{text}");
    }

    #[test]
    fn role_list_and_cycle_secs_round_trip() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [cpu, thermal], cycle_secs: 20, spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!(
            c.screens[0].roles().unwrap(),
            vec![Role::Cpu, Role::Thermal]
        );
        assert_eq!(c.screens[0].cycle_secs, 20);
        c.validate().unwrap();
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
    }

    #[test]
    fn role_entries_mix_bare_names_and_own_times() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [cpu, { role: iss, secs: 30 }, { role: mem }], cycle_secs: 10, spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        c.validate().unwrap();
        let roles = &c.screens[0].roles;
        assert_eq!(roles[0], RoleEntry::from("cpu"));
        assert_eq!(roles[1].secs, Some(30));
        assert_eq!(roles[2], RoleEntry::from("mem"), "a map without secs");
        assert_eq!(
            c.screen_entries(&c.screens[0]).unwrap(),
            vec![
                Entry::new(Role::Cpu, 10.0),
                Entry::new(Role::Iss, 30.0),
                Entry::new(Role::Mem, 10.0),
            ]
        );
        // bare unless timed, in YAML and in the JSON the web UI gets
        let text = c.to_yaml().unwrap();
        assert!(text.contains("- cpu\n"), "{text}");
        assert!(text.contains("- mem\n"), "{text}");
        assert!(text.contains("role: iss"), "{text}");
        assert_eq!(Config::from_yaml(&text).unwrap(), c);
        assert_eq!(
            serde_json::to_value(roles).unwrap(),
            serde_json::json!(["cpu", { "role": "iss", "secs": 30 }, "mem"])
        );
    }

    #[test]
    fn role_secs_that_are_not_whole_numbers_say_so() {
        let yaml = |secs: &str| {
            format!(
                "screens:\n  - {{ roles: [cpu], spi: 0, cs: 0, dc: 6, rst: 5 }}\n  - {{ roles: [cpu, {{ role: iss, secs: {secs} }}], spi: 0, cs: 1, dc: 13, rst: 26 }}\n"
            )
        };
        for (secs, got) in [
            ("-5", "-5"),
            ("12.5", "12.5"),
            ("soon", "soon"),
            ("true", "true"),
        ] {
            let e = format!("{:#}", Config::from_yaml(&yaml(secs)).unwrap_err());
            assert_eq!(
                e,
                format!("screen 2: iss secs must be a whole number between 3 and 300 (got {got})")
            );
        }
        // whole numbers in any spelling still load
        for secs in ["30", "30.0", "null", "~"] {
            Config::from_yaml(&yaml(secs)).unwrap();
        }
        // JSON from the web UI: the role and the value, no "untagged enum"
        let json = |secs: &str| format!(r#"["cpu", {{ "role": "iss", "secs": {secs} }}]"#);
        for (secs, got) in [("-5", "-5"), ("12.5", "12.5"), ("\"x\"", "\"x\"")] {
            let e = serde_json::from_str::<Vec<RoleEntry>>(&json(secs))
                .unwrap_err()
                .to_string();
            assert!(
                e.starts_with(&format!(
                    "iss secs must be a whole number between 3 and 300 (got {got})"
                )),
                "{e}"
            );
        }
        let ok: Vec<RoleEntry> = serde_json::from_str(&json("30")).unwrap();
        assert_eq!(ok[1].secs, Some(30));
        let ok: Vec<RoleEntry> = serde_json::from_str(&json("null")).unwrap();
        assert_eq!(ok[1].secs, None);
        // other shapes say what is expected
        let e = serde_json::from_str::<Vec<RoleEntry>>("[7]")
            .unwrap_err()
            .to_string();
        assert!(e.contains("a role name, or a map"), "{e}");
        let e = serde_json::from_str::<Vec<RoleEntry>>(r#"[{"secs": 5}]"#)
            .unwrap_err()
            .to_string();
        assert!(e.contains("missing field `role`"), "{e}");
    }

    #[test]
    fn out_of_range_role_secs_name_the_screen_and_role() {
        for secs in [2u64, 301] {
            let mut c = Config::default();
            c.screens[1].roles[0].secs = Some(secs);
            let role = c.screens[1].roles[0].role.clone();
            let err = c.validate().unwrap_err().to_string();
            assert!(err.contains("screen 2") && err.contains(&role), "{err}");
            assert!(err.contains(&secs.to_string()), "{err}");
        }
        let mut c = Config::default();
        c.screens[1].roles[0].secs = Some(3);
        c.validate().unwrap();
    }

    #[test]
    fn existing_configs_round_trip_byte_identically() {
        let c = Config::default();
        let text = c.to_yaml().unwrap();
        assert_eq!(Config::from_yaml(&text).unwrap().to_yaml().unwrap(), text);
        assert!(!text.contains("role:"), "{text}");
    }

    #[test]
    fn unknown_role_in_list_rejected() {
        let c =
            Config::from_yaml("screens:\n  - { roles: [nope], spi: 0, cs: 0, dc: 6, rst: 5 }\n")
                .unwrap();
        let err = format!("{:#}", c.validate().unwrap_err());
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn a_face_role_is_dropped_from_a_roles_list() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [cpu, face, mem], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!(
            c.screens[0].roles,
            vec![RoleEntry::from("cpu"), RoleEntry::from("mem")]
        );
        assert_eq!(c.screens[0].roles().unwrap(), vec![Role::Cpu, Role::Mem]);
        // any case, on any screen
        let c = Config::from_yaml(
            "screens:\n  - { roles: [cpu], spi: 0, cs: 0, dc: 6, rst: 5 }\n  - { roles: [mem, Face], spi: 0, cs: 1, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!(c.screens[1].roles, vec![RoleEntry::from("mem")]);
        // a legacy single `role: face` too
        let e = Config::from_yaml("screens:\n  - { role: face, spi: 0, cs: 0, dc: 6, rst: 5 }\n")
            .unwrap_err()
            .to_string();
        assert!(e.starts_with("screen 1 has no roles left"), "{e}");
    }

    #[test]
    fn a_roles_list_with_only_face_is_an_error() {
        let e =
            Config::from_yaml("screens:\n  - { roles: [face], spi: 0, cs: 0, dc: 6, rst: 5 }\n")
                .unwrap_err()
                .to_string();
        assert!(e.contains("face"), "{e}");
    }

    #[test]
    fn an_only_face_screen_errors_with_its_index() {
        let e = Config::from_yaml(
            "screens:\n  - { roles: [cpu], spi: 0, cs: 0, dc: 6, rst: 5 }\n  - { roles: [face], spi: 0, cs: 1, dc: 6, rst: 5 }\n",
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            e,
            "screen 2 has no roles left after dropping removed roles (face)"
        );
    }

    #[test]
    fn price_and_carbon_roles_are_dropped_from_a_roles_list() {
        let c = Config::from_yaml(
            "screens:\n  - { roles: [power-mix, price, renewable, Carbon], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!(
            c.screens[0].roles,
            vec![RoleEntry::from("power-mix"), RoleEntry::from("renewable")]
        );
        assert_eq!(
            c.screens[0].roles().unwrap(),
            vec![Role::PowerMix, Role::Renewable]
        );
        // a legacy single `role: price` too
        let e = Config::from_yaml("screens:\n  - { role: price, spi: 0, cs: 0, dc: 6, rst: 5 }\n")
            .unwrap_err()
            .to_string();
        assert_eq!(
            e,
            "screen 1 has no roles left after dropping removed roles (price)"
        );
    }

    #[test]
    fn an_only_removed_roles_screen_names_each_dropped_role_once() {
        let e = Config::from_yaml(
            "screens:\n  - { roles: [cpu], spi: 0, cs: 0, dc: 6, rst: 5 }\n  - { roles: [mem], spi: 0, cs: 1, dc: 6, rst: 5 }\n  - { roles: [price, carbon, PRICE], spi: 1, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            e,
            "screen 3 has no roles left after dropping removed roles (price, carbon)"
        );
        let e = Config::from_yaml(
            "screens:\n  - { roles: [carbon, face], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            e,
            "screen 1 has no roles left after dropping removed roles (carbon, face)"
        );
    }

    #[test]
    fn empty_role_list_rejected() {
        let c = Config::from_yaml("screens:\n  - { roles: [], spi: 0, cs: 0, dc: 6, rst: 5 }\n")
            .unwrap();
        assert!(c.validate().is_err());
    }

    #[test]
    fn sen_fps_is_10_15_or_30_and_hidden_at_its_default() {
        let mut c = Config::default();
        assert_eq!(c.sen.fps, 15);
        assert!(
            !c.to_yaml().unwrap().contains("sen:"),
            "default not written"
        );
        c.sen.fps = 10;
        c.validate().unwrap();
        assert!(c.to_yaml().unwrap().contains("fps: 10"));
        c.sen.fps = 12;
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("sen.fps"), "{err}");
    }

    #[test]
    fn out_of_range_cycle_secs_rejected() {
        for secs in [1u64, 301] {
            let mut c = Config::default();
            c.screens[0].cycle_secs = secs;
            let err = c.validate().unwrap_err().to_string();
            assert!(err.contains("cycle_secs"), "{err}");
        }
    }

    #[test]
    fn a_legacy_price_block_is_ignored() {
        let c = Config::from_yaml(
            "price:\n  enabled: true\n  zone: NL\n  vat_pct: 21\n  poll_secs: 1800\nnight:\n  enabled: false\n",
        )
        .unwrap();
        assert!(!c.night.enabled);
        assert!(
            !c.to_yaml().unwrap().contains("price:"),
            "never written back"
        );
    }

    #[test]
    fn zoneless_electricity_rejected() {
        let mut c = Config::default();
        c.electricity.enabled = true;
        c.electricity.zone = "  ".into();
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("electricity.zone"), "{err}");
        c.electricity.zone = "NL".into();
        c.validate().unwrap();
    }

    #[test]
    fn electricity_and_threshold_defaults() {
        let c = Config::default();
        assert!(!c.electricity.enabled);
        assert_eq!(c.electricity.zone, "NL");
        assert_eq!(c.electricity.poll_secs, 300);
        assert_eq!(c.thresholds.hot_temp, 70.0);
    }

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

    #[test]
    fn expand_home_works() {
        assert!(expand_home("/abs").starts_with("/abs"));
        assert!(!expand_home("~/x").to_string_lossy().starts_with('~'));
    }

    #[test]
    fn expand_home_with_uses_given_home() {
        let home = Path::new("/home/other");
        assert_eq!(
            expand_home_with("~/x", home),
            PathBuf::from("/home/other/x")
        );
        assert_eq!(expand_home_with("/abs", home), PathBuf::from("/abs"));
        assert_eq!(expand_home_with("rel", home), PathBuf::from("rel"));
        assert_eq!(expand_home_with("~x", home), PathBuf::from("~x"));
    }

    #[test]
    fn a_legacy_face_block_is_ignored() {
        let c = Config::from_yaml(
            "face:\n  bored_after_mins: 4\n  idle_habits: false\nnight:\n  enabled: false\n",
        )
        .unwrap();
        assert!(!c.night.enabled);
        assert!(
            !c.to_yaml().unwrap().contains("face:"),
            "never written back"
        );
    }
    #[test]
    fn bambu_and_home_assistant_parse_resolve_and_validate() {
        let yaml = r#"
bambu: { enabled: true, host: 192.168.1.50, serial: 01P00A000000000, access_code: "12345678" }
homeassistant:
  enabled: true
  url: http://ha.local:8123
  token: t
  entities:
    - { name: living, entity: sensor.living_room_temperature }
    - { name: door, entity: binary_sensor.front_door, label: Front door }
    - { name: spare, entity: light.desk, min: 0, max: 255 }
screens:
  - { roles: [printer, "ha:door", cpu], spi: 0, cs: 0, dc: 24, rst: 25 }
"#;
        let c: Config = serde_yaml_ng::from_str(yaml).unwrap();
        c.validate().unwrap();
        assert!(c.bambu.ready() && c.homeassistant.ready());
        assert_eq!(c.homeassistant.poll_secs, 15);
        assert_eq!(
            c.screen_roles(&c.screens[0]).unwrap(),
            vec![Role::Printer, Role::Ha(1), Role::Cpu]
        );
        assert_eq!(c.role_name(Role::Ha(1)), "ha:door");
        let used: Vec<u16> = c.used_ha().into_iter().map(|(i, _)| i).collect();
        assert_eq!(used, vec![1], "only shown entities are polled");
        assert!(c.pickable_roles().contains(&Role::Ha(2)));
        let slots = c.ha_slots();
        assert_eq!(slots[1].label.as_deref(), Some("Front door"));
        assert_eq!((slots[2].min, slots[2].max), (Some(0.0), Some(255.0)));

        let mut bad = c.clone();
        bad.screens[0].roles = vec!["ha:nope".into()];
        assert!(
            format!("{:#}", bad.screen_roles(&bad.screens[0]).unwrap_err())
                .contains("homeassistant.entities")
        );
        for (edit, msg) in [
            (
                Box::new(|c: &mut Config| c.homeassistant.entities[0].entity = "Sensor.X".into())
                    as Box<dyn Fn(&mut Config)>,
                "entity id",
            ),
            (
                Box::new(|c: &mut Config| c.homeassistant.entities[2].name = "living".into()),
                "listed twice",
            ),
            (
                Box::new(|c: &mut Config| c.homeassistant.entities[2].max = None),
                "both min and max",
            ),
            (
                Box::new(|c: &mut Config| c.homeassistant.url = "ha.local".into()),
                "homeassistant.url",
            ),
            (
                Box::new(|c: &mut Config| c.homeassistant.poll_secs = 2),
                "poll_secs",
            ),
            (
                Box::new(|c: &mut Config| c.bambu.host = "http://192.168.1.50".into()),
                "bambu.host",
            ),
        ] {
            let mut b = c.clone();
            edit(&mut b);
            let err = format!("{:#}", b.validate().unwrap_err());
            assert!(err.contains(msg), "{err}");
        }
        assert!(
            valid_entity_id("sensor.a_1") && !valid_entity_id("sensor") && !valid_entity_id(".x")
        );
        assert_eq!(c.bambu.mode, BambuMode::Lan, "LAN unless asked");

        let mut cloud = c.clone();
        cloud.bambu = serde_yaml_ng::from_str(
            "{ enabled: true, mode: cloud, region: china, serial: 01P00A000000000 }",
        )
        .unwrap();
        cloud.validate().unwrap();
        assert!(cloud.bambu.ready(), "cloud needs no host or access code");
        assert_eq!(
            cloud.bambu.region,
            rackscreen_sources::bambu_cloud::Region::China
        );
        let mut no_serial = cloud.clone();
        no_serial.bambu.serial.clear();
        assert!(!no_serial.bambu.ready());
        assert!(serde_yaml_ng::from_str::<BambuCfg>("{ mode: wifi }").is_err());

        let mut off = c.clone();
        off.bambu.access_code.clear();
        off.homeassistant.token = " ".into();
        assert!(!off.bambu.ready() && !off.homeassistant.ready());
    }

    #[test]
    fn media_github_ci_and_jellyfin_defaults_and_validation() {
        let c = Config::default();
        assert!(!c.media.enabled);
        assert_eq!(c.media.namespace, "arr-stack");
        assert_eq!(
            (c.media.radarr.service.as_str(), c.media.radarr.port),
            ("auto", 7878)
        );
        assert_eq!(
            (c.media.sonarr.service.as_str(), c.media.sonarr.port),
            ("auto", 8989)
        );
        assert_eq!(
            (c.media.lidarr.service.as_str(), c.media.lidarr.port),
            ("auto", 8686)
        );
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
        assert!(c
            .validate()
            .unwrap_err()
            .to_string()
            .contains("jellyfin.poll_secs"));
        c.jellyfin.poll_secs = 2;
        c.jellyfin.url = "192.168.0.10:8096".into();
        assert!(c
            .validate()
            .unwrap_err()
            .to_string()
            .contains("jellyfin.url"));
        c.jellyfin.url = "http://192.168.0.10:8096".into();
        c.validate().unwrap();
        c.jellyfin.url = "https://jf.example".into();
        c.validate().unwrap();
        let yaml = "github: { ci_repos: [a/b, c/d] }\njellyfin: { enabled: true, url: \"http://x:8096\", api_key: k }\nmedia: { enabled: true, radarr: { api_key: r } }\n";
        let c = Config::from_yaml(yaml).unwrap();
        assert_eq!(
            c.github.ci_repos,
            vec!["a/b".to_string(), "c/d".to_string()]
        );
        assert!(c.jellyfin.enabled && c.jellyfin.api_key == "k");
        assert_eq!(
            c.media.radarr.port, 7878,
            "partial arr block keeps the defaults"
        );
        assert_eq!(c.media.radarr.api_key, "r");
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
    }

    #[test]
    fn spotify_defaults_and_validation() {
        let c = Config::default();
        assert!(!c.spotify.enabled && c.spotify.client_id.is_empty());
        let mut c = c;
        c.spotify.enabled = true;
        c.validate().unwrap();
        c.spotify.client_id = "not a client id!".into();
        assert!(c
            .validate()
            .unwrap_err()
            .to_string()
            .contains("spotify.client_id"));
        c.spotify.client_id = "0123456789abcdef0123456789ABCDEF".into();
        c.validate().unwrap();
        let yaml = "spotify: { enabled: true, client_id: abc123 }\n";
        let c = Config::from_yaml(yaml).unwrap();
        assert!(c.spotify.enabled && c.spotify.client_id == "abc123");
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
    }

    #[test]
    fn web_block_defaults_round_trips_and_validates() {
        let c = Config::default();
        assert!(c.web.enabled);
        assert_eq!((c.web.bind.as_str(), c.web.port), ("0.0.0.0", 8740));
        assert!(c.web.password.is_empty());
        c.validate().unwrap();
        let c = Config::from_yaml(
            "web: { port: 9000, password: hunter2, bind: 127.0.0.1 }\nscreens:\n  - { roles: [cpu], spi: 0, cs: 0, dc: 6, rst: 5 }\n",
        )
        .unwrap();
        assert_eq!((c.web.port, c.web.password.as_str()), (9000, "hunter2"));
        assert!(c.web.enabled, "enabled unless the file says otherwise");
        c.validate().unwrap();
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
        let mut bad = c.clone();
        bad.web.bind = "everywhere".into();
        assert!(bad.validate().unwrap_err().to_string().contains("web.bind"));
        bad.web.bind = "::".into();
        bad.web.port = 0;
        assert!(bad.validate().unwrap_err().to_string().contains("web.port"));
        // an old file without the block gets the defaults
        let c = Config::from_yaml("night: { enabled: false }\n").unwrap();
        assert_eq!(c.web, WebCfg::default());
    }

    #[test]
    fn shows_defaults_and_validation() {
        let c = Config::default();
        assert!(c.shows.enabled);
        assert_eq!((c.shows.every_mins, c.shows.gap_mm), (15, 25.0));
        let all: Vec<String> = rackscreen_core::shows::SHOWS
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(c.shows.list, all, "every show by default");
        assert_eq!(c.shows.order, None);
        c.validate().unwrap();
        assert_eq!(c.shows.defs().len(), all.len());
        assert_eq!(c.shows.screen_order(4), vec![0, 1, 2, 3]);

        let err = |c: &Config| c.validate().unwrap_err().to_string();
        let mut bad = c.clone();
        bad.shows.every_mins = 1;
        assert!(err(&bad).contains("shows.every_mins"), "{}", err(&bad));
        bad.shows.every_mins = 2;
        bad.validate().unwrap();
        for gap in [-0.1, 200.1, f32::NAN, f32::INFINITY] {
            bad.shows.gap_mm = gap;
            assert!(err(&bad).contains("shows.gap_mm"), "{gap}: {}", err(&bad));
        }
        for gap in [0.0, 200.0] {
            bad.shows.gap_mm = gap;
            bad.validate().unwrap();
        }
        bad.shows.gap_mm = 25.0;
        bad.shows.list = vec!["freight".into(), "nope".into()];
        assert!(
            err(&bad).contains("nope"),
            "names the unknown id: {}",
            err(&bad)
        );
        bad.shows.list = vec!["freight".into()];
        for order in [vec![0, 1, 2], vec![0, 1, 2, 2], vec![0, 1, 2, 4]] {
            bad.shows.order = Some(order.clone());
            assert!(
                err(&bad).contains("shows.order"),
                "{order:?}: {}",
                err(&bad)
            );
        }
        bad.shows.order = Some(vec![2, 0, 1, 3]);
        bad.validate().unwrap();
        assert_eq!(bad.shows.screen_order(4), vec![2, 0, 1, 3]);

        let c = Config::from_yaml("shows: { every_mins: 2, order: [3, 2, 1, 0] }\n").unwrap();
        assert_eq!((c.shows.every_mins, c.shows.gap_mm), (2, 25.0));
        assert!(c.shows.enabled && c.shows.list == all);
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
        // the default list is not written out, so new shows reach old configs
        assert!(!Config::default().to_yaml().unwrap().contains("list:"));
    }

    const GIF_YAML: &str = "gifs:\n  - { name: party, url: \"https://giphy.com/gifs/some-slug-AbCdEf123\" }\n  - { name: nod, url: \"https://tenor.com/view/some-slug-12345678\" }\nscreens:\n  - { roles: [cpu, \"gif:party\", mem], cycle_secs: 10, spi: 0, cs: 0, dc: 6, rst: 5 }\n  - { roles: [\"gif:nod\"], spi: 0, cs: 1, dc: 13, rst: 26 }\n  - { roles: [\"gif:party\"], spi: 1, cs: 0, dc: 23, rst: 22 }\n";

    #[test]
    fn gif_roles_resolve_against_the_gifs_list() {
        let c = Config::from_yaml(GIF_YAML).unwrap();
        c.validate().unwrap();
        assert_eq!(c.gifs.len(), 2);
        assert_eq!(c.gifs[1].name, "nod");
        assert_eq!(
            c.screen_roles(&c.screens[0]).unwrap(),
            vec![Role::Cpu, Role::Gif(0), Role::Mem]
        );
        assert_eq!(c.screen_roles(&c.screens[1]).unwrap(), vec![Role::Gif(1)]);
        assert_eq!(c.role_name(Role::Gif(1)), "gif:nod");
        assert_eq!(c.role_name(Role::Cpu), "cpu");
        assert_eq!(c.role_name(Role::Gif(9)), "gif:?");
        assert!(
            c.screens[0].roles().is_err(),
            "without the gifs list a gif entry is unknown"
        );
        let mut pick = Role::ALL.to_vec();
        pick.extend([Role::Gif(0), Role::Gif(1)]);
        assert_eq!(c.pickable_roles(), pick);
        // each used GIF once, in list order
        assert_eq!(
            c.used_gifs(),
            vec![
                (
                    0,
                    "party".to_string(),
                    "https://giphy.com/gifs/some-slug-AbCdEf123".to_string()
                ),
                (
                    1,
                    "nod".to_string(),
                    "https://tenor.com/view/some-slug-12345678".to_string()
                ),
            ]
        );
        assert_eq!(Config::from_yaml(&c.to_yaml().unwrap()).unwrap(), c);
        assert!(
            !Config::default().to_yaml().unwrap().contains("gifs"),
            "no gifs: block until one is added"
        );
    }

    #[test]
    fn gif_config_errors_name_the_entry() {
        let err = |c: &Config| format!("{:#}", c.validate().unwrap_err());
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.screens[1].roles = vec!["gif:dance".into()];
        let e = err(&c);
        assert!(e.contains("gif:dance"), "{e}");
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.gifs[1].name = "Nod Nod".into();
        c.screens[1].roles = vec!["cpu".into()];
        assert!(err(&c).contains("Nod Nod"), "{}", err(&c));
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.gifs[1].name = "party".into();
        assert!(err(&c).contains("twice"), "{}", err(&c));
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.gifs[0].url = "https://example.com/a.png".into();
        let e = err(&c);
        assert!(e.contains("party") && e.contains("example.com"), "{e}");
        // an unused GIF is still checked, and fine when valid
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.screens[1].roles = vec!["cpu".into()];
        c.validate().unwrap();
        assert_eq!(c.used_gifs().len(), 1);
    }

    #[test]
    fn two_gifs_with_the_same_link_are_refused_naming_both() {
        let mut c = Config::from_yaml(GIF_YAML).unwrap();
        c.gifs[1].url = format!(" {} ", c.gifs[0].url);
        let e = format!("{:#}", c.validate().unwrap_err());
        assert!(
            e.contains("'party'") && e.contains("'nod'") && e.contains("same link"),
            "{e}"
        );
    }

    #[test]
    fn gif_search_defaults_to_no_keys_and_rating_g() {
        let c = Config::from_yaml(GIF_YAML).unwrap();
        assert_eq!(c.gif_search, GifSearchCfg::default());
        assert_eq!(c.gif_search.rating, "g");
        let mut c = Config::default();
        for r in GIF_RATINGS {
            c.gif_search.rating = r.into();
            assert!(c.validate().is_ok(), "{r}");
        }
        c.gif_search.rating = "nc-17".into();
        let e = format!("{:#}", c.validate().unwrap_err());
        assert!(e.contains("gif_search.rating"), "{e}");
    }

    #[test]
    fn gif_cache_dir_follows_xdg_cache_home() {
        let home = Path::new("/home/u");
        assert_eq!(
            gif_cache_dir_with(Some("/var/cache/x"), home),
            PathBuf::from("/var/cache/x/rackscreen/gifs")
        );
        for unset in [None, Some(""), Some("relative/path")] {
            assert_eq!(
                gif_cache_dir_with(unset, home),
                PathBuf::from("/home/u/.cache/rackscreen/gifs"),
                "{unset:?}"
            );
        }
    }
}

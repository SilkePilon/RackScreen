//! YAML configuration with defaults matching config.example.yaml.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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
    pub price: PriceCfg,
    #[serde(default)]
    pub screens: Vec<ScreenCfg>,
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
    /// Let only one screen run its iris transition at a time, in random order.
    pub one_at_a_time: bool,
}

/// Electricity Maps: grid mix, carbon intensity and renewable share.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ElectricityCfg {
    pub enabled: bool,
    pub zone: String,
    pub token: String,
    pub poll_secs: u64,
}

/// Day-ahead electricity price.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PriceCfg {
    /// `energyzero`, `entsoe` or `none`.
    pub source: String,
    pub entsoe_token: String,
    pub entsoe_zone: String,
    pub include_vat: bool,
    pub poll_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ScreenCfg {
    /// Legacy single role; upgraded into `roles` by `Config::normalize`.
    #[serde(default, skip_serializing)]
    pub role: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
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

fn default_hz() -> u32 {
    40_000_000
}

fn default_cycle() -> u64 {
    15
}

impl ScreenCfg {
    /// Every configured role, in cycling order. At least one.
    pub fn roles(&self) -> Result<Vec<Role>> {
        anyhow::ensure!(!self.roles.is_empty(), "screen has no roles");
        self.roles
            .iter()
            .map(|r| Role::parse(r).with_context(|| format!("unknown screen role '{r}'")))
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
impl Default for PriceCfg {
    fn default() -> Self {
        Self {
            source: "energyzero".into(),
            entsoe_token: String::new(),
            entsoe_zone: String::new(),
            include_vat: true,
            poll_secs: 900,
        }
    }
}
impl Default for DisplayCfg {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            fps: 30,
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
fn sudo_user() -> Option<nix::unistd::User> {
    if !nix::unistd::geteuid().is_root() {
        return None;
    }
    let name = std::env::var("SUDO_USER").ok().filter(|n| !n.is_empty())?;
    nix::unistd::User::from_name(&name).ok().flatten()
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

impl Config {
    pub fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_PATH)
    }

    pub fn from_yaml(text: &str) -> Result<Config> {
        let mut cfg: Config = serde_yaml_ng::from_str(text).context("parse yaml")?;
        cfg.normalize();
        Ok(cfg)
    }

    /// Upgrade legacy fields in place.
    pub fn normalize(&mut self) {
        for s in &mut self.screens {
            if let Some(r) = s.role.take() {
                if s.roles.is_empty() {
                    s.roles = vec![r];
                }
            }
        }
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
            "# RackScreen configuration (written by rackscreen setup)\n{}",
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

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.screens.is_empty(),
            "config needs at least one [[screens]] entry"
        );
        for s in &self.screens {
            let roles = s.roles()?;
            let name = roles[0].name();
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
        }
        anyhow::ensure!(
            matches!(self.price.source.as_str(), "energyzero" | "entsoe" | "none"),
            "price.source must be energyzero, entsoe or none (got '{}')",
            self.price.source
        );
        anyhow::ensure!(
            !self.electricity.enabled || !self.electricity.zone.trim().is_empty(),
            "electricity.zone must be set when electricity is enabled"
        );
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
        assert_eq!(c.screens[2].hz, 16_000_000);
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
        assert_eq!(c.screens[0].hz, 40_000_000);
        assert_eq!(c.screens[0].rotate, 0);
        assert_eq!(c.display.fps, 30);
        assert!(c.display.one_at_a_time, "on unless the file says otherwise");
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
        assert_eq!(c.screens[0].roles, vec!["cpu".to_string()]);
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
    fn unknown_role_in_list_rejected() {
        let c =
            Config::from_yaml("screens:\n  - { roles: [nope], spi: 0, cs: 0, dc: 6, rst: 5 }\n")
                .unwrap();
        let err = format!("{:#}", c.validate().unwrap_err());
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn empty_role_list_rejected() {
        let c = Config::from_yaml("screens:\n  - { roles: [], spi: 0, cs: 0, dc: 6, rst: 5 }\n")
            .unwrap();
        assert!(c.validate().is_err());
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
    fn bad_price_source_and_zoneless_electricity_rejected() {
        let mut c = Config::default();
        c.price.source = "foo".into();
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("price.source") && err.contains("foo"), "{err}");

        let mut c = Config::default();
        c.electricity.enabled = true;
        c.electricity.zone = "  ".into();
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("electricity.zone"), "{err}");
        c.electricity.zone = "NL".into();
        c.validate().unwrap();
    }

    #[test]
    fn electricity_price_and_threshold_defaults() {
        let c = Config::default();
        assert!(!c.electricity.enabled);
        assert_eq!(c.electricity.zone, "NL");
        assert_eq!(c.electricity.poll_secs, 300);
        assert_eq!(c.price.source, "energyzero");
        assert!(c.price.include_vat);
        assert_eq!(c.price.poll_secs, 900);
        assert_eq!(c.thresholds.hot_temp, 70.0);
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
}

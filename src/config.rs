//! TOML configuration with defaults matching config.example.toml.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rackscreen_core::theme::Role;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
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
    pub screens: Vec<ScreenCfg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct K8sCfg {
    pub kubeconfig: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PromCfg {
    pub namespace: String,
    pub service: String,
    pub port: u16,
    pub poll_secs: u64,
    pub ignore_alerts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NightCfg {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThresholdsCfg {
    pub hot_cpu: f32,
    pub hot_mem: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DisplayCfg {
    pub brightness: f32,
    pub fps: u32,
    pub spi_chunk: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScreenCfg {
    pub role: String,
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

impl ScreenCfg {
    pub fn role(&self) -> Result<Role> {
        Ok(match self.role.as_str() {
            "cpu" => Role::Cpu,
            "mem" => Role::Mem,
            "pods" => Role::Pods,
            "health" => Role::Health,
            other => anyhow::bail!("unknown screen role '{other}'"),
        })
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
        }
    }
}
impl Default for DisplayCfg {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            fps: 30,
            spi_chunk: 4096,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        toml::from_str(include_str!("../config.example.toml"))
            .expect("config.example.toml is valid")
    }
}

pub fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rackscreen/config.toml")
    }

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
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        anyhow::ensure!(
            !cfg.screens.is_empty(),
            "config needs at least one [[screens]] entry"
        );
        for s in &cfg.screens {
            s.role()?;
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses_with_four_screens() {
        let c = Config::default();
        assert_eq!(c.screens.len(), 4);
        assert_eq!(c.screens[3].role().unwrap(), Role::Health);
        assert_eq!(c.screens[2].hz, 16_000_000);
        assert_eq!(c.prometheus.port, 9090);
        assert_eq!(c.prometheus.ignore_alerts, ["Watchdog", "InfoInhibitor"]);
        assert!(c.qbittorrent.enabled);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let c: Config = toml::from_str("[night]\nenabled = false\n[[screens]]\nrole = \"cpu\"\nspi = 0\ncs = 0\ndc = 6\nrst = 5\n").unwrap();
        assert!(!c.night.enabled);
        assert_eq!(c.night.start, "23:00");
        assert_eq!(c.screens[0].hz, 40_000_000);
        assert_eq!(c.display.fps, 30);
        assert_eq!(c.prometheus.ignore_alerts, ["Watchdog", "InfoInhibitor"]);
    }

    #[test]
    fn bad_role_rejected() {
        let s = ScreenCfg {
            role: "nope".into(),
            spi: 0,
            cs: 0,
            dc: 0,
            rst: 0,
            rotate: 0,
            hflip: false,
            hz: 1,
        };
        assert!(s.role().is_err());
    }

    #[test]
    fn expand_home_works() {
        assert!(expand_home("/abs").starts_with("/abs"));
        assert!(!expand_home("~/x").to_string_lossy().starts_with('~'));
    }
}

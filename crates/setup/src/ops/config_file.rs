//! Small mutations on the YAML config used by the setup screens.

use std::path::Path;

use anyhow::{Context, Result};
pub use rackscreen_app::config::Config;
use rackscreen_core::theme::Role;

use crate::ops::paths::{is_root, service_user};

/// Save the config and, when running as root, hand the file to the service user.
/// `Config::save` writes mode 0640 (it may hold the qBittorrent password), so the
/// `rackscreen@<user>` service can only read it if that user owns it.
pub fn save_config(cfg: &Config, path: &Path) -> Result<()> {
    cfg.save(path)?;
    if is_root() {
        let user = service_user();
        if let Some(u) =
            nix::unistd::User::from_name(&user).with_context(|| format!("look up user {user}"))?
        {
            nix::unistd::chown(path, Some(u.uid), Some(u.gid))
                .with_context(|| format!("chown {} to {user}", path.display()))?;
        }
    }
    Ok(())
}

pub fn set_orientation(cfg: &mut Config, index: usize, rotate: u32, hflip: bool) {
    if let Some(s) = cfg.screens.get_mut(index) {
        s.rotate = rotate;
        s.hflip = hflip;
    }
}

pub fn set_spi_chunk(cfg: &mut Config, chunk: usize) {
    cfg.display.spi_chunk = chunk;
}

/// A ready-made screen layout offered by the Screens editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Cluster,
    Electricity,
    Mixed,
    Sky,
}

impl Preset {
    /// Roles for the first four screens, in order.
    pub fn rows(self) -> Vec<Vec<Role>> {
        use Role::*;
        match self {
            Preset::Cluster => vec![vec![Cpu], vec![Mem], vec![Pods], vec![Health]],
            Preset::Electricity => vec![vec![PowerMix], vec![Price], vec![Carbon], vec![Renewable]],
            Preset::Mixed => vec![
                vec![Cpu, PowerMix],
                vec![Mem, Price],
                vec![Pods, Carbon],
                vec![Health, Renewable],
            ],
            Preset::Sky => vec![
                vec![Weather, Aqi],
                vec![Rain, Wind],
                vec![Sun, Moon],
                vec![Iss, GhActivity],
            ],
        }
    }
}

pub fn set_screen_roles(cfg: &mut Config, index: usize, roles: &[Role], cycle_secs: u64) {
    if let Some(s) = cfg.screens.get_mut(index) {
        s.roles = roles.iter().map(|r| r.name().to_string()).collect();
        s.cycle_secs = cycle_secs.clamp(3, 300);
    }
}

/// Apply a preset to the first four screens; extra screens keep their roles.
pub fn apply_preset(cfg: &mut Config, preset: Preset) {
    for (i, roles) in preset.rows().into_iter().enumerate() {
        set_screen_roles(cfg, i, &roles, 15);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_app::config::ScreenCfg;

    #[test]
    fn mutations_touch_only_their_fields() {
        let mut c = Config::default();
        let before = c.clone();
        set_orientation(&mut c, 1, 90, false);
        set_orientation(&mut c, 99, 0, true);
        set_spi_chunk(&mut c, 65536);
        assert_eq!(c.screens[1].rotate, 90);
        assert!(!c.screens[1].hflip);
        assert_eq!(c.display.spi_chunk, 65536);
        assert_eq!(c.screens[0], before.screens[0]);
        assert_eq!(c.prometheus, before.prometheus);
    }

    #[test]
    fn preset_rewrites_the_first_four_screens_only() {
        let mut c = Config::default();
        let fifth = c.screens[0].clone();
        c.screens.push(ScreenCfg {
            roles: vec!["thermal".into()],
            ..fifth
        });
        apply_preset(&mut c, Preset::Electricity);
        assert_eq!(c.screens[0].roles, vec!["power-mix".to_string()]);
        assert_eq!(c.screens[0].cycle_secs, 15);
        assert_eq!(c.screens[3].roles, vec!["renewable".to_string()]);
        assert_eq!(c.screens[4].roles, vec!["thermal".to_string()]);
        set_screen_roles(&mut c, 0, &[Role::Cpu, Role::Mem], 1000);
        assert_eq!(c.screens[0].roles, vec!["cpu".to_string(), "mem".into()]);
        assert_eq!(c.screens[0].cycle_secs, 300);
        set_screen_roles(&mut c, 99, &[Role::Cpu], 15);
    }

    #[test]
    fn save_config_writes_a_loadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("etc/rackscreen/config.yaml");
        save_config(&Config::default(), &path).unwrap();
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
    }

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
}

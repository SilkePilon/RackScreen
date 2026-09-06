//! Small mutations on the YAML config used by the setup screens.

use std::path::Path;

use anyhow::{Context, Result};
pub use rackscreen_app::config::Config;

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn save_config_writes_a_loadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("etc/rackscreen/config.yaml");
        save_config(&Config::default(), &path).unwrap();
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
    }
}

//! Put the SPI1 driver in polling mode.
//!
//! SPI1 on a Pi is the "aux" controller: no DMA and a FIFO of a few words, so
//! in its default interrupt mode it takes an interrupt every dozen bytes and
//! reaches about a third of the wire speed (2 MB/s at 50 MHz, against 6 MB/s on
//! SPI0). Two panels on SPI1 then cannot keep up with a GIF and a transition at
//! once, and both stutter. Polling keeps the FIFO full: measured on a Pi 3B+,
//! a full frame went from 46–62 ms to 25 ms. `spi_bcm2835aux` polls a transfer
//! whose estimated time is below `polling_limit_us`; the panels write chunks
//! of at most `AUX_CHUNK` bytes (see `rackscreen_display::gc9a01`), which fit
//! the limit and the driver's own two-jiffy polling window.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::paths::Paths;

/// Transfers estimated shorter than this many microseconds are polled.
pub const POLLING_LIMIT_US: u32 = 10_000;

/// The modprobe drop-in, so the setting survives a reboot.
pub fn modprobe_text() -> String {
    format!(
        "# Written by rackscreen: poll SPI1 (no DMA) instead of an interrupt per FIFO fill.\n\
         options spi_bcm2835aux polling_limit_us={POLLING_LIMIT_US}\n"
    )
}

pub fn modprobe_path(paths: &Paths) -> PathBuf {
    paths.root.join("etc/modprobe.d/rackscreen-spi.conf")
}

/// The live module parameter; present while `spi_bcm2835aux` is loaded.
pub fn live_param_path(paths: &Paths) -> PathBuf {
    paths
        .root
        .join("sys/module/spi_bcm2835aux/parameters/polling_limit_us")
}

/// What `ensure` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tuned {
    /// The modprobe drop-in was written (it was missing or different).
    pub wrote_conf: bool,
    /// The running driver was switched over (it was loaded and not set yet).
    pub set_live: bool,
}

impl Tuned {
    pub fn describe(&self) -> String {
        match (self.wrote_conf, self.set_live) {
            (false, false) => "already set".into(),
            (true, true) => format!("polling_limit_us={POLLING_LIMIT_US}, now and at boot"),
            (true, false) => format!("polling_limit_us={POLLING_LIMIT_US} from the next boot"),
            (false, true) => format!("polling_limit_us={POLLING_LIMIT_US} set on the driver"),
        }
    }
}

fn live_value(p: &Path) -> Option<u32> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// Write the modprobe drop-in and, while the driver is loaded, raise its live
/// parameter. Needs root. A live value already at or above the limit is kept.
pub fn ensure(paths: &Paths) -> Result<Tuned> {
    let conf = modprobe_path(paths);
    let want = modprobe_text();
    let wrote_conf = std::fs::read_to_string(&conf).ok().as_deref() != Some(want.as_str());
    if wrote_conf {
        if let Some(dir) = conf.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        std::fs::write(&conf, &want).with_context(|| format!("write {}", conf.display()))?;
    }
    let live = live_param_path(paths);
    let set_live = match live_value(&live) {
        Some(v) if v < POLLING_LIMIT_US => {
            std::fs::write(&live, format!("{POLLING_LIMIT_US}\n"))
                .with_context(|| format!("write {}", live.display()))?;
            true
        }
        _ => false,
    };
    Ok(Tuned {
        wrote_conf,
        set_live,
    })
}

/// Remove the modprobe drop-in (uninstall). Returns whether it was there.
pub fn remove(paths: &Paths) -> Result<bool> {
    let conf = modprobe_path(paths);
    match std::fs::remove_file(&conf) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("remove {}", conf.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_dropin_and_raises_the_live_value_once() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let live = live_param_path(&paths);
        std::fs::create_dir_all(live.parent().unwrap()).unwrap();
        std::fs::write(&live, "30\n").unwrap();

        let t = ensure(&paths).unwrap();
        assert_eq!(
            t,
            Tuned {
                wrote_conf: true,
                set_live: true
            }
        );
        assert_eq!(
            std::fs::read_to_string(modprobe_path(&paths)).unwrap(),
            modprobe_text()
        );
        assert!(modprobe_text().contains("options spi_bcm2835aux polling_limit_us=10000\n"));
        assert_eq!(live_value(&live), Some(POLLING_LIMIT_US));

        let again = ensure(&paths).unwrap();
        assert_eq!(
            again,
            Tuned {
                wrote_conf: false,
                set_live: false
            }
        );
        assert_eq!(again.describe(), "already set");
    }

    #[test]
    fn without_the_driver_only_the_dropin_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let t = ensure(&paths).unwrap();
        assert!(t.wrote_conf && !t.set_live);
        assert!(!live_param_path(&paths).exists());
        assert!(t.describe().contains("next boot"));
    }

    #[test]
    fn a_higher_live_value_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let live = live_param_path(&paths);
        std::fs::create_dir_all(live.parent().unwrap()).unwrap();
        std::fs::write(&live, "50000\n").unwrap();
        assert!(!ensure(&paths).unwrap().set_live);
        assert_eq!(live_value(&live), Some(50000));
    }

    #[test]
    fn remove_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        assert!(!remove(&paths).unwrap());
        ensure(&paths).unwrap();
        assert!(remove(&paths).unwrap());
        assert!(!modprobe_path(&paths).exists());
    }
}

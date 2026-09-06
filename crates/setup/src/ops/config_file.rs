//! Small mutations on the YAML config used by the setup screens.

pub use rackscreen_app::config::Config;

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
}

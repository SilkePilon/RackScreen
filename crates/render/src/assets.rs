//! Embedded icons: the Lucide arrow (calibrate) and the energy-source set
//! (power-mix capsules). The fonts live in core.

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        pub const ICON_NAMES: &[&str] = &[$($name),*];
        pub fn icon_svg(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_bytes!(concat!("../../../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }
    };
}

icons!(
    "arrow-up",
    "em-biomass",
    "em-geothermal",
    "em-hydro",
    "em-solar",
    "em-wind",
    "em-nuclear",
    "em-battery-storage",
    "em-hydro-storage",
    "em-coal",
    "em-gas",
    "em-oil",
    "em-unknown",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_parses_as_svg() {
        for name in ICON_NAMES {
            let data = icon_svg(name).expect(name);
            let tree = usvg::Tree::from_data(data, &usvg::Options::default())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                matches!(tree.size().width() as u32, 8 | 16 | 24),
                "{name}: {}",
                tree.size().width()
            );
        }
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(icon_svg("nope").is_none());
    }
}

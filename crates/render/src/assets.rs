//! Embedded font and Lucide icons.

pub const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");

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
    "cpu",
    "memory-stick",
    "box",
    "heart-pulse",
    "download",
    "package-plus",
    "package-x",
    "package-minus",
    "flame",
    "circle-check",
    "server-off",
    "server",
    "triangle-alert",
    "shield-check",
    "plug-zap",
    "plug",
    "cloud-off",
    "euro",
    "cloud",
    "leaf",
    "thermometer",
    "database",
    "database-zap",
    "key-round",
    "zap",
    "github",
    "git-commit-horizontal",
    "git-merge",
    "star",
    "circle-x",
    "tag",
    "sun",
    "moon",
    "cloud-sun",
    "cloud-moon",
    "cloud-fog",
    "cloud-drizzle",
    "cloud-rain",
    "cloud-lightning",
    "snowflake",
    "wind",
    "haze",
    "umbrella",
    "sunset",
    "sunrise",
    "satellite",
    "battery-charging",
    "battery-warning",
    "arrow-down-up",
    "rocket",
    "map-pin",
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

    #[test]
    fn font_loads() {
        let font = fontdue::Font::from_bytes(FONT_BOLD, fontdue::FontSettings::default()).unwrap();
        let (m, _) = font.rasterize('4', 15.0);
        assert!(m.width > 0 && m.height > 0);
    }
}

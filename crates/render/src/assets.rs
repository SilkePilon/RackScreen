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
            assert_eq!(tree.size().width(), 24.0, "{name}");
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

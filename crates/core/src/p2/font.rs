//! The three embedded Outfit instances and read-only metrics, so core can lay
//! text out (centre, pill width, marquee overflow, roll cells) without drawing.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId {
    Medium,
    SemiBold,
    Bold,
}

impl FontId {
    pub const ALL: [FontId; 3] = [FontId::Medium, FontId::SemiBold, FontId::Bold];

    pub fn index(self) -> usize {
        match self {
            FontId::Medium => 0,
            FontId::SemiBold => 1,
            FontId::Bold => 2,
        }
    }
}

pub const OUTFIT_MEDIUM: &[u8] = include_bytes!("../../../../assets/fonts/Outfit-Medium.ttf");
pub const OUTFIT_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/Outfit-SemiBold.ttf");
pub const OUTFIT_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Outfit-Bold.ttf");

/// The TTF bytes for a face; `render` rasterises from these, nothing is embedded twice.
pub fn bytes(id: FontId) -> &'static [u8] {
    match id {
        FontId::Medium => OUTFIT_MEDIUM,
        FontId::SemiBold => OUTFIT_SEMIBOLD,
        FontId::Bold => OUTFIT_BOLD,
    }
}

fn fonts() -> &'static [Font; 3] {
    static FONTS: OnceLock<[Font; 3]> = OnceLock::new();
    FONTS.get_or_init(|| {
        let load = |b: &'static [u8]| {
            Font::from_bytes(b, FontSettings::default()).expect("embedded Outfit parses")
        };
        [
            load(OUTFIT_MEDIUM),
            load(OUTFIT_SEMIBOLD),
            load(OUTFIT_BOLD),
        ]
    })
}

/// The parsed face, for metrics only (drawing is `render`'s job).
pub fn font(id: FontId) -> &'static Font {
    &fonts()[id.index()]
}

/// Advance width of one character in px.
pub fn advance(id: FontId, px: f32, ch: char) -> f32 {
    font(id).metrics(ch, px).advance_width
}

/// Width of a run: advances plus `letter_spacing` between characters.
pub fn measure(id: FontId, px: f32, text: &str, letter_spacing: f32) -> f32 {
    let n = text.chars().count();
    if n == 0 {
        return 0.0;
    }
    let adv: f32 = text.chars().map(|c| advance(id, px, c)).sum();
    adv + letter_spacing * (n as f32 - 1.0)
}

/// The tabular digit cell: as wide as `0`, Outfit's widest digit.
pub fn digit_width(id: FontId, px: f32) -> f32 {
    advance(id, px, '0')
}

/// Height of `H` above the baseline in px.
pub fn cap_height(id: FontId, px: f32) -> f32 {
    let m = font(id).metrics('H', px);
    m.height as f32 + m.ymin as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outfit_metrics_are_sane() {
        assert!((digit_width(FontId::Bold, 64.0) - advance(FontId::Bold, 64.0, '0')).abs() < 1e-6);
        let w = measure(FontId::Bold, 64.0, "42", 0.0);
        assert!(w > 60.0 && w < 90.0, "{w}");
        let spaced =
            measure(FontId::Bold, 10.0, "CPU", 1.5) - measure(FontId::Bold, 10.0, "CPU", 0.0);
        assert!((spaced - 3.0).abs() < 1e-4, "two gaps of 1.5: {spaced}");
        let cap = cap_height(FontId::Bold, 64.0);
        assert!(
            cap > 40.0 && cap < 50.0,
            "Outfit cap height is 0.7 em: {cap}"
        );
        assert!(advance(FontId::Bold, 20.0, 'M') > advance(FontId::Medium, 20.0, 'M'));
        assert_eq!(measure(FontId::Medium, 10.0, "", 1.0), 0.0);
        for id in FontId::ALL {
            for ch in ['€', '°', '↓', '↑', '→', '·', '—', '–', '×', '±', '%', '/'] {
                assert!(font(id).has_glyph(ch), "{id:?} lacks {ch}");
            }
            assert!(!bytes(id).is_empty());
        }
    }

    #[test]
    fn outfit_digits_share_one_width_per_size() {
        for id in FontId::ALL {
            let w0 = digit_width(id, 64.0);
            for d in '0'..='9' {
                let w = measure(id, 64.0, &d.to_string(), 0.0);
                assert!(
                    w <= w0 + 0.5,
                    "{id:?} digit {d} ({w}) wider than the cell ({w0})"
                );
            }
        }
    }
}

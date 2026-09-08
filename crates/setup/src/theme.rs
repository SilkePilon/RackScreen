//! Colours and glyphs. Same palette as the panels; ASCII fallback for non-UTF-8 locales.

use rackscreen_core::theme as pal;
use ratatui::style::{Color, Modifier, Style};

pub struct Glyphs {
    pub pointer: &'static str,
    pub pending: &'static str,
    pub done: &'static str,
    pub failed: &'static str,
    pub warn: &'static str,
    pub dot: &'static str,
    pub arrow_up: &'static str,
    pub spinner: &'static [&'static str],
    pub ring: &'static [&'static str],
}

const UNICODE: Glyphs = Glyphs {
    pointer: "▸",
    pending: "○",
    done: "✓",
    failed: "✗",
    warn: "!",
    dot: "●",
    arrow_up: "↑",
    spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    ring: &["◐", "◓", "◑", "◒"],
};

const ASCII: Glyphs = Glyphs {
    pointer: ">",
    pending: "-",
    done: "OK",
    failed: "X",
    warn: "!",
    dot: "*",
    arrow_up: "^",
    spinner: &["|", "/", "-", "\\"],
    ring: &["(", "^", ")", "v"],
};

#[derive(Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub violet: Color,
    pub blue: Color,
    pub ok: Color,
    pub err: Color,
    pub warn: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    /// Background of the focused row.
    pub highlight: Color,
    pub unicode: bool,
    /// The terminal accepts 24-bit colour, so the half-block preview is usable.
    pub truecolor: bool,
}

fn rgb(c: pal::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

fn to_pal(c: Color) -> Option<pal::Color> {
    match c {
        Color::Rgb(r, g, b) => Some(pal::Color::rgb(r, g, b)),
        _ => None,
    }
}

impl Theme {
    pub fn detect() -> Theme {
        let lang = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        let unicode = lang.to_ascii_uppercase().contains("UTF-8")
            || lang.to_ascii_uppercase().contains("UTF8");
        let mut th = Theme::new(unicode);
        th.truecolor = Theme::truecolor_from(&std::env::var("COLORTERM").unwrap_or_default());
        th
    }

    /// `COLORTERM` advertises 24-bit colour as `truecolor` or `24bit`.
    pub fn truecolor_from(colorterm: &str) -> bool {
        let c = colorterm.to_ascii_lowercase();
        c.contains("truecolor") || c.contains("24bit")
    }

    pub fn new(unicode: bool) -> Theme {
        Theme {
            accent: rgb(pal::AMBER),
            violet: rgb(pal::VIOLET),
            blue: rgb(pal::BLUE),
            ok: rgb(pal::GREEN),
            err: rgb(pal::RED),
            warn: rgb(pal::AMBER),
            text: Color::Rgb(0xee, 0xee, 0xee),
            dim: rgb(pal::GREY),
            faint: Color::Rgb(0x44, 0x44, 0x44),
            highlight: Color::Rgb(0x1b, 0x17, 0x0e),
            unicode,
            truecolor: true,
        }
    }

    /// Blend two colours; non-RGB colours (Reset, indexed) snap at the midpoint.
    pub fn mix(&self, from: Color, to: Color, t: f32) -> Color {
        match (to_pal(from), to_pal(to)) {
            (Some(a), Some(b)) => rgb(a.mix(b, t)),
            _ => {
                if t < 0.5 {
                    from
                } else {
                    to
                }
            }
        }
    }

    /// Accent for physical screen `index`: amber, green, blue, violet, repeating.
    pub fn panel_color(&self, index: usize) -> Color {
        [self.accent, self.ok, self.blue, self.violet][index % 4]
    }

    pub fn glyphs(&self) -> &'static Glyphs {
        if self.unicode {
            &UNICODE
        } else {
            &ASCII
        }
    }

    pub fn title(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    pub fn normal(&self) -> Style {
        Style::new().fg(self.text)
    }
    pub fn muted(&self) -> Style {
        Style::new().fg(self.dim)
    }
    pub fn faint_style(&self) -> Style {
        Style::new().fg(self.faint)
    }
    pub fn selected(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    /// The focused row's background only; the text keeps its own foreground style.
    pub fn highlighted(&self) -> Style {
        Style::new().bg(self.highlight)
    }
    pub fn good(&self) -> Style {
        Style::new().fg(self.ok)
    }
    pub fn bad(&self) -> Style {
        Style::new().fg(self.err)
    }
    pub fn warning(&self) -> Style {
        Style::new().fg(self.warn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_sets_switch() {
        assert_eq!(Theme::new(true).glyphs().done, "✓");
        assert_eq!(Theme::new(false).glyphs().done, "OK");
    }

    #[test]
    fn mix_blends_rgb_and_snaps_other_colours() {
        let th = Theme::new(true);
        let black = Color::Rgb(0, 0, 0);
        let white = Color::Rgb(255, 255, 255);
        assert_eq!(th.mix(black, white, 0.0), black);
        assert_eq!(th.mix(black, white, 1.0), white);
        assert_eq!(th.mix(black, white, 0.5), Color::Rgb(128, 128, 128));
        // non-RGB colours cannot be blended: snap at the midpoint
        assert_eq!(th.mix(Color::Reset, white, 0.4), Color::Reset);
        assert_eq!(th.mix(Color::Reset, white, 0.6), white);
    }

    #[test]
    fn panel_colours_cycle_amber_green_blue_violet() {
        let th = Theme::new(true);
        assert_eq!(th.panel_color(0), th.accent);
        assert_eq!(th.panel_color(1), th.ok);
        assert_eq!(th.panel_color(2), th.blue);
        assert_eq!(th.panel_color(3), th.violet);
        assert_eq!(th.panel_color(4), th.accent);
    }

    #[test]
    fn truecolor_comes_from_colorterm() {
        assert!(Theme::truecolor_from("truecolor"));
        assert!(Theme::truecolor_from("24bit"));
        assert!(!Theme::truecolor_from(""));
        assert!(!Theme::truecolor_from("xterm-256color"));
    }
}

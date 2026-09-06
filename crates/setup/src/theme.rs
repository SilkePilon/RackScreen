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
    pub unicode: bool,
}

fn rgb(c: pal::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

impl Theme {
    pub fn detect() -> Theme {
        let lang = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        Theme::new(
            lang.to_ascii_uppercase().contains("UTF-8")
                || lang.to_ascii_uppercase().contains("UTF8"),
        )
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
            unicode,
        }
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
}

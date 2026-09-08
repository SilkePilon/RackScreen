//! Shared drawing helpers: shell chrome (header, sidebar, footer), help line, half-block
//! pixel preview, step list, confirm dialog.

use rackscreen_core::anim::Secs;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph};
use ratatui::Frame;
use tiny_skia::Pixmap;

use crate::anim::{pulse, ring_glyph, spinner_frame};
use crate::theme::Theme;

/// How the header colours a screen's status word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Ok,
    Warn,
}

/// One row of `─` across `area`.
pub fn rule(f: &mut Frame, area: Rect, th: &Theme) {
    if area.height == 0 {
        return;
    }
    let ch = if th.unicode { "─" } else { "-" };
    let line = Line::from(Span::styled(
        ch.repeat(area.width as usize),
        th.faint_style(),
    ));
    f.render_widget(Paragraph::new(line), Rect { height: 1, ..area });
}

/// Two rows: `◐ RackScreen › crumb` with the status word or version at the right, then a rule.
pub fn header(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    now: Secs,
    version: &str,
    crumb: Option<&str>,
    status: Option<(&str, StatusTone)>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let g = th.glyphs();
    let mut left = vec![
        Span::raw(" "),
        Span::styled(ring_glyph(g.ring, now).to_string(), th.title()),
        Span::styled(" RackScreen", th.title()),
    ];
    if let Some(c) = crumb {
        left.push(Span::styled(format!(" › {c}"), th.muted()));
    }
    let right = match status {
        Some((s, StatusTone::Warn)) => Line::from(Span::styled(
            format!("{} {s} ", g.dot),
            Style::new().fg(th.mix(th.accent, th.dim, pulse(now) * 0.6)),
        )),
        Some((s, StatusTone::Ok)) => Line::from(Span::styled(format!("{} {s} ", g.dot), th.good())),
        None => Line::from(Span::styled(format!("v{version} "), th.muted())),
    };
    let top = Rect { height: 1, ..area };
    f.render_widget(Paragraph::new(Line::from(left)), top);
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), top);
    if area.height > 1 {
        rule(
            f,
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
            th,
        );
    }
}

/// Two rows: a rule, then the key hints.
pub fn footer(f: &mut Frame, area: Rect, th: &Theme, keys: &str) {
    if area.height == 0 {
        return;
    }
    rule(f, area, th);
    if area.height > 1 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {keys}"), th.muted()))),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }
}

pub struct SidebarItem {
    pub label: String,
    /// Drawn at the right edge of the row (the update arrow).
    pub hint: Option<(String, Style)>,
}

/// What a screen wants in the sidebar instead of the main menu (Configure's groups).
pub struct SidebarView {
    pub title: String,
    pub items: Vec<String>,
    pub selected: usize,
}

/// The 16-column navigation column. With `focused`, the row nearest `bar_pos` gets the
/// highlight bar and pointer; otherwise `current` gets the pointer without a bar.
#[allow(clippy::too_many_arguments)]
pub fn sidebar(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    title: Option<&str>,
    items: &[SidebarItem],
    bar_pos: f32,
    current: Option<usize>,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let g = th.glyphs();
    let mut lines = Vec::new();
    match title {
        Some(t) => lines.push(Line::from(Span::styled(format!("  {t}"), th.muted()))),
        None => lines.push(Line::from("")),
    }
    let width = area.width as usize;
    for (i, item) in items.iter().enumerate() {
        let hot = if focused {
            (bar_pos - i as f32).abs() < 0.5
        } else {
            current == Some(i)
        };
        let pointer = if hot { g.pointer } else { " " };
        let label_style = if hot {
            th.selected()
        } else if current == Some(i) {
            th.normal()
        } else {
            th.muted()
        };
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(format!("{pointer} "), th.selected()),
            Span::styled(item.label.clone(), label_style),
        ];
        let used = 3 + item.label.chars().count();
        if let Some((h, style)) = &item.hint {
            let hw = h.chars().count() + 1;
            if used + hw <= width {
                spans.push(Span::raw(" ".repeat(width - used - hw)));
                spans.push(Span::styled(h.clone(), *style));
            }
        }
        let mut line = Line::from(spans);
        if hot && focused {
            line = line.style(th.highlighted());
        }
        lines.push(line);
    }
    f.render_widget(Paragraph::new(lines), area);
}

/// `ⓘ text` in the last row of the pane.
pub fn help_line(f: &mut Frame, area: Rect, th: &Theme, text: &str) {
    if area.height == 0 {
        return;
    }
    let mark = if th.unicode { "ⓘ" } else { "i" };
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(mark, Style::new().fg(th.blue)),
        Span::styled(format!(" {text}"), th.muted()),
    ]);
    f.render_widget(Paragraph::new(line), Rect { height: 1, ..area });
}

/// Near-black is drawn as terminal background so the round panel keeps its shape.
fn sample(px: &Pixmap, x: u32, y: u32) -> Option<Color> {
    let w = px.width();
    if x >= w || y >= px.height() {
        return None;
    }
    let d = px.data();
    let i = ((y * w + x) * 4) as usize;
    let (r, g, b) = (d[i], d[i + 1], d[i + 2]);
    if r < 8 && g < 8 && b < 8 {
        None
    } else {
        Some(Color::Rgb(r, g, b))
    }
}

/// Paint a square pixmap into `area` with `▀`/`▄` half blocks, two pixels per cell,
/// centred, keeping a 1:1 aspect (one cell is about twice as tall as wide).
pub fn pixels(f: &mut Frame, area: Rect, px: &Pixmap) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let cols = area.width.min(area.height.saturating_mul(2)).max(1);
    let rows = (cols / 2).max(1);
    let x0 = area.x + (area.width - cols.min(area.width)) / 2;
    let y0 = area.y + (area.height - rows.min(area.height)) / 2;
    let step = px.width() as f32 / cols as f32;
    let buf = f.buffer_mut();
    for cy in 0..rows.min(area.height) {
        for cx in 0..cols.min(area.width) {
            let sx = ((cx as f32 + 0.5) * step) as u32;
            let top = sample(px, sx, ((cy as f32 * 2.0 + 0.5) * step) as u32);
            let bottom = sample(px, sx, ((cy as f32 * 2.0 + 1.5) * step) as u32);
            let Some(cell) = buf.cell_mut((x0 + cx, y0 + cy)) else {
                continue;
            };
            match (top, bottom) {
                (None, None) => {
                    cell.set_char(' ');
                }
                (Some(t), None) => {
                    cell.set_char('▀').set_fg(t);
                }
                (None, Some(b)) => {
                    cell.set_char('▄').set_fg(b);
                }
                (Some(t), Some(b)) => {
                    cell.set_char('▀').set_fg(t).set_bg(b);
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Done(String),
    Skipped(String),
    Warn(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct StepView {
    pub title: String,
    pub state: StepState,
}

/// Rows of steps with a state glyph, plus an eased progress bar underneath.
pub fn step_list(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    steps: &[StepView],
    progress: f32,
    now: Secs,
) {
    let g = th.glyphs();
    let [list, bar] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let mut lines = Vec::new();
    for s in steps {
        let (glyph, style, note): (String, Style, Option<&str>) = match &s.state {
            StepState::Pending => (g.pending.into(), th.faint_style(), None),
            StepState::Running => (spinner_frame(g.spinner, now).into(), th.selected(), None),
            StepState::Done(n) => (g.done.into(), th.good(), Some(n)),
            StepState::Skipped(n) => (g.done.into(), th.muted(), Some(n)),
            StepState::Warn(n) => (g.warn.into(), th.warning(), Some(n)),
            StepState::Failed(n) => (g.failed.into(), th.bad(), Some(n)),
        };
        let title_style = if matches!(s.state, StepState::Pending) {
            th.muted()
        } else {
            th.normal()
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{glyph:<3}"), style),
            Span::styled(s.title.clone(), title_style),
        ]));
        if let Some(n) = note {
            if !n.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled(n.to_string(), style),
                ]));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), list);
    let gauge = Gauge::default()
        .ratio(progress.clamp(0.0, 1.0) as f64)
        .gauge_style(Style::new().fg(th.accent).bg(th.faint))
        .label("");
    f.render_widget(
        gauge,
        Rect {
            x: bar.x + 2,
            width: bar.width.saturating_sub(4),
            ..bar
        },
    );
}

/// Centred modal with a coloured border. `lines` are the body; the last footer line lists keys.
pub fn confirm_dialog(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    title: &str,
    lines: &[String],
    keys: &str,
    danger: bool,
) {
    let height = (lines.len() as u16 + 4).min(area.height);
    // clamp's min must not exceed its max: terminals narrower than 30 columns are legal
    let width = (lines.iter().map(|l| l.len()).max().unwrap_or(20) as u16 + 6)
        .clamp(30.min(area.width), area.width);
    let [dialog] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [dialog] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(dialog);
    f.render_widget(Clear, dialog);
    let border = if danger { th.bad() } else { th.selected() };
    let block = Block::bordered()
        .border_style(border)
        .title(Line::from(Span::styled(format!(" {title} "), border)))
        .title_bottom(
            Line::from(Span::styled(format!(" {keys} "), th.muted())).alignment(Alignment::Right),
        );
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);
    let body: Vec<Line> = lines
        .iter()
        .map(|l| Line::from(Span::styled(format!(" {l}"), th.normal())))
        .collect();
    f.render_widget(
        Paragraph::new(body),
        Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use rackscreen_render::frame::new_pixmap;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// ratatui 0.30's `TestBackend` renders each row quoted; strip the quotes so the
    /// assertions can look at the drawn text itself.
    fn text(term: &Terminal<TestBackend>) -> String {
        term.backend()
            .to_string()
            .lines()
            .map(|l| l.trim_start_matches('"').trim_end_matches('"'))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every widget must survive terminals smaller than its natural size.
    #[test]
    fn narrow_terminal_does_not_panic() {
        let th = Theme::new(true);
        let steps = [
            StepView {
                title: "install".into(),
                state: StepState::Running,
            },
            StepView {
                title: "enable".into(),
                state: StepState::Failed("boom".into()),
            },
        ];
        let items = [
            SidebarItem {
                label: "Home".into(),
                hint: None,
            },
            SidebarItem {
                label: "Update".into(),
                hint: Some(("↑".into(), th.warning())),
            },
        ];
        let px = new_pixmap();
        for (w, h) in [(1u16, 1u16), (4, 2), (12, 3), (40, 10), (80, 24)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                let a = f.area();
                step_list(f, a, &th, &steps, 0.5, 0.2);
                confirm_dialog(f, a, &th, "Uninstall", &["remove?".into()], "y/n", true);
                header(
                    f,
                    a,
                    &th,
                    0.0,
                    "0.5.0",
                    Some("Configure"),
                    Some(("unsaved", StatusTone::Warn)),
                );
                footer(f, a, &th, "q quit");
                sidebar(f, a, &th, Some("Configure"), &items, 0.5, Some(1), true);
                help_line(f, a, &th, "help");
                pixels(f, a, &px);
                rule(f, a, &th);
            })
            .unwrap();
        }
    }

    #[test]
    fn header_shows_crumb_status_or_version() {
        let th = Theme::new(true);
        let mut term = Terminal::new(TestBackend::new(60, 2)).unwrap();
        term.draw(|f| header(f, f.area(), &th, 0.0, "0.5.0", None, None))
            .unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen"));
        assert!(t.contains("v0.5.0"));
        assert!(!t.contains("›"));
        term.draw(|f| {
            header(
                f,
                f.area(),
                &th,
                0.0,
                "0.5.0",
                Some("Configure"),
                Some(("unsaved", StatusTone::Warn)),
            )
        })
        .unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen › Configure"));
        assert!(t.contains("● unsaved"));
        assert!(!t.contains("v0.5.0"), "status replaces the version");
    }

    #[test]
    fn sidebar_marks_hover_when_focused_and_current_otherwise() {
        let th = Theme::new(true);
        let items: Vec<SidebarItem> = ["Home", "Screens", "Configure"]
            .iter()
            .map(|l| SidebarItem {
                label: l.to_string(),
                hint: None,
            })
            .collect();
        let mut term = Terminal::new(TestBackend::new(16, 6)).unwrap();
        term.draw(|f| sidebar(f, f.area(), &th, None, &items, 1.0, None, true))
            .unwrap();
        let t = text(&term);
        assert!(t.contains("▸ Screens"), "{t}");
        assert!(!t.contains("▸ Home"));
        term.draw(|f| {
            sidebar(
                f,
                f.area(),
                &th,
                Some("Configure"),
                &items,
                0.0,
                Some(2),
                false,
            )
        })
        .unwrap();
        let t = text(&term);
        assert!(t.contains("Configure"));
        assert!(
            t.contains("▸ Configure"),
            "current item gets the pointer when unfocused: {t}"
        );
        assert!(!t.contains("▸ Home"));
    }

    #[test]
    fn sidebar_hint_sits_at_the_right_edge() {
        let th = Theme::new(true);
        let items = [SidebarItem {
            label: "Update".into(),
            hint: Some(("↑".into(), th.warning())),
        }];
        let mut term = Terminal::new(TestBackend::new(16, 3)).unwrap();
        term.draw(|f| sidebar(f, f.area(), &th, None, &items, 0.0, None, true))
            .unwrap();
        let row: String = text(&term).lines().nth(1).unwrap().to_string();
        assert!(row.trim_end().ends_with('↑'), "{row:?}");
    }

    #[test]
    fn pixels_paints_half_blocks_from_the_pixmap() {
        let mut px = new_pixmap();
        // a 16x8 red block in the top-left corner: the first cell's top half
        {
            let w = px.width() as usize;
            let d = px.data_mut();
            for y in 0..8usize {
                for x in 0..16usize {
                    let i = (y * w + x) * 4;
                    d[i] = 255;
                    d[i + 1] = 0;
                    d[i + 2] = 0;
                    d[i + 3] = 255;
                }
            }
        }
        let mut term = Terminal::new(TestBackend::new(30, 15)).unwrap();
        term.draw(|f| pixels(f, f.area(), &px)).unwrap();
        let buf = term.backend().buffer();
        let c = buf.cell((0, 0)).unwrap();
        assert_eq!(c.symbol(), "▀");
        assert_eq!(c.fg, ratatui::style::Color::Rgb(255, 0, 0));
        // black pixels are left as terminal background so the disc reads as round
        let far = buf.cell((15, 7)).unwrap();
        assert_eq!(far.symbol(), " ");
        assert_eq!(far.fg, ratatui::style::Color::Reset);
    }
}

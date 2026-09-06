//! Shared drawing helpers: header, footer, step list, confirm dialog.

use rackscreen_core::anim::Secs;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;

use crate::anim::{ring_glyph, spinner_frame};
use crate::theme::Theme;

pub fn header(f: &mut Frame, area: Rect, th: &Theme, now: Secs, version: &str, subtitle: &str) {
    let g = th.glyphs();
    let title = Line::from(vec![Span::styled(" RackScreen ".to_string(), th.title())]);
    let block = Block::new().borders(Borders::TOP).title(title).title(
        Line::from(Span::styled(format!(" v{version} "), th.muted())).alignment(Alignment::Right),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(ring_glyph(g.ring, now).to_string(), th.title()),
        Span::raw("  "),
        Span::styled(subtitle.to_string(), th.normal()),
    ]);
    f.render_widget(
        Paragraph::new(line),
        Rect {
            y: inner.y + inner.height.saturating_sub(1).min(1),
            height: 1,
            ..inner
        },
    );
}

pub fn footer(f: &mut Frame, area: Rect, th: &Theme, keys: &str) {
    let block = Block::new()
        .borders(Borders::TOP)
        .title(Line::from(Span::styled(format!(" {keys} "), th.muted())));
    f.render_widget(block, area);
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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// The dialog and step list must survive terminals smaller than their natural size.
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
        for (w, h) in [(1u16, 1u16), (4, 2), (12, 3), (40, 10)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| step_list(f, f.area(), &th, &steps, 0.5, 0.2))
                .unwrap();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                confirm_dialog(
                    f,
                    f.area(),
                    &th,
                    "Uninstall",
                    &["remove the service?".into()],
                    "y/n",
                    true,
                )
            })
            .unwrap();
        }
    }
}

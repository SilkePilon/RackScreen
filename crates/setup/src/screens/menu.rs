//! Main menu with a sliding highlight bar.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::{Action, Screen, ScreenId, Shared};

pub const ITEMS: [(ScreenId, &str, &str); 6] = [
    (ScreenId::Install, "Install", "set up service + config"),
    (
        ScreenId::Calibrate,
        "Calibrate screens",
        "fix rotation / mirroring",
    ),
    (ScreenId::Configure, "Configure", "cluster, night, display"),
    (ScreenId::Status, "Status", "service, links, logs"),
    (ScreenId::RunHere, "Run here", "foreground with logs"),
    (ScreenId::Uninstall, "Uninstall", "remove everything"),
];

pub struct Menu {
    selected: usize,
    bar: Slide,
}

impl Menu {
    pub fn new(_shared: &Shared) -> Menu {
        Menu {
            selected: 0,
            bar: Slide::fixed(0.0),
        }
    }

    fn select(&mut self, idx: usize, now: Secs) {
        self.selected = idx;
        self.bar = self.bar.to(idx as f32, now, 0.15);
    }
}

impl Screen for Menu {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, now: Secs) -> Action {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = (self.selected + ITEMS.len() - 1) % ITEMS.len();
                self.select(i, now);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                let i = (self.selected + 1) % ITEMS.len();
                self.select(i, now);
                Action::None
            }
            KeyCode::Enter => Action::Go(ITEMS[self.selected].0),
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            _ => Action::None,
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, _, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(ITEMS.len() as u16),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .areas(area);
        let bar_pos = self.bar.value(now);
        let mut lines = Vec::new();
        for (i, (_, title, desc)) in ITEMS.iter().enumerate() {
            let dist = (bar_pos - i as f32).abs();
            let hot = dist < 0.5;
            let pointer = if hot { g.pointer } else { " " };
            let title_style = if hot { th.selected() } else { th.normal() };
            let desc_style = if hot { th.muted() } else { th.faint_style() };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(format!("{pointer} "), th.selected()),
                Span::styled(format!("{title:<20}"), title_style),
                Span::styled((*desc).to_string(), desc_style),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);

        let (svc_style, svc_text) = match shared.service_active {
            Some(true) => (th.good(), "service: active"),
            Some(false) => (th.bad(), "service: inactive"),
            None => (th.muted(), "service: unknown"),
        };
        let mut foot = vec![Line::from(vec![
            Span::raw("   "),
            Span::styled(g.dot, svc_style),
            Span::styled(format!(" {svc_text}     "), th.muted()),
            Span::styled(g.dot, th.muted()),
            Span::styled(
                format!(" config: {}", shared.ctx.config_path.display()),
                th.muted(),
            ),
        ])];
        if let Some(b) = &shared.banner {
            foot.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(b.clone(), th.warning()),
            ]));
        }
        f.render_widget(Paragraph::new(foot), status);
    }

    fn keys(&self) -> String {
        "↑↓ move  ⏎ select  q quit".into()
    }
    fn subtitle(&self) -> String {
        "Kubernetes rack monitor".into()
    }
    fn animating(&self, now: Secs) -> bool {
        !self.bar.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn shared() -> Shared {
        Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: Some(true),
            banner: None,
        }
    }

    #[test]
    fn menu_renders_items_and_pointer() {
        let sh = shared();
        let menu = Menu::new(&sh);
        let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
        term.draw(|f| menu.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("▸ Install"));
        assert!(text.contains("Uninstall"));
        assert!(text.contains("service: active"));
    }

    #[test]
    fn navigation_wraps_and_enter_goes() {
        let mut sh = shared();
        let mut menu = Menu::new(&sh);
        let up = KeyEvent::from(KeyCode::Up);
        assert!(matches!(menu.handle(up, &mut sh, 0.0), Action::None));
        assert_eq!(menu.selected, ITEMS.len() - 1);
        assert!(matches!(
            menu.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 1.0),
            Action::Go(ScreenId::Uninstall)
        ));
        assert!(menu.animating(0.01));
    }
}

//! Stand-in for screens that later tasks implement.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::{Action, Screen, ScreenId, Shared};

pub struct Placeholder {
    id: ScreenId,
}

impl Placeholder {
    pub fn new(id: ScreenId) -> Self {
        Self { id }
    }
}

impl Screen for Placeholder {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::Back,
            _ => Action::None,
        }
    }
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        f.render_widget(
            Paragraph::new(Line::styled(
                format!("  {:?}: not implemented yet", self.id),
                shared.theme.muted(),
            )),
            area,
        );
    }
    fn keys(&self) -> String {
        "Esc back".into()
    }
    fn subtitle(&self) -> String {
        format!("{:?}", self.id)
    }
}

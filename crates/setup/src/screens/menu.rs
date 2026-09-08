//! Home: the main menu items (drawn by the shell's sidebar) and the live overview pane.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::{Action, Screen, ScreenId, Shared};

pub const ITEMS: [(ScreenId, &str, &str); 8] = [
    (ScreenId::Install, "Install", "set up service + config"),
    (ScreenId::Calibrate, "Calibrate", "fix rotation / mirroring"),
    (
        ScreenId::Screens,
        "Screens",
        "what each screen shows, cycling",
    ),
    (
        ScreenId::Configure,
        "Configure",
        "cluster, services, sky, display",
    ),
    (ScreenId::Status, "Status", "service, links, logs"),
    (ScreenId::RunHere, "Run here", "foreground with logs"),
    (
        ScreenId::Update,
        "Update",
        "download and install the latest release",
    ),
    (ScreenId::Uninstall, "Uninstall", "remove everything"),
];

pub struct Home;

impl Home {
    pub fn new(_shared: &Shared) -> Home {
        Home
    }
}

impl Screen for Home {
    fn handle(&mut self, _key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        // The shell drives the sidebar; Home has no keys of its own.
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("  overview", th.muted()))),
            area,
        );
    }

    fn keys(&self) -> String {
        "↑↓ move   ⏎ open   q quit".into()
    }
    fn subtitle(&self) -> String {
        "Home".into()
    }
}

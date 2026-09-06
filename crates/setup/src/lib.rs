//! Interactive setup: install, calibrate, screens, configure, status, run, uninstall.

pub mod anim;
pub mod ops;
pub mod screens;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_app::logs::LogSink;
use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::{DefaultTerminal, Frame};

use crate::anim::Slide;
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::theme::Theme;

#[derive(Clone, Debug)]
pub struct Ctx {
    pub config_path: PathBuf,
    pub sim: bool,
    pub version: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Menu,
    Calibrate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenId {
    Menu,
    Install,
    Calibrate,
    Screens,
    Configure,
    Status,
    RunHere,
    Uninstall,
}

pub enum Action {
    None,
    Go(ScreenId),
    Back,
    Quit,
}

/// State shared by all screens.
pub struct Shared {
    pub ctx: Ctx,
    pub theme: Theme,
    pub service_active: Option<bool>,
    pub banner: Option<String>,
    pub log_sink: LogSink,
}

impl Shared {
    /// A detached copy, so a test can pass `&mut Shared` while keeping its own.
    #[cfg(test)]
    pub fn clone_for_test(&self) -> Shared {
        Shared {
            ctx: self.ctx.clone(),
            theme: self.theme,
            service_active: self.service_active,
            banner: self.banner.clone(),
            log_sink: self.log_sink.clone(),
        }
    }
}

pub trait Screen {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, now: Secs) -> Action;
    fn tick(&mut self, _shared: &mut Shared, _now: Secs) {}
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs);
    fn keys(&self) -> String;
    fn subtitle(&self) -> String;
    fn animating(&self, _now: Secs) -> bool {
        false
    }
}

const SLIDE_SECS: Secs = 0.2;

struct App {
    shared: Shared,
    current: Box<dyn Screen>,
    current_id: ScreenId,
    slide: Slide, // 1.0 = fully off to the right, 0.0 = in place
}

impl App {
    fn new(start: Start, ctx: Ctx, log_sink: LogSink) -> App {
        // Best effort, so the menu footer and Configure's restart offer are right from
        // the first draw instead of waiting for a Status visit.
        let service_active = if ctx.sim {
            None
        } else {
            let sh = RealShell;
            Systemd::new(&sh, &service_user()).is_active().ok()
        };
        let shared = Shared {
            ctx,
            theme: Theme::detect(),
            service_active,
            banner: None,
            log_sink,
        };
        let id = match start {
            Start::Menu => ScreenId::Menu,
            Start::Calibrate => ScreenId::Calibrate,
        };
        let current = screens::make(id, &shared);
        App {
            shared,
            current,
            current_id: id,
            slide: Slide::fixed(0.0),
        }
    }

    fn go(&mut self, id: ScreenId, now: Secs) {
        self.current = screens::make(id, &self.shared);
        self.current_id = id;
        self.slide = Slide::fixed(1.0).to(0.0, now, SLIDE_SECS);
    }

    fn handle(&mut self, key: KeyEvent, now: Secs) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match self.current.handle(key, &mut self.shared, now) {
            Action::None => {}
            Action::Go(id) => self.go(id, now),
            Action::Back => {
                if self.current_id == ScreenId::Menu {
                    return false;
                }
                self.go(ScreenId::Menu, now);
            }
            Action::Quit => return false,
        }
        true
    }

    fn draw(&self, f: &mut Frame, now: Secs) {
        let area = f.area();
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(area);
        widgets::header(
            f,
            head,
            &self.shared.theme,
            now,
            self.shared.ctx.version,
            &self.current.subtitle(),
        );
        let offset = (self.slide.value(now) * body.width as f32) as u16;
        let shifted = Rect {
            x: body.x + offset,
            width: body.width.saturating_sub(offset),
            ..body
        };
        if shifted.width > 0 {
            self.current.draw(f, shifted, &self.shared, now);
        }
        widgets::footer(f, foot, &self.shared.theme, &self.current.keys());
    }

    fn animating(&self, now: Secs) -> bool {
        !self.slide.done(now) || self.current.animating(now)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let t0 = Instant::now();
        loop {
            let now = t0.elapsed().as_secs_f64();
            self.current.tick(&mut self.shared, now);
            terminal.draw(|f| self.draw(f, now))?;
            // header glyph animates continuously; 60 Hz only while something moves, else 10 Hz
            let wait = if self.animating(now) { 16 } else { 100 };
            if event::poll(Duration::from_millis(wait))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press && !self.handle(k, now) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Run the TUI. Installs a tracing subscriber that writes into the in-memory log sink so
/// nothing is printed over the UI.
pub fn run(start: Start, ctx: Ctx) -> Result<()> {
    let sink = LogSink::new(500);
    rackscreen_app::logs::install_sink_subscriber(&sink);
    let mut terminal = ratatui::init();
    let result = App::new(start, ctx, sink).run(&mut terminal);
    ratatui::restore();
    result
}

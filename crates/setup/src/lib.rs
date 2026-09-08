//! Interactive setup: install, calibrate, screens, configure, status, run, uninstall.

pub mod anim;
pub mod ops;
pub mod screens;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_app::logs::LogSink;
use rackscreen_core::anim::Secs;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};

use crate::anim::{Settle, Slide};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::ops::update::UpdateInfo;
use crate::theme::Theme;
use crate::widgets::{SidebarItem, SidebarView, StatusTone};

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
    Update,
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
    /// What the background release check found, `None` until it answers (or if it failed).
    pub update: Option<UpdateInfo>,
    /// Set by a screen that handed the terminal away and needs a full repaint.
    pub redraw: bool,
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
            update: self.update.clone(),
            redraw: self.redraw,
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
    /// Word for the header's right edge (`unsaved`, `panels live`); `None` shows the version.
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        None
    }
    /// True when the screen uses `←` itself, so the shell must not treat it as "back".
    fn consumes_left(&self) -> bool {
        false
    }
    /// Replace the main menu in the sidebar (Configure shows its groups).
    fn sidebar(&self) -> Option<SidebarView> {
        None
    }
}

/// Which area receives keys: the sidebar (only on Home) or the screen's pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Content,
}

const BAR_SECS: Secs = 0.15;
const SIDEBAR_W: u16 = 16;
const MIN_SIDEBAR_COLS: u16 = 72;
const MIN_COLS: u16 = 40;
const MIN_ROWS: u16 = 12;

/// Ask GitHub once, in the background, so the menu can show an update hint. A failure
/// (offline, rate limited) just drops the sender and the UI stays quiet about updates.
fn spawn_update_check(version: &'static str) -> Option<Receiver<UpdateInfo>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            if let Ok(release) = ops::update::fetch_latest() {
                let _ = tx.send(UpdateInfo {
                    newer: ops::update::is_newer(version, &release.tag),
                    latest: release.tag,
                });
            }
        })
        .ok()
        .map(|_| rx)
}

struct App {
    shared: Shared,
    current: Box<dyn Screen>,
    current_id: ScreenId,
    /// Main menu item under the sidebar bar (also the screen we came from).
    hover: usize,
    bar: Slide,
    focus: Focus,
    settle: Settle,
    update_rx: Option<Receiver<UpdateInfo>>,
}

impl App {
    fn new(start: Start, ctx: Ctx, log_sink: LogSink) -> App {
        // Best effort, so the Home footer and Configure's restart offer are right from
        // the first draw instead of waiting for a Status visit.
        let service_active = if ctx.sim {
            None
        } else {
            let sh = RealShell;
            Systemd::new(&sh, &service_user()).is_active().ok()
        };
        let update_rx = spawn_update_check(ctx.version);
        let shared = Shared {
            ctx,
            theme: Theme::detect(),
            service_active,
            banner: None,
            log_sink,
            update: None,
            redraw: false,
        };
        App::with_shared(start, shared, update_rx)
    }

    /// No update check, no systemd, unicode theme: for tests.
    #[cfg(test)]
    fn for_test(start: Start, ctx: Ctx) -> App {
        let shared = Shared {
            ctx,
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        };
        App::with_shared(start, shared, None)
    }

    fn with_shared(start: Start, shared: Shared, update_rx: Option<Receiver<UpdateInfo>>) -> App {
        let id = match start {
            Start::Menu => ScreenId::Menu,
            Start::Calibrate => ScreenId::Calibrate,
        };
        let current = screens::make(id, &shared);
        let hover = Self::item_index(id).unwrap_or(0);
        App {
            shared,
            current,
            current_id: id,
            hover,
            bar: Slide::fixed(hover as f32),
            focus: if id == ScreenId::Menu {
                Focus::Sidebar
            } else {
                Focus::Content
            },
            settle: Settle::finished(),
            update_rx,
        }
    }

    fn item_index(id: ScreenId) -> Option<usize> {
        screens::menu::ITEMS.iter().position(|(i, _, _)| *i == id)
    }

    fn hover_to(&mut self, idx: usize, now: Secs) {
        self.hover = idx;
        self.bar = self.bar.to(idx as f32, now, BAR_SECS);
    }

    fn go(&mut self, id: ScreenId, now: Secs) {
        // A banner is a one-off note for Home; once the user moves on it makes way for
        // the Recent logs again.
        if self.current_id == ScreenId::Menu && id != ScreenId::Menu {
            self.shared.banner = None;
        }
        self.current = screens::make(id, &self.shared);
        self.current_id = id;
        if let Some(i) = Self::item_index(id) {
            self.hover_to(i, now);
        }
        self.focus = if id == ScreenId::Menu {
            Focus::Sidebar
        } else {
            Focus::Content
        };
        self.settle = Settle::new(now);
    }

    fn handle(&mut self, key: KeyEvent, now: Secs) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if self.focus == Focus::Sidebar {
            let n = screens::menu::ITEMS.len();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.hover_to((self.hover + n - 1) % n, now),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    self.hover_to((self.hover + 1) % n, now)
                }
                KeyCode::Enter | KeyCode::Right => self.go(screens::menu::ITEMS[self.hover].0, now),
                KeyCode::Char('q') | KeyCode::Esc => return false,
                _ => {}
            }
            return true;
        }
        let mut action = self.current.handle(key, &mut self.shared, now);
        // A screen that ignores ← gets it again as Esc, so ← means exactly what Esc means
        // there: a dirty editor asks first, a running install stays, Status closes its
        // log view. The shell never drops a screen on its own.
        if matches!(action, Action::None)
            && key.code == KeyCode::Left
            && !self.current.consumes_left()
        {
            action = self
                .current
                .handle(KeyEvent::from(KeyCode::Esc), &mut self.shared, now);
        }
        match action {
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

    fn main_items(&self) -> Vec<SidebarItem> {
        let th = &self.shared.theme;
        let g = th.glyphs();
        screens::menu::ITEMS
            .iter()
            .map(|(id, title, _)| SidebarItem {
                label: (*title).to_string(),
                hint: match (id, &self.shared.update) {
                    (ScreenId::Update, Some(u)) if u.newer => {
                        Some((g.arrow_up.to_string(), th.warning()))
                    }
                    _ => None,
                },
            })
            .collect()
    }

    fn draw_sidebar(&self, f: &mut Frame, area: Rect, now: Secs) {
        let th = &self.shared.theme;
        match self.current.sidebar() {
            Some(view) => {
                let items: Vec<SidebarItem> = view
                    .items
                    .iter()
                    .map(|l| SidebarItem {
                        label: l.clone(),
                        hint: None,
                    })
                    .collect();
                widgets::sidebar(
                    f,
                    area,
                    th,
                    Some(&view.title),
                    &items,
                    view.selected as f32,
                    Some(view.selected),
                    true,
                );
            }
            None => {
                let current = Self::item_index(self.current_id);
                widgets::sidebar(
                    f,
                    area,
                    th,
                    None,
                    &self.main_items(),
                    self.bar.value(now),
                    current,
                    self.focus == Focus::Sidebar,
                );
            }
        }
    }

    /// Mix every cell's foreground from faint toward its final colour by row age.
    fn settle_pane(buf: &mut Buffer, pane: Rect, th: &Theme, settle: &Settle, now: Secs) {
        for row in 0..pane.height {
            let t = settle.amount(row as usize, now);
            if t >= 1.0 {
                continue;
            }
            for col in 0..pane.width {
                if let Some(c) = buf.cell_mut((pane.x + col, pane.y + row)) {
                    c.fg = th.mix(th.faint, c.fg, t);
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame, now: Secs) {
        let area = f.area();
        let th = &self.shared.theme;
        if area.width < MIN_COLS || area.height < MIN_ROWS {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" terminal too small (need {MIN_COLS}×{MIN_ROWS})"),
                    th.warning(),
                ))),
                area,
            );
            return;
        }
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .areas(area);
        let crumb = (self.current_id != ScreenId::Menu).then(|| self.current.subtitle());
        let status = self.current.status(&self.shared);
        widgets::header(
            f,
            head,
            th,
            now,
            self.shared.ctx.version,
            crumb.as_deref(),
            status.as_ref().map(|(s, tone)| (s.as_str(), *tone)),
        );
        let wide = area.width >= MIN_SIDEBAR_COLS;
        let pane = if wide {
            let [side, sep, pane] = Layout::horizontal([
                Constraint::Length(SIDEBAR_W),
                Constraint::Length(1),
                Constraint::Min(10),
            ])
            .areas(body);
            self.draw_sidebar(f, side, now);
            let bar = if th.unicode { "│" } else { "|" };
            let lines: Vec<Line> = (0..sep.height)
                .map(|_| Line::from(Span::styled(bar, th.faint_style())))
                .collect();
            f.render_widget(Paragraph::new(lines), sep);
            pane
        } else {
            body
        };
        if !wide && self.current_id == ScreenId::Menu {
            // No room for a sidebar: Home is the menu itself.
            self.draw_sidebar(f, pane, now);
        } else {
            self.current.draw(f, pane, &self.shared, now);
            Self::settle_pane(f.buffer_mut(), pane, th, &self.settle, now);
        }
        widgets::footer(f, foot, th, &self.current.keys());
    }

    fn animating(&self, now: Secs) -> bool {
        !self.bar.done(now) || !self.settle.done(now) || self.current.animating(now)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let t0 = Instant::now();
        loop {
            let now = t0.elapsed().as_secs_f64();
            if let Some(rx) = &self.update_rx {
                match rx.try_recv() {
                    Ok(info) => {
                        self.shared.update = Some(info);
                        self.update_rx = None;
                    }
                    // Offline or rate limited: stay quiet and stop looking.
                    Err(mpsc::TryRecvError::Disconnected) => self.update_rx = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            self.current.tick(&mut self.shared, now);
            if self.shared.redraw {
                self.shared.redraw = false;
                terminal.clear()?;
            }
            terminal.draw(|f| self.draw(f, now))?;
            // header glyph and pulse animate at 10 Hz; 60 Hz only while something moves
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn new_app(dir: &std::path::Path, w: u16, h: u16) -> (App, Terminal<TestBackend>) {
        let ctx = Ctx {
            config_path: dir.join("config.yaml"),
            sim: true,
            version: "0.5.0",
        };
        let app = App::for_test(Start::Menu, ctx);
        let term = Terminal::new(TestBackend::new(w, h)).unwrap();
        (app, term)
    }

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

    #[test]
    fn home_shows_sidebar_with_pointer_and_footer_keys() {
        let dir = tempfile::tempdir().unwrap();
        let (app, mut term) = new_app(dir.path(), 100, 30);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen"));
        assert!(t.contains("v0.5.0"));
        assert!(t.contains("▸ Install"), "{t}");
        assert!(t.contains("Uninstall"));
        assert!(t.contains("⏎ open"));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn sidebar_navigation_wraps_and_enter_opens_with_content_focus() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut term) = new_app(dir.path(), 100, 30);
        assert!(app.handle(KeyEvent::from(KeyCode::Up), 0.0));
        assert_eq!(app.hover, screens::menu::ITEMS.len() - 1);
        assert!(app.animating(0.01), "the bar slides");
        for _ in 0..4 {
            app.handle(KeyEvent::from(KeyCode::Down), 1.0);
        }
        assert_eq!(screens::menu::ITEMS[app.hover].0, ScreenId::Configure);
        assert!(app.handle(KeyEvent::from(KeyCode::Enter), 2.0));
        assert_eq!(app.current_id, ScreenId::Configure);
        assert_eq!(app.focus, Focus::Content);
        term.draw(|f| app.draw(f, 2.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen › Configure"), "{t}");
        // Esc returns to Home with the sidebar focused and Configure still hovered
        assert!(app.handle(KeyEvent::from(KeyCode::Esc), 3.0));
        assert_eq!(app.current_id, ScreenId::Menu);
        assert_eq!(app.focus, Focus::Sidebar);
        assert_eq!(screens::menu::ITEMS[app.hover].0, ScreenId::Configure);
    }

    #[test]
    fn left_leaves_a_screen_unless_it_consumes_left() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = new_app(dir.path(), 100, 30);
        // Screens does not use ←: it goes back to Home
        app.go(ScreenId::Screens, 0.0);
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 1.0));
        assert_eq!(app.current_id, ScreenId::Menu);
        // Configure uses ← for groups: it stays
        app.go(ScreenId::Configure, 2.0);
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 3.0));
        assert_eq!(app.current_id, ScreenId::Configure);
        // Calibrate uses ← to select the previous panel: it stays too
        app.go(ScreenId::Calibrate, 4.0);
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 5.0));
        assert_eq!(app.current_id, ScreenId::Calibrate);
    }

    #[test]
    fn left_on_a_dirty_screens_editor_asks_before_discarding() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut term) = new_app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 0.0);
        // `+` lengthens the cycle: the editor is now dirty
        assert!(app.handle(KeyEvent::from(KeyCode::Char('+')), 1.0));
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 2.0));
        assert_eq!(
            app.current_id,
            ScreenId::Screens,
            "← never drops unsaved edits"
        );
        term.draw(|f| app.draw(f, 2.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("Discard changes?"), "← acts as Esc: {t}");
        // and ← while the dialog is up keeps editing, exactly like Esc
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 3.0));
        assert_eq!(app.current_id, ScreenId::Screens);
    }

    #[test]
    fn left_on_a_running_install_stays_put() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = new_app(dir.path(), 100, 30);
        // `preview` builds an Install in `Phase::Running` without a worker thread
        app.current = Box::new(screens::install::Install::preview(Vec::new()));
        app.current_id = ScreenId::Install;
        app.focus = Focus::Content;
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 1.0));
        assert_eq!(
            app.current_id,
            ScreenId::Install,
            "a running install is never abandoned"
        );
    }

    #[test]
    fn banner_shows_on_home_once_and_is_cleared_on_leaving() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = new_app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 0.0);
        // a screen leaves a note, Home shows it
        app.shared.banner = Some("screens saved".into());
        app.go(ScreenId::Menu, 1.0);
        assert_eq!(app.shared.banner.as_deref(), Some("screens saved"));
        // opening any screen from Home clears it, so the Recent logs come back
        app.go(ScreenId::Status, 2.0);
        assert_eq!(app.shared.banner, None);
    }

    #[test]
    fn q_quits_from_home_and_ctrl_c_quits_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = new_app(dir.path(), 100, 30);
        assert!(!app.handle(KeyEvent::from(KeyCode::Char('q')), 0.0));
        let (mut app, _term) = new_app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 0.0);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.handle(ctrl_c, 1.0));
    }

    #[test]
    fn narrow_terminals_hide_the_sidebar_and_tiny_ones_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let (app, mut term) = new_app(dir.path(), 60, 24);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        let t = text(&term);
        assert!(
            t.contains("▸ Install"),
            "Home lists the menu in the pane: {t}"
        );
        assert!(!t.contains("│"), "no sidebar separator below 72 columns");
        let (app, mut term) = new_app(dir.path(), 30, 8);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        assert!(text(&term).contains("terminal too small"));
        let (app, mut term) = new_app(dir.path(), 1, 1);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
    }

    #[test]
    fn settle_fades_the_pane_in_from_faint() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut term) = new_app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 10.0);
        term.draw(|f| app.draw(f, 10.0)).unwrap();
        let faint = app.shared.theme.faint;
        // the first pane row is still faint right after opening
        let buf = term.backend().buffer().clone();
        let row = 2u16; // header is two rows
        let any_faint = (17..100u16).any(|x| buf.cell((x, row)).unwrap().fg == faint);
        assert!(any_faint, "pane text starts faint");
        term.draw(|f| app.draw(f, 12.0)).unwrap();
        let buf = term.backend().buffer().clone();
        let still_faint_text = (17..100u16).filter(|x| {
            let c = buf.cell((*x, row)).unwrap();
            c.symbol() != " " && c.fg == faint
        });
        assert_eq!(
            still_faint_text.count(),
            0,
            "after 2 s everything has settled"
        );
        assert!(!app.animating(12.0));
    }
}

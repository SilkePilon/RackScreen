//! Screens editor: which roles each physical screen cycles through, and how fast.

use rackscreen_app::config::Config;
use rackscreen_core::anim::Secs;
use rackscreen_core::theme::Role;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::config_file::{save_config, set_screen_roles, Preset};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::widgets::confirm_dialog;
use crate::{Action, Screen, Shared};

/// Pure editor state, tested without a terminal.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    rows: Vec<(Vec<Role>, u64)>,
    pub selected: usize,
    picker: Option<Picker>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    /// Ordered roles of the screen being edited.
    pub chosen: Vec<Role>,
    /// Cursor over `Role::ALL`.
    pub cursor: usize,
}

impl Editor {
    pub fn new(rows: Vec<(Vec<Role>, u64)>) -> Editor {
        Editor {
            rows,
            selected: 0,
            picker: None,
        }
    }
    pub fn rows(&self) -> &[(Vec<Role>, u64)] {
        &self.rows
    }
    pub fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }
    pub fn move_selection(&mut self, delta: i32) {
        let n = self.rows.len().max(1) as i32;
        self.selected = ((self.selected as i32 + delta).rem_euclid(n)) as usize;
    }
    pub fn adjust_cycle(&mut self, delta: i64) {
        if let Some(r) = self.rows.get_mut(self.selected) {
            r.1 = ((r.1 as i64 + delta).clamp(3, 300)) as u64;
        }
    }
    pub fn open_picker(&mut self) {
        let chosen = self
            .rows
            .get(self.selected)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        self.picker = Some(Picker { chosen, cursor: 0 });
    }
    pub fn picker_move(&mut self, delta: i32) {
        if let Some(p) = &mut self.picker {
            let n = Role::ALL.len() as i32;
            p.cursor = ((p.cursor as i32 + delta).rem_euclid(n)) as usize;
        }
    }
    /// Space: add the role under the cursor (at the end) or remove it.
    pub fn picker_toggle(&mut self) {
        if let Some(p) = &mut self.picker {
            let role = Role::ALL[p.cursor];
            if let Some(i) = p.chosen.iter().position(|r| *r == role) {
                p.chosen.remove(i);
            } else {
                p.chosen.push(role);
            }
        }
    }
    /// J/K: move the role under the cursor within the chosen order.
    pub fn picker_shift(&mut self, delta: i32) {
        if let Some(p) = &mut self.picker {
            let role = Role::ALL[p.cursor];
            if let Some(i) = p.chosen.iter().position(|r| *r == role) {
                let j = (i as i32 + delta).clamp(0, p.chosen.len() as i32 - 1) as usize;
                p.chosen.swap(i, j);
            }
        }
    }
    /// Enter: commit the picker (empty selection keeps the old roles).
    pub fn close_picker(&mut self) {
        if let Some(p) = self.picker.take() {
            if !p.chosen.is_empty() {
                if let Some(r) = self.rows.get_mut(self.selected) {
                    r.0 = p.chosen;
                }
            }
        }
    }
    pub fn cancel_picker(&mut self) {
        self.picker = None;
    }
    pub fn apply_preset(&mut self, preset: Preset) {
        for (i, roles) in preset.rows().into_iter().enumerate() {
            if let Some(r) = self.rows.get_mut(i) {
                *r = (roles, 15);
            }
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        for (i, (roles, secs)) in self.rows.iter().enumerate() {
            if roles.is_empty() {
                return Err(format!("screen {} has no roles", i + 1));
            }
            if !(3..=300).contains(secs) {
                return Err(format!("screen {} cycle must be 3..300 s", i + 1));
            }
        }
        Ok(())
    }
}

enum Mode {
    Edit,
    AskRestart,
    Restarting(std::sync::mpsc::Receiver<String>),
}

pub struct Screens {
    cfg: Result<Config, String>,
    editor: Editor,
    mode: Mode,
    error: Option<String>,
    dirty: bool,
}

fn rows_from(cfg: &Config) -> Vec<(Vec<Role>, u64)> {
    cfg.screens
        .iter()
        .map(|s| (s.roles().unwrap_or_else(|_| vec![Role::Cpu]), s.cycle_secs))
        .collect()
}

impl Screens {
    pub fn new(shared: &Shared) -> Screens {
        let cfg = Config::load_or_default(&shared.ctx.config_path)
            .map_err(|e| format!("config unreadable: {e:#}; fix the file by hand"));
        let editor = Editor::new(cfg.as_ref().map(rows_from).unwrap_or_else(|_| {
            Preset::Cluster
                .rows()
                .into_iter()
                .map(|r| (r, 15))
                .collect()
        }));
        Screens {
            error: cfg.as_ref().err().cloned(),
            cfg,
            editor,
            mode: Mode::Edit,
            dirty: false,
        }
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if let Err(e) = self.editor.validate() {
            self.error = Some(e);
            return Action::None;
        }
        let Ok(cfg) = &mut self.cfg else {
            return Action::None;
        };
        for (i, (roles, secs)) in self.editor.rows().iter().enumerate() {
            set_screen_roles(cfg, i, roles, *secs);
        }
        match save_config(cfg, &shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                self.error = None;
                shared.banner = Some("screens saved".into());
                let sh = RealShell;
                let active = !shared.ctx.sim
                    && Systemd::new(&sh, &service_user())
                        .is_active()
                        .unwrap_or(false);
                shared.service_active = Some(active);
                if active {
                    self.mode = Mode::AskRestart;
                    Action::None
                } else {
                    Action::Back
                }
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                Action::None
            }
        }
    }

    fn spawn_restart(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("restart".into())
            .spawn(move || {
                let sh = RealShell;
                let msg = match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "service restarted".to_string(),
                    Err(e) => format!("restart failed: {e:#}"),
                };
                let _ = tx.send(msg);
            })
            .expect("spawn restart");
        self.mode = Mode::Restarting(rx);
    }

    /// The `!` hint for one role: the grid roles need an Electricity Maps token,
    /// and `price` needs a source that can actually fetch anything.
    fn role_hint(&self, r: Role) -> Option<&'static str> {
        let c = self.cfg.as_ref().ok()?;
        match r {
            Role::PowerMix | Role::Carbon | Role::Renewable => {
                c.electricity.token.is_empty().then_some("no token")
            }
            Role::Price => match c.price.source.as_str() {
                "entsoe" => c.price.entsoe_token.is_empty().then_some("no price source"),
                "none" => Some("no price source"),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Screen for Screens {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        if matches!(self.mode, Mode::Restarting(_)) {
            return Action::None;
        }
        if matches!(self.mode, Mode::AskRestart) {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                self.spawn_restart();
                return Action::None;
            }
            self.mode = Mode::Edit;
            return Action::Back;
        }
        if self.editor.picker().is_some() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.editor.picker_move(-1),
                KeyCode::Down | KeyCode::Char('j') => self.editor.picker_move(1),
                KeyCode::Char(' ') => {
                    self.editor.picker_toggle();
                    self.dirty = true;
                }
                KeyCode::Char('K') => self.editor.picker_shift(-1),
                KeyCode::Char('J') => self.editor.picker_shift(1),
                KeyCode::Enter => self.editor.close_picker(),
                KeyCode::Esc => self.editor.cancel_picker(),
                _ => {}
            }
            return Action::None;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.editor.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.editor.move_selection(1),
            KeyCode::Enter => self.editor.open_picker(),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.editor.adjust_cycle(5);
                self.dirty = true;
            }
            KeyCode::Char('-') => {
                self.editor.adjust_cycle(-5);
                self.dirty = true;
            }
            KeyCode::Char('c') => {
                self.editor.apply_preset(Preset::Cluster);
                self.dirty = true;
            }
            KeyCode::Char('e') => {
                self.editor.apply_preset(Preset::Electricity);
                self.dirty = true;
                if let Ok(cfg) = &self.cfg {
                    if cfg.electricity.token.is_empty() {
                        self.error = Some(
                            "electricity preset needs an API token: set it under Configure".into(),
                        );
                    }
                }
            }
            KeyCode::Char('m') => {
                self.editor.apply_preset(Preset::Mixed);
                self.dirty = true;
            }
            KeyCode::Char('s') => return self.save(shared),
            KeyCode::Esc | KeyCode::Char('q') => return Action::Back,
            _ => {}
        }
        Action::None
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        let done = if let Mode::Restarting(rx) = &self.mode {
            match rx.try_recv() {
                Ok(msg) => Some(msg),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some("restart thread died".to_string())
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            }
        } else {
            None
        };
        if let Some(msg) = done {
            shared.banner = Some(msg);
            self.mode = Mode::Edit;
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, roles, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(self.editor.rows().len() as u16 + 1),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .areas(area);
        let mut lines = Vec::new();
        for (i, (rs, secs)) in self.editor.rows().iter().enumerate() {
            let sel = i == self.editor.selected;
            let names = rs.iter().map(|r| r.name()).collect::<Vec<_>>().join(" › ");
            let timing = if rs.len() > 1 {
                format!("every {secs} s")
            } else {
                "static".to_string()
            };
            let warn = rs.iter().find_map(|r| self.role_hint(*r));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", if sel { g.pointer } else { " " }),
                    th.selected(),
                ),
                Span::styled(format!("{}  ", i + 1), th.muted()),
                Span::styled(
                    format!("{names:<30}"),
                    if sel { th.selected() } else { th.normal() },
                ),
                Span::styled(timing, th.muted()),
                Span::styled(
                    warn.map(|w| format!("  ! {w}")).unwrap_or_default(),
                    th.warning(),
                ),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);

        let mut body = vec![Line::from(Span::styled("  Roles:", th.muted()))];
        if let Some(p) = self.editor.picker() {
            for (i, r) in Role::ALL.iter().enumerate() {
                let on = p.chosen.iter().position(|x| x == r);
                let mark = match on {
                    Some(k) => format!("{}{}", g.done, k + 1),
                    None => g.pending.to_string(),
                };
                let cur = i == p.cursor;
                body.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{} ", if cur { g.pointer } else { " " }),
                        th.selected(),
                    ),
                    Span::styled(
                        format!("{mark:<3}"),
                        if on.is_some() {
                            th.good()
                        } else {
                            th.faint_style()
                        },
                    ),
                    Span::styled(
                        r.name().to_string(),
                        if cur { th.selected() } else { th.normal() },
                    ),
                ]));
            }
        } else {
            body.push(Line::from(Span::styled(
                format!(
                    "    {}",
                    Role::ALL
                        .iter()
                        .map(|r| r.name())
                        .collect::<Vec<_>>()
                        .join("  ")
                ),
                th.faint_style(),
            )));
            body.push(Line::from(""));
            body.push(Line::from(vec![
                Span::styled("  Presets: ", th.muted()),
                Span::styled("c", th.selected()),
                Span::styled(" cluster   ", th.muted()),
                Span::styled("e", th.selected()),
                Span::styled(" electricity   ", th.muted()),
                Span::styled("m", th.selected()),
                Span::styled(" mixed", th.muted()),
            ]));
        }
        f.render_widget(Paragraph::new(body), roles);

        let msg = match (&self.error, self.dirty) {
            (Some(e), _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (None, true) => Line::from(Span::styled("  unsaved changes: s to save", th.warning())),
            (None, false) => Line::from(Span::styled(
                format!("  {}", shared.ctx.config_path.display()),
                th.faint_style(),
            )),
        };
        f.render_widget(Paragraph::new(msg), foot);
        if matches!(self.mode, Mode::AskRestart) {
            confirm_dialog(
                f,
                area,
                th,
                "Restart service?",
                &["Apply the new screen layout to the running service now?".to_string()],
                "y/⏎ restart   Esc later",
                false,
            );
        }
    }

    fn keys(&self) -> String {
        if self.editor.picker().is_some() {
            "↑↓ move  space toggle  J/K reorder  ⏎ done  Esc cancel".into()
        } else {
            "↑↓ screen  ⏎ roles  +/- interval  c/e/m preset  s save  Esc back".into()
        }
    }
    fn subtitle(&self) -> String {
        "Screens".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_toggle_reorder_and_presets() {
        let mut e = Editor::new(vec![(vec![Role::Cpu], 15), (vec![Role::Mem], 15)]);
        e.move_selection(-1);
        assert_eq!(e.selected, 1);
        e.adjust_cycle(-20);
        assert_eq!(e.rows()[1].1, 3);
        e.adjust_cycle(500);
        assert_eq!(e.rows()[1].1, 300);
        e.open_picker();
        e.picker_move(Role::Thermal.index() as i32);
        e.picker_toggle();
        assert_eq!(e.picker().unwrap().chosen, vec![Role::Mem, Role::Thermal]);
        e.picker_shift(-1);
        assert_eq!(e.picker().unwrap().chosen, vec![Role::Thermal, Role::Mem]);
        e.close_picker();
        assert_eq!(e.rows()[1].0, vec![Role::Thermal, Role::Mem]);
        e.open_picker();
        e.picker_move(Role::Thermal.index() as i32);
        e.picker_toggle();
        e.picker_move(Role::Mem.index() as i32 - Role::Thermal.index() as i32);
        e.picker_toggle();
        e.close_picker();
        assert_eq!(
            e.rows()[1].0,
            vec![Role::Thermal, Role::Mem],
            "empty selection keeps old roles"
        );
        e.apply_preset(Preset::Electricity);
        assert_eq!(e.rows()[0].0, vec![Role::PowerMix]);
        assert_eq!(e.rows().len(), 2, "preset only touches existing rows");
        assert!(e.validate().is_ok());
    }

    fn test_shared(dir: &std::path::Path) -> Shared {
        use crate::theme::Theme;
        use crate::Ctx;
        Shared {
            ctx: Ctx {
                config_path: dir.join("c.yaml"),
                sim: true,
                version: "0.3.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: rackscreen_app::logs::LogSink::new(10),
        }
    }

    #[test]
    fn hints_flag_a_missing_token_and_a_dead_price_source() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let dir = tempfile::tempdir().unwrap();
        let sh = test_shared(dir.path());
        let mut s = Screens::new(&sh);
        // defaults: no Electricity Maps token, prices from EnergyZero
        assert_eq!(s.role_hint(Role::PowerMix), Some("no token"));
        assert_eq!(s.role_hint(Role::Carbon), Some("no token"));
        assert_eq!(s.role_hint(Role::Price), None, "energyzero needs nothing");
        assert_eq!(s.role_hint(Role::Cpu), None);
        {
            let cfg = s.cfg.as_mut().unwrap();
            cfg.electricity.token = "tok".into();
            cfg.price.source = "entsoe".into();
        }
        assert_eq!(s.role_hint(Role::PowerMix), None);
        assert_eq!(
            s.role_hint(Role::Price),
            Some("no price source"),
            "entsoe without a token cannot fetch"
        );
        s.cfg.as_mut().unwrap().price.entsoe_token = "t".into();
        assert_eq!(s.role_hint(Role::Price), None);
        s.cfg.as_mut().unwrap().price.source = "none".into();
        assert_eq!(s.role_hint(Role::Price), Some("no price source"));
        // and the row says so
        s.editor.apply_preset(Preset::Electricity);
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("! no price source"), "{text}");
    }

    #[test]
    fn screens_render_rows_and_picker() {
        use crate::theme::Theme;
        use crate::Ctx;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let dir = tempfile::tempdir().unwrap();
        let sh = Shared {
            ctx: Ctx {
                config_path: dir.path().join("c.yaml"),
                sim: true,
                version: "0.3.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: rackscreen_app::logs::LogSink::new(10),
        };
        let mut s = Screens::new(&sh);
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("▸ 1  cpu"));
        assert!(text.contains("static"));
        assert!(text.contains("Presets"));
        s.handle(
            KeyEvent::from(KeyCode::Enter),
            &mut sh.clone_for_test(),
            0.0,
        );
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("power-mix"));
        assert!(text.contains("✓1 cpu") || text.contains("✓1  cpu"));
    }
}

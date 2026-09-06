//! Configure: a form over the YAML fields with inline editing and validation.

use rackscreen_app::config::Config;
use rackscreen_core::anim::Secs;
use rackscreen_core::night::parse_hhmm;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::widgets::confirm_dialog;
use crate::{Action, Screen, Shared};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Secret,
    Number,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Kubeconfig,
    PromNamespace,
    PromService,
    PromPort,
    PromPoll,
    QbitEnabled,
    QbitNamespace,
    QbitService,
    QbitPort,
    QbitUser,
    QbitPass,
    QbitPoll,
    NightEnabled,
    NightStart,
    NightEnd,
    HotCpu,
    HotMem,
    Brightness,
    Fps,
    SpiChunk,
}

pub const FIELDS: [(Field, &str, FieldKind); 20] = [
    (Field::Kubeconfig, "kubeconfig path", FieldKind::Text),
    (
        Field::PromNamespace,
        "prometheus namespace",
        FieldKind::Text,
    ),
    (Field::PromService, "prometheus service", FieldKind::Text),
    (Field::PromPort, "prometheus port", FieldKind::Number),
    (Field::PromPoll, "prometheus poll secs", FieldKind::Number),
    (Field::QbitEnabled, "qbittorrent enabled", FieldKind::Bool),
    (
        Field::QbitNamespace,
        "qbittorrent namespace",
        FieldKind::Text,
    ),
    (Field::QbitService, "qbittorrent service", FieldKind::Text),
    (Field::QbitPort, "qbittorrent port", FieldKind::Number),
    (Field::QbitUser, "qbittorrent user", FieldKind::Text),
    (Field::QbitPass, "qbittorrent password", FieldKind::Secret),
    (Field::QbitPoll, "qbittorrent poll secs", FieldKind::Number),
    (Field::NightEnabled, "night mode", FieldKind::Bool),
    (Field::NightStart, "night start (HH:MM)", FieldKind::Text),
    (Field::NightEnd, "night end (HH:MM)", FieldKind::Text),
    (Field::HotCpu, "hot node cpu %", FieldKind::Number),
    (Field::HotMem, "hot node mem %", FieldKind::Number),
    (Field::Brightness, "brightness 0.1-1.0", FieldKind::Number),
    (Field::Fps, "fps", FieldKind::Number),
    (Field::SpiChunk, "spi chunk bytes", FieldKind::Number),
];

pub fn get(cfg: &Config, f: Field) -> String {
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig.clone(),
        Field::PromNamespace => cfg.prometheus.namespace.clone(),
        Field::PromService => cfg.prometheus.service.clone(),
        Field::PromPort => cfg.prometheus.port.to_string(),
        Field::PromPoll => cfg.prometheus.poll_secs.to_string(),
        Field::QbitEnabled => cfg.qbittorrent.enabled.to_string(),
        Field::QbitNamespace => cfg.qbittorrent.namespace.clone(),
        Field::QbitService => cfg.qbittorrent.service.clone(),
        Field::QbitPort => cfg.qbittorrent.port.to_string(),
        Field::QbitUser => cfg.qbittorrent.user.clone(),
        Field::QbitPass => cfg.qbittorrent.pass.clone(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs.to_string(),
        Field::NightEnabled => cfg.night.enabled.to_string(),
        Field::NightStart => cfg.night.start.clone(),
        Field::NightEnd => cfg.night.end.clone(),
        Field::HotCpu => format!("{}", cfg.thresholds.hot_cpu),
        Field::HotMem => format!("{}", cfg.thresholds.hot_mem),
        Field::Brightness => format!("{}", cfg.display.brightness),
        Field::Fps => cfg.display.fps.to_string(),
        Field::SpiChunk => cfg.display.spi_chunk.to_string(),
    }
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    s.trim()
        .parse::<T>()
        .map_err(|_| format!("{what}: not a number"))
}

pub fn set(cfg: &mut Config, f: Field, text: &str) -> Result<(), String> {
    let t = text.trim();
    match f {
        Field::Kubeconfig => cfg.k8s.kubeconfig = t.into(),
        Field::PromNamespace => cfg.prometheus.namespace = t.into(),
        Field::PromService => cfg.prometheus.service = t.into(),
        Field::PromPort => cfg.prometheus.port = num(t, "port")?,
        Field::PromPoll => cfg.prometheus.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::QbitEnabled => cfg.qbittorrent.enabled = t == "true",
        Field::QbitNamespace => cfg.qbittorrent.namespace = t.into(),
        Field::QbitService => cfg.qbittorrent.service = t.into(),
        Field::QbitPort => cfg.qbittorrent.port = num(t, "port")?,
        Field::QbitUser => cfg.qbittorrent.user = t.into(),
        Field::QbitPass => cfg.qbittorrent.pass = text.into(),
        Field::QbitPoll => cfg.qbittorrent.poll_secs = num::<u64>(t, "poll secs")?.max(1),
        Field::NightEnabled => cfg.night.enabled = t == "true",
        Field::NightStart => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.start = t.into();
        }
        Field::NightEnd => {
            parse_hhmm(t).ok_or("expected HH:MM")?;
            cfg.night.end = t.into();
        }
        Field::HotCpu => cfg.thresholds.hot_cpu = num(t, "cpu %")?,
        Field::HotMem => cfg.thresholds.hot_mem = num(t, "mem %")?,
        Field::Brightness => {
            let b: f32 = num(t, "brightness")?;
            if !(0.1..=1.0).contains(&b) {
                return Err("brightness must be between 0.1 and 1.0".into());
            }
            cfg.display.brightness = b;
        }
        Field::Fps => cfg.display.fps = num::<u32>(t, "fps")?.clamp(1, 60),
        Field::SpiChunk => cfg.display.spi_chunk = num::<usize>(t, "spi chunk")?.max(64),
    }
    Ok(())
}

enum Mode {
    Browse,
    Edit(String),
    AskRestart,
}

pub struct Configure {
    cfg: Config,
    row: usize,
    mode: Mode,
    error: Option<String>,
    dirty: bool,
    scroll: usize,
}

impl Configure {
    pub fn new(shared: &Shared) -> Configure {
        let cfg = Config::load_or_default(&shared.ctx.config_path).unwrap_or_default();
        Configure {
            cfg,
            row: 0,
            mode: Mode::Browse,
            error: None,
            dirty: false,
            scroll: 0,
        }
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if let Err(e) = self.cfg.validate() {
            self.error = Some(format!("{e:#}"));
            return Action::None;
        }
        match self.cfg.save(&shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                shared.banner = Some("config saved".into());
                if shared.service_active == Some(true) && !shared.ctx.sim {
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
}

impl Screen for Configure {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let (field, _, kind) = FIELDS[self.row];
        // Take the mode out so the arms can replace it without a live borrow.
        let mode = std::mem::replace(&mut self.mode, Mode::Browse);
        match mode {
            Mode::Browse => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.row = (self.row + FIELDS.len() - 1) % FIELDS.len()
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    self.row = (self.row + 1) % FIELDS.len()
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if kind == FieldKind::Bool {
                        let cur = get(&self.cfg, field) == "true";
                        let _ = set(&mut self.cfg, field, if cur { "false" } else { "true" });
                        self.dirty = true;
                    } else {
                        self.mode = Mode::Edit(get(&self.cfg, field));
                    }
                }
                KeyCode::Char('s') => return self.save(shared),
                KeyCode::Esc | KeyCode::Char('q') => return Action::Back,
                _ => {}
            },
            Mode::Edit(mut buf) => match key.code {
                KeyCode::Enter => match set(&mut self.cfg, field, &buf) {
                    Ok(()) => {
                        self.error = None;
                        self.dirty = true;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.mode = Mode::Edit(buf);
                    }
                },
                KeyCode::Esc => self.error = None,
                KeyCode::Backspace => {
                    buf.pop();
                    self.mode = Mode::Edit(buf);
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.mode = Mode::Edit(buf);
                }
                _ => self.mode = Mode::Edit(buf),
            },
            Mode::AskRestart => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                    let sh = RealShell;
                    let _ = Systemd::new(&sh, &service_user()).restart();
                }
                return Action::Back;
            }
        }
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .areas(area);
        let visible = list.height as usize;
        let scroll = if self.row >= visible {
            self.row + 1 - visible
        } else {
            0
        };
        let _ = self.scroll;
        let mut lines = Vec::new();
        for (i, (field, label, kind)) in FIELDS.iter().enumerate().skip(scroll).take(visible) {
            let selected = i == self.row;
            let raw = get(&self.cfg, *field);
            let shown = match (&self.mode, selected, kind) {
                (Mode::Edit(buf), true, FieldKind::Secret) => {
                    format!("{}_", "*".repeat(buf.chars().count()))
                }
                (Mode::Edit(buf), true, _) => format!("{buf}_"),
                (_, _, FieldKind::Secret) => "*".repeat(raw.chars().count()),
                (_, _, FieldKind::Bool) => {
                    if raw == "true" {
                        format!("{} on", g.done)
                    } else {
                        format!("{} off", g.pending)
                    }
                }
                _ => raw,
            };
            let pointer = if selected { g.pointer } else { " " };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{pointer} "), th.selected()),
                Span::styled(
                    format!("{label:<24}"),
                    if selected { th.selected() } else { th.normal() },
                ),
                Span::styled(
                    shown,
                    if matches!(self.mode, Mode::Edit(_)) && selected {
                        th.title()
                    } else {
                        th.muted()
                    },
                ),
            ]));
        }
        f.render_widget(Paragraph::new(lines), list);
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
                &["Apply the new config to the running service now?".to_string()],
                "y/⏎ restart   Esc later",
                false,
            );
        }
    }

    fn keys(&self) -> String {
        match self.mode {
            Mode::Browse => "↑↓ move  ⏎ edit/toggle  s save  Esc back".into(),
            Mode::Edit(_) => "type  ⏎ apply  Esc cancel".into(),
            Mode::AskRestart => "y restart  Esc later".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Configure".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_round_trip_and_validation() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::PromPort), "9090");
        set(&mut c, Field::PromPort, " 9091 ").unwrap();
        assert_eq!(c.prometheus.port, 9091);
        assert!(set(&mut c, Field::PromPort, "x")
            .unwrap_err()
            .contains("not a number"));
        assert!(set(&mut c, Field::NightStart, "25:00").is_err());
        set(&mut c, Field::NightStart, "22:30").unwrap();
        assert_eq!(c.night.start, "22:30");
        assert!(set(&mut c, Field::Brightness, "1.5").is_err());
        set(&mut c, Field::Brightness, "0.7").unwrap();
        set(&mut c, Field::QbitEnabled, "false").unwrap();
        assert!(!c.qbittorrent.enabled);
        set(&mut c, Field::Fps, "500").unwrap();
        assert_eq!(c.display.fps, 60);
        for (f, _, _) in FIELDS {
            let v = get(&c, f);
            assert!(set(&mut c, f, &v).is_ok(), "{f:?} round trip with {v:?}");
        }
    }
}

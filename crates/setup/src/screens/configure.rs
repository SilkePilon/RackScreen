//! Configure: a form over the YAML fields with inline editing and validation.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use rackscreen_app::config::Config;
use rackscreen_core::anim::Secs;
use rackscreen_core::night::parse_hhmm;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::config_file::save_config;
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
    /// One of a fixed set of strings, cycled with Enter/space.
    Choice,
}

/// The values a `FieldKind::Choice` field cycles through, in order.
pub const PRICE_SOURCES: [&str; 3] = ["energyzero", "entsoe", "none"];

/// The value after `cur` in `PRICE_SOURCES`, wrapping; the first one if `cur` is unknown.
fn next_choice(cur: &str) -> &'static str {
    let i = PRICE_SOURCES.iter().position(|v| *v == cur);
    PRICE_SOURCES[i.map_or(0, |i| (i + 1) % PRICE_SOURCES.len())]
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
    OneAtATime,
    ElecEnabled,
    ElecZone,
    ElecToken,
    ElecPoll,
    PriceSource,
    EntsoeToken,
    EntsoeZone,
    IncludeVat,
    PricePoll,
    HotTemp,
}

pub const FIELDS: [(Field, &str, FieldKind); 31] = [
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
    (Field::OneAtATime, "screens one at a time", FieldKind::Bool),
    (Field::ElecEnabled, "electricity enabled", FieldKind::Bool),
    (Field::ElecZone, "electricity zone", FieldKind::Text),
    (Field::ElecToken, "electricity api token", FieldKind::Secret),
    (Field::ElecPoll, "electricity poll secs", FieldKind::Number),
    (Field::PriceSource, "price source", FieldKind::Choice),
    (Field::EntsoeToken, "entsoe token", FieldKind::Secret),
    (Field::EntsoeZone, "entsoe zone (EIC)", FieldKind::Text),
    (Field::IncludeVat, "price incl. VAT", FieldKind::Bool),
    (Field::PricePoll, "price poll secs", FieldKind::Number),
    (Field::HotTemp, "hot node temp °C", FieldKind::Number),
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
        Field::OneAtATime => cfg.display.one_at_a_time.to_string(),
        Field::ElecEnabled => cfg.electricity.enabled.to_string(),
        Field::ElecZone => cfg.electricity.zone.clone(),
        Field::ElecToken => cfg.electricity.token.clone(),
        Field::ElecPoll => cfg.electricity.poll_secs.to_string(),
        Field::PriceSource => cfg.price.source.clone(),
        Field::EntsoeToken => cfg.price.entsoe_token.clone(),
        Field::EntsoeZone => cfg.price.entsoe_zone.clone(),
        Field::IncludeVat => cfg.price.include_vat.to_string(),
        Field::PricePoll => cfg.price.poll_secs.to_string(),
        Field::HotTemp => format!("{}", cfg.thresholds.hot_temp),
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
        Field::OneAtATime => cfg.display.one_at_a_time = t == "true",
        Field::ElecEnabled => cfg.electricity.enabled = t == "true",
        Field::ElecZone => cfg.electricity.zone = t.into(),
        Field::ElecToken => cfg.electricity.token = text.into(),
        Field::ElecPoll => cfg.electricity.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::PriceSource => {
            if !PRICE_SOURCES.contains(&t) {
                return Err(format!(
                    "price source must be one of {}",
                    PRICE_SOURCES.join(", ")
                ));
            }
            cfg.price.source = t.into();
        }
        Field::EntsoeToken => cfg.price.entsoe_token = text.into(),
        Field::EntsoeZone => cfg.price.entsoe_zone = t.into(),
        Field::IncludeVat => cfg.price.include_vat = t == "true",
        Field::PricePoll => cfg.price.poll_secs = num::<u64>(t, "poll secs")?.max(60),
        Field::HotTemp => cfg.thresholds.hot_temp = num(t, "temp °C")?,
    }
    Ok(())
}

enum Mode {
    Browse,
    Edit(String),
    AskRestart,
    /// `systemctl restart` is running on a worker thread; keys wait for its result.
    Restarting,
}

pub struct Configure {
    cfg: Config,
    row: usize,
    mode: Mode,
    error: Option<String>,
    /// Non-error status shown in the footer (e.g. the restart result).
    note: Option<String>,
    dirty: bool,
    scroll: usize,
    /// The file on disk could not be loaded: `cfg` holds defaults for display only and
    /// `s` must never overwrite the user's file with them.
    unreadable: bool,
    restart_rx: Option<Receiver<String>>,
}

impl Configure {
    pub fn new(shared: &Shared) -> Configure {
        Configure::from_load(Config::load_or_default(&shared.ctx.config_path))
    }

    /// Build from the result of loading the config file.
    pub fn from_load(loaded: anyhow::Result<Config>) -> Configure {
        let (cfg, error, unreadable) = match loaded {
            Ok(cfg) => (cfg, None, false),
            Err(e) => (
                Config::default(),
                Some(format!("config unreadable: {e:#}; fix the file by hand")),
                true,
            ),
        };
        Configure {
            cfg,
            row: 0,
            mode: Mode::Browse,
            error,
            note: None,
            dirty: false,
            scroll: 0,
            unreadable,
            restart_rx: None,
        }
    }

    fn save(&mut self, shared: &mut Shared) -> Action {
        if self.unreadable {
            self.error =
                Some("not saved: the existing config is unreadable; fix the file by hand".into());
            return Action::None;
        }
        if let Err(e) = self.cfg.validate() {
            self.error = Some(format!("{e:#}"));
            return Action::None;
        }
        match save_config(&self.cfg, &shared.ctx.config_path) {
            Ok(()) => {
                self.dirty = false;
                shared.banner = Some("config saved".into());
                if shared.ctx.sim {
                    return Action::Back;
                }
                // Ask systemd now rather than trusting a cached value: on a fresh run
                // nothing has populated `shared.service_active` yet.
                let sh = RealShell;
                let active = Systemd::new(&sh, &service_user())
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
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("restart".into())
            .spawn(move || {
                let sh = RealShell;
                let msg = match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "config saved; service restarted".to_string(),
                    Err(e) => format!("config saved; restart failed: {e:#}"),
                };
                let _ = tx.send(msg);
            })
            .expect("spawn restart");
        self.restart_rx = Some(rx);
        self.mode = Mode::Restarting;
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
                    } else if kind == FieldKind::Choice {
                        let next = next_choice(&get(&self.cfg, field));
                        let _ = set(&mut self.cfg, field, next);
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
                    self.spawn_restart();
                    return Action::None;
                }
                return Action::Back;
            }
            Mode::Restarting => {
                // Leaving early is allowed; the restart finishes on its own.
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    return Action::Back;
                }
                self.mode = Mode::Restarting;
            }
        }
        Action::None
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        let Some(rx) = &self.restart_rx else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(TryRecvError::Disconnected) => "restart thread died".to_string(),
            Err(TryRecvError::Empty) => return,
        };
        self.restart_rx = None;
        shared.banner = Some(msg.clone());
        self.note = Some(msg);
        if matches!(self.mode, Mode::Restarting) {
            self.mode = Mode::Browse;
        }
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
        let msg = match (&self.mode, &self.error, &self.note, self.dirty) {
            (Mode::Restarting, _, _, _) => {
                Line::from(Span::styled("  restarting service...", th.warning()))
            }
            (_, Some(e), _, _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (_, None, _, true) => {
                Line::from(Span::styled("  unsaved changes: s to save", th.warning()))
            }
            (_, None, Some(n), false) => Line::from(Span::styled(format!("  {n}"), th.good())),
            (_, None, None, false) => Line::from(Span::styled(
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
            Mode::Restarting => "restarting...  Esc back".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Configure".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use rackscreen_app::logs::LogSink;
    use std::path::Path;

    fn shared_at(path: &Path) -> Shared {
        Shared {
            ctx: Ctx {
                config_path: path.to_path_buf(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        }
    }

    #[test]
    fn unreadable_config_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = shared_at(&path);
        let mut screen = Configure::from_load(Err(anyhow::anyhow!("parse config.yaml: bad")));
        assert!(screen.error.as_deref().unwrap().contains("unreadable"));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::None
        ));
        assert!(!path.exists(), "defaults must not replace the user's file");
        assert!(screen.error.as_deref().unwrap().contains("not saved"));
        assert!(sh.banner.is_none());
    }

    #[test]
    fn readable_config_saves_and_goes_back_in_sim() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = shared_at(&path);
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::Back
        ));
        assert_eq!(Config::load_or_default(&path).unwrap(), Config::default());
        assert_eq!(sh.banner.as_deref(), Some("config saved"));
    }

    #[test]
    fn restart_result_reaches_banner_on_tick() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        let (tx, rx) = mpsc::channel();
        screen.restart_rx = Some(rx);
        screen.mode = Mode::Restarting;
        screen.tick(&mut sh, 0.0);
        assert!(sh.banner.is_none(), "nothing yet");
        tx.send("config saved; service restarted".to_string())
            .unwrap();
        screen.tick(&mut sh, 0.1);
        assert_eq!(
            sh.banner.as_deref(),
            Some("config saved; service restarted")
        );
        assert!(matches!(screen.mode, Mode::Browse));
        assert!(screen.restart_rx.is_none());
    }

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
        assert_eq!(get(&c, Field::OneAtATime), "true");
        set(&mut c, Field::OneAtATime, "false").unwrap();
        assert!(!c.display.one_at_a_time);
        set(&mut c, Field::OneAtATime, "true").unwrap();
        assert!(c.display.one_at_a_time);
        for (f, _, _) in FIELDS {
            let v = get(&c, f);
            assert!(set(&mut c, f, &v).is_ok(), "{f:?} round trip with {v:?}");
        }
    }

    #[test]
    fn electricity_price_and_temp_fields() {
        let mut c = Config::default();
        assert_eq!(FIELDS.len(), 31);
        assert_eq!(get(&c, Field::ElecZone), "NL");
        assert_eq!(get(&c, Field::PriceSource), "energyzero");
        assert_eq!(get(&c, Field::HotTemp), "70");
        set(&mut c, Field::ElecEnabled, "true").unwrap();
        assert!(c.electricity.enabled);
        set(&mut c, Field::ElecZone, " DE ").unwrap();
        assert_eq!(c.electricity.zone, "DE");
        set(&mut c, Field::ElecToken, "tok").unwrap();
        assert_eq!(c.electricity.token, "tok");
        set(&mut c, Field::ElecPoll, "10").unwrap();
        assert_eq!(c.electricity.poll_secs, 60, "poll secs are floored at 60");
        set(&mut c, Field::EntsoeToken, "et").unwrap();
        set(&mut c, Field::EntsoeZone, "10YNL----------L").unwrap();
        assert_eq!(c.price.entsoe_zone, "10YNL----------L");
        set(&mut c, Field::IncludeVat, "false").unwrap();
        assert!(!c.price.include_vat);
        set(&mut c, Field::PricePoll, "1800").unwrap();
        assert_eq!(c.price.poll_secs, 1800);
        set(&mut c, Field::HotTemp, "82.5").unwrap();
        assert_eq!(c.thresholds.hot_temp, 82.5);
        assert!(set(&mut c, Field::HotTemp, "warm").is_err());
        // the choice field only takes the three known sources
        assert!(set(&mut c, Field::PriceSource, "nordpool")
            .unwrap_err()
            .contains("price source must be one of"));
        set(&mut c, Field::PriceSource, "entsoe").unwrap();
        assert_eq!(c.price.source, "entsoe");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn choice_field_cycles_on_enter() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        screen.row = FIELDS
            .iter()
            .position(|(f, _, _)| *f == Field::PriceSource)
            .unwrap();
        assert_eq!(screen.cfg.price.source, "energyzero");
        for want in ["entsoe", "none", "energyzero"] {
            screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0);
            assert_eq!(screen.cfg.price.source, want);
        }
        assert!(screen.dirty);
        // space cycles too, and an unknown value falls back to the first
        screen.cfg.price.source = "nordpool".into();
        screen.handle(KeyEvent::from(KeyCode::Char(' ')), &mut sh, 0.0);
        assert_eq!(screen.cfg.price.source, "energyzero");
        assert!(matches!(screen.mode, Mode::Browse), "no edit buffer opened");
    }
}

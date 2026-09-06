//! Run here: the monitor in-process, logs streaming into the TUI, q stops it cleanly.

use rackscreen_app::config::Config;
use rackscreen_app::run::{Monitor, RunOptions};
use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::{Action, Screen, Shared};

pub struct RunHere {
    monitor: Option<Monitor>,
    error: Option<String>,
    service_was_active: bool,
    stopping: bool,
}

impl RunHere {
    pub fn new(shared: &Shared) -> RunHere {
        shared.log_sink.clear();
        let mut me = RunHere {
            monitor: None,
            error: None,
            service_was_active: false,
            stopping: false,
        };
        // Stop the service only now that `me` exists: a panic in `Monitor::start` still
        // runs `Drop`, which starts it again.
        if !shared.ctx.sim {
            let sh = RealShell;
            let sd = Systemd::new(&sh, &service_user());
            if sd.is_active().unwrap_or(false) {
                tracing::info!("stopping the service while running in the foreground");
                me.service_was_active = true;
                let _ = sd.stop();
            }
        }
        let cfg = match Config::load_or_default(&shared.ctx.config_path) {
            Ok(c) => c,
            Err(e) => {
                me.error = Some(format!("{e:#}"));
                return me;
            }
        };
        let opts = RunOptions {
            sim: shared.ctx.sim,
            ..RunOptions::default()
        };
        match Monitor::start(&cfg, opts) {
            Ok(m) => me.monitor = Some(m),
            Err(e) => me.error = Some(format!("{e:#}")),
        }
        me
    }

    fn finish(&mut self) {
        if let Some(m) = self.monitor.take() {
            m.shutdown();
        }
        if self.service_was_active {
            let sh = RealShell;
            let _ = Systemd::new(&sh, &service_user()).start();
            self.service_was_active = false;
        }
    }
}

impl Drop for RunHere {
    fn drop(&mut self) {
        self.finish();
    }
}

impl Screen for RunHere {
    fn handle(&mut self, key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.stopping = true;
                if let Some(m) = &self.monitor {
                    m.stop();
                }
                self.finish();
                Action::Back
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, _shared: &mut Shared, _now: Secs) {
        if let Some(m) = &self.monitor {
            if m.is_stopped() && !self.stopping {
                self.error = Some("monitor stopped on its own (see log)".into());
                self.finish();
            }
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let [head, logs] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(area);
        let status = match (&self.error, self.monitor.is_some()) {
            (Some(e), _) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            (None, true) => Line::from(Span::styled(
                "  running in the foreground, q to stop",
                th.good(),
            )),
            (None, false) => Line::from(Span::styled("  stopped", th.muted())),
        };
        f.render_widget(Paragraph::new(status), head);
        let lines = shared.log_sink.lines();
        let tail: Vec<Line> = lines
            .iter()
            .rev()
            .take(logs.height as usize)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|l| {
                let style = if l.contains("WARN") || l.contains("ERROR") {
                    th.warning()
                } else {
                    th.faint_style()
                };
                Line::from(Span::styled(format!("  {l}"), style))
            })
            .collect();
        f.render_widget(Paragraph::new(tail), logs);
    }

    fn keys(&self) -> String {
        "q stop and back".into()
    }
    fn subtitle(&self) -> String {
        "Run here".into()
    }
    // No `animating` override: the 10 Hz idle redraw is plenty for a log tail, and
    // 60 Hz would only burn CPU next to the monitor.
}

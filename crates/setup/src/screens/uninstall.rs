//! Uninstall screen: confirm, then run the uninstaller with the same step list look.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::ops::install::Outcome;
use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::RealShell;
use crate::ops::uninstall::{UEvent, UStep, Uninstaller};
use crate::widgets::{confirm_dialog, step_list, StepState, StepView};
use crate::{Action, Screen, Shared};

enum Phase {
    Confirm,
    Running(Receiver<UEvent>),
    Done,
}

pub struct Uninstall {
    steps: Vec<StepView>,
    phase: Phase,
    progress: Slide,
}

impl Uninstall {
    pub fn new(_shared: &Shared) -> Uninstall {
        let steps = UStep::ALL
            .iter()
            .map(|s| StepView {
                title: s.title().into(),
                state: StepState::Pending,
            })
            .collect();
        Uninstall {
            steps,
            phase: Phase::Confirm,
            progress: Slide::fixed(0.0),
        }
    }

    fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        let u = Uninstaller {
            sh: Arc::new(RealShell),
            paths: Paths::system(),
            user: service_user(),
        };
        std::thread::Builder::new()
            .name("uninstall".into())
            .spawn(move || u.run(tx))
            .expect("spawn uninstaller");
        self.phase = Phase::Running(rx);
    }

    fn idx(id: UStep) -> usize {
        UStep::ALL.iter().position(|s| *s == id).unwrap_or(0)
    }
}

impl Screen for Uninstall {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let confirm = matches!(self.phase, Phase::Confirm);
        let done = matches!(self.phase, Phase::Done);
        match key.code {
            KeyCode::Char('y') if confirm => {
                self.start();
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') if confirm => Action::Back,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') if done => {
                shared.service_active = Some(false);
                Action::Quit
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, _shared: &mut Shared, now: Secs) {
        let mut finished = false;
        if let Phase::Running(rx) = &self.phase {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    UEvent::Started(id) => self.steps[Self::idx(id)].state = StepState::Running,
                    UEvent::Finished(id, o) => {
                        self.steps[Self::idx(id)].state = match o {
                            Outcome::Done(n) => StepState::Done(n),
                            Outcome::Skipped(n) => StepState::Skipped(n),
                            Outcome::Warn(n) => StepState::Warn(n),
                            Outcome::Failed(n) => StepState::Failed(n),
                        }
                    }
                    UEvent::Complete => finished = true,
                }
            }
            let done = self
                .steps
                .iter()
                .filter(|s| !matches!(s.state, StepState::Pending | StepState::Running))
                .count();
            let target = done as f32 / self.steps.len() as f32;
            if (self.progress.target() - target).abs() > f32::EPSILON {
                self.progress = self.progress.to(target, now, 0.3);
            }
        }
        if finished {
            self.phase = Phase::Done;
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let [_, list, msg] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .areas(area);
        step_list(f, list, th, &self.steps, self.progress.value(now), now);
        if matches!(self.phase, Phase::Done) {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  RackScreen removed. Press ⏎ to exit.",
                    th.good(),
                ))),
                msg,
            );
        }
        if matches!(self.phase, Phase::Confirm) {
            let lines = vec![
                "This removes:".into(),
                "  the systemd service (stopped and disabled)".into(),
                "  /etc/rackscreen (your config)".into(),
                "  /usr/local/bin/rackscreen".into(),
                String::new(),
                "Boot file lines (SPI overlays) are left in place.".into(),
            ];
            confirm_dialog(
                f,
                area,
                th,
                "Uninstall RackScreen?",
                &lines,
                "y remove   n/Esc cancel",
                true,
            );
        }
    }

    fn keys(&self) -> String {
        match self.phase {
            Phase::Confirm => "y remove  n cancel".into(),
            Phase::Running(_) => "please wait".into(),
            Phase::Done => "⏎ exit".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Uninstall".into()
    }
    fn animating(&self, now: Secs) -> bool {
        matches!(self.phase, Phase::Running(_)) || !self.progress.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn confirm_dialog_shows_first() {
        let sh = Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: Some(true),
            banner: None,
        };
        let screen = Uninstall::new(&sh);
        let mut term = Terminal::new(TestBackend::new(70, 18)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("Uninstall RackScreen?"));
        assert!(text.contains("/etc/rackscreen"));
        assert!(text.contains("left in place"));
    }
}

//! Install screen: runs the installer on a worker thread and animates the step list.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::ops::boot::BootChange;
use crate::ops::install::{Event, Installer, Outcome, StepId};
use crate::ops::paths::{current_exe, service_user, Paths};
use crate::ops::shell::RealShell;
use crate::widgets::{confirm_dialog, step_list, StepState, StepView};
use crate::{Action, Screen, ScreenId, Shared};

enum Phase {
    Running,
    AskBoot(Vec<BootChange>),
    Finished { reboot: bool, ok: bool },
}

pub struct Install {
    steps: Vec<StepView>,
    events: Receiver<Event>,
    replies: Sender<bool>,
    phase: Phase,
    progress: Slide,
}

impl Install {
    pub fn new(shared: &Shared) -> Install {
        let (etx, erx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        let steps = StepId::ALL
            .iter()
            .map(|s| StepView {
                title: s.title().into(),
                state: StepState::Pending,
            })
            .collect();
        let installer = Installer {
            sh: Arc::new(RealShell),
            paths: Paths::system(),
            user: service_user(),
            self_exe: current_exe().unwrap_or_default(),
        };
        let _ = &shared.ctx; // config path is fixed to the system path for install
        std::thread::Builder::new()
            .name("install".into())
            .spawn(move || installer.run(etx, rrx))
            .expect("spawn installer");
        Install {
            steps,
            events: erx,
            replies: rtx,
            phase: Phase::Running,
            progress: Slide::fixed(0.0),
        }
    }

    /// Build a screen in a given state without a worker (tests and previews).
    pub fn preview(steps: Vec<StepView>) -> Install {
        let (_etx, erx) = mpsc::channel();
        let (rtx, _rrx) = mpsc::channel();
        Install {
            steps,
            events: erx,
            replies: rtx,
            phase: Phase::Running,
            progress: Slide::fixed(0.4),
        }
    }

    fn idx(id: StepId) -> usize {
        StepId::ALL.iter().position(|s| *s == id).unwrap_or(0)
    }

    fn done_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| !matches!(s.state, StepState::Pending | StepState::Running))
            .count()
    }
}

fn state_of(o: Outcome) -> StepState {
    match o {
        Outcome::Done(n) => StepState::Done(n),
        Outcome::Skipped(n) => StepState::Skipped(n),
        Outcome::Warn(n) => StepState::Warn(n),
        Outcome::Failed(n) => StepState::Failed(n),
    }
}

impl Screen for Install {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        // Decide on copies of the phase flags first; matching on `&self.phase` would keep
        // it borrowed while the arms assign to it.
        let asking = matches!(self.phase, Phase::AskBoot(_));
        let finished_ok = matches!(self.phase, Phase::Finished { ok: true, .. });
        let finished = matches!(self.phase, Phase::Finished { .. });
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') if asking => {
                let _ = self.replies.send(true);
                self.phase = Phase::Running;
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('n') if asking => {
                let _ = self.replies.send(false);
                self.phase = Phase::Running;
                Action::None
            }
            KeyCode::Enter if finished_ok => {
                shared.service_active = Some(true);
                Action::Go(ScreenId::Calibrate)
            }
            KeyCode::Esc | KeyCode::Char('q') if finished => Action::Back,
            _ => Action::None, // while running, let it finish
        }
    }

    fn tick(&mut self, shared: &mut Shared, now: Secs) {
        loop {
            match self.events.try_recv() {
                Ok(ev) => {
                    match ev {
                        Event::Started(id) => self.steps[Self::idx(id)].state = StepState::Running,
                        Event::Finished(id, o) => self.steps[Self::idx(id)].state = state_of(o),
                        Event::AskBoot(changes) => self.phase = Phase::AskBoot(changes),
                        Event::Complete { reboot_needed } => {
                            let ok = !self
                                .steps
                                .iter()
                                .any(|s| matches!(s.state, StepState::Failed(_)));
                            if reboot_needed {
                                shared.banner = Some("reboot required to enable SPI".into());
                            }
                            shared.service_active = Some(ok);
                            self.phase = Phase::Finished {
                                reboot: reboot_needed,
                                ok,
                            };
                        }
                    }
                    let target = self.done_count() as f32 / self.steps.len() as f32;
                    if (self.progress.target() - target).abs() > f32::EPSILON {
                        self.progress = self.progress.to(target, now, 0.3);
                    }
                }
                Err(TryRecvError::Empty) => break,
                // The worker died without reporting: don't sit in Running forever.
                Err(TryRecvError::Disconnected) => {
                    if matches!(self.phase, Phase::Running | Phase::AskBoot(_)) {
                        if let Some(s) = self
                            .steps
                            .iter_mut()
                            .find(|s| matches!(s.state, StepState::Running))
                        {
                            s.state = StepState::Failed("worker stopped unexpectedly".into());
                        }
                        self.phase = Phase::Finished {
                            reboot: false,
                            ok: false,
                        };
                    }
                    break;
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let [_, list, msg] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .areas(area);
        step_list(f, list, th, &self.steps, self.progress.value(now), now);
        let line = match &self.phase {
            Phase::Running => Line::from(Span::styled("  installing...", th.muted())),
            Phase::AskBoot(_) => Line::from(Span::styled("  waiting for confirmation", th.muted())),
            Phase::Finished { ok: false, .. } => Line::from(Span::styled("  install failed, see the step above", th.bad())),
            Phase::Finished { reboot: true, .. } => Line::from(Span::styled("  done. Reboot, then run `rackscreen` again to calibrate.  ⏎ calibrate now  Esc menu", th.warning())),
            Phase::Finished { .. } => Line::from(Span::styled("  done.  ⏎ calibrate screens now   Esc back to menu", th.good())),
        };
        f.render_widget(Paragraph::new(line), msg);
        if let Phase::AskBoot(changes) = &self.phase {
            let mut lines: Vec<String> =
                vec!["Edit the boot files to enable SPI?".into(), String::new()];
            lines.extend(changes.iter().map(BootChange::describe));
            lines.push(String::new());
            lines.push("A reboot is needed afterwards.".into());
            confirm_dialog(
                f,
                area,
                th,
                "Boot files",
                &lines,
                "y/⏎ yes   n/Esc skip",
                false,
            );
        }
    }

    fn keys(&self) -> String {
        match self.phase {
            Phase::Running => "please wait".into(),
            Phase::AskBoot(_) => "y yes  n skip".into(),
            Phase::Finished { .. } => "⏎ calibrate  Esc menu".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Install".into()
    }
    fn animating(&self, now: Secs) -> bool {
        matches!(self.phase, Phase::Running) || !self.progress.done(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::Ctx;
    use rackscreen_app::logs::LogSink;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn step_list_renders_states() {
        let sh = Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
        };
        let steps = vec![
            StepView {
                title: "Check platform".into(),
                state: StepState::Done("Raspberry Pi".into()),
            },
            StepView {
                title: "Install binary".into(),
                state: StepState::Running,
            },
            StepView {
                title: "Write config".into(),
                state: StepState::Pending,
            },
            StepView {
                title: "Boot".into(),
                state: StepState::Failed("no permission".into()),
            },
        ];
        let screen = Install::preview(steps);
        let mut term = Terminal::new(TestBackend::new(70, 16)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("✓  Check platform"));
        assert!(text.contains("Raspberry Pi"));
        assert!(text.contains("○  Write config"));
        assert!(text.contains("✗  Boot"));
        assert!(text.contains("no permission"));
    }

    #[test]
    fn dead_worker_finishes_the_screen() {
        let mut sh = Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
        };
        // `preview` drops the sender, so the receiver is disconnected right away: the same
        // state a panicking worker thread leaves behind.
        let mut screen = Install::preview(vec![
            StepView {
                title: "Check platform".into(),
                state: StepState::Done("Raspberry Pi".into()),
            },
            StepView {
                title: "Install binary".into(),
                state: StepState::Running,
            },
            StepView {
                title: "Write config".into(),
                state: StepState::Pending,
            },
        ]);
        screen.tick(&mut sh, 0.0);
        assert!(matches!(screen.phase, Phase::Finished { ok: false, .. }));
        assert!(matches!(screen.steps[1].state, StepState::Failed(_)));
        assert!(matches!(screen.steps[2].state, StepState::Pending));
    }
}

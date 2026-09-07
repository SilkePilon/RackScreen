//! Update screen: downloads the latest release on a worker thread, then offers a relaunch.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::anim::Slide;
use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::RealShell;
use crate::ops::update::{Event, Outcome, StepId, Updater};
use crate::widgets::{step_list, StepState, StepView};
use crate::{Action, Screen, Shared};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Running,
    UpToDate,
    Installed(String),
    Failed,
}

pub struct Update {
    steps: Vec<StepView>,
    events: Receiver<Event>,
    phase: Phase,
    progress: Slide,
    download: Option<(u64, u64)>,
    binary: PathBuf,
    error: Option<String>,
}

impl Update {
    pub fn new(shared: &Shared) -> Update {
        let (tx, rx) = mpsc::channel();
        let updater = Updater {
            sh: Arc::new(RealShell),
            paths: Paths::system(),
            user: service_user(),
            current: shared.ctx.version.to_string(),
        };
        let binary = updater.paths.binary();
        std::thread::Builder::new()
            .name("update".into())
            .spawn(move || updater.run(tx))
            .expect("spawn updater");
        Update {
            steps: pending_steps(),
            events: rx,
            phase: Phase::Running,
            progress: Slide::fixed(0.0),
            download: None,
            binary,
            error: None,
        }
    }

    /// Build a screen in a given state without a worker (tests and previews).
    pub fn preview(steps: Vec<StepView>, phase: Phase, download: Option<(u64, u64)>) -> Update {
        let (_tx, rx) = mpsc::channel();
        Update {
            steps,
            events: rx,
            phase,
            progress: Slide::fixed(0.4),
            download,
            binary: Paths::system().binary(),
            error: None,
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

    /// Hand the terminal back and become the new binary. Only returns when `exec` failed.
    fn relaunch(&mut self, shared: &mut Shared) -> Action {
        use std::os::unix::process::CommandExt;
        ratatui::restore();
        let err = std::process::Command::new(&self.binary).arg("setup").exec();
        // `exec` only returns on failure: take the terminal back and stay put.
        let _ = ratatui::try_init();
        shared.redraw = true;
        self.error = Some(format!(
            "relaunch failed: {err}. Run `{} setup` yourself.",
            self.binary.display()
        ));
        Action::None
    }
}

fn pending_steps() -> Vec<StepView> {
    StepId::ALL
        .iter()
        .map(|s| StepView {
            title: s.title().into(),
            state: StepState::Pending,
        })
        .collect()
}

fn state_of(o: Outcome) -> StepState {
    match o {
        Outcome::Done(n) => StepState::Done(n),
        Outcome::Skipped(n) => StepState::Skipped(n),
        Outcome::Warn(n) => StepState::Warn(n),
        Outcome::Failed(n) => StepState::Failed(n),
    }
}

impl Screen for Update {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let installed = matches!(self.phase, Phase::Installed(_));
        match key.code {
            // Nothing is interruptible while the binary is being swapped.
            _ if matches!(self.phase, Phase::Running) => Action::None,
            KeyCode::Enter if installed => self.relaunch(shared),
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => Action::Back,
            _ => Action::None,
        }
    }

    fn tick(&mut self, shared: &mut Shared, now: Secs) {
        loop {
            match self.events.try_recv() {
                Ok(ev) => {
                    match ev {
                        Event::Started(id) => self.steps[Self::idx(id)].state = StepState::Running,
                        Event::Progress { done, total } => self.download = Some((done, total)),
                        Event::Finished(id, o) => self.steps[Self::idx(id)].state = state_of(o),
                        Event::Complete { installed, ok } => {
                            self.phase = match (ok, installed) {
                                (true, Some(tag)) => {
                                    shared.update = None;
                                    Phase::Installed(tag)
                                }
                                (true, None) => Phase::UpToDate,
                                (false, _) => Phase::Failed,
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
                    if matches!(self.phase, Phase::Running) {
                        if let Some(s) = self
                            .steps
                            .iter_mut()
                            .find(|s| matches!(s.state, StepState::Running))
                        {
                            s.state = StepState::Failed("worker stopped unexpectedly".into());
                        }
                        self.phase = Phase::Failed;
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
        let mut lines = vec![match &self.phase {
            Phase::Running => match self.download {
                Some((done, total)) if total > 0 => Line::from(Span::styled(
                    format!("  downloading {} %", done * 100 / total),
                    th.muted(),
                )),
                _ => Line::from(Span::styled("  working...", th.muted())),
            },
            Phase::UpToDate => Line::from(Span::styled(
                "  already up to date.   Esc back to menu",
                th.good(),
            )),
            Phase::Installed(tag) => Line::from(Span::styled(
                format!("  installed {tag}, press ⏎ to restart the setup UI   Esc menu"),
                th.good(),
            )),
            Phase::Failed => Line::from(Span::styled(
                "  update failed, see the step above.   Esc back to menu",
                th.bad(),
            )),
        }];
        if let Some(e) = &self.error {
            lines.push(Line::from(Span::styled(format!("  {e}"), th.bad())));
        }
        f.render_widget(Paragraph::new(lines), msg);
    }

    fn keys(&self) -> String {
        match self.phase {
            Phase::Running => "please wait".into(),
            Phase::Installed(_) => "⏎ restart  Esc menu".into(),
            _ => "Esc back".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Update".into()
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

    fn shared() -> Shared {
        Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.3.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        }
    }

    fn render(screen: &Update) -> String {
        let sh = shared();
        let mut term = Terminal::new(TestBackend::new(74, 16)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        term.backend().to_string()
    }

    #[test]
    fn up_to_date_state_renders() {
        let mut steps = pending_steps();
        steps[0].state = StepState::Skipped("already up to date (v0.3.0)".into());
        let text = render(&Update::preview(steps, Phase::UpToDate, None));
        assert!(text.contains("Check the latest release"), "{text}");
        assert!(text.contains("already up to date"), "{text}");
    }

    #[test]
    fn downloading_state_shows_progress() {
        let mut steps = pending_steps();
        steps[0].state = StepState::Done("v0.3.0 → v0.3.1".into());
        steps[1].state = StepState::Done("aarch64, /usr/local/bin/rackscreen".into());
        steps[2].state = StepState::Running;
        let screen = Update::preview(steps, Phase::Running, Some((4_000_000, 10_000_000)));
        let text = render(&screen);
        assert!(text.contains("downloading 40 %"), "{text}");
        assert!(text.contains("v0.3.0 → v0.3.1"), "{text}");
        assert!(screen.animating(0.0));
    }

    #[test]
    fn platform_failure_is_shown_and_escapable() {
        let mut sh = shared();
        let mut steps = pending_steps();
        steps[0].state = StepState::Done("v0.3.0 → v0.3.1".into());
        steps[1].state = StepState::Failed(
            crate::ops::update::platform_ok("x86_64", false)
                .unwrap_err()
                .to_string(),
        );
        let mut screen = Update::preview(steps, Phase::Failed, None);
        let text = render(&screen);
        assert!(text.contains("aarch64 binary"), "{text}");
        assert!(text.contains("update failed"), "{text}");
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::Back
        ));
    }

    #[test]
    fn dead_worker_finishes_the_screen() {
        let mut sh = shared();
        let mut steps = pending_steps();
        steps[0].state = StepState::Running;
        // `preview` drops the sender, the same state a panicking worker leaves behind.
        let mut screen = Update::preview(steps, Phase::Running, None);
        screen.tick(&mut sh, 0.0);
        assert_eq!(screen.phase, Phase::Failed);
        assert!(matches!(screen.steps[0].state, StepState::Failed(_)));
        // Keys are ignored while running, but work once it has stopped.
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0),
            Action::Back
        ));
    }
}

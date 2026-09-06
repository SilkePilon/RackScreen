//! Status: service state, boot readiness, link dots and a log tail, refreshed every 2 s.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ops::boot::{readiness, Readiness};
use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::{RealShell, Shell};
use crate::ops::systemd::{links_from_logs, Dot, LinkDots, ServiceInfo, Systemd};
use crate::{Action, Screen, Shared};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub info: ServiceInfo,
    pub enabled: bool,
    pub binary_present: bool,
    pub config_present: bool,
    pub ready: Option<Readiness>,
    pub logs: Vec<String>,
    pub links: LinkDots,
}

pub fn collect(sh: &dyn Shell, paths: &Paths, user: &str) -> Snapshot {
    let sd = Systemd::new(sh, user);
    let info = sd.info().unwrap_or_default();
    let enabled = sd.is_enabled().unwrap_or(false);
    let logs = sd.journal_tail(60).unwrap_or_default();
    let ready = match (paths.config_txt(), paths.cmdline_txt()) {
        (Some(c), Some(l)) => Some(readiness(
            &std::fs::read_to_string(c).unwrap_or_default(),
            &std::fs::read_to_string(l).unwrap_or_default(),
        )),
        _ => None,
    };
    Snapshot {
        links: links_from_logs(&logs),
        info,
        enabled,
        binary_present: paths.binary().exists(),
        config_present: paths.config().exists(),
        ready,
        logs,
    }
}

pub struct Status {
    snap: Option<Snapshot>,
    rx: Receiver<Snapshot>,
    log_view: bool,
    scroll: usize,
    restart_note: Option<String>,
}

impl Status {
    pub fn new(_shared: &Shared) -> Status {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("status".into())
            .spawn(move || {
                let sh = RealShell;
                let paths = Paths::system();
                let user = service_user();
                loop {
                    if tx.send(collect(&sh, &paths, &user)).is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            })
            .expect("spawn status");
        Status {
            snap: None,
            rx,
            log_view: false,
            scroll: 0,
            restart_note: None,
        }
    }

    pub fn with_snapshot(snap: Snapshot) -> Status {
        let (_tx, rx) = mpsc::channel();
        Status {
            snap: Some(snap),
            rx,
            log_view: false,
            scroll: 0,
            restart_note: None,
        }
    }
}

fn dot_style(d: Dot, th: &crate::theme::Theme) -> Style {
    match d {
        Dot::Up => th.good(),
        Dot::Down => th.bad(),
        Dot::Unknown => th.muted(),
    }
}

fn fmt_uptime(secs: u64) -> String {
    if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

impl Screen for Status {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.log_view {
                    self.log_view = false;
                    Action::None
                } else {
                    Action::Back
                }
            }
            KeyCode::Char('l') => {
                self.log_view = !self.log_view;
                self.scroll = 0;
                Action::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_add(1);
                Action::None
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('r') => {
                let sh = RealShell;
                self.restart_note = Some(match Systemd::new(&sh, &service_user()).restart() {
                    Ok(()) => "service restarted".into(),
                    Err(e) => format!("restart failed: {e:#}"),
                });
                let _ = &shared;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        while let Ok(s) = self.rx.try_recv() {
            shared.service_active = Some(s.info.active == "active");
            self.snap = Some(s);
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let Some(s) = &self.snap else {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled("  collecting...", th.muted()))),
                area,
            );
            return;
        };
        if self.log_view {
            let lines: Vec<Line> = s
                .logs
                .iter()
                .rev()
                .skip(self.scroll)
                .take(area.height as usize)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|l| Line::from(Span::styled(l.clone(), th.normal())))
                .collect();
            f.render_widget(Paragraph::new(lines), area);
            return;
        }
        let [_, top, logs] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Min(3),
        ])
        .areas(area);
        let (svc_style, svc_text) = match s.info.active.as_str() {
            "active" => (th.good(), format!("active ({})", s.info.sub)),
            "failed" => (th.bad(), "failed".to_string()),
            other => (
                th.warning(),
                if other.is_empty() {
                    "not installed".into()
                } else {
                    other.to_string()
                },
            ),
        };
        let mut lines = vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(g.dot, svc_style),
                Span::styled(format!(" service {svc_text}"), th.normal()),
                Span::styled(
                    s.info
                        .uptime_secs
                        .map(|u| format!("   up {}", fmt_uptime(u)))
                        .unwrap_or_default(),
                    th.muted(),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(g.dot, if s.enabled { th.good() } else { th.muted() }),
                Span::styled(
                    if s.enabled {
                        " starts at boot"
                    } else {
                        " not enabled at boot"
                    },
                    th.normal(),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    g.dot,
                    if s.binary_present {
                        th.good()
                    } else {
                        th.bad()
                    },
                ),
                Span::styled(" /usr/local/bin/rackscreen", th.normal()),
                Span::raw("   "),
                Span::styled(
                    g.dot,
                    if s.config_present {
                        th.good()
                    } else {
                        th.bad()
                    },
                ),
                Span::styled(" /etc/rackscreen/config.yaml", th.normal()),
            ]),
        ];
        match s.ready {
            Some(r) => lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(g.dot, if r.spi_on { th.good() } else { th.bad() }),
                Span::styled(" spi on   ", th.normal()),
                Span::styled(g.dot, if r.spi1_overlay { th.good() } else { th.bad() }),
                Span::styled(" spi1 overlay   ", th.normal()),
                Span::styled(g.dot, if r.bufsiz { th.good() } else { th.warning() }),
                Span::styled(" spidev.bufsiz", th.normal()),
            ])),
            None => lines.push(Line::from(Span::styled(
                "  boot files: not a Raspberry Pi",
                th.muted(),
            ))),
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(g.dot, dot_style(s.links.api, th)),
            Span::styled(" kubernetes   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.prometheus, th)),
            Span::styled(" prometheus   ", th.normal()),
            Span::styled(g.dot, dot_style(s.links.qbittorrent, th)),
            Span::styled(" qbittorrent", th.normal()),
        ]));
        if let Some(n) = &self.restart_note {
            lines.push(Line::from(Span::styled(format!("  {n}"), th.warning())));
        }
        f.render_widget(Paragraph::new(lines), top);
        let tail: Vec<Line> = s
            .logs
            .iter()
            .rev()
            .take(logs.height as usize)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|l| Line::from(Span::styled(format!("  {l}"), th.faint_style())))
            .collect();
        f.render_widget(Paragraph::new(tail), logs);
    }

    fn keys(&self) -> String {
        if self.log_view {
            "↑↓ scroll  Esc back".into()
        } else {
            "r restart  l logs  Esc back".into()
        }
    }
    fn subtitle(&self) -> String {
        "Status".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::shell::{FakeShell, Output};
    use crate::theme::Theme;
    use crate::Ctx;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn collect_uses_shell_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(dir.path().join("boot/firmware")).unwrap();
        std::fs::write(
            dir.path().join("boot/firmware/config.txt"),
            "dtparam=spi=on\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("boot/firmware/cmdline.txt"), "root=x\n").unwrap();
        let sh = FakeShell::new();
        sh.respond(
            "systemctl show",
            Output::ok("ActiveState=active\nSubState=running\nActiveEnterTimestampMonotonic=1\n"),
        );
        sh.respond(
            "journalctl",
            Output::ok("prometheus: forwarding to monitoring/p\n"),
        );
        let s = collect(&sh, &paths, "pi");
        assert_eq!(s.info.active, "active");
        assert_eq!(s.links.prometheus, Dot::Up);
        assert!(s.ready.unwrap().spi_on);
        assert!(!s.binary_present);
    }

    #[test]
    fn renders_snapshot() {
        let snap = Snapshot {
            info: ServiceInfo {
                active: "active".into(),
                sub: "running".into(),
                uptime_secs: Some(4000),
            },
            enabled: true,
            binary_present: true,
            config_present: true,
            ready: Some(Readiness {
                spi_on: true,
                spi1_overlay: false,
                bufsiz: true,
            }),
            logs: vec!["line one".into(), "line two".into()],
            links: LinkDots {
                api: Dot::Up,
                prometheus: Dot::Down,
                qbittorrent: Dot::Unknown,
            },
        };
        let sh = Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: false,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
        };
        let screen = Status::with_snapshot(snap);
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("service active (running)"));
        assert!(text.contains("up 1h 06m"));
        assert!(text.contains("spi1 overlay"));
        assert!(text.contains("line two"));
        assert_eq!(fmt_uptime(90_000), "1d 1h");
    }
}

//! Home: the main menu items (drawn by the shell's sidebar) and the live overview pane.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::{Action, Screen, ScreenId, Shared};

pub const ITEMS: [(ScreenId, &str, &str); 8] = [
    (ScreenId::Install, "Install", "set up service + config"),
    (ScreenId::Calibrate, "Calibrate", "fix rotation / mirroring"),
    (
        ScreenId::Screens,
        "Screens",
        "what each screen shows, cycling",
    ),
    (
        ScreenId::Configure,
        "Configure",
        "cluster, services, sky, display",
    ),
    (ScreenId::Status, "Status", "service, links, logs"),
    (ScreenId::RunHere, "Run here", "foreground with logs"),
    (
        ScreenId::Update,
        "Update",
        "download and install the latest release",
    ),
    (ScreenId::Uninstall, "Uninstall", "remove everything"),
];

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use rackscreen_app::config::Config;
use rackscreen_core::theme::Role;

use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::RealShell;
use crate::ops::systemd::Dot;
use crate::screens::status::{collect, Snapshot};

const SNAPSHOT_SECS: u64 = 10;

/// Join role names with ` › ` and drop whole names from the end until the text (plus a
/// ` +N` count for the dropped ones) fits in `width`.
pub fn fit_roles(names: &[&str], width: usize) -> String {
    let mut out = String::new();
    for (i, n) in names.iter().enumerate() {
        let candidate = if i == 0 {
            n.to_string()
        } else {
            format!("{out} › {n}")
        };
        let rest = names.len() - i - 1;
        let tail = if rest > 0 {
            format!(" +{rest}").chars().count()
        } else {
            0
        };
        if candidate.chars().count() + tail > width {
            let dropped = names.len() - i;
            return if out.is_empty() {
                format!("+{dropped}")
            } else {
                format!("{out} +{dropped}")
            };
        }
        out = candidate;
    }
    out
}

pub struct Home {
    rows: Vec<(Vec<Role>, u64)>,
    snap: Option<Snapshot>,
    rx: Option<Receiver<Snapshot>>,
}

impl Home {
    pub fn new(shared: &Shared) -> Home {
        let rows = Config::load_or_default(&shared.ctx.config_path)
            .map(|c| {
                c.screens
                    .iter()
                    .map(|s| (s.roles().unwrap_or_default(), s.cycle_secs))
                    .collect()
            })
            .unwrap_or_default();
        let rx = (!shared.ctx.sim).then(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("home".into())
                .spawn(move || {
                    let sh = RealShell;
                    let paths = Paths::system();
                    let user = service_user();
                    loop {
                        if tx.send(collect(&sh, &paths, &user)).is_err() {
                            return;
                        }
                        std::thread::sleep(Duration::from_secs(SNAPSHOT_SECS));
                    }
                })
                .expect("spawn home");
            rx
        });
        Home {
            rows,
            snap: None,
            rx,
        }
    }

    /// A Home with fixed data and no collector thread (tests).
    pub fn with_snapshot(rows: Vec<(Vec<Role>, u64)>, snap: Option<Snapshot>) -> Home {
        Home {
            rows,
            snap,
            rx: None,
        }
    }
}

fn dot_style(d: Dot, th: &crate::theme::Theme) -> ratatui::style::Style {
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

const LABEL_W: usize = 13;

fn labelled<'a>(th: &crate::theme::Theme, label: &str, spans: Vec<Span<'a>>) -> Line<'a> {
    let mut all = vec![
        Span::raw("  "),
        Span::styled(format!("{label:<LABEL_W$}"), th.muted()),
    ];
    all.extend(spans);
    Line::from(all)
}

impl Screen for Home {
    fn handle(&mut self, _key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        // The shell drives the sidebar; Home has no keys of its own.
        Action::None
    }

    fn tick(&mut self, shared: &mut Shared, _now: Secs) {
        let Some(rx) = &self.rx else { return };
        while let Ok(s) = rx.try_recv() {
            shared.service_active = Some(s.info.active == "active");
            self.snap = Some(s);
        }
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let mut lines: Vec<Line> = vec![Line::from("")];
        match &self.snap {
            Some(s) => {
                let (style, word) = match s.info.active.as_str() {
                    "active" => (th.good(), "active".to_string()),
                    "failed" => (th.bad(), "failed".to_string()),
                    "" => (th.warning(), "not installed".to_string()),
                    other => (th.warning(), other.to_string()),
                };
                let mut svc = vec![
                    Span::styled(g.dot, style),
                    Span::styled(format!(" {word}"), th.normal()),
                    Span::styled(
                        s.info
                            .uptime_secs
                            .map(|u| format!("  up {}", fmt_uptime(u)))
                            .unwrap_or_default(),
                        th.muted(),
                    ),
                ];
                if let Some(u) = &shared.update {
                    if u.newer {
                        svc.push(Span::styled(
                            format!("     {} {}", g.arrow_up, u.latest),
                            th.warning(),
                        ));
                    }
                }
                lines.push(labelled(th, "Service", svc));
                lines.push(labelled(
                    th,
                    "Boot",
                    vec![
                        Span::styled(g.dot, if s.enabled { th.good() } else { th.muted() }),
                        Span::styled(
                            if s.enabled {
                                " enabled"
                            } else {
                                " not enabled"
                            },
                            th.normal(),
                        ),
                    ],
                ));
                let link = |d: Dot, name: &str| {
                    vec![
                        Span::styled(g.dot.to_string(), dot_style(d, th)),
                        Span::styled(format!(" {name}  "), th.normal()),
                    ]
                };
                let mut row1 = Vec::new();
                row1.extend(link(s.links.api, "k8s"));
                row1.extend(link(s.links.prometheus, "prometheus"));
                row1.extend(link(s.links.qbittorrent, "qbittorrent"));
                row1.extend(link(s.links.argocd, "argocd"));
                lines.push(labelled(th, "Links", row1));
                let mut row2 = Vec::new();
                row2.extend(link(s.links.weather, "weather"));
                row2.extend(link(s.links.rain, "rain"));
                row2.extend(link(s.links.github, "github"));
                row2.extend(link(s.links.electricity, "electricity"));
                row2.extend(link(s.links.prices, "prices"));
                lines.push(labelled(th, "", row2));
            }
            None => {
                let msg = if shared.ctx.sim {
                    "not available (sim)"
                } else {
                    "collecting…"
                };
                lines.push(labelled(th, "Service", vec![Span::styled(msg, th.muted())]));
            }
        }
        lines.push(Line::from(""));
        // Screens: number in the panel colour, roles fitted to the width, timing right.
        let timing_w = 8;
        let roles_w = (area.width as usize).saturating_sub(2 + LABEL_W + 3 + timing_w + 2);
        for (i, (roles, secs)) in self.rows.iter().enumerate() {
            let names: Vec<&str> = roles.iter().map(|r| r.name()).collect();
            let fitted = fit_roles(&names, roles_w);
            let timing = if roles.len() > 1 {
                format!("{secs} s")
            } else {
                "static".to_string()
            };
            lines.push(labelled(
                th,
                if i == 0 { "Screens" } else { "" },
                vec![
                    Span::styled(format!("{} ", i + 1), Style::new().fg(th.panel_color(i))),
                    Span::styled(format!("{fitted:<roles_w$}"), th.normal()),
                    Span::styled(format!("  {timing}"), th.muted()),
                ],
            ));
        }
        if self.rows.is_empty() {
            lines.push(labelled(
                th,
                "Screens",
                vec![Span::styled("none configured", th.muted())],
            ));
        }
        lines.push(Line::from(""));
        match &shared.banner {
            Some(b) => lines.push(labelled(
                th,
                "",
                vec![Span::styled(b.clone(), th.warning())],
            )),
            None => {
                if let Some(s) = &self.snap {
                    let tail = s.logs.iter().rev().take(3).collect::<Vec<_>>();
                    for (k, l) in tail.into_iter().rev().enumerate() {
                        lines.push(labelled(
                            th,
                            if k == 0 { "Recent" } else { "" },
                            vec![Span::styled(l.clone(), th.faint_style())],
                        ));
                    }
                }
            }
        }
        f.render_widget(Paragraph::new(lines), area);
    }

    fn keys(&self) -> String {
        "↑↓ move   ⏎ open   q quit".into()
    }
    fn subtitle(&self) -> String {
        "Home".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::systemd::{Dot, LinkDots, ServiceInfo};
    use crate::screens::status::Snapshot;
    use crate::theme::Theme;
    use crate::Ctx;
    use rackscreen_app::logs::LogSink;
    use rackscreen_core::theme::Role;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn shared() -> Shared {
        Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.5.0",
            },
            theme: Theme::new(true),
            service_active: Some(true),
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            info: ServiceInfo {
                active: "active".into(),
                sub: "running".into(),
                uptime_secs: Some(3 * 86_400 + 4 * 3600),
            },
            enabled: true,
            binary_present: true,
            config_present: true,
            ready: None,
            logs: vec![
                "12:00:31 rain  dry".into(),
                "12:00:49 k8s   142 pods".into(),
                "12:01:04 prom  7 nodes".into(),
                "12:01:20 net   ok".into(),
            ],
            links: LinkDots {
                api: Dot::Up,
                prometheus: Dot::Up,
                qbittorrent: Dot::Down,
                argocd: Dot::Unknown,
                electricity: Dot::Unknown,
                prices: Dot::Unknown,
                weather: Dot::Up,
                rain: Dot::Up,
                github: Dot::Unknown,
            },
        }
    }

    #[test]
    fn fit_roles_truncates_whole_names_with_a_count() {
        let names = ["cpu", "mem", "pods"];
        assert_eq!(fit_roles(&names, 40), "cpu › mem › pods");
        assert_eq!(fit_roles(&names, 16), "cpu › mem › pods");
        assert_eq!(fit_roles(&names, 15), "cpu › mem +1");
        assert_eq!(fit_roles(&names, 12), "cpu › mem +1");
        assert_eq!(fit_roles(&names, 11), "cpu +2");
        assert_eq!(fit_roles(&names, 5), "+3");
        assert_eq!(fit_roles(&[], 10), "");
    }

    #[test]
    fn home_renders_service_screens_and_logs() {
        let sh = shared();
        let rows = vec![
            (
                vec![
                    Role::Cpu,
                    Role::Mem,
                    Role::Pods,
                    Role::Health,
                    Role::Thermal,
                    Role::Storage,
                    Role::Net,
                    Role::Ups,
                    Role::Deploys,
                ],
                15,
            ),
            (vec![Role::Health], 15),
        ];
        let home = Home::with_snapshot(rows, Some(snapshot()));
        let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();
        term.draw(|f| home.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("● active"), "{t}");
        assert!(t.contains("up 3d 4h"));
        assert!(t.contains("● enabled"));
        assert!(t.contains("prometheus"));
        assert!(
            t.contains("+"),
            "long role list is truncated with a count: {t}"
        );
        assert!(t.contains("15 s"));
        assert!(t.contains("static"));
        assert!(t.contains("12:01:20 net"), "newest log line shown");
        assert!(
            !t.contains("12:00:31 rain"),
            "only the last three log lines"
        );
    }

    #[test]
    fn home_without_snapshot_says_collecting_or_sim() {
        let mut sh = shared();
        let home = Home::with_snapshot(Vec::new(), None);
        let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();
        term.draw(|f| home.draw(f, f.area(), &sh, 0.0)).unwrap();
        assert!(term.backend().to_string().contains("not available (sim)"));
        sh.ctx.sim = false;
        term.draw(|f| home.draw(f, f.area(), &sh, 0.0)).unwrap();
        assert!(term.backend().to_string().contains("collecting"));
    }

    #[test]
    fn home_shows_update_and_banner() {
        let mut sh = shared();
        sh.update = Some(crate::ops::update::UpdateInfo {
            latest: "v9.9.9".into(),
            newer: true,
        });
        sh.banner = Some("config saved".into());
        let home = Home::with_snapshot(Vec::new(), Some(snapshot()));
        let mut term = Terminal::new(TestBackend::new(70, 20)).unwrap();
        term.draw(|f| home.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("↑ v9.9.9"), "{t}");
        assert!(t.contains("config saved"));
    }
}

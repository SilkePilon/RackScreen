//! Calibrate: drive the panels with a test pattern and adjust rotate/hflip per screen.

use rackscreen_app::calibrate::test_pattern;
use rackscreen_app::config::Config;
use rackscreen_app::panels::{open_panels, Panels};
use rackscreen_core::anim::Secs;
use rackscreen_display::DisplayCmd;
use rackscreen_render::frame::{new_pixmap, Orient, Rect};
use rackscreen_render::renderer::Renderer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect as TRect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tiny_skia::Pixmap;

use crate::ops::config_file::{save_config, set_orientation};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::{Action, Screen, Shared};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orientation {
    pub rotate: u32,
    pub hflip: bool,
}

pub struct Calibrate {
    cfg: Config,
    orient: Vec<Orientation>,
    selected: usize,
    panels: Option<Panels>,
    renderer: Option<Renderer>,
    frames: Vec<Pixmap>,
    scratch: Pixmap,
    error: Option<String>,
    service_was_active: bool,
    message: String,
    /// The file on disk could not be loaded: the panels show defaults for testing, but
    /// `s` must never overwrite the user's file with them.
    unreadable: bool,
}

/// The terminal preview of one panel after rotate/hflip: 5 text rows with an arrow,
/// the number, and a marker in the corner the amber dot ends up in.
pub fn mini_panel(rotate: u32, hflip: bool, number: usize, unicode: bool) -> Vec<String> {
    // Arrow direction after clockwise rotation, then mirrored horizontally.
    let arrows = if unicode {
        ["↑", "→", "↓", "←"]
    } else {
        ["^", ">", "v", "<"]
    };
    let turns = ((rotate % 360) / 90) as usize;
    let mut dir = turns; // 0 up, 1 right, 2 down, 3 left
    if hflip && (dir == 1 || dir == 3) {
        dir = 4 - dir;
    }
    // Marker starts top-right; rotation moves it clockwise; hflip swaps left/right.
    // Stepping is the same as the arrow's, but from the rotation alone: the flip is
    // applied below and must not compound with the arrow's mirroring.
    let mut corner = turns; // 0 TR, 1 BR, 2 BL, 3 TL
    if hflip {
        corner = match corner {
            0 => 3,
            1 => 2,
            2 => 1,
            _ => 0,
        };
    }
    let (o, sp) = if unicode { ("●", " ") } else { ("*", " ") };
    let a = arrows[dir];
    let tl = if corner == 3 { o } else { sp };
    let tr = if corner == 0 { o } else { sp };
    let bl = if corner == 2 { o } else { sp };
    let br = if corner == 1 { o } else { sp };
    vec![
        format!(" {tl}     {tr} "),
        "   .---.   ".to_string(),
        format!("   | {a} |   "),
        format!("   | {number} |   "),
        format!(" {bl} '---' {br} "),
    ]
}

impl Calibrate {
    pub fn new(shared: &Shared) -> Calibrate {
        let (cfg, error, unreadable) = match Config::load_or_default(&shared.ctx.config_path) {
            Ok(cfg) => (cfg, None, false),
            Err(e) => (
                Config::default(),
                Some(format!("config unreadable: {e:#}; fix the file by hand")),
                true,
            ),
        };
        let orient = cfg
            .screens
            .iter()
            .map(|s| Orientation {
                rotate: s.rotate,
                hflip: s.hflip,
            })
            .collect();
        let mut me = Calibrate {
            cfg,
            orient,
            selected: 0,
            panels: None,
            renderer: None,
            frames: Vec::new(),
            scratch: new_pixmap(),
            error,
            service_was_active: false,
            message: "adjust each screen until the arrow points up and the dot is top-right".into(),
            unreadable,
        };
        // Stop the service only now that `me` exists: a panic while opening the panels
        // still runs `Drop`, which starts it again.
        if !shared.ctx.sim {
            let sh = RealShell;
            let sd = Systemd::new(&sh, &service_user());
            if sd.is_active().unwrap_or(false) {
                me.service_was_active = true;
                let _ = sd.stop();
            }
        }
        let opened =
            open_panels(&me.cfg, shared.ctx.sim, false, None).and_then(|p| match Renderer::new() {
                Ok(r) => Ok((p, r)),
                Err(e) => {
                    p.shutdown();
                    Err(e)
                }
            });
        match opened {
            Ok((p, r)) => {
                me.frames = (0..p.handles.len()).map(|_| new_pixmap()).collect();
                me.panels = Some(p);
                me.renderer = Some(r);
                for i in 0..me.orient.len() {
                    me.push(i);
                }
            }
            Err(e) => {
                me.error = Some(match me.error.take() {
                    Some(prev) => format!("{prev}; {e:#}"),
                    None => format!("{e:#}"),
                })
            }
        }
        me
    }

    fn push(&mut self, i: usize) {
        let (Some(p), Some(r)) = (&self.panels, &mut self.renderer) else {
            return;
        };
        let Some(h) = p.handles.get(i) else { return };
        let o = self.orient[i];
        let base = &mut self.frames[i];
        r.render(&test_pattern(i, h.first_role()), base);
        // In the simulator the panel orientation is identity, so apply the candidate
        // orientation here; on real panels do the same (the handle's orient is only used by
        // the monitor, calibration always orients explicitly).
        Orient::new(o.rotate, o.hflip).apply(base, &mut self.scratch);
        h.mailbox
            .put(DisplayCmd::Frame(self.scratch.clone(), Rect::full()));
    }

    fn save(&mut self, shared: &mut Shared) -> Result<(), String> {
        if self.unreadable {
            return Err(
                "not saved: the existing config is unreadable; fix the file by hand".into(),
            );
        }
        for (i, o) in self.orient.iter().enumerate() {
            set_orientation(&mut self.cfg, i, o.rotate, o.hflip);
        }
        save_config(&self.cfg, &shared.ctx.config_path).map_err(|e| format!("{e:#}"))
    }

    /// Build a screen without panels or a renderer (tests).
    pub fn from_parts(cfg: Config, orient: Vec<Orientation>) -> Calibrate {
        Calibrate {
            cfg,
            orient,
            selected: 0,
            panels: None,
            renderer: None,
            frames: Vec::new(),
            scratch: new_pixmap(),
            error: None,
            service_was_active: false,
            message: String::new(),
            unreadable: false,
        }
    }

    fn close(&mut self) {
        if let Some(p) = self.panels.take() {
            p.shutdown();
        }
        if self.service_was_active {
            let sh = RealShell;
            let _ = Systemd::new(&sh, &service_user()).start();
            self.service_was_active = false;
        }
    }
}

impl Drop for Calibrate {
    fn drop(&mut self) {
        self.close();
    }
}

impl Screen for Calibrate {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, _now: Secs) -> Action {
        let n = self.orient.len();
        // With `screens: []` in the config there is nothing to select or rotate; every
        // arm below indexes or takes a modulus by `n`, so bail out before any of them.
        if n == 0 {
            return if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                self.close();
                Action::Back
            } else {
                Action::None
            };
        }
        match key.code {
            KeyCode::Char(c @ '1'..='4') => {
                let i = (c as u8 - b'1') as usize;
                if i < n {
                    self.selected = i;
                }
            }
            KeyCode::Left | KeyCode::Up => self.selected = (self.selected + n - 1) % n.max(1),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % n.max(1)
            }
            KeyCode::Char('r') => {
                let o = &mut self.orient[self.selected];
                o.rotate = (o.rotate + 90) % 360;
                self.push(self.selected);
            }
            KeyCode::Char('R') => {
                let o = &mut self.orient[self.selected];
                o.rotate = (o.rotate + 270) % 360;
                self.push(self.selected);
            }
            KeyCode::Char('f') => {
                self.orient[self.selected].hflip = !self.orient[self.selected].hflip;
                self.push(self.selected);
            }
            KeyCode::Char('a') => {
                let o = self.orient[self.selected];
                for i in 0..n {
                    self.orient[i] = o;
                    self.push(i);
                }
            }
            KeyCode::Char('s') => {
                return match self.save(shared) {
                    Ok(()) => {
                        self.close();
                        shared.banner = Some("orientation saved".into());
                        Action::Back
                    }
                    Err(e) => {
                        self.error = Some(e);
                        Action::None
                    }
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close();
                return Action::Back;
            }
            _ => {}
        }
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: TRect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let [left, right] = Layout::horizontal([
            Constraint::Length(14 * self.orient.len().max(1) as u16 + 2),
            Constraint::Min(20),
        ])
        .areas(area);
        // mini panels side by side
        let cols = Layout::horizontal(vec![Constraint::Length(14); self.orient.len().max(1)])
            .split(TRect {
                y: left.y + 1,
                height: 6,
                ..left
            });
        for (i, o) in self.orient.iter().enumerate() {
            let lines = mini_panel(o.rotate, o.hflip, i + 1, th.unicode);
            let style = if i == self.selected {
                th.selected()
            } else {
                th.muted()
            };
            let body: Vec<Line> = lines
                .into_iter()
                .map(|l| Line::from(Span::styled(l, style)))
                .collect();
            f.render_widget(Paragraph::new(body), cols[i]);
        }
        let mut info = vec![
            Line::from(Span::styled(self.message.clone(), th.normal())),
            Line::from(""),
            Line::from(vec![
                Span::styled("1-4", th.selected()),
                Span::styled(" select screen", th.muted()),
            ]),
            Line::from(vec![
                Span::styled("r / R", th.selected()),
                Span::styled(" rotate +90 / -90", th.muted()),
            ]),
            Line::from(vec![
                Span::styled("f", th.selected()),
                Span::styled(" flip horizontally", th.muted()),
            ]),
            Line::from(vec![
                Span::styled("a", th.selected()),
                Span::styled(" apply this orientation to all", th.muted()),
            ]),
            Line::from(vec![
                Span::styled("s", th.selected()),
                Span::styled(" save and exit    ", th.muted()),
                Span::styled("Esc", th.selected()),
                Span::styled(" discard", th.muted()),
            ]),
            Line::from(""),
        ];
        if let Some(o) = self.orient.get(self.selected) {
            info.push(Line::from(Span::styled(
                format!(
                    "screen {}: rotate {}  hflip {}",
                    self.selected + 1,
                    o.rotate,
                    o.hflip
                ),
                th.normal(),
            )));
        }
        if let Some(e) = &self.error {
            info.push(Line::from(Span::styled(format!("error: {e}"), th.bad())));
        }
        f.render_widget(
            Paragraph::new(info),
            TRect {
                y: right.y + 1,
                height: right.height.saturating_sub(1),
                ..right
            },
        );
    }

    fn keys(&self) -> String {
        "1-4 screen  r/R rotate  f flip  a all  s save  Esc discard".into()
    }
    fn subtitle(&self) -> String {
        "Calibrate screens".into()
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
    fn mini_panel_follows_rotation_and_flip() {
        let up = mini_panel(0, false, 1, true);
        assert!(up[2].contains('↑'));
        assert!(up[0].ends_with("● "), "marker top-right: {:?}", up[0]);
        let right = mini_panel(90, false, 2, true);
        assert!(right[2].contains('→'));
        assert!(right[4].ends_with("● "), "marker bottom-right after 90 cw");
        let flipped = mini_panel(0, true, 3, true);
        assert!(flipped[2].contains('↑'));
        assert!(
            flipped[0].starts_with(" ●"),
            "marker top-left when mirrored"
        );
        let both = mini_panel(90, true, 4, true);
        assert!(both[2].contains('←'), "right arrow mirrored becomes left");
        assert!(both[4].starts_with(" ●"));
        assert!(mini_panel(0, false, 1, false)[2].contains('^'));
    }

    #[test]
    fn no_screens_does_not_panic() {
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
        let mut screen = Calibrate::from_parts(Config::default(), Vec::new());
        for code in [
            KeyCode::Up,
            KeyCode::Left,
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Tab,
            KeyCode::Char('1'),
            KeyCode::Char('r'),
            KeyCode::Char('R'),
            KeyCode::Char('f'),
            KeyCode::Char('a'),
        ] {
            assert!(matches!(
                screen.handle(KeyEvent::from(code), &mut sh, 0.0),
                Action::None
            ));
        }
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::Back
        ));
        // Drawing with no screens must be safe too.
        let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
    }

    #[test]
    fn unreadable_config_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut sh = Shared {
            ctx: Ctx {
                config_path: path.clone(),
                sim: true,
                version: "0.2.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
        };
        // One screen, so `s` reaches `save` (with none the key is ignored outright).
        let one = vec![Orientation {
            rotate: 0,
            hflip: false,
        }];
        let mut screen = Calibrate::from_parts(Config::default(), one);
        screen.unreadable = true;
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('s')), &mut sh, 0.0),
            Action::None
        ));
        assert!(!path.exists(), "defaults must not replace the user's file");
        assert!(screen.error.as_deref().unwrap().contains("not saved"));
    }

    /// Corner (0 TR, 1 BR, 2 BL, 3 TL) holding the amber marker, found by the centroid
    /// of pixels close to `#ffb020` inside the ring (radius < 96 excludes the ring).
    fn marker_corner(px: &Pixmap) -> Option<usize> {
        let d = px.data();
        let close = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 24;
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0usize);
        for y in 0..240usize {
            for x in 0..240usize {
                let (dx, dy) = (x as f64 - 120.0, y as f64 - 120.0);
                if (dx * dx + dy * dy).sqrt() >= 96.0 {
                    continue;
                }
                let i = (y * 240 + x) * 4;
                if close(d[i], 0xff) && close(d[i + 1], 0xb0) && close(d[i + 2], 0x20) {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1;
                }
            }
        }
        if n < 20 {
            return None;
        }
        let (cx, cy) = (sx / n as f64, sy / n as f64);
        Some(match (cx > 120.0, cy > 120.0) {
            (true, false) => 0,
            (true, true) => 1,
            (false, true) => 2,
            (false, false) => 3,
        })
    }

    /// The corner `mini_panel` puts its marker in, same numbering as `marker_corner`.
    fn mini_corner(lines: &[String]) -> usize {
        if lines[0].ends_with("● ") {
            0
        } else if lines[4].ends_with("● ") {
            1
        } else if lines[4].starts_with(" ●") {
            2
        } else {
            assert!(lines[0].starts_with(" ●"), "{lines:?}");
            3
        }
    }

    /// The terminal preview and the pixels sent to a panel must agree for every
    /// rotate/hflip combination, otherwise the user calibrates against a lie.
    #[test]
    fn oriented_marker_matches_mini_panel_corner() {
        use rackscreen_core::theme::Role;
        let mut r = Renderer::new().unwrap();
        let mut base = new_pixmap();
        r.render(&test_pattern(0, Role::Pods), &mut base);
        assert_eq!(marker_corner(&base), Some(0), "source marker is top-right");
        let mut out = new_pixmap();
        for rotate in [0, 90, 180, 270] {
            for hflip in [false, true] {
                Orient::new(rotate, hflip).apply(&base, &mut out);
                let got = marker_corner(&out).expect("marker visible after orienting");
                let want = mini_corner(&mini_panel(rotate, hflip, 1, true));
                assert_eq!(got, want, "rotate {rotate} hflip {hflip}");
            }
        }
    }
}

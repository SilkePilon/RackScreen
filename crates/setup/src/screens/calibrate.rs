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

use crate::anim::Slide;
use crate::ops::config_file::{save_config, set_orientation};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::widgets::{pixels, StatusTone};
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
    /// The oriented frame each panel is showing, for the terminal preview.
    shown: Vec<Pixmap>,
    /// Horizontal offset of the preview, 1.0 = fully off to the right, 0.0 = in place.
    slide: Slide,
    error: Option<String>,
    service_was_active: bool,
    message: String,
    /// The file on disk could not be loaded: the panels show defaults for testing, but
    /// `s` must never overwrite the user's file with them.
    unreadable: bool,
}

/// A round panel as 7 rows of 13 columns: the arrow after rotate/hflip in row 2, the
/// number in row 3, and a marker in the corner the amber dot ends up in.
pub fn circle_panel(rotate: u32, hflip: bool, number: usize, unicode: bool) -> Vec<String> {
    let arrows = if unicode {
        ["↑", "→", "↓", "←"]
    } else {
        ["^", ">", "v", "<"]
    };
    let turns = ((rotate % 360) / 90) as usize;
    // Arrow direction after clockwise rotation, then mirrored horizontally.
    let mut dir = turns; // 0 up, 1 right, 2 down, 3 left
    if hflip && (dir == 1 || dir == 3) {
        dir = 4 - dir;
    }
    // Marker starts top-right; rotation moves it clockwise; hflip swaps left/right.
    let mut corner = turns; // 0 TR, 1 BR, 2 BL, 3 TL
    if hflip {
        corner = match corner {
            0 => 3,
            1 => 2,
            2 => 1,
            _ => 0,
        };
    }
    let o = if unicode { "●" } else { "*" };
    let m = |c: usize| if corner == c { o } else { " " };
    let (tl, tr, bl, br) = (m(3), m(0), m(2), m(1));
    let a = arrows[dir];
    let n = number % 10;
    if unicode {
        vec![
            "   ╭─────╮   ".to_string(),
            " ╭╯       ╰╮ ".to_string(),
            format!("╭╯ {tl}  {a}  {tr} ╰╮"),
            format!("│     {n}     │"),
            format!("╰╮ {bl}     {br} ╭╯"),
            " ╰╮       ╭╯ ".to_string(),
            "   ╰─────╯   ".to_string(),
        ]
    } else {
        vec![
            "   .-----.   ".to_string(),
            "  /       \\  ".to_string(),
            format!(" / {tl}  {a}  {tr} \\ "),
            format!(" |    {n}    | "),
            format!(" \\ {bl}     {br} / "),
            "  \\       /  ".to_string(),
            "   '-----'   ".to_string(),
        ]
    }
}

/// `ok` for the identity, otherwise the change: `rot 90`, `flip`, `rot 180 flip`.
pub fn orient_label(o: Orientation) -> String {
    let mut parts = Vec::new();
    if !o.rotate.is_multiple_of(360) {
        parts.push(format!("rot {}", o.rotate % 360));
    }
    if o.hflip {
        parts.push("flip".to_string());
    }
    if parts.is_empty() {
        "ok".to_string()
    } else {
        parts.join(" ")
    }
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
            shown: Vec::new(),
            slide: Slide::fixed(0.0),
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
                me.shown = (0..p.handles.len()).map(|_| new_pixmap()).collect();
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
        r.render(&test_pattern(i, h.first_role()), &mut self.frames[i]);
        // In the simulator the panel orientation is identity, so apply the candidate
        // orientation here; on real panels do the same (the handle's orient is only used by
        // the monitor, calibration always orients explicitly).
        Orient::new(o.rotate, o.hflip).apply(&self.frames[i], &mut self.shown[i]);
        h.mailbox
            .put(DisplayCmd::Frame(self.shown[i].clone(), Rect::full()));
    }

    fn select(&mut self, i: usize, now: Secs) {
        if i != self.selected {
            self.selected = i;
            self.slide = Slide::fixed(1.0).to(0.0, now, 0.2);
        }
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
            shown: Vec::new(),
            slide: Slide::fixed(0.0),
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
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, now: Secs) -> Action {
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
                    self.select(i, now);
                }
            }
            KeyCode::Left | KeyCode::Up => self.select((self.selected + n - 1) % n, now),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                self.select((self.selected + 1) % n, now)
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

    fn draw(&self, f: &mut Frame, area: TRect, shared: &Shared, now: Secs) {
        let th = &shared.theme;
        let [_, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(7),
            Constraint::Length(2),
        ])
        .areas(area);
        let [preview, info] =
            Layout::horizontal([Constraint::Length(34), Constraint::Min(20)]).areas(body);
        // Preview: the selected panel, sliding in from the right on a change.
        let inner = TRect {
            x: preview.x + 2,
            width: preview.width.saturating_sub(4),
            ..preview
        };
        let off = (self.slide.value(now).abs() * inner.width as f32) as u16;
        let shifted = TRect {
            x: inner.x + off,
            width: inner.width.saturating_sub(off),
            ..inner
        };
        match self.shown.get(self.selected) {
            Some(px) if th.unicode && th.truecolor => pixels(f, shifted, px),
            _ => {
                if let Some(o) = self.orient.get(self.selected) {
                    let lines = circle_panel(o.rotate, o.hflip, self.selected + 1, th.unicode);
                    let h = lines.len() as u16;
                    let top = shifted.y + shifted.height.saturating_sub(h) / 2;
                    let left = shifted.x + shifted.width.saturating_sub(13) / 2;
                    let body: Vec<Line> = lines
                        .into_iter()
                        .map(|l| Line::from(Span::styled(l, th.selected())))
                        .collect();
                    f.render_widget(
                        Paragraph::new(body),
                        TRect {
                            x: left,
                            y: top,
                            width: 13.min(shifted.width),
                            height: h.min(shifted.height),
                        },
                    );
                }
            }
        }
        // Info: strip of all panels, the selected one underlined, then keys and the goal.
        let mut strip = vec![Span::raw("  ")];
        let mut under = String::from("  ");
        let mark = if th.unicode { "▔" } else { "^" };
        for (i, o) in self.orient.iter().enumerate() {
            let label = format!("{} {}", i + 1, orient_label(*o));
            let sel = i == self.selected;
            let style = if sel {
                th.selected()
            } else if label.ends_with("ok") {
                th.good()
            } else {
                th.muted()
            };
            let w = label.chars().count();
            strip.push(Span::styled(label, style));
            strip.push(Span::raw("    "));
            under.push_str(&if sel { mark.repeat(w) } else { " ".repeat(w) });
            under.push_str("    ");
        }
        let mut lines = vec![
            Line::from(strip),
            Line::from(Span::styled(under, th.selected())),
            Line::from(""),
        ];
        if let Some(o) = self.orient.get(self.selected) {
            lines.push(Line::from(vec![
                Span::styled(format!("  Screen {}", self.selected + 1), th.normal()),
                Span::styled(
                    format!(
                        "   rotate {}°   flip {}",
                        o.rotate,
                        if o.hflip { "on" } else { "off" }
                    ),
                    th.muted(),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  no screens in the config",
                th.muted(),
            )));
        }
        lines.push(Line::from(""));
        let key = |k: &str, what: &str| {
            vec![
                Span::styled(format!("  {k}"), th.selected()),
                Span::styled(format!(" {what}"), th.muted()),
            ]
        };
        let mut k1 = key("r", "rotate +90");
        k1.extend(key("R", "rotate −90"));
        k1.extend(key("f", "flip"));
        lines.push(Line::from(k1));
        let mut k2 = key("a", "apply to all");
        k2.extend(key("s", "save"));
        k2.extend(key("Esc", "discard"));
        lines.push(Line::from(k2));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Match the preview to the glass:",
            th.muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  arrow up, amber dot top-right.",
            th.muted(),
        )));
        f.render_widget(
            Paragraph::new(lines),
            TRect {
                y: info.y + 1,
                height: info.height.saturating_sub(1),
                ..info
            },
        );
        let msg = match &self.error {
            Some(e) => Line::from(Span::styled(format!("  error: {e}"), th.bad())),
            None => Line::from(Span::styled(
                format!("  {}", self.message),
                th.faint_style(),
            )),
        };
        f.render_widget(Paragraph::new(msg), foot);
    }

    fn keys(&self) -> String {
        "1-4 screen  r/R rotate  f flip  a all  s save  Esc discard".into()
    }
    fn subtitle(&self) -> String {
        "Calibrate".into()
    }
    fn animating(&self, now: Secs) -> bool {
        !self.slide.done(now)
    }
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        self.panels
            .is_some()
            .then(|| ("panels live".to_string(), StatusTone::Ok))
    }
    fn consumes_left(&self) -> bool {
        true
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

    /// Corner (0 TR, 1 BR, 2 BL, 3 TL) holding the marker in a `circle_panel`.
    fn circle_corner(lines: &[String]) -> usize {
        let at = |row: usize, col: usize| lines[row].chars().nth(col).unwrap();
        let mark = |c: char| c == '●' || c == '*';
        if mark(at(2, 9)) {
            0
        } else if mark(at(4, 9)) {
            1
        } else if mark(at(4, 3)) {
            2
        } else {
            assert!(mark(at(2, 3)), "{lines:?}");
            3
        }
    }

    #[test]
    fn circle_panel_is_7_by_13_and_follows_rotation_and_flip() {
        let up = circle_panel(0, false, 1, true);
        assert_eq!(up.len(), 7);
        for l in &up {
            assert_eq!(l.chars().count(), 13, "{l:?}");
        }
        assert_eq!(up[2].chars().nth(6), Some('↑'));
        assert_eq!(up[3].chars().nth(6), Some('1'));
        assert_eq!(circle_corner(&up), 0);
        let right = circle_panel(90, false, 2, true);
        assert_eq!(right[2].chars().nth(6), Some('→'));
        assert_eq!(circle_corner(&right), 1);
        let flipped = circle_panel(0, true, 3, true);
        assert_eq!(flipped[2].chars().nth(6), Some('↑'));
        assert_eq!(circle_corner(&flipped), 3);
        let both = circle_panel(90, true, 4, true);
        assert_eq!(
            both[2].chars().nth(6),
            Some('←'),
            "right arrow mirrored becomes left"
        );
        assert_eq!(circle_corner(&both), 2);
        let ascii = circle_panel(0, false, 1, false);
        assert_eq!(ascii[2].chars().nth(6), Some('^'));
        for l in &ascii {
            assert!(l.is_ascii(), "{l:?}");
            assert_eq!(l.len(), 13);
        }
    }

    #[test]
    fn orient_label_names_the_change() {
        let o = |rotate, hflip| Orientation { rotate, hflip };
        assert_eq!(orient_label(o(0, false)), "ok");
        assert_eq!(orient_label(o(90, false)), "rot 90");
        assert_eq!(orient_label(o(0, true)), "flip");
        assert_eq!(orient_label(o(180, true)), "rot 180 flip");
    }

    #[test]
    fn selecting_another_screen_slides_the_preview() {
        let mut sh = Shared {
            ctx: Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.5.0",
            },
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        };
        let two = vec![
            Orientation {
                rotate: 0,
                hflip: false
            };
            2
        ];
        let mut screen = Calibrate::from_parts(Config::default(), two);
        assert!(!screen.animating(0.0));
        screen.handle(KeyEvent::from(KeyCode::Char('2')), &mut sh, 1.0);
        assert!(screen.animating(1.01));
        assert!(!screen.animating(1.3));
        assert_eq!(screen.status(&sh), None, "no panels opened in tests");
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 1.3)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("1 ok"), "{t}");
        assert!(t.contains("2 ok"));
        assert!(t.contains("Screen 2"));
        assert!(
            t.contains("╭─────╮"),
            "drawn circle fallback without a pixmap: {t}"
        );
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
            update: None,
            redraw: false,
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
            update: None,
            redraw: false,
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

    /// The terminal preview and the pixels sent to a panel must agree for every
    /// rotate/hflip combination, otherwise the user calibrates against a lie.
    #[test]
    fn oriented_marker_matches_circle_panel_corner() {
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
                let want = circle_corner(&circle_panel(rotate, hflip, 1, true));
                assert_eq!(got, want, "rotate {rotate} hflip {hflip}");
            }
        }
    }
}

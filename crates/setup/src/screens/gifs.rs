//! The Configure › GIFs group: the config's `gifs:` list of name + link,
//! with add, edit and remove. Screens show a GIF as the role `gif:<name>`.

use rackscreen_app::config::{Config, GifCfg, GIF_ROLE_PREFIX};
use rackscreen_core::gif::{classify_link, valid_gif_name};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;
use crate::widgets::confirm_dialog;

/// Config indices (0-based) of the screens whose roles include `gif:<name>`.
pub fn screens_using(cfg: &Config, name: &str) -> Vec<usize> {
    let role = format!("{GIF_ROLE_PREFIX}{name}");
    cfg.screens
        .iter()
        .enumerate()
        .filter(|(_, s)| s.roles.contains(&role))
        .map(|(i, _)| i)
        .collect()
}

/// Add (`index` None) or replace the GIF at `index`. The name must be valid
/// and unused, the link one the role accepts and no other GIF's. A rename rewrites `gif:<old>`
/// in every screen's roles.
pub fn save_gif(
    cfg: &mut Config,
    index: Option<usize>,
    name: &str,
    url: &str,
) -> Result<(), String> {
    let (name, url) = (name.trim(), url.trim());
    if !valid_gif_name(name) {
        return Err("name: lowercase letters, digits and - only".into());
    }
    if cfg
        .gifs
        .iter()
        .enumerate()
        .any(|(i, g)| Some(i) != index && g.name == name)
    {
        return Err(format!("there is already a GIF called {name}"));
    }
    classify_link(url)?;
    if let Some(o) = cfg
        .gifs
        .iter()
        .enumerate()
        .find(|(i, g)| Some(*i) != index && g.url.trim() == url)
        .map(|(_, g)| g)
    {
        return Err(format!("{} already has this link", o.name));
    }
    let gif = GifCfg {
        name: name.to_string(),
        url: url.to_string(),
    };
    match index.and_then(|i| cfg.gifs.get_mut(i)) {
        Some(g) => {
            let (old, new) = (
                format!("{GIF_ROLE_PREFIX}{}", g.name),
                format!("{GIF_ROLE_PREFIX}{name}"),
            );
            for s in &mut cfg.screens {
                for r in &mut s.roles {
                    if *r == old {
                        *r = new.clone();
                    }
                }
            }
            *g = gif;
        }
        None => cfg.gifs.push(gif),
    }
    Ok(())
}

/// Remove the GIF at `index` and its `gif:<name>` entries from every screen.
/// Refused when a screen would be left with no roles.
pub fn remove_gif(cfg: &mut Config, index: usize) -> Result<(), String> {
    let Some(g) = cfg.gifs.get(index) else {
        return Ok(());
    };
    let role = format!("{GIF_ROLE_PREFIX}{}", g.name);
    if let Some(i) = cfg
        .screens
        .iter()
        .position(|s| !s.roles.is_empty() && s.roles.iter().all(|r| *r == role))
    {
        return Err(format!(
            "screen {} shows only {role}: give it another role first",
            i + 1
        ));
    }
    for s in &mut cfg.screens {
        s.roles.retain(|r| *r != role);
    }
    cfg.gifs.remove(index);
    Ok(())
}

/// One GIF being added or edited.
#[derive(Clone, Debug, PartialEq)]
struct GifEdit {
    /// `None` adds a new GIF.
    index: Option<usize>,
    name: String,
    url: String,
    /// Typing goes to the link, else to the name.
    on_url: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum PaneMode {
    List,
    Edit(GifEdit),
    /// Removing a GIF that these screens (0-based) show.
    AskRemove {
        index: usize,
        screens: Vec<usize>,
    },
}

/// What a key did to the pane.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaneResult {
    /// The pane used the key; otherwise Configure handles it (group keys,
    /// save, back).
    pub consumed: bool,
    /// The config changed.
    pub changed: bool,
    pub error: Option<String>,
}

impl PaneResult {
    fn used() -> PaneResult {
        PaneResult {
            consumed: true,
            ..PaneResult::default()
        }
    }
}

/// The GIFs list: one row per GIF, then "+ add GIF".
#[derive(Clone, Debug, PartialEq)]
pub struct GifPane {
    pub cursor: usize,
    mode: PaneMode,
}

impl Default for GifPane {
    fn default() -> Self {
        GifPane {
            cursor: 0,
            mode: PaneMode::List,
        }
    }
}

impl GifPane {
    /// Editing or asking: every key belongs to the pane.
    pub fn busy(&self) -> bool {
        !matches!(self.mode, PaneMode::List)
    }

    pub fn editing(&self) -> bool {
        matches!(self.mode, PaneMode::Edit(_))
    }

    pub fn handle(&mut self, key: KeyCode, cfg: &mut Config) -> PaneResult {
        let n = cfg.gifs.len();
        self.cursor = self.cursor.min(n);
        let mode = std::mem::replace(&mut self.mode, PaneMode::List);
        match mode {
            PaneMode::List => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.cursor = (self.cursor + n) % (n + 1);
                    PaneResult::used()
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.cursor = (self.cursor + 1) % (n + 1);
                    PaneResult::used()
                }
                KeyCode::Enter | KeyCode::Char(' ') if self.cursor < n => {
                    let g = &cfg.gifs[self.cursor];
                    self.mode = PaneMode::Edit(GifEdit {
                        index: Some(self.cursor),
                        name: g.name.clone(),
                        url: g.url.clone(),
                        on_url: false,
                    });
                    PaneResult::used()
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('a') => {
                    self.mode = PaneMode::Edit(GifEdit {
                        index: None,
                        name: String::new(),
                        url: String::new(),
                        on_url: false,
                    });
                    PaneResult::used()
                }
                KeyCode::Char('d') | KeyCode::Delete if self.cursor < n => {
                    let screens = screens_using(cfg, &cfg.gifs[self.cursor].name);
                    if screens.is_empty() {
                        self.remove(cfg, self.cursor)
                    } else {
                        self.mode = PaneMode::AskRemove {
                            index: self.cursor,
                            screens,
                        };
                        PaneResult::used()
                    }
                }
                _ => PaneResult::default(),
            },
            PaneMode::Edit(mut e) => {
                let mut r = PaneResult::used();
                match key {
                    KeyCode::Esc => return r,
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                        e.on_url = !e.on_url;
                    }
                    KeyCode::Enter if !e.on_url => e.on_url = true,
                    KeyCode::Enter => match save_gif(cfg, e.index, &e.name, &e.url) {
                        Ok(()) => {
                            self.cursor = e.index.unwrap_or(cfg.gifs.len() - 1);
                            r.changed = true;
                            return r;
                        }
                        Err(msg) => r.error = Some(msg),
                    },
                    KeyCode::Backspace => {
                        if e.on_url {
                            e.url.pop();
                        } else {
                            e.name.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if e.on_url {
                            e.url.push(c);
                        } else {
                            e.name.push(c);
                        }
                    }
                    _ => {}
                }
                self.mode = PaneMode::Edit(e);
                r
            }
            PaneMode::AskRemove { index, .. } => {
                if matches!(key, KeyCode::Enter | KeyCode::Char('y')) {
                    self.remove(cfg, index)
                } else {
                    PaneResult::used()
                }
            }
        }
    }

    fn remove(&mut self, cfg: &mut Config, index: usize) -> PaneResult {
        match remove_gif(cfg, index) {
            Ok(()) => {
                self.cursor = self.cursor.min(cfg.gifs.len());
                PaneResult {
                    consumed: true,
                    changed: true,
                    error: None,
                }
            }
            Err(e) => PaneResult {
                consumed: true,
                changed: false,
                error: Some(e),
            },
        }
    }

    /// The help line for the row or field in focus.
    pub fn help(&self) -> &'static str {
        match &self.mode {
            PaneMode::Edit(e) if e.on_url => {
                "A GIPHY or Tenor page, or an https link ending in .gif."
            }
            PaneMode::Edit(_) => "Lowercase letters, digits and -; screens use gif:<name>.",
            _ => "Paste GIPHY/Tenor links; a GIF uses up to 35 MB of RAM.",
        }
    }

    pub fn keys(&self) -> String {
        match self.mode {
            PaneMode::List => {
                "↑↓ GIF  ⏎ edit  a add  d remove  Tab/←→ group  s save  Esc back".into()
            }
            PaneMode::Edit(_) => "type  Tab name/link  ⏎ next/apply  Esc cancel".into(),
            PaneMode::AskRemove { .. } => "y remove  Esc keep".into(),
        }
    }

    pub fn draw(&self, f: &mut Frame, list: Rect, area: Rect, th: &Theme, cfg: &Config) {
        let g = th.glyphs();
        let width = list.width as usize;
        let mut lines = vec![Line::from(Span::styled("  GIFs", th.normal()))];
        let mut focus = None;
        for (i, gif) in cfg.gifs.iter().enumerate() {
            let sel = i == self.cursor;
            if sel {
                focus = Some(lines.len());
            }
            let used = screens_using(cfg, &gif.name);
            let used = match used.len() {
                0 => "unused".to_string(),
                _ => format!(
                    "screen {}",
                    used.iter()
                        .map(|s| (s + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            let link = gif.url.trim().trim_start_matches("https://");
            let room = width.saturating_sub(4 + 16 + used.chars().count() + 4);
            let link: String = if link.chars().count() > room {
                let mut s: String = link.chars().take(room.saturating_sub(1)).collect();
                s.push('…');
                s
            } else {
                link.to_string()
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", if sel { g.pointer } else { " " }),
                    th.selected(),
                ),
                Span::styled(
                    format!("{:<16}", gif.name),
                    if sel { th.selected() } else { th.normal() },
                ),
                Span::styled(format!("{link:<room$}"), th.muted()),
                Span::styled(format!("  {used}"), th.faint_style()),
            ]));
        }
        let add_sel = self.cursor >= cfg.gifs.len();
        if add_sel {
            focus = Some(lines.len());
        }
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} ", if add_sel { g.pointer } else { " " }),
                th.selected(),
            ),
            Span::styled(
                "+ add GIF",
                if add_sel { th.selected() } else { th.muted() },
            ),
        ]));
        if let PaneMode::Edit(e) = &self.mode {
            lines.push(Line::from(""));
            let title = if e.index.is_some() {
                "  Edit GIF"
            } else {
                "  New GIF"
            };
            lines.push(Line::from(Span::styled(title, th.normal())));
            for (label, buf, on) in [("name", &e.name, !e.on_url), ("link", &e.url, e.on_url)] {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{label:<6}"),
                        if on { th.selected() } else { th.muted() },
                    ),
                    Span::styled(
                        if on { format!("{buf}_") } else { buf.clone() },
                        if on { th.title() } else { th.muted() },
                    ),
                ]));
            }
        }
        f.render_widget(Paragraph::new(lines), list);
        if let Some(row) = focus.filter(|r| (*r as u16) < list.height) {
            f.buffer_mut().set_style(
                Rect {
                    y: list.y + row as u16,
                    height: 1,
                    ..list
                },
                th.highlighted(),
            );
        }
        if let PaneMode::AskRemove { index, screens } = &self.mode {
            let name = cfg.gifs.get(*index).map_or("", |g| g.name.as_str());
            let on = screens
                .iter()
                .map(|s| (s + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            confirm_dialog(
                f,
                area,
                th,
                "Remove GIF?",
                &[
                    format!("{GIF_ROLE_PREFIX}{name} is on screen {on}."),
                    "Remove it from those screens too?".to_string(),
                ],
                "y/⏎ remove   Esc keep",
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARTY: &str = "https://giphy.com/gifs/some-slug-AbCdEf123";
    const NOD: &str = "https://tenor.com/view/some-slug-12345678";

    fn cfg() -> Config {
        let mut c = Config::default();
        save_gif(&mut c, None, "party", PARTY).unwrap();
        save_gif(&mut c, None, "nod", NOD).unwrap();
        c.screens[0].roles = vec!["cpu".into(), "gif:party".into()];
        c.screens[2].roles = vec!["gif:party".into(), "mem".into()];
        c
    }

    fn keys(p: &mut GifPane, c: &mut Config, ks: &[KeyCode]) -> PaneResult {
        let mut last = PaneResult::default();
        for k in ks {
            last = p.handle(*k, c);
        }
        last
    }

    fn typed(s: &str) -> Vec<KeyCode> {
        s.chars().map(KeyCode::Char).collect()
    }

    #[test]
    fn save_checks_name_uniqueness_and_link() {
        let mut c = cfg();
        assert_eq!(c.gifs.len(), 2);
        assert!(save_gif(&mut c, None, "Party Time", PARTY).is_err());
        assert!(save_gif(&mut c, None, "nod", PARTY)
            .unwrap_err()
            .contains("already"));
        assert!(save_gif(&mut c, None, "x", "https://example.com/x.png").is_err());
        assert!(
            save_gif(&mut c, Some(1), "nod", NOD).is_ok(),
            "same name, same slot"
        );
        let e = save_gif(&mut c, Some(1), "nod", &format!(" {PARTY}")).unwrap_err();
        assert!(e.contains("party"), "the same link as another GIF: {e}");
        assert!(
            save_gif(&mut c, Some(0), "party", PARTY).is_ok(),
            "its own link"
        );
        c.validate().unwrap();
    }

    #[test]
    fn rename_rewrites_the_screens() {
        let mut c = cfg();
        save_gif(&mut c, Some(0), "dance", PARTY).unwrap();
        assert_eq!(c.screens[0].roles, vec!["cpu", "gif:dance"]);
        assert_eq!(c.screens[2].roles, vec!["gif:dance", "mem"]);
        c.validate().unwrap();
    }

    #[test]
    fn remove_drops_the_gif_from_screens_unless_one_would_be_empty() {
        let mut c = cfg();
        assert_eq!(screens_using(&c, "party"), vec![0, 2]);
        remove_gif(&mut c, 0).unwrap();
        assert_eq!(c.gifs.len(), 1);
        assert_eq!(c.screens[0].roles, vec!["cpu"]);
        assert_eq!(c.screens[2].roles, vec!["mem"]);
        c.validate().unwrap();
        c.screens[1].roles = vec!["gif:nod".into()];
        let e = remove_gif(&mut c, 0).unwrap_err();
        assert!(e.contains("screen 2"), "{e}");
        assert_eq!(c.gifs.len(), 1, "nothing changed");
    }

    #[test]
    fn add_a_gif_by_typing_name_then_link() {
        let mut c = Config::default();
        let mut p = GifPane::default();
        let r = p.handle(KeyCode::Enter, &mut c);
        assert!(r.consumed && p.editing(), "Enter on + add GIF");
        keys(&mut p, &mut c, &typed("party"));
        p.handle(KeyCode::Enter, &mut c);
        assert!(p.help().contains("GIPHY or Tenor"), "now on the link");
        keys(&mut p, &mut c, &typed("https://example.com/x.png"));
        let r = p.handle(KeyCode::Enter, &mut c);
        assert!(
            r.error.is_some() && p.editing(),
            "bad link stays in the editor"
        );
        for _ in 0.."x.png".len() {
            p.handle(KeyCode::Backspace, &mut c);
        }
        keys(&mut p, &mut c, &typed("y.gif"));
        let r = p.handle(KeyCode::Enter, &mut c);
        assert!(r.changed && !p.busy(), "{r:?}");
        assert_eq!(c.gifs[0].name, "party");
        assert_eq!(c.gifs[0].url, "https://example.com/y.gif");
        assert_eq!(p.cursor, 0);
        // Esc leaves an edit without changes
        p.handle(KeyCode::Char('a'), &mut c);
        keys(&mut p, &mut c, &typed("zz"));
        let r = p.handle(KeyCode::Esc, &mut c);
        assert!(r.consumed && !r.changed && !p.busy());
        assert_eq!(c.gifs.len(), 1);
        // navigation keys are left to Configure
        assert!(!p.handle(KeyCode::Right, &mut c).consumed);
        assert!(!p.handle(KeyCode::Char('s'), &mut c).consumed);
    }

    #[test]
    fn removing_a_used_gif_asks_first() {
        let mut c = cfg();
        let mut p = GifPane::default();
        p.handle(KeyCode::Char('d'), &mut c);
        assert!(p.busy(), "party is on screens 1 and 3");
        let r = p.handle(KeyCode::Esc, &mut c);
        assert!(!r.changed && !p.busy());
        assert_eq!(c.gifs.len(), 2);
        p.handle(KeyCode::Char('d'), &mut c);
        let r = p.handle(KeyCode::Char('y'), &mut c);
        assert!(r.changed);
        assert_eq!(c.gifs[0].name, "nod");
        assert_eq!(c.screens[0].roles, vec!["cpu"]);
        // nod is unused: removed at once
        let r = p.handle(KeyCode::Char('d'), &mut c);
        assert!(r.changed && !p.busy());
        assert!(c.gifs.is_empty());
        c.validate().unwrap();
    }
}

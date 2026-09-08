# Setup TUI Shell Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the look and navigation of the setup TUI (`crates/setup`) as a sidebar shell with a live Home, a grouped Configure with a help line, a hybrid Calibrate that renders the real panel frame, and settle-in motion, without changing any feature, key, config field or `ops` behaviour.

**Architecture:** Evolve in place. `App` in `crates/setup/src/lib.rs` owns the shell (header, sidebar, pane, footer, focus, settle animation) and drives the main menu; every screen keeps its `Screen` impl and rewrites only `draw()` plus small additions (`status()`, `consumes_left()`, `sidebar()`). New pure helpers (`fit_roles`, `wrap_roles`, `circle_panel`, `humanise_poll`, `Settle`, `pulse`) are unit tested without a terminal; widgets are tested with `ratatui::backend::TestBackend`.

**Tech Stack:** Rust 2021 workspace, ratatui 0.30 (crossterm backend), tiny-skia pixmaps from `rackscreen-render`, `rackscreen_core::anim::{Tween, Easing}` for easing, `rackscreen_core::theme::Color::mix` for colour blending.

Spec: `docs/superpowers/specs/2026-09-08-setup-tui-shell-redesign-design.md`.

## Global Constraints

- Sidebar is 16 columns; shown only when the terminal is at least 72 columns wide. Below 40 columns or 12 rows the pane shows `terminal too small`.
- Header is 2 rows (content + rule), footer is 2 rows (rule + keys).
- Highlight row background is `#1b170e`. Panel colours per screen index: amber, green, blue, violet (`rackscreen_core::theme::{AMBER, GREEN, BLUE, RED}` are already mapped in `Theme`).
- Settle animation: 0.25 s per row, rows staggered 40 ms, stagger capped at 12 rows. Bar slide 0.15 s. Calibrate preview slide 0.2 s. Unsaved pulse 1.2 s period.
- Half-block preview needs `Theme.unicode && Theme.truecolor`; otherwise the drawn circle (7 rows × 13 columns) is used.
- No change to `crates/app`, `crates/core`, `crates/render`, `crates/display`, `ops/`, config fields, defaults, validation or YAML shape.
- Every widget must survive a 1×1 area without panicking.
- All existing tests in `crates/setup` keep passing (71 at the start of this plan), updated only where they assert on drawn text that the spec changes.
- Run `cargo fmt` and `cargo clippy -p rackscreen-setup --all-targets -- -D warnings` before every commit. Workspace `rust-toolchain.toml` pins the toolchain; do not change it.
- Commit messages: conventional prefix (`feat`, `fix`, `refactor`, `test`, `docs`), imperative, no trailing period, end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## File map

| File | Responsibility after this plan |
|---|---|
| `crates/setup/src/lib.rs` | `App`: shell layout, focus, sidebar state, settle post-processing, key routing; `Screen` trait with `status`, `consumes_left`, `sidebar` |
| `crates/setup/src/anim.rs` | `Slide` (exists), `Settle`, `pulse`, `spinner_frame`, `ring_glyph` |
| `crates/setup/src/theme.rs` | `Theme` with `truecolor`, `highlight`, `mix`, `panel_color`, `highlighted()` |
| `crates/setup/src/widgets.rs` | `header`, `footer`, `sidebar`, `SidebarItem`, `SidebarView`, `StatusTone`, `help_line`, `pixels`, `step_list`, `confirm_dialog` |
| `crates/setup/src/screens/menu.rs` | `ITEMS`, `Home` screen (overview), `fit_roles` |
| `crates/setup/src/screens/configure.rs` | `Group`, `FieldSpec`, `FIELDS`, `humanise_poll`, grouped form with help line |
| `crates/setup/src/screens/calibrate.rs` | `circle_panel`, `orient_label`, hybrid draw with `pixels`, preview slide |
| `crates/setup/src/screens/screens.rs` | `wrap_roles`, wrapped rows, discard dialog |
| `README.md` | Setup TUI section describing the shell |

---

### Task 1: Theme and animation primitives

**Files:**
- Modify: `crates/setup/src/theme.rs`
- Modify: `crates/setup/src/anim.rs`

**Interfaces:**
- Produces: `Theme { truecolor: bool, highlight: Color, .. }`, `Theme::mix(&self, from: Color, to: Color, t: f32) -> Color`, `Theme::panel_color(&self, index: usize) -> Color`, `Theme::highlighted(&self) -> Style`, `anim::Settle::new(now) -> Settle`, `Settle::amount(&self, row: usize, now: Secs) -> f32`, `Settle::done(&self, now: Secs) -> bool`, `anim::pulse(now: Secs) -> f32`.
- `Theme::new(unicode)` keeps its signature (truecolor defaults to `true` so existing tests are unchanged).

- [ ] **Step 1: Write the failing theme tests**

Append to the `tests` module in `crates/setup/src/theme.rs`:

```rust
    #[test]
    fn mix_blends_rgb_and_snaps_other_colours() {
        let th = Theme::new(true);
        let black = Color::Rgb(0, 0, 0);
        let white = Color::Rgb(255, 255, 255);
        assert_eq!(th.mix(black, white, 0.0), black);
        assert_eq!(th.mix(black, white, 1.0), white);
        assert_eq!(th.mix(black, white, 0.5), Color::Rgb(128, 128, 128));
        // non-RGB colours cannot be blended: snap at the midpoint
        assert_eq!(th.mix(Color::Reset, white, 0.4), Color::Reset);
        assert_eq!(th.mix(Color::Reset, white, 0.6), white);
    }

    #[test]
    fn panel_colours_cycle_amber_green_blue_violet() {
        let th = Theme::new(true);
        assert_eq!(th.panel_color(0), th.accent);
        assert_eq!(th.panel_color(1), th.ok);
        assert_eq!(th.panel_color(2), th.blue);
        assert_eq!(th.panel_color(3), th.violet);
        assert_eq!(th.panel_color(4), th.accent);
    }

    #[test]
    fn truecolor_comes_from_colorterm() {
        assert!(Theme::truecolor_from("truecolor"));
        assert!(Theme::truecolor_from("24bit"));
        assert!(!Theme::truecolor_from(""));
        assert!(!Theme::truecolor_from("xterm-256color"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup theme::`
Expected: compile error, `no method named mix`, `no method named panel_color`, `no function truecolor_from`.

- [ ] **Step 3: Implement the theme additions**

In `crates/setup/src/theme.rs`, change the `Theme` struct and impl:

```rust
#[derive(Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub violet: Color,
    pub blue: Color,
    pub ok: Color,
    pub err: Color,
    pub warn: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    /// Background of the focused row.
    pub highlight: Color,
    pub unicode: bool,
    /// The terminal accepts 24-bit colour, so the half-block preview is usable.
    pub truecolor: bool,
}

fn rgb(c: pal::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

fn to_pal(c: Color) -> Option<pal::Color> {
    match c {
        Color::Rgb(r, g, b) => Some(pal::Color::rgb(r, g, b)),
        _ => None,
    }
}

impl Theme {
    pub fn detect() -> Theme {
        let lang = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        let unicode = lang.to_ascii_uppercase().contains("UTF-8")
            || lang.to_ascii_uppercase().contains("UTF8");
        let mut th = Theme::new(unicode);
        th.truecolor = Theme::truecolor_from(&std::env::var("COLORTERM").unwrap_or_default());
        th
    }

    /// `COLORTERM` advertises 24-bit colour as `truecolor` or `24bit`.
    pub fn truecolor_from(colorterm: &str) -> bool {
        let c = colorterm.to_ascii_lowercase();
        c.contains("truecolor") || c.contains("24bit")
    }

    pub fn new(unicode: bool) -> Theme {
        Theme {
            accent: rgb(pal::AMBER),
            violet: rgb(pal::VIOLET),
            blue: rgb(pal::BLUE),
            ok: rgb(pal::GREEN),
            err: rgb(pal::RED),
            warn: rgb(pal::AMBER),
            text: Color::Rgb(0xee, 0xee, 0xee),
            dim: rgb(pal::GREY),
            faint: Color::Rgb(0x44, 0x44, 0x44),
            highlight: Color::Rgb(0x1b, 0x17, 0x0e),
            unicode,
            truecolor: true,
        }
    }

    /// Blend two colours; non-RGB colours (Reset, indexed) snap at the midpoint.
    pub fn mix(&self, from: Color, to: Color, t: f32) -> Color {
        match (to_pal(from), to_pal(to)) {
            (Some(a), Some(b)) => rgb(a.mix(b, t)),
            _ => {
                if t < 0.5 {
                    from
                } else {
                    to
                }
            }
        }
    }

    /// Accent for physical screen `index`: amber, green, blue, violet, repeating.
    pub fn panel_color(&self, index: usize) -> Color {
        [self.accent, self.ok, self.blue, self.violet][index % 4]
    }

    pub fn glyphs(&self) -> &'static Glyphs {
        if self.unicode {
            &UNICODE
        } else {
            &ASCII
        }
    }

    pub fn title(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    pub fn normal(&self) -> Style {
        Style::new().fg(self.text)
    }
    pub fn muted(&self) -> Style {
        Style::new().fg(self.dim)
    }
    pub fn faint_style(&self) -> Style {
        Style::new().fg(self.faint)
    }
    pub fn selected(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    /// The focused row: selected text on the highlight background.
    pub fn highlighted(&self) -> Style {
        Style::new().bg(self.highlight)
    }
    pub fn good(&self) -> Style {
        Style::new().fg(self.ok)
    }
    pub fn bad(&self) -> Style {
        Style::new().fg(self.err)
    }
    pub fn warning(&self) -> Style {
        Style::new().fg(self.warn)
    }
}
```

`mix` at `t = 0.5` from 0 to 255 gives `(0 + 255 × 0.5).round() = 128`, matching the test.

- [ ] **Step 4: Run the theme tests**

Run: `cargo test -p rackscreen-setup theme::`
Expected: 4 tests pass (`glyph_sets_switch` plus the three new ones).

- [ ] **Step 5: Write the failing anim tests**

Append to the `tests` module in `crates/setup/src/anim.rs`:

```rust
    #[test]
    fn settle_staggers_rows_and_finishes() {
        let s = Settle::new(10.0);
        assert_eq!(s.amount(0, 10.0), 0.0);
        assert_eq!(s.amount(5, 10.0), 0.0, "later rows have not started");
        assert!((s.amount(0, 10.25) - 1.0).abs() < 1e-5, "row 0 done after 0.25 s");
        assert!(s.amount(1, 10.25) < 1.0, "row 1 started 40 ms later");
        assert!(s.amount(0, 10.1) > s.amount(1, 10.1));
        // the stagger is capped so long lists finish together
        assert_eq!(s.amount(12, 10.0 + 0.48 + 0.25), s.amount(40, 10.0 + 0.48 + 0.25));
        assert!(!s.done(10.5));
        assert!(s.done(10.0 + 0.25 + 0.48 + 0.001));
        // before it started nothing is visible, and a fixed settle is always done
        assert!(Settle::finished().done(0.0));
        assert_eq!(Settle::finished().amount(3, 0.0), 1.0);
    }

    #[test]
    fn pulse_stays_in_unit_range_with_a_1_2_s_period() {
        for i in 0..100 {
            let v = pulse(i as f64 * 0.037);
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
        assert!((pulse(0.0) - pulse(1.2)).abs() < 1e-4);
        assert!((pulse(0.3) - 1.0).abs() < 1e-4, "peak a quarter period in");
    }
```

- [ ] **Step 6: Run the anim tests to verify they fail**

Run: `cargo test -p rackscreen-setup anim::`
Expected: compile error, `cannot find struct Settle`, `cannot find function pulse`.

- [ ] **Step 7: Implement `Settle` and `pulse`**

Add to `crates/setup/src/anim.rs` after `Slide`:

```rust
const SETTLE_SECS: Secs = 0.25;
const SETTLE_STAGGER: Secs = 0.04;
const SETTLE_MAX_ROWS: usize = 12;

/// Rows of a freshly opened pane fade from faint to their final colour, each row a
/// little after the one above. `amount` is 0.0 (faint) to 1.0 (final).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settle {
    start: Secs,
}

impl Settle {
    pub fn new(now: Secs) -> Settle {
        Settle { start: now }
    }
    /// A settle that is already over (initial state, tests).
    pub fn finished() -> Settle {
        Settle {
            start: f64::NEG_INFINITY,
        }
    }
    pub fn amount(&self, row: usize, now: Secs) -> f32 {
        let delay = row.min(SETTLE_MAX_ROWS) as Secs * SETTLE_STAGGER;
        let t = ((now - self.start - delay) / SETTLE_SECS) as f32;
        Easing::OutCubic.apply(t)
    }
    pub fn done(&self, now: Secs) -> bool {
        now - self.start >= SETTLE_SECS + SETTLE_MAX_ROWS as Secs * SETTLE_STAGGER
    }
}

/// A 1.2 s sine between 0 and 1, for the unsaved dot.
pub fn pulse(now: Secs) -> f32 {
    (0.5 + 0.5 * (now * std::f64::consts::TAU / 1.2).sin()) as f32
}
```

`Easing::apply` clamps `t` to 0..1, so a negative `t` (row not started) gives 0.0 and the `NEG_INFINITY` start gives 1.0.

- [ ] **Step 8: Run the anim tests**

Run: `cargo test -p rackscreen-setup anim::`
Expected: 4 tests pass.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/theme.rs crates/setup/src/anim.rs
git commit -m "feat(setup): theme mixing, panel colours, settle and pulse animations

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Shell widgets: header, footer, sidebar, help line, pixels

**Files:**
- Modify: `crates/setup/src/widgets.rs`
- Modify: `crates/setup/Cargo.toml` (no new dependency; `tiny-skia` is already a dependency)

**Interfaces:**
- Consumes: `Theme::{mix, highlight, highlighted, panel_color}` from Task 1, `anim::{ring_glyph, pulse}`.
- Produces:
  - `pub enum StatusTone { Ok, Warn }`
  - `pub fn header(f, area, th, now, version: &str, crumb: Option<&str>, status: Option<(&str, StatusTone)>)`
  - `pub fn footer(f, area, th, keys: &str)` (now 2 rows: rule then keys)
  - `pub struct SidebarItem { pub label: String, pub hint: Option<(String, Style)> }`
  - `pub struct SidebarView { pub title: String, pub items: Vec<String>, pub selected: usize }`
  - `pub fn sidebar(f, area, th, title: Option<&str>, items: &[SidebarItem], bar_pos: f32, current: Option<usize>, focused: bool)`
  - `pub fn help_line(f, area, th, text: &str)`
  - `pub fn pixels(f, area, px: &tiny_skia::Pixmap)`
  - `pub fn rule(f, area, th)` (one row of `─`)

- [ ] **Step 1: Write the failing widget tests**

Replace the `tests` module at the bottom of `crates/setup/src/widgets.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use rackscreen_render::frame::new_pixmap;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn text(term: &Terminal<TestBackend>) -> String {
        term.backend().to_string()
    }

    /// Every widget must survive terminals smaller than its natural size.
    #[test]
    fn narrow_terminal_does_not_panic() {
        let th = Theme::new(true);
        let steps = [
            StepView {
                title: "install".into(),
                state: StepState::Running,
            },
            StepView {
                title: "enable".into(),
                state: StepState::Failed("boom".into()),
            },
        ];
        let items = [
            SidebarItem {
                label: "Home".into(),
                hint: None,
            },
            SidebarItem {
                label: "Update".into(),
                hint: Some(("↑".into(), th.warning())),
            },
        ];
        let px = new_pixmap();
        for (w, h) in [(1u16, 1u16), (4, 2), (12, 3), (40, 10), (80, 24)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                let a = f.area();
                step_list(f, a, &th, &steps, 0.5, 0.2);
                confirm_dialog(f, a, &th, "Uninstall", &["remove?".into()], "y/n", true);
                header(f, a, &th, 0.0, "0.5.0", Some("Configure"), Some(("unsaved", StatusTone::Warn)));
                footer(f, a, &th, "q quit");
                sidebar(f, a, &th, Some("Configure"), &items, 0.5, Some(1), true);
                help_line(f, a, &th, "help");
                pixels(f, a, &px);
                rule(f, a, &th);
            })
            .unwrap();
        }
    }

    #[test]
    fn header_shows_crumb_status_or_version() {
        let th = Theme::new(true);
        let mut term = Terminal::new(TestBackend::new(60, 2)).unwrap();
        term.draw(|f| header(f, f.area(), &th, 0.0, "0.5.0", None, None)).unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen"));
        assert!(t.contains("v0.5.0"));
        assert!(!t.contains("›"));
        term.draw(|f| {
            header(f, f.area(), &th, 0.0, "0.5.0", Some("Configure"), Some(("unsaved", StatusTone::Warn)))
        })
        .unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen › Configure"));
        assert!(t.contains("● unsaved"));
        assert!(!t.contains("v0.5.0"), "status replaces the version");
    }

    #[test]
    fn sidebar_marks_hover_when_focused_and_current_otherwise() {
        let th = Theme::new(true);
        let items: Vec<SidebarItem> = ["Home", "Screens", "Configure"]
            .iter()
            .map(|l| SidebarItem {
                label: l.to_string(),
                hint: None,
            })
            .collect();
        let mut term = Terminal::new(TestBackend::new(16, 6)).unwrap();
        term.draw(|f| sidebar(f, f.area(), &th, None, &items, 1.0, None, true)).unwrap();
        let t = text(&term);
        assert!(t.contains("▸ Screens"), "{t}");
        assert!(!t.contains("▸ Home"));
        term.draw(|f| sidebar(f, f.area(), &th, Some("Configure"), &items, 0.0, Some(2), false))
            .unwrap();
        let t = text(&term);
        assert!(t.contains("Configure"));
        assert!(t.contains("▸ Configure"), "current item gets the pointer when unfocused: {t}");
        assert!(!t.contains("▸ Home"));
    }

    #[test]
    fn sidebar_hint_sits_at_the_right_edge() {
        let th = Theme::new(true);
        let items = [SidebarItem {
            label: "Update".into(),
            hint: Some(("↑".into(), th.warning())),
        }];
        let mut term = Terminal::new(TestBackend::new(16, 3)).unwrap();
        term.draw(|f| sidebar(f, f.area(), &th, None, &items, 0.0, None, true)).unwrap();
        let row: String = text(&term).lines().nth(1).unwrap().to_string();
        assert!(row.trim_end().ends_with('↑'), "{row:?}");
    }

    #[test]
    fn pixels_paints_half_blocks_from_the_pixmap() {
        let mut px = new_pixmap();
        // a 16x8 red block in the top-left corner: the first cell's top half
        {
            let w = px.width() as usize;
            let d = px.data_mut();
            for y in 0..8usize {
                for x in 0..16usize {
                    let i = (y * w + x) * 4;
                    d[i] = 255;
                    d[i + 1] = 0;
                    d[i + 2] = 0;
                    d[i + 3] = 255;
                }
            }
        }
        let mut term = Terminal::new(TestBackend::new(30, 15)).unwrap();
        term.draw(|f| pixels(f, f.area(), &px)).unwrap();
        let buf = term.backend().buffer();
        let c = buf.cell((0, 0)).unwrap();
        assert_eq!(c.symbol(), "▀");
        assert_eq!(c.fg, ratatui::style::Color::Rgb(255, 0, 0));
        // black pixels are left as terminal background so the disc reads as round
        let far = buf.cell((15, 7)).unwrap();
        assert_eq!(far.symbol(), " ");
        assert_eq!(far.fg, ratatui::style::Color::Reset);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup widgets::`
Expected: compile errors for `StatusTone`, `SidebarItem`, `sidebar`, `help_line`, `pixels`, `rule`, and header arity.

- [ ] **Step 3: Implement the widgets**

Replace everything above the `StepState` enum in `crates/setup/src/widgets.rs` (the module doc, imports, `header`, `footer`) with:

```rust
//! Shared drawing helpers: shell chrome (header, sidebar, footer), help line, half-block
//! pixel preview, step list, confirm dialog.

use rackscreen_core::anim::Secs;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;
use tiny_skia::Pixmap;

use crate::anim::{pulse, ring_glyph, spinner_frame};
use crate::theme::Theme;

/// How the header colours a screen's status word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Ok,
    Warn,
}

/// One row of `─` across `area`.
pub fn rule(f: &mut Frame, area: Rect, th: &Theme) {
    if area.height == 0 {
        return;
    }
    let ch = if th.unicode { "─" } else { "-" };
    let line = Line::from(Span::styled(ch.repeat(area.width as usize), th.faint_style()));
    f.render_widget(Paragraph::new(line), Rect { height: 1, ..area });
}

/// Two rows: `◐ RackScreen › crumb` with the status word or version at the right, then a rule.
pub fn header(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    now: Secs,
    version: &str,
    crumb: Option<&str>,
    status: Option<(&str, StatusTone)>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let g = th.glyphs();
    let mut left = vec![
        Span::raw(" "),
        Span::styled(ring_glyph(g.ring, now).to_string(), th.title()),
        Span::styled(" RackScreen", th.title()),
    ];
    if let Some(c) = crumb {
        left.push(Span::styled(format!(" › {c}"), th.muted()));
    }
    let right = match status {
        Some((s, StatusTone::Warn)) => Line::from(Span::styled(
            format!("{} {s} ", g.dot),
            Style::new().fg(th.mix(th.accent, th.dim, pulse(now) * 0.6)),
        )),
        Some((s, StatusTone::Ok)) => Line::from(Span::styled(format!("{} {s} ", g.dot), th.good())),
        None => Line::from(Span::styled(format!("v{version} "), th.muted())),
    };
    let top = Rect { height: 1, ..area };
    f.render_widget(Paragraph::new(Line::from(left)), top);
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), top);
    if area.height > 1 {
        rule(f, Rect { y: area.y + 1, height: 1, ..area }, th);
    }
}

/// Two rows: a rule, then the key hints.
pub fn footer(f: &mut Frame, area: Rect, th: &Theme, keys: &str) {
    if area.height == 0 {
        return;
    }
    rule(f, area, th);
    if area.height > 1 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {keys}"), th.muted()))),
            Rect { y: area.y + 1, height: 1, ..area },
        );
    }
}

pub struct SidebarItem {
    pub label: String,
    /// Drawn at the right edge of the row (the update arrow).
    pub hint: Option<(String, Style)>,
}

/// What a screen wants in the sidebar instead of the main menu (Configure's groups).
pub struct SidebarView {
    pub title: String,
    pub items: Vec<String>,
    pub selected: usize,
}

/// The 16-column navigation column. With `focused`, the row nearest `bar_pos` gets the
/// highlight bar and pointer; otherwise `current` gets the pointer without a bar.
pub fn sidebar(
    f: &mut Frame,
    area: Rect,
    th: &Theme,
    title: Option<&str>,
    items: &[SidebarItem],
    bar_pos: f32,
    current: Option<usize>,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let g = th.glyphs();
    let mut lines = Vec::new();
    match title {
        Some(t) => lines.push(Line::from(Span::styled(format!("  {t}"), th.muted()))),
        None => lines.push(Line::from("")),
    }
    let width = area.width as usize;
    for (i, item) in items.iter().enumerate() {
        let hot = if focused {
            (bar_pos - i as f32).abs() < 0.5
        } else {
            current == Some(i)
        };
        let pointer = if hot { g.pointer } else { " " };
        let label_style = if hot {
            th.selected()
        } else if current == Some(i) {
            th.normal()
        } else {
            th.muted()
        };
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(format!("{pointer} "), th.selected()),
            Span::styled(item.label.clone(), label_style),
        ];
        let used = 3 + item.label.chars().count();
        if let Some((h, style)) = &item.hint {
            let hw = h.chars().count() + 1;
            if used + hw <= width {
                spans.push(Span::raw(" ".repeat(width - used - hw)));
                spans.push(Span::styled(h.clone(), *style));
            }
        }
        let mut line = Line::from(spans);
        if hot && focused {
            line = line.style(th.highlighted());
        }
        lines.push(line);
    }
    f.render_widget(Paragraph::new(lines), area);
}

/// `ⓘ text` in the last row of the pane.
pub fn help_line(f: &mut Frame, area: Rect, th: &Theme, text: &str) {
    if area.height == 0 {
        return;
    }
    let mark = if th.unicode { "ⓘ" } else { "i" };
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(mark, Style::new().fg(th.blue)),
        Span::styled(format!(" {text}"), th.muted()),
    ]);
    f.render_widget(Paragraph::new(line), Rect { height: 1, ..area });
}

/// Near-black is drawn as terminal background so the round panel keeps its shape.
fn sample(px: &Pixmap, x: u32, y: u32) -> Option<Color> {
    let w = px.width();
    if x >= w || y >= px.height() {
        return None;
    }
    let d = px.data();
    let i = ((y * w + x) * 4) as usize;
    let (r, g, b) = (d[i], d[i + 1], d[i + 2]);
    if r < 8 && g < 8 && b < 8 {
        None
    } else {
        Some(Color::Rgb(r, g, b))
    }
}

/// Paint a square pixmap into `area` with `▀`/`▄` half blocks, two pixels per cell,
/// centred, keeping a 1:1 aspect (one cell is about twice as tall as wide).
pub fn pixels(f: &mut Frame, area: Rect, px: &Pixmap) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let cols = area.width.min(area.height.saturating_mul(2)).max(1);
    let rows = (cols / 2).max(1);
    let x0 = area.x + (area.width - cols.min(area.width)) / 2;
    let y0 = area.y + (area.height - rows.min(area.height)) / 2;
    let step = px.width() as f32 / cols as f32;
    let buf = f.buffer_mut();
    for cy in 0..rows.min(area.height) {
        for cx in 0..cols.min(area.width) {
            let sx = ((cx as f32 + 0.5) * step) as u32;
            let top = sample(px, sx, ((cy as f32 * 2.0 + 0.5) * step) as u32);
            let bottom = sample(px, sx, ((cy as f32 * 2.0 + 1.5) * step) as u32);
            let Some(cell) = buf.cell_mut((x0 + cx, y0 + cy)) else {
                continue;
            };
            match (top, bottom) {
                (None, None) => {
                    cell.set_char(' ');
                }
                (Some(t), None) => {
                    cell.set_char('▀').set_fg(t);
                }
                (None, Some(b)) => {
                    cell.set_char('▄').set_fg(b);
                }
                (Some(t), Some(b)) => {
                    cell.set_char('▀').set_fg(t).set_bg(b);
                }
            }
        }
    }
}
```

Keep `StepState`, `StepView`, `step_list` and `confirm_dialog` exactly as they are below this.

- [ ] **Step 4: Run the widget tests**

Run: `cargo test -p rackscreen-setup widgets::`
Expected: 5 tests pass. `lib.rs` still calls the old `header` signature, so first fix the call in `App::draw` temporarily to compile:

```rust
        widgets::header(
            f,
            head,
            &self.shared.theme,
            now,
            self.shared.ctx.version,
            Some(&self.current.subtitle()),
            None,
        );
```

(Task 3 rewrites `App::draw` fully.)

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/widgets.rs crates/setup/src/lib.rs
git commit -m "feat(setup): shell widgets, sidebar, help line and half-block pixels

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: The shell in `App`: layout, focus, sidebar navigation, settle

**Files:**
- Modify: `crates/setup/src/lib.rs`
- Modify: `crates/setup/src/screens/menu.rs` (Menu becomes a passive `Home` placeholder here; Task 4 fills it)
- Modify: `crates/setup/src/screens/mod.rs`

**Interfaces:**
- Consumes: Task 1 `Settle`, `Theme::mix`; Task 2 `header`, `footer`, `sidebar`, `SidebarItem`, `SidebarView`, `StatusTone`.
- Produces: `Screen` trait gains
  ```rust
  fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> { None }
  fn consumes_left(&self) -> bool { false }
  fn sidebar(&self) -> Option<SidebarView> { None }
  ```
  `pub enum Focus { Sidebar, Content }` in `lib.rs`. `menu::ITEMS` stays; `menu::Home` replaces `menu::Menu`.

- [ ] **Step 1: Turn `Menu` into a passive `Home`**

Replace the whole of `crates/setup/src/screens/menu.rs` with (Task 4 replaces the draw; this keeps the crate compiling):

```rust
//! Home: the main menu items (drawn by the shell's sidebar) and the live overview pane.

use rackscreen_core::anim::Secs;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::{Action, Screen, ScreenId, Shared};

pub const ITEMS: [(ScreenId, &str, &str); 8] = [
    (ScreenId::Install, "Install", "set up service + config"),
    (ScreenId::Calibrate, "Calibrate", "fix rotation / mirroring"),
    (ScreenId::Screens, "Screens", "what each screen shows, cycling"),
    (ScreenId::Configure, "Configure", "cluster, services, sky, display"),
    (ScreenId::Status, "Status", "service, links, logs"),
    (ScreenId::RunHere, "Run here", "foreground with logs"),
    (ScreenId::Update, "Update", "download and install the latest release"),
    (ScreenId::Uninstall, "Uninstall", "remove everything"),
];

pub struct Home;

impl Home {
    pub fn new(_shared: &Shared) -> Home {
        Home
    }
}

impl Screen for Home {
    fn handle(&mut self, _key: KeyEvent, _shared: &mut Shared, _now: Secs) -> Action {
        // The shell drives the sidebar; Home has no keys of its own.
        Action::None
    }

    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("  overview", th.muted()))),
            area,
        );
    }

    fn keys(&self) -> String {
        "↑↓ move   ⏎ open   q quit".into()
    }
    fn subtitle(&self) -> String {
        "Home".into()
    }
}
```

In `crates/setup/src/screens/mod.rs` change the Menu arm:

```rust
        ScreenId::Menu => Box::new(menu::Home::new(shared)),
```

- [ ] **Step 2: Write the failing shell tests**

Append to `crates/setup/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn app(dir: &std::path::Path, w: u16, h: u16) -> (App, Terminal<TestBackend>) {
        let ctx = Ctx {
            config_path: dir.join("config.yaml"),
            sim: true,
            version: "0.5.0",
        };
        let app = App::for_test(Start::Menu, ctx);
        let term = Terminal::new(TestBackend::new(w, h)).unwrap();
        (app, term)
    }

    fn text(term: &Terminal<TestBackend>) -> String {
        term.backend().to_string()
    }

    #[test]
    fn home_shows_sidebar_with_pointer_and_footer_keys() {
        let dir = tempfile::tempdir().unwrap();
        let (app, mut term) = app(dir.path(), 100, 30);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen"));
        assert!(t.contains("v0.5.0"));
        assert!(t.contains("▸ Install"), "{t}");
        assert!(t.contains("Uninstall"));
        assert!(t.contains("⏎ open"));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn sidebar_navigation_wraps_and_enter_opens_with_content_focus() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut term) = app(dir.path(), 100, 30);
        assert!(app.handle(KeyEvent::from(KeyCode::Up), 0.0));
        assert_eq!(app.hover, screens::menu::ITEMS.len() - 1);
        assert!(app.animating(0.01), "the bar slides");
        for _ in 0..4 {
            app.handle(KeyEvent::from(KeyCode::Down), 1.0);
        }
        assert_eq!(screens::menu::ITEMS[app.hover].0, ScreenId::Configure);
        assert!(app.handle(KeyEvent::from(KeyCode::Enter), 2.0));
        assert_eq!(app.current_id, ScreenId::Configure);
        assert_eq!(app.focus, Focus::Content);
        term.draw(|f| app.draw(f, 2.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("RackScreen › Configure"), "{t}");
        // Esc returns to Home with the sidebar focused and Configure still hovered
        assert!(app.handle(KeyEvent::from(KeyCode::Esc), 3.0));
        assert_eq!(app.current_id, ScreenId::Menu);
        assert_eq!(app.focus, Focus::Sidebar);
        assert_eq!(screens::menu::ITEMS[app.hover].0, ScreenId::Configure);
    }

    #[test]
    fn left_leaves_a_screen_unless_it_consumes_left() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = app(dir.path(), 100, 30);
        // Screens does not use ←: it goes back to Home
        app.go(ScreenId::Screens, 0.0);
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 1.0));
        assert_eq!(app.current_id, ScreenId::Menu);
        // Configure uses ← for groups: it stays
        app.go(ScreenId::Configure, 2.0);
        assert!(app.handle(KeyEvent::from(KeyCode::Left), 3.0));
        assert_eq!(app.current_id, ScreenId::Configure);
    }

    #[test]
    fn q_quits_from_home_and_ctrl_c_quits_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _term) = app(dir.path(), 100, 30);
        assert!(!app.handle(KeyEvent::from(KeyCode::Char('q')), 0.0));
        let (mut app, _term) = app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 0.0);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.handle(ctrl_c, 1.0));
    }

    #[test]
    fn narrow_terminals_hide_the_sidebar_and_tiny_ones_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let (app, mut term) = app(dir.path(), 60, 24);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        let t = text(&term);
        assert!(t.contains("▸ Install"), "Home lists the menu in the pane: {t}");
        assert!(!t.contains("│"), "no sidebar separator below 72 columns");
        let (app, mut term) = app(dir.path(), 30, 8);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
        assert!(text(&term).contains("terminal too small"));
        let (app, mut term) = app(dir.path(), 1, 1);
        term.draw(|f| app.draw(f, 0.0)).unwrap();
    }

    #[test]
    fn settle_fades_the_pane_in_from_faint() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut term) = app(dir.path(), 100, 30);
        app.go(ScreenId::Screens, 10.0);
        term.draw(|f| app.draw(f, 10.0)).unwrap();
        let faint = app.shared.theme.faint;
        // the first pane row is still faint right after opening
        let buf = term.backend().buffer().clone();
        let row = 2u16; // header is two rows
        let any_faint = (17..100u16).any(|x| buf.cell((x, row)).unwrap().fg == faint);
        assert!(any_faint, "pane text starts faint");
        term.draw(|f| app.draw(f, 12.0)).unwrap();
        let buf = term.backend().buffer().clone();
        let still_faint_text = (17..100u16).filter(|x| {
            let c = buf.cell((*x, row)).unwrap();
            c.symbol() != " " && c.fg == faint
        });
        assert_eq!(still_faint_text.count(), 0, "after 2 s everything has settled");
        assert!(!app.animating(12.0));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup tests::`
Expected: compile errors: `no function App::for_test`, `no field hover`, `no field focus`, `Focus` not found, `go` argument count.

- [ ] **Step 4: Implement the shell**

Rewrite `crates/setup/src/lib.rs` from the `Screen` trait down to (and including) `run`. Keep everything above (`Ctx`, `Start`, `ScreenId`, `Action`, `Shared`, `clone_for_test`, `spawn_update_check`) as it is, except add the imports and change `Shared`'s doc as needed.

Imports at the top of the file become:

```rust
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_app::logs::LogSink;
use rackscreen_core::anim::Secs;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};

use crate::anim::{Settle, Slide};
use crate::ops::paths::service_user;
use crate::ops::shell::RealShell;
use crate::ops::systemd::Systemd;
use crate::ops::update::UpdateInfo;
use crate::theme::Theme;
use crate::widgets::{SidebarItem, SidebarView, StatusTone};
```

Then:

```rust
pub trait Screen {
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, now: Secs) -> Action;
    fn tick(&mut self, _shared: &mut Shared, _now: Secs) {}
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, now: Secs);
    fn keys(&self) -> String;
    fn subtitle(&self) -> String;
    fn animating(&self, _now: Secs) -> bool {
        false
    }
    /// Word for the header's right edge (`unsaved`, `panels live`); `None` shows the version.
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        None
    }
    /// True when the screen uses `←` itself, so the shell must not treat it as "back".
    fn consumes_left(&self) -> bool {
        false
    }
    /// Replace the main menu in the sidebar (Configure shows its groups).
    fn sidebar(&self) -> Option<SidebarView> {
        None
    }
}

/// Which area receives keys: the sidebar (only on Home) or the screen's pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Content,
}

const BAR_SECS: Secs = 0.15;
const SIDEBAR_W: u16 = 16;
const MIN_SIDEBAR_COLS: u16 = 72;
const MIN_COLS: u16 = 40;
const MIN_ROWS: u16 = 12;

struct App {
    shared: Shared,
    current: Box<dyn Screen>,
    current_id: ScreenId,
    /// Main menu item under the sidebar bar (also the screen we came from).
    hover: usize,
    bar: Slide,
    focus: Focus,
    settle: Settle,
    update_rx: Option<Receiver<UpdateInfo>>,
}

impl App {
    fn new(start: Start, ctx: Ctx, log_sink: LogSink) -> App {
        // Best effort, so the Home footer and Configure's restart offer are right from
        // the first draw instead of waiting for a Status visit.
        let service_active = if ctx.sim {
            None
        } else {
            let sh = RealShell;
            Systemd::new(&sh, &service_user()).is_active().ok()
        };
        let update_rx = spawn_update_check(ctx.version);
        let shared = Shared {
            ctx,
            theme: Theme::detect(),
            service_active,
            banner: None,
            log_sink,
            update: None,
            redraw: false,
        };
        App::with_shared(start, shared, update_rx)
    }

    /// No update check, no systemd, unicode theme: for tests.
    #[cfg(test)]
    fn for_test(start: Start, ctx: Ctx) -> App {
        let shared = Shared {
            ctx,
            theme: Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: LogSink::new(10),
            update: None,
            redraw: false,
        };
        App::with_shared(start, shared, None)
    }

    fn with_shared(start: Start, shared: Shared, update_rx: Option<Receiver<UpdateInfo>>) -> App {
        let id = match start {
            Start::Menu => ScreenId::Menu,
            Start::Calibrate => ScreenId::Calibrate,
        };
        let current = screens::make(id, &shared);
        let hover = Self::item_index(id).unwrap_or(0);
        App {
            shared,
            current,
            current_id: id,
            hover,
            bar: Slide::fixed(hover as f32),
            focus: if id == ScreenId::Menu {
                Focus::Sidebar
            } else {
                Focus::Content
            },
            settle: Settle::finished(),
            update_rx,
        }
    }

    fn item_index(id: ScreenId) -> Option<usize> {
        screens::menu::ITEMS.iter().position(|(i, _, _)| *i == id)
    }

    fn hover_to(&mut self, idx: usize, now: Secs) {
        self.hover = idx;
        self.bar = self.bar.to(idx as f32, now, BAR_SECS);
    }

    fn go(&mut self, id: ScreenId, now: Secs) {
        self.current = screens::make(id, &self.shared);
        self.current_id = id;
        if let Some(i) = Self::item_index(id) {
            self.hover_to(i, now);
        }
        self.focus = if id == ScreenId::Menu {
            Focus::Sidebar
        } else {
            Focus::Content
        };
        self.settle = Settle::new(now);
    }

    fn handle(&mut self, key: KeyEvent, now: Secs) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if self.focus == Focus::Sidebar {
            let n = screens::menu::ITEMS.len();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.hover_to((self.hover + n - 1) % n, now),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    self.hover_to((self.hover + 1) % n, now)
                }
                KeyCode::Enter | KeyCode::Right => {
                    self.go(screens::menu::ITEMS[self.hover].0, now)
                }
                KeyCode::Char('q') | KeyCode::Esc => return false,
                _ => {}
            }
            return true;
        }
        match self.current.handle(key, &mut self.shared, now) {
            Action::None => {
                // A screen that ignores ← hands it to the shell as "back".
                if key.code == KeyCode::Left && !self.current.consumes_left() {
                    self.go(ScreenId::Menu, now);
                }
            }
            Action::Go(id) => self.go(id, now),
            Action::Back => {
                if self.current_id == ScreenId::Menu {
                    return false;
                }
                self.go(ScreenId::Menu, now);
            }
            Action::Quit => return false,
        }
        true
    }

    fn main_items(&self) -> Vec<SidebarItem> {
        let th = &self.shared.theme;
        let g = th.glyphs();
        screens::menu::ITEMS
            .iter()
            .map(|(id, title, _)| SidebarItem {
                label: (*title).to_string(),
                hint: match (id, &self.shared.update) {
                    (ScreenId::Update, Some(u)) if u.newer => {
                        Some((g.arrow_up.to_string(), th.warning()))
                    }
                    _ => None,
                },
            })
            .collect()
    }

    fn draw_sidebar(&self, f: &mut Frame, area: Rect, now: Secs) {
        let th = &self.shared.theme;
        match self.current.sidebar() {
            Some(view) => {
                let items: Vec<SidebarItem> = view
                    .items
                    .iter()
                    .map(|l| SidebarItem {
                        label: l.clone(),
                        hint: None,
                    })
                    .collect();
                widgets::sidebar(
                    f,
                    area,
                    th,
                    Some(&view.title),
                    &items,
                    view.selected as f32,
                    Some(view.selected),
                    true,
                );
            }
            None => {
                let current = Self::item_index(self.current_id);
                widgets::sidebar(
                    f,
                    area,
                    th,
                    None,
                    &self.main_items(),
                    self.bar.value(now),
                    current,
                    self.focus == Focus::Sidebar,
                );
            }
        }
    }

    /// Mix every cell's foreground from faint toward its final colour by row age.
    fn settle_pane(buf: &mut Buffer, pane: Rect, th: &Theme, settle: &Settle, now: Secs) {
        for row in 0..pane.height {
            let t = settle.amount(row as usize, now);
            if t >= 1.0 {
                continue;
            }
            for col in 0..pane.width {
                if let Some(c) = buf.cell_mut((pane.x + col, pane.y + row)) {
                    c.fg = th.mix(th.faint, c.fg, t);
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame, now: Secs) {
        let area = f.area();
        let th = &self.shared.theme;
        if area.width < MIN_COLS || area.height < MIN_ROWS {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" terminal too small (need {MIN_COLS}×{MIN_ROWS})"),
                    th.warning(),
                ))),
                area,
            );
            return;
        }
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .areas(area);
        let crumb = (self.current_id != ScreenId::Menu).then(|| self.current.subtitle());
        let status = self.current.status(&self.shared);
        widgets::header(
            f,
            head,
            th,
            now,
            self.shared.ctx.version,
            crumb.as_deref(),
            status.as_ref().map(|(s, tone)| (s.as_str(), *tone)),
        );
        let wide = area.width >= MIN_SIDEBAR_COLS;
        let pane = if wide {
            let [side, sep, pane] = Layout::horizontal([
                Constraint::Length(SIDEBAR_W),
                Constraint::Length(1),
                Constraint::Min(10),
            ])
            .areas(body);
            self.draw_sidebar(f, side, now);
            let bar = if th.unicode { "│" } else { "|" };
            let lines: Vec<Line> = (0..sep.height)
                .map(|_| Line::from(Span::styled(bar, th.faint_style())))
                .collect();
            f.render_widget(Paragraph::new(lines), sep);
            pane
        } else {
            body
        };
        if !wide && self.current_id == ScreenId::Menu {
            // No room for a sidebar: Home is the menu itself.
            self.draw_sidebar(f, pane, now);
        } else {
            self.current.draw(f, pane, &self.shared, now);
            Self::settle_pane(f.buffer_mut(), pane, th, &self.settle, now);
        }
        widgets::footer(f, foot, th, &self.current.keys());
    }

    fn animating(&self, now: Secs) -> bool {
        !self.bar.done(now) || !self.settle.done(now) || self.current.animating(now)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let t0 = Instant::now();
        loop {
            let now = t0.elapsed().as_secs_f64();
            if let Some(rx) = &self.update_rx {
                match rx.try_recv() {
                    Ok(info) => {
                        self.shared.update = Some(info);
                        self.update_rx = None;
                    }
                    // Offline or rate limited: stay quiet and stop looking.
                    Err(mpsc::TryRecvError::Disconnected) => self.update_rx = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            self.current.tick(&mut self.shared, now);
            if self.shared.redraw {
                self.shared.redraw = false;
                terminal.clear()?;
            }
            terminal.draw(|f| self.draw(f, now))?;
            // header glyph and pulse animate at 10 Hz; 60 Hz only while something moves
            let wait = if self.animating(now) { 16 } else { 100 };
            if event::poll(Duration::from_millis(wait))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press && !self.handle(k, now) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Run the TUI. Installs a tracing subscriber that writes into the in-memory log sink so
/// nothing is printed over the UI.
pub fn run(start: Start, ctx: Ctx) -> Result<()> {
    let sink = LogSink::new(500);
    rackscreen_app::logs::install_sink_subscriber(&sink);
    let mut terminal = ratatui::init();
    let result = App::new(start, ctx, sink).run(&mut terminal);
    ratatui::restore();
    result
}
```

Remove the now-unused `SLIDE_SECS` constant and the old `slide` field.

For `left_leaves_a_screen_unless_it_consumes_left` to pass now, add to `impl Screen for Configure` in `crates/setup/src/screens/configure.rs`:

```rust
    fn consumes_left(&self) -> bool {
        true
    }
```

(Task 6 gives `←` its group meaning.) `Screens::new` and `Configure::new` load the config from the temp path and fall back to defaults, so no real shell is touched in these tests.

- [ ] **Step 5: Fix the old Menu tests and run everything**

The old `menu.rs` tests were deleted in Step 1 (their behaviour is now covered by the `lib.rs` tests). Run:

```bash
cargo test -p rackscreen-setup
```
Expected: all tests pass (71 − 3 old menu tests + 3 theme + 2 anim + 4 widgets + 6 shell = 83).

- [ ] **Step 6: Try it by hand**

Run: `cargo run -- setup --sim --config /tmp/rs.yaml`
Expected: header with ring glyph and `v0.4.0`, sidebar with the eight items and the bar sliding on `↑↓`, `Configure` opening with the pane fading in, `Esc` returning to Home. Quit with `q`.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/lib.rs crates/setup/src/screens/menu.rs crates/setup/src/screens/mod.rs crates/setup/src/screens/configure.rs
git commit -m "feat(setup): sidebar shell with focus, settle-in pane and narrow fallback

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Home overview

**Files:**
- Modify: `crates/setup/src/screens/menu.rs`

**Interfaces:**
- Consumes: `status::{collect, Snapshot}` (exists), `ops::systemd::Dot` (exists), `Theme::panel_color`.
- Produces: `pub fn fit_roles(names: &[&str], width: usize) -> String`; `Home::with_snapshot(rows: Vec<(Vec<Role>, u64)>, snap: Option<Snapshot>) -> Home` for tests.

- [ ] **Step 1: Write the failing tests**

Append to `crates/setup/src/screens/menu.rs`:

```rust
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
                vec![Role::Cpu, Role::Mem, Role::Pods, Role::Health, Role::Thermal, Role::Storage, Role::Net, Role::Ups, Role::Deploys],
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
        assert!(t.contains("+"), "long role list is truncated with a count: {t}");
        assert!(t.contains("15 s"));
        assert!(t.contains("static"));
        assert!(t.contains("12:01:20 net"), "newest log line shown");
        assert!(!t.contains("12:00:31 rain"), "only the last three log lines");
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup menu::`
Expected: compile errors: `fit_roles` and `Home::with_snapshot` not found.

- [ ] **Step 3: Implement Home**

Replace the `Home` struct, its impl and the `Screen` impl in `crates/setup/src/screens/menu.rs` (keep `ITEMS`) with:

```rust
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
                            if s.enabled { " enabled" } else { " not enabled" },
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
            lines.push(labelled(th, "Screens", vec![Span::styled("none configured", th.muted())]));
        }
        lines.push(Line::from(""));
        match &shared.banner {
            Some(b) => lines.push(labelled(th, "", vec![Span::styled(b.clone(), th.warning())])),
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
```

Add `use ratatui::style::Style;` to the imports at the top of the file.

`fit_roles` check against the test: `["cpu","mem","pods"]` at width 15: `"cpu › mem › pods"` is 16 chars, too wide; at `i = 2`, `out = "cpu › mem"` (9), so it returns `"cpu › mem +1"`. At width 11: `i = 1`, candidate `"cpu › mem"` (9) + tail `" +1"` (3) = 12 > 11, so returns `"cpu +2"`. At width 5: `i = 0`, `"cpu"` (3) + `" +2"` (3) = 6 > 5, returns `"+3"`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-setup`
Expected: all pass, including the 4 new Home tests.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/screens/menu.rs
git commit -m "feat(setup): Home overview with service, links, fitted screen roles and recent logs

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Configure data model: groups, sections, help, humanised poll

**Files:**
- Modify: `crates/setup/src/screens/configure.rs` (the `FIELDS` table and the tests that destructure it)

**Interfaces:**
- Produces:
  ```rust
  pub enum Group { Cluster, Services, Energy, Sky, Display, Thresholds }
  impl Group { pub const ALL: [Group; 6]; pub fn name(self) -> &'static str; pub fn index(self) -> usize }
  pub struct FieldSpec { pub field: Field, pub group: Group, pub section: &'static str, pub label: &'static str, pub kind: FieldKind, pub help: &'static str }
  pub const FIELDS: [FieldSpec; 44]
  pub fn group_indices(g: Group) -> Vec<usize>
  pub fn section_enabled(cfg: &Config, section: &str) -> Option<bool>
  pub fn humanise_poll(secs: u64) -> String
  ```
- `get`, `set`, `PRICE_SOURCES`, `next_choice`, `Field`, `FieldKind` unchanged.

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `crates/setup/src/screens/configure.rs`, add:

```rust
    #[test]
    fn every_field_is_in_exactly_one_group_in_order() {
        assert_eq!(FIELDS.len(), 44);
        let mut seen: Vec<Field> = Vec::new();
        for s in FIELDS.iter() {
            assert!(!seen.contains(&s.field), "{:?} listed twice", s.field);
            seen.push(s.field);
            assert!(!s.help.is_empty(), "{:?} has no help", s.field);
            assert!(s.help.chars().count() <= 60, "{:?} help too long", s.field);
        }
        // groups are contiguous in FIELDS, in Group::ALL order
        let mut last = 0usize;
        for s in FIELDS.iter() {
            assert!(s.group.index() >= last, "{:?} out of group order", s.field);
            last = s.group.index();
        }
        for g in Group::ALL {
            assert!(!group_indices(g).is_empty(), "{g:?} is empty");
        }
        assert_eq!(group_indices(Group::Cluster), vec![0, 1, 2, 3, 4]);
        assert_eq!(group_indices(Group::Thresholds).len(), 3);
    }

    #[test]
    fn section_enabled_reads_the_bool_of_that_section() {
        let mut c = Config::default();
        assert_eq!(section_enabled(&c, "Prometheus"), None, "always on, no toggle");
        assert_eq!(section_enabled(&c, "Weather"), Some(false));
        c.weather.enabled = true;
        assert_eq!(section_enabled(&c, "Weather"), Some(true));
        assert_eq!(section_enabled(&c, "Nope"), None);
    }

    #[test]
    fn poll_is_humanised() {
        assert_eq!(humanise_poll(15), "every 15 s");
        assert_eq!(humanise_poll(60), "every 1 min");
        assert_eq!(humanise_poll(600), "every 10 min");
        assert_eq!(humanise_poll(90), "every 90 s");
    }
```

Update the three existing tests that destructure tuples:

```rust
        // in get_set_round_trip_and_validation:
        for s in FIELDS.iter() {
            let v = get(&c, s.field);
            assert!(set(&mut c, s.field, &v).is_ok(), "{:?} round trip with {v:?}", s.field);
        }
        // in choice_field_cycles_on_enter:
        screen.row = FIELDS
            .iter()
            .position(|s| s.field == Field::PriceSource)
            .unwrap();
        // in new_fields_round_trip:
        assert!(FIELDS
            .iter()
            .any(|s| s.field == Field::GithubToken && s.kind == FieldKind::Secret));
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup configure::`
Expected: compile errors: `Group`, `FieldSpec`, `group_indices`, `section_enabled`, `humanise_poll` not found.

- [ ] **Step 3: Replace the `FIELDS` table**

Replace the `pub const FIELDS: [(Field, &str, FieldKind); 44] = [...]` block with:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cluster,
    Services,
    Energy,
    Sky,
    Display,
    Thresholds,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Cluster,
        Group::Services,
        Group::Energy,
        Group::Sky,
        Group::Display,
        Group::Thresholds,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Group::Cluster => "Cluster",
            Group::Services => "Services",
            Group::Energy => "Energy",
            Group::Sky => "Sky",
            Group::Display => "Display",
            Group::Thresholds => "Thresholds",
        }
    }
    pub fn index(self) -> usize {
        Group::ALL.iter().position(|g| *g == self).expect("group in ALL")
    }
}

/// One row of the Configure form.
pub struct FieldSpec {
    pub field: Field,
    pub group: Group,
    /// Module header the field sits under (`Prometheus`, `Night`).
    pub section: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    /// One sentence for the help line, under 60 characters.
    pub help: &'static str,
}

const fn spec(
    field: Field,
    group: Group,
    section: &'static str,
    label: &'static str,
    kind: FieldKind,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        field,
        group,
        section,
        label,
        kind,
        help,
    }
}

use FieldKind::{Bool, Choice, Number, Secret, Text};
use Group::{Cluster, Display, Energy, Services, Sky, Thresholds};

pub const FIELDS: [FieldSpec; 44] = [
    spec(Field::Kubeconfig, Cluster, "Kubernetes", "kubeconfig", Text, "Path to the kubeconfig; blank uses in-cluster access."),
    spec(Field::PromNamespace, Cluster, "Prometheus", "namespace", Text, "Namespace the Prometheus service runs in."),
    spec(Field::PromService, Cluster, "Prometheus", "service", Text, "Name of the Prometheus service."),
    spec(Field::PromPort, Cluster, "Prometheus", "port", Number, "Port of the Prometheus service inside the cluster."),
    spec(Field::PromPoll, Cluster, "Prometheus", "poll", Number, "Seconds between Prometheus queries, at least 1."),
    spec(Field::QbitEnabled, Services, "qBittorrent", "enabled", Bool, "Show torrent traffic on the ring."),
    spec(Field::QbitNamespace, Services, "qBittorrent", "namespace", Text, "Namespace of the qBittorrent service."),
    spec(Field::QbitService, Services, "qBittorrent", "service", Text, "Name of the qBittorrent service."),
    spec(Field::QbitPort, Services, "qBittorrent", "port", Number, "Web UI port of qBittorrent."),
    spec(Field::QbitUser, Services, "qBittorrent", "user", Text, "Web UI user name."),
    spec(Field::QbitPass, Services, "qBittorrent", "password", Secret, "Web UI password, stored in the config file."),
    spec(Field::QbitPoll, Services, "qBittorrent", "poll", Number, "Seconds between qBittorrent polls, at least 1."),
    spec(Field::ArgoEnabled, Services, "Argo CD", "enabled", Bool, "Read Applications for the deploys role."),
    spec(Field::ArgoNamespace, Services, "Argo CD", "namespace", Text, "Namespace where Argo CD runs."),
    spec(Field::GithubEnabled, Services, "GitHub", "enabled", Bool, "Fetch your contribution graph."),
    spec(Field::GithubToken, Services, "GitHub", "token", Secret, "Personal access token with read:user."),
    spec(Field::GithubPoll, Services, "GitHub", "poll", Number, "Seconds between GitHub polls, at least 60."),
    spec(Field::ElecEnabled, Energy, "Electricity Maps", "enabled", Bool, "Power mix, carbon and renewable roles."),
    spec(Field::ElecZone, Energy, "Electricity Maps", "zone", Text, "Electricity Maps zone, for example NL."),
    spec(Field::ElecToken, Energy, "Electricity Maps", "api token", Secret, "Electricity Maps API token."),
    spec(Field::ElecPoll, Energy, "Electricity Maps", "poll", Number, "Seconds between polls, at least 60."),
    spec(Field::PriceSource, Energy, "Prices", "source", Choice, "energyzero, entsoe or none; Enter cycles."),
    spec(Field::EntsoeToken, Energy, "Prices", "entsoe token", Secret, "ENTSO-E transparency platform token."),
    spec(Field::EntsoeZone, Energy, "Prices", "entsoe zone", Text, "Bidding zone EIC code for ENTSO-E."),
    spec(Field::IncludeVat, Energy, "Prices", "incl. VAT", Bool, "Show prices including VAT."),
    spec(Field::PricePoll, Energy, "Prices", "poll", Number, "Seconds between price polls, at least 60."),
    spec(Field::LocLat, Sky, "Location", "latitude", Number, "Decimal degrees, -90 to 90; blank clears."),
    spec(Field::LocLon, Sky, "Location", "longitude", Number, "Decimal degrees, -180 to 180; blank clears."),
    spec(Field::WeatherEnabled, Sky, "Weather", "enabled", Bool, "Weather, wind and air quality roles."),
    spec(Field::WeatherPoll, Sky, "Weather", "poll", Number, "Seconds between Open-Meteo polls, at least 60."),
    spec(Field::RainEnabled, Sky, "Rain", "enabled", Bool, "Buienradar nowcast; Netherlands and Belgium."),
    spec(Field::RainPoll, Sky, "Rain", "poll", Number, "Seconds between rain polls, at least 60."),
    spec(Field::IssEnabled, Sky, "ISS", "enabled", Bool, "Countdown to the next ISS pass."),
    spec(Field::IssMinElevation, Sky, "ISS", "min elevation", Number, "Lowest pass elevation to count, 0 to 90°."),
    spec(Field::Brightness, Display, "Panels", "brightness", Number, "0.1 to 1.0; night mode dims further."),
    spec(Field::Fps, Display, "Panels", "fps", Number, "Frames per second, 1 to 60."),
    spec(Field::SpiChunk, Display, "Panels", "spi chunk", Number, "Bytes per SPI transfer, at least 64."),
    spec(Field::OneAtATime, Display, "Panels", "one at a time", Bool, "Only one screen irises at a time."),
    spec(Field::NightEnabled, Display, "Night", "enabled", Bool, "Dim the panels at night."),
    spec(Field::NightStart, Display, "Night", "start", Text, "HH:MM when night begins."),
    spec(Field::NightEnd, Display, "Night", "end", Text, "HH:MM when night ends."),
    spec(Field::HotCpu, Thresholds, "Hot node", "cpu %", Number, "CPU percent that marks a node hot."),
    spec(Field::HotMem, Thresholds, "Hot node", "mem %", Number, "Memory percent that marks a node hot."),
    spec(Field::HotTemp, Thresholds, "Hot node", "temp °C", Number, "Temperature that marks a node hot."),
];

/// Indices into `FIELDS` for one group, in display order.
pub fn group_indices(g: Group) -> Vec<usize> {
    FIELDS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.group == g)
        .map(|(i, _)| i)
        .collect()
}

/// The `enabled` toggle of a section, if it has one.
pub fn section_enabled(cfg: &Config, section: &str) -> Option<bool> {
    FIELDS
        .iter()
        .find(|s| s.section == section && s.kind == FieldKind::Bool && s.label == "enabled")
        .map(|s| get(cfg, s.field) == "true")
}

/// `every 15 s`, or `every 10 min` for whole minutes.
pub fn humanise_poll(secs: u64) -> String {
    if secs >= 60 && secs % 60 == 0 {
        format!("every {} min", secs / 60)
    } else {
        format!("every {secs} s")
    }
}
```

The `use FieldKind::…` and `use Group::…` lines are module-level imports placed right before `FIELDS`; `rustfmt` will keep the long `spec(...)` lines as they are only if they fit within 100 columns, otherwise it wraps them, which is fine.

Fix the two remaining uses of the old tuple shape in `Configure::handle` and `Configure::draw`:

```rust
        // handle():
        let (field, kind) = (FIELDS[self.row].field, FIELDS[self.row].kind);
        // draw(), inside the loop header:
        for (i, s) in FIELDS.iter().enumerate().skip(scroll).take(visible) {
            let (field, label, kind) = (s.field, s.label, s.kind);
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-setup configure::`
Expected: all Configure tests pass, including the 3 new ones.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/screens/configure.rs
git commit -m "refactor(setup): group Configure fields into six groups with sections and help text

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Configure UI: group navigation, sectioned pane, help line, discard dialog

**Files:**
- Modify: `crates/setup/src/screens/configure.rs`

**Interfaces:**
- Consumes: Task 5 `Group`, `FieldSpec`, `group_indices`, `section_enabled`, `humanise_poll`; Task 2 `help_line`, `SidebarView`, `StatusTone`; Task 1 `Theme::highlighted`.
- Produces: `Configure::group(&self) -> Group`; `Mode::AskDiscard`; `Screen::{status, sidebar, consumes_left}` for Configure.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `configure.rs`:

```rust
    #[test]
    fn up_down_stay_in_the_group_and_left_right_switch_groups() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert_eq!(screen.group(), Group::Cluster);
        screen.handle(KeyEvent::from(KeyCode::Up), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Cluster, "wraps inside the group");
        assert_eq!(FIELDS[screen.row].field, Field::PromPoll);
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Services);
        assert_eq!(FIELDS[screen.row].field, Field::QbitEnabled, "first field of the group");
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        screen.handle(KeyEvent::from(KeyCode::Left), &mut sh, 0.0);
        assert_eq!(screen.group(), Group::Thresholds, "groups wrap");
        assert!(screen.consumes_left());
        assert_eq!(screen.sidebar().unwrap().selected, Group::Thresholds.index());
        assert_eq!(screen.sidebar().unwrap().items.len(), 6);
    }

    #[test]
    fn esc_with_changes_asks_before_discarding() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::Back
        ));
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        screen.handle(KeyEvent::from(KeyCode::Char(' ')), &mut sh, 0.0);
        assert!(screen.dirty);
        assert_eq!(screen.status(&sh).unwrap().0, "unsaved");
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::None
        ));
        assert!(matches!(screen.mode, Mode::AskDiscard));
        // anything but y/Enter keeps editing
        screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0);
        assert!(matches!(screen.mode, Mode::Browse));
        screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0);
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Char('y')), &mut sh, 0.0),
            Action::Back
        ));
    }

    #[test]
    fn pane_shows_sections_badges_help_and_humanised_poll() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Configure::from_load(Ok(Config::default()));
        let mut term = Terminal::new(TestBackend::new(70, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("Kubernetes"), "{t}");
        assert!(t.contains("Prometheus"));
        assert!(t.contains("every 5 s"), "default Prometheus poll, humanised: {t}");
        assert!(t.contains("ⓘ Path to the kubeconfig"), "help for the focused field: {t}");
        screen.handle(KeyEvent::from(KeyCode::Right), &mut sh, 0.0);
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("qBittorrent"));
        assert!(t.contains("● on"), "qBittorrent is on by default: {t}");
        assert!(t.contains("○ off"), "GitHub is off by default: {t}");
        assert!(!t.contains("Kubernetes"), "other groups are not drawn");
    }
```

Add `use ratatui::backend::TestBackend; use ratatui::Terminal;` to the test imports.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup configure::`
Expected: compile errors: `group`, `sidebar`, `status`, `Mode::AskDiscard` not found.

- [ ] **Step 3: Implement navigation, modes and the pane**

In `configure.rs`, extend `Mode`:

```rust
enum Mode {
    Browse,
    Edit(String),
    AskRestart,
    /// Esc with unsaved changes: confirm before leaving.
    AskDiscard,
    /// `systemctl restart` is running on a worker thread; keys wait for its result.
    Restarting,
}
```

Add to `impl Configure` (after `from_load`):

```rust
    pub fn group(&self) -> Group {
        FIELDS[self.row].group
    }

    fn move_row(&mut self, delta: i32) {
        let idx = group_indices(self.group());
        let pos = idx.iter().position(|i| *i == self.row).unwrap_or(0) as i32;
        let n = idx.len() as i32;
        self.row = idx[(pos + delta).rem_euclid(n) as usize];
    }

    fn move_group(&mut self, delta: i32) {
        let n = Group::ALL.len() as i32;
        let g = Group::ALL[(self.group().index() as i32 + delta).rem_euclid(n) as usize];
        self.row = group_indices(g)[0];
    }
```

Replace the `Mode::Browse` arm in `handle` with:

```rust
            Mode::Browse => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_row(-1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.move_row(1),
                KeyCode::Left | KeyCode::Char('h') => self.move_group(-1),
                KeyCode::Right | KeyCode::Char('l') => self.move_group(1),
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
                KeyCode::Esc | KeyCode::Char('q') => {
                    if self.dirty {
                        self.mode = Mode::AskDiscard;
                    } else {
                        return Action::Back;
                    }
                }
                _ => {}
            },
```

Add the `AskDiscard` arm after `AskRestart`:

```rust
            Mode::AskDiscard => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                    return Action::Back;
                }
                // any other key keeps the changes and returns to browsing
            }
```

Replace `draw`, `keys`, `subtitle` and add the new trait methods:

```rust
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let [_, list, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(area);

        enum Row {
            Section(&'static str, Option<bool>),
            Field(usize),
            Blank,
        }
        let mut rows: Vec<Row> = Vec::new();
        let mut section = "";
        for i in group_indices(self.group()) {
            let s = &FIELDS[i];
            if s.section != section {
                if !rows.is_empty() {
                    rows.push(Row::Blank);
                }
                rows.push(Row::Section(s.section, section_enabled(&self.cfg, s.section)));
                section = s.section;
            }
            rows.push(Row::Field(i));
        }
        let focus_pos = rows
            .iter()
            .position(|r| matches!(r, Row::Field(i) if *i == self.row))
            .unwrap_or(0);
        let visible = list.height as usize;
        let scroll = if focus_pos >= visible {
            focus_pos + 1 - visible
        } else {
            0
        };
        let width = list.width as usize;
        let mut lines = Vec::new();
        for row in rows.iter().skip(scroll).take(visible) {
            match row {
                Row::Blank => lines.push(Line::from("")),
                Row::Section(name, enabled) => {
                    let mut spans = vec![Span::styled(format!("  {name}"), th.normal())];
                    if let Some(on) = enabled {
                        let badge = if *on {
                            format!("{} on", g.dot)
                        } else {
                            format!("{} off", g.pending)
                        };
                        let used = 2 + name.chars().count();
                        let bw = badge.chars().count() + 2;
                        if used + bw <= width {
                            spans.push(Span::raw(" ".repeat(width - used - bw)));
                        }
                        spans.push(Span::styled(badge, if *on { th.good() } else { th.muted() }));
                    }
                    lines.push(Line::from(spans));
                }
                Row::Field(i) => {
                    let s = &FIELDS[*i];
                    let selected = *i == self.row;
                    let raw = get(&self.cfg, s.field);
                    let off = section_enabled(&self.cfg, s.section) == Some(false)
                        && s.label != "enabled";
                    let shown = match (&self.mode, selected, s.kind) {
                        (Mode::Edit(buf), true, FieldKind::Secret) => {
                            format!("{}_", "•".repeat(buf.chars().count()))
                        }
                        (Mode::Edit(buf), true, _) => format!("{buf}_"),
                        (_, _, FieldKind::Secret) => "•".repeat(raw.chars().count()),
                        (_, _, FieldKind::Bool) => {
                            if raw == "true" {
                                format!("{} on", g.done)
                            } else {
                                format!("{} off", g.pending)
                            }
                        }
                        (_, _, FieldKind::Choice) => format!("‹ {raw} ›"),
                        (_, _, FieldKind::Number) if s.label == "poll" => {
                            raw.parse::<u64>().map(humanise_poll).unwrap_or(raw)
                        }
                        _ => raw,
                    };
                    let editing = matches!(self.mode, Mode::Edit(_)) && selected;
                    let label_style = if selected {
                        th.selected()
                    } else if off {
                        th.faint_style()
                    } else {
                        th.normal()
                    };
                    let value_style = if editing {
                        th.title()
                    } else if s.kind == FieldKind::Bool && raw == "true" && !off {
                        th.good()
                    } else if off {
                        th.faint_style()
                    } else {
                        th.muted()
                    };
                    let pointer = if selected { g.pointer } else { " " };
                    let mut line = Line::from(vec![
                        Span::raw("  "),
                        Span::styled(format!("{pointer} "), th.selected()),
                        Span::styled(format!("{:<16}", s.label), label_style),
                        Span::styled(shown, value_style),
                    ]);
                    if selected {
                        line = line.style(th.highlighted());
                    }
                    lines.push(line);
                }
            }
        }
        f.render_widget(Paragraph::new(lines), list);

        match (&self.mode, &self.error, &self.note) {
            (Mode::Restarting, _, _) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  restarting service...",
                    th.warning(),
                ))),
                help,
            ),
            (_, Some(e), _) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("  {e}"), th.bad()))),
                help,
            ),
            (_, None, Some(n)) => f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("  {n}"), th.good()))),
                help,
            ),
            (_, None, None) => help_line(f, help, th, FIELDS[self.row].help),
        }

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
        if matches!(self.mode, Mode::AskDiscard) {
            confirm_dialog(
                f,
                area,
                th,
                "Discard changes?",
                &["You have unsaved changes. Leave without saving?".to_string()],
                "y/⏎ discard   Esc keep editing",
                true,
            );
        }
    }

    fn keys(&self) -> String {
        match self.mode {
            Mode::Browse => "↑↓ field  ←→ group  ⏎ edit/toggle  s save  Esc back".into(),
            Mode::Edit(_) => "type  ⏎ apply  Esc cancel".into(),
            Mode::AskRestart => "y restart  Esc later".into(),
            Mode::AskDiscard => "y discard  Esc keep".into(),
            Mode::Restarting => "restarting...  Esc back".into(),
        }
    }
    fn subtitle(&self) -> String {
        "Configure".into()
    }
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        self.dirty.then(|| ("unsaved".to_string(), StatusTone::Warn))
    }
    fn consumes_left(&self) -> bool {
        true
    }
    fn sidebar(&self) -> Option<SidebarView> {
        Some(SidebarView {
            title: "Configure".into(),
            items: Group::ALL.iter().map(|g| g.name().to_string()).collect(),
            selected: self.group().index(),
        })
    }
```

Update imports at the top of `configure.rs`:

```rust
use crate::widgets::{confirm_dialog, help_line, SidebarView, StatusTone};
```

Remove the unused `scroll` field from `Configure` (and its initialisers in `from_load`) since the pane now computes scroll from the row position.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-setup`
Expected: all pass. The existing `choice_field_cycles_on_enter` still passes because `row` is set directly to the `PriceSource` index.

- [ ] **Step 5: Try it by hand**

Run: `cargo run -- setup --sim --config /tmp/rs.yaml`, open Configure. Expected: sidebar lists the six groups with the bar on Cluster; `→` moves to Services and the pane shows qBittorrent, Argo CD, GitHub with `○ off` badges; toggling qBittorrent turns its fields from faint to normal; the header shows a pulsing `● unsaved`; Esc asks to discard.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/screens/configure.rs
git commit -m "feat(setup): Configure pane with groups in the sidebar, section badges, help line and discard dialog

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Calibrate: round preview, live pixels, strip, slide

**Files:**
- Modify: `crates/setup/src/screens/calibrate.rs`

**Interfaces:**
- Consumes: Task 2 `pixels`, `StatusTone`; `anim::Slide`; `Theme::{unicode, truecolor}`.
- Produces: `pub fn circle_panel(rotate: u32, hflip: bool, number: usize, unicode: bool) -> Vec<String>` (replaces `mini_panel`), `pub fn orient_label(o: Orientation) -> String`.

- [ ] **Step 1: Update and add the failing tests**

In the `tests` module of `calibrate.rs`:

Replace `mini_panel_follows_rotation_and_flip` with:

```rust
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
        assert_eq!(both[2].chars().nth(6), Some('←'), "right arrow mirrored becomes left");
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
        let two = vec![Orientation { rotate: 0, hflip: false }; 2];
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
        assert!(t.contains("╭─────╮"), "drawn circle fallback without a pixmap: {t}");
    }
```

In `oriented_marker_matches_mini_panel_corner`, rename to `oriented_marker_matches_circle_panel_corner` and replace the `want` line:

```rust
                let want = circle_corner(&circle_panel(rotate, hflip, 1, true));
```

Delete the old `mini_corner` helper.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup calibrate::`
Expected: compile errors: `circle_panel`, `orient_label` not found, `status` not found.

- [ ] **Step 3: Implement `circle_panel`, `orient_label`, the hybrid draw**

Replace `mini_panel` in `calibrate.rs` with:

```rust
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
    if o.rotate % 360 != 0 {
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
```

Column check for the unicode rows: `"╭╯ {tl}  {a}  {tr} ╰╮"` is `╭ ╯ ␣ tl ␣ ␣ a ␣ ␣ tr ␣ ╰ ╮` = 13 characters with `tl` at index 3, `a` at 6, `tr` at 9. `"│     {n}     │"` puts `n` at index 6. The ASCII rows have the same indices.

Add fields to `Calibrate`:

```rust
    /// The oriented frame each panel is showing, for the terminal preview.
    shown: Vec<Pixmap>,
    /// Horizontal offset of the preview, 1.0 = fully off to the right, 0.0 = in place.
    slide: Slide,
```

Initialise `shown: Vec::new(), slide: Slide::fixed(0.0)` in both `new` and `from_parts`. In `new`, where `frames` is created:

```rust
                me.frames = (0..p.handles.len()).map(|_| new_pixmap()).collect();
                me.shown = (0..p.handles.len()).map(|_| new_pixmap()).collect();
```

Rewrite `push`:

```rust
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
```

Remove the `scratch` field and its initialisers.

Add a selection helper and use it in `handle` (replace the four arms that assign `self.selected`):

```rust
    fn select(&mut self, i: usize, now: Secs) {
        if i != self.selected {
            self.selected = i;
            self.slide = Slide::fixed(1.0).to(0.0, now, 0.2);
        }
    }
```

```rust
    fn handle(&mut self, key: KeyEvent, shared: &mut Shared, now: Secs) -> Action {
        // …
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
```

Replace `draw`, `keys`, `subtitle`, and add the trait methods:

```rust
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
                Span::styled(format!("   rotate {}°   flip {}", o.rotate, if o.hflip { "on" } else { "off" }), th.muted()),
            ]));
        } else {
            lines.push(Line::from(Span::styled("  no screens in the config", th.muted())));
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
        lines.push(Line::from(Span::styled("  Match the preview to the glass:", th.muted())));
        lines.push(Line::from(Span::styled("  arrow up, amber dot top-right.", th.muted())));
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
            None => Line::from(Span::styled(format!("  {}", self.message), th.faint_style())),
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
```

Update the imports:

```rust
use crate::anim::Slide;
use crate::widgets::{pixels, StatusTone};
```

and remove the unused `new_pixmap` import only if it is no longer used (it is still used in `new`; keep it).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-setup calibrate::`
Expected: all Calibrate tests pass, including `oriented_marker_matches_circle_panel_corner` for all 8 orientations.

- [ ] **Step 5: Try it by hand**

Run: `cargo run -- setup --sim --config /tmp/rs.yaml`, open Calibrate (the sim opens a window per panel). Expected: the left third shows the selected panel as a round half-block picture, `2` slides the next panel in from the right, `r` rotates both the window and the preview, the strip reads `1 ok    2 rot 90 …` with `▔` under the selected one. Run again with `COLORTERM= cargo run -- setup --sim --config /tmp/rs.yaml`: the preview is the drawn circle.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/screens/calibrate.rs
git commit -m "feat(setup): hybrid Calibrate with a live half-block preview and round fallback

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Screens editor: wrapped roles, panel colours, discard dialog

**Files:**
- Modify: `crates/setup/src/screens/screens.rs`

**Interfaces:**
- Consumes: `Theme::{panel_color, highlighted}`, `StatusTone`, `confirm_dialog`.
- Produces: `pub fn wrap_roles(names: &[&str], width: usize) -> Vec<String>`, `Mode::AskDiscard`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `screens.rs` (it already has a `shared()`-style helper or constructs `Shared` inline; reuse whatever is there, else add one identical to the Home test's `shared()` with `sim: true`):

```rust
    #[test]
    fn wrap_roles_breaks_after_a_separator() {
        let names = ["cpu", "mem", "pods", "health", "thermal"];
        assert_eq!(
            wrap_roles(&names, 20),
            vec!["cpu › mem › pods ›".to_string(), "health › thermal".to_string()]
        );
        assert_eq!(wrap_roles(&names, 100), vec!["cpu › mem › pods › health › thermal".to_string()]);
        assert_eq!(wrap_roles(&["cpu"], 1), vec!["cpu".to_string()], "a name never splits");
        assert_eq!(wrap_roles(&[], 10), Vec::<String>::new());
    }

    #[test]
    fn pane_wraps_long_role_lists_and_marks_unsaved() {
        let dir = tempfile::tempdir().unwrap();
        let mut sh = shared_at(&dir.path().join("config.yaml"));
        let mut screen = Screens::new(&sh);
        screen.editor = Editor::new(vec![
            (
                vec![Role::Cpu, Role::Mem, Role::Pods, Role::Health, Role::Thermal, Role::Storage, Role::Net, Role::Ups, Role::Deploys],
                15,
            ),
            (vec![Role::Health], 15),
        ]);
        let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();
        term.draw(|f| screen.draw(f, f.area(), &sh, 0.0)).unwrap();
        let t = term.backend().to_string();
        assert!(t.contains("cpu › mem"), "{t}");
        assert!(t.contains("deploys"), "the tail of the list is on a later row: {t}");
        assert!(t.contains("every 15 s"));
        assert!(t.contains("static"));
        assert_eq!(screen.status(&sh), None);
        screen.handle(KeyEvent::from(KeyCode::Char('+')), &mut sh, 0.0);
        assert_eq!(screen.status(&sh).unwrap().0, "unsaved");
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Esc), &mut sh, 0.0),
            Action::None
        ));
        assert!(matches!(screen.mode, Mode::AskDiscard));
        assert!(matches!(
            screen.handle(KeyEvent::from(KeyCode::Enter), &mut sh, 0.0),
            Action::Back
        ));
    }
```

If the test module lacks a `shared_at(path)` helper, add:

```rust
    fn shared_at(path: &std::path::Path) -> Shared {
        Shared {
            ctx: crate::Ctx {
                config_path: path.to_path_buf(),
                sim: true,
                version: "0.5.0",
            },
            theme: crate::theme::Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: rackscreen_app::logs::LogSink::new(10),
            update: None,
            redraw: false,
        }
    }
```

and the imports `use ratatui::backend::TestBackend; use ratatui::Terminal;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rackscreen-setup screens::screens::`
Expected: compile errors: `wrap_roles`, `Mode::AskDiscard`, `status` not found.

- [ ] **Step 3: Implement**

Add near the top of `screens.rs` (after the imports):

```rust
/// Join role names with ` › ` onto rows no wider than `width`, breaking after a
/// separator so continuation rows start with a name. A single name never splits.
pub fn wrap_roles(names: &[&str], width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for (i, n) in names.iter().enumerate() {
        let more = i + 1 < names.len();
        let candidate = if cur.is_empty() {
            n.to_string()
        } else {
            format!("{cur} › {n}")
        };
        let tail = if more { 2 } else { 0 }; // room for the trailing " ›"
        if cur.is_empty() || candidate.chars().count() + tail <= width {
            cur = candidate;
        } else {
            lines.push(format!("{cur} ›"));
            cur = n.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
```

Extend `Mode`:

```rust
enum Mode {
    Edit,
    AskRestart,
    AskDiscard,
    Restarting(std::sync::mpsc::Receiver<String>),
}
```

In `handle`, add after the `AskRestart` block:

```rust
        if matches!(self.mode, Mode::AskDiscard) {
            self.mode = Mode::Edit;
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('y')) {
                return Action::Back;
            }
            return Action::None;
        }
```

and change the Esc arm of the main match:

```rust
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.dirty {
                    self.mode = Mode::AskDiscard;
                } else {
                    return Action::Back;
                }
            }
```

Replace `draw` and add `status`:

```rust
    fn draw(&self, f: &mut Frame, area: Rect, shared: &Shared, _now: Secs) {
        let th = &shared.theme;
        let g = th.glyphs();
        let timing_w = 12usize;
        let prefix_w = 7usize; // "  ▸ 1  "
        let roles_w = (area.width as usize).saturating_sub(prefix_w + timing_w + 2).max(8);
        // Rows per screen, wrapped, plus the one-at-a-time toggle and a blank row.
        let mut lines = Vec::new();
        for (i, (rs, secs)) in self.editor.rows().iter().enumerate() {
            let sel = i == self.editor.selected;
            let names: Vec<&str> = rs.iter().map(|r| r.name()).collect();
            let wrapped = wrap_roles(&names, roles_w);
            let timing = if rs.len() > 1 {
                format!("every {secs} s")
            } else {
                "static".to_string()
            };
            let warn = rs.iter().find_map(|r| self.role_hint(*r));
            for (k, text) in wrapped.iter().enumerate() {
                let mut spans = if k == 0 {
                    vec![
                        Span::raw("  "),
                        Span::styled(format!("{} ", if sel { g.pointer } else { " " }), th.selected()),
                        Span::styled(format!("{}  ", i + 1), Style::new().fg(th.panel_color(i))),
                        Span::styled(format!("{text:<roles_w$}"), if sel { th.selected() } else { th.normal() }),
                        Span::styled(format!("  {timing}"), th.muted()),
                    ]
                } else {
                    vec![
                        Span::raw(" ".repeat(prefix_w)),
                        Span::styled(text.clone(), if sel { th.selected() } else { th.normal() }),
                    ]
                };
                if k == 0 {
                    if let Some(w) = warn {
                        spans.push(Span::styled(format!("  ! {w}"), th.warning()));
                    }
                }
                let mut line = Line::from(spans);
                if sel && k == 0 {
                    line = line.style(th.highlighted());
                }
                lines.push(line);
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(prefix_w)),
            Span::styled("one screen at a time  ", th.muted()),
            Span::styled(
                if self.editor.one_at_a_time() {
                    format!("{} on", g.done)
                } else {
                    format!("{} off", g.pending)
                },
                if self.editor.one_at_a_time() { th.good() } else { th.muted() },
            ),
            Span::styled("   o", th.faint_style()),
        ]));
        let list_h = (lines.len() as u16 + 1).min(area.height.saturating_sub(6).max(1));
        let [_, list, roles, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(list_h),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .areas(area);
        f.render_widget(Paragraph::new(lines), list);

        let mut body = vec![Line::from(Span::styled("  Roles", th.muted()))];
        if let Some(p) = self.editor.picker() {
            // `Role::ALL` is taller than the pane on a small terminal: scroll with the cursor.
            let visible = (roles.height as usize).saturating_sub(1).max(1);
            let first = (p.cursor + 1).saturating_sub(visible);
            for (i, r) in Role::ALL.iter().enumerate().skip(first).take(visible) {
                let on = p.chosen.iter().position(|x| x == r);
                let mark = match on {
                    Some(k) => format!("{}{}", g.done, k + 1),
                    None => g.pending.to_string(),
                };
                let cur = i == p.cursor;
                let mut spans = vec![
                    Span::raw("    "),
                    Span::styled(format!("{} ", if cur { g.pointer } else { " " }), th.selected()),
                    Span::styled(format!("{mark:<3}"), if on.is_some() { th.good() } else { th.faint_style() }),
                    Span::styled(format!("{:<14}", r.name()), if cur { th.selected() } else { th.normal() }),
                ];
                if let Some(h) = self.role_hint(*r) {
                    spans.push(Span::styled(format!("! {h}"), th.warning()));
                }
                let mut line = Line::from(spans);
                if cur {
                    line = line.style(th.highlighted());
                }
                body.push(line);
            }
        } else {
            body.push(Line::from(Span::styled(
                format!("    {}", Role::ALL.iter().map(|r| r.name()).collect::<Vec<_>>().join("  ")),
                th.faint_style(),
            )));
            body.push(Line::from(""));
            body.push(Line::from(vec![
                Span::styled("  Presets  ", th.muted()),
                Span::styled("c", th.selected()),
                Span::styled(" cluster   ", th.muted()),
                Span::styled("e", th.selected()),
                Span::styled(" electricity   ", th.muted()),
                Span::styled("m", th.selected()),
                Span::styled(" mixed   ", th.muted()),
                Span::styled("w", th.selected()),
                Span::styled(" sky", th.muted()),
            ]));
        }
        f.render_widget(Paragraph::new(body), roles);

        let msg = match &self.error {
            Some(e) => Line::from(Span::styled(format!("  {e}"), th.bad())),
            None => Line::from(Span::styled(format!("  {}", shared.ctx.config_path.display()), th.faint_style())),
        };
        f.render_widget(Paragraph::new(msg), foot);
        if matches!(self.mode, Mode::AskRestart) {
            confirm_dialog(
                f,
                area,
                th,
                "Restart service?",
                &["Apply the new screen layout to the running service now?".to_string()],
                "y/⏎ restart   Esc later",
                false,
            );
        }
        if matches!(self.mode, Mode::AskDiscard) {
            confirm_dialog(
                f,
                area,
                th,
                "Discard changes?",
                &["You have unsaved changes. Leave without saving?".to_string()],
                "y/⏎ discard   Esc keep editing",
                true,
            );
        }
    }

    fn keys(&self) -> String {
        if self.editor.picker().is_some() {
            "↑↓ move  space toggle  J/K reorder  ⏎ done  Esc cancel".into()
        } else if matches!(self.mode, Mode::AskDiscard) {
            "y discard  Esc keep".into()
        } else {
            "↑↓ screen  ⏎ roles  +/- interval  c/e/m/w preset  o solo  s save  Esc back".into()
        }
    }
    fn subtitle(&self) -> String {
        "Screens".into()
    }
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        self.dirty.then(|| ("unsaved".to_string(), StatusTone::Warn))
    }
```

Add `use ratatui::style::Style;` and `use crate::widgets::{confirm_dialog, StatusTone};` to the imports.

`wrap_roles` check against the test at width 20: `cpu` → `cpu › mem` (9 + 2 ≤ 20) → `cpu › mem › pods` (16 + 2 ≤ 20) → `cpu › mem › pods › health` is 25 + 2 > 20, push `cpu › mem › pods ›`, `cur = health` → `health › thermal` (16 + 0 ≤ 20). Two rows.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-setup`
Expected: all pass.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p rackscreen-setup --all-targets -- -D warnings
git add crates/setup/src/screens/screens.rs
git commit -m "feat(setup): Screens editor with wrapped roles, panel colours and a discard dialog

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Status, Install, Update, Uninstall, Run in the pane; README; version

**Files:**
- Modify: `crates/setup/src/screens/status.rs`
- Modify: `crates/setup/src/screens/install.rs`, `update.rs`, `uninstall.rs`, `run.rs` (only if the manual check shows a clipped row)
- Modify: `README.md`
- Modify: `Cargo.toml` (workspace version)

**Interfaces:**
- Consumes: everything above.
- Produces: `Status::status()` returning `("restarting", Warn)` while a restart runs.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `status.rs`:

```rust
    #[test]
    fn restart_shows_in_the_header_status() {
        let sh = crate::Shared {
            ctx: crate::Ctx {
                config_path: "/etc/rackscreen/config.yaml".into(),
                sim: true,
                version: "0.5.0",
            },
            theme: crate::theme::Theme::new(true),
            service_active: None,
            banner: None,
            log_sink: rackscreen_app::logs::LogSink::new(10),
            update: None,
            redraw: false,
        };
        let mut s = Status::with_snapshot(snapshot_for_test());
        assert_eq!(s.status(&sh), None);
        let (_tx, rx) = mpsc::channel();
        s.restart_rx = Some(rx);
        assert_eq!(s.status(&sh).unwrap().0, "restarting");
    }
```

where `snapshot_for_test()` is the `Snapshot` literal already built inside `renders_snapshot`; extract it into a `fn snapshot_for_test() -> Snapshot` helper in the test module and use it in both tests.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rackscreen-setup status::`
Expected: compile error, `status` not found for `Status` (the trait default exists, but `StatusTone` import and `restart_rx` visibility are checked by the compiler: `restart_rx` is a private field in the same module, so the test compiles once `status` is implemented; before that the assertion on `.0` fails to type-check because the default returns `None`). If it compiles, the assertion `Some("restarting")` fails at runtime.

- [ ] **Step 3: Implement**

In `status.rs` add to `impl Screen for Status`:

```rust
    fn status(&self, _shared: &Shared) -> Option<(String, StatusTone)> {
        self.restart_rx
            .is_some()
            .then(|| ("restarting".to_string(), StatusTone::Warn))
    }
```

with `use crate::widgets::StatusTone;`. Change the `Line::from("")` spacer at the top of the detail view so the first row is not flush with the header rule: the existing `Constraint::Length(1)` already does this; no other change.

- [ ] **Step 4: Run the whole suite and the manual pass**

```bash
cargo test -p rackscreen-setup
cargo run -- setup --sim --config /tmp/rs.yaml
```

Walk every screen in a 100×30 terminal and again in an 80×24 one: Home, Screens (open the picker), Configure (each group, edit a number, toggle a bool, Esc with changes), Calibrate, Status (`l` logs, back), Run here (`q` stops), Update (it will say up to date or offer a download; Esc), Uninstall (Esc at the confirm). Expected: no clipped rows, footer keys change per screen, header crumb matches. If a step list is clipped on 24 rows, reduce that screen's leading `Constraint::Length(1)` spacer to `0`; otherwise leave these files alone.

- [ ] **Step 5: Update the README Setup TUI section**

Replace the `## Setup TUI` section body (the intro sentence and the bullet list) in `README.md` with:

```markdown
## Setup TUI

`rackscreen setup` (what the installer opens) is a ratatui shell: a sidebar on the left with the eight actions, a pane on the right, and a help row at the bottom. `↑↓` move the sidebar bar, `⏎` opens, `Esc` or `←` returns to Home. Below 72 columns the sidebar folds away and Home shows the menu itself.

- **Home** — a live overview: service state and uptime, boot enablement, a dot per link (`k8s`, `prometheus`, `qbittorrent`, `argocd`, `weather`, `rain`, `github`, `electricity`, `prices`), what each screen cycles through (long lists end in `+N`), and the last three log lines.
- **Install** — set up service + config: binary, `/etc/rackscreen/config.yaml`, the `rackscreen@<user>` unit, optional SPI in the boot files.
- **Calibrate** — fix rotation / mirroring. The selected panel is drawn large in the terminal from the very frame the glass shows (half-block pixels, needs a true-colour terminal; otherwise a drawn circle), the other panels are a strip underneath. Press `r`/`R`/`f` until the arrow points up and the dot is top-right, `a` copies to all, `s` writes `rotate` and `hflip`.
- **Screens** — what each screen shows and how fast it cycles; see [Screens and roles](#screens-and-roles).
- **Configure** — every setting in six groups (Cluster, Services, Energy, Sky, Display, Thresholds) picked with `←→` in the sidebar; each module has an on/off badge and the focused field shows a one-line explanation. `s` saves everything and offers a restart; `Esc` with unsaved changes asks first.
- **Status** — the full service detail, links, boot files and the log tail; `l` for the log view, `r` to restart.
- **Run here** — run the daemon in the foreground with its logs, without touching the service.
- **Update** — download the latest release, verify its SHA-256, swap the binary and restart the service.
- **Uninstall** — remove the binary, config and service; the boot file lines stay.
```

- [ ] **Step 6: Bump the version**

In the workspace `Cargo.toml` change `version = "0.4.0"` to `version = "0.5.0"`, then run `cargo build` so `Cargo.lock` follows.

- [ ] **Step 7: Full verification**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clean format, no clippy warnings, all tests pass across the workspace (the render golden tests are untouched by this plan and must still pass).

- [ ] **Step 8: Commit**

```bash
git add crates/setup/src/screens/status.rs README.md Cargo.toml Cargo.lock
git commit -m "feat(setup): header status for restarts, README for the shell, v0.5.0

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review against the spec

- Layout, header, sidebar, focus, footer, narrow terminals: Task 3 (plus widgets in Task 2). Esc handling stays in the screens; the shell intercepts only `←`, which matches the spec's "screens that use Esc for cancel keep working".
- Home: Task 4 (snapshot every 10 s, `not available (sim)`, panel colours, `fit_roles`, three log lines, banner, update hint).
- Configure groups, sections, labels, help, humanised poll, badges, faint fields for off modules, scroll, keys, discard dialog, `● unsaved`: Tasks 5 and 6.
- Calibrate hybrid, `shown` pixmaps, strip with `▔`, slide from the right, fallback circle, `truecolor` detection, `panels live`: Tasks 1, 2, 7.
- Screens editor wrapping, colours, hint in the picker, discard dialog: Task 8.
- Status/Install/Update/Uninstall/Run unchanged apart from the header status: Task 9.
- Motion table: bar (exists, reused by shell), settle (Task 1 and 3), ring (exists), pulse (Task 1 and 2), preview slide (Task 7), gauge and spinner (exist). The spec's toggle flash was removed from the spec together with this plan: a 0.15 s effect that needs per-field timestamps in two screens.
- Tests listed in the spec: pure functions (Tasks 1, 4, 5, 7, 8), widget sizes (Task 2), focus and width (Task 3), manual Pi check (Task 9 step 4 covers the desktop; the Pi check over SSH is part of release, as with earlier milestones).

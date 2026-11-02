//! The small chart inside the card, one kind per role.

use crate::anim::Secs;
use crate::p2::font::FontId;
use crate::p2::layout::CAPTION_PX;
use crate::p2::text::{marquee_run, middle_baseline, run_centred};
use crate::p2::ui::ScreenUi;
use crate::scene::Drawable;
use crate::theme::p2::{BLUE, CYAN, OK, TEXT, TEXT2, TEXT3, TRACK};
use crate::theme::Color;

/// One text line of a `Card::Lines`, `dy` below the chart-area top.
#[derive(Clone, Debug, PartialEq)]
pub struct CardLine {
    pub text: String,
    pub font: FontId,
    pub px: f32,
    pub color: Color,
    pub dy: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Card {
    /// A smooth line; `prev` is the sample that just scrolled off (drawn while
    /// `scroll` < 1 so the line slides left one step), `now_idx` marks a point
    /// other than the newest with a dashed line.
    Spark {
        series: Vec<f32>,
        prev: Option<f32>,
        scroll: f32,
        min: f32,
        max: f32,
        now_idx: Option<usize>,
        color: Color,
        caption: String,
    },
    /// Rounded bars, one colour each; `now` full, earlier at `past_alpha`, later at 0.6.
    Bars {
        values: Vec<f32>,
        colors: Vec<Color>,
        now: usize,
        past_alpha: f32,
        caption: String,
    },
    /// Monday to Sunday on track slots; `today` full, earlier 45 %, later empty.
    Week {
        values: [f32; 7],
        today: usize,
        color: Color,
    },
    /// Two or three `(value, label, value colour)` columns.
    Stats(Vec<(String, String, Color)>),
    /// One rounded bar split into coloured parts; the free part is track.
    StackBar {
        parts: Vec<(f32, Color)>,
        caption: String,
    },
    /// One bar per node on track slots; `highlight` full, the rest at 50 %.
    NodeBars {
        values: Vec<f32>,
        min: f32,
        max: f32,
        labels: Vec<String>,
        colors: Vec<Color>,
        highlight: Option<usize>,
    },
    /// A sliding strip of ticks and letters with `deg` under the centre marker.
    Compass { deg: f32, label: String },
    /// The media download: a marqueed title, a 5 px progress bar (`BLUE`,
    /// `OK` when done) and a caption such as `35 % · 19 min left`.
    Download {
        title: String,
        progress: f32,
        label: String,
        done: bool,
    },
    /// Up to three marqueed text lines (the Jellyfin card).
    Lines(Vec<CardLine>),
}

/// Sample a horizontal-tangent cubic between consecutive points (no overshoot).
pub fn smooth_curve(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    const STEPS: usize = 6;
    let mut out = Vec::with_capacity(pts.len() * STEPS);
    if let Some(first) = pts.first() {
        out.push(*first);
    }
    for w in pts.windows(2) {
        let (o, q) = (w[0], w[1]);
        let m = (o.0 + q.0) / 2.0;
        for s in 1..=STEPS {
            let t = s as f32 / STEPS as f32;
            let u = 1.0 - t;
            let x = u * u * u * o.0 + 3.0 * u * u * t * m + 3.0 * u * t * t * m + t * t * t * q.0;
            let y =
                u * u * u * o.1 + 3.0 * u * u * t * o.1 + 3.0 * u * t * t * q.1 + t * t * t * q.1;
            out.push((x, y));
        }
    }
    out
}

/// Drop everything left of `x0`, inserting the crossing point.
fn clip_left(pts: &[(f32, f32)], x0: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        if p.0 >= x0 {
            if i > 0 && pts[i - 1].0 < x0 {
                let q = pts[i - 1];
                let t = (x0 - q.0) / (p.0 - q.0);
                out.push((x0, q.1 + (p.1 - q.1) * t));
            }
            out.push(*p);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn caption(text: &str, x: f32, y: f32, w: f32, h: f32, dy: f32, px: f32, color: Color) -> Drawable {
    run_centred(
        x + w / 2.0,
        middle_baseline(FontId::Medium, px, y + h + dy),
        text,
        FontId::Medium,
        px,
        0.0,
        color,
        1.0,
    )
}

fn bar(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color, alpha: f32) -> Drawable {
    Drawable::RoundRect {
        x,
        y,
        w,
        h,
        r,
        color,
        alpha,
    }
}

/// Eight-point compass name for a direction in degrees clockwise from north.
pub fn compass_name(deg: f32) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    NAMES[((deg.rem_euclid(360.0) + 22.5) / 45.0).floor() as usize % 8]
}

pub fn card_drawables(
    card: &Card,
    ui: &mut ScreenUi,
    area: (f32, f32, f32, f32),
    now: Secs,
) -> Vec<Drawable> {
    let (x, y, w, h) = area;
    let mut out = Vec::new();
    match card {
        Card::Spark {
            series,
            prev,
            scroll,
            min,
            max,
            now_idx,
            color,
            caption: cap,
        } => {
            if series.len() >= 2 {
                let n = series.len();
                let span = (max - min).max(1e-3);
                let norm = |v: f32| y + h - ((v.clamp(*min, *max) - min) / span) * h;
                let dx = w / (n as f32 - 1.0);
                let shift = if prev.is_some() {
                    dx * (1.0 - scroll.clamp(0.0, 1.0))
                } else {
                    0.0
                };
                let mut pts: Vec<(f32, f32)> = Vec::with_capacity(n + 1);
                if let Some(p) = prev {
                    pts.push((x - dx + shift, norm(*p)));
                }
                for (i, v) in series.iter().enumerate() {
                    pts.push((x + i as f32 * dx + shift, norm(*v)));
                }
                let curve = clip_left(&smooth_curve(&pts), x);
                out.push(Drawable::Polyline {
                    points: curve,
                    width: 1.8,
                    color: *color,
                    alpha: 1.0,
                });
                let li = now_idx.unwrap_or(n - 1).min(n - 1) + usize::from(prev.is_some());
                let lp = pts[li];
                if now_idx.is_some() {
                    let mut yy = y - 2.0;
                    while yy < y + h + 2.0 {
                        out.push(Drawable::Polyline {
                            points: vec![(lp.0, yy), (lp.0, (yy + 2.0).min(y + h + 2.0))],
                            width: 1.0,
                            color: TEXT3,
                            alpha: 1.0,
                        });
                        yy += 4.0;
                    }
                }
                out.push(Drawable::Ellipse {
                    cx: lp.0,
                    cy: lp.1,
                    rx: 2.8,
                    ry: 2.8,
                    color: TEXT,
                    alpha: 1.0,
                });
            }
            out.push(caption(cap, x, y, w, h, 11.0, CAPTION_PX, TEXT3));
        }
        Card::Bars {
            values,
            colors,
            now: now_i,
            past_alpha,
            caption: cap,
        } => {
            let n = values.len().max(1);
            let gap = if n > 12 { 1.6 } else { 2.0 };
            let bw = (w - gap * (n as f32 - 1.0)) / n as f32;
            let mx = values.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
            for (i, v) in values.iter().enumerate() {
                let hh = (ui.glide(("bar", i as u32), v / mx, now) * h).max(1.5);
                let col = colors.get(i).copied().unwrap_or(TEXT2);
                let alpha = if i == *now_i {
                    1.0
                } else if i < *now_i {
                    *past_alpha
                } else {
                    0.6
                };
                out.push(bar(
                    x + i as f32 * (bw + gap),
                    y + h - hh,
                    bw,
                    hh,
                    (bw / 2.0).min(1.5),
                    col,
                    alpha,
                ));
            }
            let cx = x + *now_i as f32 * (bw + gap) + bw / 2.0;
            out.push(Drawable::Ellipse {
                cx,
                cy: y + h + 5.0,
                rx: 1.6,
                ry: 1.6,
                color: TEXT,
                alpha: 1.0,
            });
            out.push(caption(cap, x, y, w, h, 13.0, CAPTION_PX, TEXT3));
        }
        Card::Week {
            values,
            today,
            color,
        } => {
            const LETTERS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
            let gap = 5.0;
            let bw = (w - gap * 6.0) / 7.0;
            let mx = values.iter().cloned().fold(1.0_f32, f32::max);
            let hh0 = h - 9.0;
            for (i, v) in values.iter().enumerate() {
                let x0 = x + i as f32 * (bw + gap);
                out.push(bar(x0, y, bw, hh0, 3.0, TRACK, 1.0));
                if i <= *today && *v > 0.0 {
                    let hh = (ui.glide(("week", i as u32), v / mx, now) * hh0).max(3.0);
                    let alpha = if i == *today { 1.0 } else { 0.45 };
                    out.push(bar(x0, y + hh0 - hh, bw, hh, 3.0, *color, alpha));
                }
                let (col, font) = if i == *today {
                    (TEXT, FontId::Bold)
                } else {
                    (TEXT3, FontId::Medium)
                };
                out.push(run_centred(
                    x0 + bw / 2.0,
                    middle_baseline(font, 8.0, y + h + 3.0),
                    LETTERS[i],
                    font,
                    8.0,
                    0.0,
                    col,
                    1.0,
                ));
            }
        }
        Card::Stats(items) => {
            let n = items.len().max(1);
            let longest = items
                .iter()
                .map(|(v, _, _)| v.chars().count())
                .max()
                .unwrap_or(0);
            let mut vpx = if n >= 3 {
                11.5
            } else if longest > 5 {
                12.5
            } else {
                15.0
            };
            // every value must fit its column with a little air either side
            let col_w = w / n as f32 - 2.0;
            while vpx > 9.0
                && items.iter().any(|(v, _, _)| {
                    crate::p2::font::measure(FontId::SemiBold, vpx, v, 0.0) > col_w
                })
            {
                vpx -= 0.5;
            }
            for (i, (v, l, c)) in items.iter().enumerate() {
                let cx = x + w * (i as f32 + 0.5) / n as f32;
                let vy = y + h * 0.32;
                out.push(run_centred(
                    cx,
                    middle_baseline(FontId::SemiBold, vpx, vy),
                    v,
                    FontId::SemiBold,
                    vpx,
                    0.0,
                    *c,
                    1.0,
                ));
                out.push(run_centred(
                    cx,
                    middle_baseline(FontId::Medium, 8.5, vy + 15.0),
                    l,
                    FontId::Medium,
                    8.5,
                    0.0,
                    TEXT2,
                    1.0,
                ));
            }
            if n == 2 {
                out.push(bar(
                    x + w / 2.0 - 0.5,
                    y + 2.0,
                    1.0,
                    h - 4.0,
                    0.0,
                    TRACK,
                    1.0,
                ));
            }
        }
        Card::StackBar {
            parts,
            caption: cap,
        } => {
            let bh = 8.0;
            let by = y + 4.0;
            out.push(bar(x, by, w, bh, bh / 2.0, TRACK, 1.0));
            let mut fx = x;
            for (i, (f, c)) in parts.iter().enumerate() {
                let pw = ui.glide(("stack", i as u32), f.clamp(0.0, 1.0), now) * w;
                if pw > 1.5 {
                    out.push(bar(fx, by, pw - 1.5, bh, bh / 2.0, *c, 1.0));
                }
                fx += pw;
            }
            out.push(run_centred(
                x + w / 2.0,
                middle_baseline(FontId::Medium, 9.0, by + bh + 11.0),
                cap,
                FontId::Medium,
                9.0,
                0.0,
                TEXT2,
                1.0,
            ));
        }
        Card::NodeBars {
            values,
            min,
            max,
            labels,
            colors,
            highlight,
        } => {
            let n = values.len().max(1);
            let gap = if n > 5 { 4.0 } else { 7.0 };
            let bw = (w - gap * (n as f32 - 1.0)) / n as f32;
            let hh0 = h - 8.0;
            let span = (max - min).max(1e-3);
            for (i, v) in values.iter().enumerate() {
                let x0 = x + i as f32 * (bw + gap);
                let f = ((v - min) / span).clamp(0.04, 1.0);
                let hh = ui.glide(("node", i as u32), f, now) * hh0;
                let col = colors.get(i).copied().unwrap_or(TEXT2);
                let alpha = if highlight.is_none() || *highlight == Some(i) {
                    1.0
                } else {
                    0.5
                };
                out.push(bar(x0, y, bw, hh0, 2.5, TRACK, 1.0));
                out.push(bar(x0, y + hh0 - hh, bw, hh, 2.5, col, alpha));
                let lc = if *highlight == Some(i) { TEXT } else { TEXT3 };
                let label = labels.get(i).map(String::as_str).unwrap_or("");
                out.push(run_centred(
                    x0 + bw / 2.0,
                    middle_baseline(FontId::SemiBold, 7.5, y + h + 3.0),
                    label,
                    FontId::SemiBold,
                    7.5,
                    0.0,
                    lc,
                    1.0,
                ));
            }
        }
        Card::Compass { deg, label } => {
            let deg = ui.glide_angle(("compass", 0), *deg, now, 1.2);
            let cx = x + w / 2.0;
            let pxd = w / 180.0;
            let mut d = -180.0;
            while d <= 540.0 {
                let xx = cx + (d - deg) * pxd;
                if xx >= x && xx <= x + w {
                    let dd = (d as i32).rem_euclid(360);
                    let major = dd % 45 == 0;
                    out.push(Drawable::Polyline {
                        points: vec![(xx, y + if major { 2.0 } else { 5.0 }), (xx, y + 9.0)],
                        width: 1.0,
                        color: if major { TEXT2 } else { TEXT3 },
                        alpha: 1.0,
                    });
                    if major {
                        let col = if (xx - cx).abs() < 6.0 { TEXT } else { TEXT2 };
                        out.push(run_centred(
                            xx,
                            middle_baseline(FontId::SemiBold, 8.0, y + 17.0),
                            compass_name(dd as f32),
                            FontId::SemiBold,
                            8.0,
                            0.0,
                            col,
                            1.0,
                        ));
                    }
                }
                d += 15.0;
            }
            out.push(Drawable::Polygon {
                points: vec![(cx - 4.0, y - 3.0), (cx + 4.0, y - 3.0), (cx, y + 3.0)],
                color: CYAN,
                alpha: 1.0,
            });
            out.push(caption(label, x, y, w, h, 3.0, CAPTION_PX, TEXT3));
        }
        Card::Download {
            title,
            progress,
            label,
            done,
        } => {
            out.extend(marquee_run(
                ui,
                ("dl-title", 0),
                title,
                x + w / 2.0,
                middle_baseline(FontId::SemiBold, 10.0, y + 3.0),
                w,
                FontId::SemiBold,
                10.0,
                TEXT,
                y - 5.0,
                16.0,
                now,
            ));
            let bh = 5.0;
            let by = y + 13.0;
            out.push(bar(x, by, w, bh, bh / 2.0, TRACK, 1.0));
            let p = ui.glide_with(("dl", 0), progress.clamp(0.0, 1.0), now, 0.6);
            let col = if *done { OK } else { BLUE };
            out.push(bar(x, by, (p * w).max(bh), bh, bh / 2.0, col, 1.0));
            out.push(caption(
                label,
                x,
                by,
                w,
                bh,
                10.0,
                CAPTION_PX,
                if *done { OK } else { TEXT2 },
            ));
        }
        Card::Lines(lines) => {
            for (i, l) in lines.iter().enumerate() {
                out.extend(marquee_run(
                    ui,
                    ("line", i as u32),
                    &l.text,
                    x + w / 2.0,
                    middle_baseline(l.font, l.px, y + l.dy),
                    w,
                    l.font,
                    l.px,
                    l.color,
                    y + l.dy - l.px * 0.8,
                    l.px * 1.6,
                    now,
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::ui::ScreenUi;
    use crate::theme::p2::{BLUE, GREEN, TEXT, TEXT3, TRACK};

    const AREA: (f32, f32, f32, f32) = (76.0, 152.0, 88.0, 26.0);

    fn count<F: Fn(&Drawable) -> bool>(ds: &[Drawable], f: F) -> usize {
        ds.iter().filter(|d| f(d)).count()
    }

    fn rects(ds: &[Drawable]) -> Vec<(f32, f32, f32, f32, Color, f32)> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::RoundRect {
                    x,
                    y,
                    w,
                    h,
                    color,
                    alpha,
                    ..
                } => Some((*x, *y, *w, *h, *color, *alpha)),
                _ => None,
            })
            .collect()
    }

    fn texts(ds: &[Drawable]) -> Vec<String> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn spark_draws_a_clipped_line_a_dot_and_a_caption() {
        let mut ui = ScreenUi::new();
        let series: Vec<f32> = (0..31).map(|i| 30.0 + i as f32).collect();
        let card = Card::Spark {
            series: series.clone(),
            prev: Some(29.0),
            scroll: 0.5,
            min: 0.0,
            max: 100.0,
            now_idx: None,
            color: BLUE,
            caption: "last 60 s".into(),
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let line = ds
            .iter()
            .find_map(|d| match d {
                Drawable::Polyline {
                    points,
                    width,
                    color,
                    ..
                } if *width == 1.8 && *color == BLUE => Some(points.clone()),
                _ => None,
            })
            .expect("the line");
        assert!(
            line.iter().all(|p| p.0 >= AREA.0 - 1e-3),
            "clipped at the left edge"
        );
        assert!(line.len() > series.len(), "the cubic is sampled");
        let dot = ds.iter().find(|d| matches!(d, Drawable::Ellipse { rx, color, .. } if *rx == 2.8 && *color == TEXT)).unwrap();
        if let Drawable::Ellipse { cx, cy, .. } = dot {
            assert!(
                (cx - (AREA.0 + AREA.2 + (88.0 / 30.0) * 0.5)).abs() < 1e-3,
                "newest point, shifted by the half-step scroll"
            );
            assert!((cy - (AREA.1 + AREA.3 - 0.6 * AREA.3)).abs() < 1e-3);
        }
        assert_eq!(texts(&ds), vec!["last 60 s".to_string()]);
        let now_card = Card::Spark {
            series,
            prev: None,
            scroll: 1.0,
            min: 0.0,
            max: 100.0,
            now_idx: Some(10),
            color: BLUE,
            caption: "24 h".into(),
        };
        let ds = card_drawables(&now_card, &mut ui, AREA, 0.0);
        assert!(
            count(
                &ds,
                |d| matches!(d, Drawable::Polyline { width, color, .. } if *width == 1.0 && *color == TEXT3)
            ) >= 6,
            "dashed now line"
        );
        let short = Card::Spark {
            series: vec![1.0],
            prev: None,
            scroll: 1.0,
            min: 0.0,
            max: 1.0,
            now_idx: None,
            color: BLUE,
            caption: "24 h".into(),
        };
        assert_eq!(
            card_drawables(&short, &mut ui, AREA, 0.0).len(),
            1,
            "fewer than 2 points: caption only"
        );
    }

    #[test]
    fn bars_highlight_now_and_dim_past_and_future() {
        let mut ui = ScreenUi::new();
        let card = Card::Bars {
            values: vec![1.0, 2.0, 4.0, 2.0],
            colors: vec![BLUE; 4],
            now: 2,
            past_alpha: 0.28,
            caption: "per hour".into(),
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].5, 0.28);
        assert_eq!(r[2].5, 1.0);
        assert_eq!(r[3].5, 0.6);
        assert!(
            (r[2].3 - AREA.3).abs() < 1e-3,
            "the max bar fills the height"
        );
        assert!((r[0].3 - AREA.3 / 4.0).abs() < 1e-3);
        let bw = (AREA.2 - 2.0 * 3.0) / 4.0;
        assert!((r[1].0 - (AREA.0 + bw + 2.0)).abs() < 1e-3);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Ellipse { rx, cy, .. } if *rx == 1.6 && (*cy - (AREA.1 + AREA.3 + 5.0)).abs() < 1e-3)));
        assert_eq!(texts(&ds), vec!["per hour".to_string()]);
    }

    #[test]
    fn week_has_seven_slots_letters_and_no_future_bars() {
        let mut ui = ScreenUi::new();
        let card = Card::Week {
            values: [3.0, 0.0, 5.0, 2.0, 1.0, 0.0, 0.0],
            today: 4,
            color: GREEN,
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(
            r.iter().filter(|x| x.4 == TRACK).count(),
            7,
            "seven track slots"
        );
        let bars: Vec<_> = r.iter().filter(|x| x.4 == GREEN).collect();
        assert_eq!(bars.len(), 4, "days with a count up to today");
        assert_eq!(bars[3].5, 1.0, "today full");
        assert_eq!(bars[0].5, 0.45);
        assert_eq!(texts(&ds), ["M", "T", "W", "T", "F", "S", "S"]);
    }

    #[test]
    fn stats_pick_their_size_and_divide_two_columns() {
        let mut ui = ScreenUi::new();
        let two = Card::Stats(vec![
            ("2".into(), "pending".into(), TEXT),
            ("0".into(), "failed".into(), TEXT3),
        ]);
        let ds = card_drawables(&two, &mut ui, AREA, 0.0);
        assert_eq!(texts(&ds), ["2", "pending", "0", "failed"]);
        assert!(ds
            .iter()
            .any(|d| matches!(d, Drawable::Run { px, .. } if *px == 15.0)));
        assert_eq!(rects(&ds).len(), 1, "one divider");
        let wide = Card::Stats(vec![
            ("21:14".into(), "start".into(), TEXT),
            ("6 min".into(), "duration".into(), TEXT),
        ]);
        let ds = card_drawables(&wide, &mut ui, AREA, 0.0);
        assert!(
            ds.iter().any(
                |d| matches!(d, Drawable::Run { px, text, .. } if *px == 15.0 && text == "21:14")
            ),
            "5 chars still 15 px"
        );
        let wider = Card::Stats(vec![
            ("49 min".into(), "runtime".into(), TEXT),
            ("7%".into(), "load".into(), TEXT),
        ]);
        let ds = card_drawables(&wider, &mut ui, AREA, 0.0);
        assert!(ds.iter().any(
            |d| matches!(d, Drawable::Run { px, text, .. } if *px == 12.5 && text == "49 min")
        ));
        let three = Card::Stats(vec![("a".into(), "x".into(), TEXT); 3]);
        let ds = card_drawables(&three, &mut ui, AREA, 0.0);
        assert!(ds
            .iter()
            .any(|d| matches!(d, Drawable::Run { px, .. } if *px == 11.5)));
        assert_eq!(rects(&ds).len(), 0);
    }

    #[test]
    fn stackbar_nodebars_and_compass() {
        let mut ui = ScreenUi::new();
        let sb = Card::StackBar {
            parts: vec![(0.16, BLUE), (0.13, GREEN)],
            caption: "1.9 TB free".into(),
        };
        let ds = card_drawables(&sb, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 3, "track and two segments");
        assert!((r[1].2 - (0.16 * AREA.2 - 1.5)).abs() < 1e-3);
        assert!((r[2].0 - (AREA.0 + 0.16 * AREA.2)).abs() < 1e-3);
        assert_eq!(texts(&ds), vec!["1.9 TB free".to_string()]);
        let nb = Card::NodeBars {
            values: vec![46.0, 54.0, 70.0],
            min: 30.0,
            max: 85.0,
            labels: vec!["c1".into(), "p1".into(), "p2".into()],
            colors: vec![GREEN, GREEN, BLUE],
            highlight: Some(2),
        };
        let ds = card_drawables(&nb, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 6);
        assert_eq!(r[5].5, 1.0, "highlighted bar full");
        assert_eq!(r[1].5, 0.5);
        assert!((r[5].3 / (AREA.3 - 8.0) - (40.0 / 55.0)).abs() < 1e-3);
        assert_eq!(texts(&ds), ["c1", "p1", "p2"]);
        let c = Card::Compass {
            deg: 225.0,
            label: "SW · 225°".into(),
        };
        let ds = card_drawables(&c, &mut ui, AREA, 0.0);
        assert!(ds
            .iter()
            .any(|d| matches!(d, Drawable::Polygon { points, .. } if points.len() == 3)));
        let t = texts(&ds);
        assert!(t.contains(&"SW".to_string()) && t.contains(&"SW · 225°".to_string()));
        assert!(
            !t.contains(&"N".to_string()),
            "north is 225° away, off the strip"
        );
        assert_eq!(compass_name(225.0), "SW");
        assert_eq!(compass_name(359.0), "N");
        assert_eq!(compass_name(100.0), "E");
    }
    #[test]
    fn download_card_has_title_bar_and_caption() {
        let mut ui = ScreenUi::new();
        let card = Card::Download {
            title: "Dune: Part Two".into(),
            progress: 0.35,
            label: "35 % · 19 min left".into(),
            done: false,
        };
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let r = rects(&ds);
        assert_eq!(r.len(), 2, "track and fill");
        assert_eq!(
            (r[0].0, r[0].1, r[0].2, r[0].3, r[0].4),
            (AREA.0, AREA.1 + 13.0, AREA.2, 5.0, TRACK)
        );
        assert_eq!(r[1].4, BLUE);
        assert!((r[1].2 - 0.35 * AREA.2).abs() < 1e-3);
        let t = texts(&ds);
        assert_eq!(t, ["Dune: Part Two", "35 % · 19 min left"]);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, px, font, color, .. } if text == "Dune: Part Two" && *px == 10.0 && *font == FontId::SemiBold && *color == TEXT)));
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, px, color, .. } if text == "35 % · 19 min left" && *px == 8.5 && *color == TEXT2)));
        let done = Card::Download {
            title: "Dune: Part Two".into(),
            progress: 1.0,
            label: "added just now".into(),
            done: true,
        };
        let ds = card_drawables(&done, &mut ui, AREA, 5.0);
        let r = rects(&ds);
        assert_eq!(r[1].4, OK);
        assert!(ds.iter().any(|d| matches!(d, Drawable::Run { text, color, .. } if text == "added just now" && *color == OK)));
        let tiny = Card::Download {
            title: "x".into(),
            progress: 0.0,
            label: "0 % · --".into(),
            done: false,
        };
        let ds = card_drawables(&tiny, &mut ScreenUi::new(), AREA, 6.0);
        assert!(
            (rects(&ds)[1].2 - 5.0).abs() < 1e-3,
            "never thinner than its height"
        );
    }

    #[test]
    fn lines_card_places_each_line_and_marquees_long_ones() {
        let mut ui = ScreenUi::new();
        let card = Card::Lines(vec![
            CardLine {
                text: "Blade Runner".into(),
                font: FontId::Bold,
                px: 11.0,
                color: TEXT,
                dy: 7.0,
            },
            CardLine {
                text: "2017 · 2h 44m".into(),
                font: FontId::Medium,
                px: 8.5,
                color: TEXT2,
                dy: 21.0,
            },
            CardLine {
                text: "silke · Living room TV  ·  +1 more".into(),
                font: FontId::Medium,
                px: 8.5,
                color: TEXT3,
                dy: 32.0,
            },
        ]);
        let ds = card_drawables(&card, &mut ui, AREA, 0.0);
        let t = texts(&ds);
        assert_eq!(t.len(), 4, "the long third line is drawn twice");
        assert_eq!(
            &t[..3],
            [
                "Blade Runner",
                "2017 · 2h 44m",
                "silke · Living room TV  ·  +1 more"
            ]
        );
        let base: Vec<f32> = ds
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { baseline, .. } => Some(*baseline),
                _ => None,
            })
            .collect();
        assert!((base[0] - middle_baseline(FontId::Bold, 11.0, AREA.1 + 7.0)).abs() < 1e-3);
        assert!((base[1] - middle_baseline(FontId::Medium, 8.5, AREA.1 + 21.0)).abs() < 1e-3);
        assert!(base[2] > base[1] && base[1] > base[0]);
        assert_eq!(rects(&ds).len(), 0, "no bars, no divider");
    }
}

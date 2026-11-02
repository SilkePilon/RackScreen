//! Text placed by core: the curved title, the tabular rolling hero with its
//! unit, the pill (with a marquee when the text is too wide) and scale ends.

use crate::anim::Secs;
use crate::p2::font::{advance, cap_height, digit_width, measure, FontId};
use crate::p2::layout::*;
use crate::p2::marquee::GAP_PX;
use crate::p2::ui::{ScreenUi, UiKey};
use crate::scene::{ClipRect, Drawable};
use crate::theme::layout::CX;
use crate::theme::p2::{TEXT2, TEXT3};
use crate::theme::Color;

/// Baseline that puts the cap height of `font` at `px` centred on `cy`.
pub fn middle_baseline(font: FontId, px: f32, cy: f32) -> f32 {
    cy + cap_height(font, px) / 2.0
}

#[allow(clippy::too_many_arguments)]
pub fn run_centred(
    cx: f32,
    baseline: f32,
    text: &str,
    font: FontId,
    px: f32,
    letter_spacing: f32,
    color: Color,
    alpha: f32,
) -> Drawable {
    let w = measure(font, px, text, letter_spacing);
    Drawable::Run {
        x: cx - w / 2.0,
        baseline,
        text: text.to_string(),
        font,
        px,
        letter_spacing,
        color,
        alpha,
        clip: None,
    }
}

/// The curved title in the ring gap: upper case, first word only when the
/// whole title would not fit the gap.
pub fn curved_title(title: &str) -> Drawable {
    let upper = title.to_uppercase();
    let text = if measure(FontId::Bold, TITLE_PX, &upper, TITLE_LS) > TITLE_MAX_W {
        upper.split(' ').next().unwrap_or("").to_string()
    } else {
        upper
    };
    Drawable::CurvedText {
        text,
        radius: TITLE_R,
        centre_deg: 0.0,
        px: TITLE_PX,
        font: FontId::Bold,
        letter_spacing: TITLE_LS,
        color: TEXT2,
        alpha: 1.0,
    }
}

/// Cell widths for a hero string: digits take the `0` cell, the rest their advance.
fn cell_widths(text: &str, px: f32) -> Vec<f32> {
    let digit = digit_width(FontId::Bold, px);
    text.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                digit
            } else {
                advance(FontId::Bold, px, c)
            }
        })
        .collect()
}

/// Width of a hero at `scale`, for the fit tests.
pub fn hero_width(text: &str, scale: f32) -> f32 {
    cell_widths(text, HERO_PX * scale).iter().sum()
}

pub struct HeroLayout {
    pub drawables: Vec<Drawable>,
    pub x0: f32,
    pub x1: f32,
    pub px: f32,
    pub baseline: f32,
}

/// The rolling hero, centred on the screen: one clipped `Run` per glyph.
pub fn hero(
    ui: &mut ScreenUi,
    text: &str,
    scale: f32,
    baseline: f32,
    color: Color,
    now: Secs,
) -> HeroLayout {
    hero_at(ui, text, scale, CX, baseline, color, now)
}

/// The rolling hero centred on `cx`.
#[allow(clippy::too_many_arguments)]
pub fn hero_at(
    ui: &mut ScreenUi,
    text: &str,
    scale: f32,
    cx: f32,
    baseline: f32,
    color: Color,
    now: Secs,
) -> HeroLayout {
    let px = HERO_PX * scale;
    let roll = ui.roll(("hero", 0), text, now);
    let widths = cell_widths(roll.text(), px);
    let total: f32 = widths.iter().sum();
    let x0 = cx - total / 2.0;
    let mut xs = Vec::with_capacity(widths.len());
    let mut x = x0;
    for w in &widths {
        xs.push(x);
        x += w;
    }
    let mut drawables = Vec::with_capacity(widths.len() * 2);
    for g in roll.glyphs(now) {
        let w = widths[g.cell];
        let gx = xs[g.cell] + (w - advance(FontId::Bold, px, g.ch)) / 2.0;
        drawables.push(Drawable::Run {
            x: gx,
            baseline: baseline + g.dy * px,
            text: g.ch.to_string(),
            font: FontId::Bold,
            px,
            letter_spacing: 0.0,
            color,
            alpha: g.alpha,
            clip: Some(ClipRect {
                x: xs[g.cell] - 2.0,
                y: baseline - px * 0.95,
                w: w + 4.0,
                h: px * 1.15,
            }),
        });
    }
    HeroLayout {
        drawables,
        x0,
        x1: x0 + total,
        px,
        baseline,
    }
}

/// Width of the hero plus its superscript unit (gap included).
pub fn hero_group_width(text: &str, unit: Option<&str>, scale: f32) -> f32 {
    let px = HERO_PX * scale;
    hero_width(text, scale)
        + unit.map_or(0.0, |u| {
            UNIT_GAP + measure(FontId::Bold, px * UNIT_SCALE, u, 0.0)
        })
}

/// All four corners of the hero group box are at least `HERO_RING_MARGIN`
/// inside the ring's inner edge.
pub fn hero_fits(text: &str, unit: Option<&str>, scale: f32, baseline: f32) -> bool {
    use crate::theme::layout::CY;
    let half = hero_group_width(text, unit, scale) / 2.0;
    let top = baseline - cap_height(FontId::Bold, HERO_PX * scale);
    let limit = RING_R - RING_W / 2.0 - HERO_RING_MARGIN;
    [top, baseline]
        .iter()
        .all(|y| (half * half + (y - CY) * (y - CY)).sqrt() <= limit)
}

/// The largest scale up to `max_scale` at which the hero group fits the ring.
pub fn hero_fit_scale(text: &str, unit: Option<&str>, max_scale: f32, baseline: f32) -> f32 {
    if hero_fits(text, unit, max_scale, baseline) {
        return max_scale;
    }
    let (mut lo, mut hi) = (0.2_f32, max_scale);
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        if hero_fits(text, unit, mid, baseline) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// A unit to the right of the hero, Bold at 0.36 × the hero size, its top on
/// the hero's cap height.
pub fn unit_superscript(h: &HeroLayout, unit: &str) -> Drawable {
    let upx = h.px * UNIT_SCALE;
    let top = h.baseline - cap_height(FontId::Bold, h.px);
    Drawable::Run {
        x: h.x1 + UNIT_GAP,
        baseline: top + cap_height(FontId::Bold, upx),
        text: unit.to_string(),
        font: FontId::Bold,
        px: upx,
        letter_spacing: 0.0,
        color: TEXT2,
        alpha: 1.0,
        clip: None,
    }
}

/// A unit word centred under the hero (`Mb/s`, `km/h`, `€/kWh`).
pub fn unit_below(unit: &str) -> Drawable {
    run_centred(
        CX,
        HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY,
        unit,
        FontId::Medium,
        UNIT_BELOW_PX,
        0.0,
        TEXT2,
        1.0,
    )
}

/// The status pill: fill at 16 % of the colour, SemiBold text; text wider
/// than 132 px scrolls, hard-clipped to the slot.
pub fn pill(ui: &mut ScreenUi, text: &str, color: Color, cy: f32, now: Secs) -> Vec<Drawable> {
    let tw = measure(FontId::SemiBold, PILL_PX, text, 0.0);
    let slot = tw.min(PILL_MAX_TEXT_W);
    let w = slot + PILL_PAD;
    let baseline = middle_baseline(FontId::SemiBold, PILL_PX, cy + 0.5);
    let mut out = vec![Drawable::RoundRect {
        x: CX - w / 2.0,
        y: cy - PILL_H / 2.0,
        w,
        h: PILL_H,
        r: PILL_R,
        color,
        alpha: PILL_FILL_ALPHA,
    }];
    let run = |x: f32, clip: Option<ClipRect>| Drawable::Run {
        x,
        baseline,
        text: text.to_string(),
        font: FontId::SemiBold,
        px: PILL_PX,
        letter_spacing: 0.0,
        color,
        alpha: 1.0,
        clip,
    };
    let mq = ui.marquee(("pill", 0), text, now);
    match mq.offset(tw, PILL_MAX_TEXT_W, now) {
        None => out.push(run(CX - tw / 2.0, None)),
        Some(off) => {
            let x0 = CX - PILL_MAX_TEXT_W / 2.0;
            let clip = Some(ClipRect {
                x: x0,
                y: cy - PILL_H / 2.0,
                w: PILL_MAX_TEXT_W,
                h: PILL_H,
            });
            out.push(run(x0 - off, clip));
            out.push(run(x0 - off + tw + GAP_PX, clip));
        }
    }
    out
}

/// Scale labels just inside the two ends of the ring (start upper right, end
/// upper left), Medium 8.5 px `TEXT3`.
pub fn ring_ends(start: &str, end: &str) -> Vec<Drawable> {
    let at = |deg: f32, text: &str| {
        let a = deg.to_radians();
        let (cx, cy) = (
            CX + ENDS_R * a.sin(),
            crate::theme::layout::CY - ENDS_R * a.cos(),
        );
        run_centred(
            cx,
            middle_baseline(FontId::Medium, ENDS_PX, cy),
            text,
            FontId::Medium,
            ENDS_PX,
            0.0,
            TEXT3,
            1.0,
        )
    };
    vec![
        at(RING_START_DEG, start),
        at(RING_START_DEG + RING_SWEEP_DEG, end),
    ]
}

/// A text run centred on `cx` that scrolls (hold, slide, wrap) when wider than
/// `slot_w`, hard-clipped to `slot_w × clip_h` at `clip_y`. Cards use it for
/// titles and captions; the pill has its own copy of the same rule.
#[allow(clippy::too_many_arguments)]
pub fn marquee_run(
    ui: &mut ScreenUi,
    key: UiKey,
    text: &str,
    cx: f32,
    baseline: f32,
    slot_w: f32,
    font: FontId,
    px: f32,
    color: Color,
    clip_y: f32,
    clip_h: f32,
    now: Secs,
) -> Vec<Drawable> {
    let tw = measure(font, px, text, 0.0);
    let run = |x: f32, clip: Option<ClipRect>| Drawable::Run {
        x,
        baseline,
        text: text.to_string(),
        font,
        px,
        letter_spacing: 0.0,
        color,
        alpha: 1.0,
        clip,
    };
    let mq = ui.marquee(key, text, now);
    match mq.offset(tw, slot_w, now) {
        None => vec![run(cx - tw / 2.0, None)],
        Some(off) => {
            let x0 = cx - slot_w / 2.0;
            let clip = Some(ClipRect {
                x: x0,
                y: clip_y,
                w: slot_w,
                h: clip_h,
            });
            vec![run(x0 - off, clip), run(x0 - off + tw + GAP_PX, clip)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::font::{cap_height, digit_width, measure};
    use crate::p2::ui::ScreenUi;
    use crate::theme::p2::{BLUE, TEXT, TEXT2};

    fn runs(ds: &[Drawable]) -> Vec<(f32, f32, String, f32, f32, bool)> {
        ds.iter()
            .filter_map(|d| match d {
                Drawable::Run {
                    x,
                    baseline,
                    text,
                    px,
                    alpha,
                    clip,
                    ..
                } => Some((*x, *baseline, text.clone(), *px, *alpha, clip.is_some())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn long_titles_use_their_first_word() {
        match curved_title("partly cloudy forecast") {
            Drawable::CurvedText {
                text,
                radius,
                centre_deg,
                px,
                letter_spacing,
                color,
                ..
            } => {
                assert_eq!(text, "PARTLY");
                assert_eq!(
                    (radius, centre_deg, px, letter_spacing, color),
                    (TITLE_R, 0.0, TITLE_PX, TITLE_LS, TEXT2)
                );
            }
            _ => panic!("curved text"),
        }
        assert!(matches!(curved_title("cpu"), Drawable::CurvedText { text, .. } if text == "CPU"));
        assert!(
            matches!(curved_title("air quality"), Drawable::CurvedText { text, .. } if text == "AIR QUALITY")
        );
    }

    #[test]
    fn hero_lays_digits_in_tabular_cells_and_rolls_changed_ones() {
        let mut ui = ScreenUi::new();
        let h = hero(&mut ui, "42", 1.0, HERO_BASELINE, TEXT, 0.0);
        let cell = digit_width(FontId::Bold, HERO_PX);
        assert!((h.x1 - h.x0 - 2.0 * cell).abs() < 1e-3);
        assert!(((h.x0 + h.x1) / 2.0 - 120.0).abs() < 1e-3, "centred");
        let r = runs(&h.drawables);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].2, "4");
        assert!(r[0].5, "every glyph is clipped to its cell");
        assert_eq!(r[0].3, HERO_PX);
        assert!((hero_width("14:32", 0.82) - hero_width("14:32", 1.0) * 0.82).abs() < 1e-3);
        assert!(hero_width("100", 1.0) < HERO_MAX_W && hero_width("16/16", 0.82) < HERO_MAX_W);
        let h2 = hero(&mut ui, "43", 1.0, HERO_BASELINE, TEXT, 1.0);
        let r2 = runs(&h2.drawables);
        assert_eq!(
            r2.len(),
            2,
            "at the start of the roll cell 1 still shows the old 2"
        );
        assert_eq!(r2[1].2, "2");
        let h3 = hero(&mut ui, "43", 1.0, HERO_BASELINE, TEXT, 1.2);
        let r3 = runs(&h3.drawables);
        assert_eq!(r3.len(), 3, "cell 0 holds, cell 1 rolls with two glyphs");
        assert!(r3[1].4 < 1.0 && r3[2].4 < 1.0);
        assert!(
            r3[1].1 < HERO_BASELINE && r3[2].1 > HERO_BASELINE,
            "old leaves up, new enters from below"
        );
    }

    #[test]
    fn units_sit_at_cap_height_or_below() {
        let mut ui = ScreenUi::new();
        let h = hero(&mut ui, "42", 1.0, HERO_BASELINE, TEXT, 0.0);
        let r = runs(&[unit_superscript(&h, "%")]);
        let upx = HERO_PX * UNIT_SCALE;
        assert_eq!(r[0].3, upx);
        assert!((r[0].0 - (h.x1 + UNIT_GAP)).abs() < 1e-4);
        let want =
            HERO_BASELINE - cap_height(FontId::Bold, HERO_PX) + cap_height(FontId::Bold, upx);
        assert!(
            (r[0].1 - want).abs() < 1e-3,
            "top-aligned at the hero cap height"
        );
        let b = runs(&[unit_below("Mb/s")]);
        assert_eq!(b[0].3, UNIT_BELOW_PX);
        assert!((b[0].1 - (HERO_BASELINE_UNIT_BELOW + UNIT_BELOW_DY)).abs() < 1e-4);
        let w = measure(FontId::Medium, UNIT_BELOW_PX, "Mb/s", 0.0);
        assert!((b[0].0 - (120.0 - w / 2.0)).abs() < 1e-3);
    }

    #[test]
    fn pill_fits_or_scrolls() {
        let mut ui = ScreenUi::new();
        let short = pill(&mut ui, "On mains", BLUE, PILL_CY, 0.0);
        let tw = measure(FontId::SemiBold, PILL_PX, "On mains", 0.0);
        match &short[0] {
            Drawable::RoundRect {
                x,
                y,
                w,
                h,
                r,
                color,
                alpha,
            } => {
                assert!((*w - (tw + PILL_PAD)).abs() < 1e-3);
                assert!((*x - (120.0 - w / 2.0)).abs() < 1e-3);
                assert_eq!(
                    (*y, *h, *r, *color, *alpha),
                    (
                        PILL_CY - PILL_H / 2.0,
                        PILL_H,
                        PILL_R,
                        BLUE,
                        PILL_FILL_ALPHA
                    )
                );
            }
            _ => panic!("pill background first"),
        }
        let r = runs(&short);
        assert_eq!(r.len(), 1);
        assert!(!r[0].5, "fitting text is not clipped");
        let long = "ubuntu-24.04.2-live-server-amd64.iso";
        let mut ui2 = ScreenUi::new();
        let wide = pill(&mut ui2, long, BLUE, PILL_CY, 0.0);
        assert!(
            matches!(&wide[0], Drawable::RoundRect { w, .. } if (*w - (PILL_MAX_TEXT_W + PILL_PAD)).abs() < 1e-3)
        );
        let r = runs(&wide);
        assert_eq!(r.len(), 2, "text and its repeat");
        assert!(r[0].5 && r[1].5);
        assert!(
            (r[0].0 - (120.0 - PILL_MAX_TEXT_W / 2.0)).abs() < 1e-3,
            "holding at the slot's left edge"
        );
        let later = pill(&mut ui2, long, BLUE, PILL_CY, 2.5);
        let r2 = runs(&later);
        assert!(
            (r2[0].0 - (r[0].0 - 30.0)).abs() < 1e-3,
            "30 px/s after the 1.5 s hold"
        );
    }

    #[test]
    fn ring_ends_sit_inside_the_ring_at_both_ends() {
        let r = runs(&ring_ends("11°", "21°"));
        assert_eq!(r.len(), 2);
        assert!(
            r[0].0 > 120.0 && r[1].0 < 120.0,
            "start is upper right, end upper left"
        );
        assert!(r[0].1 < 60.0 && r[1].1 < 60.0);
    }
    #[test]
    fn marquee_run_centres_short_text_and_scrolls_long_text() {
        let mut ui = ScreenUi::new();
        let short = marquee_run(
            &mut ui,
            ("line", 0),
            "Dune",
            120.0,
            160.0,
            88.0,
            FontId::SemiBold,
            10.0,
            TEXT,
            150.0,
            20.0,
            0.0,
        );
        let r = runs(&short);
        assert_eq!(r.len(), 1);
        let w = measure(FontId::SemiBold, 10.0, "Dune", 0.0);
        assert!((r[0].0 - (120.0 - w / 2.0)).abs() < 1e-3, "centred");
        assert!(!r[0].5, "no clip when it fits");
        let long = "The Lord of the Rings: The Return of the King";
        let at = |ui: &mut ScreenUi, now: f64| {
            marquee_run(
                ui,
                ("line", 1),
                long,
                120.0,
                160.0,
                88.0,
                FontId::SemiBold,
                10.0,
                TEXT,
                150.0,
                20.0,
                now,
            )
        };
        let wide = at(&mut ui, 0.0);
        let r = runs(&wide);
        assert_eq!(r.len(), 2, "text and its repeat");
        assert!(r[0].5 && r[1].5);
        assert!(
            (r[0].0 - (120.0 - 44.0)).abs() < 1e-3,
            "holding at the slot's left edge"
        );
        match &wide[0] {
            Drawable::Run { clip: Some(c), .. } => {
                assert_eq!((c.x, c.y, c.w, c.h), (76.0, 150.0, 88.0, 20.0));
            }
            _ => panic!("clipped run"),
        }
        let moved = runs(&at(&mut ui, 2.5));
        assert!(
            (moved[0].0 - (r[0].0 - 30.0)).abs() < 1e-3,
            "30 px/s after the 1.5 s hold"
        );
    }
    #[test]
    fn hero_fit_keeps_short_numbers_and_shrinks_wide_ones() {
        assert_eq!(hero_fit_scale("42", Some("%"), 1.0, HERO_BASELINE), 1.0);
        assert!(
            hero_fit_scale("100", None, 1.0, HERO_BASELINE) > 0.95,
            "three bare digits barely shrink"
        );
        let k = hero_fit_scale("100", Some("%"), 1.0, HERO_BASELINE);
        assert!(k < 1.0 && k > 0.7, "100 % shrinks a little: {k}");
        assert!(hero_fits("100", Some("%"), k, HERO_BASELINE));
        assert!(
            !hero_fits("100", Some("%"), k + 0.02, HERO_BASELINE),
            "largest that fits"
        );
        assert!(
            hero_fit_scale("16/16", None, 0.82, HERO_BASELINE) <= 0.82,
            "the role's scale is a ceiling"
        );
        assert!(hero_group_width("42", Some("%"), 1.0) > hero_width("42", 1.0));
    }

    #[test]
    fn hero_at_centres_on_the_given_x() {
        let mut ui = ScreenUi::new();
        let h = hero_at(&mut ui, "42", 1.0, 100.0, HERO_BASELINE, TEXT, 0.0);
        assert!(((h.x0 + h.x1) / 2.0 - 100.0).abs() < 1e-3);
    }
}

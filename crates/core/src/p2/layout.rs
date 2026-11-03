//! P2 geometry on the 240 × 240 screen, centre (120, 120).

use crate::theme::layout::{CX, CY};

pub const RING_R: f32 = 106.0;
pub const RING_W: f32 = 9.0;
/// The ring sweeps clockwise from 16° right of 12 o'clock round to 16° left
/// of it; the 32° gap at the top holds the role's icon.
pub const RING_START_DEG: f32 = 16.0;
pub const RING_SWEEP_DEG: f32 = 328.0;
/// Nothing is drawn past this radius: the panel's visible edge.
pub const EDGE_R: f32 = 116.0;

/// The role icon in the ring gap: Material Symbols Rounded at this size,
/// centred on the ring's radius at 12 o'clock.
pub const ICON_PX: f32 = 22.0;
pub const ICON_ALPHA: f32 = 0.70;
pub const ICON_CY: f32 = CY - RING_R;

pub const HERO_PX: f32 = 64.0;
pub const HERO_BASELINE: f32 = 100.0;
pub const HERO_BASELINE_UNIT_BELOW: f32 = 104.0;
pub const HERO_MAX_W: f32 = 180.0;
/// The hero group keeps this far inside the ring's inner edge.
pub const HERO_RING_MARGIN: f32 = 10.0;
/// A fitted hero size change glides over this long.
pub const HERO_FIT_GLIDE_SECS: f64 = 0.4;
/// Superscript unit size relative to the hero size.
pub const UNIT_SCALE: f32 = 0.36;
pub const UNIT_GAP: f32 = 2.0;
pub const UNIT_BELOW_PX: f32 = 10.5;
/// Baseline of a unit word below the hero, relative to the hero baseline.
pub const UNIT_BELOW_DY: f32 = 18.0;

pub const PILL_CY: f32 = 126.0;
pub const PILL_CY_UNIT_BELOW: f32 = 134.0;
pub const PILL_H: f32 = 20.0;
pub const PILL_R: f32 = 10.0;
pub const PILL_PAD: f32 = 20.0;
pub const PILL_PX: f32 = 10.5;
pub const PILL_MAX_TEXT_W: f32 = 132.0;
pub const PILL_FILL_ALPHA: f32 = 0.30;

pub const CARD_W: f32 = 108.0;
pub const CARD_H: f32 = 48.0;
pub const CARD_R: f32 = 14.0;
/// Card top relative to the pill centre.
pub const CARD_DY: f32 = 18.0;
pub const CHART_INSET_X: f32 = 10.0;
pub const CHART_INSET_TOP: f32 = 8.0;
pub const CHART_INSET_BOTTOM: f32 = 14.0;
pub const CAPTION_PX: f32 = 8.5;

/// Centre of the health role's live dot, between the card and the ring.
pub const LIVE_CY: f32 = 204.0;
/// Radius and size of the scale-end labels at the two ends of the ring.
pub const ENDS_R: f32 = 90.0;
pub const ENDS_PX: f32 = 8.5;

pub fn card_top(pill_cy: f32) -> f32 {
    pill_cy + CARD_DY
}

/// `(x, y, w, h)` of the chart area inside the card.
pub fn chart_rect(pill_cy: f32) -> (f32, f32, f32, f32) {
    let top = card_top(pill_cy);
    (
        CX - CARD_W / 2.0 + CHART_INSET_X,
        top + CHART_INSET_TOP,
        CARD_W - 2.0 * CHART_INSET_X,
        CARD_H - CHART_INSET_TOP - CHART_INSET_BOTTOM,
    )
}

/// The screen centre, for callers that only import this module.
pub const CENTRE: (f32, f32) = (CX, CY);

//! Ring kinds (fill, parts, discrete segments, bands with one lit, a route
//! of bands with milestone markers) drawn as round-capped arcs over a `TRACK`
//! arc.

use crate::p2::layout::{RING_R, RING_START_DEG, RING_SWEEP_DEG, RING_W};
use crate::scene::Drawable;
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{INK, TRACK};
use crate::theme::{Color, BLACK};

#[derive(Clone, Debug, PartialEq)]
pub enum RingKind {
    /// Just the track.
    Empty,
    /// A coloured arc from the start to `frac` of the sweep.
    Fill { frac: f32, color: Color },
    /// Consecutive coloured runs with a gap between them; the rest is track.
    Parts(Vec<(f32, Color)>),
    /// `n` equal segments, each its own colour (`TRACK` for off).
    Discrete(Vec<Color>),
    /// `n` equal bands; the one holding `frac` at full alpha, the rest dimmed.
    Bands { colors: Vec<Color>, frac: f32 },
    /// Equal bands lit up to `frac` (behind it full, the band holding it lit
    /// to `frac` over a dimmed band, ahead dimmed), with a round marker per
    /// milestone: reached filled, current outlined and bright, ahead faint.
    Route {
        frac: f32,
        bands: Vec<Color>,
        markers: Vec<RouteMarker>,
    },
}

/// A milestone marker on a `Route` ring, centred on band `band`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteMarker {
    pub band: usize,
    pub icon: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RingSpec {
    pub kind: RingKind,
    pub r: f32,
    pub width: f32,
    pub start_deg: f32,
    pub sweep_deg: f32,
}

pub const PARTS_GAP_K: f32 = 1.3;
pub const DISCRETE_GAP_K: f32 = 1.6;
pub const BANDS_GAP_K: f32 = 1.4;
pub const DIM_BAND_ALPHA: f32 = 0.3;
/// The route ring sits 2 px inside the usual radius so its markers fit.
pub const ROUTE_R: f32 = 104.0;
/// Dimmed route bands; 0.3 would put `TEXT3` under the panel's floor.
pub const ROUTE_DIM_ALPHA: f32 = 0.35;
pub const MARKER_R: f32 = 8.0;
pub const MARKER_CUT_R: f32 = 9.5;
pub const MARKER_STROKE: f32 = 1.6;
pub const MARKER_ICON_PX: f32 = 10.5;
/// Runs shorter than this fraction of the sweep are not drawn.
pub const MIN_FRAC: f32 = 0.002;
/// A fill never vanishes: at least a dot at the start.
pub const MIN_FILL: f32 = 0.004;

impl RingSpec {
    pub fn new(kind: RingKind) -> RingSpec {
        RingSpec {
            kind,
            r: RING_R,
            width: RING_W,
            start_deg: RING_START_DEG,
            sweep_deg: RING_SWEEP_DEG,
        }
    }
    pub fn empty() -> RingSpec {
        RingSpec::new(RingKind::Empty)
    }
    pub fn fill(frac: f32, color: Color) -> RingSpec {
        RingSpec::new(RingKind::Fill { frac, color })
    }
    pub fn parts(parts: Vec<(f32, Color)>) -> RingSpec {
        RingSpec::new(RingKind::Parts(parts))
    }
    pub fn discrete(colors: Vec<Color>) -> RingSpec {
        RingSpec::new(RingKind::Discrete(colors))
    }
    pub fn bands(colors: Vec<Color>, frac: f32) -> RingSpec {
        RingSpec::new(RingKind::Bands { colors, frac })
    }
    pub fn route(frac: f32, bands: Vec<Color>, markers: Vec<RouteMarker>) -> RingSpec {
        RingSpec {
            r: ROUTE_R,
            ..RingSpec::new(RingKind::Route {
                frac,
                bands,
                markers,
            })
        }
    }

    /// `k` stroke widths of gap, as a fraction of the sweep.
    pub fn gap_frac(&self, k: f32) -> f32 {
        self.width * k / self.r / self.sweep_deg.to_radians()
    }

    /// An arc from `f0` to `f1` of the sweep, or `None` when too short to show.
    pub fn arc(&self, f0: f32, f1: f32, color: Color, alpha: f32) -> Option<Drawable> {
        if f1 - f0 < MIN_FRAC {
            return None;
        }
        Some(Drawable::Arc {
            cx: CX,
            cy: CY,
            r: self.r,
            width: self.width,
            start_deg: self.start_deg + self.sweep_deg * f0,
            sweep_deg: self.sweep_deg * (f1 - f0),
            color,
            alpha,
        })
    }

    /// The point on the ring at `f` of the sweep.
    pub fn point(&self, f: f32) -> (f32, f32) {
        let a = (self.start_deg + self.sweep_deg * f).to_radians();
        (CX + self.r * a.sin(), CY - self.r * a.cos())
    }

    /// The full-length background track.
    pub fn track(&self) -> Vec<Drawable> {
        self.arc(0.0, 1.0, TRACK, 1.0).into_iter().collect()
    }

    /// Everything drawn over the track.
    pub fn overlay(&self) -> Vec<Drawable> {
        let mut out = Vec::new();
        match &self.kind {
            RingKind::Empty => {}
            RingKind::Fill { frac, color } => {
                out.extend(self.arc(0.0, frac.clamp(MIN_FILL, 1.0), *color, 1.0));
            }
            RingKind::Parts(parts) => {
                let sum: f32 = parts.iter().map(|(f, _)| f.max(0.0)).sum();
                let scale = if sum > 1.0 { 1.0 / sum } else { 1.0 };
                let gap = self.gap_frac(PARTS_GAP_K);
                let mut f = 0.0;
                for (p, c) in parts {
                    let p = p.max(0.0) * scale;
                    if p > 0.003 {
                        let lead = if f > 0.0 { gap / 2.0 } else { 0.0 };
                        out.extend(self.arc(f + lead, f + p - gap / 2.0, *c, 1.0));
                    }
                    f += p;
                }
            }
            RingKind::Discrete(colors) => {
                let n = colors.len().max(1) as f32;
                let gap = self.gap_frac(DISCRETE_GAP_K);
                for (i, c) in colors.iter().enumerate() {
                    let i = i as f32;
                    out.extend(self.arc(i / n + gap / 2.0, (i + 1.0) / n - gap / 2.0, *c, 1.0));
                }
            }
            RingKind::Bands { colors, frac } => {
                if colors.is_empty() {
                    return out;
                }
                let n = colors.len() as f32;
                let gap = self.gap_frac(BANDS_GAP_K);
                let fr = frac.clamp(0.0, 0.999);
                let cur = (fr * n).floor() as usize;
                for (i, c) in colors.iter().enumerate() {
                    let alpha = if i == cur { 1.0 } else { DIM_BAND_ALPHA };
                    let i = i as f32;
                    out.extend(self.arc(i / n + gap / 2.0, (i + 1.0) / n - gap / 2.0, *c, alpha));
                }
            }
            RingKind::Route {
                frac,
                bands,
                markers,
            } => {
                if bands.is_empty() {
                    return out;
                }
                let n = bands.len() as f32;
                let gap = self.gap_frac(BANDS_GAP_K);
                let fr = frac.clamp(0.0, 1.0);
                let lit_of = |i: usize| ((fr * n) - i as f32).clamp(0.0, 1.0);
                for (i, c) in bands.iter().enumerate() {
                    let (a, b) = (i as f32 / n + gap / 2.0, (i as f32 + 1.0) / n - gap / 2.0);
                    let lit = lit_of(i);
                    if lit >= 1.0 {
                        out.extend(self.arc(a, b, *c, 1.0));
                    } else {
                        out.extend(self.arc(a, b, *c, ROUTE_DIM_ALPHA));
                        if lit > 0.0 {
                            out.extend(self.arc(a, a + (b - a) * lit, *c, 1.0));
                        }
                    }
                }
                for m in markers {
                    let Some(c) = bands.get(m.band).copied() else {
                        continue;
                    };
                    let (cx, cy) = self.point((m.band as f32 + 0.5) / n);
                    let lit = lit_of(m.band);
                    let (fill, stroke_a, icon_c, icon_a) = if lit >= 1.0 {
                        (Some(c), 1.0, INK, 0.9)
                    } else if lit > 0.0 {
                        (None, 1.0, c, 1.0)
                    } else {
                        (None, 0.45, c, 0.55)
                    };
                    out.push(Drawable::Ellipse {
                        cx,
                        cy,
                        rx: MARKER_CUT_R,
                        ry: MARKER_CUT_R,
                        color: BLACK,
                        alpha: 1.0,
                    });
                    if let Some(f) = fill {
                        out.push(Drawable::Ellipse {
                            cx,
                            cy,
                            rx: MARKER_R,
                            ry: MARKER_R,
                            color: f,
                            alpha: 1.0,
                        });
                    }
                    out.push(Drawable::Arc {
                        cx,
                        cy,
                        r: MARKER_R,
                        width: MARKER_STROKE,
                        start_deg: 0.0,
                        sweep_deg: 360.0,
                        color: c,
                        alpha: stroke_a,
                    });
                    out.push(Drawable::Icon {
                        name: m.icon,
                        cx,
                        cy,
                        size: MARKER_ICON_PX,
                        color: icon_c,
                        alpha: icon_a,
                        scale: 1.0,
                        dy: 0.0,
                    });
                }
            }
        }
        out
    }

    pub fn drawables(&self) -> Vec<Drawable> {
        let mut out = self.track();
        out.extend(self.overlay());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::layout::{EDGE_R, RING_START_DEG, RING_SWEEP_DEG};
    use crate::theme::p2::{BLUE, CRIT, OK, TRACK, WARN};

    fn arcs(spec: &RingSpec) -> Vec<(f32, f32, Color, f32)> {
        spec.drawables()
            .into_iter()
            .filter_map(|d| match d {
                Drawable::Arc {
                    start_deg,
                    sweep_deg,
                    color,
                    alpha,
                    r,
                    width,
                    ..
                } => {
                    assert!(r + width / 2.0 <= EDGE_R, "caps stay inside the edge");
                    Some((start_deg, sweep_deg, color, alpha))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fill_is_track_plus_one_arc() {
        let a = arcs(&RingSpec::fill(0.5, BLUE));
        assert_eq!(a.len(), 2);
        assert_eq!(a[0], (RING_START_DEG, RING_SWEEP_DEG, TRACK, 1.0));
        assert_eq!(a[1], (RING_START_DEG, RING_SWEEP_DEG * 0.5, BLUE, 1.0));
        assert_eq!(
            arcs(&RingSpec::fill(7.0, BLUE))[1].1,
            RING_SWEEP_DEG,
            "clamps above one"
        );
        assert!(
            (arcs(&RingSpec::fill(-1.0, BLUE))[1].1 - RING_SWEEP_DEG * 0.004).abs() < 1e-4,
            "a dot at the start, never nothing"
        );
        assert_eq!(arcs(&RingSpec::empty()).len(), 1);
    }

    #[test]
    fn parts_are_consecutive_with_gaps_and_normalise() {
        let spec = RingSpec::parts(vec![(0.5, OK), (0.3, WARN), (0.2, CRIT)]);
        let a = arcs(&spec);
        assert_eq!(a.len(), 4);
        let gap = spec.gap_frac(1.3) * RING_SWEEP_DEG;
        assert!(
            (a[1].0 - RING_START_DEG).abs() < 1e-4,
            "first run starts at the start"
        );
        assert!((a[1].1 - (RING_SWEEP_DEG * 0.5 - gap / 2.0)).abs() < 1e-3);
        assert!((a[2].0 - (RING_START_DEG + RING_SWEEP_DEG * 0.5 + gap / 2.0)).abs() < 1e-3);
        assert!((a[2].1 - (RING_SWEEP_DEG * 0.3 - gap)).abs() < 1e-3);
        assert_eq!(a[3].2, CRIT);
        let over = arcs(&RingSpec::parts(vec![(1.0, OK), (1.0, WARN)]));
        assert!(
            (over[1].1 + over[2].1 + 1.5 * gap - RING_SWEEP_DEG).abs() < 1e-2,
            "sum over one is normalised"
        );
        assert_eq!(
            arcs(&RingSpec::parts(vec![(0.0, OK)])).len(),
            1,
            "empty parts draw nothing"
        );
    }

    #[test]
    fn discrete_segments_are_equal_and_coloured() {
        let colors = vec![OK, OK, CRIT, OK, OK, OK, OK];
        let spec = RingSpec::discrete(colors.clone());
        let a = arcs(&spec);
        assert_eq!(a.len(), 8);
        let gap = spec.gap_frac(1.6) * RING_SWEEP_DEG;
        for (i, seg) in a[1..].iter().enumerate() {
            assert!((seg.1 - (RING_SWEEP_DEG / 7.0 - gap)).abs() < 1e-3);
            assert_eq!(seg.2, colors[i]);
        }
        assert!(gap > 0.0 && gap < 10.0, "{gap}");
    }

    #[test]
    fn bands_light_the_current_band_and_dim_the_rest() {
        let colors = vec![OK, WARN, CRIT];
        let spec = RingSpec::bands(colors.clone(), 0.5);
        let a = arcs(&spec);
        // track, then the 3 bands; no marker
        assert_eq!(a.len(), 4);
        assert_eq!(a[1].3, DIM_BAND_ALPHA);
        assert_eq!((a[2].2, a[2].3), (WARN, 1.0), "band 1 holds 0.5");
        assert_eq!(a[3].3, DIM_BAND_ALPHA);
        assert!(
            !spec
                .drawables()
                .iter()
                .any(|d| matches!(d, Drawable::Ellipse { .. })),
            "no dot on a band ring"
        );
        let top = arcs(&RingSpec::bands(colors, 1.5));
        assert_eq!(top[3].3, 1.0, "clamps into the last band");
    }

    fn route(frac: f32) -> RingSpec {
        use crate::theme::p2::{GREEN, TEXT3};
        let mut bands = vec![TEXT3; 5];
        bands.extend([GREEN; 5]);
        RingSpec::route(
            frac,
            bands,
            vec![
                RouteMarker {
                    band: 0,
                    icon: "ms-construction",
                },
                RouteMarker {
                    band: 5,
                    icon: "ms-directions_run",
                },
            ],
        )
    }

    fn route_arcs(d: &[Drawable]) -> Vec<(f32, f32)> {
        d.iter()
            .filter_map(|x| match x {
                Drawable::Arc {
                    sweep_deg,
                    alpha,
                    r,
                    ..
                } if (*r - ROUTE_R).abs() < 0.01 => Some((*sweep_deg, *alpha)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn route_lights_bands_up_to_frac() {
        let d = route(0.25).overlay();
        let a = route_arcs(&d);
        // two full bands, the third dimmed with its first half lit, seven dimmed
        let lit = a.iter().filter(|(_, al)| *al == 1.0).count();
        let dim = a.iter().filter(|(_, al)| *al == ROUTE_DIM_ALPHA).count();
        assert_eq!((lit, dim), (3, 8));
        assert_eq!(route(0.25).r, ROUTE_R);
    }

    #[test]
    fn route_markers_show_reached_current_and_ahead() {
        use crate::theme::p2::{GREEN, INK};
        let icon = |d: &[Drawable], name: &str| {
            d.iter()
                .find_map(|x| match x {
                    Drawable::Icon {
                        name: n,
                        color,
                        alpha,
                        ..
                    } if *n == name => Some((*color, *alpha)),
                    _ => None,
                })
                .unwrap()
        };
        // band 0 reached, band 5 ahead
        let d = route(0.3).overlay();
        assert_eq!(icon(&d, "ms-construction"), (INK, 0.9));
        assert_eq!(icon(&d, "ms-directions_run"), (GREEN, 0.55));
        // band 5 current
        let d = route(0.55).overlay();
        assert_eq!(icon(&d, "ms-directions_run"), (GREEN, 1.0));
        // every marker cuts the ring with a black disc first
        let cuts = d
            .iter()
            .filter(|x| {
                matches!(x, Drawable::Ellipse { rx, color, .. }
                if *rx == MARKER_CUT_R && *color == crate::theme::BLACK)
            })
            .count();
        assert_eq!(cuts, 2);
    }
}

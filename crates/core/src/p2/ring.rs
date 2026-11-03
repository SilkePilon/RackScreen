//! Ring kinds (fill, parts, discrete segments, bands with a marker) drawn as
//! round-capped arcs over a `TRACK` arc.

use crate::p2::layout::{RING_R, RING_START_DEG, RING_SWEEP_DEG, RING_W};
use crate::scene::Drawable;
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{TEXT, TRACK};
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
    /// `n` equal bands, the one holding `frac` at full alpha, a marker at `frac`.
    Bands { colors: Vec<Color>, frac: f32 },
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
                let (mx, my) = self.point(fr);
                out.push(Drawable::Ellipse {
                    cx: mx,
                    cy: my,
                    rx: 0.95 * self.width,
                    ry: 0.95 * self.width,
                    color: BLACK,
                    alpha: 1.0,
                });
                out.push(Drawable::Ellipse {
                    cx: mx,
                    cy: my,
                    rx: 0.5 * self.width,
                    ry: 0.5 * self.width,
                    color: TEXT,
                    alpha: 1.0,
                });
                out.push(Drawable::Arc {
                    cx: mx,
                    cy: my,
                    r: 0.95 * self.width,
                    width: 2.5,
                    start_deg: 0.0,
                    sweep_deg: 360.0,
                    color: colors[cur],
                    alpha: 1.0,
                });
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
    use crate::p2::layout::{EDGE_R, RING_START_DEG, RING_SWEEP_DEG, RING_W};
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
        assert_eq!(a[1], (RING_START_DEG, 150.0, BLUE, 1.0));
        assert_eq!(
            arcs(&RingSpec::fill(7.0, BLUE))[1].1,
            300.0,
            "clamps above one"
        );
        assert!(
            (arcs(&RingSpec::fill(-1.0, BLUE))[1].1 - 300.0 * 0.004).abs() < 1e-4,
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
        assert!((a[1].1 - (150.0 - gap / 2.0)).abs() < 1e-3);
        assert!((a[2].0 - (RING_START_DEG + 150.0 + gap / 2.0)).abs() < 1e-3);
        assert!((a[2].1 - (90.0 - gap)).abs() < 1e-3);
        assert_eq!(a[3].2, CRIT);
        let over = arcs(&RingSpec::parts(vec![(1.0, OK), (1.0, WARN)]));
        assert!(
            (over[1].1 + over[2].1 + 1.5 * gap - 300.0).abs() < 1e-2,
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
            assert!((seg.1 - (300.0 / 7.0 - gap)).abs() < 1e-3);
            assert_eq!(seg.2, colors[i]);
        }
        assert!(gap > 0.0 && gap < 10.0, "{gap}");
    }

    #[test]
    fn bands_dim_all_but_the_current_and_place_a_marker() {
        let colors = vec![OK, WARN, CRIT];
        let spec = RingSpec::bands(colors.clone(), 0.5);
        let a = arcs(&spec);
        // track, 3 bands, then the marker ring (a full 360 arc)
        assert_eq!(a.len(), 5);
        assert_eq!(a[1].3, 0.3);
        assert_eq!(a[2].3, 1.0, "band 1 holds 0.5");
        assert_eq!(a[3].3, 0.3);
        assert_eq!((a[4].1, a[4].2), (360.0, WARN));
        let (mx, my) = spec.point(0.5);
        let ell: Vec<(f32, f32, f32)> = spec
            .drawables()
            .into_iter()
            .filter_map(|d| match d {
                Drawable::Ellipse { cx, cy, rx, .. } => Some((cx, cy, rx)),
                _ => None,
            })
            .collect();
        assert_eq!(ell.len(), 2);
        assert!((ell[0].0 - mx).abs() < 1e-4 && (ell[0].1 - my).abs() < 1e-4);
        assert!((ell[0].2 - 0.95 * RING_W).abs() < 1e-4 && (ell[1].2 - 0.5 * RING_W).abs() < 1e-4);
        assert!(((mx - 120.0).powi(2) + (my - 120.0).powi(2)).sqrt() + 0.95 * RING_W <= EDGE_R);
        let top = arcs(&RingSpec::bands(colors, 1.5));
        assert_eq!(top[3].3, 1.0, "clamps into the last band");
    }
}

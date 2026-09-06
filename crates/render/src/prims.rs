//! tiny-skia helpers: colours, paths for segments, badges, ripples, dots.

use std::collections::HashMap;

use rackscreen_core::theme::Color;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

pub fn skia_color(c: Color, alpha: f32) -> tiny_skia::Color {
    let a = (c.a as f32 / 255.0) * alpha.clamp(0.0, 1.0);
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, (a * 255.0).round() as u8)
}

pub fn paint(c: Color, alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(skia_color(c, alpha));
    p.anti_alias = true;
    p
}

pub fn fill(px: &mut Pixmap, path: &Path, c: Color, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    px.fill_path(
        path,
        &paint(c, alpha),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Stroke a path into a fillable outline.
pub fn outline(path: &Path, width: f32) -> Option<Path> {
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    path.stroke(&stroke, 1.0)
}

/// One ring segment: a radial tick centred on `radius`, at `angle_deg` clockwise from 12 o'clock.
pub fn segment_outline(
    cx: f32,
    cy: f32,
    radius: f32,
    angle_deg: f32,
    len: f32,
    width: f32,
) -> Path {
    let a = angle_deg.to_radians();
    let (s, c) = (a.sin(), a.cos());
    let r0 = radius - len / 2.0;
    let r1 = radius + len / 2.0;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r0 * s, cy - r0 * c);
    pb.line_to(cx + r1 * s, cy - r1 * c);
    let line = pb.finish().expect("segment path");
    outline(&line, width).expect("segment outline")
}

pub struct SegmentCache {
    map: HashMap<(u32, u32, u32, usize), Vec<Path>>,
}

impl SegmentCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn segments(&mut self, cx: f32, cy: f32, radius: f32, n: usize) -> &[Path] {
        use rackscreen_core::theme::layout::{SEG_LEN, SEG_W};
        let key = (
            (cx * 10.0) as u32,
            (cy * 10.0) as u32,
            (radius * 10.0) as u32,
            n,
        );
        self.map
            .entry(key)
            .or_insert_with(|| {
                (0..n)
                    .map(|i| {
                        segment_outline(cx, cy, radius, i as f32 * 360.0 / n as f32, SEG_LEN, SEG_W)
                    })
                    .collect()
            })
            .as_slice()
    }
}

impl Default for SegmentCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.5523 * r;
    let (x1, y1) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x1 - r, y);
    pb.cubic_to(x1 - r + k, y, x1, y + r - k, x1, y + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x + r, y1);
    pb.cubic_to(x + r - k, y1, x, y1 - r + k, x, y1 - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish().expect("rounded rect")
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Path {
    PathBuilder::from_circle(cx, cy, r.max(0.01)).expect("circle")
}

pub fn circle_stroke(cx: f32, cy: f32, r: f32, thickness: f32) -> Option<Path> {
    if r < 0.5 {
        return None;
    }
    outline(&circle(cx, cy, r), thickness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::{AMBER, WHITE};

    fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let i = (y as usize * p.width() as usize + x as usize) * 4;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    #[test]
    fn segment_zero_is_at_top() {
        let mut px = Pixmap::new(240, 240).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let seg = segment_outline(120.0, 120.0, 102.0, 0.0, 12.0, 4.0);
        fill(&mut px, &seg, AMBER, 1.0);
        let (r, g, b) = pixel(&px, 120, 18);
        assert!(r > 200 && g > 140 && b < 80, "got {r},{g},{b}");
        assert_eq!(pixel(&px, 120, 222), (0, 0, 0));
    }

    #[test]
    fn segment_cache_reuses_and_counts() {
        let mut c = SegmentCache::new();
        let n1 = c.segments(120.0, 120.0, 102.0, 60).len();
        let n2 = c.segments(120.0, 120.0, 102.0, 60).len();
        assert_eq!((n1, n2), (60, 60));
        assert_eq!(c.map.len(), 1);
        c.segments(120.0, 120.0, 86.0, 51);
        assert_eq!(c.map.len(), 2);
    }

    #[test]
    fn rounded_rect_fills_centre_not_corner() {
        let mut px = Pixmap::new(100, 100).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let rr = rounded_rect(10.0, 10.0, 64.0, 26.0, 7.0);
        fill(&mut px, &rr, WHITE, 1.0);
        assert_eq!(pixel(&px, 42, 23), (255, 255, 255));
        assert_eq!(pixel(&px, 10, 10), (0, 0, 0), "corner is rounded away");
    }

    #[test]
    fn ripple_is_hollow() {
        let mut px = Pixmap::new(100, 100).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        let ring = circle_stroke(50.0, 50.0, 30.0, 3.0).unwrap();
        fill(&mut px, &ring, WHITE, 1.0);
        assert_eq!(pixel(&px, 50, 50), (0, 0, 0));
        assert!(pixel(&px, 80, 50).0 > 200);
        assert!(circle_stroke(50.0, 50.0, 0.0, 3.0).is_none());
    }

    #[test]
    fn alpha_blends_toward_black() {
        let mut px = Pixmap::new(10, 10).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        fill(&mut px, &circle(5.0, 5.0, 4.0), WHITE, 0.5);
        let (r, ..) = pixel(&px, 5, 5);
        assert!((120..=136).contains(&r), "got {r}");
    }
}

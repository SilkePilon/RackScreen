//! tiny-skia helpers: colours, fills, strokes and the paths behind the P2 drawables.

use rackscreen_core::theme::Color;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

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

/// Points along an arc at most 2° apart; angles clockwise from 12 o'clock.
pub fn arc_points(cx: f32, cy: f32, r: f32, start_deg: f32, sweep_deg: f32) -> Vec<(f32, f32)> {
    let steps = ((sweep_deg.abs() / 2.0).ceil() as usize).max(1);
    (0..=steps)
        .map(|i| {
            let a = (start_deg + sweep_deg * i as f32 / steps as f32).to_radians();
            (cx + r * a.sin(), cy - r * a.cos())
        })
        .collect()
}

pub fn polyline_path(points: &[(f32, f32)]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    pb.finish()
}

pub fn polygon_path(points: &[(f32, f32)]) -> Option<Path> {
    if points.len() < 3 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for p in &points[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let rect = Rect::from_xywh(cx - rx, cy - ry, (2.0 * rx).max(0.01), (2.0 * ry).max(0.01))?;
    PathBuilder::from_oval(rect)
}

pub fn stroke_round(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

/// Stroke `path` with round caps and joins.
pub fn stroke(px: &mut Pixmap, path: &Path, width: f32, c: Color, alpha: f32) {
    if alpha <= 0.0 || width <= 0.0 {
        return;
    }
    px.stroke_path(
        path,
        &paint(c, alpha),
        &stroke_round(width),
        Transform::identity(),
        None,
    );
}

/// Stroke `path` as round-capped dashes `on` long, `off` apart.
pub fn stroke_dashed(
    px: &mut Pixmap,
    path: &Path,
    width: f32,
    on: f32,
    off: f32,
    c: Color,
    alpha: f32,
) {
    if alpha <= 0.0 || width <= 0.0 {
        return;
    }
    let mut st = stroke_round(width);
    st.dash = tiny_skia::StrokeDash::new(vec![on, off], 0.0);
    if st.dash.is_none() {
        return;
    }
    px.stroke_path(path, &paint(c, alpha), &st, Transform::identity(), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::WHITE;

    fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let i = (y as usize * p.width() as usize + x as usize) * 4;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
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
    fn alpha_blends_toward_black() {
        let mut px = Pixmap::new(10, 10).unwrap();
        px.fill(tiny_skia::Color::BLACK);
        fill(
            &mut px,
            &ellipse_path(5.0, 5.0, 4.0, 4.0).unwrap(),
            WHITE,
            0.5,
        );
        let (r, ..) = pixel(&px, 5, 5);
        assert!((120..=136).contains(&r), "got {r}");
    }
    #[test]
    fn arc_points_are_dense_and_clockwise() {
        let pts = arc_points(120.0, 120.0, 100.0, 0.0, 90.0);
        assert_eq!(pts.len(), 46, "90 degrees in 2 degree steps plus the start");
        assert!((pts[0].0 - 120.0).abs() < 1e-3 && (pts[0].1 - 20.0).abs() < 1e-3);
        let last = pts[45];
        assert!((last.0 - 220.0).abs() < 1e-3 && (last.1 - 120.0).abs() < 1e-3);
        assert!(polyline_path(&pts[..1]).is_none());
        assert!(polygon_path(&pts[..2]).is_none());
        assert!(ellipse_path(10.0, 10.0, 0.0, 0.0).is_some());
    }
}

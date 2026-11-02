//! The face role: two solid vector eyes with mood-shaped lids. No ring, title,
//! pill or card.

use crate::anim::Secs;
use crate::face::FaceFrame;
use crate::model::Model;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::TEXT;
use crate::theme::BLACK;

pub const EYE_W: f32 = 56.0;
pub const EYE_H: f32 = 68.0;
pub const EYE_CY: f32 = 118.0;
pub const EYE_DX: f32 = 48.0;
pub const EYE_RADIUS: f32 = 18.0;
pub const GAZE_X: f32 = 10.0;
pub const GAZE_Y: f32 = 8.0;
/// A full blink still leaves this much of the eye.
pub const MIN_OPEN: f32 = 0.04;

pub fn face_scene(model: &Model, now: Secs) -> Scene {
    render_face(&model.face_frame(now))
}

pub fn render_face(f: &FaceFrame) -> Scene {
    let e = &f.e;
    let open = (e.open * (1.0 - f.blink)).max(MIN_OPEN);
    let mut s = Scene::new();
    for side in [-1.0f32, 1.0] {
        let w = EYE_W * e.scale;
        let h = EYE_H * e.scale * open;
        let cx = CX + side * EYE_DX + f.gaze.0 * GAZE_X + f.wobble.0;
        let cy = EYE_CY + e.lift + f.gaze.1 * GAZE_Y + f.wobble.1;
        s.push(Drawable::RoundRect {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
            r: (h / 2.0).min(EYE_RADIUS),
            color: TEXT,
            alpha: 1.0,
        });
        if e.tilt.abs() > 0.01 {
            let k = 0.55 * w * e.tilt.abs();
            let top = cy - h / 2.0;
            let inner = cx - side * w / 2.0;
            let outer = cx + side * w / 2.0;
            let d_inner = if e.tilt > 0.0 { k } else { 0.0 };
            let d_outer = if e.tilt < 0.0 { k } else { 0.0 };
            s.push(Drawable::Polygon {
                points: vec![
                    (inner, top - 60.0),
                    (outer, top - 60.0),
                    (outer, top + d_outer),
                    (inner, top + d_inner),
                ],
                color: BLACK,
                alpha: 1.0,
            });
        }
        if e.lower > 0.01 {
            let rise = 0.75 * e.lower * EYE_H * e.scale;
            s.push(Drawable::Ellipse {
                cx,
                cy: cy + h / 2.0 + rise * 0.4,
                rx: 0.75 * w,
                ry: 1.4 * rise,
                color: BLACK,
                alpha: 1.0,
            });
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::FaceFrame;
    use crate::face_expr::Expr;
    use crate::mood::Mood;
    use crate::theme::p2::TEXT;

    fn frame(m: Mood) -> FaceFrame {
        FaceFrame {
            e: Expr::of(m),
            blink: 0.0,
            gaze: (0.0, 0.0),
            wobble: (0.0, 0.0),
            mood: m,
        }
    }

    fn eyes(s: &Scene) -> Vec<(f32, f32, f32, f32, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::RoundRect {
                    x,
                    y,
                    w,
                    h,
                    r,
                    color,
                    ..
                } if *color == TEXT => Some((*x, *y, *w, *h, *r)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn content_is_two_open_eyes_and_nothing_else() {
        let s = render_face(&frame(Mood::Content));
        let e = eyes(&s);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].2, e[0].3, e[0].4), (56.0, 68.0, 18.0));
        assert!(
            (e[0].0 + 28.0 - 72.0).abs() < 1e-3,
            "left eye centred at 120 - 48"
        );
        assert!((e[1].0 + 28.0 - 168.0).abs() < 1e-3);
        assert!((e[0].1 + 34.0 - 118.0).abs() < 1e-3);
        assert_eq!(s.items.len(), 3, "clear and two eyes");
        assert!(!s.items.iter().any(|d| matches!(
            d,
            Drawable::Arc { .. } | Drawable::CurvedText { .. } | Drawable::Icon { .. }
        )));
    }

    #[test]
    fn moods_add_lids_blinks_squash_and_gaze_shifts() {
        let sad = render_face(&frame(Mood::Sad));
        let lids: Vec<&Drawable> = sad
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Polygon { .. }))
            .collect();
        assert_eq!(lids.len(), 2, "an upper lid per eye");
        if let Drawable::Polygon { points, .. } = lids[0] {
            // left eye, side -1: inner corner (x = cx + w/2) at the top, outer corner lowered by k = 0.55 * 56 * 1.0
            assert!(
                (points[3].1 - points[2].1 + 30.8).abs() < 1e-3,
                "tilt -1 lowers the outer corner: {points:?}"
            );
        }
        let happy = render_face(&frame(Mood::Happy));
        let lower: Vec<&Drawable> = happy
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Ellipse { .. }))
            .collect();
        assert_eq!(lower.len(), 2, "a lower lid per eye");
        if let Drawable::Ellipse { rx, ry, .. } = lower[0] {
            let rise = 0.75 * 0.7 * 68.0;
            assert!((*rx - 42.0).abs() < 1e-3 && (*ry - 1.4 * rise).abs() < 1e-3);
        }
        let mut blink = frame(Mood::Content);
        blink.blink = 1.0;
        let shut = eyes(&render_face(&blink));
        assert!(
            (shut[0].3 - 68.0 * 0.04).abs() < 1e-3,
            "a full blink leaves a sliver"
        );
        let mut look = frame(Mood::Content);
        look.gaze = (1.0, -1.0);
        look.wobble = (2.0, 3.0);
        let moved = eyes(&render_face(&look));
        assert!(
            (moved[0].0 - (eyes(&render_face(&frame(Mood::Content)))[0].0 + 12.0)).abs() < 1e-3
        );
        assert!((moved[0].1 - (eyes(&render_face(&frame(Mood::Content)))[0].1 - 5.0)).abs() < 1e-3);
        let scared = eyes(&render_face(&frame(Mood::Scared)));
        assert!(
            (scared[0].2 - 56.0 * 0.85).abs() < 1e-3
                && (scared[0].3 - 68.0 * 0.85 * 1.3).abs() < 1e-3
        );
    }
}

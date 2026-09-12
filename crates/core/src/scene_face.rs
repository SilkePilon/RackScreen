//! The `face` role: two eyes on a 24×24 dot matrix, sampled from the shapes
//! the player describes and emitted as `Drawable::Dots` rows.

use crate::anim::Secs;
use crate::face_acts::{EyeOv, Overlay, Placed};
use crate::face_player::FaceFrame;
use crate::face_sprites::Sprite;
use crate::model::Model;
use crate::scene::{Drawable, Scene};
use crate::theme::{Color, AMBER};

pub const CELL: f32 = 10.0;
pub const N: usize = 24;
/// Cells whose centre is farther from the middle than this are never lit.
pub const VISIBLE_R: f32 = 116.0;
/// The `rim` post-effect lights cells beyond this radius.
pub const RIM_R: f32 = 104.0;
pub const LIT_R: f32 = 3.6;
pub const UNLIT_R: f32 = 2.3;
pub const LIT_MIN: f32 = 0.2;
/// Eye geometry in the 240 px space.
pub const EYE_W: f32 = 70.0;
pub const EYE_H: f32 = 84.0;
pub const EYE_CY: f32 = 120.0;
pub const EYE_DX: f32 = 50.0;
pub const EYE_RADIUS: f32 = 20.0;
/// Half width of the lid cuts.
pub const LID_W: f32 = 48.0;

pub fn face_scene(_model: &Model, _now: Secs) -> Scene {
    Scene::new()
}

/// Amber for brightness `b` in 0..1, before alpha.
pub fn amber(b: f32) -> Color {
    Color::rgb(255, (150.0 + 60.0 * b.clamp(0.0, 1.0)).round() as u8, 40)
}

/// One eye's shape in the face's local space.
struct Eye {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    /// Upper-lid slope, already mirrored for this side.
    slope: f32,
    /// Lower-lid rise in px (0 = none).
    rise: f32,
    sprite: Option<&'static Sprite>,
}

impl Eye {
    fn inside(&self, x: f32, y: f32) -> bool {
        if let Some(sp) = self.sprite {
            let gx = ((self.cx - 35.0) / CELL).round();
            let gy = ((self.cy - sp.h() as f32 * 5.0) / CELL).round();
            let i = ((x / CELL) - gx).floor();
            let j = ((y / CELL) - gy).floor();
            return i >= 0.0 && j >= 0.0 && sp.on(i as usize, j as usize);
        }
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        let r = EYE_RADIUS.min(hw).min(hh);
        let dx = (x - self.cx).abs() - (hw - r);
        let dy = (y - self.cy).abs() - (hh - r);
        let outside = if dx > 0.0 && dy > 0.0 {
            dx * dx + dy * dy > r * r
        } else {
            dx > r || dy > r
        };
        if outside {
            return false;
        }
        // upper lid: black above the slanted line
        let top = self.cy - hh + 2.0 + self.slope * (x - self.cx);
        if y < top {
            return false;
        }
        // lower lid: black below the parabola
        if self.rise > 0.0 {
            let u = (x - self.cx) / LID_W;
            let bottom = self.cy + hh - 1.3 * self.rise + 0.95 * self.rise * u * u;
            if y > bottom {
                return false;
            }
        }
        true
    }
}

fn eyes_of(f: &FaceFrame) -> [Eye; 2] {
    let e = &f.e;
    let build = |side: f32, ov: &EyeOv, sprite: Option<&'static Sprite>| {
        let open = (e.open * ov.open.unwrap_or(1.0) * (1.0 - f.blink)).max(0.04);
        let scale = e.scale * ov.scale.unwrap_or(1.0);
        Eye {
            cx: 120.0 - side * EYE_DX * e.sep,
            cy: EYE_CY,
            w: EYE_W * scale,
            h: EYE_H * scale * open,
            slope: (e.tilt + ov.tilt.unwrap_or(0.0)) * 0.55 * side,
            rise: if e.lower >= 0.02 {
                0.75 * e.lower * EYE_H
            } else {
                0.0
            },
            sprite,
        }
    };
    [
        build(1.0, &f.eyes[0], f.sprite.map(|s| s[0])),
        build(-1.0, &f.eyes[1], f.sprite.map(|s| s[1])),
    ]
}

/// The face centre in the local 240 px space: the eye pair is centred here
/// before the lift, and the head tilt turns about it.
const FACE_C: (f32, f32) = (120.0, 120.0);

/// The frame's local → screen map, with the per-frame trigonometry evaluated
/// once instead of once per sample.
struct Xform {
    tx: f32,
    ty: f32,
    /// `sin` and `cos` of `-rot`, i.e. of the *inverse* rotation.
    sin: f32,
    cos: f32,
}

/// Build the transform for `f` at `now`: the translation
/// `T = (off.x + 10·gaze.x, off.y + lift + bob + 8·gaze.y)` with
/// `bob = 1.5·sin(0.9·now)`, and the inverse of the head tilt `rot`.
fn xform(f: &FaceFrame, now: Secs) -> Xform {
    let bob = ((now * 0.9).sin() * 1.5) as f32;
    let (sin, cos) = (-f.rot).sin_cos();
    Xform {
        tx: f.off.0 + f.gaze.0 * 10.0,
        ty: f.off.1 + f.e.lift + bob + f.gaze.1 * 8.0,
        sin,
        cos,
    }
}

/// Screen point → face-local point.
///
/// The face is drawn in a local 240 px space and mapped to the screen by
/// `S = T + C + R(rot)·(L − C)`, where `C = (120, 120)` is the face centre and
/// `T` is the translation of [`xform`]: the tilt is of the whole translated
/// face, so the centre moves with it. This is the inverse,
/// `L = C + R(−rot)·(S − T − C)`.
fn to_local(xf: &Xform, x: f32, y: f32) -> (f32, f32) {
    let (cx, cy) = FACE_C;
    let (dx, dy) = (x - xf.tx - cx, y - xf.ty - cy);
    (
        cx + dx * xf.cos - dy * xf.sin,
        cy + dx * xf.sin + dy * xf.cos,
    )
}

fn sprite_hit(p: &Placed, x: f32, y: f32) -> bool {
    let i = (x / CELL - p.x).floor();
    let j = (y / CELL - p.y).floor();
    i >= 0.0 && j >= 0.0 && p.sprite.on(i as usize, j as usize)
}

fn overlay(f: &FaceFrame, i: usize, j: usize) -> f32 {
    match f.post.overlay {
        Overlay::None => 0.0,
        Overlay::Confetti(t) => {
            let a = ((t * 9.0).floor() as i64 + i as i64 * 3) % 6 == 0;
            let b = (j as i64 + ((i * 7 + 3) % 11) as i64 + (t * 7.0).floor() as i64) % 4 == 0;
            if a && b {
                0.9
            } else {
                0.0
            }
        }
        Overlay::FogBand(t) => {
            let band = 6.0 + (t * 2.0) % 12.0;
            if (j as f32 - band).abs() < 1.5 && (i + j).is_multiple_of(2) {
                0.35
            } else {
                0.0
            }
        }
    }
}

/// Brightness 0..1 of cell `(i, j)` and the tint of the sprite covering it,
/// before the flicker multiplier.
pub fn cell_brightness(f: &FaceFrame, now: Secs, i: usize, j: usize) -> (f32, Option<Color>) {
    cell_brightness_with(f, &xform(f, now), i, j)
}

fn cell_brightness_with(f: &FaceFrame, xf: &Xform, i: usize, j: usize) -> (f32, Option<Color>) {
    let (cx, cy) = (i as f32 * CELL + 5.0, j as f32 * CELL + 5.0);
    if ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() > VISIBLE_R {
        return (0.0, None);
    }
    let eyes = eyes_of(f);
    let mut sum = 0.0;
    let mut tint = None;
    for dy in [-3.0, 0.0, 3.0] {
        for dx in [-3.0, 0.0, 3.0] {
            let (sx, sy) = (cx + dx, cy + dy);
            let (lx, ly) = to_local(xf, sx, sy);
            let mut v: f32 = if eyes.iter().any(|e| e.inside(lx, ly)) {
                1.0
            } else {
                0.0
            };
            for p in &f.placed {
                if p.alpha > 0.0 && sprite_hit(p, sx, sy) {
                    v = v.max(p.alpha);
                    if p.tint.is_some() {
                        tint = p.tint;
                    }
                }
            }
            sum += v;
        }
    }
    let mut b: f32 = sum / 9.0;
    if f.post.rim > 0.0 && ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() > RIM_R {
        b = b.max(f.post.rim);
    }
    b = b.max(overlay(f, i, j));
    (b, tint)
}

/// The whole matrix: a clear, then for each row a `Dots` of lit cells
/// (radius 3.6) and one of unlit cells (radius 2.3), the other cells
/// transparent in each.
pub fn render_frame(f: &FaceFrame, now: Secs) -> Scene {
    let mut s = Scene::new();
    let xf = xform(f, now);
    let clear = Color { a: 0, ..AMBER };
    for j in 0..N {
        let mut lit = Vec::with_capacity(N);
        let mut unlit = Vec::with_capacity(N);
        for i in 0..N {
            let (cx, cy) = (i as f32 * CELL + 5.0, j as f32 * CELL + 5.0);
            let visible = ((cx - 120.0).powi(2) + (cy - 120.0).powi(2)).sqrt() <= VISIBLE_R;
            let (b, tint) = cell_brightness_with(f, &xf, i, j);
            if !visible {
                lit.push(clear);
                unlit.push(clear);
            } else if b > LIT_MIN {
                let a = ((0.45 + 0.55 * b) * f.post.flicker).clamp(0.0, 1.0);
                lit.push(tint.unwrap_or_else(|| amber(b)).with_alpha(a));
                unlit.push(clear);
            } else {
                lit.push(clear);
                unlit.push(AMBER.with_alpha(0.08));
            }
        }
        let cy = j as f32 * CELL + 5.0;
        s.push(Drawable::Dots {
            cx: 120.0,
            cy,
            spacing: CELL,
            r: LIT_R,
            colors: lit,
        });
        s.push(Drawable::Dots {
            cx: 120.0,
            cy,
            spacing: CELL,
            r: UNLIT_R,
            colors: unlit,
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face_acts::{Post, SLOT_X, SLOT_Y};
    use crate::face_expr::Expr;
    use crate::face_sprites::{BOX, T_K8S, XEYE};

    fn frame(e: Expr) -> FaceFrame {
        FaceFrame {
            e,
            blink: 0.0,
            off: (0.0, 0.0),
            rot: 0.0,
            gaze: (0.0, 0.0),
            eyes: [EyeOv::default(); 2],
            sprite: None,
            placed: Vec::new(),
            post: Post::default(),
        }
    }

    fn dots(s: &Scene) -> Vec<(f32, f32, &Vec<Color>)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Dots { cy, r, colors, .. } => Some((*cy, *r, colors)),
                _ => None,
            })
            .collect()
    }

    fn lit_cells(f: &FaceFrame) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for j in 0..N {
            for i in 0..N {
                if cell_brightness(f, 0.0, i, j).0 > LIT_MIN {
                    v.push((i, j));
                }
            }
        }
        v
    }

    #[test]
    fn scene_is_a_clear_and_forty_eight_rows_of_twenty_four() {
        let s = render_frame(&frame(Expr::CONTENT), 0.0);
        assert_eq!(s.items.len(), 49);
        let d = dots(&s);
        assert_eq!(d.len(), 48);
        for (k, (cy, r, colors)) in d.iter().enumerate() {
            assert_eq!(colors.len(), 24);
            assert_eq!(*cy, (k / 2) as f32 * CELL + 5.0);
            assert_eq!(*r, if k % 2 == 0 { LIT_R } else { UNLIT_R });
        }
        if let Drawable::Dots { cx, spacing, .. } = &s.items[1] {
            assert_eq!((*cx, *spacing), (120.0, CELL));
        } else {
            panic!("second item is not Dots");
        }
        // corners are outside the panel: transparent in both rows
        assert_eq!(d[0].2[0].a, 0);
        assert_eq!(d[1].2[0].a, 0);
        assert_eq!(d[46].2[23].a, 0);
    }

    #[test]
    fn content_eyes_are_two_amber_blobs_with_a_dark_gap() {
        let f = frame(Expr::CONTENT);
        let lit = lit_cells(&f);
        assert!(lit.len() > 40 && lit.len() < 200, "{} lit", lit.len());
        let (b, tint) = cell_brightness(&f, 0.0, 7, 12);
        assert!(b > 0.9 && tint.is_none(), "left eye centre lit amber");
        let (b, _) = cell_brightness(&f, 0.0, 16, 12);
        assert!(b > 0.9, "right eye centre lit");
        let (b, _) = cell_brightness(&f, 0.0, 12, 12);
        assert!(b < 0.1, "between the eyes is dark");
        let c = amber(1.0);
        assert_eq!((c.r, c.g, c.b), (255, 210, 40));
        assert_eq!(amber(0.0).g, 150);
        // the lit row carries colour, the unlit row is transparent there
        let s = render_frame(&f, 0.0);
        let d = dots(&s);
        assert!(d[24].2[7].a > 200 && d[25].2[7].a == 0);
        // and vice versa in the gap
        assert_eq!(d[24].2[12].a, 0);
        assert!(
            (d[25].2[12].a as i32 - 20).abs() <= 2,
            "unlit at 8 %: {}",
            d[25].2[12].a
        );
    }

    #[test]
    fn every_mood_lights_a_sane_number_of_cells() {
        for m in [
            Expr::CONTENT,
            Expr::HAPPY,
            Expr::EXCITED,
            Expr::WORRIED,
            Expr::SAD,
            Expr::ANGRY,
            Expr::HOT,
            Expr::SCARED,
            Expr::SLEEPY,
            Expr::BORED,
        ] {
            let n = lit_cells(&frame(m)).len();
            assert!((20..=200).contains(&n), "{m:?}: {n}");
        }
    }

    #[test]
    fn sad_sits_lower_and_happy_is_two_arcs() {
        let top = |e: Expr| lit_cells(&frame(e)).iter().map(|c| c.1).min().unwrap();
        assert!(top(Expr::SAD) > top(Expr::CONTENT));
        assert!(
            top(Expr::HAPPY) <= top(Expr::CONTENT),
            "happy lifts (by less than a row)"
        );
        // happy: the lower half of each eye is cut away
        let (b, _) = cell_brightness(&frame(Expr::HAPPY), 0.0, 7, 14);
        assert!(b < 0.3, "below the arc is dark: {b}");
    }

    #[test]
    fn a_blink_leaves_at_most_two_rows_per_eye() {
        let mut f = frame(Expr::CONTENT);
        f.blink = 1.0;
        let rows: std::collections::HashSet<usize> = lit_cells(&f)
            .iter()
            .filter(|c| c.0 < 12)
            .map(|c| c.1)
            .collect();
        assert!(rows.len() <= 2, "{rows:?}");
    }

    #[test]
    fn tilt_and_per_eye_override_change_the_shape() {
        let angry = lit_cells(&frame(Expr::ANGRY));
        // inner corners down: the topmost lit cell of the left eye is on its outer side
        let top_row = angry
            .iter()
            .filter(|c| c.0 < 12)
            .map(|c| c.1)
            .min()
            .unwrap();
        let cols: Vec<usize> = angry
            .iter()
            .filter(|c| c.0 < 12 && c.1 == top_row)
            .map(|c| c.0)
            .collect();
        assert!(
            cols.iter().all(|c| *c <= 7),
            "angry left eye top is outer: {cols:?}"
        );
        let mut f = frame(Expr::CONTENT);
        f.eyes[0].open = Some(0.06);
        let left: Vec<_> = lit_cells(&f).into_iter().filter(|c| c.0 < 12).collect();
        let right: Vec<_> = lit_cells(&f).into_iter().filter(|c| c.0 >= 12).collect();
        assert!(left.len() * 3 < right.len(), "winking left eye is small");
    }

    #[test]
    fn sprites_replace_eyes_and_icons_carry_their_tint() {
        let mut f = frame(Expr::CONTENT);
        f.sprite = Some([&XEYE, &XEYE]);
        let (b, _) = cell_brightness(&f, 0.0, 7, 12);
        assert!(b > 0.9, "X centre");
        let (b, _) = cell_brightness(&f, 0.0, 6, 9);
        assert!(b < 0.1, "X corner gap");
        let mut f = frame(Expr::CONTENT);
        f.placed.push(Placed {
            sprite: &BOX,
            x: SLOT_X,
            y: SLOT_Y,
            tint: Some(T_K8S),
            alpha: 1.0,
        });
        let (b, tint) = cell_brightness(&f, 0.0, 11, 17);
        assert!(b > 0.9 && tint == Some(T_K8S), "box top row is blue");
        let s = render_frame(&f, 0.0);
        let d = dots(&s);
        let c = d[34].2[11];
        assert_eq!((c.r, c.g, c.b), (T_K8S.r, T_K8S.g, T_K8S.b));
        // half alpha sprite: dim but lit
        f.placed[0].alpha = 0.5;
        let (b, _) = cell_brightness(&f, 0.0, 11, 17);
        assert!((b - 0.5).abs() < 0.05);
    }

    #[test]
    fn post_effects_apply() {
        let mut f = frame(Expr::CONTENT);
        f.post.rim = 0.5;
        let (b, _) = cell_brightness(&f, 0.0, 12, 1);
        assert!((b - 0.5).abs() < 1e-6, "rim lights the top edge");
        let (b, _) = cell_brightness(&f, 0.0, 12, 12);
        assert!(b < 0.1, "not the middle");
        f.post.rim = 0.0;
        f.post.flicker = 0.08;
        let s = render_frame(&f, 0.0);
        let c = dots(&s)[24].2[7];
        assert!(c.a < 30, "flicker dims the eye: {}", c.a);
        f.post.flicker = 3.0;
        let s = render_frame(&f, 0.0);
        assert_eq!(dots(&s)[24].2[7].a, 255, "clamped");
        f.post.flicker = 1.0;
        f.post.overlay = Overlay::FogBand(0.0);
        let band: Vec<usize> = (0..N)
            .filter(|i| cell_brightness(&f, 0.0, *i, 6).0 > 0.3 && (i + 6) % 2 == 0)
            .collect();
        assert!(band.len() >= 8, "fog band on row 6: {band:?}");
        f.post.overlay = Overlay::Confetti(0.0);
        let n = lit_cells(&f).len();
        assert!(
            n > lit_cells(&frame(Expr::CONTENT)).len(),
            "confetti adds cells"
        );
    }

    #[test]
    fn transforms_move_the_eyes() {
        let mut f = frame(Expr::CONTENT);
        f.off = (0.0, -30.0);
        let top = lit_cells(&f).iter().map(|c| c.1).min().unwrap();
        assert!(
            top < lit_cells(&frame(Expr::CONTENT))
                .iter()
                .map(|c| c.1)
                .min()
                .unwrap()
        );
        let mut f = frame(Expr::CONTENT);
        f.gaze = (1.0, 0.0);
        let left = lit_cells(&f).iter().map(|c| c.0).min().unwrap();
        assert!(
            left > lit_cells(&frame(Expr::CONTENT))
                .iter()
                .map(|c| c.0)
                .min()
                .unwrap()
        );
        let mut f = frame(Expr::CONTENT);
        f.rot = 0.3;
        let cells = lit_cells(&f);
        let l = cells
            .iter()
            .filter(|c| c.0 < 12)
            .map(|c| c.1)
            .min()
            .unwrap();
        let r = cells
            .iter()
            .filter(|c| c.0 >= 12)
            .map(|c| c.1)
            .min()
            .unwrap();
        assert_ne!(l, r, "tilted: the eyes are at different heights");
        assert_eq!(render_frame(&f, 1.0), render_frame(&f, 1.0));
    }

    #[test]
    fn rotation_is_about_the_moved_face_centre() {
        // with the face raised 30 px, a head tilt must not move the eye pair's centroid
        let centroid = |f: &FaceFrame| {
            let cells = lit_cells(f);
            let n = cells.len() as f32;
            (
                cells.iter().map(|c| c.0 as f32).sum::<f32>() / n,
                cells.iter().map(|c| c.1 as f32).sum::<f32>() / n,
            )
        };
        let mut plain = frame(Expr::CONTENT);
        plain.off = (0.0, -30.0);
        let mut tilted = plain.clone();
        tilted.rot = 0.4;
        let (px, py) = centroid(&plain);
        let (tx, ty) = centroid(&tilted);
        assert!(
            (px - tx).abs() < 0.6 && (py - ty).abs() < 0.6,
            "centroid moved: {:?} -> {:?}",
            (px, py),
            (tx, ty)
        );
        assert!(
            (py - 9.0).abs() < 1.0,
            "raised face sits around row 9: {py}"
        );
        // and the bob is part of the translation, not the rotation centre
        let mut bobbed = tilted.clone();
        bobbed.off = (0.0, 0.0);
        let (_, by) = centroid(&bobbed);
        assert!(
            (by - 12.0).abs() < 1.0,
            "unraised tilted face sits around row 12: {by}"
        );
    }
}

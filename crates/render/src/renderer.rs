//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::scene::{Drawable, Scene};
use rackscreen_core::theme::layout::{CX, CY};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::frame::new_pixmap;
use crate::icons::IconCache;
use crate::prims::{
    arc_points, ellipse_path, fill, polygon_path, polyline_path, rounded_rect, skia_color, stroke,
};
use crate::text::TextRenderer;

/// Below this the scene is drawn through the scratch pixmap and scaled; at 1.0
/// the zoom is a no-op and skipped.
const FULL: f32 = 0.999;

pub struct Renderer {
    painter: Painter,
    /// Full-size scene rendered here before being scaled into the target.
    scratch: Pixmap,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            painter: Painter {
                icons: IconCache::new()?,
                text: TextRenderer::new()?,
            },
            scratch: new_pixmap(),
        })
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        let zoom = scene.zoom.clamp(0.0, 1.0);
        if zoom >= FULL {
            for d in &scene.items {
                self.painter.draw_item(px, d);
            }
            return;
        }
        px.fill(tiny_skia::Color::BLACK);
        if zoom <= 0.0 {
            return;
        }
        for d in &scene.items {
            self.painter.draw_item(&mut self.scratch, d);
        }
        let ts = Transform::from_translate(CX, CY)
            .pre_scale(zoom, zoom)
            .pre_translate(-CX, -CY);
        px.draw_pixmap(
            0,
            0,
            self.scratch.as_ref(),
            &PixmapPaint::default(),
            ts,
            None,
        );
    }
}

/// The caches a single drawable needs; separate from `Renderer` so the scratch
/// pixmap can be borrowed alongside them.
struct Painter {
    icons: IconCache,
    text: TextRenderer,
}

impl Painter {
    /// Draw one item.
    fn draw_item(&mut self, px: &mut Pixmap, d: &Drawable) {
        match d {
            Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
            Drawable::Icon {
                name,
                cx,
                cy,
                size,
                color,
                alpha,
                scale,
                dy,
            } => {
                self.icons
                    .draw(px, name, *cx, *cy, *size, *color, *alpha, *scale, *dy);
            }
            Drawable::Arc {
                cx,
                cy,
                r,
                width,
                start_deg,
                sweep_deg,
                color,
                alpha,
            } => {
                if let Some(p) = polyline_path(&arc_points(*cx, *cy, *r, *start_deg, *sweep_deg)) {
                    stroke(px, &p, *width, *color, *alpha);
                }
            }
            Drawable::RoundRect {
                x,
                y,
                w,
                h,
                r,
                color,
                alpha,
            } => {
                if *w > 0.0 && *h > 0.0 {
                    fill(px, &rounded_rect(*x, *y, *w, *h, *r), *color, *alpha);
                }
            }
            Drawable::Polygon {
                points,
                color,
                alpha,
            } => {
                if let Some(p) = polygon_path(points) {
                    fill(px, &p, *color, *alpha);
                }
            }
            Drawable::Ellipse {
                cx,
                cy,
                rx,
                ry,
                color,
                alpha,
            } => {
                if let Some(p) = ellipse_path(*cx, *cy, *rx, *ry) {
                    fill(px, &p, *color, *alpha);
                }
            }
            Drawable::Polyline {
                points,
                width,
                color,
                alpha,
            } => {
                if let Some(p) = polyline_path(points) {
                    stroke(px, &p, *width, *color, *alpha);
                }
            }
            Drawable::Run {
                x,
                baseline,
                text,
                font,
                px: size,
                letter_spacing,
                color,
                alpha,
                clip,
            } => {
                self.text.draw_run(
                    px,
                    text,
                    *font,
                    *size,
                    *x,
                    *baseline,
                    *letter_spacing,
                    *color,
                    *alpha,
                    *clip,
                );
            }
            Drawable::CurvedText {
                text,
                radius,
                centre_deg,
                px: size,
                font,
                letter_spacing,
                color,
                alpha,
            } => {
                self.text.draw_curved(
                    px,
                    text,
                    *font,
                    *size,
                    CX,
                    CY,
                    *radius,
                    *centre_deg,
                    *letter_spacing,
                    *color,
                    *alpha,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::event::{Event, LinkTarget};
    use rackscreen_core::model::{Model, Thresholds};
    use rackscreen_core::p2::ui::ScreenUi;
    use rackscreen_core::theme::Role;

    fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * p.width() + x) * 4) as usize;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    fn cpu_model(pct: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        for target in [LinkTarget::K8sApi, LinkTarget::Prometheus] {
            m.apply(Event::Link { target, up: true }, 0.0);
        }
        m.apply(
            Event::Metrics {
                cpu_pct: pct,
                mem_pct: 50.0,
                mem_used_gb: 1.0,
                mem_total_gb: 8.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m
    }

    #[test]
    fn zoom_scales_the_scene_about_the_centre() {
        let m = cpu_model(42.0);
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = m.scene_for_role(Role::Cpu, 5.0, &mut ScreenUi::new());
        r.render(&s, &mut px);
        let bluish = |(r, g, b): (u8, u8, u8)| b > 200 && r < 160 && g > 120;
        assert!(
            bluish(pixel(&px, 226, 120)),
            "ring fill at 3 o'clock at full scale"
        );
        s.zoom = 0.5;
        r.render(&s, &mut px);
        assert_eq!(
            pixel(&px, 226, 120),
            (0, 0, 0),
            "the full-scale ring is gone"
        );
        assert!(
            bluish(pixel(&px, 173, 120)),
            "ring at half scale, got {:?}",
            pixel(&px, 173, 120)
        );
        s.zoom = 0.0;
        r.render(&s, &mut px);
        let lit = px
            .data()
            .chunks_exact(4)
            .filter(|p| p[..3] != [0, 0, 0])
            .count();
        assert_eq!(lit, 0, "zoom 0 draws nothing but black");
    }

    use rackscreen_core::p2::font::FontId;
    use rackscreen_core::theme::WHITE;

    fn render_one(d: Drawable) -> Pixmap {
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = Scene::new();
        s.push(d);
        r.render(&s, &mut px);
        px
    }

    fn lit(p: &Pixmap, x: u32, y: u32) -> bool {
        pixel(p, x, y).0 > 60
    }

    #[test]
    fn arc_runs_clockwise_from_twelve_with_round_caps() {
        let px = render_one(Drawable::Arc {
            cx: CX,
            cy: CY,
            r: 100.0,
            width: 8.0,
            start_deg: 0.0,
            sweep_deg: 90.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&px, 120, 20), "start at 12 o'clock");
        assert!(lit(&px, 191, 49), "half way, 45 degrees");
        assert!(lit(&px, 220, 120), "end at 3 o'clock");
        assert!(!lit(&px, 120, 220), "nothing at 6 o'clock");
        assert!(!lit(&px, 20, 120), "nothing at 9 o'clock");
        assert!(
            lit(&px, 117, 20),
            "round cap reaches a little past the start"
        );
    }

    #[test]
    fn filled_shapes_cover_their_interior_only() {
        let rr = render_one(Drawable::RoundRect {
            x: 20.0,
            y: 20.0,
            w: 60.0,
            h: 30.0,
            r: 10.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&rr, 50, 35));
        assert!(!lit(&rr, 20, 20), "corner rounded away");
        let poly = render_one(Drawable::Polygon {
            points: vec![(120.0, 20.0), (220.0, 120.0), (20.0, 120.0)],
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&poly, 120, 100));
        assert!(!lit(&poly, 30, 40));
        let ell = render_one(Drawable::Ellipse {
            cx: 120.0,
            cy: 120.0,
            rx: 40.0,
            ry: 10.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&ell, 150, 120));
        assert!(!lit(&ell, 120, 100), "ten tall, not forty");
        let line = render_one(Drawable::Polyline {
            points: vec![(20.0, 200.0), (120.0, 100.0), (220.0, 200.0)],
            width: 3.0,
            color: WHITE,
            alpha: 1.0,
        });
        assert!(lit(&line, 70, 150));
        assert!(!lit(&line, 120, 200));
    }

    #[test]
    fn run_and_curved_text_land_where_asked() {
        let run = render_one(Drawable::Run {
            x: 100.0,
            baseline: 130.0,
            text: "42".into(),
            font: FontId::Bold,
            px: 30.0,
            letter_spacing: 0.0,
            color: WHITE,
            alpha: 1.0,
            clip: None,
        });
        let n = (100..140)
            .flat_map(|x| (105..131).map(move |y| (x, y)))
            .filter(|&(x, y)| lit(&run, x, y))
            .count();
        assert!(n > 100, "glyph pixels inside the run box: {n}");
        assert!(
            !(0..240).any(|x| lit(&run, x, 140)),
            "nothing below the baseline"
        );
        let curved = render_one(Drawable::CurvedText {
            text: "MEMORY".into(),
            radius: 106.0,
            centre_deg: 0.0,
            px: 10.0,
            font: FontId::Bold,
            letter_spacing: 1.5,
            color: WHITE,
            alpha: 1.0,
        });
        let top = (0..240)
            .flat_map(|x| (0..40).map(move |y| (x, y)))
            .filter(|&(x, y)| lit(&curved, x, y))
            .count();
        assert!(top > 60, "title pixels near the top: {top}");
        assert!(
            !(0..240).any(|x| (60..240).any(|y| lit(&curved, x, y))),
            "nothing below y = 60"
        );
        let bottom = render_one(Drawable::CurvedText {
            text: "MEMORY".into(),
            radius: 106.0,
            centre_deg: 180.0,
            px: 10.0,
            font: FontId::Bold,
            letter_spacing: 1.5,
            color: WHITE,
            alpha: 1.0,
        });
        assert!((0..240).any(|x| (200..240).any(|y| lit(&bottom, x, y))));
    }
}

//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::scene::{Drawable, Scene, SegState};
use rackscreen_core::theme::layout::{CX, CY};
use rackscreen_core::theme::OFF;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::frame::new_pixmap;
use crate::icons::IconCache;
use crate::prims::{
    circle, circle_stroke, fill, outline, rounded_rect, segment_outline, skia_color, SegmentCache,
};
use crate::text::TextRenderer;

/// Below this the scene is drawn through the scratch pixmap (zoom) or with
/// truncated rings (reveal); at 1.0 both are no-ops and skipped.
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
                segs: SegmentCache::new(),
                icons: IconCache::new()?,
                text: TextRenderer::new()?,
            },
            scratch: new_pixmap(),
        })
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        let zoom = scene.zoom.clamp(0.0, 1.0);
        let reveal = scene.ring_reveal.clamp(0.0, 1.0);
        if zoom >= FULL {
            for d in &scene.items {
                self.painter.draw_item(px, d, reveal);
            }
            return;
        }
        px.fill(tiny_skia::Color::BLACK);
        if zoom <= 0.0 {
            return;
        }
        for d in &scene.items {
            self.painter.draw_item(&mut self.scratch, d, reveal);
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
    segs: SegmentCache,
    icons: IconCache,
    text: TextRenderer,
}

impl Painter {
    /// Draw one item. Rings show only the first `reveal` fraction of their
    /// segments lit; the rest are drawn OFF.
    fn draw_item(&mut self, px: &mut Pixmap, d: &Drawable, reveal: f32) {
        match d {
            Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
            Drawable::Ring {
                cx,
                cy,
                radius,
                n,
                states,
                pitch_deg,
                start_deg,
            } => {
                let keep = if reveal >= FULL {
                    states.len()
                } else {
                    (states.len() as f32 * reveal).round() as usize
                };
                let segs = self
                    .segs
                    .segments_pitched(*cx, *cy, *radius, *n, *pitch_deg, *start_deg);
                for (i, (path, st)) in segs.iter().zip(states).enumerate() {
                    let st = if i >= keep { &SegState::Off } else { st };
                    match st {
                        SegState::Off => fill(px, path, OFF, 1.0),
                        SegState::On(c, a) => {
                            if *a < 1.0 {
                                fill(px, path, OFF, 1.0);
                            }
                            fill(px, path, *c, *a);
                        }
                    }
                }
            }
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
            Drawable::Badge {
                cx,
                cy,
                w,
                h,
                radius,
                stroke,
                fill: fill_c,
                text,
                text_px,
                text_color,
                alpha,
            } => {
                let rr = rounded_rect(cx - w / 2.0, cy - h / 2.0, *w, *h, *radius);
                fill(px, &rr, *fill_c, *alpha);
                if let Some(border) = outline(&rr, 2.0) {
                    fill(px, &border, *stroke, *alpha);
                }
                self.text
                    .draw_centered(px, text, *text_px, *cx, *cy, *text_color, *alpha);
            }
            Drawable::Ripple {
                cx,
                cy,
                r,
                thickness,
                color,
                alpha,
            } => {
                if let Some(ring) = circle_stroke(*cx, *cy, *r, *thickness) {
                    fill(px, &ring, *color, *alpha);
                }
            }
            Drawable::Dots {
                cx,
                cy,
                spacing,
                r,
                colors,
            } => {
                let n = colors.len();
                if n == 0 {
                    return;
                }
                let x0 = cx - (n as f32 - 1.0) * spacing / 2.0;
                for (i, c) in colors.iter().enumerate() {
                    let a = c.a as f32 / 255.0;
                    let solid = rackscreen_core::theme::Color { a: 255, ..*c };
                    fill(px, &circle(x0 + i as f32 * spacing, *cy, *r), solid, a);
                }
            }
            // A radial tick: `segment_outline` centres a line of length
            // `r1 - r0` on the midpoint radius, so it covers r0..r1 plus the
            // round cap's `width / 2` at each end, exactly like a ring segment.
            Drawable::Tick {
                cx,
                cy,
                angle_deg,
                r0,
                r1,
                width,
                color,
                alpha,
            } => {
                let path = segment_outline(*cx, *cy, (r0 + r1) / 2.0, *angle_deg, r1 - r0, *width);
                fill(px, &path, *color, *alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::event::{Event, LinkTarget};
    use rackscreen_core::model::{Model, Thresholds};
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

    fn amberish((r, g, b): (u8, u8, u8)) -> bool {
        r > 200 && g > 140 && b < 80
    }

    fn off_grey((r, ..): (u8, u8, u8)) -> bool {
        (20..=40).contains(&r)
    }

    #[test]
    fn zoom_scales_the_scene_about_the_centre() {
        let m = cpu_model(42.0);
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = m.scene_for_role(Role::Cpu, 5.0);
        r.render(&s, &mut px);
        assert!(amberish(pixel(&px, 120, 18)), "segment 0 lit at full scale");
        s.zoom = 0.5;
        r.render(&s, &mut px);
        assert_eq!(
            pixel(&px, 120, 18),
            (0, 0, 0),
            "full-scale segment 0 is gone"
        );
        assert!(
            amberish(pixel(&px, 120, 69)),
            "segment 0 at half scale, got {:?}",
            pixel(&px, 120, 69)
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

    #[test]
    fn ring_reveal_truncates_lit_segments() {
        let m = cpu_model(100.0);
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = m.scene_for_role(Role::Cpu, 5.0);
        r.render(&s, &mut px);
        assert!(
            amberish(pixel(&px, 120, 222)),
            "segment 30 lit on a full ring"
        );
        s.ring_reveal = 0.5;
        r.render(&s, &mut px);
        assert!(amberish(pixel(&px, 120, 18)), "segment 0 still lit");
        assert!(
            off_grey(pixel(&px, 120, 222)),
            "segment 30 is OFF, got {:?}",
            pixel(&px, 120, 222)
        );
    }

    #[test]
    fn tick_spans_r0_to_r1() {
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut s = Scene::new();
        s.push(Drawable::Tick {
            cx: CX,
            cy: CY,
            angle_deg: 0.0,
            r0: 80.0,
            r1: 88.0,
            width: 4.0,
            color: rackscreen_core::theme::WHITE,
            alpha: 1.0,
        });
        r.render(&s, &mut px);
        assert!(pixel(&px, 120, 36).0 > 200, "r = 84, inside r0..r1");
        assert_eq!(pixel(&px, 120, 26), (0, 0, 0), "r = 94, past r1 + cap");
        assert_eq!(pixel(&px, 120, 48), (0, 0, 0), "r = 72, short of r0 - cap");
    }
}

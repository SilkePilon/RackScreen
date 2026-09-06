//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::scene::{Drawable, Scene, SegState};
use rackscreen_core::theme::OFF;
use tiny_skia::Pixmap;

use crate::icons::IconCache;
use crate::prims::{circle, circle_stroke, fill, outline, rounded_rect, skia_color, SegmentCache};
use crate::text::TextRenderer;

pub struct Renderer {
    segs: SegmentCache,
    icons: IconCache,
    text: TextRenderer,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            segs: SegmentCache::new(),
            icons: IconCache::new()?,
            text: TextRenderer::new()?,
        })
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        for d in &scene.items {
            match d {
                Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
                Drawable::Ring {
                    cx,
                    cy,
                    radius,
                    n,
                    states,
                } => {
                    let segs = self.segs.segments(*cx, *cy, *radius, *n);
                    for (path, st) in segs.iter().zip(states) {
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
                        continue;
                    }
                    let x0 = cx - (n as f32 - 1.0) * spacing / 2.0;
                    for (i, c) in colors.iter().enumerate() {
                        let a = c.a as f32 / 255.0;
                        let solid = rackscreen_core::theme::Color { a: 255, ..*c };
                        fill(px, &circle(x0 + i as f32 * spacing, *cy, *r), solid, a);
                    }
                }
            }
        }
    }
}

//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::scene::{Drawable, Scene};
use rackscreen_core::shows::world::{WorldFrame, SCALE, WORLD_H};
use tiny_skia::Pixmap;

use crate::icons::IconCache;
use crate::prims::{
    arc_points, ellipse_path, fill, polygon_path, polyline_path, rounded_rect, skia_color, stroke,
};
use crate::text::TextRenderer;

pub struct Renderer {
    painter: Painter,
    /// The last base layer (`scene.items[..base_len]`) and its raster.
    base: Option<(Vec<Drawable>, Pixmap)>,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            painter: Painter {
                icons: IconCache::new()?,
                text: TextRenderer::new()?,
            },
            base: None,
        })
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        let n = scene.base_len.min(scene.items.len());
        let cacheable = n > 0 && matches!(scene.items.first(), Some(Drawable::Clear(_)));
        let start = if cacheable {
            let base = &scene.items[..n];
            match &self.base {
                Some((d, raster))
                    if d.as_slice() == base && raster.data().len() == px.data().len() =>
                {
                    px.data_mut().copy_from_slice(raster.data());
                }
                _ => {
                    for d in base {
                        self.painter.draw_item(px, d);
                    }
                    self.base = Some((base.to_vec(), px.clone()));
                }
            }
            n
        } else {
            0
        };
        for d in &scene.items[start..] {
            self.painter.draw_item(px, d);
        }
        if scene.fade < 1.0 {
            crate::frame::darken(px, scene.fade);
        }
    }
}

/// The caches a single drawable needs; separate from `Renderer` so the base
/// cache can be borrowed alongside them.
struct Painter {
    icons: IconCache,
    text: TextRenderer,
}

impl Painter {
    /// Draw one item.
    fn draw_item(&mut self, px: &mut Pixmap, d: &Drawable) {
        match d {
            Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
            Drawable::World { frame, x0 } => blit_world(px, &frame.0, *x0),
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
        }
    }
}

/// Copy world columns `x0..` into `px`, each world pixel a `SCALE`² block.
/// World pixels are opaque (or fully clear), so premultiplied bytes equal
/// the straight RGBA bytes.
fn blit_world(px: &mut Pixmap, world: &WorldFrame, x0: u32) {
    let (w, h) = (px.width(), px.height());
    let data = px.data_mut();
    for py in 0..h {
        let wy = py / SCALE;
        if wy >= WORLD_H {
            break;
        }
        for pxx in 0..w {
            let wx = x0 + pxx / SCALE;
            if wx >= world.width {
                break;
            }
            let src = ((wy * world.width + wx) * 4) as usize;
            let dst = ((py * w + pxx) * 4) as usize;
            data[dst..dst + 4].copy_from_slice(&world.rgba[src..src + 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::new_pixmap;
    use rackscreen_core::theme::layout::{CX, CY};

    fn pixel(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * p.width() + x) * 4) as usize;
        (p.data()[i], p.data()[i + 1], p.data()[i + 2])
    }

    use rackscreen_core::p2::font::FontId;
    use rackscreen_core::theme::Color;
    use rackscreen_core::theme::WHITE;

    fn based_scene(hero_x: f32) -> Scene {
        let mut s = Scene::new();
        s.push(Drawable::Arc {
            cx: 120.0,
            cy: 120.0,
            r: 106.0,
            width: 9.0,
            start_deg: 30.0,
            sweep_deg: 300.0,
            color: Color::hex(0x2C2C33),
            alpha: 1.0,
        });
        s.push(Drawable::RoundRect {
            x: 66.0,
            y: 150.0,
            w: 108.0,
            h: 48.0,
            r: 14.0,
            color: Color::hex(0x24242A),
            alpha: 1.0,
        });
        s.base_len = s.items.len();
        s.push(Drawable::RoundRect {
            x: hero_x,
            y: 60.0,
            w: 30.0,
            h: 30.0,
            r: 4.0,
            color: Color::hex(0xEDEDEF),
            alpha: 1.0,
        });
        s
    }

    #[test]
    fn base_cache_draws_the_same_pixels_as_one_pass() {
        let mut cached = Renderer::new().unwrap();
        let mut plain = Renderer::new().unwrap();
        for x in [50.0, 80.0, 80.0, 110.0] {
            let s = based_scene(x);
            let mut flat = s.clone();
            flat.base_len = 0;
            let (mut a, mut b) = (new_pixmap(), new_pixmap());
            cached.render(&s, &mut a);
            plain.render(&flat, &mut b);
            assert_eq!(a.data(), b.data(), "hero at {x}");
        }
    }

    #[test]
    fn scene_fade_darkens_the_whole_frame() {
        let mut r = Renderer::new().unwrap();
        let mut s = based_scene(50.0);
        let mut full = new_pixmap();
        r.render(&s, &mut full);
        s.fade = 0.5;
        let mut half = new_pixmap();
        r.render(&s, &mut half);
        for (a, b) in full.data().chunks_exact(4).zip(half.data().chunks_exact(4)) {
            for c in 0..3 {
                assert_eq!(b[c], (a[c] as f32 * 0.5) as u8);
            }
        }
        s.fade = 0.0;
        r.render(&s, &mut half);
        assert!(half.data().chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
        // the cached base raster was not darkened
        s.fade = 1.0;
        let mut again = new_pixmap();
        r.render(&s, &mut again);
        assert_eq!(again.data(), full.data());
    }

    #[test]
    fn base_without_leading_clear_is_not_cached() {
        let mut r = Renderer::new().unwrap();
        let mut s = based_scene(50.0);
        s.items.remove(0);
        s.base_len -= 1;
        let mut px = new_pixmap();
        r.render(&s, &mut px);
        assert!(r.base.is_none());
    }

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
    fn world_blits_its_panel_slice_scaled_by_four() {
        use rackscreen_core::scene::WorldRef;
        use rackscreen_core::shows::world::{Layout, World, WorldFrame};
        use std::sync::Arc;
        let layout = Layout { panels: 2, gap: 6 };
        let mut f = WorldFrame::new(layout.width());
        let mut w = World {
            frame: &mut f,
            layout,
        };
        w.clear(0x0000ff);
        w.p(0xff0000, 66.0, 0.0, 60, 60);
        w.p(0x00ff00, 61.0, 0.0, 1, 60); // in the gap: never shown
        let frame = WorldRef(Arc::new(f));
        let red = render_one(Drawable::World {
            frame: frame.clone(),
            x0: 66,
        });
        let blue = render_one(Drawable::World { frame, x0: 0 });
        for (x, y) in [(0, 0), (239, 239), (120, 3), (3, 120), (239, 0)] {
            assert_eq!(pixel(&red, x, y), (255, 0, 0), "({x},{y})");
            assert_eq!(pixel(&blue, x, y), (0, 0, 255), "({x},{y})");
        }
        assert!(red.data().chunks(4).all(|p| p == [255, 0, 0, 255]));
        assert!(blue.data().chunks(4).all(|p| p == [0, 0, 255, 255]));
    }

    #[test]
    fn world_ref_compares_by_identity() {
        use rackscreen_core::scene::WorldRef;
        use rackscreen_core::shows::world::WorldFrame;
        use std::sync::Arc;
        let a = WorldRef(Arc::new(WorldFrame::new(60)));
        let b = WorldRef(Arc::new(WorldFrame::new(60)));
        assert_eq!(a, a.clone());
        assert_ne!(a, b, "equal pixels, different frames");
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
    fn run_lands_where_asked() {
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
    }
}

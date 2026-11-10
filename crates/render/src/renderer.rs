//! Turns a `Scene` into pixels.

use anyhow::Result;
use rackscreen_core::event::CoverRef;
use rackscreen_core::gif::GifFrames;
use rackscreen_core::scene::{Drawable, Scene};
use rackscreen_core::shows::world::{WorldFrame, SCALE, WORLD_H};
use tiny_skia::{FillRule, FilterQuality, Paint, Pattern, Pixmap, SpreadMode, Transform};

use crate::icons::IconCache;
use crate::prims::{
    arc_points, ellipse_path, fill, polygon_path, polyline_path, rounded_rect, skia_color, stroke,
    stroke_dashed,
};
use crate::text::TextRenderer;

pub struct Renderer {
    painter: Painter,
    /// The last base layers (`scene.items[..base_len]`) and their rasters,
    /// newest last: two, so both sides of a transition stay cached.
    base: Vec<(Vec<Drawable>, Pixmap)>,
}

/// Base rasters kept: the two scenes of a transition.
const BASE_CACHE: usize = 2;

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            painter: Painter {
                icons: IconCache::new()?,
                text: TextRenderer::new()?,
                covers: Vec::new(),
                globes: 0,
            },
            base: Vec::new(),
        })
    }

    /// How many globes have been rasterised (not reused from the base).
    pub fn globe_rasters(&self) -> u32 {
        self.painter.globes
    }

    pub fn render(&mut self, scene: &Scene, px: &mut Pixmap) {
        let n = scene.base_len.min(scene.items.len());
        let cacheable = n > 0 && matches!(scene.items.first(), Some(Drawable::Clear(_)));
        let start = if cacheable {
            let base = &scene.items[..n];
            let hit = self.base.iter().position(|(d, raster)| {
                d.as_slice() == base && raster.data().len() == px.data().len()
            });
            match hit {
                Some(i) => {
                    px.data_mut().copy_from_slice(self.base[i].1.data());
                    // keep the hit newest, so the other side survives
                    let entry = self.base.remove(i);
                    self.base.push(entry);
                }
                None => {
                    for d in base {
                        self.painter.draw_item(px, d);
                    }
                    // the oldest entry's buffer takes the new raster: a base
                    // that changes every second allocates nothing
                    let old = (self.base.len() >= BASE_CACHE).then(|| self.base.remove(0).1);
                    let raster = match old {
                        Some(mut raster) if raster.data().len() == px.data().len() => {
                            raster.data_mut().copy_from_slice(px.data());
                            raster
                        }
                        _ => px.clone(),
                    };
                    self.base.push((base.to_vec(), raster));
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
    /// Premultiplied pixmaps of the last few covers, newest last.
    covers: Vec<(CoverRef, Pixmap)>,
    /// Globes rasterised so far (the costly drawable; tests count them).
    globes: u32,
}

/// Covers kept as pixmaps: one per music source plus the last played.
const COVER_CACHE: usize = 4;

impl Painter {
    /// The cover's premultiplied pixmap, built once per key.
    fn cover(&mut self, cover: &CoverRef) -> Option<&Pixmap> {
        if let Some(i) = self.covers.iter().position(|(c, _)| c == cover) {
            let hit = self.covers.remove(i);
            self.covers.push(hit);
        } else {
            let c = &cover.0;
            let mut pm = Pixmap::new(c.size, c.size)?;
            if c.rgba.len() != pm.data().len() {
                return None;
            }
            for (dst, src) in pm
                .data_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(c.rgba.as_chunks::<4>().0)
            {
                let a = src[3] as u32;
                let pre = |v: u8| ((v as u32 * a + 127) / 255) as u8;
                dst.copy_from_slice(&[pre(src[0]), pre(src[1]), pre(src[2]), src[3]]);
            }
            if self.covers.len() >= COVER_CACHE {
                self.covers.remove(0);
            }
            self.covers.push((cover.clone(), pm));
        }
        self.covers.last().map(|(_, p)| p)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_cover(
        &mut self,
        px: &mut Pixmap,
        cover: &CoverRef,
        x: f32,
        y: f32,
        size: f32,
        r: f32,
        alpha: f32,
    ) {
        if alpha <= 0.0 || size <= 0.0 {
            return;
        }
        let Some(img) = self.cover(cover) else {
            return;
        };
        let k = size / img.width() as f32;
        let paint = Paint {
            anti_alias: true,
            shader: Pattern::new(
                img.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                alpha.clamp(0.0, 1.0),
                Transform::from_row(k, 0.0, 0.0, k, x, y),
            ),
            ..Paint::default()
        };
        px.fill_path(
            &rounded_rect(x, y, size, size, r),
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Draw one item.
    fn draw_item(&mut self, px: &mut Pixmap, d: &Drawable) {
        match d {
            Drawable::Clear(c) => px.fill(skia_color(*c, 1.0)),
            Drawable::World { frame, x0 } => blit_world(px, &frame.0, *x0),
            Drawable::Image { frames, frame } => blit_image(px, &frames.0, *frame),
            Drawable::Cover {
                cover,
                x,
                y,
                size,
                r,
                alpha,
            } => self.draw_cover(px, cover, *x, *y, *size, *r, *alpha),
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
            Drawable::Dashed {
                points,
                width,
                on,
                off,
                color,
                alpha,
            } => {
                if let Some(p) = polyline_path(points) {
                    stroke_dashed(px, &p, *width, *on, *off, *color, *alpha);
                }
            }
            Drawable::IconOutline {
                name,
                cx,
                cy,
                size,
                width,
                color,
                alpha,
            } => {
                self.icons
                    .draw_outline(px, name, *cx, *cy, *size, *width, *color, *alpha);
            }
            Drawable::Globe {
                cam_lat,
                cam_lon,
                sun_lat,
                sun_lon,
                r,
            } => {
                self.globes += 1;
                crate::globe::draw_globe(px, *cam_lat, *cam_lon, *sun_lat, *sun_lon, *r);
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
    let stride = w as usize * 4;
    // The panel pixels the world covers; a slice past the world's right
    // edge is cut off there.
    let cols = (world.width.saturating_sub(x0) * SCALE).min(w) as usize;
    let mut row = vec![0u8; cols * 4];
    for wy in 0..h.div_ceil(SCALE).min(WORLD_H) {
        // Scale one world row once...
        let src = &world.rgba[(wy * world.width) as usize * 4..];
        for (pxx, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let s = (x0 as usize + pxx / SCALE as usize) * 4;
            out.copy_from_slice(&src[s..s + 4]);
        }
        // ...then copy it into its SCALE scanlines.
        for py in wy * SCALE..((wy + 1) * SCALE).min(h) {
            let dst = py as usize * stride;
            data[dst..dst + row.len()].copy_from_slice(&row);
        }
    }
}

/// Unpack a GIF frame over the whole pixmap. Frames are opaque premultiplied
/// RGBA of the panel's size; a pixmap of any other size (never built) is
/// left alone.
fn blit_image(px: &mut Pixmap, frames: &GifFrames, frame: u16) {
    frames.blit(frame as usize, px.data_mut());
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
        for (a, b) in full
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(half.data().as_chunks::<4>().0)
        {
            for c in 0..3 {
                assert_eq!(b[c], (a[c] as f32 * 0.5) as u8);
            }
        }
        s.fade = 0.0;
        r.render(&s, &mut half);
        assert!(half
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3] == [0, 0, 0]));
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
        assert!(r.base.is_empty());
    }

    #[test]
    fn two_bases_alternate_without_rerendering() {
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        let mut a = based_scene(50.0);
        a.items[1] = Drawable::Clear(rackscreen_core::theme::Color::hex(0x101010));
        let b = based_scene(50.0);
        r.render(&a, &mut px);
        r.render(&b, &mut px);
        assert_eq!(r.base.len(), 2);
        let bases: Vec<Vec<Drawable>> = r.base.iter().map(|(d, _)| d.clone()).collect();
        // both hit: the cache keeps the same two entries
        r.render(&a, &mut px);
        r.render(&b, &mut px);
        let again: Vec<Vec<Drawable>> = r.base.iter().map(|(d, _)| d.clone()).collect();
        assert_eq!(bases, again);
        // a third base pushes the oldest out
        let mut c = based_scene(50.0);
        c.items[1] = Drawable::Clear(rackscreen_core::theme::Color::hex(0x202020));
        r.render(&c, &mut px);
        assert_eq!(r.base.len(), 2);
        assert_eq!(r.base[1].0, c.items[..c.base_len].to_vec());
        assert_eq!(r.base[0].0, b.items[..b.base_len].to_vec());
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

    /// The per-pixel loop `blit_world` replaced, kept as the reference.
    fn blit_world_by_pixel(px: &mut Pixmap, world: &WorldFrame, x0: u32) {
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

    #[test]
    fn world_blit_by_rows_matches_the_per_pixel_loop() {
        let width = 150;
        let mut world = WorldFrame::new(width);
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        for b in world.rgba.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x as u8;
        }
        // whole slices, one ending at the world's right edge, one cut off by it
        for x0 in [0, 1, 45, 90, 120, 149, 150] {
            let mut a = crate::frame::new_pixmap();
            let mut b = crate::frame::new_pixmap();
            a.fill(tiny_skia::Color::from_rgba8(7, 8, 9, 255));
            b.fill(tiny_skia::Color::from_rgba8(7, 8, 9, 255));
            blit_world(&mut a, &world, x0);
            blit_world_by_pixel(&mut b, &world, x0);
            assert!(a.data() == b.data(), "x0 {x0}");
        }
    }

    #[test]
    fn image_copies_the_gif_frame_bytes() {
        use rackscreen_core::gif::{GifFrames, GifRef, FRAME_BYTES, GIF_SIDE};
        use std::sync::Arc;
        let pattern = |seed: u8| -> Vec<u8> {
            (0..FRAME_BYTES)
                .map(|i| {
                    if i % 4 == 3 {
                        255
                    } else {
                        (i / 4 % 251) as u8 ^ seed
                    }
                })
                .collect()
        };
        let g = GifRef(Arc::new(
            GifFrames::new(vec![(pattern(0), 100), (pattern(0x5a), 100)]).unwrap(),
        ));
        let px = render_one(Drawable::Image {
            frames: g.clone(),
            frame: 1,
        });
        // the whole frame is copied; the round panel hides the corners
        let want = g.0.frame(1);
        let r = GIF_SIDE as f32 / 2.0;
        for y in 0..GIF_SIDE {
            for x in 0..GIF_SIDE {
                let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
                if dx * dx + dy * dy <= r * r {
                    let i = ((y * GIF_SIDE + x) * 4) as usize;
                    assert_eq!(&px.data()[i..i + 4], &want[i..i + 4], "({x},{y})");
                }
            }
        }
        assert_eq!(px.data(), &want[..]);
    }

    fn cover(key: &str, rgb: (u8, u8, u8), size: u32) -> rackscreen_core::event::CoverRef {
        let rgba = (0..size * size)
            .flat_map(|_| [rgb.0, rgb.1, rgb.2, 255])
            .collect();
        rackscreen_core::event::CoverRef::new(rackscreen_core::event::Cover {
            key: key.into(),
            size,
            rgba,
        })
    }

    #[test]
    fn cover_fills_a_rounded_square_scaled_to_size() {
        let full = render_one(Drawable::Cover {
            cover: cover("red", (250, 0, 0), 40),
            x: 78.0,
            y: 36.0,
            size: 84.0,
            r: 14.0,
            alpha: 1.0,
        });
        assert_eq!(pixel(&full, 120, 78), (250, 0, 0), "centre");
        assert_eq!(pixel(&full, 158, 78), (250, 0, 0), "scaled up past 40 px");
        assert_eq!(pixel(&full, 78, 36), (0, 0, 0), "rounded corner");
        assert_eq!(pixel(&full, 120, 125), (0, 0, 0), "below the square");
        let dim = render_one(Drawable::Cover {
            cover: cover("red", (250, 0, 0), 84),
            x: 78.0,
            y: 36.0,
            size: 84.0,
            r: 14.0,
            alpha: 0.5,
        });
        let r = pixel(&dim, 120, 78).0;
        assert!((120..=130).contains(&r), "half alpha: {r}");
    }

    #[test]
    fn cover_pixmaps_are_cached_by_key() {
        let mut r = Renderer::new().unwrap();
        let mut px = new_pixmap();
        for (key, rgb) in [("a", (200, 0, 0)), ("b", (0, 200, 0)), ("a", (200, 0, 0))] {
            let mut s = Scene::new();
            s.push(Drawable::Cover {
                cover: cover(key, rgb, 8),
                x: 0.0,
                y: 0.0,
                size: 8.0,
                r: 0.0,
                alpha: 1.0,
            });
            r.render(&s, &mut px);
            assert_eq!(pixel(&px, 4, 4), rgb);
        }
        assert_eq!(r.painter.covers.len(), 2);
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

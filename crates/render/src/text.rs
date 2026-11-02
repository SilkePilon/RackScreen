//! Glyph rasterizing with fontdue and centred text placement.

use std::collections::HashMap;

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings, Metrics};
use rackscreen_core::p2::font::{advance, bytes, cap_height, measure, FontId};
use rackscreen_core::scene::ClipRect;
use rackscreen_core::theme::Color;
use tiny_skia::{BlendMode, FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

use crate::assets::FONT_BOLD;

pub struct TextRenderer {
    /// JetBrains Mono, for the legacy centred badge text (removed in the P2 cleanup).
    font: Font,
    /// Inter Medium, SemiBold, Bold in `FontId` order.
    inter: [Font; 3],
    cache: HashMap<(char, u32), (Metrics, Vec<u8>)>,
    /// Glyphs keyed by face, character and size in half pixels.
    run_cache: HashMap<(FontId, char, u32), (Metrics, Vec<u8>)>,
    /// Tinted glyph pixmaps for rotated drawing, keyed by face, char, half px, rgb.
    #[allow(clippy::type_complexity)]
    curved_cache: HashMap<(FontId, char, u32, u32), Option<(Metrics, Pixmap)>>,
}

struct Glyph {
    x: i32,
    top: i32,
    metrics: Metrics,
    bitmap: Vec<u8>,
}

impl TextRenderer {
    pub fn new() -> Result<Self> {
        let font = Font::from_bytes(FONT_BOLD, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("font: {e}"))
            .context("load embedded font")?;
        let load = |id: FontId| {
            Font::from_bytes(bytes(id), FontSettings::default())
                .map_err(|e| anyhow::anyhow!("inter {id:?}: {e}"))
        };
        let inter = [
            load(FontId::Medium)?,
            load(FontId::SemiBold)?,
            load(FontId::Bold)?,
        ];
        Ok(Self {
            font,
            inter,
            cache: HashMap::new(),
            run_cache: HashMap::new(),
            curved_cache: HashMap::new(),
        })
    }

    /// Size quantised to half pixels, so a rolling hero does not fill the cache.
    fn half_px(px: f32) -> u32 {
        (px * 2.0).round().max(1.0) as u32
    }

    fn inter_glyph(&mut self, id: FontId, ch: char, px: f32) -> (Metrics, Vec<u8>) {
        let key = (id, ch, Self::half_px(px));
        if let Some(g) = self.run_cache.get(&key) {
            return g.clone();
        }
        let g = self.inter[id.index()].rasterize(ch, key.2 as f32 / 2.0);
        self.run_cache.insert(key, g.clone());
        g
    }

    /// A glyph pre-tinted into its own premultiplied pixmap, for rotated drawing.
    fn curved_glyph(
        &mut self,
        id: FontId,
        ch: char,
        px: f32,
        color: Color,
    ) -> Option<(Metrics, Pixmap)> {
        let rgb = ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32;
        let key = (id, ch, Self::half_px(px), rgb);
        if let Some(g) = self.curved_cache.get(&key) {
            return g.clone();
        }
        let (m, cov) = self.inter_glyph(id, ch, px);
        let built = IntSize::from_wh(m.width as u32, m.height as u32).and_then(|size| {
            let mut data = Vec::with_capacity(cov.len() * 4);
            for c in &cov {
                let a = *c as u32;
                data.push((color.r as u32 * a / 255) as u8);
                data.push((color.g as u32 * a / 255) as u8);
                data.push((color.b as u32 * a / 255) as u8);
                data.push(*c);
            }
            Pixmap::from_vec(data, size).map(|p| (m, p))
        });
        self.curved_cache.insert(key, built.clone());
        built
    }

    /// Text along a circle of `radius` around `(cx, cy)`, each glyph upright
    /// away from the centre, the run centred on `centre_deg` (clockwise from
    /// 12 o'clock) with its cap height centred on the circle.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_curved(
        &mut self,
        pix: &mut Pixmap,
        text: &str,
        font: FontId,
        px: f32,
        cx: f32,
        cy: f32,
        radius: f32,
        centre_deg: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
    ) {
        if alpha <= 0.0 || text.is_empty() || radius <= 0.0 {
            return;
        }
        let total = measure(font, px, text, letter_spacing);
        let cap = cap_height(font, px);
        let mut a = centre_deg.to_radians() - total / 2.0 / radius;
        let paint = PixmapPaint {
            opacity: alpha.clamp(0.0, 1.0),
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Bilinear,
        };
        for ch in text.chars() {
            let adv = advance(font, px, ch);
            let w = adv + letter_spacing;
            let ac = a + w / 2.0 / radius;
            a += w / radius;
            let Some((m, glyph)) = self.curved_glyph(font, ch, px, color) else {
                continue;
            };
            let gx = cx + radius * ac.sin();
            let gy = cy - radius * ac.cos();
            let ts = Transform::from_translate(gx, gy)
                .pre_rotate(ac.to_degrees())
                .pre_translate(
                    m.xmin as f32 - adv / 2.0,
                    -(m.height as f32 + m.ymin as f32) + cap / 2.0,
                );
            pix.draw_pixmap(0, 0, glyph.as_ref(), &paint, ts, None);
        }
    }

    /// Draw `text` with its left edge at `x` and its baseline at `baseline`,
    /// `letter_spacing` px between characters, optionally clipped.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_run(
        &mut self,
        pix: &mut Pixmap,
        text: &str,
        font: FontId,
        px: f32,
        x: f32,
        baseline: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
        clip: Option<ClipRect>,
    ) {
        if alpha <= 0.0 || text.is_empty() {
            return;
        }
        let pw = pix.width() as i32;
        let ph = pix.height() as i32;
        let (cx0, cy0, cx1, cy1) = match clip {
            Some(c) => (
                (c.x.floor() as i32).max(0),
                (c.y.floor() as i32).max(0),
                ((c.x + c.w).ceil() as i32).min(pw),
                ((c.y + c.h).ceil() as i32).min(ph),
            ),
            None => (0, 0, pw, ph),
        };
        let base = baseline.round() as i32;
        let mut pen = x;
        for ch in text.chars() {
            let (m, bitmap) = self.inter_glyph(font, ch, px);
            let gx = pen.round() as i32 + m.xmin;
            let gtop = base - (m.height as i32 + m.ymin);
            let data = pix.data_mut();
            for row in 0..m.height as i32 {
                let y = gtop + row;
                if y < cy0 || y >= cy1 {
                    continue;
                }
                for col in 0..m.width as i32 {
                    let xx = gx + col;
                    if xx < cx0 || xx >= cx1 {
                        continue;
                    }
                    let cov = bitmap[row as usize * m.width + col as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = (cov as f32 / 255.0) * alpha * (color.a as f32 / 255.0);
                    let i = ((y * pw + xx) * 4) as usize;
                    blend(&mut data[i..i + 4], color, a);
                }
            }
            pen += m.advance_width + letter_spacing;
        }
    }

    fn glyph(&mut self, ch: char, px: f32) -> (Metrics, Vec<u8>) {
        let key = (ch, (px * 4.0) as u32);
        if let Some(g) = self.cache.get(&key) {
            return g.clone();
        }
        let g = self.font.rasterize(ch, px);
        self.cache.insert(key, g.clone());
        g
    }

    /// Lay out glyphs with the baseline at y=0. Returns glyphs and (width, top, bottom).
    fn layout(&mut self, text: &str, px: f32) -> (Vec<Glyph>, f32, i32, i32) {
        let mut x = 0.0f32;
        let mut glyphs = Vec::new();
        let mut top = i32::MAX;
        let mut bottom = i32::MIN;
        for ch in text.chars() {
            let (m, bitmap) = self.glyph(ch, px);
            let gx = x.round() as i32 + m.xmin;
            let gtop = -(m.height as i32 + m.ymin);
            if m.width > 0 && m.height > 0 {
                top = top.min(gtop);
                bottom = bottom.max(gtop + m.height as i32);
            }
            glyphs.push(Glyph {
                x: gx,
                top: gtop,
                metrics: m,
                bitmap,
            });
            x += m.advance_width;
        }
        if top == i32::MAX {
            top = 0;
            bottom = 0;
        }
        (glyphs, x, top, bottom)
    }

    pub fn measure(&mut self, text: &str, px: f32) -> (f32, f32) {
        let (_, w, top, bottom) = self.layout(text, px);
        (w, (bottom - top) as f32)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered(
        &mut self,
        pix: &mut Pixmap,
        text: &str,
        px: f32,
        cx: f32,
        cy: f32,
        color: Color,
        alpha: f32,
    ) {
        if alpha <= 0.0 || text.is_empty() {
            return;
        }
        let (glyphs, width, top, bottom) = self.layout(text, px);
        let x0 = (cx - width / 2.0).round() as i32;
        let baseline = (cy - (top + bottom) as f32 / 2.0).round() as i32;
        let pw = pix.width() as i32;
        let ph = pix.height() as i32;
        let data = pix.data_mut();
        for g in glyphs {
            for gy in 0..g.metrics.height as i32 {
                let y = baseline + g.top + gy;
                if y < 0 || y >= ph {
                    continue;
                }
                for gx in 0..g.metrics.width as i32 {
                    let x = x0 + g.x + gx;
                    if x < 0 || x >= pw {
                        continue;
                    }
                    let cov = g.bitmap[(gy as usize) * g.metrics.width + gx as usize];
                    if cov == 0 {
                        continue;
                    }
                    let a = (cov as f32 / 255.0) * alpha * (color.a as f32 / 255.0);
                    let i = ((y * pw + x) * 4) as usize;
                    blend(&mut data[i..i + 4], color, a);
                }
            }
        }
    }
}

/// Source-over onto an opaque pixel.
fn blend(dst: &mut [u8], c: Color, a: f32) {
    let a = a.clamp(0.0, 1.0);
    let mix = |d: u8, s: u8| (d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
    dst[0] = mix(dst[0], c.r);
    dst[1] = mix(dst[1], c.g);
    dst[2] = mix(dst[2], c.b);
    dst[3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::p2::font::FontId;
    use rackscreen_core::scene::ClipRect;
    use rackscreen_core::theme::WHITE;

    #[test]
    fn draw_run_places_left_edge_and_baseline() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(120, 60).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_run(
            &mut p,
            "H",
            FontId::Bold,
            20.0,
            10.0,
            40.0,
            0.0,
            WHITE,
            1.0,
            None,
        );
        let (x0, _, _, y1) = lit_bbox(&p).expect("drawn");
        assert!((10..=13).contains(&x0), "left edge near x: {x0}");
        assert!((38..=40).contains(&y1), "bottom on the baseline: {y1}");
    }

    #[test]
    fn draw_run_letter_spacing_and_clip() {
        let mut t = TextRenderer::new().unwrap();
        let mut a = Pixmap::new(120, 60).unwrap();
        a.fill(tiny_skia::Color::BLACK);
        t.draw_run(
            &mut a,
            "HH",
            FontId::Bold,
            20.0,
            10.0,
            40.0,
            0.0,
            WHITE,
            1.0,
            None,
        );
        let mut b = Pixmap::new(120, 60).unwrap();
        b.fill(tiny_skia::Color::BLACK);
        t.draw_run(
            &mut b,
            "HH",
            FontId::Bold,
            20.0,
            10.0,
            40.0,
            4.0,
            WHITE,
            1.0,
            None,
        );
        let wa = lit_bbox(&a).unwrap().2;
        let wb = lit_bbox(&b).unwrap().2;
        assert_eq!(wb - wa, 4, "one gap of 4 px");
        let mut c = Pixmap::new(120, 60).unwrap();
        c.fill(tiny_skia::Color::BLACK);
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            w: 16.0,
            h: 60.0,
        };
        t.draw_run(
            &mut c,
            "HH",
            FontId::Bold,
            20.0,
            10.0,
            40.0,
            0.0,
            WHITE,
            1.0,
            Some(clip),
        );
        assert!(lit_bbox(&c).unwrap().2 <= 15, "clipped at x = 16");
        t.draw_run(
            &mut c,
            "HH",
            FontId::Medium,
            20.0,
            10.0,
            40.0,
            0.0,
            WHITE,
            0.0,
            None,
        );
    }

    fn lit_bbox(p: &Pixmap) -> Option<(u32, u32, u32, u32)> {
        let w = p.width();
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..p.height() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if p.data()[i] > 40 {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        b
    }

    #[test]
    fn text_is_centred() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(120, 60).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_centered(&mut p, "42%", 15.0, 60.0, 30.0, WHITE, 1.0);
        let (x0, y0, x1, y1) = lit_bbox(&p).expect("something drawn");
        let cx = (x0 + x1) as f32 / 2.0;
        let cy = (y0 + y1) as f32 / 2.0;
        assert!((cx - 60.0).abs() <= 1.5, "cx {cx}");
        assert!((cy - 30.0).abs() <= 1.5, "cy {cy}");
        assert!(y1 - y0 >= 9 && y1 - y0 <= 14, "height {}", y1 - y0);
    }

    #[test]
    fn measure_grows_with_text() {
        let mut t = TextRenderer::new().unwrap();
        let (w1, _) = t.measure("4", 15.0);
        let (w3, _) = t.measure("444", 15.0);
        assert!(w3 > w1 * 2.5);
    }

    #[test]
    fn zero_alpha_draws_nothing() {
        let mut t = TextRenderer::new().unwrap();
        let mut p = Pixmap::new(50, 50).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        t.draw_centered(&mut p, "9", 15.0, 25.0, 25.0, WHITE, 0.0);
        assert!(lit_bbox(&p).is_none());
    }
}

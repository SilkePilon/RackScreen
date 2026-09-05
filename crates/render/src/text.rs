//! Glyph rasterizing with fontdue and centred text placement.

use std::collections::HashMap;

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings, Metrics};
use rackscreen_core::theme::Color;
use tiny_skia::Pixmap;

use crate::assets::FONT_BOLD;

pub struct TextRenderer {
    font: Font,
    cache: HashMap<(char, u32), (Metrics, Vec<u8>)>,
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
        Ok(Self { font, cache: HashMap::new() })
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
            glyphs.push(Glyph { x: gx, top: gtop, metrics: m, bitmap });
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
    pub fn draw_centered(&mut self, pix: &mut Pixmap, text: &str, px: f32, cx: f32, cy: f32, color: Color, alpha: f32) {
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
    use rackscreen_core::theme::WHITE;

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

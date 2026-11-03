//! Album art: decode a JPEG or PNG, crop it square and scale it to the
//! screen's cover size, and keep the last few in memory so an album is
//! downloaded once.

use anyhow::{bail, Context, Result};
use rackscreen_core::event::{Cover, CoverRef};
use rackscreen_core::music::COVER_PX;

/// Covers kept in memory, newest last.
pub const CACHE_LEN: usize = 8;

/// Decode JPEG or PNG bytes to `(width, height, straight RGBA)`.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        decode_jpeg(bytes)
    } else if bytes.starts_with(b"\x89PNG") {
        decode_png(bytes)
    } else {
        bail!("cover is neither JPEG nor PNG")
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), opts);
    let rgba = d.decode().map_err(|e| anyhow::anyhow!("jpeg: {e:?}"))?;
    let (w, h) = d.dimensions().context("jpeg: no dimensions")?;
    anyhow::ensure!(rgba.len() == w * h * 4, "jpeg: unexpected output size");
    Ok((w as u32, h as u32, rgba))
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().context("png header")?;
    let mut buf = vec![0; r.output_buffer_size().context("png: too large")?];
    let info = r.next_frame(&mut buf).context("png frame")?;
    let (w, h) = (info.width, info.height);
    let px = (w * h) as usize;
    let src = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => src
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => bail!("png: palette not expanded"),
    };
    anyhow::ensure!(rgba.len() == px * 4, "png: unexpected output size");
    Ok((w, h, rgba))
}

/// Centre-crop to a square and box-filter it down (or sample it up) to
/// `out × out`.
pub fn square(w: u32, h: u32, rgba: &[u8], out: u32) -> Vec<u8> {
    let side = w.min(h).max(1);
    let (x0, y0) = ((w - side.min(w)) / 2, (h - side.min(h)) / 2);
    let mut dst = Vec::with_capacity((out * out * 4) as usize);
    for oy in 0..out {
        let sy0 = y0 + oy * side / out;
        let sy1 = (y0 + (oy + 1) * side / out).max(sy0 + 1).min(h);
        for ox in 0..out {
            let sx0 = x0 + ox * side / out;
            let sx1 = (x0 + (ox + 1) * side / out).max(sx0 + 1).min(w);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[i + c] as u32;
                    }
                    n += 1;
                }
            }
            for a in acc {
                dst.push(((a + n / 2) / n.max(1)) as u8);
            }
        }
    }
    dst
}

/// Decode and square downloaded art into a `COVER_PX` cover.
pub fn cover_from_bytes(key: &str, bytes: &[u8]) -> Result<Cover> {
    let (w, h, rgba) = decode(bytes)?;
    Ok(Cover {
        key: key.to_string(),
        size: COVER_PX,
        rgba: square(w, h, &rgba, COVER_PX),
    })
}

/// The last `CACHE_LEN` covers by key, least recently used first.
#[derive(Default)]
pub struct CoverCache {
    items: Vec<CoverRef>,
}

impl CoverCache {
    pub fn get(&mut self, key: &str) -> Option<CoverRef> {
        let i = self.items.iter().position(|c| c.0.key == key)?;
        let hit = self.items.remove(i);
        self.items.push(hit.clone());
        Some(hit)
    }

    pub fn insert(&mut self, cover: Cover) -> CoverRef {
        self.items.retain(|c| c.0.key != cover.key);
        if self.items.len() >= CACHE_LEN {
            self.items.remove(0);
        }
        let r = CoverRef::new(cover);
        self.items.push(r.clone());
        r
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The cover for `key`: from memory, else downloaded with `req` once.
    /// A failed download is logged and gives `None` (retried next poll).
    pub async fn fetch(&mut self, key: &str, req: reqwest::RequestBuilder) -> Option<CoverRef> {
        if let Some(c) = self.get(key) {
            return Some(c);
        }
        let bytes = match req.send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => r.bytes().await.ok()?,
            Err(e) => {
                tracing::debug!("cover {key}: {e}");
                return None;
            }
        };
        match cover_from_bytes(key, &bytes) {
            Ok(c) => Some(self.insert(c)),
            Err(e) => {
                tracing::debug!("cover {key}: {e:#}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JPEG: &[u8] = include_bytes!("../tests/fixtures/cover-200x150.jpg");

    fn png_bytes(w: u32, h: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(color);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(data).unwrap();
        wr.finish().unwrap();
        out
    }

    #[test]
    fn jpeg_decodes_to_rgba() {
        let (w, h, rgba) = decode(JPEG).unwrap();
        assert_eq!((w, h), (200, 150));
        assert_eq!(rgba.len(), 200 * 150 * 4);
        let at = |x: u32, y: u32| &rgba[((y * w + x) * 4) as usize..][..4];
        assert!(at(20, 75)[0] > 150 && at(20, 75)[2] < 90, "red left");
        assert!(at(180, 75)[2] > 150 && at(180, 75)[0] < 90, "blue right");
        assert_eq!(at(20, 75)[3], 255);
    }

    #[test]
    fn png_rgb_and_gray_decode_to_rgba() {
        let rgb = png_bytes(2, 1, png::ColorType::Rgb, &[10, 20, 30, 40, 50, 60]);
        assert_eq!(
            decode(&rgb).unwrap(),
            (2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])
        );
        let gray = png_bytes(1, 1, png::ColorType::Grayscale, &[77]);
        assert_eq!(decode(&gray).unwrap(), (1, 1, vec![77, 77, 77, 255]));
        assert!(decode(b"GIF89a").is_err());
    }

    #[test]
    fn square_crops_the_middle_and_averages() {
        // 4 × 2: two black, then a white/grey pair, cropped to the middle 2 × 2
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for v in [0u8, 255, 100, 0] {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let one = square(4, 2, &rgba, 1);
        assert_eq!(one, vec![178, 178, 178, 255], "mean of 255 and 100");
        let two = square(4, 2, &rgba, 2);
        assert_eq!(&two[..4], &[255, 255, 255, 255]);
        assert_eq!(&two[4..8], &[100, 100, 100, 255]);
        let up = square(1, 1, &[9, 8, 7, 255], 3);
        assert_eq!(up.len(), 36);
        assert!(up.chunks(4).all(|p| p == [9, 8, 7, 255]));
    }

    #[test]
    fn jpeg_cover_is_84_square_with_both_halves() {
        let c = cover_from_bytes("album", JPEG).unwrap();
        assert_eq!((c.key.as_str(), c.size), ("album", 84));
        assert_eq!(c.rgba.len(), 84 * 84 * 4);
        let at = |x: u32| &c.rgba[((42 * 84 + x) * 4) as usize..][..3];
        assert!(at(5)[0] > 150, "red on the left of the crop");
        assert!(at(78)[2] > 150, "blue on the right of the crop");
    }

    #[test]
    fn cache_keeps_the_last_eight_by_use() {
        let mut cache = CoverCache::default();
        let cover = |k: &str| Cover {
            key: k.into(),
            size: 1,
            rgba: vec![0, 0, 0, 255],
        };
        for i in 0..8 {
            cache.insert(cover(&format!("a{i}")));
        }
        assert!(cache.get("a0").is_some(), "a0 is now the newest");
        cache.insert(cover("a8"));
        assert_eq!(cache.len(), 8);
        assert!(cache.get("a1").is_none(), "the least recently used went");
        assert!(cache.get("a0").is_some() && cache.get("a8").is_some());
        cache.insert(cover("a8"));
        assert_eq!(cache.len(), 8, "re-inserting a key replaces it");
    }
}

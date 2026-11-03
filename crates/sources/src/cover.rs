//! Album art: download it (capped), decode a JPEG or PNG, crop it square
//! and scale it to the screen's cover size, and keep the last few in memory
//! so an album is downloaded once.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use rackscreen_core::event::{Cover, CoverRef};
use rackscreen_core::music::COVER_PX;

use crate::MAX_IMAGE_PX;

/// Covers kept in memory, newest last.
pub const CACHE_LEN: usize = 8;
/// A cover download stops here; a 640 px album JPEG is about 100 kB.
pub const MAX_COVER_BYTES: usize = 2 * 1024 * 1024;

fn jpeg_decoder(bytes: &[u8]) -> zune_jpeg::JpegDecoder<std::io::Cursor<&[u8]>> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    // strict: a truncated or damaged file is an error, not a grey half
    let opts = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_strict_mode(true);
    zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), opts)
}

/// Width and height from the image header, without decoding any pixels.
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut d = jpeg_decoder(bytes);
        d.decode_headers().map_err(|e| anyhow!("jpeg: {e:?}"))?;
        let (w, h) = d.dimensions().context("jpeg: no dimensions")?;
        Ok((w as u32, h as u32))
    } else if bytes.starts_with(b"\x89PNG") {
        let r = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .context("png header")?;
        let info = r.info();
        Ok((info.width, info.height))
    } else {
        bail!("cover is neither JPEG nor PNG")
    }
}

/// Decode JPEG or PNG bytes to `(width, height, straight RGBA)`. An image
/// over `MAX_IMAGE_PX` is refused from its header.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let (w, h) = dimensions(bytes)?;
    anyhow::ensure!(w > 0 && h > 0, "cover is empty");
    anyhow::ensure!(
        w as u64 * h as u64 <= MAX_IMAGE_PX,
        "cover {w}x{h} is too large"
    );
    if bytes.starts_with(&[0xFF, 0xD8]) {
        decode_jpeg(bytes)
    } else {
        decode_png(bytes)
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut d = jpeg_decoder(bytes);
    let rgba = d.decode().map_err(|e| anyhow!("jpeg: {e:?}"))?;
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

    /// The cover for `key`: from memory, else downloaded with `req` once
    /// (at most `MAX_COVER_BYTES`) and decoded off the async threads. A
    /// failed download or decode is logged and gives `None`.
    pub async fn fetch(&mut self, key: &str, req: reqwest::RequestBuilder) -> Option<CoverRef> {
        if let Some(c) = self.get(key) {
            return Some(c);
        }
        let got = async {
            let bytes = download(req, MAX_COVER_BYTES).await?;
            let k = key.to_string();
            crate::http::blocking(move || cover_from_bytes(&k, &bytes)).await?
        }
        .await;
        match got {
            Ok(c) => Some(self.insert(c)),
            Err(e) => {
                tracing::debug!("cover {key}: {e:#}");
                None
            }
        }
    }
}

/// Send `req` and read at most `max` bytes of a successful answer.
pub async fn download(req: reqwest::RequestBuilder, max: usize) -> Result<Vec<u8>> {
    let resp = req
        .send()
        .await
        .context("cover request")?
        .error_for_status()
        .context("cover request")?;
    let what = resp.url().to_string();
    crate::http::read_capped(resp, max, &what).await
}

/// A request for a Spotify album image: https to a public host only, and
/// every redirect checked the same way (`gif::check_url` rules).
pub fn spotify_request(url: &str) -> Result<reqwest::RequestBuilder> {
    let parsed = reqwest::Url::parse(url).with_context(|| format!("bad cover link {url}"))?;
    crate::gif::check_url(&parsed).map_err(|e| anyhow!(e))?;
    Ok(cover_client().get(parsed))
}

/// The client for Spotify covers: the shared one's user agent and timeout,
/// with redirects held to `gif::redirect_check`.
fn cover_client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    match crate::gif::redirect_check(attempt.url(), attempt.previous().len()) {
                        Ok(()) => attempt.follow(),
                        Err(e) => attempt.error(e),
                    }
                }))
                .build()
                .expect("reqwest client")
        })
        .clone()
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

    const TRUNCATED_JPEG: &[u8] = include_bytes!("../tests/fixtures/cover-truncated.jpg");
    const TRUNCATED_PNG: &[u8] = include_bytes!("../tests/fixtures/cover-truncated.png");
    const GARBAGE: &[u8] = include_bytes!("../tests/fixtures/cover-garbage.bin");

    #[test]
    fn truncated_and_garbage_covers_are_errors() {
        for (name, bytes) in [
            ("truncated jpeg", TRUNCATED_JPEG),
            ("truncated png", TRUNCATED_PNG),
            ("garbage", GARBAGE),
            ("empty", &[][..]),
            ("jpeg magic only", &[0xFF, 0xD8][..]),
            ("png magic only", &b"\x89PNG"[..]),
        ] {
            assert!(decode(bytes).is_err(), "{name}");
            assert!(cover_from_bytes("k", bytes).is_err(), "{name}");
        }
    }

    /// The fixture JPEG with its frame header claiming `w × h`.
    fn jpeg_claiming(w: u16, h: u16) -> Vec<u8> {
        let mut b = JPEG.to_vec();
        let sof = b
            .windows(2)
            .position(|m| m[0] == 0xFF && (0xC0..=0xC2).contains(&m[1]))
            .expect("a frame header");
        b[sof + 5..sof + 7].copy_from_slice(&h.to_be_bytes());
        b[sof + 7..sof + 9].copy_from_slice(&w.to_be_bytes());
        b
    }

    #[test]
    fn header_sizes_are_read_and_big_images_refused_before_decoding() {
        assert_eq!(dimensions(JPEG).unwrap(), (200, 150));
        let gray = png_bytes(3, 2, png::ColorType::Grayscale, &[0; 6]);
        assert_eq!(dimensions(&gray).unwrap(), (3, 2));
        let big = jpeg_claiming(4000, 2000);
        assert_eq!(dimensions(&big).unwrap(), (4000, 2000));
        let e = decode(&big).unwrap_err().to_string();
        assert!(e.contains("too large"), "{e}");
        let big = png_bytes(2001, 2000, png::ColorType::Grayscale, &vec![0; 2001 * 2000]);
        let e = decode(&big).unwrap_err().to_string();
        assert!(e.contains("too large"), "{e}");
        assert!(decode(&png_bytes(
            2000,
            2000,
            png::ColorType::Grayscale,
            &vec![0; 4_000_000]
        ))
        .is_ok());
        assert_eq!(crate::MAX_IMAGE_PX, 4_000_000);
        assert_eq!(crate::gif::MAX_CANVAS_PX, crate::MAX_IMAGE_PX, "one limit");
    }

    #[test]
    fn spotify_cover_links_must_be_https_on_a_public_host() {
        assert!(spotify_request("https://i.scdn.co/image/ab67616d").is_ok());
        for bad in [
            "http://i.scdn.co/image/ab67616d",
            "https://127.0.0.1/x.jpg",
            "https://192.168.1.5/x.jpg",
            "https://localhost/x.jpg",
            "not a url",
        ] {
            assert!(spotify_request(bad).is_err(), "{bad}");
        }
    }

    /// A one-route HTTP server on 127.0.0.1 that answers every request with
    /// `body` (chunked when asked) and counts the requests.
    async fn serve(body: Vec<u8>, chunked: bool) -> (String, Arc<AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/cover", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                count.fetch_add(1, Ordering::SeqCst);
                let body = body.clone();
                tokio::spawn(async move {
                    let mut req = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => req.extend_from_slice(&buf[..n]),
                        }
                    }
                    let head = if chunked {
                        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                            .to_string()
                    } else {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                    };
                    if sock.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    if chunked {
                        for part in body.chunks(64 * 1024) {
                            let size = format!("{:x}\r\n", part.len());
                            let ok = sock.write_all(size.as_bytes()).await.is_ok()
                                && sock.write_all(part).await.is_ok()
                                && sock.write_all(b"\r\n").await.is_ok();
                            if !ok {
                                return;
                            }
                        }
                        let _ = sock.write_all(b"0\r\n\r\n").await;
                    } else {
                        let _ = sock.write_all(&body).await;
                    }
                });
            }
        });
        (url, hits)
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn downloads_stop_at_two_megabytes() {
        let big = vec![0u8; MAX_COVER_BYTES + 1];
        for chunked in [false, true] {
            let (url, _) = serve(big.clone(), chunked).await;
            let e = download(crate::http::client().get(&url), MAX_COVER_BYTES)
                .await
                .unwrap_err();
            assert!(
                format!("{e:#}").contains("larger than"),
                "chunked {chunked}: {e:#}"
            );
        }
        let (url, _) = serve(JPEG.to_vec(), true).await;
        let got = download(crate::http::client().get(&url), MAX_COVER_BYTES)
            .await
            .unwrap();
        assert_eq!(got, JPEG);
        let mut cache = CoverCache::default();
        let c = cache.fetch("ok", crate::http::client().get(&url)).await;
        assert_eq!(c.unwrap().0.size, COVER_PX, "decoded off the runtime");
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

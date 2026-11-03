//! GIFs for the `gif:<name>` role: resolve a pasted GIPHY/Tenor/`.gif` link,
//! download it once into the cache, decode it into panel-sized frames.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use rackscreen_core::event::Event;
use rackscreen_core::gif::{
    cap_frames, classify_link, delay_ms, keep_mask, GifFrames, GifLink, GifRef, GIF_SIDE,
    MAX_FRAMES,
};

use crate::http::client;
use crate::SourceCtx;

/// Downloads stop here.
pub const MAX_DOWNLOAD_BYTES: usize = 20 * 1024 * 1024;
/// A Tenor page is read up to this much.
const MAX_PAGE_BYTES: usize = 4 * 1024 * 1024;
/// GIFs with a larger canvas are refused: the canvas alone would be 16 MB.
pub const MAX_CANVAS_PX: u64 = 4_000_000;
/// A failed GIF is tried again after this long.
pub const RETRY: Duration = Duration::from_secs(3600);
/// One download may take this long (the shared client's 10 s is for APIs).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// One GIF a screen uses: its index in the config's `gifs:` list, its name
/// (for the log) and the pasted link.
#[derive(Clone, Debug, PartialEq)]
pub struct GifSpec {
    pub index: u16,
    pub name: String,
    pub url: String,
}

/// The `.gif` URL in a Tenor page: the `og:image` meta tag, else the first
/// JSON-LD `contentUrl`, whichever ends in `.gif` first.
pub fn tenor_gif_url(html: &str) -> Option<String> {
    let og = html
        .split('<')
        .filter(|tag| tag.starts_with("meta") && tag.contains("\"og:image\""))
        .filter_map(|tag| attr(tag, "content"));
    let ld = html.split("\"contentUrl\"").skip(1).filter_map(|rest| {
        let rest = rest.trim_start().strip_prefix(':')?.trim_start();
        let rest = rest.strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_string())
    });
    og.chain(ld).map(|u| unescape(&u)).find(|u| {
        u.starts_with("https://")
            && u.split(['?', '#'])
                .next()
                .is_some_and(|p| p.to_ascii_lowercase().ends_with(".gif"))
    })
}

/// The value of `name="..."` in one HTML tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let len = tag[start..].find('"')?;
    Some(tag[start..start + len].to_string())
}

/// The escapes Tenor uses in URLs: JSON `/` and `\/`, HTML `&amp;`.
fn unescape(s: &str) -> String {
    s.replace("\\u002F", "/")
        .replace("\\u002f", "/")
        .replace("\\/", "/")
        .replace("&amp;", "&")
}

/// Where the raw download of `url` is cached: `<sha256 of the link>.gif`.
pub fn cache_path(dir: &Path, url: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(url.trim().as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{hex}.gif"))
}

/// Decode a GIF into frames that cover the panel (scale to fill, centre
/// crop), composed with their disposal modes, at most `MAX_FRAMES` of them.
/// Two passes: the first only reads the frame delays, so a long GIF never
/// holds more than the frames it keeps.
pub fn decode(bytes: &[u8]) -> Result<GifFrames> {
    if !bytes.starts_with(b"GIF8") {
        bail!("not a GIF");
    }
    // pass 1: delays only
    let mut opts = gif::DecodeOptions::new();
    opts.skip_frame_decoding(true);
    let mut dec = opts.read_info(Cursor::new(bytes)).context("read GIF")?;
    let (w, h) = (dec.width() as u32, dec.height() as u32);
    anyhow::ensure!(w > 0 && h > 0, "empty GIF canvas");
    anyhow::ensure!(
        (w as u64) * (h as u64) <= MAX_CANVAS_PX,
        "GIF canvas {w}x{h} is too large"
    );
    let mut delays = Vec::new();
    loop {
        match dec.read_next_frame() {
            Ok(Some(f)) => delays.push(delay_ms(f.delay)),
            Ok(None) => break,
            // A truncated GIF plays the frames before the damage.
            Err(e) if !delays.is_empty() => {
                tracing::debug!("gif: stopped after {} frames: {e}", delays.len());
                break;
            }
            Err(e) => return Err(e).context("read GIF frame"),
        }
    }
    anyhow::ensure!(!delays.is_empty(), "GIF has no frames");
    let keep = keep_mask(delays.len(), MAX_FRAMES);
    let kept_delays: Vec<u32> = cap_frames(delays.iter().map(|d| ((), *d)).collect(), MAX_FRAMES)
        .into_iter()
        .map(|(_, d)| d)
        .collect();

    // pass 2: compose every frame, scale the kept ones
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut dec = opts.read_info(Cursor::new(bytes)).context("read GIF")?;
    let mut canvas = vec![0u8; (w * h * 4) as usize];
    let mut saved: Option<Vec<u8>> = None;
    let mut prev: Option<(Rect, gif::DisposalMethod)> = None;
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(kept_delays.len());
    for keep_this in keep.iter().copied() {
        let f = match dec.read_next_frame() {
            Ok(Some(f)) => f,
            Ok(None) | Err(_) => break,
        };
        if let Some((r, d)) = prev.take() {
            match d {
                gif::DisposalMethod::Background => clear(&mut canvas, w, h, r),
                gif::DisposalMethod::Previous => {
                    if let Some(s) = saved.take() {
                        canvas = s;
                    }
                }
                _ => {}
            }
        }
        let rect = Rect {
            x: f.left as u32,
            y: f.top as u32,
            w: f.width as u32,
            h: f.height as u32,
        };
        if f.dispose == gif::DisposalMethod::Previous {
            saved = Some(canvas.clone());
        }
        draw(&mut canvas, w, h, rect, &f.buffer);
        prev = Some((rect, f.dispose));
        if keep_this {
            out.push(cover(&canvas, w, h));
        }
    }
    anyhow::ensure!(!out.is_empty(), "GIF has no frames");
    GifFrames::new(out.into_iter().zip(kept_delays).collect()).map_err(|e| anyhow!(e))
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Clear a frame's rectangle to transparent (disposal "restore to background").
fn clear(canvas: &mut [u8], cw: u32, ch: u32, r: Rect) {
    for y in r.y..(r.y + r.h).min(ch) {
        for x in r.x..(r.x + r.w).min(cw) {
            let o = ((y * cw + x) * 4) as usize;
            canvas[o..o + 4].fill(0);
        }
    }
}

/// Draw a frame's RGBA pixels onto the canvas; transparent pixels leave the
/// canvas as it is, parts outside the canvas are cut off.
fn draw(canvas: &mut [u8], cw: u32, ch: u32, r: Rect, rgba: &[u8]) {
    for fy in 0..r.h {
        let y = r.y + fy;
        if y >= ch {
            break;
        }
        for fx in 0..r.w {
            let x = r.x + fx;
            if x >= cw {
                break;
            }
            let s = ((fy * r.w + fx) * 4) as usize;
            let Some(p) = rgba.get(s..s + 4) else { return };
            if p[3] != 0 {
                let o = ((y * cw + x) * 4) as usize;
                canvas[o..o + 4].copy_from_slice(p);
            }
        }
    }
}

/// For each output pixel along one axis, the source pixels it covers when
/// `src` pixels are scaled by `scale` and centred on `GIF_SIDE`: a box
/// filter when shrinking, the nearest pixel when growing.
fn spans(src: u32, scale: f64) -> Vec<(u32, u32)> {
    let off = (src as f64 - GIF_SIDE as f64 / scale) / 2.0;
    (0..GIF_SIDE)
        .map(|o| {
            let a = off + o as f64 / scale;
            let b = off + (o + 1) as f64 / scale;
            let (lo, hi) = if b - a < 1.0 {
                let c = ((a + b) / 2.0).floor();
                (c, c + 1.0)
            } else {
                ((a + 1e-6).floor(), (b - 1e-6).ceil())
            };
            let lo = (lo.max(0.0) as u32).min(src - 1);
            let hi = (hi as u32).clamp(lo + 1, src);
            (lo, hi)
        })
        .collect()
}

/// Scale the straight-RGBA canvas to cover the panel (scale to fill, centre
/// crop) and composite it over black: opaque, so premultiplied equals straight.
fn cover(canvas: &[u8], w: u32, h: u32) -> Vec<u8> {
    let scale = (GIF_SIDE as f64 / w as f64).max(GIF_SIDE as f64 / h as f64);
    let xs = spans(w, scale);
    let ys = spans(h, scale);
    let mut out = vec![0u8; (GIF_SIDE * GIF_SIDE * 4) as usize];
    for (oy, &(y0, y1)) in ys.iter().enumerate() {
        for (ox, &(x0, x1)) in xs.iter().enumerate() {
            let mut acc = [0u32; 3];
            let n = (y1 - y0) * (x1 - x0);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &canvas[((y * w + x) * 4) as usize..][..4];
                    let a = p[3] as u32;
                    for k in 0..3 {
                        acc[k] += p[k] as u32 * a;
                    }
                }
            }
            let o = (oy * GIF_SIDE as usize + ox) * 4;
            for k in 0..3 {
                out[o + k] = ((acc[k] + n * 255 / 2) / (n * 255)) as u8;
            }
            out[o + 3] = 255;
        }
    }
    out
}

/// Fetch `url` asking for `accept`, refusing anything larger than `max` bytes.
async fn fetch_capped(url: &str, accept: &str, max: usize) -> Result<Vec<u8>> {
    let mut resp = client()
        .get(url)
        .header(reqwest::header::ACCEPT, accept)
        .timeout(DOWNLOAD_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url}"))?;
    if resp.content_length().is_some_and(|n| n as usize > max) {
        bail!("{url} is larger than {} MB", max / (1024 * 1024));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.with_context(|| format!("read {url}"))? {
        body.extend_from_slice(&chunk);
        if body.len() > max {
            bail!("{url} is larger than {} MB", max / (1024 * 1024));
        }
    }
    Ok(body)
}

/// The `.gif` URL behind a pasted link; a Tenor page is fetched for it.
async fn resolve(link: GifLink) -> Result<String> {
    match link {
        GifLink::Direct(u) => Ok(u),
        GifLink::TenorPage(page) => {
            let html = fetch_capped(&page, "text/html", MAX_PAGE_BYTES).await?;
            tenor_gif_url(&String::from_utf8_lossy(&html))
                .ok_or_else(|| anyhow!("no .gif in the Tenor page {page}"))
        }
    }
}

/// The GIF bytes for a pasted link: the cached file when there is one, else
/// resolve, download, check it is a GIF and cache it.
pub async fn load(url: &str, cache_dir: &Path) -> Result<Vec<u8>> {
    let path = cache_path(cache_dir, url);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let link = classify_link(url).map_err(|e| anyhow!(e))?;
    let gif_url = resolve(link).await?;
    let bytes = fetch_capped(&gif_url, "image/gif", MAX_DOWNLOAD_BYTES).await?;
    if !bytes.starts_with(b"GIF8") {
        bail!("{gif_url} is not a GIF");
    }
    if let Err(e) = write_cache(&path, &bytes) {
        tracing::warn!("gif cache {}: {e:#}", path.display());
    }
    Ok(bytes)
}

fn write_cache(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let tmp = path.with_extension("gif.tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename {}", tmp.display()))
}

/// Fetch and decode every GIF the screens use, one task each. A GIF that
/// fails is reported (`Event::GifFailed`) and tried again every `RETRY`.
/// Decoding runs one GIF at a time, so the memory peak stays one GIF's.
pub async fn run_gifs(gifs: Vec<GifSpec>, cache_dir: PathBuf, ctx: SourceCtx) {
    let decode_lock = Arc::new(tokio::sync::Semaphore::new(1));
    let tasks: Vec<_> = gifs
        .into_iter()
        .map(|spec| {
            let ctx = ctx.clone();
            let dir = cache_dir.clone();
            let lock = decode_lock.clone();
            tokio::spawn(async move { run_one(spec, dir, lock, ctx).await })
        })
        .collect();
    for t in tasks {
        let _ = t.await;
    }
}

async fn run_one(
    spec: GifSpec,
    cache_dir: PathBuf,
    lock: Arc<tokio::sync::Semaphore>,
    ctx: SourceCtx,
) {
    loop {
        match load_and_decode(&spec, &cache_dir, &lock).await {
            Ok(frames) => {
                tracing::info!(
                    "gif:{}: {} frames, {:.1} s loop",
                    spec.name,
                    frames.len(),
                    frames.total_ms() as f64 / 1000.0
                );
                ctx.emit(Event::Gif {
                    index: spec.index,
                    frames: GifRef(Arc::new(frames)),
                });
                return;
            }
            Err(e) => {
                tracing::warn!("gif:{}: {e:#}; retrying in an hour", spec.name);
                ctx.emit(Event::GifFailed {
                    index: spec.index,
                    reason: format!("{e:#}"),
                });
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(RETRY) => {}
        }
    }
}

async fn load_and_decode(
    spec: &GifSpec,
    cache_dir: &Path,
    lock: &tokio::sync::Semaphore,
) -> Result<GifFrames> {
    let bytes = load(&spec.url, cache_dir).await?;
    let _permit = lock.acquire().await.context("decode lock")?;
    let result = tokio::task::spawn_blocking(move || decode(&bytes))
        .await
        .context("decode task")?;
    if result.is_err() {
        // A broken cached file is fetched again on the next try.
        let _ = std::fs::remove_file(cache_path(cache_dir, &spec.url));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::gif::FRAME_BYTES;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }

    fn px(frames: &GifFrames, i: usize, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * GIF_SIDE + x) * 4) as usize;
        frames.frame(i)[o..o + 4].try_into().unwrap()
    }

    fn near(a: [u8; 4], b: [u8; 3]) -> bool {
        a[3] == 255 && (0..3).all(|k| (a[k] as i32 - b[k] as i32).abs() <= 12)
    }

    /// `(left, top, w, h, rgb, delay cs, dispose)`.
    type TestFrame = (u16, u16, u16, u16, [u8; 3], u16, gif::DisposalMethod);

    /// Encode a GIF of single-colour frames.
    fn encode(w: u16, h: u16, frames: &[TestFrame]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, w, h, &[]).unwrap();
            for &(left, top, fw, fh, rgb, delay, dispose) in frames {
                let pixels: Vec<u8> = (0..fw as usize * fh as usize).flat_map(|_| rgb).collect();
                let mut f = gif::Frame::from_rgb(fw, fh, &pixels);
                f.left = left;
                f.top = top;
                f.delay = delay;
                f.dispose = dispose;
                enc.write_frame(&f).unwrap();
            }
        }
        out
    }

    #[test]
    fn tenor_page_gives_the_og_image_gif() {
        let html = String::from_utf8(fixture("tenor-view.html")).unwrap();
        assert_eq!(
            tenor_gif_url(&html).as_deref(),
            Some("https://media.tenor.com/BBBBBBBBBBB/test-dance.gif")
        );
    }

    #[test]
    fn tenor_page_without_og_gif_falls_back_to_json_ld() {
        let html = String::from_utf8(fixture("tenor-view.html")).unwrap();
        let html = html.replace(
            "test-dance.gif\" property=\"og:image\"",
            "x.png\" property=\"og:image\"",
        );
        assert_eq!(
            tenor_gif_url(&html).as_deref(),
            Some("https://media.tenor.com/DDDDDDDDDDD/test-dance.gif"),
            "the first contentUrl ending in .gif, unescaped"
        );
        assert_eq!(tenor_gif_url("<html><head></head></html>"), None);
        assert_eq!(
            tenor_gif_url(
                r#"<meta property="og:image" content="https://a.example/x.gif?a=1&amp;b=2">"#
            )
            .as_deref(),
            Some("https://a.example/x.gif?a=1&b=2")
        );
    }

    #[test]
    fn cache_path_is_the_sha256_of_the_link() {
        let dir = Path::new("/c");
        let a = cache_path(dir, "https://giphy.com/gifs/x-AbC");
        assert_eq!(a.parent(), Some(dir));
        let name = a.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with(".gif"));
        assert_eq!(name.len(), 64 + 4);
        assert_eq!(a, cache_path(dir, "  https://giphy.com/gifs/x-AbC "));
        assert_ne!(a, cache_path(dir, "https://giphy.com/gifs/x-AbD"));
        assert_eq!(
            cache_path(dir, "abc"),
            dir.join("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.gif")
        );
    }

    #[test]
    fn fixture_decodes_to_cover_cropped_frames_with_native_timing() {
        let g = decode(&fixture("gif-fixture.gif")).unwrap();
        assert_eq!(g.len(), 3);
        assert_eq!(
            (0..3).map(|i| g.delay_ms(i)).collect::<Vec<_>>(),
            vec![100, 100, 500],
            "the 0 ms frame plays at 100 ms"
        );
        for i in 0..3 {
            assert_eq!(g.frame(i).len(), FRAME_BYTES);
            assert!(g.frame(i).chunks(4).all(|p| p[3] == 255), "opaque");
        }
        // 80x60 scales by 4 to 320x240; 40 px go each side, with the red bars
        for y in [2, 120, 200] {
            assert!(
                near(px(&g, 0, 0, y), [40, 60, 160]),
                "{:?}",
                px(&g, 0, 0, y)
            );
            assert!(
                near(px(&g, 0, 239, y), [40, 60, 160]),
                "{:?}",
                px(&g, 0, 239, y)
            );
        }
        assert!(near(px(&g, 1, 120, 10), [30, 130, 110]));
    }

    #[test]
    fn frames_compose_with_their_disposal_modes() {
        use gif::DisposalMethod::{Background, Keep, Previous};
        let bytes = encode(
            4,
            4,
            &[
                (0, 0, 4, 4, [255, 0, 0], 10, Keep),
                (0, 0, 2, 2, [0, 255, 0], 10, Previous),
                (2, 2, 2, 2, [0, 0, 255], 10, Background),
                (0, 0, 1, 1, [255, 255, 255], 10, Keep),
            ],
        );
        let g = decode(&bytes).unwrap();
        assert_eq!(g.len(), 4);
        let (tl, br, tr) = ((10, 10), (230, 230), (230, 10));
        let at = |i: usize, (x, y): (u32, u32)| px(&g, i, x, y);
        assert!(near(at(1, tl), [0, 255, 0]));
        assert!(near(at(1, br), [255, 0, 0]));
        assert!(near(at(2, tl), [255, 0, 0]), "Previous restores the red");
        assert!(near(at(2, br), [0, 0, 255]));
        assert!(
            near(at(3, br), [0, 0, 0]),
            "Background clears to transparent: black"
        );
        assert!(near(at(3, tl), [255, 255, 255]));
        assert!(near(at(3, tr), [255, 0, 0]));
    }

    #[test]
    fn long_gifs_are_capped_while_decoding_and_keep_their_length() {
        use gif::DisposalMethod::Keep;
        let frames: Vec<_> = (0..160u16)
            .map(|i| (0, 0, 2, 2, [i as u8, 0, 0], 2, Keep))
            .collect();
        let g = decode(&encode(2, 2, &frames)).unwrap();
        assert_eq!(g.len(), 80);
        assert_eq!(g.delay_ms(0), 40);
        assert_eq!(g.total_ms(), 160 * 20);
        assert!(near(px(&g, 1, 100, 100), [2, 0, 0]), "every second frame");
    }

    #[test]
    fn junk_and_huge_canvases_are_refused() {
        assert!(decode(b"<html>not a gif</html>").is_err());
        assert!(decode(b"").is_err());
        let huge = encode(
            3000,
            2000,
            &[(0, 0, 1, 1, [1, 2, 3], 10, gif::DisposalMethod::Keep)],
        );
        let e = decode(&huge).unwrap_err().to_string();
        assert!(e.contains("3000x2000"), "{e}");
    }

    #[tokio::test]
    async fn a_cached_gif_needs_no_network() {
        let dir = tempfile_dir();
        let url = "https://unreachable.invalid/party.gif";
        let bytes = fixture("gif-fixture.gif");
        std::fs::write(cache_path(&dir, url), &bytes).unwrap();
        assert_eq!(load(url, &dir).await.unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_uncached_bad_link_fails_without_fetching() {
        let dir = tempfile_dir();
        assert!(load("https://example.com/x.png", &dir).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rackscreen-gif-test-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

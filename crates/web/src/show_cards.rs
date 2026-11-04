//! `/api/shows/{id}/card.png?frame=K`: a still of a show for its card in
//! the web UI. The show's world is drawn across four gapless panels at one
//! of a few key moments and cropped to a 2:1 window around the action, at
//! native resolution (the browser scales it up, pixelated).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use axum::extract::{Path, Query};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use rackscreen_core::shows::world::{Layout, WorldFrame, WORLD_H};
use rackscreen_core::shows::{self, ShowDef};
use serde::Deserialize;

use crate::ApiError;

/// Stills per show; `frame` picks one.
pub const FRAMES: usize = 4;
/// Card crop: 2:1, the full world height.
pub const CARD_W: u32 = 2 * WORLD_H;
pub const CARD_H: u32 = WORLD_H;

const LAYOUT: Layout = Layout { panels: 4, gap: 0 };

/// The moments, as fractions of the show's length, each still is taken at.
/// The first is the card's face, so it is the show's best moment.
fn moments(id: &str) -> [f64; FRAMES] {
    match id {
        "storm" => [0.5, 0.3, 0.55, 0.8],
        "day" => [0.45, 0.15, 0.7, 0.9],
        "paper-plane" => [0.6, 0.3, 0.2, 0.9],
        "space" => [0.65, 0.35, 0.5, 0.8],
        "fireworks" => [0.85, 0.65, 0.75, 0.35],
        "hyperspace" => [0.57, 0.25, 0.45, 0.66],
        "ufo" => [0.43, 0.2, 0.83, 0.65],
        "moon-landing" => [0.88, 0.48, 0.2, 0.7],
        _ => [0.5, 0.35, 0.65, 0.8],
    }
}

/// How busy the `CARD_W` window at `x0` is: edges between differing
/// neighbours, plus (weighted) pixels that move between `a` and `b`, since
/// a show's subject is what moves.
fn score(a: &WorldFrame, b: &WorldFrame, x0: u32) -> u64 {
    let mut s = 0;
    for y in 0..CARD_H {
        for x in x0..x0 + CARD_W {
            let p = a.at(x, y);
            if x + 1 < x0 + CARD_W && a.at(x + 1, y) != p {
                s += 1;
            }
            if y + 1 < CARD_H && a.at(x, y + 1) != p {
                s += 1;
            }
            if b.at(x, y) != p {
                s += 6;
            }
        }
    }
    s
}

/// The still of `def` number `k`: the busiest window of its world.
pub fn card_rgb(def: &ShowDef, k: usize) -> Vec<u8> {
    card_rgb_at(def, moments(def.id)[k])
}

/// The still of `def` at `frac` of its length.
fn card_rgb_at(def: &ShowDef, frac: f64) -> Vec<u8> {
    let t = def.secs * frac;
    let a = shows::render(def, LAYOUT, t);
    let b = shows::render(def, LAYOUT, t + 0.25);
    let max_x = a.width - CARD_W;
    let x0 = (0..=max_x)
        .step_by(4)
        .max_by_key(|&x| (score(&a, &b, x), std::cmp::Reverse(x.abs_diff(max_x / 2))))
        .unwrap_or(0);
    let mut rgb = Vec::with_capacity((CARD_W * CARD_H * 3) as usize);
    for y in 0..CARD_H {
        for x in x0..x0 + CARD_W {
            rgb.extend_from_slice(&a.at(x, y)[..3]);
        }
    }
    rgb
}

pub fn encode(rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, CARD_W, CARD_H);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(rgb).expect("png data");
    }
    out
}

type Cache = Mutex<HashMap<(&'static str, usize), Arc<Vec<u8>>>>;
static CACHE: LazyLock<Cache> = LazyLock::new(Default::default);

/// The PNG of still `k` of show `id`, encoded once.
pub fn card_png(id: &str, k: usize) -> Option<Arc<Vec<u8>>> {
    let def = shows::by_id(id)?;
    let k = k.min(FRAMES - 1);
    if let Some(png) = CACHE.lock().unwrap().get(&(def.id, k)) {
        return Some(png.clone());
    }
    let png = Arc::new(encode(&card_rgb(def, k)));
    CACHE.lock().unwrap().insert((def.id, k), png.clone());
    Some(png)
}

#[derive(Deserialize)]
pub struct CardQuery {
    #[serde(default)]
    frame: usize,
}

pub async fn card(
    Path(id): Path<String>,
    Query(q): Query<CardQuery>,
) -> Result<Response, ApiError> {
    let png = tokio::task::spawn_blocking(move || card_png(&id, q.frame))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("no such show"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        png.as_ref().clone(),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_show_has_busy_card_stills() {
        for def in shows::SHOWS {
            for k in 0..FRAMES {
                let png = card_png(def.id, k).unwrap();
                let mut dec = png::Decoder::new(std::io::Cursor::new(png.as_slice()))
                    .read_info()
                    .unwrap();
                let mut buf = vec![0; dec.output_buffer_size().unwrap()];
                let info = dec.next_frame(&mut buf).unwrap();
                assert_eq!((info.width, info.height), (CARD_W, CARD_H));
                let px: std::collections::HashSet<_> = buf.as_chunks::<3>().0.iter().collect();
                assert!(px.len() > 2, "{} still {k} is flat", def.id);
            }
        }
        assert!(card_png("nope", 0).is_none());
    }
}

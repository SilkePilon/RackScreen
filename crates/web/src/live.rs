//! `/api/live`: a WebSocket pushing each screen's latest frame as PNG, only
//! when it changed. Message = one byte screen index, then the PNG.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use rackscreen_app::hub::{FrameTap, DEFAULT_LIVE_FPS, MAX_LIVE_FPS};
use rackscreen_core::theme::layout::SIZE;

use crate::AppState;

/// `rgba` (SIZE×SIZE×4, opaque) as an RGB PNG with fast compression.
pub fn encode_png(rgba: &[u8]) -> Vec<u8> {
    let n = (SIZE * SIZE) as usize;
    let mut rgb = Vec::with_capacity(n * 3);
    for px in rgba.chunks_exact(4).take(n) {
        rgb.extend_from_slice(&px[..3]);
    }
    let mut out = Vec::with_capacity(32 * 1024);
    {
        let mut enc = png::Encoder::new(&mut out, SIZE, SIZE);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(&rgb).expect("png data");
    }
    out
}

/// The PNG of screen `i` at `seq`, encoded once and shared between clients.
async fn png_for(state: &Arc<AppState>, tap: &FrameTap, i: usize) -> Option<(u64, Arc<Vec<u8>>)> {
    let frame = tap.latest(i)?;
    if let Some(Some((seq, png))) = state.png_cache.lock().unwrap().get(i) {
        if *seq == frame.seq {
            return Some((*seq, png.clone()));
        }
    }
    let rgba = frame.rgba.clone();
    let png = tokio::task::spawn_blocking(move || encode_png(&rgba))
        .await
        .ok()?;
    let png = Arc::new(png);
    if let Some(slot) = state.png_cache.lock().unwrap().get_mut(i) {
        *slot = Some((frame.seq, png.clone()));
    }
    Some((frame.seq, png))
}

pub async fn live(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| session(socket, state))
}

fn parse_fps(text: &str) -> Option<u32> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let fps = v.get("fps")?.as_u64()?;
    Some((fps as u32).clamp(1, MAX_LIVE_FPS))
}

async fn session(mut socket: WebSocket, state: Arc<AppState>) {
    let tap = state.hub.frames.clone();
    let _guard = tap.watch();
    let mut fps = DEFAULT_LIVE_FPS;
    tap.set_fps(fps);
    let n = tap.screens();
    let mut sent = vec![0u64; n];
    let mut ticker = tokio::time::interval(Duration::from_millis(1000 / fps as u64));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Some(f) = parse_fps(&t) {
                            if f != fps {
                                fps = f;
                                tap.set_fps(fps);
                                ticker = tokio::time::interval(Duration::from_millis(1000 / fps as u64));
                                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            _ = ticker.tick() => {
                for (i, last) in sent.iter_mut().enumerate() {
                    let seq = tap.seq(i);
                    if seq == 0 || seq == *last {
                        continue;
                    }
                    let Some((seq, png)) = png_for(&state, &tap, i).await else {
                        continue;
                    };
                    let mut msg = Vec::with_capacity(png.len() + 1);
                    msg.push(i as u8);
                    msg.extend_from_slice(&png);
                    if socket.send(Message::Binary(msg.into())).await.is_err() {
                        return;
                    }
                    *last = seq;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_is_small_and_decodes_to_the_frame() {
        let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            let x = (i as u32 % SIZE) as u8;
            px[0] = x;
            px[1] = 40;
            px[2] = 200;
            px[3] = 255;
        }
        let png = encode_png(&rgba);
        assert!(png.len() < 20 * 1024, "{} bytes", png.len());
        let dec = png::Decoder::new(std::io::Cursor::new(&png));
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (SIZE, SIZE));
        assert_eq!(info.color_type, png::ColorType::Rgb);
        assert_eq!(&buf[..3], &[0, 40, 200]);
        assert_eq!(&buf[3 * 239..3 * 240], &[239, 40, 200]);
        assert_eq!(parse_fps(r#"{"fps": 5}"#), Some(5));
        assert_eq!(parse_fps(r#"{"fps": 99}"#), Some(15));
        assert_eq!(parse_fps("nope"), None);
    }
}

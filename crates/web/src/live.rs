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
    for px in rgba.as_chunks::<4>().0.iter().take(n) {
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

/// How often the server pings a live client.
pub const PING_EVERY: Duration = Duration::from_secs(15);
/// A client that sent nothing (not even a pong) for this long is gone.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// A send that does not finish in this long means the peer stopped reading.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

fn ticker_for(fps: u32) -> tokio::time::Interval {
    let mut t = tokio::time::interval(Duration::from_millis(1000 / fps as u64));
    t.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    t
}

async fn send_timed(socket: &mut WebSocket, msg: Message) -> bool {
    matches!(
        tokio::time::timeout(SEND_TIMEOUT, socket.send(msg)).await,
        Ok(Ok(()))
    )
}

async fn session(mut socket: WebSocket, state: Arc<AppState>) {
    let tap = state.hub.frames.clone();
    // dropped on every return: the tap stops copying once nobody watches
    let guard = tap.watch();
    let mut fps = DEFAULT_LIVE_FPS;
    let mut sent = vec![0u64; tap.screens()];
    let mut ticker = ticker_for(fps);
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_seen = tokio::time::Instant::now();
    loop {
        tokio::select! {
            msg = socket.recv() => {
                last_seen = tokio::time::Instant::now();
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Some(f) = parse_fps(&t) {
                            if f != fps {
                                fps = f;
                                guard.set_fps(fps);
                                ticker = ticker_for(fps);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() >= IDLE_TIMEOUT {
                    tracing::debug!("live: client silent for {IDLE_TIMEOUT:?}, closing");
                    break;
                }
                if !send_timed(&mut socket, Message::Ping(Vec::new().into())).await {
                    break;
                }
            }
            _ = ticker.tick() => {
                let active = tap.active();
                state.trim_png_cache(active);
                for (i, last) in sent.iter_mut().enumerate().take(active) {
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
                    if !send_timed(&mut socket, Message::Binary(msg.into())).await {
                        return;
                    }
                    *last = seq;
                }
            }
        }
    }
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Close(None))).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_is_small_and_decodes_to_the_frame() {
        let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
        for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
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

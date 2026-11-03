//! `/api/logs`: the ring after a sequence number, and a Server-Sent Events
//! stream of new lines.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use futures::StreamExt;
use rackscreen_app::logs::LogLine;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_stream::wrappers::BroadcastStream;

use crate::{ApiResult, AppState};

pub const MAX_LIMIT: usize = 2000;

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default)]
    pub after: u64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    500
}

pub fn line_json(l: &LogLine) -> Value {
    json!({
        "seq": l.seq,
        "unix_ms": l.unix_ms,
        "level": l.level,
        "target": l.target,
        "msg": l.msg,
    })
}

pub async fn logs(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LogsQuery>,
) -> ApiResult<Value> {
    let (lines, next) = state.hub.logs.after(q.after, q.limit.clamp(1, MAX_LIMIT));
    Ok(Json(json!({
        "lines": lines.iter().map(line_json).collect::<Vec<_>>(),
        "next": next,
    })))
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.hub.logs.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|r| async move {
        // a lagged receiver just skips what it missed
        let line = r.ok()?;
        Some(Ok(Event::default()
            .event("log")
            .data(line_json(&line).to_string())))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

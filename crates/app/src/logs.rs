//! An in-memory ring of structured log lines that tracing writes into, for
//! the web UI's log page: every line has a sequence number, so a client can
//! ask for "everything after N", and new lines are broadcast for streaming.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub seq: u64,
    pub unix_ms: u64,
    pub level: &'static str,
    pub target: String,
    pub msg: String,
}

struct Inner {
    lines: VecDeque<LogLine>,
    next_seq: u64,
}

#[derive(Clone)]
pub struct LogSink {
    inner: Arc<Mutex<Inner>>,
    capacity: usize,
    live: tokio::sync::broadcast::Sender<LogLine>,
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl LogSink {
    pub fn new(capacity: usize) -> LogSink {
        let (live, _) = tokio::sync::broadcast::channel(256);
        LogSink {
            inner: Arc::new(Mutex::new(Inner {
                lines: VecDeque::new(),
                next_seq: 1,
            })),
            capacity: capacity.max(1),
            live,
        }
    }

    /// Append one line; returns its sequence number.
    pub fn push(&self, level: &'static str, target: &str, msg: &str) -> u64 {
        self.push_at(unix_ms(), level, target, msg)
    }

    pub fn push_at(&self, unix_ms: u64, level: &'static str, target: &str, msg: &str) -> u64 {
        let line = {
            let mut inner = self.inner.lock().unwrap();
            let seq = inner.next_seq;
            inner.next_seq += 1;
            let line = LogLine {
                seq,
                unix_ms,
                level,
                target: target.to_string(),
                msg: msg.to_string(),
            };
            if inner.lines.len() >= self.capacity {
                inner.lines.pop_front();
            }
            inner.lines.push_back(line.clone());
            line
        };
        let seq = line.seq;
        // no receiver: nobody is streaming, which is fine
        let _ = self.live.send(line);
        seq
    }

    /// Lines with `seq > after`, at most `limit`, oldest first, and the
    /// sequence number the next request should ask after (the last line
    /// returned, or `after` when nothing is newer).
    pub fn after(&self, after: u64, limit: usize) -> (Vec<LogLine>, u64) {
        let inner = self.inner.lock().unwrap();
        let lines: Vec<LogLine> = inner
            .lines
            .iter()
            .filter(|l| l.seq > after)
            .take(limit)
            .cloned()
            .collect();
        let next = lines.last().map_or(after, |l| l.seq);
        (lines, next)
    }

    /// The newest `n` lines, oldest first.
    pub fn tail(&self, n: usize) -> Vec<LogLine> {
        let inner = self.inner.lock().unwrap();
        let skip = inner.lines.len().saturating_sub(n);
        inner.lines.iter().skip(skip).cloned().collect()
    }

    /// Plain text of every line, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.inner
            .lock()
            .unwrap()
            .lines
            .iter()
            .map(|l| format!("{} {} {}", l.level, l.target, l.msg))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sequence number the next pushed line gets.
    pub fn next_seq(&self) -> u64 {
        self.inner.lock().unwrap().next_seq
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().lines.clear();
    }

    /// New lines as they arrive (a receiver that falls 256 lines behind skips).
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<LogLine> {
        self.live.subscribe()
    }

    /// A tracing layer that writes every event into this sink.
    pub fn layer(&self) -> SinkLayer {
        SinkLayer(self.clone())
    }
}

/// `tracing_subscriber::Layer` feeding a `LogSink`; stack it next to the
/// usual stdout `fmt` layer.
pub struct SinkLayer(LogSink);

struct MessageVisitor {
    message: String,
    rest: Vec<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.rest.push(format!("{}={value:?}", field.name()));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.rest.push(format!("{}={value}", field.name()));
        }
    }
}

fn level_name(l: &tracing::Level) -> &'static str {
    match *l {
        tracing::Level::ERROR => "ERROR",
        tracing::Level::WARN => "WARN",
        tracing::Level::INFO => "INFO",
        tracing::Level::DEBUG => "DEBUG",
        tracing::Level::TRACE => "TRACE",
    }
}

impl<S: Subscriber> Layer<S> for SinkLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut v = MessageVisitor {
            message: String::new(),
            rest: Vec::new(),
        };
        event.record(&mut v);
        let mut msg = v.message;
        if !v.rest.is_empty() {
            if !msg.is_empty() {
                msg.push(' ');
            }
            msg.push_str(&v.rest.join(" "));
        }
        let meta = event.metadata();
        self.0.push(level_name(meta.level()), meta.target(), &msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn sequence_numbers_grow_and_the_ring_caps() {
        let sink = LogSink::new(3);
        assert_eq!(sink.push("INFO", "t", "one"), 1);
        assert_eq!(sink.push("WARN", "t", "two"), 2);
        assert_eq!(sink.push("INFO", "t", "three"), 3);
        assert_eq!(sink.push("INFO", "t", "four"), 4);
        let msgs: Vec<&str> = sink
            .lines()
            .iter()
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .iter()
            .map(|_| "")
            .collect();
        assert_eq!(msgs.len(), 3);
        assert_eq!(sink.len(), 3);
        assert_eq!(sink.next_seq(), 5);
        let (lines, next) = sink.after(0, 500);
        assert_eq!(
            lines.iter().map(|l| l.seq).collect::<Vec<_>>(),
            vec![2, 3, 4],
            "the oldest line fell out"
        );
        assert_eq!(next, 4);
        let (lines, next) = sink.after(3, 500);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].msg, "four");
        assert_eq!(lines[0].level, "INFO");
        assert_eq!(next, 4);
        let (lines, next) = sink.after(4, 500);
        assert!(lines.is_empty());
        assert_eq!(next, 4, "nothing newer: ask after the same seq again");
        let (lines, next) = sink.after(0, 2);
        assert_eq!(lines.len(), 2, "limit honoured");
        assert_eq!(next, 3);
        assert_eq!(sink.tail(1)[0].msg, "four");
        sink.clear();
        assert!(sink.is_empty());
        assert_eq!(sink.push("INFO", "t", "five"), 5, "seq keeps counting");
    }

    #[test]
    fn tracing_events_land_in_the_sink_with_level_and_target() {
        let sink = LogSink::new(10);
        let mut rx = sink.subscribe();
        let sub = tracing_subscriber::registry().with(sink.layer());
        tracing::subscriber::with_default(sub, || {
            tracing::info!(target: "rackscreen_sources::k8s", "hello {}", "sink");
            tracing::warn!(count = 3, "with fields");
        });
        let lines = sink.tail(10);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].level, "INFO");
        assert_eq!(lines[0].target, "rackscreen_sources::k8s");
        assert_eq!(lines[0].msg, "hello sink");
        assert_eq!(lines[1].level, "WARN");
        assert_eq!(lines[1].msg, "with fields count=3");
        assert!(lines[0].unix_ms > 1_700_000_000_000);
        assert_eq!(rx.try_recv().unwrap().msg, "hello sink");
        assert_eq!(rx.try_recv().unwrap().seq, 2);
    }
}

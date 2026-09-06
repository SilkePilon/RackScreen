//! An in-memory ring of log lines that tracing can write into (for the TUI's Run screen).

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
pub struct LogSink {
    inner: Arc<Mutex<VecDeque<String>>>,
    capacity: usize,
    partial: Arc<Mutex<String>>,
}

impl LogSink {
    pub fn new(capacity: usize) -> LogSink {
        LogSink {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            capacity,
            partial: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn lines(&self) -> Vec<String> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    fn push_bytes(&self, buf: &[u8]) {
        let mut partial = self.partial.lock().unwrap();
        partial.push_str(&String::from_utf8_lossy(buf));
        let mut lines = self.inner.lock().unwrap();
        while let Some(pos) = partial.find('\n') {
            let line: String = partial.drain(..=pos).collect();
            let line = strip_ansi(line.trim_end_matches('\n'));
            if !line.is_empty() {
                if lines.len() >= self.capacity {
                    lines.pop_front();
                }
                lines.push_back(line);
            }
        }
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub struct SinkWriter(LogSink);

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.push_bytes(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogSink {
    type Writer = SinkWriter;
    fn make_writer(&'a self) -> SinkWriter {
        SinkWriter(self.clone())
    }
}

/// Route all tracing output into the sink (no stderr). Safe to call more than once.
pub fn install_sink_subscriber(sink: &LogSink) {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(sink.clone())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_lines_and_caps() {
        let sink = LogSink::new(3);
        let mut w = sink.make_writer();
        w.write_all(b"one\ntw").unwrap();
        w.write_all(b"o\n\x1b[32mthree\x1b[0m\nfour\n").unwrap();
        assert_eq!(sink.lines(), vec!["two", "three", "four"]);
        sink.clear();
        assert!(sink.lines().is_empty());
    }

    #[test]
    fn tracing_goes_into_sink() {
        let sink = LogSink::new(10);
        let sub = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(sink.clone())
            .finish();
        tracing::subscriber::with_default(sub, || {
            tracing::info!("hello sink");
        });
        assert!(sink.lines().iter().any(|l| l.contains("hello sink")));
    }
}

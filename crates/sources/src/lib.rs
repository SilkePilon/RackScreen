//! Data sources: they run on tokio and push `Event`s into a std mpsc channel
//! that the render thread drains with `try_recv`.

use rackscreen_core::event::Event;
use tokio_util::sync::CancellationToken;

pub mod electricity;
pub mod fake;
pub mod http;
pub mod k8s;
pub mod prices;
pub mod prometheus;
pub mod qbittorrent;
pub mod tunnel;

pub type EventTx = std::sync::mpsc::Sender<Event>;

#[derive(Clone)]
pub struct SourceCtx {
    pub tx: EventTx,
    pub shutdown: CancellationToken,
}

impl SourceCtx {
    pub fn emit(&self, ev: Event) {
        let _ = self.tx.send(ev);
    }
    pub fn emit_all(&self, evs: Vec<Event>) {
        for ev in evs {
            self.emit(ev);
        }
    }
}

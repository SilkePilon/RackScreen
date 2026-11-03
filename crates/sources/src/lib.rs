//! Data sources: they run on tokio and push `Event`s into a std mpsc channel
//! that the render thread drains with `try_recv`.

use rackscreen_core::event::Event;
use tokio_util::sync::CancellationToken;

pub mod argocd;
pub mod arr;
pub mod astro;
pub mod buienradar;
pub mod cover;
pub mod electricity;
pub mod fake;
pub mod gif;
pub mod github;
pub mod http;
pub mod iss;
pub mod jellyfin;
pub mod k8s;
pub mod open_meteo;
pub mod owned_fs;
pub mod prometheus;
pub mod qbittorrent;
pub mod spotify;
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

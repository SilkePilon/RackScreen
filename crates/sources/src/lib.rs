//! Data sources: they run on tokio and push `Event`s into a std mpsc channel
//! that the render thread drains with `try_recv`.

use rackscreen_core::event::Event;
use tokio_util::sync::CancellationToken;

pub mod argocd;
pub mod arr;
pub mod astro;
pub mod bambu;
pub mod bambu_cloud;
pub mod buienradar;
pub mod cover;
pub mod electricity;
pub mod fake;
pub mod gif;
pub mod github;
pub mod homeassistant;
pub mod http;
pub mod iss;
pub mod jellyfin;
pub mod k8s;
pub mod mem;
pub mod open_meteo;
pub mod owned_fs;
pub mod prometheus;
pub mod qbittorrent;
pub mod sen;
pub mod spotify;
pub mod tunnel;

/// Images with more pixels than this are refused from their header, before
/// any pixel is decoded: a GIF canvas, an album cover. 4 Mpx of RGBA is 16 MB.
pub const MAX_IMAGE_PX: u64 = 4_000_000;

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

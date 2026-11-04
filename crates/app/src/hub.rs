//! What the web server shares with the running monitor: the config path, the
//! log sink, a tap on the rendered frames, a once-a-second status snapshot, a
//! control channel into the current render loop and the supervisor's mailbox.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use rackscreen_core::event::Event;
use rackscreen_core::gif::GifState;
use rackscreen_core::shows::schedule::Request;
use rackscreen_core::theme::Role;
use serde::Serialize;

use crate::logs::LogSink;

// ---------------------------------------------------------------- frames

/// The last rendered frame of one screen: a sequence number and 240×240
/// straight RGBA, unrotated.
#[derive(Clone)]
pub struct TappedFrame {
    pub seq: u64,
    pub rgba: Arc<[u8]>,
}

struct TapSlot {
    frame: Option<TappedFrame>,
    last_copy: Option<Instant>,
}

/// Render workers copy their frames in here, but only while at least one
/// live client watches (`watchers > 0`) and at most `fps` times a second per
/// screen, so an idle Pi pays nothing.
pub struct FrameTap {
    watchers: AtomicUsize,
    /// Minimum milliseconds between two copies of one screen.
    interval_ms: AtomicU64,
    /// The fps each live connection asked for, by watch id; the fastest wins.
    fps: Mutex<FpsRequests>,
    /// Sequence numbers are unique across screens and monitor restarts, so
    /// a slot cleared and filled again never repeats a number a client saw.
    next_seq: AtomicU64,
    /// Screens the current monitor drives (`<= slots.len()`).
    active: AtomicUsize,
    slots: Vec<Mutex<TapSlot>>,
}

#[derive(Default)]
struct FpsRequests {
    next_id: u64,
    by_id: HashMap<u64, u32>,
}

pub const DEFAULT_LIVE_FPS: u32 = 10;
pub const MAX_LIVE_FPS: u32 = 15;
/// Tap slots: the hub outlives config reloads, so it is sized for the most
/// screens a config may list rather than the current count.
pub const MAX_SCREENS: usize = 8;

impl FrameTap {
    pub fn new(screens: usize) -> FrameTap {
        FrameTap {
            watchers: AtomicUsize::new(0),
            interval_ms: AtomicU64::new(1000 / DEFAULT_LIVE_FPS as u64),
            fps: Mutex::new(FpsRequests::default()),
            next_seq: AtomicU64::new(1),
            active: AtomicUsize::new(screens),
            slots: (0..screens)
                .map(|_| {
                    Mutex::new(TapSlot {
                        frame: None,
                        last_copy: None,
                    })
                })
                .collect(),
        }
    }

    /// Tap slots (the most screens there can be).
    pub fn screens(&self) -> usize {
        self.slots.len()
    }

    /// Screens the current monitor drives.
    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst).min(self.slots.len())
    }

    /// A (re)started monitor drives `n` screens: forget the frames of the
    /// screens past that, so no client keeps being sent a stale one.
    pub fn set_active(&self, n: usize) {
        let n = n.min(self.slots.len());
        self.active.store(n, Ordering::SeqCst);
        for slot in &self.slots[n..] {
            let mut slot = slot.lock().unwrap();
            slot.frame = None;
            slot.last_copy = None;
        }
    }

    /// Register a live client (asking for `DEFAULT_LIVE_FPS` until it says
    /// otherwise); drop the guard to unregister.
    pub fn watch(self: &Arc<Self>) -> WatchGuard {
        let id = {
            let mut f = self.fps.lock().unwrap();
            let id = f.next_id;
            f.next_id += 1;
            f.by_id.insert(id, DEFAULT_LIVE_FPS);
            id
        };
        self.watchers.fetch_add(1, Ordering::SeqCst);
        self.recompute_fps();
        WatchGuard {
            tap: self.clone(),
            id,
        }
    }

    pub fn watchers(&self) -> usize {
        self.watchers.load(Ordering::SeqCst)
    }

    /// The interval for the fastest live connection (the default when none).
    fn recompute_fps(&self) {
        let f = self.fps.lock().unwrap();
        let fps = f
            .by_id
            .values()
            .copied()
            .max()
            .unwrap_or(DEFAULT_LIVE_FPS)
            .clamp(1, MAX_LIVE_FPS);
        self.interval_ms.store(1000 / fps as u64, Ordering::Relaxed);
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    /// Whether the worker of `screen` should copy its frame now.
    pub fn wants(&self, screen: usize, now: Instant) -> bool {
        if self.watchers.load(Ordering::Relaxed) == 0 {
            return false;
        }
        let Some(slot) = self.slots.get(screen) else {
            return false;
        };
        let slot = slot.lock().unwrap();
        match slot.last_copy {
            None => true,
            Some(t) => now.duration_since(t).as_millis() as u64 >= self.interval_ms(),
        }
    }

    /// Store a copy of `rgba` (240×240×4) for `screen`.
    pub fn publish(&self, screen: usize, rgba: &[u8], now: Instant) {
        let Some(slot) = self.slots.get(screen) else {
            return;
        };
        let mut slot = slot.lock().unwrap();
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        slot.frame = Some(TappedFrame {
            seq,
            rgba: Arc::from(rgba),
        });
        slot.last_copy = Some(now);
    }

    /// The latest frame of `screen`, if any was published.
    pub fn latest(&self, screen: usize) -> Option<TappedFrame> {
        self.slots.get(screen)?.lock().unwrap().frame.clone()
    }

    /// The sequence number of the latest frame of `screen`, 0 when none.
    pub fn seq(&self, screen: usize) -> u64 {
        self.slots
            .get(screen)
            .and_then(|s| s.lock().unwrap().frame.as_ref().map(|f| f.seq))
            .unwrap_or(0)
    }
}

/// One live connection's registration with the tap.
pub struct WatchGuard {
    tap: Arc<FrameTap>,
    id: u64,
}

impl WatchGuard {
    /// This connection wants at most `fps` frames a second
    /// (1..=`MAX_LIVE_FPS`). The fastest live connection sets the pace.
    pub fn set_fps(&self, fps: u32) {
        self.tap
            .fps
            .lock()
            .unwrap()
            .by_id
            .insert(self.id, fps.clamp(1, MAX_LIVE_FPS));
        self.tap.recompute_fps();
    }
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        self.tap.fps.lock().unwrap().by_id.remove(&self.id);
        self.tap.watchers.fetch_sub(1, Ordering::SeqCst);
        self.tap.recompute_fps();
    }
}

// ---------------------------------------------------------------- control

/// Things the web UI asks the render loop to do that are not model events.
#[derive(Clone, Debug)]
pub enum Control {
    /// Every panel shows its number (the calibration pattern) for `secs`.
    Identify { secs: f64 },
    /// The calibration pattern on every panel until turned off (or 10 min).
    Pattern { on: bool },
    /// `screen` jumps to `role` now (it must be in that screen's list).
    Jump { screen: usize, role: Role },
    /// Play a show now (refused at night, like the request file).
    Play(Request),
    /// `screen` (or every cycling screen, left to right) changes to its
    /// next role now, in the configured style: the web UI's "try it".
    TransitionTest { screen: Option<usize> },
}

/// Senders into the current render loop: model events (`Event::ForceNight`
/// and friends) and controls.
#[derive(Clone)]
pub struct ControlHandle {
    pub events: mpsc::Sender<Event>,
    pub control: mpsc::Sender<Control>,
}

// ---------------------------------------------------------------- status

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ScreenStatus {
    pub index: usize,
    pub role: String,
    pub title: String,
    pub roles: Vec<String>,
    pub cycle_secs: u64,
    pub cycle_progress: f64,
    /// `role`, `show`, `night`, `pattern` or `identify`.
    pub showing: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LinkStatus {
    pub name: &'static str,
    /// `ok`, `warn`, `down` or `off`.
    pub state: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct ScreenStat {
    pub index: usize,
    pub render_ms: f32,
    pub render_p95_ms: f32,
    pub push_ms: f32,
    pub fps: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TileStatus {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub unit: &'static str,
    pub tone: &'static str,
}

impl From<rackscreen_core::tiles::Tile> for TileStatus {
    fn from(t: rackscreen_core::tiles::Tile) -> Self {
        TileStatus {
            key: t.key,
            label: t.label,
            value: t.value,
            unit: t.unit,
            tone: t.tone.name(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct NightStatus {
    /// `auto`, `on` or `off`.
    pub mode: &'static str,
    pub active: bool,
}

/// `auto`, `on` or `off` for a night override.
pub fn night_mode_name(v: Option<bool>) -> &'static str {
    match v {
        None => "auto",
        Some(true) => "on",
        Some(false) => "off",
    }
}

impl Default for NightStatus {
    fn default() -> Self {
        NightStatus {
            mode: "auto",
            active: false,
        }
    }
}

/// One configured GIF as the render loop holds it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GifStatus {
    /// Its name under `gifs:`.
    pub name: String,
    /// Some screen of the running config shows it (only those are decoded).
    pub used: bool,
    /// `off` (no screen shows it), `loading`, `ready` or `error`.
    pub state: &'static str,
    /// Decoded frames, when ready.
    pub frames: Option<usize>,
    /// Bytes the decoded frames take, when ready.
    pub bytes: Option<usize>,
    /// Why the last attempt failed.
    pub error: Option<String>,
}

/// The status row of GIF `name` in `state`; `used` when a screen shows it.
pub fn gif_status(name: &str, used: bool, state: &GifState) -> GifStatus {
    let (state, frames, bytes, error) = match state {
        GifState::Ready(f) => ("ready", Some(f.0.len()), Some(f.0.bytes()), None),
        // an unused GIF is never downloaded: whatever the model holds is stale
        _ if !used => ("off", None, None, None),
        GifState::Loading => ("loading", None, None, None),
        GifState::Failed(e) => ("error", None, None, Some(e.clone())),
    };
    GifStatus {
        name: name.to_string(),
        used,
        state,
        frames,
        bytes,
        error,
    }
}

/// What the render loop publishes about once a second.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct StatusSnapshot {
    pub night: NightStatus,
    pub screens: Vec<ScreenStatus>,
    pub links: Vec<LinkStatus>,
    pub stats: Vec<ScreenStat>,
    pub tiles: Vec<TileStatus>,
    /// Every configured GIF, in `gifs:` order.
    pub gifs: Vec<GifStatus>,
}

/// The supervisor's view of the monitor.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MonitorState {
    /// `starting`, `running`, `stopped` or `error`.
    pub state: &'static str,
    pub error: Option<String>,
    pub since_unix: u64,
}

impl Default for MonitorState {
    fn default() -> Self {
        MonitorState {
            state: "starting",
            error: None,
            since_unix: unix_now(),
        }
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- supervisor

/// What the web asks the supervisor to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorCmd {
    /// Drop the monitor, reload the config, start again.
    RestartMonitor,
    /// Exit the process with code 0 (the `web:` block changed; systemd
    /// starts us again). In `--sim` the supervisor only logs it.
    RestartProcess,
}

// ---------------------------------------------------------------- hub

pub struct Hub {
    pub config_path: PathBuf,
    pub sim: bool,
    pub version: &'static str,
    pub started_unix: u64,
    pub logs: LogSink,
    pub frames: Arc<FrameTap>,
    status: Mutex<StatusSnapshot>,
    /// When the render loop last published `status`.
    status_at: Mutex<Option<Instant>>,
    monitor: Mutex<MonitorState>,
    control: Mutex<Option<ControlHandle>>,
    /// The night override from the web UI (`None` = auto). It outlives the
    /// render loop: every new loop gets it again.
    night_override: Mutex<Option<bool>>,
    supervisor_tx: Mutex<Option<mpsc::Sender<SupervisorCmd>>>,
}

impl Hub {
    pub fn new(
        config_path: PathBuf,
        sim: bool,
        version: &'static str,
        logs: LogSink,
        screens: usize,
    ) -> Hub {
        Hub {
            config_path,
            sim,
            version,
            started_unix: unix_now(),
            logs,
            frames: Arc::new(FrameTap::new(screens)),
            status: Mutex::new(StatusSnapshot::default()),
            status_at: Mutex::new(None),
            monitor: Mutex::new(MonitorState::default()),
            control: Mutex::new(None),
            night_override: Mutex::new(None),
            supervisor_tx: Mutex::new(None),
        }
    }

    /// The latest snapshot, with the night mode the web UI asked for (the
    /// render loop may not have published since).
    pub fn status(&self) -> StatusSnapshot {
        let mut s = self.status.lock().unwrap().clone();
        s.night.mode = night_mode_name(*self.night_override.lock().unwrap());
        s
    }

    pub fn night_override(&self) -> Option<bool> {
        *self.night_override.lock().unwrap()
    }

    /// Remember the night override and hand it to the current render loop.
    /// Returns whether a render loop took it (it is kept either way and
    /// sent to the next loop).
    pub fn set_night_override(&self, v: Option<bool>) -> bool {
        let mut cur = self.night_override.lock().unwrap();
        *cur = v;
        let control = self.control.lock().unwrap();
        control
            .as_ref()
            .is_some_and(|h| h.events.send(Event::ForceNight(v)).is_ok())
    }

    pub fn publish_status(&self, s: StatusSnapshot) {
        *self.status.lock().unwrap() = s;
        *self.status_at.lock().unwrap() = Some(Instant::now());
    }

    /// Milliseconds since the last published snapshot (0 before the first),
    /// so a client can tell how old `cycle_progress` is.
    pub fn status_age_ms(&self) -> u64 {
        self.status_at
            .lock()
            .unwrap()
            .map_or(0, |t| t.elapsed().as_millis() as u64)
    }

    pub fn monitor(&self) -> MonitorState {
        self.monitor.lock().unwrap().clone()
    }

    pub fn set_monitor(&self, state: &'static str, error: Option<String>) {
        *self.monitor.lock().unwrap() = MonitorState {
            state,
            error,
            since_unix: unix_now(),
        };
    }

    pub fn control(&self) -> Option<ControlHandle> {
        self.control.lock().unwrap().clone()
    }

    pub fn set_control(&self, handle: Option<ControlHandle>) {
        let night = self.night_override.lock().unwrap();
        if let (Some(h), Some(v)) = (&handle, *night) {
            // a fresh render loop starts in auto: give it the override again
            let _ = h.events.send(Event::ForceNight(Some(v)));
        }
        let mut control = self.control.lock().unwrap();
        *control = handle;
        drop(night);
        if control.is_none() {
            drop(control);
            // the render loop is gone: its last snapshot is stale
            let mut s = self.status.lock().unwrap();
            s.screens.clear();
            s.stats.clear();
            s.gifs.clear();
        }
    }

    /// Wire the supervisor's mailbox; returns the receiving end.
    pub fn supervisor_mailbox(&self) -> mpsc::Receiver<SupervisorCmd> {
        let (tx, rx) = mpsc::channel();
        *self.supervisor_tx.lock().unwrap() = Some(tx);
        rx
    }

    /// Ask the supervisor; false when no supervisor is listening.
    pub fn request(&self, cmd: SupervisorCmd) -> bool {
        self.supervisor_tx
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|tx| tx.send(cmd).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tap_copies_only_with_watchers_and_throttles() {
        let tap = Arc::new(FrameTap::new(2));
        let t0 = Instant::now();
        assert!(!tap.wants(0, t0), "nobody watches");
        let guard = tap.watch();
        assert_eq!(tap.watchers(), 1);
        assert!(tap.wants(0, t0));
        assert!(!tap.wants(5, t0), "no such screen");
        let frame = vec![7u8; 240 * 240 * 4];
        tap.publish(0, &frame, t0);
        assert_eq!(tap.seq(0), 1);
        assert_eq!(tap.seq(1), 0);
        assert!(
            !tap.wants(0, t0 + Duration::from_millis(50)),
            "under 100 ms"
        );
        assert!(tap.wants(0, t0 + Duration::from_millis(100)));
        assert!(tap.wants(1, t0), "the other screen is free");
        guard.set_fps(20);
        assert_eq!(tap.interval_ms(), 1000 / 15, "capped at 15 fps");
        guard.set_fps(0);
        assert_eq!(tap.interval_ms(), 1000);
        tap.publish(0, &frame, t0 + Duration::from_secs(1));
        let f = tap.latest(0).unwrap();
        assert_eq!(f.seq, 2);
        assert_eq!(f.rgba.len(), 240 * 240 * 4);
        drop(guard);
        assert_eq!(tap.watchers(), 0);
        assert!(!tap.wants(1, t0 + Duration::from_secs(5)));
    }

    #[test]
    fn fps_is_the_fastest_live_connection() {
        let tap = Arc::new(FrameTap::new(1));
        assert_eq!(tap.interval_ms(), 100);
        let a = tap.watch();
        let b = tap.watch();
        a.set_fps(2);
        assert_eq!(tap.interval_ms(), 100, "b still wants the default 10");
        b.set_fps(5);
        assert_eq!(tap.interval_ms(), 200);
        b.set_fps(15);
        a.set_fps(1);
        assert_eq!(tap.interval_ms(), 1000 / 15, "a slower a does not win");
        drop(b);
        assert_eq!(tap.interval_ms(), 1000, "only a is left");
        drop(a);
        assert_eq!(tap.interval_ms(), 100, "back to the default");
        assert_eq!(tap.watchers(), 0);
    }

    #[test]
    fn fewer_screens_clear_the_rest_and_seqs_never_repeat() {
        let tap = Arc::new(FrameTap::new(4));
        let _g = tap.watch();
        let t0 = Instant::now();
        let frame = vec![1u8; 240 * 240 * 4];
        for i in 0..4 {
            tap.publish(i, &frame, t0);
        }
        let old = tap.seq(3);
        assert_eq!(tap.active(), 4);
        tap.set_active(2);
        assert_eq!(tap.active(), 2);
        assert_eq!(tap.seq(2), 0);
        assert!(tap.latest(3).is_none());
        assert!(tap.seq(1) > 0, "kept");
        tap.set_active(4);
        tap.publish(3, &frame, t0);
        assert!(tap.seq(3) > old, "a refilled slot gets a new number");
        tap.set_active(99);
        assert_eq!(tap.active(), 4, "capped at the slots");
    }

    #[test]
    fn night_override_survives_a_new_render_loop() {
        let hub = Hub::new("/tmp/x.yaml".into(), true, "0.0.0", LogSink::new(5), 1);
        assert!(!hub.set_night_override(Some(true)), "no loop yet, but kept");
        assert_eq!(hub.status().night.mode, "on");
        let handle = || {
            let (etx, erx) = mpsc::channel();
            let (ctx, _crx) = mpsc::channel();
            (
                ControlHandle {
                    events: etx,
                    control: ctx,
                },
                erx,
            )
        };
        let (h1, rx1) = handle();
        hub.set_control(Some(h1));
        assert!(matches!(rx1.try_recv(), Ok(Event::ForceNight(Some(true)))));
        assert!(hub.set_night_override(Some(false)));
        assert!(matches!(rx1.try_recv(), Ok(Event::ForceNight(Some(false)))));
        // the monitor restarts: the new loop gets the override again
        hub.set_control(None);
        let (h2, rx2) = handle();
        hub.set_control(Some(h2));
        assert!(matches!(rx2.try_recv(), Ok(Event::ForceNight(Some(false)))));
        assert_eq!(hub.status().night.mode, "off");
        // back to auto: a new loop is left alone
        assert!(hub.set_night_override(None));
        let (h3, rx3) = handle();
        hub.set_control(Some(h3));
        assert!(rx3.try_recv().is_err());
        assert_eq!(hub.status().night.mode, "auto");
    }

    #[test]
    fn gif_rows_tell_state_size_and_errors() {
        use rackscreen_core::gif::{GifFrames, GifRef, FRAME_BYTES};
        let frames = GifFrames::new(vec![(vec![0; FRAME_BYTES], 50); 3]).unwrap();
        let ready = GifState::Ready(GifRef(Arc::new(frames)));
        let r = gif_status("cat", true, &ready);
        assert_eq!(
            (r.state, r.frames, r.bytes),
            ("ready", Some(3), Some(3 * FRAME_BYTES))
        );
        let l = gif_status("dog", true, &GifState::Loading);
        assert_eq!((l.state, l.frames, l.error), ("loading", None, None));
        let e = gif_status("x", true, &GifState::Failed("404".into()));
        assert_eq!((e.state, e.error.as_deref()), ("error", Some("404")));
        assert_eq!(gif_status("x", false, &GifState::Loading).state, "off");
        assert_eq!(
            gif_status("x", false, &GifState::Failed("old".into())).error,
            None,
            "not on a screen: not tried"
        );
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["name"], "cat");
        assert_eq!(json["used"], true);
    }

    #[test]
    fn hub_holds_status_monitor_and_supervisor_mailbox() {
        let hub = Hub::new("/tmp/x.yaml".into(), true, "0.0.0", LogSink::new(5), 4);
        assert_eq!(hub.monitor().state, "starting");
        assert!(
            !hub.request(SupervisorCmd::RestartMonitor),
            "no supervisor yet"
        );
        let rx = hub.supervisor_mailbox();
        assert!(hub.request(SupervisorCmd::RestartProcess));
        assert_eq!(rx.try_recv().unwrap(), SupervisorCmd::RestartProcess);
        hub.set_monitor("error", Some("boom".into()));
        assert_eq!(hub.monitor().error.as_deref(), Some("boom"));
        let mut s = StatusSnapshot::default();
        s.screens.push(ScreenStatus {
            index: 0,
            role: "cpu".into(),
            title: "CPU".into(),
            roles: vec!["cpu".into()],
            cycle_secs: 15,
            cycle_progress: 0.0,
            showing: "role",
        });
        hub.publish_status(s);
        assert_eq!(hub.status().screens.len(), 1);
        assert!(hub.control().is_none());
        hub.set_control(None);
        assert!(
            hub.status().screens.is_empty(),
            "cleared with the render loop"
        );
        let json = serde_json::to_value(hub.status()).unwrap();
        assert_eq!(json["night"]["mode"], "auto");
        assert_eq!(hub.frames.screens(), 4);
    }
}

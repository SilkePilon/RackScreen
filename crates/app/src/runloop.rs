//! The 30 Hz render loop: drain events, advance the model, render four scenes,
//! orient, diff, hand frames to display threads. Also night mode.

use rackscreen_core::p2::ui::ScreenUi;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Timelike;
use rackscreen_core::anim::Secs;
use rackscreen_core::event::{Event, LinkTarget};
use rackscreen_core::model::{Model, Thresholds};
use rackscreen_core::night::is_night;
use rackscreen_core::p2::status::night_scene;
use rackscreen_core::screens::Entry;
use rackscreen_core::shows::schedule::{Request, ShowScheduler};
use rackscreen_core::shows::world::Layout;
use rackscreen_core::theme::Role;
use rackscreen_core::transition::Transitions;
use rackscreen_display::{DisplayCmd, Mailbox, PushStats};
use rackscreen_render::frame::Orient;

use crate::hub::{
    gif_status, Control, GifStatus, Hub, LinkStatus, NightStatus, ScreenStat, ScreenStatus,
    StatusSnapshot,
};
use crate::night::{NightFrame, NightPhase};
use crate::worker::{Job, ScreenWorker};

pub struct ScreenSlot {
    /// Physical position, top to bottom; the model is asked for `scene(index, now)`.
    pub index: usize,
    /// Role this panel was opened as (calibrate pattern, logging); the model owns
    /// which role is actually shown.
    pub role: Role,
    pub orient: Orient,
    pub mailbox: Mailbox,
    /// Rolls, glides and marquees of whatever role this panel is showing.
    pub ui: ScreenUi,
    /// Brightness applied while packing (by the screen's render worker).
    pub brightness: f32,
}

impl ScreenSlot {
    pub fn new(
        index: usize,
        role: Role,
        orient: Orient,
        mailbox: Mailbox,
        brightness: f32,
    ) -> Self {
        Self {
            index,
            role,
            orient,
            mailbox,
            ui: ScreenUi::new(),
            brightness,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NightWindow {
    pub enabled: bool,
    pub start_min: u32,
    pub end_min: u32,
}

/// Which links the config has turned on, so the status can say `off` for
/// the rest instead of `down`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkCfg {
    pub k8s: bool,
    pub qbit: bool,
    pub electricity: bool,
    pub weather: bool,
    pub rain: bool,
    pub github: bool,
    pub argocd: bool,
    /// `--source fake`: every link reports `ok`, detail `simulated`.
    pub simulated: bool,
}

/// The calibration pattern on every panel: `identify` for a few seconds,
/// or `pattern` until turned off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Identify,
    Pattern,
}

impl PatternKind {
    fn showing(self) -> &'static str {
        match self {
            PatternKind::Identify => "identify",
            PatternKind::Pattern => "pattern",
        }
    }
}

/// How long `pattern on` stays without being turned off.
pub const PATTERN_MAX: Duration = Duration::from_secs(600);
/// The longest `identify`.
pub const IDENTIFY_MAX_SECS: f64 = 60.0;

/// How often the status snapshot goes to the hub.
const STATUS_EVERY: Duration = Duration::from_secs(1);

/// The link rows of the status, from the model's link flags, what the config
/// turned on and which links have reported at all since the start.
pub fn link_rows(
    link: rackscreen_core::model::LinkState,
    cfg: LinkCfg,
    heard: &HashSet<LinkTarget>,
    model: &Model,
) -> Vec<LinkStatus> {
    let st = model.state();
    let rows: [(LinkTarget, &'static str, bool, bool, String); 8] = [
        (
            LinkTarget::K8sApi,
            "Kubernetes",
            cfg.k8s,
            link.api,
            if st.have_nodes {
                format!("{} nodes", st.nodes_total)
            } else {
                String::new()
            },
        ),
        (
            LinkTarget::Prometheus,
            "Prometheus",
            cfg.k8s,
            link.prom,
            if st.have_metrics {
                format!("{} alerts", st.alerts.len())
            } else {
                String::new()
            },
        ),
        (
            LinkTarget::QBittorrent,
            "qBittorrent",
            cfg.k8s && cfg.qbit,
            link.qbit,
            if link.qbit {
                format!("{} torrents", st.torrents.len())
            } else {
                String::new()
            },
        ),
        (
            LinkTarget::Electricity,
            "Electricity Maps",
            cfg.electricity,
            link.electricity,
            if model.electricity().have {
                model.electricity().zone.clone()
            } else {
                String::new()
            },
        ),
        (
            LinkTarget::Weather,
            "Weather",
            cfg.weather,
            link.weather,
            if model.weather().have {
                format!("{:.1} °C", model.weather().temp_c)
            } else {
                String::new()
            },
        ),
        (LinkTarget::Rain, "Rain", cfg.rain, link.rain, String::new()),
        (
            LinkTarget::Github,
            "GitHub",
            cfg.github,
            link.github,
            if model.github().have {
                format!("{} today", model.github().today())
            } else {
                String::new()
            },
        ),
        (
            LinkTarget::ArgoCd,
            "Argo CD",
            cfg.k8s && cfg.argocd,
            link.argocd,
            if model.apps().have {
                format!("{} apps", model.apps().apps.len())
            } else {
                String::new()
            },
        ),
    ];
    if cfg.simulated {
        return rows
            .into_iter()
            .map(|(_, name, ..)| LinkStatus {
                name,
                state: "ok",
                detail: "simulated".into(),
            })
            .collect();
    }
    rows.into_iter()
        .map(|(target, name, on, up, detail)| LinkStatus {
            name,
            state: if !on {
                "off"
            } else if up {
                "ok"
            } else if heard.contains(&target) {
                "down"
            } else {
                "warn"
            },
            detail: if !on {
                String::new()
            } else if !up && !heard.contains(&target) {
                "waiting".into()
            } else {
                detail
            },
        })
        .collect()
}

fn merge_push(acc: &mut PushStats, p: PushStats) {
    acc.frames += p.frames;
    acc.total_ms += p.total_ms;
    acc.max_ms = acc.max_ms.max(p.max_ms);
    acc.bytes += p.bytes;
}

/// The next frame deadline: one tick after the last, or `now` when the loop
/// fell more than a tick behind it (no burst of catch-up frames).
pub fn advance(deadline: Instant, tick: Duration, now: Instant) -> Instant {
    let next = deadline + tick;
    if next + tick < now {
        now
    } else {
        next
    }
}

/// Local wall clock as `(minutes since midnight, unix seconds, seconds east of UTC)`;
/// one clock read per tick.
fn clock() -> (u32, i64, i32) {
    let t = chrono::Local::now();
    (
        t.hour() * 60 + t.minute(),
        t.timestamp(),
        t.offset().local_minus_utc(),
    )
}

/// Sends `Quit` to every display thread and raises `stop` when dropped, so an
/// early return (renderer setup failure) or a panic in the render loop never
/// leaves display threads blocked in `Mailbox::take` or main waiting for a
/// signal that will not come.
pub struct ShutdownGuard {
    mailboxes: Vec<Mailbox>,
    stop: Arc<AtomicBool>,
}

impl ShutdownGuard {
    pub fn new(mailboxes: Vec<Mailbox>, stop: Arc<AtomicBool>) -> Self {
        Self { mailboxes, stop }
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for mb in &self.mailboxes {
            mb.put(DisplayCmd::Quit);
        }
    }
}

pub struct RenderLoop {
    pub rx: Receiver<Event>,
    pub screens: Vec<ScreenSlot>,
    /// Each physical screen's cycle (roles and their dwells), same order as `screens`.
    pub screen_entries: Vec<Vec<Entry>>,
    pub thresholds: Thresholds,
    pub night: NightWindow,
    pub fps: u32,
    pub stop: Arc<AtomicBool>,
    /// Whether an Electricity Maps token is configured; without one the
    /// electricity screens show "no token" instead of "waiting for data".
    pub token_present: bool,
    /// `location.lat`/`lon` are set; without them the sky screens show a pin.
    pub location_present: bool,
    /// A GitHub token is configured; without one `gh-activity` shows a key.
    pub github_token_present: bool,
    /// An arr API key is configured: the media card never shows the torrent.
    pub arr_configured: bool,
    /// `jellyfin.url` is set; without it the role says "Check config".
    pub jellyfin_configured: bool,
    /// A Jellyfin URL (or the fake source); with a Spotify login (checked
    /// every few seconds in `spotify_token`) the music role is configured
    /// too, otherwise it says "Check config".
    pub music_configured: bool,
    /// The Spotify token file while Spotify is on with a client id.
    pub spotify_token: Option<PathBuf>,
    /// `bambu` is on with a host, serial and access code.
    pub printer_configured: bool,
    /// `homeassistant` is on with a URL and token.
    pub ha_configured: bool,
    /// One per `homeassistant.entities` entry.
    pub ha_slots: Vec<rackscreen_core::model::HaSlot>,
    /// Only one screen may transition at a time: four at once stall the SPI bus.
    pub one_at_a_time: bool,
    /// The transition styles for role changes and shows.
    pub transitions: Transitions,
    /// The four-screen show schedule, the world layout and the screen order.
    pub shows: ShowScheduler,
    pub show_layout: Layout,
    pub show_order: Vec<usize>,
    /// The play-now request file; `None` when there is no home directory.
    pub play_path: Option<PathBuf>,
    /// Controls from the web UI (identify, pattern, jump, play).
    pub control_rx: Receiver<Control>,
    /// Where the status snapshot goes and the live frames come from.
    pub hub: Option<Arc<Hub>>,
    /// Which links the config turned on.
    pub links: LinkCfg,
    /// Names of the config's `gifs:`, so `Role::Gif(i)` can be named.
    pub gif_names: Vec<String>,
    /// Names of the config's `homeassistant.entities`, for `Role::Ha(i)`.
    pub ha_names: Vec<String>,
}

/// How often the Spotify token file is looked for, so a login made in
/// setup shows without a restart.
const LOGIN_POLL: Duration = Duration::from_secs(5);

/// The music role has a source: the configured ones, or a Spotify login.
pub fn music_ready(configured: bool, spotify_token: Option<&Path>) -> bool {
    configured || spotify_token.is_some_and(crate::spotify::logged_in)
}

/// How often the play-now request file is checked.
const PLAY_POLL: Duration = Duration::from_secs(1);

/// Consume a play-now request, if one is waiting.
fn poll_play(path: &Path, model: &mut Model, phase: NightPhase) {
    let req = match crate::play::take(path) {
        None => return,
        Some(Err(e)) => {
            tracing::warn!("play request ignored: {e:#}");
            return;
        }
        Some(Ok(r)) => r,
    };
    apply_play(req, model, phase);
}

/// Start a show now: refused at night (it is dropped, not kept for the
/// morning), while one plays, and with fewer than two screens.
fn apply_play(req: Request, model: &mut Model, phase: NightPhase) {
    let name = match req {
        Request::Id(d) => d.id,
        Request::Random => "random",
    };
    if phase != NightPhase::Day {
        tracing::info!("play request {name} dropped: night mode");
    } else if model.show_run().is_some() {
        tracing::info!("play request {name} dropped: a show is already playing");
    } else if model.request_show(req) {
        tracing::info!("play request: {name}");
    } else {
        tracing::info!("play request {name} dropped: shows need at least two screens");
    }
}

impl RenderLoop {
    pub fn run(mut self) -> Result<()> {
        let _guard = ShutdownGuard::new(
            self.screens.iter().map(|s| s.mailbox.clone()).collect(),
            self.stop.clone(),
        );
        // Declared after `_guard`, so the render threads are joined before the
        // guard sends `Quit` to the display threads.
        let tap = self.hub.as_ref().map(|h| h.frames.clone());
        let workers: Vec<ScreenWorker> = self
            .screens
            .iter()
            .map(|s| {
                ScreenWorker::spawn(
                    s.index,
                    s.orient.clone(),
                    s.mailbox.clone(),
                    s.brightness,
                    tap.clone(),
                )
            })
            .collect::<Result<_>>()?;
        let mut force_full = true;
        let start = Instant::now();
        let mut model = Model::new(self.thresholds);
        model.set_screen_entries(self.screen_entries.clone());
        model.set_one_at_a_time(self.one_at_a_time);
        model.set_transitions(self.transitions);
        // A fresh seed per boot, so the screens do not cycle in the same order
        // after every restart.
        model.set_rng_seed(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(1)
                | 1,
        );
        model.set_token_present(self.token_present);
        model.set_location_present(self.location_present);
        model.set_github_token_present(self.github_token_present);
        model.set_arr_configured(self.arr_configured);
        model.set_jellyfin_configured(self.jellyfin_configured);
        model.set_printer_configured(self.printer_configured);
        model.set_ha_configured(self.ha_configured);
        model.set_ha_slots(std::mem::take(&mut self.ha_slots));
        model.set_music_configured(music_ready(
            self.music_configured,
            self.spotify_token.as_deref(),
        ));
        let mut next_login_poll = Instant::now() + LOGIN_POLL;
        let shows = std::mem::replace(&mut self.shows, ShowScheduler::new(false, 15, Vec::new()));
        model.set_shows(
            shows,
            self.show_layout,
            std::mem::take(&mut self.show_order),
        );
        let mut next_play_poll = Instant::now();
        let mut show_started: Option<Secs> = None;
        let tick = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let mut phase = NightPhase::Day;
        let mut stats = crate::stats::FrameStats::new(self.screens.len(), 60.0, 0.0);
        let mut deadline = Instant::now();
        // push stats accumulate per second for the status and per minute for the log
        let mut push_acc: Vec<PushStats> = vec![PushStats::default(); self.screens.len()];
        let mut web_stats: Vec<ScreenStat> = (0..self.screens.len())
            .map(|index| ScreenStat {
                index,
                ..ScreenStat::default()
            })
            .collect();
        let mut next_status = Instant::now();
        let mut last_status = Instant::now();
        let mut heard: HashSet<LinkTarget> = HashSet::new();
        let mut pattern: Option<(PatternKind, Instant)> = None;

        while !self.stop.load(Ordering::Relaxed) {
            let now = start.elapsed().as_secs_f64();
            // the clock first: events bucketed by local day and hour read it
            let (minutes, unix, offset) = clock();
            model.set_unix_now(unix);
            model.set_utc_offset_secs(offset);
            while let Ok(ev) = self.rx.try_recv() {
                if let Event::Link { target, .. } = &ev {
                    heard.insert(*target);
                }
                model.apply(ev, now);
            }
            while let Ok(c) = self.control_rx.try_recv() {
                match c {
                    Control::Identify { secs } => {
                        let secs = secs.clamp(1.0, IDENTIFY_MAX_SECS);
                        tracing::info!("identify: panels show their number for {secs:.0} s");
                        pattern = Some((
                            PatternKind::Identify,
                            Instant::now() + Duration::from_secs_f64(secs),
                        ));
                    }
                    Control::Pattern { on: true } => {
                        tracing::info!("test pattern on");
                        pattern = Some((PatternKind::Pattern, Instant::now() + PATTERN_MAX));
                    }
                    Control::Pattern { on: false } => {
                        if pattern.is_some() {
                            tracing::info!("test pattern off");
                        }
                        pattern = None;
                    }
                    Control::Jump { screen, role } => {
                        if model.jump_to(screen, role, now) {
                            tracing::info!("screen {} jumps to {}", screen + 1, role.name());
                        } else {
                            tracing::warn!(
                                "screen {} cannot jump to {}: not in its list, or a show plays",
                                screen + 1,
                                role.name()
                            );
                        }
                    }
                    Control::Play(req) => apply_play(req, &mut model, phase),
                    Control::TransitionTest { screen } => {
                        let which = match screen {
                            Some(i) => format!("screen {}", i + 1),
                            None => "every screen".to_string(),
                        };
                        if phase != NightPhase::Day {
                            tracing::info!("transition test dropped: night mode");
                        } else if model.force_transition(screen, now) {
                            tracing::info!("transition test: {which} changes role now");
                        } else {
                            tracing::info!(
                                "transition test dropped: a show plays, or {which} has one role"
                            );
                        }
                    }
                }
            }
            if let Some((_, until)) = pattern {
                if Instant::now() >= until {
                    pattern = None;
                    tracing::info!("test pattern off");
                }
            }

            let night = match model.night_override() {
                Some(v) => v,
                None => {
                    self.night.enabled
                        && is_night(minutes, self.night.start_min, self.night.end_min)
                }
            };

            let prev = phase;
            phase = phase.step(night, now);
            if phase != prev {
                tracing::info!("night mode: {prev:?} -> {phase:?}");
            }
            if self.spotify_token.is_some() && Instant::now() >= next_login_poll {
                next_login_poll = Instant::now() + LOGIN_POLL;
                model.set_music_configured(music_ready(
                    self.music_configured,
                    self.spotify_token.as_deref(),
                ));
            }
            if let Some(path) = &self.play_path {
                if Instant::now() >= next_play_poll {
                    next_play_poll = Instant::now() + PLAY_POLL;
                    poll_play(path, &mut model, phase);
                }
            }
            // Shows start only in `Day`; the phase is known before the tick,
            // so none starts on the tick the night fade begins.
            model.set_night_day(phase == NightPhase::Day);
            model.tick(now);
            if let Some(hub) = &self.hub {
                if Instant::now() >= next_status {
                    let t = Instant::now();
                    let secs = t.duration_since(last_status).as_secs_f32().max(1e-3);
                    last_status = t;
                    next_status = t + STATUS_EVERY;
                    for (i, s) in self.screens.iter().enumerate() {
                        let p = s.mailbox.take_push_stats();
                        let st = stats.screen(i);
                        web_stats[i] = ScreenStat {
                            index: i,
                            render_ms: st.render.avg(),
                            render_p95_ms: st.render.p95(),
                            push_ms: if p.frames > 0 {
                                (p.total_ms / p.frames as f64) as f32
                            } else {
                                0.0
                            },
                            fps: p.frames as f32 / secs,
                        };
                        merge_push(&mut push_acc[i], p);
                    }
                    hub.publish_status(
                        self.snapshot(&model, phase, now, pattern, &heard, &web_stats),
                    );
                }
            }
            if let Some((_, _)) = pattern {
                for (i, s) in self.screens.iter().enumerate() {
                    workers[i].send(Job::single(
                        crate::calibrate::test_pattern(s.index, s.role),
                        None,
                        force_full,
                    ))?;
                }
                force_full = false;
                for w in &workers {
                    w.wait()?;
                }
                deadline = advance(deadline, tick, Instant::now());
                if let Some(d) = deadline.checked_duration_since(Instant::now()) {
                    std::thread::sleep(d);
                }
                continue;
            }
            let run = model.show_run().copied();
            if run.map(|r| r.started) != show_started {
                if let Some(r) = run {
                    tracing::info!("show: {} ({:.1} s)", r.def.id, r.total());
                }
                show_started = run.map(|r| r.started);
            }
            // The moon is up and still. The same scene goes to every worker each
            // idle tick: it is skipped for free, but a panel whose last push
            // failed (`needs_full`) gets a full frame again within 500 ms.
            if prev == NightPhase::Icon && phase == NightPhase::Icon {
                for w in &workers {
                    w.send(Job::single(night_scene(1.0), None, false))?;
                }
                for w in &workers {
                    w.wait()?;
                }
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            if prev == NightPhase::Icon && phase != NightPhase::Icon {
                // the hours under the moon belong to no stats period
                for (i, s) in self.screens.iter().enumerate() {
                    let _ = s.mailbox.take_push_stats();
                    push_acc[i] = PushStats::default();
                }
                stats.reset(now);
            }
            if prev.shows_icon() && phase == NightPhase::Day {
                for (i, s) in self.screens.iter().enumerate() {
                    // pushes from before the night belong to no period
                    let _ = s.mailbox.take_push_stats();
                    push_acc[i] = PushStats::default();
                }
                force_full = true;
                // the night is not part of the stats period
                stats.reset(now);
                // the first frame after the night is not late
                deadline = Instant::now();
                model.tick(now);
            }
            let frame = phase.frame(now);
            let fade = match frame {
                NightFrame::Fade(f) => Some(f),
                NightFrame::Day | NightFrame::Icon(_) => None,
            };

            for (i, s) in self.screens.iter_mut().enumerate() {
                let t = Instant::now();
                let frame = match frame {
                    NightFrame::Icon(a) => night_scene(a).into(),
                    NightFrame::Day | NightFrame::Fade(_) => model.frame(s.index, now, &mut s.ui),
                };
                stats
                    .screen(i)
                    .build
                    .push(t.elapsed().as_secs_f32() * 1000.0);
                workers[i].send(Job {
                    scene: frame.scene,
                    blend: frame.blend,
                    fade,
                    force_full,
                })?;
            }
            force_full = false;
            for (i, w) in workers.iter().enumerate() {
                let d = w.wait()?;
                let st = stats.screen(i);
                if d.skipped {
                    st.skipped += 1;
                } else {
                    st.render.push(d.render_ms);
                    st.flush.push(d.flush_ms);
                }
            }
            if stats.due(now) {
                for (i, s) in self.screens.iter().enumerate() {
                    let mut p = std::mem::take(&mut push_acc[i]);
                    merge_push(&mut p, s.mailbox.take_push_stats());
                    tracing::info!("{}", stats.line(i, p, stats.elapsed(now)));
                }
                stats.reset(now);
            }

            deadline = advance(deadline, tick, Instant::now());
            if let Some(d) = deadline.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
        }
        // `_guard` sends Quit to every mailbox and sets `stop` on drop.
        Ok(())
    }

    /// The name a role has in the config: `gif:<name>` for a GIF.
    fn role_name(&self, role: Role) -> String {
        match role {
            Role::Gif(i) => format!(
                "{}{}",
                crate::config::GIF_ROLE_PREFIX,
                self.gif_names.get(i as usize).map_or("?", |n| n.as_str())
            ),
            Role::Ha(i) => format!(
                "{}{}",
                crate::config::HA_ROLE_PREFIX,
                self.ha_names.get(i as usize).map_or("?", |n| n.as_str())
            ),
            r => r.name().to_string(),
        }
    }

    /// Where each configured GIF stands; only the ones a screen shows are
    /// downloaded, the rest are `off`.
    fn gif_rows(&self, model: &Model) -> Vec<GifStatus> {
        self.gif_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let i = i.min(u16::MAX as usize) as u16;
                let used = self
                    .screen_entries
                    .iter()
                    .flatten()
                    .any(|e| e.role == Role::Gif(i));
                gif_status(name, used, model.gif(i))
            })
            .collect()
    }

    fn snapshot(
        &self,
        model: &Model,
        phase: NightPhase,
        now: Secs,
        pattern: Option<(PatternKind, Instant)>,
        heard: &HashSet<LinkTarget>,
        stats: &[ScreenStat],
    ) -> StatusSnapshot {
        let show_on = model.show_run().is_some_and(|r| r.phase(now).is_some());
        let showing = match pattern {
            Some((k, _)) => k.showing(),
            None if phase != NightPhase::Day => "night",
            None if show_on => "show",
            None => "role",
        };
        let screens = self
            .screens
            .iter()
            .map(|s| {
                let st = model.screen(s.index);
                let role = model.current_role(s.index);
                ScreenStatus {
                    index: s.index,
                    role: self.role_name(role),
                    title: role.title().to_string(),
                    roles: st.map_or_else(Vec::new, |st| {
                        st.entries()
                            .iter()
                            .map(|e| self.role_name(e.role))
                            .collect()
                    }),
                    cycle_secs: st.map_or(0, |st| st.cycle_secs().round() as u64),
                    cycle_progress: st.map_or(0.0, |st| st.progress(now)),
                    showing,
                }
            })
            .collect();
        StatusSnapshot {
            night: NightStatus {
                mode: crate::hub::night_mode_name(model.night_override()),
                active: phase != NightPhase::Day,
            },
            screens,
            links: link_rows(model.link(), self.links, heard, model),
            stats: stats.to_vec(),
            tiles: model.tiles().into_iter().map(Into::into).collect(),
            gifs: self.gif_rows(model),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_is_ready_with_jellyfin_or_a_spotify_login() {
        use rackscreen_sources::spotify::{write_token, TokenFile};
        let dir = tempfile::tempdir().unwrap();
        let path = crate::spotify::token_path_in(dir.path());
        assert!(music_ready(true, None));
        assert!(!music_ready(false, None));
        assert!(!music_ready(false, Some(&path)), "no login yet");
        let tok = TokenFile {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 1,
            display_name: None,
            error: None,
        };
        write_token(&path, &tok, None).unwrap();
        assert!(music_ready(false, Some(&path)), "picked up once written");
    }

    #[test]
    fn advance_keeps_cadence_and_resyncs_when_far_behind() {
        let t0 = Instant::now();
        let tick = Duration::from_millis(16);
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(5)), t0 + tick);
        // one tick late: keep the cadence, the next sleep is short
        assert_eq!(advance(t0, tick, t0 + Duration::from_millis(20)), t0 + tick);
        // more than a tick behind the next deadline: start over from now
        let late = t0 + Duration::from_millis(50);
        assert_eq!(advance(t0, tick, late), late);
    }

    #[test]
    fn shutdown_guard_quits_mailboxes_and_sets_stop() {
        let a = Mailbox::new();
        let b = Mailbox::new();
        let stop = Arc::new(AtomicBool::new(false));
        let guard = ShutdownGuard::new(vec![a.clone(), b.clone()], stop.clone());
        assert!(a.try_take().is_none());
        drop(guard);
        assert!(stop.load(Ordering::Relaxed));
        assert!(matches!(a.take(), DisplayCmd::Quit));
        assert!(matches!(b.take(), DisplayCmd::Quit));
    }

    #[test]
    fn link_rows_distinguish_off_waiting_ok_and_down() {
        let mut model = Model::new(Thresholds {
            hot_cpu: 90.0,
            hot_mem: 90.0,
            hot_temp: 70.0,
        });
        let cfg = LinkCfg {
            k8s: true,
            qbit: false,
            electricity: true,
            ..LinkCfg::default()
        };
        let mut heard = HashSet::new();
        let rows = link_rows(model.link(), cfg, &heard, &model);
        let by = |rows: &[LinkStatus], n: &str| rows.iter().find(|r| r.name == n).cloned().unwrap();
        assert_eq!(rows.len(), 8);
        assert_eq!(by(&rows, "Kubernetes").state, "warn");
        assert_eq!(by(&rows, "Kubernetes").detail, "waiting");
        assert_eq!(by(&rows, "qBittorrent").state, "off");
        assert_eq!(by(&rows, "Weather").state, "off");
        assert_eq!(by(&rows, "Electricity Maps").state, "warn");
        model.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            0.0,
        );
        heard.insert(LinkTarget::K8sApi);
        model.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 3,
                not_ready: vec![],
            },
            0.0,
        );
        model.apply(
            Event::Link {
                target: LinkTarget::Electricity,
                up: false,
            },
            0.0,
        );
        heard.insert(LinkTarget::Electricity);
        let rows = link_rows(model.link(), cfg, &heard, &model);
        assert_eq!(by(&rows, "Kubernetes").state, "ok");
        assert_eq!(by(&rows, "Kubernetes").detail, "3 nodes");
        assert_eq!(by(&rows, "Electricity Maps").state, "down");
    }

    #[test]
    fn fake_source_reports_every_link_simulated() {
        let model = Model::new(Thresholds {
            hot_cpu: 90.0,
            hot_mem: 90.0,
            hot_temp: 70.0,
        });
        let cfg = LinkCfg {
            simulated: true,
            ..LinkCfg::default()
        };
        let rows = link_rows(model.link(), cfg, &HashSet::new(), &model);
        assert_eq!(rows.len(), 8);
        assert!(rows
            .iter()
            .all(|r| r.state == "ok" && r.detail == "simulated"));
    }
}

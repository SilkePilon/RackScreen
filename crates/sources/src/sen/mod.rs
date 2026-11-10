//! The `sen` role's source: Sen's live 4K Earth view from the ISS, taken
//! from its YouTube HLS stream at 360p.
//!
//! Nothing runs until a screen wants the stream (`demand`). Then one task
//! fetches segments at the live edge and one thread decodes them, tells live
//! footage from the rest and sends panel frames paced to the stream's
//! clock. When demand has been gone for a while both end and everything
//! they held is freed.

pub mod decode;
pub mod detect;
pub mod player;
pub mod resolve;
pub mod ts;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use rackscreen_core::event::{Event, SenEvent};
use tokio::sync::watch;

use crate::http::{client, read_capped};
use crate::SourceCtx;
use player::{Player, Segment};
use resolve::SegmentUrl;

/// Segments are five seconds long.
const SEGMENT: Duration = Duration::from_secs(5);
/// The stream ends this long after the last screen stopped wanting it.
const LINGER: Duration = Duration::from_secs(10);
/// Stream URLs die after about six hours: resolve again after five.
const RESOLVE_TTL: Duration = Duration::from_secs(5 * 3600);
/// Waits after the stream failed, by failures in a row.
const BACKOFF: [Duration; 4] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
];
/// The next segment's request waits at the server until the segment exists.
const SEGMENT_TIMEOUT: Duration = Duration::from_secs(20);
/// Segments waiting for the decoder; an older one is dropped for a newer.
const QUEUE: usize = 2;
/// While nothing live is on the stream only every n-th segment is fetched.
const PROBE_STEP: u64 = 3;
/// Failed requests in a row before the stream gives up and backs off.
const MAX_ERRORS: u32 = 6;

const MAX_PAGE: usize = 4 * 1024 * 1024;
const MAX_PLAYER: usize = 1024 * 1024;
const MAX_MASTER: usize = 512 * 1024;
const MAX_PLAYLIST: usize = 8 * 1024 * 1024;
/// The end of the media playlist that is kept: its last few segments.
const PLAYLIST_TAIL: usize = 8 * 1024;
/// A 360p segment is some 600 KB.
const MAX_SEGMENT: usize = 4 * 1024 * 1024;

/// The wait after `failures` failures in a row (at least one).
pub fn backoff(failures: u32) -> Duration {
    BACKOFF[(failures.max(1) as usize - 1).min(BACKOFF.len() - 1)]
}

/// A request the server refused in a way that says the URL is dead (403,
/// 404, 410) rather than the network.
#[derive(Debug)]
struct Gone(u16);

impl std::fmt::Display for Gone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}", self.0)
    }
}

impl std::error::Error for Gone {}

fn gone(e: &anyhow::Error) -> Option<u16> {
    e.downcast_ref::<Gone>().map(|g| g.0)
}

async fn get(req: reqwest::RequestBuilder, what: &str) -> Result<reqwest::Response> {
    let resp = req
        .send()
        .await
        .with_context(|| format!("request {what}"))?;
    let status = resp.status();
    if matches!(status.as_u16(), 403 | 404 | 410) {
        return Err(anyhow::Error::new(Gone(status.as_u16())).context(what.to_string()));
    }
    if !status.is_success() {
        bail!("{what}: HTTP {status}");
    }
    Ok(resp)
}

/// The channel's live page, read only as far as the two values in it.
async fn fetch_live_page() -> Result<resolve::LivePage> {
    let req = client()
        .get(resolve::LIVE_PAGE_URL)
        .header(reqwest::header::USER_AGENT, resolve::USER_AGENT)
        .header(reqwest::header::ACCEPT_LANGUAGE, "en");
    let mut resp = get(req, "live page").await?;
    let mut scan = resolve::PageScan::default();
    let mut read = 0usize;
    while let Some(chunk) = resp.chunk().await.context("read live page")? {
        read += chunk.len();
        if read > MAX_PAGE {
            break;
        }
        if let Some(found) = scan.push(&chunk) {
            return Ok(found);
        }
    }
    bail!("the channel is not live (no video on its live page)")
}

/// From the channel to the 360p media playlist's URL.
async fn resolve_stream() -> Result<String> {
    let page = fetch_live_page().await?;
    let req = client()
        .post(resolve::PLAYER_URL)
        .header(reqwest::header::USER_AGENT, resolve::USER_AGENT)
        .header("X-Goog-Visitor-Id", &page.visitor)
        .header("X-YouTube-Client-Name", resolve::CLIENT_NAME_ID)
        .header("X-YouTube-Client-Version", resolve::CLIENT_VERSION)
        .json(&resolve::player_request(&page));
    let body = read_capped(get(req, "player").await?, MAX_PLAYER, "player response").await?;
    let manifest = resolve::parse_player(&body)?;
    let master = read_capped(
        get(client().get(&manifest), "master playlist").await?,
        MAX_MASTER,
        "master playlist",
    )
    .await?;
    resolve::pick_variant(&String::from_utf8_lossy(&master))
}

/// The newest segment of the media playlist. The playlist lists an hour of
/// segments (900 KB, 10 KB gzipped on the wire); only its end is kept.
async fn newest_segment(media_url: &str) -> Result<SegmentUrl> {
    let mut resp = get(client().get(media_url), "media playlist").await?;
    let mut tail: Vec<u8> = Vec::with_capacity(2 * PLAYLIST_TAIL);
    let mut total = 0usize;
    while let Some(chunk) = resp.chunk().await.context("read media playlist")? {
        total += chunk.len();
        if total > MAX_PLAYLIST {
            bail!("media playlist is larger than {MAX_PLAYLIST} bytes");
        }
        tail.extend_from_slice(&chunk);
        if tail.len() > 2 * PLAYLIST_TAIL {
            tail.drain(..tail.len() - PLAYLIST_TAIL);
        }
    }
    resolve::newest_segment(&String::from_utf8_lossy(&tail))
}

async fn fetch_segment(url: &str) -> Result<Vec<u8>> {
    let resp = get(client().get(url).timeout(SEGMENT_TIMEOUT), "segment").await?;
    read_capped(resp, MAX_SEGMENT, "segment").await
}

/// What the fetch task and the decode thread share.
struct Shared {
    queue: Mutex<VecDeque<Segment>>,
    wake: Condvar,
    stop: AtomicBool,
    /// The decoder sees nothing live: segments may be skipped.
    probing: AtomicBool,
}

impl Shared {
    fn queue(&self) -> std::sync::MutexGuard<'_, VecDeque<Segment>> {
        // the queue is plain data: a poisoned lock is still usable
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sleep for `d`; false when the stream is stopping.
    fn wait(&self, d: Duration) -> bool {
        let deadline = Instant::now() + d;
        let mut guard = self.queue();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return false;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return true;
            }
            guard = self.wake_stop(guard, left);
        }
    }

    fn wake_stop<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, VecDeque<Segment>>,
        left: Duration,
    ) -> std::sync::MutexGuard<'a, VecDeque<Segment>> {
        match self.wake.wait_timeout(guard, left) {
            Ok((g, _)) => g,
            Err(e) => e.into_inner().0,
        }
    }

    /// The next segment to decode; `None` when the stream is stopping.
    fn next(&self) -> Option<Segment> {
        let mut guard = self.queue();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(seg) = guard.pop_front() {
                return Some(seg);
            }
            guard = match self.wake.wait(guard) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }
    }
}

/// Counts the streams started, so a decode thread knows at its end whether
/// it still is the newest.
type Generation = Arc<AtomicU64>;

/// The decode thread: plays queued segments until told to stop, then frees
/// the decoder and its buffers and says so, unless a newer stream has
/// started meanwhile (its pictures must not be cleared by this one's end).
fn decode_thread(shared: Arc<Shared>, ctx: SourceCtx, generation: Generation, id: u64) {
    match Player::new(true) {
        Ok(mut player) => {
            while let Some(seg) = shared.next() {
                let was_live = player.live();
                let done = player.play(&seg, &mut |d| shared.wait(d), &mut |e| {
                    ctx.emit(Event::Sen(e))
                });
                shared.probing.store(player.probing(), Ordering::Relaxed);
                if player.live() != was_live {
                    tracing::info!(
                        "sen: {} ({:?})",
                        if player.live() { "live" } else { "not live" },
                        player.verdict()
                    );
                }
                if !done {
                    break;
                }
            }
        }
        Err(e) => tracing::warn!("sen: {e:#}"),
    }
    // the decoder and the frame buffers are gone: give the pages back
    crate::mem::trim();
    if generation.load(Ordering::Relaxed) == id {
        ctx.emit(Event::Sen(SenEvent::Stopped));
    }
}

/// The running decode thread; dropping it stops the thread.
struct Decoder {
    shared: Arc<Shared>,
}

impl Decoder {
    fn spawn(ctx: SourceCtx, generation: &Generation) -> Result<Decoder> {
        let id = generation.fetch_add(1, Ordering::Relaxed) + 1;
        let generation = generation.clone();
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::with_capacity(QUEUE + 1)),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            probing: AtomicBool::new(false),
        });
        let theirs = shared.clone();
        std::thread::Builder::new()
            .name("sen-decode".into())
            .spawn(move || decode_thread(theirs, ctx, generation, id))
            .context("spawn the sen decode thread")?;
        Ok(Decoder { shared })
    }

    /// Queue a segment; the oldest waiting one is dropped when the decoder
    /// is behind.
    fn push(&self, seg: Segment) {
        let mut q = self.shared.queue();
        while q.len() >= QUEUE {
            q.pop_front();
        }
        q.push_back(seg);
        drop(q);
        self.shared.wake.notify_all();
    }

    fn probing(&self) -> bool {
        self.shared.probing.load(Ordering::Relaxed)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.queue().clear();
        self.shared.wake.notify_all();
    }
}

/// Where the segments come from.
#[derive(Clone, Copy)]
pub enum Feed {
    /// Sen's stream.
    Live,
    /// One segment played over and over (the simulator).
    Loop(&'static [u8]),
}

/// A resolved stream: the media playlist's URL and when it was made.
struct Resolved {
    media_url: String,
    at: Instant,
}

/// Fetch and play until something fails; ended from outside by dropping the
/// future. `played` is set once a segment went to the decoder.
async fn stream(
    feed: Feed,
    ctx: &SourceCtx,
    generation: &Generation,
    resolved: &mut Option<Resolved>,
    played: &mut bool,
) -> Result<()> {
    let decoder = Decoder::spawn(ctx.clone(), generation)?;
    if let Feed::Loop(data) = feed {
        let mut sq = 0;
        loop {
            decoder.push(Segment {
                sq,
                data: data.to_vec(),
            });
            *played = true;
            sq += 1;
            tokio::time::sleep(SEGMENT).await;
        }
    }
    let mut errors = 0u32;
    // rounds of resolving and reading the playlist that led to no segment
    let mut idle_rounds = 0u32;
    loop {
        if idle_rounds > 0 {
            if idle_rounds > 3 {
                bail!("the stream gives no segments");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        idle_rounds += 1;
        let fresh = resolved
            .as_ref()
            .filter(|r| r.at.elapsed() < RESOLVE_TTL)
            .map(|r| r.media_url.clone());
        let media_url = match fresh {
            Some(url) => url,
            None => {
                let media_url = resolve_stream().await?;
                tracing::info!("sen: stream resolved");
                *resolved = Some(Resolved {
                    media_url: media_url.clone(),
                    at: Instant::now(),
                });
                media_url
            }
        };
        let newest = match newest_segment(&media_url).await {
            Ok(seg) => seg,
            Err(e) if gone(&e).is_some() => {
                tracing::info!("sen: {e:#}; resolving again");
                *resolved = None;
                continue;
            }
            Err(e) => {
                errors += 1;
                if errors >= MAX_ERRORS {
                    return Err(e);
                }
                tracing::debug!("sen: {e:#}");
                continue;
            }
        };
        let mut sq = newest.sq;
        loop {
            if resolved
                .as_ref()
                .is_none_or(|r| r.at.elapsed() >= RESOLVE_TTL)
            {
                *resolved = None;
                idle_rounds = 0;
                break;
            }
            match fetch_segment(&newest.url(sq)).await {
                Ok(data) => {
                    errors = 0;
                    idle_rounds = 0;
                    *played = true;
                    decoder.push(Segment { sq, data });
                    // nothing live: look at every third segment only
                    let step = if decoder.probing() { PROBE_STEP } else { 1 };
                    if step > 1 {
                        tokio::time::sleep(SEGMENT * (step as u32 - 1)).await;
                    }
                    sq += step;
                }
                // 403: the URLs expired (or the address changed). Others:
                // off the live edge, the playlist says where it is now.
                Err(e) if gone(&e).is_some() => {
                    tracing::debug!("sen: {e:#}");
                    if gone(&e) == Some(403) {
                        *resolved = None;
                    }
                    break;
                }
                Err(e) => {
                    errors += 1;
                    if errors >= MAX_ERRORS {
                        return Err(e);
                    }
                    tracing::debug!("sen: {e:#}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if errors.is_multiple_of(3) {
                        break;
                    }
                }
            }
        }
    }
}

/// Resolves once demand has been off for `LINGER` in one stretch.
async fn demand_gone(demand: &mut watch::Receiver<bool>) {
    loop {
        if demand.wait_for(|on| !*on).await.is_err() {
            return;
        }
        tokio::select! {
            _ = tokio::time::sleep(LINGER) => return,
            r = demand.wait_for(|on| *on) => {
                if r.is_err() {
                    return;
                }
            }
        }
    }
}

/// Run the SEN stream while `demand` is on (the render loop turns it on
/// while a screen shows the role or is about to, on the station's day side)
/// and until `LINGER` after. Failures back off 1, 5, 15, 60 minutes; the
/// role is unavailable meanwhile.
pub async fn run_sen(feed: Feed, ctx: SourceCtx, mut demand: watch::Receiver<bool>) {
    let mut resolved: Option<Resolved> = None;
    let mut failures = 0u32;
    let mut retry_at: Option<Instant> = None;
    let generation = Generation::default();
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            r = demand.wait_for(|on| *on) => {
                if r.is_err() {
                    return;
                }
            }
        }
        if let Some(at) = retry_at {
            let left = at.saturating_duration_since(Instant::now());
            if !left.is_zero() {
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => return,
                    _ = tokio::time::sleep(left) => {}
                }
                continue;
            }
        }
        tracing::info!("sen: stream starts");
        let mut played = false;
        let result = tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = demand_gone(&mut demand) => Ok(()),
            r = stream(feed, &ctx, &generation, &mut resolved, &mut played) => r,
        };
        // the decode thread ends by itself and reports `Stopped`
        match result {
            Ok(()) => {
                tracing::info!("sen: stream stops");
                failures = 0;
                retry_at = None;
            }
            Err(e) => {
                // a stream that played before failing starts over at one minute
                failures = if played { 1 } else { failures + 1 };
                resolved = None;
                let wait = backoff(failures);
                tracing::warn!("sen: {e:#}; next try in {} min", wait.as_secs() / 60);
                retry_at = Some(Instant::now() + wait);
            }
        }
    }
}

/// Resolve the stream and play `secs` seconds of it as fast as it arrives,
/// for a look from the command line: what each segment showed and how many
/// frames came out.
pub async fn probe(secs: u64, mut report: impl FnMut(String)) -> Result<()> {
    let media_url = resolve_stream().await?;
    report("resolved the 360p media playlist".into());
    let newest = newest_segment(&media_url).await?;
    report(format!("newest segment {}", newest.sq));
    let mut player = Player::new(false).map_err(|e| anyhow!("{e:#}"))?;
    let segments = secs.div_ceil(SEGMENT.as_secs()).max(1);
    // start a little behind the edge so the first requests do not wait
    let first = newest.sq.saturating_sub(segments.saturating_sub(1));
    for sq in first..first + segments {
        let t = Instant::now();
        let data = fetch_segment(&newest.url(sq)).await?;
        let fetch_ms = t.elapsed().as_millis();
        let (mut frames, mut changes) = (0, Vec::new());
        let t = Instant::now();
        let seg = Segment { sq, data };
        player.play(&seg, &mut |_| true, &mut |e| match e {
            SenEvent::Frame(_) => frames += 1,
            SenEvent::Live(on) => changes.push(on),
            SenEvent::Stopped => {}
        });
        report(format!(
            "segment {sq}: {} KB in {fetch_ms} ms, played in {} ms, {:?}, live {}, {frames} frames{}",
            seg.data.len() / 1024,
            t.elapsed().as_millis(),
            player.verdict(),
            player.live(),
            if changes.is_empty() {
                String::new()
            } else {
                format!(", live changed to {changes:?}")
            }
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn failures_back_off_1_5_15_60_minutes() {
        let mins: Vec<u64> = (0..7).map(|n| backoff(n).as_secs() / 60).collect();
        assert_eq!(mins, vec![1, 1, 5, 15, 60, 60, 60]);
    }

    #[test]
    fn a_dead_url_is_told_from_a_network_error() {
        let e = anyhow::Error::new(Gone(403)).context("segment");
        assert_eq!(gone(&e), Some(403));
        assert!(format!("{e:#}").contains("HTTP 403"));
        assert_eq!(gone(&anyhow!("timed out")), None);
    }

    fn sen_events(rx: &mpsc::Receiver<Event>, wait: Duration) -> Vec<SenEvent> {
        let deadline = Instant::now() + wait;
        let mut out = Vec::new();
        while let Ok(ev) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            if let Event::Sen(e) = ev {
                let last = e == SenEvent::Stopped;
                out.push(e);
                if last {
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn the_queue_keeps_the_newest_segments() {
        let (tx, rx) = mpsc::channel();
        let ctx = SourceCtx {
            tx,
            shutdown: CancellationToken::new(),
        };
        let generation = Generation::default();
        let d = Decoder::spawn(ctx.clone(), &generation).unwrap();
        // hold the lock so the thread cannot take any while they are queued
        {
            let mut q = d.shared.queue();
            for sq in 0..5 {
                while q.len() >= QUEUE {
                    q.pop_front();
                }
                q.push_back(Segment {
                    sq,
                    data: Vec::new(),
                });
            }
            let left: Vec<u64> = q.iter().map(|s| s.sq).collect();
            assert_eq!(left, vec![3, 4]);
        }
        drop(d);
        let events = sen_events(&rx, Duration::from_secs(5));
        assert_eq!(events.last(), Some(&SenEvent::Stopped));
        // a stream that was replaced by a newer one ends without a word
        let old = Decoder::spawn(ctx.clone(), &generation).unwrap();
        let new = Decoder::spawn(ctx, &generation).unwrap();
        drop(old);
        assert!(sen_events(&rx, Duration::from_millis(300)).is_empty());
        drop(new);
        let events = sen_events(&rx, Duration::from_secs(5));
        assert_eq!(events, vec![SenEvent::Stopped]);
    }

    /// The whole source on the fixture segment: demand starts it, frames
    /// come paced, and it stops and frees everything after demand ends.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn demand_starts_and_stops_the_stream() {
        const LIVE: &[u8] = include_bytes!("../../tests/fixtures/sen_live.ts");
        let (tx, rx) = mpsc::channel();
        let ctx = SourceCtx {
            tx,
            shutdown: CancellationToken::new(),
        };
        let (demand_tx, demand_rx) = watch::channel(false);
        let task = tokio::spawn(run_sen(Feed::Loop(LIVE), ctx.clone(), demand_rx));
        // no demand: nothing happens
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(rx.try_recv().is_err());
        demand_tx.send(true).unwrap();
        // 2.5 s: a second to go live, then 15 frames a second
        let t = Instant::now();
        let events = tokio::task::spawn_blocking(move || {
            let ev = sen_events(&rx, Duration::from_millis(2500));
            (ev, rx)
        })
        .await
        .unwrap();
        let (events, rx) = events;
        let frames = events
            .iter()
            .filter(|e| matches!(e, SenEvent::Frame(_)))
            .count();
        assert!(events.contains(&SenEvent::Live(true)), "{events:?}");
        assert!(
            (10..=26).contains(&frames),
            "{frames} frames in {:?}: paced to 15 fps",
            t.elapsed()
        );
        // shutdown ends the task; the thread reports that it stopped
        ctx.shutdown.cancel();
        task.await.unwrap();
        let rest = tokio::task::spawn_blocking(move || sen_events(&rx, Duration::from_secs(3)))
            .await
            .unwrap();
        assert_eq!(rest.last(), Some(&SenEvent::Stopped));
    }

    #[tokio::test(start_paused = true)]
    async fn demand_has_to_stay_away_to_count_as_gone() {
        let (tx, mut rx) = watch::channel(true);
        let gone = tokio::spawn(async move { demand_gone(&mut rx).await });
        tokio::time::sleep(Duration::from_secs(30)).await;
        assert!(!gone.is_finished(), "demand is on");
        tx.send(false).unwrap();
        tokio::time::sleep(LINGER - Duration::from_secs(1)).await;
        assert!(!gone.is_finished());
        // back within the linger: the count starts over
        tx.send(true).unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        tx.send(false).unwrap();
        tokio::time::sleep(LINGER - Duration::from_secs(1)).await;
        assert!(!gone.is_finished());
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(gone.is_finished());
    }
}

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
pub mod v4l2;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use rackscreen_core::event::{Event, SenEvent};
use tokio::sync::watch;

use crate::http::read_capped;
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
/// The decoder's numbers go to the log this long after a stream started and
/// then at this interval.
const STATS_FIRST: Duration = Duration::from_secs(30);
const STATS_EVERY: Duration = Duration::from_secs(300);

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

/// The stream's own HTTP client. YouTube binds the stream URLs to the
/// address that asked for them, so every request goes out over IPv4: on a
/// dual-stack network the player and the video hosts could otherwise be
/// reached over different families and the segments answer 403. Redirects
/// are followed only to the stream's hosts. It lives as long as the stream
/// runs.
fn stream_client() -> Result<reqwest::Client> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(resolve::USER_AGENT)
        .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
        .redirect(reqwest::redirect::Policy::custom(
            |attempt| match resolve::check_redirect(attempt.url(), attempt.previous().len()) {
                Ok(()) => attempt.follow(),
                Err(e) => attempt.error(e),
            },
        ))
        .build()
        .context("sen http client")
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

/// Unpacks a gzip response body piece by piece (the live page and the
/// media playlist shrink ten to ninety times); a plain body passes through.
struct Inflate(Option<flate2::write::GzDecoder<Vec<u8>>>);

impl Inflate {
    fn of(resp: &reqwest::Response) -> Inflate {
        let gzip = resp
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"gzip"));
        Inflate(gzip.then(|| flate2::write::GzDecoder::new(Vec::new())))
    }

    /// The plain bytes of the next piece of the body.
    fn push<'a>(&'a mut self, chunk: &'a [u8]) -> Result<&'a [u8]> {
        use std::io::Write;
        match &mut self.0 {
            None => Ok(chunk),
            Some(dec) => {
                dec.get_mut().clear();
                dec.write_all(chunk).context("gzip body")?;
                dec.flush().context("gzip body")?;
                Ok(dec.get_ref())
            }
        }
    }
}

/// The channel's live page, read only as far as the two values in it.
async fn fetch_live_page(client: &reqwest::Client) -> Result<resolve::LivePage> {
    let req = client
        .get(resolve::LIVE_PAGE_URL)
        .header(reqwest::header::ACCEPT_ENCODING, "gzip")
        .header(reqwest::header::ACCEPT_LANGUAGE, "en");
    let mut resp = get(req, "live page").await?;
    let mut inflate = Inflate::of(&resp);
    let mut scan = resolve::PageScan::default();
    let mut read = 0usize;
    while let Some(chunk) = resp.chunk().await.context("read live page")? {
        let plain = inflate.push(&chunk)?;
        read += plain.len();
        if read > MAX_PAGE {
            break;
        }
        if let Some(found) = scan.push(plain) {
            return Ok(found);
        }
    }
    bail!("the channel is not live (no video on its live page)")
}

/// From the channel to the 360p media playlist's URL.
async fn resolve_stream(client: &reqwest::Client) -> Result<String> {
    let page = fetch_live_page(client).await?;
    let req = client
        .post(resolve::PLAYER_URL)
        .header("X-Goog-Visitor-Id", &page.visitor)
        .header("X-YouTube-Client-Name", resolve::CLIENT_NAME_ID)
        .header("X-YouTube-Client-Version", resolve::CLIENT_VERSION)
        .json(&resolve::player_request(&page));
    let body = read_capped(get(req, "player").await?, MAX_PLAYER, "player response").await?;
    let manifest = resolve::parse_player(&body)?;
    let master = read_capped(
        get(client.get(&manifest), "master playlist").await?,
        MAX_MASTER,
        "master playlist",
    )
    .await?;
    resolve::pick_variant(&String::from_utf8_lossy(&master))
}

/// The newest segment of the media playlist. The playlist lists an hour of
/// segments (900 KB, 10 KB gzipped on the wire); only its end is kept.
async fn newest_segment(client: &reqwest::Client, media_url: &str) -> Result<SegmentUrl> {
    let req = client
        .get(media_url)
        .header(reqwest::header::ACCEPT_ENCODING, "gzip");
    let mut resp = get(req, "media playlist").await?;
    let mut inflate = Inflate::of(&resp);
    let mut tail: Vec<u8> = Vec::with_capacity(2 * PLAYLIST_TAIL);
    let mut total = 0usize;
    while let Some(chunk) = resp.chunk().await.context("read media playlist")? {
        let plain = inflate.push(&chunk)?;
        total += plain.len();
        if total > MAX_PLAYLIST {
            bail!("media playlist is larger than {MAX_PLAYLIST} bytes");
        }
        // only the end matters: a big piece is cut before it is kept
        let keep = &plain[plain.len().saturating_sub(PLAYLIST_TAIL)..];
        tail.extend_from_slice(keep);
        if tail.len() > 2 * PLAYLIST_TAIL {
            tail.drain(..tail.len() - PLAYLIST_TAIL);
        }
    }
    resolve::newest_segment(&String::from_utf8_lossy(&tail))
}

async fn fetch_segment(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let resp = get(client.get(url).timeout(SEGMENT_TIMEOUT), "segment").await?;
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

/// CPU time this thread has used.
fn thread_cpu() -> Option<Duration> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: the call only writes the timespec it is given.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0 {
            return Some(Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32));
        }
    }
    None
}

/// What the decoder did over a stretch of time, for the log: which one it
/// is, how many pictures, the time in the decoder per picture and the share
/// of one core the whole decode thread used.
fn stats_line(
    name: &str,
    then: decode::Stats,
    now: decode::Stats,
    wall: Duration,
    cpu: Option<Duration>,
) -> String {
    let pictures = now.pictures.saturating_sub(then.pictures);
    let busy = now.busy.saturating_sub(then.busy);
    let each = busy.as_secs_f64() * 1000.0 / pictures.max(1) as f64;
    let mut line = format!(
        "decoder {name}: {pictures} pictures in {:.0} s, {each:.2} ms each",
        wall.as_secs_f64()
    );
    if let Some(cpu) = cpu {
        let share = 100.0 * cpu.as_secs_f64() / wall.as_secs_f64().max(0.001);
        line.push_str(&format!(", thread cpu {share:.1} %"));
    }
    line
}

/// Writes the decoder's numbers to the log now and then.
struct Meter {
    at: Instant,
    every: Duration,
    stats: decode::Stats,
    cpu: Option<Duration>,
}

impl Meter {
    fn new() -> Meter {
        Meter {
            at: Instant::now(),
            every: STATS_FIRST,
            stats: decode::Stats::default(),
            cpu: thread_cpu(),
        }
    }

    fn tick(&mut self, player: &Player) {
        let wall = self.at.elapsed();
        if wall < self.every {
            return;
        }
        let (name, stats) = player.decoder();
        let cpu = thread_cpu();
        let used = cpu
            .zip(self.cpu)
            .map(|(now, then)| now.saturating_sub(then));
        tracing::info!("sen: {}", stats_line(name, self.stats, stats, wall, used));
        *self = Meter {
            at: Instant::now(),
            every: STATS_EVERY,
            stats,
            cpu,
        };
    }
}

/// Pictures shown a second (the config's `sen.fps`); one stream per process.
static FPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(15);

/// Show `fps` pictures a second (10, 15 or 30) from the next stream start.
pub fn set_fps(fps: u32) {
    FPS.store(fps, Ordering::Relaxed);
}

/// Counts the streams started, so a decode thread knows at its end whether
/// it still is the newest.
type Generation = Arc<AtomicU64>;

/// The decode thread: plays queued segments until told to stop, then frees
/// the decoder and its buffers and says so, unless a newer stream has
/// started meanwhile (its pictures must not be cleared by this one's end).
fn decode_thread(shared: Arc<Shared>, ctx: SourceCtx, generation: Generation, id: u64) {
    match Player::new(true).map(|p| p.with_fps(FPS.load(Ordering::Relaxed))) {
        Ok(mut player) => {
            let mut meter = Meter::new();
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
                meter.tick(&player);
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
    let client = stream_client()?;
    let mut errors = 0u32;
    // rounds of resolving and reading the playlist that led to no segment,
    // and what went wrong in the last of them
    let mut idle_rounds = 0u32;
    let mut last_error: Option<anyhow::Error> = None;
    loop {
        if idle_rounds > 0 {
            if idle_rounds > 3 {
                return Err(match last_error {
                    Some(e) => e.context("the stream gives no segments"),
                    None => anyhow!("the stream gives no segments"),
                });
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
                let media_url = resolve_stream(&client).await?;
                tracing::info!("sen: stream resolved");
                *resolved = Some(Resolved {
                    media_url: media_url.clone(),
                    at: Instant::now(),
                });
                media_url
            }
        };
        let newest = match newest_segment(&client, &media_url).await {
            Ok(seg) => seg,
            Err(e) if gone(&e).is_some() => {
                tracing::info!("sen: {e:#}; resolving again");
                *resolved = None;
                last_error = Some(e);
                continue;
            }
            Err(e) => {
                errors += 1;
                if errors >= MAX_ERRORS {
                    return Err(e);
                }
                tracing::debug!("sen: {e:#}");
                last_error = Some(e);
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
            match fetch_segment(&client, &newest.url(sq)).await {
                Ok(data) => {
                    errors = 0;
                    idle_rounds = 0;
                    last_error = None;
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
                    last_error = Some(e);
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
                        last_error = Some(e);
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
    let client = stream_client()?;
    let media_url = resolve_stream(&client).await?;
    report("resolved the 360p media playlist".into());
    let newest = newest_segment(&client, &media_url).await?;
    report(format!("newest segment {}", newest.sq));
    let mut player = Player::new(false).map_err(|e| anyhow!("{e:#}"))?;
    report(format!("decoder {}", player.decoder().0));
    let segments = secs.div_ceil(SEGMENT.as_secs()).max(1);
    // start a little behind the edge so the first requests do not wait
    let first = newest.sq.saturating_sub(segments.saturating_sub(1));
    for sq in first..first + segments {
        let t = Instant::now();
        let data = fetch_segment(&client, &newest.url(sq)).await?;
        // SEN_PROBE_SAVE=<dir> keeps the segments, to look at them later
        if let Some(dir) = std::env::var_os("SEN_PROBE_SAVE") {
            let path = std::path::Path::new(&dir).join(format!("{sq}.ts"));
            std::fs::write(&path, &data).with_context(|| format!("write {}", path.display()))?;
        }
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
    let (name, stats) = player.decoder();
    report(format!(
        "decoder {name} at the end: {} pictures, {:.2} ms each",
        stats.pictures,
        stats.busy.as_secs_f64() * 1000.0 / stats.pictures.max(1) as f64
    ));
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
    fn the_stats_line_names_the_decoder_and_its_cost() {
        let then = decode::Stats {
            units: 10,
            pictures: 100,
            busy: Duration::from_millis(50),
        };
        let now = decode::Stats {
            units: 910,
            pictures: 1000,
            busy: Duration::from_millis(500),
        };
        let wall = Duration::from_secs(30);
        assert_eq!(
            stats_line("v4l2", then, now, wall, Some(Duration::from_millis(600))),
            "decoder v4l2: 900 pictures in 30 s, 0.50 ms each, thread cpu 2.0 %"
        );
        // no pictures and no clock: still a line, no division by zero
        assert_eq!(
            stats_line("software", now, now, Duration::ZERO, None),
            "decoder software: 0 pictures in 0 s, 0.00 ms each"
        );
        // this thread's clock runs
        let a = thread_cpu();
        let mut x = 0u64;
        for i in 0..2_000_000u64 {
            x = std::hint::black_box(x.wrapping_add(i));
        }
        if let (Some(a), Some(b)) = (a, thread_cpu()) {
            assert!(b > a);
        }
    }

    #[test]
    fn a_dead_url_is_told_from_a_network_error() {
        let e = anyhow::Error::new(Gone(403)).context("segment");
        assert_eq!(gone(&e), Some(403));
        assert!(format!("{e:#}").contains("HTTP 403"));
        assert_eq!(gone(&anyhow!("timed out")), None);
    }

    #[test]
    fn gzip_bodies_unpack_piece_by_piece() {
        use std::io::Write;
        let text: Vec<u8> = (0..50_000u32)
            .flat_map(|i| i.to_string().into_bytes())
            .collect();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&text).unwrap();
        let packed = enc.finish().unwrap();
        let mut inflate = Inflate(Some(flate2::write::GzDecoder::new(Vec::new())));
        let mut out = Vec::new();
        for chunk in packed.chunks(1000) {
            out.extend_from_slice(inflate.push(chunk).unwrap());
        }
        assert_eq!(out, text);
        // a plain body passes through; a broken gzip one is an error
        assert_eq!(Inflate(None).push(b"plain").unwrap(), b"plain");
        let mut bad = Inflate(Some(flate2::write::GzDecoder::new(Vec::new())));
        assert!(bad.push(&[0x55; 64]).is_err());
    }

    #[test]
    fn the_stream_client_builds() {
        stream_client().unwrap();
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

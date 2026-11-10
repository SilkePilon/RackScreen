//! Segments in, panel frames out: demux, decode, classify, crop and pace.
//! One `Player` lives on the decode thread for as long as the stream runs.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use rackscreen_core::event::SenEvent;
use rackscreen_core::video::{CropMap, Planes, VideoFrame, VideoRef};

use super::decode::H264;
use super::detect::{classify, Detector, Verdict};
use super::ts::Demuxer;

/// Every n-th decoded picture is looked at and shown: 15 of the stream's 30
/// a second.
pub const SHOW_EVERY: u64 = 2;
/// Live pictures in a row before the stream counts as live: a second.
pub const LIVE_AFTER: u32 = 15;
/// Frame buffers kept for reuse: the one the model holds, the ones the render
/// workers still compare against and the one on its way.
const POOL: usize = 4;
/// A picture this late (the next segment was slow) restarts the clock from
/// it instead of rushing after the lost time.
const LATE: Duration = Duration::from_millis(250);
/// Presentation times are 90 kHz ticks.
const TICKS: u64 = 90_000;
/// A gap this long between two units is a jump in the timeline, not a wait.
const MAX_GAP_TICKS: u64 = 10 * TICKS;

/// One HLS segment.
pub struct Segment {
    /// Its sequence number; consecutive segments decode without a break.
    pub sq: u64,
    pub data: Vec<u8>,
}

/// Maps presentation times onto the wall clock.
#[derive(Default)]
struct Pacer {
    base: Option<(Instant, u64)>,
}

impl Pacer {
    fn reset(&mut self) {
        self.base = None;
    }

    /// How long to wait before the unit at `pts` is due.
    fn delay(&mut self, pts: u64, now: Instant) -> Duration {
        if let Some((t0, p0)) = self.base {
            if pts >= p0 && pts - p0 <= MAX_GAP_TICKS {
                let due = t0 + Duration::from_micros((pts - p0) * 1_000_000 / TICKS);
                if now <= due + LATE {
                    // keep the base near: the offsets stay small
                    if pts - p0 > 4 * TICKS {
                        self.base = Some((due, pts));
                    }
                    return due.saturating_duration_since(now);
                }
            }
        }
        self.base = Some((now, pts));
        Duration::ZERO
    }
}

pub struct Player {
    dec: H264,
    demux: Demuxer,
    detector: Detector,
    map: Option<CropMap>,
    pool: Vec<Arc<VideoFrame>>,
    pacer: Pacer,
    paced: bool,
    last_sq: Option<u64>,
    /// Pictures decoded so far.
    pictures: u64,
    verdict: Option<Verdict>,
}

impl Player {
    /// `paced`: wait for each unit's presentation time (the real stream);
    /// unpaced plays as fast as it decodes (tests, benchmarks).
    pub fn new(paced: bool) -> Result<Player> {
        Ok(Player {
            dec: H264::new()?,
            demux: Demuxer::new(),
            detector: Detector::new(LIVE_AFTER),
            map: None,
            pool: Vec::with_capacity(POOL),
            pacer: Pacer::default(),
            paced,
            last_sq: None,
            pictures: 0,
            verdict: None,
        })
    }

    pub fn live(&self) -> bool {
        self.detector.live()
    }

    /// What the last picture looked at showed.
    pub fn verdict(&self) -> Option<Verdict> {
        self.verdict
    }

    /// Nothing live is in sight: the last picture was not live footage.
    /// The rest of such a segment is skipped, and the fetcher may skip
    /// segments, until a segment starts with live footage again.
    pub fn probing(&self) -> bool {
        self.verdict.is_some_and(|v| v != Verdict::Live)
    }

    /// Play one segment: `emit` gets a frame for every shown picture while
    /// the stream is live, and the changes of that state. `wait` sleeps
    /// until a unit is due and says whether to go on. Returns false when
    /// `wait` stopped it.
    pub fn play(
        &mut self,
        seg: &Segment,
        wait: &mut dyn FnMut(Duration) -> bool,
        emit: &mut dyn FnMut(SenEvent),
    ) -> bool {
        if self.last_sq.is_none_or(|last| last + 1 != seg.sq) {
            self.dec.discontinuity();
            self.pacer.reset();
        }
        self.last_sq = Some(seg.sq);
        self.demux.begin();
        while let Some(au) = self.demux.next(&seg.data) {
            if self.paced {
                if let Some(pts) = au.pts {
                    let delay = self.pacer.delay(pts, Instant::now());
                    if !delay.is_zero() && !wait(delay) {
                        return false;
                    }
                }
            }
            let (detector, map, pool, pictures, verdict) = (
                &mut self.detector,
                &mut self.map,
                &mut self.pool,
                &mut self.pictures,
                &mut self.verdict,
            );
            let mut looked = None;
            self.dec.decode(&au, |_, planes| {
                *pictures += 1;
                if *pictures % SHOW_EVERY != 0 {
                    return;
                }
                let v = classify(planes);
                looked = Some(v);
                *verdict = Some(v);
                let change = detector.update(v);
                if change == Some(false) {
                    emit(SenEvent::Live(false));
                }
                if detector.live() {
                    if let Some(frame) = convert(map, pool, planes) {
                        emit(SenEvent::Frame(frame));
                    }
                }
                // after the frame, so the first live picture is there to show
                if change == Some(true) {
                    emit(SenEvent::Live(true));
                }
            });
            // not live footage: nothing in the rest of this segment is
            // shown, and the next one starts with a keyframe to look at
            if looked.is_some_and(|v| v != Verdict::Live) {
                self.dec.discontinuity();
                self.pacer.reset();
                self.last_sq = None;
                break;
            }
        }
        true
    }
}

/// The panel frame of a picture, in a buffer nobody else holds any more.
fn convert(
    map: &mut Option<CropMap>,
    pool: &mut Vec<Arc<VideoFrame>>,
    planes: &Planes,
) -> Option<VideoRef> {
    if !map.as_ref().is_some_and(|m| m.fits(planes)) {
        *map = CropMap::new(planes.width, planes.height);
    }
    let map = map.as_ref()?;
    let free = pool.iter().position(|f| Arc::strong_count(f) == 1);
    let i = match free {
        Some(i) => i,
        None => {
            // every buffer is still shown somewhere: let the oldest go
            if pool.len() >= POOL {
                pool.remove(0);
            }
            pool.push(Arc::new(VideoFrame::new()));
            pool.len() - 1
        }
    };
    let frame = Arc::get_mut(&mut pool[i])?;
    map.convert(planes, frame)
        .then(|| VideoRef(pool[i].clone()))
}

#[cfg(test)]
mod tests {
    use super::super::detect::tests::{LIVE, SLATE_TS};
    use super::*;

    fn play(player: &mut Player, sq: u64, data: &[u8]) -> Vec<SenEvent> {
        let mut out = Vec::new();
        let seg = Segment {
            sq,
            data: data.to_vec(),
        };
        assert!(player.play(&seg, &mut |_| true, &mut |e| out.push(e)));
        out
    }

    fn frames(events: &[SenEvent]) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, SenEvent::Frame(_)))
            .count()
    }

    #[test]
    fn a_live_segment_goes_live_after_a_second_and_then_shows_15_fps() {
        let mut p = Player::new(false).unwrap();
        let first = play(&mut p, 10, LIVE);
        // 74 looked-at pictures; the first 14 only count towards going live
        assert_eq!(frames(&first), 74 - (LIVE_AFTER as usize - 1));
        let live_at = first
            .iter()
            .position(|e| *e == SenEvent::Live(true))
            .unwrap();
        assert_eq!(live_at, 1, "right after the first frame");
        assert!(matches!(first[0], SenEvent::Frame(_)));
        assert!(p.live() && !p.probing());
        assert_eq!(p.verdict(), Some(Verdict::Live));
        // the next segment follows without a break: every other picture
        let second = play(&mut p, 11, LIVE);
        assert_eq!(frames(&second), 75);
        assert!(!second.iter().any(|e| matches!(e, SenEvent::Live(_))));
    }

    #[test]
    fn the_slate_is_never_shown_and_ends_live() {
        let mut p = Player::new(false).unwrap();
        play(&mut p, 1, LIVE);
        assert!(p.live());
        // the live segment's last picture comes out with the slate's first
        // unit; then the slate ends it
        let slate = play(&mut p, 2, SLATE_TS);
        assert_eq!(slate.len(), 2, "{slate:?}");
        assert!(matches!(slate[0], SenEvent::Frame(_)));
        assert_eq!(slate[1], SenEvent::Live(false));
        assert!(!p.live() && p.probing());
        assert_eq!(p.verdict(), Some(Verdict::Slate));
        // while the slate stays, a segment costs one look
        let before = p.pictures;
        assert!(play(&mut p, 3, SLATE_TS).is_empty());
        assert!(p.pictures - before <= SHOW_EVERY + 1);
        // live footage again: live after a second of it
        let back = play(&mut p, 4, LIVE);
        assert!(back.contains(&SenEvent::Live(true)));
        assert!(frames(&back) > 50);
    }

    #[test]
    fn frame_buffers_are_recycled_once_nobody_holds_them() {
        let mut p = Player::new(false).unwrap();
        let mut held: Vec<VideoRef> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let seg = Segment {
            sq: 1,
            data: LIVE.to_vec(),
        };
        p.play(&seg, &mut |_| true, &mut |e| {
            if let SenEvent::Frame(f) = e {
                // a new frame is never one that is still held
                assert!(held.iter().all(|h| *h != f));
                seen.insert(Arc::as_ptr(&f.0) as usize);
                // hold the newest two, as the model and a worker do
                held.push(f);
                if held.len() > 2 {
                    held.remove(0);
                }
            }
        });
        assert!(seen.len() <= POOL, "{} buffers for 60 frames", seen.len());
        assert!(p.pool.len() <= POOL);
    }

    #[test]
    fn a_holder_of_every_buffer_does_not_grow_the_pool() {
        let mut p = Player::new(false).unwrap();
        let mut held: Vec<VideoRef> = Vec::new();
        let seg = Segment {
            sq: 1,
            data: LIVE.to_vec(),
        };
        p.play(&seg, &mut |_| true, &mut |e| {
            if let SenEvent::Frame(f) = e {
                held.push(f);
            }
        });
        assert!(held.len() > 50);
        assert!(p.pool.len() <= POOL);
    }

    #[test]
    fn damaged_segments_play_without_a_panic() {
        let mut p = Player::new(false).unwrap();
        for (sq, cut) in [(1, 500), (2, 40_000), (3, 100_000), (5, LIVE.len())] {
            play(&mut p, sq, &LIVE[..cut]);
        }
        play(&mut p, 6, &[0x47; 10_000]);
        play(&mut p, 7, &[]);
        let mut bad = LIVE.to_vec();
        for i in (2000..bad.len()).step_by(997) {
            bad[i] ^= 0x5A;
        }
        play(&mut p, 8, &bad);
        // and it still plays a good one after all that
        play(&mut p, 9, LIVE);
        assert!(frames(&play(&mut p, 10, LIVE)) > 50);
    }

    #[test]
    fn a_stop_while_waiting_ends_the_segment() {
        let mut p = Player::new(true).unwrap();
        let seg = Segment {
            sq: 1,
            data: LIVE.to_vec(),
        };
        let mut waits = 0;
        let done = p.play(
            &seg,
            &mut |d| {
                // nothing sleeps here, so the third unit is two frames away
                assert!(d <= Duration::from_millis(100), "{d:?}");
                waits += 1;
                waits < 3
            },
            &mut |_| {},
        );
        assert!(!done);
        assert_eq!(waits, 3);
    }

    #[test]
    fn the_pacer_follows_the_presentation_times() {
        let mut p = Pacer::default();
        let t0 = Instant::now();
        assert_eq!(
            p.delay(1_000_000, t0),
            Duration::ZERO,
            "the first is due now"
        );
        let d = p.delay(1_000_000 + 3000, t0);
        assert!((d.as_micros() as i64 - 33_333).abs() < 10, "{d:?}");
        // a little late: no wait, the clock stays
        let late = t0 + Duration::from_millis(100);
        assert_eq!(p.delay(1_000_000 + 6000, late), Duration::ZERO);
        let d = p.delay(1_000_000 + 12_000, late);
        assert!((d.as_millis() as i64 - 33).abs() <= 1, "{d:?}");
        // very late (a slow segment): the clock restarts from this unit
        let stalled = t0 + Duration::from_secs(3);
        assert_eq!(p.delay(1_000_000 + 15_000, stalled), Duration::ZERO);
        let d = p.delay(1_000_000 + 18_000, stalled);
        assert!((d.as_millis() as i64 - 33).abs() <= 1, "{d:?}");
        // a jump back or far ahead in the timeline restarts it too
        assert_eq!(p.delay(5, stalled), Duration::ZERO);
        assert_eq!(p.delay(5 + 60 * TICKS, stalled), Duration::ZERO);
        // long runs keep their pace across the base moving along
        let mut p = Pacer::default();
        for i in 0..600u64 {
            let now = t0 + Duration::from_micros(i * 33_333);
            let d = p.delay(i * 3000, now);
            assert!(d <= Duration::from_millis(1), "unit {i}: {d:?}");
        }
    }
}

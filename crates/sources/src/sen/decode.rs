//! H.264 decoding for the SEN stream behind one small wrapper: the board's
//! hardware decoder (V4L2) where there is one, OpenH264 otherwise and
//! whenever the hardware fails. Either way the wrapper waits for a keyframe
//! after anything went wrong and never hands out a picture from before a
//! break.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use openh264::OpenH264API;
use rackscreen_core::video::Planes;

use super::ts::{is_keyframe, AccessUnit};
use super::v4l2;

/// Presentation times waiting for their picture; more than a decoder ever
/// reorders.
const MAX_PENDING_PTS: usize = 32;

/// Which decoder to use: `RACKSCREEN_SEN_DECODER=auto|v4l2|software`.
pub const DECODER_ENV: &str = "RACKSCREEN_SEN_DECODER";

/// Failures of the hardware decoder in a row after which it is left alone
/// until the service starts again.
const MAX_STRIKES: u32 = 3;
/// Pictures after which a hardware decoder has shown that it works.
const PROVEN_PICTURES: u64 = 300;

/// Hardware failures in a row, over all streams of this process.
static STRIKES: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// The hardware decoder if it works, else software.
    Auto,
    /// Like `Auto`, and earlier failures do not count against the hardware.
    Hardware,
    Software,
}

impl Choice {
    pub fn parse(value: Option<&str>) -> Choice {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("software" | "sw" | "openh264") => Choice::Software,
            Some("v4l2" | "hardware" | "hw") => Choice::Hardware,
            _ => Choice::Auto,
        }
    }

    fn from_env() -> Choice {
        // the tests' numbers are OpenH264's, whatever the machine has
        if cfg!(test) {
            return Choice::Software;
        }
        Choice::parse(std::env::var(DECODER_ENV).ok().as_deref())
    }
}

/// What the decoder has done so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Access units given to the decoder.
    pub units: u64,
    /// Pictures that came out.
    pub pictures: u64,
    /// Time spent in the decoder, without what was done with the pictures.
    pub busy: Duration,
}

/// OpenH264, with the presentation times of the pictures still to come.
struct Software {
    dec: Decoder,
    /// Times of the units decoded so far whose picture is still to come.
    /// Pictures come out in display order, so each takes the smallest.
    pending: Vec<u64>,
    /// The next unit is the first after a break.
    restart: bool,
}

fn planes<'a>(yuv: &'a DecodedYUV<'a>) -> Planes<'a> {
    let (width, height) = yuv.dimensions();
    let (y_stride, c_stride, _) = yuv.strides();
    Planes {
        y: yuv.y(),
        u: yuv.u(),
        v: yuv.v(),
        width,
        height,
        y_stride,
        c_stride,
    }
}

impl Software {
    fn new() -> Result<Software> {
        // no flushing between units: the decoder hands pictures out in
        // display order by itself
        let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
        let dec = Decoder::with_api_config(OpenH264API::from_source(), config)
            .map_err(|e| anyhow!("h264 decoder: {e}"))?;
        Ok(Software {
            dec,
            pending: Vec::with_capacity(MAX_PENDING_PTS),
            restart: true,
        })
    }

    fn reset(&mut self) {
        self.restart = true;
        self.pending.clear();
    }

    fn decode(
        &mut self,
        au: &AccessUnit,
        on_picture: &mut dyn FnMut(Option<u64>, &Planes),
    ) -> bool {
        // A picture that comes out with the keyframe after a break is the
        // last one from before it (the decoder runs a picture behind).
        let restart = std::mem::take(&mut self.restart);
        if let Some(pts) = au.pts {
            if self.pending.len() >= MAX_PENDING_PTS {
                self.pending.remove(0);
            }
            let at = self.pending.partition_point(|p| *p <= pts);
            self.pending.insert(at, pts);
        }
        match self.dec.decode(au.data) {
            Ok(Some(_)) if restart => true,
            Ok(Some(yuv)) => {
                let pts = (!self.pending.is_empty()).then(|| self.pending.remove(0));
                let p = planes(&yuv);
                if p.valid() {
                    on_picture(pts, &p);
                }
                true
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }
}

enum Backend {
    Hardware(Box<v4l2::Decoder>),
    Software(Software),
    /// The hardware failed and OpenH264 did not start: nothing decodes.
    None,
}

pub struct H264 {
    backend: Backend,
    /// Skip units until the next keyframe (at the start and after an error).
    need_key: bool,
    stats: Stats,
    /// Pictures of the hardware decoder, until it has proven itself.
    proving: Option<u64>,
}

/// The hardware decoder, if it is wanted and opens.
fn hardware(choice: Choice) -> Option<v4l2::Decoder> {
    if choice == Choice::Software {
        return None;
    }
    if choice == Choice::Auto && STRIKES.load(Ordering::Relaxed) >= MAX_STRIKES {
        return None;
    }
    match v4l2::Decoder::open() {
        Ok(dec) => Some(dec),
        Err(e) => {
            // a board without the device has none later either
            let missing = e.chain().any(|c| {
                c.downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
            });
            strike(if missing { MAX_STRIKES } else { 1 });
            tracing::info!("sen: no hardware decoder ({e:#})");
            None
        }
    }
}

fn strike(n: u32) {
    let before = STRIKES.fetch_add(n, Ordering::Relaxed);
    if before < MAX_STRIKES && before + n >= MAX_STRIKES {
        tracing::info!("sen: the hardware decoder is left alone from now on");
    }
}

impl H264 {
    /// The decoder the environment asks for (`DECODER_ENV`), by default the
    /// hardware's with OpenH264 behind it.
    pub fn new() -> Result<H264> {
        H264::with(Choice::from_env())
    }

    pub fn with(choice: Choice) -> Result<H264> {
        let backend = match hardware(choice) {
            Some(dec) => {
                tracing::info!("sen: decoder v4l2 ({})", dec.name());
                Backend::Hardware(Box::new(dec))
            }
            None => {
                tracing::info!("sen: decoder software (openh264)");
                Backend::Software(Software::new()?)
            }
        };
        Ok(H264 {
            proving: matches!(backend, Backend::Hardware(_)).then_some(0),
            backend,
            need_key: true,
            stats: Stats::default(),
        })
    }

    /// The decoder in use: "v4l2" or "software".
    pub fn name(&self) -> &'static str {
        match self.backend {
            Backend::Hardware(_) => "v4l2",
            Backend::Software(_) => "software",
            Backend::None => "none",
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// The next unit does not follow the last one (a segment was skipped):
    /// wait for a keyframe again.
    pub fn discontinuity(&mut self) {
        self.need_key = true;
    }

    /// Decode one access unit. `on_picture` gets each picture that came
    /// out (OpenH264: at most one, a unit behind; the hardware: those of
    /// earlier units that are ready) with its presentation time (90 kHz);
    /// the planes are only valid during the call. Returns false when the
    /// unit was skipped or did not decode.
    pub fn decode(
        &mut self,
        au: &AccessUnit,
        mut on_picture: impl FnMut(Option<u64>, &Planes),
    ) -> bool {
        if self.need_key {
            if !is_keyframe(au.data) {
                return false;
            }
            self.need_key = false;
            match &mut self.backend {
                Backend::Hardware(dec) => dec.reset(),
                Backend::Software(dec) => dec.reset(),
                Backend::None => {}
            }
        }
        let start = Instant::now();
        let mut outside = Duration::ZERO;
        let mut pictures = 0u64;
        let mut timed = |pts: Option<u64>, p: &Planes| {
            let t = Instant::now();
            on_picture(pts, p);
            outside += t.elapsed();
            pictures += 1;
        };
        let ok = match &mut self.backend {
            Backend::Hardware(dec) => match dec.decode(au.data, au.pts, &mut timed) {
                Ok(ok) => ok,
                Err(e) => {
                    self.fall_back(&e);
                    false
                }
            },
            Backend::Software(dec) => dec.decode(au, &mut timed),
            Backend::None => false,
        };
        self.stats.units += 1;
        self.stats.pictures += pictures;
        self.stats.busy += start.elapsed().saturating_sub(outside);
        if let Some(n) = &mut self.proving {
            *n += pictures;
            if *n >= PROVEN_PICTURES {
                STRIKES.store(0, Ordering::Relaxed);
                self.proving = None;
            }
        }
        if !ok {
            self.need_key = true;
        }
        ok
    }

    /// The hardware decoder failed: close it and go on in software from
    /// the next keyframe.
    fn fall_back(&mut self, why: &anyhow::Error) {
        // dropped first, so the device is closed whatever comes
        self.backend = Backend::None;
        self.proving = None;
        strike(1);
        match Software::new() {
            Ok(dec) => {
                tracing::warn!(
                    "sen: hardware decoder failed ({why:#}); decoder software (openh264)"
                );
                self.backend = Backend::Software(dec);
            }
            Err(e) => tracing::warn!("sen: hardware decoder failed ({why:#}) and {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ts::Demuxer;
    use super::*;

    const LIVE: &[u8] = include_bytes!("../../tests/fixtures/sen_live.ts");

    #[test]
    fn the_fixture_decodes_to_360p_pictures_in_order() {
        let mut dec = H264::new().unwrap();
        let mut demux = Demuxer::new();
        demux.begin();
        let mut times = Vec::new();
        while let Some(au) = demux.next(LIVE) {
            dec.decode(&au, |pts, p| {
                assert_eq!((p.width, p.height), (640, 360));
                assert!(p.valid());
                times.push(pts.unwrap());
            });
        }
        // a picture comes out when the next unit arrives, so the last one
        // waits for the next segment
        assert_eq!(times.len(), 149);
        assert!(times.windows(2).all(|w| w[1] - w[0] == 3000));
    }

    #[test]
    fn decoding_starts_at_a_keyframe_and_survives_damage() {
        let mut dec = H264::new().unwrap();
        let mut demux = Demuxer::new();
        // a segment cut short (inside its keyframe, then after it), garbage,
        // then the whole segment again
        let mut pictures = 0;
        for data in [&LIVE[..40_000], &LIVE[..100_000], &[0x47u8; 4000][..], LIVE] {
            demux.begin();
            dec.discontinuity();
            while let Some(au) = demux.next(data) {
                dec.decode(&au, |_, _| pictures += 1);
            }
        }
        assert!(pictures > 160, "{pictures} pictures");
        // after a break, the picture left over from before it is not handed out
        let slate: &[u8] = include_bytes!("../../tests/fixtures/sen_slate.ts");
        demux.begin();
        dec.discontinuity();
        let mut first = None;
        let mut units = 0;
        while let Some(au) = demux.next(slate) {
            units += 1;
            let n = units;
            dec.decode(&au, |pts, _| {
                first.get_or_insert((n, pts, au.pts));
            });
        }
        let (unit, pts, unit_pts) = first.unwrap();
        assert_eq!(unit, 2, "the keyframe's picture comes with the next unit");
        assert_eq!(pts.unwrap() + 3000, unit_pts.unwrap());
        // units that are not H.264 at all
        let junk = AccessUnit {
            pts: Some(1),
            data: &[0, 0, 0, 1, 0x65, 0xFF, 0xFF, 0xFF, 0xFF],
        };
        dec.discontinuity();
        dec.decode(&junk, |_, _| {});
        assert!(!dec.decode(
            &AccessUnit {
                pts: None,
                data: &[0, 0, 1, 0x41, 1, 2, 3]
            },
            |_, _| {}
        ));
    }

    #[test]
    fn the_decoder_is_chosen_by_name() {
        assert_eq!(Choice::parse(None), Choice::Auto);
        assert_eq!(Choice::parse(Some("auto")), Choice::Auto);
        assert_eq!(Choice::parse(Some("")), Choice::Auto);
        assert_eq!(Choice::parse(Some("whatever")), Choice::Auto);
        assert_eq!(Choice::parse(Some("software")), Choice::Software);
        assert_eq!(Choice::parse(Some(" Software\n")), Choice::Software);
        assert_eq!(Choice::parse(Some("v4l2")), Choice::Hardware);
        assert_eq!(Choice::parse(Some("V4L2")), Choice::Hardware);
        // the tests themselves never touch a device
        assert_eq!(Choice::from_env(), Choice::Software);
        assert_eq!(H264::new().unwrap().name(), "software");
        assert_eq!(H264::with(Choice::Software).unwrap().name(), "software");
    }

    #[test]
    fn the_stats_count_units_pictures_and_the_time_in_the_decoder() {
        let mut dec = H264::new().unwrap();
        let mut demux = Demuxer::new();
        demux.begin();
        let mut units = 0;
        let wall = Instant::now();
        let with_picture = Duration::from_millis(1);
        while let Some(au) = demux.next(LIVE) {
            units += 1;
            dec.decode(&au, |_, _| std::thread::sleep(with_picture));
        }
        let wall = wall.elapsed();
        let s = dec.stats();
        assert_eq!((s.units, s.pictures), (units, 149));
        assert!(s.busy > Duration::ZERO);
        // time spent with the pictures is not the decoder's
        assert!(s.busy + 149 * with_picture <= wall, "{:?}", s.busy);
        // skipped units are not counted: nothing was done with them
        let mut dec = H264::new().unwrap();
        dec.decode(
            &AccessUnit {
                pts: None,
                data: &[0, 0, 1, 0x41, 1, 2, 3],
            },
            |_, _| {},
        );
        assert_eq!(dec.stats(), Stats::default());
    }
}

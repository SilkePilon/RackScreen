//! H.264 decoding for the SEN stream: OpenH264 behind a small wrapper that
//! pairs pictures with presentation times and waits for a keyframe after
//! anything went wrong.

use anyhow::{anyhow, Result};
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use openh264::OpenH264API;
use rackscreen_core::video::Planes;

use super::ts::{is_keyframe, AccessUnit};

/// Presentation times waiting for their picture; more than a decoder ever
/// reorders.
const MAX_PENDING_PTS: usize = 32;

pub struct H264 {
    dec: Decoder,
    /// Times of the units decoded so far whose picture is still to come.
    /// Pictures come out in display order, so each takes the smallest.
    pending: Vec<u64>,
    /// Skip units until the next keyframe (at the start and after an error).
    need_key: bool,
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

impl H264 {
    pub fn new() -> Result<H264> {
        // no flushing between units: the decoder hands pictures out in
        // display order by itself
        let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
        let dec = Decoder::with_api_config(OpenH264API::from_source(), config)
            .map_err(|e| anyhow!("h264 decoder: {e}"))?;
        Ok(H264 {
            dec,
            pending: Vec::with_capacity(MAX_PENDING_PTS),
            need_key: true,
        })
    }

    /// The next unit does not follow the last one (a segment was skipped):
    /// wait for a keyframe again.
    pub fn discontinuity(&mut self) {
        self.need_key = true;
        self.pending.clear();
    }

    /// Decode one access unit. `on_picture` gets the picture that came out,
    /// if one did, with its presentation time (90 kHz); the planes are only
    /// valid during the call. Returns false when the unit was skipped or
    /// did not decode.
    pub fn decode(
        &mut self,
        au: &AccessUnit,
        on_picture: impl FnOnce(Option<u64>, &Planes),
    ) -> bool {
        // A picture that comes out with the keyframe after a break is the
        // last one from before it (the decoder runs a picture behind).
        let restart = self.need_key;
        if restart {
            if !is_keyframe(au.data) {
                return false;
            }
            self.need_key = false;
            self.pending.clear();
        }
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
            Err(_) => {
                self.need_key = true;
                false
            }
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
}

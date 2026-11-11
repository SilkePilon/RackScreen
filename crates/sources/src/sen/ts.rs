//! A small MPEG-TS demuxer for the SEN stream's HLS segments: PAT → PMT →
//! the H.264 PID → PES packets, each handed out as one Annex B access unit
//! with its presentation time. Audio and everything else is dropped.
//!
//! Sections are expected to fit in one TS packet (they do in every muxer's
//! PAT/PMT) and are not CRC-checked: a wrong table at worst picks a PID with
//! no decodable video, which the decoder rejects.

const PACKET: usize = 188;
const SYNC: u8 = 0x47;
const STREAM_TYPE_H264: u8 = 0x1B;
/// An access unit larger than this is dropped: 360p frames are a few KB,
/// a keyframe some tens of KB.
const MAX_AU: usize = 2 * 1024 * 1024;

/// One video access unit: Annex B bytes and the PTS in 90 kHz ticks.
pub struct AccessUnit<'a> {
    pub pts: Option<u64>,
    pub data: &'a [u8],
}

/// Demuxer state; reuse one across segments so its buffers are recycled.
/// Each segment starts with `begin` (HLS segments carry their own PAT/PMT).
#[derive(Default)]
pub struct Demuxer {
    pos: usize,
    pmt_pid: Option<u16>,
    video_pid: Option<u16>,
    /// The PES being collected and its PTS.
    cur: Vec<u8>,
    cur_pts: Option<u64>,
    /// A PES is open in `cur` (its start was seen and nothing was lost).
    open: bool,
    /// The last finished PES, lent out by `next`.
    out: Vec<u8>,
    cc: Option<u8>,
    flushed: bool,
}

impl Demuxer {
    pub fn new() -> Demuxer {
        Demuxer::default()
    }

    /// Start a new segment.
    pub fn begin(&mut self) {
        self.pos = 0;
        self.pmt_pid = None;
        self.video_pid = None;
        self.cur.clear();
        self.cur_pts = None;
        self.open = false;
        self.cc = None;
        self.flushed = false;
    }

    /// The next video access unit of `data` (the same segment on every call
    /// since `begin`), `None` at its end.
    pub fn next<'a>(&'a mut self, data: &[u8]) -> Option<AccessUnit<'a>> {
        while let Some(pkt) = self.next_packet(data) {
            if let Some(pts) = self.packet(pkt) {
                return Some(AccessUnit {
                    pts,
                    data: &self.out,
                });
            }
        }
        // the last PES ends with the segment
        if !self.flushed {
            self.flushed = true;
            if self.open && !self.cur.is_empty() {
                self.open = false;
                std::mem::swap(&mut self.cur, &mut self.out);
                self.cur.clear();
                return Some(AccessUnit {
                    pts: self.cur_pts,
                    data: &self.out,
                });
            }
        }
        None
    }

    /// The next 188-byte packet, resynchronising on the sync byte after
    /// garbage; `None` when less than a packet is left.
    fn next_packet<'d>(&mut self, data: &'d [u8]) -> Option<&'d [u8]> {
        loop {
            let rest = data.get(self.pos..)?;
            if rest.len() < PACKET {
                return None;
            }
            let aligned = rest[0] == SYNC && rest.get(PACKET).is_none_or(|b| *b == SYNC);
            if aligned {
                self.pos += PACKET;
                return Some(&rest[..PACKET]);
            }
            self.pos += 1;
        }
    }

    /// Take one packet in; `Some(pts)` when it completed a PES into `out`.
    fn packet(&mut self, p: &[u8]) -> Option<Option<u64>> {
        if p[1] & 0x80 != 0 {
            // transport error indicator: the payload cannot be trusted
            self.lose(p);
            return None;
        }
        let pusi = p[1] & 0x40 != 0;
        let pid = ((p[1] as u16 & 0x1F) << 8) | p[2] as u16;
        let afc = (p[3] >> 4) & 0x3;
        let cc = p[3] & 0x0F;
        let mut off = 4;
        if afc & 0x2 != 0 {
            off += 1 + p[4] as usize;
        }
        // no payload, or an adaptation field that runs past the packet
        if afc & 0x1 == 0 || off >= PACKET {
            return None;
        }
        let payload = &p[off..];
        if pid == 0 {
            self.pmt_pid = self.pmt_pid.or_else(|| pat(section(payload, pusi)?));
            return None;
        }
        if Some(pid) == self.pmt_pid {
            self.video_pid = self.video_pid.or_else(|| pmt(section(payload, pusi)?));
            return None;
        }
        if Some(pid) != self.video_pid {
            return None;
        }
        // a repeated counter is a duplicate packet, anything else but +1 a loss
        match self.cc.replace(cc) {
            Some(prev) if prev == cc => return None,
            Some(prev) if (prev + 1) & 0x0F != cc => self.drop_open(),
            _ => {}
        }
        let mut done = None;
        if pusi {
            if self.open && !self.cur.is_empty() {
                std::mem::swap(&mut self.cur, &mut self.out);
                done = Some(self.cur_pts);
            }
            self.cur.clear();
            match pes_header(payload) {
                Some((pts, body)) => {
                    self.open = true;
                    self.cur_pts = pts;
                    self.cur.extend_from_slice(body);
                }
                None => self.drop_open(),
            }
        } else if self.open {
            if self.cur.len() + payload.len() > MAX_AU {
                self.drop_open();
            } else {
                self.cur.extend_from_slice(payload);
            }
        }
        done
    }

    /// A damaged packet: if it is video, the open PES is lost.
    fn lose(&mut self, p: &[u8]) {
        let pid = ((p[1] as u16 & 0x1F) << 8) | p[2] as u16;
        if Some(pid) == self.video_pid {
            self.drop_open();
            self.cc = None;
        }
    }

    /// Forget the PES being collected, its time included.
    fn drop_open(&mut self) {
        self.open = false;
        self.cur.clear();
        self.cur_pts = None;
    }
}

/// The section that starts in this payload (after the pointer field), from
/// its table id to the end of the packet.
fn section(payload: &[u8], pusi: bool) -> Option<&[u8]> {
    if !pusi {
        return None;
    }
    let pointer = *payload.first()? as usize;
    payload.get(1 + pointer..)
}

/// Section body between the 8-byte header and the CRC, by `section_length`.
fn section_body(s: &[u8], table_id: u8) -> Option<&[u8]> {
    if *s.first()? != table_id {
        return None;
    }
    let len = ((*s.get(1)? as usize & 0x0F) << 8) | *s.get(2)? as usize;
    // the length counts from byte 3 and includes the 4 CRC bytes
    let end = (3 + len).checked_sub(4)?;
    s.get(8..end.min(s.len()))
}

/// The PMT PID of the first programme in a PAT section.
fn pat(s: &[u8]) -> Option<u16> {
    section_body(s, 0x00)?
        .as_chunks::<4>()
        .0
        .iter()
        .find(|e| e[0] != 0 || e[1] != 0)
        .map(|e| ((e[2] as u16 & 0x1F) << 8) | e[3] as u16)
}

/// The PID of the first H.264 stream in a PMT section.
fn pmt(s: &[u8]) -> Option<u16> {
    let body = section_body(s, 0x02)?;
    let info_len = ((*body.get(2)? as usize & 0x0F) << 8) | *body.get(3)? as usize;
    let mut es = body.get(4 + info_len..)?;
    while es.len() >= 5 {
        let pid = ((es[1] as u16 & 0x1F) << 8) | es[2] as u16;
        if es[0] == STREAM_TYPE_H264 {
            return Some(pid);
        }
        let es_info = ((es[3] as usize & 0x0F) << 8) | es[4] as usize;
        es = es.get(5 + es_info..)?;
    }
    None
}

/// The PTS (90 kHz) and the elementary stream bytes after a PES header.
fn pes_header(p: &[u8]) -> Option<(Option<u64>, &[u8])> {
    if p.len() < 9 || p[0] != 0 || p[1] != 0 || p[2] != 1 {
        return None;
    }
    let flags = p[7];
    let body = p.get(9 + p[8] as usize..)?;
    let pts = if flags & 0x80 != 0 && p[8] >= 5 && p.len() >= 14 {
        let b = &p[9..14];
        Some(
            ((b[0] as u64 >> 1) & 0x07) << 30
                | (b[1] as u64) << 22
                | (b[2] as u64 >> 1) << 15
                | (b[3] as u64) << 7
                | (b[4] as u64 >> 1),
        )
    } else {
        None
    };
    Some((pts, body))
}

/// The NAL unit types in an Annex B access unit, in order.
pub fn nal_types(au: &[u8]) -> impl Iterator<Item = u8> + '_ {
    let mut i = 0;
    std::iter::from_fn(move || {
        while i + 3 < au.len() {
            if au[i] == 0 && au[i + 1] == 0 && au[i + 2] == 1 {
                let t = au[i + 3] & 0x1F;
                i += 4;
                return Some(t);
            }
            i += 1;
        }
        None
    })
}

/// The access unit can start decoding: it carries an IDR slice.
pub fn is_keyframe(au: &[u8]) -> bool {
    nal_types(au).any(|t| t == 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE: &[u8] = include_bytes!("../../tests/fixtures/sen_live.ts");
    const SLATE: &[u8] = include_bytes!("../../tests/fixtures/sen_slate.ts");

    fn units(data: &[u8]) -> Vec<(Option<u64>, Vec<u8>)> {
        let mut d = Demuxer::new();
        d.begin();
        let mut out = Vec::new();
        while let Some(au) = d.next(data) {
            out.push((au.pts, au.data.to_vec()));
        }
        out
    }

    #[test]
    fn a_live_segment_gives_five_seconds_of_access_units() {
        let aus = units(LIVE);
        assert_eq!(aus.len(), 150, "5 s at 30 fps");
        assert!(is_keyframe(&aus[0].1), "a segment starts with an IDR");
        assert!(nal_types(&aus[0].1).any(|t| t == 7), "and its SPS");
        for (pts, au) in &aus {
            assert!(pts.is_some());
            assert_eq!(&au[..3], &[0, 0, 0], "Annex B start code");
            assert!(
                nal_types(au).any(|t| t == 1 || t == 5),
                "a slice in every unit"
            );
        }
        let mut pts: Vec<u64> = aus.iter().map(|(p, _)| p.unwrap()).collect();
        pts.sort_unstable();
        assert!(
            pts.windows(2).all(|w| w[1] - w[0] == 3000),
            "30 fps in 90 kHz ticks"
        );
    }

    #[test]
    fn the_slate_segment_demuxes_too() {
        let aus = units(SLATE);
        assert!(aus.len() >= 100);
        assert!(is_keyframe(&aus[0].1));
    }

    #[test]
    fn the_demuxer_is_reusable() {
        let mut d = Demuxer::new();
        for data in [LIVE, SLATE, LIVE] {
            d.begin();
            let mut n = 0;
            while d.next(data).is_some() {
                n += 1;
            }
            assert_eq!(n, units(data).len());
        }
    }

    #[test]
    fn truncated_and_damaged_segments_never_panic() {
        // every cut of the first packets, then coarser cuts of the rest
        let cuts = (0..2000).chain((2000..LIVE.len()).step_by(977));
        for cut in cuts {
            let n = units(&LIVE[..cut]).len();
            assert!(n <= 150);
        }
        // flipped bytes all over, a fixed pattern
        let mut bad = LIVE.to_vec();
        let mut x = 0x9E37_79B9u32;
        for _ in 0..4000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % bad.len();
            bad[i] ^= (x >> 24) as u8 | 1;
        }
        let _ = units(&bad);
        // a stray byte after the tables, a misaligned start, pure noise
        let mut shifted = LIVE[..3 * PACKET].to_vec();
        shifted.push(0xFF);
        shifted.extend_from_slice(&LIVE[3 * PACKET..]);
        assert!(units(&shifted).len() >= 148, "resyncs after a stray byte");
        let _ = units(&LIVE[1..]);
        assert!(units(&[0x47; 5000]).is_empty());
        assert!(units(&[]).is_empty());
    }

    #[test]
    fn a_gap_forgets_the_open_unit_and_its_time() {
        let mut d = Demuxer::new();
        d.begin();
        // far enough in to have a unit open
        let mut n = 0;
        while d.next_packet(LIVE).is_some_and(|p| {
            d.packet(p);
            n += 1;
            n < 20
        }) {}
        assert!(d.open && d.cur_pts.is_some() && !d.cur.is_empty());
        d.drop_open();
        assert!(!d.open && d.cur_pts.is_none() && d.cur.is_empty());
    }

    #[test]
    fn a_lost_packet_drops_only_its_access_unit() {
        // drop one video packet from the middle of the segment
        let cut = (LIVE.len() / 2 / PACKET) * PACKET;
        let mut data = LIVE[..cut].to_vec();
        data.extend_from_slice(&LIVE[cut + PACKET..]);
        let n = units(&data).len();
        assert!((148..=150).contains(&n), "{n} units");
    }
}

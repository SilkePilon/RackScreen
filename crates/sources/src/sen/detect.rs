//! Tells live Earth footage from everything else the SEN stream shows: the
//! camera-switch slate, replays (orange badge), promos without the HUD and
//! the night side. A few thousand samples of the I420 planes per check.
//!
//! All positions are in the 640×360 picture and scale with the real size.

use rackscreen_core::video::Planes;

const REF_W: usize = 640;
const REF_H: usize = 360;

/// What one picture shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Live Earth footage.
    Live,
    /// The teal "switching camera" card.
    Slate,
    /// No red LIVE badge: a replay (orange badge) or other content.
    NoBadge,
    /// No info bar: a promo or other full-frame content.
    NoHud,
    /// The picture is black: the night side.
    Dark,
}

/// The slate signature grid: 40 px cells over the picture above the info
/// bar. The cells under the badge and the logo are left out.
const GRID_COLS: usize = 16;
const GRID_ROWS: usize = 7;
const CELL: usize = 40;

/// `(Y, Cb, Cr)` cell means of the slate, from the `sen_slate.ts` fixture
/// (the test `the_slate_signature_matches_the_fixture` prints a fresh one).
#[rustfmt::skip]
const SLATE: [[u8; 3]; GRID_COLS * GRID_ROWS] = [
    [51,134,129],[52,134,131],[42,137,119],[42,137,119],[40,136,121],[38,135,122],[35,135,124],[31,134,127],[27,133,131],[23,131,134],[20,129,136],[19,127,136],[19,127,135],[19,127,134],[31,128,133],[51,128,132],
    [42,137,119],[42,137,119],[42,137,119],[42,136,119],[41,136,119],[39,136,120],[37,135,122],[34,134,123],[30,133,127],[26,132,131],[22,131,133],[20,129,134],[19,129,133],[19,129,132],[20,130,131],[20,130,130],
    [42,137,119],[42,137,119],[42,137,119],[42,136,119],[41,137,118],[41,137,116],[40,137,114],[38,136,115],[36,135,116],[33,134,119],[29,133,123],[25,132,126],[25,132,125],[26,133,123],[27,133,121],[28,133,121],
    [42,137,119],[42,137,119],[42,137,118],[42,137,117],[43,137,113],[44,138,109],[46,138,108],[47,137,107],[47,137,107],[45,136,108],[42,136,110],[41,135,111],[40,135,111],[41,135,111],[41,135,111],[41,135,111],
    [42,137,119],[42,137,118],[42,137,115],[44,138,111],[46,138,108],[50,139,105],[119,134,112],[162,132,117],[162,132,116],[123,135,107],[63,138,97],[62,138,97],[60,137,98],[58,137,99],[57,137,100],[56,137,101],
    [42,137,119],[42,137,118],[44,138,112],[46,138,108],[51,139,104],[56,139,101],[77,140,95],[92,141,86],[94,140,86],[89,141,84],[82,140,84],[82,140,84],[79,140,86],[75,139,89],[72,139,90],[71,138,91],
    [42,137,119],[42,137,116],[45,138,109],[49,139,106],[55,139,102],[62,140,97],[69,140,92],[77,141,87],[84,141,83],[92,142,78],[97,142,74],[98,142,74],[93,141,77],[88,141,80],[85,140,82],[83,140,83],
];

/// Mean absolute difference per value under which a picture is the slate.
/// Live pictures are 15 and more away, the slate itself under 2.
const SLATE_MAX_DIFF: u32 = 6;

/// The crop is dark below this mean luma (video black is 16).
const DARK_LUMA: u32 = 28;

struct Pic<'a> {
    p: &'a Planes<'a>,
}

impl Pic<'_> {
    fn sx(&self, x: usize) -> usize {
        (x * self.p.width / REF_W).min(self.p.width - 1)
    }
    fn sy(&self, y: usize) -> usize {
        (y * self.p.height / REF_H).min(self.p.height - 1)
    }
    /// Mean luma of a reference-space rectangle, every `step`-th sample.
    fn luma(&self, x0: usize, x1: usize, y0: usize, y1: usize, step: usize) -> u32 {
        mean(
            self.p.y,
            self.p.y_stride,
            (self.sx(x0), self.sx(x1)),
            (self.sy(y0), self.sy(y1)),
            step,
        )
    }
    /// Mean `(Cb, Cr)` of a reference-space rectangle.
    fn chroma(&self, x0: usize, x1: usize, y0: usize, y1: usize, step: usize) -> (u32, u32) {
        let xs = (self.sx(x0) / 2, self.sx(x1) / 2);
        let ys = (self.sy(y0) / 2, self.sy(y1) / 2);
        (
            mean(self.p.u, self.p.c_stride, xs, ys, step),
            mean(self.p.v, self.p.c_stride, xs, ys, step),
        )
    }
}

fn mean(plane: &[u8], stride: usize, xs: (usize, usize), ys: (usize, usize), step: usize) -> u32 {
    let (mut sum, mut n) = (0u32, 0u32);
    for y in (ys.0..ys.1.max(ys.0 + 1)).step_by(step) {
        let row = &plane[y * stride..];
        for x in (xs.0..xs.1.max(xs.0 + 1)).step_by(step) {
            sum += row[x] as u32;
            n += 1;
        }
    }
    sum / n.max(1)
}

/// Whether a signature cell is compared (not under the badge or the logo).
fn cell_used(col: usize, row: usize) -> bool {
    row > 0 || (2..GRID_COLS - 2).contains(&col)
}

/// The picture's `(Y, Cb, Cr)` cell means, in `SLATE`'s layout.
pub fn signature(p: &Planes) -> [[u8; 3]; GRID_COLS * GRID_ROWS] {
    let mut out = [[0u8; 3]; GRID_COLS * GRID_ROWS];
    if !p.valid() {
        return out;
    }
    let pic = Pic { p };
    for row in 0..GRID_ROWS {
        for col in 0..GRID_COLS {
            let (x, y) = (col * CELL, row * CELL);
            let (cb, cr) = pic.chroma(x, x + CELL, y, y + CELL, 2);
            out[row * GRID_COLS + col] = [
                pic.luma(x, x + CELL, y, y + CELL, 4) as u8,
                cb as u8,
                cr as u8,
            ];
        }
    }
    out
}

fn is_slate(p: &Planes) -> bool {
    let sig = signature(p);
    let (mut diff, mut n) = (0u32, 0u32);
    for (i, (a, b)) in sig.iter().zip(&SLATE).enumerate() {
        if cell_used(i % GRID_COLS, i / GRID_COLS) {
            diff += a
                .iter()
                .zip(b)
                .map(|(a, b)| a.abs_diff(*b) as u32)
                .sum::<u32>();
            n += 3;
        }
    }
    diff < SLATE_MAX_DIFF * n
}

/// The badge at (18..62, 15..32) is red: strong Cr with Cb near the middle.
/// The orange replay badge has the Cr but a much lower Cb.
fn badge_is_red(pic: &Pic) -> bool {
    let (cb, cr) = pic.chroma(20, 60, 18, 30, 1);
    cr >= 160 && (105..=140).contains(&cb)
}

/// The three cyan labels at the right of the info bar ("TIME UTC" and the
/// two under it) and the white digits next to each.
const LABEL_X: (usize, usize) = (480, 512);
const DIGITS_X: (usize, usize) = (524, 560);
const LABEL_ROWS: [(usize, usize); 3] = [(316, 322), (328, 334), (340, 346)];
/// The labels' summed Cr is at least this far under the digits': 30 to 55
/// on every picture with the bar (day, night, slate), about 0 without it.
const LABEL_CR_GAP: u32 = 18;

/// The info bar is there: its labels are cyan (low Cr) next to neutral
/// digits, whatever the picture behind the bar looks like. (A step in
/// brightness at the bar's top edge is not reliable: over dark ocean with
/// clouds the picture differs more across the edge than the bar dims it.)
fn has_hud(pic: &Pic) -> bool {
    let (mut labels, mut digits) = (0, 0);
    for (y0, y1) in LABEL_ROWS {
        labels += pic.chroma(LABEL_X.0, LABEL_X.1, y0, y1, 1).1;
        digits += pic.chroma(DIGITS_X.0, DIGITS_X.1, y0, y1, 1).1;
    }
    labels + LABEL_CR_GAP <= digits
}

fn is_dark(pic: &Pic) -> bool {
    pic.luma(180, 460, 15, 295, 8) < DARK_LUMA
}

/// Classify one picture.
pub fn classify(p: &Planes) -> Verdict {
    if !p.valid() {
        return Verdict::NoHud;
    }
    let pic = Pic { p };
    if is_slate(p) {
        Verdict::Slate
    } else if !badge_is_red(&pic) {
        Verdict::NoBadge
    } else if !has_hud(&pic) {
        Verdict::NoHud
    } else if is_dark(&pic) {
        Verdict::Dark
    } else {
        Verdict::Live
    }
}

/// Pictures in a row that look dark or lack the info bar before a live
/// stream stops counting as live: a quarter second at 15 looks a second.
/// One such picture is often a dark stretch of ocean or the bar fading.
pub const DOUBTS_TO_LEAVE: u32 = 4;

/// Debounce: `need` live pictures in a row to go live. The slate and a
/// missing badge end it at once (they are unmistakable); a dark picture or
/// a missing info bar only after `DOUBTS_TO_LEAVE` in a row.
pub struct Detector {
    live: bool,
    streak: u32,
    need: u32,
    doubts: u32,
}

impl Detector {
    pub fn new(need: u32) -> Detector {
        Detector {
            live: false,
            streak: 0,
            need: need.max(1),
            doubts: 0,
        }
    }
    pub fn live(&self) -> bool {
        self.live
    }
    /// Take a verdict in; returns the new state when it changed.
    pub fn update(&mut self, v: Verdict) -> Option<bool> {
        let was = self.live;
        match v {
            Verdict::Live => {
                self.doubts = 0;
                self.streak = self.streak.saturating_add(1);
                self.live = self.live || self.streak >= self.need;
            }
            Verdict::Dark | Verdict::NoHud if self.live => {
                self.doubts += 1;
                if self.doubts >= DOUBTS_TO_LEAVE {
                    self.leave();
                }
            }
            _ => self.leave(),
        }
        (self.live != was).then_some(self.live)
    }

    fn leave(&mut self) {
        self.live = false;
        self.streak = 0;
        self.doubts = 0;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::decode::H264;
    use super::super::ts::Demuxer;
    use super::*;

    pub(crate) const LIVE: &[u8] = include_bytes!("../../tests/fixtures/sen_live.ts");
    pub(crate) const SLATE_TS: &[u8] = include_bytes!("../../tests/fixtures/sen_slate.ts");

    /// An owned copy of a decoded picture.
    #[derive(Clone)]
    pub(crate) struct Owned {
        pub y: Vec<u8>,
        pub u: Vec<u8>,
        pub v: Vec<u8>,
        pub width: usize,
        pub height: usize,
        pub y_stride: usize,
        pub c_stride: usize,
    }

    impl Owned {
        pub fn planes(&self) -> Planes<'_> {
            Planes {
                y: &self.y,
                u: &self.u,
                v: &self.v,
                width: self.width,
                height: self.height,
                y_stride: self.y_stride,
                c_stride: self.c_stride,
            }
        }
        /// Fill a reference-space rectangle with one colour.
        pub fn fill(&mut self, xs: (usize, usize), ys: (usize, usize), yuv: [u8; 3]) {
            for y in ys.0..ys.1 {
                for x in xs.0..xs.1 {
                    self.y[y * self.y_stride + x] = yuv[0];
                    self.u[(y / 2) * self.c_stride + x / 2] = yuv[1];
                    self.v[(y / 2) * self.c_stride + x / 2] = yuv[2];
                }
            }
        }
    }

    /// Every `every`-th picture of a segment.
    pub(crate) fn pictures(data: &[u8], every: usize) -> Vec<Owned> {
        let mut dec = H264::new().unwrap();
        let mut demux = Demuxer::new();
        demux.begin();
        let (mut out, mut n) = (Vec::new(), 0);
        while let Some(au) = demux.next(data) {
            dec.decode(&au, |_, p| {
                if n % every == 0 {
                    out.push(Owned {
                        y: p.y.to_vec(),
                        u: p.u.to_vec(),
                        v: p.v.to_vec(),
                        width: p.width,
                        height: p.height,
                        y_stride: p.y_stride,
                        c_stride: p.c_stride,
                    });
                }
                n += 1;
            });
        }
        out
    }

    #[test]
    fn the_live_fixture_is_live() {
        let pics = pictures(LIVE, 15);
        assert_eq!(pics.len(), 10);
        for p in &pics {
            assert_eq!(classify(&p.planes()), Verdict::Live);
        }
    }

    /// Dark ocean with bright clouds across the bar's edge: the picture
    /// that an edge-step check for the info bar took for a promo.
    #[test]
    fn live_footage_of_dark_ocean_is_live() {
        let ocean: &[u8] = include_bytes!("../../tests/fixtures/sen_live_ocean.ts");
        let pics = pictures(ocean, 2);
        assert!(pics.len() >= 3, "{} pictures", pics.len());
        for p in &pics {
            assert_eq!(classify(&p.planes()), Verdict::Live);
        }
    }

    #[test]
    fn the_slate_fixture_is_the_slate() {
        for p in &pictures(SLATE_TS, 15) {
            assert_eq!(classify(&p.planes()), Verdict::Slate);
        }
    }

    #[test]
    fn the_night_fixture_is_dark() {
        let night: &[u8] = include_bytes!("../../tests/fixtures/sen_night.ts");
        let pics = pictures(night, 30);
        assert_eq!(pics.len(), 5);
        for p in &pics {
            assert_eq!(classify(&p.planes()), Verdict::Dark);
        }
    }

    #[test]
    fn the_slate_signature_matches_the_fixture() {
        let pics = pictures(SLATE_TS, 60);
        let sig = signature(&pics[1].planes());
        let worst = sig
            .iter()
            .zip(&SLATE)
            .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)))
            .max()
            .unwrap();
        if worst > 3 {
            // paste this over SLATE when the slate artwork changes
            for row in sig.chunks(GRID_COLS) {
                let cells: Vec<String> = row
                    .iter()
                    .map(|c| format!("[{},{},{}]", c[0], c[1], c[2]))
                    .collect();
                println!("    {},", cells.join(","));
            }
        }
        assert!(worst <= 3, "slate signature is off by {worst}");
        // the live picture is far from it
        let live = signature(&pictures(LIVE, 60)[1].planes());
        let diff: u32 = live
            .iter()
            .zip(&SLATE)
            .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b) as u32))
            .sum();
        assert!(diff / (SLATE.len() as u32 * 3) >= 15, "live is {diff} away");
    }

    #[test]
    fn an_orange_or_missing_badge_is_not_live() {
        let live = pictures(LIVE, 60).remove(1);
        // the replay badge: orange in place of red
        let mut replay = live.clone();
        replay.fill((18, 62), (15, 32), [160, 60, 185]);
        assert_eq!(classify(&replay.planes()), Verdict::NoBadge);
        // no badge at all: the picture runs under it
        let mut bare = live.clone();
        bare.fill((0, 100), (0, 40), [120, 135, 124]);
        assert_eq!(classify(&bare.planes()), Verdict::NoBadge);
    }

    #[test]
    fn a_promo_without_the_info_bar_is_not_live() {
        let mut promo = pictures(LIVE, 60).remove(1);
        // the picture runs on where the bar was; the badge stays to isolate
        // the bar check
        promo.fill((0, 640), (296, 360), [130, 135, 124]);
        assert_eq!(classify(&promo.planes()), Verdict::NoHud);
    }

    #[test]
    fn a_black_picture_with_the_hud_is_the_night_side() {
        let mut night = pictures(LIVE, 60).remove(1);
        // black from under the badge row down to the bar, as at night
        night.fill((0, 640), (40, 304), [17, 128, 128]);
        night.fill((80, 560), (0, 40), [17, 128, 128]);
        assert_eq!(classify(&night.planes()), Verdict::Dark);
        // a little light (city lights, the limb) is still night
        night.fill((300, 340), (100, 140), [200, 128, 128]);
        assert_eq!(classify(&night.planes()), Verdict::Dark);
    }

    #[test]
    fn broken_pictures_are_not_live() {
        let live = pictures(LIVE, 60).remove(1);
        let mut short = live.planes();
        short.y = &short.y[..100];
        assert_eq!(classify(&short), Verdict::NoHud);
        // a tiny picture scales the checks instead of reading outside it
        let tiny = Owned {
            y: vec![200; 32 * 18],
            u: vec![128; 16 * 9],
            v: vec![128; 16 * 9],
            width: 32,
            height: 18,
            y_stride: 32,
            c_stride: 16,
        };
        assert_ne!(classify(&tiny.planes()), Verdict::Live);
    }

    #[test]
    fn a_dark_or_barless_picture_needs_company_to_end_live() {
        for doubt in [Verdict::Dark, Verdict::NoHud] {
            let mut d = Detector::new(2);
            d.update(Verdict::Live);
            assert_eq!(d.update(Verdict::Live), Some(true));
            // one, two, three: still live; live footage clears the count
            for _ in 0..DOUBTS_TO_LEAVE - 1 {
                assert_eq!(d.update(doubt), None);
            }
            assert_eq!(d.update(Verdict::Live), None);
            assert!(d.live());
            for _ in 0..DOUBTS_TO_LEAVE - 1 {
                assert_eq!(d.update(doubt), None);
            }
            assert_eq!(d.update(doubt), Some(false), "four in a row");
            // and going live again takes the full run of live pictures
            assert_eq!(d.update(Verdict::Live), None);
            assert_eq!(d.update(Verdict::Live), Some(true));
            // the slate and a missing badge end it at once, also mid-doubt
            assert_eq!(d.update(doubt), None);
            assert_eq!(d.update(Verdict::Slate), Some(false));
            d.update(Verdict::Live);
            d.update(Verdict::Live);
            assert_eq!(d.update(Verdict::NoBadge), Some(false));
        }
    }

    #[test]
    fn going_live_takes_two_checks_and_leaving_the_slate_one() {
        let mut d = Detector::new(2);
        assert!(!d.live());
        assert_eq!(d.update(Verdict::Live), None);
        assert_eq!(d.update(Verdict::Live), Some(true));
        assert_eq!(d.update(Verdict::Live), None);
        assert_eq!(d.update(Verdict::Slate), Some(false));
        assert_eq!(d.update(Verdict::Live), None);
        assert_eq!(d.update(Verdict::Dark), None);
        assert_eq!(d.update(Verdict::Live), None);
        assert_eq!(d.update(Verdict::Live), Some(true));
        assert!(d.live());
    }
}

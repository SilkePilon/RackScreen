//! Live video frames for the `sen` role: the panel-sized frame the scene
//! carries, and the fixed crop-and-scale map that fills it straight from a
//! decoder's I420 planes.

use std::sync::Arc;

/// Frames are square, the panel's size.
pub const VIDEO_SIDE: usize = 240;
/// Bytes in one frame: opaque RGBA, `VIDEO_SIDE`².
pub const VIDEO_FRAME_BYTES: usize = VIDEO_SIDE * VIDEO_SIDE * 4;

/// The crop in a 640×360 picture: the 280×280 square at (180, 15), above the
/// stream's info bar and clear of its badge and logo. Other picture sizes
/// use the same share of the picture.
const REF_W: f64 = 640.0;
const REF_H: f64 = 360.0;
const CROP_X: f64 = 180.0;
const CROP_Y: f64 = 15.0;
const CROP_SIDE: f64 = 280.0;
/// Pixels this far from the centre are on the round panel (a pixel past its
/// edge, like the GIF frames).
const RADIUS: f64 = VIDEO_SIDE as f64 / 2.0 + 1.0;

/// One opaque RGBA frame of the panel's size. Pixels outside the panel's
/// circle are black and never written.
pub struct VideoFrame {
    rgba: Box<[u8]>,
}

impl Default for VideoFrame {
    fn default() -> Self {
        VideoFrame::new()
    }
}

impl VideoFrame {
    /// A black frame.
    pub fn new() -> VideoFrame {
        let mut rgba = vec![0u8; VIDEO_FRAME_BYTES].into_boxed_slice();
        for p in rgba.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
        VideoFrame { rgba }
    }
    /// Opaque RGBA, which is also premultiplied RGBA.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn rgba_mut(&mut self) -> &mut [u8] {
        &mut self.rgba
    }
}

/// A shared video frame, compared by identity: a new picture is a new
/// `Arc` (or a recycled one nobody else holds), so equal scenes stay equal
/// and an unchanged frame is skipped by the render workers.
#[derive(Clone)]
pub struct VideoRef(pub Arc<VideoFrame>);

impl PartialEq for VideoRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for VideoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VideoRef({:p})", Arc::as_ptr(&self.0))
    }
}

/// A decoded I420 picture: full-size luma, half-size chroma.
#[derive(Clone, Copy)]
pub struct Planes<'a> {
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub y_stride: usize,
    pub c_stride: usize,
}

impl Planes<'_> {
    /// The planes hold what the sizes promise, so indexing by them is safe.
    pub fn valid(&self) -> bool {
        let (cw, ch) = (self.width.div_ceil(2), self.height.div_ceil(2));
        let need = |stride: usize, w: usize, h: usize| (h - 1) * stride + w;
        self.width >= 16
            && self.height >= 16
            && self.y_stride >= self.width
            && self.c_stride >= cw
            && self.y.len() >= need(self.y_stride, self.width, self.height)
            && self.u.len() >= need(self.c_stride, cw, ch)
            && self.v.len() >= need(self.c_stride, cw, ch)
    }
}

/// Where one output column or row samples the picture: the first of two
/// luma taps, the weight of the second (0..=64), and the chroma sample.
#[derive(Clone, Copy)]
struct Tap {
    i0: u16,
    f: u8,
    c: u16,
}

fn taps(origin: f64, side: f64, limit: usize) -> Vec<Tap> {
    let scale = side / VIDEO_SIDE as f64;
    let last = (limit - 1) as f64;
    (0..VIDEO_SIDE)
        .map(|i| {
            // the output pixel's centre in source pixel-centre coordinates
            let centre = origin + (i as f64 + 0.5) * scale;
            let s = (centre - 0.5).clamp(0.0, last);
            let i0 = (s.floor() as usize).min(limit.saturating_sub(2));
            let f = ((s - i0 as f64) * 64.0).round().clamp(0.0, 64.0) as u8;
            let c = ((centre / 2.0).floor().max(0.0) as usize).min(limit.div_ceil(2) - 1);
            Tap {
                i0: i0 as u16,
                f,
                c: c as u16,
            }
        })
        .collect()
}

/// The fixed map from a `width`×`height` picture to the panel: bilinear luma
/// and nearest chroma taps per column and row, and each row's span inside
/// the panel's circle.
pub struct CropMap {
    width: usize,
    height: usize,
    cols: Vec<Tap>,
    rows: Vec<Tap>,
    /// First and one-past-last column inside the circle, per row.
    spans: Vec<(u8, u8)>,
}

impl CropMap {
    /// `None` for a picture too small to crop.
    pub fn new(width: usize, height: usize) -> Option<CropMap> {
        if width < 16 || height < 16 || width > 4096 || height > 4096 {
            return None;
        }
        let (kx, ky) = (width as f64 / REF_W, height as f64 / REF_H);
        let c = VIDEO_SIDE as f64 / 2.0;
        let spans = (0..VIDEO_SIDE)
            .map(|y| {
                let dy = y as f64 + 0.5 - c;
                let half = (RADIUS * RADIUS - dy * dy).max(0.0).sqrt();
                let x0 = (c - half).floor().clamp(0.0, VIDEO_SIDE as f64);
                let x1 = (c + half).ceil().clamp(0.0, VIDEO_SIDE as f64);
                (x0 as u8, x1 as u8)
            })
            .collect();
        Some(CropMap {
            width,
            height,
            // the crop stays square in picture pixels: both scale with the height
            cols: taps(CROP_X * kx, CROP_SIDE * ky, width),
            rows: taps(CROP_Y * ky, CROP_SIDE * ky, height),
            spans,
        })
    }

    /// The map was built for pictures of this size.
    pub fn fits(&self, p: &Planes) -> bool {
        self.width == p.width && self.height == p.height
    }

    /// Fill `out` from `p` (BT.709 video range). False, with `out` untouched,
    /// when the picture is not the size the map was built for.
    pub fn convert(&self, p: &Planes, out: &mut VideoFrame) -> bool {
        if !self.fits(p) || !p.valid() {
            return false;
        }
        let dst = out.rgba_mut();
        for (row, (tap, &(x0, x1))) in self.rows.iter().zip(&self.spans).enumerate() {
            let (x0, x1) = (x0 as usize, x1 as usize);
            let ya = &p.y[tap.i0 as usize * p.y_stride..][..p.width];
            let yb = &p.y[(tap.i0 as usize + 1) * p.y_stride..][..p.width];
            let u = &p.u[tap.c as usize * p.c_stride..];
            let v = &p.v[tap.c as usize * p.c_stride..];
            let (fy, gy) = (tap.f as u32, 64 - tap.f as u32);
            let line = &mut dst[row * VIDEO_SIDE * 4..][x0 * 4..x1 * 4];
            for (px, t) in line
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(&self.cols[x0..x1])
            {
                let i = t.i0 as usize;
                let (fx, gx) = (t.f as u32, 64 - t.f as u32);
                let top = ya[i] as u32 * gx + ya[i + 1] as u32 * fx;
                let bottom = yb[i] as u32 * gx + yb[i + 1] as u32 * fx;
                // luma in 1/4096ths, then BT.709 video range in 1/256ths
                let luma = ((top * gy + bottom * fy + 2048) >> 12) as i32;
                let c = 298 * (luma - 16) + 128;
                let d = u[t.c as usize] as i32 - 128;
                let e = v[t.c as usize] as i32 - 128;
                px[0] = ((c + 459 * e) >> 8).clamp(0, 255) as u8;
                px[1] = ((c - 55 * d - 136 * e) >> 8).clamp(0, 255) as u8;
                px[2] = ((c + 541 * d) >> 8).clamp(0, 255) as u8;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Picture {
        y: Vec<u8>,
        u: Vec<u8>,
        v: Vec<u8>,
        w: usize,
        h: usize,
        stride: usize,
    }

    impl Picture {
        /// A picture with padded strides, luma from `luma(x, y)`, grey chroma.
        fn new(w: usize, h: usize, luma: impl Fn(usize, usize) -> u8) -> Picture {
            let stride = w + 32;
            let mut y = vec![0u8; stride * h];
            for yy in 0..h {
                for xx in 0..w {
                    y[yy * stride + xx] = luma(xx, yy);
                }
            }
            let c = vec![128u8; (stride / 2) * (h / 2)];
            Picture {
                y,
                u: c.clone(),
                v: c,
                w,
                h,
                stride,
            }
        }
        fn planes(&self) -> Planes<'_> {
            Planes {
                y: &self.y,
                u: &self.u,
                v: &self.v,
                width: self.w,
                height: self.h,
                y_stride: self.stride,
                c_stride: self.stride / 2,
            }
        }
    }

    fn px(f: &VideoFrame, x: usize, y: usize) -> [u8; 4] {
        let i = (y * VIDEO_SIDE + x) * 4;
        f.rgba()[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_new_frame_is_opaque_black() {
        let f = VideoFrame::new();
        assert_eq!(f.rgba().len(), VIDEO_FRAME_BYTES);
        assert!(f
            .rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255]));
    }

    #[test]
    fn video_refs_compare_by_identity() {
        let a = VideoRef(Arc::new(VideoFrame::new()));
        let b = VideoRef(Arc::new(VideoFrame::new()));
        assert_eq!(a, a.clone());
        assert_ne!(a, b);
    }

    #[test]
    fn the_crop_is_the_square_above_the_hud() {
        // white inside the 280² square at (180, 15), black outside it
        let pic = Picture::new(640, 360, |x, y| {
            if (180..460).contains(&x) && (15..295).contains(&y) {
                235
            } else {
                16
            }
        });
        let map = CropMap::new(640, 360).unwrap();
        let mut f = VideoFrame::new();
        assert!(map.convert(&pic.planes(), &mut f));
        for (x, y) in [(120, 0), (120, 239), (0, 120), (239, 120), (120, 120)] {
            assert_eq!(px(&f, x, y), [255, 255, 255, 255], "({x}, {y})");
        }
        // the corners are off the round panel and stay black
        for (x, y) in [(0, 0), (239, 0), (0, 239), (239, 239), (20, 20)] {
            assert_eq!(px(&f, x, y), [0, 0, 0, 255], "({x}, {y})");
        }
    }

    #[test]
    fn luma_is_interpolated_and_colours_follow_bt709() {
        // a horizontal ramp: neighbouring output pixels differ by 7/6 steps
        let pic = Picture::new(640, 360, |x, _| (x / 2) as u8);
        let map = CropMap::new(640, 360).unwrap();
        let mut f = VideoFrame::new();
        assert!(map.convert(&pic.planes(), &mut f));
        let row: Vec<u8> = (10..230).map(|x| px(&f, x, 120)[0]).collect();
        assert!(row.windows(2).all(|w| w[1] >= w[0] && w[1] - w[0] <= 2));
        assert!(row[219] > row[0] + 100);

        let mut red = Picture::new(640, 360, |_, _| 63);
        red.u.fill(102);
        red.v.fill(240);
        assert!(map.convert(&red.planes(), &mut f));
        let [r, g, b, a] = px(&f, 120, 120);
        assert!(r > 250 && g < 5 && b < 5 && a == 255, "{r} {g} {b}");
    }

    #[test]
    fn other_picture_sizes_use_the_same_share() {
        let pic = Picture::new(426, 240, |x, y| {
            if (120..306).contains(&x) && (10..196).contains(&y) {
                235
            } else {
                16
            }
        });
        let map = CropMap::new(426, 240).unwrap();
        let mut f = VideoFrame::new();
        assert!(map.convert(&pic.planes(), &mut f));
        for (x, y) in [(120, 2), (120, 237), (2, 120), (237, 120)] {
            assert!(px(&f, x, y)[0] > 200, "({x}, {y})");
        }
    }

    #[test]
    fn wrong_or_short_pictures_are_refused() {
        let map = CropMap::new(640, 360).unwrap();
        let mut f = VideoFrame::new();
        let small = Picture::new(426, 240, |_, _| 200);
        assert!(!map.convert(&small.planes(), &mut f));
        let pic = Picture::new(640, 360, |_, _| 200);
        let mut short = pic.planes();
        short.y = &short.y[..1000];
        assert!(!map.convert(&short, &mut f));
        let mut narrow = pic.planes();
        narrow.y_stride = 100;
        assert!(!map.convert(&narrow, &mut f));
        assert!(f
            .rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255]));
        assert!(CropMap::new(0, 0).is_none());
        assert!(CropMap::new(8, 8).is_none());
        // odd and tiny sizes still map inside the picture
        for (w, h) in [(17, 17), (16, 240), (641, 361), (4096, 16)] {
            let pic = Picture::new(w, h, |_, _| 99);
            let map = CropMap::new(w, h).unwrap();
            let mut planes = pic.planes();
            planes.c_stride = pic.stride.div_ceil(2);
            let c = vec![128u8; planes.c_stride * h.div_ceil(2)];
            planes.u = &c;
            planes.v = &c;
            assert!(map.convert(&planes, &mut f), "{w}x{h}");
        }
    }
}

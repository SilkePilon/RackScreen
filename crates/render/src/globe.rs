//! The Earth as an orthographic globe, drawn per pixel from two small
//! embedded maps: land classes (five flat colours) and city lights.
//!
//! One raster costs a few milliseconds on the Pi, so the work per pixel is
//! kept small: a pixel whose whole footprint lies in one land class (most of
//! them: open ocean, the inside of a continent) takes a single sample and no
//! filtering; only pixels near a coast or the limb take four bilinear
//! samples. Which texels are "all one class around here" is worked out once,
//! when the maps are decoded. The caller caches the raster (the scene keeps
//! the globe in its base), so this runs once per change of the parameters.

use std::f32::consts::PI;
use std::sync::OnceLock;

use tiny_skia::Pixmap;

const CLASSES_RLE: &[u8] = include_bytes!("../../../assets/earth/classes.rle");
/// City lights, 0..255, row-major from the north-west corner; read in place.
const LIGHTS: &[u8] = include_bytes!("../../../assets/earth/lights.bin");

pub const MAP_W: usize = 1024;
pub const MAP_H: usize = 512;
const LIGHTS_W: usize = 512;
const LIGHTS_H: usize = 256;

/// Ocean, forest, grass, desert, ice.
pub const PALETTE: [[f32; 3]; 5] = [
    [22.0, 58.0, 92.0],
    [44.0, 92.0, 62.0],
    [96.0, 130.0, 72.0],
    [196.0, 166.0, 108.0],
    [232.0, 238.0, 242.0],
];
const AMBER: [f32; 3] = [242.0, 184.0, 75.0];

/// Brightness of the night side and the extra the twilight and day steps add.
const NIGHT: f32 = 0.30;
const TWILIGHT_STEP: f32 = 0.32;
const DAY_STEP: f32 = 0.38;
/// The twilight band: the sun's dot product with the ground normal.
const BAND: f32 = 0.06;
/// Half-width of the blend between two steps, about 1.5 px at the centre.
const BLEND: f32 = 0.016;
/// Blue is boosted by this much at full night.
const BLUE_BOOST: f32 = 0.25;
/// City lights fade in between these sun dot products...
const LIGHTS_FROM: f32 = -0.04;
const LIGHTS_FULL: f32 = -0.12;
/// ...where the lights map is brighter than this (fully lit at the second).
/// The mockup took 0.3 and 0.5 of a map four times as fine; this map is the
/// mean of its 4 × 4 blocks, which flattens the peaks, and these two give
/// the same amount of light from it.
const LIGHTS_LO: f32 = 0.16 * 255.0;
const LIGHTS_HI: f32 = 0.60 * 255.0;
/// Brightness at the very edge of the disc.
const LIMB: f32 = 0.88;

const CLASS_MASK: u8 = 0x07;
/// The texel and its neighbourhood are one class: a pixel that looks
/// straight down on it (`NEAR`: normal z from `NEAR_Z` up) or at a slant
/// (`FAR`: from `FAR_Z` up) needs one sample only.
const NEAR: u8 = 0x40;
const FAR: u8 = 0x80;
const NEAR_Z: f32 = 0.5;
const FAR_Z: f32 = 0.2;

/// The class map with its one-class flags, decoded on first use.
fn classes() -> &'static [u8] {
    static MAP: OnceLock<Box<[u8]>> = OnceLock::new();
    MAP.get_or_init(|| flag_uniform(decode_rle(CLASSES_RLE), MAP_W, MAP_H, 95.0))
}

/// `(run, class)` byte pairs, row-major, into one byte per texel.
fn decode_rle(rle: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MAP_W * MAP_H);
    for pair in rle.as_chunks::<2>().0 {
        let n = out.len() + pair[0] as usize;
        out.resize(n, pair[1].min(PALETTE.len() as u8 - 1));
    }
    out.resize(MAP_W * MAP_H, 0);
    out
}

/// Set `NEAR` and `FAR` on every texel whose surroundings, as far as the
/// four samples of a pixel can reach, are all its own class. A pixel is
/// `1 / r` radians wide at the centre of a globe of radius `r` px and
/// `1 / z` times that toward the limb; a step on the sphere is at most as
/// large in latitude and `1 / cos(lat)` times larger in longitude.
fn flag_uniform(mut cls: Vec<u8>, w: usize, h: usize, r: f32) -> Box<[u8]> {
    // per texel: how many texels to either side in its row share its class
    let mut reach = vec![0u8; w * h];
    for (row, out) in cls.chunks_exact(w).zip(reach.chunks_exact_mut(w)) {
        if row.iter().all(|c| *c == row[0]) {
            out.fill(255);
            continue;
        }
        // twice round, so runs wrap across the date line
        let mut n = 0u8;
        for i in 0..2 * w {
            let x = i % w;
            n = if row[x] == row[(x + w - 1) % w] {
                n.saturating_add(1)
            } else {
                0
            };
            out[x] = n;
        }
        for i in (0..2 * w).rev() {
            let x = i % w;
            n = if row[x] == row[(x + 1) % w] {
                n.saturating_add(1)
            } else {
                0
            };
            out[x] = out[x].min(n);
        }
    }
    // a sub-sample is at most this far from its pixel's centre, in texels
    let texel = 2.0 * PI / w as f32;
    let sample = 0.25 * std::f32::consts::SQRT_2 / r / texel;
    let mut run = vec![0u8; w];
    let mut above = vec![0u8; w * h];
    for (bit, z) in [(NEAR, NEAR_Z), (FAR, FAR_Z)] {
        // nearest texel (half a texel off) plus the bilinear neighbour
        let rows = (1.5 + sample / z).ceil() as usize;
        let cols = |y: usize| -> usize {
            // the row nearest a pole is the widest; two windows away, so
            // every row of a texel's window is held to that texel's width
            let edge = if y < h / 2 {
                y.saturating_sub(2 * rows)
            } else {
                (y + 2 * rows + 1).min(h)
            };
            let lat = (0.5 - edge as f32 / h as f32) * PI;
            (1.5 + sample / z / lat.cos().max(1e-4)).ceil() as usize
        };
        // per column, how many rows in a row are one class and wide enough:
        // counted from above, then from below
        let count = |run: &mut [u8], cls: &[u8], y: usize, from: Option<usize>| {
            let need = cols(y).min(256);
            for (x, run) in run.iter_mut().enumerate() {
                let i = y * w + x;
                let same = from.is_some_and(|f| cls[i] & CLASS_MASK == cls[f * w + x] & CLASS_MASK);
                *run = match (reach[i] as usize >= need, same) {
                    (false, _) => 0,
                    (true, true) => run.saturating_add(1),
                    (true, false) => 1,
                };
            }
        };
        run.fill(0);
        for y in 0..h {
            count(&mut run, &cls, y, y.checked_sub(1));
            above[y * w..(y + 1) * w].copy_from_slice(&run);
        }
        run.fill(0);
        for y in (0..h).rev() {
            count(&mut run, &cls, y, (y + 1 < h).then_some(y + 1));
            for (x, below) in run.iter().enumerate() {
                let i = y * w + x;
                if above[i] as usize > rows && *below as usize > rows {
                    cls[i] |= bit;
                }
            }
        }
    }
    cls.into_boxed_slice()
}

/// `sqrt(1 - q)` for pixel centres of a disc of radius `r`: one quadrant.
struct Normals {
    r: f32,
    n: usize,
    z: Vec<f32>,
}

impl Normals {
    fn new(r: f32) -> Normals {
        let n = r.ceil() as usize + 1;
        let z = (0..n * n)
            .map(|i| {
                let (x, y) = ((i % n) as f32 + 0.5, (i / n) as f32 + 0.5);
                (1.0 - (x * x + y * y) / (r * r)).max(0.0).sqrt()
            })
            .collect();
        Normals { r, n, z }
    }
}

/// The table for the first radius asked for (there is one globe size).
fn normals(r: f32) -> Option<&'static Normals> {
    static TABLE: OnceLock<Normals> = OnceLock::new();
    let t = TABLE.get_or_init(|| Normals::new(r));
    (t.r == r).then_some(t)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `atan2(y, x)`, within 3e-4 rad: a twentieth of a texel.
#[inline]
fn fast_atan2(y: f32, x: f32) -> f32 {
    // no data-dependent branches: they would mispredict all along a row
    let (ax, ay) = (x.abs(), y.abs());
    let a = ax.min(ay) / ax.max(ay).max(1e-20);
    let s = a * a;
    let r = ((-0.046_496_475 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    let steep = (ay > ax) as u32 as f32;
    let r = r + steep * (PI / 2.0 - 2.0 * r);
    let back = (x < 0.0) as u32 as f32;
    let r = r + back * (PI - 2.0 * r);
    r.copysign(y)
}

/// `asin(z)` for `|z| <= 1`, within 7e-5 rad (Abramowitz and Stegun 4.4.45).
#[inline]
fn fast_asin(z: f32) -> f32 {
    let a = z.abs().min(1.0);
    let p = ((-0.018_729_3 * a + 0.074_261) * a - 0.212_114_4) * a + 1.570_728_8;
    (PI / 2.0 - (1.0 - a).sqrt() * p).copysign(z)
}

/// A view of the globe: the camera's axes and the sun, Earth-fixed.
struct View {
    east: [f32; 3],
    north: [f32; 3],
    front: [f32; 3],
    sun: [f32; 3],
}

impl View {
    fn new(cam_lat: f32, cam_lon: f32, sun_lat: f32, sun_lon: f32) -> View {
        let (sp, cp) = cam_lat.to_radians().sin_cos();
        let (sl, cl) = cam_lon.to_radians().sin_cos();
        let (ss, cs) = sun_lat.to_radians().sin_cos();
        let (sm, cm) = sun_lon.to_radians().sin_cos();
        View {
            east: [-sl, cl, 0.0],
            north: [-sp * cl, -sp * sl, cp],
            front: [cp * cl, cp * sl, sp],
            sun: [cs * cm, cs * sm, ss],
        }
    }

    /// The ground normal under the disc point `(x, y, z)` (y up).
    #[inline]
    fn world(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        [
            x * self.east[0] + y * self.north[0] + z * self.front[0],
            x * self.east[1] + y * self.north[1] + z * self.front[1],
            x * self.east[2] + y * self.north[2] + z * self.front[2],
        ]
    }

    /// Map coordinates in texels of a `w` × `h` map.
    #[inline]
    fn uv(n: [f32; 3], w: f32, h: f32) -> (f32, f32) {
        let lon = fast_atan2(n[1], n[0]);
        let lat = fast_asin(n[2]);
        ((lon / PI + 1.0) * 0.5 * w, (0.5 - lat / PI) * h)
    }

    /// Day, twilight or night on `base`, the city lights, the limb. Most
    /// of the globe is plain day or plain night: those skip the blends.
    #[inline]
    fn shade(&self, base: [f32; 3], uv: (f32, f32), k: f32, z: f32) -> [f32; 3] {
        let e = limb(z);
        if k >= BAND + BLEND {
            return [base[0] * e, base[1] * e, base[2] * e];
        }
        let mut c = if k <= LIGHTS_FULL {
            [
                base[0] * NIGHT,
                base[1] * NIGHT,
                base[2] * (NIGHT * (1.0 + BLUE_BOOST)),
            ]
        } else {
            let band = NIGHT
                + TWILIGHT_STEP * smooth(-BAND - BLEND, -BAND + BLEND, k)
                + DAY_STEP * smooth(BAND - BLEND, BAND + BLEND, k);
            let blue = 1.0 + BLUE_BOOST * (1.0 - band) / (1.0 - NIGHT);
            [base[0] * band, base[1] * band, base[2] * band * blue]
        };
        if k < LIGHTS_FROM {
            lights(&mut c, uv, k);
        }
        [c[0] * e, c[1] * e, c[2] * e]
    }
}

/// Blend the city lights at `uv` (in class-map texels) into `c`.
#[inline]
fn lights(c: &mut [f32; 3], uv: (f32, f32), k: f32) {
    let scale = LIGHTS_W as f32 / MAP_W as f32;
    let l = bilinear_lights(uv.0 * scale, uv.1 * scale);
    if l > LIGHTS_LO {
        let a = (1.0 - smooth(LIGHTS_FULL, LIGHTS_FROM, k)) * smooth(LIGHTS_LO, LIGHTS_HI, l);
        for (c, amber) in c.iter_mut().zip(AMBER) {
            *c += (amber - *c) * a;
        }
    }
}

#[inline]
fn limb(z: f32) -> f32 {
    LIMB + (1.0 - LIMB) * (z * 3.0).min(1.0)
}

/// The four texels around `(u, v)` and their weights: wraps in longitude,
/// clamps at the poles.
#[inline]
fn corners(u: f32, v: f32, w: usize, h: usize) -> ([usize; 4], [f32; 4]) {
    let (u, v) = (u - 0.5, v - 0.5);
    let (fu, fv) = (u.floor(), v.floor());
    let (tx, ty) = (u - fu, v - fv);
    // `u` is within a texel of 0..w: one conditional step wraps it
    let xi = fu as i32;
    let x0 = (if xi < 0 {
        xi + w as i32
    } else if xi >= w as i32 {
        xi - w as i32
    } else {
        xi
    }) as usize;
    let x1 = if x0 + 1 == w { 0 } else { x0 + 1 };
    let yi = fv as i32;
    let y0 = yi.clamp(0, h as i32 - 1) as usize;
    let y1 = (yi + 1).clamp(0, h as i32 - 1) as usize;
    (
        [y0 * w + x0, y0 * w + x1, y1 * w + x0, y1 * w + x1],
        [
            (1.0 - tx) * (1.0 - ty),
            tx * (1.0 - ty),
            (1.0 - tx) * ty,
            tx * ty,
        ],
    )
}

#[inline]
fn bilinear_lights(u: f32, v: f32) -> f32 {
    let (i, k) = corners(u, v, LIGHTS_W, LIGHTS_H);
    LIGHTS[i[0]] as f32 * k[0]
        + LIGHTS[i[1]] as f32 * k[1]
        + LIGHTS[i[2]] as f32 * k[2]
        + LIGHTS[i[3]] as f32 * k[3]
}

/// The palette colours of the four texels around `(u, v)`, blended.
#[inline]
fn bilinear_colour(map: &[u8], u: f32, v: f32) -> [f32; 3] {
    let (i, k) = corners(u, v, MAP_W, MAP_H);
    let c = i.map(|i| (map[i] & CLASS_MASK) as usize);
    if c[0] == c[1] && c[0] == c[2] && c[0] == c[3] {
        return PALETTE[c[0]];
    }
    let mut out = [0.0; 3];
    for (c, k) in c.iter().zip(k) {
        let p = PALETTE[*c];
        out[0] += p[0] * k;
        out[1] += p[1] * k;
        out[2] += p[2] * k;
    }
    out
}

/// Sub-sample offsets in pixels: a regular 2 × 2 grid.
const SAMPLES: [(f32, f32); 4] = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];

/// Draw the globe of radius `r` at the centre of `px`, over what is there
/// (its rim is anti-aliased). Angles in degrees.
pub fn draw_globe(px: &mut Pixmap, cam_lat: f32, cam_lon: f32, sun_lat: f32, sun_lon: f32, r: f32) {
    if r < 1.0 || !(cam_lat + cam_lon + sun_lat + sun_lon).is_finite() {
        return;
    }
    let map = classes();
    let view = View::new(cam_lat, cam_lon, sun_lat, sun_lon);
    let table = normals(r);
    let (w, h) = (px.width() as usize, px.height() as usize);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let inv_r = 1.0 / r;
    let outer = r + 0.5;
    let data = px.data_mut();
    let y0 = (cy - outer).floor().max(0.0) as usize;
    let y1 = ((cy + outer).ceil() as usize).min(h);
    for y in y0..y1 {
        let dy = y as f32 + 0.5 - cy;
        let half = (outer * outer - dy * dy).max(0.0).sqrt();
        let x0 = (cx - half).floor().max(0.0) as usize;
        let x1 = ((cx + half).ceil() as usize).min(w);
        let iy = dy.abs() as usize;
        let py = -dy * inv_r;
        let row = &mut data[(y * w + x0) * 4..(y * w + x1) * 4];
        for (i, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let dx = (x0 + i) as f32 + 0.5 - cx;
            let px_ = dx * inv_r;
            let q = px_ * px_ + py * py;
            // one sample where the pixel sees a single land class
            if q <= 1.0 - FAR_Z * FAR_Z {
                let z = match table {
                    Some(t) => t.z[iy * t.n + dx.abs() as usize],
                    None => (1.0 - q).sqrt(),
                };
                let n = view.world(px_, py, z);
                let (u, v) = View::uv(n, MAP_W as f32, MAP_H as f32);
                let texel = map[(v as usize).min(MAP_H - 1) * MAP_W + (u as usize).min(MAP_W - 1)];
                let need = if z >= NEAR_Z { NEAR | FAR } else { FAR };
                if texel & need != 0 {
                    let class = (texel & CLASS_MASK) as usize;
                    let k = n[0] * view.sun[0] + n[1] * view.sun[1] + n[2] * view.sun[2];
                    let c = view.shade(PALETTE[class], (u, v), k, z);
                    *out = [
                        (c[0] + 0.5) as u8,
                        (c[1] + 0.5) as u8,
                        (c[2] + 0.5) as u8,
                        255,
                    ];
                    continue;
                }
            }
            // a coast, or the limb: four samples, each bilinear
            let mut c = [0.0f32; 3];
            for (ox, oy) in SAMPLES {
                let (mut sx, mut sy) = ((dx + ox) * inv_r, -(dy + oy) * inv_r);
                let mut q = sx * sx + sy * sy;
                if q > 0.998 {
                    // past the edge: the colour of the edge
                    let s = (0.998 / q).sqrt();
                    sx *= s;
                    sy *= s;
                    q = 0.998;
                }
                let z = (1.0 - q).sqrt();
                let n = view.world(sx, sy, z);
                let (u, v) = View::uv(n, MAP_W as f32, MAP_H as f32);
                let k = n[0] * view.sun[0] + n[1] * view.sun[1] + n[2] * view.sun[2];
                let s = view.shade(bilinear_colour(map, u, v), (u, v), k, z);
                c[0] += s[0];
                c[1] += s[1];
                c[2] += s[2];
            }
            let c = [c[0] * 0.25, c[1] * 0.25, c[2] * 0.25];
            // coverage of the pixel by the disc
            let a = (outer - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            if a >= 1.0 {
                *out = [
                    (c[0] + 0.5) as u8,
                    (c[1] + 0.5) as u8,
                    (c[2] + 0.5) as u8,
                    255,
                ];
            } else if a > 0.0 {
                // premultiplied source over
                for ch in 0..3 {
                    out[ch] = (c[ch] * a + out[ch] as f32 * (1.0 - a) + 0.5) as u8;
                }
                out[3] = (255.0 * a + out[3] as f32 * (1.0 - a) + 0.5) as u8;
            }
        }
    }
}

/// How much memory the decoded maps hold (the lights are read in place).
pub fn map_bytes() -> usize {
    classes().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::new_pixmap;

    fn pixel(p: &Pixmap, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * p.width() + x) * 4) as usize;
        [p.data()[i], p.data()[i + 1], p.data()[i + 2]]
    }

    fn globe(cam: (f32, f32), sun: (f32, f32)) -> Pixmap {
        let mut px = new_pixmap();
        px.fill(tiny_skia::Color::BLACK);
        draw_globe(&mut px, cam.0, cam.1, sun.0, sun.1, 95.0);
        px
    }

    fn class_at(lat: f32, lon: f32) -> u8 {
        let x = ((lon + 180.0) / 360.0 * MAP_W as f32) as usize % MAP_W;
        let y = (((90.0 - lat) / 180.0 * MAP_H as f32) as usize).min(MAP_H - 1);
        classes()[y * MAP_W + x] & CLASS_MASK
    }

    #[test]
    fn the_maps_decode_to_their_size_and_know_the_continents() {
        assert_eq!(classes().len(), MAP_W * MAP_H);
        assert_eq!(LIGHTS.len(), LIGHTS_W * LIGHTS_H);
        assert!(map_bytes() < 1024 * 1024, "under a megabyte");
        assert!(classes().iter().all(|c| (c & CLASS_MASK) < 5));
        assert_eq!(class_at(0.0, -30.0), 0, "the Atlantic");
        assert_eq!(class_at(-20.0, -140.0), 0, "the Pacific");
        assert_eq!(class_at(23.0, 12.0), 3, "the Sahara");
        assert_eq!(class_at(-80.0, 60.0), 4, "Antarctica");
        assert_eq!(class_at(75.0, -40.0), 4, "Greenland");
        assert!(matches!(class_at(-5.0, -62.0), 1 | 2), "the Amazon");
        // Europe by night is brighter than the Sahara
        let light = |lat: f32, lon: f32| {
            let x = ((lon + 180.0) / 360.0 * LIGHTS_W as f32) as usize;
            let y = ((90.0 - lat) / 180.0 * LIGHTS_H as f32) as usize;
            LIGHTS[y * LIGHTS_W + x]
        };
        assert!(light(51.0, 4.5) > 100 && light(23.0, 12.0) < 40);
    }

    #[test]
    fn one_class_flags_mark_open_water_and_not_the_coast() {
        let at = |lat: f32, lon: f32| {
            let x = ((lon + 180.0) / 360.0 * MAP_W as f32) as usize % MAP_W;
            let y = ((90.0 - lat) / 180.0 * MAP_H as f32) as usize;
            classes()[y * MAP_W + x]
        };
        assert_eq!(at(-20.0, -140.0) & (NEAR | FAR), NEAR | FAR, "mid Pacific");
        assert_eq!(at(23.0, 12.0) & NEAR, NEAR, "mid Sahara");
        // the Dutch coast: sea and land within a texel or two
        assert_eq!(at(52.4, 4.5) & (NEAR | FAR), 0);
        let flagged = classes().iter().filter(|c| **c & NEAR != 0).count();
        let far = classes().iter().filter(|c| **c & FAR != 0).count();
        assert!(far < flagged, "the wider test passes less often");
        assert!(
            flagged * 10 > MAP_W * MAP_H * 6,
            "most of the map is one class: {flagged}"
        );
        // every flagged texel really has one-class surroundings: check the
        // window the flag promises on a synthetic map with one coast
        let (w, h) = (64, 32);
        let mut m = vec![0u8; w * h];
        for y in 0..h {
            for x in 20..40 {
                m[y * w + x] = 2;
            }
        }
        let f = flag_uniform(m, w, h, 6.0);
        for y in 12..20 {
            assert_eq!(f[y * w + 20] & (NEAR | FAR), 0, "the coast itself");
            assert_eq!(f[y * w + 39] & (NEAR | FAR), 0);
            assert_eq!(f[y * w + 30] & NEAR, NEAR, "inland");
            assert_eq!(f[y * w + 55] & NEAR, NEAR, "at sea, across the wrap");
            assert_eq!(f[y * w + 30] & CLASS_MASK, 2);
        }
    }

    #[test]
    fn fast_trig_is_close_enough_for_a_texel() {
        let mut worst = 0.0f32;
        for i in 0..2000 {
            let a = i as f32 / 2000.0 * 2.0 * PI - PI;
            let (s, c) = a.sin_cos();
            worst = worst.max((fast_atan2(s, c) - s.atan2(c)).abs());
        }
        assert!(worst < 3e-4, "atan2 off by {worst}");
        let mut worst = 0.0f32;
        for i in 0..=2000 {
            let z = i as f32 / 1000.0 - 1.0;
            worst = worst.max((fast_asin(z) - z.asin()).abs());
        }
        assert!(worst < 1e-4, "asin off by {worst}");
        assert_eq!(fast_atan2(0.0, 0.0), 0.0);
    }

    #[test]
    fn the_globe_is_a_lit_disc_with_a_soft_rim() {
        // above the south-east Pacific at noon there; both ends of the
        // middle row are open water on the equator
        let px = globe((-40.0, -100.0), (-40.0, -100.0));
        assert_eq!(pixel(&px, 120, 120), [22, 58, 92], "day ocean, full colour");
        assert_eq!(pixel(&px, 120, 20), [0, 0, 0], "outside the disc");
        assert_eq!(pixel(&px, 4, 4), [0, 0, 0]);
        // the rim fades over a pixel; just inside it the limb is darker
        let edge = pixel(&px, 25, 120);
        let inside = pixel(&px, 27, 120);
        let outside = pixel(&px, 24, 120);
        assert!(
            inside[2] >= 70 && inside[2] < 92,
            "limb darkening: {inside:?}"
        );
        assert!(edge[2] > 0 && edge[2] <= inside[2], "{edge:?}");
        assert!(outside[2] < edge[2], "{outside:?} {edge:?}");
        // the disc is as wide as asked
        let lit = (0..240).filter(|x| pixel(&px, *x, 120)[2] > 30).count();
        assert!((189..=191).contains(&lit), "{lit}");
    }

    #[test]
    fn night_is_dim_and_blue_with_amber_cities() {
        // above the mid Pacific at midnight there
        let px = globe((-20.0, -140.0), (20.0, 40.0));
        let night = pixel(&px, 120, 120);
        assert_eq!(night, [7, 17, 35], "30 %, blue ×1.25");
        // Europe at night: a few pixels are city amber, far redder than blue
        let eu = globe((48.0, 8.0), (-10.0, -172.0));
        let amber = (60..180)
            .flat_map(|y| (60..180).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pixel(&eu, x, y);
                p[0] as i32 > p[2] as i32 + 20
            })
            .count();
        assert!((5..400).contains(&amber), "lit cities: {amber}");
        // by day the same view has none
        let day = globe((48.0, 8.0), (48.0, 8.0));
        let amber = (60..180)
            .flat_map(|y| (60..180).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pixel(&day, x, y);
                p[0] as i32 > p[2] as i32 + 100
            })
            .count();
        assert_eq!(amber, 0);
    }

    #[test]
    fn the_terminator_steps_from_day_through_twilight_to_night() {
        // the sun 90° east of the camera, both on the equator over the
        // Pacific: the terminator runs down the middle of the disc
        let px = globe((0.0, -150.0), (0.0, -60.0));
        let blue = |x: u32| pixel(&px, x, 120)[2];
        assert_eq!(pixel(&px, 150, 120), [22, 58, 92], "day");
        assert_eq!(pixel(&px, 90, 120), [7, 17, 35], "night");
        // twilight: sun dot in -0.06..0.06, i.e. within 5.7 px of x = 120
        let tw = pixel(&px, 120, 120);
        assert_eq!(tw, [14, 36, 65], "62 %, blue boosted by 0.25 × 0.38 / 0.7");
        // brightness never falls going toward the sun
        for x in 80..160 {
            assert!(blue(x + 1) >= blue(x), "x {x}");
        }
        // each step is blended over a few pixels, not a hard line
        let steps = (80..160)
            .filter(|x| ![35, 65, 92].contains(&blue(*x)))
            .count();
        assert!((2..=8).contains(&steps), "{steps} blended pixels");
    }

    #[test]
    fn the_single_sample_path_matches_four_samples_where_it_is_taken() {
        // with every one-class flag cleared the whole disc takes the slow
        // path; the fast path may differ only by rounding
        let fast = globe((30.0, 20.0), (10.0, 60.0));
        let mut slow = new_pixmap();
        slow.fill(tiny_skia::Color::BLACK);
        draw_reference(&mut slow, 30.0, 20.0, 10.0, 60.0, 95.0);
        let mut off = 0;
        for (i, (a, b)) in fast
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(slow.data().as_chunks::<4>().0)
            .enumerate()
        {
            // the rim is anti-aliased differently: compare inside it
            let (dx, dy) = ((i % 240) as f32 - 119.5, (i / 240) as f32 - 119.5);
            if dx.hypot(dy) > 92.0 {
                continue;
            }
            let d = (0..3)
                .map(|c| (a[c] as i32 - b[c] as i32).abs())
                .max()
                .unwrap();
            // a city light is one sample here and four there
            assert!(d <= 48, "{a:?} vs {b:?}");
            if d > 3 {
                off += 1;
            }
        }
        // the few that differ sit on the terminator's blend or a light
        assert!(off < 300, "{off} pixels differ visibly");
    }

    /// The mockup's renderer, unoptimised: four exact samples everywhere.
    fn draw_reference(
        px: &mut Pixmap,
        cam_lat: f32,
        cam_lon: f32,
        sun_lat: f32,
        sun_lon: f32,
        r: f32,
    ) {
        let map = classes();
        let view = View::new(cam_lat, cam_lon, sun_lat, sun_lon);
        let w = px.width() as usize;
        let c0 = w as f32 / 2.0;
        let data = px.data_mut();
        for y in 0..w {
            for x in 0..w {
                let (dx, dy) = (x as f32 + 0.5 - c0, y as f32 + 0.5 - c0);
                let mut c = [0.0f32; 3];
                let mut hits = 0;
                for (ox, oy) in SAMPLES {
                    let (sx, sy) = ((dx + ox) / r, -(dy + oy) / r);
                    let q = sx * sx + sy * sy;
                    if q >= 1.0 {
                        continue;
                    }
                    hits += 1;
                    let z = (1.0 - q).sqrt();
                    let n = view.world(sx, sy, z);
                    let lon = n[1].atan2(n[0]);
                    let lat = n[2].clamp(-1.0, 1.0).asin();
                    let (u, v) = (
                        (lon / PI + 1.0) * 0.5 * MAP_W as f32,
                        (0.5 - lat / PI) * MAP_H as f32,
                    );
                    let k = n[0] * view.sun[0] + n[1] * view.sun[1] + n[2] * view.sun[2];
                    let s = view.shade(bilinear_colour(map, u, v), (u, v), k, z);
                    for ch in 0..3 {
                        c[ch] += s[ch] / 4.0;
                    }
                }
                if hits == 4 {
                    let i = (y * w + x) * 4;
                    for ch in 0..3 {
                        data[i + ch] = (c[ch] + 0.5) as u8;
                    }
                }
            }
        }
    }
}

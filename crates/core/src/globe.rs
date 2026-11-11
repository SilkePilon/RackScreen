//! Sphere maths for the ISS globe: the station's position between ephemeris
//! points, the subsolar point, the camera that follows the station, and the
//! near-side stretch of the ground track. Unit vectors are Earth-fixed:
//! x through (0°, 0°), y through (0°, 90° E), z through the north pole.

use crate::event::IssTrack;

pub type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The unit vector of a point on the ground, degrees in.
pub fn unit(lat: f64, lon: f64) -> V3 {
    let (sp, cp) = lat.to_radians().sin_cos();
    let (sl, cl) = lon.to_radians().sin_cos();
    [cp * cl, cp * sl, sp]
}

/// `(lat, lon)` in degrees of a unit vector.
pub fn lat_lon(v: V3) -> (f64, f64) {
    (
        v[2].clamp(-1.0, 1.0).asin().to_degrees(),
        v[1].atan2(v[0]).to_degrees(),
    )
}

/// Along the great circle from `a` to `b`, both unit vectors.
pub fn slerp(a: V3, b: V3, t: f64) -> V3 {
    let d = dot(a, b).clamp(-1.0, 1.0);
    let (ka, kb) = if d > 0.999_999_9 {
        (1.0 - t, t)
    } else {
        let th = d.acos();
        let s = th.sin();
        (((1.0 - t) * th).sin() / s, (t * th).sin() / s)
    };
    let v = [
        ka * a[0] + kb * b[0],
        ka * a[1] + kb * b[1],
        ka * a[2] + kb * b[2],
    ];
    let n = dot(v, v).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

impl IssTrack {
    /// The sub-satellite point at `unix`, interpolated between the two
    /// ephemeris points around it; `None` outside the ephemeris.
    pub fn at(&self, unix: i64) -> Option<V3> {
        let step = self.step_secs.max(1) as i64;
        let dt = unix - self.t0;
        if dt < 0 {
            return None;
        }
        let i = (dt / step) as usize;
        let p = |i: usize| {
            self.points
                .get(i)
                .map(|(lat, lon)| unit(*lat as f64, *lon as f64))
        };
        let a = p(i)?;
        let rem = dt % step;
        if rem == 0 {
            return Some(a);
        }
        Some(slerp(a, p(i + 1)?, rem as f64 / step as f64))
    }
}

/// `(lat, lon)` in degrees of the point with the sun overhead. A low
/// precision almanac formula, good to a few hundredths of a degree.
pub fn subsolar(unix: i64) -> (f64, f64) {
    let n = unix as f64 / 86_400.0 + 2_440_587.5 - 2_451_545.0;
    let l = (280.460 + 0.985_647_4 * n).rem_euclid(360.0);
    let g = (357.528 + 0.985_600_3 * n).rem_euclid(360.0).to_radians();
    let lambda = (l + 1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let eps = (23.439 - 0.000_000_4 * n).to_radians();
    let dec = (eps.sin() * lambda.sin()).asin();
    let ra = (eps.cos() * lambda.sin()).atan2(lambda.cos()).to_degrees();
    let gmst = (280.460_618_37 + 360.985_647_366_29 * n).rem_euclid(360.0);
    let lon = (ra - gmst + 540.0).rem_euclid(360.0) - 180.0;
    (dec.to_degrees(), lon)
}

/// The station's height above the ground as a share of the Earth's radius
/// (about 420 km); the ephemeris carries ground points only.
const ISS_ALTITUDE: f64 = 420.0 / 6371.0;

/// Whether a station above ground point `iss` is in the Earth's shadow with
/// the sun over `sun` (both unit vectors): on the night side of the centre
/// and inside the shadow cylinder.
pub fn in_shadow(iss: V3, sun: V3) -> bool {
    let along = dot(iss, sun);
    if along >= 0.0 {
        return false;
    }
    let r = 1.0 + ISS_ALTITUDE;
    r * (1.0 - along * along).max(0.0).sqrt() < 1.0
}

impl IssTrack {
    /// Whether the station is in the Earth's shadow at `unix`; `None`
    /// outside the ephemeris.
    pub fn in_shadow(&self, unix: i64) -> Option<bool> {
        let (lat, lon) = subsolar(unix);
        Some(in_shadow(self.at(unix)?, unit(lat, lon)))
    }

    /// Whether the ground under the station is lit at `unix` (sun above
    /// `GROUND_LIT_MIN_ELEVATION_DEG` there); `None` outside the ephemeris.
    /// The cameras look down at the ground, which goes dark well before the
    /// station itself enters the shadow.
    pub fn ground_lit(&self, unix: i64) -> Option<bool> {
        let (lat, lon) = subsolar(unix);
        let min = GROUND_LIT_MIN_ELEVATION_DEG.to_radians().sin();
        Some(dot(self.at(unix)?, unit(lat, lon)) > min)
    }
}

/// Below this sun elevation at the station's ground point the view down is
/// too dark to show (late civil twilight).
pub const GROUND_LIT_MIN_ELEVATION_DEG: f64 = -4.0;

/// The globe is centred this far west of the station, and on this share of
/// its latitude, so the station sits right of centre with its way ahead in
/// view and the poles never tip far over.
pub const CAM_LON_BEHIND_DEG: f64 = 18.0;
pub const CAM_LAT_SHARE: f64 = 0.55;

/// An orthographic camera looking at the globe's centre from above
/// `(lat, lon)`, north up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub lat: f64,
    pub lon: f64,
    east: V3,
    north: V3,
    front: V3,
}

impl Camera {
    pub fn new(lat: f64, lon: f64) -> Camera {
        let (sp, cp) = lat.to_radians().sin_cos();
        let (sl, cl) = lon.to_radians().sin_cos();
        Camera {
            lat,
            lon,
            east: [-sl, cl, 0.0],
            north: [-sp * cl, -sp * sl, cp],
            front: [cp * cl, cp * sl, sp],
        }
    }

    /// The camera that follows a station above `iss`.
    pub fn following(iss: V3) -> Camera {
        let (lat, lon) = lat_lon(iss);
        Camera::new(lat * CAM_LAT_SHARE, lon - CAM_LON_BEHIND_DEG)
    }

    /// `(x, y, z)` of a ground point on the unit disc: x right, y up, z
    /// toward the viewer (negative on the far side).
    pub fn project(&self, v: V3) -> (f64, f64, f64) {
        (dot(v, self.east), dot(v, self.north), dot(v, self.front))
    }
}

/// The ground track from the station at `unix` outward in time (`dir` +1
/// ahead, −1 back) for up to `max_secs`, as unit-disc points: only the
/// stretch that stays on the near side, its last segment cut at the limb,
/// so nothing comes back into view from behind the globe.
pub fn near_side_track(
    track: &IssTrack,
    cam: &Camera,
    unix: i64,
    dir: i64,
    max_secs: i64,
    step_secs: i64,
) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut prev: Option<(f64, f64, f64)> = None;
    let mut dt = 0;
    while dt <= max_secs {
        let Some(v) = track.at(unix + dir * dt) else {
            break;
        };
        let p = cam.project(v);
        if p.2 <= 0.0 {
            if let Some(q) = prev {
                // where the segment crosses z = 0, pushed out onto the limb
                let s = q.2 / (q.2 - p.2);
                let (x, y) = (q.0 + (p.0 - q.0) * s, q.1 + (p.1 - q.1) * s);
                let n = x.hypot(y).max(1e-9);
                out.push(((x / n) as f32, (y / n) as f32));
            }
            break;
        }
        out.push((p.0 as f32, p.1 as f32));
        prev = Some(p);
        dt += step_secs;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A circular orbit inclined 51.6° over a turning Earth, one point every
    /// 20 s from `t0`: close enough to the ISS for the geometry.
    pub(crate) fn orbit(t0: i64, points: usize) -> IssTrack {
        let inc = 51.6f64.to_radians();
        let points = (0..points)
            .map(|i| {
                let t = i as f64 * 20.0;
                let u = t / 5_560.0 * std::f64::consts::TAU;
                let lat = (inc.sin() * u.sin()).asin();
                let lon =
                    (inc.cos() * u.sin()).atan2(u.cos()) - t / 86_164.0 * std::f64::consts::TAU;
                (
                    lat.to_degrees() as f32,
                    ((lon.to_degrees() + 540.0).rem_euclid(360.0) - 180.0) as f32,
                )
            })
            .collect();
        IssTrack {
            t0,
            step_secs: 20,
            points,
            home: (52.37, 4.89),
        }
    }

    fn angle(a: V3, b: V3) -> f64 {
        dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
    }

    #[test]
    fn unit_and_lat_lon_round_trip() {
        for (lat, lon) in [(0.0, 0.0), (52.37, 4.89), (-33.9, 151.2), (80.0, -170.0)] {
            let (a, b) = lat_lon(unit(lat, lon));
            assert!((a - lat).abs() < 1e-9 && (b - lon).abs() < 1e-9);
        }
        assert_eq!(unit(90.0, 0.0)[2], 1.0);
    }

    #[test]
    fn slerp_moves_at_a_constant_rate_along_the_great_circle() {
        let (a, b) = (unit(0.0, 0.0), unit(0.0, 90.0));
        let m = lat_lon(slerp(a, b, 1.0 / 3.0));
        assert!(m.0.abs() < 1e-9 && (m.1 - 30.0).abs() < 1e-9, "{m:?}");
        // across the date line the short way
        let m = lat_lon(slerp(unit(10.0, 179.0), unit(10.0, -179.0), 0.5));
        assert!(m.1.abs() > 179.9, "{m:?}");
        // identical points do not divide by zero
        let s = slerp(a, a, 0.5);
        assert!(s.iter().all(|c| c.is_finite()) && angle(s, a) < 1e-6);
    }

    #[test]
    fn the_ephemeris_is_interpolated_between_its_points() {
        let tr = orbit(1_000, 10);
        let p = |i: usize| unit(tr.points[i].0 as f64, tr.points[i].1 as f64);
        assert!(angle(tr.at(1_000).unwrap(), p(0)) < 1e-9);
        assert!(angle(tr.at(1_040).unwrap(), p(2)) < 1e-9);
        // a quarter of the way from point 2 to point 3
        let q = tr.at(1_045).unwrap();
        let whole = angle(p(2), p(3));
        assert!((angle(p(2), q) - whole / 4.0).abs() < 1e-6);
        assert!((angle(q, p(3)) - whole * 3.0 / 4.0).abs() < 1e-6);
        // the last point is the end
        assert!(tr.at(1_180).is_some());
        assert!(tr.at(1_181).is_none());
        assert!(tr.at(999).is_none());
    }

    #[test]
    fn the_shadow_starts_well_past_the_terminator() {
        let sun = unit(0.0, 0.0);
        assert!(!in_shadow(unit(0.0, 0.0), sun), "noon");
        assert!(!in_shadow(unit(0.0, 89.0), sun), "day side");
        // at 420 km the station sees the sun until about 20° past the terminator
        assert!(!in_shadow(unit(0.0, 105.0), sun));
        assert!(in_shadow(unit(0.0, 115.0), sun));
        assert!(in_shadow(unit(0.0, 180.0), sun), "midnight");
        assert!(in_shadow(unit(60.0, 180.0), sun));
        assert!(
            !in_shadow(unit(75.0, 180.0), sun),
            "over the pole's twilight"
        );
    }

    #[test]
    fn the_ground_goes_dark_before_the_station_does() {
        let t = 1_790_000_000;
        let (lat, lon) = subsolar(t);
        let at = |dlon: f64| IssTrack {
            t0: t,
            step_secs: 20,
            points: vec![(lat as f32, (lon + dlon) as f32); 2],
            home: (0.0, 0.0),
        };
        assert_eq!(at(60.0).ground_lit(t), Some(true));
        assert_eq!(at(92.0).ground_lit(t), Some(true), "sun just set");
        // 100° past the subsolar point: the station is still sunlit, the
        // ground below is in deep twilight
        assert_eq!(at(100.0).in_shadow(t), Some(false));
        assert_eq!(at(100.0).ground_lit(t), Some(false));
        assert_eq!(at(100.0).ground_lit(t + 60), None);
    }

    #[test]
    fn a_track_spends_about_a_third_of_an_orbit_in_shadow() {
        let tr = orbit(1_790_000_000, 300);
        let total = (tr.points.len() as i64 - 1) * tr.step_secs as i64;
        let dark = (0..total)
            .step_by(10)
            .filter(|dt| tr.in_shadow(tr.t0 + dt).unwrap())
            .count() as f64
            * 10.0
            / total as f64;
        assert!((0.2..0.45).contains(&dark), "{dark}");
        assert_eq!(tr.in_shadow(tr.t0 - 1), None);
        assert_eq!(tr.in_shadow(tr.t0 + total + 100), None);
    }

    #[test]
    fn the_sun_stands_where_the_almanac_says() {
        // the September equinox of 2026: 23 Sep, 00:05 UTC
        let (lat, lon) = subsolar(1_790_121_900);
        assert!(lat.abs() < 0.05, "declination {lat}");
        assert!((lon.abs() - 178.0).abs() < 2.5, "near the date line: {lon}");
        // the June solstice at noon UTC: 23.44° N, just west of Greenwich
        let (lat, lon) = subsolar(1_782_043_200);
        assert!((lat - 23.43).abs() < 0.03, "{lat}");
        assert!(lon.abs() < 1.0, "equation of time is about -2 min: {lon}");
        // one hour later the sun is 15° further west
        let (_, later) = subsolar(1_782_043_200 + 3600);
        assert!((lon - later - 15.0).abs() < 0.01);
    }

    #[test]
    fn the_camera_trails_the_station_and_projects_north_up() {
        let cam = Camera::following(unit(40.0, 10.0));
        assert!((cam.lat - 22.0).abs() < 1e-9 && (cam.lon + 8.0).abs() < 1e-9);
        let (x, y, z) = cam.project(unit(40.0, 10.0));
        assert!(x > 0.0 && y > 0.0 && z > 0.0, "right of and above centre");
        let c = cam.project(unit(22.0, -8.0));
        assert!(c.0.abs() < 1e-12 && c.1.abs() < 1e-12 && (c.2 - 1.0).abs() < 1e-12);
        let pole = cam.project(unit(90.0, 0.0));
        assert!(pole.0.abs() < 1e-12 && pole.1 > 0.9, "north is up");
        assert!(cam.project(unit(-22.0, 172.0)).2 < -0.99, "the far side");
    }

    #[test]
    fn the_track_is_cut_at_the_horizon_and_never_comes_back() {
        // 160 minutes of orbit; "now" is an hour in
        let tr = orbit(0, 481);
        let now = 3_600;
        let cam = Camera::following(tr.at(now).unwrap());
        for (dir, max) in [(1, 93 * 60), (-1, 50 * 60)] {
            let pts = near_side_track(&tr, &cam, now, dir, max, 30);
            let iss = cam.project(tr.at(now).unwrap());
            assert!(
                (pts[0].0 as f64 - iss.0).abs() < 1e-6 && (pts[0].1 as f64 - iss.1).abs() < 1e-6,
                "it starts at the station"
            );
            // the walk stopped well short of its time limit: at the limb
            assert!(
                pts.len() > 10 && pts.len() < (max / 30) as usize,
                "{}",
                pts.len()
            );
            let (lx, ly) = *pts.last().unwrap();
            assert!((lx.hypot(ly) - 1.0).abs() < 1e-5, "the cut is on the limb");
            for (x, y) in &pts[..pts.len() - 1] {
                assert!(x.hypot(*y) < 1.0, "inside the disc");
            }
            // the points it stopped before include some that are back on the
            // near side: a full orbit ahead the station is in view again
            if dir == 1 {
                let again = cam.project(tr.at(now + 92 * 60).unwrap());
                assert!(again.2 > 0.0, "the next lap is in view, and left out");
            }
        }
        // a short limit ends on the near side, with no cut
        let short = near_side_track(&tr, &cam, now, 1, 300, 30);
        assert_eq!(short.len(), 11);
        assert!(short.last().unwrap().0.hypot(short.last().unwrap().1) < 0.99);
        // the walk also stops where the ephemeris does
        let cam_end = Camera::following(tr.at(9_500).unwrap());
        let end = near_side_track(&tr, &cam_end, 9_500, 1, 93 * 60, 30);
        assert_eq!(end.len(), 4, "9500, 9530, 9560, 9590; 9620 is past the end");
    }
}

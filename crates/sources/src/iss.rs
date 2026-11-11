//! ISS pass prediction on the Pi: the TLE from Celestrak once a day, SGP4
//! propagation, a topocentric elevation search for the next pass, and the
//! ground track around now for the globe screen.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use rackscreen_core::event::{Event, IssPass, IssTrack};

use crate::astro::{sun_direction, sun_elevation};
use crate::electricity::HttpStatus;
use crate::http::client;
use crate::SourceCtx;

const TLE_URL: &str = "https://celestrak.org/NORAD/elements/gp.php?CATNR=25544&FORMAT=TLE";
/// ARISS publishes the same ISS TLE; asked when Celestrak refuses.
const TLE_FALLBACK_URL: &str = "https://live.ariss.org/iss.txt";
pub const EARTH_RADIUS_KM: f64 = 6_371.0;
const WGS84_A: f64 = 6_378.137;
const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// Propagation step of the pass search.
const STEP_SECS: i64 = 10;
/// How far ahead to look for a pass.
const HORIZON_SECS: i64 = 24 * 3600;
/// The TLE is fetched again after this long, and used for up to a week when
/// the fetch keeps failing.
const TLE_REFRESH: Duration = Duration::from_secs(24 * 3600);
const TLE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// A failed TLE fetch is tried again after this long, then after
/// `TLE_RETRY_MAX`: asking again sooner only prolongs a Celestrak block.
const TLE_RETRY: Duration = Duration::from_secs(15 * 60);
const TLE_RETRY_MAX: Duration = Duration::from_secs(2 * 3600);
/// Passes are recomputed this often, and right after one ends.
const RECOMPUTE: Duration = Duration::from_secs(600);
/// The observer counts as dark below this sun elevation (civil twilight).
const DARK_DEG: f64 = -6.0;
/// The station is seen by eye from this elevation up.
const VISIBLE_MIN_DEG: f64 = 10.0;
/// Step of the visible-window search, and how far around the pass it looks
/// (a pass above a higher `min_elevation` is shorter than its 10° stretch).
const VISIBLE_STEP_SECS: i64 = 5;
const VISIBLE_MARGIN_SECS: i64 = 300;
/// The ephemeris for the globe: a point every `TRACK_STEP_SECS` from an
/// hour ago to 100 minutes ahead, sent again every `TRACK_EVERY`.
pub const TRACK_STEP_SECS: u32 = 20;
pub const TRACK_BACK_SECS: i64 = 60 * 60;
pub const TRACK_AHEAD_SECS: i64 = 100 * 60;
const TRACK_EVERY: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct IssConfig {
    pub lat: f64,
    pub lon: f64,
    pub min_elevation: f32,
    /// Where the last TLE is kept, so a restart does not fetch it again
    /// (Celestrak refuses clients that ask for the same data too often).
    pub cache: Option<std::path::PathBuf>,
}

/// The TLE cache file under the service's state directory.
pub fn tle_cache_path_in(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("rackscreen").join("iss.tle")
}

/// The cached TLE and how old it is, when the file is there and parses.
pub fn read_tle_cache(path: &std::path::Path) -> Option<((String, String), Duration)> {
    let text = std::fs::read_to_string(path).ok()?;
    let age = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .unwrap_or(Duration::ZERO);
    Some((parse_tle(&text).ok()?, age))
}

/// Keep the TLE for the next start: a temp file renamed into place.
pub fn write_tle_cache(path: &std::path::Path, tle: &(String, String)) -> Result<()> {
    let dir = path.parent().context("cache path has no parent")?;
    std::fs::create_dir_all(dir).context("create the cache directory")?;
    let tmp = path.with_extension("tle.tmp");
    std::fs::write(&tmp, format!("ISS (ZARYA)\n{}\n{}\n", tle.0, tle.1)).context("write")?;
    std::fs::rename(&tmp, path).context("rename")?;
    Ok(())
}

/// The two element lines out of a Celestrak TLE response.
pub fn parse_tle(text: &str) -> Result<(String, String)> {
    let l1 = text
        .lines()
        .map(str::trim_end)
        .find(|l| l.starts_with("1 ") && l.len() >= 69)
        .ok_or_else(|| anyhow!("TLE line 1 missing"))?;
    let l2 = text
        .lines()
        .map(str::trim_end)
        .find(|l| l.starts_with("2 ") && l.len() >= 69)
        .ok_or_else(|| anyhow!("TLE line 2 missing"))?;
    Ok((l1.to_string(), l2.to_string()))
}

fn elements(l1: &str, l2: &str) -> Result<(sgp4::Elements, sgp4::Constants)> {
    let el = sgp4::Elements::from_tle(Some("ISS".into()), l1.as_bytes(), l2.as_bytes())
        .map_err(|e| anyhow!("tle: {e}"))?;
    let k = sgp4::Constants::from_elements(&el).map_err(|e| anyhow!("sgp4: {e}"))?;
    Ok((el, k))
}

/// Satellite position in TEME kilometres at a unix time.
pub fn position_teme(l1: &str, l2: &str, unix: i64) -> Result<[f64; 3]> {
    let (el, k) = elements(l1, l2)?;
    propagate(&el, &k, unix)
}

fn propagate(el: &sgp4::Elements, k: &sgp4::Constants, unix: i64) -> Result<[f64; 3]> {
    let dt = chrono::DateTime::from_timestamp(unix, 0)
        .context("timestamp")?
        .naive_utc();
    let t = el
        .datetime_to_minutes_since_epoch(&dt)
        .map_err(|e| anyhow!("epoch: {e}"))?;
    let p = k.propagate(t).map_err(|e| anyhow!("propagate: {e}"))?;
    Ok(p.position)
}

/// Greenwich mean sidereal time in radians.
pub fn gmst_rad(unix: f64) -> f64 {
    let jd = unix / 86_400.0 + 2_440_587.5;
    let d = jd - 2_451_545.0;
    let t = d / 36_525.0;
    let g =
        280.460_618_37 + 360.985_647_366_29 * d + 0.000_387_933 * t * t - t * t * t / 38_710_000.0;
    g.rem_euclid(360.0).to_radians()
}

fn teme_to_ecef(r: [f64; 3], gmst: f64) -> [f64; 3] {
    let (s, c) = gmst.sin_cos();
    [r[0] * c + r[1] * s, -r[0] * s + r[1] * c, r[2]]
}

/// WGS84 observer position at sea level, kilometres.
pub fn observer_ecef(lat: f64, lon: f64) -> [f64; 3] {
    let (latr, lonr) = (lat.to_radians(), lon.to_radians());
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let n = WGS84_A / (1.0 - e2 * latr.sin().powi(2)).sqrt();
    [
        n * latr.cos() * lonr.cos(),
        n * latr.cos() * lonr.sin(),
        n * (1.0 - e2) * latr.sin(),
    ]
}

/// Elevation of an ECEF point above the observer's horizon, degrees.
pub fn topocentric_elevation(sat: [f64; 3], obs: [f64; 3], lat: f64, lon: f64) -> f64 {
    let (latr, lonr) = (lat.to_radians(), lon.to_radians());
    let d = [sat[0] - obs[0], sat[1] - obs[1], sat[2] - obs[2]];
    let e = -lonr.sin() * d[0] + lonr.cos() * d[1];
    let n = -latr.sin() * lonr.cos() * d[0] - latr.sin() * lonr.sin() * d[1] + latr.cos() * d[2];
    let u = latr.cos() * lonr.cos() * d[0] + latr.cos() * lonr.sin() * d[1] + latr.sin() * d[2];
    u.atan2((e * e + n * n).sqrt()).to_degrees()
}

/// Elevation of the ISS from the observer at a unix time.
pub fn elevation_deg(l1: &str, l2: &str, lat: f64, lon: f64, unix: i64) -> Result<f64> {
    let r = position_teme(l1, l2, unix)?;
    let sat = teme_to_ecef(r, gmst_rad(unix as f64));
    Ok(topocentric_elevation(
        sat,
        observer_ecef(lat, lon),
        lat,
        lon,
    ))
}

/// The satellite is lit when it is on the sun's side of the Earth's centre or
/// outside the Earth's shadow cylinder.
fn sunlit(r_teme: [f64; 3], unix: i64) -> bool {
    let s = sun_direction(unix);
    let along = r_teme[0] * s[0] + r_teme[1] * s[1] + r_teme[2] * s[2];
    if along > 0.0 {
        return true;
    }
    let perp = [
        r_teme[0] - along * s[0],
        r_teme[1] - along * s[1],
        r_teme[2] - along * s[2],
    ];
    (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt() > EARTH_RADIUS_KM
}

/// Geodetic `(lat, lon)` in degrees of the point under an ECEF position.
pub fn ground_point(ecef: [f64; 3]) -> (f64, f64) {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let p = ecef[0].hypot(ecef[1]);
    let mut lat = ecef[2].atan2(p * (1.0 - e2));
    for _ in 0..3 {
        let (sin, cos) = lat.sin_cos();
        let root = (1.0 - e2 * sin * sin).sqrt();
        let n = WGS84_A / root;
        // height above the ellipsoid, well behaved at the poles too
        let h = p * cos + ecef[2] * sin - WGS84_A * root;
        lat = ecef[2].atan2(p * (1.0 - e2 * n / (n + h)));
    }
    (lat.to_degrees(), ecef[1].atan2(ecef[0]).to_degrees())
}

/// The ground track from `TRACK_BACK_SECS` before `now` to
/// `TRACK_AHEAD_SECS` after it, for an observer at `(lat, lon)`.
pub fn ephemeris(l1: &str, l2: &str, lat: f64, lon: f64, now: i64) -> Result<IssTrack> {
    let (el, k) = elements(l1, l2)?;
    let t0 = now - TRACK_BACK_SECS;
    let n = (TRACK_BACK_SECS + TRACK_AHEAD_SECS) / TRACK_STEP_SECS as i64;
    let points = (0..=n)
        .map(|i| {
            let t = t0 + i * TRACK_STEP_SECS as i64;
            let r = propagate(&el, &k, t)?;
            let (lat, lon) = ground_point(teme_to_ecef(r, gmst_rad(t as f64)));
            Ok((lat as f32, lon as f32))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(IssTrack {
        t0,
        step_secs: TRACK_STEP_SECS,
        points,
        home: (lat as f32, lon as f32),
    })
}

/// The stretch around a pass (`start` to `end`) in which the station can be
/// seen by eye: at least 10° up, the observer in the dark, the station in
/// sunlight. `None` for a daytime pass or one in the Earth's shadow.
fn visible_window(
    el: &sgp4::Elements,
    k: &sgp4::Constants,
    lat: f64,
    lon: f64,
    start: i64,
    end: i64,
) -> Result<Option<(i64, i64)>> {
    let obs = observer_ecef(lat, lon);
    let mut window: Option<(i64, i64)> = None;
    let mut t = start - VISIBLE_MARGIN_SECS;
    while t <= end + VISIBLE_MARGIN_SECS {
        let r = propagate(el, k, t)?;
        let up = topocentric_elevation(teme_to_ecef(r, gmst_rad(t as f64)), obs, lat, lon);
        if up >= VISIBLE_MIN_DEG && sun_elevation(lat, lon, t) < DARK_DEG && sunlit(r, t) {
            window = Some((window.map_or(t, |w| w.0), t + VISIBLE_STEP_SECS));
        } else if window.is_some() {
            break;
        }
        t += VISIBLE_STEP_SECS;
    }
    Ok(window)
}

/// The first pass above `min_elevation` that ends after `from`, searched in
/// ten-second steps over the next 24 hours.
pub fn next_pass(
    l1: &str,
    l2: &str,
    lat: f64,
    lon: f64,
    min_elevation: f32,
    from: i64,
) -> Result<Option<IssPass>> {
    let (el, k) = elements(l1, l2)?;
    let obs = observer_ecef(lat, lon);
    let min = min_elevation as f64;
    let mut t = from;
    let mut start: Option<i64> = None;
    let mut max_el = f64::MIN;
    let mut max_at = from;
    let mut max_r = [0.0; 3];
    while t <= from + HORIZON_SECS {
        let r = propagate(&el, &k, t)?;
        let elv = topocentric_elevation(teme_to_ecef(r, gmst_rad(t as f64)), obs, lat, lon);
        if elv >= min {
            if start.is_none() {
                start = Some(t);
                max_el = f64::MIN;
            }
            if elv > max_el {
                max_el = elv;
                max_at = t;
                max_r = r;
            }
        } else if let Some(s) = start {
            let visible = sun_elevation(lat, lon, max_at) < DARK_DEG && sunlit(max_r, max_at);
            return Ok(Some(IssPass {
                start: s,
                end: t,
                max_elevation_deg: max_el as f32,
                visible,
                vis: visible_window(&el, &k, lat, lon, s, t)?,
            }));
        }
        t += STEP_SECS;
    }
    Ok(None)
}

async fn fetch_tle() -> Result<(String, String)> {
    match fetch_tle_from(TLE_URL).await {
        Ok(tle) => Ok(tle),
        Err(first) => fetch_tle_from(TLE_FALLBACK_URL)
            .await
            .with_context(|| format!("celestrak: {first:#}; ariss")),
    }
}

async fn fetch_tle_from(url: &str) -> Result<(String, String)> {
    let resp = client().get(url).send().await.context("request")?;
    let status = resp.status();
    let body = crate::http::read_capped(resp, 64 * 1024, "the TLE").await?;
    let body = String::from_utf8_lossy(&body);
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    let tle = parse_tle(&body)?;
    // a TLE the propagator refuses is no TLE
    elements(&tle.0, &tle.1)?;
    Ok(tle)
}

/// Refresh the TLE, then recompute the next pass and the ground track on
/// their timers. With no usable TLE the loop emits nothing at all:
/// `Event::IssPass(None)` would set `IssState::have`, which is what the
/// role's no-data state keys off, so a Pi with no internet would claim "no
/// pass in the next 24 h" instead of showing `cloud-off`. A TLE that has
/// gone stale keeps serving its last computed pass until a fetch succeeds or
/// `TLE_MAX_AGE` drops it.
pub async fn run_iss(cfg: IssConfig, ctx: SourceCtx) {
    let mut tle: Option<(String, String)> = None;
    let mut fetched = std::time::Instant::now();
    if let Some((cached, age)) = cfg.cache.as_deref().and_then(read_tle_cache) {
        if age < TLE_MAX_AGE {
            tracing::info!("iss: TLE from the cache, {} h old", age.as_secs() / 3600);
            tle = Some(cached);
            // an old cache is refreshed now; a fresh one when it is a day old
            fetched = std::time::Instant::now()
                .checked_sub(age)
                .unwrap_or_else(std::time::Instant::now);
        }
    }
    // after a failed fetch: when to try again, and the wait after that
    let mut retry_at: Option<std::time::Instant> = None;
    let mut retry_wait = TLE_RETRY;
    // when the pass was last computed, and when it has to be again
    let mut pass_at: Option<std::time::Instant> = None;
    let mut pass_due = RECOMPUTE;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let stale = tle.is_none() || fetched.elapsed() >= TLE_REFRESH;
        if stale && retry_at.is_none_or(|at| at.elapsed() > Duration::ZERO) {
            match fetch_tle().await {
                Ok(t) => {
                    tracing::info!("iss: TLE refreshed");
                    if let Some(path) = &cfg.cache {
                        if let Err(e) = write_tle_cache(path, &t) {
                            tracing::debug!("iss: TLE cache: {e:#}");
                        }
                    }
                    tle = Some(t);
                    fetched = std::time::Instant::now();
                    pass_at = None;
                    retry_at = None;
                    retry_wait = TLE_RETRY;
                }
                Err(e) => {
                    tracing::warn!(
                        "iss: TLE fetch: {e:#}; next try in {} min",
                        retry_wait.as_secs() / 60
                    );
                    retry_at = Some(std::time::Instant::now() + retry_wait);
                    retry_wait = (retry_wait * 2).min(TLE_RETRY_MAX);
                    if fetched.elapsed() >= TLE_MAX_AGE {
                        tle = None;
                    }
                }
            }
        }
        let mut wait = TRACK_EVERY;
        // no TLE: stay silent so the role keeps `have: false` and renders
        // its no-data icon rather than an empty ring
        if let Some((l1, l2)) = &tle {
            let now = chrono::Utc::now().timestamp();
            if pass_at.is_none_or(|at| at.elapsed() >= pass_due) {
                pass_due = RECOMPUTE;
                match next_pass(l1, l2, cfg.lat, cfg.lon, cfg.min_elevation, now) {
                    Ok(pass) => {
                        if let Some(p) = pass {
                            tracing::info!(
                                "iss: next pass in {} min, max {:.0}°{}",
                                (p.start - now) / 60,
                                p.max_elevation_deg,
                                if p.visible { ", visible" } else { "" }
                            );
                            // recompute right after the pass ends
                            let until_end = Duration::from_secs((p.end - now).max(1) as u64 + 5);
                            pass_due = pass_due.min(until_end);
                        }
                        pass_at = Some(std::time::Instant::now());
                        ctx.emit(Event::IssPass(pass));
                    }
                    Err(e) => tracing::warn!("iss: {e:#}"),
                }
            }
            // the pass first: the globe only shows once both are known
            match ephemeris(l1, l2, cfg.lat, cfg.lon, now) {
                Ok(track) => ctx.emit(Event::IssTrack(track)),
                Err(e) => tracing::warn!("iss: track: {e:#}"),
            }
            if let Some(at) = pass_at {
                wait = wait.min(pass_due.saturating_sub(at.elapsed()));
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait.max(Duration::from_secs(1))) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TLE: &str = include_str!("../tests/fixtures/iss.tle");
    /// 2026-09-07T12:00Z, the TLE epoch.
    const EPOCH: i64 = 1_788_782_400;

    #[test]
    fn the_tle_cache_round_trips_and_knows_its_age() {
        let dir = tempfile::tempdir().unwrap();
        let path = tle_cache_path_in(dir.path());
        assert!(read_tle_cache(&path).is_none(), "no file yet");
        let tle = parse_tle(include_str!("../tests/fixtures/iss.tle")).unwrap();
        write_tle_cache(&path, &tle).unwrap();
        let (back, age) = read_tle_cache(&path).unwrap();
        assert_eq!(back, tle);
        assert!(age < Duration::from_secs(60));
        std::fs::write(&path, "garbage").unwrap();
        assert!(
            read_tle_cache(&path).is_none(),
            "an unparsable cache is ignored"
        );
    }

    #[test]
    fn the_ariss_tle_format_propagates() {
        // ARISS writes the drag exponent as "00000-0" where Celestrak has "00000+0"
        let text = "ISS (ZARYA)\n1 25544U 98067A   26314.17503267  .00009346  00000-0  17575-3 0  9997\n2 25544  51.6322 297.6644 0007292   8.9462 351.1656 15.49289712594382\n";
        let (l1, l2) = parse_tle(text).unwrap();
        let r = position_teme(&l1, &l2, 1_794_355_200).unwrap();
        let alt = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() - EARTH_RADIUS_KM;
        assert!((380.0..450.0).contains(&alt), "{alt}");
    }

    #[test]
    fn parses_the_two_lines() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        assert!(l1.starts_with("1 25544U"));
        assert!(l2.starts_with("2 25544 "));
        assert!(parse_tle("ISS\n1 nope\n").is_err());
        assert!(parse_tle("").is_err());
    }

    #[test]
    fn iss_is_at_orbital_altitude() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let r = position_teme(&l1, &l2, EPOCH + 600).unwrap();
        let alt = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() - EARTH_RADIUS_KM;
        assert!(alt > 380.0 && alt < 460.0, "altitude {alt} km");
    }

    #[test]
    fn a_pass_over_amsterdam_exists_within_a_day() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let pass = next_pass(&l1, &l2, 52.37, 4.89, 10.0, EPOCH)
            .unwrap()
            .expect("the ISS passes Amsterdam several times a day");
        assert!(pass.start >= EPOCH && pass.end > pass.start);
        assert!(pass.end - pass.start < 15 * 60, "passes are minutes long");
        assert!(pass.max_elevation_deg >= 10.0 && pass.max_elevation_deg <= 90.0);
        // a pass is where the elevation really clears the threshold
        let mid = (pass.start + pass.end) / 2;
        let el = elevation_deg(&l1, &l2, 52.37, 4.89, mid).unwrap();
        assert!(el > 5.0, "mid-pass elevation {el}");
        // the next pass after this one starts after it ends
        let later = next_pass(&l1, &l2, 52.37, 4.89, 10.0, pass.end + 1)
            .unwrap()
            .unwrap();
        assert!(later.start > pass.end);
        // an impossible threshold finds nothing
        assert!(next_pass(&l1, &l2, 52.37, 4.89, 89.9, EPOCH)
            .unwrap()
            .is_none());
    }

    #[test]
    fn observer_and_frames() {
        let o = observer_ecef(0.0, 0.0);
        assert!((o[0] - 6378.137).abs() < 0.01 && o[1].abs() < 1e-6 && o[2].abs() < 1e-6);
        let p = observer_ecef(90.0, 0.0);
        assert!((p[2] - 6356.75).abs() < 0.1, "{p:?}");
        // GMST at J2000 noon is about 18.697 h = 280.46°
        let g = gmst_rad(946_728_000.0).to_degrees();
        assert!((g - 280.46).abs() < 0.05, "{g}");
        // a point straight above the observer has elevation 90
        let obs = observer_ecef(52.37, 4.89);
        let up = [obs[0] * 1.06, obs[1] * 1.06, obs[2] * 1.06];
        assert!((topocentric_elevation(up, obs, 52.37, 4.89) - 90.0).abs() < 0.5);
    }

    #[test]
    fn the_ground_point_is_geodetic() {
        let (lat, lon) = ground_point(observer_ecef(52.37, 4.89));
        assert!((lat - 52.37).abs() < 1e-6 && (lon - 4.89).abs() < 1e-6);
        // 400 km above it, the same point is underneath
        let o = observer_ecef(-33.9, 151.2);
        let (lat, lon) = ground_point([o[0] * 1.06, o[1] * 1.06, o[2] * 1.06]);
        assert!(
            (lat + 33.9).abs() < 0.15 && (lon - 151.2).abs() < 1e-6,
            "{lat}"
        );
        let (lat, _) = ground_point([0.0, 0.0, 6_800.0]);
        assert!((lat - 90.0).abs() < 1e-6);
    }

    #[test]
    fn the_ephemeris_covers_an_hour_back_and_an_orbit_ahead() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let now = EPOCH + 3 * 3600;
        let tr = ephemeris(&l1, &l2, 52.37, 4.89, now).unwrap();
        assert_eq!((tr.t0, tr.step_secs), (now - 3600, 20));
        assert_eq!(tr.points.len(), 481, "160 minutes in 20 s steps, both ends");
        assert_eq!(tr.home, (52.37, 4.89));
        assert!(tr.at(now - 3600).is_some() && tr.at(now + 6000).is_some());
        assert!(tr.at(now + 6001).is_none());
        // the track stays inside the orbit's inclination and reaches it
        let max = tr.points.iter().map(|p| p.0.abs()).fold(0.0, f32::max);
        assert!(max > 51.0 && max < 52.0, "{max}");
        assert!(tr.points.iter().all(|p| p.1.abs() <= 180.0));
        // consecutive points are 20 s of orbit apart: about 1.2° of arc
        for w in tr.points.windows(2) {
            let (a, b) = (
                rackscreen_core::globe::unit(w[0].0 as f64, w[0].1 as f64),
                rackscreen_core::globe::unit(w[1].0 as f64, w[1].1 as f64),
            );
            let arc = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            assert!(arc > 1.0 && arc < 1.4, "{arc}");
        }
        // the interpolated point is where SGP4 puts the station then
        let t = now + 1234;
        let r = position_teme(&l1, &l2, t).unwrap();
        let (lat, lon) = ground_point(teme_to_ecef(r, gmst_rad(t as f64)));
        let (ilat, ilon) = rackscreen_core::globe::lat_lon(tr.at(t).unwrap());
        assert!((lat - ilat).abs() < 0.02 && (lon - ilon).abs() < 0.02);
        // during a pass the track runs close by the observer
        let pass = next_pass(&l1, &l2, 52.37, 4.89, 40.0, EPOCH)
            .unwrap()
            .expect("a high pass within a day");
        let mid = (pass.start + pass.end) / 2;
        let tr = ephemeris(&l1, &l2, 52.37, 4.89, mid).unwrap();
        let (lat, lon) = rackscreen_core::globe::lat_lon(tr.at(mid).unwrap());
        assert!(
            (lat - 52.37).abs() < 5.0 && (lon - 4.89).abs() < 8.0,
            "overhead at ({lat}, {lon})"
        );
    }

    /// Every pass over Amsterdam in the fixture's first five days.
    fn passes(days: i64) -> Vec<IssPass> {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let mut out = Vec::new();
        let mut from = EPOCH;
        while from < EPOCH + days * 86_400 {
            match next_pass(&l1, &l2, 52.37, 4.89, 10.0, from).unwrap() {
                Some(p) => {
                    from = p.end + 60;
                    out.push(p);
                }
                None => from += HORIZON_SECS,
            }
        }
        out
    }

    #[test]
    fn the_visible_window_is_the_dark_sunlit_stretch_above_ten_degrees() {
        let (l1, l2) = parse_tle(TLE).unwrap();
        let (el, k) = elements(&l1, &l2).unwrap();
        let obs = observer_ecef(52.37, 4.89);
        let all = passes(5);
        assert!(all.len() > 15, "{} passes in five days", all.len());
        let seen: Vec<_> = all.iter().filter(|p| p.vis.is_some()).collect();
        assert!(!seen.is_empty(), "September evenings have visible passes");
        assert!(seen.len() < all.len(), "daytime passes have none");
        let holds = |t: i64| {
            let r = propagate(&el, &k, t).unwrap();
            let up = topocentric_elevation(teme_to_ecef(r, gmst_rad(t as f64)), obs, 52.37, 4.89);
            up >= 10.0 && sun_elevation(52.37, 4.89, t) < -6.0 && sunlit(r, t)
        };
        for p in &all {
            match p.vis {
                Some((a, b)) => {
                    assert!(a < b && b - a <= p.end - p.start + 10, "{p:?}");
                    assert!(a >= p.start - 5 && b <= p.end + 5, "inside the 10° pass");
                    assert!(holds(a) && holds(b - 5) && holds((a + b) / 2));
                    assert!(!holds(a - 6) && !holds(b + 1), "and no longer: {p:?}");
                }
                None => {
                    let mut t = p.start;
                    while t <= p.end {
                        assert!(!holds(t), "{p:?} is visible at {t}");
                        t += 5;
                    }
                }
            }
            // a pass visible at its peak has a window around the peak
            if p.visible {
                assert!(p.vis.is_some(), "{p:?}");
            }
        }
        // a higher threshold shortens the pass, not the window
        let high = next_pass(&l1, &l2, 52.37, 4.89, 40.0, seen[0].start - 600).unwrap();
        if let Some(h) = high.filter(|h| h.start < seen[0].end) {
            let (a, b) = h.vis.expect("the same pass, still visible");
            assert!(a <= h.start && b >= h.end.min(seen[0].vis.unwrap().1));
        }
    }
}

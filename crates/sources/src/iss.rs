//! ISS pass prediction on the Pi: the TLE from Celestrak once a day, SGP4
//! propagation, a topocentric elevation search for the next pass.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use rackscreen_core::event::{Event, IssPass};

use crate::astro::{sun_direction, sun_elevation};
use crate::electricity::HttpStatus;
use crate::http::client;
use crate::SourceCtx;

const TLE_URL: &str = "https://celestrak.org/NORAD/elements/gp.php?CATNR=25544&FORMAT=TLE";
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
/// Passes are recomputed this often, and right after one ends.
const RECOMPUTE: Duration = Duration::from_secs(600);
/// The observer counts as dark below this sun elevation (civil twilight).
const DARK_DEG: f64 = -6.0;

#[derive(Clone, Debug)]
pub struct IssConfig {
    pub lat: f64,
    pub lon: f64,
    pub min_elevation: f32,
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
            }));
        }
        t += STEP_SECS;
    }
    Ok(None)
}

async fn fetch_tle() -> Result<(String, String)> {
    let resp = client().get(TLE_URL).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    parse_tle(&body)
}

pub async fn run_iss(cfg: IssConfig, ctx: SourceCtx) {
    let mut tle: Option<(String, String)> = None;
    let mut fetched = std::time::Instant::now();
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let stale = tle.is_none() || fetched.elapsed() >= TLE_REFRESH;
        if stale {
            match fetch_tle().await {
                Ok(t) => {
                    tracing::info!("iss: TLE refreshed");
                    tle = Some(t);
                    fetched = std::time::Instant::now();
                }
                Err(e) => {
                    tracing::warn!("iss: TLE fetch: {e:#}");
                    if fetched.elapsed() >= TLE_MAX_AGE {
                        tle = None;
                    }
                }
            }
        }
        let mut wait = RECOMPUTE;
        match &tle {
            None => ctx.emit(Event::IssPass(None)),
            Some((l1, l2)) => {
                let now = chrono::Utc::now().timestamp();
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
                            wait = wait.min(until_end);
                        }
                        ctx.emit(Event::IssPass(pass));
                    }
                    Err(e) => tracing::warn!("iss: {e:#}"),
                }
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
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
}

//! Sun and moon without a network: NOAA's solar position algorithm and a
//! mean-synodic moon, both good to a minute or two, which is all a ring needs.

use std::f64::consts::PI;
use std::time::Duration;

use rackscreen_core::event::{Event, MoonPhase};

use crate::SourceCtx;

const SYNODIC_DAYS: f64 = 29.530_588_853;
/// A reference new moon: 2000-01-06 18:14 UTC.
const NEW_MOON_REF: i64 = 947_182_440;
/// Sunrise and sunset are the sun's centre at this zenith (refraction included).
const RISE_SET_ZENITH_DEG: f64 = 90.833;

fn julian_day(unix: f64) -> f64 {
    unix / 86_400.0 + 2_440_587.5
}

/// Solar declination (deg), equation of time (minutes) and the apparent
/// longitude / obliquity (deg) at a unix time. NOAA general solar position.
fn solar(unix: f64) -> (f64, f64, f64, f64) {
    let jc = (julian_day(unix) - 2_451_545.0) / 36_525.0;
    let l0 = (280.466_46 + jc * (36_000.769_83 + jc * 0.000_303_2)).rem_euclid(360.0);
    let m = 357.529_11 + jc * (35_999.050_29 - 0.000_153_7 * jc);
    let e = 0.016_708_634 - jc * (0.000_042_037 + 0.000_000_126_7 * jc);
    let mr = m.to_radians();
    let c = mr.sin() * (1.914_602 - jc * (0.004_817 + 0.000_014 * jc))
        + (2.0 * mr).sin() * (0.019_993 - 0.000_101 * jc)
        + (3.0 * mr).sin() * 0.000_289;
    let true_long = l0 + c;
    let omega = (125.04 - 1_934.136 * jc).to_radians();
    let lambda = true_long - 0.005_69 - 0.004_78 * omega.sin();
    let eps0 =
        23.0 + (26.0 + (21.448 - jc * (46.815 + jc * (0.000_59 - jc * 0.001_813))) / 60.0) / 60.0;
    let eps = eps0 + 0.002_56 * omega.cos();
    let decl = (eps.to_radians().sin() * lambda.to_radians().sin())
        .asin()
        .to_degrees();
    let y = (eps.to_radians() / 2.0).tan().powi(2);
    let l0r = l0.to_radians();
    let eot = 4.0
        * (y * (2.0 * l0r).sin() - 2.0 * e * mr.sin() + 4.0 * e * y * mr.sin() * (2.0 * l0r).cos()
            - 0.5 * y * y * (4.0 * l0r).sin()
            - 1.25 * e * e * (2.0 * mr).sin())
        .to_degrees();
    (decl, eot, lambda, eps)
}

/// Sunrise and sunset (unix seconds) for the local day starting at
/// `local_midnight` (unix seconds). `None` for polar day or night.
pub fn sun_times(lat: f64, lon: f64, local_midnight: i64) -> (Option<i64>, Option<i64>) {
    let local_noon = local_midnight + 43_200;
    let utc_day = local_noon.div_euclid(86_400) * 86_400;
    let (decl, eot, _, _) = solar(local_noon as f64);
    let (latr, dr) = (lat.to_radians(), decl.to_radians());
    let cos_ha =
        RISE_SET_ZENITH_DEG.to_radians().cos() / (latr.cos() * dr.cos()) - latr.tan() * dr.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return (None, None);
    }
    let ha_min = cos_ha.acos().to_degrees() * 4.0;
    let noon_min = 720.0 - 4.0 * lon - eot;
    let at = |min: f64| utc_day + (min * 60.0).round() as i64;
    (Some(at(noon_min - ha_min)), Some(at(noon_min + ha_min)))
}

/// Sun elevation above the horizon in degrees, no refraction.
pub fn sun_elevation(lat: f64, lon: f64, unix: i64) -> f64 {
    let (decl, eot, _, _) = solar(unix as f64);
    let minutes = (unix.rem_euclid(86_400)) as f64 / 60.0;
    let tst = (minutes + eot + 4.0 * lon).rem_euclid(1_440.0);
    let ha = (tst / 4.0 - 180.0).to_radians();
    let (latr, dr) = (lat.to_radians(), decl.to_radians());
    (latr.sin() * dr.sin() + latr.cos() * dr.cos() * ha.cos())
        .asin()
        .to_degrees()
}

/// Unit vector toward the sun in equatorial (TEME-compatible) coordinates.
pub fn sun_direction(unix: i64) -> [f64; 3] {
    let (_, _, lambda, eps) = solar(unix as f64);
    let (l, e) = (lambda.to_radians(), eps.to_radians());
    [l.cos(), e.cos() * l.sin(), e.sin() * l.sin()]
}

/// Illuminated fraction, whether the moon is waxing, and the phase name.
pub fn moon(unix: i64) -> (f32, bool, MoonPhase) {
    let age = ((unix - NEW_MOON_REF) as f64 / 86_400.0).rem_euclid(SYNODIC_DAYS);
    let p = age / SYNODIC_DAYS;
    let illumination = (1.0 - (2.0 * PI * p).cos()) / 2.0;
    let phase = match p {
        p if p < 0.0625 => MoonPhase::New,
        p if p < 0.1875 => MoonPhase::WaxingCrescent,
        p if p < 0.3125 => MoonPhase::FirstQuarter,
        p if p < 0.4375 => MoonPhase::WaxingGibbous,
        p if p < 0.5625 => MoonPhase::Full,
        p if p < 0.6875 => MoonPhase::WaningGibbous,
        p if p < 0.8125 => MoonPhase::LastQuarter,
        p if p < 0.9375 => MoonPhase::WaningCrescent,
        _ => MoonPhase::New,
    };
    (illumination as f32, p < 0.5, phase)
}

/// The `Sky` event for `unix` at an observer: today's sunrise and sunset in
/// the local day, or tomorrow's sunrise once today's sunset has passed.
pub fn sky_event(lat: f64, lon: f64, unix: i64, utc_offset_secs: i32) -> Event {
    let offset = utc_offset_secs as i64;
    let local_midnight = (unix + offset).div_euclid(86_400) * 86_400 - offset;
    let (mut sunrise, sunset) = sun_times(lat, lon, local_midnight);
    if sunset.is_some_and(|s| s < unix) {
        sunrise = sun_times(lat, lon, local_midnight + 86_400).0;
    }
    let (moon_illumination, moon_waxing, moon_phase) = moon(unix);
    Event::Sky {
        sunrise,
        sunset,
        sun_elevation_deg: sun_elevation(lat, lon, unix) as f32,
        moon_illumination,
        moon_waxing,
        moon_phase,
    }
}

pub async fn run_astro(lat: f64, lon: f64, ctx: SourceCtx) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = ticker.tick() => {
                let now = chrono::Local::now();
                let offset = now.offset().local_minus_utc();
                ctx.emit(sky_event(lat, lon, now.timestamp(), offset));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMS: (f64, f64) = (52.37, 4.89);
    const JUN21_UTC: i64 = 1_782_000_000; // 2026-06-21T00:00Z
    const DEC21_UTC: i64 = 1_797_811_200; // 2026-12-21T00:00Z

    fn close(a: i64, b: i64, tol: i64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn amsterdam_sunrise_and_sunset_on_the_solstices() {
        // local midnight CEST is 22:00Z the day before
        let (rise, set) = sun_times(AMS.0, AMS.1, JUN21_UTC - 7200);
        // 05:18 CEST = 03:18Z, 22:05 CEST = 20:05Z, within 15 minutes
        assert!(
            close(rise.unwrap(), JUN21_UTC + 3 * 3600 + 18 * 60, 900),
            "{rise:?}"
        );
        assert!(
            close(set.unwrap(), JUN21_UTC + 20 * 3600 + 5 * 60, 900),
            "{set:?}"
        );
        // CET: local midnight is 23:00Z the day before
        let (rise, set) = sun_times(AMS.0, AMS.1, DEC21_UTC - 3600);
        // 08:47 CET = 07:47Z, 16:31 CET = 15:31Z
        assert!(
            close(rise.unwrap(), DEC21_UTC + 7 * 3600 + 47 * 60, 900),
            "{rise:?}"
        );
        assert!(
            close(set.unwrap(), DEC21_UTC + 15 * 3600 + 31 * 60, 900),
            "{set:?}"
        );
    }

    #[test]
    fn polar_day_and_night_have_no_events() {
        assert_eq!(sun_times(80.0, 20.0, JUN21_UTC), (None, None));
        assert_eq!(sun_times(80.0, 20.0, DEC21_UTC), (None, None));
    }

    #[test]
    fn sun_elevation_is_high_at_noon_and_negative_at_midnight() {
        let noon = sun_elevation(AMS.0, AMS.1, JUN21_UTC + 11 * 3600 + 40 * 60);
        assert!(noon > 60.0 && noon < 62.5, "{noon}");
        let night = sun_elevation(AMS.0, AMS.1, JUN21_UTC);
        assert!(night < -10.0, "{night}");
        let d = sun_direction(JUN21_UTC);
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
        assert!(
            d[2] > 0.38,
            "the sun is far north of the equator in June: {d:?}"
        );
    }

    #[test]
    fn moon_phases_on_known_dates() {
        // full moon 2026-01-03 10:03Z, new moon 2026-01-18 19:52Z
        let (illum, waxing, phase) = moon(1_767_434_400);
        assert!(illum > 0.9, "{illum}");
        assert_eq!(phase, MoonPhase::Full);
        let (illum, _, phase) = moon(1_768_765_920);
        assert!(illum < 0.1, "{illum}");
        assert_eq!(phase, MoonPhase::New);
        // a week after new: waxing, about half
        let (illum, waxing2, phase) = moon(1_768_765_920 + 7 * 86_400 + 12 * 3600);
        assert!(illum > 0.4 && illum < 0.6, "{illum}");
        assert!(waxing2);
        assert_eq!(phase, MoonPhase::FirstQuarter);
        let _ = waxing;
    }

    #[test]
    fn sky_event_uses_tomorrows_sunrise_after_sunset() {
        // 2026-06-21 22:30 local (20:30Z): sunset has passed
        let now = JUN21_UTC + 20 * 3600 + 30 * 60;
        let Event::Sky {
            sunrise, sunset, ..
        } = sky_event(AMS.0, AMS.1, now, 7200)
        else {
            panic!()
        };
        assert!(sunset.unwrap() < now, "today's sunset is in the past");
        assert!(
            sunrise.unwrap() > now,
            "so the sunrise offered is tomorrow's"
        );
        assert!(sunrise.unwrap() - now < 8 * 3600);
    }
}

//! Sun and moon derived values for the P2 cards: days to the next full and
//! new moon, the next sun event, daylight and progress through day or night.

use std::f32::consts::TAU;

pub const SYNODIC_DAYS: f32 = 29.530_588;

/// Days until the next full and the next new moon, from the illuminated
/// fraction and whether it is growing (a mean synodic month).
pub fn moon_days(illumination: f32, waxing: bool) -> (f32, f32) {
    let p = (1.0 - 2.0 * illumination.clamp(0.0, 1.0)).acos() / TAU;
    let phase = if waxing { p } else { 1.0 - p };
    let to_full = (0.5 - phase).rem_euclid(1.0) * SYNODIC_DAYS;
    let to_new = (1.0 - phase).rem_euclid(1.0) * SYNODIC_DAYS;
    (to_full, to_new)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SunEvent {
    Sunrise,
    Sunset,
}

/// The next event after `unix`: sunset by day, sunrise by night (tomorrow's
/// when the source has not rolled it yet). `None` on polar days and nights.
pub fn next_sun_event(
    sunrise: Option<i64>,
    sunset: Option<i64>,
    unix: i64,
) -> Option<(SunEvent, i64)> {
    let (r, s) = (sunrise?, sunset?);
    if unix >= r && unix < s {
        Some((SunEvent::Sunset, s))
    } else if unix < r {
        Some((SunEvent::Sunrise, r))
    } else {
        Some((SunEvent::Sunrise, if r > s { r } else { r + 86_400 }))
    }
}

pub fn daylight_secs(sunrise: Option<i64>, sunset: Option<i64>) -> Option<i64> {
    let (r, s) = (sunrise?, sunset?);
    let r = if r > s { r - 86_400 } else { r };
    (s > r).then_some(s - r)
}

/// Fraction of the current span that has passed: of the daylight by day, of
/// the night by night.
pub fn sun_progress(sunrise: Option<i64>, sunset: Option<i64>, unix: i64) -> Option<f32> {
    let (r, s) = (sunrise?, sunset?);
    let day = daylight_secs(sunrise, sunset)? as f32;
    let night = 86_400.0 - day;
    let r_today = if r > s { r - 86_400 } else { r };
    let f = if unix >= r_today && unix < s {
        (unix - r_today) as f32 / day
    } else if unix >= s {
        (unix - s) as f32 / night
    } else {
        (unix - (s - 86_400)) as f32 / night
    };
    Some(f.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_days_follow_the_synodic_month() {
        let (full, new) = moon_days(0.0, true);
        assert!(
            (full - SYNODIC_DAYS / 2.0).abs() < 0.01 && (new - 0.0).abs() < 0.01
                || (new - SYNODIC_DAYS).abs() < 0.01
        );
        let (full, new) = moon_days(1.0, true);
        assert!(full < 0.01 || (full - SYNODIC_DAYS).abs() < 0.01);
        assert!((new - SYNODIC_DAYS / 2.0).abs() < 0.01);
        let (full, new) = moon_days(0.5, true);
        assert!(
            (full - SYNODIC_DAYS / 4.0).abs() < 0.01,
            "first quarter: a week to full"
        );
        assert!((new - 3.0 * SYNODIC_DAYS / 4.0).abs() < 0.01);
        let (full, new) = moon_days(0.5, false);
        assert!(
            (full - 3.0 * SYNODIC_DAYS / 4.0).abs() < 0.01,
            "last quarter: three weeks to full"
        );
        assert!((new - SYNODIC_DAYS / 4.0).abs() < 0.01);
    }

    #[test]
    fn next_event_daylight_and_progress() {
        let (rise, set) = (Some(7 * 3600), Some(19 * 3600));
        assert_eq!(
            next_sun_event(rise, set, 12 * 3600),
            Some((SunEvent::Sunset, 19 * 3600))
        );
        assert_eq!(
            next_sun_event(rise, set, 5 * 3600),
            Some((SunEvent::Sunrise, 7 * 3600))
        );
        assert_eq!(
            next_sun_event(rise, set, 20 * 3600),
            Some((SunEvent::Sunrise, 31 * 3600)),
            "tonight: tomorrow's sunrise"
        );
        assert_eq!(
            next_sun_event(Some(31 * 3600), set, 20 * 3600),
            Some((SunEvent::Sunrise, 31 * 3600)),
            "the source already rolled sunrise"
        );
        assert_eq!(next_sun_event(None, set, 0), None);
        assert_eq!(daylight_secs(rise, set), Some(12 * 3600));
        assert_eq!(daylight_secs(Some(31 * 3600), set), Some(12 * 3600));
        assert!(
            (sun_progress(rise, set, 13 * 3600).unwrap() - 0.5).abs() < 1e-6,
            "midday"
        );
        assert!(
            (sun_progress(rise, set, 22 * 3600).unwrap() - 0.25).abs() < 1e-6,
            "three hours into a twelve hour night"
        );
        assert!(
            (sun_progress(rise, set, 4 * 3600).unwrap() - 0.75).abs() < 1e-6,
            "three hours before sunrise"
        );
        assert_eq!(sun_progress(None, None, 0), None);
    }
}

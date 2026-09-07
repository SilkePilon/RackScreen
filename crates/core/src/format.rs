//! Compact number formatting for the badge.

/// Bytes per second to a short badge string, e.g. `12.4M`, `850K`, `12B`.
pub fn fmt_speed(bps: i64) -> String {
    let b = bps.max(0) as f64;
    if b >= 1_048_576.0 {
        format!("{:.1}M", b / 1_048_576.0)
    } else if b >= 1024.0 {
        format!("{:.0}K", b / 1024.0)
    } else {
        format!("{}B", b as i64)
    }
}

/// Seconds to a short ETA, e.g. `45s`, `12m`, `1h05`. Unknown becomes `--`.
pub fn fmt_eta(secs: i64) -> String {
    if !(0..=864_000).contains(&secs) {
        return "--".into();
    }
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{:02}", secs / 3600, (secs % 3600) / 60)
    }
}

/// Seconds until something, as a countdown: `-42m`, `-5h48`. Past or unknown is `--`.
pub fn fmt_until(secs: i64) -> String {
    if secs < 0 {
        return "--".into();
    }
    let mins = (secs + 59) / 60;
    if mins < 60 {
        format!("-{}m", mins.max(1))
    } else {
        format!("-{}h{:02}", mins / 60, mins % 60)
    }
}

/// UPS runtime: `42 min` under an hour, `1h12` above.
pub fn fmt_runtime(secs: u32) -> String {
    let mins = secs / 60;
    if mins < 60 {
        format!("{mins} min")
    } else {
        format!("{}h{:02}", mins / 60, mins % 60)
    }
}

/// Bits per second as `512 kb`, `41 Mb` or `2.4 Gb`.
pub fn fmt_mbit(bps: f64) -> String {
    let b = bps.max(0.0);
    if b >= 1e9 {
        format!("{:.1} Gb", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} Mb", b / 1e6)
    } else {
        format!("{:.0} kb", b / 1e3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_units() {
        assert_eq!(fmt_speed(13_002_342), "12.4M");
        assert_eq!(fmt_speed(870_400), "850K");
        assert_eq!(fmt_speed(12), "12B");
        assert_eq!(fmt_speed(-5), "0B");
    }

    #[test]
    fn until_runtime_and_mbit() {
        assert_eq!(fmt_until(42 * 60), "-42m");
        assert_eq!(fmt_until(5 * 3600 + 48 * 60), "-5h48");
        assert_eq!(fmt_until(30), "-1m");
        assert_eq!(fmt_until(-5), "--");
        assert_eq!(fmt_runtime(42 * 60), "42 min");
        assert_eq!(fmt_runtime(72 * 60), "1h12");
        assert_eq!(fmt_runtime(30), "0 min");
        assert_eq!(fmt_mbit(41_300_000.0), "41 Mb");
        assert_eq!(fmt_mbit(512_000.0), "512 kb");
        assert_eq!(fmt_mbit(2_400_000_000.0), "2.4 Gb");
        assert_eq!(fmt_mbit(0.0), "0 kb");
    }

    #[test]
    fn eta_units() {
        assert_eq!(fmt_eta(45), "45s");
        assert_eq!(fmt_eta(720), "12m");
        assert_eq!(fmt_eta(3900), "1h05");
        assert_eq!(fmt_eta(-1), "--");
        assert_eq!(fmt_eta(8_640_000), "--");
    }
}

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

/// Seconds since something happened: `just now`, `42 s ago`, `2 m ago`, `2 h ago`.
pub fn fmt_ago(secs: f64) -> String {
    let s = secs.max(0.0);
    if s < 4.0 {
        "just now".into()
    } else if s < 60.0 {
        format!("{} s ago", s as i64)
    } else if s < 3600.0 {
        format!("{} m ago", (s / 60.0) as i64)
    } else {
        format!("{} h ago", (s / 3600.0) as i64)
    }
}

/// Seconds of the local day (any offset) as `HH:MM`.
pub fn fmt_hhmm(local_secs: i64) -> String {
    let s = local_secs.rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

/// Time until something: `in 2h 14m`, `in 15 min`, `now`.
pub fn fmt_in(secs: i64) -> String {
    let mins = (secs + 30) / 60;
    if mins < 1 {
        "now".into()
    } else if mins < 60 {
        format!("in {mins} min")
    } else {
        format!("in {}h {:02}m", mins / 60, mins % 60)
    }
}

/// A length of time as `8h 56m`.
pub fn fmt_hm(secs: i64) -> String {
    let mins = secs.max(0) / 60;
    format!("{}h {:02}m", mins / 60, mins % 60)
}

/// A countdown as `mm:ss` under 100 minutes, `1h40` above.
pub fn fmt_mmss(secs: i64) -> String {
    let s = secs.max(0);
    if s < 100 * 60 {
        format!("{:02}:{:02}", s / 60, s % 60)
    } else {
        format!("{}h{:02}", s / 3600, (s % 3600) / 60)
    }
}

/// `("1.2 of 3.1 TB", "1.9 TB free")`, in GB when the capacity is under a TiB.
pub fn fmt_capacity(used: u64, cap: u64) -> (String, String) {
    const TB: f64 = 1_099_511_627_776.0;
    const GB: f64 = 1_073_741_824.0;
    let free = cap.saturating_sub(used);
    if cap as f64 >= TB {
        (
            format!("{:.1} of {:.1} TB", used as f64 / TB, cap as f64 / TB),
            format!("{:.1} TB free", free as f64 / TB),
        )
    } else {
        (
            format!("{:.0} of {:.0} GB", used as f64 / GB, cap as f64 / GB),
            format!("{:.0} GB free", free as f64 / GB),
        )
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
    #[test]
    fn p2_formatters() {
        assert_eq!(fmt_ago(2.0), "just now");
        assert_eq!(fmt_ago(42.0), "42 s ago");
        assert_eq!(fmt_ago(150.0), "2 m ago");
        assert_eq!(fmt_ago(7300.0), "2 h ago");
        assert_eq!(fmt_hhmm(16 * 3600 + 48 * 60), "16:48");
        assert_eq!(fmt_hhmm(86_400 + 5 * 60), "00:05", "wraps the day");
        assert_eq!(fmt_in(2 * 3600 + 14 * 60), "in 2h 14m");
        assert_eq!(fmt_in(14 * 60 + 30), "in 15 min");
        assert_eq!(fmt_in(20), "now");
        assert_eq!(fmt_hm(8 * 3600 + 56 * 60), "8h 56m");
        assert_eq!(fmt_mmss(14 * 60 + 32), "14:32");
        assert_eq!(fmt_mmss(5), "00:05");
        assert_eq!(fmt_mmss(100 * 60), "1h40");
        let tb = 1u64 << 40;
        assert_eq!(
            fmt_capacity(tb + tb / 5, 3 * tb + tb / 10),
            ("1.2 of 3.1 TB".to_string(), "1.9 TB free".to_string())
        );
        let gb = 1u64 << 30;
        assert_eq!(
            fmt_capacity(49 * gb, 128 * gb),
            ("49 of 128 GB".to_string(), "79 GB free".to_string())
        );
        assert_eq!(fmt_capacity(0, 0).0, "0 of 0 GB");
    }
}

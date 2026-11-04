//! The Bambu Lab printer and the Home Assistant entity screens.

use crate::anim::Secs;
use crate::event::{HaEntity, HaStatus, PrintStage, PrinterReport, PrinterStatus};
use crate::format::{fmt_ago, fmt_hhmm, fmt_left};
use crate::model::{HaState, Model};
use crate::p2::chart::{Card, CardLine};
use crate::p2::font::FontId;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{
    AMBER, BLUE, CRIT, CYAN, GREEN, OK, ORANGE, TEXT, TEXT2, TEXT3, VIOLET, WARN,
};
use crate::theme::Color;

fn line(text: String, font: FontId, px: f32, color: Color, dy: f32) -> CardLine {
    CardLine {
        text,
        font,
        px,
        color,
        dy,
    }
}

fn empty_tile(title: &str, pill: (String, Color)) -> P2Screen {
    let mut s = P2Screen::new(title, "—");
    s.hero_color = TEXT3;
    s.pill = Some(pill);
    s
}

/// Relative luminance, 0 (black) to 1 (white).
fn luminance(c: Color) -> f32 {
    (0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32) / 255.0
}

/// The ring colour for a print: the filament's own, lifted when it is too
/// dark to see on black; `BLUE` without one.
pub fn filament_ring(filament: Option<Color>) -> Color {
    match filament {
        Some(c) if luminance(c) < 0.25 => c.mix(TEXT, 0.45),
        Some(c) => Color { a: 255, ..c },
        None => BLUE,
    }
}

/// `Layer 120 / 300`, `220° · 60° · PLA`: the second and third card lines.
fn print_lines(r: &PrinterReport) -> Card {
    let job = if r.job.trim().is_empty() {
        "Print job".to_string()
    } else {
        r.job.trim().to_string()
    };
    let layers = match (r.layer, r.total_layers) {
        (Some(l), Some(t)) if t > 0 => format!("Layer {l} / {t}"),
        (Some(l), _) => format!("Layer {l}"),
        _ => "Layer —".into(),
    };
    let mut temps: Vec<String> = [r.nozzle_c, r.bed_c]
        .into_iter()
        .flatten()
        .map(|c| format!("{c:.0}°"))
        .collect();
    if let Some((_, kind)) = &r.filament {
        if !kind.trim().is_empty() {
            temps.push(kind.trim().to_string());
        }
    }
    Card::Lines(vec![
        line(job, FontId::Bold, 11.0, TEXT, 7.0),
        line(layers, FontId::Medium, 8.5, TEXT2, 21.0),
        line(temps.join(" · "), FontId::Medium, 8.5, TEXT3, 32.0),
    ])
}

/// The last job, for the idle and off screens.
fn last_job_card(model: &Model) -> Option<Card> {
    let (job, stage, at) = model.printer().last_job.as_ref()?;
    let how = match stage {
        PrintStage::Failed => "failed",
        PrintStage::Finished => "done",
        _ => "last print",
    };
    let ago = fmt_ago((model.unix_now() - at) as f64);
    Some(Card::Lines(vec![
        line(how.into(), FontId::Medium, 8.5, TEXT3, 4.0),
        line(
            format!("{job} · {ago}"),
            FontId::SemiBold,
            10.0,
            TEXT2,
            18.0,
        ),
    ]))
}

pub fn printer(model: &Model, _now: Secs) -> P2Screen {
    const TITLE: &str = "3D PRINTER";
    let p = model.printer();
    let report = match (p.status, &p.report) {
        (PrinterStatus::Unauthorized, _) => {
            return empty_tile(TITLE, ("Check access code".into(), WARN))
        }
        (PrinterStatus::Offline, _) | (PrinterStatus::Ok, None) => {
            let mut t = empty_tile(TITLE, ("Printer off".into(), TEXT2));
            t.card = last_job_card(model);
            return t;
        }
        (PrinterStatus::Ok, Some(r)) => r,
    };
    if report.stage == PrintStage::Idle {
        let mut t = empty_tile(TITLE, ("Idle".into(), TEXT2));
        t.card = last_job_card(model);
        return t;
    }
    let pct = report.percent.clamp(0.0, 100.0);
    let color = filament_ring(report.filament.as_ref().map(|(c, _)| *c));
    let mut t = P2Screen::new(TITLE, format!("{pct:.0}"));
    t.unit = Some("%".into());
    t.ring = RingSpec::fill(pct / 100.0, color);
    let left = || match report.remaining_mins {
        Some(m) => fmt_left(m as i64 * 60),
        None => "Printing".into(),
    };
    t.pill = Some(match report.stage {
        PrintStage::Preparing => ("Preparing".into(), TEXT2),
        PrintStage::Printing => (left(), TEXT2),
        PrintStage::Paused => {
            t.hero_color = TEXT2;
            t.ring = RingSpec::fill(pct / 100.0, color.with_alpha(0.45));
            ("Paused".into(), WARN)
        }
        PrintStage::Finished => {
            t.hero = "100".into();
            t.ring = RingSpec::fill(1.0, OK);
            let text = match p.ended_at {
                Some(at) => format!("Done · {}", fmt_ago((model.unix_now() - at) as f64)),
                None => "Done".into(),
            };
            (text, OK)
        }
        PrintStage::Failed => {
            t.hero_color = TEXT2;
            t.ring = RingSpec::fill(pct / 100.0, CRIT);
            ("Failed".into(), CRIT)
        }
        PrintStage::Idle => unreachable!("handled above"),
    });
    t.card = Some(print_lines(report));
    t
}

/// The Material Symbols icon for an entity, by device class then domain.
pub fn ha_icon(e: Option<&HaEntity>) -> &'static str {
    let Some(e) = e else { return "ms-home" };
    match e.device_class.as_deref() {
        Some("temperature") => return "ms-thermostat",
        Some("humidity") => return "ms-humidity_percentage",
        Some("moisture") => return "ms-water_drop",
        Some("power" | "energy" | "current" | "voltage" | "power_factor") => return "ms-bolt",
        Some("battery") => return "ms-battery_charging_full",
        Some("door" | "window" | "garage_door" | "opening") => return "ms-sensor_door",
        _ => {}
    }
    match e.domain.as_str() {
        "light" => "ms-lightbulb",
        "switch" | "input_boolean" | "fan" => "ms-toggle_on",
        "climate" => "ms-thermostat",
        _ => "ms-home",
    }
}

/// The colour of a numeric entity by device class.
fn ha_color(e: &HaEntity, v: f32) -> Color {
    match e.device_class.as_deref() {
        Some("temperature") => ORANGE,
        Some("humidity" | "moisture") => CYAN,
        Some("power" | "energy" | "current" | "voltage" | "illuminance") => AMBER,
        Some("battery") if v < 20.0 => CRIT,
        Some("battery") => GREEN,
        Some("carbon_dioxide" | "pm25" | "pm10" | "aqi") => VIOLET,
        _ => BLUE,
    }
}

/// `21.4`, `55`, `1234`, `0.42`: one decimal below 100, two below 1.
pub fn fmt_ha_number(v: f32) -> String {
    let a = v.abs();
    if a >= 100.0 || v.fract() == 0.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// The ring's range: the config's, else the unit's, else the sparkline's
/// own (padded), else none.
fn ha_range(
    slot_min: Option<f32>,
    slot_max: Option<f32>,
    e: &HaEntity,
    hist: &[f32],
) -> Option<(f32, f32)> {
    if let (Some(lo), Some(hi)) = (slot_min, slot_max) {
        return (hi > lo).then_some((lo, hi));
    }
    match (e.unit.as_deref(), e.device_class.as_deref()) {
        (Some("%"), _) => return Some((0.0, 100.0)),
        (Some("°C"), _) | (_, Some("temperature")) if e.unit.as_deref() != Some("°F") => {
            return Some((-10.0, 35.0))
        }
        (Some("°F"), _) => return Some((14.0, 95.0)),
        _ => {}
    }
    let lo = hist.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = hist.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if hi > lo {
        let pad = (hi - lo) * 0.1;
        Some((lo - pad, hi + pad))
    } else {
        None
    }
}

/// The sparkline's own range: its samples, padded, at least one unit tall
/// so sensor noise does not fill the card.
fn spark_range(series: &[f32]) -> (f32, f32) {
    let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mid = (lo + hi) / 2.0;
    let half = ((hi - lo) * 0.6).max(0.5);
    (mid - half, mid + half)
}

/// `on` to `On`, `not_home` to `Away`, a door's `on` to `Open`.
pub fn ha_state_word(e: &HaEntity) -> (String, Option<bool>) {
    let on = match e.state.as_str() {
        "on" | "open" | "unlocked" | "home" | "playing" | "heat" | "cool" | "heat_cool" => {
            Some(true)
        }
        "off" | "closed" | "locked" | "not_home" | "idle" | "paused" | "standby" => Some(false),
        _ => None,
    };
    let opening = matches!(
        e.device_class.as_deref(),
        Some("door" | "window" | "garage_door" | "opening")
    );
    let motion = matches!(
        e.device_class.as_deref(),
        Some("motion" | "occupancy" | "presence")
    );
    let word = match (e.state.as_str(), on) {
        (_, Some(true)) if opening => "Open".to_string(),
        (_, Some(false)) if opening => "Closed".to_string(),
        (_, Some(true)) if motion => "Motion".to_string(),
        (_, Some(false)) if motion => "Clear".to_string(),
        ("not_home", _) => "Away".to_string(),
        (s, _) => {
            let s = s.replace('_', " ");
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        }
    };
    (word.chars().take(12).collect(), on)
}

/// The unit as it sits by the hero: `°` for degrees, superscript when short,
/// below the hero when long.
fn ha_unit(unit: &str) -> (String, bool) {
    match unit {
        "°C" | "°F" => ("°".into(), false),
        u if u.chars().count() <= 2 => (u.into(), false),
        u => (u.into(), true),
    }
}

fn since_card(model: &Model, e: &HaEntity) -> Option<Card> {
    let at = e.last_changed?;
    let local = at + model.utc_offset_secs() as i64;
    Some(Card::Lines(vec![
        line("changed".into(), FontId::Medium, 8.5, TEXT3, 4.0),
        line(
            format!(
                "{} · {}",
                fmt_hhmm(local),
                fmt_ago((model.unix_now() - at) as f64)
            ),
            FontId::SemiBold,
            10.0,
            TEXT2,
            18.0,
        ),
    ]))
}

pub fn ha(model: &Model, index: u16, now: Secs) -> P2Screen {
    const TITLE: &str = "HOME";
    let Some(st) = model.ha(index) else {
        return empty_tile(TITLE, ("Check config".into(), WARN));
    };
    let mut t = ha_screen(model, st, now);
    t.icon = ha_icon(st.entity.as_ref());
    t
}

fn ha_screen(model: &Model, st: &HaState, now: Secs) -> P2Screen {
    const TITLE: &str = "HOME";
    match st.status {
        HaStatus::Unauthorized => return empty_tile(TITLE, ("Check token".into(), WARN)),
        HaStatus::NotFound => return empty_tile(TITLE, ("Entity not found".into(), WARN)),
        HaStatus::Offline | HaStatus::Ok => {}
    }
    let Some(e) = &st.entity else {
        return empty_tile(TITLE, ("Home Assistant offline".into(), WARN));
    };
    let label = st
        .slot
        .label
        .clone()
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| e.name.clone());
    if st.status == HaStatus::Offline {
        return empty_tile(TITLE, ("Home Assistant offline".into(), WARN));
    }
    if matches!(e.state.as_str(), "unavailable" | "unknown" | "") {
        let mut t = empty_tile(TITLE, ("Unavailable".into(), WARN));
        t.card = Some(Card::Lines(vec![line(
            label,
            FontId::SemiBold,
            10.0,
            TEXT2,
            11.0,
        )]));
        return t;
    }
    if let Ok(v) = e.state.parse::<f32>() {
        let series = st.hist.series();
        let color = ha_color(e, v);
        let mut t = P2Screen::new(TITLE, fmt_ha_number(v));
        if let Some(u) = e.unit.as_deref().filter(|u| !u.trim().is_empty()) {
            let (u, below) = ha_unit(u.trim());
            t.unit = Some(u);
            t.unit_below = below;
        }
        let range = ha_range(st.slot.min, st.slot.max, e, &series);
        t.ring = match range {
            Some((lo, hi)) => RingSpec::fill(((v - lo) / (hi - lo)).clamp(0.0, 1.0), color),
            None => RingSpec::fill(1.0, color.with_alpha(0.45)),
        };
        t.pill = Some((label, TEXT2));
        t.card = if series.len() >= 2 {
            let (lo, hi) = spark_range(&series);
            Some(Card::Spark {
                series,
                prev: st.hist.prev(),
                scroll: st.hist.scroll(now),
                min: lo,
                max: hi,
                now_idx: None,
                color,
                caption: "last 30 min".into(),
            })
        } else {
            since_card(model, e)
        };
        return t;
    }
    let (word, on) = ha_state_word(e);
    let mut t = P2Screen::new(TITLE, word);
    let opening = matches!(
        e.device_class.as_deref(),
        Some("door" | "window" | "garage_door" | "opening")
    );
    match on {
        Some(true) => t.ring = RingSpec::fill(1.0, if opening { WARN } else { OK }),
        Some(false) => t.hero_color = TEXT2,
        None => {}
    }
    t.pill = Some((label, TEXT2));
    t.card = since_card(model, e);
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, HaEntity, HaStatus, PrinterReport, PrinterStatus};
    use crate::model::{HaSlot, Model, Thresholds};
    use crate::p2::ring::RingKind;

    fn report(stage: PrintStage, pct: f32) -> PrinterReport {
        PrinterReport {
            stage,
            percent: pct,
            remaining_mins: Some(65),
            layer: Some(120),
            total_layers: Some(300),
            nozzle_c: Some(219.6),
            bed_c: Some(60.0),
            job: "Benchy".into(),
            filament: Some((Color::hex(0xff6a13), "PLA".into())),
        }
    }

    fn printing(stage: PrintStage, pct: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_000);
        m.apply(
            Event::Printer {
                status: PrinterStatus::Ok,
                report: Some(report(stage, pct)),
            },
            1.0,
        );
        m
    }

    fn fill(s: &P2Screen) -> (f32, Color) {
        match &s.ring.kind {
            RingKind::Fill { frac, color } => (*frac, *color),
            k => panic!("not a fill: {k:?}"),
        }
    }

    #[test]
    fn printing_shows_percent_time_left_and_the_job() {
        let s = printer(&printing(PrintStage::Printing, 42.4), 1.0);
        assert_eq!(s.hero, "42");
        assert_eq!(s.unit.as_deref(), Some("%"));
        assert_eq!(s.pill, Some(("1h 05m left".into(), TEXT2)));
        let (f, c) = fill(&s);
        assert!((f - 0.424).abs() < 1e-4);
        assert_eq!(c, Color::hex(0xff6a13), "the filament colour");
        let Some(Card::Lines(lines)) = &s.card else {
            panic!("lines")
        };
        assert_eq!(lines[0].text, "Benchy");
        assert_eq!(lines[1].text, "Layer 120 / 300");
        assert_eq!(lines[2].text, "220° · 60° · PLA");
    }

    #[test]
    fn dark_filament_is_lifted_off_the_black() {
        let c = filament_ring(Some(Color::hex(0x101010)));
        assert!(luminance(c) > 0.3, "{c:?}");
        assert_eq!(filament_ring(None), BLUE);
    }

    #[test]
    fn paused_finished_and_failed() {
        let s = printer(&printing(PrintStage::Paused, 50.0), 1.0);
        assert_eq!(s.pill, Some(("Paused".into(), WARN)));
        assert_eq!(s.hero_color, TEXT2);

        let mut m = printing(PrintStage::Printing, 99.0);
        m.set_unix_now(1_600);
        m.apply(
            Event::Printer {
                status: PrinterStatus::Ok,
                report: Some(report(PrintStage::Finished, 100.0)),
            },
            2.0,
        );
        m.set_unix_now(1_600 + 180);
        let s = printer(&m, 2.0);
        assert_eq!(s.hero, "100");
        assert_eq!(s.pill, Some(("Done · 3 m ago".into(), OK)));
        assert_eq!(fill(&s), (1.0, OK));

        let s = printer(&printing(PrintStage::Failed, 30.0), 1.0);
        assert_eq!(s.pill, Some(("Failed".into(), CRIT)));
        assert_eq!(fill(&s).1, CRIT);
    }

    #[test]
    fn finished_at_the_first_report_has_no_age() {
        let s = printer(&printing(PrintStage::Finished, 100.0), 1.0);
        assert_eq!(s.pill, Some(("Done".into(), OK)));
    }

    #[test]
    fn idle_and_off_remember_the_last_job() {
        let mut m = printing(PrintStage::Printing, 10.0);
        m.set_unix_now(1_000 + 7_200);
        m.apply(
            Event::Printer {
                status: PrinterStatus::Offline,
                report: None,
            },
            2.0,
        );
        let s = printer(&m, 2.0);
        assert_eq!(s.pill, Some(("Printer off".into(), TEXT2)));
        let Some(Card::Lines(lines)) = &s.card else {
            panic!("lines")
        };
        assert_eq!(lines[1].text, "Benchy · 2 h ago");

        let mut idle = Model::new(Thresholds::default());
        idle.apply(
            Event::Printer {
                status: PrinterStatus::Ok,
                report: Some(report(PrintStage::Idle, 0.0)),
            },
            1.0,
        );
        let s = printer(&idle, 1.0);
        assert_eq!(s.pill, Some(("Idle".into(), TEXT2)));
        assert!(s.card.is_none(), "no job seen yet");

        let mut bad = Model::new(Thresholds::default());
        bad.apply(
            Event::Printer {
                status: PrinterStatus::Unauthorized,
                report: None,
            },
            1.0,
        );
        assert_eq!(
            printer(&bad, 1.0).pill,
            Some(("Check access code".into(), WARN))
        );
    }

    fn entity(state: &str, unit: Option<&str>, class: Option<&str>, domain: &str) -> HaEntity {
        HaEntity {
            state: state.into(),
            name: "Living room".into(),
            unit: unit.map(Into::into),
            device_class: class.map(Into::into),
            domain: domain.into(),
            last_changed: Some(3_600),
        }
    }

    fn ha_model(slot: HaSlot, polls: &[(Secs, HaEntity)]) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3_600 + 120);
        m.set_ha_slots(vec![slot]);
        for (t, e) in polls {
            m.apply(
                Event::HomeAssistant {
                    index: 0,
                    status: HaStatus::Ok,
                    entity: Some(e.clone()),
                },
                *t,
            );
        }
        m
    }

    #[test]
    fn a_temperature_gets_degrees_its_range_and_a_sparkline() {
        let e = |v: &str| entity(v, Some("°C"), Some("temperature"), "sensor");
        let m = ha_model(
            HaSlot::default(),
            &[(0.0, e("20.0")), (30.0, e("20.5")), (60.0, e("21.4"))],
        );
        let s = ha(&m, 0, 60.0);
        assert_eq!(s.hero, "21.4");
        assert_eq!(s.unit.as_deref(), Some("°"));
        assert!(!s.unit_below);
        assert_eq!(s.icon, "ms-thermostat");
        assert_eq!(s.pill, Some(("Living room".into(), TEXT2)));
        let (f, c) = fill(&s);
        assert!((f - 31.4 / 45.0).abs() < 1e-3, "{f}");
        assert_eq!(c, ORANGE);
        match &s.card {
            Some(Card::Spark { series, .. }) => {
                assert_eq!(series, &vec![20.0, 21.4], "one sample a minute")
            }
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn the_config_range_and_label_win_and_long_units_sit_below() {
        let slot = HaSlot {
            label: Some("Solar".into()),
            min: Some(0.0),
            max: Some(4000.0),
        };
        let m = ha_model(
            slot,
            &[(0.0, entity("1234.5", Some("kWh"), Some("energy"), "sensor"))],
        );
        let s = ha(&m, 0, 0.0);
        assert_eq!(s.hero, "1234");
        assert_eq!(s.unit.as_deref(), Some("kWh"));
        assert!(s.unit_below);
        assert_eq!(s.pill, Some(("Solar".into(), TEXT2)));
        assert!((fill(&s).0 - 1234.5 / 4000.0).abs() < 1e-4);
        assert_eq!(s.icon, "ms-bolt");
        assert!(
            matches!(&s.card, Some(Card::Lines(_))),
            "one sample: since card"
        );
    }

    #[test]
    fn states_read_as_words() {
        let door = entity("on", None, Some("door"), "binary_sensor");
        let m = ha_model(HaSlot::default(), &[(0.0, door)]);
        let s = ha(&m, 0, 0.0);
        assert_eq!(s.hero, "Open");
        assert_eq!(fill(&s), (1.0, WARN));
        assert_eq!(s.icon, "ms-sensor_door");
        let Some(Card::Lines(lines)) = &s.card else {
            panic!("lines")
        };
        assert_eq!(lines[1].text, "01:00 · 2 m ago");

        let light = entity("off", None, None, "light");
        let s = ha(&ha_model(HaSlot::default(), &[(0.0, light)]), 0, 0.0);
        assert_eq!(s.hero, "Off");
        assert_eq!(s.hero_color, TEXT2);
        assert_eq!(s.icon, "ms-lightbulb");

        let away = entity("not_home", None, None, "person");
        assert_eq!(ha_state_word(&away).0, "Away");
        let mode = entity("heat_cool", None, None, "climate");
        assert_eq!(ha_state_word(&mode).0, "Heat cool");
    }

    #[test]
    fn unavailable_offline_and_errors() {
        let gone = entity("unavailable", None, None, "sensor");
        let s = ha(&ha_model(HaSlot::default(), &[(0.0, gone)]), 0, 0.0);
        assert_eq!(s.pill, Some(("Unavailable".into(), WARN)));

        let mut m = ha_model(
            HaSlot::default(),
            &[(0.0, entity("1", None, None, "sensor"))],
        );
        m.apply(
            Event::HomeAssistant {
                index: 0,
                status: HaStatus::Offline,
                entity: None,
            },
            5.0,
        );
        assert_eq!(
            ha(&m, 0, 5.0).pill,
            Some(("Home Assistant offline".into(), WARN))
        );
        assert_eq!(
            ha(&m, 0, 5.0).icon,
            "ms-home",
            "last entity kept for the icon"
        );
        for (status, pill) in [
            (HaStatus::Unauthorized, "Check token"),
            (HaStatus::NotFound, "Entity not found"),
        ] {
            m.apply(
                Event::HomeAssistant {
                    index: 0,
                    status,
                    entity: None,
                },
                6.0,
            );
            assert_eq!(ha(&m, 0, 6.0).pill, Some((pill.into(), WARN)));
        }
    }

    #[test]
    fn numbers_format_by_size() {
        assert_eq!(fmt_ha_number(21.43), "21.4");
        assert_eq!(fmt_ha_number(55.0), "55");
        assert_eq!(fmt_ha_number(123.6), "124");
        assert_eq!(fmt_ha_number(0.423), "0.42");
        assert_eq!(fmt_ha_number(-3.5), "-3.5");
    }
}

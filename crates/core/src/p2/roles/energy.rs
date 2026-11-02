//! Electricity roles with a hero: price and carbon (power-mix draws itself,
//! renewable lives in `system.rs` with the other fill rings).

use crate::anim::Secs;
use crate::electricity::{price_level, PriceLevel};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{TEXT2, TEXT3};
use crate::theme::Color;

/// The five price bands, cheapest first.
pub fn band_colors() -> Vec<Color> {
    PriceLevel::ALL.iter().map(|l| l.p2_color()).collect()
}

pub fn price(model: &Model, _now: Secs) -> P2Screen {
    let p = model.prices();
    let hourly: Vec<f32> = (0..24)
        .map(|h| {
            let slots: Vec<f32> = p
                .eur
                .iter()
                .skip(h * 4)
                .take(4)
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            if slots.is_empty() {
                f32::NAN
            } else {
                slots.iter().sum::<f32>() / slots.len() as f32
            }
        })
        .collect();
    let finite: Vec<f32> = p.eur.iter().copied().filter(|v| v.is_finite()).collect();
    let min = finite.iter().cloned().fold(f32::MAX, f32::min);
    let max = finite.iter().cloned().fold(f32::MIN, f32::max);
    let cur = model.current_price();
    let mut s = P2Screen::new(
        "ELECTRICITY",
        cur.map(|v| format!("{v:.3}")).unwrap_or_else(|| "—".into()),
    );
    s.hero_scale = 0.78;
    s.unit = Some("€/kWh".into());
    s.unit_below = true;
    if cur.is_none() {
        s.hero_color = TEXT3;
    }
    let frac = match cur {
        Some(v) if max > min => (v - min) / (max - min),
        _ => 0.5,
    };
    s.ring = RingSpec::bands(band_colors(), frac);
    s.pill = Some(match cur {
        Some(v) => {
            let (lvl, _) = price_level(v, p.avg);
            (lvl.name().into(), lvl.p2_color())
        }
        None => ("no price".into(), TEXT2),
    });
    let floor = if finite.is_empty() { 0.0 } else { min.min(0.0) };
    s.card = Some(Card::Bars {
        values: hourly
            .iter()
            .map(|v| if v.is_finite() { v - floor } else { 0.0 })
            .collect(),
        colors: hourly
            .iter()
            .map(|v| {
                if v.is_finite() {
                    price_level(*v, p.avg).0.p2_color()
                } else {
                    TEXT3
                }
            })
            .collect(),
        now: (model.local_slot() / 4) as usize,
        past_alpha: 0.28,
        caption: "today".into(),
    });
    s
}

/// Carbon intensity bands over 0..500 gCO₂/kWh, green to red.
pub const CARBON_BANDS: [u32; 5] = [0x45D6A4, 0x9BD07A, 0xE8C860, 0xF2A65A, 0xF0605D];

pub fn carbon(model: &Model, _now: Secs) -> P2Screen {
    let e = model.electricity();
    let g = e.carbon_gco2;
    let hist = model.carbon_hourly();
    let colors: Vec<Color> = CARBON_BANDS.iter().map(|c| Color::hex(*c)).collect();
    let band = ((g / 500.0).clamp(0.0, 0.999) * 5.0) as usize;
    let mut s = P2Screen::new("CARBON", format!("{g:.0}"));
    s.unit = Some("gCO₂/kWh".into());
    s.unit_below = true;
    s.ring = RingSpec::bands(colors.clone(), g / 500.0);
    s.pill = Some(match hist.mean() {
        Some(mean) if hist.len() >= 2 && g < mean * 0.95 => {
            ("Cleaner than usual".into(), Color::hex(0x7CCF8E))
        }
        Some(mean) if hist.len() >= 2 && g > mean * 1.05 => {
            ("Dirtier than usual".into(), Color::hex(0xF2A65A))
        }
        Some(_) if hist.len() >= 2 => ("About usual".into(), TEXT2),
        _ => (format!("{} grid", e.zone), TEXT2),
    });
    let series = hist.values();
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(f32::MIN, f32::max);
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: if lo.is_finite() { lo * 0.9 } else { 0.0 },
        max: if hi.is_finite() {
            (hi * 1.1).max(lo * 0.9 + 1.0)
        } else {
            500.0
        },
        color: colors[band],
        caption: "24 h".into(),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::electricity::{PriceLevel, Source};
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{TEXT2, TEXT3};

    fn price_model(at_now: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let mut eur: Vec<f32> = (0..96).map(|i| (6.2 + (i / 4) as f32) / 100.0).collect();
        eur[56] = at_now;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.18,
                currency: "EUR".into(),
            },
            0.0,
        );
        m
    }

    #[test]
    fn price_hero_bands_and_hourly_bars() {
        let m = price_model(0.221);
        let s = price(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.hero_scale,
                s.unit.as_deref(),
                s.unit_below
            ),
            ("ELECTRICITY", "0.221", 0.78, Some("€/kWh"), true)
        );
        let (lvl, _) = crate::electricity::price_level(0.221, 0.18);
        assert_eq!(s.pill, Some((lvl.name().into(), lvl.p2_color())));
        match &s.ring.kind {
            RingKind::Bands { colors, frac } => {
                assert_eq!(colors, &band_colors());
                // today runs 0.062 .. 0.292 (plus the 0.221 at 14:00)
                assert!(
                    (frac - (0.221 - 0.062) / (0.292 - 0.062)).abs() < 1e-3,
                    "{frac}"
                );
            }
            _ => panic!("bands"),
        }
        match &s.card {
            Some(Card::Bars {
                values,
                colors,
                now,
                caption,
                ..
            }) => {
                assert_eq!(
                    (values.len(), colors.len(), *now, caption.as_str()),
                    (24, 24, 14, "today")
                );
                assert!((values[0] - 0.062).abs() < 1e-4);
                assert_eq!(colors[0], PriceLevel::VeryCheap.p2_color());
            }
            _ => panic!("bars"),
        }
        let neg = price(&price_model(-0.006), 0.0);
        assert_eq!(neg.hero, "-0.006");
        assert!(
            matches!(&neg.card, Some(Card::Bars { values, .. }) if values.iter().all(|v| *v >= 0.0)),
            "bars shift up for negative prices"
        );
        let mut hole = Model::new(Thresholds::default());
        hole.set_local_slot(56);
        hole.apply(
            Event::Prices {
                date: "x".into(),
                eur_per_kwh: vec![f32::NAN; 96],
                avg_eur_per_kwh: 0.2,
                currency: "EUR".into(),
            },
            0.0,
        );
        let s = price(&hole, 0.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert_eq!(s.pill, Some(("no price".into(), TEXT2)));
    }

    fn carbon_model(samples: &[(i64, f32)]) -> Model {
        let mut m = Model::new(Thresholds::default());
        for (hour, g) in samples {
            m.set_unix_now(hour * 3600);
            m.apply(
                Event::Electricity {
                    zone: "NL".into(),
                    mix_mw: vec![(Source::Gas, 100.0)],
                    renewable_pct: 10.0,
                    fossil_free_pct: 10.0,
                    carbon_gco2: *g,
                    updated_at: String::new(),
                },
                0.0,
            );
        }
        m
    }

    #[test]
    fn carbon_compares_with_the_day_and_sparks_24_hours() {
        let m = carbon_model(&[(1, 300.0), (2, 280.0), (3, 200.0)]);
        let s = carbon(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("CARBON", "200", Some("gCO₂/kWh"), true)
        );
        assert!(
            matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors.len() == 5 && (frac - 0.4).abs() < 1e-5)
        );
        assert_eq!(s.pill.as_ref().unwrap().0, "Cleaner than usual");
        assert!(
            matches!(&s.card, Some(Card::Spark { series, now_idx, caption, .. }) if series == &vec![300.0, 280.0, 200.0] && *now_idx == Some(2) && caption == "24 h")
        );
        assert_eq!(
            carbon(&carbon_model(&[(1, 200.0), (2, 300.0)]), 0.0)
                .pill
                .unwrap()
                .0,
            "Dirtier than usual"
        );
        assert_eq!(
            carbon(&carbon_model(&[(1, 250.0), (2, 255.0)]), 0.0)
                .pill
                .unwrap()
                .0,
            "About usual"
        );
        assert_eq!(
            carbon(&carbon_model(&[(1, 250.0)]), 0.0).pill,
            Some(("NL grid".into(), TEXT2))
        );
    }
}

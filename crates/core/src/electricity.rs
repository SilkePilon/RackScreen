//! Electricity Maps sources, colours, icons and the power-mix ring partition.

use crate::theme::{Color, AMBER, GREEN, RED};

/// The five bands of the price level gauge, cheapest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceLevel {
    VeryCheap,
    Cheap,
    Normal,
    Pricey,
    VeryPricey,
}

impl PriceLevel {
    pub const ALL: [PriceLevel; 5] = [
        PriceLevel::VeryCheap,
        PriceLevel::Cheap,
        PriceLevel::Normal,
        PriceLevel::Pricey,
        PriceLevel::VeryPricey,
    ];

    /// The badge word.
    pub fn word(self) -> &'static str {
        match self {
            PriceLevel::VeryCheap => "V.CHEAP",
            PriceLevel::Cheap => "CHEAP",
            PriceLevel::Normal => "NORMAL",
            PriceLevel::Pricey => "PRICEY",
            PriceLevel::VeryPricey => "V.PRICEY",
        }
    }

    /// The band colour, green through amber to red.
    pub fn color(self) -> Color {
        match self {
            PriceLevel::VeryCheap => GREEN,
            PriceLevel::Cheap => GREEN.mix(AMBER, 0.5),
            PriceLevel::Normal => AMBER,
            PriceLevel::Pricey => AMBER.mix(RED, 0.5),
            PriceLevel::VeryPricey => RED,
        }
    }

    /// 0 for the cheapest band, 4 for the dearest.
    pub fn index(self) -> usize {
        PriceLevel::ALL
            .iter()
            .position(|l| *l == self)
            .expect("every level is in ALL")
    }
}

/// Band edges as `price / average` ratios (Tibber's cut-offs), with the outer
/// clamps: below 0.60 is very cheap, above 1.40 very pricey.
pub const LEVEL_EDGES: [f32; 6] = [0.30, 0.60, 0.90, 1.15, 1.40, 1.70];

/// Where `price` sits against `avg`: the band, and the needle position in
/// `0..=1` that is linear inside each fifth of the arc. Ratios past the outer
/// edges clamp; an average that is not positive (or not finite), or a price
/// that is not finite, reads as `Normal` in the middle of the arc.
pub fn price_level(price: f32, avg: f32) -> (PriceLevel, f32) {
    if !avg.is_finite() || avg <= 0.0 || !price.is_finite() {
        return (PriceLevel::Normal, 0.5);
    }
    let ratio = price / avg;
    let band = LEVEL_EDGES[1..5]
        .iter()
        .position(|edge| ratio < *edge)
        .unwrap_or(4);
    let (lo, hi) = (LEVEL_EDGES[band], LEVEL_EDGES[band + 1]);
    let inner = ((ratio - lo) / (hi - lo)).clamp(0.0, 1.0);
    (PriceLevel::ALL[band], (band as f32 + inner) / 5.0)
}

/// The Energy-Charts bidding zone an Electricity Maps zone maps to.
pub fn energy_charts_zone_for(zone: &str) -> Option<&'static str> {
    Some(match zone.trim().to_ascii_uppercase().as_str() {
        "NL" => "NL",
        "BE" => "BE",
        "FR" => "FR",
        "AT" => "AT",
        "CH" => "CH",
        "ES" => "ES",
        "PT" => "PT",
        "PL" => "PL",
        "FI" => "FI",
        "CZ" => "CZ",
        "DE" | "DE-LU" => "DE-LU",
        "DK-DK1" => "DK1",
        "DK-DK2" => "DK2",
        "NO-NO1" => "NO1",
        "NO-NO2" => "NO2",
        "NO-NO3" => "NO3",
        "NO-NO4" => "NO4",
        "NO-NO5" => "NO5",
        "SE-SE1" => "SE1",
        "SE-SE2" => "SE2",
        "SE-SE3" => "SE3",
        "SE-SE4" => "SE4",
        "IT-NO" => "IT-North",
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Nuclear,
    Geothermal,
    Biomass,
    Coal,
    Wind,
    Solar,
    Hydro,
    Gas,
    Oil,
    Unknown,
    HydroStorage,
    BatteryStorage,
}

impl Source {
    pub const ALL: [Source; 12] = [
        Source::Nuclear,
        Source::Geothermal,
        Source::Biomass,
        Source::Coal,
        Source::Wind,
        Source::Solar,
        Source::Hydro,
        Source::Gas,
        Source::Oil,
        Source::Unknown,
        Source::HydroStorage,
        Source::BatteryStorage,
    ];

    /// The key Electricity Maps uses in `powerProductionBreakdown`.
    pub fn from_api_key(k: &str) -> Option<Source> {
        Some(match k {
            "nuclear" => Source::Nuclear,
            "geothermal" => Source::Geothermal,
            "biomass" => Source::Biomass,
            "coal" => Source::Coal,
            "wind" => Source::Wind,
            "solar" => Source::Solar,
            "hydro" => Source::Hydro,
            "gas" => Source::Gas,
            "oil" => Source::Oil,
            "unknown" => Source::Unknown,
            "hydro discharge" => Source::HydroStorage,
            "battery discharge" => Source::BatteryStorage,
            _ => return None,
        })
    }

    pub fn color(self) -> Color {
        Color::hex(match self {
            Source::Biomass => 0x008043,
            Source::Geothermal => 0xA73C15,
            Source::Hydro => 0x1878EA,
            Source::Solar => 0xFFC700,
            Source::Wind => 0x69D6F8,
            Source::Nuclear => 0x9D71F7,
            Source::BatteryStorage => 0x1DA484,
            Source::HydroStorage => 0x2B3CD8,
            Source::Coal => 0xac8c35,
            Source::Gas => 0xAAA189,
            Source::Oil => 0x584745,
            Source::Unknown => 0xACACAC,
        })
    }

    pub fn icon(self) -> &'static str {
        match self {
            Source::Biomass => "em-biomass",
            Source::Geothermal => "em-geothermal",
            Source::Hydro => "em-hydro",
            Source::Solar => "em-solar",
            Source::Wind => "em-wind",
            Source::Nuclear => "em-nuclear",
            Source::BatteryStorage => "em-battery-storage",
            Source::HydroStorage => "em-hydro-storage",
            Source::Coal => "em-coal",
            Source::Gas => "em-gas",
            Source::Oil => "em-oil",
            Source::Unknown => "em-unknown",
        }
    }

    /// Position in `ALL`; the tie-break for equal shares.
    pub fn index(self) -> usize {
        Source::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// A section shorter than this gets no icon: there is no room for one.
pub const MIX_ICON_MIN_SEGS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub source: Source,
    /// First segment index (0 = 12 o'clock).
    pub start: usize,
    /// Lit segments, gap excluded.
    pub len: usize,
}

/// Split `n` segments by share. Input is (source, share) with shares > 0, any order.
/// Output is sorted by share descending; each source gets at least one lit segment;
/// between two sources one segment stays unlit (taken from the larger neighbour).
pub fn partition(shares: &[(Source, f32)], n: usize) -> Vec<Section> {
    let mut items: Vec<(Source, f32)> = shares.iter().copied().filter(|(_, s)| *s > 0.0).collect();
    if items.is_empty() || n == 0 {
        return Vec::new();
    }
    items.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| a.0.index().cmp(&b.0.index()))
    });
    let count = items.len().min(n);
    items.truncate(count);
    let total: f32 = items.iter().map(|(_, s)| s).sum();
    let gaps = if count > 1 { count } else { 0 };
    let usable = n.saturating_sub(gaps).max(count);
    // largest remainder apportionment with a floor of 1
    let mut alloc: Vec<usize> = items
        .iter()
        .map(|(_, s)| ((s / total) * usable as f32).floor() as usize)
        .collect();
    for a in alloc.iter_mut() {
        if *a == 0 {
            *a = 1;
        }
    }
    let mut used: usize = alloc.iter().sum();
    while used > usable {
        let idx = alloc
            .iter()
            .enumerate()
            .max_by_key(|(_, a)| **a)
            .map(|(i, _)| i)
            .expect("alloc is not empty");
        alloc[idx] -= 1;
        used -= 1;
    }
    let mut remainders: Vec<(usize, f32)> = items
        .iter()
        .enumerate()
        .map(|(i, (_, s))| (i, (s / total) * usable as f32 - alloc[i] as f32))
        .collect();
    remainders.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut i = 0;
    while used < usable {
        let idx = remainders[i % remainders.len()].0;
        alloc[idx] += 1;
        used += 1;
        i += 1;
    }
    let mut out = Vec::with_capacity(count);
    let mut start = 0;
    for (k, (source, _)) in items.iter().enumerate() {
        out.push(Section {
            source: *source,
            start,
            len: alloc[k],
        });
        start += alloc[k] + if gaps > 0 { 1 } else { 0 };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keys_and_palette() {
        assert_eq!(
            Source::from_api_key("hydro discharge"),
            Some(Source::HydroStorage)
        );
        assert_eq!(
            Source::from_api_key("battery discharge"),
            Some(Source::BatteryStorage)
        );
        assert_eq!(Source::from_api_key("fusion"), None);
        assert_eq!(Source::Solar.color(), Color::hex(0xFFC700));
        assert_eq!(Source::Coal.icon(), "em-coal");
        for s in Source::ALL {
            assert_eq!(Source::ALL[s.index()], s);
        }
    }

    #[test]
    fn partition_sorts_gaps_and_floors() {
        let p = partition(
            &[
                (Source::Wind, 24.0),
                (Source::Solar, 48.0),
                (Source::Gas, 13.0),
                (Source::Hydro, 0.4),
                (Source::Nuclear, 0.0),
            ],
            60,
        );
        assert_eq!(p.len(), 4, "zero share absent");
        assert_eq!(p[0].source, Source::Solar);
        assert_eq!(p[0].start, 0);
        let lit: usize = p.iter().map(|s| s.len).sum();
        assert_eq!(lit + 4, 60, "one gap per section, including after the last");
        assert!(p.iter().all(|s| s.len >= 1));
        for w in p.windows(2) {
            assert_eq!(w[1].start, w[0].start + w[0].len + 1);
        }
        assert!(p[0].len > p[1].len && p[1].len > p[2].len);
    }

    #[test]
    fn partition_single_and_empty() {
        let p = partition(&[(Source::Solar, 1.0)], 60);
        assert_eq!(
            p,
            vec![Section {
                source: Source::Solar,
                start: 0,
                len: 60
            }]
        );
        assert!(partition(&[], 60).is_empty());
        assert!(partition(&[(Source::Solar, 0.0)], 60).is_empty());
    }

    #[test]
    fn partition_fits_the_ring_with_every_source() {
        let many: Vec<(Source, f32)> = Source::ALL.iter().map(|s| (*s, 1.0)).collect();
        let p = partition(&many, 60);
        assert_eq!(p.len(), 12);
        let end = p.last().map(|s| s.start + s.len).unwrap_or(0);
        assert!(end <= 60, "last section ends at {end}");
        assert!(p.iter().all(|s| s.len >= 1));
        assert!(partition(&many, 0).is_empty());
    }

    #[test]
    fn price_level_bands_and_needle() {
        let avg = 0.20;
        let at = |ratio: f32| price_level(ratio * avg, avg);
        // band edges land on the fifths of the arc
        assert_eq!(at(0.60), (PriceLevel::Cheap, 0.2));
        assert_eq!(at(0.90), (PriceLevel::Normal, 0.4));
        assert_eq!(at(1.15), (PriceLevel::Pricey, 0.6));
        assert_eq!(at(1.40), (PriceLevel::VeryPricey, 0.8));
        // linear inside a band: the average sits 0.1 into the 0.25-wide NORMAL band
        let (lvl, t) = at(1.0);
        assert_eq!(lvl, PriceLevel::Normal);
        assert!((t - 0.48).abs() < 1e-5, "{t}");
        // clamped at both ends
        assert_eq!(at(0.30), (PriceLevel::VeryCheap, 0.0));
        assert_eq!(at(0.05), (PriceLevel::VeryCheap, 0.0));
        assert_eq!(at(1.70), (PriceLevel::VeryPricey, 1.0));
        assert_eq!(at(3.00), (PriceLevel::VeryPricey, 1.0));
        // a negative price against a positive average is as cheap as it gets
        assert_eq!(price_level(-0.006, avg), (PriceLevel::VeryCheap, 0.0));
        // no usable average: normal, mid-arc
        assert_eq!(price_level(0.1, 0.0), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(0.1, -0.2), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(0.1, f32::NAN), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(0.1, f32::INFINITY), (PriceLevel::Normal, 0.5));
        assert_eq!(
            price_level(0.1, f32::NEG_INFINITY),
            (PriceLevel::Normal, 0.5)
        );
        assert_eq!(price_level(f32::NAN, avg), (PriceLevel::Normal, 0.5));
    }

    #[test]
    fn price_level_words_colours_and_order() {
        let words: Vec<&str> = PriceLevel::ALL.iter().map(|l| l.word()).collect();
        assert_eq!(words, ["V.CHEAP", "CHEAP", "NORMAL", "PRICEY", "V.PRICEY"]);
        for (i, l) in PriceLevel::ALL.iter().enumerate() {
            assert_eq!(l.index(), i);
        }
        assert_eq!(PriceLevel::VeryCheap.color(), GREEN);
        assert_eq!(PriceLevel::Normal.color(), AMBER);
        assert_eq!(PriceLevel::VeryPricey.color(), RED);
        assert_eq!(PriceLevel::Cheap.color(), GREEN.mix(AMBER, 0.5));
        assert_eq!(PriceLevel::Pricey.color(), AMBER.mix(RED, 0.5));
    }

    #[test]
    fn energy_charts_zones() {
        assert_eq!(energy_charts_zone_for("nl"), Some("NL"));
        assert_eq!(energy_charts_zone_for("DE"), Some("DE-LU"));
        assert_eq!(energy_charts_zone_for("DE-LU"), Some("DE-LU"));
        assert_eq!(energy_charts_zone_for("DK-DK1"), Some("DK1"));
        assert_eq!(energy_charts_zone_for("NO-NO5"), Some("NO5"));
        assert_eq!(energy_charts_zone_for("SE-SE3"), Some("SE3"));
        assert_eq!(energy_charts_zone_for("IT-NO"), Some("IT-North"));
        for z in ["BE", "FR", "AT", "CH", "ES", "PT", "PL", "FI", "CZ"] {
            assert_eq!(energy_charts_zone_for(z), Some(z), "{z}");
        }
        assert_eq!(energy_charts_zone_for("XX"), None);
        assert_eq!(energy_charts_zone_for(""), None);
    }
}

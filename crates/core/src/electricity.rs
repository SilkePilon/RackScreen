//! Electricity Maps sources, colours, icons and the power-mix ring partition.

use crate::theme::Color;

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
    /// The P2 pill text.
    pub fn name(self) -> &'static str {
        match self {
            PriceLevel::VeryCheap => "Very cheap",
            PriceLevel::Cheap => "Cheap",
            PriceLevel::Normal => "Normal",
            PriceLevel::Pricey => "Pricey",
            PriceLevel::VeryPricey => "Very pricey",
        }
    }

    /// The P2 band colour, green through yellow to red.
    pub fn p2_color(self) -> Color {
        Color::hex(match self {
            PriceLevel::VeryCheap => 0x45D6A4,
            PriceLevel::Cheap => 0x7CCF8E,
            PriceLevel::Normal => 0xD9CB6C,
            PriceLevel::Pricey => 0xF2A65A,
            PriceLevel::VeryPricey => 0xF0605D,
        })
    }

    pub const ALL: [PriceLevel; 5] = [
        PriceLevel::VeryCheap,
        PriceLevel::Cheap,
        PriceLevel::Normal,
        PriceLevel::Pricey,
        PriceLevel::VeryPricey,
    ];

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
        assert_eq!(Source::Coal.icon(), "em-coal");
        for s in Source::ALL {
            assert_eq!(Source::ALL[s.index()], s);
        }
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
    fn price_level_order() {
        for (i, l) in PriceLevel::ALL.iter().enumerate() {
            assert_eq!(l.index(), i);
        }
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
    #[test]
    fn p2_level_names_and_colours() {
        assert_eq!(PriceLevel::VeryCheap.name(), "Very cheap");
        assert_eq!(PriceLevel::VeryPricey.name(), "Very pricey");
        assert_eq!(PriceLevel::Normal.p2_color(), Color::hex(0xD9CB6C));
        let colours: std::collections::HashSet<u32> = PriceLevel::ALL
            .iter()
            .map(|l| {
                let c = l.p2_color();
                ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32
            })
            .collect();
        assert_eq!(colours.len(), 5);
    }
}

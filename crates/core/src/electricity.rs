//! Electricity Maps production sources and their icons.

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
}

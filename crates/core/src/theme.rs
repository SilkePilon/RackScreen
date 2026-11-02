//! Colours, screen roles and fixed layout constants (all in 240x240 px space).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn hex(v: u32) -> Self {
        Self::rgb(
            ((v >> 16) & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            (v & 0xff) as u8,
        )
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }
    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self {
            r: l(self.r, other.r),
            g: l(self.g, other.g),
            b: l(self.b, other.b),
            a: l(self.a, other.a),
        }
    }
}

pub const WHITE: Color = Color::hex(0xffffff);
pub const BLACK: Color = Color::hex(0x000000);

/// European Air Quality Index band, 0 (good) to 5 (extremely poor).
pub fn eaqi_band(v: f32) -> usize {
    match v {
        v if v < 20.0 => 0,
        v if v < 40.0 => 1,
        v if v < 60.0 => 2,
        v if v < 80.0 => 3,
        v if v < 100.0 => 4,
        _ => 5,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Cpu,
    Mem,
    Pods,
    Health,
    Thermal,
    Storage,
    PowerMix,
    Price,
    Carbon,
    Renewable,
    GhActivity,
    Weather,
    Wind,
    Aqi,
    Rain,
    Sun,
    Moon,
    Iss,
    Ups,
    Net,
    Deploys,
    Face,
}

impl Role {
    pub const ALL: [Role; 22] = [
        Role::Cpu,
        Role::Mem,
        Role::Pods,
        Role::Health,
        Role::Thermal,
        Role::Storage,
        Role::PowerMix,
        Role::Price,
        Role::Carbon,
        Role::Renewable,
        Role::GhActivity,
        Role::Weather,
        Role::Wind,
        Role::Aqi,
        Role::Rain,
        Role::Sun,
        Role::Moon,
        Role::Iss,
        Role::Ups,
        Role::Net,
        Role::Deploys,
        Role::Face,
    ];

    pub fn index(self) -> usize {
        Role::ALL
            .iter()
            .position(|r| *r == self)
            .expect("role in ALL")
    }
    pub fn from_index(i: usize) -> Option<Role> {
        Role::ALL.get(i).copied()
    }
    /// Config identifier.
    /// The curved title in the P2 look (upper case); `weather` and `sun` show
    /// a live title instead and use this one only while waiting for data.
    pub fn title(self) -> &'static str {
        match self {
            Role::Cpu => "CPU",
            Role::Mem => "MEMORY",
            Role::Pods => "PODS",
            Role::Health => "CLUSTER",
            Role::Thermal => "THERMAL",
            Role::Storage => "STORAGE",
            Role::PowerMix => "POWER MIX",
            Role::Price => "ELECTRICITY",
            Role::Carbon => "CARBON",
            Role::Renewable => "RENEWABLE",
            Role::GhActivity => "CONTRIBUTIONS",
            Role::Weather => "WEATHER",
            Role::Wind => "WIND",
            Role::Aqi => "AIR QUALITY",
            Role::Rain => "RAIN",
            Role::Sun => "SUN",
            Role::Moon => "MOON",
            Role::Iss => "ISS PASS IN",
            Role::Ups => "UPS",
            Role::Net => "NETWORK",
            Role::Deploys => "DEPLOYS TODAY",
            Role::Face => "FACE",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "mem",
            Role::Pods => "pods",
            Role::Health => "health",
            Role::Thermal => "thermal",
            Role::Storage => "storage",
            Role::PowerMix => "power-mix",
            Role::Price => "price",
            Role::Carbon => "carbon",
            Role::Renewable => "renewable",
            Role::GhActivity => "gh-activity",
            Role::Weather => "weather",
            Role::Wind => "wind",
            Role::Aqi => "aqi",
            Role::Rain => "rain",
            Role::Sun => "sun",
            Role::Moon => "moon",
            Role::Iss => "iss",
            Role::Ups => "ups",
            Role::Net => "net",
            Role::Deploys => "deploys",
            Role::Face => "face",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|r| r.name() == s)
    }
    /// Cluster roles need the Kubernetes API link; the electricity, sky and
    /// GitHub roles are fed by public APIs and work without a cluster.
    pub fn is_cluster(self) -> bool {
        matches!(
            self,
            Role::Cpu
                | Role::Mem
                | Role::Pods
                | Role::Health
                | Role::Thermal
                | Role::Storage
                | Role::Ups
                | Role::Net
                | Role::Deploys
                | Role::Face
        )
    }
    /// Roles that need `location.lat` / `location.lon` in the config.
    pub fn is_sky(self) -> bool {
        matches!(
            self,
            Role::Weather
                | Role::Wind
                | Role::Aqi
                | Role::Rain
                | Role::Sun
                | Role::Moon
                | Role::Iss
        )
    }
}

/// The P2 "Tiles" palette.
pub mod p2 {
    use super::Color;
    pub const BG: Color = Color::hex(0x000000);
    pub const TEXT: Color = Color::hex(0xEDEDEF);
    pub const TEXT2: Color = Color::hex(0x8E8E93);
    pub const TEXT3: Color = Color::hex(0x4A4A4F);
    pub const TRACK: Color = Color::hex(0x17171A);
    pub const CARD: Color = Color::hex(0x121214);
    pub const OK: Color = Color::hex(0x5BD18B);
    pub const WARN: Color = Color::hex(0xF2B84B);
    pub const CRIT: Color = Color::hex(0xF0605D);
    pub const BLUE: Color = Color::hex(0x6EA8FE);
    pub const VIOLET: Color = Color::hex(0xA48BFA);
    pub const TEAL: Color = Color::hex(0x5CC8C3);
    pub const INDIGO: Color = Color::hex(0x8C9CFF);
    pub const CYAN: Color = Color::hex(0x67D1EE);
    pub const AMBER: Color = Color::hex(0xF2B84B);
    pub const GREEN: Color = Color::hex(0x4CC38A);
    /// The dark half of the sun ring.
    pub const NIGHT: Color = Color::hex(0x2A2A33);
    pub const MOON: Color = Color::hex(0xD6D6E0);
    /// Capsule icon and text on the power-mix ring.
    pub const INK: Color = Color::hex(0x0B0B0C);
}

pub mod layout {
    pub const SIZE: u32 = 240;
    pub const CX: f32 = 120.0;
    pub const CY: f32 = 120.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_channels() {
        assert_eq!(
            Color::hex(0xffb020),
            Color {
                r: 0xff,
                g: 0xb0,
                b: 0x20,
                a: 255
            }
        );
    }

    #[test]
    fn mix_midpoint() {
        let c = BLACK.mix(WHITE, 0.5);
        assert_eq!((c.r, c.g, c.b), (128, 128, 128));
    }

    #[test]
    fn role_index_roundtrip() {
        for r in Role::ALL {
            assert_eq!(Role::from_index(r.index()), Some(r));
        }
    }

    #[test]
    fn cluster_roles_are_the_kubernetes_ones() {
        let cluster: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_cluster()).collect();
        assert_eq!(
            cluster,
            vec![
                Role::Cpu,
                Role::Mem,
                Role::Pods,
                Role::Health,
                Role::Thermal,
                Role::Storage,
                Role::Ups,
                Role::Net,
                Role::Deploys,
                Role::Face,
            ]
        );
        assert!(!Role::PowerMix.is_cluster() && !Role::Price.is_cluster());
        assert!(!Role::Weather.is_cluster() && !Role::GhActivity.is_cluster());
    }

    #[test]
    fn twenty_two_roles_with_unique_names() {
        assert_eq!(Role::ALL.len(), 22);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 22);
        assert_eq!(Role::parse("face"), Some(Role::Face));
        assert!(Role::Face.is_cluster());
        assert!(!Role::Face.is_sky());
        assert_eq!(Role::ALL[21], Role::Face);
        assert_eq!(Role::parse("gh-activity"), Some(Role::GhActivity));
        assert_eq!(Role::parse("deploys"), Some(Role::Deploys));
        let sky: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_sky()).collect();
        assert_eq!(
            sky,
            vec![
                Role::Weather,
                Role::Wind,
                Role::Aqi,
                Role::Rain,
                Role::Sun,
                Role::Moon,
                Role::Iss
            ]
        );
    }

    #[test]
    fn air_quality_bands() {
        assert_eq!(eaqi_band(0.0), 0);
        assert_eq!(eaqi_band(19.9), 0);
        assert_eq!(eaqi_band(20.0), 1);
        assert_eq!(eaqi_band(45.0), 2);
        assert_eq!(eaqi_band(79.0), 3);
        assert_eq!(eaqi_band(99.0), 4);
        assert_eq!(eaqi_band(150.0), 5);
    }

    #[test]
    fn role_names_parse_roundtrip() {
        assert_eq!(Role::parse("power-mix"), Some(Role::PowerMix));
        assert_eq!(Role::parse("nope"), None);
        for r in Role::ALL {
            assert_eq!(Role::parse(r.name()), Some(r));
        }
    }
    #[test]
    fn p2_tokens_and_titles() {
        assert_eq!(p2::TEXT, Color::hex(0xEDEDEF));
        assert_eq!(p2::TRACK, Color::hex(0x17171A));
        assert_eq!(p2::WARN, p2::AMBER);
        assert_eq!(Role::Health.title(), "CLUSTER");
        assert_eq!(Role::Deploys.title(), "DEPLOYS TODAY");
        assert_eq!(Role::GhActivity.title(), "CONTRIBUTIONS");
        for r in Role::ALL {
            assert!(!r.title().is_empty());
            assert_eq!(r.title(), r.title().to_uppercase());
        }
    }
}

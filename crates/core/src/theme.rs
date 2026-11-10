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
    Media,
    Traffic,
    Restarts,
    Uptime,
    Fans,
    Mains,
    Load,
    Ci,
    Jellyfin,
    Music,
    /// A Bambu Lab 3D printer over its local MQTT report.
    Printer,
    /// Sen's live Earth view from the ISS. Shows only while the stream has
    /// live daylight footage; otherwise the screen skips it.
    Sen,
    /// `gif:<name>`: the GIF at this index of the config's `gifs:` list. Not
    /// in `ALL`; the config knows which exist and what they are called.
    Gif(u16),
    /// `ha:<name>`: the Home Assistant entity at this index of the config's
    /// `homeassistant.entities` list. Not in `ALL`, like `Gif`.
    Ha(u16),
}

impl Role {
    pub const ALL: [Role; 31] = [
        Role::Cpu,
        Role::Mem,
        Role::Pods,
        Role::Health,
        Role::Thermal,
        Role::Storage,
        Role::PowerMix,
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
        Role::Media,
        Role::Traffic,
        Role::Restarts,
        Role::Uptime,
        Role::Fans,
        Role::Mains,
        Role::Load,
        Role::Ci,
        Role::Jellyfin,
        Role::Music,
        Role::Printer,
        Role::Sen,
    ];

    /// Position in `ALL`; `Gif(i)` and `Ha(i)` count on after it, taking
    /// turns, so indices stay unique.
    pub fn index(self) -> usize {
        match self {
            Role::Gif(i) => return Role::ALL.len() + 2 * i as usize,
            Role::Ha(i) => return Role::ALL.len() + 2 * i as usize + 1,
            _ => {}
        }
        Role::ALL
            .iter()
            .position(|r| *r == self)
            .expect("role in ALL")
    }
    /// The built-in role at `i` in `ALL`.
    pub fn from_index(i: usize) -> Option<Role> {
        Role::ALL.get(i).copied()
    }
    /// A short upper-case name for the role.
    pub fn title(self) -> &'static str {
        match self {
            Role::Cpu => "CPU",
            Role::Mem => "MEMORY",
            Role::Pods => "PODS",
            Role::Health => "CLUSTER",
            Role::Thermal => "THERMAL",
            Role::Storage => "STORAGE",
            Role::PowerMix => "POWER MIX",
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
            Role::Media => "MEDIA",
            Role::Traffic => "WEB TRAFFIC",
            Role::Restarts => "RESTARTS · 24H",
            Role::Uptime => "UPTIME",
            Role::Fans => "FANS",
            Role::Mains => "MAINS",
            Role::Load => "LOAD",
            Role::Ci => "CI",
            Role::Jellyfin => "NOW PLAYING",
            Role::Music => "MUSIC",
            Role::Printer => "3D PRINTER",
            Role::Sen => "SEN LIVE",
            Role::Gif(_) => "GIF",
            Role::Ha(_) => "HOME",
        }
    }

    /// The Material Symbols icon (asset name) shown in the ring gap at the
    /// top of the role's screen.
    pub fn icon(self) -> &'static str {
        match self {
            Role::Cpu => "ms-memory",
            Role::Mem => "ms-memory_alt",
            Role::Pods => "ms-deployed_code",
            Role::Health => "ms-monitor_heart",
            Role::Thermal => "ms-thermostat",
            Role::Storage => "ms-hard_drive",
            Role::PowerMix => "ms-bolt",
            Role::Renewable => "ms-eco",
            Role::GhActivity => "ms-commit",
            Role::Weather => "ms-partly_cloudy_day",
            Role::Wind => "ms-air",
            Role::Aqi => "ms-aq",
            Role::Rain => "ms-rainy",
            Role::Sun => "ms-wb_sunny",
            Role::Moon => "ms-dark_mode",
            Role::Iss => "ms-satellite_alt",
            Role::Ups => "ms-battery_charging_full",
            Role::Net => "ms-lan",
            Role::Deploys => "ms-rocket_launch",
            Role::Media => "ms-video_library",
            Role::Traffic => "ms-swap_vert",
            Role::Restarts => "ms-restart_alt",
            Role::Uptime => "ms-timer",
            Role::Fans => "ms-mode_fan",
            Role::Mains => "ms-electrical_services",
            Role::Load => "ms-speed",
            Role::Ci => "ms-build",
            Role::Jellyfin => "ms-movie",
            Role::Music => "ms-music_note",
            Role::Printer => "ms-view_in_ar",
            Role::Sen => "ms-videocam",
            Role::Gif(_) => "ms-gif_box",
            // the screen picks one by the entity's domain and device class
            Role::Ha(_) => "ms-home",
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
            Role::Media => "media",
            Role::Traffic => "traffic",
            Role::Restarts => "restarts",
            Role::Uptime => "uptime",
            Role::Fans => "fans",
            Role::Mains => "mains",
            Role::Load => "load",
            Role::Ci => "ci",
            Role::Jellyfin => "jellyfin",
            Role::Music => "music",
            Role::Printer => "printer",
            Role::Sen => "sen",
            // `gif:<name>` and `ha:<name>` come from the config (`Config::role_name`).
            Role::Gif(_) => "gif",
            Role::Ha(_) => "ha",
        }
    }
    /// A built-in role by name; `gif:<name>` is resolved by the config.
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
                | Role::Media
                | Role::Traffic
                | Role::Restarts
                | Role::Uptime
                | Role::Fans
                | Role::Mains
                | Role::Load
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
    pub const TEXT3: Color = Color::hex(0x80808A);
    pub const TRACK: Color = Color::hex(0x2C2C33);
    pub const CARD: Color = Color::hex(0x24242A);
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
    /// Movies on the media ring and the Jellyfin movie ring.
    pub const ORANGE: Color = Color::hex(0xF2A65A);
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
    fn every_role_has_a_material_icon() {
        for r in Role::ALL {
            assert!(r.icon().starts_with("ms-"), "{r:?}");
        }
        assert_eq!(Role::Cpu.icon(), "ms-memory");
        assert_eq!(Role::Jellyfin.icon(), "ms-movie");
    }

    #[test]
    fn gif_roles_sit_outside_all_with_their_own_icon() {
        let g = Role::Gif(3);
        assert!(!Role::ALL.contains(&g));
        assert_eq!(g.icon(), "ms-gif_box");
        assert_eq!(g.title(), "GIF");
        assert_eq!(g.name(), "gif", "the full gif:<name> needs the config");
        assert!(!g.is_cluster() && !g.is_sky());
        assert_eq!(Role::parse("gif"), None);
        assert_eq!(Role::Gif(0).index(), Role::ALL.len());
        assert_eq!(g.index(), Role::ALL.len() + 6);
        assert_eq!(Role::Ha(0).index(), Role::ALL.len() + 1);
        assert_eq!(Role::Ha(3).index(), Role::ALL.len() + 7);
        assert_eq!(Role::Ha(0).name(), "ha");
        assert_eq!(Role::Ha(0).icon(), "ms-home");
        assert!(!Role::Ha(0).is_cluster() && !Role::ALL.contains(&Role::Ha(0)));
        assert_ne!(Role::Gif(0), Role::Gif(1));
        assert_eq!(
            Role::from_index(g.index()),
            None,
            "only built-in roles map back"
        );
    }

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
                Role::Media,
                Role::Traffic,
                Role::Restarts,
                Role::Uptime,
                Role::Fans,
                Role::Mains,
                Role::Load,
            ]
        );
        assert!(!Role::PowerMix.is_cluster() && !Role::Renewable.is_cluster());
        assert!(!Role::Weather.is_cluster() && !Role::GhActivity.is_cluster());
    }

    #[test]
    fn thirty_one_roles_with_unique_names() {
        assert_eq!(Role::ALL.len(), 31);
        let names: std::collections::HashSet<&str> = Role::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), 31);
        assert_eq!(Role::parse("printer"), Some(Role::Printer));
        assert_eq!(Role::Printer.icon(), "ms-view_in_ar");
        assert!(!Role::Printer.is_cluster() && !Role::Printer.is_sky());
        for gone in ["face", "price", "carbon"] {
            assert_eq!(Role::parse(gone), None);
        }
        assert_eq!(Role::ALL[18], Role::Deploys);
        assert_eq!(
            &Role::ALL[19..28],
            &[
                Role::Media,
                Role::Traffic,
                Role::Restarts,
                Role::Uptime,
                Role::Fans,
                Role::Mains,
                Role::Load,
                Role::Ci,
                Role::Jellyfin
            ]
        );
        for (name, role) in [
            ("media", Role::Media),
            ("traffic", Role::Traffic),
            ("restarts", Role::Restarts),
            ("uptime", Role::Uptime),
            ("fans", Role::Fans),
            ("mains", Role::Mains),
            ("load", Role::Load),
            ("ci", Role::Ci),
            ("jellyfin", Role::Jellyfin),
        ] {
            assert_eq!(Role::parse(name), Some(role));
            assert!(!role.is_sky());
        }
        assert_eq!(Role::Restarts.title(), "RESTARTS · 24H");
        assert_eq!(Role::Traffic.title(), "WEB TRAFFIC");
        assert_eq!(Role::Jellyfin.title(), "NOW PLAYING");
        assert!(!Role::Ci.is_cluster() && !Role::Jellyfin.is_cluster());
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
        assert_eq!(p2::ORANGE, Color::hex(0xF2A65A));
    }

    #[test]
    fn music_is_the_last_role() {
        assert_eq!(Role::ALL[28], Role::Music);
        assert_eq!(Role::parse("music"), Some(Role::Music));
        assert_eq!(Role::Music.title(), "MUSIC");
        assert_eq!(Role::Music.icon(), "ms-music_note");
        assert!(!Role::Music.is_cluster() && !Role::Music.is_sky());
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
        assert_eq!(p2::TRACK, Color::hex(0x2C2C33));
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

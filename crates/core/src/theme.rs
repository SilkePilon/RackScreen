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

pub const AMBER: Color = Color::hex(0xffb020);
pub const VIOLET: Color = Color::hex(0xa78bfa);
pub const BLUE: Color = Color::hex(0x4f8dff);
pub const GREEN: Color = Color::hex(0x3ddc97);
pub const RED: Color = Color::hex(0xff4d4d);
pub const OFF: Color = Color::hex(0x1c1c1c);
pub const BADGE_FILL: Color = Color::hex(0x0e0e0e);
pub const WHITE: Color = Color::hex(0xffffff);
pub const GREY: Color = Color::hex(0x888888);
pub const DIM_GREY: Color = Color::hex(0x2a2a2a);
pub const BLACK: Color = Color::hex(0x000000);

pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    a.mix(b, t)
}

/// Node temperature colour: 35 °C blue, 55 °C amber, 70 °C red.
pub fn temp_color(c: f32) -> Color {
    if c <= 55.0 {
        BLUE.mix(AMBER, ((c - 35.0) / 20.0).clamp(0.0, 1.0))
    } else {
        AMBER.mix(RED, ((c - 55.0) / 15.0).clamp(0.0, 1.0))
    }
}

/// Electricity Maps carbon intensity scale (gCO2eq/kWh).
pub fn carbon_color(g: f32) -> Color {
    const STOPS: [(f32, u32); 4] = [
        (0.0, 0x2AA364),
        (150.0, 0xF5EB4D),
        (600.0, 0x9E4229),
        (800.0, 0x381D02),
    ];
    let g = g.clamp(0.0, 800.0);
    for w in STOPS.windows(2) {
        let (g0, c0) = w[0];
        let (g1, c1) = w[1];
        if g <= g1 {
            return Color::hex(c0).mix(Color::hex(c1), (g - g0) / (g1 - g0));
        }
    }
    Color::hex(STOPS[3].1)
}

/// 0 = cheapest of the day (green) .. 1 = most expensive (red).
pub fn price_color(t: f32) -> Color {
    GREEN.mix(RED, t.clamp(0.0, 1.0))
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
}

impl Role {
    pub const ALL: [Role; 10] = [
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
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|r| r.name() == s)
    }
    /// Cluster roles need the Kubernetes API link; the electricity roles are fed
    /// by public grid APIs and work without a cluster.
    pub fn is_cluster(self) -> bool {
        matches!(
            self,
            Role::Cpu | Role::Mem | Role::Pods | Role::Health | Role::Thermal | Role::Storage
        )
    }
    pub fn accent(self) -> Color {
        match self {
            Role::Cpu => AMBER,
            Role::Mem => VIOLET,
            Role::Pods => BLUE,
            Role::Health => GREEN,
            Role::Thermal => AMBER,
            Role::Storage => VIOLET,
            Role::PowerMix => Color::hex(0xFFC700),
            Role::Price => GREEN,
            Role::Carbon => Color::hex(0x2AA364),
            Role::Renewable => GREEN,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Role::Cpu => "cpu",
            Role::Mem => "memory-stick",
            Role::Pods => "box",
            Role::Health => "heart-pulse",
            Role::Thermal => "thermometer",
            Role::Storage => "database",
            Role::PowerMix => "zap",
            Role::Price => "euro",
            Role::Carbon => "cloud",
            Role::Renewable => "leaf",
        }
    }
}

pub mod layout {
    pub const SIZE: u32 = 240;
    pub const CX: f32 = 120.0;
    pub const CY: f32 = 120.0;
    pub const RING_R: f32 = 102.0;
    pub const SEG_N: usize = 60;
    pub const SEG_LEN: f32 = 12.0;
    pub const SEG_W: f32 = 4.0;
    pub const ICON_CY: f32 = 98.0;
    pub const ICON_SIZE: f32 = 72.0;
    pub const BADGE_CY: f32 = 165.0;
    pub const BADGE_W: f32 = 64.0;
    pub const BADGE_H: f32 = 26.0;
    pub const BADGE_RADIUS: f32 = 7.0;
    pub const BADGE_TEXT_PX: f32 = 15.0;
    pub const MARKER_CY: f32 = 188.0;
    pub const TORRENT_RADII: [f32; 3] = [102.0, 86.0, 70.0];
    pub const TORRENT_ICON_CY: f32 = 104.0;
    pub const TORRENT_ICON_SIZE: f32 = 44.0;
    pub const TORRENT_BADGE_CY: f32 = 151.0;
    pub const BIG_ICON_SIZE: f32 = 96.0;
    pub const DOT_R: f32 = 5.0;
    pub const DOT_SPACING: f32 = 14.0;
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
    fn temp_color_ramps_blue_amber_red() {
        assert_eq!(temp_color(35.0), BLUE);
        assert_eq!(temp_color(20.0), BLUE);
        assert_eq!(temp_color(55.0), AMBER);
        assert_eq!(temp_color(80.0), RED);
        let mid = temp_color(45.0);
        assert!(mid.r > BLUE.r && mid.r < AMBER.r, "45 °C sits between");
        assert_eq!(lerp(BLACK, WHITE, 0.5), BLACK.mix(WHITE, 0.5));
    }

    #[test]
    fn carbon_and_price_scales() {
        assert_eq!(carbon_color(0.0), Color::hex(0x2AA364));
        assert_eq!(carbon_color(-50.0), Color::hex(0x2AA364));
        assert_eq!(carbon_color(150.0), Color::hex(0xF5EB4D));
        assert_eq!(carbon_color(600.0), Color::hex(0x9E4229));
        assert_eq!(carbon_color(1000.0), Color::hex(0x381D02));
        let mid = carbon_color(75.0);
        assert_eq!(mid, Color::hex(0x2AA364).mix(Color::hex(0xF5EB4D), 0.5));
        assert_eq!(price_color(0.0), GREEN);
        assert_eq!(price_color(1.0), RED);
        assert_eq!(price_color(-1.0), GREEN);
        assert_eq!(price_color(0.5), GREEN.mix(RED, 0.5));
    }

    #[test]
    fn role_index_roundtrip() {
        for r in Role::ALL {
            assert_eq!(Role::from_index(r.index()), Some(r));
        }
    }

    #[test]
    fn cluster_roles_are_the_six_kubernetes_ones() {
        let cluster: Vec<Role> = Role::ALL.into_iter().filter(|r| r.is_cluster()).collect();
        assert_eq!(
            cluster,
            vec![
                Role::Cpu,
                Role::Mem,
                Role::Pods,
                Role::Health,
                Role::Thermal,
                Role::Storage
            ]
        );
        assert!(!Role::PowerMix.is_cluster() && !Role::Price.is_cluster());
    }

    #[test]
    fn role_names_parse_roundtrip() {
        assert_eq!(Role::parse("power-mix"), Some(Role::PowerMix));
        assert_eq!(Role::parse("nope"), None);
        for r in Role::ALL {
            assert_eq!(Role::parse(r.name()), Some(r));
        }
    }
}

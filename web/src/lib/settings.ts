// Every individually addressable setting: its title, help line (wording from
// the old setup TUI), the page it lives on and its built-in default. Setting
// rows read their text from here, and the command palette searches it.

export interface SettingMeta {
  title: string
  description: string
  page: string
  section: string
  /** The built-in default from config.rs, for "Restore defaults". */
  def?: unknown
  keywords?: string
}

const C = "/integrations/cluster"
const E = "/integrations/energy"
const S = "/integrations/sky"
const M = "/integrations/media"
const SP = "/integrations/spotify"
const G = "/integrations/github"
const D = "/display"
const SH = "/shows"
const SYS = "/system"

export const SETTINGS: Record<string, SettingMeta> = {
  "k8s.kubeconfig": {
    title: "Kubeconfig",
    description: "Path to the kubeconfig; blank uses in-cluster access.",
    page: C,
    section: "Kubernetes",
    def: "~/k8s-monitor.yaml",
  },
  "prometheus.namespace": {
    title: "Namespace",
    description: "Namespace the Prometheus service runs in.",
    page: C,
    section: "Prometheus",
    def: "monitoring",
  },
  "prometheus.service": {
    title: "Service",
    description: "Name of the Prometheus service, or auto to pick the first one that exposes the port.",
    page: C,
    section: "Prometheus",
    def: "auto",
  },
  "prometheus.port": {
    title: "Port",
    description: "Port of the Prometheus service inside the cluster.",
    page: C,
    section: "Prometheus",
    def: 9090,
  },
  "prometheus.poll_secs": {
    title: "Poll interval",
    description: "Seconds between Prometheus queries, at least 1.",
    page: C,
    section: "Prometheus",
    def: 5,
  },
  "prometheus.ignore_alerts": {
    title: "Ignored alerts",
    description: "Alert names that never count as firing.",
    page: C,
    section: "Prometheus",
    def: ["Watchdog", "InfoInhibitor"],
  },
  "argocd.enabled": {
    title: "Argo CD",
    description: "Read Applications for the deploys role.",
    page: C,
    section: "Argo CD",
    def: true,
  },
  "argocd.namespace": {
    title: "Namespace",
    description: "Namespace where Argo CD runs.",
    page: C,
    section: "Argo CD",
    def: "argocd",
  },
  "qbittorrent.enabled": {
    title: "qBittorrent",
    description: "Show torrent traffic on the ring.",
    page: C,
    section: "qBittorrent",
    def: true,
  },
  "qbittorrent.namespace": {
    title: "Namespace",
    description: "Namespace of the qBittorrent service.",
    page: C,
    section: "qBittorrent",
    def: "arr-stack",
  },
  "qbittorrent.service": {
    title: "Service",
    description: "Name of the qBittorrent service.",
    page: C,
    section: "qBittorrent",
    def: "qbittorrent",
  },
  "qbittorrent.port": {
    title: "Port",
    description: "Web UI port of qBittorrent.",
    page: C,
    section: "qBittorrent",
    def: 8080,
  },
  "qbittorrent.user": {
    title: "User",
    description: "Web UI user name; empty relies on “bypass auth for localhost”.",
    page: C,
    section: "qBittorrent",
    def: "",
  },
  "qbittorrent.pass": {
    title: "Password",
    description: "Web UI password, stored in the config file.",
    page: C,
    section: "qBittorrent",
  },
  "qbittorrent.poll_secs": {
    title: "Poll interval",
    description: "Seconds between qBittorrent polls, at least 1.",
    page: C,
    section: "qBittorrent",
    def: 3,
  },

  "electricity.enabled": {
    title: "Electricity Maps",
    description: "Power mix and renewable roles.",
    page: E,
    section: "Electricity Maps",
    def: false,
  },
  "electricity.zone": {
    title: "Zone",
    description: "Electricity Maps zone, for example NL.",
    page: E,
    section: "Electricity Maps",
    def: "NL",
  },
  "electricity.token": {
    title: "API token",
    description: "Your free token from app.electricitymaps.com.",
    page: E,
    section: "Electricity Maps",
  },
  "electricity.poll_secs": {
    title: "Poll interval",
    description: "Seconds between polls, at least 60.",
    page: E,
    section: "Electricity Maps",
    def: 300,
  },

  "location.lat": {
    title: "Latitude",
    description: "Decimal degrees, -90 to 90; blank clears.",
    page: S,
    section: "Location",
    def: null,
  },
  "location.lon": {
    title: "Longitude",
    description: "Decimal degrees, -180 to 180; blank clears.",
    page: S,
    section: "Location",
    def: null,
  },
  "weather.enabled": {
    title: "Weather",
    description: "Weather, wind and air quality roles from Open-Meteo; no key.",
    page: S,
    section: "Weather",
    def: false,
  },
  "weather.poll_secs": {
    title: "Poll interval",
    description: "Seconds between Open-Meteo polls, at least 60.",
    page: S,
    section: "Weather",
    def: 600,
  },
  "rain.enabled": {
    title: "Rain",
    description: "Buienradar two-hour nowcast; Netherlands and Belgium.",
    page: S,
    section: "Rain",
    def: false,
  },
  "rain.poll_secs": {
    title: "Poll interval",
    description: "Seconds between rain polls, at least 60.",
    page: S,
    section: "Rain",
    def: 300,
  },
  "iss.enabled": {
    title: "ISS passes",
    description: "Countdown to the next ISS pass, computed from the Celestrak TLE.",
    page: S,
    section: "ISS",
    def: false,
  },
  "iss.min_elevation": {
    title: "Minimum elevation",
    description: "Lowest pass elevation to count, 0 to 90°.",
    page: S,
    section: "ISS",
    def: 10,
  },

  "media.enabled": {
    title: "Media queues",
    description: "Poll the arr queues for the media download card.",
    page: M,
    section: "Media",
    def: false,
  },
  "media.namespace": {
    title: "Namespace",
    description: "Namespace the arr services run in.",
    page: M,
    section: "Media",
    def: "arr-stack",
  },
  ...arr("radarr", "Radarr", 7878),
  ...arr("sonarr", "Sonarr", 8989),
  ...arr("lidarr", "Lidarr", 8686),
  "jellyfin.enabled": {
    title: "Jellyfin",
    description: "Now playing from your Jellyfin server.",
    page: M,
    section: "Jellyfin",
    def: false,
  },
  "jellyfin.url": {
    title: "Server address",
    description: "http://host:8096; the server is outside the cluster.",
    page: M,
    section: "Jellyfin",
    def: "",
  },
  "jellyfin.api_key": {
    title: "API key",
    description: "Jellyfin Dashboard → API Keys.",
    page: M,
    section: "Jellyfin",
  },
  "jellyfin.poll_secs": {
    title: "Poll interval",
    description: "Seconds between session polls, at least 2.",
    page: M,
    section: "Jellyfin",
    def: 5,
  },

  "spotify.enabled": {
    title: "Spotify",
    description: "Now playing from Spotify on the music screen.",
    page: SP,
    section: "Spotify",
    def: false,
  },
  "spotify.client_id": {
    title: "Client ID",
    description: "Your own app at developer.spotify.com.",
    page: SP,
    section: "Spotify",
    def: "",
  },

  "github.enabled": {
    title: "GitHub",
    description: "Fetch your contribution graph and CI runs.",
    page: G,
    section: "GitHub",
    def: false,
  },
  "github.token": {
    title: "Token",
    description: "Fine-grained token: read access to contributions, events and Actions.",
    page: G,
    section: "GitHub",
  },
  "github.poll_secs": {
    title: "Poll interval",
    description: "Seconds between GitHub polls, at least 60.",
    page: G,
    section: "GitHub",
    def: 60,
  },
  "github.ci_repos": {
    title: "CI repositories",
    description: "owner/repo for the ci role; empty: your repos with runs in the last 7 days.",
    page: G,
    section: "GitHub",
    def: [],
  },

  "display.brightness": {
    title: "Brightness",
    description: "0.1 to 1.0; night mode dims further.",
    page: D,
    section: "Panels",
    def: 1,
  },
  "display.fps": {
    title: "Frame rate",
    description: "Frames per second, 1 to 60.",
    page: D,
    section: "Panels",
    def: 60,
  },
  "display.spi_chunk": {
    title: "SPI chunk",
    description: "Bytes per SPI transfer; 65536 once spidev.bufsiz=65536 is in cmdline.txt.",
    page: D,
    section: "Panels",
    def: 4096,
  },
  "display.one_at_a_time": {
    title: "One at a time",
    description: "Only one screen changes to its next role at a time, in random order.",
    page: D,
    section: "Panels",
    def: true,
  },
  "night.enabled": {
    title: "Night mode",
    description: "Fade the panels to a dim moon at night.",
    page: D,
    section: "Night",
    def: true,
  },
  "night.start": {
    title: "Starts at",
    description: "When night begins.",
    page: D,
    section: "Night",
    def: "23:00",
  },
  "night.end": {
    title: "Ends at",
    description: "When night ends.",
    page: D,
    section: "Night",
    def: "07:00",
  },
  "thresholds.hot_cpu": {
    title: "Hot CPU",
    description: "CPU percent that marks a node hot.",
    page: D,
    section: "Thresholds",
    def: 90,
  },
  "thresholds.hot_mem": {
    title: "Hot memory",
    description: "Memory percent that marks a node hot.",
    page: D,
    section: "Thresholds",
    def: 90,
  },
  "thresholds.hot_temp": {
    title: "Hot temperature",
    description: "Temperature that marks a node hot.",
    page: D,
    section: "Thresholds",
    def: 70,
  },

  "shows.enabled": {
    title: "Shows",
    description: "Pixel-art shows across all screens as one wide scene, never at night.",
    page: SH,
    section: "Schedule",
    def: true,
  },
  "shows.every_mins": {
    title: "Every",
    description: "Minutes between the end of one show and the start of the next, at least 2.",
    page: SH,
    section: "Schedule",
    def: 15,
  },
  "shows.gap_mm": {
    title: "Bezel gap",
    description: "Gap between two panels in mm, so a scene lines up across them.",
    page: SH,
    section: "Schedule",
    def: 25,
  },
  "shows.order": {
    title: "Panel order",
    description: "Which screen sits where, from the left. Drag to match your rack.",
    page: SH,
    section: "Schedule",
    def: undefined,
  },

  "web.password": {
    title: "Password",
    description: "Ask for a password before anyone can open this page. Empty: no login.",
    page: SYS,
    section: "Web access",
  },
  "web.port": {
    title: "Port",
    description: "Port the web page listens on. Changing it restarts the service.",
    page: SYS,
    section: "Web access",
    def: 8740,
  },
  "web.bind": {
    title: "Listen address",
    description: "0.0.0.0 for every network, 127.0.0.1 for this Pi only.",
    page: SYS,
    section: "Web access",
    def: "0.0.0.0",
  },
}

function arr(key: string, name: string, port: number): Record<string, SettingMeta> {
  return {
    [`media.${key}.service`]: {
      title: "Service",
      description: "Service name, or auto.",
      page: M,
      section: name,
      def: "auto",
      keywords: name,
    },
    [`media.${key}.port`]: {
      title: "Port",
      description: `${name} web port, usually ${port}.`,
      page: M,
      section: name,
      def: port,
      keywords: name,
    },
    [`media.${key}.api_key`]: {
      title: "API key",
      description: `${name} Settings → General → API Key.`,
      page: M,
      section: name,
      keywords: name,
    },
  }
}

/** The settings on one page that have a default to restore. */
export function defaultsFor(page: string): [string, unknown][] {
  return Object.entries(SETTINGS)
    .filter(([, m]) => m.page === page && "def" in m)
    .map(([id, m]) => [id, m.def])
}

import type { Config, LogLevel, RoleInfo, ShowInfo } from "@/lib/types"

export const SHOWS: ShowInfo[] = [
  { id: "freight", title: "Dusk freight" },
  { id: "deep-sea", title: "Deep sea lantern" },
  { id: "paper-plane", title: "Air mail" },
  { id: "storm", title: "Thunderstorm" },
  { id: "day", title: "Day in a minute" },
  { id: "space", title: "Space hitchhiker" },
  { id: "birds", title: "Birds at sunset" },
  { id: "fireworks", title: "Harbour fireworks" },
  { id: "alpine", title: "Mountains: alpine dawn" },
  { id: "caravan", title: "Desert: caravan" },
  { id: "desert-night", title: "Desert: night tumbleweed" },
  { id: "boulder", title: "Temple run" },
  { id: "time-car", title: "Time machine" },
  { id: "kaiju", title: "Monster in the harbour" },
  { id: "heist", title: "The heist" },
  { id: "shark", title: "Beach day" },
  { id: "dino-chase", title: "Dinosaur chase" },
  { id: "dragon", title: "Dragon over the castle" },
  { id: "pirates", title: "Broadside" },
  { id: "aurora", title: "Northern lights" },
  { id: "launch", title: "Lift-off" },
  { id: "hyperspace", title: "Jump to hyperspace" },
  { id: "ufo", title: "Close encounter" },
  { id: "moon-landing", title: "Lunar landing" },
  { id: "mech", title: "Giant robot at dusk" },
  { id: "black-hole", title: "Event horizon" },
  { id: "lighthouse", title: "Lighthouse keeper" },
]

type R = [name: string, title: string, group: string, description: string]
export const ROLE_DEFS: R[] = [
  ["cpu", "CPU", "Cluster", "Cluster CPU use with the busiest node on the ring."],
  ["mem", "MEMORY", "Cluster", "Cluster memory use and the fullest node."],
  ["pods", "PODS", "Cluster", "Running pods, with pending and failed counts."],
  ["health", "CLUSTER", "Cluster", "Nodes ready, alerts firing and an overall verdict."],
  ["restarts", "RESTARTS · 24H", "Cluster", "Container restarts in the last day."],
  ["uptime", "UPTIME", "Cluster", "How long the nodes have been up."],
  ["load", "LOAD", "Cluster", "Load average across the nodes."],
  ["deploys", "DEPLOYS TODAY", "Cluster", "Argo CD syncs today and apps out of sync."],
  ["thermal", "THERMAL", "Hardware", "Hottest node temperature."],
  ["storage", "STORAGE", "Hardware", "Disk use of the fullest volume."],
  ["fans", "FANS", "Hardware", "Fan speed in percent."],
  ["ups", "UPS", "Hardware", "Battery charge and load of the UPS."],
  ["mains", "MAINS", "Hardware", "Mains voltage and power draw."],
  ["net", "NETWORK", "Hardware", "Network throughput in and out."],
  ["traffic", "WEB TRAFFIC", "Hardware", "Requests per second through the ingress."],
  ["power-mix", "POWER MIX", "Energy", "Where your grid power comes from right now."],
  ["renewable", "RENEWABLE", "Energy", "Renewable share of the grid."],
  ["weather", "WEATHER", "Sky", "Current conditions and temperature."],
  ["wind", "WIND", "Sky", "Wind speed and direction."],
  ["aqi", "AIR QUALITY", "Sky", "European air quality index."],
  ["rain", "RAIN", "Sky", "Rain expected in the next two hours."],
  ["sun", "SUN", "Sky", "Sunrise, sunset and daylight left."],
  ["moon", "MOON", "Sky", "Moon phase and illumination."],
  ["iss", "ISS PASS IN", "Sky", "Countdown to the next visible ISS pass."],
  ["media", "MEDIA", "Media", "Library counts and the current download."],
  ["jellyfin", "NOW PLAYING", "Media", "What Jellyfin is playing."],
  ["music", "MUSIC", "Media", "Now playing from Spotify or Jellyfin."],
  ["printer", "3D PRINTER", "Home", "Bambu Lab print progress, time left and temperatures."],
  ["gh-activity", "CONTRIBUTIONS", "Developer", "Your GitHub contribution graph."],
  ["ci", "CI", "Developer", "Latest GitHub Actions runs."],
]

export function rolesFor(cfg: Config, secretSet: (p: string) => boolean): RoleInfo[] {
  const loc = cfg.location.lat !== null && cfg.location.lon !== null
  const needs = (name: string): string | null => {
    if (name === "power-mix" || name === "renewable")
      return secretSet("electricity.token") ? null : "Electricity Maps token"
    if (["weather", "wind", "aqi", "rain", "sun", "moon", "iss"].includes(name))
      return loc ? null : "Location"
    if (name === "gh-activity" || name === "ci") return secretSet("github.token") ? null : "GitHub token"
    if (name === "deploys") return cfg.argocd.enabled ? null : "Argo CD"
    if (name === "jellyfin") return cfg.jellyfin.enabled && cfg.jellyfin.url ? null : "Jellyfin address"
    if (name === "printer")
      return cfg.bambu.enabled && cfg.bambu.host && secretSet("bambu.access_code") ? null : "Bambu printer"
    if (name.startsWith("ha:"))
      return cfg.homeassistant.enabled && cfg.homeassistant.url && secretSet("homeassistant.token")
        ? null
        : "Home Assistant token"
    if (name === "music") {
      const j = cfg.jellyfin.enabled && cfg.jellyfin.url
      const s = cfg.spotify.enabled && cfg.spotify.client_id
      return j || s ? null : "Spotify or Jellyfin"
    }
    return null
  }
  const builtin = ROLE_DEFS.map(([name, title, group, description]) => ({
    name,
    title,
    group,
    description,
    needs: needs(name),
  }))
  const gifs = (cfg.gifs ?? []).map((g) => ({
    name: `gif:${g.name}`,
    title: "GIF",
    group: "GIFs",
    description: g.url,
    needs: null,
  }))
  const ha = (cfg.homeassistant.entities ?? []).map((e) => ({
    name: `ha:${e.name}`,
    title: e.label || e.name,
    group: "Home",
    description: "A Home Assistant entity: a number with its sparkline, or a state such as on, open or home.",
    needs: needs(`ha:${e.name}`),
  }))
  return [...builtin, ...gifs, ...ha]
}

export function defaultConfig(): Config {
  const hz = 50_000_000
  return {
    k8s: { kubeconfig: "~/k8s-monitor.yaml" },
    prometheus: {
      namespace: "monitoring",
      service: "auto",
      port: 9090,
      poll_secs: 5,
      ignore_alerts: ["Watchdog", "InfoInhibitor"],
    },
    qbittorrent: {
      enabled: true,
      namespace: "arr-stack",
      service: "qbittorrent",
      port: 8080,
      user: "",
      pass: null,
      poll_secs: 3,
    },
    night: { enabled: true, start: "23:00", end: "07:00" },
    thresholds: { hot_cpu: 90, hot_mem: 90, hot_temp: 70 },
    display: { brightness: 0.85, fps: 60, spi_chunk: 4096, one_at_a_time: true },
    electricity: { enabled: true, zone: "NL", token: null, poll_secs: 300 },
    location: { lat: 52.0907, lon: 5.1214 },
    weather: { enabled: true, poll_secs: 600 },
    rain: { enabled: true, poll_secs: 300 },
    iss: { enabled: false, min_elevation: 10 },
    github: { enabled: true, token: null, poll_secs: 60, ci_repos: ["SilkePilon/RackScreen"] },
    argocd: { enabled: true, namespace: "argocd" },
    media: {
      enabled: false,
      namespace: "arr-stack",
      radarr: { service: "auto", port: 7878, api_key: null },
      sonarr: { service: "auto", port: 8989, api_key: null },
      lidarr: { service: "auto", port: 8686, api_key: null },
    },
    jellyfin: { enabled: false, url: "", api_key: null, poll_secs: 5 },
    bambu: { enabled: false, host: "", serial: "", access_code: null },
    homeassistant: {
      enabled: true,
      url: "http://homeassistant.local:8123",
      token: null,
      poll_secs: 15,
      entities: [{ name: "living-temp", entity: "sensor.living_room_temperature", label: "Living room" }],
    },
    spotify: { enabled: true, client_id: "4f1c9a2b7e0d4c3f8a6b5d2e1f0a9c8b" },
    shows: { enabled: true, every_mins: 15, gap_mm: 25 },
    transitions: { style: "fade-black", direction: "", secs: 0.4, random: false },
    gifs: [
      { name: "party", url: "https://giphy.com/gifs/party-parrot-l3q2zVr6cu95nF6O4" },
      { name: "nod", url: "https://tenor.com/view/nod-yes-gif-12345678" },
    ],
    gif_search: { giphy_key: null, tenor_key: null, rating: "g" },
    screens: [
      {
        roles: ["cpu", "power-mix"],
        cycle_secs: 15,
        spi: 0,
        cs: 0,
        dc: 6,
        rst: 5,
        rotate: 270,
        hflip: false,
        hz,
      },
      {
        roles: ["mem", "renewable", "weather"],
        cycle_secs: 15,
        spi: 0,
        cs: 1,
        dc: 13,
        rst: 26,
        rotate: 270,
        hflip: true,
        hz,
      },
      {
        roles: ["pods", "music"],
        cycle_secs: 20,
        spi: 1,
        cs: 0,
        dc: 23,
        rst: 22,
        rotate: 270,
        hflip: true,
        hz,
      },
      {
        roles: ["health", "gh-activity", "gif:party"],
        cycle_secs: 15,
        spi: 1,
        cs: 1,
        dc: 4,
        rst: 27,
        rotate: 270,
        hflip: true,
        hz,
      },
    ],
    web: { enabled: true, bind: "0.0.0.0", port: 8740, password: null },
  }
}

const TARGET: Record<string, string> = {
  prometheus: "rackscreen_sources::prometheus",
  k8s: "rackscreen_sources::k8s",
  render: "rackscreen_app::render",
  weather: "rackscreen_sources::weather",
  spotify: "rackscreen_sources::spotify",
  electricity: "rackscreen_sources::electricity",
  monitor: "rackscreen_app::monitor",
  web: "rackscreen_web::server",
  qbittorrent: "rackscreen_sources::qbittorrent",
  shows: "rackscreen_app::shows",
  spi: "rackscreen_app::spi",
}
const MESSAGES: [LogLevel, string][] = [
  ["INFO", "prometheus: 14 series in 38 ms"],
  ["INFO", "k8s: watch pods resync, 112 pods on 7 nodes"],
  ["DEBUG", "render: screen 2 frame 3.1 ms, push 20.8 ms"],
  ["INFO", "weather: Utrecht 11.4 °C, partly cloudy"],
  ["WARN", "spotify: 429 Too Many Requests, retry after 12 s"],
  ["INFO", "electricity: NL 61% renewable, 214 gCO2eq/kWh"],
  ["DEBUG", "monitor: role cycle screen 1 → power-mix"],
  ["INFO", "web: GET /api/status 200 in 0.4 ms"],
  ["WARN", "prometheus: alert KubeMemoryOvercommit firing"],
  ["ERROR", "qbittorrent: connect 10.43.12.7:8080: connection refused"],
  ["INFO", "shows: next show in 14 min"],
  ["DEBUG", "spi: chunk 4096 B, 28 transfers per frame"],
]
let seq = 1
export function makeLog(at = Date.now()) {
  const [level, msg] = MESSAGES[Math.floor(Math.random() * MESSAGES.length)]
  const target = TARGET[msg.split(":")[0]] ?? "rackscreen_app"
  return { seq: seq++, unix_ms: at, level, target, msg }
}

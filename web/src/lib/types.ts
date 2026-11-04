// The JSON shapes of the RackScreen HTTP API. `Config` mirrors the serde form
// of crates/app/src/config.rs (snake_case) plus the `web` block. Secret fields
// arrive as `null`; on PUT, `null` keeps the stored value, "" clears it and any
// other string replaces it.

export type Secret = string | null

export interface K8sCfg {
  kubeconfig: string
}
export interface PromCfg {
  namespace: string
  service: string
  port: number
  poll_secs: number
  ignore_alerts: string[]
}
export interface QbitCfg {
  enabled: boolean
  namespace: string
  service: string
  port: number
  user: string
  pass: Secret
  poll_secs: number
}
export interface NightCfg {
  enabled: boolean
  start: string
  end: string
}
export interface ThresholdsCfg {
  hot_cpu: number
  hot_mem: number
  hot_temp: number
}
export interface DisplayCfg {
  brightness: number
  fps: number
  spi_chunk: number
  one_at_a_time: boolean
}
export interface ElectricityCfg {
  enabled: boolean
  zone: string
  token: Secret
  poll_secs: number
}
export interface LocationCfg {
  lat: number | null
  lon: number | null
}
export interface WeatherCfg {
  enabled: boolean
  poll_secs: number
}
export interface RainCfg {
  enabled: boolean
  poll_secs: number
}
export interface IssCfg {
  enabled: boolean
  min_elevation: number
}
export interface GithubCfg {
  enabled: boolean
  token: Secret
  poll_secs: number
  ci_repos: string[]
}
export interface ArgocdCfg {
  enabled: boolean
  namespace: string
}
export interface ArrCfg {
  service: string
  port: number
  api_key: Secret
}
export interface MediaCfg {
  enabled: boolean
  namespace: string
  radarr: ArrCfg
  sonarr: ArrCfg
  lidarr: ArrCfg
}
export interface JellyfinCfg {
  enabled: boolean
  url: string
  api_key: Secret
  poll_secs: number
}
export interface SpotifyCfg {
  enabled: boolean
  client_id: string
}
export interface ShowsCfg {
  enabled: boolean
  every_mins: number
  gap_mm: number
  /** Left out while it is every show. */
  list?: string[]
  /** Config index of each screen from the left; left out = config order. */
  order?: number[] | null
}
export interface GifCfg {
  name: string
  url: string
}
export type GifRating = "g" | "pg" | "pg-13" | "r"
export interface GifSearchCfg {
  giphy_key: Secret
  tenor_key: Secret
  rating: GifRating
}
export interface ScreenCfg {
  roles: string[]
  cycle_secs: number
  spi: number
  cs: number
  dc: number
  rst: number
  rotate: number
  hflip: boolean
  hz: number
}
export interface WebCfg {
  enabled: boolean
  bind: string
  port: number
  password: Secret
}

export interface Config {
  k8s: K8sCfg
  prometheus: PromCfg
  qbittorrent: QbitCfg
  night: NightCfg
  thresholds: ThresholdsCfg
  display: DisplayCfg
  electricity: ElectricityCfg
  location: LocationCfg
  weather: WeatherCfg
  rain: RainCfg
  iss: IssCfg
  github: GithubCfg
  argocd: ArgocdCfg
  media: MediaCfg
  jellyfin: JellyfinCfg
  spotify: SpotifyCfg
  shows: ShowsCfg
  /** Left out when empty. */
  gifs?: GifCfg[]
  gif_search: GifSearchCfg
  screens: ScreenCfg[]
  web: WebCfg
}

/** Dotted paths of every secret field. */
export const SECRET_PATHS = [
  "qbittorrent.pass",
  "electricity.token",
  "github.token",
  "media.radarr.api_key",
  "media.sonarr.api_key",
  "media.lidarr.api_key",
  "jellyfin.api_key",
  "web.password",
  "gif_search.giphy_key",
  "gif_search.tenor_key",
] as const
export type SecretPath = (typeof SECRET_PATHS)[number]

export type SecretsMap = Partial<Record<SecretPath, boolean>>

export interface ConfigResponse {
  config: Config
  secrets: SecretsMap
}
export interface SaveResponse extends ConfigResponse {
  restart: "monitor" | "process" | "none"
}

export type GifProvider = "giphy" | "tenor"
export interface GifHit {
  id: string
  title: string
  /** The link to store under gifs:. */
  url: string
  preview: string
  width: number
  height: number
  /** Frames of the full GIF when the provider says (GIPHY), else null. */
  frames: number | null
}
export interface GifPage {
  results: GifHit[]
  /** Pass back as offset for the next page; null at the end. */
  next: string | null
}
export type GifProviders = Record<GifProvider, boolean>

export interface AuthState {
  required: boolean
  authenticated: boolean
}

export interface Info {
  version: string
  hostname: string
  sim: boolean
  started_unix: number
  config_path: string
  config_writable: boolean
  fix_hint: string | null
  addresses: string[]
  screens: number
}

export type NightMode = "auto" | "on" | "off"
export type Showing = "role" | "show" | "night" | "pattern" | "identify"
export type LinkState = "ok" | "warn" | "down" | "off"
export type Tone = "ok" | "warn" | "bad" | "muted"

export interface ScreenStatus {
  index: number
  role: string
  title: string
  roles: string[]
  cycle_secs: number
  cycle_progress: number
  showing: Showing
}
export interface LinkStatus {
  name: string
  state: LinkState
  detail: string
}
export interface FrameStats {
  index: number
  render_ms: number
  render_p95_ms: number
  push_ms: number
  fps: number
}
export interface Tile {
  key: string
  label: string
  value: string
  unit: string
  tone: Tone
}
export type MonitorState = "starting" | "running" | "stopped" | "error"

export interface Status {
  monitor: { state: MonitorState; error: string | null; since_unix: number }
  night: { mode: NightMode; active: boolean }
  screens: ScreenStatus[]
  links: LinkStatus[]
  stats: FrameStats[]
  tiles: Tile[]
  /** How old `screens[].cycle_progress` was when this answer left the Pi. */
  age_ms?: number
  /** Client only: `performance.now()` when this answer arrived. */
  received?: number
  host: {
    cpu_temp_c: number | null
    load1: number | null
    mem_used_mb: number | null
    mem_total_mb: number | null
  }
}

export interface RoleInfo {
  name: string
  title: string
  group: string
  description: string
  needs: string | null
}
export interface ShowInfo {
  id: string
  title: string
}

export type LogLevel = "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"
export interface LogLine {
  seq: number
  unix_ms: number
  level: LogLevel
  target: string
  msg: string
}
export interface LogsPage {
  lines: LogLine[]
  next: number
}

export interface SpotifyState {
  enabled: boolean
  client_id_set: boolean
  logged_in: boolean
  status: string
}

export interface UpdateState {
  current: string
  latest: string | null
  newer: boolean
  can_self_update: boolean
  job: { state: "idle" | "running" | "done" | "failed"; log: string[] }
}

export interface SystemInfo {
  model: string | null
  os: string | null
  kernel: string | null
  arch: string
  hostname: string
  uptime_secs: number | null
  cpu: {
    cores: number | null
    usage_pct: number | null
    temp_c: number | null
    freq_mhz: number | null
    load: [number, number, number] | null
  }
  memory: {
    total_mb: number
    available_mb: number
    used_mb: number
    swap_total_mb: number
    swap_used_mb: number
  } | null
  disk: { path: string; total_gb: number; used_gb: number } | null
  process: { rss_mb: number | null; cpu_pct: number | null; threads: number | null; uptime_secs: number }
  throttled: { now: boolean; since_boot: boolean; flags: string[]; raw: number } | null
  network: { iface: string; ipv4: string }[]
}

export type GifLoadState = "off" | "loading" | "ready" | "error"
export interface GifMemoryRow {
  name: string
  used: boolean
  state: GifLoadState
  frames: number | null
  mb: number | null
  error: string | null
}
export interface GifMemory {
  memory: { total_mb: number | null; available_mb: number | null; process_rss_mb: number | null }
  frame_bytes: number
  max_frames: number
  max_gif_mb: number
  reserve_mb: number
  gifs: GifMemoryRow[]
  decoded_mb: number
  pending_mb: number
  budget_mb: number | null
  left_mb: number | null
  fits_more: number | null
}

export type TailscaleBackendState =
  | "NoState"
  | "NeedsLogin"
  | "NeedsMachineAuth"
  | "Starting"
  | "Running"
  | "Stopped"

export interface TailscaleDevice {
  hostname: string
  /** MagicDNS name without the trailing dot; "" when there is none. */
  dns_name: string
  ips: string[]
  os: string
  online: boolean
  last_seen: string | null
  exit_node: boolean
}

export interface TailscaleState {
  installed: boolean
  version: string | null
  /** The service user is Tailscale's operator (can run up/down/set without root). */
  operator_ok: boolean
  /** sudo allows the one-click `rackscreen tailscale-setup`. */
  can_setup: boolean
  /** `web.password` is set; every Tailscale action is refused (403) without it. */
  password_set: boolean
  backend_state: TailscaleBackendState | null
  auth_url: string | null
  self: TailscaleDevice | null
  tailnet: string | null
  user: { login: string; display_name: string } | null
  peers: TailscaleDevice[]
  ssh: boolean
  serve: { on: boolean; url: string | null }
  health: string[]
  /** The Pi's hostname (the default device name). */
  hostname: string
  error: string | null
}

export interface TailscaleSetupJob {
  state: "idle" | "running" | "done" | "failed"
  log: string[]
}

export interface TailscaleLoginResult {
  auth_url: string | null
  running: boolean
}

export interface TailscaleServeResult {
  on: boolean
  url: string | null
  /** HTTPS is off on the tailnet: where to turn it on. */
  enable_url?: string
  error?: string
}

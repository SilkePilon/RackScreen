import type {
  AuthState,
  Config,
  ConfigResponse,
  GifMemory,
  GifPage,
  GifProvider,
  GifProviders,
  Info,
  LogsPage,
  NightMode,
  RoleInfo,
  SaveResponse,
  ShowInfo,
  SpotifyState,
  BambuState,
  BambuPrinter,
  BambuRegion,
  Status,
  SystemInfo,
  TailscaleLoginResult,
  TailscaleServeResult,
  TailscaleSetupJob,
  TailscaleState,
  UpdateState,
} from "@/lib/types"

/** An answer from the API that is not 2xx; `status` 0 means unreachable. */
export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
    this.name = "ApiError"
  }
  get unreachable() {
    return this.status === 0 || this.status === 502 || this.status === 503 || this.status === 504
  }
}

type Listener = () => void
const unauthorizedListeners = new Set<Listener>()
/** Called whenever any request answers 401 (the session expired). */
export function onUnauthorized(fn: Listener) {
  unauthorizedListeners.add(fn)
  return () => {
    unauthorizedListeners.delete(fn)
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json" }
  if (method !== "GET") headers["X-RackScreen"] = "1"
  if (body !== undefined) headers["Content-Type"] = "application/json"
  let res: Response
  try {
    res = await fetch(`/api${path}`, {
      method,
      headers,
      credentials: "same-origin",
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch {
    throw new ApiError(0, "Can't reach RackScreen")
  }
  const text = await res.text()
  let data: unknown = null
  if (text) {
    try {
      data = JSON.parse(text)
    } catch {
      data = null
    }
  }
  if (!res.ok) {
    if (res.status === 401 && path !== "/auth/login") {
      unauthorizedListeners.forEach((fn) => fn())
    }
    const msg =
      data && typeof data === "object" && "error" in data && typeof data.error === "string"
        ? data.error
        : `${res.status} ${res.statusText || "error"}`
    throw new ApiError(res.status, msg)
  }
  return data as T
}

const get = <T>(path: string) => request<T>("GET", path)
const post = <T>(path: string, body?: unknown) => request<T>("POST", path, body ?? {})
const put = <T>(path: string, body: unknown) => request<T>("PUT", path, body)

export const api = {
  auth: () => get<AuthState>("/auth"),
  login: (password: string) => post<{ ok: true }>("/auth/login", { password }),
  logout: () => post<{ ok: true }>("/auth/logout"),

  info: () => get<Info>("/info"),
  status: () => get<Status>("/status"),
  roles: () => get<RoleInfo[]>("/roles"),
  shows: () => get<ShowInfo[]>("/shows"),
  system: () => get<SystemInfo>("/system"),
  gifMemory: () => get<GifMemory>("/gifs/memory"),

  config: () => get<ConfigResponse>("/config"),
  saveConfig: (config: Config) => put<SaveResponse>("/config", { config }),
  validateConfig: (config: unknown) => post<{ ok: boolean; error?: string }>("/config/validate", { config }),
  validateYaml: (yaml: string) => post<{ ok: boolean; error?: string }>("/config/validate", { yaml }),
  yaml: () => get<{ yaml: string }>("/config/yaml"),
  saveYaml: (yaml: string) => put<SaveResponse>("/config/yaml", { yaml }),

  identify: (secs = 5) => post<unknown>("/control/identify", { secs }),
  pattern: (on: boolean) => post<unknown>("/control/pattern", { on }),
  jumpRole: (screen: number, role: string) => post<unknown>("/control/role", { screen, role }),
  play: (show: string) => post<unknown>("/control/play", { show }),
  transitionTest: (screen?: number) =>
    post<unknown>("/control/transition-test", screen === undefined ? {} : { screen }),
  night: (mode: NightMode) => post<unknown>("/control/night", { mode }),
  restart: () => post<unknown>("/control/restart"),

  logs: (after?: number, limit = 500) =>
    get<LogsPage>(`/logs?${after !== undefined ? `after=${after}&` : ""}limit=${limit}`),

  bambu: () => get<BambuState>("/bambu"),
  bambuLogin: (account: string, password: string, region: BambuRegion) =>
    post<BambuState | { needs: "code" | "tfa" }>("/bambu/login", { account, password, region }),
  bambuCode: (code: string) => post<BambuState>("/bambu/code", { code }),
  bambuResend: () => post<unknown>("/bambu/resend"),
  bambuPrinters: () => get<{ printers: BambuPrinter[] }>("/bambu/printers"),
  bambuLogout: () => post<BambuState>("/bambu/logout"),

  spotify: () => get<SpotifyState>("/spotify"),
  spotifyLogin: () => post<{ url: string }>("/spotify/login"),
  spotifyCallback: (pasted: string) => post<{ ok: boolean; status: string }>("/spotify/callback", { pasted }),
  spotifyLogout: () => post<unknown>("/spotify/logout"),

  gifProviders: () => get<GifProviders>("/gifs/providers"),
  gifSearch: (provider: GifProvider, q: string, offset?: string | null) =>
    get<GifPage>(`/gifs/search?${new URLSearchParams({ provider, q, ...(offset ? { offset } : {}) })}`),
  gifTrending: (provider: GifProvider, offset?: string | null) =>
    get<GifPage>(`/gifs/trending?${new URLSearchParams({ provider, ...(offset ? { offset } : {}) })}`),
  gifTrendingTerms: (provider: GifProvider) =>
    get<{ terms: string[] }>(`/gifs/trending-terms?provider=${provider}`),

  update: (force = false) => get<UpdateState>(force ? "/update?force=1" : "/update"),
  startUpdate: () => post<unknown>("/update"),

  tailscale: () => get<TailscaleState>("/tailscale"),
  tailscaleSetupJob: () => get<TailscaleSetupJob>("/tailscale/setup"),
  tailscaleSetup: () => post<unknown>("/tailscale/setup"),
  tailscaleLogin: (body: { hostname?: string; auth_key?: string; ssh?: boolean }) =>
    post<TailscaleLoginResult>("/tailscale/login", body),
  tailscaleUp: () => post<unknown>("/tailscale/up"),
  tailscaleDown: () => post<unknown>("/tailscale/down"),
  tailscaleLogout: () => post<unknown>("/tailscale/logout"),
  tailscaleSsh: (on: boolean) => post<unknown>("/tailscale/ssh", { on }),
  tailscaleServe: (on: boolean) => post<TailscaleServeResult>("/tailscale/serve", { on }),
}

export function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message
  return String(e)
}

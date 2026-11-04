// A fake RackScreen API for `npm run dev:mock`. Patches fetch for /api/*,
// and swaps the live-frame socket and log stream for local generators.
// Handy toggles from the browser console: __mock.offline(true), __mock.login(true).
// URL flags: ?login (password "rackscreen"), ?offline, ?update, ?nogifkeys (no GIF search keys).

import { parse, stringify } from "yaml"

import { setLiveFactory, setLogStreamFactory, type LiveSocket, type LogSource } from "@/lib/live"
import { setAt, getAt } from "@/lib/path"
import type { Config, GifHit, GifPage, GifProvider, LogLine, NightMode, SecretPath, Showing, Status, UpdateState } from "@/lib/types"
import { SECRET_PATHS } from "@/lib/types"
import { defaultConfig, makeLog, rolesFor, ROLE_DEFS, SHOWS } from "@/mock/data"
import { drawFrame } from "@/mock/frames"
import "@/mock/show-cards"

const params = new URLSearchParams(location.search)

const state = {
  config: defaultConfig(),
  secretValues: {
    "electricity.token": "em-xxxxxxxx",
    "github.token": "github_pat_xxx",
    "qbittorrent.pass": "hunter2",
    ...(params.has("nogifkeys")
      ? {}
      : { "gif_search.giphy_key": "giphy-mock-key", "gif_search.tenor_key": "tenor-mock-key" }),
  } as Partial<Record<SecretPath, string>>,
  night: "auto" as NightMode,
  patternUntil: 0,
  identifyUntil: 0,
  showUntil: 0,
  showId: "",
  restartingUntil: 0,
  offline: params.has("offline"),
  loginRequired: params.has("login"),
  authenticated: false,
  spotifyLoggedIn: false,
  started: Math.floor(Date.now() / 1000) - 3 * 86400 - 4 * 3600,
  monitorSince: Math.floor(Date.now() / 1000) - 3 * 86400,
  jumps: new Map<number, { role: string; at: number }>(),
  update: {
    current: "0.13.0",
    latest: params.has("update") ? "v0.13.1" : "v0.13.0",
    newer: params.has("update"),
    can_self_update: true,
    job: { state: "idle", log: [] },
  } as UpdateState,
  logs: [] as LogLine[],
}

// Seed some history.
{
  const now = Date.now()
  for (let i = 240; i > 0; i--) state.logs.push(makeLog(now - i * 1500))
}

const secretSet = (p: string) => !!state.secretValues[p as SecretPath]
const titleOf = (role: string) =>
  role.startsWith("gif:") ? "GIF" : (ROLE_DEFS.find((r) => r[0] === role)?.[1] ?? role.toUpperCase())

function maskedConfig(): Config {
  let c = structuredClone(state.config)
  for (const p of SECRET_PATHS) c = setAt(c, p, null)
  return c
}
function secretsMap() {
  return Object.fromEntries(SECRET_PATHS.map((p) => [p, secretSet(p)]))
}
function fullConfig(): Config {
  let c = structuredClone(state.config)
  for (const p of SECRET_PATHS) c = setAt(c, p, state.secretValues[p] ?? "")
  return c
}

function screenNow(i: number, now: number) {
  const s = state.config.screens[i]
  const jump = state.jumps.get(i)
  const period = s.cycle_secs * 1000
  let idx: number
  let progress: number
  if (jump && now - jump.at < period && s.roles.includes(jump.role)) {
    idx = s.roles.indexOf(jump.role)
    progress = (now - jump.at) / period
  } else {
    const offset = i * 3700
    const k = Math.floor((now + offset) / period)
    idx = k % s.roles.length
    progress = ((now + offset) % period) / period
  }
  let showing: Showing = "role"
  if (now < state.identifyUntil) showing = "identify"
  else if (now < state.patternUntil) showing = "pattern"
  else if (state.night === "on") showing = "night"
  else if (now < state.showUntil) showing = "show"
  return { role: s.roles[idx], progress, showing }
}

function status(): Status {
  const now = Date.now()
  const restarting = now < state.restartingUntil
  const wob = (k: number) => (Math.sin(now / 4000 + k) + 1) / 2
  return {
    monitor: {
      state: restarting ? "starting" : "running",
      error: null,
      since_unix: state.monitorSince,
    },
    age_ms: 0,
    night: { mode: state.night, active: state.night === "on" },
    screens: state.config.screens.map((s, i) => {
      const n = screenNow(i, now)
      return {
        index: i,
        role: n.role,
        title: titleOf(n.role),
        roles: s.roles,
        cycle_secs: s.cycle_secs,
        cycle_progress: n.progress,
        showing: n.showing,
      }
    }),
    links: [
      { name: "Kubernetes", state: "ok", detail: "7 nodes, 112 pods" },
      { name: "Prometheus", state: "ok", detail: "38 ms per query" },
      { name: "Argo CD", state: "ok", detail: "23 apps, all synced" },
      { name: "qBittorrent", state: "down", detail: "connection refused" },
      { name: "Electricity Maps", state: "ok", detail: "NL, updated 2 min ago" },
      { name: "Open-Meteo", state: "ok", detail: "updated 6 min ago" },
      { name: "Spotify", state: state.spotifyLoggedIn ? "ok" : "warn", detail: state.spotifyLoggedIn ? "Logged in as Silke" : "Not logged in" },
      { name: "GitHub", state: "ok", detail: "4,812 requests left" },
      { name: "Jellyfin", state: "off", detail: "Turned off" },
    ],
    stats: state.config.screens.map((_, i) => ({
      index: i,
      render_ms: +(2.4 + wob(i) * 1.2).toFixed(1),
      render_p95_ms: +(4.6 + wob(i + 1) * 1.4).toFixed(1),
      push_ms: +(20.2 + wob(i + 2) * 1.6).toFixed(1),
      fps: 60,
    })),
    tiles: [
      { key: "cpu", label: "Cluster CPU", value: String(Math.round(18 + wob(0) * 30)), unit: "%", tone: "ok" },
      { key: "mem", label: "Cluster memory", value: String(Math.round(61 + wob(1) * 6)), unit: "%", tone: "ok" },
      { key: "pods", label: "Pods running", value: "112", unit: "", tone: "ok" },
      { key: "nodes", label: "Nodes ready", value: "7/7", unit: "", tone: "ok" },
      { key: "alerts", label: "Alerts firing", value: "1", unit: "", tone: "warn" },
      { key: "temp", label: "Hottest node", value: String(Math.round(58 + wob(2) * 8)), unit: "°C", tone: "muted" },
      { key: "weather", label: "Utrecht", value: "11", unit: "°C", tone: "muted" },
      { key: "renewable", label: "Renewable", value: "61", unit: "%", tone: "ok" },
      { key: "music", label: "Now playing", value: "Teardrop", unit: "Massive Attack", tone: "muted" },
    ],
    host: { cpu_temp_c: +(49 + wob(3) * 4).toFixed(1), load1: +(0.3 + wob(4) * 0.4).toFixed(2), mem_used_mb: 312, mem_total_mb: 906 },
  }
}

function validate(c: Config): string | null {
  if (!c.screens?.length) return "config needs at least one [[screens]] entry"
  const gifs = c.gifs ?? []
  for (const [i, g] of gifs.entries()) {
    if (!/^[a-z0-9-]+$/.test(g.name)) return `gifs: '${g.name}' is not a valid name (lowercase letters, digits and -)`
    if (gifs.slice(0, i).some((o) => o.name === g.name)) return `gifs: '${g.name}' is listed twice`
    const u = g.url.trim()
    if (!/^https:\/\//.test(u) || !/(giphy\.com|tenor\.com|\.gif($|\?))/.test(u))
      return `gifs: '${g.name}': not a GIPHY, Tenor or .gif link (got ${u})`
  }
  const known = new Set(ROLE_DEFS.map((r) => r[0]))
  for (const [i, s] of c.screens.entries()) {
    if (!s.roles.length) return `screen ${i + 1}: screen has no roles`
    for (const r of s.roles) {
      if (r.startsWith("gif:")) {
        if (!gifs.some((g) => `gif:${g.name}` === r)) return `unknown GIF '${r}': add '${r.slice(4)}' under gifs:`
      } else if (!known.has(r)) return `unknown screen role '${r}'`
    }
    if (![0, 90, 180, 270].includes(s.rotate)) return `screen '${s.roles[0]}': rotate must be 0, 90, 180 or 270 (got ${s.rotate})`
    if (s.cycle_secs < 3 || s.cycle_secs > 300)
      return `screen '${s.roles[0]}': cycle_secs must be between 3 and 300 (got ${s.cycle_secs})`
  }
  if (!["g", "pg", "pg-13", "r"].includes(c.gif_search?.rating ?? "g"))
    return `gif_search.rating must be g, pg, pg-13 or r (got ${c.gif_search.rating})`
  if (c.electricity.enabled && !c.electricity.zone.trim()) return "electricity.zone must be set when electricity is enabled"
  if ((c.location.lat === null) !== (c.location.lon === null)) return "location needs both lat and lon, or neither"
  if ((c.weather.enabled || c.rain.enabled || c.iss.enabled) && c.location.lat === null)
    return "weather, rain and iss need location.lat and location.lon"
  for (const [n, s] of [["weather", c.weather.poll_secs], ["rain", c.rain.poll_secs], ["github", c.github.poll_secs]] as const)
    if (s < 60) return `${n}.poll_secs must be at least 60 (got ${s})`
  if (c.shows.every_mins < 2) return `shows.every_mins must be at least 2 (got ${c.shows.every_mins})`
  if (c.shows.gap_mm < 0 || c.shows.gap_mm > 200) return `shows.gap_mm must be between 0 and 200 (got ${c.shows.gap_mm})`
  const url = c.jellyfin.url.trim()
  if (url && !/^https?:\/\//.test(url)) return `jellyfin.url must start with http:// or https:// (got ${url})`
  if (!/^[A-Za-z0-9]*$/.test(c.spotify.client_id.trim()))
    return `spotify.client_id must be letters and digits only (got ${c.spotify.client_id})`
  return null
}

function save(incoming: Config, fromYaml: boolean) {
  const err = validate(incoming)
  if (err) return json(400, { error: err })
  const webChanged =
    JSON.stringify({ ...state.config.web, password: null }) !== JSON.stringify({ ...incoming.web, password: null })
  for (const p of SECRET_PATHS) {
    const v = getAt(incoming, p)
    if (fromYaml) {
      state.secretValues[p] = typeof v === "string" ? v : ""
    } else if (typeof v === "string") state.secretValues[p] = v
  }
  if (state.secretValues["web.password"]) state.loginRequired = true
  else if (!params.has("login")) state.loginRequired = false
  let c = structuredClone(incoming)
  for (const p of SECRET_PATHS) c = setAt(c, p, null)
  state.config = c
  const onlySearch =
    JSON.stringify({ ...state.config, gif_search: null }) === JSON.stringify({ ...c, gif_search: null })
  const restart = webChanged ? "process" : onlySearch ? "none" : "monitor"
  if (webChanged) {
    state.started = Math.floor(Date.now() / 1000) + 4
    state.offline = true
    setTimeout(() => (state.offline = false), 4000)
  } else if (restart === "monitor") {
    state.restartingUntil = Date.now() + 1500
    state.monitorSince = Math.floor(Date.now() / 1000) + 1
  }
  pushLog("INFO", `config saved; restarting the ${restart}`)
  return json(200, { config: maskedConfig(), secrets: secretsMap(), restart })
}

// Fake GIF search: animated SVG previews as data URLs, nothing from the network.
const WORDS = ["happy", "dance", "party", "cat", "wow", "yes", "spin", "wave", "jump", "cheers", "hype", "chill"]
function hash(s: string) {
  let h = 2166136261
  for (const ch of s) h = Math.imul(h ^ ch.charCodeAt(0), 16777619)
  return h >>> 0
}
function fakePreview(seed: number, w: number, h: number) {
  const hue = seed % 360
  const hue2 = (hue + 60) % 360
  const r = Math.min(w, h) / 4
  const dur = 1.2 + (seed % 7) / 5
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 70% 32%)"/><stop offset="1" stop-color="hsl(${hue2} 70% 18%)"/></linearGradient></defs>
<rect width="${w}" height="${h}" fill="url(#g)"/>
<circle cx="${w / 2}" cy="${h / 2}" r="${r}" fill="hsl(${hue2} 90% 70%)" opacity=".9">
<animate attributeName="cy" values="${h / 2 - r / 2};${h / 2 + r / 2};${h / 2 - r / 2}" dur="${dur}s" repeatCount="indefinite"/>
<animate attributeName="r" values="${r};${r * 0.7};${r}" dur="${dur}s" repeatCount="indefinite"/></circle>
<rect x="${w / 2 - r * 1.6}" y="${h - r / 2 - 6}" width="${r * 3.2}" height="4" rx="2" fill="#000" opacity=".25"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}
function fakeGifs(provider: GifProvider, q: string, offset: number): GifPage {
  const total = 60
  const results: GifHit[] = []
  for (let i = offset; i < Math.min(offset + 25, total); i++) {
    const seed = hash(`${provider}:${q}:${i}`)
    const word = q || WORDS[seed % WORDS.length]
    const [w, h] = [[200, 113], [200, 200], [200, 150], [200, 267], [200, 112]][seed % 5]
    const id = seed.toString(36) + i.toString(36)
    const slug = word.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "gif"
    results.push({
      id,
      title: `${word[0].toUpperCase()}${word.slice(1)} ${["GIF", "Reaction", "Loop", "Mood"][seed % 4]}`,
      url: provider === "giphy" ? `https://giphy.com/gifs/${id}` : `https://media.tenor.com/${id}AAAAC/${slug}.gif`,
      preview: fakePreview(seed, w, h),
      width: w,
      height: h,
    })
  }
  return { results, next: offset + 25 < total ? String(offset + 25) : null }
}

function pushLog(level: LogLine["level"], msg: string) {
  const l = { ...makeLog(), level, msg, target: "rackscreen_web::api" }
  state.logs.push(l)
  logSubscribers.forEach((fn) => fn(l))
}

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  })
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

async function handle(method: string, path: string, body: unknown, headers: Headers): Promise<Response> {
  const url = new URL(path, location.origin)
  const p = url.pathname.replace(/^\/api/, "")
  const b = (body ?? {}) as Record<string, unknown>
  if (method !== "GET" && headers.get("X-RackScreen") !== "1") return json(403, { error: "missing X-RackScreen header" })

  if (p === "/auth" && method === "GET")
    return json(200, { required: state.loginRequired, authenticated: !state.loginRequired || state.authenticated })
  if (p === "/auth/login") {
    const ok = b.password === (state.secretValues["web.password"] || "rackscreen")
    if (!ok) return json(401, { error: "Wrong password" })
    state.authenticated = true
    return json(200, { ok: true })
  }
  if (p === "/auth/logout") {
    state.authenticated = false
    return json(200, { ok: true })
  }
  if (state.loginRequired && !state.authenticated) return json(401, { error: "login required" })

  const now = Date.now()
  switch (`${method} ${p}`) {
    case "GET /info":
      return json(200, {
        version: state.update.current,
        hostname: "raspberrypi",
        sim: false,
        started_unix: state.started,
        config_path: "/etc/rackscreen/config.yaml",
        config_writable: true,
        fix_hint: null,
        addresses: ["http://192.168.1.172:8740", "http://raspberrypi.local:8740"],
        screens: state.config.screens.length,
      })
    case "GET /status":
      return json(200, status())
    case "GET /roles":
      return json(200, rolesFor(state.config, secretSet))
    case "GET /shows":
      return json(200, SHOWS)
    case "GET /config":
      return json(200, { config: maskedConfig(), secrets: secretsMap() })
    case "PUT /config":
      return save(b.config as Config, false)
    case "POST /config/validate": {
      let cfg = b.config as Config
      if (typeof b.yaml === "string") {
        try {
          cfg = { ...defaultConfig(), ...(parse(b.yaml) as Config) }
        } catch (e) {
          return json(200, { ok: false, error: `parse yaml: ${(e as Error).message}` })
        }
      }
      const err = validate(cfg)
      return json(200, err ? { ok: false, error: err } : { ok: true })
    }
    case "GET /config/yaml":
      return json(200, { yaml: "# RackScreen configuration (written by rackscreen)\n" + stringify(fullConfig()) })
    case "PUT /config/yaml": {
      let parsed: Config
      try {
        parsed = { ...defaultConfig(), ...(parse(String(b.yaml)) as Config) }
      } catch (e) {
        return json(400, { error: `parse yaml: ${(e as Error).message}` })
      }
      return save(parsed, true)
    }
    case "POST /control/identify":
      state.identifyUntil = now + Number(b.secs ?? 5) * 1000
      return json(200, { ok: true })
    case "POST /control/pattern":
      state.patternUntil = b.on ? now + 600_000 : 0
      return json(200, { ok: true })
    case "POST /control/role": {
      const screen = Number(b.screen)
      const s = state.config.screens[screen]
      if (!s || !s.roles.includes(String(b.role))) return json(400, { error: `screen ${screen + 1} has no role '${b.role}'` })
      state.jumps.set(screen, { role: String(b.role), at: now })
      return json(200, { ok: true })
    }
    case "POST /control/play":
      if (state.night === "on") return json(400, { error: "no shows at night" })
      state.showId = b.show === "random" ? SHOWS[Math.floor(Math.random() * SHOWS.length)].id : String(b.show)
      state.showUntil = now + 20_000
      pushLog("INFO", `shows: playing ${state.showId}`)
      return json(200, { ok: true })
    case "POST /control/night":
      state.night = b.mode as NightMode
      return json(200, { ok: true })
    case "POST /control/restart":
      state.restartingUntil = now + 1500
      state.monitorSince = Math.floor(now / 1000) + 1
      pushLog("INFO", "monitor: restart requested from the web UI")
      return json(200, { ok: true })
    case "GET /logs": {
      const after = Number(url.searchParams.get("after") ?? 0)
      const limit = Math.min(2000, Math.max(1, Number(url.searchParams.get("limit") ?? 500)))
      // Like the service: the OLDEST `limit` lines after `after`; next = last seq sent.
      const lines = state.logs.filter((l) => l.seq > after).slice(0, limit)
      return json(200, { lines, next: lines.at(-1)?.seq ?? after })
    }
    case "GET /spotify":
      return json(200, {
        enabled: state.config.spotify.enabled,
        client_id_set: !!state.config.spotify.client_id,
        logged_in: state.spotifyLoggedIn,
        status: state.spotifyLoggedIn ? "Logged in as Silke" : "Not logged in",
      })
    case "POST /spotify/login":
      return json(200, {
        url: `https://accounts.spotify.com/authorize?client_id=${state.config.spotify.client_id}&response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%3A8888%2Fcallback&code_challenge_method=S256&code_challenge=mockchallenge&state=mockstate&scope=user-read-currently-playing%20user-read-playback-state`,
      })
    case "POST /spotify/callback": {
      const pasted = String(b.pasted ?? "")
      if (!/code=/.test(pasted)) return json(400, { error: "That address has no code in it. Copy the whole address bar after logging in." })
      state.spotifyLoggedIn = true
      return json(200, { ok: true, status: "Logged in as Silke" })
    }
    case "POST /spotify/logout":
      state.spotifyLoggedIn = false
      return json(200, { ok: true })
    case "GET /gifs/providers":
      return json(200, { giphy: secretSet("gif_search.giphy_key"), tenor: secretSet("gif_search.tenor_key") })
    case "GET /gifs/search":
    case "GET /gifs/trending": {
      const provider = url.searchParams.get("provider") as GifProvider
      if (provider !== "giphy" && provider !== "tenor") return json(400, { error: "provider must be giphy or tenor" })
      if (!secretSet(`gif_search.${provider}_key`))
        return json(400, { error: `No ${provider === "giphy" ? "GIPHY" : "Tenor"} API key: add one under GIF search and save` })
      const q = (url.searchParams.get("q") ?? "").trim()
      if (p === "/gifs/search" && (q.length < 1 || q.length > 100)) return json(400, { error: "q must be 1 to 100 characters" })
      return json(200, fakeGifs(provider, p === "/gifs/search" ? q : "", Number(url.searchParams.get("offset") ?? 0)))
    }
    case "GET /gifs/trending-terms":
      return json(200, {
        terms: ["happy", "good morning", "dance", "cat", "thank you", "excited", "love", "party", "wow", "monday"],
      })
    case "GET /update":
      return json(200, state.update)
    case "POST /update": {
      const steps = [
        "Checking https://github.com/SilkePilon/RackScreen/releases/latest",
        `Downloading rackscreen ${state.update.latest} (aarch64)`,
        "Verifying checksum",
        "Installing to /usr/local/bin/rackscreen",
        "Restarting rackscreen@silke",
      ]
      state.update.job = { state: "running", log: [] }
      steps.forEach((s, i) =>
        setTimeout(() => {
          state.update.job.log.push(s)
          if (i === steps.length - 1) {
            state.offline = true
            setTimeout(() => {
              state.offline = false
              state.update = {
                ...state.update,
                current: (state.update.latest ?? "v0.13.1").replace(/^v/, ""),
                newer: false,
                job: { state: "done", log: state.update.job.log },
              }
            }, 3500)
          }
        }, 700 * (i + 1)),
      )
      return json(200, { ok: true })
    }
  }
  return json(404, { error: `no route ${method} ${p}` })
}

// --- fetch -----------------------------------------------------------------
const realFetch = window.fetch.bind(window)
window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
  const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url
  const path = url.startsWith("http") ? new URL(url).pathname + new URL(url).search : url
  if (!path.startsWith("/api")) return realFetch(input, init)
  await sleep(path.startsWith("/api/status") ? 40 : 120 + Math.random() * 260)
  if (state.offline) throw new TypeError("Failed to fetch")
  const method = (init?.method ?? "GET").toUpperCase()
  const body = typeof init?.body === "string" ? JSON.parse(init.body) : undefined
  return handle(method, path, body, new Headers(init?.headers))
}

// --- live frames -------------------------------------------------------------
class MockLive implements LiveSocket {
  binaryType = "arraybuffer"
  onopen: ((ev: Event) => void) | null = null
  onclose: ((ev: CloseEvent) => void) | null = null
  onmessage: ((ev: MessageEvent) => void) | null = null
  private timer: ReturnType<typeof setInterval> | undefined
  private canvas = new OffscreenCanvas(240, 240)
  private ctx = this.canvas.getContext("2d")!
  private busy = false

  constructor() {
    setTimeout(() => {
      if (state.offline) {
        this.onclose?.(new CloseEvent("close"))
        return
      }
      this.onopen?.(new Event("open"))
      this.timer = setInterval(() => void this.tick(), 1000 / 8)
    }, 300)
  }
  send() {}
  close() {
    clearInterval(this.timer)
  }
  private async tick() {
    if (this.busy) return
    if (state.offline) {
      this.close()
      this.onclose?.(new CloseEvent("close"))
      return
    }
    this.busy = true
    const now = Date.now()
    for (let i = 0; i < state.config.screens.length; i++) {
      const n = screenNow(i, now)
      drawFrame(this.ctx, { screen: i, role: n.role, title: titleOf(n.role), t: now / 1000, showing: n.showing })
      const blob = await this.canvas.convertToBlob({ type: "image/png" })
      const png = new Uint8Array(await blob.arrayBuffer())
      const msg = new Uint8Array(png.length + 1)
      msg[0] = i
      msg.set(png, 1)
      this.onmessage?.(new MessageEvent("message", { data: msg.buffer }))
    }
    this.busy = false
  }
}
setLiveFactory(() => new MockLive())

// --- log stream --------------------------------------------------------------
const logSubscribers = new Set<(l: LogLine) => void>()
setInterval(() => {
  const l = makeLog()
  state.logs.push(l)
  if (state.logs.length > 2000) state.logs.splice(0, state.logs.length - 2000)
  logSubscribers.forEach((fn) => fn(l))
}, 900)

class MockLogSource implements LogSource {
  onerror: ((ev: Event) => void) | null = null
  onopen: ((ev: Event) => void) | null = null
  private fns: ((ev: MessageEvent) => void)[] = []
  private sub = (l: LogLine) => {
    if (state.offline) return
    this.fns.forEach((fn) => fn(new MessageEvent("log", { data: JSON.stringify(l) })))
  }
  constructor() {
    logSubscribers.add(this.sub)
    setTimeout(() => this.onopen?.(new Event("open")), 50)
  }
  addEventListener(type: string, fn: (ev: MessageEvent) => void) {
    if (type === "log") this.fns.push(fn)
  }
  close() {
    logSubscribers.delete(this.sub)
  }
}
setLogStreamFactory(() => new MockLogSource())

Object.assign(window, {
  __mock: {
    state,
    offline: (v: boolean) => (state.offline = v),
    login: (v: boolean) => {
      state.loginRequired = v
      state.authenticated = false
    },
  },
})
console.info("%cRackScreen mock API active", "color:#3a72ff")

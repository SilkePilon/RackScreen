// Live panel frames over the /api/live WebSocket: binary messages, byte 0 =
// screen index, the rest a PNG of the 240x240 unrotated frame. One shared
// connection, opened while at least one mirror is mounted. It asks for 10 fps
// while a large mirror is visible and 5 fps for thumbnails or a hidden tab.

export interface LiveSocket {
  binaryType: string
  onopen: ((ev: Event) => void) | null
  onclose: ((ev: CloseEvent) => void) | null
  onmessage: ((ev: MessageEvent) => void) | null
  send: (data: string) => void
  close: () => void
}

type Factory = () => LiveSocket
type Listener = (bmp: ImageBitmap) => void

let factory: Factory = () => {
  const proto = location.protocol === "https:" ? "wss" : "ws"
  return new WebSocket(`${proto}://${location.host}/api/live`) as unknown as LiveSocket
}

/** The mock API swaps in a synthetic frame source. */
export function setLiveFactory(f: Factory) {
  factory = f
}

const frames = new Map<number, ImageBitmap>()
const listeners = new Map<number, Set<Listener>>()
const stateListeners = new Set<(connected: boolean) => void>()
let socket: LiveSocket | null = null
let users = 0
/** Subscribers that show a large mirror; with none, thumbnails get a lower rate. */
let bigUsers = 0
let retry = 0
let retryTimer: ReturnType<typeof setTimeout> | undefined
let lingerTimer: ReturnType<typeof setTimeout> | undefined
let connected = false
let sentFps = 0
/** Frames for screen indexes at or above this are dropped (from /api/status). */
let screenCount = Infinity
// Per screen: the sequence number of the newest message received and of the
// newest frame shown, so a slow decode never overwrites a newer frame.
const received = new Map<number, number>()
const shown = new Map<number, number>()

function setConnected(v: boolean) {
  connected = v
  stateListeners.forEach((fn) => fn(v))
}

function wantedFps() {
  if (typeof document !== "undefined" && document.visibilityState === "hidden") return 5
  return bigUsers > 0 ? 10 : 5
}

function syncFps() {
  if (!socket || !connected) return
  const fps = wantedFps()
  if (fps === sentFps) return
  sentFps = fps
  try {
    socket.send(JSON.stringify({ fps }))
  } catch {
    // The socket is closing; onclose reconnects.
  }
}

if (typeof document !== "undefined") document.addEventListener("visibilitychange", syncFps)

function dropScreen(screen: number) {
  frames.get(screen)?.close()
  frames.delete(screen)
  received.delete(screen)
  shown.delete(screen)
}

/** Tell the live view how many screens exist; frames for others are dropped. */
export function setLiveScreenCount(n: number) {
  screenCount = n
  for (const i of [...frames.keys()]) if (i >= n) dropScreen(i)
}

function open() {
  clearTimeout(retryTimer)
  retryTimer = undefined
  const ws = factory()
  socket = ws
  sentFps = 0
  ws.binaryType = "arraybuffer"
  ws.onopen = () => {
    if (socket !== ws) return
    retry = 0
    setConnected(true)
    syncFps()
  }
  ws.onmessage = async (ev) => {
    if (socket !== ws) return
    if (!(ev.data instanceof ArrayBuffer) || ev.data.byteLength < 2) return
    const view = new Uint8Array(ev.data)
    const screen = view[0]
    if (screen >= screenCount) return
    const seq = (received.get(screen) ?? 0) + 1
    received.set(screen, seq)
    let bmp: ImageBitmap
    try {
      bmp = await createImageBitmap(new Blob([view.subarray(1)], { type: "image/png" }))
    } catch {
      // A broken frame: keep the previous one.
      return
    }
    // Closed meanwhile, the screen went away, or a newer frame already won.
    if (socket !== ws || screen >= screenCount || seq <= (shown.get(screen) ?? 0)) {
      bmp.close()
      return
    }
    shown.set(screen, seq)
    frames.get(screen)?.close()
    frames.set(screen, bmp)
    listeners.get(screen)?.forEach((fn) => fn(bmp))
  }
  ws.onclose = () => {
    if (socket !== ws) return
    socket = null
    setConnected(false)
    clearTimeout(retryTimer)
    retryTimer = undefined
    if (users > 0) {
      retry = Math.min(retry + 1, 6)
      retryTimer = setTimeout(() => {
        retryTimer = undefined
        if (users > 0 && !socket) open()
      }, 500 * 2 ** retry)
    }
  }
}

function shutdown() {
  clearTimeout(retryTimer)
  retryTimer = undefined
  if (socket) {
    const s = socket
    socket = null
    s.close()
    setConnected(false)
  }
}

export function subscribeFrames(screen: number, fn: Listener, opts: { thumb?: boolean } = {}): () => void {
  let set = listeners.get(screen)
  if (!set) listeners.set(screen, (set = new Set()))
  set.add(fn)
  const last = frames.get(screen)
  if (last) fn(last)
  users++
  if (!opts.thumb) bigUsers++
  clearTimeout(lingerTimer)
  lingerTimer = undefined
  if (!socket && !retryTimer) open()
  else syncFps()
  let done = false
  return () => {
    if (done) return
    done = true
    set.delete(fn)
    users--
    if (!opts.thumb) bigUsers--
    syncFps()
    if (users === 0) {
      // Linger briefly so route changes don't reconnect.
      clearTimeout(lingerTimer)
      lingerTimer = setTimeout(() => {
        lingerTimer = undefined
        if (users === 0) shutdown()
      }, 3000)
    }
  }
}

export function subscribeLiveState(fn: (connected: boolean) => void) {
  stateListeners.add(fn)
  fn(connected)
  return () => {
    stateListeners.delete(fn)
  }
}

// Logs: Server-Sent Events, event "log".
export interface LogSource {
  addEventListener: (type: string, fn: (ev: MessageEvent) => void) => void
  onerror: ((ev: Event) => void) | null
  onopen: ((ev: Event) => void) | null
  close: () => void
}
let logFactory: () => LogSource = () =>
  new EventSource("/api/logs/stream", { withCredentials: true }) as unknown as LogSource
export function setLogStreamFactory(f: () => LogSource) {
  logFactory = f
}
export function openLogStream(): LogSource {
  return logFactory()
}

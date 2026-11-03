// Live panel frames over the /api/live WebSocket: binary messages, byte 0 =
// screen index, the rest a PNG of the 240x240 unrotated frame. One shared
// connection, opened while at least one mirror is mounted.

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
let retry = 0
let retryTimer: ReturnType<typeof setTimeout> | undefined
let connected = false

function setConnected(v: boolean) {
  connected = v
  stateListeners.forEach((fn) => fn(v))
}

function open() {
  clearTimeout(retryTimer)
  const ws = factory()
  socket = ws
  ws.binaryType = "arraybuffer"
  ws.onopen = () => {
    retry = 0
    setConnected(true)
    ws.send(JSON.stringify({ fps: 10 }))
  }
  ws.onmessage = async (ev) => {
    if (!(ev.data instanceof ArrayBuffer) || ev.data.byteLength < 2) return
    const view = new Uint8Array(ev.data)
    const screen = view[0]
    try {
      const bmp = await createImageBitmap(new Blob([view.subarray(1)], { type: "image/png" }))
      frames.get(screen)?.close()
      frames.set(screen, bmp)
      listeners.get(screen)?.forEach((fn) => fn(bmp))
    } catch {
      // A broken frame: keep the previous one.
    }
  }
  ws.onclose = () => {
    if (socket !== ws) return
    socket = null
    setConnected(false)
    if (users > 0) {
      retry = Math.min(retry + 1, 6)
      retryTimer = setTimeout(open, 500 * 2 ** retry)
    }
  }
}

export function subscribeFrames(screen: number, fn: Listener): () => void {
  let set = listeners.get(screen)
  if (!set) listeners.set(screen, (set = new Set()))
  set.add(fn)
  const last = frames.get(screen)
  if (last) fn(last)
  users++
  if (!socket) open()
  return () => {
    set.delete(fn)
    users--
    if (users === 0) {
      // Linger briefly so route changes don't reconnect.
      setTimeout(() => {
        if (users === 0 && socket) {
          const s = socket
          socket = null
          s.close()
          setConnected(false)
        }
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

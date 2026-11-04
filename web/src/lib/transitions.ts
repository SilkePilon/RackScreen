// The transition styles the panels can play, and a port of the service's
// compositing (crates/core/src/transition.rs) so the previews on the
// Display page show the real look at a small size.

export type TransitionStyle =
  | "fade-black"
  | "crossfade"
  | "cut"
  | "slide"
  | "cover"
  | "wipe"
  | "iris"
  | "clock"
  | "dissolve"
  | "zoom"
  | "blinds"

export type TransitionDirection =
  | "left"
  | "right"
  | "up"
  | "down"
  | "in"
  | "out"
  | "cw"
  | "ccw"
  | "horizontal"
  | "vertical"

export interface StyleInfo {
  id: TransitionStyle
  title: string
  blurb: string
  directions: TransitionDirection[]
  defaultDirection: TransitionDirection
  /** Changes every pixel every frame: a full SPI push per frame. */
  heavy: boolean
}

export const STYLES: StyleInfo[] = [
  { id: "fade-black", title: "Fade", blurb: "Dims to black, then up.", directions: [], defaultDirection: "left", heavy: false },
  { id: "crossfade", title: "Crossfade", blurb: "Blends into the next one.", directions: [], defaultDirection: "left", heavy: true },
  { id: "cut", title: "Cut", blurb: "No animation.", directions: [], defaultDirection: "left", heavy: false },
  { id: "slide", title: "Slide", blurb: "Pushes the old one out.", directions: ["left", "right", "up", "down"], defaultDirection: "left", heavy: true },
  { id: "cover", title: "Cover", blurb: "Slides in over the old one.", directions: ["left", "right", "up", "down"], defaultDirection: "left", heavy: true },
  { id: "wipe", title: "Wipe", blurb: "A soft edge sweeps across.", directions: ["left", "right", "up", "down"], defaultDirection: "left", heavy: false },
  { id: "iris", title: "Iris", blurb: "A circle opens, or closes.", directions: ["in", "out"], defaultDirection: "out", heavy: false },
  { id: "clock", title: "Clock", blurb: "Sweeps round from twelve.", directions: ["cw", "ccw"], defaultDirection: "cw", heavy: false },
  { id: "dissolve", title: "Dissolve", blurb: "Melts over in soft patches.", directions: [], defaultDirection: "left", heavy: true },
  { id: "zoom", title: "Zoom", blurb: "Scales away, scales in.", directions: ["in", "out"], defaultDirection: "in", heavy: true },
  { id: "blinds", title: "Blinds", blurb: "Strips reveal the next one.", directions: ["horizontal", "vertical"], defaultDirection: "horizontal", heavy: false },
]

export const DIRECTION_LABELS: Record<TransitionDirection, { label: string; aria: string }> = {
  left: { label: "←", aria: "Left" },
  right: { label: "→", aria: "Right" },
  up: { label: "↑", aria: "Up" },
  down: { label: "↓", aria: "Down" },
  in: { label: "In", aria: "In" },
  out: { label: "Out", aria: "Out" },
  cw: { label: "↻", aria: "Clockwise" },
  ccw: { label: "↺", aria: "Counter-clockwise" },
  horizontal: { label: "Horizontal", aria: "Horizontal strips" },
  vertical: { label: "Vertical", aria: "Vertical strips" },
}

export const MIN_SECS = 0.15
export const MAX_SECS = 2

export function styleInfo(id: string): StyleInfo {
  return STYLES.find((s) => s.id === id) ?? STYLES[0]
}

/** The direction a style plays in: `dir` when it is one of its own, else its default. */
export function effectiveDirection(style: string, dir: string | null | undefined): TransitionDirection {
  const s = styleInfo(style)
  if (s.directions.length === 0) return s.defaultDirection
  return s.directions.includes(dir as TransitionDirection) ? (dir as TransitionDirection) : s.defaultDirection
}

export function formatSecs(secs: number): string {
  return `${secs.toFixed(2).replace(/0$/, "")} s`
}

// ---------------------------------------------------------------------------
// Compositing, like the service's: eased, soft edges, the same constants.

const easeInOutCubic = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)

const BLIND_COUNT = 8
const BLIND_STAGGER = 0.25
const DISSOLVE_SOFT = 0.35
const COVER_SHADOW = 0.45

interface Tables {
  w: number
  soft: number
  ang: Float32Array
  rad: Float32Array
  noise: Float32Array
}

const tables = new Map<number, Tables>()

function tablesFor(w: number): Tables {
  let t = tables.get(w)
  if (t) return t
  const n = w * w
  const c = w / 2
  const ang = new Float32Array(n)
  const rad = new Float32Array(n)
  for (let i = 0; i < n; i++) {
    const dx = (i % w) + 0.5 - c
    const dy = Math.floor(i / w) + 0.5 - c
    let a = Math.atan2(dx, -dy)
    if (a < 0) a += Math.PI * 2
    ang[i] = a
    rad[i] = Math.sqrt(dx * dx + dy * dy)
  }
  // soft patches with grain, like the service (a different seed is fine)
  let x = 0x9e3779b9
  const rnd = () => {
    x ^= x << 13
    x ^= x >>> 17
    x ^= x << 5
    return (x >>> 0) / 4294967296
  }
  const cell = Math.max(2, Math.round((w * 20) / 240))
  const cells = Math.ceil(w / cell) + 1
  const grid = new Float32Array(cells * cells)
  for (let i = 0; i < grid.length; i++) grid[i] = rnd()
  const noise = new Float32Array(n)
  for (let y = 0; y < w; y++) {
    const cy = Math.floor(y / cell)
    const fy = (y % cell) / cell
    for (let xx = 0; xx < w; xx++) {
      const cx = Math.floor(xx / cell)
      const fx = (xx % cell) / cell
      const g = (i: number, j: number) => grid[j * cells + i]
      const top = g(cx, cy) * (1 - fx) + g(cx + 1, cy) * fx
      const bottom = g(cx, cy + 1) * (1 - fx) + g(cx + 1, cy + 1) * fx
      const smooth = top * (1 - fy) + bottom * fy
      noise[y * w + xx] = smooth * 0.65 + rnd() * 0.35
    }
  }
  t = { w, soft: Math.max(1, (2 * w) / 240), ang, rad, noise }
  tables.set(w, t)
  return t
}

type Frame = Uint8ClampedArray

function mixPx(f: Frame, t: Frame, o: Frame, i: number, a: number) {
  const b = 1 - a
  const j = i * 4
  o[j] = f[j] * b + t[j] * a
  o[j + 1] = f[j + 1] * b + t[j + 1] * a
  o[j + 2] = f[j + 2] * b + t[j + 2] * a
  o[j + 3] = 255
}

function copyPx(src: Frame, o: Frame, si: number, di: number) {
  const s = si * 4
  const d = di * 4
  o[d] = src[s]
  o[d + 1] = src[s + 1]
  o[d + 2] = src[s + 2]
  o[d + 3] = 255
}

function blendMap(w: number, f: Frame, t: Frame, o: Frame, cov: (x: number, y: number) => number) {
  for (let y = 0; y < w; y++)
    for (let x = 0; x < w; x++) {
      const i = y * w + x
      const a = cov(x, y)
      if (a <= 0) copyPx(f, o, i, i)
      else if (a >= 1) copyPx(t, o, i, i)
      else mixPx(f, t, o, i, a)
    }
}

const edge = (d: number, soft: number) => Math.min(1, Math.max(0, d / soft + 0.5))

function slide(w: number, f: Frame, t: Frame, o: Frame, e: number, dir: string) {
  const d = Math.min(w, Math.round(e * w))
  for (let y = 0; y < w; y++)
    for (let x = 0; x < w; x++) {
      const i = y * w + x
      switch (dir) {
        case "left":
          if (x + d < w) copyPx(f, o, i + d, i)
          else copyPx(t, o, i + d - w, i)
          break
        case "right":
          if (x >= d) copyPx(f, o, i - d, i)
          else copyPx(t, o, i + w - d, i)
          break
        case "up":
          if (y + d < w) copyPx(f, o, i + d * w, i)
          else copyPx(t, o, i + (d - w) * w, i)
          break
        default:
          if (y >= d) copyPx(f, o, i - d * w, i)
          else copyPx(t, o, i + (w - d) * w, i)
      }
    }
}

function cover(w: number, f: Frame, t: Frame, o: Frame, e: number, dir: string, shadowPx: number) {
  const d = Math.min(w, Math.round(e * w))
  for (let y = 0; y < w; y++)
    for (let x = 0; x < w; x++) {
      const i = y * w + x
      // distance from this pixel to the new frame's leading edge, on the old side
      let covered: boolean
      let ahead: number
      switch (dir) {
        case "left":
          covered = x >= w - d
          ahead = w - d - 1 - x
          break
        case "right":
          covered = x < d
          ahead = x - d
          break
        case "up":
          covered = y >= w - d
          ahead = w - d - 1 - y
          break
        default:
          covered = y < d
          ahead = y - d
      }
      if (covered) {
        const si =
          dir === "left" ? i - (w - d) : dir === "right" ? i + (w - d) : dir === "up" ? i - (w - d) * w : i + (w - d) * w
        copyPx(t, o, si, i)
      } else {
        copyPx(f, o, i, i)
        if (d > 0 && ahead >= 0 && ahead < shadowPx) {
          const k = 1 - COVER_SHADOW * (1 - ahead / shadowPx)
          const j = i * 4
          o[j] *= k
          o[j + 1] *= k
          o[j + 2] *= k
        }
      }
    }
}

/** Bilinear sample of `src` scaled by `s` about the centre at output pixel (x, y). */
function sample(src: Frame, w: number, s: number, x: number, y: number, out: number[]) {
  const c = w / 2
  const sx = c + (x + 0.5 - c) / s - 0.5
  const sy = c + (y + 0.5 - c) / s - 0.5
  const xi = Math.floor(sx)
  const yi = Math.floor(sy)
  const fx = sx - xi
  const fy = sy - yi
  const tap = (px: number, py: number, ch: number) =>
    px < 0 || py < 0 || px >= w || py >= w ? 0 : src[(py * w + px) * 4 + ch]
  for (let ch = 0; ch < 3; ch++) {
    out[ch] =
      tap(xi, yi, ch) * (1 - fx) * (1 - fy) +
      tap(xi + 1, yi, ch) * fx * (1 - fy) +
      tap(xi, yi + 1, ch) * (1 - fx) * fy +
      tap(xi + 1, yi + 1, ch) * fx * fy
  }
}

/**
 * Write the frame between `from` and `to` at `t` (0 = `from`, 1 = `to`) into
 * `out`; all three are `w`×`w` RGBA.
 */
export function composite(
  style: string,
  dir: string,
  from: Frame,
  to: Frame,
  out: Frame,
  t: number,
  w: number,
) {
  if (t <= 0) {
    out.set(from)
    return
  }
  if (t >= 1) {
    out.set(to)
    return
  }
  const e = easeInOutCubic(t)
  const T = tablesFor(w)
  const soft = T.soft
  switch (style) {
    case "slide":
      slide(w, from, to, out, e, dir)
      return
    case "cover":
      cover(w, from, to, out, e, dir, Math.max(2, Math.round((10 * w) / 240)))
      return
    case "wipe": {
      const p = e * (w + 2 * soft) - soft
      const along = (v: number) => (dir === "left" || dir === "up" ? v + 0.5 - (w - p) : p - (v + 0.5))
      if (dir === "left" || dir === "right") blendMap(w, from, to, out, (x) => edge(along(x), soft))
      else blendMap(w, from, to, out, (_x, y) => edge(along(y), soft))
      return
    }
    case "iris": {
      const rim = w / 2 + 0.5
      const full = w / 2 + soft
      const r = dir === "out" ? e * full : (1 - e) * full
      blendMap(w, from, to, out, (x, y) => {
        const dist = Math.min(T.rad[y * w + x], rim)
        return edge(dir === "out" ? r - dist : dist - r, soft)
      })
      return
    }
    case "clock": {
      const s = e * Math.PI * 2
      blendMap(w, from, to, out, (x, y) => {
        const i = y * w + x
        let a = T.ang[i]
        if (dir === "ccw" && a > 0) a = Math.PI * 2 - a
        return edge((s - a) * T.rad[i], soft)
      })
      return
    }
    case "dissolve": {
      const lead = e * (1 + DISSOLVE_SOFT)
      blendMap(w, from, to, out, (x, y) => (lead - T.noise[y * w + x]) / DISSOLVE_SOFT)
      return
    }
    case "blinds": {
      const px = w / BLIND_COUNT
      const span = px + 2 * soft
      const cov = (v: number) => {
        const strip = Math.min(BLIND_COUNT - 1, Math.floor(v / px))
        const local = v - strip * px + 0.5
        const start = (strip * BLIND_STAGGER) / (BLIND_COUNT - 1)
        const p = Math.min(1, Math.max(0, (e - start) / (1 - BLIND_STAGGER)))
        return edge(p * span - soft - local, soft)
      }
      if (dir === "vertical") blendMap(w, from, to, out, (x) => cov(x))
      else blendMap(w, from, to, out, (_x, y) => cov(y))
      return
    }
    case "zoom": {
      const [sf, st] = dir === "in" ? [1 + 0.2 * e, 0.85 + 0.15 * e] : [1 - 0.15 * e, 1.2 - 0.2 * e]
      const pf = [0, 0, 0]
      const pt = [0, 0, 0]
      for (let y = 0; y < w; y++)
        for (let x = 0; x < w; x++) {
          sample(from, w, sf, x, y, pf)
          sample(to, w, st, x, y, pt)
          const j = (y * w + x) * 4
          for (let ch = 0; ch < 3; ch++) out[j + ch] = pf[ch] * (1 - e) + pt[ch] * e
          out[j + 3] = 255
        }
      return
    }
    case "cut":
      out.set(t < 0.5 ? from : to)
      return
    case "fade-black": {
      // dim out, then up: the single-scene fade the service plays
      const k = t < 0.5 ? 1 - easeInOutCubic(2 * t) : easeInOutCubic(2 * t - 1)
      const src = t < 0.5 ? from : to
      for (let j = 0; j < out.length; j += 4) {
        out[j] = src[j] * k
        out[j + 1] = src[j + 1] * k
        out[j + 2] = src[j + 2] * k
        out[j + 3] = 255
      }
      return
    }
    default: {
      // crossfade
      for (let j = 0; j < out.length; j += 4) {
        out[j] = from[j] * (1 - e) + to[j] * e
        out[j + 1] = from[j + 1] * (1 - e) + to[j + 1] * e
        out[j + 2] = from[j + 2] * (1 - e) + to[j + 2] * e
        out[j + 3] = 255
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Two sample screens for the previews: a blue ring with "62" and an amber
// one with "47", in the panels' own layout.

export type Sample = "a" | "b"

const samples = new Map<string, Frame>()

export function sampleFrame(which: Sample, w: number): Frame {
  const key = `${which}:${w}`
  const hit = samples.get(key)
  if (hit) return hit
  const canvas = document.createElement("canvas")
  canvas.width = w
  canvas.height = w
  const ctx = canvas.getContext("2d")
  if (!ctx) return new Uint8ClampedArray(w * w * 4)
  const u = w / 240
  const c = w / 2
  ctx.fillStyle = "#000"
  ctx.fillRect(0, 0, w, w)
  // ring track and fill, like the P2 screens (anti-clockwise gap at the bottom)
  const r = 106 * u
  ctx.lineWidth = 9 * u
  ctx.lineCap = "round"
  const start = Math.PI * 0.5 + Math.PI / 6
  const sweep = Math.PI * 2 - Math.PI / 3
  ctx.strokeStyle = "#2c2c33"
  ctx.beginPath()
  ctx.arc(c, c, r, start, start + sweep)
  ctx.stroke()
  const fill = which === "a" ? 0.62 : 0.47
  ctx.strokeStyle = which === "a" ? "#5b8cff" : "#f0a830"
  ctx.beginPath()
  ctx.arc(c, c, r, start, start + sweep * fill)
  ctx.stroke()
  // the value
  ctx.fillStyle = "#ededef"
  ctx.font = `700 ${Math.round(74 * u)}px Inter, system-ui, sans-serif`
  ctx.textAlign = "center"
  ctx.textBaseline = "middle"
  ctx.fillText(which === "a" ? "62" : "47", c - 4 * u, c - 26 * u)
  // the pill
  const pw = 92 * u
  const ph = 22 * u
  ctx.fillStyle = "#24242a"
  ctx.beginPath()
  ctx.roundRect(c - pw / 2, c + 22 * u, pw, ph, ph / 2)
  ctx.fill()
  // the card
  const cw = 108 * u
  const ch = 48 * u
  ctx.beginPath()
  ctx.roundRect(c - cw / 2, c + 50 * u, cw, ch, 14 * u)
  ctx.fill()
  ctx.strokeStyle = which === "a" ? "#5b8cff" : "#f0a830"
  ctx.lineWidth = 2 * u
  ctx.beginPath()
  for (let i = 0; i <= 12; i++) {
    const x = c - cw / 2 + 12 * u + (i / 12) * (cw - 24 * u)
    const y = c + 74 * u + Math.sin(i * 0.9 + (which === "a" ? 0 : 2)) * 8 * u
    if (i === 0) ctx.moveTo(x, y)
    else ctx.lineTo(x, y)
  }
  ctx.stroke()
  const data = ctx.getImageData(0, 0, w, w).data
  samples.set(key, data)
  return data
}

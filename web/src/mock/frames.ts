// Synthetic 240x240 panel frames in the P2 "Tiles" palette, so the live
// mirrors animate without a Pi.

const P = {
  bg: "#000000",
  text: "#EDEDEF",
  text2: "#8E8E93",
  track: "#2C2C33",
  ok: "#5BD18B",
  warn: "#F2B84B",
  crit: "#F0605D",
  blue: "#6EA8FE",
  violet: "#A48BFA",
  teal: "#5CC8C3",
  cyan: "#67D1EE",
  orange: "#F2A65A",
  moon: "#D6D6E0",
}

export interface FrameInput {
  screen: number
  role: string
  title: string
  t: number
  showing: "role" | "show" | "night" | "pattern" | "identify"
  show?: string
}

const ACCENT: Record<string, string> = {
  cpu: P.blue,
  mem: P.violet,
  pods: P.teal,
  health: P.ok,
  "power-mix": P.warn,
  renewable: P.ok,
  weather: P.cyan,
  music: P.orange,
  "gh-activity": P.ok,
}

function ring(ctx: OffscreenCanvasRenderingContext2D, frac: number, color: string, r = 104, w = 12) {
  ctx.lineCap = "round"
  ctx.lineWidth = w
  ctx.strokeStyle = P.track
  ctx.beginPath()
  ctx.arc(120, 120, r, Math.PI * 0.75, Math.PI * 2.25)
  ctx.stroke()
  ctx.strokeStyle = color
  ctx.beginPath()
  ctx.arc(120, 120, r, Math.PI * 0.75, Math.PI * 0.75 + Math.PI * 1.5 * Math.max(0.01, frac))
  ctx.stroke()
}

function text(ctx: OffscreenCanvasRenderingContext2D, s: string, y: number, size: number, color: string, weight = 600) {
  ctx.fillStyle = color
  ctx.font = `${weight} ${size}px Inter Variable, system-ui, sans-serif`
  ctx.textAlign = "center"
  ctx.textBaseline = "middle"
  ctx.fillText(s, 120, y)
}

export function drawFrame(ctx: OffscreenCanvasRenderingContext2D, f: FrameInput) {
  const { t, screen } = f
  ctx.fillStyle = P.bg
  ctx.fillRect(0, 0, 240, 240)

  if (f.showing === "identify" || f.showing === "pattern") {
    const colors = [P.crit, P.ok, P.blue, P.warn, P.violet, P.teal]
    for (let i = 0; i < 6; i++) {
      ctx.fillStyle = colors[i]
      ctx.fillRect(i * 40, 0, 40, 24)
    }
    ctx.strokeStyle = P.text2
    ctx.lineWidth = 1
    for (let i = 20; i < 240; i += 20) {
      ctx.beginPath()
      ctx.moveTo(i, 24)
      ctx.lineTo(i, 240)
      ctx.moveTo(0, i)
      ctx.lineTo(240, i)
      ctx.stroke()
    }
    ctx.strokeStyle = P.text
    ctx.lineWidth = 3
    ctx.beginPath()
    ctx.arc(120, 120, 116, 0, Math.PI * 2)
    ctx.stroke()
    // Arrow pointing up: shows the panel's top.
    ctx.fillStyle = P.warn
    ctx.beginPath()
    ctx.moveTo(120, 34)
    ctx.lineTo(106, 56)
    ctx.lineTo(134, 56)
    ctx.fill()
    ctx.fillStyle = "rgba(0,0,0,.75)"
    ctx.beginPath()
    ctx.arc(120, 128, 48, 0, Math.PI * 2)
    ctx.fill()
    text(ctx, String(screen + 1), 130, 72, P.text, 700)
    return
  }

  if (f.showing === "night") {
    ctx.fillStyle = "#08080c"
    ctx.fillRect(0, 0, 240, 240)
    ctx.fillStyle = "#3a3a44"
    ctx.beginPath()
    ctx.arc(120, 120, 34, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = "#08080c"
    ctx.beginPath()
    ctx.arc(134, 110, 30, 0, Math.PI * 2)
    ctx.fill()
    return
  }

  if (f.showing === "show") {
    // A wide sunset scene slid across the four panels.
    const g = ctx.createLinearGradient(0, 0, 0, 240)
    g.addColorStop(0, "#1b1340")
    g.addColorStop(0.55, "#d9575b")
    g.addColorStop(0.75, "#f2a65a")
    g.addColorStop(1, "#2a1a2e")
    ctx.fillStyle = g
    ctx.fillRect(0, 0, 240, 240)
    const x = ((t * 40 - screen * 240) % 1200) - 120
    ctx.fillStyle = "#120c18"
    ctx.fillRect(0, 186, 240, 54)
    for (let i = 0; i < 5; i++) {
      ctx.fillStyle = i === 0 ? "#2b2233" : "#1e1726"
      ctx.fillRect(x - i * 52, 160, 48, 26)
      ctx.fillStyle = "#f2b84b"
      ctx.fillRect(x - i * 52 + 6, 166, 8, 6)
    }
    return
  }

  const accent = ACCENT[f.role] ?? (f.role.startsWith("gif:") ? P.orange : P.blue)
  const wobble = (Math.sin(t * 0.7 + screen * 1.7) + 1) / 2
  if (f.role.startsWith("gif:")) {
    for (let i = 0; i < 6; i++) {
      const a = t * 1.5 + (i * Math.PI) / 3
      ctx.fillStyle = [P.crit, P.warn, P.ok, P.cyan, P.blue, P.violet][i]
      ctx.beginPath()
      ctx.arc(120 + Math.cos(a) * 60, 120 + Math.sin(a) * 60, 18 + 6 * Math.sin(t * 3 + i), 0, Math.PI * 2)
      ctx.fill()
    }
    text(ctx, f.role.slice(4), 120, 22, P.text, 600)
    return
  }
  if (f.role === "power-mix") {
    const parts = [0.34, 0.22, 0.18, 0.26]
    const cols = [P.ok, P.cyan, P.warn, P.text2]
    let a = -Math.PI / 2 + t * 0.05
    ctx.lineWidth = 16
    for (let i = 0; i < parts.length; i++) {
      ctx.strokeStyle = cols[i]
      ctx.beginPath()
      ctx.arc(120, 120, 100, a + 0.04, a + parts[i] * Math.PI * 2 - 0.04)
      ctx.stroke()
      a += parts[i] * Math.PI * 2
    }
    text(ctx, "POWER MIX", 82, 13, P.text2, 600)
    text(ctx, "61", 124, 64, P.text, 650)
    text(ctx, "% renewable", 170, 14, P.text2, 500)
    return
  }
  const value =
    f.role === "pods" ? 112 + Math.round(wobble * 3) : f.role === "health" ? 7 : Math.round(18 + wobble * 30)
  const unit = f.role === "pods" ? "running" : f.role === "health" ? "nodes ready" : "%"
  const frac = f.role === "pods" ? 0.72 : f.role === "health" ? 1 : value / 100
  ring(ctx, frac, accent)
  // Little satellite dot riding the ring.
  const ang = Math.PI * 0.75 + Math.PI * 1.5 * frac
  ctx.fillStyle = P.text
  ctx.beginPath()
  ctx.arc(120 + Math.cos(ang) * 104, 120 + Math.sin(ang) * 104, 3, 0, Math.PI * 2)
  ctx.fill()
  text(ctx, f.title, 80, 13, P.text2, 600)
  text(ctx, String(value), 124, 64, P.text, 650)
  text(ctx, unit, 170, 14, P.text2, 500)
}

import * as React from "react"

import { subscribeFrames } from "@/lib/live"
import { cn } from "@/lib/utils"

/**
 * One round panel, mirrored live from /api/live: a 240x240 canvas clipped to
 * a circle inside a soft bezel. Optionally a thin cycle-progress arc rides
 * the bezel, interpolated between status polls.
 */
export function PanelMirror({
  screen,
  className,
  cycle,
  label,
  dim,
  thumb,
}: {
  screen: number
  className?: string
  /** The screen's role cycle, drawn as a thin arc on the bezel. */
  cycle?: Cycle
  label?: string
  dim?: boolean
  /** A small thumbnail: the live view may send it fewer frames. */
  thumb?: boolean
}) {
  const canvas = React.useRef<HTMLCanvasElement>(null)
  const [hasFrame, setHasFrame] = React.useState(false)

  React.useEffect(() => {
    const ctx = canvas.current?.getContext("2d")
    if (!ctx) return
    return subscribeFrames(
      screen,
      (bmp) => {
        ctx.drawImage(bmp, 0, 0, 240, 240)
        setHasFrame(true)
      },
      { thumb }
    )
  }, [screen, thumb])

  return (
    <div
      className={cn(
        "relative aspect-square w-full rounded-full p-[5%]",
        "bg-[radial-gradient(circle_at_30%_20%,#2a2a2e,#0c0c0e_60%)] shadow-[inset_0_1px_0_rgba(255,255,255,.08),0_10px_30px_-10px_rgba(0,0,0,.8)]",
        className
      )}
      role="img"
      aria-label={label ?? `Screen ${screen + 1} live view`}
    >
      {cycle && <CycleRing {...cycle} />}
      <div className="relative size-full overflow-hidden rounded-full bg-black ring-1 ring-black">
        <canvas
          ref={canvas}
          width={240}
          height={240}
          className={cn(
            "size-full transition-opacity duration-500",
            hasFrame ? "opacity-100" : "opacity-0",
            dim && "opacity-40"
          )}
        />
        {!hasFrame && (
          <div className="absolute inset-0 animate-pulse rounded-full bg-[radial-gradient(circle,#18181b,#050505_70%)]" />
        )}
        {/* Glass: a faint highlight like the real panel cover. */}
        <div className="pointer-events-none absolute inset-0 rounded-full bg-[linear-gradient(160deg,rgba(255,255,255,.07),transparent_40%)]" />
      </div>
    </div>
  )
}

export interface Cycle {
  /** False while a show, night or the test pattern has the panel, or with one role. */
  active: boolean
  role: string
  /** 0..1 of the current dwell when the Pi sampled it. */
  progress: number
  cycleSecs: number
  /** `performance.now()` time the sample was taken on the Pi (receipt minus its age). */
  at: number
}

const R = 48.5
const C = 2 * Math.PI * R
/** How far a fresh sample may disagree before the clock follows it, ms. */
const SNAP_MS = 1500
const DRAIN_MS = 520
const FINISH_MS = 220
const GROW_MS = 600

const easeOut = (x: number) => 1 - (1 - x) ** 3
const easeInOut = (x: number) => (x < 0.5 ? 2 * x * x : 1 - (-2 * x + 2) ** 2 / 2)

/** Draw the arc from `tail` to `head` (0..1, clockwise from 12 o'clock). */
function setArc(el: SVGCircleElement | null, tail: number, head: number) {
  if (!el) return
  const len = Math.max(0, head - tail) * C
  el.setAttribute("stroke-dasharray", `${len} ${C}`)
  el.setAttribute("stroke-dashoffset", String(-tail * C))
  el.style.opacity = len > 0.01 ? "1" : "0"
}

/**
 * The dwell clock. Each status poll gives a progress sampled up to a second
 * earlier; from it we work out when the dwell started and run one steady
 * clock from there, so polls never nudge the arc back and forth. A new role
 * drains the old arc away clockwise while the next one grows in behind it.
 */
function CycleRing({ active, role, progress, cycleSecs, at }: Cycle) {
  const cur = React.useRef<SVGCircleElement>(null)
  const old = React.useRef<SVGCircleElement>(null)
  // `start`: when the dwell began; `appear`: when this arc first showed, so
  // it grows in from 0 instead of popping up mid-way; `done`: it filled and
  // was drained already.
  const clock = React.useRef<{ role: string; start: number; appear: number; done: boolean } | null>(null)
  // The arc being drained: where its head was and when the drain began.
  const drain = React.useRef<{ head: number; since: number } | null>(null)
  const visible = active && at > 0

  const secs = Math.max(1, cycleSecs) * 1000

  // Fold each sample into the clock.
  React.useEffect(() => {
    if (!active || !at) {
      clock.current = null
      return
    }
    const now = performance.now()
    const est = at - progress * secs
    const c = clock.current
    if (!c || c.role !== role) {
      // A new role: drain the old arc from wherever it got to (unless it
      // already filled and drained), then grow the new one in.
      if (c && !c.done) drain.current = { head: Math.min(1, (now - c.start) / secs), since: now }
      clock.current = { role, start: est, appear: now, done: false }
      return
    }
    // A sample stuck at 1 means the screen waits for its turn to switch;
    // it says nothing about when the dwell started.
    if (progress >= 0.999) return
    if (Math.abs(est - c.start) > SNAP_MS) c.start = est
    else c.start += (est - c.start) * 0.15
  }, [active, role, progress, secs, at])

  // One animation loop per ring.
  React.useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    let raf = 0
    const frame = (now: number) => {
      const c = clock.current
      let head = 0
      if (c && !c.done) {
        const real = Math.min(1, Math.max(0, (now - c.start) / secs))
        const grow = reduce ? 1 : easeOut(Math.min(1, (now - c.appear) / GROW_MS))
        head = real * grow
        if (real >= 1 && grow >= 1) {
          // Full: drain it now rather than when the next poll reports the
          // switch, so the ring never sits full waiting for news.
          c.done = true
          drain.current = { head: 1, since: now }
          head = 0
        }
      }
      setArc(cur.current, 0, head)
      const d = drain.current
      if (d) {
        const t = now - d.since
        // finish the sweep, then pull the tail round after it
        const dh = d.head + (1 - d.head) * easeInOut(Math.min(1, t / FINISH_MS))
        const tail = easeInOut(Math.min(1, Math.max(0, t - FINISH_MS * 0.4) / DRAIN_MS))
        setArc(old.current, tail, dh)
        if (tail >= 1 || reduce) {
          drain.current = null
          setArc(old.current, 0, 0)
        }
      }
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)
    return () => cancelAnimationFrame(raf)
  }, [secs])

  return (
    <svg
      viewBox="0 0 100 100"
      className={cn(
        "pointer-events-none absolute inset-0 size-full -rotate-90 transition-opacity duration-500 ease-out",
        visible ? "opacity-100" : "opacity-0"
      )}
      aria-hidden
    >
      <circle cx="50" cy="50" r={R} fill="none" stroke="rgba(255,255,255,.06)" strokeWidth="1" />
      <circle
        ref={old}
        cx="50"
        cy="50"
        r={R}
        fill="none"
        className="stroke-primary"
        strokeWidth="1.2"
        strokeLinecap="round"
        style={{ opacity: 0 }}
      />
      <circle
        ref={cur}
        cx="50"
        cy="50"
        r={R}
        fill="none"
        className="stroke-primary"
        strokeWidth="1.2"
        strokeLinecap="round"
        style={{ opacity: 0 }}
      />
    </svg>
  )
}

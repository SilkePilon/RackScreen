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
  progress,
  cycleSecs,
  label,
  dim,
  thumb,
}: {
  screen: number
  className?: string
  progress?: number
  cycleSecs?: number
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
      { thumb },
    )
  }, [screen, thumb])

  return (
    <div
      className={cn(
        "relative aspect-square w-full rounded-full p-[5%]",
        "bg-[radial-gradient(circle_at_30%_20%,#2a2a2e,#0c0c0e_60%)] shadow-[inset_0_1px_0_rgba(255,255,255,.08),0_10px_30px_-10px_rgba(0,0,0,.8)]",
        className,
      )}
      role="img"
      aria-label={label ?? `Screen ${screen + 1} live view`}
    >
      {progress !== undefined && <CycleRing progress={progress} cycleSecs={cycleSecs} />}
      <div className="relative size-full overflow-hidden rounded-full bg-black ring-1 ring-black">
        <canvas
          ref={canvas}
          width={240}
          height={240}
          className={cn(
            "size-full transition-opacity duration-500",
            hasFrame ? "opacity-100" : "opacity-0",
            dim && "opacity-40",
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

function CycleRing({ progress, cycleSecs }: { progress: number; cycleSecs?: number }) {
  const arc = React.useRef<SVGCircleElement>(null)
  const C = 2 * Math.PI * 48.5
  React.useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    if (reduce || !cycleSecs) {
      // No animation loop: show the polled progress as is.
      arc.current?.setAttribute("stroke-dashoffset", String(C * (1 - progress)))
      return
    }
    const start = performance.now()
    let raf = 0
    const tick = (now: number) => {
      const p = Math.min(1, progress + (now - start) / 1000 / cycleSecs)
      arc.current?.setAttribute("stroke-dashoffset", String(C * (1 - p)))
      if (p < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [progress, cycleSecs, C])
  return (
    <svg viewBox="0 0 100 100" className="pointer-events-none absolute inset-0 size-full -rotate-90" aria-hidden>
      <circle cx="50" cy="50" r="48.5" fill="none" stroke="rgba(255,255,255,.06)" strokeWidth="1" />
      <circle
        ref={arc}
        cx="50"
        cy="50"
        r="48.5"
        fill="none"
        className="stroke-primary"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeDasharray={C}
        strokeDashoffset={C * (1 - progress)}
      />
    </svg>
  )
}

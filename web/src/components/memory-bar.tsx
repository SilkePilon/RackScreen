import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

export interface BarSegment {
  key: string
  label: string
  value: number
  /** Tailwind background for the swatch and the segment (a text colour when dashed). */
  tone: string
  /** Drawn hatched in the text colour: a projection, not a measurement. */
  dashed?: boolean
  /** Shown in the legend instead of the formatted value. */
  display?: string
}

/** Hatched fill for projected segments (uses the segment's text colour). */
const HATCH =
  "bg-[repeating-linear-gradient(135deg,currentColor_0_3px,transparent_3px_6px)] opacity-80 ring-1 ring-inset ring-current"

/** A stacked horizontal bar over `total`, with an optional marker line. */
export function StackedBar({
  segments,
  total,
  marker,
  className,
  format,
}: {
  segments: BarSegment[]
  total: number
  marker?: { at: number; label: string }
  className?: string
  format: (v: number) => string
}) {
  const pct = (v: number) => `${Math.max(0, Math.min(100, (v / Math.max(total, 1)) * 100))}%`
  return (
    <div className={cn("relative", className)}>
      <div
        className="flex h-2.5 w-full gap-px overflow-hidden rounded-full bg-muted"
        role="img"
        aria-label={segments.map((s) => `${s.label} ${s.display ?? format(s.value)}`).join(", ")}
      >
        {segments
          .filter((s) => s.value > 0)
          .map((s) => (
            <Tooltip key={s.key}>
              <TooltipTrigger asChild>
                <div
                  className={cn(
                    "h-full shrink-0 transition-[width] duration-700 ease-out first:rounded-l-full last:rounded-r-full",
                    s.dashed && HATCH,
                    s.tone
                  )}
                  style={{ width: pct(s.value) }}
                />
              </TooltipTrigger>
              <TooltipContent>
                {s.label} · {s.display ?? format(s.value)}
              </TooltipContent>
            </Tooltip>
          ))}
      </div>
      {marker && marker.at > 0 && marker.at < total && (
        <div
          className="pointer-events-none absolute -top-1 -bottom-1 w-px bg-foreground/40"
          style={{ left: pct(marker.at) }}
          aria-hidden
        />
      )}
    </div>
  )
}

export function Legend({ segments, format }: { segments: BarSegment[]; format: (v: number) => string }) {
  return (
    <dl className="flex flex-wrap gap-x-5 gap-y-2">
      {segments.map((s) => (
        <div key={s.key} className="flex items-center gap-2 text-[13px]">
          <span
            className={cn(
              "size-2 shrink-0 rounded-full",
              s.dashed && HATCH,
              s.tone
            )}
            aria-hidden
          />
          <dt className="text-muted-foreground">{s.label}</dt>
          <dd className="tabular font-medium text-foreground">{s.display ?? format(s.value)}</dd>
        </div>
      ))}
    </dl>
  )
}

/** A single-value meter: `value` of `total`, warning past `warnAt` (0..1). */
export function Meter({
  value,
  total,
  warnAt = 0.85,
  className,
}: {
  value: number
  total: number
  warnAt?: number
  className?: string
}) {
  const f = total > 0 ? Math.max(0, Math.min(1, value / total)) : 0
  return (
    <div className={cn("h-1.5 w-full overflow-hidden rounded-full bg-muted", className)}>
      <div
        className={cn(
          "h-full rounded-full transition-[width] duration-700 ease-out",
          f >= warnAt ? "bg-warning" : "bg-primary"
        )}
        style={{ width: `${f * 100}%` }}
      />
    </div>
  )
}

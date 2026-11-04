import { AlertCircleIcon, TriangleAlertIcon } from "lucide-react"

import { Legend, StackedBar, type BarSegment } from "@/components/memory-bar"
import { Section, SettingsCard } from "@/components/settings"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { errorMessage } from "@/lib/api"
import { formatMb, useGifBudget, type GifBudget } from "@/lib/gif-memory"
import { cn } from "@/lib/utils"

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`

function headline(b: GifBudget) {
  const { mem, fitsMore, projectedMb } = b
  const used = mem.decoded_mb
  const parts = [
    used > 0
      ? `GIFs on screens use ${formatMb(used)}`
      : projectedMb > 0
        ? "No GIF is decoded yet"
        : "No GIF is on a screen",
  ]
  if (fitsMore !== null)
    parts.push(
      fitsMore === 0
        ? "no room for another full-length GIF"
        : `room for about ${plural(fitsMore, "more full-length GIF")}`
    )
  return parts.join(" · ")
}

/** The Pi's RAM split into other programs, RackScreen, its GIFs and free. */
export function GifMemorySection() {
  const q = useGifBudget()
  const b = q.data
  return (
    <Section
      title="Memory"
      description={
        b
          ? `Each GIF a screen shows stays decoded in RAM, packed: ${formatMb((b.mem.frame_bytes * b.mem.max_frames) / 1024 / 1024)} at most (${b.mem.max_frames} frames), usually a quarter of that.`
          : "Each GIF a screen shows stays decoded in RAM."
      }
    >
      <SettingsCard>
        {b ? (
          <MemoryBody b={b} />
        ) : q.isError ? (
          <div className="p-5">
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>Can't read the memory use</AlertTitle>
              <AlertDescription>{errorMessage(q.error)}</AlertDescription>
            </Alert>
          </div>
        ) : (
          <div className="flex flex-col gap-4 p-5">
            <div className="flex items-center justify-between">
              <Skeleton className="h-4 w-72 max-w-[60%]" />
              <Skeleton className="h-3 w-24" />
            </div>
            <Skeleton className="h-2.5 w-full rounded-full" />
            <div className="flex gap-5">
              {[110, 96, 80, 72].map((w, i) => (
                <Skeleton key={i} className="h-3" style={{ width: w }} />
              ))}
            </div>
          </div>
        )}
      </SettingsCard>
    </Section>
  )
}

function MemoryBody({ b }: { b: GifBudget }) {
  const { mem, projectedMb, projected } = b
  const total = mem.memory.total_mb
  const avail = mem.memory.available_mb
  if (total === null || avail === null) {
    return (
      <div className="flex items-center justify-between gap-4 px-5 py-4">
        <span className="text-sm font-medium">{headline(b)}</span>
        <span className="text-[13px] text-muted-foreground">RAM size unknown on this system</span>
      </div>
    )
  }
  const gifs = mem.decoded_mb
  const rss = mem.memory.process_rss_mb ?? gifs
  const service = Math.max(0, rss - gifs)
  const others = Math.max(0, total - avail - Math.max(rss, gifs))
  const pending = Math.min(projectedMb, avail)
  const free = Math.max(0, avail - pending)
  const unsaved = projected.some((p) => p.unsaved)
  const estimated = projected.every((p) => p.known)
  const segments: BarSegment[] = [
    { key: "others", label: "Other programs", value: others, tone: "bg-muted-foreground/35" },
    { key: "service", label: "RackScreen", value: service, tone: "bg-chart-1" },
    { key: "gifs", label: "GIFs", value: gifs, tone: "bg-chart-5" },
    ...(projectedMb > 0
      ? [
          {
            key: "pending",
            label: unsaved ? "After saving" : "Loading",
            value: pending,
            tone: "text-chart-5",
            dashed: true,
            display: `${estimated ? "~" : "up to "}${formatMb(projectedMb)}`,
          },
        ]
      : []),
    { key: "free", label: "Free", value: free, tone: "bg-transparent" },
  ]
  const overBudget = mem.budget_mb !== null && mem.decoded_mb + projectedMb > mem.budget_mb
  return (
    <>
      <div className="flex flex-col gap-4 p-5">
        <div className="flex flex-col gap-1 sm:flex-row sm:items-baseline sm:justify-between sm:gap-4">
          <span className="text-sm font-medium text-foreground">{headline(b)}</span>
          <span className="tabular text-[13px] text-muted-foreground">{formatMb(total)} RAM</span>
        </div>
        <StackedBar
          segments={segments}
          total={total}
          format={formatMb}
          marker={{ at: total - mem.reserve_mb, label: "Reserve" }}
        />
        <Legend
          segments={segments.map((s) =>
            s.key === "free" ? { ...s, tone: "bg-muted ring-1 ring-inset ring-border" } : s
          )}
          format={formatMb}
        />
        <p className="text-xs text-muted-foreground">
          The line marks the {formatMb(mem.reserve_mb)} kept free for the system. Room left for GIFs:{" "}
          <span className="tabular text-foreground">{formatMb(b.leftMb ?? 0)}</span>.
          {projectedMb > 0 &&
            ` The hatched part is ${plural(projected.length, "GIF")} ${unsaved ? "you added but did not save yet" : "still loading"}${estimated ? "" : ", counted at full length"}.`}
        </p>
      </div>
      {overBudget && (
        <div className="flex items-start gap-3 bg-warning/[0.06] px-5 py-3.5 text-[13px]">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0 text-warning" />
          <p className="text-pretty text-foreground">
            {unsaved ? "With these GIFs" : "With the GIFs on screens"} the Pi would have less than{" "}
            {formatMb(mem.reserve_mb)} left for everything else. Take a GIF off a screen, or pick shorter
            ones.
          </p>
        </div>
      )}
    </>
  )
}

/** Load state, frames and size of one configured GIF, for its row. */
export function GifMemoryMeta({ name, onScreen }: { name: string; onScreen: boolean }) {
  const q = useGifBudget()
  const b = q.data
  if (!b) return q.isError ? null : <Skeleton className="h-4 w-16" />
  const row = b.rows.get(name)
  const proj = b.projected.find((p) => p.name === name)
  if (row?.state === "ready" && row.mb !== null) {
    return (
      <div className="flex flex-col items-end gap-0.5 text-right">
        <span className="tabular text-[13px] font-medium text-foreground">{formatMb(row.mb)}</span>
        <span className="tabular text-xs text-muted-foreground">{plural(row.frames ?? 0, "frame")}</span>
      </div>
    )
  }
  if (row?.state === "error" && onScreen && !proj?.unsaved) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge variant="destructive" className="font-normal">
            <AlertCircleIcon />
            Failed
          </Badge>
        </TooltipTrigger>
        <TooltipContent className="max-w-xs">
          {row.error ?? "The download failed; it is retried."}
        </TooltipContent>
      </Tooltip>
    )
  }
  if (proj) {
    return (
      <div className="flex flex-col items-end gap-0.5 text-right">
        {proj.unsaved ? (
          <span className="tabular text-[13px] text-muted-foreground">
            {proj.known ? "~" : "≤ "}
            {formatMb(proj.mb)}
          </span>
        ) : (
          <Badge variant="secondary" className="gap-1.5 font-normal text-muted-foreground">
            <Spinner className="size-3" />
            Loading
          </Badge>
        )}
        <span className={cn("text-xs text-muted-foreground", !proj.unsaved && "tabular")}>
          {proj.unsaved ? "after saving" : `${proj.known ? "~" : "≤ "}${formatMb(proj.mb)}`}
        </span>
      </div>
    )
  }
  return null
}

/** Amber note when a GIF of `mb` would not fit what is left. */
export function BudgetWarning({ mb, known }: { mb: number; known: boolean }) {
  const q = useGifBudget()
  const left = q.data?.leftMb
  if (left === null || left === undefined || mb <= left) return null
  return (
    <Alert className="border-warning/30 bg-warning/[0.06]">
      <TriangleAlertIcon className="text-warning!" />
      <AlertTitle>Tight on memory</AlertTitle>
      <AlertDescription>
        This GIF takes {known ? "about" : "up to"} {formatMb(mb)} and only {formatMb(left)} is left for GIFs.
        The Pi may start swapping or stop the panels.
      </AlertDescription>
    </Alert>
  )
}

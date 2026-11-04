import * as React from "react"
import { SparklesIcon } from "lucide-react"

import { Segmented } from "@/components/segmented"
import { Row, SettingsCard, Section, SwitchRow } from "@/components/settings"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Slider } from "@/components/ui/slider"
import { api } from "@/lib/api"
import { useDraft, useField } from "@/lib/draft"
import { useControl, useStatus } from "@/lib/queries"
import {
  composite,
  DIRECTION_LABELS,
  effectiveDirection,
  formatSecs,
  MAX_SECS,
  MIN_SECS,
  sampleFrame,
  STYLES,
  styleInfo,
  type TransitionDirection,
} from "@/lib/transitions"
import type { ShowTransitionCfg } from "@/lib/types"
import { cn } from "@/lib/utils"

// ---------------------------------------------------------------------------
// One shared clock drives every preview, so eleven canvases cost one frame
// callback and all loop in step.

interface PreviewSpec {
  style: string
  dir: string
  secs: number
}

type Draw = (nowMs: number) => void

const drawers = new Set<Draw>()
let raf = 0

function tick(now: number) {
  for (const d of drawers) d(now)
  raf = drawers.size ? requestAnimationFrame(tick) : 0
}

function register(d: Draw) {
  drawers.add(d)
  if (!raf) raf = requestAnimationFrame(tick)
  return () => {
    drawers.delete(d)
    if (!drawers.size && raf) {
      cancelAnimationFrame(raf)
      raf = 0
    }
  }
}

/** Preview resolution: the panels at a fifth. */
const PX = 72
/** The pause on each sample between transitions, in ms. */
const HOLD_MS = 900

function useReducedMotion() {
  const [reduced, setReduced] = React.useState(
    () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  )
  React.useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)")
    const on = () => setReduced(mq.matches)
    mq.addEventListener("change", on)
    return () => mq.removeEventListener("change", on)
  }, [])
  return reduced
}

/** A round panel playing `spec` between the two samples, forever. */
function Preview({ spec, className }: { spec: PreviewSpec; className?: string }) {
  const ref = React.useRef<HTMLCanvasElement>(null)
  const reduced = useReducedMotion()
  const specRef = React.useRef(spec)
  React.useEffect(() => {
    specRef.current = spec
  }, [spec])
  React.useEffect(() => {
    const canvas = ref.current
    const ctx = canvas?.getContext("2d")
    if (!canvas || !ctx) return
    const a = sampleFrame("a", PX)
    const b = sampleFrame("b", PX)
    const img = ctx.createImageData(PX, PX)
    const out = img.data
    let last = -1
    const paint = (t: number, forward: boolean) => {
      const key = Math.round(t * 400) * 2 + (forward ? 1 : 0)
      if (key === last) return
      last = key
      const s = specRef.current
      if (forward) composite(s.style, s.dir, a, b, out, t, PX)
      else composite(s.style, s.dir, b, a, out, t, PX)
      ctx.putImageData(img, 0, 0)
    }
    if (reduced) {
      paint(0.5, true)
      return
    }
    const t0 = performance.now()
    return register((now) => {
      const s = specRef.current
      const trans = s.secs * 1000
      const period = 2 * (HOLD_MS + trans)
      const p = (now - t0) % period
      if (p < HOLD_MS) paint(0, true)
      else if (p < HOLD_MS + trans) paint((p - HOLD_MS) / trans, true)
      else if (p < 2 * HOLD_MS + trans) paint(0, false)
      else paint((p - 2 * HOLD_MS - trans) / trans, false)
    })
  }, [reduced])
  return (
    <canvas
      ref={ref}
      width={PX}
      height={PX}
      aria-hidden
      className={cn("size-14 rounded-full bg-black ring-1 ring-white/10 [image-rendering:auto]", className)}
    />
  )
}

// ---------------------------------------------------------------------------

function StyleCards() {
  const style = useField<string>("transitions.style")
  const dir = useField<string>("transitions.direction")
  const secs = useField<number>("transitions.secs")
  const random = useField<boolean>("transitions.random")
  const pick = (id: string) => {
    style.set(id)
    dir.set(styleInfo(id).defaultDirection)
  }
  const dirty = style.dirty || dir.dirty
  return (
    <Row
      id="transitions.style"
      dirty={dirty}
      onReset={() => {
        style.reset()
        dir.reset()
      }}
      stack
      muted={random.value}
    >
      <div role="radiogroup" aria-label="Transition style" className="grid w-full grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
        {STYLES.map((s) => {
          const selected = style.value === s.id
          const previewDir = selected ? effectiveDirection(s.id, dir.value) : s.defaultDirection
          return (
            <button
              key={s.id}
              type="button"
              role="radio"
              aria-checked={selected}
              data-style={s.id}
              onClick={() => pick(s.id)}
              className={cn(
                "group flex items-center gap-3 rounded-lg border p-2.5 text-left transition-[border-color,background-color,box-shadow]",
                "hover:bg-accent/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                selected
                  ? "border-primary/70 bg-primary/[0.06] shadow-[inset_0_0_0_1px_var(--primary)] dark:bg-primary/[0.08]"
                  : "border-border bg-background/40 dark:bg-white/[0.015]",
              )}
            >
              <Preview spec={{ style: s.id, dir: previewDir, secs: secs.value ?? 0.4 }} className="shrink-0" />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className={cn("text-[13px] font-medium", selected ? "text-foreground" : "text-foreground/90")}>
                  {s.title}
                </span>
                <span className="line-clamp-2 text-xs leading-snug text-muted-foreground">{s.blurb}</span>
              </div>
            </button>
          )
        })}
      </div>
    </Row>
  )
}

function DirectionControl({
  style,
  value,
  onChange,
  label,
}: {
  style: string
  value: string
  onChange: (d: TransitionDirection) => void
  label: string
}) {
  const info = styleInfo(style)
  if (info.directions.length === 0) return null
  return (
    <Segmented
      label={label}
      value={effectiveDirection(style, value)}
      onChange={onChange}
      options={info.directions.map((d) => ({
        value: d,
        label: DIRECTION_LABELS[d].label,
        aria: DIRECTION_LABELS[d].aria,
      }))}
    />
  )
}

function Direction() {
  const style = useField<string>("transitions.style")
  const dir = useField<string>("transitions.direction")
  const random = useField<boolean>("transitions.random")
  if (styleInfo(style.value).directions.length === 0) return null
  return (
    <Row id="transitions.direction" dirty={dir.dirty} onReset={dir.reset} muted={random.value}>
      <DirectionControl style={style.value} value={dir.value} onChange={dir.set} label="Direction" />
    </Row>
  )
}

function SecsSlider({
  value,
  onChange,
  label,
  className,
}: {
  value: number
  onChange: (v: number) => void
  label: string
  className?: string
}) {
  return (
    <div className={cn("flex w-full items-center gap-4 sm:w-64", className)}>
      <Slider
        min={MIN_SECS}
        max={MAX_SECS}
        step={0.05}
        value={[value]}
        onValueChange={([v]) => onChange(Math.round(v * 100) / 100)}
        aria-label={label}
      />
      <span className="w-12 text-right text-sm text-muted-foreground tabular">{formatSecs(value)}</span>
    </div>
  )
}

function Duration() {
  const f = useField<number>("transitions.secs")
  return (
    <Row id="transitions.secs" dirty={f.dirty} onReset={f.reset}>
      <SecsSlider value={f.value ?? 0.4} onChange={f.set} label="Transition length" />
    </Row>
  )
}

const SAME = "same"

function ShowsUse() {
  const f = useField<ShowTransitionCfg | null | undefined>("transitions.shows")
  const screens = useField<string>("transitions.style")
  const own = f.value ?? null
  const set = (next: ShowTransitionCfg | null) => f.set(next)
  return (
    <Row id="transitions.shows" dirty={f.dirty} onReset={f.reset} stack={!!own}>
      <div className="flex w-full flex-col gap-3 sm:items-end">
        <Select
          value={own ? own.style : SAME}
          onValueChange={(v) =>
            set(v === SAME ? null : { style: v, direction: styleInfo(v).defaultDirection, secs: own?.secs ?? 0.4 })
          }
        >
          <SelectTrigger className="w-full sm:w-56" aria-label="Transition around shows">
            <SelectValue />
          </SelectTrigger>
          <SelectContent align="end">
            <SelectGroup>
              <SelectItem value={SAME}>Same as screens ({styleInfo(screens.value).title})</SelectItem>
              {STYLES.map((s) => (
                <SelectItem key={s.id} value={s.id}>
                  {s.title}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
        {own && (
          <div className="flex w-full flex-col gap-3 sm:flex-row sm:items-center sm:justify-end sm:gap-5">
            <DirectionControl
              style={own.style}
              value={own.direction}
              onChange={(d) => set({ ...own, direction: d })}
              label="Direction around shows"
            />
            <SecsSlider
              value={own.secs ?? 0.4}
              onChange={(secs) => set({ ...own, secs })}
              label="Transition length around shows"
            />
          </div>
        )}
      </div>
    </Row>
  )
}

function TryIt() {
  const d = useDraft()
  const status = useStatus()
  const style = useField<string>("transitions.style")
  const random = useField<boolean>("transitions.random")
  const unsaved = d.changes.length > 0
  const showing = status.data?.screens[0]?.showing
  const busy = showing === "show" || showing === "night" || showing === "pattern" || showing === "identify"
  const cycling = (status.data?.screens ?? []).some((s) => s.roles.length >= 2)
  const test = useControl(() => api.transitionTest(), () => "Changing roles on the panels now")
  const heavy = !random.value && styleInfo(style.value).heavy
  const why = unsaved
    ? "Save first: the panels play the saved style."
    : showing === "show"
      ? "Wait for the show to finish."
      : showing === "night"
        ? "The panels are asleep."
        : showing === "pattern" || showing === "identify"
          ? "The test pattern is on."
          : status.data && !cycling
            ? "No screen cycles roles yet."
            : "Every cycling screen changes to its next role now, left to right."
  return (
    <Row
      title="Try it on the panels"
      description={
        <span className="flex flex-col gap-1">
          <span>{why}</span>
          {heavy && (
            <span className="text-amber-600/90 dark:text-amber-400/80">
              {styleInfo(style.value).title} changes the whole panel every frame, so each frame is a full SPI push:
              smoother at 30–60 fps.
            </span>
          )}
        </span>
      }
    >
      <Button
        variant="outline"
        size="sm"
        disabled={unsaved || busy || test.isPending || (!!status.data && !cycling)}
        onClick={() => test.mutate(undefined)}
      >
        <SparklesIcon data-icon="inline-start" />
        Try it
      </Button>
    </Row>
  )
}

export function TransitionsSection() {
  return (
    <Section
      id="transitions"
      title="Transitions"
      description="How one role gives way to the next, and the roles to a show and back."
    >
      <SettingsCard>
        <StyleCards />
        <Direction />
        <Duration />
        <SwitchRow id="transitions.random" />
        <ShowsUse />
        <TryIt />
      </SettingsCard>
    </Section>
  )
}

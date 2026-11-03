import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import { restrictToHorizontalAxis, restrictToParentElement } from "@dnd-kit/modifiers"
import {
  arrayMove,
  horizontalListSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { PlayIcon, ShuffleIcon } from "lucide-react"
import { useEffect, useState } from "react"

import { HeaderActions, PageBody, PageIntro } from "@/components/page"
import { PanelMirror } from "@/components/panel-mirror"
import { RestoreDefaults } from "@/components/restore-defaults"
import { NumberRow, Row, RowSkeleton, Section, SettingsCard, SwitchRow, useEnabled } from "@/components/settings"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { api } from "@/lib/api"
import { useDraft, useField } from "@/lib/draft"
import { useControl, useShows, useStatus } from "@/lib/queries"
import { SHOW_CARD_FRAMES, showCardUrl } from "@/lib/show-card"
import type { ShowInfo } from "@/lib/types"
import { cn } from "@/lib/utils"

export default function Shows() {
  const d = useDraft()
  const shows = useShows()
  const status = useStatus()
  const play = useControl(
    (id: string) => api.play(id),
    (id) => (id === "random" ? "Playing a random show" : `Playing ${shows.data?.find((s) => s.id === id)?.title ?? id}`),
  )
  const night = status.data?.night.active ?? false
  return (
    <PageBody>
      <RestoreDefaults page="/shows" />
      <HeaderActions>
        <Button variant="ghost" size="sm" onClick={() => play.mutate("random")} disabled={night}>
          <ShuffleIcon data-icon="inline-start" />
          <span className="hidden sm:inline">Play random</span>
        </Button>
      </HeaderActions>
      <PageIntro title="Shows">
        Short pixel-art scenes that run across all screens as one wide picture, every so often and never at night.
      </PageIntro>
      {d.ready ? <Schedule /> : <RowSkeleton count={4} />}
      <Section
        title="Rotation"
        description={night ? "It is night; shows wait until morning." : "Shows picked at random from the ones that are on."}
      >
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {shows.data && d.ready
            ? shows.data.map((s) => (
                <ShowCard
                  key={s.id}
                  show={s}
                  all={shows.data}
                  onPlay={() => play.mutate(s.id)}
                  playDisabled={night || play.isPending}
                />
              ))
            : Array.from({ length: 6 }, (_, i) => <Skeleton key={i} className="aspect-[2/1.38] rounded-xl" />)}
        </div>
      </Section>
    </PageBody>
  )
}

function Schedule() {
  const on = useEnabled("shows.enabled")
  return (
    <Section title="Schedule">
      <SettingsCard>
        <SwitchRow id="shows.enabled" />
        <NumberRow id="shows.every_mins" min={2} max={240} unit="min" muted={!on} />
        <NumberRow id="shows.gap_mm" min={0} max={200} unit="mm" integer={false} muted={!on} />
        <PanelOrder muted={!on} />
      </SettingsCard>
    </Section>
  )
}

function PanelOrder({ muted }: { muted?: boolean }) {
  const d = useDraft()
  const f = useField<number[] | null | undefined>("shows.order")
  const n = d.draft?.screens.length ?? 0
  const order = f.value && f.value.length === n ? f.value : Array.from({ length: n }, (_, i) => i)
  const ids = order.map(String)
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )
  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return
    const next = arrayMove(order, ids.indexOf(String(e.active.id)), ids.indexOf(String(e.over.id)))
    f.set(next.every((v, i) => v === i) ? undefined : next)
  }
  return (
    <Row id="shows.order" dirty={f.dirty} onReset={f.reset} muted={muted} stack>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={onDragEnd}
        modifiers={[restrictToHorizontalAxis, restrictToParentElement]}
      >
        <SortableContext items={ids} strategy={horizontalListSortingStrategy}>
          <ol className="flex w-full max-w-md gap-3" aria-label="Screens from left to right">
            {order.map((s, pos) => (
              <OrderChip key={s} screen={s} position={pos} />
            ))}
          </ol>
        </SortableContext>
      </DndContext>
    </Row>
  )
}

function OrderChip({ screen, position }: { screen: number; position: number }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: String(screen) })
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={cn("flex-1", isDragging && "z-10")}
    >
      <button
        type="button"
        {...attributes}
        {...listeners}
        aria-label={`Screen ${screen + 1}, position ${position + 1} from the left`}
        className={cn(
          "flex w-full cursor-grab touch-none flex-col items-center gap-2 rounded-xl border bg-background/40 p-3 transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none active:cursor-grabbing",
          isDragging && "bg-accent shadow-xl",
        )}
      >
        <PanelMirror thumb screen={screen} className="w-12 p-[3px]" />
        <span className="text-xs font-medium tabular">Screen {screen + 1}</span>
      </button>
    </li>
  )
}

function ShowCard({
  show,
  all,
  onPlay,
  playDisabled,
}: {
  show: ShowInfo
  all: ShowInfo[]
  onPlay: () => void
  playDisabled: boolean
}) {
  const f = useField<string[] | undefined>("shows.list")
  const allIds = all.map((s) => s.id)
  const list = f.value ?? allIds
  const on = list.includes(show.id)
  const last = on && list.length === 1
  const toggle = (v: boolean) => {
    const next = allIds.filter((id) => (id === show.id ? v : list.includes(id)))
    f.set(next.length === allIds.length ? undefined : next)
  }
  const [active, setActive] = useState(false)
  return (
    <article
      onPointerEnter={() => setActive(true)}
      onPointerLeave={() => setActive(false)}
      onFocus={() => setActive(true)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setActive(false)
      }}
      className={cn(
        "group flex flex-col overflow-hidden rounded-xl border bg-card transition-[border-color,opacity]",
        !on && "opacity-60 hover:opacity-100",
      )}
    >
      <div className="relative">
        <ShowStill id={show.id} active={active} />
        <Button
          size="sm"
          variant="secondary"
          onClick={onPlay}
          disabled={playDisabled}
          className="absolute top-2.5 right-2.5 h-7 bg-black/50 text-white opacity-0 backdrop-blur transition-opacity group-hover:opacity-100 hover:bg-black/70 focus-visible:opacity-100 disabled:opacity-0 group-hover:disabled:opacity-50 max-sm:opacity-100"
        >
          <PlayIcon data-icon="inline-start" />
          Play now
        </Button>
      </div>
      <div className="flex items-center gap-3 px-4 py-3">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{show.title}</span>
          <span className="truncate font-mono text-xs text-muted-foreground">{show.id}</span>
        </div>
        <Switch
          checked={on}
          onCheckedChange={toggle}
          disabled={last}
          aria-label={`${show.title} in the rotation`}
          title={last ? "At least one show stays on" : undefined}
        />
      </div>
    </article>
  )
}

const CYCLE_MS = 600

/**
 * The show's own pixels: a still of its world at native resolution, scaled
 * up crisp. While `active` (hover or focus) it cycles through the other
 * stills, which are only fetched on the first hover.
 */
function ShowStill({ id, active }: { id: string; active: boolean }) {
  const [loaded, setLoaded] = useState(false)
  const [warm, setWarm] = useState(false)
  const [frame, setFrame] = useState(0)
  const [still] = useState(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches)
  const cycle = active && !still
  if (cycle && !warm) setWarm(true)
  useEffect(() => {
    if (!cycle) return
    let k = 0
    const timer = window.setInterval(() => setFrame((k = (k % (SHOW_CARD_FRAMES - 1)) + 1)), CYCLE_MS)
    return () => {
      window.clearInterval(timer)
      setFrame(0)
    }
  }, [cycle])
  const frames = warm ? Array.from({ length: SHOW_CARD_FRAMES }, (_, k) => k) : [0]
  return (
    <div className="relative aspect-[2/1] overflow-hidden bg-black" aria-hidden>
      {!loaded && <Skeleton className="absolute inset-0 rounded-none" />}
      {frames.map((k) => (
        <img
          key={k}
          src={showCardUrl(id, k)}
          alt=""
          width={120}
          height={60}
          loading={k === 0 ? "lazy" : "eager"}
          decoding="async"
          draggable={false}
          onLoad={k === 0 ? () => setLoaded(true) : undefined}
          className={cn(
            "absolute inset-0 size-full [image-rendering:pixelated]",
            k === frame ? "opacity-100" : "opacity-0",
            !loaded && "invisible",
          )}
        />
      ))}
    </div>
  )
}

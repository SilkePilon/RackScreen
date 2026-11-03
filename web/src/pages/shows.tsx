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
import type { ShowInfo } from "@/lib/types"
import { cn } from "@/lib/utils"

const PANEL_MASK = [12.5, 37.5, 62.5, 87.5]
  .map((x) => `radial-gradient(circle at ${x}% 50%, #000 26px, transparent 27px)`)
  .join(", ")

// A tiny painted horizon per show, so the cards read at a glance.
const SCENES: Record<string, [string, string, string]> = {
  freight: ["#1b1340", "#d9575b", "#f2a65a"],
  "deep-sea": ["#03142b", "#0b4a6e", "#2bb3c0"],
  "paper-plane": ["#9fd3ff", "#e8f4ff", "#ffffff"],
  storm: ["#0d1017", "#2c3445", "#8c9cff"],
  day: ["#ffcf7a", "#7cc4ff", "#1b2a4a"],
  space: ["#000005", "#1a1240", "#a48bfa"],
  birds: ["#3b1d3f", "#f06a5d", "#ffc56e"],
  fireworks: ["#05060f", "#1d1b3a", "#f2b84b"],
  alpine: ["#2b3a67", "#f7b2a6", "#ffe3c4"],
  caravan: ["#f2a65a", "#f7d08a", "#c06a3a"],
  "desert-night": ["#060a1a", "#1d2547", "#d6d6e0"],
}

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
            : Array.from({ length: 6 }, (_, i) => <Skeleton key={i} className="h-48 rounded-xl" />)}
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
        <PanelMirror screen={screen} className="w-12 p-[3px]" />
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
  const [a, b, c] = SCENES[show.id] ?? ["#111", "#333", "#777"]
  return (
    <article
      className={cn(
        "group flex flex-col overflow-hidden rounded-xl border bg-card transition-[border-color,opacity]",
        !on && "opacity-60 hover:opacity-100",
      )}
    >
      <div className="relative h-28 overflow-hidden bg-black" aria-hidden>
        {/* The scene seen through four round panels, like the rack shows it. */}
        <div
          className="absolute inset-0 transition-transform duration-700 ease-out group-hover:scale-105 motion-reduce:transition-none"
          style={{
            background: `linear-gradient(180deg, transparent 62%, rgba(0,0,0,.55) 62%), linear-gradient(180deg, ${a} 12%, ${b} 55%, ${c} 88%)`,
            maskImage: PANEL_MASK,
            WebkitMaskImage: PANEL_MASK,
          }}
        />
        <Button
          size="sm"
          variant="secondary"
          onClick={onPlay}
          disabled={playDisabled}
          className="absolute top-2.5 right-2.5 h-7 bg-black/50 text-white opacity-0 backdrop-blur transition-opacity group-hover:opacity-100 hover:bg-black/70 focus-visible:opacity-100 max-sm:opacity-100"
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

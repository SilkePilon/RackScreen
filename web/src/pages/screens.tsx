import * as React from "react"
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
import { ChevronRightIcon, LayoutTemplateIcon, PlusIcon, XIcon } from "lucide-react"
import { useSearchParams } from "react-router"
import { toast } from "sonner"

import { HeaderActions, PageBody, PageIntro } from "@/components/page"
import { PanelMirror } from "@/components/panel-mirror"
import { NeedsBadge, RoleGallery, roleTitle } from "@/components/role-gallery"
import { RolePreview } from "@/components/role-preview"
import { NumberInput, ResetButton } from "@/components/settings"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Skeleton } from "@/components/ui/skeleton"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft } from "@/lib/draft"
import { useRoles, useStatus } from "@/lib/queries"
import { PRESETS } from "@/lib/roles"
import type { RoleInfo, ScreenCfg, ScreenStatus } from "@/lib/types"
import { cn } from "@/lib/utils"

export default function Screens() {
  const d = useDraft()
  const screens = d.draft?.screens
  const [params, setParams] = useSearchParams()
  const n = screens?.length ?? 0
  const asked = Number(params.get("screen")) - 1
  const selected = n ? Math.min(Math.max(Number.isFinite(asked) ? asked : 0, 0), n - 1) : 0
  const select = (i: number) =>
    setParams(
      (p) => {
        p.set("screen", String(i + 1))
        return p
      },
      { replace: true },
    )

  return (
    <PageBody>
      <Presets />
      <PageIntro title="Screens">
        Pick a screen in the rack, then choose what it cycles through. Each role shows for a few seconds; drag to
        reorder.
      </PageIntro>
      <div className="flex flex-col gap-4">
        <Rack count={n} selected={selected} onSelect={select} />
        {screens ? (
          <ScreenEditor key={selected} index={selected} screen={screens[selected]} />
        ) : (
          <Skeleton className="h-80 rounded-xl" />
        )}
      </div>
    </PageBody>
  )
}

// ---------------------------------------------------------------- presets

function Presets() {
  const d = useDraft()
  const roles = useRoles()
  const n = d.draft?.screens.length ?? 0
  const apply = (rows: string[][], title: string) => {
    d.update((c) => ({
      ...c,
      screens: c.screens.map((s, i) => (rows[i] ? { ...s, roles: rows[i], cycle_secs: 15 } : s)),
    }))
    toast(`${title} preset applied`, { description: "Review the screens, then Save & apply." })
  }
  return (
    <HeaderActions>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="sm" disabled={!d.ready}>
            <LayoutTemplateIcon data-icon="inline-start" />
            Presets
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-80">
          <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
            Replace the roles of the first screens
          </DropdownMenuLabel>
          <DropdownMenuGroup>
            {PRESETS.map((p) => (
              <DropdownMenuItem
                key={p.id}
                onSelect={() => apply(p.rows, p.title)}
                className="flex-col items-start gap-1.5 py-2"
              >
                <span className="flex w-full items-center justify-between gap-2">
                  <span className="font-medium">{p.title}</span>
                  <span className="flex items-center gap-1.5" aria-hidden>
                    {p.rows.map((row, i) => (
                      <span key={i} className="flex -space-x-1.5">
                        {row.map((r) => (
                          <RolePreview
                            key={r}
                            name={r}
                            label={roleTitle(r, roles.data?.find((x) => x.name === r))}
                            className="w-5 p-px ring-1 ring-popover"
                          />
                        ))}
                      </span>
                    ))}
                  </span>
                </span>
                <span className="text-xs text-muted-foreground">
                  {p.description} {p.rows.length < n ? `Screens ${p.rows.length + 1}+ keep theirs.` : ""}
                </span>
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    </HeaderActions>
  )
}

// ---------------------------------------------------------------- rack

/** The panels as they hang in the rack, live; one is the screen being edited. */
function Rack({ count, selected, onSelect }: { count: number; selected: number; onSelect: (i: number) => void }) {
  const d = useDraft()
  const status = useStatus()
  const roles = useRoles()
  const tabs = React.useRef<(HTMLButtonElement | null)[]>([])
  const sampledAt = (status.data?.received ?? 0) - (status.data?.age_ms ?? 0)

  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0
    const to = e.key === "Home" ? 0 : e.key === "End" ? count - 1 : (selected + step + count) % count
    if (!step && e.key !== "Home" && e.key !== "End") return
    e.preventDefault()
    onSelect(to)
    tabs.current[to]?.focus()
  }

  return (
    <div className="scrollbar-thin overflow-x-auto rounded-2xl border bg-card">
      <div
        role="tablist"
        aria-label="Screens"
        onKeyDown={onKeyDown}
        className="grid gap-2 p-3 sm:gap-4 sm:p-5"
        style={{ gridTemplateColumns: `repeat(${Math.max(count, 4)}, minmax(68px, 1fr))` }}
      >
        {count === 0
          ? Array.from({ length: 4 }, (_, i) => (
              <div key={i} className="flex flex-col items-center gap-2">
                <Skeleton className="aspect-square w-full max-w-36 rounded-full" />
                <Skeleton className="h-3 w-14" />
              </div>
            ))
          : Array.from({ length: count }, (_, i) => {
              const s = status.data?.screens.find((x) => x.index === i)
              const active = i === selected
              const dirty = d.isDirty(`screens.${i}`)
              return (
                <button
                  key={i}
                  ref={(el) => {
                    tabs.current[i] = el
                  }}
                  type="button"
                  role="tab"
                  id={`screen-tab-${i}`}
                  aria-selected={active}
                  aria-controls="screen-editor"
                  tabIndex={active ? 0 : -1}
                  onClick={() => onSelect(i)}
                  className="group/panel flex min-w-0 flex-col items-center gap-2 rounded-xl p-1 focus-visible:outline-none sm:gap-3"
                >
                  <span
                    className={cn(
                      "w-full max-w-36 rounded-full ring-offset-4 ring-offset-card transition-[box-shadow,opacity] duration-200 group-focus-visible/panel:ring-2 group-focus-visible/panel:ring-ring",
                      active
                        ? "ring-2 ring-primary"
                        : "opacity-70",
                    )}
                  >
                    <PanelMirror
                      screen={i}
                      thumb={!active}
                      cycle={
                        s && {
                          active: s.showing === "role" && s.roles.length > 1,
                          role: s.role,
                          progress: s.cycle_progress,
                          cycleSecs: s.cycle_secs,
                          at: sampledAt,
                        }
                      }
                    />
                  </span>
                  <span className="flex max-w-full flex-col items-center gap-0.5">
                    <span
                      className={cn(
                        "flex items-center gap-1.5 text-[11px] font-medium tracking-wider uppercase tabular",
                        active ? "text-foreground" : "text-muted-foreground",
                      )}
                    >
                      Screen {i + 1}
                      {dirty && (
                        <span className="size-1.5 rounded-full bg-primary" aria-label="unsaved changes" />
                      )}
                    </span>
                    <span className="hidden max-w-full truncate text-xs text-muted-foreground sm:block">
                      {s ? roleTitle(s.role, roles.data?.find((x) => x.name === s.role)) : " "}
                    </span>
                  </span>
                </button>
              )
            })}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------- editor

function ScreenEditor({ index, screen }: { index: number; screen: ScreenCfg }) {
  const d = useDraft()
  const roles = useRoles()
  const status = useStatus()
  const live = status.data?.screens.find((x) => x.index === index)
  const base = `screens.${index}`
  const [hover, setHover] = React.useState<string | null>(null)
  const [dragging, setDragging] = React.useState(false)
  const [gallery, setGallery] = React.useState(false)
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )
  const setRoles = (r: string[]) => d.set(`${base}.roles`, r)
  const onDragEnd = (e: DragEndEvent) => {
    setDragging(false)
    if (!e.over || e.active.id === e.over.id) return
    const from = screen.roles.indexOf(String(e.active.id))
    const to = screen.roles.indexOf(String(e.over.id))
    setRoles(arrayMove(screen.roles, from, to))
  }
  const info = (r: string) => roles.data?.find((x) => x.name === r)
  const needs = screen.roles
    .map((r) => ({ r, info: info(r) }))
    .filter((x): x is { r: string; info: RoleInfo & { needs: string } } => !!x.info?.needs)
  const rolesDirty = d.isDirty(`${base}.roles`)
  const cycleDirty = d.isDirty(`${base}.cycle_secs`)
  const total = screen.roles.length * screen.cycle_secs

  return (
    <section
      id="screen-editor"
      role="tabpanel"
      aria-labelledby={`screen-tab-${index}`}
      data-setting-id={`screen-${index}`}
      className="overflow-hidden rounded-xl border bg-card"
    >
      <header className="flex flex-wrap items-center gap-4 border-b border-hairline px-5 py-4">
        <div className="flex min-w-40 flex-1 flex-col gap-0.5">
          <div className="flex items-center gap-1.5">
            <h2 className="text-[15px] font-semibold tracking-tight">Screen {index + 1}</h2>
            {rolesDirty && <ResetButton onClick={() => d.reset(`${base}.roles`)} label="Reset roles to saved" />}
          </div>
          <p className="text-xs text-muted-foreground tabular">
            SPI {screen.spi} · CS {screen.cs} · rotated {screen.rotate}°{screen.hflip ? " · mirrored" : ""}
          </p>
        </div>
        <div className="flex items-center gap-2 max-sm:w-full">
          {cycleDirty && <ResetButton onClick={() => d.reset(`${base}.cycle_secs`)} />}
          <label htmlFor={`cycle-${index}`} className="text-[13px] text-muted-foreground max-sm:flex-1">
            Each role
          </label>
          <NumberInput
            id={`cycle-${index}`}
            value={screen.cycle_secs}
            onChange={(v) => v !== null && d.set(`${base}.cycle_secs`, v)}
            min={3}
            max={300}
            unit="s"
            className="w-24"
          />
        </div>
      </header>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragStart={() => setDragging(true)}
        onDragCancel={() => setDragging(false)}
        onDragEnd={onDragEnd}
        modifiers={[restrictToHorizontalAxis, restrictToParentElement]}
      >
        <SortableContext items={screen.roles} strategy={horizontalListSortingStrategy}>
          <ol
            className="scrollbar-thin flex gap-5 overflow-x-auto px-5 pt-6 pb-4 sm:gap-7"
            aria-label={`Roles on screen ${index + 1}, in order`}
          >
            {screen.roles.map((r, i) => (
              <Tile
                key={r}
                name={r}
                position={i}
                info={info(r)}
                loading={roles.isPending}
                last={i === screen.roles.length - 1}
                live={live?.showing === "role" && live.role === r}
                highlighted={hover === r}
                dragging={dragging}
                onHover={setHover}
                canRemove={screen.roles.length > 1}
                onRemove={() => setRoles(screen.roles.filter((x) => x !== r))}
              />
            ))}
            <li className="flex w-20 shrink-0 flex-col items-center gap-2 sm:w-24">
              <button
                type="button"
                onClick={() => setGallery(true)}
                className="flex aspect-square w-full items-center justify-center rounded-full border-2 border-dashed border-input text-muted-foreground transition-colors hover:border-primary hover:bg-primary/5 hover:text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                aria-label={`Add a role to screen ${index + 1}`}
              >
                <PlusIcon className="size-6" />
              </button>
              <span className="text-xs font-medium text-muted-foreground">Add role</span>
            </li>
          </ol>
        </SortableContext>
      </DndContext>

      <Timeline
        roles={screen.roles}
        cycleSecs={screen.cycle_secs}
        live={live}
        hover={hover}
        onHover={setHover}
        label={(r) => roleTitle(r, info(r))}
      />

      <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-hairline px-5 py-3">
        <span className="text-xs text-muted-foreground">
          {screen.roles.length === 1
            ? "One role: the screen never changes."
            : `${screen.roles.length} roles, a full cycle every ${total} s.`}
        </span>
        {needs.length > 0 && (
          <span className="flex flex-wrap items-center gap-1.5">
            {[...new Set(needs.map((x) => x.info.needs))].map((n) => (
              <NeedsBadge key={n} needs={n} />
            ))}
          </span>
        )}
      </footer>

      <RoleGallery
        open={gallery}
        onOpenChange={setGallery}
        screen={index}
        taken={screen.roles}
        onAdd={(r) => setRoles([...screen.roles, r])}
      />
    </section>
  )
}

function Tile({
  name,
  position,
  info,
  loading,
  last,
  live,
  highlighted,
  dragging,
  onHover,
  canRemove,
  onRemove,
}: {
  name: string
  position: number
  info?: RoleInfo
  loading: boolean
  last: boolean
  live: boolean
  highlighted: boolean
  dragging: boolean
  onHover: (r: string | null) => void
  canRemove: boolean
  onRemove: () => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: name })
  const title = roleTitle(name, info)
  const unknown = !loading && !info
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      onMouseEnter={() => onHover(name)}
      onMouseLeave={() => onHover(null)}
      className={cn("group/tile relative flex w-20 shrink-0 flex-col items-center gap-2 sm:w-24", isDragging && "z-10")}
    >
      <div
        {...attributes}
        {...listeners}
        aria-label={`${position + 1}. ${title}${live ? ", on the panel now" : ""}. Press space to move.`}
        onFocus={() => onHover(name)}
        onBlur={() => onHover(null)}
        className="relative w-full cursor-grab touch-none rounded-full focus-visible:outline-none active:cursor-grabbing"
      >
        <RolePreview
          name={name}
          label={title}
          className={cn(
            "w-full ring-offset-2 ring-offset-card transition-[box-shadow,transform] duration-150",
            highlighted && !isDragging && "ring-2 ring-primary/60",
            unknown && "ring-2 ring-destructive",
            isDragging && "scale-105 shadow-2xl ring-2 ring-primary motion-reduce:scale-100",
          )}
        />
        <span className="absolute -top-1 -left-1 flex size-5 items-center justify-center rounded-full border bg-background text-[10px] font-semibold text-muted-foreground tabular">
          {position + 1}
        </span>
        {info?.needs && (
          <Tooltip>
            <TooltipTrigger asChild>
              <span
                className="absolute right-0.5 bottom-0.5 size-3 rounded-full bg-warning ring-2 ring-card"
                aria-label={`Needs ${info.needs}`}
              />
            </TooltipTrigger>
            <TooltipContent>Needs {info.needs}</TooltipContent>
          </Tooltip>
        )}
      </div>
      {canRemove && (
        <Button
          variant="outline"
          size="icon-xs"
          onClick={onRemove}
          aria-label={`Remove ${title}`}
          className={cn(
            "absolute -top-1 -right-1 rounded-full bg-background opacity-0 shadow-sm transition-opacity group-focus-within/tile:opacity-100 group-hover/tile:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100",
            dragging && "hidden",
          )}
        >
          <XIcon />
        </Button>
      )}
      {!last && (
        <ChevronRightIcon
          aria-hidden
          className={cn(
            "absolute top-[calc(50%-1.75rem)] -right-[18px] size-3.5 text-muted-foreground/50 transition-opacity sm:-right-[22px]",
            dragging && "opacity-0",
          )}
        />
      )}
      <span className="flex max-w-full flex-col items-center">
        {loading ? (
          <Skeleton className="h-3.5 w-14" />
        ) : (
          <span className={cn("max-w-full truncate text-xs font-medium", unknown && "text-destructive")}>
            {unknown ? "Unknown role" : title}
          </span>
        )}
        <span
          className={cn(
            "flex h-4 items-center gap-1 text-[10px] font-medium tracking-wide uppercase",
            live ? "text-primary" : "text-transparent",
          )}
          aria-hidden={!live}
        >
          <span className={cn("size-1.5 rounded-full", live ? "animate-pulse bg-primary" : "bg-transparent")} />
          On now
        </span>
      </span>
    </li>
  )
}

/** One segment per role: how the screen spends a full cycle. */
function Timeline({
  roles,
  cycleSecs,
  live,
  hover,
  onHover,
  label,
}: {
  roles: string[]
  cycleSecs: number
  live?: ScreenStatus
  hover: string | null
  onHover: (r: string | null) => void
  label: (r: string) => string
}) {
  const total = roles.length * cycleSecs
  return (
    <div className="flex flex-col gap-1.5 px-5 pb-5">
      <div className="flex h-2 gap-0.5 overflow-hidden rounded-full" role="list" aria-label="Cycle timeline">
        {roles.map((r, i) => {
          const on = live?.showing === "role" && live.role === r
          return (
            <Tooltip key={r}>
              <TooltipTrigger asChild>
                <div
                  role="listitem"
                  aria-label={`${label(r)}: ${i * cycleSecs}–${(i + 1) * cycleSecs} s`}
                  onMouseEnter={() => onHover(r)}
                  onMouseLeave={() => onHover(null)}
                  className={cn(
                    "relative flex-1 overflow-hidden bg-muted transition-colors duration-150",
                    hover === r && "bg-primary/50",
                  )}
                >
                  {on && roles.length > 1 && (
                    <div
                      className="absolute inset-y-0 left-0 bg-primary transition-[width] duration-[1500ms] ease-linear"
                      style={{ width: `${Math.round(live.cycle_progress * 100)}%` }}
                    />
                  )}
                </div>
              </TooltipTrigger>
              <TooltipContent>
                {label(r)} · {i * cycleSecs}–{(i + 1) * cycleSecs} s
              </TooltipContent>
            </Tooltip>
          )
        })}
      </div>
      <div className="flex justify-between text-[11px] text-muted-foreground tabular">
        <span>0 s</span>
        <span>{total} s</span>
      </div>
    </div>
  )
}

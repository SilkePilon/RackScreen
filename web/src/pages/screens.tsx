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
import { restrictToParentElement, restrictToVerticalAxis } from "@dnd-kit/modifiers"
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { CheckIcon, GripVerticalIcon, LayoutTemplateIcon, PlusIcon, TriangleAlertIcon, XIcon } from "lucide-react"
import { Link } from "react-router"
import { toast } from "sonner"

import { HeaderActions, PageBody, PageIntro } from "@/components/page"
import { PanelMirror } from "@/components/panel-mirror"
import { NumberInput, ResetButton } from "@/components/settings"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Skeleton } from "@/components/ui/skeleton"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft } from "@/lib/draft"
import { useRoles } from "@/lib/queries"
import { PRESETS, prettyTitle, roleLabel } from "@/lib/roles"
import type { RoleInfo, ScreenCfg } from "@/lib/types"
import { cn } from "@/lib/utils"

export default function Screens() {
  const d = useDraft()
  const screens = d.draft?.screens
  return (
    <PageBody>
      <Presets />
      <PageIntro title="Screens">
        Each screen cycles through its roles, a few seconds each. Drag to reorder; the first role shows first.
      </PageIntro>
      <div className="flex flex-col gap-6">
        {screens
          ? screens.map((s, i) => <ScreenCard key={i} index={i} screen={s} />)
          : Array.from({ length: 4 }, (_, i) => <Skeleton key={i} className="h-48 rounded-xl" />)}
      </div>
    </PageBody>
  )
}

function Presets() {
  const d = useDraft()
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
        <DropdownMenuContent align="end" className="w-72">
          <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
            Replace the roles of the first screens
          </DropdownMenuLabel>
          <DropdownMenuGroup>
            {PRESETS.map((p) => (
              <DropdownMenuItem
                key={p.id}
                onSelect={() => apply(p.rows, p.title)}
                className="flex-col items-start gap-0.5"
              >
                <span className="font-medium">{p.title}</span>
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

function ScreenCard({ index, screen }: { index: number; screen: ScreenCfg }) {
  const d = useDraft()
  const roles = useRoles()
  const base = `screens.${index}`
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )
  const setRoles = (r: string[]) => d.set(`${base}.roles`, r)
  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return
    const from = screen.roles.indexOf(String(e.active.id))
    const to = screen.roles.indexOf(String(e.over.id))
    setRoles(arrayMove(screen.roles, from, to))
  }
  const rolesDirty = d.isDirty(`${base}.roles`)
  const cycleDirty = d.isDirty(`${base}.cycle_secs`)

  return (
    <section
      aria-label={`Screen ${index + 1}`}
      data-setting-id={`screen-${index}`}
      className="overflow-hidden rounded-xl border bg-card"
    >
      <header className="flex flex-wrap items-center gap-4 border-b border-hairline px-5 py-4">
        <PanelMirror screen={index} className="w-14 shrink-0 p-[4px]" />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <div className="flex items-center gap-1.5">
            <h2 className="text-[15px] font-semibold tracking-tight">Screen {index + 1}</h2>
            {rolesDirty && <ResetButton onClick={() => d.reset(`${base}.roles`)} label="Reset roles to saved" />}
          </div>
          <p className="text-xs text-muted-foreground tabular">
            SPI {screen.spi} · CS {screen.cs} · rotated {screen.rotate}°{screen.hflip ? " · mirrored" : ""}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {cycleDirty && <ResetButton onClick={() => d.reset(`${base}.cycle_secs`)} />}
          <label htmlFor={`cycle-${index}`} className="text-[13px] text-muted-foreground">
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
        onDragEnd={onDragEnd}
        modifiers={[restrictToVerticalAxis, restrictToParentElement]}
      >
        <SortableContext items={screen.roles} strategy={verticalListSortingStrategy}>
          <ol className="flex flex-col p-2" aria-label={`Roles on screen ${index + 1}`}>
            {screen.roles.map((r, i) => (
              <RoleItem
                key={r}
                name={r}
                position={i}
                info={roles.data?.find((x) => x.name === r)}
                loading={roles.isPending}
                canRemove={screen.roles.length > 1}
                onRemove={() => setRoles(screen.roles.filter((x) => x !== r))}
              />
            ))}
          </ol>
        </SortableContext>
      </DndContext>
      <footer className="flex items-center justify-between gap-3 border-t border-hairline px-5 py-3">
        <span className="text-xs text-muted-foreground">
          {screen.roles.length === 1
            ? "One role: the screen never changes."
            : `${screen.roles.length} roles, a full cycle every ${screen.roles.length * screen.cycle_secs} s.`}
        </span>
        <AddRole screen={screen} onAdd={(r) => setRoles([...screen.roles, r])} />
      </footer>
    </section>
  )
}

function RoleItem({
  name,
  position,
  info,
  loading,
  canRemove,
  onRemove,
}: {
  name: string
  position: number
  info?: RoleInfo
  loading: boolean
  canRemove: boolean
  onRemove: () => void
}) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id: name,
  })
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={cn(
        "group/role relative flex items-center gap-3 rounded-lg px-2 py-2 transition-colors hover:bg-accent/60",
        isDragging && "z-10 bg-accent shadow-lg ring-1 ring-border",
      )}
    >
      <button
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        aria-label={`Move ${roleLabel(name, info ? [info] : undefined)}`}
        className="flex size-6 cursor-grab touch-none items-center justify-center rounded text-muted-foreground/60 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none active:cursor-grabbing"
      >
        <GripVerticalIcon className="size-4" />
      </button>
      <span className="w-4 text-center text-xs text-muted-foreground tabular">{position + 1}</span>
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {loading ? (
          <Skeleton className="h-4 w-24" />
        ) : (
          <span className="truncate text-sm font-medium">
            {name.startsWith("gif:") ? `GIF · ${name.slice(4)}` : info ? prettyTitle(info.title) : name}
          </span>
        )}
        <span className="hidden truncate font-mono text-xs text-muted-foreground sm:inline">{name}</span>
        {!loading && !info && (
          <Badge variant="destructive" className="font-normal">
            Unknown role
          </Badge>
        )}
        {info?.needs && <NeedsBadge needs={info.needs} />}
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <span>
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={onRemove}
              disabled={!canRemove}
              aria-label={`Remove ${name}`}
              className="text-muted-foreground opacity-0 group-hover/role:opacity-100 focus-visible:opacity-100 disabled:opacity-0"
            >
              <XIcon />
            </Button>
          </span>
        </TooltipTrigger>
        <TooltipContent>{canRemove ? "Remove" : "A screen needs at least one role"}</TooltipContent>
      </Tooltip>
    </li>
  )
}

const NEEDS_PAGE: [RegExp, string][] = [
  [/electricity/i, "/integrations/energy"],
  [/location/i, "/integrations/sky"],
  [/github/i, "/integrations/github"],
  [/argo/i, "/integrations/cluster"],
  [/jellyfin/i, "/integrations/media"],
  [/spotify/i, "/integrations/spotify"],
]

function NeedsBadge({ needs }: { needs: string }) {
  const to = NEEDS_PAGE.find(([re]) => re.test(needs))?.[1]
  const badge = (
    <Badge variant="outline" className="gap-1 border-warning/40 font-normal text-warning">
      <TriangleAlertIcon className="size-3" />
      Needs {needs}
    </Badge>
  )
  return to ? (
    <Link to={to} className="rounded-full focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none">
      {badge}
    </Link>
  ) : (
    badge
  )
}

function AddRole({ screen, onAdd }: { screen: ScreenCfg; onAdd: (r: string) => void }) {
  const roles = useRoles()
  const [open, setOpen] = React.useState(false)
  const groups = React.useMemo(() => {
    const m = new Map<string, RoleInfo[]>()
    for (const r of roles.data ?? []) {
      if (!m.has(r.group)) m.set(r.group, [])
      m.get(r.group)!.push(r)
    }
    return [...m.entries()]
  }, [roles.data])
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm">
          <PlusIcon data-icon="inline-start" />
          Add role
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[22rem] p-0">
        <Command loop>
          <CommandInput placeholder="Find a role…" />
          <CommandList className="scrollbar-thin max-h-80">
            <CommandEmpty>{roles.isPending ? "Loading roles…" : "No role matches."}</CommandEmpty>
            {groups.map(([group, list]) => (
              <CommandGroup key={group} heading={group}>
                {list.map((r) => {
                  const added = screen.roles.includes(r.name)
                  return (
                    <CommandItem
                      key={r.name}
                      value={`${r.name} ${r.title} ${r.group} ${r.description}`}
                      disabled={added}
                      onSelect={() => {
                        onAdd(r.name)
                        setOpen(false)
                      }}
                      className="items-start"
                    >
                      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span className="flex items-center gap-2">
                          <span className="font-medium">
                            {r.name.startsWith("gif:") ? r.name.slice(4) : prettyTitle(r.title)}
                          </span>
                          {r.needs && (
                            <span className="flex items-center gap-1 text-xs text-warning">
                              <TriangleAlertIcon className="size-3" /> {r.needs}
                            </span>
                          )}
                        </span>
                        <span className="line-clamp-1 text-xs text-muted-foreground">{r.description}</span>
                      </div>
                      {added && <CheckIcon className="mt-0.5 text-muted-foreground" />}
                    </CommandItem>
                  )
                })}
              </CommandGroup>
            ))}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}

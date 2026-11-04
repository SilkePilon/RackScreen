import * as React from "react"
import { CheckIcon, PlusIcon, SearchIcon, TriangleAlertIcon } from "lucide-react"
import { Link } from "react-router"

import { RolePreview } from "@/components/role-preview"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { useRoles } from "@/lib/queries"
import { prettyTitle } from "@/lib/roles"
import type { RoleInfo } from "@/lib/types"
import { cn } from "@/lib/utils"

/** The name a person reads: "Air quality", or the GIF's own name. */
export function roleTitle(name: string, info?: RoleInfo): string {
  if (name.startsWith("gif:")) return name.slice(4)
  return info ? prettyTitle(info.title) : name
}

const NEEDS_PAGE: [RegExp, string][] = [
  [/electricity/i, "/integrations/energy"],
  [/location|weather/i, "/integrations/sky"],
  [/github/i, "/integrations/github"],
  [/argo/i, "/integrations/cluster"],
  [/jellyfin/i, "/integrations/media"],
  [/spotify/i, "/integrations/spotify"],
]

export function needsPage(needs: string): string | undefined {
  return NEEDS_PAGE.find(([re]) => re.test(needs))?.[1]
}

export function NeedsBadge({ needs }: { needs: string }) {
  const to = needsPage(needs)
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

/**
 * Pick a role by how it looks: a searchable grid of panel previews with a
 * detail pane for the one in focus.
 */
export function RoleGallery({
  open,
  onOpenChange,
  screen,
  taken,
  onAdd,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 0-based screen index, for the title. */
  screen: number
  /** Roles already on the screen. */
  taken: string[]
  onAdd: (name: string) => void
}) {
  const roles = useRoles()
  const [query, setQuery] = React.useState("")
  const [group, setGroup] = React.useState<string | null>(null)
  const [selected, setSelected] = React.useState<string | null>(null)

  // Start fresh each time the dialog opens.
  const [wasOpen, setWasOpen] = React.useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) {
      setQuery("")
      setGroup(null)
      setSelected(null)
    }
  }

  const groups = React.useMemo(() => [...new Set((roles.data ?? []).map((r) => r.group))], [roles.data])
  const list = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return (roles.data ?? []).filter(
      (r) =>
        (!group || r.group === group) &&
        (!q || `${r.name} ${r.title} ${r.group} ${r.description}`.toLowerCase().includes(q)),
    )
  }, [roles.data, query, group])

  // Keep the detail pane on something that is still in view.
  const current =
    list.find((r) => r.name === selected) ?? list.find((r) => !taken.includes(r.name)) ?? list[0] ?? null

  const add = (r: RoleInfo) => {
    if (taken.includes(r.name)) return
    onAdd(r.name)
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="gap-1 border-b border-hairline px-5 pt-5 pb-4">
          <DialogTitle>Add a role to Screen {screen + 1}</DialogTitle>
          <DialogDescription>Pick what the panel shows. Every preview is the real panel design.</DialogDescription>
          <div className="relative mt-3">
            <SearchIcon className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Find a role…"
              aria-label="Find a role"
              className="pl-9"
              autoFocus
            />
          </div>
          <div className="scrollbar-none -mx-5 mt-3 flex gap-1.5 overflow-x-auto px-5" role="group" aria-label="Group">
            {[null, ...groups].map((g) => (
              <button
                key={g ?? "all"}
                type="button"
                aria-pressed={group === g}
                onClick={() => setGroup(g)}
                className={cn(
                  "h-7 shrink-0 rounded-full border px-3 text-xs font-medium transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                  group === g
                    ? "border-foreground bg-foreground text-background"
                    : "border-border text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                {g ?? "All"}
              </button>
            ))}
          </div>
        </DialogHeader>

        <div className="grid min-h-0 md:grid-cols-[minmax(0,1fr)_15rem]">
          <div className="scrollbar-thin max-h-[42vh] overflow-y-auto p-3 md:max-h-[26rem]">
            {roles.isPending ? (
              <div className="grid grid-cols-3 gap-2 sm:grid-cols-4">
                {Array.from({ length: 8 }, (_, i) => (
                  <Skeleton key={i} className="aspect-square rounded-full" />
                ))}
              </div>
            ) : list.length === 0 ? (
              <p className="py-12 text-center text-sm text-muted-foreground">No role matches.</p>
            ) : (
              <ul className="grid grid-cols-3 gap-1 sm:grid-cols-4" aria-label="Roles">
                {list.map((r) => {
                  const added = taken.includes(r.name)
                  const active = current?.name === r.name
                  const title = roleTitle(r.name, r)
                  return (
                    <li key={r.name}>
                      <button
                        type="button"
                        aria-pressed={active}
                        aria-label={`${title}${added ? ", already on this screen" : ""}`}
                        onClick={() => setSelected(r.name)}
                        onDoubleClick={() => add(r)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault()
                            add(r)
                          }
                        }}
                        onFocus={() => setSelected(r.name)}
                        className={cn(
                          "group/tile flex w-full flex-col items-center gap-2 rounded-xl px-1.5 pt-2.5 pb-2 transition-colors focus-visible:outline-none",
                          active ? "bg-accent" : "hover:bg-accent/60",
                        )}
                      >
                        <span className="relative w-full max-w-[84px]">
                          <RolePreview
                            name={r.name}
                            label={title}
                            dim={added}
                            className={cn(
                              "w-full ring-offset-2 ring-offset-popover transition-shadow duration-150",
                              active && "ring-2 ring-primary",
                            )}
                          />
                          {added && (
                            <span className="absolute -right-0.5 -bottom-0.5 flex size-5 items-center justify-center rounded-full bg-foreground text-background ring-2 ring-popover">
                              <CheckIcon className="size-3" />
                            </span>
                          )}
                          {!added && r.needs && (
                            <span className="absolute top-0.5 right-0.5 size-2.5 rounded-full bg-warning ring-2 ring-popover" />
                          )}
                        </span>
                        <span className="line-clamp-1 text-center text-xs font-medium">{title}</span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            )}
          </div>

          <aside
            className="flex flex-col gap-3 border-t border-hairline bg-muted/40 p-4 md:border-t-0 md:border-l"
            aria-live="polite"
          >
            {current ? (
              <Detail info={current} added={taken.includes(current.name)} screen={screen} onAdd={() => add(current)} />
            ) : (
              <Skeleton className="h-40 w-full rounded-xl" />
            )}
          </aside>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function Detail({
  info,
  added,
  screen,
  onAdd,
}: {
  info: RoleInfo
  added: boolean
  screen: number
  onAdd: () => void
}) {
  const title = roleTitle(info.name, info)
  return (
    <>
      <div className="flex items-center gap-4 md:flex-col md:items-stretch">
        <RolePreview name={info.name} label={title} className="w-20 md:mx-auto md:w-40" />
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-baseline gap-2">
            <h3 className="truncate text-[15px] font-semibold tracking-tight">{title}</h3>
            <span className="shrink-0 text-xs text-muted-foreground">{info.group}</span>
          </div>
          <p className="line-clamp-3 text-xs text-pretty text-muted-foreground md:line-clamp-none">
            {info.description}
          </p>
        </div>
      </div>
      {info.needs && (
        <div>
          <NeedsBadge needs={info.needs} />
        </div>
      )}
      <Button onClick={onAdd} disabled={added} className="mt-auto w-full">
        {added ? (
          <>
            <CheckIcon data-icon="inline-start" /> On Screen {screen + 1}
          </>
        ) : (
          <>
            <PlusIcon data-icon="inline-start" /> Add to Screen {screen + 1}
          </>
        )}
      </Button>
    </>
  )
}

import * as React from "react"
import { useNavigate } from "react-router"
import {
  CornerDownLeftIcon,
  MoonIcon,
  PlayIcon,
  RotateCwIcon,
  ScanEyeIcon,
  SlidersHorizontalIcon,
  SunIcon,
} from "lucide-react"

import { useTheme } from "@/components/theme-provider"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command"
import { api } from "@/lib/api"
import { PAGES, pageFor } from "@/lib/nav"
import { useControl } from "@/lib/queries"
import { SETTINGS } from "@/lib/settings"

function isTyping(t: EventTarget | null) {
  return (
    t instanceof HTMLElement &&
    (t.isContentEditable || !!t.closest("input, textarea, select, [contenteditable='true'], .cm-editor"))
  )
}

/** Every typed word must appear in the item; earlier matches rank higher. */
function paletteFilter(value: string, search: string) {
  const v = value.toLowerCase()
  const words = search.toLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return 1
  let score = 1
  for (const w of words) {
    const i = v.indexOf(w)
    if (i === -1) return 0
    score += 1 / (1 + i)
  }
  return score
}

export function useCommandPalette() {
  const [open, setOpen] = React.useState(false)
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === "k" || e.key === "K") && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen((o) => !o)
      } else if (e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !isTyping(e.target)) {
        e.preventDefault()
        setOpen(true)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])
  return { open, setOpen }
}

export function CommandPalette({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const navigate = useNavigate()
  const { theme, setTheme } = useTheme()
  const identify = useControl(() => api.identify(5), () => "Each panel shows its number for 5 s")
  const play = useControl(() => api.play("random"), () => "Playing a random show")
  const restart = useControl(() => api.restart(), () => "Panels restarting")
  const night = useControl(
    (m: "on" | "off" | "auto") => api.night(m),
    (m) => (m === "auto" ? "Night mode follows the schedule" : `Night mode ${m}`),
  )

  const run = (fn: () => void) => {
    onOpenChange(false)
    fn()
  }

  const settings = React.useMemo(
    () =>
      Object.entries(SETTINGS).map(([id, m]) => ({
        id,
        ...m,
        pageTitle: pageFor(m.page)?.title ?? "",
      })),
    [],
  )

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Search"
      description="Jump to a page or a setting, or run an action."
      className="max-w-xl! border bg-popover shadow-2xl shadow-black/60"
    >
      <Command loop className="bg-popover" filter={paletteFilter}>
        <CommandInput placeholder="Search pages, settings and actions…" />
        <CommandList className="scrollbar-thin max-h-[min(60vh,420px)]">
          <CommandEmpty>Nothing matches. Try “brightness” or “spotify”.</CommandEmpty>
          <CommandGroup heading="Pages">
            {PAGES.map((p) => (
              <CommandItem
                key={p.path}
                value={`page ${p.title} ${p.group} ${p.keywords ?? ""}`}
                onSelect={() => run(() => navigate(p.path))}
              >
                <p.icon />
                <span>{p.title}</span>
                {p.group === "Integrations" && (
                  <span className="text-xs text-muted-foreground">Integrations</span>
                )}
              </CommandItem>
            ))}
          </CommandGroup>
          <CommandGroup heading="Actions">
            <CommandItem value="identify panels show numbers" onSelect={() => run(() => identify.mutate(undefined))}>
              <ScanEyeIcon />
              Identify panels
            </CommandItem>
            <CommandItem value="play random show now" onSelect={() => run(() => play.mutate(undefined))}>
              <PlayIcon />
              Play a random show
            </CommandItem>
            <CommandItem value="night mode on dim" onSelect={() => run(() => night.mutate("on"))}>
              <MoonIcon />
              Night mode on
            </CommandItem>
            <CommandItem value="night mode off" onSelect={() => run(() => night.mutate("off"))}>
              <SunIcon />
              Night mode off
            </CommandItem>
            <CommandItem value="night mode auto schedule" onSelect={() => run(() => night.mutate("auto"))}>
              <MoonIcon />
              Night mode follows the schedule
            </CommandItem>
            <CommandItem value="restart panels monitor" onSelect={() => run(() => restart.mutate(undefined))}>
              <RotateCwIcon />
              Restart the panels
            </CommandItem>
            <CommandItem
              value="toggle theme dark light appearance"
              onSelect={() => run(() => setTheme(theme === "light" ? "dark" : "light"))}
            >
              {theme === "light" ? <MoonIcon /> : <SunIcon />}
              Switch to {theme === "light" ? "dark" : "light"} theme
            </CommandItem>
          </CommandGroup>
          <CommandGroup heading="Settings">
            {settings.map((s) => (
              <CommandItem
                key={s.id}
                value={`setting ${s.title} ${s.section} ${s.pageTitle} ${s.description} ${s.keywords ?? ""} ${s.id}`}
                onSelect={() => run(() => navigate(`${s.page}#${s.id}`))}
              >
                <SlidersHorizontalIcon />
                <div className="flex min-w-0 flex-col">
                  <span className="truncate">
                    {s.title}
                    {s.title !== s.section && <span className="text-muted-foreground"> · {s.section}</span>}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">{s.description}</span>
                </div>
                <CommandShortcut className="hidden sm:inline">{s.pageTitle}</CommandShortcut>
              </CommandItem>
            ))}
          </CommandGroup>
        </CommandList>
        <div className="flex items-center justify-between border-t px-3 py-2 text-xs text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <CornerDownLeftIcon className="size-3" /> to open
          </span>
          <span>Esc to close</span>
        </div>
      </Command>
    </CommandDialog>
  )
}

import * as React from "react"
import { ArrowDownIcon, CopyIcon, PauseIcon, PlayIcon, SearchIcon, Trash2Icon, XIcon } from "lucide-react"
import { toast } from "sonner"

import { copyText } from "@/components/copy-button"
import { HeaderActions, PageBody } from "@/components/page"
import { Segmented } from "@/components/segmented"
import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { api, errorMessage } from "@/lib/api"
import { openLogStream } from "@/lib/live"
import type { LogLevel, LogLine } from "@/lib/types"
import { cn } from "@/lib/utils"

const MAX = 2000
const RANK: Record<LogLevel, number> = { TRACE: 0, DEBUG: 1, INFO: 2, WARN: 3, ERROR: 4 }
type Filter = "all" | "info" | "warn" | "error"
const MIN: Record<Filter, number> = { all: 0, info: 2, warn: 3, error: 4 }

const LEVEL_STYLE: Record<LogLevel, string> = {
  TRACE: "text-muted-foreground/60",
  DEBUG: "text-muted-foreground",
  INFO: "text-primary",
  WARN: "text-warning",
  ERROR: "text-destructive",
}

function time(ms: number) {
  const d = new Date(ms)
  const p = (n: number, w = 2) => String(n).padStart(w, "0")
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`
}

function shortTarget(t: string) {
  return t.replace(/^rackscreen_/, "")
}

function lineText(l: LogLine) {
  return `${new Date(l.unix_ms).toISOString()} ${l.level.padEnd(5)} ${l.target}: ${l.msg}`
}

function Highlight({ text, q }: { text: string; q: string }) {
  if (!q) return <>{text}</>
  const parts = text.split(new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig"))
  return (
    <>
      {parts.map((p, i) =>
        i % 2 ? (
          <mark key={i} className="rounded-[3px] bg-warning/30 text-foreground">
            {p}
          </mark>
        ) : (
          p
        ),
      )}
    </>
  )
}

/** Page through the ring from the start and keep the newest MAX lines. */
async function loadTail(): Promise<LogLine[]> {
  let all: LogLine[] = []
  let after = 0
  for (let i = 0; i < 50; i++) {
    const page = await api.logs(after, MAX)
    if (page.lines.length) all = [...all, ...page.lines].slice(-MAX)
    const next = page.lines.at(-1)?.seq ?? page.next
    if (!page.lines.length || next <= after || page.lines.length < MAX) break
    after = next
  }
  return all
}

export default function Logs() {
  const [lines, setLines] = React.useState<LogLine[] | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [live, setLive] = React.useState<"connecting" | "live" | "down">("connecting")
  const [paused, setPaused] = React.useState(false)
  const [follow, setFollow] = React.useState(true)
  const [filter, setFilter] = React.useState<Filter>("all")
  const [query, setQuery] = React.useState("")
  const [held, setHeld] = React.useState(0)
  const pausedRef = React.useRef(false)
  const buffer = React.useRef<LogLine[]>([])
  const lastSeq = React.useRef(0)
  const scroller = React.useRef<HTMLDivElement>(null)
  const search = React.useRef<HTMLInputElement>(null)

  React.useEffect(() => {
    pausedRef.current = paused
    if (!paused && buffer.current.length) {
      const add = buffer.current
      buffer.current = []
      setHeld(0)
      setLines((l) => [...(l ?? []), ...add].slice(-MAX))
    }
  }, [paused])

  React.useEffect(() => {
    let alive = true
    // While a baseline fetch runs, stream lines wait here and are merged after.
    let syncing = true
    let pending: LogLine[] = []
    let wasDown = false

    const push = (l: LogLine) => {
      lastSeq.current = l.seq
      if (pausedRef.current) {
        buffer.current.push(l)
        if (buffer.current.length > MAX) buffer.current.shift()
        setHeld(buffer.current.length)
      } else setLines((prev) => [...(prev ?? []), l].slice(-MAX))
    }

    // Load the newest MAX lines and restart sequence tracking from them. Runs
    // at mount, after an SSE reconnect and when the sequence goes backwards
    // (the service restarted and its log ring started over at 1).
    const baseline = async () => {
      syncing = true
      try {
        const all = await loadTail()
        if (!alive) return
        buffer.current = []
        setHeld(0)
        setError(null)
        setLines(all)
        lastSeq.current = all.at(-1)?.seq ?? 0
        const waiting = pending
        pending = []
        syncing = false
        // Lines that arrived during the fetch: keep the ones it did not include.
        // After a restart the stream may carry seqs below lastSeq from the old
        // process; those are already gone from the ring, so skip them too.
        for (const l of waiting) if (l.seq > lastSeq.current) push(l)
      } catch (e) {
        if (!alive) return
        syncing = false
        pending = []
        setError(errorMessage(e))
      }
    }

    const source = openLogStream()
    source.onopen = () => {
      setLive("live")
      if (wasDown) {
        wasDown = false
        void baseline()
      }
    }
    source.onerror = () => {
      wasDown = true
      setLive("down")
    }
    source.addEventListener("log", (ev) => {
      setLive("live")
      let l: LogLine
      try {
        l = JSON.parse(ev.data) as LogLine
      } catch {
        return
      }
      if (syncing) {
        pending.push(l)
        return
      }
      if (l.seq < lastSeq.current) {
        // The sequence went backwards: a new process. Start over.
        pending.push(l)
        void baseline()
        return
      }
      if (l.seq === lastSeq.current) return
      push(l)
    })
    void baseline()
    return () => {
      alive = false
      source.close()
    }
  }, [])

  const q = query.trim().toLowerCase()
  const shown = React.useMemo(
    () =>
      (lines ?? []).filter(
        (l) =>
          RANK[l.level] >= MIN[filter] &&
          (!q || l.msg.toLowerCase().includes(q) || l.target.toLowerCase().includes(q)),
      ),
    [lines, filter, q],
  )

  React.useLayoutEffect(() => {
    if (follow && scroller.current) scroller.current.scrollTop = scroller.current.scrollHeight
  }, [shown, follow])

  const onScroll = () => {
    const el = scroller.current
    if (!el) return
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24
    if (atBottom !== follow) setFollow(atBottom)
  }

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === "f" || e.key === "F") && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        search.current?.focus()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  const copy = async () => {
    const ok = await copyText(shown.map(lineText).join("\n"))
    if (ok) toast.success(`Copied ${shown.length} lines`)
    else toast.error("Could not copy")
  }

  return (
    <PageBody wide className="gap-4 pb-6 md:pt-6">
      <HeaderActions>
        <span
          className={cn(
            "mr-2 hidden items-center gap-1.5 text-xs sm:flex",
            live === "live" && !paused ? "text-success" : "text-muted-foreground",
          )}
          role="status"
        >
          <span
            className={cn(
              "size-1.5 rounded-full",
              paused ? "bg-muted-foreground" : live === "live" ? "bg-success animate-pulse" : "bg-warning",
            )}
          />
          {paused ? "Paused" : live === "live" ? "Live" : live === "down" ? "Reconnecting" : "Connecting"}
        </span>
      </HeaderActions>
      <div className="flex flex-wrap items-center gap-2">
        <InputGroup className="w-full sm:w-72">
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            ref={search}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter messages"
            aria-label="Filter messages"
          />
          {query && (
            <InputGroupAddon align="inline-end">
              <InputGroupButton size="icon-xs" aria-label="Clear filter" onClick={() => setQuery("")}>
                <XIcon />
              </InputGroupButton>
            </InputGroupAddon>
          )}
        </InputGroup>
        <Segmented
          label="Minimum level"
          value={filter}
          onChange={setFilter}
          options={[
            { value: "all", label: "All" },
            { value: "info", label: "Info" },
            { value: "warn", label: "Warn" },
            { value: "error", label: "Error" },
          ]}
        />
        <div className="flex-1" />
        <Button
          variant="outline"
          size="sm"
          onClick={() => setPaused((p) => !p)}
          aria-pressed={paused}
          aria-label={paused ? (held ? `Resume live view, ${held} new lines` : "Resume live view") : "Pause live view"}
        >
          {paused ? <PlayIcon data-icon="inline-start" /> : <PauseIcon data-icon="inline-start" />}
          {paused ? (held ? `Resume · ${held} new` : "Resume") : "Pause"}
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void copy()}
          disabled={!shown.length}
          aria-label={`Copy ${shown.length} shown lines`}
        >
          <CopyIcon data-icon="inline-start" />
          Copy
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setLines([])} aria-label="Clear view">
          <Trash2Icon />
        </Button>
      </div>
      <div className="relative h-[calc(100svh-15rem)] min-h-[360px] overflow-hidden rounded-xl border bg-card">
        <div
          ref={scroller}
          onScroll={onScroll}
          className="scrollbar-thin absolute inset-0 overflow-auto py-2 font-mono text-[12.5px] leading-[1.7]"
          role="log"
          aria-label="Service log"
          aria-live={paused ? "off" : "polite"}
          aria-relevant="additions"
        >
          {error ? (
            <p className="px-4 py-3 text-destructive">Could not load the logs: {error}</p>
          ) : !lines ? (
            <div className="flex flex-col gap-2 px-4 py-2">
              {Array.from({ length: 14 }, (_, i) => (
                <Skeleton key={i} className="h-3.5" style={{ width: `${40 + ((i * 37) % 55)}%` }} />
              ))}
            </div>
          ) : shown.length === 0 ? (
            <p className="px-4 py-3 font-sans text-sm text-muted-foreground">
              {lines.length ? "No line matches the filter." : "No log lines yet."}
            </p>
          ) : (
            shown.map((l) => (
              <div
                key={l.seq}
                className={cn(
                  "flex gap-3 px-4 whitespace-pre-wrap hover:bg-accent/40",
                  l.level === "ERROR" && "bg-destructive/[0.06]",
                  l.level === "WARN" && "bg-warning/[0.04]",
                )}
              >
                <span className="shrink-0 text-muted-foreground/70 tabular select-none">{time(l.unix_ms)}</span>
                <span className={cn("w-11 shrink-0 font-medium", LEVEL_STYLE[l.level])}>{l.level}</span>
                <span className="hidden w-40 shrink-0 truncate text-muted-foreground lg:inline" title={l.target}>
                  {shortTarget(l.target)}
                </span>
                <span className="min-w-0 flex-1 break-words text-foreground/90">
                  <Highlight text={l.msg} q={q} />
                </span>
              </div>
            ))
          )}
        </div>
        {!follow && shown.length > 0 && (
          <Button
            size="sm"
            variant="secondary"
            className="absolute right-4 bottom-4 shadow-lg animate-in fade-in-0 slide-in-from-bottom-1"
            onClick={() => setFollow(true)}
            aria-label="Scroll to the latest line"
          >
            <ArrowDownIcon data-icon="inline-start" />
            Latest
          </Button>
        )}
      </div>
      <p className="text-xs text-muted-foreground tabular">
        {shown.length} of {lines?.length ?? 0} lines · the service keeps the last {MAX}.
      </p>
    </PageBody>
  )
}

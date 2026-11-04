import * as React from "react"
import { useInfiniteQuery, useQuery } from "@tanstack/react-query"
import { KeyRoundIcon, SearchIcon, SearchXIcon, TrendingUpIcon, XIcon } from "lucide-react"
import { toast } from "sonner"

import { Segmented } from "@/components/segmented"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, ApiError, errorMessage } from "@/lib/api"
import { useDraft } from "@/lib/draft"
import { gifNameError, suggestName } from "@/lib/gif"
import { formatMb, gifMb, keptFrames, MAX_FRAMES, rememberFrames } from "@/lib/gif-memory"
import type { Config, GifCfg, GifHit, GifProvider } from "@/lib/types"
import { cn } from "@/lib/utils"
import { BudgetWarning } from "@/pages/integrations/gif-memory"

const PROVIDER_NAME: Record<GifProvider, string> = { giphy: "GIPHY", tenor: "Tenor" }
const NO_SCREEN = "none"

/** Add `gif` to the config, and `gif:<name>` to screen `screen` when given. */
export function addGif(c: Config, gif: GifCfg, screen: number | null): Config {
  return {
    ...c,
    gifs: [...(c.gifs ?? []), gif],
    screens:
      screen === null
        ? c.screens
        : c.screens.map((s, i) => (i === screen ? { ...s, roles: [...s.roles, `gif:${gif.name}`] } : s)),
  }
}

/** "Don't add to a screen" or one screen; the role goes at the end of its list. */
export function ScreenSelect({
  value,
  onChange,
  id,
}: {
  value: number | null
  onChange: (v: number | null) => void
  id?: string
}) {
  const d = useDraft()
  const screens = d.draft?.screens ?? []
  return (
    <Select
      value={value === null ? NO_SCREEN : String(value)}
      onValueChange={(v) => onChange(v === NO_SCREEN ? null : Number(v))}
    >
      <SelectTrigger id={id} className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectItem value={NO_SCREEN}>Not on a screen yet</SelectItem>
          {screens.map((s, i) => (
            <SelectItem key={i} value={String(i)}>
              Show on screen {i + 1}
              <span className="text-muted-foreground">
                {" "}
                · {s.roles.length} {s.roles.length === 1 ? "role" : "roles"}
              </span>
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = React.useState(value)
  React.useEffect(() => {
    const t = setTimeout(() => setV(value), ms)
    return () => clearTimeout(t)
  }, [value, ms])
  return v
}

/** Scroll to a setting row and flash it, like a #hash jump. */
function jumpTo(id: string) {
  const el = document.querySelector<HTMLElement>(`[data-setting-id="${CSS.escape(id)}"]`)
  if (!el) return
  el.scrollIntoView({ block: "center", behavior: "smooth" })
  el.classList.remove("setting-flash")
  void el.offsetWidth
  el.classList.add("setting-flash")
  setTimeout(() => el.classList.remove("setting-flash"), 2000)
}

export function GifSearch() {
  const d = useDraft()
  const giphySet = !!d.secrets["gif_search.giphy_key"]
  const tenorSet = !!d.secrets["gif_search.tenor_key"]
  // keyed on the saved secrets, so a save with a new key asks again
  const providers = useQuery({
    queryKey: ["gifs", "providers", giphySet, tenorSet],
    queryFn: api.gifProviders,
    staleTime: Infinity,
  })
  const available = (["giphy", "tenor"] as const).filter((p) => providers.data?.[p])
  const [chosen, setChosen] = React.useState<GifProvider>("giphy")
  const provider = available.includes(chosen) ? chosen : (available[0] ?? chosen)
  const [text, setText] = React.useState("")
  const q = useDebounced(text.trim(), 300)
  const [pick, setPick] = React.useState<GifHit | null>(null)
  const inputRef = React.useRef<HTMLInputElement>(null)

  if (providers.isPending) {
    return (
      <div className="flex flex-col gap-4 p-5">
        <Skeleton className="h-9 w-full rounded-md" />
        <SkeletonGrid count={8} />
      </div>
    )
  }
  if (providers.isError) {
    return (
      <div className="p-5">
        <Alert variant="destructive">
          <AlertTitle>Can't check the search keys</AlertTitle>
          <AlertDescription>{errorMessage(providers.error)}</AlertDescription>
        </Alert>
      </div>
    )
  }
  if (available.length === 0) {
    return (
      <Empty className="py-10">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <KeyRoundIcon />
          </EmptyMedia>
          <EmptyTitle>Add a search key</EmptyTitle>
          <EmptyDescription>
            Searching needs a free API key. Get a GIPHY key at{" "}
            <a href="https://developers.giphy.com/dashboard/" target="_blank" rel="noreferrer">
              developers.giphy.com
            </a>{" "}
            (create an app, pick API), or a Tenor key as a Google Cloud API key with the Tenor API enabled.
            Paste it under Search keys below and save. Pasting a link works without one.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="outline" onClick={() => jumpTo("gif_search.giphy_key")}>
            <KeyRoundIcon data-icon="inline-start" />
            Go to the key fields
          </Button>
        </EmptyContent>
      </Empty>
    )
  }

  return (
    <div className="flex flex-col gap-4 p-5" data-setting-id="gif-search">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <InputGroup className="flex-1">
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            ref={inputRef}
            value={text}
            onChange={(e) => setText(e.target.value.slice(0, 100))}
            onKeyDown={(e) => {
              if (e.key === "Escape" && text) {
                e.preventDefault()
                setText("")
              }
            }}
            placeholder={`Search ${PROVIDER_NAME[provider]}`}
            aria-label="Search GIFs"
            spellCheck={false}
            autoComplete="off"
            enterKeyHint="search"
          />
          {text && (
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                size="icon-xs"
                aria-label="Clear search"
                onClick={() => {
                  setText("")
                  inputRef.current?.focus()
                }}
              >
                <XIcon />
              </InputGroupButton>
            </InputGroupAddon>
          )}
        </InputGroup>
        <Segmented
          label="Search provider"
          value={provider}
          onChange={setChosen}
          options={(["giphy", "tenor"] as const).map((p) => ({
            value: p,
            label: PROVIDER_NAME[p],
            disabled: !providers.data?.[p],
            aria: providers.data?.[p] ? PROVIDER_NAME[p] : `${PROVIDER_NAME[p]} (no key)`,
          }))}
        />
      </div>
      <TrendingTerms
        provider={provider}
        active={q}
        onPick={(t) => {
          setText(t)
          inputRef.current?.focus()
        }}
      />
      <Results key={`${provider}|${q}`} provider={provider} q={q} onPick={setPick} />
      <AddPickDialog pick={pick} query={q} provider={provider} onClose={() => setPick(null)} />
    </div>
  )
}

function TrendingTerms({
  provider,
  active,
  onPick,
}: {
  provider: GifProvider
  active: string
  onPick: (t: string) => void
}) {
  const terms = useQuery({
    queryKey: ["gifs", "terms", provider],
    queryFn: () => api.gifTrendingTerms(provider),
    staleTime: 30 * 60_000,
    retry: false,
  })
  if (terms.isPending)
    return (
      <div className="flex gap-1.5 overflow-hidden" aria-hidden>
        {[56, 72, 48, 88, 64, 52].map((w, i) => (
          <Skeleton key={i} className="h-7 shrink-0 rounded-full" style={{ width: w }} />
        ))}
      </div>
    )
  if (!terms.data?.terms.length) return null
  return (
    <div
      className="-mx-1 flex scrollbar-thin items-center gap-1.5 overflow-x-auto px-1 pb-0.5"
      aria-label="Popular searches"
    >
      <TrendingUpIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
      {terms.data.terms.map((t) => (
        <button
          key={t}
          type="button"
          onClick={() => onPick(t)}
          aria-pressed={active.toLowerCase() === t.toLowerCase()}
          className={cn(
            "h-7 shrink-0 rounded-full border px-3 text-[12.5px] whitespace-nowrap text-muted-foreground transition-colors",
            "hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
            "aria-pressed:border-primary/40 aria-pressed:bg-primary/10 aria-pressed:text-foreground"
          )}
        >
          {t}
        </button>
      ))}
    </div>
  )
}

const SKELETON_RATIOS = [0.56, 1, 0.75, 1.33, 0.6, 0.9, 1.2, 0.7, 0.56, 1, 0.8, 1.1]

function SkeletonGrid({ count }: { count: number }) {
  return (
    <div className="columns-2 gap-2 sm:columns-3 md:columns-4" aria-hidden>
      {Array.from({ length: count }, (_, i) => (
        <Skeleton
          key={i}
          className="mb-2 w-full break-inside-avoid rounded-lg"
          style={{ aspectRatio: `1 / ${SKELETON_RATIOS[i % SKELETON_RATIOS.length]}` }}
        />
      ))}
    </div>
  )
}

function Results({ provider, q, onPick }: { provider: GifProvider; q: string; onPick: (h: GifHit) => void }) {
  const query = useInfiniteQuery({
    queryKey: ["gifs", provider, q],
    queryFn: ({ pageParam }) =>
      q ? api.gifSearch(provider, q, pageParam) : api.gifTrending(provider, pageParam),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next,
    staleTime: 10 * 60_000,
    retry: (n, e) => !(e instanceof ApiError && e.status >= 400 && e.status < 500) && n < 1,
  })
  const hits = React.useMemo(() => {
    const seen = new Set<string>()
    return (query.data?.pages ?? [])
      .flatMap((p) => p.results)
      .filter((h) => !seen.has(h.id) && !!seen.add(h.id))
  }, [query.data])

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between gap-3">
        <h3 className="text-[13px] font-medium text-foreground">
          {q ? (
            <>
              Results for <span className="text-muted-foreground">“{q}”</span>
            </>
          ) : (
            "Trending now"
          )}
        </h3>
        <span className="text-xs text-muted-foreground">
          {provider === "giphy" ? "Powered by GIPHY" : "Via Tenor"}
        </span>
      </div>
      {query.isPending ? (
        <SkeletonGrid count={12} />
      ) : query.isError ? (
        <Alert variant="destructive">
          <AlertTitle>Search failed</AlertTitle>
          <AlertDescription>{errorMessage(query.error)}</AlertDescription>
        </Alert>
      ) : hits.length === 0 ? (
        <Empty className="py-8">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <SearchXIcon />
            </EmptyMedia>
            <EmptyTitle>No GIFs found</EmptyTitle>
            <EmptyDescription>Try another word, or the other provider.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ResultsScroll
          label={q ? `Results for ${q}` : "Trending GIFs"}
          more={query.hasNextPage && !query.isFetchNextPageError ? () => void query.fetchNextPage() : undefined}
          busy={query.isFetchingNextPage}
        >
          <ul className="columns-2 gap-2 sm:columns-3 md:columns-4">
            {hits.map((h) => (
              <li key={h.id} className="mb-2 break-inside-avoid">
                <Tile hit={h} onClick={() => onPick(h)} />
              </li>
            ))}
            {query.isFetchingNextPage &&
              Array.from({ length: 4 }, (_, i) => (
                <li key={`s${i}`} className="mb-2 break-inside-avoid" aria-hidden>
                  <Skeleton
                    className="w-full rounded-lg"
                    style={{ aspectRatio: `1 / ${SKELETON_RATIOS[i]}` }}
                  />
                </li>
              ))}
          </ul>
          {query.hasNextPage && (
            <div className="flex justify-center py-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => void query.fetchNextPage()}
                disabled={query.isFetchingNextPage}
              >
                {query.isFetchingNextPage && <Spinner data-icon="inline-start" />}
                Load more
              </Button>
            </div>
          )}
          {query.isFetchNextPageError && (
            <p className="pb-2 text-center text-xs text-destructive">Couldn't load more. Try again.</p>
          )}
        </ResultsScroll>
      )}
    </div>
  )
}

function Tile({ hit, onClick }: { hit: GifHit; onClick: () => void }) {
  const [loaded, setLoaded] = React.useState(false)
  const label = hit.title || "Untitled GIF"
  return (
    <button
      type="button"
      onClick={onClick}
      title={hit.frames ? `${label} · ${hit.frames} frames, ~${formatMb(gifMb(hit.frames))} on the Pi` : label}
      aria-label={`Add ${label}`}
      className="group relative block w-full overflow-hidden rounded-lg bg-muted ring-1 ring-border transition-[box-shadow,transform] hover:ring-2 hover:ring-primary/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none active:scale-[0.98]"
      style={{ aspectRatio: `${hit.width || 1} / ${hit.height || 1}` }}
    >
      {!loaded && <Skeleton className="absolute inset-0 rounded-none" />}
      <img
        src={hit.preview}
        alt=""
        loading="lazy"
        decoding="async"
        referrerPolicy="no-referrer"
        onLoad={() => setLoaded(true)}
        className={cn(
          "absolute inset-0 size-full object-cover transition-opacity",
          loaded ? "opacity-100" : "opacity-0"
        )}
      />
      <span className="pointer-events-none absolute inset-x-0 bottom-0 flex items-end gap-2 bg-gradient-to-t from-black/80 to-transparent px-2 pt-6 pb-1.5 text-left text-xs text-white opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {hit.frames ? <span className="shrink-0 text-white/75 tabular">~{formatMb(gifMb(hit.frames))}</span> : null}
      </span>
    </button>
  )
}

function AddPickDialog({
  pick,
  query,
  provider,
  onClose,
}: {
  pick: GifHit | null
  query: string
  provider: GifProvider
  onClose: () => void
}) {
  const d = useDraft()
  const gifs = d.draft?.gifs ?? []
  const [name, setName] = React.useState("")
  const [screen, setScreen] = React.useState<number | null>(null)
  const [touched, setTouched] = React.useState(false)
  const [openedFor, setOpenedFor] = React.useState<string | null>(null)
  if ((pick?.id ?? null) !== openedFor) {
    setOpenedFor(pick?.id ?? null)
    if (pick) {
      setName(
        suggestName(
          query || pick.title,
          gifs.map((g) => g.name)
        )
      )
      setScreen(null)
      setTouched(false)
    }
  }
  const n = name.trim()
  const nameErr = gifNameError(n, gifs, null)
  const dupe = pick ? gifs.find((g) => g.url.trim() === pick.url) : undefined

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setTouched(true)
    if (!pick || nameErr || dupe) return
    rememberFrames(pick.url, pick.frames)
    d.update((c) => addGif(c, { name: n, url: pick.url }, screen))
    toast.success(`Added ${n}`, {
      description: screen === null ? "Save to keep it." : `On screen ${screen + 1} once you save.`,
    })
    onClose()
  }

  return (
    <Dialog open={!!pick} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">
        {pick && (
          <form onSubmit={submit} className="flex flex-col gap-5">
            <DialogHeader>
              <DialogTitle>Add this GIF</DialogTitle>
              <DialogDescription className="truncate">
                {pick.title || "Untitled"} · {PROVIDER_NAME[provider]}
              </DialogDescription>
            </DialogHeader>
            <div className="flex items-center gap-4 rounded-lg border bg-muted/40 p-3">
              <div className="flex size-20 shrink-0 items-center justify-center overflow-hidden rounded-full bg-black ring-1 ring-border">
                <img
                  src={pick.preview}
                  alt="Preview"
                  referrerPolicy="no-referrer"
                  className="size-full object-cover"
                />
              </div>
              <div className="flex flex-col gap-1 text-[13px] text-muted-foreground">
                <p>Roughly how it sits on a round panel. The Pi downloads the full GIF once.</p>
                <p className="text-foreground tabular">
                  {pick.frames
                    ? `${pick.frames} frames · about ${formatMb(gifMb(pick.frames))} of memory${pick.frames > MAX_FRAMES ? `, ${keptFrames(pick.frames)} frames kept` : ""}`
                    : `Up to ${formatMb(gifMb(null))} of memory`}
                </p>
              </div>
            </div>
            <FieldGroup>
              <Field data-invalid={(touched && !!nameErr) || undefined}>
                <FieldLabel htmlFor="pick-name">Name</FieldLabel>
                <Input
                  id="pick-name"
                  value={name}
                  autoFocus
                  onChange={(e) => setName(e.target.value.toLowerCase().replace(/\s+/g, "-"))}
                  className="font-mono"
                  aria-invalid={(touched && !!nameErr) || undefined}
                  spellCheck={false}
                  autoComplete="off"
                />
                {(touched || n) && nameErr ? (
                  <FieldError>{nameErr}</FieldError>
                ) : (
                  <FieldDescription>
                    The role is <span className="font-mono">gif:{n || "name"}</span>.
                  </FieldDescription>
                )}
              </Field>
              <Field>
                <FieldLabel htmlFor="pick-screen">Screen</FieldLabel>
                <ScreenSelect id="pick-screen" value={screen} onChange={setScreen} />
              </Field>
            </FieldGroup>
            {screen !== null && !dupe && <BudgetWarning mb={gifMb(pick.frames)} known={!!pick.frames} />}
            {dupe && <p className="text-[13px] text-destructive">{dupe.name} already is this GIF.</p>}
            <DialogFooter>
              <Button type="button" variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button type="submit" disabled={!!dupe}>
                Add GIF
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  )
}

/**
 * A fixed-height box the results scroll in vertically; the next page loads
 * by itself when the bottom comes into view.
 */
function ResultsScroll({
  label,
  more,
  busy,
  children,
}: {
  label: string
  more?: () => void
  busy: boolean
  children: React.ReactNode
}) {
  const box = React.useRef<HTMLDivElement>(null)
  const end = React.useRef<HTMLDivElement>(null)
  React.useEffect(() => {
    if (!more || busy || !box.current || !end.current) return
    const io = new IntersectionObserver((e) => e.some((x) => x.isIntersecting) && more(), {
      root: box.current,
      rootMargin: "0px 0px 240px 0px",
    })
    io.observe(end.current)
    return () => io.disconnect()
  }, [more, busy])
  return (
    <div
      ref={box}
      role="region"
      aria-label={label}
      tabIndex={0}
      className="-mr-2 h-[440px] scrollbar-thin overflow-x-hidden overflow-y-auto overscroll-contain rounded-lg pr-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      {children}
      <div ref={end} className="h-px" aria-hidden />
    </div>
  )
}

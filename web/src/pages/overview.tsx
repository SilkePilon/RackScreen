import * as React from "react"
import { Link } from "react-router"
import {
  ArrowUpCircleIcon,
  CheckIcon,
  ChevronDownIcon,
  CpuIcon,
  MoonIcon,
  PlayIcon,
  RotateCwIcon,
  ScanEyeIcon,
  ShuffleIcon,
  TriangleAlertIcon,
} from "lucide-react"

import { PageBody } from "@/components/page"
import { PanelMirror } from "@/components/panel-mirror"
import { TsLogo } from "@/components/ts-logo"
import { Segmented } from "@/components/segmented"
import { Section, SettingsCard } from "@/components/settings"
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Skeleton } from "@/components/ui/skeleton"
import { api } from "@/lib/api"
import { useDraft } from "@/lib/draft"
import { nightLabel, useControl, useRoles, useShows, useStatus, useTailscale, useUpdate } from "@/lib/queries"
import { prettyTitle, roleLabel } from "@/lib/roles"
import { tailscaleAddress } from "@/lib/tailscale"
import type { LinkState, NightMode, ScreenStatus, Tone } from "@/lib/types"
import { cn } from "@/lib/utils"

const SHOWING: Record<string, string> = {
  show: "Show",
  night: "Night",
  pattern: "Test pattern",
  identify: "Identify",
}

function PanelCard({ s, sampledAt }: { s: ScreenStatus; sampledAt: number }) {
  const roles = useRoles()
  const jump = useControl(
    (role: string) => api.jumpRole(s.index, role),
    (role) => `Screen ${s.index + 1} → ${roleLabel(role, roles.data)}`,
  )
  return (
    <div className="flex min-w-0 flex-col items-center gap-3 sm:gap-4">
      <PanelMirror
        screen={s.index}
        cycle={{
          active: s.showing === "role" && s.roles.length > 1,
          role: s.role,
          progress: s.cycle_progress,
          cycleSecs: s.cycle_secs,
          at: sampledAt,
        }}
        className="max-w-[220px]"
      />
      <div className="flex w-full flex-col items-center gap-1">
        <span className="text-[11px] font-medium tracking-wider text-muted-foreground uppercase tabular">
          Screen {s.index + 1}
        </span>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" className="h-8 max-w-full gap-1 px-2 text-sm font-medium sm:h-7">
              <span className="truncate">{s.role.startsWith("gif:") ? roleLabel(s.role) : prettyTitle(s.title)}</span>
              <ChevronDownIcon data-icon="inline-end" className="text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="center" className="min-w-52">
            <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
              Show now on screen {s.index + 1}
            </DropdownMenuLabel>
            <DropdownMenuGroup>
              {s.roles.map((r) => (
                <DropdownMenuItem key={r} onSelect={() => jump.mutate(r)}>
                  {r === s.role ? <CheckIcon /> : <span className="size-4" />}
                  {roleLabel(r, roles.data)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuItem asChild>
                <Link to="/screens">Edit roles…</Link>
              </DropdownMenuItem>
            </DropdownMenuGroup>
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="flex min-h-5 max-w-full items-center gap-1.5 text-center text-xs text-muted-foreground tabular">
          {s.showing !== "role" ? (
            <Badge variant="secondary" className="h-5 font-normal">
              {SHOWING[s.showing]}
            </Badge>
          ) : s.roles.length > 1 ? (
            <span>
              {s.roles.indexOf(s.role) + 1} of {s.roles.length} · {s.cycle_secs} s
            </span>
          ) : (
            <span>Single role</span>
          )}
        </div>
      </div>
    </div>
  )
}

function Panels() {
  const status = useStatus()
  const { saved } = useDraft()
  const screens = status.data?.screens
  // Physical order, left to right, when the config says so.
  const order = saved?.shows.order
  const ordered =
    screens && order && order.length === screens.length
      ? order.map((i) => screens[i]).filter(Boolean)
      : screens
  return (
    <div className="grid grid-cols-2 gap-x-4 gap-y-8 rounded-2xl border bg-card px-4 py-6 sm:gap-x-6 sm:gap-y-10 sm:px-8 sm:py-8 lg:grid-cols-4">
      {ordered
        ? ordered.map((s) => <PanelCard key={s.index} s={s} sampledAt={(status.data?.received ?? 0) - (status.data?.age_ms ?? 0)} />)
        : Array.from({ length: 4 }, (_, i) => (
            <div key={i} className="flex flex-col items-center gap-4">
              <Skeleton className="aspect-square w-full max-w-[220px] rounded-full" />
              <Skeleton className="h-3 w-16" />
              <Skeleton className="h-4 w-24" />
            </div>
          ))}
    </div>
  )
}

function QuickActions() {
  const status = useStatus()
  const shows = useShows()
  const identify = useControl(() => api.identify(5), () => "Each panel shows its number for 5 s")
  const play = useControl(
    (id: string) => api.play(id),
    (id) => (id === "random" ? "Playing a random show" : `Playing ${shows.data?.find((s) => s.id === id)?.title ?? id}`),
  )
  const night = useControl((m: NightMode) => api.night(m), (m) => nightLabel[m])
  const restart = useControl(() => api.restart(), () => "Panels restarting")
  const mode = status.data?.night.mode ?? "auto"
  const nightActive = status.data?.night.active

  return (
    <SettingsCard className="sm:grid sm:grid-cols-2 sm:gap-px sm:divide-y-0 sm:bg-hairline xl:flex xl:flex-row xl:gap-0 xl:divide-x xl:bg-card [&>*]:bg-card">
      <div className="flex flex-1 flex-col gap-3 p-4">
        <ActionTitle icon={ScanEyeIcon} title="Identify" hint="Number on each panel" />
        <Button variant="outline" onClick={() => identify.mutate(undefined)} disabled={identify.isPending}>
          Show numbers
        </Button>
      </div>
      <div className="flex flex-1 flex-col gap-3 p-4">
        <ActionTitle icon={PlayIcon} title="Shows" hint="Across all four panels" />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" disabled={nightActive}>
              Play a show
              <ChevronDownIcon data-icon="inline-end" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="max-h-80 min-w-56">
            <DropdownMenuGroup>
              <DropdownMenuItem onSelect={() => play.mutate("random")}>
                <ShuffleIcon />
                Random
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              {shows.data?.map((s) => (
                <DropdownMenuItem key={s.id} onSelect={() => play.mutate(s.id)}>
                  {s.title}
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="flex flex-1 flex-col gap-3 p-4">
        <ActionTitle
          icon={MoonIcon}
          title="Night mode"
          hint={nightActive ? "Dimmed right now" : "Awake right now"}
        />
        <Segmented
          label="Night mode"
          value={mode}
          onChange={(m) => night.mutate(m)}
          className="w-full [&>*]:flex-1"
          options={[
            { value: "auto", label: "Auto" },
            { value: "on", label: "On" },
            { value: "off", label: "Off" },
          ]}
        />
      </div>
      <div className="flex flex-1 flex-col gap-3 p-4">
        <ActionTitle icon={RotateCwIcon} title="Restart" hint="Panels go dark ~1 s" />
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="outline">Restart panels</Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Restart the panels?</AlertDialogTitle>
              <AlertDialogDescription>
                The monitor reloads the config and reconnects to every source. The panels go dark for about a
                second; this page stays up.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction onClick={() => restart.mutate(undefined)}>Restart</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </SettingsCard>
  )
}

function ActionTitle({ icon: Icon, title, hint }: { icon: React.ElementType; title: string; hint: string }) {
  return (
    <div className="flex items-start gap-2.5">
      <Icon className="mt-0.5 size-4 text-muted-foreground" strokeWidth={1.6} />
      <div className="flex flex-col">
        <span className="text-sm font-medium">{title}</span>
        <span className="text-xs text-muted-foreground">{hint}</span>
      </div>
    </div>
  )
}

const TONE: Record<Tone, string> = {
  ok: "text-muted-foreground",
  warn: "text-warning",
  bad: "text-destructive",
  muted: "text-muted-foreground",
}

function Tiles() {
  const status = useStatus()
  const tiles = status.data?.tiles
  return (
    <div className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border bg-hairline sm:grid-cols-3">
      {tiles
        ? tiles.map((t) => (
            <div key={t.key} className="flex min-w-0 flex-col gap-2 bg-card px-5 py-4 max-sm:last:odd:col-span-2">
              <span className={cn("text-[13px]", TONE[t.tone])}>
                {t.label}
              </span>
              <span className="flex min-w-0 items-baseline gap-1.5">
                <span className="truncate text-2xl font-semibold tracking-tight tabular">{t.value}</span>
                {t.unit && <span className="truncate text-sm text-muted-foreground">{t.unit}</span>}
              </span>
            </div>
          ))
        : Array.from({ length: 6 }, (_, i) => (
            <div key={i} className="flex flex-col gap-3 bg-card px-5 py-4">
              <Skeleton className="h-3 w-24" />
              <Skeleton className="h-6 w-14" />
            </div>
          ))}
    </div>
  )
}

const LINK: Record<LinkState, { text: string; label: string }> = {
  ok: { text: "", label: "Connected" },
  warn: { text: "text-warning", label: "Degraded" },
  down: { text: "text-destructive", label: "Down" },
  off: { text: "text-muted-foreground", label: "Off" },
}

function Links() {
  const status = useStatus()
  const links = status.data?.links
  return (
    <SettingsCard>
      {links
        ? links.map((l) => (
            <div key={l.name} className="flex items-center gap-3 px-5 py-3">
              <span className="sr-only">{LINK[l.state].label}:</span>
              <span className={cn("shrink-0 text-sm font-medium", LINK[l.state].text)}>{l.name}</span>
              <span className="ml-auto truncate text-right text-[13px] text-muted-foreground">{l.detail}</span>
            </div>
          ))
        : Array.from({ length: 5 }, (_, i) => (
            <div key={i} className="flex items-center gap-3 px-5 py-3.5">
              <Skeleton className="h-3.5 w-28" />
              <Skeleton className="ml-auto h-3 w-24" />
            </div>
          ))}
    </SettingsCard>
  )
}

function Host() {
  const status = useStatus()
  const d = status.data
  const rows: [string, string][] = d
    ? [
        ["CPU temperature", d.host.cpu_temp_c !== null ? `${d.host.cpu_temp_c.toFixed(1)} °C` : "n/a"],
        ["Load", d.host.load1 !== null ? d.host.load1.toFixed(2) : "n/a"],
        [
          "Memory",
          d.host.mem_used_mb !== null && d.host.mem_total_mb !== null
            ? `${d.host.mem_used_mb} / ${d.host.mem_total_mb} MB`
            : "n/a",
        ],
        ...d.stats.map(
          (s): [string, string] => [
            `Screen ${s.index + 1}`,
            `${s.render_ms.toFixed(1)} ms render · ${s.push_ms.toFixed(1)} ms push · ${s.fps} fps`,
          ],
        ),
      ]
    : []
  return (
    <SettingsCard>
      {d
        ? rows.map(([k, v]) => (
            <div key={k} className="flex flex-wrap items-center justify-between gap-x-4 gap-y-0.5 px-5 py-3 sm:flex-nowrap">
              <span className="shrink-0 text-sm text-muted-foreground">{k}</span>
              <span className="ml-auto min-w-0 text-right text-[13px] text-foreground tabular sm:truncate">{v}</span>
            </div>
          ))
        : Array.from({ length: 5 }, (_, i) => (
            <div key={i} className="flex items-center justify-between px-5 py-3.5">
              <Skeleton className="h-3.5 w-24" />
              <Skeleton className="h-3 w-40" />
            </div>
          ))}
    </SettingsCard>
  )
}

function Banners() {
  const update = useUpdate()
  const status = useStatus()
  const err = status.data?.monitor.error
  return (
    <>
      {err && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>The panels stopped</AlertTitle>
          <AlertDescription className="font-mono text-xs">{err}</AlertDescription>
        </Alert>
      )}
      {update.data?.newer && (
        <Alert className="border-primary/30 bg-primary/[0.06]">
          <ArrowUpCircleIcon className="text-primary!" />
          <AlertTitle>RackScreen {update.data.latest} is available</AlertTitle>
          <AlertDescription>You run v{update.data.current}. Updating takes about a minute.</AlertDescription>
          <AlertAction>
            <Button size="sm" asChild>
              <Link to="/system#update">Update</Link>
            </Button>
          </AlertAction>
        </Alert>
      )}
    </>
  )
}

/** "Tailscale · <MagicDNS name>" when the Pi is on a tailnet. */
function TailscaleChip() {
  const t = useTailscale().data
  const dns = tailscaleAddress(t)
  if (!dns) return null
  return (
    <Link
      to="/connections/tailscale"
      className="inline-flex h-6 max-w-[16rem] items-center gap-1.5 rounded-full border bg-card px-2.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      title={t?.serve.url ?? dns}
    >
      <TsLogo className="size-3.5" />
      <span className="truncate">
        Tailscale · <span className="font-mono">{dns.split(".")[0]}</span>
      </span>
    </Link>
  )
}

export default function Overview() {
  return (
    <PageBody wide className="gap-12">
      <Banners />
      <Section title="Panels" description="Live from the rack. Pick a role to show it right now.">
        <Panels />
      </Section>
      <Section title="Quick actions">
        <QuickActions />
      </Section>
      <Section title="At a glance">
        <Tiles />
      </Section>
      <div className="grid gap-12 lg:grid-cols-2 lg:gap-8">
        <Section title="Connections" description="Every source the panels read from.">
          <Links />
        </Section>
        <Section
          title="This Pi"
          action={<TailscaleChip />}
          description={
            <span className="inline-flex items-center gap-1.5">
              <CpuIcon className="size-3.5" /> Host and render timings.
            </span>
          }
        >
          <Host />
        </Section>
      </div>
    </PageBody>
  )
}

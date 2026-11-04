import * as React from "react"
import { Link } from "react-router"
import { AlertCircleIcon, TriangleAlertIcon, ZapIcon } from "lucide-react"

import { CopyButton } from "@/components/copy-button"
import { Meter } from "@/components/memory-bar"
import { Section, SettingsCard } from "@/components/settings"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Skeleton } from "@/components/ui/skeleton"
import { errorMessage } from "@/lib/api"
import { getCpuHistory, useSystem, useTailscale } from "@/lib/queries"
import { tailscaleAddress } from "@/lib/tailscale"
import type { SystemInfo } from "@/lib/types"
import { cn } from "@/lib/utils"

export function duration(secs: number) {
  const s = Math.max(0, Math.floor(secs))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  return d ? `${d} d ${h} h` : h ? `${h} h ${m} min` : `${m} min`
}

const fmt = (v: number | null | undefined, digits = 0, unit = "") =>
  v === null || v === undefined ? "n/a" : `${v.toFixed(digits)}${unit}`

const THROTTLE: Record<string, string> = {
  under_voltage: "under-voltage",
  freq_capped: "frequency capped",
  throttled: "throttled",
  soft_temp_limit: "soft temperature limit",
}

function throttleList(flags: string[], past: boolean) {
  return flags
    .filter((f) => f.endsWith("_since_boot") === past)
    .map((f) => THROTTLE[f.replace(/_since_boot$/, "")] ?? f)
}

export function ThisPi() {
  const q = useSystem()
  const s = q.data
  return (
    <Section title="This Pi" description="Live from the Pi, every 5 seconds.">
      {s && <PowerAlert s={s} />}
      {q.isError && !s ? (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>Can't read the system facts</AlertTitle>
          <AlertDescription>{errorMessage(q.error)}</AlertDescription>
        </Alert>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-px overflow-hidden rounded-xl border bg-hairline sm:grid-cols-2 lg:grid-cols-3">
            {s ? (
              <StatCards s={s} at={q.dataUpdatedAt} />
            ) : (
              Array.from({ length: 6 }, (_, i) => <StatSkeleton key={i} />)
            )}
          </div>
          <SettingsCard>{s ? <Facts s={s} /> : <FactsSkeleton />}</SettingsCard>
        </>
      )}
    </Section>
  )
}

function PowerAlert({ s }: { s: SystemInfo }) {
  const t = s.throttled
  if (!t || (!t.now && !t.since_boot)) return null
  const under = t.flags.includes("under_voltage")
  const underPast = t.flags.includes("under_voltage_since_boot")
  const now = throttleList(t.flags, false)
  const past = throttleList(t.flags, true)
  if (under || underPast) {
    return (
      <Alert
        variant={under ? "destructive" : "default"}
        className={
          under ? "border-destructive/30 bg-destructive/[0.06]" : "border-warning/30 bg-warning/[0.06]"
        }
      >
        <ZapIcon className={under ? undefined : "text-warning!"} />
        <AlertTitle>{under ? "Under-voltage right now" : "Under-voltage since the last boot"}</AlertTitle>
        <AlertDescription>
          The supply dipped below 4.63 V, so the Pi slows itself down and the SD card is at risk. Use a 5.1 V,
          2.5 A supply and a short, thick cable; four panels draw extra current.
        </AlertDescription>
      </Alert>
    )
  }
  return (
    <Alert className="border-warning/30 bg-warning/[0.06]">
      <TriangleAlertIcon className="text-warning!" />
      <AlertTitle>
        {t.now ? `The Pi is ${now.join(", ")}` : "The Pi was throttled since the last boot"}
      </AlertTitle>
      <AlertDescription>
        {t.now ? "It runs slower to stay cool." : `Seen: ${past.join(", ")}.`} Better airflow or a heatsink
        helps.
      </AlertDescription>
    </Alert>
  )
}

function Stat({
  label,
  value,
  unit,
  sub,
  children,
  tone,
}: {
  label: string
  value: React.ReactNode
  unit?: string
  sub?: React.ReactNode
  children?: React.ReactNode
  tone?: "ok" | "warn" | "bad"
}) {
  return (
    <div className="flex min-w-0 flex-col gap-2 bg-card px-5 py-4">
      <span
        className={cn(
          "text-[13px]",
          tone === "warn" ? "text-warning" : tone === "bad" ? "text-destructive" : "text-muted-foreground"
        )}
      >
        {label}
      </span>
      <span className="flex min-w-0 items-baseline gap-1.5">
        <span className="tabular truncate text-2xl font-semibold tracking-tight">{value}</span>
        {unit && <span className="truncate text-sm text-muted-foreground">{unit}</span>}
      </span>
      {children}
      {sub && <span className="tabular truncate text-xs text-muted-foreground">{sub}</span>}
    </div>
  )
}

function StatSkeleton() {
  return (
    <div className="flex flex-col gap-3 bg-card px-5 py-4">
      <Skeleton className="h-3 w-20" />
      <Skeleton className="h-6 w-24" />
      <Skeleton className="h-1.5 w-full rounded-full" />
      <Skeleton className="h-3 w-36" />
    </div>
  )
}

function Sparkline({ points, className }: { points: number[]; className?: string }) {
  const id = React.useId()
  if (points.length < 2) return <div className={cn("h-7", className)} aria-hidden />
  const w = 120
  const h = 28
  const step = w / (points.length - 1)
  const y = (v: number) => h - 1 - (Math.max(0, Math.min(100, v)) / 100) * (h - 2)
  const line = points.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)},${y(v).toFixed(1)}`).join(" ")
  return (
    <svg
      viewBox={`0 0 ${w} ${h}`}
      preserveAspectRatio="none"
      className={cn("h-7 w-full text-chart-1", className)}
      aria-hidden
    >
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="currentColor" stopOpacity="0.28" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={`${line} L${w},${h} L0,${h} Z`} fill={`url(#${id})`} />
      <path
        d={line}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        vectorEffect="non-scaling-stroke"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function StatCards({ s, at }: { s: SystemInfo; at: number }) {
  const cpu = s.cpu
  const hist = getCpuHistory().map((p) => p.pct)
  const temp = cpu.temp_c
  const mem = s.memory
  const disk = s.disk
  const t = s.throttled
  const memPct = mem ? (mem.used_mb / Math.max(mem.total_mb, 1)) * 100 : 0
  return (
    <>
      <Stat
        label="CPU"
        value={fmt(cpu.usage_pct, 0)}
        unit={cpu.usage_pct !== null ? `%${cpu.cores ? ` of ${cpu.cores} cores` : ""}` : undefined}
        tone={temp === null ? undefined : temp >= 80 ? "bad" : temp >= 70 ? "warn" : "ok"}
        sub={[
          temp !== null && `${temp.toFixed(1)} °C`,
          cpu.freq_mhz !== null && `${cpu.freq_mhz} MHz`,
          cpu.load && `load ${cpu.load[0].toFixed(2)}`,
        ]
          .filter(Boolean)
          .join(" · ")}
      >
        <Sparkline points={hist} />
      </Stat>
      <Stat
        label="Memory"
        value={mem ? mem.used_mb : "n/a"}
        unit={mem ? `of ${mem.total_mb} MB` : undefined}
        tone={mem ? (memPct >= 90 ? "bad" : memPct >= 80 ? "warn" : "ok") : undefined}
        sub={
          mem
            ? `${mem.available_mb} MB available${mem.swap_total_mb ? ` · swap ${mem.swap_used_mb} of ${mem.swap_total_mb} MB` : ""}`
            : undefined
        }
      >
        {mem && <Meter value={mem.used_mb} total={mem.total_mb} warnAt={0.8} className="my-1.5" />}
      </Stat>
      <Stat
        label="Disk"
        value={disk ? disk.used_gb.toFixed(1) : "n/a"}
        unit={disk ? `of ${disk.total_gb.toFixed(1)} GB` : undefined}
        sub={
          disk
            ? `${Math.round((disk.used_gb / Math.max(disk.total_gb, 0.1)) * 100)}% used`
            : undefined
        }
      >
        {disk && <Meter value={disk.used_gb} total={disk.total_gb} warnAt={0.9} className="my-1.5" />}
      </Stat>
      <Stat
        label="RackScreen"
        value={fmt(s.process.rss_mb, 0)}
        unit={s.process.rss_mb !== null ? "MB" : undefined}
        sub={[
          s.process.cpu_pct !== null && `${s.process.cpu_pct.toFixed(1)}% CPU`,
          s.process.threads !== null && `${s.process.threads} threads`,
          `up ${duration(s.process.uptime_secs)}`,
        ]
          .filter(Boolean)
          .join(" · ")}
      />
      <Stat
        label="Uptime"
        value={s.uptime_secs !== null ? duration(s.uptime_secs) : "n/a"}
        sub={
          s.uptime_secs !== null
            ? `since ${new Date(at - s.uptime_secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}`
            : undefined
        }
      />
      <Stat
        label="Power"
        value={!t ? "n/a" : t.flags.includes("under_voltage") ? "Under-voltage" : t.now ? "Throttled" : "OK"}
        tone={!t ? undefined : t.now ? "bad" : t.since_boot ? "warn" : "ok"}
        sub={
          !t
            ? "No firmware report here"
            : t.since_boot && !t.now
              ? `Since boot: ${throttleList(t.flags, true).join(", ")}`
              : t.now
                ? throttleList(t.flags, false).join(", ")
                : "No under-voltage or throttling since boot"
        }
      />
    </>
  )
}

function FactRow({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 px-5 py-3">
      <span className="shrink-0 text-sm text-muted-foreground">{k}</span>
      <span className="flex min-w-0 items-center gap-1 truncate text-right text-[13px] text-foreground">
        {children}
      </span>
    </div>
  )
}

function Facts({ s }: { s: SystemInfo }) {
  return (
    <>
      <FactRow k="Model">{s.model ?? "Not a Raspberry Pi"}</FactRow>
      <FactRow k="System">
        <span className="truncate">{[s.os, s.arch].filter(Boolean).join(" · ")}</span>
      </FactRow>
      <FactRow k="Kernel">
        <span className="truncate font-mono text-xs">{s.kernel ?? "n/a"}</span>
      </FactRow>
      <FactRow k="Hostname">
        <span className="font-mono text-xs">{s.hostname}</span>
      </FactRow>
      {s.network.length === 0 ? (
        <FactRow k="Network">No address</FactRow>
      ) : (
        s.network.map((n) => (
          <FactRow key={`${n.iface}-${n.ipv4}`} k={n.iface}>
            <span className="tabular font-mono text-xs">{n.ipv4}</span>
            <span className="-my-1">
              <CopyButton text={n.ipv4} iconOnly size="icon-xs" label={`Copy ${n.ipv4}`} variant="ghost" />
            </span>
          </FactRow>
        ))
      )}
      <TailscaleFacts />
    </>
  )
}

/** The Pi's Tailscale address and MagicDNS name, when it is on a tailnet. */
function TailscaleFacts() {
  const t = useTailscale().data
  const dns = tailscaleAddress(t)
  if (!t?.self || !dns) return null
  const ip = t.self.ips.find((i) => !i.includes(":"))
  const url = t.serve.on && t.serve.url ? t.serve.url : null
  return (
    <>
      {ip && (
        <FactRow k="tailscale0">
          <span className="tabular font-mono text-xs">{ip}</span>
          <span className="-my-1">
            <CopyButton text={ip} iconOnly size="icon-xs" label={`Copy ${ip}`} variant="ghost" />
          </span>
        </FactRow>
      )}
      <FactRow k="MagicDNS">
        {url ? (
          <a href={url} target="_blank" rel="noreferrer" className="truncate font-mono text-xs underline-offset-4 hover:underline">
            {url}
          </a>
        ) : (
          <Link to="/connections/tailscale" className="truncate font-mono text-xs underline-offset-4 hover:underline">
            {dns}
          </Link>
        )}
        <span className="-my-1">
          <CopyButton text={url ?? dns} iconOnly size="icon-xs" label={`Copy ${url ?? dns}`} variant="ghost" />
        </span>
      </FactRow>
    </>
  )
}

function FactsSkeleton() {
  return Array.from({ length: 5 }, (_, i) => (
    <div key={i} className="flex items-center justify-between px-5 py-3.5">
      <Skeleton className="h-3.5 w-20" />
      <Skeleton className="h-3 w-44" />
    </div>
  ))
}

import * as React from "react"
import { ChevronRightIcon, GridIcon, ScanEyeIcon } from "lucide-react"

import { HeaderActions, PageBody, PageIntro } from "@/components/page"
import { PanelMirror } from "@/components/panel-mirror"
import { Segmented } from "@/components/segmented"
import { NumberInput, Row, SettingsCard } from "@/components/settings"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { api } from "@/lib/api"
import { useDraft, useField } from "@/lib/draft"
import { useControl, useStatus } from "@/lib/queries"
import { cn } from "@/lib/utils"

const CLOCKS = [
  { hz: 40_000_000, label: "40 MHz" },
  { hz: 50_000_000, label: "50 MHz" },
  { hz: 66_666_666, label: "66.7 MHz" },
]

export default function Calibrate() {
  const d = useDraft()
  const status = useStatus()
  const patternOn = status.data?.screens.some((s) => s.showing === "pattern") ?? false
  const identify = useControl(() => api.identify(5), () => "Each panel shows its number for 5 s")
  const pattern = useControl(
    (on: boolean) => api.pattern(on),
    (on) => (on ? "Test pattern on; it turns off by itself after 10 min" : "Test pattern off"),
  )
  return (
    <PageBody>
      <HeaderActions>
        <Button variant="ghost" size="sm" onClick={() => identify.mutate(undefined)}>
          <ScanEyeIcon data-icon="inline-start" />
          <span className="hidden sm:inline">Identify</span>
        </Button>
        <Button
          variant={patternOn ? "secondary" : "ghost"}
          size="sm"
          onClick={() => pattern.mutate(!patternOn)}
          aria-pressed={patternOn}
        >
          <GridIcon data-icon="inline-start" />
          <span className="hidden sm:inline">{patternOn ? "Pattern on" : "Test pattern"}</span>
        </Button>
      </HeaderActions>
      <PageIntro title="Calibrate">
        Turn on the test pattern, then rotate and mirror each screen until its arrow points up and the colour bar
        reads red to violet. Each preview turns with your settings, like the glass would.
      </PageIntro>
      <div className="grid gap-6 md:grid-cols-2">
        {d.draft
          ? d.draft.screens.map((_, i) => <ScreenCalibration key={i} index={i} />)
          : Array.from({ length: 4 }, (_, i) => <Skeleton key={i} className="h-80 rounded-xl" />)}
      </div>
    </PageBody>
  )
}

function ScreenCalibration({ index }: { index: number }) {
  const base = `screens.${index}`
  const rotate = useField<number>(`${base}.rotate`)
  const hflip = useField<boolean>(`${base}.hflip`)
  const spi = useField<number>(`${base}.spi`)
  const cs = useField<number>(`${base}.cs`)
  const dc = useField<number>(`${base}.dc`)
  const rst = useField<number>(`${base}.rst`)
  const [open, setOpen] = React.useState(false)
  return (
    <section
      aria-label={`Screen ${index + 1}`}
      data-setting-id={`calibrate-${index}`}
      className="flex flex-col overflow-hidden rounded-xl border bg-card"
    >
      <div className="flex items-center gap-5 px-5 pt-5 pb-4">
        <div className="w-28 shrink-0">
          <div
            className="transition-transform duration-500 ease-out motion-reduce:transition-none"
            style={{ transform: `rotate(${rotate.value}deg) scaleX(${hflip.value ? -1 : 1})` }}
          >
            <PanelMirror screen={index} />
          </div>
        </div>
        <div className="flex flex-col gap-1">
          <h2 className="text-[15px] font-semibold tracking-tight">Screen {index + 1}</h2>
          <p className="text-[13px] text-muted-foreground tabular">
            Rotated {rotate.value}°{hflip.value ? ", mirrored" : ""}
          </p>
          <p className="text-xs text-muted-foreground/70 tabular">
            SPI {spi.value} · CS {cs.value} · DC {dc.value} · RST {rst.value}
          </p>
        </div>
      </div>
      <SettingsCard className="rounded-none border-x-0 border-b-0">
        <Row title="Rotation" description="Clockwise." dirty={rotate.dirty} onReset={rotate.reset}>
          <Segmented
            label={`Rotation of screen ${index + 1}`}
            value={String(rotate.value)}
            onChange={(v) => rotate.set(Number(v))}
            options={[0, 90, 180, 270].map((r) => ({ value: String(r), label: `${r}°` }))}
          />
        </Row>
        <Row title="Mirror" description="Flip left and right." dirty={hflip.dirty} onReset={hflip.reset}>
          <Switch checked={hflip.value} onCheckedChange={hflip.set} aria-label={`Mirror screen ${index + 1}`} />
        </Row>
        <Collapsible open={open} onOpenChange={setOpen}>
          <CollapsibleTrigger asChild>
            <button
              type="button"
              className="flex w-full items-center gap-2 px-5 py-3 text-left text-[13px] text-muted-foreground transition-colors hover:bg-accent/40 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-inset"
            >
              <ChevronRightIcon className={cn("size-4 transition-transform", open && "rotate-90")} />
              Wiring and SPI clock
            </button>
          </CollapsibleTrigger>
          <CollapsibleContent className="data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down overflow-hidden">
            <div className="flex flex-col divide-y divide-hairline border-t border-hairline">
              <PinRow base={base} field="spi" title="SPI bus" description="spidev bus number." max={6} />
              <PinRow base={base} field="cs" title="Chip select" description="CS line on that bus." max={3} />
              <PinRow base={base} field="dc" title="DC pin" description="GPIO (BCM) for data/command." max={27} />
              <PinRow base={base} field="rst" title="Reset pin" description="GPIO (BCM) wired to RST." max={27} />
              <ClockRow base={base} />
            </div>
          </CollapsibleContent>
        </Collapsible>
      </SettingsCard>
    </section>
  )
}

function PinRow({
  base,
  field,
  title,
  description,
  max,
}: {
  base: string
  field: string
  title: string
  description: string
  max: number
}) {
  const f = useField<number>(`${base}.${field}`)
  return (
    <Row title={title} description={description} dirty={f.dirty} onReset={f.reset} className="py-3">
      <NumberInput value={f.value} onChange={(v) => v !== null && f.set(v)} min={0} max={max} className="w-20" />
    </Row>
  )
}

function ClockRow({ base }: { base: string }) {
  const f = useField<number>(`${base}.hz`)
  const known = CLOCKS.some((c) => c.hz === f.value)
  const opts = known ? CLOCKS : [...CLOCKS, { hz: f.value, label: `${(f.value / 1e6).toFixed(1)} MHz` }]
  return (
    <Row
      title="SPI clock"
      description="The Pi 3B+ steps are 40, 50 and 66.7 MHz; 50 ran clean on all four panels."
      dirty={f.dirty}
      onReset={f.reset}
      className="py-3"
    >
      <Select value={String(f.value)} onValueChange={(v) => f.set(Number(v))}>
        <SelectTrigger className="w-32 tabular" aria-label="SPI clock">
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectGroup>
            {opts.map((c) => (
              <SelectItem key={c.hz} value={String(c.hz)}>
                {c.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </Row>
  )
}

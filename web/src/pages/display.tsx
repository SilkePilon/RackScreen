import { PageBody, PageIntro } from "@/components/page"
import { RestoreDefaults } from "@/components/restore-defaults"
import { Segmented } from "@/components/segmented"
import { TransitionsSection } from "@/components/transitions"
import { NumberRow, Row, RowSkeleton, Section, SettingsCard, SwitchRow, useEnabled } from "@/components/settings"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Slider } from "@/components/ui/slider"
import { api } from "@/lib/api"
import { useDraft, useField } from "@/lib/draft"
import { nightLabel, useControl, useStatus } from "@/lib/queries"
import type { NightMode } from "@/lib/types"

export default function Display() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/display" />
      <PageIntro title="Display">How bright and how fast the panels run, how roles change, when they sleep, and what counts as hot.</PageIntro>
      {ready ? (
        <Body />
      ) : (
        <>
          <RowSkeleton count={4} />
          <RowSkeleton count={4} />
          <RowSkeleton count={3} />
        </>
      )}
    </PageBody>
  )
}

function Brightness() {
  const f = useField<number>("display.brightness")
  return (
    <Row id="display.brightness" dirty={f.dirty} onReset={f.reset}>
      <div className="flex w-full items-center gap-4 sm:w-64">
        <Slider
          min={0.1}
          max={1}
          step={0.05}
          value={[f.value]}
          onValueChange={([v]) => f.set(Math.round(v * 100) / 100)}
          aria-label="Brightness"
        />
        <span className="w-11 text-right text-sm text-muted-foreground tabular">{Math.round(f.value * 100)}%</span>
      </div>
    </Row>
  )
}

const CHUNKS = [4096, 8192, 16384, 32768, 65536]

function SpiChunk() {
  const f = useField<number>("display.spi_chunk")
  const opts = CHUNKS.includes(f.value) ? CHUNKS : [...CHUNKS, f.value].sort((a, b) => a - b)
  return (
    <Row id="display.spi_chunk" dirty={f.dirty} onReset={f.reset}>
      <Select value={String(f.value)} onValueChange={(v) => f.set(Number(v))}>
        <SelectTrigger className="w-36 tabular" aria-label="SPI chunk">
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectGroup>
            {opts.map((n) => (
              <SelectItem key={n} value={String(n)} className="tabular">
                {n.toLocaleString("en-US")} B
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </Row>
  )
}

function TimeRow({ id, muted }: { id: string; muted?: boolean }) {
  const f = useField<string>(id)
  const bad = !/^([01]\d|2[0-3]):[0-5]\d$/.test(f.value ?? "")
  return (
    <Row id={id} dirty={f.dirty} onReset={f.reset} muted={muted}>
      <Input
        type="time"
        value={f.value}
        onChange={(e) => f.set(e.target.value)}
        aria-invalid={bad || undefined}
        aria-label={id === "night.start" ? "Night starts at" : "Night ends at"}
        className="w-32 tabular [&::-webkit-calendar-picker-indicator]:opacity-50 dark:[&::-webkit-calendar-picker-indicator]:invert"
      />
    </Row>
  )
}

function NightNow() {
  const status = useStatus()
  const night = useControl((m: NightMode) => api.night(m), (m) => nightLabel[m])
  const mode = status.data?.night.mode ?? "auto"
  return (
    <Row
      title="Right now"
      description={
        mode === "auto"
          ? `Following the schedule${status.data?.night.active ? "; it is night" : ""}. Not saved in the config.`
          : `Forced ${mode} until you switch back to Auto or the service restarts.`
      }
    >
      <Segmented
        label="Night mode right now"
        value={mode}
        onChange={(m) => night.mutate(m)}
        options={[
          { value: "auto", label: "Auto" },
          { value: "on", label: "On" },
          { value: "off", label: "Off" },
        ]}
      />
    </Row>
  )
}

function Body() {
  const night = useEnabled("night.enabled")
  return (
    <>
      <Section title="Panels">
        <SettingsCard>
          <Brightness />
          <NumberRow id="display.fps" min={1} max={60} unit="fps" />
          <SpiChunk />
          <SwitchRow id="display.one_at_a_time" />
        </SettingsCard>
      </Section>
      <TransitionsSection />
      <Section title="Night" description="The panels fade to a dim moon; shows never play at night.">
        <SettingsCard>
          <SwitchRow id="night.enabled" />
          <TimeRow id="night.start" muted={!night} />
          <TimeRow id="night.end" muted={!night} />
          <NightNow />
        </SettingsCard>
      </Section>
      <Section title="Thresholds" description="When a node turns the ring amber.">
        <SettingsCard>
          <NumberRow id="thresholds.hot_cpu" min={1} max={100} unit="%" integer={false} />
          <NumberRow id="thresholds.hot_mem" min={1} max={100} unit="%" integer={false} />
          <NumberRow id="thresholds.hot_temp" min={20} max={120} unit="°C" integer={false} />
        </SettingsCard>
      </Section>
    </>
  )
}

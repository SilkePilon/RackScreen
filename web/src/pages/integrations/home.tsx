import * as React from "react"
import { HouseIcon, PlusIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { PageBody, PageIntro } from "@/components/page"
import { RestoreDefaults } from "@/components/restore-defaults"
import {
  NumberRow,
  RowSkeleton,
  SecretRow,
  Section,
  SettingsCard,
  SwitchRow,
  TextRow,
  useEnabled,
} from "@/components/settings"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft } from "@/lib/draft"
import type { Config, HaEntityCfg } from "@/lib/types"
import { BambuSection } from "@/pages/integrations/bambu"
import { ScreenSelect } from "@/pages/integrations/gif-search"

const NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/
const ENTITY = /^[a-z0-9_]+\.[a-z0-9_]+$/

/** `sensor.living_room_temperature` → `living-room-temperature`. */
function suggestName(entity: string) {
  const object = entity.split(".")[1] ?? ""
  return object.replace(/_+/g, "-").replace(/^-|-$/g, "").slice(0, 32)
}

function addEntity(c: Config, e: HaEntityCfg, screen: number | null): Config {
  return {
    ...c,
    homeassistant: { ...c.homeassistant, entities: [...(c.homeassistant.entities ?? []), e] },
    screens:
      screen === null
        ? c.screens
        : c.screens.map((s, i) => (i === screen ? { ...s, roles: [...s.roles, `ha:${e.name}`] } : s)),
  }
}

export default function Home() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/home" />
      <PageIntro title="Home & printer">
        A Bambu Lab printer feeds the <code className="font-mono text-xs">printer</code> role. Each Home
        Assistant entity you add here is a role of its own,{" "}
        <code className="font-mono text-xs">ha:&lt;name&gt;</code>.
      </PageIntro>
      {ready ? (
        <Body />
      ) : (
        <>
          <RowSkeleton count={4} />
          <RowSkeleton count={4} />
        </>
      )}
    </PageBody>
  )
}

function Body() {
  const ha = useEnabled("homeassistant.enabled")
  const d = useDraft()
  const url = ((d.get("homeassistant.url") as string) ?? "").trim()
  return (
    <>
      <BambuSection />
      <Section
        title="Home Assistant"
        description="The Pi talks to Home Assistant directly, so give the address it can reach."
      >
        <SettingsCard>
          <SwitchRow id="homeassistant.enabled" />
          <TextRow
            id="homeassistant.url"
            mono
            muted={!ha}
            width="sm:w-72"
            placeholder="http://homeassistant.local:8123"
            validate={() =>
              url && !/^https?:\/\//.test(url)
                ? "Start with http:// or https://."
                : ha && !url
                  ? "Needed while Home Assistant is on."
                  : null
            }
          />
          <SecretRow id="homeassistant.token" muted={!ha} />
          <NumberRow id="homeassistant.poll_secs" min={5} unit="s" muted={!ha} />
        </SettingsCard>
      </Section>
      <Section
        title="Add an entity"
        description="Any entity: a sensor shows its number and a sparkline, a light or door its state."
      >
        <SettingsCard>
          <AddForm />
        </SettingsCard>
      </Section>
      <Entities />
    </>
  )
}

function num(v: string): number | null {
  const n = Number(v.trim().replace(",", "."))
  return v.trim() && Number.isFinite(n) ? n : null
}

function AddForm() {
  const d = useDraft()
  const list = d.draft?.homeassistant.entities ?? []
  const [entity, setEntity] = React.useState("")
  const [name, setName] = React.useState("")
  const [label, setLabel] = React.useState("")
  const [attribute, setAttribute] = React.useState("")
  const [unit, setUnit] = React.useState("")
  const [min, setMin] = React.useState("")
  const [max, setMax] = React.useState("")
  const [screen, setScreen] = React.useState<number | null>(null)
  const [touched, setTouched] = React.useState(false)
  const id = entity.trim()
  const n = (name.trim() || suggestName(id)).toLowerCase()
  const entityErr = !id
    ? "Give an entity id."
    : !ENTITY.test(id)
      ? "Like sensor.living_room_temperature."
      : null
  const nameErr = !n
    ? "Give it a name."
    : !NAME.test(n)
      ? "Lowercase letters, digits and - only."
      : list.some((e) => e.name === n)
        ? "Another entity has this name."
        : null
  const lo = num(min)
  const hi = num(max)
  const rangeErr =
    (min.trim() && lo === null) || (max.trim() && hi === null)
      ? "Numbers only."
      : (lo === null) !== (hi === null)
        ? "Give both, or neither."
        : lo !== null && hi !== null && hi <= lo
          ? "Max must be above min."
          : null

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setTouched(true)
    if (entityErr || nameErr || rangeErr) return
    const cfg: HaEntityCfg = { name: n, entity: id }
    if (label.trim()) cfg.label = label.trim()
    if (attribute.trim()) cfg.attribute = attribute.trim()
    if (unit.trim()) cfg.unit = unit.trim()
    if (lo !== null && hi !== null) {
      cfg.min = lo
      cfg.max = hi
    }
    d.update((c) => addEntity(c, cfg, screen))
    toast.success(`Added ha:${n}`, {
      description: screen === null ? "Save to keep it." : `On screen ${screen + 1} once you save.`,
    })
    for (const set of [setEntity, setName, setLabel, setAttribute, setUnit, setMin, setMax]) set("")
    setScreen(null)
    setTouched(false)
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-5 p-5">
      <FieldGroup>
        <div className="grid gap-5 sm:grid-cols-2">
          <Field data-invalid={(touched && !!entityErr) || undefined}>
            <FieldLabel htmlFor="ha-entity">Entity id</FieldLabel>
            <Input
              id="ha-entity"
              value={entity}
              onChange={(e) => setEntity(e.target.value.trim().toLowerCase())}
              placeholder="sensor.living_room_temperature"
              className="font-mono"
              aria-invalid={(touched && !!entityErr) || undefined}
              spellCheck={false}
              autoComplete="off"
            />
            {touched && entityErr ? (
              <FieldError>{entityErr}</FieldError>
            ) : (
              <FieldDescription>Settings → Entities in Home Assistant.</FieldDescription>
            )}
          </Field>
          <Field data-invalid={(touched && !!nameErr) || undefined}>
            <FieldLabel htmlFor="ha-name">Name</FieldLabel>
            <Input
              id="ha-name"
              value={name}
              onChange={(e) => setName(e.target.value.toLowerCase().replace(/\s+/g, "-"))}
              placeholder={suggestName(id) || "living-room"}
              className="font-mono"
              aria-invalid={(touched && !!nameErr) || undefined}
              spellCheck={false}
              autoComplete="off"
            />
            {touched && nameErr ? (
              <FieldError>{nameErr}</FieldError>
            ) : (
              <FieldDescription>
                The role is <span className="font-mono">ha:{n || "name"}</span>.
              </FieldDescription>
            )}
          </Field>
          <Field>
            <FieldLabel htmlFor="ha-label">Label</FieldLabel>
            <Input
              id="ha-label"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="Friendly name"
            />
            <FieldDescription>The pill under the value; empty uses the entity's own name.</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor="ha-screen">Screen</FieldLabel>
            <ScreenSelect id="ha-screen" value={screen} onChange={setScreen} />
          </Field>
          <Field>
            <FieldLabel htmlFor="ha-attribute">Attribute</FieldLabel>
            <Input
              id="ha-attribute"
              value={attribute}
              onChange={(e) => setAttribute(e.target.value)}
              placeholder="current_temperature"
              className="font-mono"
              spellCheck={false}
              autoComplete="off"
            />
            <FieldDescription>Optional: show an attribute instead of the state.</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor="ha-unit">Unit</FieldLabel>
            <Input id="ha-unit" value={unit} onChange={(e) => setUnit(e.target.value)} placeholder="°C" />
            <FieldDescription>Optional: replaces the entity's unit.</FieldDescription>
          </Field>
          <Field data-invalid={(touched && !!rangeErr) || undefined} className="sm:col-span-2">
            <FieldLabel htmlFor="ha-min">Ring range</FieldLabel>
            <div className="flex items-center gap-2">
              <Input
                id="ha-min"
                value={min}
                onChange={(e) => setMin(e.target.value)}
                placeholder="min"
                inputMode="decimal"
                className="w-28"
              />
              <span className="text-muted-foreground">to</span>
              <Input
                value={max}
                onChange={(e) => setMax(e.target.value)}
                placeholder="max"
                inputMode="decimal"
                className="w-28"
                aria-label="max"
              />
            </div>
            {touched && rangeErr ? (
              <FieldError>{rangeErr}</FieldError>
            ) : (
              <FieldDescription>
                Optional: how full the ring is for a number. Percent and temperatures pick one themselves.
              </FieldDescription>
            )}
          </Field>
        </div>
      </FieldGroup>
      <div className="flex justify-end">
        <Button type="submit">
          <PlusIcon data-icon="inline-start" />
          Add entity
        </Button>
      </div>
    </form>
  )
}

function Entities() {
  const d = useDraft()
  const list = d.draft?.homeassistant.entities ?? []
  if (list.length === 0)
    return (
      <Empty className="border bg-card">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HouseIcon />
          </EmptyMedia>
          <EmptyTitle>No entities yet</EmptyTitle>
          <EmptyDescription>Add one above, then pick it as a role on the Screens page.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  return (
    <Section
      title="Your entities"
      description={`${list.length} ${list.length === 1 ? "entity" : "entities"}`}
    >
      <SettingsCard>
        {list.map((e, i) => (
          <EntityRow key={`${e.name}-${i}`} e={e} index={i} />
        ))}
      </SettingsCard>
    </Section>
  )
}

function EntityRow({ e, index }: { e: HaEntityCfg; index: number }) {
  const d = useDraft()
  const role = `ha:${e.name}`
  const used = (d.draft?.screens ?? []).flatMap((s, i) => (s.roles.includes(role) ? [i] : []))
  const remove = () =>
    d.update((c) => ({
      ...c,
      homeassistant: {
        ...c.homeassistant,
        entities: (c.homeassistant.entities ?? []).filter((_, i) => i !== index),
      },
      screens: c.screens.map((s) => ({ ...s, roles: s.roles.filter((r) => r !== role) })),
    }))
  const extra = [
    e.attribute && `attribute ${e.attribute}`,
    e.unit && `unit ${e.unit}`,
    e.min != null && e.max != null && `${e.min} to ${e.max}`,
  ].filter(Boolean)
  return (
    <div className="flex items-center gap-4 px-5 py-3.5" data-setting-id={`ha-${e.name}`}>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-mono text-sm font-medium">{e.name}</span>
          {e.label && <span className="text-sm text-muted-foreground">{e.label}</span>}
          {used.length > 0 ? (
            <Badge variant="secondary" className="tabular font-normal">
              Screen {used.map((i) => i + 1).join(", ")}
            </Badge>
          ) : (
            <Badge variant="outline" className="font-normal text-muted-foreground">
              Not on a screen
            </Badge>
          )}
        </div>
        <span className="truncate font-mono text-[13px] text-muted-foreground">
          {e.entity}
          {extra.length > 0 && ` · ${extra.join(" · ")}`}
        </span>
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon-sm" onClick={remove} aria-label={`Remove ${e.name}`}>
            <Trash2Icon />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Remove{used.length > 0 ? " from the list and its screens" : ""}</TooltipContent>
      </Tooltip>
    </div>
  )
}

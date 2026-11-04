import * as React from "react"
import { EyeIcon, EyeOffIcon, PlusIcon, Undo2Icon, XIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group"
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
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft, useField } from "@/lib/draft"
import { SETTINGS } from "@/lib/settings"
import type { SecretPath } from "@/lib/types"
import { cn } from "@/lib/utils"

// ---------------------------------------------------------------------------
// Layout: a section heading over a bordered card of hairline-separated rows.

export function Section({
  title,
  description,
  action,
  children,
  id,
}: {
  title: string
  description?: React.ReactNode
  action?: React.ReactNode
  children: React.ReactNode
  id?: string
}) {
  return (
    <section className="flex scroll-mt-6 flex-col gap-3" id={id} aria-label={title}>
      <div className="flex items-end justify-between gap-4 px-1">
        <div className="flex flex-col gap-1">
          <h2 className="text-[15px] font-semibold tracking-tight text-foreground">{title}</h2>
          {description && <p className="text-[13px] text-muted-foreground">{description}</p>}
        </div>
        {action && <div className="flex shrink-0 items-center gap-2">{action}</div>}
      </div>
      {children}
    </section>
  )
}

export function SettingsCard({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex flex-col divide-y divide-hairline overflow-hidden rounded-xl border bg-card",
        className,
      )}
      {...props}
    />
  )
}

export function ResetButton({ onClick, label = "Reset to saved" }: { onClick: () => void; label?: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          aria-label={label}
          className="inline-flex size-5 items-center justify-center rounded-md text-muted-foreground transition-colors animate-in fade-in-0 zoom-in-75 hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        >
          <Undo2Icon className="size-3.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

/** One setting: title and help on the left, control on the right. */
export function Row({
  id,
  title,
  description,
  dirty,
  onReset,
  children,
  muted,
  stack,
  badge,
  className,
  htmlFor,
}: {
  id?: string
  title?: React.ReactNode
  description?: React.ReactNode
  dirty?: boolean
  onReset?: () => void
  children?: React.ReactNode
  muted?: boolean
  /** Put the control under the text (wide controls). */
  stack?: boolean
  badge?: React.ReactNode
  className?: string
  htmlFor?: string
}) {
  const meta = id ? SETTINGS[id] : undefined
  const t = title ?? meta?.title
  const d = description ?? meta?.description
  return (
    <div
      data-setting-id={id}
      className={cn(
        "flex scroll-mt-24 gap-x-6 gap-y-3 px-5 py-4 transition-[opacity,background-color]",
        stack ? "flex-col" : "flex-col sm:flex-row sm:items-center sm:justify-between",
        muted && "opacity-55 focus-within:opacity-100 hover:opacity-100",
        className,
      )}
    >
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex min-h-5 items-center gap-1.5">
          {htmlFor ? (
            <label htmlFor={htmlFor} className="text-sm font-medium text-foreground">
              {t}
            </label>
          ) : (
            <span className="text-sm font-medium text-foreground">{t}</span>
          )}
          {badge}
          {dirty && onReset && <ResetButton onClick={onReset} />}
        </div>
        {d && <div className="max-w-xl text-[13px] leading-relaxed text-pretty text-muted-foreground">{d}</div>}
      </div>
      {children !== undefined && (
        <div className={cn("flex shrink-0 items-center gap-2", !stack && "sm:justify-end")}>{children}</div>
      )}
    </div>
  )
}

export function RowSkeleton({ count = 3 }: { count?: number }) {
  return (
    <SettingsCard>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="flex items-center justify-between gap-6 px-5 py-4">
          <div className="flex flex-col gap-2">
            <Skeleton className="h-3.5 w-32" />
            <Skeleton className="h-3 w-64 max-w-[50vw]" />
          </div>
          <Skeleton className="h-6 w-28 rounded-md" />
        </div>
      ))}
    </SettingsCard>
  )
}

const inputId = (id: string) => `f-${id.replaceAll(".", "-")}`

// ---------------------------------------------------------------------------
// Draft-bound rows.

export function SwitchRow({ id, muted, description }: { id: string; muted?: boolean; description?: React.ReactNode }) {
  const f = useField<boolean>(id)
  return (
    <Row id={id} dirty={f.dirty} onReset={f.reset} muted={muted} description={description} htmlFor={inputId(id)}>
      <Switch id={inputId(id)} checked={!!f.value} onCheckedChange={f.set} />
    </Row>
  )
}

export function TextRow({
  id,
  placeholder,
  mono,
  muted,
  width = "sm:w-64",
  validate,
  type = "text",
}: {
  id: string
  placeholder?: string
  mono?: boolean
  muted?: boolean
  width?: string
  validate?: (v: string) => string | null
  type?: string
}) {
  const f = useField<string>(id)
  const err = validate ? validate(f.value ?? "") : null
  return (
    <Row
      id={id}
      dirty={f.dirty}
      onReset={f.reset}
      muted={muted}
      htmlFor={inputId(id)}
      description={err ? <span className="text-destructive">{err}</span> : undefined}
    >
      <Input
        id={inputId(id)}
        type={type}
        value={f.value ?? ""}
        placeholder={placeholder}
        onChange={(e) => f.set(e.target.value)}
        aria-invalid={!!err || undefined}
        spellCheck={false}
        autoComplete="off"
        className={cn("w-full", width, mono && "font-mono text-[13px]")}
      />
    </Row>
  )
}

/** A number input that keeps a local text while typing and commits valid numbers. */
export function NumberInput({
  value,
  onChange,
  min,
  max,
  step = 1,
  unit,
  nullable,
  id,
  className,
  integer = true,
  placeholder,
}: {
  value: number | null
  onChange: (v: number | null) => void
  min?: number
  max?: number
  step?: number
  unit?: string
  nullable?: boolean
  id?: string
  className?: string
  integer?: boolean
  placeholder?: string
}) {
  const d = useDraft()
  const [text, setText] = React.useState(value === null || value === undefined ? "" : String(value))
  const [prev, setPrev] = React.useState(value)
  const [prevEpoch, setPrevEpoch] = React.useState(d.epoch)
  if (prev !== value || prevEpoch !== d.epoch) {
    const replaced = prevEpoch !== d.epoch
    setPrev(value)
    setPrevEpoch(d.epoch)
    if (replaced || Number(text) !== value || (value === null && text !== "")) {
      setText(value === null || value === undefined ? "" : String(value))
    }
  }
  const n = Number(text)
  const invalid =
    text.trim() === ""
      ? !nullable
      : !Number.isFinite(n) ||
        (integer && !Number.isInteger(n)) ||
        (min !== undefined && n < min) ||
        (max !== undefined && n > max)
  // Report invalid text to the draft so Save waits until it is fixed.
  const key = React.useId()
  const { setInvalid } = d
  React.useEffect(() => {
    setInvalid(key, invalid)
    return () => setInvalid(key, false)
  }, [key, invalid, setInvalid])
  const hint =
    min !== undefined && max !== undefined
      ? `${min} to ${max}`
      : min !== undefined
        ? `at least ${min}`
        : max !== undefined
          ? `at most ${max}`
          : undefined
  const input = (
    <InputGroupInput
      id={id}
      inputMode={integer ? "numeric" : "decimal"}
      value={text}
      placeholder={placeholder}
      aria-invalid={invalid || undefined}
      onChange={(e) => {
        const t = e.target.value
        setText(t)
        if (t.trim() === "") {
          if (nullable) onChange(null)
          return
        }
        const v = Number(t)
        if (!Number.isFinite(v)) return
        if (integer && !Number.isInteger(v)) return
        if ((min !== undefined && v < min) || (max !== undefined && v > max)) return
        onChange(v)
      }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return
        e.preventDefault()
        const base = Number.isFinite(n) && text !== "" ? n : (min ?? 0)
        let v = base + (e.key === "ArrowUp" ? step : -step) * (e.shiftKey ? 10 : 1)
        if (min !== undefined) v = Math.max(min, v)
        if (max !== undefined) v = Math.min(max, v)
        v = Math.round(v * 1000) / 1000
        setText(String(v))
        onChange(v)
      }}
      className="tabular text-right"
    />
  )
  return (
    <Tooltip open={invalid && hint ? undefined : false}>
      <TooltipTrigger asChild>
        <InputGroup className={cn("w-32", className)}>
          {input}
          {unit && (
            <InputGroupAddon align="inline-end">
              <InputGroupText className="text-xs">{unit}</InputGroupText>
            </InputGroupAddon>
          )}
        </InputGroup>
      </TooltipTrigger>
      {hint && <TooltipContent>{hint}</TooltipContent>}
    </Tooltip>
  )
}

export function NumberRow({
  id,
  min,
  max,
  step,
  unit,
  muted,
  nullable,
  integer = true,
  placeholder,
}: {
  id: string
  min?: number
  max?: number
  step?: number
  unit?: string
  muted?: boolean
  nullable?: boolean
  integer?: boolean
  placeholder?: string
}) {
  const f = useField<number | null>(id)
  return (
    <Row id={id} dirty={f.dirty} onReset={f.reset} muted={muted} htmlFor={inputId(id)}>
      <NumberInput
        id={inputId(id)}
        value={f.value}
        onChange={f.set}
        min={min}
        max={max}
        step={step}
        unit={unit}
        nullable={nullable}
        integer={integer}
        placeholder={placeholder}
      />
    </Row>
  )
}

export function SelectRow({
  id,
  options,
  muted,
  width = "w-44",
}: {
  id: string
  options: { value: string; label: string }[]
  muted?: boolean
  width?: string
}) {
  const f = useField<string | number>(id)
  const isNum = typeof f.value === "number"
  return (
    <Row id={id} dirty={f.dirty} onReset={f.reset} muted={muted}>
      <Select value={String(f.value)} onValueChange={(v) => f.set(isNum ? Number(v) : v)}>
        <SelectTrigger className={width} aria-label={SETTINGS[id]?.title}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectGroup>
            {options.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </Row>
  )
}

/**
 * A secret: the browser never sees the stored value. null keeps it, "" clears
 * it, any other string replaces it.
 */
export function SecretRow({ id, muted }: { id: SecretPath; muted?: boolean }) {
  const d = useDraft()
  const f = useField<string | null>(id)
  const isSet = !!d.secrets[id]
  const [editing, setEditing] = React.useState(false)
  const [show, setShow] = React.useState(false)
  const ref = React.useRef<HTMLInputElement>(null)
  const value = f.value
  const editingNow = editing || (typeof value === "string" && value !== "")
  const cleared = value === ""

  const status = cleared ? (
    <Badge variant="destructive" className="font-normal">
      Will be cleared
    </Badge>
  ) : typeof value === "string" ? (
    <Badge variant="secondary" className="font-normal">
      New value
    </Badge>
  ) : isSet ? (
    <Badge variant="secondary" className="font-normal text-muted-foreground">
      Set
    </Badge>
  ) : (
    <Badge variant="outline" className="font-normal text-muted-foreground">
      Not set
    </Badge>
  )

  return (
    <Row
      id={id}
      dirty={f.dirty}
      onReset={() => {
        f.reset()
        setEditing(false)
      }}
      muted={muted}
      badge={status}
      htmlFor={inputId(id)}
    >
      {editingNow ? (
        <InputGroup className="w-full sm:w-72">
          <InputGroupInput
            ref={ref}
            id={inputId(id)}
            type={show ? "text" : "password"}
            autoComplete="new-password"
            spellCheck={false}
            placeholder={isSet ? "New value" : "Paste value"}
            value={value ?? ""}
            onChange={(e) => f.set(e.target.value === "" ? null : e.target.value)}
            className="font-mono text-[13px]"
          />
          <InputGroupAddon align="inline-end">
            <InputGroupButton
              size="icon-xs"
              aria-label={show ? "Hide" : "Show"}
              onClick={() => setShow((s) => !s)}
            >
              {show ? <EyeOffIcon /> : <EyeIcon />}
            </InputGroupButton>
            <InputGroupButton
              size="icon-xs"
              aria-label="Cancel"
              onClick={() => {
                f.set(null)
                setEditing(false)
              }}
            >
              <XIcon />
            </InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      ) : (
        <>
          {isSet && !cleared && (
            <Button variant="ghost" size="sm" onClick={() => f.set("")}>
              Clear
            </Button>
          )}
          {cleared && (
            <Button variant="ghost" size="sm" onClick={() => f.set(null)}>
              Keep
            </Button>
          )}
          <InputGroup
            className="w-full cursor-text sm:w-56"
            onClick={() => {
              setEditing(true)
              requestAnimationFrame(() => ref.current?.focus())
            }}
          >
            <InputGroupInput
              id={inputId(id)}
              readOnly
              value=""
              placeholder={isSet && !cleared ? "••••••••  set" : "Not set"}
              onFocus={() => {
                setEditing(true)
                requestAnimationFrame(() => ref.current?.focus())
              }}
              className="cursor-text"
            />
            <InputGroupAddon align="inline-end">
              <InputGroupText className="text-xs">{isSet ? "Replace" : "Add"}</InputGroupText>
            </InputGroupAddon>
          </InputGroup>
        </>
      )}
    </Row>
  )
}

/** A list of short strings as removable chips plus an add field. */
export function TagsRow({
  id,
  placeholder,
  validate,
  muted,
}: {
  id: string
  placeholder: string
  validate?: (v: string) => string | null
  muted?: boolean
}) {
  const f = useField<string[]>(id)
  const list = f.value ?? []
  const [text, setText] = React.useState("")
  const err = text.trim() && validate ? validate(text.trim()) : null
  const add = () => {
    const parts = text
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
    if (!parts.length || (validate && parts.some((p) => validate(p)))) return
    f.set([...list, ...parts.filter((p) => !list.includes(p))])
    setText("")
  }
  return (
    <Row id={id} dirty={f.dirty} onReset={f.reset} muted={muted} stack>
      <div className="flex w-full flex-col gap-2.5">
        {list.length > 0 && (
          <ul className="flex flex-wrap gap-1.5" aria-label={SETTINGS[id]?.title}>
            {list.map((t) => (
              <li key={t}>
                <Badge variant="secondary" className="h-7 gap-1 rounded-md pr-1 pl-2.5 font-mono text-xs font-normal">
                  {t}
                  <button
                    type="button"
                    className="inline-flex size-5 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                    aria-label={`Remove ${t}`}
                    onClick={() => f.set(list.filter((x) => x !== t))}
                  >
                    <XIcon className="size-3" />
                  </button>
                </Badge>
              </li>
            ))}
          </ul>
        )}
        <form
          className="flex max-w-md items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            add()
          }}
        >
          <Input
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={placeholder}
            aria-invalid={!!err || undefined}
            aria-label={`Add to ${SETTINGS[id]?.title ?? id}`}
            spellCheck={false}
            className="font-mono text-[13px]"
          />
          <Button type="submit" variant="outline" disabled={!text.trim() || !!err}>
            <PlusIcon data-icon="inline-start" />
            Add
          </Button>
        </form>
        {err && <p className="text-xs text-destructive">{err}</p>}
      </div>
    </Row>
  )
}

/** "Enabled" master switch that leads a card; the other rows dim while it's off. */
export function useEnabled(id: string) {
  const f = useField<boolean>(id)
  return !!f.value
}

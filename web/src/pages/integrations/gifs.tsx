import * as React from "react"
import {
  ExternalLinkIcon,
  ImageIcon,
  ImagePlayIcon,
  LinkIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  Trash2Icon,
} from "lucide-react"
import { toast } from "sonner"

import { PageBody, PageIntro } from "@/components/page"
import { Segmented } from "@/components/segmented"
import { RowSkeleton, SecretRow, Section, SelectRow, SettingsCard } from "@/components/settings"
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
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft } from "@/lib/draft"
import { classifyLink, gifNameError } from "@/lib/gif"
import { gifMb } from "@/lib/gif-memory"
import type { Config, GifCfg } from "@/lib/types"
import { BudgetWarning, GifMemoryMeta, GifMemorySection } from "@/pages/integrations/gif-memory"
import { addGif, GifSearch, ScreenSelect } from "@/pages/integrations/gif-search"

const RATINGS = [
  { value: "g", label: "G · everyone" },
  { value: "pg", label: "PG" },
  { value: "pg-13", label: "PG-13" },
  { value: "r", label: "R" },
]

export default function Gifs() {
  const d = useDraft()
  const [editing, setEditing] = React.useState<number | null>(null)
  const [mode, setMode] = React.useState<"search" | "paste">("search")
  const gifs = d.draft?.gifs ?? []
  return (
    <PageBody>
      <PageIntro title="GIFs">
        Search GIPHY and Tenor, or paste any https link ending in .gif. A screen shows a GIF as the role{" "}
        <code className="font-mono text-xs">gif:&lt;name&gt;</code>. Only GIFs a screen shows are downloaded and kept
        in memory.
      </PageIntro>
      <GifMemorySection />
      <Section
        title="Add a GIF"
        description={
          mode === "search"
            ? "Pick one to name it and put it on a screen."
            : "GIPHY, Tenor, or any https link ending in .gif."
        }
        action={
          <Segmented
            label="How to add"
            value={mode}
            onChange={setMode}
            options={[
              {
                value: "search",
                label: (
                  <span className="flex items-center gap-1.5">
                    <SearchIcon className="size-3.5" />
                    Search
                  </span>
                ),
              },
              {
                value: "paste",
                label: (
                  <span className="flex items-center gap-1.5">
                    <LinkIcon className="size-3.5" />
                    Paste link
                  </span>
                ),
              },
            ]}
          />
        }
      >
        {!d.ready ? (
          <RowSkeleton count={1} />
        ) : (
          <SettingsCard>{mode === "search" ? <GifSearch /> : <PasteForm />}</SettingsCard>
        )}
      </Section>
      {!d.ready ? (
        <RowSkeleton count={2} />
      ) : gifs.length === 0 ? (
        <Empty className="border bg-card">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ImagePlayIcon />
            </EmptyMedia>
            <EmptyTitle>No GIFs yet</EmptyTitle>
            <EmptyDescription>Search or paste one above, then pick it as a role on the Screens page.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Section title="Your GIFs" description={`${gifs.length} ${gifs.length === 1 ? "link" : "links"}`}>
          <SettingsCard>
            {gifs.map((g, i) => (
              <GifRow key={`${g.name}-${i}`} gif={g} index={i} onEdit={() => setEditing(i)} />
            ))}
          </SettingsCard>
        </Section>
      )}
      <Section
        title="Search keys"
        id="search-keys"
        description="Free keys for the search above. The Pi makes the calls; the keys never reach this browser."
      >
        {!d.ready ? (
          <RowSkeleton count={3} />
        ) : (
          <SettingsCard>
            <SecretRow id="gif_search.giphy_key" />
            <SecretRow id="gif_search.tenor_key" />
            <SelectRow id="gif_search.rating" options={RATINGS} />
          </SettingsCard>
        )}
      </Section>
      <GifDialog open={editing !== null} index={editing ?? 0} onOpenChange={(o) => !o && setEditing(null)} />
    </PageBody>
  )
}

function PasteForm() {
  const d = useDraft()
  const gifs = d.draft?.gifs ?? []
  const [name, setName] = React.useState("")
  const [url, setUrl] = React.useState("")
  const [screen, setScreen] = React.useState<number | null>(null)
  const [touched, setTouched] = React.useState(false)
  const n = name.trim()
  const nameErr = gifNameError(n, gifs, null)
  const link = classifyLink(url)
  const dupe = gifs.find((g) => g.url.trim() === url.trim())
  const urlErr = !url.trim()
    ? "Paste a link."
    : !link.ok
      ? link.error
      : dupe
        ? `${dupe.name} already has this link.`
        : null
  const preview = link.ok ? link.preview : null

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setTouched(true)
    if (nameErr || urlErr) return
    d.update((c) => addGif(c, { name: n, url: url.trim() }, screen))
    toast.success(`Added ${n}`, {
      description: screen === null ? "Save to keep it." : `On screen ${screen + 1} once you save.`,
    })
    setName("")
    setUrl("")
    setScreen(null)
    setTouched(false)
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-5 p-5">
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start">
        <div className="flex size-20 shrink-0 items-center justify-center self-center overflow-hidden rounded-full bg-black ring-1 ring-border sm:self-start">
          {preview ? (
            <img src={preview} alt="Preview" referrerPolicy="no-referrer" className="size-full object-cover" />
          ) : (
            <ImageIcon className="size-5 text-muted-foreground" />
          )}
        </div>
        <FieldGroup className="flex-1">
          <Field data-invalid={(touched && !!urlErr) || (!!url && !link.ok) || undefined}>
            <FieldLabel htmlFor="paste-url">Link</FieldLabel>
            <Input
              id="paste-url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://giphy.com/gifs/…"
              aria-invalid={(touched && !!urlErr) || (!!url && !link.ok) || undefined}
              spellCheck={false}
              autoComplete="off"
            />
            {(touched || (url && !link.ok)) && urlErr ? (
              <FieldError>{urlErr}</FieldError>
            ) : (
              <FieldDescription>
                {link.ok && !preview
                  ? "Tenor pages are resolved on the Pi; no preview here."
                  : "GIPHY, Tenor, or any https link ending in .gif."}
              </FieldDescription>
            )}
          </Field>
          <div className="grid gap-5 sm:grid-cols-2">
            <Field data-invalid={(touched && !!nameErr) || undefined}>
              <FieldLabel htmlFor="paste-name">Name</FieldLabel>
              <Input
                id="paste-name"
                value={name}
                onChange={(e) => setName(e.target.value.toLowerCase().replace(/\s+/g, "-"))}
                placeholder="party"
                className="font-mono"
                aria-invalid={(touched && !!nameErr) || undefined}
                spellCheck={false}
                autoComplete="off"
              />
              {touched && nameErr ? (
                <FieldError>{nameErr}</FieldError>
              ) : (
                <FieldDescription>
                  The role is <span className="font-mono">gif:{n || "name"}</span>.
                </FieldDescription>
              )}
            </Field>
            <Field>
              <FieldLabel htmlFor="paste-screen">Screen</FieldLabel>
              <ScreenSelect id="paste-screen" value={screen} onChange={setScreen} />
            </Field>
          </div>
        </FieldGroup>
      </div>
      {screen !== null && link.ok && <BudgetWarning mb={gifMb(null)} known={false} />}
      <div className="flex justify-end">
        <Button type="submit">
          <PlusIcon data-icon="inline-start" />
          Add GIF
        </Button>
      </div>
    </form>
  )
}

function usedBy(cfg: Config | null, name: string) {
  return (cfg?.screens ?? []).flatMap((s, i) => (s.roles.includes(`gif:${name}`) ? [i] : []))
}

function GifRow({ gif, index, onEdit }: { gif: GifCfg; index: number; onEdit: () => void }) {
  const d = useDraft()
  const link = classifyLink(gif.url)
  const used = usedBy(d.draft, gif.name)
  const strands = used.filter((i) => d.draft!.screens[i].roles.length === 1)
  const [broken, setBroken] = React.useState(false)
  const preview = link.ok ? link.preview : null
  const remove = () =>
    d.update((c) => ({
      ...c,
      gifs: (c.gifs ?? []).filter((_, i) => i !== index),
      screens: c.screens.map((s) => ({ ...s, roles: s.roles.filter((r) => r !== `gif:${gif.name}`) })),
    }))
  return (
    <div className="flex items-center gap-4 px-5 py-3.5" data-setting-id={`gif-${gif.name}`}>
      <div className="flex size-14 shrink-0 items-center justify-center overflow-hidden rounded-full bg-black ring-1 ring-border">
        {preview && !broken ? (
          <img
            src={preview}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => setBroken(true)}
            className="size-full object-cover"
          />
        ) : (
          <ImageIcon className="size-5 text-muted-foreground" />
        )}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm font-medium">{gif.name}</span>
          {used.length > 0 ? (
            <Badge variant="secondary" className="font-normal tabular">
              Screen {used.map((i) => i + 1).join(", ")}
            </Badge>
          ) : (
            <Badge variant="outline" className="font-normal text-muted-foreground">
              Not on a screen
            </Badge>
          )}
          {!link.ok && (
            <Badge variant="destructive" className="font-normal">
              Bad link
            </Badge>
          )}
        </div>
        <a
          href={gif.url}
          target="_blank"
          rel="noreferrer"
          className="inline-flex min-w-0 items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
        >
          <span className="truncate">{gif.url}</span>
          <ExternalLinkIcon className="size-3 shrink-0" />
        </a>
      </div>
      <div className="hidden shrink-0 sm:block">
        <GifMemoryMeta name={gif.name} onScreen={used.length > 0} />
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" onClick={onEdit} aria-label={`Edit ${gif.name}`}>
              <PencilIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Rename or change link</TooltipContent>
        </Tooltip>
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label={`Delete ${gif.name}`}>
              <Trash2Icon />
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Delete {gif.name}?</AlertDialogTitle>
              <AlertDialogDescription>
                {strands.length > 0
                  ? `Screen ${strands.map((i) => i + 1).join(", ")} shows only this GIF. Give it another role on the Screens page first.`
                  : used.length > 0
                    ? `It is also taken off screen ${used.map((i) => i + 1).join(", ")}. Nothing changes until you save.`
                    : "Nothing changes until you save."}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction variant="destructive" onClick={remove} disabled={strands.length > 0}>
                Delete
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </div>
  )
}

function GifDialog({
  open,
  index,
  onOpenChange,
}: {
  open: boolean
  index: number
  onOpenChange: (o: boolean) => void
}) {
  const d = useDraft()
  const gifs = d.draft?.gifs ?? []
  const current = open ? gifs[index] : undefined
  const [name, setName] = React.useState("")
  const [url, setUrl] = React.useState("")
  const [touched, setTouched] = React.useState(false)
  const [openedFor, setOpenedFor] = React.useState<string | null>(null)
  const key = open ? `${index}` : null
  if (key !== openedFor) {
    setOpenedFor(key)
    setName(current?.name ?? "")
    setUrl(current?.url ?? "")
    setTouched(false)
  }

  const n = name.trim()
  const nameErr = gifNameError(n, gifs, index)
  const link = classifyLink(url)
  const dupe = gifs.find((g, i) => i !== index && g.url.trim() === url.trim())
  const urlErr = !url.trim()
    ? "Paste a link."
    : !link.ok
      ? link.error
      : dupe
        ? `${dupe.name} already has this link.`
        : null
  const preview = link.ok ? link.preview : null

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setTouched(true)
    if (nameErr || urlErr || !current) return
    d.update((c) => {
      const list = [...(c.gifs ?? [])]
      list[index] = { name: n, url: url.trim() }
      const renamed = current && current.name !== n
      return {
        ...c,
        gifs: list,
        screens: renamed
          ? c.screens.map((s) => ({
              ...s,
              roles: s.roles.map((r) => (r === `gif:${current.name}` ? `gif:${n}` : r)),
            }))
          : c.screens,
      }
    })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={submit} className="flex flex-col gap-5">
          <DialogHeader>
            <DialogTitle>Edit {current?.name}</DialogTitle>
            <DialogDescription>Renaming updates every screen that shows it.</DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field data-invalid={(touched && !!nameErr) || undefined}>
              <FieldLabel htmlFor="gif-name">Name</FieldLabel>
              <Input
                id="gif-name"
                value={name}
                autoFocus
                onChange={(e) => setName(e.target.value.toLowerCase().replace(/\s+/g, "-"))}
                placeholder="party"
                className="font-mono"
                aria-invalid={(touched && !!nameErr) || undefined}
                spellCheck={false}
              />
              {touched && nameErr ? (
                <FieldError>{nameErr}</FieldError>
              ) : (
                <FieldDescription>
                  The role is <span className="font-mono">gif:{n || "name"}</span>.
                </FieldDescription>
              )}
            </Field>
            <Field data-invalid={(touched && !!urlErr) || (!!url && !link.ok) || undefined}>
              <FieldLabel htmlFor="gif-url">Link</FieldLabel>
              <Input
                id="gif-url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://giphy.com/gifs/…"
                aria-invalid={(touched && !!urlErr) || (!!url && !link.ok) || undefined}
                spellCheck={false}
              />
              {(touched || (url && !link.ok)) && urlErr ? (
                <FieldError>{urlErr}</FieldError>
              ) : (
                <FieldDescription>GIPHY, Tenor, or any https link ending in .gif.</FieldDescription>
              )}
            </Field>
          </FieldGroup>
          {link.ok && (
            <div className="flex items-center gap-4 rounded-lg border bg-muted/40 p-3 animate-in fade-in-0">
              <div className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-full bg-black">
                {preview ? (
                  <img src={preview} alt="Preview" referrerPolicy="no-referrer" className="size-full object-cover" />
                ) : (
                  <ImageIcon className="size-5 text-muted-foreground" />
                )}
              </div>
              <p className="text-[13px] text-muted-foreground">
                {preview
                  ? "This is roughly how it sits on a round panel."
                  : "Tenor pages are resolved on the Pi; no preview here."}
              </p>
            </div>
          )}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit">Done</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

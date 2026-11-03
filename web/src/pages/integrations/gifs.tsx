import * as React from "react"
import { ExternalLinkIcon, ImageIcon, ImagePlayIcon, PencilIcon, PlusIcon, Trash2Icon } from "lucide-react"

import { HeaderActions, PageBody, PageIntro } from "@/components/page"
import { RowSkeleton, Section, SettingsCard } from "@/components/settings"
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
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDraft } from "@/lib/draft"
import { classifyLink, GIF_NAME } from "@/lib/gif"
import type { Config, GifCfg } from "@/lib/types"

export default function Gifs() {
  const d = useDraft()
  const [editing, setEditing] = React.useState<{ index: number | null } | null>(null)
  const gifs = d.draft?.gifs ?? []
  return (
    <PageBody>
      <HeaderActions>
        <Button size="sm" variant="ghost" onClick={() => setEditing({ index: null })} disabled={!d.ready}>
          <PlusIcon data-icon="inline-start" />
          Add GIF
        </Button>
      </HeaderActions>
      <PageIntro title="GIFs">
        Paste a GIPHY or Tenor link, or any https link ending in .gif. A screen shows it as the role{" "}
        <code className="font-mono text-xs">gif:&lt;name&gt;</code>. Each GIF a screen uses takes up to about 35 MB
        of memory on the Pi.
      </PageIntro>
      {!d.ready ? (
        <RowSkeleton count={2} />
      ) : gifs.length === 0 ? (
        <Empty className="border bg-card">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ImagePlayIcon />
            </EmptyMedia>
            <EmptyTitle>No GIFs yet</EmptyTitle>
            <EmptyDescription>Add one, then pick it as a role on the Screens page.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button onClick={() => setEditing({ index: null })}>
              <PlusIcon data-icon="inline-start" />
              Add a GIF
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <Section title="Your GIFs" description={`${gifs.length} ${gifs.length === 1 ? "link" : "links"}`}>
          <SettingsCard>
            {gifs.map((g, i) => (
              <GifRow key={`${g.name}-${i}`} gif={g} index={i} onEdit={() => setEditing({ index: i })} />
            ))}
          </SettingsCard>
        </Section>
      )}
      <GifDialog
        open={!!editing}
        index={editing?.index ?? null}
        onOpenChange={(o) => !o && setEditing(null)}
      />
    </PageBody>
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
  index: number | null
  onOpenChange: (o: boolean) => void
}) {
  const d = useDraft()
  const gifs = d.draft?.gifs ?? []
  const current = index !== null ? gifs[index] : undefined
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
  const nameErr = !n
    ? "Give it a name."
    : !GIF_NAME.test(n)
      ? "Lowercase letters, digits and - only."
      : gifs.some((g, i) => g.name === n && i !== index)
        ? "Another GIF has this name."
        : null
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
    if (nameErr || urlErr) return
    d.update((c) => {
      const list = [...(c.gifs ?? [])]
      const next = { name: n, url: url.trim() }
      if (index === null) list.push(next)
      else list[index] = next
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
            <DialogTitle>{current ? `Edit ${current.name}` : "Add a GIF"}</DialogTitle>
            <DialogDescription>
              {current ? "Renaming updates every screen that shows it." : "Then add it to a screen as a role."}
            </DialogDescription>
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
            <Button type="submit">{current ? "Done" : "Add GIF"}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

import * as React from "react"
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands"
import { yaml as yamlLang } from "@codemirror/lang-yaml"
import { bracketMatching, HighlightStyle, indentOnInput, syntaxHighlighting } from "@codemirror/language"
import { EditorState } from "@codemirror/state"
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view"
import { tags as t } from "@lezer/highlight"
import { CircleCheckIcon, CircleXIcon, RotateCcwIcon } from "lucide-react"
import { useBlocker } from "react-router"
import { parse } from "yaml"
import { toast } from "sonner"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, errorMessage } from "@/lib/api"
import { useDraft } from "@/lib/draft"
import { announceSave } from "@/lib/restart"
import { cn } from "@/lib/utils"

const highlight = HighlightStyle.define([
  { tag: t.comment, color: "var(--muted-foreground)", fontStyle: "italic" },
  { tag: [t.propertyName, t.definition(t.propertyName)], color: "var(--chart-1)" },
  { tag: [t.string, t.special(t.string)], color: "var(--chart-2)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--chart-3)" },
  { tag: [t.punctuation, t.separator, t.bracket], color: "var(--muted-foreground)" },
  { tag: t.keyword, color: "var(--chart-5)" },
])

const theme = EditorView.theme({
  "&": { fontSize: "12.5px", backgroundColor: "transparent", color: "var(--foreground)", height: "100%" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.7" },
  ".cm-content": { caretColor: "var(--primary)", padding: "10px 0" },
  ".cm-gutters": { backgroundColor: "transparent", color: "var(--muted-foreground)", border: "none", opacity: ".6" },
  ".cm-activeLine": { backgroundColor: "color-mix(in oklab, var(--foreground) 4%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--foreground)" },
  "&.cm-focused": { outline: "none" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in oklab, var(--primary) 30%, transparent) !important",
  },
  ".cm-cursor": { borderLeftColor: "var(--primary)" },
})

type Check = { state: "idle" } | { state: "ok" } | { state: "error"; message: string }

export default function YamlEditor() {
  const d = useDraft()
  const host = React.useRef<HTMLDivElement>(null)
  const view = React.useRef<EditorView | null>(null)
  const [loaded, setLoaded] = React.useState<{ text: string; v: number } | null>(null)
  const [dirty, setDirty] = React.useState(false)
  const [check, setCheck] = React.useState<Check>({ state: "idle" })
  const [busy, setBusy] = React.useState<"validate" | "save" | null>(null)

  const load = React.useCallback(async () => {
    try {
      const r = await api.yaml()
      setLoaded((l) => ({ text: r.yaml, v: (l?.v ?? 0) + 1 }))
      setDirty(false)
      setCheck({ state: "idle" })
    } catch (e) {
      toast.error("Could not load the YAML", { description: errorMessage(e) })
    }
  }, [])

  React.useEffect(() => {
    let alive = true
    api
      .yaml()
      .then((r) => alive && setLoaded({ text: r.yaml, v: 0 }))
      .catch((e) => toast.error("Could not load the YAML", { description: errorMessage(e) }))
    return () => {
      alive = false
    }
  }, [])

  React.useEffect(() => {
    if (loaded === null || !host.current || view.current) return
    view.current = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: loaded.text,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          history(),
          drawSelection(),
          indentOnInput(),
          bracketMatching(),
          yamlLang(),
          syntaxHighlighting(highlight),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          theme,
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": "Config YAML" }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) {
              setDirty(true)
              setCheck({ state: "idle" })
            }
          }),
        ],
      }),
    })
    return () => {
      view.current?.destroy()
      view.current = null
    }
  }, [loaded])

  const text = () => view.current?.state.doc.toString() ?? ""

  // Unsaved YAML: ask before leaving the page or closing the tab.
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => dirty && currentLocation.pathname !== nextLocation.pathname,
  )
  React.useEffect(() => {
    if (!dirty) return
    const onUnload = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ""
    }
    window.addEventListener("beforeunload", onUnload)
    return () => window.removeEventListener("beforeunload", onUnload)
  }, [dirty])

  const validate = async (): Promise<boolean> => {
    setBusy("validate")
    try {
      let parsed: unknown
      try {
        parsed = parse(text())
      } catch (e) {
        setCheck({ state: "error", message: errorMessage(e) })
        return false
      }
      const r = await api.validateConfig(parsed)
      setCheck(r.ok ? { state: "ok" } : { state: "error", message: r.error ?? "Invalid config" })
      return r.ok
    } catch (e) {
      setCheck({ state: "error", message: errorMessage(e) })
      return false
    } finally {
      setBusy(null)
    }
  }

  const save = async () => {
    if (!(await validate())) return
    setBusy("save")
    try {
      const r = await api.saveYaml(text())
      const before = d.saved
      d.adopt({ config: r.config, secrets: r.secrets })
      announceSave(r, before)
      setDirty(false)
      await load()
    } catch (e) {
      setCheck({ state: "error", message: errorMessage(e) })
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="flex flex-col">
      <div className="relative h-[460px] overflow-hidden border-b border-hairline">
        {loaded === null && (
          <div className="flex flex-col gap-2 p-4">
            {Array.from({ length: 12 }, (_, i) => (
              <Skeleton key={i} className="h-3.5" style={{ width: `${25 + ((i * 29) % 50)}%` }} />
            ))}
          </div>
        )}
        <div ref={host} className="scrollbar-thin h-full [&_.cm-editor]:h-full" />
      </div>
      <div className="flex flex-col gap-3 px-5 py-3 sm:flex-row sm:items-center">
        <div className="flex min-w-0 flex-1 items-start gap-2 text-[13px]">
          {check.state === "ok" && (
            <>
              <CircleCheckIcon className="mt-0.5 size-4 shrink-0 text-success" />
              <span className="text-muted-foreground">Valid. Saving restarts the panels.</span>
            </>
          )}
          {check.state === "error" && (
            <>
              <CircleXIcon className="mt-0.5 size-4 shrink-0 text-destructive" />
              <span className="font-mono text-xs leading-relaxed whitespace-pre-wrap text-destructive">
                {check.message}
              </span>
            </>
          )}
          {check.state === "idle" && (
            <span className={cn("text-muted-foreground", d.changes.length > 0 && "text-warning")}>
              {d.changes.length > 0
                ? "Saving here drops the unsaved changes made on the other pages."
                : "Secrets show in full here. Comments are not kept."}
            </span>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button variant="ghost" size="sm" onClick={() => void load()} disabled={!dirty || !!busy}>
            <RotateCcwIcon data-icon="inline-start" />
            Revert
          </Button>
          <Button variant="outline" size="sm" onClick={() => void validate()} disabled={!!busy || loaded === null}>
            {busy === "validate" && <Spinner data-icon="inline-start" />}
            Validate
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={!dirty || !!busy}>
            {busy === "save" && <Spinner data-icon="inline-start" />}
            Save &amp; apply
          </Button>
        </div>
      </div>
      <AlertDialog open={blocker.state === "blocked"} onOpenChange={(o) => !o && blocker.reset?.()}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Leave the YAML editor?</AlertDialogTitle>
            <AlertDialogDescription>The text you changed there is not saved and will be lost.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Stay</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => blocker.proceed?.()}>
              Leave
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

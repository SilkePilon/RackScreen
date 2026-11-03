import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { api, errorMessage } from "@/lib/api"
import { deepEqual, diffPaths, getAt, setAt } from "@/lib/path"
import { keys, useConfig } from "@/lib/queries"
import type { Config, ConfigResponse, SaveResponse, SecretsMap } from "@/lib/types"

// The draft config: every edit on every page lands here first. The floating
// save bar shows while it differs from the saved copy, and "Save & apply"
// PUTs the whole draft. Secret fields stay null (= keep) unless replaced.

interface DraftState {
  ready: boolean
  loadError: unknown
  saved: Config | null
  draft: Config | null
  secrets: SecretsMap
  changes: string[]
  saving: boolean
  error: string | null
  get: (path: string) => unknown
  set: (path: string, value: unknown) => void
  update: (fn: (c: Config) => Config) => void
  reset: (path: string) => void
  isDirty: (path: string) => boolean
  discard: () => void
  save: () => Promise<SaveResponse | undefined>
  /** Replace both copies after a save elsewhere (YAML editor, upload). */
  adopt: (r: ConfigResponse) => void
}

const DraftContext = React.createContext<DraftState | null>(null)

export function restartMessage(r: SaveResponse["restart"]): string {
  switch (r) {
    case "monitor":
      return "Saved. The panels restart in a moment."
    case "process":
      return "Saved. RackScreen restarts; this page reconnects by itself."
    default:
      return "Saved."
  }
}

export function DraftProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient()
  const query = useConfig()
  const [saved, setSaved] = React.useState<Config | null>(null)
  const [draft, setDraft] = React.useState<Config | null>(null)
  const [secrets, setSecrets] = React.useState<SecretsMap>({})
  const [error, setError] = React.useState<string | null>(null)
  const [seen, setSeen] = React.useState<ConfigResponse | undefined>(undefined)

  // Adopt fresh server data while nothing is being edited (render-time sync).
  if (query.data && query.data !== seen) {
    setSeen(query.data)
    if (!draft || !saved || deepEqual(draft, saved)) {
      setSaved(query.data.config)
      setDraft(query.data.config)
    } else {
      setSaved(query.data.config)
    }
    setSecrets(query.data.secrets)
  }

  const changes = React.useMemo(
    () => (saved && draft ? diffPaths(saved, draft) : []),
    [saved, draft],
  )

  const mutation = useMutation({
    mutationFn: (c: Config) => api.saveConfig(c),
    onSuccess: (r) => {
      setError(null)
      qc.setQueryData(keys.config, { config: r.config, secrets: r.secrets })
      setSaved(r.config)
      setDraft(r.config)
      setSecrets(r.secrets)
      toast.success(restartMessage(r.restart))
      void qc.invalidateQueries({ queryKey: keys.roles })
      void qc.invalidateQueries({ queryKey: keys.status })
      void qc.invalidateQueries({ queryKey: keys.spotify })
    },
    onError: (e) => {
      setError(errorMessage(e))
      toast.error("Not saved", { description: errorMessage(e) })
    },
  })

  const value = React.useMemo<DraftState>(
    () => ({
      ready: !!draft,
      loadError: query.error,
      saved,
      draft,
      secrets,
      changes,
      saving: mutation.isPending,
      error,
      get: (path) => getAt(draft, path),
      set: (path, v) => {
        setError(null)
        setDraft((d) => (d ? setAt(d, path, v) : d))
      },
      update: (fn) => {
        setError(null)
        setDraft((d) => (d ? fn(d) : d))
      },
      reset: (path) => setDraft((d) => (d ? setAt(d, path, getAt(saved, path)) : d)),
      isDirty: (path) => !deepEqual(getAt(saved, path), getAt(draft, path)),
      discard: () => {
        setError(null)
        setDraft(saved)
      },
      save: async () => {
        if (!draft) return undefined
        try {
          return await mutation.mutateAsync(draft)
        } catch {
          return undefined
        }
      },
      adopt: (r) => {
        qc.setQueryData(keys.config, r)
        setSaved(r.config)
        setDraft(r.config)
        setSecrets(r.secrets)
        setError(null)
      },
    }),
    [draft, saved, secrets, changes, mutation, error, query.error, qc],
  )

  return <DraftContext.Provider value={value}>{children}</DraftContext.Provider>
}

export function useDraft() {
  const ctx = React.useContext(DraftContext)
  if (!ctx) throw new Error("useDraft outside DraftProvider")
  return ctx
}

/** One field of the draft: value, setter, dirty flag and reset. */
export function useField<T>(path: string) {
  const d = useDraft()
  return {
    value: d.get(path) as T,
    set: (v: T) => d.set(path, v),
    dirty: d.isDirty(path),
    reset: () => d.reset(path),
    ready: d.ready,
  }
}

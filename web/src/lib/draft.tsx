import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { api, errorMessage } from "@/lib/api"
import { deepEqual, diffPaths, getAt, setAt } from "@/lib/path"
import { keys, useConfig } from "@/lib/queries"
import { announceSave } from "@/lib/restart"
import { SECRET_PATHS, type Config, type ConfigResponse, type SaveResponse, type SecretsMap } from "@/lib/types"

// The draft config: every edit on every page lands here first. The floating
// save bar shows while it differs from the saved copy, and "Save & apply"
// PUTs the whole draft. Secret fields stay null (= keep) unless replaced.
//
// The draft survives a remount (the login screen after a 401) in
// sessionStorage, without replaced secrets. When the server copy changes
// while the draft is dirty, `remote` holds it until the user picks a side.

interface DraftState {
  ready: boolean
  loadError: unknown
  saved: Config | null
  draft: Config | null
  secrets: SecretsMap
  changes: string[]
  saving: boolean
  error: string | null
  /** Inputs that hold text the draft could not take (out of range, say). */
  invalid: number
  /** Bumped when the draft is replaced wholesale; inputs drop local text. */
  epoch: number
  /** The config changed on the Pi while this draft was dirty. */
  remote: Config | null
  get: (path: string) => unknown
  set: (path: string, value: unknown) => void
  update: (fn: (c: Config) => Config) => void
  reset: (path: string) => void
  isDirty: (path: string) => boolean
  discard: () => void
  save: () => Promise<SaveResponse | undefined>
  /** Replace both copies after a save elsewhere (YAML editor, upload). */
  adopt: (r: ConfigResponse) => void
  setInvalid: (key: string, invalid: boolean) => void
  /** Keep my changes on top of the config from the Pi. */
  rebase: () => void
  /** Drop my changes and take the config from the Pi. */
  takeRemote: () => void
}

const DraftContext = React.createContext<DraftState | null>(null)

const STORE = "rackscreen.draft"

interface Stored {
  saved: Config
  draft: Config
}

function loadStored(): Stored | null {
  try {
    const raw = sessionStorage.getItem(STORE)
    if (!raw) return null
    const s = JSON.parse(raw) as Stored
    return s && s.saved && s.draft ? s : null
  } catch {
    return null
  }
}

function store(saved: Config, draft: Config) {
  try {
    if (deepEqual(saved, draft)) {
      sessionStorage.removeItem(STORE)
      return
    }
    // Never park a typed secret in storage: null keeps the stored value.
    let d = draft
    for (const p of SECRET_PATHS) if (getAt(d, p) !== null) d = setAt(d, p, null)
    sessionStorage.setItem(STORE, JSON.stringify({ saved, draft: d }))
  } catch {
    // Storage full or blocked: the draft just does not survive a remount.
  }
}

/** My edits (saved → draft) applied on top of `base`. */
function rebaseDraft(saved: Config, draft: Config, base: Config): Config {
  let next = base
  for (const p of diffPaths(saved, draft)) next = setAt(next, p, getAt(draft, p))
  return next
}

export function DraftProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient()
  const query = useConfig()
  const [saved, setSaved] = React.useState<Config | null>(null)
  const [draft, setDraft] = React.useState<Config | null>(null)
  const [secrets, setSecrets] = React.useState<SecretsMap>({})
  const [error, setError] = React.useState<string | null>(null)
  const [seen, setSeen] = React.useState<ConfigResponse | undefined>(undefined)
  const [remote, setRemote] = React.useState<Config | null>(null)
  const [invalidKeys, setInvalidKeys] = React.useState<ReadonlySet<string>>(() => new Set())
  const [epoch, setEpoch] = React.useState(0)
  // The latest draft for callbacks that outlive a render (the save mutation).
  const draftRef = React.useRef(draft)
  React.useEffect(() => {
    draftRef.current = draft
  }, [draft])

  // Adopt fresh server data (render-time sync). Re-baseline only while the
  // draft is clean; a dirty draft keeps its baseline and flags the conflict.
  if (query.data && query.data !== seen) {
    setSeen(query.data)
    setSecrets(query.data.secrets)
    const incoming = query.data.config
    if (!saved || !draft) {
      const s = loadStored()
      if (s && !deepEqual(s.saved, s.draft)) {
        setSaved(s.saved)
        setDraft(s.draft)
        if (!deepEqual(s.saved, incoming)) setRemote(incoming)
      } else {
        setSaved(incoming)
        setDraft(incoming)
      }
    } else if (deepEqual(incoming, saved)) {
      setRemote(null)
    } else if (deepEqual(draft, saved)) {
      setSaved(incoming)
      setDraft(incoming)
      setRemote(null)
    } else {
      setRemote(incoming)
    }
  }

  React.useEffect(() => {
    if (saved && draft) store(saved, draft)
  }, [saved, draft])

  const changes = React.useMemo(
    () => (saved && draft ? diffPaths(saved, draft) : []),
    [saved, draft],
  )

  const mutation = useMutation({
    mutationFn: (c: Config) => api.saveConfig(c),
    onSuccess: (r, sent) => {
      const before = saved
      setError(null)
      qc.setQueryData(keys.config, { config: r.config, secrets: r.secrets })
      setSaved(r.config)
      setRemote(null)
      // Edits made while the PUT was in flight stay in the draft.
      const current = draftRef.current
      if (!current || deepEqual(current, sent)) {
        setDraft(r.config)
        setEpoch((e) => e + 1)
      } else {
        // Secrets that went out with this save are stored now: back to "keep".
        let d = current
        for (const p of SECRET_PATHS) {
          const v = getAt(d, p)
          if (v !== null && v === getAt(sent, p)) d = setAt(d, p, null)
        }
        setDraft(d)
      }
      setSecrets(r.secrets)
      announceSave(r, before)
      void qc.invalidateQueries({ queryKey: keys.roles })
      void qc.invalidateQueries({ queryKey: keys.status })
      void qc.invalidateQueries({ queryKey: keys.spotify })
    },
    onError: (e) => {
      setError(errorMessage(e))
      toast.error("Not saved; see the message below")
    },
  })

  const setInvalid = React.useCallback((key: string, on: boolean) => {
    setInvalidKeys((s) => {
      if (s.has(key) === on) return s
      const next = new Set(s)
      if (on) next.add(key)
      else next.delete(key)
      return next
    })
  }, [])

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
      invalid: invalidKeys.size,
      epoch,
      remote,
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
        setEpoch((e) => e + 1)
      },
      save: async () => {
        if (!draft || invalidKeys.size > 0) return undefined
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
        setRemote(null)
        setError(null)
        setEpoch((e) => e + 1)
      },
      setInvalid,
      rebase: () => {
        if (!remote || !saved || !draft) return
        setDraft(rebaseDraft(saved, draft, remote))
        setSaved(remote)
        setRemote(null)
        setEpoch((e) => e + 1)
      },
      takeRemote: () => {
        if (!remote) return
        setSaved(remote)
        setDraft(remote)
        setRemote(null)
        setError(null)
        setEpoch((e) => e + 1)
      },
    }),
    [draft, saved, secrets, changes, mutation, error, query.error, qc, invalidKeys, epoch, remote, setInvalid],
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

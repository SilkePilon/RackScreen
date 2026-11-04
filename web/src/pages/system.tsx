import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import {
  ArrowUpCircleIcon,
  CheckIcon,
  CodeIcon,
  DownloadIcon,
  LogOutIcon,
  RefreshCwIcon,
  RotateCwIcon,
  TriangleAlertIcon,
  UploadIcon,
} from "lucide-react"
import { toast } from "sonner"

import { CopyButton } from "@/components/copy-button"
import { PageBody, PageIntro } from "@/components/page"
import { Segmented } from "@/components/segmented"
import {
  NumberRow,
  Row,
  RowSkeleton,
  Section,
  SecretRow,
  SettingsCard,
  TextRow,
} from "@/components/settings"
import { useTheme } from "@/components/theme-provider"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
import { Progress } from "@/components/ui/progress"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, errorMessage } from "@/lib/api"
import { useDraft } from "@/lib/draft"
import { announceSave } from "@/lib/restart"
import { keys, useAuth, useControl, useInfo, useStatus, useUpdate } from "@/lib/queries"

import { ThisPi } from "@/pages/system-pi"

const YamlEditor = React.lazy(() => import("@/components/yaml-editor"))

export default function System() {
  return (
    <PageBody>
      <PageIntro title="System">
        The Pi itself, version and updates, who can open this page, and the config file itself.
      </PageIntro>
      <ThisPi />
      <UpdateSection />
      <WebAccess />
      <Appearance />
      <ConfigSection />
      <Service />
    </PageBody>
  )
}

// The update restarts the service, which may reset the job state. Remember
// what we updated from, so the outcome can be told after reconnecting.
const UPDATE_KEY = "rackscreen.update"
interface PendingUpdate {
  from: string
  to: string | null
  at: number
}
function loadPendingUpdate(): PendingUpdate | null {
  try {
    const raw = sessionStorage.getItem(UPDATE_KEY)
    return raw ? (JSON.parse(raw) as PendingUpdate) : null
  } catch {
    return null
  }
}
function setPendingUpdate(p: PendingUpdate | null) {
  try {
    if (p) sessionStorage.setItem(UPDATE_KEY, JSON.stringify(p))
    else sessionStorage.removeItem(UPDATE_KEY)
  } catch {
    // Without storage the outcome is still told from the job state.
  }
}

function UpdateSection() {
  const qc = useQueryClient()
  const update = useUpdate()
  const info = useInfo()
  const u = update.data
  const start = useMutation({
    mutationFn: api.startUpdate,
    onMutate: () => {
      if (u) setPendingUpdate({ from: u.current, to: u.latest, at: Date.now() })
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.update }),
    onError: (e) => {
      setPendingUpdate(null)
      toast.error("Update did not start", { description: errorMessage(e) })
    },
  })
  const [prevState, setPrevState] = React.useState(u?.job.state)
  if (u && u.job.state !== prevState) {
    setPrevState(u.job.state)
    // An update started elsewhere (no pending record here) that failed in view.
    if (prevState === "running" && u.job.state === "failed" && !loadPendingUpdate())
      toast.error("Update failed; see the log below")
  }

  const version = info.data?.version
  React.useEffect(() => {
    const p = loadPendingUpdate()
    if (!p || !u || u.job.state === "running" || update.isError) return
    const now = version ?? u.current
    const age = Date.now() - p.at
    if (now !== p.from || u.current !== p.from) {
      setPendingUpdate(null)
      toast.success(`Updated to v${u.current !== p.from ? u.current : now}`)
    } else if (u.job.state === "failed") {
      setPendingUpdate(null)
      toast.error("Update failed; see the log below")
    } else if ((u.job.state === "idle" && age > 15_000) || age > 5 * 60_000) {
      // Back up on the old version with no job: the update did not take.
      setPendingUpdate(null)
      toast.error("Update failed", { description: `RackScreen is still on v${p.from}.` })
    }
  }, [u, version, update.isError])

  const running = u?.job.state === "running"
  return (
    <Section title="Version">
      <SettingsCard>
        <Row
          id="update"
          title={u ? `RackScreen v${u.current}` : <Skeleton className="h-4 w-36" />}
          description={
            !u
              ? undefined
              : u.newer
                ? `${u.latest} is out.${u.can_self_update ? "" : " Run this on the Pi to install it:"}`
                : u.latest
                  ? "You run the latest release."
                  : "Could not check for a newer release; the Pi may be offline."
          }
          badge={
            u?.newer ? (
              <Badge className="gap-1 font-normal">
                <ArrowUpCircleIcon />
                {u.latest}
              </Badge>
            ) : u?.latest ? (
              <Badge variant="secondary" className="gap-1 font-normal text-muted-foreground">
                <CheckIcon />
                Up to date
              </Badge>
            ) : null
          }
        >
          {u?.newer && u.can_self_update ? (
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button disabled={running || start.isPending}>
                  {running && <Spinner data-icon="inline-start" />}
                  {running ? "Updating…" : `Update to ${u.latest}`}
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Update to {u.latest}?</AlertDialogTitle>
                  <AlertDialogDescription>
                    RackScreen downloads the release, replaces itself and restarts. The panels and this page go
                    away for a moment and come back on their own.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Not now</AlertDialogCancel>
                  <AlertDialogAction onClick={() => start.mutate()}>Update</AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          ) : u?.newer ? (
            <div className="flex items-center gap-1.5">
              <code className="rounded-md border bg-muted/50 px-2 py-1 font-mono text-xs">sudo rackscreen update</code>
              <CopyButton text="sudo rackscreen update" iconOnly label="Copy command" variant="ghost" />
            </div>
          ) : (
            <Button variant="ghost" size="sm" onClick={() => void update.refetch()} disabled={update.isFetching}>
              <RefreshCwIcon data-icon="inline-start" className={update.isFetching ? "animate-spin" : undefined} />
              Check again
            </Button>
          )}
        </Row>
        {u && u.job.state !== "idle" && u.job.log.length > 0 && (
          <div className="flex flex-col gap-3 px-5 py-4">
            {running && <Progress value={Math.min(95, u.job.log.length * 20)} className="h-1" />}
            <ol className="flex flex-col gap-1 font-mono text-xs text-muted-foreground">
              {u.job.log.map((l, i) => (
                <li key={i} className="flex gap-2">
                  <span className="text-muted-foreground/50 tabular">{String(i + 1).padStart(2, "0")}</span>
                  {l}
                </li>
              ))}
            </ol>
          </div>
        )}
      </SettingsCard>
    </Section>
  )
}

function WebAccess() {
  const d = useDraft()
  const auth = useAuth()
  const qc = useQueryClient()
  const logout = useMutation({
    mutationFn: api.logout,
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.auth }),
  })
  return (
    <Section title="Web access" description="Changing the address or port restarts the whole service.">
      {d.ready ? (
        <SettingsCard>
          <SecretRow id="web.password" />
          <TextRow id="web.bind" mono width="sm:w-40" />
          <NumberRow id="web.port" min={1} max={65535} />
          {auth.data?.required && (
            <Row title="This browser" description="You are logged in for 30 days.">
              <Button variant="outline" size="sm" onClick={() => logout.mutate()}>
                <LogOutIcon data-icon="inline-start" />
                Log out
              </Button>
            </Row>
          )}
        </SettingsCard>
      ) : (
        <RowSkeleton count={3} />
      )}
    </Section>
  )
}

function Appearance() {
  const { theme, setTheme } = useTheme()
  return (
    <Section title="Appearance">
      <SettingsCard>
        <Row title="Theme" description="Stored in this browser only.">
          <Segmented
            label="Theme"
            value={theme}
            onChange={setTheme}
            options={[
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
              { value: "system", label: "System" },
            ]}
          />
        </Row>
      </SettingsCard>
    </Section>
  )
}

function ConfigSection() {
  const info = useInfo()
  const d = useDraft()
  const [editor, setEditor] = React.useState(false)
  const file = React.useRef<HTMLInputElement>(null)
  const [pending, setPending] = React.useState<{ name: string; text: string } | null>(null)
  const [uploading, setUploading] = React.useState(false)

  const download = async () => {
    try {
      const r = await api.yaml()
      const blob = new Blob([r.yaml], { type: "application/yaml" })
      const a = document.createElement("a")
      const stamp = new Date().toISOString().slice(0, 10)
      a.href = URL.createObjectURL(blob)
      a.download = `rackscreen-${info.data?.hostname ?? "config"}-${stamp}.yaml`
      a.click()
      setTimeout(() => URL.revokeObjectURL(a.href), 1000)
    } catch (e) {
      toast.error("Could not download the config", { description: errorMessage(e) })
    }
  }

  const restore = async () => {
    if (!pending) return
    setUploading(true)
    try {
      const r = await api.saveYaml(pending.text)
      const before = d.saved
      d.adopt({ config: r.config, secrets: r.secrets })
      announceSave(r, before, `Restored ${pending.name}`)
      setPending(null)
    } catch (e) {
      toast.error("Not restored", { description: errorMessage(e) })
    } finally {
      setUploading(false)
    }
  }

  return (
    <Section title="Configuration">
      {info.data && !info.data.config_writable && (
        <Alert className="border-warning/30 bg-warning/[0.06]">
          <TriangleAlertIcon className="text-warning!" />
          <AlertTitle>The config file is read-only for the service</AlertTitle>
          <AlertDescription>
            {info.data.fix_hint ?? "Run `sudo rackscreen install` once to let the web UI save the config."}
          </AlertDescription>
        </Alert>
      )}
      <SettingsCard>
        <Row
          title="Config file"
          description={
            info.data ? (
              <span className="font-mono text-xs">{info.data.config_path}</span>
            ) : (
              <Skeleton className="mt-1 h-3 w-48" />
            )
          }
        >
          <Button variant="outline" size="sm" onClick={() => void download()}>
            <DownloadIcon data-icon="inline-start" />
            Download
          </Button>
          <Button variant="outline" size="sm" onClick={() => file.current?.click()}>
            <UploadIcon data-icon="inline-start" />
            Restore…
          </Button>
          <input
            ref={file}
            type="file"
            accept=".yaml,.yml,text/yaml,application/yaml"
            className="hidden"
            onChange={async (e) => {
              const f = e.target.files?.[0]
              e.target.value = ""
              if (f) setPending({ name: f.name, text: await f.text() })
            }}
          />
        </Row>
        <Row
          title="Edit as YAML"
          description="The whole file, secrets included. Validated before it is saved."
          stack={editor}
          className={editor ? "pb-0" : undefined}
        >
          {!editor && (
            <Button variant="outline" size="sm" onClick={() => setEditor(true)}>
              <CodeIcon data-icon="inline-start" />
              Open editor
            </Button>
          )}
        </Row>
        {editor && (
          <React.Suspense
            fallback={
              <div className="p-5">
                <Skeleton className="h-[420px] w-full rounded-lg" />
              </div>
            }
          >
            <YamlEditor />
          </React.Suspense>
        )}
      </SettingsCard>
      <AlertDialog open={!!pending} onOpenChange={(o) => !o && setPending(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Restore {pending?.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              It replaces the whole config, secrets included, after it passes validation. The panels restart.
              {d.changes.length > 0 && " Unsaved changes on the other pages are dropped."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault()
                void restore()
              }}
              disabled={uploading}
            >
              {uploading && <Spinner data-icon="inline-start" />}
              Restore
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Section>
  )
}

function uptime(since: number) {
  const s = Math.max(0, Math.floor(Date.now() / 1000 - since))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  return d ? `${d} d ${h} h` : h ? `${h} h ${m} min` : `${m} min`
}

function Service() {
  const info = useInfo()
  const status = useStatus()
  const restart = useControl(() => api.restart(), () => "Panels restarting")
  const i = info.data
  return (
    <Section title="Service">
      <SettingsCard>
        <Row
          title="Panels"
          description={
            status.data
              ? `${status.data.monitor.state === "running" ? "Running" : status.data.monitor.state} for ${uptime(status.data.monitor.since_unix)}. Restart reloads the config and reconnects every source.`
              : undefined
          }
        >
          <Button variant="outline" size="sm" onClick={() => restart.mutate(undefined)} disabled={restart.isPending}>
            <RotateCwIcon data-icon="inline-start" />
            Restart panels
          </Button>
        </Row>
        <Row
          title="Host"
          description={
            i ? `${i.hostname}${i.sim ? " · simulator" : ""} · up ${uptime(i.started_unix)}` : undefined
          }
        />
        {i?.addresses.map((a) => (
          <Row key={a} title="Address" description={<span className="font-mono text-xs">{a}</span>}>
            <CopyButton text={a} iconOnly label={`Copy ${a}`} variant="ghost" />
          </Row>
        ))}
      </SettingsCard>
    </Section>
  )
}

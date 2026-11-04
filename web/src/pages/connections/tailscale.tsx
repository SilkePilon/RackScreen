import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { Link } from "react-router"
import QRCode from "qrcode"
import {
  ChevronRightIcon,
  ExternalLinkIcon,
  GlobeLockIcon,
  KeyRoundIcon,
  LockKeyholeIcon,
  LaptopIcon,
  LogOutIcon,
  MonitorIcon,
  RouterIcon,
  ServerIcon,
  ShieldCheckIcon,
  SmartphoneIcon,
  TerminalIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { toast } from "sonner"

import { CopyButton } from "@/components/copy-button"
import { PageBody, PageIntro } from "@/components/page"
import { TsLogo } from "@/components/ts-logo"
import { Row, RowSkeleton, Section, SettingsCard } from "@/components/settings"
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
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { api, errorMessage } from "@/lib/api"
import { keys, useTailscale, useTailscaleSetup } from "@/lib/queries"
import { deviceName } from "@/lib/tailscale"
import type { TailscaleDevice, TailscaleState } from "@/lib/types"
import { cn } from "@/lib/utils"

const SETUP_CMD = "sudo rackscreen tailscale-setup"
const ADMIN = "https://login.tailscale.com/admin/machines"
const ACLS = "https://login.tailscale.com/admin/acls"

function ago(iso: string | null) {
  if (!iso) return "Never seen"
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
  if (!Number.isFinite(s)) return "Never seen"
  if (s < 90) return "Seen just now"
  if (s < 3600) return `Seen ${Math.round(s / 60)} min ago`
  if (s < 86400) return `Seen ${Math.round(s / 3600)} h ago`
  const d = Math.round(s / 86400)
  return `Seen ${d} day${d === 1 ? "" : "s"} ago`
}

type Phase = "setup" | "login" | "approve" | "connected"

function phaseOf(t: TailscaleState): Phase {
  if (!t.installed || !t.operator_ok || t.backend_state === null) return "setup"
  if (t.backend_state === "NeedsMachineAuth") return "approve"
  if (t.backend_state === "Running" || t.backend_state === "Starting") return "connected"
  // Stopped: logged in but down (tailscale down); still a known device
  if (t.backend_state === "Stopped" && t.self) return "connected"
  return "login"
}

export default function Tailscale() {
  const [waiting, setWaiting] = React.useState(false)
  const ts = useTailscale(waiting ? 2000 : 5000)
  const t = ts.data
  // Tell when a login that was waiting here goes through.
  const phase = t ? phaseOf(t) : null
  const [prev, setPrev] = React.useState(phase)
  if (prev !== phase) {
    setPrev(phase)
    if (phase === "connected" && (prev === "login" || prev === "approve")) {
      setWaiting(false)
      toast.success(t?.tailnet ? `Connected to ${t.tailnet}` : "Connected to Tailscale")
    }
  }
  return (
    <PageBody>
      <PageIntro title="Tailscale">
        Reach this page and the Pi from anywhere over your own private Tailscale network. No ports to open,
        free for personal use, and set up from here.
      </PageIntro>
      {ts.isError && !t ? (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>Could not read the Tailscale state</AlertTitle>
          <AlertDescription>{errorMessage(ts.error)}</AlertDescription>
        </Alert>
      ) : !t ? (
        <>
          <RowSkeleton count={1} />
          <RowSkeleton count={3} />
        </>
      ) : (
        <Body t={t} waiting={waiting} setWaiting={setWaiting} />
      )}
    </PageBody>
  )
}

function Body({
  t,
  waiting,
  setWaiting,
}: {
  t: TailscaleState
  waiting: boolean
  setWaiting: (v: boolean) => void
}) {
  const phase = phaseOf(t)
  const locked = !t.password_set
  return (
    <>
      {locked && <NeedsPassword />}
      {t.health.length > 0 && phase === "connected" && (
        <Alert>
          <TriangleAlertIcon />
          <AlertTitle>Tailscale reports {t.health.length === 1 ? "a problem" : "problems"}</AlertTitle>
          <AlertDescription>
            <ul className="flex flex-col gap-1">
              {t.health.map((h) => (
                <li key={h}>{h}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}
      {/* A disabled fieldset turns off every input, switch and button inside. */}
      <fieldset
        disabled={locked}
        aria-disabled={locked || undefined}
        className="contents [&_[role=switch]:disabled]:cursor-not-allowed [&_[role=switch]:disabled]:opacity-50"
      >
        {phase === "setup" && <Setup t={t} />}
        {phase === "login" && <Login t={t} waiting={waiting} setWaiting={setWaiting} />}
        {phase === "approve" && <Approve />}
        {phase === "connected" && <Connected t={t} />}
      </fieldset>
    </>
  )
}

/** Tailscale can open the Pi to a whole tailnet, so it waits for a web password. */
function NeedsPassword() {
  return (
    <Alert>
      <LockKeyholeIcon />
      <AlertTitle>Set a web password first</AlertTitle>
      <AlertDescription>
        <p>
          Tailscale can give remote access to this Pi, so RackScreen only manages it once this page asks for a
          password. Set one under System → Web access, then come back here.
        </p>
        <Button asChild size="sm" className="mt-2 no-underline!">
          <Link to="/system#web.password">Set a password</Link>
        </Button>
      </AlertDescription>
    </Alert>
  )
}

/**
 * The Tailscale SSH switch. Turning it on asks first: the tailnet's access
 * rules, not this page, decide who may log in and as whom.
 */
function SshSwitch({
  id,
  checked,
  onChange,
  disabled,
}: {
  id?: string
  checked: boolean
  onChange: (on: boolean) => void
  disabled?: boolean
}) {
  const [asking, setAsking] = React.useState(false)
  return (
    <>
      <Switch
        id={id}
        checked={checked}
        onCheckedChange={(v) => (v ? setAsking(true) : onChange(false))}
        disabled={disabled}
        aria-label="Tailscale SSH"
      />
      <AlertDialog open={asking} onOpenChange={setAsking}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Turn on Tailscale SSH?</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="flex flex-col gap-2">
                <p>
                  Your tailnet's access rules (ACLs) decide who may SSH into the Pi and as which user,
                  possibly root. RackScreen does not check them.
                </p>
                <p>
                  Look at the <span className="text-foreground">ssh</span> section of your{" "}
                  <a
                    href={ACLS}
                    target="_blank"
                    rel="noreferrer"
                    className="text-foreground underline underline-offset-4"
                  >
                    access controls
                  </a>{" "}
                  before you turn this on, especially on a shared tailnet.
                </p>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={() => onChange(true)}>Turn on SSH</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

// ---------------------------------------------------------------------------
// Install / finish setup.

function Setup({ t }: { t: TailscaleState }) {
  const qc = useQueryClient()
  const [started, setStarted] = React.useState(false)
  const job = useTailscaleSetup(started)
  const j = job.data
  const running = j?.state === "running"
  const start = useMutation({
    mutationFn: api.tailscaleSetup,
    onSuccess: () => {
      setStarted(true)
      void qc.invalidateQueries({ queryKey: keys.tailscaleSetup })
    },
    onError: (e) => toast.error("Setup did not start", { description: errorMessage(e) }),
  })
  const [prevState, setPrevState] = React.useState(j?.state)
  if (j && j.state !== prevState) {
    setPrevState(j.state)
    if (prevState === "running" && j.state === "done") {
      setStarted(false)
      toast.success(t.installed ? "Tailscale is ready" : "Tailscale is installed")
      void qc.invalidateQueries({ queryKey: keys.tailscale })
    } else if (prevState === "running" && j.state === "failed") {
      setStarted(false)
      toast.error("Tailscale setup failed; see the log")
    }
  }
  const [logOpen, setLogOpen] = React.useState(false)
  const failed = j?.state === "failed"

  const title = !t.installed ? "Tailscale is not installed" : "Tailscale needs one more step"
  const description = !t.installed
    ? "RackScreen installs it with Tailscale's official installer, starts it, and lets this service manage it, so you can log in right here. Takes a minute or two."
    : (t.error ??
      "Tailscale is installed, but this service may not manage it yet. Setup starts it and allows that.")

  return (
    <Section title="Set up" description="Once, as root. Everything after happens from this page.">
      <SettingsCard>
        <Row
          title={
            <span className="flex items-center gap-2">
              <TsLogo className="size-4" />
              {title}
            </span>
          }
          description={description}
          badge={
            t.version ? (
              <Badge variant="secondary" className="tabular font-normal text-muted-foreground">
                v{t.version}
              </Badge>
            ) : undefined
          }
        >
          {t.can_setup && (
            <Button onClick={() => start.mutate()} disabled={running || start.isPending}>
              {(running || start.isPending) && <Spinner data-icon="inline-start" />}
              {running
                ? t.installed
                  ? "Setting up…"
                  : "Installing…"
                : t.installed
                  ? "Finish setup"
                  : "Install Tailscale"}
            </Button>
          )}
        </Row>
        {!t.can_setup && (
          <div className="flex flex-col gap-3 px-5 py-4 text-[13px] text-muted-foreground">
            <p>Run this once on the Pi over SSH, then come back here to log in:</p>
            <div className="flex items-center gap-1.5">
              <code className="rounded-md border bg-muted/50 px-2 py-1 font-mono text-xs text-foreground">
                {SETUP_CMD}
              </code>
              <CopyButton text={SETUP_CMD} iconOnly label="Copy command" variant="ghost" />
            </div>
            <p>
              Or run <code className="font-mono text-xs text-foreground">sudo rackscreen install</code> once
              to allow the one-click setup from this page.
            </p>
          </div>
        )}
        {j && j.state !== "idle" && j.log.length > 0 && (
          <Collapsible open={logOpen || failed} onOpenChange={setLogOpen} className="px-5 py-3">
            <CollapsibleTrigger className="group flex w-full items-center gap-2 text-[13px] text-muted-foreground transition-colors hover:text-foreground">
              <ChevronRightIcon className="size-3.5 transition-transform group-data-[state=open]:rotate-90" />
              {running ? (
                <span className="flex items-center gap-2">
                  {j.log.at(-1)}
                  <span className="inline-block size-1.5 animate-pulse rounded-full bg-primary" />
                </span>
              ) : failed ? (
                <span className="text-destructive">Setup failed</span>
              ) : (
                "Setup log"
              )}
            </CollapsibleTrigger>
            <CollapsibleContent>
              <ol className="mt-3 flex max-h-64 scrollbar-thin flex-col gap-1 overflow-y-auto font-mono text-xs text-muted-foreground">
                {j.log.map((l, i) => (
                  <li key={i} className="flex gap-2">
                    <span className="tabular text-muted-foreground/50">{String(i + 1).padStart(2, "0")}</span>
                    <span className="break-all">{l}</span>
                  </li>
                ))}
              </ol>
            </CollapsibleContent>
          </Collapsible>
        )}
      </SettingsCard>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Log in.

function Login({
  t,
  waiting,
  setWaiting,
}: {
  t: TailscaleState
  waiting: boolean
  setWaiting: (v: boolean) => void
}) {
  const [name, setName] = React.useState("")
  const [ssh, setSsh] = React.useState(false)
  const [key, setKey] = React.useState("")
  const [showKey, setShowKey] = React.useState(false)
  const [url, setUrl] = React.useState<string | null>(null)
  const [qr, setQr] = React.useState("")
  const qc = useQueryClient()

  const nameErr = name && !/^[a-z0-9-]{1,63}$/.test(name) ? "Lowercase letters, digits and dashes." : null
  const keyErr =
    key.trim() && !/^tskey-[A-Za-z0-9_-]+$/.test(key.trim()) ? "An auth key starts with tskey-." : null
  const base = { ...(name ? { hostname: name } : {}), ssh }

  const browser = useMutation({
    mutationFn: () => api.tailscaleLogin(base),
    onSuccess: (r) => {
      if (r.auth_url) {
        setUrl(r.auth_url)
        setWaiting(true)
        window.open(r.auth_url, "_blank", "noopener")
      }
      void qc.invalidateQueries({ queryKey: keys.tailscale })
    },
    onError: (e) => toast.error("Could not start the login", { description: errorMessage(e) }),
  })
  const withKey = useMutation({
    mutationFn: () => api.tailscaleLogin({ ...base, auth_key: key.trim() }),
    onSuccess: () => {
      setKey("")
      setWaiting(true)
      void qc.invalidateQueries({ queryKey: keys.tailscale })
    },
  })

  React.useEffect(() => {
    if (!url) return
    let alive = true
    void QRCode.toString(url, { type: "svg", margin: 1, errorCorrectionLevel: "L" }).then((svg) => {
      if (alive) setQr(svg)
    })
    return () => {
      alive = false
    }
  }, [url])

  const busy = browser.isPending || withKey.isPending
  return (
    <>
      <Section title="This device" description="How the Pi shows up on your tailnet.">
        <SettingsCard>
          <Row
            title="Device name"
            htmlFor="ts-name"
            description={
              nameErr ? (
                <span className="text-destructive">{nameErr}</span>
              ) : (
                <>
                  Becomes <span className="font-mono text-xs text-foreground">{name || t.hostname}</span> in
                  MagicDNS. Leave empty to keep the Pi's hostname.
                </>
              )
            }
          >
            <Input
              id="ts-name"
              value={name}
              placeholder={t.hostname}
              onChange={(e) => setName(e.target.value.toLowerCase().replace(/\s+/g, "-"))}
              disabled={waiting}
              aria-invalid={!!nameErr || undefined}
              spellCheck={false}
              autoComplete="off"
              className="w-full font-mono text-[13px] sm:w-56"
            />
          </Row>
          <Row
            title="Allow Tailscale SSH"
            htmlFor="ts-ssh"
            description="Off by default. Devices your tailnet's access rules allow can SSH into the Pi, signed in with Tailscale instead of keys. You can change this later."
          >
            <SshSwitch id="ts-ssh" checked={ssh} onChange={setSsh} disabled={waiting} />
          </Row>
        </SettingsCard>
      </Section>

      <Section
        title="Log in"
        description="Pick one. The Pi joins the tailnet of the account you log in with."
      >
        <SettingsCard>
          <Row
            title="Log in with your browser"
            description="Opens Tailscale's login in a new tab. Approve the Pi there, on this device or on your phone."
          >
            <Button
              variant={url ? "outline" : "default"}
              onClick={() => browser.mutate()}
              disabled={busy || !!nameErr}
            >
              {browser.isPending && <Spinner data-icon="inline-start" />}
              {url ? "Start over" : "Log in with browser"}
              {!browser.isPending && !url && <ExternalLinkIcon data-icon="inline-end" />}
            </Button>
          </Row>
          {url && (
            <div className="grid animate-in gap-6 px-5 py-5 fade-in-0 slide-in-from-top-1 md:grid-cols-[1fr_auto]">
              <div className="flex flex-col gap-4">
                <div className="flex items-center gap-3">
                  <span className="relative flex size-2.5">
                    <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary opacity-60" />
                    <span className="relative inline-flex size-2.5 rounded-full bg-primary" />
                  </span>
                  <span className="text-sm font-medium">Waiting for you to approve the Pi…</span>
                </div>
                <p className="max-w-md text-[13px] text-muted-foreground">
                  A new tab opened with the Tailscale login. No tab? Open it below, or scan the code with your
                  phone. This page notices on its own once you are done.
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button asChild size="sm">
                    <a href={url} target="_blank" rel="noreferrer">
                      Open login
                      <ExternalLinkIcon data-icon="inline-end" />
                    </a>
                  </Button>
                  <CopyButton text={url} label="Copy link" />
                </div>
              </div>
              <div className="flex flex-col items-center gap-2 self-start">
                <div className="size-36 overflow-hidden rounded-xl bg-white p-2 shadow-sm">
                  {qr ? (
                    <div className="size-full [&>svg]:size-full" dangerouslySetInnerHTML={{ __html: qr }} />
                  ) : (
                    <Skeleton className="size-full bg-black/10" />
                  )}
                </div>
                <span className="text-xs text-muted-foreground">Scan to log in on your phone</span>
              </div>
            </div>
          )}
          <Row
            title="Use an auth key"
            htmlFor="ts-key"
            description={
              keyErr ? (
                <span className="text-destructive">{keyErr}</span>
              ) : (
                <>
                  For a headless login: create one under{" "}
                  <a
                    href="https://login.tailscale.com/admin/settings/keys"
                    target="_blank"
                    rel="noreferrer"
                    className="text-foreground underline-offset-4 hover:underline"
                  >
                    Settings → Keys
                  </a>{" "}
                  in the admin console. It is used once and not stored.
                </>
              )
            }
          >
            <form
              className="flex w-full items-center gap-2 sm:w-auto"
              onSubmit={(e) => {
                e.preventDefault()
                if (key.trim() && !keyErr && !nameErr) withKey.mutate()
              }}
            >
              <InputGroup className="w-full sm:w-60">
                <InputGroupAddon>
                  <KeyRoundIcon />
                </InputGroupAddon>
                <InputGroupInput
                  id="ts-key"
                  type={showKey ? "text" : "password"}
                  value={key}
                  onChange={(e) => {
                    setKey(e.target.value)
                    withKey.reset()
                  }}
                  placeholder="tskey-auth-…"
                  autoComplete="off"
                  spellCheck={false}
                  aria-invalid={!!keyErr || withKey.isError || undefined}
                  className="font-mono text-[13px]"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupButton size="xs" onClick={() => setShowKey((s) => !s)}>
                    {showKey ? "Hide" : "Show"}
                  </InputGroupButton>
                </InputGroupAddon>
              </InputGroup>
              <Button type="submit" variant="outline" disabled={!key.trim() || !!keyErr || !!nameErr || busy}>
                {withKey.isPending && <Spinner data-icon="inline-start" />}
                Connect
              </Button>
            </form>
          </Row>
          {withKey.isError && (
            <div className="px-5 py-3">
              <Alert variant="destructive">
                <TriangleAlertIcon />
                <AlertTitle>Not connected</AlertTitle>
                <AlertDescription>{errorMessage(withKey.error)}</AlertDescription>
              </Alert>
            </div>
          )}
        </SettingsCard>
      </Section>
      {waiting && !url && (
        <p className="flex items-center gap-2 px-1 text-[13px] text-muted-foreground">
          <Spinner className="size-3.5" /> Connecting…
        </p>
      )}
    </>
  )
}

function Approve() {
  return (
    <Section title="Waiting for approval">
      <SettingsCard>
        <Row
          title={
            <span className="flex items-center gap-2">
              <span className="relative flex size-2.5">
                <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary opacity-60" />
                <span className="relative inline-flex size-2.5 rounded-full bg-primary" />
              </span>
              An admin needs to approve the Pi
            </span>
          }
          description="Your tailnet requires device approval. Approve the Pi in the admin console; this page updates on its own."
        >
          <Button asChild variant="outline">
            <a href={ADMIN} target="_blank" rel="noreferrer">
              Open admin console
              <ExternalLinkIcon data-icon="inline-end" />
            </a>
          </Button>
        </Row>
      </SettingsCard>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Connected.

function FactRow({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 px-5 py-3">
      <span className="shrink-0 text-sm text-muted-foreground">{k}</span>
      <span className="flex min-w-0 items-center gap-1 text-right text-[13px] text-foreground">
        {children}
      </span>
    </div>
  )
}

function Copyable({ text }: { text: string }) {
  return (
    <>
      <span className="tabular truncate font-mono text-xs">{text}</span>
      <span className="-my-1">
        <CopyButton text={text} iconOnly size="icon-xs" label={`Copy ${text}`} variant="ghost" />
      </span>
    </>
  )
}

function useTsAction<A>(fn: (a: A) => Promise<unknown>, ok?: (a: A) => string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSuccess: (_d, a) => {
      if (ok) toast.success(ok(a))
      void qc.invalidateQueries({ queryKey: keys.tailscale })
    },
    onError: (e) => toast.error(errorMessage(e)),
  })
}

function Connected({ t }: { t: TailscaleState }) {
  const running = t.backend_state === "Running"
  const me = t.self
  const upDown = useTsAction(
    (on: boolean) => (on ? api.tailscaleUp() : api.tailscaleDown()),
    (on) => (on ? "Connected to Tailscale" : "Disconnected from Tailscale")
  )
  const ssh = useTsAction(api.tailscaleSsh, (on) => (on ? "Tailscale SSH on" : "Tailscale SSH off"))
  const logout = useTsAction(api.tailscaleLogout, () => "Logged out of Tailscale")
  // Optimistic switches while the command runs.
  const connected = upDown.isPending ? upDown.variables : running
  const sshOn = ssh.isPending ? ssh.variables : t.ssh
  const ipv4 = me?.ips.find((ip) => !ip.includes(":"))
  const ipv6 = me?.ips.find((ip) => ip.includes(":"))

  return (
    <>
      <Section title="This device">
        <SettingsCard>
          <Row
            title={
              <span className="flex items-center gap-2.5">
                <TsLogo className="size-4" />
                {me ? deviceName(me) : t.hostname}
              </span>
            }
            badge={
              connected ? (
                <Badge variant="secondary" className="gap-1.5 font-normal">
                  <span className="size-1.5 rounded-full bg-success" />
                  Connected
                </Badge>
              ) : (
                <Badge variant="outline" className="gap-1.5 font-normal text-muted-foreground">
                  <span className="size-1.5 rounded-full bg-muted-foreground/60" />
                  {t.backend_state === "Starting" ? "Starting" : "Disconnected"}
                </Badge>
              )
            }
            description={
              connected
                ? "On your tailnet. Turn off to leave it for now without logging out."
                : "Logged in, but not on the tailnet right now."
            }
          >
            <Switch
              checked={!!connected}
              onCheckedChange={(v) => upDown.mutate(v)}
              disabled={upDown.isPending}
              aria-label="Connected"
            />
          </Row>
          {me?.dns_name && (
            <FactRow k="MagicDNS">
              <Copyable text={me.dns_name} />
            </FactRow>
          )}
          {ipv4 && (
            <FactRow k="Tailscale IPv4">
              <Copyable text={ipv4} />
            </FactRow>
          )}
          {ipv6 && (
            <FactRow k="Tailscale IPv6">
              <Copyable text={ipv6} />
            </FactRow>
          )}
          {t.tailnet && <FactRow k="Tailnet">{t.tailnet}</FactRow>}
          {t.user && (
            <FactRow k="Account">
              <span className="truncate">
                {t.user.display_name}
                {t.user.login !== t.user.display_name && (
                  <span className="text-muted-foreground"> · {t.user.login}</span>
                )}
              </span>
            </FactRow>
          )}
          {t.version && (
            <FactRow k="Version">
              <span className="tabular">{t.version}</span>
            </FactRow>
          )}
        </SettingsCard>
      </Section>

      <Section title="Access" description="What your other devices can do with the Pi over Tailscale.">
        <SettingsCard>
          <ServeRow t={t} />
          <Row
            title={
              <span className="flex items-center gap-2">
                <TerminalIcon className="size-4 text-muted-foreground" />
                Tailscale SSH
              </span>
            }
            description="Devices your tailnet's access rules allow can SSH into the Pi, signed in with Tailscale instead of keys."
          >
            <SshSwitch checked={!!sshOn} onChange={(v) => ssh.mutate(v)} disabled={ssh.isPending} />
          </Row>
        </SettingsCard>
      </Section>

      <Section
        title="Devices"
        description={
          t.peers.length
            ? `${t.peers.filter((p) => p.online).length} of ${t.peers.length} online on your tailnet.`
            : "The other devices on your tailnet show up here."
        }
        action={
          <a
            href={ADMIN}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
          >
            Admin console <ExternalLinkIcon className="size-3" />
          </a>
        }
      >
        <SettingsCard>
          {t.peers.length === 0 ? (
            <div className="px-5 py-6 text-center text-[13px] text-muted-foreground">
              Just the Pi so far. Install Tailscale on your phone or laptop to reach it from there.
            </div>
          ) : (
            <ul className="flex max-h-[26rem] scrollbar-thin flex-col divide-y divide-hairline overflow-y-auto">
              {t.peers.map((p) => (
                <PeerRow key={`${p.dns_name}-${p.ips[0] ?? p.hostname}`} p={p} />
              ))}
            </ul>
          )}
        </SettingsCard>
      </Section>

      <Section title="Account">
        <SettingsCard>
          <Row
            title="Log out"
            description="The Pi leaves your tailnet. This page stays reachable on your local network."
          >
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="outline" disabled={logout.isPending}>
                  {logout.isPending ? (
                    <Spinner data-icon="inline-start" />
                  ) : (
                    <LogOutIcon data-icon="inline-start" />
                  )}
                  Log out
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Log the Pi out of Tailscale?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Devices on your tailnet can no longer reach it, and an open Tailscale SSH session ends. If
                    you are on this page over Tailscale right now, it goes away; use the Pi's local address
                    instead.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction variant="destructive" onClick={() => logout.mutate(undefined)}>
                    Log out
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </Row>
        </SettingsCard>
      </Section>
    </>
  )
}

function ServeRow({ t }: { t: TailscaleState }) {
  const qc = useQueryClient()
  const [enableUrl, setEnableUrl] = React.useState<string | null>(null)
  const serve = useMutation({
    mutationFn: api.tailscaleServe,
    onSuccess: (r, on) => {
      if (r.enable_url) {
        setEnableUrl(r.enable_url)
      } else {
        setEnableUrl(null)
        toast.success(on ? "This page is shared over Tailscale" : "Stopped sharing over Tailscale")
      }
      void qc.invalidateQueries({ queryKey: keys.tailscale })
    },
    onError: (e) => toast.error(errorMessage(e)),
  })
  const on = serve.isPending ? serve.variables : t.serve.on
  return (
    <>
      <Row
        title={
          <span className="flex items-center gap-2">
            <GlobeLockIcon className="size-4 text-muted-foreground" />
            Share this page over Tailscale (HTTPS)
          </span>
        }
        description={
          <>
            Opens RackScreen at a private https address with a real certificate, for your tailnet only. That
            also makes <span className="text-foreground">Use my location</span> on the Sky page work, which
            browsers only allow over https.
          </>
        }
      >
        <Switch
          checked={on}
          onCheckedChange={(v) => serve.mutate(v)}
          disabled={serve.isPending}
          aria-label="Share this page over Tailscale"
        />
      </Row>
      {t.serve.on && t.serve.url && (
        <div className="flex animate-in flex-wrap items-center gap-2 bg-primary/[0.04] px-5 py-3 fade-in-0">
          <ShieldCheckIcon className="size-4 text-success" />
          <a
            href={t.serve.url}
            target="_blank"
            rel="noreferrer"
            className="font-mono text-[13px] text-foreground underline-offset-4 hover:underline"
          >
            {t.serve.url}
          </a>
          <span className="flex-1" />
          <CopyButton text={t.serve.url} label="Copy address" size="sm" variant="ghost" />
        </div>
      )}
      {enableUrl && !t.serve.on && (
        <div className="px-5 py-3">
          <Alert>
            <TriangleAlertIcon />
            <AlertTitle>Turn on HTTPS for your tailnet first</AlertTitle>
            <AlertDescription>
              <p>
                Tailscale needs HTTPS certificates allowed once for your tailnet. Allow it, then flip the
                switch again.
              </p>
              <Button asChild size="sm" variant="outline" className="mt-2">
                <a href={enableUrl} target="_blank" rel="noreferrer">
                  Allow HTTPS
                  <ExternalLinkIcon data-icon="inline-end" />
                </a>
              </Button>
            </AlertDescription>
          </Alert>
        </div>
      )}
    </>
  )
}

const OS_ICON: Record<string, typeof LaptopIcon> = {
  android: SmartphoneIcon,
  iOS: SmartphoneIcon,
  ios: SmartphoneIcon,
  macOS: LaptopIcon,
  windows: MonitorIcon,
  linux: ServerIcon,
}

const OS_LABEL: Record<string, string> = {
  android: "Android",
  iOS: "iOS",
  ios: "iOS",
  macOS: "macOS",
  windows: "Windows",
  linux: "Linux",
  freebsd: "FreeBSD",
}

function PeerRow({ p }: { p: TailscaleDevice }) {
  const Icon = p.exit_node ? RouterIcon : (OS_ICON[p.os] ?? MonitorIcon)
  const ip = p.ips.find((i) => !i.includes(":")) ?? p.ips[0]
  return (
    <li className="flex items-center gap-3 px-5 py-3">
      <span className="relative flex size-8 shrink-0 items-center justify-center rounded-lg border bg-muted/40">
        <Icon className={cn("size-4", p.online ? "text-foreground" : "text-muted-foreground")} />
        <span
          className={cn(
            "absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border-2 border-card",
            p.online ? "bg-success" : "bg-muted-foreground/50"
          )}
          aria-label={p.online ? "Online" : "Offline"}
        />
      </span>
      <div className="flex min-w-0 flex-1 flex-col">
        <span
          className={cn("flex items-center gap-2 truncate text-sm", !p.online && "text-muted-foreground")}
        >
          <span className="truncate">{deviceName(p)}</span>
          {p.exit_node && (
            <Badge variant="outline" className="h-5 font-normal text-muted-foreground">
              Exit node
            </Badge>
          )}
        </span>
        <span className="truncate text-xs text-muted-foreground">
          {OS_LABEL[p.os] ?? p.os}
          {" · "}
          {p.online ? "Online" : ago(p.last_seen)}
        </span>
      </div>
      {ip && (
        <span className="hidden items-center gap-1 text-muted-foreground sm:flex">
          <Copyable text={ip} />
        </span>
      )}
    </li>
  )
}

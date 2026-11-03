import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import QRCode from "qrcode"
import { CircleCheckIcon, ExternalLinkIcon, LogOutIcon, MusicIcon, TriangleAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { CopyButton } from "@/components/copy-button"
import { PageBody, PageIntro } from "@/components/page"
import { RestoreDefaults } from "@/components/restore-defaults"
import { Row, RowSkeleton, Section, SettingsCard, SwitchRow, TextRow } from "@/components/settings"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, errorMessage } from "@/lib/api"
import { useDraft } from "@/lib/draft"
import { keys, useSpotify } from "@/lib/queries"

const REDIRECT = "http://127.0.0.1:8888/callback"

export default function Spotify() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/spotify" />
      <PageIntro title="Spotify">
        Now playing from Spotify on the music role. Spotify needs your own (free) developer app; RackScreen then
        logs in once and keeps the token on the Pi.
      </PageIntro>
      {ready ? (
        <Body />
      ) : (
        <>
          <RowSkeleton count={2} />
          <RowSkeleton count={1} />
        </>
      )}
    </PageBody>
  )
}

function Body() {
  const d = useDraft()
  const id = ((d.get("spotify.client_id") as string) ?? "").trim()
  return (
    <>
      <Section
        title="App"
        action={
          <a
            href="https://developer.spotify.com/dashboard"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
          >
            Spotify dashboard <ExternalLinkIcon className="size-3" />
          </a>
        }
      >
        <SettingsCard>
          <SwitchRow id="spotify.enabled" />
          <TextRow
            id="spotify.client_id"
            mono
            width="sm:w-80"
            placeholder="32 letters and digits"
            validate={() => (/^[A-Za-z0-9]*$/.test(id) ? null : "Letters and digits only.")}
          />
          <Row
            title="Redirect URI"
            description="Add exactly this address to your app's Redirect URIs in the Spotify dashboard."
          >
            <code className="rounded-md border bg-muted/50 px-2 py-1 font-mono text-xs">{REDIRECT}</code>
            <CopyButton text={REDIRECT} iconOnly label="Copy redirect URI" variant="ghost" />
          </Row>
        </SettingsCard>
      </Section>
      <Login />
    </>
  )
}

function Login() {
  const qc = useQueryClient()
  const d = useDraft()
  const spotify = useSpotify()
  const [url, setUrl] = React.useState<string | null>(null)
  const [pasted, setPasted] = React.useState("")
  const [qr, setQr] = React.useState<string>("")
  const unsaved = d.isDirty("spotify.client_id") || d.isDirty("spotify.enabled")

  const start = useMutation({
    mutationFn: api.spotifyLogin,
    onSuccess: (r) => setUrl(r.url),
    onError: (e) => toast.error("Could not start the login", { description: errorMessage(e) }),
  })
  const finish = useMutation({
    mutationFn: () => api.spotifyCallback(pasted.trim()),
    onSuccess: (r) => {
      toast.success(r.status)
      setUrl(null)
      setPasted("")
      void qc.invalidateQueries({ queryKey: keys.spotify })
    },
  })
  const logout = useMutation({
    mutationFn: api.spotifyLogout,
    onSuccess: () => {
      toast("Logged out of Spotify")
      void qc.invalidateQueries({ queryKey: keys.spotify })
    },
    onError: (e) => toast.error(errorMessage(e)),
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

  const s = spotify.data
  return (
    <Section title="Login" description="Log in once from any device; the token stays on the Pi.">
      <SettingsCard>
        <Row
          title={
            s ? (
              <span className="flex items-center gap-2">
                {s.logged_in ? (
                  <CircleCheckIcon className="size-4 text-success" />
                ) : (
                  <MusicIcon className="size-4 text-muted-foreground" />
                )}
                {s.status}
              </span>
            ) : (
              <Skeleton className="h-4 w-36" />
            )
          }
          description={
            !s
              ? undefined
              : !s.enabled || !s.client_id_set
                ? "Turn Spotify on and save a client ID first."
                : s.logged_in
                  ? "The music role shows what you play."
                  : "Log in so the music role can see what you play."
          }
        >
          {s?.logged_in ? (
            <Button variant="outline" onClick={() => logout.mutate()} disabled={logout.isPending}>
              <LogOutIcon data-icon="inline-start" />
              Log out
            </Button>
          ) : (
            <Button
              onClick={() => start.mutate()}
              disabled={!s?.enabled || !s.client_id_set || unsaved || start.isPending}
            >
              {start.isPending && <Spinner data-icon="inline-start" />}
              {url ? "Restart login" : "Log in with Spotify"}
            </Button>
          )}
        </Row>
        {unsaved && !s?.logged_in && (
          <div className="px-5 py-3 text-[13px] text-muted-foreground">Save your changes before logging in.</div>
        )}
        {url && !s?.logged_in && (
          <div className="grid gap-6 px-5 py-5 animate-in fade-in-0 slide-in-from-top-1 md:grid-cols-[1fr_auto]">
            <ol className="flex flex-col gap-5 text-sm">
              <Step n={1} title="Open the Spotify login">
                <p className="text-muted-foreground">On this device, or scan the code with your phone.</p>
                <div className="flex flex-wrap gap-2 pt-1">
                  <Button asChild size="sm">
                    <a href={url} target="_blank" rel="noreferrer">
                      Open Spotify
                      <ExternalLinkIcon data-icon="inline-end" />
                    </a>
                  </Button>
                  <CopyButton text={url} label="Copy link" />
                </div>
              </Step>
              <Step n={2} title="Approve, then copy the address">
                <p className="text-muted-foreground">
                  Spotify sends you to <span className="font-mono text-xs">127.0.0.1:8888</span>, which will not
                  load. That is expected: copy the whole address from the address bar.
                </p>
              </Step>
              <Step n={3} title="Paste it here">
                <form
                  className="flex flex-col gap-2 pt-1 sm:flex-row"
                  onSubmit={(e) => {
                    e.preventDefault()
                    finish.mutate()
                  }}
                >
                  <Input
                    value={pasted}
                    onChange={(e) => {
                      setPasted(e.target.value)
                      finish.reset()
                    }}
                    placeholder={`${REDIRECT}?code=…`}
                    className="font-mono text-xs"
                    aria-label="Address after logging in"
                    aria-invalid={finish.isError || undefined}
                    spellCheck={false}
                  />
                  <Button type="submit" disabled={!pasted.trim() || finish.isPending}>
                    {finish.isPending && <Spinner data-icon="inline-start" />}
                    Finish login
                  </Button>
                </form>
                {finish.isError && (
                  <Alert variant="destructive" className="mt-2">
                    <TriangleAlertIcon />
                    <AlertTitle>Login not finished</AlertTitle>
                    <AlertDescription>{errorMessage(finish.error)}</AlertDescription>
                  </Alert>
                )}
              </Step>
            </ol>
            <div className="flex flex-col items-center gap-2 self-start">
              <div className="size-40 overflow-hidden rounded-xl bg-white p-2 shadow-sm">
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
      </SettingsCard>
    </Section>
  )
}

function Step({ n, title, children }: { n: number; title: string; children: React.ReactNode }) {
  return (
    <li className="flex gap-3">
      <span className="flex size-6 shrink-0 items-center justify-center rounded-full border bg-muted text-xs font-medium tabular">
        {n}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-medium">{title}</span>
        {children}
      </div>
    </li>
  )
}

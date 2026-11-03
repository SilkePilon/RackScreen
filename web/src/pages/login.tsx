import * as React from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowRightIcon, LockKeyholeIcon } from "lucide-react"

import { RackGlyph } from "@/components/app-sidebar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { api, errorMessage } from "@/lib/api"

export function LoginPage() {
  const qc = useQueryClient()
  const [password, setPassword] = React.useState("")
  const [error, setError] = React.useState<string | null>(null)
  const [busy, setBusy] = React.useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await api.login(password)
      await qc.invalidateQueries()
    } catch (err) {
      setError(errorMessage(err) === "Wrong password" ? "That password is not right." : errorMessage(err))
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-svh items-center justify-center bg-canvas p-4">
      <div className="w-full max-w-sm animate-in duration-500 fade-in-0 zoom-in-95">
        <div className="mb-8 flex flex-col items-center gap-4 text-center">
          <div className="flex size-14 items-center justify-center rounded-2xl border bg-card shadow-lg">
            <RackGlyph className="size-7" />
          </div>
          <div className="flex flex-col gap-1">
            <h1 className="text-xl font-semibold tracking-tight">RackScreen</h1>
            <p className="text-sm text-muted-foreground">Enter the password to manage your panels.</p>
          </div>
        </div>
        <form onSubmit={submit} className="flex flex-col gap-3 rounded-xl border bg-card p-4">
          <label htmlFor="password" className="flex items-center gap-2 text-sm font-medium">
            <LockKeyholeIcon className="size-3.5 text-muted-foreground" />
            Password
          </label>
          <Input
            id="password"
            type="password"
            autoFocus
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            aria-invalid={!!error || undefined}
            aria-describedby={error ? "login-error" : undefined}
          />
          {error && (
            <p id="login-error" className="text-[13px] text-destructive">
              {error}
            </p>
          )}
          <Button type="submit" size="lg" disabled={busy || !password} className="mt-1">
            {busy ? <Spinner data-icon="inline-start" /> : null}
            Log in
            {!busy && <ArrowRightIcon data-icon="inline-end" />}
          </Button>
        </form>
        <p className="mt-6 text-center text-xs text-muted-foreground">
          Forgot it? Set <code className="font-mono">web.password: ""</code> in{" "}
          <code className="font-mono">/etc/rackscreen/config.yaml</code> and restart the service.
        </p>
      </div>
    </div>
  )
}

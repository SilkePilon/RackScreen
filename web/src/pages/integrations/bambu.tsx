import * as React from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { CircleCheckIcon, LogOutIcon, PrinterIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { Segmented } from "@/components/segmented"
import {
  Row,
  SecretRow,
  Section,
  SelectRow,
  SettingsCard,
  SwitchRow,
  TextRow,
  useEnabled,
} from "@/components/settings"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, errorMessage } from "@/lib/api"
import { useDraft, useField } from "@/lib/draft"
import { keys, useBambu, useBambuPrinters } from "@/lib/queries"
import type { BambuMode, BambuRegion } from "@/lib/types"

const LAN_HELP =
  "On the printer: Settings → WLAN → LAN only mode on, then Developer Mode on. The serial and access code are on the same screens."
const CLOUD_HELP =
  "Log in with your Bambu Lab account. The printer stays on Bambu's cloud (LAN only mode off); the password goes to Bambu Lab and is not kept."

export function BambuSection() {
  const on = useEnabled("bambu.enabled")
  const mode = useField<BambuMode>("bambu.mode")
  const cloud = mode.value === "cloud"
  return (
    <Section title="Bambu Lab" description={cloud ? CLOUD_HELP : LAN_HELP}>
      <SettingsCard>
        <SwitchRow id="bambu.enabled" />
        <Row id="bambu.mode" dirty={mode.dirty} onReset={mode.reset} muted={!on}>
          <Segmented<BambuMode>
            label="Connection"
            value={mode.value ?? "lan"}
            onChange={mode.set}
            options={[
              { value: "lan", label: "LAN" },
              { value: "cloud", label: "Bambu Cloud" },
            ]}
          />
        </Row>
        {cloud ? <Cloud on={on} /> : <Lan on={on} />}
      </SettingsCard>
    </Section>
  )
}

function Lan({ on }: { on: boolean }) {
  const d = useDraft()
  const host = ((d.get("bambu.host") as string) ?? "").trim()
  return (
    <>
      <TextRow
        id="bambu.host"
        mono
        muted={!on}
        placeholder="192.168.1.50"
        validate={() =>
          /:\/\/|\/|\s/.test(host)
            ? "Only the address, without http://."
            : on && !host
              ? "Needed while the printer is on."
              : null
        }
      />
      <TextRow id="bambu.serial" mono muted={!on} placeholder="01P00A000000000" />
      <SecretRow id="bambu.access_code" muted={!on} />
    </>
  )
}

function daysLeft(expires: number | null) {
  if (!expires) return null
  return Math.max(0, Math.round((expires - Date.now() / 1000) / 86400))
}

function Cloud({ on }: { on: boolean }) {
  const qc = useQueryClient()
  const bambu = useBambu()
  const region = useField<BambuRegion>("bambu.region")
  const s = bambu.data
  const refresh = () => void qc.invalidateQueries({ queryKey: keys.bambu })

  const logout = useMutation({
    mutationFn: api.bambuLogout,
    onSuccess: () => {
      toast("Logged out of Bambu Lab")
      qc.removeQueries({ queryKey: keys.bambuPrinters })
      refresh()
    },
    onError: (e) => toast.error(errorMessage(e)),
  })

  const days = daysLeft(s?.expires_at ?? null)
  return (
    <>
      <SelectRow
        id="bambu.region"
        muted={!on}
        options={[
          { value: "global", label: "Global" },
          { value: "china", label: "China" },
        ]}
      />
      <Row
        muted={!on}
        title={
          s ? (
            <span className="flex items-center gap-2">
              {s.logged_in ? (
                <CircleCheckIcon className="size-4 text-success" />
              ) : (
                <PrinterIcon className="size-4 text-muted-foreground" />
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
            : s.logged_in
              ? `The login lasts about three months${days !== null ? `; ${days} days left` : ""}. Log in again here when it runs out.`
              : "Accounts made with Google or Apple sign-in need a password first: set one at bambulab.com."
        }
      >
        {s?.logged_in && (
          <Button variant="outline" onClick={() => logout.mutate()} disabled={logout.isPending}>
            <LogOutIcon data-icon="inline-start" />
            Log out
          </Button>
        )}
      </Row>
      {s &&
        !s.logged_in &&
        (s.pending ? (
          <CodeForm kind={s.pending} onDone={refresh} />
        ) : (
          <LoginForm region={region.value ?? "global"} onDone={refresh} />
        ))}
      {s?.logged_in ? (
        <PrinterPicker on={on} />
      ) : (
        <TextRow id="bambu.serial" mono muted={!on} placeholder="01P00A000000000" />
      )}
    </>
  )
}

function LoginForm({ region, onDone }: { region: BambuRegion; onDone: () => void }) {
  const [account, setAccount] = React.useState("")
  const [password, setPassword] = React.useState("")
  const login = useMutation({
    mutationFn: () => api.bambuLogin(account.trim(), password, region),
    onSuccess: (r) => {
      setPassword("")
      if ("needs" in r)
        toast(
          r.needs === "tfa" ? "Enter the code from your authenticator app" : "Bambu Lab emailed you a code"
        )
      else toast.success(r.status)
      onDone()
    },
  })
  return (
    <form
      className="flex flex-col gap-3 px-5 py-4"
      onSubmit={(e) => {
        e.preventDefault()
        login.mutate()
      }}
    >
      <div className="flex flex-col gap-2 sm:flex-row">
        <Input
          type="email"
          autoComplete="username"
          placeholder="Email"
          aria-label="Bambu Lab email"
          value={account}
          onChange={(e) => {
            setAccount(e.target.value)
            login.reset()
          }}
        />
        <Input
          type="password"
          autoComplete="current-password"
          placeholder="Password"
          aria-label="Bambu Lab password"
          value={password}
          onChange={(e) => {
            setPassword(e.target.value)
            login.reset()
          }}
        />
        <Button type="submit" disabled={!account.trim() || !password || login.isPending}>
          {login.isPending && <Spinner data-icon="inline-start" />}
          Log in
        </Button>
      </div>
      {login.isError && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>Not logged in</AlertTitle>
          <AlertDescription>{errorMessage(login.error)}</AlertDescription>
        </Alert>
      )}
    </form>
  )
}

function CodeForm({ kind, onDone }: { kind: "code" | "tfa"; onDone: () => void }) {
  const qc = useQueryClient()
  const [code, setCode] = React.useState("")
  const submit = useMutation({
    mutationFn: () => api.bambuCode(code.trim()),
    onSuccess: (r) => {
      toast.success(r.status)
      setCode("")
      void qc.invalidateQueries({ queryKey: keys.bambuPrinters })
      onDone()
    },
    // an expired or cleared login drops back to the password form
    onError: () => onDone(),
  })
  const resend = useMutation({
    mutationFn: api.bambuResend,
    onSuccess: () => toast("A new code is on its way"),
    onError: (e) => toast.error(errorMessage(e)),
  })
  const logout = useMutation({
    mutationFn: api.bambuLogout,
    onSuccess: onDone,
    onError: (e) => toast.error(errorMessage(e)),
  })
  return (
    <form
      className="flex animate-in flex-col gap-3 px-5 py-4 fade-in-0 slide-in-from-top-1"
      onSubmit={(e) => {
        e.preventDefault()
        submit.mutate()
      }}
    >
      <p className="text-[13px] text-muted-foreground">
        {kind === "tfa"
          ? "Your account has two-factor login on: enter the code from your authenticator app."
          : "Bambu Lab emailed you a code (check the spam folder too). It is good for a few minutes."}
      </p>
      <div className="flex flex-col gap-2 sm:flex-row">
        <Input
          inputMode="numeric"
          autoComplete="one-time-code"
          placeholder="123456"
          aria-label="Login code"
          className="font-mono sm:w-40"
          value={code}
          onChange={(e) => {
            setCode(e.target.value)
            submit.reset()
          }}
        />
        <Button type="submit" disabled={!code.trim() || submit.isPending}>
          {submit.isPending && <Spinner data-icon="inline-start" />}
          Finish login
        </Button>
        {kind === "code" && (
          <Button type="button" variant="ghost" onClick={() => resend.mutate()} disabled={resend.isPending}>
            Send a new code
          </Button>
        )}
        <Button type="button" variant="ghost" onClick={() => logout.mutate()}>
          Start over
        </Button>
      </div>
      {submit.isError && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertTitle>Login not finished</AlertTitle>
          <AlertDescription>{errorMessage(submit.error)}</AlertDescription>
        </Alert>
      )}
    </form>
  )
}

function PrinterPicker({ on }: { on: boolean }) {
  const serial = useField<string>("bambu.serial")
  const printers = useBambuPrinters(true)
  const list = printers.data?.printers ?? []
  const value = (serial.value ?? "").trim()
  // pick the only printer on the account without asking
  React.useEffect(() => {
    if (list.length === 1 && !value) serial.set(list[0].serial)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list.length, value])
  const known = list.some((p) => p.serial === value)
  return (
    <Row
      id="bambu.serial"
      title="Printer"
      description={
        printers.isError
          ? errorMessage(printers.error)
          : printers.isSuccess && !list.length
            ? "No printers on this account. Bind the printer in Bambu Handy or Bambu Studio first."
            : "A printer bound to your account."
      }
      dirty={serial.dirty}
      onReset={serial.reset}
      muted={!on}
    >
      {printers.isPending ? (
        <Skeleton className="h-9 w-56" />
      ) : (
        <Select value={known ? value : undefined} onValueChange={serial.set}>
          <SelectTrigger className="w-56" aria-label="Printer">
            <SelectValue placeholder={value ? `${value} (not on this account)` : "Choose a printer"} />
          </SelectTrigger>
          <SelectContent align="end">
            <SelectGroup>
              {list.map((p) => (
                <SelectItem key={p.serial} value={p.serial}>
                  {p.name}
                  <span className="text-muted-foreground">
                    {p.model ? ` · ${p.model}` : ""}
                    {p.online ? "" : " · offline"}
                  </span>
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      )}
      <Button
        variant="ghost"
        size="icon"
        aria-label="Reload the printer list"
        onClick={() => void printers.refetch()}
        disabled={printers.isFetching}
      >
        <RefreshCwIcon className={printers.isFetching ? "animate-spin" : undefined} />
      </Button>
    </Row>
  )
}

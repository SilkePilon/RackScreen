import { toast } from "sonner"

import { api, ApiError } from "@/lib/api"
import { keys, queryClient } from "@/lib/queries"
import type { Config, Info, SaveResponse } from "@/lib/types"

// What to tell the user after a save, and for a whole-process restart, wait
// for the service to come back and reload (at the new address if it moved).

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

/** The URL the web UI answers on after `web` changed, or null if it is off. */
export function webUrl(c: Config): string | null {
  if (!c.web.enabled) return null
  const host = location.hostname.includes(":") ? `[${location.hostname}]` : location.hostname
  return `${location.protocol}//${host}:${c.web.port}`
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const TOAST_ID = "process-restart"
let waiting = false

/** True once `url` answers at all; cross-origin answers are opaque but still count. */
async function reachable(url: string, sameOrigin: boolean): Promise<{ ok: boolean; started?: number }> {
  if (sameOrigin) {
    try {
      const i = await api.info()
      return { ok: true, started: i.started_unix }
    } catch (e) {
      // Any real answer (a 401 after a password change, say) means it is up.
      return { ok: e instanceof ApiError && !e.unreachable }
    }
  }
  try {
    await fetch(`${url}/api/info`, { mode: "no-cors", cache: "no-store" })
    return { ok: true }
  } catch {
    return { ok: false }
  }
}

/**
 * Poll until the restarted service answers, then load the page from it. The
 * service counts as back once it was seen down, or its start time changed.
 */
async function waitForRestart(target: string, startedBefore: number | undefined) {
  if (waiting) return
  waiting = true
  const sameOrigin = target === location.origin
  let sawDown = false
  const until = Date.now() + 90_000
  await sleep(1000)
  while (Date.now() < until) {
    const r = await reachable(target, sameOrigin)
    if (!r.ok) sawDown = true
    else if (sawDown || (r.started !== undefined && startedBefore !== undefined && r.started !== startedBefore)) {
      toast.loading("RackScreen is back; reloading…", { id: TOAST_ID })
      if (sameOrigin) location.reload()
      else location.href = `${target}${location.pathname}${location.search}${location.hash}`
      return
    }
    await sleep(1000)
  }
  waiting = false
  toast.error("RackScreen did not come back yet", {
    id: TOAST_ID,
    duration: Infinity,
    description: sameOrigin
      ? "Reload this page once it is running again."
      : `Try ${target} once it is running again.`,
  })
}

/**
 * Toast the outcome of a save. `before` is the config the save replaced; with
 * it a process restart can tell whether the address changes.
 */
export function announceSave(r: SaveResponse, before: Config | null, title?: string) {
  if (r.restart !== "process") {
    if (title) toast.success(title, { description: restartMessage(r.restart) })
    else toast.success(restartMessage(r.restart))
    return
  }
  const startedBefore = queryClient.getQueryData<Info>(keys.info)?.started_unix
  const w = before?.web
  const moved =
    !!w && (w.port !== r.config.web.port || w.bind !== r.config.web.bind || w.enabled !== r.config.web.enabled)
  const url = webUrl(r.config)
  if (!url) {
    toast.warning(title ?? "Saved. RackScreen restarts with the web UI turned off.", {
      id: TOAST_ID,
      duration: Infinity,
      description: title
        ? "RackScreen restarts with the web UI turned off; this page stops working."
        : "This page stops working. Turn it on again in the config file on the Pi.",
    })
    return
  }
  const target = w && w.port !== r.config.web.port ? url : location.origin
  toast.loading(title ?? "Saved. RackScreen restarts…", {
    id: TOAST_ID,
    duration: Infinity,
    description: moved
      ? `The address may change. Once it is back the page opens ${url}`
      : "This page reloads once it is back.",
    action: moved ? { label: "Open", onClick: () => (location.href = url) } : undefined,
  })
  void waitForRestart(target, startedBefore)
}

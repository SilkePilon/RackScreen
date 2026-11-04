// The Tailscale part of the mock API. `?ts=` picks where the flow starts:
//   missing  not installed, one-click setup allowed (then: install job, login, running)
//   nosudo   not installed, sudo not allowed (shows the command to run by hand)
//   setup    installed, but the service is not Tailscale's operator yet
//   login    needs login
//   approve  waiting for an admin to approve the device
//   running  logged in and connected (the default)
// Add `&nohttps` to have "Share over HTTPS" ask to turn on HTTPS first.

import type { TailscaleDevice, TailscaleSetupJob, TailscaleState } from "@/lib/types"

const params = new URLSearchParams(location.search)
const start = params.get("ts") ?? "running"

const hoursAgo = (h: number) => new Date(Date.now() - h * 3600_000).toISOString()

const PEERS: TailscaleDevice[] = [
  {
    hostname: "Silke's MacBook Pro",
    dns_name: "silkes-macbook-pro.tail1234.ts.net",
    ips: ["100.109.7.90", "fd7a:115c:a1e0::9a3a:75a"],
    os: "macOS",
    online: true,
    last_seen: null,
    exit_node: false,
  },
  {
    hostname: "homelab-k8s-operator",
    dns_name: "homelab-k8s-operator.tail1234.ts.net",
    ips: ["100.104.89.72"],
    os: "linux",
    online: true,
    last_seen: null,
    exit_node: false,
  },
  {
    hostname: "home-router",
    dns_name: "home-router.tail1234.ts.net",
    ips: ["100.64.0.9"],
    os: "linux",
    online: true,
    last_seen: null,
    exit_node: true,
  },
  {
    hostname: "Pixel 9",
    dns_name: "pixel-9.tail1234.ts.net",
    ips: ["100.89.250.1"],
    os: "android",
    online: false,
    last_seen: hoursAgo(0.4),
    exit_node: false,
  },
  {
    hostname: "iPad",
    dns_name: "ipad.tail1234.ts.net",
    ips: ["100.71.3.18"],
    os: "iOS",
    online: false,
    last_seen: hoursAgo(26),
    exit_node: false,
  },
  {
    hostname: "DESKTOP-GAMING",
    dns_name: "desktop-gaming.tail1234.ts.net",
    ips: ["100.90.12.4"],
    os: "windows",
    online: false,
    last_seen: hoursAgo(24 * 9),
    exit_node: false,
  },
]

const ts = {
  installed: start !== "missing" && start !== "nosudo",
  operator: !["missing", "nosudo", "setup"].includes(start),
  canSetup: start !== "nosudo",
  backend: (start === "running" ? "Running" : start === "approve" ? "NeedsMachineAuth" : "NeedsLogin") as
    "Running" | "Stopped" | "NeedsLogin" | "NeedsMachineAuth",
  name: "rackscreen",
  ssh: false,
  serve: false,
  httpsAllowed: !params.has("nohttps"),
  authUrl: null as string | null,
  job: { state: "idle", log: [] } as TailscaleSetupJob,
}

const loggedIn = () => ts.backend === "Running" || ts.backend === "Stopped"

function view(): TailscaleState {
  const dns = `${ts.name}.tail1234.ts.net`
  return {
    installed: ts.installed,
    version: ts.installed ? "1.90.6" : null,
    operator_ok: ts.installed && ts.operator,
    can_setup: ts.canSetup,
    backend_state: !ts.installed ? null : !ts.operator ? "Stopped" : ts.backend,
    auth_url: ts.backend === "NeedsLogin" ? ts.authUrl : null,
    self: loggedIn()
      ? {
          hostname: ts.name,
          dns_name: dns,
          ips: ["100.101.102.103", "fd7a:115c:a1e0::1a01:6667"],
          os: "linux",
          online: ts.backend === "Running",
          last_seen: null,
          exit_node: false,
        }
      : null,
    tailnet: loggedIn() ? "silke@example.com" : null,
    user: loggedIn() ? { login: "silke@example.com", display_name: "Silke Pilon" } : null,
    peers: loggedIn() ? PEERS : [],
    ssh: ts.ssh,
    serve: { on: ts.serve, url: ts.serve ? `https://${dns}` : null },
    health: [],
    hostname: "raspberrypi",
    error: null,
  }
}

let loginTimer: ReturnType<typeof setTimeout> | undefined

/** Answers `/tailscale*`, or null when the path is not one of ours. */
export function tailscaleRoute(
  method: string,
  p: string,
  b: Record<string, unknown>
): [number, unknown] | null {
  if (!p.startsWith("/tailscale")) return null
  const needOperator = () =>
    !ts.installed
      ? "Tailscale is not installed on the Pi"
      : !ts.operator
        ? "RackScreen may not manage Tailscale yet: finish the setup first"
        : null
  switch (`${method} ${p}`) {
    case "GET /tailscale":
      return [200, view()]
    case "GET /tailscale/setup":
      return [200, ts.job]
    case "POST /tailscale/setup": {
      if (ts.job.state === "running") return [400, { error: "the Tailscale setup is already running" }]
      const lines = [
        "$ sudo -n /usr/local/bin/rackscreen tailscale-setup",
        ...(ts.installed
          ? ["Tailscale 1.90.6 is installed"]
          : [
              "Installing Tailscale with the official installer (this takes a minute or two)",
              "Installing Tailscale for debian bookworm, using method apt",
              "+ curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.noarmor.gpg",
              "+ apt-get update",
              "+ apt-get install -y tailscale tailscale-archive-keyring",
              "Setting up tailscale (1.90.6) ...",
              "Installation complete! Log in to start using Tailscale by running:",
              "Installed Tailscale 1.90.6",
            ]),
        "Starting tailscaled",
        "silke may now manage Tailscale without sudo; finish in the web UI",
      ]
      ts.job = { state: "running", log: [lines[0]] }
      lines.slice(1).forEach((l, i) =>
        setTimeout(
          () => {
            ts.job.log.push(l)
            if (i === lines.length - 2) {
              ts.job.state = "done"
              ts.installed = true
              ts.operator = true
              ts.backend = "NeedsLogin"
            }
          },
          650 * (i + 1)
        )
      )
      return [200, { ok: true }]
    }
    case "POST /tailscale/login": {
      const err = needOperator()
      if (err) return [400, { error: err }]
      const host = typeof b.hostname === "string" ? b.hostname.trim() : ""
      if (host && !/^[a-z0-9-]{1,63}$/.test(host))
        return [400, { error: "device name: 1 to 63 lowercase letters, digits and dashes" }]
      if (host) ts.name = host
      if (typeof b.ssh === "boolean") ts.ssh = b.ssh
      clearTimeout(loginTimer)
      const key = typeof b.auth_key === "string" ? b.auth_key.trim() : ""
      if (key) {
        if (!/^tskey-[A-Za-z0-9_-]+$/.test(key))
          return [400, { error: "an auth key starts with tskey- and has only letters, digits, - and _" }]
        if (key.includes("bad"))
          return [400, { error: "Tailscale did not accept that auth key (expired, used up or mistyped?)" }]
        ts.backend = "Running"
        return [200, { auth_url: null, running: true }]
      }
      ts.authUrl = `https://login.tailscale.com/a/${Math.random().toString(16).slice(2, 14)}`
      loginTimer = setTimeout(() => {
        ts.backend = "Running"
        ts.authUrl = null
      }, 6000)
      return [200, { auth_url: ts.authUrl, running: false }]
    }
    case "POST /tailscale/up":
      ts.backend = "Running"
      return [200, { ok: true }]
    case "POST /tailscale/down":
      ts.backend = "Stopped"
      return [200, { ok: true }]
    case "POST /tailscale/logout":
      ts.backend = "NeedsLogin"
      ts.serve = false
      ts.authUrl = null
      return [200, { ok: true }]
    case "POST /tailscale/ssh":
      ts.ssh = !!b.on
      return [200, { ok: true, ssh: ts.ssh }]
    case "POST /tailscale/serve":
      if (b.on && !ts.httpsAllowed) {
        ts.httpsAllowed = true // allowed "in the admin console" for the next try
        return [
          200,
          {
            on: false,
            url: null,
            enable_url: "https://login.tailscale.com/f/serve?node=nAbCdEf11CNTRL",
            error: "Turn on HTTPS for your tailnet first, then try again",
          },
        ]
      }
      ts.serve = !!b.on
      return [200, { on: ts.serve, url: ts.serve ? `https://${ts.name}.tail1234.ts.net` : null }]
  }
  return null
}

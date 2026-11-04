# Web UI replaces the setup TUI

Date: 2026-11-03. Status: approved by Silke up front ("I approve anything, don't ask").

## Goal

Drop the ratatui setup TUI. The running service (`rackscreen run`, systemd unit
`rackscreen@<user>`) serves a fully interactive website from the same Pi. Vite +
React + TypeScript + Tailwind v4 + shadcn/ui, restyled to the dark "T3 Code
settings" look (near-black canvas, slightly lighter sidebar, bordered rounded
cards of setting rows with title + description left and the control right, blue
accent switches, muted group labels, breadcrumb header with actions on the
right, `/` search). Skeleton loading, toasts, smooth but restrained motion.

## Architecture

```
rackscreen run ──┬── Supervisor (main thread)
                 │     loop { Monitor::start(cfg) ; wait for Restart/Quit ; drop monitor ; reload cfg }
                 ├── Web server (own tokio runtime + thread, lives across monitor restarts)
                 │     axum, binds web.bind:web.port (default 0.0.0.0:8740)
                 │     serves embedded UI (crates/web, include_dir of web/dist via build.rs)
                 └── Hub (Arc, shared): config path, LogSink, live frames, status snapshot,
                       control channel into the current monitor, restart request
```

* New crate `crates/web` (`rackscreen-web`): axum 0.8 server, API handlers,
  static file serving with SPA fallback, precompressed `.br`/`.gz` siblings
  when the client accepts them, `Cache-Control: immutable` for `/assets/*`.
* `build.rs` copies `web/dist` into `$OUT_DIR/ui` when `web/dist/index.html`
  exists, else writes a placeholder page ("UI not built: run `npm --prefix web
  ci && npm --prefix web run build`"). `include_dir!("$OUT_DIR/ui")`.
* Hot restart: saving config validates, writes atomically (`Config::save`),
  then asks the supervisor to restart the monitor in-process (~1 s, panels
  go dark briefly). The web server keeps running. A change to the `web:` block
  needs a process restart: the process exits 0 and systemd (`Restart=always`)
  starts it again; the UI waits and reconnects.
* Logging: `run` tees tracing output into an in-memory `LogSink` (2000
  lines, each with a sequence number and level) besides stdout/journal.
* Live preview: each screen's render worker copies its freshly rendered,
  unrotated 240×240 frame into the hub only while at least one live client is
  connected (atomic counter), throttled to ≤ 10 fps per screen; the web side
  PNG-encodes (fast compression) and pushes it.
* Control: the hub holds a `Sender<Event>` into the current render loop (the
  same channel sources use) plus a small control channel for things that are not
  model events (identify, test pattern, jump to role). `Event::ForceNight`
  already exists for the night override.
* Status snapshot: the render loop publishes, once a second, a JSON-able
  snapshot: per screen current role and cycle progress, link state, frame
  stats, and a list of summary tiles taken from the model.

## Config

New block (defaults shown):

```yaml
web:
  enabled: true
  bind: 0.0.0.0
  port: 8740
  password: ""   # empty = no login
```

Secret fields (never sent to the browser in `/api/config`): `qbittorrent.pass`,
`electricity.token`, `github.token`, `media.radarr.api_key`,
`media.sonarr.api_key`, `media.lidarr.api_key`, `jellyfin.api_key`,
`web.password`.

## CLI after the change

* `rackscreen run [--config] [--sim] [--sim-grid] [--source] [--fps] [--seed] [--no-web] [--web-port N]`
* `rackscreen install [--yes] [--no-boot]`: non-interactive, idempotent:
  binary, SPI boot lines (asks y/N on a TTY unless `--yes`, prints "reboot
  needed"), default config, `/etc/rackscreen` owned by the service user (so the
  service can save), sudoers drop-in `/etc/sudoers.d/rackscreen` allowing only
  `<user> ALL=(root) NOPASSWD: /usr/local/bin/rackscreen update` and
  `<user> ALL=(root) NOPASSWD: /usr/local/bin/rackscreen tailscale-setup`
  (exact commands, no wildcards; written to a temp file, checked with
  `visudo -cf`, then moved, mode 0440), unit, enable + start, then prints
  `Open http://<ip>:8740`.
* `rackscreen uninstall [--yes] [--purge]`: removes unit, binary, sudoers drop-in;
  `--purge` also the config.
* `rackscreen update [--check]`: as today.
* `rackscreen play <show|random>`: as today.
* `rackscreen tailscale-setup` (root; re-runs itself through sudo): installs
  Tailscale with the official installer when `tailscale` is missing
  (`curl -fsSL --proto '=https' --proto-redir '=https' --tlsv1.2 -o <dir>/install.sh
  https://tailscale.com/install.sh` into a fresh root-owned 0700 directory, then
  `sh <dir>/install.sh` once the download finished; the directory is removed
  afterwards), `systemctl enable --now tailscaled`, then
  `tailscale set --operator=<service user>` (retried while tailscaled starts).
  Idempotent. The service user is the one of the `rackscreen@<user>.service`
  systemd lists (`systemctl list-units 'rackscreen@*'`), else `SUDO_USER`.
* `setup` and `calibrate` subcommands are removed; running with no subcommand
  prints help plus the web address hint. The whole `crates/setup` TUI layer
  (screens, widgets, anim, theme, ratatui) is deleted; the non-interactive
  ops (install, uninstall, update, boot, systemd, paths, shell, config_file)
  move into `crates/setup` without ratatui (or a renamed `crates/ops`).
* `install.sh`: download, `sudo ./rackscreen install`, print the URL.

## HTTP API (the contract between backend and frontend)

All JSON. Prefix `/api`. Errors: HTTP 4xx/5xx with `{"error": "message"}`.
Every non-GET request must carry header `X-RackScreen: 1` (CSRF guard; the
server answers 403 without it). When `web.password` is set, every `/api` route
except `GET /api/auth` and `POST /api/auth/login` answers 401 without a valid
session cookie `rs_session` (HttpOnly, SameSite=Strict, random 32 bytes,
in-memory, 30 days).

### Auth
* `GET /api/auth` → `{ "required": bool, "authenticated": bool }`
* `POST /api/auth/login` `{ "password": "..." }` → 200 `{ "ok": true }` + cookie, or 401.
* `POST /api/auth/logout` → `{ "ok": true }`.

### Info and status
* `GET /api/info` →
  ```json
  { "version": "0.13.0", "hostname": "raspberrypi", "sim": false,
    "started_unix": 1793700000, "config_path": "/etc/rackscreen/config.yaml",
    "config_writable": true,
    "fix_hint": null,                         // or "Run `sudo rackscreen install` once to let the web UI save the config"
    "addresses": ["http://192.168.1.172:8740"],
    "screens": 4 }
  ```
* `GET /api/status` (poll every 1–2 s) →
  ```json
  { "monitor": { "state": "running", "error": null, "since_unix": 1793700000 },
    "night": { "mode": "auto", "active": false },       // mode auto|on|off
    "screens": [ { "index": 0, "role": "cpu", "title": "CPU",
                   "roles": ["cpu","mem"], "cycle_secs": 15,
                   "cycle_progress": 0.4, "showing": "role" } ],   // showing: role|show|night|pattern|identify
    "links": [ { "name": "Kubernetes", "state": "ok", "detail": "7 nodes" } ],  // state ok|warn|down|off
    "stats": [ { "index": 0, "render_ms": 2.9, "render_p95_ms": 5.1, "push_ms": 21.0, "fps": 60 } ],
    "tiles": [ { "key": "cpu", "label": "Cluster CPU", "value": "23", "unit": "%", "tone": "ok" } ],  // tone ok|warn|bad|muted
    "host": { "cpu_temp_c": 51.2, "load1": 0.42, "mem_used_mb": 310, "mem_total_mb": 906 } }
  ```
  `tiles` is a free list the backend fills from the model (cluster cpu/mem,
  pods, nodes, alerts, weather, now playing, ...); the frontend renders it
  generically. `host` fields may be null off-Pi.
* `GET /api/roles` → `[ { "name": "cpu", "title": "CPU", "group": "Cluster", "description": "...", "needs": null } ]`
  every pickable role incl. `gif:<name>`; `needs` names a missing setting
  (e.g. `"GitHub token"`) or null.
* `GET /api/shows` → `[ { "id": "freight", "title": "Freight" } ]`.

### Config
* `GET /api/config` → `{ "config": <Config as JSON, secret fields null>, "secrets": { "github.token": true, ... } }`.
  `Config` JSON is exactly the serde form of `crates/app/src/config.rs`
  (snake_case keys) plus the `web` block.
* `PUT /api/config` `{ "config": {...} }` — a secret field that is `null`
  keeps the stored value, `""` clears it, any other string sets it. Server
  runs `normalize` + `validate`, saves, hot-restarts the monitor (or the
  process for `web:` changes). → `{ "config": ..., "secrets": ..., "restart": "monitor" | "process" | "none" }`
  or 400 `{ "error": "screen 2: unknown role 'foo'" }`.
* `GET /api/config/yaml` → `{ "yaml": "..." }` (secrets included; behind auth if set).
* `PUT /api/config/yaml` `{ "yaml": "..." }` → same as `PUT /api/config`.
* `POST /api/config/validate` `{ "config": {...} }` → `{ "ok": true }` or `{ "ok": false, "error": "..." }` (no save).

### Control
* `POST /api/control/identify` `{ "secs": 5 }` → every panel shows its number (calibrate test pattern) for `secs`.
* `POST /api/control/pattern` `{ "on": true }` → test pattern on all panels until turned off (or 10 min).
* `POST /api/control/role` `{ "screen": 0, "role": "weather" }` → that screen jumps to that role now (must be in its list).
* `POST /api/control/play` `{ "show": "freight" | "random" }`.
* `POST /api/control/transition-test` `{ "screen"?: 0 }` → that screen (or
  every cycling screen, left to right in show order) changes to its next role
  now, in the saved transition style. 409 with a plain-English reason during a
  show, at night, under the test pattern, or when nothing cycles.
* `POST /api/control/night` `{ "mode": "auto" | "on" | "off" }`. The override
  is kept by the hub (it survives monitor restarts, it does not survive a
  process restart) and `/api/status` reports it right away; the answer's
  `applied` says whether a running render loop took it already.
* `POST /api/control/restart` → hot-restart the monitor.

### Live frames
* `GET /api/live` WebSocket. Server → client binary messages: byte 0 = screen
  index, rest = PNG of the 240×240 unrotated frame (only when it changed).
  On connect the server sends the latest frame of every screen. Client → server
  text `{"fps": 1..15}` optional (default 10).

### Logs
* `GET /api/logs?after=<seq>&limit=500` → `{ "lines": [ { "seq": 12, "unix_ms": 1793700000123, "level": "INFO", "target": "rackscreen_sources::k8s", "msg": "..." } ], "next": 13 }`
* `GET /api/logs/stream` Server-Sent Events, event `log`, data = one line object as above.

### Spotify
* `GET /api/spotify` → `{ "enabled": bool, "client_id_set": bool, "logged_in": bool, "status": "Logged in as Silke" }`
* `POST /api/spotify/login` → `{ "url": "https://accounts.spotify.com/authorize?..." }` (keeps the PKCE verifier/state in memory, reused on repeat calls for 10 min).
* `POST /api/spotify/callback` `{ "pasted": "http://127.0.0.1:.../callback?code=...&state=..." }` → `{ "ok": true, "status": "Logged in as Silke" }` or 400.
* `POST /api/spotify/logout` → removes the token file.

### Update
* `GET /api/update` → `{ "current": "0.13.0", "latest": "v0.13.1" | null, "newer": bool, "can_self_update": bool, "job": { "state": "idle"|"running"|"done"|"failed", "log": ["..."] } }`
  (latest checked at most hourly; `can_self_update` = `sudo -n -l /usr/local/bin/rackscreen update` succeeds).
* `POST /api/update` → starts `sudo -n /usr/local/bin/rackscreen update` in the background; the service restarts at the end and the UI reconnects.
  The job (state, start version, target, log) is kept in
  `$XDG_STATE_HOME/rackscreen/update.json` (default `~/.local/state`) of the
  service user, so after the restart `GET /api/update` reports `done` when the
  new process runs a newer version than the job started from, else `failed`.

### Tailscale

Everything runs as the service user through the `tailscale` CLI (tokio
process, 10 s timeout, 30 s for `up`/`down`/`logout`/`serve`, never a shell,
fixed argument lists; the device name must match `^[a-z0-9-]{1,63}$`, an auth
key `^tskey-[A-Za-z0-9_-]+$`; the key is never logged). Commands go through a
`Runner` trait so the API tests use a scripted fake. CLI failures become
`{ "error": "..." }` with short messages (not installed, needs setup when the
CLI says access denied, tailscaled not running, offline, rejected key).

* Every `POST /api/tailscale/*` answers 403 `{ error: "Set a web password first
  (System → Web access): …" }` while `web.password` is empty (Tailscale can open
  the Pi to a tailnet and SSH); with a password the auth guard demands the
  session as everywhere else. `GET`s stay open; `password_set` tells the page.
  Error and log text never carries an auth key (`tskey-…` → `tskey-***`).
* `GET /api/tailscale` → `{ installed, version, operator_ok, can_setup, password_set,
  backend_state: "NoState"|"NeedsLogin"|"NeedsMachineAuth"|"Starting"|"Running"|"Stopped"|other|null,
  auth_url, self: { hostname, dns_name, ips, os, online, last_seen, exit_node } | null,
  tailnet, user: { login, display_name } | null, peers: [device...] (online first),
  ssh, serve: { on, url }, health: [..], hostname, error }`, from
  `tailscale version` (cached 5 min; `installed: false` only when the program
  is missing, other start errors land in `error` and `health`), `tailscale status --json`, `tailscale debug prefs`
  (`OperatorUser` = this user → `operator_ok`; `RunSSH` → `ssh`) and
  `tailscale serve status --json` (`on` when 443 proxies to this port).
  `can_setup` = `sudo -n -l /usr/local/bin/rackscreen tailscale-setup`
  succeeds (cached 60 s); when it does not, the UI shows the command
  `sudo rackscreen tailscale-setup` to run once and mentions
  `sudo rackscreen install` to allow the one-click setup.
* `POST /api/tailscale/setup` starts `sudo -n /usr/local/bin/rackscreen tailscale-setup`
  as a background job; `GET /api/tailscale/setup` → `{ state, log }` (in memory).
* `POST /api/tailscale/login` `{ hostname?, auth_key?, ssh? }`: first
  `tailscale set --hostname=… --ssh=…` for what was given. With a key:
  `tailscale up --auth-key=file:<path> --hostname=… --ssh=…` (current values
  filled in, since `up` with flags must mention every non-default one; the key
  sits in a 0600 file in a fresh 0700 temp dir, removed when `up` returns;
  Tailscale older than 1.32 gets it on argv with a warning) → `{ running: true }`.
  Without: a bare `tailscale up` kept running in the background (a new login,
  a logout or 5 minutes kill it); the auth URL comes from its output or the status's
  `AuthURL` within ~10 s → `{ auth_url }`. The UI then polls every 2 s until Running.
  Logins run one at a time; the waiting `up` carries a generation so only its
  own handler or timer kills it. SSH defaults off in the form.
* `POST /api/tailscale/up`, `/down`, `/logout`.
* `POST /api/tailscale/ssh` `{ on }` → `tailscale set --ssh=true`, or
  `--ssh=false --accept-risk=lose-ssh`. The UI asks before turning it on: the
  tailnet ACLs decide who may SSH in, possibly as root.
* `POST /api/tailscale/serve` `{ on }` reads `tailscale serve status --json`
  first and only touches https:443 when it is free or holds nothing but our
  proxy: `tailscale serve --bg --https=443 http://127.0.0.1:<port>` (no `--yes`)
  / `tailscale serve --https=443 off` → `{ on, url }`; anything else on 443 is
  a 409 saying what is there. When HTTPS is not
  enabled on the tailnet the CLI prints a consent link, returned as
  `{ on: false, enable_url }`. Refused when `web.bind` is a LAN address
  (serve only proxies to 127.0.0.1). Serve keeps the `Host` header, so the
  same-origin check passes, and https makes geolocation ("use my location") work.

## Frontend

`web/` at repo root: Vite, React 19, TypeScript strict, Tailwind v4, shadcn/ui
(components restyled via CSS variables and small class tweaks), TanStack Query,
react-router, sonner, lucide-react, dnd-kit (role order), qrcode (Spotify
login QR), CodeMirror 6 lazy-loaded for the YAML editor, self-hosted Inter
variable font (no CDN; the Pi may be offline). Build output `web/dist`,
precompressed `.br` and `.gz`.

Pages (left sidebar like the reference, grouped):

* **Overview**: four live round panel mirrors (WebSocket canvases, true circles,
  in physical order), each with current role, cycle progress ring/bar and a role
  quick-switch menu; quick actions (identify, play show, night auto/on/off,
  restart); summary tiles; link health list; update banner.
* **Screens**: per screen card: live mirror, sortable role list (drag), add-role
  command picker grouped by category with "needs" hints, cycle seconds, presets.
* **Calibrate**: per screen rotate segmented control 0/90/180/270, mirror
  switch, test pattern toggle, identify, advanced (SPI bus, CS, DC, RST, Hz).
* **Integrations** group: Cluster (kubeconfig, Prometheus, Argo CD,
  qBittorrent), Energy, Sky (location with "use my location" + weather, rain,
  ISS), Media & Jellyfin (arr keys), Spotify (client ID, login flow with link +
  QR + paste field, status), GitHub (token, CI repos), GIFs (list, add, rename,
  delete, link validation, preview via the link itself).
* **Connections** group: **Tailscale**. Not installed: explanation and
  "Install Tailscale" (setup job with a collapsible live log) or the command to
  run by hand; installed but not operator: "Finish setup"; NeedsLogin: device
  name, "Allow Tailscale SSH", "Log in with browser" (new tab + QR code,
  pulsing wait, success toast) or an auth key; NeedsMachineAuth: link to the
  admin console; Running: device card (name, connected switch, MagicDNS and
  IPs with copy, tailnet, account, version), HTTPS sharing switch with the
  URL, Tailscale SSH switch, devices list (OS icon, online dot, IP, last
  seen), log out with confirmation. Polls every 5 s on this page only (2 s
  while waiting for a login). The Overview shows a "Tailscale · <name>" chip
  on "This Pi"; System lists `tailscale0` and the MagicDNS name. Mock:
  `?ts=missing|nosudo|setup|login|approve|running` (`&nohttps`).
* **Display**: brightness, fps, SPI chunk, one at a time, transitions (below), night window, thresholds.
* **Shows**: card grid, enable, every N min, gap, panel order, play now.
* **Logs**: live stream, level filter, search, pause, auto-scroll, copy.
* **System**: version + update (one click when allowed, else copyable command),
  web access (password), raw YAML editor with validation, config
  download/upload (backup/restore), restart.

Cross-cutting: `⌘K` / `/` command palette for pages and individual settings;
unsaved-changes bar (Discard / Save & apply) at the bottom; per-row "reset to
saved" icon on changed fields (like the reference's ↺); skeletons while loading;
reconnect banner when the API is down (service restarting); toasts for results;
mobile layout (sidebar becomes a sheet); `prefers-reduced-motion` respected;
dark theme by default with a light theme toggle.

## Transitions (added 2026-11-04)

Config block `transitions: { style, direction, secs, random, shows? }`
(`crates/app/src/config.rs`, validated against `rackscreen_core::transition`).
`direction` may be empty (the style's default) and is ignored by styles
without one. `shows` is the shows' own `{ style, direction, secs }` or absent.
An absent block is today's look: `fade-black`, 0.4 s.

Core (`crates/core/src/transition.rs`): `Style` × `Direction` → `Kind`,
`Spec { kind, secs }`, `Transitions { screens, shows, random }`, a random
pool (every kind but the cut), and `composite(kind, from, to, out, t)` over
two 240×240 RGBA frames with ease-in-out-cubic and a 2 px soft edge where
there is an edge. `fade-black` and `cut` are single-scene styles (one scene at
a brightness, the old pipeline); the rest render both scenes. The model hands
the run loop a `ScreenFrame { scene, blend: Option<Blend { to, kind, t }> }`;
the worker renders both into reusable buffers and composites. Around a show a
run is `In → Playing → Out` with one transition each; a staggering style
delays panel `pos` by `pos × 0.25 × secs` (reversed for a leftward one) so
the style sweeps along the rack; the show's clock starts half-way through the
last panel's transition in, which keeps the old 0.2 s timing for the default.

Web: a Transitions section on the Display page (`web/src/components/transitions.tsx`),
with `web/src/lib/transitions.ts` porting the compositor so each style card
plays a real preview between two sample screens at 72 px (one shared
`requestAnimationFrame` clock; a static mid-point frame under
`prefers-reduced-motion`). Direction control only for styles that have one;
duration slider 0.15–2 s; random switch; *Shows use* select with its own
direction and length; *Try it* calls the control above once the draft is
saved. Styles that change every pixel every frame carry a hint.

## Testing

Rust: unit tests for secret masking/merging, CSRF and auth middleware, config
round trip with the `web` block, log sink sequence numbers, PNG encoding, the
static file fallback. Frontend: `tsc --noEmit`, `eslint`, `vite build` must pass;
a mock API (`VITE_MOCK=1`) lets the UI run without the Pi.

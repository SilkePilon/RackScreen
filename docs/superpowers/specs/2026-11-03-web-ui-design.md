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
  `<user> ALL=(root) NOPASSWD: /usr/local/bin/rackscreen update` (written to a
  temp file, checked with `visudo -cf`, then moved, mode 0440), unit, enable +
  start, then prints `Open http://<ip>:8740`.
* `rackscreen uninstall [--yes] [--purge]`: removes unit, binary, sudoers drop-in;
  `--purge` also the config.
* `rackscreen update [--check]`: as today.
* `rackscreen play <show|random>`: as today.
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
* **Display**: brightness, fps, SPI chunk, one at a time, night window, thresholds.
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

## Testing

Rust: unit tests for secret masking/merging, CSRF and auth middleware, config
round trip with the `web` block, log sink sequence numbers, PNG encoding, the
static file fallback. Frontend: `tsc --noEmit`, `eslint`, `vite build` must pass;
a mock API (`VITE_MOCK=1`) lets the UI run without the Pi.

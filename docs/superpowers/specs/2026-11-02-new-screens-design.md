# New screens: media, traffic, restarts, uptime, fans, mains, load, CI, Jellyfin

## Purpose

Nine new roles in the P2 look defined in `2026-11-02-p2-redesign-design.md`
(spec A, built first). Seven read data Prometheus already collects in the
cluster; CI reads GitHub Actions; Jellyfin reads a Jellyfin server outside the
cluster. Layout, ring kinds, card kinds, motion and the marquee rule are spec
A's; this spec only says what each screen shows and where the data comes from.

## Decisions taken during brainstorming (2026-11-02)

- Picked from a list of candidates checked against the live Prometheus
  (2,085 metric names): media, traffic, restarts, uptime, fans, mains, load,
  GitHub CI. Dropped: DNS, disk I/O, clock, Home Assistant, countdown, and all
  AI-usage screens (Claude subscription limits are not readable with an API
  key; no Codex subscription).
- **fans** is its own screen (not the thermal card).
- **media** card = the active download (md3).
- **Jellyfin** = "jp-card" on plain black: no cover art, no image decoding.
- While checking the cluster: CoreDNS showed ~312,000 queries/s and 89
  container restarts in 24 h. Not part of this spec; reported to Silke.
- Mockups: `.superpowers/brainstorm/491126-1793618315/content/p2-rest-v2.html`
  (traffic, restarts, uptime, fans, mains, load, ci), `media-card-v2.html`
  (md3), `jellyfin-final.html`.

## Roles

`Role::ALL` grows by nine (config ids in brackets): Media (`media`), Traffic
(`traffic`), Restarts (`restarts`), Uptime (`uptime`), Fans (`fans`), Mains
(`mains`), Load (`load`), Ci (`ci`), Jellyfin (`jellyfin`). Prometheus-backed
roles are `is_cluster()` (connecting scene when the Kubernetes link is down).
The TUI Screens editor, `Role::parse`, the README role table and the config
comment listing roles follow.

## Screens

Columns as in spec A: curved title · hero · ring · pill · card.

| Role | Title | Hero | Ring | Pill | Card |
|---|---|---|---|---|---|
| media | MEDIA | library total (movies + series + artists) | Parts: movies `#F2A65A`, series cyan, artists violet | "1 downloading" (blue) or "library 121" | Download: title (marquee, SemiBold 10), 5 px progress bar (blue; `OK` when done), "35 % · 19 min left" or "added just now" (ok, 60 s); with nothing queued, the last added item and its age |
| traffic | WEB TRAFFIC | req/s (one decimal), unit `req/s` below | Parts: share of the top 3 services (indigo shades), rest `TRACK` | "<busiest service> · 0 % errors" (`WARN` when 5xx share ≥ 1 %, `CRIT` ≥ 5 %) | Spark, last 90 s |
| restarts | RESTARTS · 24H | container restarts in 24 h | Discrete, 24 segments = hours (oldest first): 0 `TRACK`, 1 `OK` 80 %, 2–5 `WARN`, ≥ 6 `CRIT` | "<worst pod> ×41" (crit when ≥ 6, else `TEXT2`); "No restarts" (ok) at 0 | Bars per hour, coloured as the ring, "per hour" |
| uptime | UPTIME | longest node uptime in days (`82` + superscript `d`) | Discrete, one per node: `OK`, `WARN` when that node is up < 1 day | "<node> rebooted 9 h ago" (warn) for the most recent reboot < 24 h, else "All up 3 d+" | NodeBars of days per node (0–max), recent reboots in `WARN` |
| fans | FANS | highest fan speed (`6.1k`), unit `rpm` below | Fill cyan, max ÷ 8000; `WARN` over 7000 | "<node> loudest" | NodeBars per node with a fan, loudest highlighted |
| mains | MAINS | input voltage (`234` + superscript `V`) | Bands over 200–260 V: crit, warn, ok, ok, warn, crit (10 V each); ends labelled 200 / 260 | "Stable · UPS load 7 %" (ok), "Low voltage"/"High voltage" (warn) outside 220–240 | Spark 24 h (hourly), `now_idx` |
| load | LOAD | highest 1-min load (two decimals) | Discrete, one per node, by load ÷ cores: `OK`, `WARN` ≥ 0.7, `CRIT` ≥ 0.9 | "<node> busiest" | Stats cluster mean load 1 min · 5 min · 15 min |
| ci | CI | passed / total of the last 12 runs (`11/12`, hero_scale 0.82) | Discrete, 12 segments, oldest first: success `OK`, failure `CRIT`, in progress blue, cancelled/skipped `TRACK` | "<repo> · building" (blue) / "· passed" (ok) / "· failed" (crit) for the newest run | Stats average run time · failed count |
| jellyfin | NOW PLAYING | time left (`1:41:54`, `h:mm:ss` or `m:ss`) | Fill, playback fraction; amber `#F2A65A` for a movie, cyan for an episode; 45 % alpha when paused | "Direct play" (ok), "Transcoding" (warn), "Paused" (warn) | Title (Bold 11, marquee); "S2 · E5 · Home" or "2017 · 2h 44m"; "user · device" plus "  ·  +1 more" when more sessions play |

Shared rules (from spec A): no data yet → hero `—`, "Waiting for data";
link down → connecting scene; long text → marquee.

Jellyfin specifics:

- Several playing sessions: show the one that started most recently and is
  not paused; if all are paused, the most recently active one.
- Position extrapolates between polls when not paused (`position + elapsed`).
- Nothing playing: hero `—` in `TEXT3`, pill "Nothing playing", card "last
  watched" and "<title> · 2h ago" from the last item seen this run; no card
  before any item was seen.
- Unreachable or 401: hero `—`, pill "Jellyfin offline" or "Check API key"
  (warn).

## Data sources

### Prometheus (existing client over the kube tunnel)

New instant queries each poll (existing poll interval):

| Role | Query |
|---|---|
| media | `radarr_movies_total`, `sonarr_series_total`, `lidarr_artists_total` |
| traffic | `sum(rate(traefik_entrypoint_requests_total[1m]))`; `topk(3, sum by (service)(rate(traefik_service_requests_total[5m])))`; 5xx share `sum(rate(traefik_service_requests_total{code=~"5.."}[5m])) / sum(rate(traefik_service_requests_total[5m]))` |
| restarts | `sum(increase(kube_pod_container_status_restarts_total[24h]))`; `topk(1, sum by (namespace, pod)(increase(kube_pod_container_status_restarts_total[24h])))` |
| uptime | `time() - node_boot_time_seconds` |
| fans | `node_hwmon_fan_rpm` |
| mains | `nut_input_voltage_volts`, `nut_load` |
| load | `node_load1`, `node_load5`, `node_load15`, `count by (instance)(node_cpu_seconds_total{mode="idle"})` |

Range queries (`query_range`), on start and then every 5 min for the hourly
ones, every poll appending for the short ones:

| Role | Query | Window / step |
|---|---|---|
| traffic | `sum(rate(traefik_entrypoint_requests_total[1m]))` | 90 s / 3 s |
| restarts | `sum(increase(kube_pod_container_status_restarts_total[1h]))` | 24 h / 1 h |
| mains | `avg(nut_input_voltage_volts)` | 24 h / 1 h |

Node names come from the existing `node_tag(instance)` mapping, so labels
match the thermal and health screens.

### Arr queue (media card)

- Config `media: { enabled, radarr, sonarr, lidarr }`, each `{ service,
  port, api_key }` (service in the `arr-stack` namespace, reached over the
  existing kube tunnel like Prometheus; `service: auto` picks the first
  service whose name contains the app name).
- Poll every 10 s: `GET /api/v3/queue?pageSize=5` (radarr, sonarr) and
  `/api/v1/queue?pageSize=5` (lidarr) with header `X-Api-Key`. The download
  shown is the one with the least time left; title is the movie title, or
  "Series · S2E5", or "Artist · Album"; progress = `1 − sizeleft / size`,
  ETA from `timeleft`.
- "added just now" fires when an item leaves the queue with status
  completed; the last added item is remembered for the idle card.
- An app without an API key is skipped. With no API keys at all the card falls
  back to the existing qBittorrent torrent (name, progress, ETA).

### GitHub Actions (ci)

- Config `github.ci_repos: [owner/repo, …]` (empty = the token owner's repos
  with Actions runs in the last 7 days, max 10), uses the existing token.
- Poll every 60 s: `GET /repos/{repo}/actions/runs?per_page=12` per repo,
  merged and sorted by `created_at`, last 12 kept. Average run time over
  completed runs (`updated_at − run_started_at`).
- Emits `CiRun { repo, ok, at }` so the face's happy rule (spec A) can use
  "last run passed < 30 min ago".

### Jellyfin

- Config `jellyfin: { enabled: false, url: "", api_key: "", poll_secs: 5 }`.
  `url` stays a placeholder until Silke fills it in (Jellyfin runs outside
  the cluster); the role shows "Check config" while `url` is empty.
- Poll `GET {url}/Sessions?activeWithinSeconds=60` with header
  `Authorization: MediaBrowser Token="{api_key}"`. Use sessions with
  `NowPlayingItem`: `Name`, `Type` (`Movie`/`Episode`), `SeriesName`,
  `ParentIndexNumber`, `IndexNumber`, `ProductionYear`, `RunTimeTicks`;
  `PlayState.PositionTicks`, `PlayState.IsPaused`; `UserName`,
  `DeviceName`; `TranscodingInfo` present ⇒ transcoding. Ticks are 100 ns.

### Face inputs

The face's worried rule ("a container restarted in the last 300 s") reads the
restarts data: the restart counter rising between two polls marks a restart.

## Config

```yaml
media:
  enabled: false
  radarr: { service: auto, port: 7878, api_key: "" }
  sonarr: { service: auto, port: 8989, api_key: "" }
  lidarr: { service: auto, port: 8686, api_key: "" }

github:
  ci_repos: []            # owner/repo; empty = your repos with recent runs

jellyfin:
  enabled: false
  url: ""                 # e.g. http://192.168.0.x:8096
  api_key: ""             # Jellyfin Dashboard → API Keys
  poll_secs: 5            # >= 2
```

Validation: `poll_secs >= 2`; `url` must start with `http://` or `https://`
when set. The setup TUI gets a "Media & Jellyfin" Configure group with
these fields (API keys masked like the GitHub token).

## Errors and edge cases

- A Prometheus query with no result (exporter missing): that role shows "No
  exporter" (warn) in the pill, hero `—`.
- Nodes without a fan (control plane) are left out of the fans card.
- Restarts with a single huge count: bars scale to the max hour; the ring
  colours are absolute (thresholds above), not scaled.
- CI repo returning 404/403: skipped with one warning in the log per hour.
- Jellyfin clock drift: position is taken from the server each poll; only
  the time between polls is extrapolated.

## Tests

- Parsers: arr queue JSON (the three apps), GitHub runs JSON, Jellyfin
  sessions JSON (movie, episode, paused, transcoding, none), Prometheus range
  matrix.
- Scene builders per role: normal, warning and no-data states; text fits or
  marquees; ring colours per thresholds.
- Session pick rule (most recent playing, else most recent paused) and
  position extrapolation.
- Render goldens for every new role.

## Docs and release

README role table and gallery gain the nine roles; `config.example.yaml`
gains the new blocks. Ships with or after spec A; version 0.8.0 if released
separately.

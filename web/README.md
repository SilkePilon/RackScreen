# RackScreen web UI

The settings and live-view site the RackScreen service serves on port 8740.
Vite, React 19, TypeScript, Tailwind v4 and shadcn/ui (Radix). The Rust
crate `crates/web` embeds `web/dist`; the HTTP contract lives in
`docs/superpowers/specs/2026-11-03-web-ui-design.md`.

## Commands

```sh
npm ci                 # install (Node 20+)
npm run dev:mock       # http://localhost:5173 against a fake API, no Pi needed
npm run dev            # same, proxying /api (and the /api/live websocket) to localhost:8740
npm run build          # type-check, then build to dist/ with .br and .gz siblings
npm run lint           # eslint
npm run typecheck      # tsc only
```

`npm run dev` expects `rackscreen run --sim` (or a real service) on port 8740.

## The mock API

`VITE_MOCK=1` loads `src/mock/` before the app starts: it patches `fetch` for
`/api/*`, draws animated 240×240 frames for the live mirrors on an
OffscreenCanvas, and streams fake log lines. Production builds never contain
it (the import sits behind `import.meta.env.VITE_MOCK`).

Handy switches while it runs:

- `?login`: the API asks for a password (`rackscreen`).
- `?offline`: the API is unreachable (shows the reconnect banner).
- `?update`: a newer release is available.
- From the console: `__mock.offline(true)`, `__mock.login(true)`, `__mock.state`.

## Layout

- `src/lib/api.ts`: typed client; every non-GET request sends `X-RackScreen: 1`.
- `src/lib/types.ts`: API and config types (serde form of `crates/app/src/config.rs`).
- `src/lib/draft.tsx`: the draft config every page edits; the save bar PUTs it.
- `src/lib/settings.ts`: title, help text, page and default of each setting
  (rows and the ⌘K palette both read it).
- `src/lib/live.ts`: the shared `/api/live` socket and the log event stream.
- `src/components/settings.tsx`: section, card and row building blocks.
- `src/pages/`: one lazily loaded file per page.

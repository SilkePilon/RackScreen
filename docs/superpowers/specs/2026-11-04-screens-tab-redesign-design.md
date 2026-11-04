# Screens tab redesign

Date: 2026-11-04

## Goal

Make the Screens page visual: the user sees what each role looks like on the
round panel before and after picking it, and edits one screen at a time with
the rack in view.

## Preview images

- `scripts/role-previews.sh` converts `crates/render/tests/goldens/p2_<role>.png`
  (the healthy state) for every built-in role to `web/public/roles/<role>.webp`
  at 160 px (ImageMagick). Rerun when goldens change; the images are committed
  and embedded with the rest of the UI.
- A test in `crates/web` checks every `Role::ALL` name has a preview file.
- GIF roles (`gif:<name>`) use the GIF's own preview (`classifyLink` in
  `lib/gif.ts`); Tenor links and unknown roles fall back to a dim disc with
  the role's initials.
- `RolePreview` component: round disc with the panel bezel look, sizes by
  class name, lazy-loaded `<img>`.

## Layout

1. **Rack strip**: the screens' live `PanelMirror`s side by side in one dark
   tray, with cycle arcs from `/api/status`. Clicking a panel selects it
   (primary ring, label below). A screen with unsaved changes shows a dot.
   Selection lives in the URL (`?screen=N`, 1-based). Phones scroll the strip
   sideways.
2. **Editor card** for the selected screen:
   - Header: "Screen N", SPI/CS/rotation line, role-reset button, "Each role"
     seconds input with reset (unchanged behaviour).
   - **Filmstrip**: horizontal row of round previews in cycle order, position
     number and title under each. dnd-kit horizontal sortable, keyboard too.
     Hover/focus shows a remove button (hidden when only one role). A role
     with `needs` gets an amber warning dot with a tooltip and a link to its
     integration page. Unknown roles get a red ring and "Unknown role". The
     last tile is an "Add role" disc that opens the gallery.
   - **Cycle timeline**: a segmented bar, one segment per role, with 0 s and
     full-cycle labels; hovering a segment or a tile highlights both.
3. **Presets** stay in the header dropdown; each preset shows its rows as
   mini preview dots.

## Gallery dialog

- Title "Add a role to Screen N", search input, group chips built from the
  role groups (All first).
- Grid of round previews with titles; roles already on the screen show a
  check and cannot be picked again.
- Clicking a role selects it and fills a detail pane (right on desktop,
  below on phones): big preview, title, group, description, needs notice,
  "Add to Screen N" button. Double-click or Enter adds right away. Adding
  closes the dialog.

## Style

Existing tokens and components only (card, hairline, primary ring, warning,
Inter, shadcn Dialog/Tooltip/Button). Motion under 200 ms and none with
reduced motion. Light and dark themes, phone widths.

## Testing

`npm run typecheck`, `npm run lint`, `npm run build`, the Rust preview test,
and a visual check of `dev:mock` in light, dark and at phone width.

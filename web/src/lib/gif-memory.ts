// What GIFs cost on the Pi: every frame is kept decoded and packed (240×240
// RGB565, LZ4; frame_bytes is the worst case), at most max_frames per GIF. The server says what is decoded now and how much
// room there is; this adds what the unsaved draft would load.

import { useDraft } from "@/lib/draft"
import { useGifMemory } from "@/lib/queries"
import { entryRole } from "@/lib/roles"
import type { Config, GifMemory, GifMemoryRow } from "@/lib/types"

/** rackscreen_core::gif::{PACKED_FRAME_MAX, MAX_FRAMES}, until the server answers. */
export const FRAME_BYTES = 240 * 240 * 2
export const MAX_FRAMES = 150
const MIB = 1024 * 1024

/** Frames kept of a GIF of `n`: every ceil(n / max)-th, like core's keep_mask. */
export function keptFrames(n: number, maxFrames = MAX_FRAMES) {
  if (n <= maxFrames) return n
  return Math.ceil(n / Math.ceil(n / maxFrames))
}

/** MB a GIF of `frames` takes decoded; the full length when unknown. */
export function gifMb(frames: number | null | undefined, frameBytes = FRAME_BYTES, maxFrames = MAX_FRAMES) {
  const n = frames && frames > 0 ? keptFrames(frames, maxFrames) : maxFrames
  return (n * frameBytes) / MIB
}

export function formatMb(mb: number) {
  if (mb <= 0) return "0 MB"
  if (mb >= 10240) return `${Math.round(mb / 1024)} GB`
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return mb >= 100 ? `${Math.round(mb)} MB` : `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`
}

// Frame counts of search results that were added, by link, so an unsaved
// GIF is estimated at its own size instead of the full length.
const pickedFrames = new Map<string, number>()
export function rememberFrames(url: string, frames: number | null) {
  if (frames) pickedFrames.set(url.trim(), frames)
}

function usedNames(c: Config | null) {
  return new Set(
    (c?.screens ?? []).flatMap((s) =>
      s.roles
        .map(entryRole)
        .filter((r) => r.startsWith("gif:"))
        .map((r) => r.slice(4))
    )
  )
}

export interface ProjectedGif {
  name: string
  mb: number
  /** Estimated from a search result's frame count, not the full length. */
  known: boolean
  /** Not in the saved config's screens yet. */
  unsaved: boolean
}

export interface GifBudget {
  mem: GifMemory
  /** Draft GIFs on a screen that are not decoded yet, with their estimate. */
  projected: ProjectedGif[]
  projectedMb: number
  /** Room left for more GIFs after the decoded and projected ones. */
  leftMb: number | null
  fitsMore: number | null
  rows: Map<string, GifMemoryRow>
  /** The estimate for a GIF the draft holds, decoded or not. */
  estimateFor: (name: string) => number | null
}

/** The server's numbers plus the draft: what saving would load. */
export function useGifBudget(): { data: GifBudget | undefined; isError: boolean; error: unknown } {
  const q = useGifMemory()
  const d = useDraft()
  const mem = q.data
  if (!mem) return { data: undefined, isError: q.isError, error: q.error }
  const rows = new Map(mem.gifs.map((g) => [g.name, g]))
  const draftUsed = usedNames(d.draft)
  const savedUsed = usedNames(d.saved)
  const est = (url: string) => gifMb(pickedFrames.get(url.trim()), mem.frame_bytes, mem.max_frames)
  const projected: ProjectedGif[] = (d.draft?.gifs ?? [])
    // decoded ones are counted already; failed ones are retried but, like
    // on the server, not counted until they arrive
    .filter((g) => {
      const st = rows.get(g.name)?.state
      return draftUsed.has(g.name) && st !== "ready" && !(st === "error" && savedUsed.has(g.name))
    })
    .map((g) => ({
      name: g.name,
      mb: est(g.url),
      known: pickedFrames.has(g.url.trim()),
      unsaved: !savedUsed.has(g.name),
    }))
  const projectedMb = projected.reduce((a, p) => a + p.mb, 0)
  const leftMb = mem.budget_mb === null ? null : Math.max(0, mem.budget_mb - mem.decoded_mb - projectedMb)
  const fitsMore = leftMb === null ? null : Math.floor(leftMb / mem.max_gif_mb)
  const estimateFor = (name: string) => {
    const r = rows.get(name)
    if (r?.state === "ready" && r.mb !== null) return r.mb
    return projected.find((p) => p.name === name)?.mb ?? null
  }
  return {
    data: { mem, projected, projectedMb, leftMb, fitsMore, rows, estimateFor },
    isError: false,
    error: null,
  }
}

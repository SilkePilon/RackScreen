// A TypeScript port of rackscreen_core::gif::classify_link, so the GIF form
// can explain a bad link before saving. The backend still has the final say.

export type GifLink = { ok: true; preview: string | null } | { ok: false; error: string }

export const GIF_NAME = /^[a-z0-9-]+$/

export function classifyLink(raw: string): GifLink {
  const url = raw.trim()
  if (!url.startsWith("https://")) return { ok: false, error: "The link must start with https://" }
  const rest = url.slice(8)
  const slash = rest.indexOf("/")
  let host = (slash === -1 ? rest : rest.slice(0, slash)).toLowerCase()
  const fullPath = slash === -1 ? "" : rest.slice(slash + 1)
  if (!host || host.includes("@") || host === "localhost" || /^\d+\.\d+\.\d+\.\d+(:\d+)?$/.test(host))
    return { ok: false, error: "GIFs must come from a public internet host" }
  host = host.replace(/^www\./, "")
  const path = fullPath.split(/[?#]/)[0]
  const segs = path.split("/").filter(Boolean)
  const giphy = (id: string): GifLink =>
    id && /^[A-Za-z0-9]+$/.test(id)
      ? { ok: true, preview: `https://i.giphy.com/${id}.gif` }
      : { ok: false, error: "No GIPHY id in this link" }
  if (host === "giphy.com") {
    if (segs[0] === "gifs" && segs[1]) return giphy(segs[1].split("-").at(-1) ?? "")
    return { ok: false, error: "A GIPHY link must be a giphy.com/gifs/… page" }
  }
  if (host === "i.giphy.com") {
    return segs.length === 1 ? giphy(segs[0].split(".")[0]) : { ok: false, error: "No GIPHY id in this link" }
  }
  if (host.startsWith("media") && host.endsWith(".giphy.com")) {
    return segs[0] === "media" && segs.length >= 3
      ? giphy(segs[segs.length - 2])
      : { ok: false, error: "No GIPHY id in this link" }
  }
  if (host === "tenor.com") {
    return segs.slice(0, 2).includes("view")
      ? { ok: true, preview: null }
      : { ok: false, error: "A Tenor link must be a tenor.com/view/… page" }
  }
  if (path.toLowerCase().endsWith(".gif")) return { ok: true, preview: url }
  return { ok: false, error: "Not a GIPHY or Tenor link, or an https link ending in .gif" }
}

/** A GIF name from a search query or a result title: "Happy Cat GIF by X" → "happy-cat". */
export function suggestName(text: string, taken: string[]): string {
  const base =
    text
      .replace(/\s+GIF(\s+by\s+.*)?$/i, "")
      .replace(/\s+(sticker|gif)$/i, "")
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 24)
      .replace(/-+$/, "") || "gif"
  if (!taken.includes(base)) return base
  for (let i = 2; ; i++) if (!taken.includes(`${base}-${i}`)) return `${base}-${i}`
}

/** Why `name` cannot be used for the GIF at `index` (null when adding), or null. */
export function gifNameError(name: string, gifs: { name: string }[], index: number | null): string | null {
  if (!name) return "Give it a name."
  if (!GIF_NAME.test(name)) return "Lowercase letters, digits and - only."
  if (gifs.some((g, i) => g.name === name && i !== index)) return "Another GIF has this name."
  return null
}

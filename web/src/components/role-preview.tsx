import * as React from "react"

import { classifyLink } from "@/lib/gif"
import { useDraft } from "@/lib/draft"
import { cn } from "@/lib/utils"

/** Where a role's still preview lives: the renderer's golden for built-ins, the GIF itself for gif: roles. */
export function usePreviewSrc(name: string): string | null {
  const d = useDraft()
  const gifs = d.draft?.gifs
  return React.useMemo(() => {
    if (name.startsWith("gif:")) {
      const g = gifs?.find((x) => x.name === name.slice(4))
      if (!g) return null
      const link = classifyLink(g.url)
      return link.ok ? link.preview : null
    }
    if (name.startsWith("ha:")) return `${import.meta.env.BASE_URL}roles/ha.webp`
    return /^[a-z0-9-]+$/.test(name) ? `${import.meta.env.BASE_URL}roles/${name}.webp` : null
  }, [name, gifs])
}

function initials(label: string) {
  const words = label
    .replace(/^GIF · /, "")
    .split(/[\s-]+/)
    .filter(Boolean)
  return (words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? "?").slice(0, 2)).toUpperCase()
}

/**
 * A role as it looks on the round panel: a still in the same bezel as the
 * live mirror. Falls back to initials when there is no image.
 */
export function RolePreview({
  name,
  label,
  className,
  dim,
}: {
  name: string
  /** Shown as initials when there is no image, and as alt text. */
  label: string
  className?: string
  dim?: boolean
}) {
  const src = usePreviewSrc(name)
  const [failed, setFailed] = React.useState<string | null>(null)
  const show = src && failed !== src
  return (
    <div
      className={cn(
        "relative aspect-square shrink-0 rounded-full p-[6%]",
        "bg-[radial-gradient(circle_at_30%_20%,#2a2a2e,#0c0c0e_60%)] shadow-[inset_0_1px_0_rgba(255,255,255,.08)]",
        className
      )}
    >
      <div className="[container-type:inline-size] relative size-full overflow-hidden rounded-full bg-black ring-1 ring-black">
        {show ? (
          <img
            src={src}
            alt={label}
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={() => setFailed(src)}
            className={cn("size-full object-cover transition-opacity duration-200", dim && "opacity-40")}
          />
        ) : (
          <div
            role="img"
            aria-label={label}
            className="flex size-full items-center justify-center bg-[radial-gradient(circle,#1c1c20,#050505_70%)] text-[clamp(10px,28cqw,22px)] font-semibold tracking-tight text-white/45"
          >
            {initials(label)}
          </div>
        )}
        <div className="pointer-events-none absolute inset-0 rounded-full bg-[linear-gradient(160deg,rgba(255,255,255,.07),transparent_40%)]" />
      </div>
    </div>
  )
}

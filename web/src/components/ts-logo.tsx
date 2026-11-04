import { cn } from "@/lib/utils"

/** Tailscale's nine dots, as a small mark. */
export function TsLogo({ className }: { className?: string }) {
  const dots: [number, number, boolean][] = [
    [4, 4, false],
    [12, 4, false],
    [20, 4, false],
    [4, 12, true],
    [12, 12, true],
    [20, 12, true],
    [4, 20, false],
    [12, 20, true],
    [20, 20, false],
  ]
  return (
    <svg viewBox="0 0 24 24" className={cn("size-4", className)} aria-hidden>
      {dots.map(([cx, cy, on]) => (
        <circle
          key={`${cx}-${cy}`}
          cx={cx}
          cy={cy}
          r={2.6}
          className={on ? "fill-foreground" : "fill-foreground/30"}
        />
      ))}
    </svg>
  )
}

import type { RoleEntry, RoleInfo } from "@/lib/types"

const ACRONYMS = new Set(["CPU", "ISS", "UPS", "CI", "GIF", "24H"])

/** Panel titles are upper case ("AIR QUALITY"); the UI uses sentence case. */
export function prettyTitle(title: string): string {
  const words = title.split(" ")
  return words
    .map((w, i) => {
      if (ACRONYMS.has(w) || !/[A-Z]/.test(w)) return w
      const lower = w.toLowerCase()
      return i === 0 ? lower[0].toUpperCase() + lower.slice(1) : lower
    })
    .join(" ")
}

export function roleLabel(name: string, roles?: RoleInfo[]): string {
  if (name.startsWith("gif:")) return `GIF · ${name.slice(4)}`
  const r = roles?.find((x) => x.name === name)
  return r ? prettyTitle(r.title) : name
}

/** The role name of a screen's role entry. */
export const entryRole = (e: RoleEntry): string => (typeof e === "string" ? e : e.role)

/** The entry's seconds on screen: its own, else the screen's `cycle_secs`. */
export const entrySecs = (e: RoleEntry, cycleSecs: number): number =>
  (typeof e === "string" ? null : e.secs) ?? cycleSecs

/** The entry with its own seconds, or bare (the screen's default) for null. */
export const withSecs = (e: RoleEntry, secs: number | null): RoleEntry =>
  secs === null ? entryRole(e) : { role: entryRole(e), secs }

export const PRESETS: { id: string; title: string; description: string; rows: string[][] }[] = [
  {
    id: "cluster",
    title: "Cluster",
    description: "CPU, memory, pods and cluster health.",
    rows: [["cpu"], ["mem"], ["pods"], ["health"]],
  },
  {
    id: "electricity",
    title: "Electricity",
    description: "Power mix and renewable share on the first two screens.",
    rows: [["power-mix"], ["renewable"]],
  },
  {
    id: "mixed",
    title: "Mixed",
    description: "Cluster and grid, alternating on the first two screens.",
    rows: [["cpu", "power-mix"], ["mem", "renewable"], ["pods"], ["health"]],
  },
  {
    id: "sky",
    title: "Sky",
    description: "Weather, air, rain, wind, sun, moon, ISS and contributions.",
    rows: [
      ["weather", "aqi"],
      ["rain", "wind"],
      ["sun", "moon"],
      ["iss", "gh-activity"],
    ],
  },
]

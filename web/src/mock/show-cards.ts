// Placeholder show stills for the mock API: a banded pixel sky, a horizon
// and a blocky subject that moves with the frame, as SVG data URLs.

import { setShowCardFactory } from "@/lib/show-card"

const SCENES: Record<string, [sky: string, mid: string, glow: string, ground: string]> = {
  freight: ["#2a1c4a", "#d9575b", "#f2a65a", "#2b1d33"],
  "deep-sea": ["#2a90c0", "#0a1a34", "#8affe0", "#060f22"],
  "paper-plane": ["#5aaee6", "#7cc6f0", "#ffffff", "#a6dcf5"],
  storm: ["#141927", "#2c3445", "#c9d0ff", "#0d1017"],
  day: ["#6cc0ec", "#a6dcf5", "#ffe58a", "#2d5a3a"],
  space: ["#05040f", "#0b0a1d", "#d9773e", "#05040f"],
  birds: ["#f0a35a", "#e0635a", "#ffd36e", "#1f2a55"],
  fireworks: ["#0a0b18", "#121327", "#b6f26b", "#14152a"],
  alpine: ["#2b3a67", "#8a86b8", "#e6ecff", "#1f2c4c"],
  caravan: ["#8fcdf0", "#f4e2a8", "#6b4a2a", "#d8a85a"],
  "desert-night": ["#1d1638", "#3b2d63", "#f2ecd0", "#231a3a"],
  boulder: ["#2a2018", "#4a3b30", "#f2c040", "#3e3026"],
  "time-car": ["#10153a", "#1e2152", "#8ae0ff", "#1c1c2a"],
  kaiju: ["#5a2a5e", "#f08a5a", "#4a9060", "#22204a"],
  heist: ["#10142a", "#1a1e3a", "#ff4050", "#1e2238"],
  shark: ["#7cc4f2", "#2a86c0", "#f0c020", "#f0d898"],
  "dino-chase": ["#4a6a72", "#e8b878", "#56683a", "#8a6a44"],
  dragon: ["#3e2462", "#d86a5e", "#c0403a", "#4a3050"],
  pirates: ["#5aa6e4", "#b8e0f4", "#f8f0dc", "#2a66a8"],
  aurora: ["#0c1430", "#1a5a6a", "#5af09a", "#d8e4f8"],
  launch: ["#4a5a9a", "#d89a8a", "#f4f4f4", "#3a4a3a"],
  hyperspace: ["#05040d", "#34164c", "#bfe6ff", "#0a1824"],
  ufo: ["#070a20", "#2a3462", "#b8ffd0", "#142a1e"],
  "moon-landing": ["#020208", "#6a6a7a", "#d8a838", "#7a7a82"],
  mech: ["#46206a", "#e0605a", "#5af0ff", "#1e1430"],
  "black-hole": ["#03020a", "#120814", "#ffd27a", "#03020a"],
}

function still(id: string, frame: number): string {
  const [sky, mid, glow, ground] = SCENES[id] ?? ["#111", "#333", "#ccc", "#222"]
  const x = 20 + frame * 22
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60" viewBox="0 0 120 60" shape-rendering="crispEdges">
<rect width="120" height="60" fill="${sky}"/>
<rect y="20" width="120" height="22" fill="${mid}"/>
<path d="M0 44h14v-4h12v3h18v-6h16v5h20v-3h14v4h26v17H0z" fill="${ground}"/>
<rect x="${x}" y="26" width="14" height="6" fill="${glow}"/>
<rect x="${x + 3}" y="23" width="8" height="3" fill="${glow}"/>
</svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

setShowCardFactory(still)

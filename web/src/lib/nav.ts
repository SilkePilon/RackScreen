import type { LucideIcon } from "lucide-react"
import {
  BoxesIcon,
  CloudSunIcon,
  CrosshairIcon,
  FilmIcon,
  GitBranchIcon,
  ImagePlayIcon,
  LayoutGridIcon,
  MonitorIcon,
  MusicIcon,
  ScrollTextIcon,
  SparklesIcon,
  SunMoonIcon,
  WrenchIcon,
  ZapIcon,
} from "lucide-react"

export interface Page {
  path: string
  title: string
  /** Breadcrumb parent and sidebar group label. */
  group: "RackScreen" | "Integrations" | "Maintenance"
  icon: LucideIcon
  /** Extra words the command palette matches. */
  keywords?: string
}

export const PAGES: Page[] = [
  { path: "/", title: "Overview", group: "RackScreen", icon: LayoutGridIcon, keywords: "home live panels dashboard status" },
  { path: "/screens", title: "Screens", group: "RackScreen", icon: MonitorIcon, keywords: "roles cycle presets order" },
  { path: "/calibrate", title: "Calibrate", group: "RackScreen", icon: CrosshairIcon, keywords: "rotate mirror spi pins test pattern identify" },
  { path: "/display", title: "Display", group: "RackScreen", icon: SunMoonIcon, keywords: "brightness fps night thresholds" },
  { path: "/shows", title: "Shows", group: "RackScreen", icon: SparklesIcon, keywords: "pixel art animation play" },
  { path: "/integrations/cluster", title: "Cluster", group: "Integrations", icon: BoxesIcon, keywords: "kubernetes prometheus argo qbittorrent kubeconfig" },
  { path: "/integrations/energy", title: "Energy", group: "Integrations", icon: ZapIcon, keywords: "electricity maps power renewable" },
  { path: "/integrations/sky", title: "Sky", group: "Integrations", icon: CloudSunIcon, keywords: "location weather rain iss" },
  { path: "/integrations/media", title: "Media & Jellyfin", group: "Integrations", icon: FilmIcon, keywords: "radarr sonarr lidarr arr jellyfin" },
  { path: "/integrations/spotify", title: "Spotify", group: "Integrations", icon: MusicIcon, keywords: "music login now playing" },
  { path: "/integrations/github", title: "GitHub", group: "Integrations", icon: GitBranchIcon, keywords: "token ci contributions" },
  { path: "/integrations/gifs", title: "GIFs", group: "Integrations", icon: ImagePlayIcon, keywords: "giphy tenor animation" },
  { path: "/logs", title: "Logs", group: "Maintenance", icon: ScrollTextIcon, keywords: "journal stream errors" },
  { path: "/system", title: "System", group: "Maintenance", icon: WrenchIcon, keywords: "update version password yaml backup restart" },
]

export function pageFor(pathname: string): Page | undefined {
  return PAGES.find((p) => p.path === pathname)
}

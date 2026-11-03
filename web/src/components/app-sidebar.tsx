import { NavLink, useLocation } from "react-router"
import { SearchIcon } from "lucide-react"

import { Kbd } from "@/components/ui/kbd"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar"
import { Skeleton } from "@/components/ui/skeleton"
import { PAGES, type Page } from "@/lib/nav"
import { useInfo, useStatus } from "@/lib/queries"
import { cn } from "@/lib/utils"

function NavGroup({ label, pages }: { label?: string; pages: Page[] }) {
  const { pathname } = useLocation()
  const { setOpenMobile } = useSidebar()
  return (
    <SidebarGroup className="py-1">
      {label && (
        <SidebarGroupLabel className="h-7 px-2.5 text-[12px] font-normal text-muted-foreground/80">
          {label}
        </SidebarGroupLabel>
      )}
      <SidebarGroupContent>
        <SidebarMenu className="gap-0.5">
          {pages.map((p) => (
            <SidebarMenuItem key={p.path}>
              <SidebarMenuButton
                asChild
                isActive={pathname === p.path}
                className="h-9 gap-2.5 rounded-lg px-2.5 text-[14px] text-sidebar-foreground transition-colors data-active:font-normal data-active:text-foreground [&_svg]:size-[17px] [&_svg]:stroke-[1.6]"
              >
                <NavLink to={p.path} onClick={() => setOpenMobile(false)}>
                  <p.icon />
                  <span>{p.title}</span>
                </NavLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  )
}

function StatusFooter() {
  const info = useInfo()
  const status = useStatus()
  const down = status.isError
  const state = down ? "offline" : (status.data?.monitor.state ?? "…")
  return (
    <div className="flex items-center rounded-lg px-2.5 py-2">
      <div className="flex min-w-0 flex-col leading-tight">
        {info.data ? (
          <>
            <span className="truncate text-[13px] font-medium text-foreground">{info.data.hostname}</span>
            <span className="truncate text-xs text-muted-foreground tabular">
              v{info.data.version} · {state}
            </span>
          </>
        ) : (
          <>
            <Skeleton className="mb-1 h-3 w-20" />
            <Skeleton className="h-2.5 w-24" />
          </>
        )}
      </div>
    </div>
  )
}

/** Four little circles: the rack, as an icon. */
export function RackGlyph({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={cn("size-4", className)} aria-hidden>
      {[3.5, 9.2, 14.8, 20.5].map((cx, i) => (
        <circle
          key={cx}
          cx={cx}
          cy={12}
          r={2.4}
          className={i === 0 ? "fill-primary" : "fill-foreground/70"}
        />
      ))}
    </svg>
  )
}

export function AppSidebar({ onSearch }: { onSearch: () => void }) {
  const { isMobile } = useSidebar()
  const top = PAGES.filter((p) => p.group === "RackScreen")
  const integrations = PAGES.filter((p) => p.group === "Integrations")
  const maintenance = PAGES.filter((p) => p.group === "Maintenance")
  return (
    <Sidebar collapsible={isMobile ? "offcanvas" : "none"} className="border-r border-sidebar-border">
      <SidebarHeader className="px-3 pt-3 pb-1">
        <button
          type="button"
          onClick={onSearch}
          className="group flex h-10 w-full items-center gap-2.5 rounded-lg px-2.5 text-[14px] text-muted-foreground transition-colors hover:bg-sidebar-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        >
          <SearchIcon className="size-[17px] stroke-[1.6]" />
          <span className="flex-1 text-left">Search</span>
          <Kbd className="h-6 min-w-6 rounded-md border border-sidebar-border bg-sidebar-accent/60">/</Kbd>
        </button>
      </SidebarHeader>
      <SidebarContent className="scrollbar-thin gap-3 px-1.5 pt-1">
        <NavGroup pages={top} />
        <NavGroup label="Integrations" pages={integrations} />
        <NavGroup label="Maintenance" pages={maintenance} />
      </SidebarContent>
      <SidebarFooter className="border-t border-sidebar-border p-2">
        <StatusFooter />
      </SidebarFooter>
    </Sidebar>
  )
}

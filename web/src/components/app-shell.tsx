import * as React from "react"
import { Link, Outlet, useLocation } from "react-router"
import { MoonIcon, SunIcon } from "lucide-react"

import { AppSidebar } from "@/components/app-sidebar"
import { CommandPalette, useCommandPalette } from "@/components/command-palette"
import { HeaderSlotContext } from "@/components/page"
import { ReconnectBanner } from "@/components/reconnect-banner"
import { SaveBar } from "@/components/save-bar"
import { useTheme } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { Skeleton } from "@/components/ui/skeleton"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { pageFor } from "@/lib/nav"

function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const dark = theme !== "light"
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
          onClick={() => setTheme(dark ? "light" : "dark")}
          className="text-muted-foreground hover:text-foreground"
        >
          {dark ? <SunIcon /> : <MoonIcon />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{dark ? "Light theme" : "Dark theme"}</TooltipContent>
    </Tooltip>
  )
}

/** Scroll to and flash the setting row named in the URL hash; else scroll to top. */
function useJumpToSetting(scroller: React.RefObject<HTMLElement | null>) {
  const { pathname, hash } = useLocation()
  React.useEffect(() => {
    if (!hash) {
      scroller.current?.scrollTo({ top: 0 })
      return
    }
    const id = decodeURIComponent(hash.slice(1))
    let tries = 0
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const timer = setInterval(() => {
      const el = document.querySelector<HTMLElement>(`[data-setting-id="${CSS.escape(id)}"]`)
      if (el) {
        clearInterval(timer)
        el.scrollIntoView({ block: "center", behavior: reduce ? "auto" : "smooth" })
        el.classList.remove("setting-flash")
        void el.offsetWidth
        el.classList.add("setting-flash")
        const focusable = el.querySelector<HTMLElement>("input, button[role=switch], button")
        focusable?.focus({ preventScroll: true })
        setTimeout(() => el.classList.remove("setting-flash"), 2000)
      } else if (++tries > 50) clearInterval(timer)
    }, 80)
    return () => clearInterval(timer)
  }, [pathname, hash, scroller])
}

export function PageFallback() {
  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-8 px-4 pt-12 sm:px-8">
      <Skeleton className="h-7 w-40" />
      <Skeleton className="h-40 w-full rounded-xl" />
      <Skeleton className="h-56 w-full rounded-xl" />
    </div>
  )
}

export function AppShell() {
  const palette = useCommandPalette()
  const { pathname } = useLocation()
  const page = pageFor(pathname)
  const [slot, setSlot] = React.useState<HTMLElement | null>(null)
  const main = React.useRef<HTMLElement>(null)
  useJumpToSetting(main)

  return (
    <div className="h-svh bg-canvas md:p-3">
      <SidebarProvider
        className="h-full min-h-0 overflow-hidden bg-background md:rounded-2xl md:border md:shadow-[0_0_0_1px_rgba(0,0,0,.4),0_30px_80px_-20px_rgba(0,0,0,.6)]"
        style={{ "--sidebar-width": "17rem" } as React.CSSProperties}
      >
        <AppSidebar onSearch={() => palette.setOpen(true)} />
        <div className="relative flex min-w-0 flex-1 flex-col">
          <header className="flex h-14 shrink-0 items-center gap-2 border-b border-hairline px-3 sm:px-6">
            <SidebarTrigger className="-ml-1 md:hidden" />
            <nav aria-label="Breadcrumb" className="flex min-w-0 items-center gap-2.5 text-[15px]">
              <Link
                to="/"
                className="hidden text-muted-foreground transition-colors hover:text-foreground sm:inline"
              >
                {page?.group === "Integrations" ? "Integrations" : page?.group === "Maintenance" ? "Maintenance" : "RackScreen"}
              </Link>
              <span className="hidden text-muted-foreground/50 sm:inline" aria-hidden>
                /
              </span>
              <span className="truncate font-medium text-foreground" aria-current="page">
                {page?.title ?? "Not found"}
              </span>
            </nav>
            <div className="flex-1" />
            <div ref={setSlot} className="flex items-center gap-1" />
            <ThemeToggle />
          </header>
          <ReconnectBanner />
          <HeaderSlotContext.Provider value={slot}>
            <main ref={main} className="scrollbar-thin relative flex-1 overflow-y-auto" tabIndex={-1}>
              <React.Suspense fallback={<PageFallback />}>
                <div key={pathname} className="animate-in duration-300 fade-in-0 motion-reduce:animate-none">
                  <Outlet />
                </div>
              </React.Suspense>
            </main>
          </HeaderSlotContext.Provider>
          <SaveBar />
        </div>
      </SidebarProvider>
      <CommandPalette open={palette.open} onOpenChange={palette.setOpen} />
    </div>
  )
}

import * as React from "react"
import { QueryClientProvider, useQueryClient } from "@tanstack/react-query"
import { createBrowserRouter, RouterProvider } from "react-router"

import { AppShell, PageFallback } from "@/components/app-shell"
import { ThemeProvider } from "@/components/theme-provider"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Skeleton } from "@/components/ui/skeleton"
import { onUnauthorized } from "@/lib/api"
import { DraftProvider } from "@/lib/draft"
import { keys, queryClient, useAuth } from "@/lib/queries"
import { LoginPage } from "@/pages/login"

const lazy = (f: () => Promise<{ default: React.ComponentType }>) => {
  const C = React.lazy(f)
  return <C />
}

const router = createBrowserRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: lazy(() => import("@/pages/overview")) },
      { path: "screens", element: lazy(() => import("@/pages/screens")) },
      { path: "calibrate", element: lazy(() => import("@/pages/calibrate")) },
      { path: "display", element: lazy(() => import("@/pages/display")) },
      { path: "shows", element: lazy(() => import("@/pages/shows")) },
      { path: "integrations/cluster", element: lazy(() => import("@/pages/integrations/cluster")) },
      { path: "integrations/energy", element: lazy(() => import("@/pages/integrations/energy")) },
      { path: "integrations/sky", element: lazy(() => import("@/pages/integrations/sky")) },
      { path: "integrations/home", element: lazy(() => import("@/pages/integrations/home")) },
      { path: "integrations/media", element: lazy(() => import("@/pages/integrations/media")) },
      { path: "integrations/spotify", element: lazy(() => import("@/pages/integrations/spotify")) },
      { path: "integrations/github", element: lazy(() => import("@/pages/integrations/github")) },
      { path: "integrations/gifs", element: lazy(() => import("@/pages/integrations/gifs")) },
      { path: "connections/tailscale", element: lazy(() => import("@/pages/connections/tailscale")) },
      { path: "logs", element: lazy(() => import("@/pages/logs")) },
      { path: "system", element: lazy(() => import("@/pages/system")) },
      { path: "*", element: lazy(() => import("@/pages/not-found")) },
    ],
  },
])

function Gate() {
  const auth = useAuth()
  const qc = useQueryClient()
  React.useEffect(() => onUnauthorized(() => void qc.invalidateQueries({ queryKey: keys.auth })), [qc])
  if (auth.isPending) {
    return (
      <div className="flex h-svh items-center justify-center bg-canvas">
        <Skeleton className="size-10 rounded-full" />
      </div>
    )
  }
  if (auth.data?.required && !auth.data.authenticated) return <LoginPage />
  return (
    <DraftProvider>
      <React.Suspense fallback={<PageFallback />}>
        <RouterProvider router={router} />
      </React.Suspense>
    </DraftProvider>
  )
}

export function App() {
  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <TooltipProvider delayDuration={300}>
          <Gate />
          <Toaster position="top-center" offset={14} />
        </TooltipProvider>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

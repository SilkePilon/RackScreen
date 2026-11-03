import * as React from "react"
import { useQueryClient } from "@tanstack/react-query"
import { WifiOffIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ApiError } from "@/lib/api"
import { useStatus } from "@/lib/queries"

/**
 * Shown while /api/status cannot be reached (the service is restarting or the
 * Pi is gone). Status keeps polling; when it answers again every query refreshes.
 */
export function ReconnectBanner() {
  const status = useStatus()
  const qc = useQueryClient()
  const down = status.isError && status.error instanceof ApiError && status.error.unreachable
  const [wasDown, setWasDown] = React.useState(false)
  if (down && !wasDown) setWasDown(true)
  if (!down && wasDown && status.isSuccess) {
    setWasDown(false)
    void qc.invalidateQueries()
  }
  if (!down) return null
  return (
    <div
      role="status"
      className="flex items-center gap-3 border-b border-warning/25 bg-warning/10 px-4 py-2 text-[13px] text-foreground animate-in fade-in-0 slide-in-from-top-1 sm:px-6"
    >
      <WifiOffIcon className="size-4 shrink-0 text-warning" />
      <span className="flex-1">
        <span className="font-medium">Can’t reach RackScreen.</span>{" "}
        <span className="text-muted-foreground">It may be restarting; this page reconnects by itself.</span>
      </span>
      <Spinner className="size-3.5 text-muted-foreground" />
      <Button size="sm" variant="ghost" onClick={() => void status.refetch()}>
        Retry now
      </Button>
    </div>
  )
}

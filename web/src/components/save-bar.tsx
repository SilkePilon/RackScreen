import * as React from "react"
import { CircleAlertIcon, RefreshCwIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useDraft } from "@/lib/draft"
import { cn } from "@/lib/utils"

/** Floats at the bottom of the content while the draft differs from the saved config. */
export function SaveBar() {
  const d = useDraft()
  const n = d.changes.length
  const show = n > 0 || d.invalid > 0 || d.saving
  // Keep the last count while the bar fades out, so it never reads "0 settings".
  const [lastN, setLastN] = React.useState(n)
  if (n > 0 && n !== lastN) setLastN(n)
  const count = n > 0 ? n : lastN
  const blocked = d.invalid > 0
  return (
    <div
      aria-hidden={!show}
      className={cn(
        "pointer-events-none absolute inset-x-0 bottom-0 z-20 flex justify-center px-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] transition-all sm:px-4 sm:pb-5 duration-300 ease-out",
        show ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0",
      )}
    >
      <div
        role="region"
        aria-label="Unsaved changes"
        aria-live="polite"
        className={cn(
          "flex w-full max-w-2xl flex-col gap-2 rounded-xl border bg-popover/95 p-2 pl-4 shadow-2xl shadow-black/50 backdrop-blur-md",
          show && "pointer-events-auto",
          d.error && "border-destructive/50",
        )}
      >
        {d.remote && n > 0 && (
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b pt-1 pr-1 pb-2 text-[13px]">
            <RefreshCwIcon className="size-3.5 shrink-0 text-warning" />
            <span className="flex-1">Config changed on the Pi.</span>
            <Button
              variant="ghost"
              size="sm"
              onClick={d.rebase}
              disabled={d.saving}
              tabIndex={show ? 0 : -1}
              title="Take the Pi's config and keep your changes on top of it"
            >
              Review
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={d.takeRemote}
              disabled={d.saving}
              tabIndex={show ? 0 : -1}
            >
              Discard my changes
            </Button>
          </div>
        )}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 max-sm:pt-1 max-sm:pr-0.5">
          <span className="relative flex size-2 shrink-0">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary opacity-50 motion-reduce:hidden" />
            <span className="relative inline-flex size-2 rounded-full bg-primary" />
          </span>
          <p className="min-w-0 flex-1 text-sm max-sm:basis-[calc(100%-1.25rem)] sm:truncate">
            <span className="font-medium text-foreground">{n > 0 || !blocked ? "Unsaved changes" : "Invalid value"}</span>
            {(n > 0 || !blocked) && (
              <span className="text-muted-foreground tabular">
                {" "}
                · {count} {count === 1 ? "setting" : "settings"}
              </span>
            )}
            {blocked && (
              <span className="text-warning">
                {" "}
                · fix {d.invalid === 1 ? "the highlighted value" : `${d.invalid} highlighted values`} to save
              </span>
            )}
          </p>
          <Button
            variant="ghost"
            onClick={d.discard}
            disabled={d.saving}
            tabIndex={show ? 0 : -1}
            className="max-sm:h-10 max-sm:flex-1"
          >
            Discard
          </Button>
          <Button
            onClick={() => void d.save()}
            disabled={d.saving || blocked || n === 0}
            tabIndex={show ? 0 : -1}
            className="max-sm:h-10 max-sm:flex-[2]"
          >
            {d.saving && <Spinner data-icon="inline-start" />}
            Save &amp; apply
          </Button>
        </div>
        {d.error && (
          <p role="alert" className="flex items-start gap-2 border-t pt-2 pr-2 pb-1 text-[13px] text-destructive">
            <CircleAlertIcon className="mt-0.5 size-3.5 shrink-0" />
            <span className="min-w-0 font-mono text-xs leading-relaxed break-words">{d.error}</span>
          </p>
        )}
      </div>
    </div>
  )
}

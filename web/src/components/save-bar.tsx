import { CircleAlertIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useDraft } from "@/lib/draft"
import { cn } from "@/lib/utils"

/** Floats at the bottom of the content while the draft differs from the saved config. */
export function SaveBar() {
  const d = useDraft()
  const n = d.changes.length
  const show = n > 0 || d.saving
  return (
    <div
      aria-hidden={!show}
      className={cn(
        "pointer-events-none absolute inset-x-0 bottom-0 z-20 flex justify-center px-4 pb-5 transition-all duration-300 ease-out",
        show ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0",
      )}
    >
      <div
        role="region"
        aria-label="Unsaved changes"
        className={cn(
          "flex w-full max-w-2xl flex-col gap-2 rounded-xl border bg-popover/95 p-2 pl-4 shadow-2xl shadow-black/50 backdrop-blur-md",
          show && "pointer-events-auto",
          d.error && "border-destructive/50",
        )}
      >
        <div className="flex items-center gap-3">
          <span className="relative flex size-2 shrink-0">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary opacity-50 motion-reduce:hidden" />
            <span className="relative inline-flex size-2 rounded-full bg-primary" />
          </span>
          <p className="flex-1 truncate text-sm">
            <span className="font-medium text-foreground">Unsaved changes</span>
            <span className="text-muted-foreground tabular">
              {" "}
              · {n} {n === 1 ? "setting" : "settings"}
            </span>
          </p>
          <Button variant="ghost" onClick={d.discard} disabled={d.saving} tabIndex={show ? 0 : -1}>
            Discard
          </Button>
          <Button onClick={() => void d.save()} disabled={d.saving} tabIndex={show ? 0 : -1}>
            {d.saving && <Spinner data-icon="inline-start" />}
            Save &amp; apply
          </Button>
        </div>
        {d.error && (
          <p className="flex items-start gap-2 border-t pt-2 pr-2 pb-1 text-[13px] text-destructive">
            <CircleAlertIcon className="mt-0.5 size-3.5 shrink-0" />
            <span className="font-mono text-xs leading-relaxed">{d.error}</span>
          </p>
        )}
      </div>
    </div>
  )
}

import { RotateCcwIcon } from "lucide-react"
import { toast } from "sonner"

import { HeaderActions } from "@/components/page"
import { Button } from "@/components/ui/button"
import { useDraft } from "@/lib/draft"
import { deepEqual, setAt } from "@/lib/path"
import { defaultsFor } from "@/lib/settings"

/** Header action: put every setting on this page back to its built-in default (as a draft). */
export function RestoreDefaults({ page }: { page: string }) {
  const d = useDraft()
  const defs = defaultsFor(page)
  const differs = d.ready && defs.some(([id, v]) => !deepEqual(d.get(id), v))
  return (
    <HeaderActions>
      <Button
        variant="ghost"
        size="sm"
        disabled={!differs}
        className="text-muted-foreground hover:text-foreground"
        onClick={() => {
          d.update((c) => defs.reduce((acc, [id, v]) => setAt(acc, id, v), c))
          toast("Defaults restored on this page", {
            description: "Review them, then Save & apply. Secrets are kept.",
          })
        }}
      >
        <RotateCcwIcon data-icon="inline-start" />
        <span className="hidden sm:inline">Restore defaults</span>
      </Button>
    </HeaderActions>
  )
}

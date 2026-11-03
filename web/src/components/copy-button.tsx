import * as React from "react"
import { CheckIcon, CopyIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    // Plain http on a LAN address has no clipboard API: fall back to a textarea.
    const ta = document.createElement("textarea")
    ta.value = text
    ta.style.position = "fixed"
    ta.style.opacity = "0"
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand("copy")
    ta.remove()
    return ok
  }
}

export function CopyButton({
  text,
  label = "Copy",
  size = "sm",
  variant = "outline",
  iconOnly,
}: {
  text: string
  label?: string
  size?: "sm" | "default" | "icon-sm" | "icon-xs"
  variant?: "outline" | "ghost" | "secondary"
  iconOnly?: boolean
}) {
  const [done, setDone] = React.useState(false)
  return (
    <Button
      type="button"
      variant={variant}
      size={iconOnly ? "icon-sm" : size}
      aria-label={iconOnly ? label : undefined}
      onClick={async () => {
        if (await copyText(text)) {
          setDone(true)
          setTimeout(() => setDone(false), 1500)
        } else toast.error("Could not copy; select the text instead")
      }}
    >
      {done ? <CheckIcon data-icon={iconOnly ? undefined : "inline-start"} /> : <CopyIcon data-icon={iconOnly ? undefined : "inline-start"} />}
      {!iconOnly && (done ? "Copied" : label)}
    </Button>
  )
}

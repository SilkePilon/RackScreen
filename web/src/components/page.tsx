import * as React from "react"
import { createPortal } from "react-dom"

import { cn } from "@/lib/utils"

// The shell owns the header; pages drop their actions into it through a portal.
export const HeaderSlotContext = React.createContext<HTMLElement | null>(null)

export function HeaderActions({ children }: { children: React.ReactNode }) {
  const el = React.useContext(HeaderSlotContext)
  return el ? createPortal(children, el) : null
}

export function PageBody({
  className,
  wide,
  ...props
}: React.ComponentProps<"div"> & { wide?: boolean }) {
  return (
    <div
      className={cn(
        "mx-auto flex w-full flex-col gap-10 px-4 pt-8 pb-32 sm:px-8 md:pt-12",
        wide ? "max-w-6xl" : "max-w-4xl",
        className,
      )}
      {...props}
    />
  )
}

export function PageIntro({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <h1 className="text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
      {children && <p className="max-w-2xl text-sm text-pretty text-muted-foreground">{children}</p>}
    </div>
  )
}

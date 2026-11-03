import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { cn } from "@/lib/utils"

/** A single-choice segmented control on top of ToggleGroup; never empty. */
export function Segmented<T extends string>({
  value,
  onChange,
  options,
  label,
  className,
  size = "default",
}: {
  value: T
  onChange: (v: T) => void
  options: { value: T; label: React.ReactNode; aria?: string; disabled?: boolean }[]
  label: string
  className?: string
  size?: "sm" | "default"
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      onValueChange={(v) => v && onChange(v as T)}
      aria-label={label}
      spacing={0}
      className={cn("gap-0.5 rounded-lg border bg-muted/60 p-0.5 dark:bg-white/[0.025]", className)}
    >
      {options.map((o) => (
        <ToggleGroupItem
          key={o.value}
          value={o.value}
          aria-label={o.aria}
          disabled={o.disabled}
          size={size}
          className={cn(
            "rounded-md px-3 text-[13px] font-normal text-muted-foreground tabular hover:bg-transparent hover:text-foreground disabled:opacity-40",
            "data-[state=on]:bg-background data-[state=on]:text-foreground data-[state=on]:shadow-sm dark:data-[state=on]:bg-[#222]",
            size === "default" && "h-7",
          )}
        >
          {o.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  )
}

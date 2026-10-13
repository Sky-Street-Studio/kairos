import type { ComponentProps } from "react";

import { cn } from "@/lib/utils";

export function Switch({
  checked,
  onCheckedChange,
  className,
  ...props
}: Omit<ComponentProps<"button">, "onChange"> & { checked: boolean; onCheckedChange(checked: boolean): void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-150 outline-none focus-visible:shadow-[0_0_0_3px_var(--selection)] disabled:opacity-45",
        checked ? "border-transparent bg-foreground" : "bg-muted",
        className,
      )}
      {...props}
    >
      <span
        className={cn(
          "size-3.5 rounded-full shadow-[0_1px_2px_rgb(0_0_0/0.3)] transition-transform duration-150 ease-out",
          checked ? "translate-x-[18px] bg-background" : "translate-x-[2px] bg-muted-foreground",
        )}
      />
    </button>
  );
}

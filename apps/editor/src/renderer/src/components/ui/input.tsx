import type { ComponentProps } from "react";

import { cn } from "@/lib/utils";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      data-slot="input"
      className={cn(
        "h-7 w-full min-w-0 rounded-md border bg-background/60 px-2 text-xs text-foreground outline-none transition-[border-color,box-shadow] placeholder:text-muted-foreground focus-visible:border-ring focus-visible:shadow-[0_0_0_3px_var(--selection)] focus-visible:outline-none aria-invalid:border-destructive",
        className,
      )}
      {...props}
    />
  );
}

export function Kbd({ className, ...props }: ComponentProps<"kbd">) {
  return (
    <kbd
      className={cn(
        "inline-flex h-4.5 min-w-4.5 items-center justify-center rounded-sm border bg-muted px-1 font-sans text-2xs text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

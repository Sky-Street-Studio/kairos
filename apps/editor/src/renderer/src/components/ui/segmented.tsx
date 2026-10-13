import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string; icon?: ReactNode }[];
  onChange(value: T): void;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-md border bg-background/50 p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          onClick={() => onChange(o.value)}
          className={cn(
            "flex h-6 items-center gap-1.5 rounded-sm px-2.5 text-xs [&_svg]:size-3.5",
            value === o.value ? "bg-accent text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
          )}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}

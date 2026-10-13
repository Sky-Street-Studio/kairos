import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function SettingsCard({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="@container rounded-lg border bg-card px-5 pt-4 pb-2 @2xl:px-6">
      <header className="flex flex-col items-start justify-between gap-3 pb-2 @sm:flex-row @sm:gap-4">
        <div>
          <h2 className="text-sm font-semibold">{title}</h2>
          {description && <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>}
        </div>
        {actions && <div className="flex shrink-0 flex-wrap gap-1.5">{actions}</div>}
      </header>
      {children}
    </section>
  );
}

export function SettingRow({
  title,
  description,
  stacked,
  children,
}: {
  title: string;
  description?: string;
  stacked?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "py-3.5 [&+&]:border-t",
        stacked
          ? "space-y-3"
          : "flex flex-col items-start gap-3 @sm:flex-row @sm:items-center @sm:justify-between @sm:gap-8",
      )}
    >
      <div className="min-w-0">
        <div className="text-[13px] font-medium">{title}</div>
        {description && (
          <p className="mt-0.5 max-w-[65ch] text-xs leading-relaxed text-muted-foreground">{description}</p>
        )}
      </div>
      <div className={cn("min-w-0", !stacked && "flex shrink-0 items-center")}>{children}</div>
    </div>
  );
}

export function Range({
  value,
  min,
  max,
  unit,
  label,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  label: string;
  onChange(v: number): void;
}) {
  return (
    <div className="flex items-center gap-3">
      <input
        type="range"
        aria-label={label}
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-1 flex-1 accent-[var(--primary)]"
      />
      <span className="w-12 text-right text-xs text-muted-foreground tabular">
        {value}
        {unit}
      </span>
    </div>
  );
}

import { useId } from "react";

/** The Kairos mark: an hourglass whose two halves take the theme's accent gradient. */
export function KairosMark({ className }: { className?: string }) {
  const id = useId();
  return (
    <svg viewBox="0 0 16 16" className={className} aria-hidden>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--primary)" />
          <stop offset="1" stopColor="var(--primary)" stopOpacity="0.55" />
        </linearGradient>
      </defs>
      <path d="M3 1.5h10L8 8z" fill={`url(#${id})`} />
      <path d="M8 8l5 6.5H3z" fill={`url(#${id})`} opacity="0.7" />
    </svg>
  );
}

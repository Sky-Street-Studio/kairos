export function KairosMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" className={className} aria-hidden>
      <path d="M3 1.5h10L8 8z" fill="var(--primary)" />
      <path d="M8 8l5 6.5H3z" fill="var(--primary)" />
    </svg>
  );
}

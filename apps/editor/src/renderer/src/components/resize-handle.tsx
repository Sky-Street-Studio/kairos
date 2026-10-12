import { useRef } from "react";

import { cn } from "@/lib/utils";

/**
 * Drag to resize a neighbouring panel. `direction` is the sign the size changes by as the pointer
 * moves right (or down for horizontal handles).
 */
export function ResizeHandle({
  orientation,
  size,
  direction,
  min,
  max,
  onResize,
}: {
  orientation: "vertical" | "horizontal";
  size: number;
  direction: 1 | -1;
  min: number;
  max: number;
  onResize(size: number): void;
}) {
  const start = useRef<{ pos: number; size: number } | null>(null);
  const vertical = orientation === "vertical";
  return (
    <div
      role="separator"
      aria-orientation={orientation}
      className={cn("group relative z-10 shrink-0", vertical ? "w-1.5 cursor-col-resize" : "h-1.5 cursor-row-resize")}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { pos: vertical ? e.clientX : e.clientY, size };
      }}
      onPointerMove={(e) => {
        if (!start.current) return;
        const delta = (vertical ? e.clientX : e.clientY) - start.current.pos;
        onResize(Math.min(max, Math.max(min, start.current.size + delta * direction)));
      }}
      onPointerUp={() => {
        start.current = null;
      }}
    >
      <div
        className={cn(
          "absolute rounded-full bg-primary opacity-0 transition-opacity duration-150 group-hover:opacity-60 group-active:opacity-100",
          vertical ? "inset-y-2 left-1/2 w-0.5 -translate-x-1/2" : "inset-x-2 top-1/2 h-0.5 -translate-y-1/2",
        )}
      />
    </div>
  );
}

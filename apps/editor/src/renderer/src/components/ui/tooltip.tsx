import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

export const TooltipProvider = TooltipPrimitive.Provider;

export function Tooltip({
  content,
  children,
  side = "bottom",
}: {
  content: ReactNode;
  children: ReactNode;
  side?: TooltipPrimitive.TooltipContentProps["side"];
}) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={6}
          className="z-50 flex items-center gap-1.5 rounded-md border bg-popover px-2 py-1 text-2xs text-popover-foreground shadow-[0_6px_20px_-6px_rgb(0_0_0/0.45)]"
        >
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}

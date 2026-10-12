import { PanelBottom, PanelLeft, PanelRight, Search, Settings2 } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/input";
import { Tooltip } from "@/components/ui/tooltip";
import { cn, isMac, modKey } from "@/lib/utils";
import { useApp } from "@/store";

import { KairosMark } from "./kairos-mark";

function PanelToggle({
  on,
  label,
  keys,
  onClick,
  children,
}: {
  on: boolean;
  label: string;
  keys: string;
  onClick(): void;
  children: ReactNode;
}) {
  return (
    <Tooltip
      content={
        <>
          {label} <Kbd>{keys}</Kbd>
        </>
      }
    >
      <Button
        variant="ghost"
        size="icon"
        aria-label={label}
        aria-pressed={on}
        onClick={onClick}
        className={cn("no-drag", on && "text-foreground")}
      >
        {children}
      </Button>
    </Tooltip>
  );
}

export function TitleBar() {
  const project = useApp((s) => s.project);
  const layout = useApp((s) => s.layout);
  const togglePanel = useApp((s) => s.togglePanel);
  const setPaletteOpen = useApp((s) => s.setPaletteOpen);
  const openView = useApp((s) => s.openView);
  const mod = modKey();

  return (
    <header
      className={cn(
        "drag-region grid h-10 shrink-0 grid-cols-[1fr_minmax(0,460px)_1fr] items-center gap-3 pr-2",
        isMac() ? "pl-20" : "pl-3",
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        <KairosMark className="size-4 shrink-0" />
        <span className="truncate text-xs font-medium">{project?.manifest.name ?? "Kairos"}</span>
        {project && (
          <span className="truncate text-2xs text-muted-foreground">
            {project.root.replace(/^\/Users\/[^/]+/, "~")}
          </span>
        )}
      </div>
      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        className="no-drag flex h-6.5 items-center gap-2 rounded-md border bg-background/45 px-2 text-xs text-muted-foreground transition-colors hover:bg-background/70 hover:text-foreground"
      >
        <Search className="size-3.5" />
        <span className="flex-1 truncate text-left">Search files, labels and commands</span>
        <Kbd>{mod}K</Kbd>
      </button>
      <div className="flex items-center justify-end gap-0.5">
        <PanelToggle on={layout.sidebar} label="Toggle sidebar" keys={`${mod}B`} onClick={() => togglePanel("sidebar")}>
          <PanelLeft />
        </PanelToggle>
        <PanelToggle on={layout.bottom} label="Toggle terminal" keys={`${mod}J`} onClick={() => togglePanel("bottom")}>
          <PanelBottom />
        </PanelToggle>
        <PanelToggle
          on={layout.preview}
          label="Toggle preview"
          keys={`${mod}\\`}
          onClick={() => togglePanel("preview")}
        >
          <PanelRight />
        </PanelToggle>
        <PanelToggle on={false} label="Settings" keys={`${mod},`} onClick={() => openView("settings")}>
          <Settings2 />
        </PanelToggle>
      </div>
    </header>
  );
}

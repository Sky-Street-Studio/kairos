import { useEffect, useState } from "react";
import { Toaster, toast } from "sonner";

import { ResizeHandle } from "@/components/resize-handle";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewBoundary } from "@/components/view-boundary";
import { BottomPanel } from "@/features/terminal/bottom-panel";
import { CommandPalette } from "@/features/palette/command-palette";
import { PreviewPane } from "@/features/preview/preview-pane";
import { Sidebar } from "@/features/sidebar/sidebar";
import { StatusBar } from "@/features/shell/status-bar";
import { TitleBar } from "@/features/shell/title-bar";
import { Welcome } from "@/features/shell/welcome";
import { Workspace } from "@/features/workspace/workspace";
import { useAppliedTheme } from "@/hooks/use-theme";
import { appStore, useApp } from "@/store";

async function bootstrap() {
  const store = appStore();
  const settings = await window.kairos.settings.get();
  store.setState({ theme: settings.theme, layout: settings.layout });
  const initial = await window.kairos.project.initial();
  if (initial) {
    await store
      .getState()
      .openProject(initial)
      .catch((e: Error) => toast.error("Could not open the last project", { description: e.message }));
  }
}

function useGlobalShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || e.defaultPrevented) return;
      const s = appStore().getState();
      const key = e.key.toLowerCase();
      const run = (fn: () => void) => {
        e.preventDefault();
        fn();
      };
      if (key === "k" || (key === "p" && e.shiftKey)) run(() => s.setPaletteOpen(!s.paletteOpen));
      else if (key === "s" && e.altKey) run(() => void s.saveAll());
      else if (key === "s") {
        const tab = s.tabs.find((t) => t.id === s.activeId);
        if (tab?.kind === "file") run(() => void s.save(tab.path));
      } else if (key === "b") run(() => s.togglePanel("sidebar"));
      else if (key === "j" || key === "`") run(() => s.togglePanel("bottom"));
      else if (key === "\\") run(() => s.togglePanel("preview"));
      else if (key === ",") run(() => s.openView("settings"));
      else if (key === "w") {
        if (s.activeId) run(() => s.closeTab(s.activeId!));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

export function App() {
  const appearance = useAppliedTheme();
  const [ready, setReady] = useState(false);
  const project = useApp((s) => s.project);
  const layout = useApp((s) => s.layout);
  const resizePanel = useApp((s) => s.resizePanel);
  useGlobalShortcuts();

  useEffect(() => {
    void bootstrap().finally(() => setReady(true));
  }, []);

  return (
    <TooltipProvider delayDuration={400}>
      <div className="flex h-full flex-col">
        <TitleBar />
        {!ready ? (
          <div className="flex-1" />
        ) : !project ? (
          <Welcome />
        ) : (
          <div className="flex min-h-0 flex-1 px-1.5">
            {layout.sidebar && (
              <>
                <ViewBoundary name="The sidebar">
                  <Sidebar width={layout.sidebarWidth} />
                </ViewBoundary>
                <ResizeHandle
                  orientation="vertical"
                  size={layout.sidebarWidth}
                  direction={1}
                  min={200}
                  max={420}
                  onResize={(v) => resizePanel("sidebarWidth", v)}
                />
              </>
            )}
            <main className="flex min-w-0 flex-1 flex-col">
              <Workspace />
              {layout.bottom && (
                <ResizeHandle
                  orientation="horizontal"
                  size={layout.bottomHeight}
                  direction={-1}
                  min={120}
                  max={560}
                  onResize={(v) => resizePanel("bottomHeight", v)}
                />
              )}
              <ViewBoundary name="The bottom panel">
                <BottomPanel open={layout.bottom} height={layout.bottomHeight} />
              </ViewBoundary>
            </main>
            {layout.preview && (
              <>
                <ResizeHandle
                  orientation="vertical"
                  size={layout.previewWidth}
                  direction={-1}
                  min={280}
                  max={760}
                  onResize={(v) => resizePanel("previewWidth", v)}
                />
                <ViewBoundary name="The preview">
                  <PreviewPane width={layout.previewWidth} />
                </ViewBoundary>
              </>
            )}
          </div>
        )}
        <StatusBar />
        <CommandPalette />
        <Toaster
          theme={appearance}
          position="bottom-right"
          offset={36}
          toastOptions={{
            className: "!bg-popover !text-popover-foreground !border-border !text-xs !rounded-lg",
          }}
        />
      </div>
    </TooltipProvider>
  );
}

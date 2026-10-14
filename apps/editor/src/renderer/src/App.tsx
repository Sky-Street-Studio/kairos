import { Files, PanelRight } from "lucide-react";
import { useEffect, useState } from "react";
import { Toaster, toast } from "sonner";

import { ResizeHandle } from "@/components/resize-handle";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewBoundary } from "@/components/view-boundary";
import { BottomPanel } from "@/features/terminal/bottom-panel";
import { CommandPalette } from "@/features/palette/command-palette";
import { PreviewPane } from "@/features/preview/preview-pane";
import { FilesPanel } from "@/features/sidebar/files-panel";
import { Sidebar } from "@/features/sidebar/sidebar";
import { StatusBar } from "@/features/shell/status-bar";
import { Welcome } from "@/features/shell/welcome";
import { SettingsView } from "@/features/settings/settings-view";
import { Workspace } from "@/features/workspace/workspace";
import { useAppliedTheme } from "@/hooks/use-theme";
import { appStore, useApp } from "@/store";
import { cn } from "@/lib/utils";

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
      const s = appStore().getState();
      if (e.key === "Escape" && s.settingsOpen) {
        e.preventDefault();
        s.toggleSettings(false);
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || e.defaultPrevented) return;
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
      else if (key === ",") run(() => s.toggleSettings());
      else if (key === "w") {
        const activeId = s.activeId;
        if (activeId) run(() => s.closeTab(activeId));
      }
    };
    window.addEventListener("keydown", onKey);
    const offMenu = window.kairos.menu.onCommand((command) => {
      const s = appStore().getState();
      if (command === "settings") s.toggleSettings();
      else if (command === "toggle-sidebar") s.togglePanel("sidebar");
      else if (command === "toggle-terminal") s.togglePanel("bottom");
      else s.togglePanel("preview");
    });
    return () => {
      window.removeEventListener("keydown", onKey);
      offMenu();
    };
  }, []);
}

function RightPanel({ width }: { width: number }) {
  const [tab, setTab] = useState<"preview" | "files">("preview");
  return (
    <aside
      style={{ width }}
      className="flex min-h-0 shrink-0 flex-col overflow-hidden border-l bg-sidebar"
      aria-label="Side panel"
    >
      <div className="drag-region flex h-9 shrink-0 items-center gap-1 border-b px-2">
        <button
          type="button"
          onClick={() => setTab("preview")}
          aria-pressed={tab === "preview"}
          className={cn(
            "no-drag flex h-7 items-center gap-1.5 rounded-md px-2 text-xs",
            tab === "preview"
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
          )}
          data-testid="panel-preview"
        >
          <PanelRight className="size-3.5" />
          Preview
        </button>
        <button
          type="button"
          onClick={() => setTab("files")}
          aria-pressed={tab === "files"}
          className={cn(
            "no-drag flex h-7 items-center gap-1.5 rounded-md px-2 text-xs",
            tab === "files"
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
          )}
          data-testid="panel-files"
        >
          <Files className="size-3.5" />
          Files
        </button>
      </div>
      {tab === "preview" ? (
        <ViewBoundary name="The preview">
          <PreviewPane />
        </ViewBoundary>
      ) : (
        <ViewBoundary name="Project files">
          <FilesPanel />
        </ViewBoundary>
      )}
    </aside>
  );
}

export function App() {
  const appearance = useAppliedTheme();
  const [ready, setReady] = useState(false);
  const project = useApp((s) => s.project);
  const layout = useApp((s) => s.layout);
  const settingsOpen = useApp((s) => s.settingsOpen);
  const resizePanel = useApp((s) => s.resizePanel);
  useGlobalShortcuts();

  useEffect(() => {
    void bootstrap().finally(() => setReady(true));
  }, []);

  return (
    <TooltipProvider delayDuration={400}>
      <div className="flex h-full w-full flex-col">
        {!ready ? (
          <div className="flex-1" />
        ) : settingsOpen ? (
          <SettingsView />
        ) : !project ? (
          <Welcome />
        ) : (
          <div className="flex min-h-0 flex-1">
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
              <StatusBar />
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
                <RightPanel width={layout.previewWidth} />
              </>
            )}
          </div>
        )}
        <CommandPalette />
        <Toaster
          theme={appearance}
          position="bottom-right"
          offset={36}
          toastOptions={{ className: "!bg-popover !text-popover-foreground !border-border !text-xs !rounded-lg" }}
        />
      </div>
    </TooltipProvider>
  );
}

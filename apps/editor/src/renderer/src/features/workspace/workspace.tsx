import { Boxes, Network, Settings2, UsersRound, X } from "lucide-react";
import { lazy, Suspense } from "react";

import { cn } from "@/lib/utils";
import { basename } from "@/lib/utils";
import { isDirty, useApp, type Tab } from "@/store";
import { fileKind } from "@shared/assets";

import { CharacterManager } from "../characters/character-manager";
import { SettingsView } from "../settings/settings-view";
import { FileIcon } from "../sidebar/file-icon";

const EditorView = lazy(() => import("../editor/editor-view"));
const FilePreview = lazy(() => import("../preview/file-preview"));
const StoryGraphView = lazy(() => import("../graph/story-graph"));
const AssetsManager = lazy(() =>
  import("../assets/assets-manager").then((module) => ({ default: module.AssetsManager })),
);

const VIEW_META = {
  graph: { title: "Story flow", icon: Network },
  characters: { title: "Characters", icon: UsersRound },
  settings: { title: "Settings", icon: Settings2 },
  assets: { title: "Assets", icon: Boxes },
} as const;

function ViewIcon({ kind }: { kind: keyof typeof VIEW_META }) {
  const Icon = VIEW_META[kind].icon;
  return <Icon className="size-3.5 shrink-0" />;
}

function TabButton({ tab, active }: { tab: Tab; active: boolean }) {
  const setActive = useApp((s) => s.setActive);
  const closeTab = useApp((s) => s.closeTab);
  const dirty = useApp((s) => (tab.kind === "file" ? isDirty(s.buffers[tab.path]) : false));
  const errors = useApp((s) =>
    tab.kind === "file" ? s.analysis.diagnostics.filter((d) => d.path === tab.path).length : 0,
  );
  const title = tab.kind === "file" ? basename(tab.path) : VIEW_META[tab.kind].title;
  return (
    <div
      role="tab"
      aria-selected={active}
      tabIndex={0}
      title={tab.kind === "file" ? tab.path : title}
      onClick={() => setActive(tab.id)}
      onKeyDown={(e) => e.key === "Enter" && setActive(tab.id)}
      onAuxClick={(e) => e.button === 1 && closeTab(tab.id)}
      className={cn(
        "group relative flex h-full max-w-52 shrink-0 cursor-default items-center gap-1.5 border-r pr-1.5 pl-3 text-xs",
        active ? "bg-card/70 text-foreground" : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
      )}
    >
      {active && <span className="bg-accent-gradient absolute inset-x-0 top-0 h-0.5" />}
      {tab.kind === "file" ? (
        <FileIcon
          path={tab.path}
          className={cn("size-3.5 shrink-0", active && fileKind(tab.path) === "script" && "text-primary")}
        />
      ) : (
        <ViewIcon kind={tab.kind} />
      )}
      <span className={cn("truncate", errors > 0 && "text-destructive")}>{title}</span>
      <button
        type="button"
        aria-label={`Close ${title}`}
        onClick={(e) => {
          e.stopPropagation();
          closeTab(tab.id);
        }}
        className="group/close relative grid size-5 shrink-0 place-items-center rounded-sm hover:bg-accent"
      >
        {dirty ? (
          <>
            <span className="size-2 rounded-full bg-foreground/70 group-hover/close:hidden" />
            <X className="hidden size-3 group-hover/close:block" />
          </>
        ) : (
          <X className={cn("size-3", !active && "opacity-0 group-hover:opacity-100")} />
        )}
      </button>
    </div>
  );
}

function EmptyWorkspace() {
  const openView = useApp((s) => s.openView);
  return (
    <div className="grid flex-1 place-items-center text-center text-xs text-muted-foreground">
      <div>
        <p>No open tabs.</p>
        <button type="button" onClick={() => openView("graph")} className="mt-1 text-primary hover:underline">
          Open the story flow
        </button>
      </div>
    </div>
  );
}

export function Workspace() {
  const tabs = useApp((s) => s.tabs);
  const activeId = useApp((s) => s.activeId);
  const active = tabs.find((t) => t.id === activeId);
  const isScript = (t: Tab | undefined): t is Extract<Tab, { kind: "file" }> =>
    t?.kind === "file" && fileKind(t.path) === "script";
  const hasScriptTab = tabs.some(isScript);
  const preview = active?.kind === "file" && fileKind(active.path) !== "script" ? active.path : null;

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border bg-panel">
      <div
        role="tablist"
        className="flex h-8.5 shrink-0 overflow-x-auto border-b bg-background/25 [scrollbar-width:none]"
      >
        {tabs.map((t) => (
          <TabButton key={t.id} tab={t} active={t.id === activeId} />
        ))}
      </div>
      <div className="relative flex min-h-0 flex-1">
        <Suspense fallback={null}>
          {hasScriptTab && (
            <div
              className={cn("absolute inset-0 flex", !isScript(active) && "pointer-events-none invisible opacity-0")}
            >
              <EditorView path={isScript(active) ? active.path : null} />
            </div>
          )}
          {preview && (
            <div className="absolute inset-0 flex bg-panel">
              <FilePreview path={preview} />
            </div>
          )}
          {active?.kind === "graph" && <StoryGraphView />}
        </Suspense>
        {active?.kind === "characters" && <CharacterManager />}
        {active?.kind === "settings" && <SettingsView />}
        {active?.kind === "assets" && <AssetsManager />}
        {!active && <EmptyWorkspace />}
      </div>
    </section>
  );
}

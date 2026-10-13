import { Boxes, Clapperboard, Network, Play, Plus, UsersRound, X } from "lucide-react";
import { lazy, Suspense } from "react";

import { ViewBoundary } from "@/components/view-boundary";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { basename, cn } from "@/lib/utils";
import { isDirty, useApp, type Tab } from "@/store";
import { fileKind } from "@shared/assets";

import { CharacterManager } from "../characters/character-manager";
import { FileIcon } from "../sidebar/file-icon";
import { copyPath, revealPath } from "../shell/path-actions";

const EditorView = lazy(() => import("../editor/editor-view"));
const FilePreview = lazy(() => import("../preview/file-preview"));
const StoryGraphView = lazy(() => import("../graph/story-graph"));
const AssetsManager = lazy(() =>
  import("../assets/assets-manager").then((module) => ({ default: module.AssetsManager })),
);
const SceneView = lazy(() => import("../scene/scene-view").then((module) => ({ default: module.SceneView })));

const VIEW_META = {
  graph: { title: "Story flow", icon: Network },
  characters: { title: "Characters", icon: UsersRound },
  assets: { title: "Assets", icon: Boxes },
} as const;

function ViewIcon({ kind }: { kind: keyof typeof VIEW_META }) {
  const Icon = VIEW_META[kind].icon;
  return <Icon className="size-3.5 shrink-0" />;
}

function TabButton({ tab, active }: { tab: Tab; active: boolean }) {
  const setActive = useApp((s) => s.setActive);
  const closeOtherTabs = useApp((s) => s.closeOtherTabs);
  const closeTabsToRight = useApp((s) => s.closeTabsToRight);
  const project = useApp((s) => s.project);
  const closeTab = useApp((s) => s.closeTab);
  const dirty = useApp((s) => (tab.kind === "file" ? isDirty(s.buffers[tab.path]) : false));
  const errors = useApp((s) =>
    tab.kind === "file" ? s.analysis.diagnostics.filter((d) => d.path === tab.path).length : 0,
  );
  const title = tab.kind === "file" ? basename(tab.path) : tab.kind === "scene" ? tab.label : VIEW_META[tab.kind].title;
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          role="tab"
          aria-selected={active}
          tabIndex={0}
          title={tab.kind === "file" ? tab.path : title}
          onClick={() => setActive(tab.id)}
          onKeyDown={(e) => e.key === "Enter" && setActive(tab.id)}
          onAuxClick={(e) => e.button === 1 && closeTab(tab.id)}
          className={cn(
            "no-drag group flex h-full max-w-52 shrink-0 cursor-default items-center gap-1.5 border-r px-2.5 text-xs",
            active ? "bg-accent/50 text-foreground" : "text-muted-foreground hover:bg-accent/40 hover:text-foreground",
          )}
        >
          {tab.kind === "file" ? (
            <FileIcon
              path={tab.path}
              className={cn("size-3.5 shrink-0", active && fileKind(tab.path) === "script" && "text-primary")}
            />
          ) : tab.kind === "scene" ? (
            <Clapperboard className="size-3.5 shrink-0" />
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
            className="grid size-5 shrink-0 place-items-center rounded-sm hover:bg-accent"
          >
            {dirty ? (
              <span className="size-2 rounded-full bg-foreground/70" />
            ) : (
              <X className={cn("size-3", !active && "opacity-0 group-hover:opacity-100")} />
            )}
          </button>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => closeTab(tab.id)}>Close</ContextMenuItem>
        <ContextMenuItem onSelect={() => closeOtherTabs(tab.id)}>Close others</ContextMenuItem>
        <ContextMenuItem onSelect={() => closeTabsToRight(tab.id)}>Close to the right</ContextMenuItem>
        {tab.kind === "file" && project && (
          <>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => void copyPath(project.root, tab.path)}>Copy path</ContextMenuItem>
            <ContextMenuItem onSelect={() => void revealPath(`${project.root}/${tab.path}`)}>
              Reveal in Finder
            </ContextMenuItem>
          </>
        )}
      </ContextMenuContent>
    </ContextMenu>
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
  const openView = useApp((s) => s.openView);
  const cursor = useApp((s) => s.cursor);
  const playFromLine = useApp((s) => s.playFromLine);
  const active = tabs.find((t) => t.id === activeId);
  const isScript = (t: Tab | undefined): t is Extract<Tab, { kind: "file" }> =>
    t?.kind === "file" && fileKind(t.path) === "script";
  const hasScriptTab = tabs.some(isScript);
  const preview = active?.kind === "file" && fileKind(active.path) !== "script" ? active.path : null;

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden bg-panel">
      <div className="drag-region flex h-9 shrink-0 items-center border-b bg-sidebar/70">
        <div role="tablist" className="flex h-full min-w-0 flex-1 overflow-x-auto [scrollbar-width:none]">
          {tabs.map((t) => (
            <TabButton key={t.id} tab={t} active={t.id === activeId} />
          ))}
        </div>
        <button
          type="button"
          aria-label="Open story graph"
          onClick={() => openView("graph")}
          className="no-drag grid size-8 shrink-0 place-items-center text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <Plus className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={() => (cursor ? playFromLine(cursor.path, cursor.line) : openView("graph"))}
          className="no-drag mr-2 flex h-6 shrink-0 items-center gap-1.5 rounded-md bg-primary px-2.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Play className="size-3" />
          Run
        </button>
      </div>
      <div className="relative flex min-h-0 flex-1">
        {hasScriptTab && (
          <div className={cn("absolute inset-0 flex", !isScript(active) && "pointer-events-none invisible opacity-0")}>
            <ViewBoundary name="The code editor">
              <Suspense fallback={null}>
                <EditorView path={isScript(active) ? active.path : null} />
              </Suspense>
            </ViewBoundary>
          </div>
        )}
        {preview && (
          <div className="absolute inset-0 flex bg-panel">
            <ViewBoundary key={preview} name="The file preview">
              <Suspense fallback={null}>
                <FilePreview path={preview} />
              </Suspense>
            </ViewBoundary>
          </div>
        )}
        {active && active.kind !== "file" && (
          <ViewBoundary
            key={active.id}
            name={active.kind === "scene" ? `Scene ${active.label}` : VIEW_META[active.kind].title}
          >
            <Suspense fallback={null}>
              {active.kind === "graph" && <StoryGraphView />}
              {active.kind === "characters" && <CharacterManager />}
              {active.kind === "assets" && <AssetsManager />}
              {active.kind === "scene" && <SceneView label={active.label} />}
            </Suspense>
          </ViewBoundary>
        )}
        {!active && <EmptyWorkspace />}
      </div>
    </section>
  );
}

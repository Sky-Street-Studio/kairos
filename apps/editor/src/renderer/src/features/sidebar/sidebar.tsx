import {
  ChevronRight,
  CircleHelp,
  Folder,
  FolderOpen,
  Images,
  Music,
  Network,
  PanelBottom,
  PanelLeft,
  Plus,
  Search,
  Settings2,
  UserRound,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from "@/components/ui/context-menu";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { isMac, modKey } from "@/lib/utils";
import { isDirty, useApp } from "@/store";
import { assetUrl, fileKind } from "@shared/assets";
import type { TreeNode } from "@shared/types";

import { startAssetDrag } from "../stage/asset-drag";
import { copyPath, revealPath } from "../shell/path-actions";
import { FileIcon } from "./file-icon";

function Section({
  title,
  count,
  actions,
  defaultOpen = true,
  grow,
  children,
}: {
  title: string;
  count?: number;
  actions?: ReactNode;
  defaultOpen?: boolean;
  grow?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className={cn("flex min-h-0 flex-col border-t first:border-t-0", open && grow && "flex-1")}>
      <div className="group flex h-7 shrink-0 items-center gap-1 pr-1.5 pl-1.5">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          aria-expanded={open}
          className="flex h-6 flex-1 items-center gap-1 rounded-md px-1 text-2xs font-semibold tracking-wide text-muted-foreground uppercase hover:text-foreground"
        >
          <ChevronRight className={cn("size-3 transition-transform duration-150", open && "rotate-90")} />
          {title}
          {count !== undefined && <span className="ml-1 font-normal tabular">{count}</span>}
        </button>
        <div className="flex opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          {actions}
        </div>
      </div>
      {open && <div className="min-h-0 overflow-y-auto pb-2">{children}</div>}
    </section>
  );
}

export function TreeRows({ nodes, depth, filter = "" }: { nodes: TreeNode[]; depth: number; filter?: string }) {
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const project = useApp((s) => s.project);
  const activeId = useApp((s) => s.activeId);
  const buffers = useApp((s) => s.buffers);
  const openFile = useApp((s) => s.openFile);
  const diagnostics = useApp((s) => s.analysis.diagnostics);
  return (
    <>
      {nodes.map((n) => {
        const pad = { paddingLeft: 8 + depth * 12 };
        if (n.kind === "dir") {
          const isOpen = Boolean(filter) || !collapsed[n.path];
          return (
            <ContextMenu key={n.path}>
              <ContextMenuTrigger asChild>
                <button
                  type="button"
                  style={pad}
                  onClick={() => setCollapsed({ ...collapsed, [n.path]: isOpen })}
                  className="flex h-6 w-full items-center gap-1.5 pr-2 text-xs text-muted-foreground hover:bg-accent hover:text-foreground [&_svg]:size-3.5"
                >
                  {isOpen ? <FolderOpen /> : <Folder />}
                  <span className="truncate">{n.name}</span>
                </button>
              </ContextMenuTrigger>
              <ContextMenuContent>
                <ContextMenuItem onSelect={() => void revealPath(`${project?.root}/${n.path}`)}>
                  Reveal in Finder
                </ContextMenuItem>
                <ContextMenuItem onSelect={() => project && void copyPath(project.root, n.path)}>
                  Copy path
                </ContextMenuItem>
                <ContextMenuItem onSelect={() => project && void copyPath(project.root, n.path, true)}>
                  Copy relative path
                </ContextMenuItem>
              </ContextMenuContent>
              {isOpen && n.children && <TreeRows nodes={n.children} depth={depth + 1} filter={filter} />}
            </ContextMenu>
          );
        }
        const kind = fileKind(n.path);
        const openable = kind === "script" ? n.path in buffers : kind !== null;
        const errors = diagnostics.filter((d) => d.path === n.path).length;
        return (
          <ContextMenu key={n.path}>
            <ContextMenuTrigger asChild>
              <button
                type="button"
                style={pad}
                disabled={!openable}
                onClick={() => openFile(n.path)}
                className={cn(
                  "flex h-6 w-full items-center gap-1.5 pr-2 text-xs hover:bg-accent disabled:cursor-default disabled:text-muted-foreground disabled:hover:bg-transparent [&_svg]:size-3.5 [&_svg]:shrink-0",
                  activeId === n.path && "bg-accent font-medium text-foreground",
                )}
                data-testid="file-tree-row"
              >
                <FileIcon path={n.path} className={cn(kind === "script" && "text-primary")} />
                <span className="truncate">{n.name}</span>
                {isDirty(buffers[n.path]) && (
                  <span
                    className="size-1.5 shrink-0 rounded-full bg-foreground/70"
                    role="status"
                    aria-label="Unsaved changes"
                  />
                )}
                {errors > 0 && <span className="ml-auto text-2xs text-destructive tabular">{errors}</span>}
              </button>
            </ContextMenuTrigger>
            <ContextMenuContent>
              {openable && <ContextMenuItem onSelect={() => openFile(n.path)}>Open</ContextMenuItem>}
              <ContextMenuItem onSelect={() => project && void revealPath(`${project.root}/${n.path}`)}>
                Reveal in Finder
              </ContextMenuItem>
              <ContextMenuItem onSelect={() => project && void copyPath(project.root, n.path)}>
                Copy path
              </ContextMenuItem>
              <ContextMenuItem onSelect={() => project && void copyPath(project.root, n.path, true)}>
                Copy relative path
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
        );
      })}
    </>
  );
}

function Characters() {
  const outlines = useApp((s) => s.analysis.outlines);
  const openView = useApp((s) => s.openView);
  const openFile = useApp((s) => s.openFile);
  const characters = useMemo(
    () => Object.entries(outlines).flatMap(([path, o]) => o.characters.map((c) => ({ ...c, path }))),
    [outlines],
  );
  return (
    <Section
      title="Characters"
      count={characters.length}
      actions={
        <Tooltip content="Manage characters">
          <Button variant="ghost" size="icon" aria-label="Manage characters" onClick={() => openView("characters")}>
            <UserRound />
          </Button>
        </Tooltip>
      }
    >
      {characters.length === 0 ? (
        <p className="px-3 py-1 text-xs text-muted-foreground">
          No characters yet.{" "}
          <button type="button" className="text-primary hover:underline" onClick={() => openView("characters")}>
            Add one
          </button>
        </p>
      ) : (
        characters.map((c) => (
          <button
            key={`${c.path}:${c.id}`}
            type="button"
            onClick={() => openFile(c.path, c.line)}
            className="flex h-6 w-full items-center gap-2 px-3 text-xs hover:bg-accent"
          >
            <span
              className="size-2.5 shrink-0 rounded-full ring-1 ring-foreground/15"
              style={{ background: c.color ?? "var(--muted-foreground)" }}
            />
            <span className="truncate">{c.name}</span>
            <span className="ml-auto font-mono text-2xs text-muted-foreground">{c.id}</span>
          </button>
        ))
      )}
    </Section>
  );
}

function Assets() {
  const assets = useApp((s) => s.project?.assets ?? []);
  const images = assets.filter((a) => a.kind === "image").slice(0, 6);
  const audio = assets.filter((a) => a.kind === "audio").slice(0, 4);
  const openView = useApp((s) => s.openView);
  const openFile = useApp((s) => s.openFile);
  return (
    <Section
      title="Assets"
      count={assets.filter((a) => a.kind !== "other").length}
      grow
      actions={
        <Tooltip content="Open asset manager">
          <Button variant="ghost" size="icon" aria-label="Open asset manager" onClick={() => openView("assets")}>
            <Images />
          </Button>
        </Tooltip>
      }
    >
      {images.length + audio.length === 0 ? (
        <p className="px-3 py-1 text-xs leading-relaxed text-muted-foreground">
          Drag assets onto the editor or preview, or open the asset manager to import files.
        </p>
      ) : (
        <>
          {images.map((a) => (
            <button
              type="button"
              key={a.path}
              aria-label={`Open ${a.name} image`}
              draggable
              onClick={() => openFile(a.path)}
              onDragStart={(e) => startAssetDrag(e, a)}
              className="flex h-7 cursor-grab items-center gap-2 px-3 text-xs hover:bg-accent active:cursor-grabbing"
              data-testid="asset-thumb"
              title={a.name}
            >
              <img src={assetUrl(a.path)} alt="" draggable={false} className="size-5 rounded-sm border object-cover" />
              <span className="truncate">{a.name}</span>
            </button>
          ))}
          {audio.map((a) => (
            <button
              type="button"
              key={a.path}
              aria-label={`Open ${a.name} audio`}
              draggable
              onClick={() => openFile(a.path)}
              onDragStart={(e) => startAssetDrag(e, a)}
              className="flex h-7 cursor-grab items-center gap-2 px-3 text-xs hover:bg-accent active:cursor-grabbing"
            >
              <Music className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="truncate">{a.name}</span>
            </button>
          ))}
          {assets.length > 10 && (
            <button
              type="button"
              onClick={() => openView("assets")}
              className="px-3 py-1 text-xs text-primary hover:underline"
            >
              View all assets
            </button>
          )}
        </>
      )}
    </Section>
  );
}

export function Sidebar({ width }: { width: number }) {
  const project = useApp((s) => s.project);
  const layout = useApp((s) => s.layout);
  const tabs = useApp((s) => s.tabs);
  const activeId = useApp((s) => s.activeId);
  const setActive = useApp((s) => s.setActive);
  const openView = useApp((s) => s.openView);
  const setPaletteOpen = useApp((s) => s.setPaletteOpen);
  const togglePanel = useApp((s) => s.togglePanel);
  const toggleSettings = useApp((s) => s.toggleSettings);
  const settingsOpen = useApp((s) => s.settingsOpen);
  const openProject = useApp((s) => s.openProject);
  const navigation = [
    { id: "graph", label: "Story graph", icon: Network },
    { id: "characters", label: "Characters", icon: UserRound },
    { id: "assets", label: "Assets", icon: Images },
  ] as const;
  const activeIndex = tabs.findIndex((tab) => tab.id === activeId);

  return (
    <aside
      style={{ width }}
      className="flex min-h-0 shrink-0 flex-col overflow-hidden border-r bg-sidebar text-sidebar-foreground"
    >
      <header className={`drag-region flex h-11 shrink-0 items-center gap-2 pr-2 ${isMac() ? "pl-[78px]" : "pl-3"}`}>
        <span className="truncate text-xs font-semibold">Kairos</span>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Toggle sidebar"
          className="no-drag ml-auto"
          onClick={() => togglePanel("sidebar")}
        >
          <PanelLeft />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Previous tab"
          className="no-drag"
          disabled={activeIndex <= 0}
          onClick={() => setActive(tabs[activeIndex - 1]?.id ?? "")}
        >
          <ChevronRight className="rotate-180" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Next tab"
          className="no-drag"
          disabled={activeIndex < 0 || activeIndex >= tabs.length - 1}
          onClick={() => setActive(tabs[activeIndex + 1]?.id ?? "")}
        >
          <ChevronRight />
        </Button>
      </header>
      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        className="mx-2 mb-2 flex h-8 shrink-0 items-center gap-2 rounded-md bg-background px-2 text-xs text-muted-foreground hover:text-foreground"
      >
        <Search className="size-3.5" />
        <span className="flex-1 text-left">Search</span>
        <span className="text-2xs">{modKey()}K</span>
      </button>
      <nav aria-label="Project navigation" className="shrink-0 px-1.5 pb-2">
        {navigation.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => openView(id)}
            className={cn(
              "flex h-7 w-full items-center gap-2 rounded-md px-2 text-xs",
              activeId === id
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
            )}
          >
            <Icon className="size-3.5" />
            {label}
          </button>
        ))}
      </nav>
      <div className="group flex h-8 shrink-0 items-center gap-1 border-t px-2">
        <ContextMenu>
          <ContextMenuTrigger asChild>
            <button
              type="button"
              className="flex min-w-0 flex-1 items-center gap-2 text-left text-xs hover:text-foreground"
            >
              <Folder className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="truncate font-medium">{project?.manifest.name ?? "Project"}</span>
            </button>
          </ContextMenuTrigger>
          <ContextMenuContent>
            {project && (
              <ContextMenuItem onSelect={() => void revealPath(project.root)}>Reveal in Finder</ContextMenuItem>
            )}
            {project && <ContextMenuItem onSelect={() => void copyPath(project.root, "")}>Copy path</ContextMenuItem>}
          </ContextMenuContent>
        </ContextMenu>
        <NewScriptButton />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Open project folder"
          onClick={async () => {
            const root = await window.kairos.project.pick();
            if (root) await openProject(root);
          }}
        >
          <FolderOpen />
        </Button>
      </div>
      {project && (
        <p className="truncate px-2 pb-2 text-2xs text-muted-foreground" title={project.root}>
          {project.root}
        </p>
      )}
      <Characters />
      <Assets />
      <footer className="mt-auto flex h-9 shrink-0 items-center justify-between border-t px-1.5">
        <div className="flex gap-0.5">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Settings"
            title={`Settings (${modKey()},)`}
            aria-pressed={settingsOpen}
            onClick={() => toggleSettings()}
          >
            <Settings2 />
          </Button>
          <Button variant="ghost" size="icon" aria-label="Help">
            <CircleHelp />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Toggle terminal"
          aria-pressed={layout.bottom}
          onClick={() => togglePanel("bottom")}
        >
          <PanelBottom />
        </Button>
      </footer>
    </aside>
  );
}

function NewScriptButton(props: object) {
  const buffers = useApp((s) => s.buffers);
  const refreshTree = useApp((s) => s.refreshTree);
  const openFile = useApp((s) => s.openFile);
  return (
    <Button
      {...props}
      variant="ghost"
      size="icon"
      aria-label="New script"
      onClick={async () => {
        let n = Object.keys(buffers).length + 1;
        while (`script/chapter${n}.kai` in buffers) n++;
        const path = `script/chapter${n}.kai`;
        await window.kairos.project.writeFile(path, `label chapter${n}:\n    pass\n`);
        await refreshTree();
        openFile(path);
      }}
    >
      <Plus />
    </Button>
  );
}

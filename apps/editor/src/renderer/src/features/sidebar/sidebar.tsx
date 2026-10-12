import {
  ChevronRight,
  FileCode2,
  FileCog,
  FileImage,
  FileMusic,
  FileText,
  Folder,
  FolderOpen,
  Music,
  Plus,
  RefreshCw,
  UserRound,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { isDirty, useApp } from "@/store";
import { assetUrl } from "@shared/assets";
import type { TreeNode } from "@shared/types";

import { startAssetDrag } from "../stage/asset-drag";

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

function fileIcon(path: string) {
  if (path.endsWith(".kai")) return <FileCode2 className="text-primary" />;
  if (path.endsWith(".toml")) return <FileCog />;
  if (/\.(png|jpe?g|webp)$/i.test(path)) return <FileImage />;
  if (/\.(ogg|mp3|wav|flac|opus|m4a)$/i.test(path)) return <FileMusic />;
  return <FileText />;
}

function TreeRows({ nodes, depth }: { nodes: TreeNode[]; depth: number }) {
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const activeId = useApp((s) => s.activeId);
  const buffers = useApp((s) => s.buffers);
  const openFile = useApp((s) => s.openFile);
  const diagnostics = useApp((s) => s.analysis.diagnostics);
  return (
    <>
      {nodes.map((n) => {
        const pad = { paddingLeft: 8 + depth * 12 };
        if (n.kind === "dir") {
          const isOpen = !collapsed[n.path];
          return (
            <div key={n.path}>
              <button
                type="button"
                style={pad}
                onClick={() => setCollapsed({ ...collapsed, [n.path]: isOpen })}
                className="flex h-6 w-full items-center gap-1.5 pr-2 text-xs text-muted-foreground hover:bg-accent hover:text-foreground [&_svg]:size-3.5"
              >
                {isOpen ? <FolderOpen /> : <Folder />}
                <span className="truncate">{n.name}</span>
              </button>
              {isOpen && n.children && <TreeRows nodes={n.children} depth={depth + 1} />}
            </div>
          );
        }
        const editable = n.path in buffers;
        const errors = diagnostics.filter((d) => d.path === n.path).length;
        return (
          <button
            key={n.path}
            type="button"
            style={pad}
            disabled={!editable}
            onClick={() => openFile(n.path)}
            className={cn(
              "flex h-6 w-full items-center gap-1.5 pr-2 text-xs hover:bg-accent disabled:cursor-default disabled:text-muted-foreground disabled:hover:bg-transparent [&_svg]:size-3.5 [&_svg]:shrink-0",
              activeId === n.path && "bg-accent font-medium text-foreground",
            )}
          >
            {fileIcon(n.path)}
            <span className="truncate">{n.name}</span>
            {isDirty(buffers[n.path]) && (
              <span className="size-1.5 shrink-0 rounded-full bg-foreground/70" aria-label="unsaved" />
            )}
            {errors > 0 && <span className="ml-auto text-2xs text-destructive tabular">{errors}</span>}
          </button>
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
  const images = assets.filter((a) => a.kind === "image");
  const audio = assets.filter((a) => a.kind === "audio");
  return (
    <Section title="Assets" count={images.length + audio.length} grow>
      {images.length + audio.length === 0 ? (
        <p className="px-3 py-1 text-xs leading-relaxed text-muted-foreground">
          Put images in <code className="font-mono text-2xs text-foreground">assets/images</code> and sound in{" "}
          <code className="font-mono text-2xs text-foreground">assets/audio</code>. Drag them onto the editor or preview
          to stage them.
        </p>
      ) : (
        <>
          {images.length > 0 && (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(72px,1fr))] gap-1.5 px-2.5 pt-0.5">
              {images.map((a) => (
                <Tooltip key={a.path} content={a.name.startsWith("bg ") ? `scene ${a.name}` : `show ${a.name}`}>
                  <div
                    draggable
                    onDragStart={(e) => startAssetDrag(e, a)}
                    className="group cursor-grab overflow-hidden rounded-md border bg-background/40 active:cursor-grabbing"
                    data-testid="asset-thumb"
                  >
                    <div className="aspect-[4/3] overflow-hidden bg-[repeating-conic-gradient(var(--muted)_0_25%,transparent_0_50%)] bg-[length:10px_10px]">
                      <img
                        src={assetUrl(a.path)}
                        alt=""
                        draggable={false}
                        loading="lazy"
                        className="size-full object-cover transition-transform duration-300 ease-[var(--ease-out-expo)] group-hover:scale-105"
                      />
                    </div>
                    <div className="truncate px-1.5 py-1 text-2xs">{a.name}</div>
                  </div>
                </Tooltip>
              ))}
            </div>
          )}
          {audio.length > 0 && (
            <div className="mt-2">
              {audio.map((a) => (
                <div
                  key={a.path}
                  draggable
                  onDragStart={(e) => startAssetDrag(e, a)}
                  className="flex h-6 cursor-grab items-center gap-2 px-3 text-xs hover:bg-accent"
                >
                  <Music className="size-3.5 shrink-0 text-muted-foreground" />
                  <span className="truncate">{a.name}</span>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </Section>
  );
}

export function Sidebar({ width }: { width: number }) {
  const tree = useApp((s) => s.project?.tree ?? []);
  const refreshTree = useApp((s) => s.refreshTree);
  return (
    <aside
      style={{ width }}
      className="flex shrink-0 flex-col overflow-hidden rounded-lg border border-sidebar-border bg-sidebar text-sidebar-foreground"
    >
      <Section
        title="Project"
        actions={
          <>
            <Tooltip content="New script">
              <NewScriptButton />
            </Tooltip>
            <Tooltip content="Refresh">
              <Button variant="ghost" size="icon" aria-label="Refresh files" onClick={() => void refreshTree()}>
                <RefreshCw />
              </Button>
            </Tooltip>
          </>
        }
      >
        <TreeRows nodes={tree} depth={0} />
      </Section>
      <Characters />
      <Assets />
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

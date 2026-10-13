import type { Block } from "@shared/edit-types";
import type { AssetEntry, PlayerStatus } from "@shared/types";
import { Clapperboard, Copy, Play, RefreshCw, RotateCcw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type DragEvent } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { appStore, useApp } from "@/store";
import { ASSET_MIME, droppedStatement, isAssetDrag } from "../stage/asset-drag";
import { flushPendingEdits } from "../scene/pending-edits";
import { assetToBlock, showBlockAt, showOpFor } from "../scene/stage-drop";
import { insertOp } from "../scene/block-ops";
import { loadPlayerModule, parsePlayerError, projectJson, type PlayerError, type WebPlayer } from "./player";

function sceneBlocks(blocks: Block[]): Block[] {
  return blocks.flatMap((block) => [
    block,
    ...(block.kind === "choice"
      ? block.options.flatMap((option) => sceneBlocks(option.blocks))
      : block.kind === "if"
        ? block.branches.flatMap((branch) => sceneBlocks(branch.blocks))
        : []),
  ]);
}

function StageDropZones({ active }: { active: boolean }) {
  return (
    <div
      className={cn(
        "pointer-events-none absolute inset-0 z-10 grid grid-cols-3 transition-opacity duration-150",
        active ? "opacity-100" : "opacity-0",
      )}
    >
      {(["left", "center", "right"] as const).map((p) => (
        <div key={p} className="flex items-end justify-center border border-dashed border-primary/50 bg-primary/8 pb-4">
          <span className="rounded-md bg-popover px-2 py-0.5 text-2xs text-foreground shadow">at {p}</span>
        </div>
      ))}
    </div>
  );
}

function MissingPlayer({ status, onRetry }: { status: PlayerStatus; onRetry(): void }) {
  return (
    <div
      className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center"
      data-testid="player-missing"
    >
      <Clapperboard className="size-7 text-muted-foreground" strokeWidth={1.5} />
      <div>
        <h3 className="text-sm font-medium">The player isn’t built yet</h3>
        <p className="mx-auto mt-1 max-w-64 text-xs text-balance text-muted-foreground">
          Build the WebAssembly player once and the preview will run your story here, reloading on every save.
        </p>
      </div>
      <button
        type="button"
        onClick={() => {
          void navigator.clipboard.writeText(status.buildHint);
          toast("Copied build command");
        }}
        className="group flex items-center gap-2 rounded-md border bg-background/60 px-2.5 py-1.5 font-mono text-xs hover:border-foreground/25"
      >
        <span className="text-muted-foreground">$</span>
        {status.buildHint}
        <Copy className="size-3 text-muted-foreground group-hover:text-foreground" />
      </button>
      <Button variant="ghost" size="sm" onClick={onRetry}>
        <RefreshCw />
        Check again
      </Button>
    </div>
  );
}

export function PreviewPane() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const playerRef = useRef<WebPlayer | null>(null);
  const [status, setStatus] = useState<PlayerStatus | null>(null);
  const [error, setError] = useState<PlayerError | null>(null);
  const [running, setRunning] = useState(false);
  const [dragging, setDragging] = useState(false);
  const previewVersion = useApp((s) => s.previewVersion);
  const playRequest = useApp((s) => s.playRequest);
  const openFile = useApp((s) => s.openFile);
  const cursor = useApp((s) => s.cursor);
  const tabs = useApp((s) => s.tabs);
  const activeId = useApp((s) => s.activeId);
  const setDebugState = useApp((s) => s.setDebugState);
  const setDebugControl = useApp((s) => s.setDebugControl);
  const lastPlay = useRef<{
    path: string;
    line: number;
    mode: "play" | "seek";
    overrides?: Record<string, unknown>;
  } | null>(null);

  const checkStatus = useCallback(() => void window.kairos.player.status().then(setStatus), []);
  useEffect(checkStatus, [checkStatus]);

  const run = useCallback(
    (
      from?: {
        path: string;
        line: number;
        mode: "play" | "seek";
        overrides?: Record<string, unknown>;
      } | null,
    ) => {
      const player = playerRef.current;
      const { project, buffers } = appStore().getState();
      if (!player || !project) return;
      try {
        const scripts = Object.values(buffers)
          .map(({ path, source }) => ({ path, source }))
          .toSorted((a, b) => a.path.localeCompare(b.path));
        setDebugState(null);
        player.load(
          projectJson(
            project.manifestSource,
            scripts,
            project.uiTheme,
            project.uiTitle,
            project.assets.map((asset) => asset.path),
            project.translations,
            project.uiScreens,
          ),
          (state) => setDebugState(JSON.parse(state)),
        );
        if (from?.mode === "seek") player.previewAt(from.path, from.line);
        else if (from) player.playFromLine(from.path, from.line, JSON.stringify(from.overrides ?? {}));
        else player.start();
        setError(null);
        setRunning(true);
      } catch (e) {
        setError(parsePlayerError(e));
        setRunning(false);
      }
    },
    [setDebugState],
  );

  useEffect(() => {
    if (!status?.available || !canvasRef.current) return;
    let disposed = false;
    const canvas = canvasRef.current;
    void (async () => {
      try {
        const mod = await loadPlayerModule(status.moduleUrl);
        if (disposed) return;
        const player = new mod.WebPlayer(canvas);
        await player.ready();
        if (disposed) return player.destroy();
        playerRef.current = player;
        setDebugControl({
          setVariable: (name, json) => player.setVariable(name, json),
          rollbackTo: (position) => player.rollbackTo(position),
        });
        const rect = canvas.getBoundingClientRect();
        player.resize(Math.round(rect.width), Math.round(rect.height));
        run(lastPlay.current);
      } catch (e) {
        setError(parsePlayerError(e));
      }
    })();
    const observer = new ResizeObserver(([entry]) => {
      const box = entry?.contentRect;
      if (box && playerRef.current) {
        playerRef.current.resize(Math.round(box.width), Math.round(box.height));
      }
    });
    observer.observe(canvas);
    return () => {
      disposed = true;
      observer.disconnect();
      playerRef.current?.destroy();
      playerRef.current = null;
      setDebugControl(null);
    };
  }, [status, run, setDebugControl]);

  useEffect(() => {
    if (previewVersion > 0) run(lastPlay.current);
  }, [previewVersion, run]);

  useEffect(() => {
    if (!playRequest) return;
    lastPlay.current = {
      path: playRequest.path,
      line: playRequest.line,
      mode: playRequest.mode,
      overrides: playRequest.overrides,
    };
    if (playerRef.current) run(lastPlay.current);
    // Seeks fire on every block selection; the missing-player panel already explains the state.
    else if (status && !status.available && playRequest.mode !== "seek")
      toast("Build the player to preview from a line", { description: status.buildHint });
  }, [playRequest, run, status]);

  const onDrop = (e: DragEvent<HTMLDivElement>) => {
    if (!isAssetDrag(e)) return;
    e.preventDefault();
    setDragging(false);
    const asset = JSON.parse(e.dataTransfer.getData(ASSET_MIME)) as AssetEntry;
    const rect = e.currentTarget.getBoundingClientRect();
    const block = assetToBlock(asset, (e.clientX - rect.left) / rect.width);
    const activeTab = tabs.find((tab) => tab.id === activeId);
    if (activeTab?.kind !== "scene") {
      const statement = droppedStatement(e, e.currentTarget);
      if (!statement) return;
      if (appStore().getState().insertAtCursor(statement)) toast(`Inserted ${statement}`);
      else toast.error("Open a script to stage assets into it");
      return;
    }
    if (!block) return;
    const sceneLabel = activeTab.label;
    void (async () => {
      await flushPendingEdits();
      const state = appStore().getState();
      const path = state.analysis.graph.nodes.find((node) => node.label === sceneLabel)?.path;
      const source = path ? state.buffers[path]?.source : undefined;
      const scene = path && source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
      if (!scene || !path) {
        toast.error("Could not add this asset because the scene changed.");
        return;
      }
      const current = appStore().getState();
      const selection =
        current.selectedBlock?.path === path
          ? sceneBlocks(scene.blocks).find((item) => item.line === current.selectedBlock?.line)
          : undefined;
      const selectedLine = selection?.line ?? scene.blocks.at(-1)?.line ?? scene.line;
      const operations =
        block.kind === "show"
          ? [
              showOpFor(
                sceneBlocks(scene.blocks),
                selectedLine,
                block.image[0]!,
                block.image,
                block.position ?? "center",
                path,
              ),
            ]
          : [
              insertOp(
                path,
                selection || scene.blocks.length > 0
                  ? { at: "after", line: selectedLine }
                  : { at: "start", header: scene.line },
                block,
              ),
            ];
      const focus = await appStore().getState().edit(operations);
      if (focus) appStore().getState().selectBlock(focus);
      else toast.error(appStore().getState().editError ?? "Could not add this asset.");
    })();
  };

  const onStageClickCapture = (e: React.MouseEvent<HTMLDivElement>) => {
    const activeTab = appStore()
      .getState()
      .tabs.find((tab) => tab.id === appStore().getState().activeId);
    const canvas = canvasRef.current;
    if (activeTab?.kind !== "scene" || !canvas || !playerRef.current) return;
    const character = playerRef.current.spriteAt(
      (e.nativeEvent.offsetX * canvas.width) / canvas.clientWidth,
      (e.nativeEvent.offsetY * canvas.height) / canvas.clientHeight,
    );
    if (!character) return;
    e.preventDefault();
    e.stopPropagation();
    const sceneLabel = activeTab.label;
    void (async () => {
      const state = appStore().getState();
      const path = state.analysis.graph.nodes.find((node) => node.label === sceneLabel)?.path;
      const source = path ? state.buffers[path]?.source : undefined;
      const scene = path && source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
      if (!scene || !path) return;
      const current = appStore().getState();
      const selection =
        current.selectedBlock?.path === path
          ? sceneBlocks(scene.blocks).find((item) => item.line === current.selectedBlock?.line)
          : undefined;
      const selectedLine = selection?.line ?? scene.blocks.at(-1)?.line ?? scene.line;
      const block = showBlockAt(sceneBlocks(scene.blocks), selectedLine, character);
      if (block) appStore().getState().selectBlock({ path, line: block.line });
    })();
  };

  return (
    <aside className="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="Preview">
      <div className="flex h-8 shrink-0 items-center gap-1 border-b px-1.5">
        {running && <span className="ml-1 size-1.5 rounded-full bg-success" role="status" aria-label="running" />}
        <div className="ml-auto flex items-center gap-0.5">
          <Tooltip content="Restart from the start label">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Restart"
              disabled={!status?.available}
              onClick={() => {
                lastPlay.current = null;
                run(null);
              }}
            >
              <RotateCcw />
            </Button>
          </Tooltip>
          <Tooltip content="Play from the cursor line">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Play from cursor"
              disabled={!cursor}
              onClick={() => cursor && appStore().getState().playFromLine(cursor.path, cursor.line)}
            >
              <Play />
            </Button>
          </Tooltip>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col p-3">
        <div
          ref={stageRef}
          role="application"
          aria-label="Preview stage"
          onDragOver={(e) => {
            if (!isAssetDrag(e)) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "copy";
            setDragging(true);
          }}
          onDragLeave={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false);
          }}
          onDrop={onDrop}
          onClickCapture={onStageClickCapture}
          className="relative aspect-video w-full overflow-hidden rounded-md border bg-[#07070a] shadow-[0_10px_30px_-12px_rgb(0_0_0/0.6)]"
          data-testid="preview-stage"
        >
          {status?.available && <canvas ref={canvasRef} className="absolute inset-0 size-full" />}
          {status && !status.available && (
            <div className="absolute inset-0 bg-card">
              <MissingPlayer status={status} onRetry={checkStatus} />
            </div>
          )}
          <StageDropZones active={dragging} />
        </div>
        {error && (
          <div
            role="alert"
            className="mt-3 flex gap-2 rounded-md border border-destructive/40 bg-destructive/10 p-2.5 text-xs"
          >
            <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-destructive" />
            <div className="min-w-0">
              <p className="selectable break-words">{error.message}</p>
              {error.path && (
                <button
                  type="button"
                  onClick={() => openFile(error.path!, error.line ?? 1)}
                  className="mt-1 font-mono text-2xs text-destructive hover:underline"
                >
                  {error.path}:{error.line}
                </button>
              )}
            </div>
          </div>
        )}
        <p className="mt-3 text-2xs leading-relaxed text-muted-foreground">
          Saving reloads the preview. Drag an image here to stage it at the cursor:{" "}
          <span className="font-mono">bg</span> images become a <span className="font-mono">scene</span>, others a{" "}
          <span className="font-mono">show</span> at the side you drop on.
        </p>
      </div>
    </aside>
  );
}

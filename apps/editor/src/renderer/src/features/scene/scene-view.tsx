import type { Block, Place, Scene } from "@shared/edit-types";
import type { AssetEntry } from "@shared/types";
import {
  AudioLines,
  Braces,
  CircleStop,
  CornerUpRight,
  Copy,
  Eye,
  EyeOff,
  GitBranch,
  Image,
  ListChecks,
  MessageSquareText,
  MoreHorizontal,
  Music2,
  Pause,
  Repeat2,
  Trash2,
  VolumeX,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { basename } from "@/lib/utils";
import { appStore, useApp } from "@/store";

import { AddBlockMenu } from "./add-block-menu";
import { BlockEditor } from "./block-editors";
import { blockDiagnostics, blockTitle, variableSummary } from "./block-labels";
import { insertOp, stripBlock, type BlockContext } from "./block-ops";
import { dropPlace, type DropRow } from "./drop-target";
import { flushPendingEdits, resolveBlock, type BlockPosition } from "./pending-edits";
import { ASSET_MIME } from "../stage/asset-drag";
import { assetToBlock } from "./stage-drop";
import { useScene } from "./use-scene";

function blockIcon(block: Block): LucideIcon {
  switch (block.kind) {
    case "dialogue":
      return MessageSquareText;
    case "background":
      return Image;
    case "show":
      return Eye;
    case "hide":
      return EyeOff;
    case "audio":
      return block.channel === "music" ? Music2 : AudioLines;
    case "stopAudio":
      return VolumeX;
    case "pause":
      return Pause;
    case "choice":
      return ListChecks;
    case "goTo":
      return CornerUpRight;
    case "call":
      return Repeat2;
    case "setVariable":
      return Braces;
    case "if":
      return GitBranch;
    case "end":
      return CircleStop;
    case "code":
      return Braces;
  }
}

function blockSummary(block: Block): string {
  switch (block.kind) {
    case "dialogue":
      return block.text;
    case "background":
    case "show":
      return block.image.join(" ");
    case "hide":
      return block.character;
    case "audio":
      return block.asset;
    case "stopAudio":
      return block.channel;
    case "pause":
      return block.seconds === null ? "Wait for a tap" : `Wait ${block.seconds} seconds`;
    case "choice":
      return `${block.options.length} ${block.options.length === 1 ? "option" : "options"}`;
    case "goTo":
    case "call":
      return block.scene;
    case "setVariable":
      return variableSummary(block);
    case "if":
      return `${block.branches.length} ${block.branches.length === 1 ? "branch" : "branches"}`;
    case "end":
      return "";
    case "code":
      return block.text;
  }
}

type PositionedDropRow = { row: DropRow; position: BlockPosition };

type DropListTarget = { header: number; position: BlockPosition; depth: number; empty: boolean };

function sceneDropLists(blocks: Block[], header: number, prefix: BlockPosition = [], depth = 0): DropListTarget[] {
  return [
    { header, position: prefix, depth, empty: blocks.length === 0 },
    ...blocks.flatMap((block, index) =>
      block.kind === "choice"
        ? block.options.flatMap((option, childIndex) =>
            sceneDropLists(
              option.blocks,
              option.line,
              [...prefix, { index, child: { kind: "choice", index: childIndex } }],
              depth + 1,
            ),
          )
        : block.kind === "if"
          ? block.branches.flatMap((branch, childIndex) =>
              sceneDropLists(
                branch.blocks,
                branch.line,
                [...prefix, { index, child: { kind: "if", index: childIndex } }],
                depth + 1,
              ),
            )
          : [],
    ),
  ];
}

function sceneDropRows(blocks: Block[], header: number, prefix: BlockPosition = [], depth = 0): PositionedDropRow[] {
  return blocks.flatMap((block, index) => {
    const position = [...prefix, { index }];
    const row: DropRow = { line: block.line, endLine: block.endLine, depth, parentHeader: header };
    const children =
      block.kind === "choice"
        ? block.options.flatMap((option, childIndex) =>
            sceneDropRows(
              option.blocks,
              option.line,
              [...prefix, { index, child: { kind: "choice", index: childIndex } }],
              depth + 1,
            ),
          )
        : block.kind === "if"
          ? block.branches.flatMap((branch, childIndex) =>
              sceneDropRows(
                branch.blocks,
                branch.line,
                [...prefix, { index, child: { kind: "if", index: childIndex } }],
                depth + 1,
              ),
            )
          : [];
    return [{ row, position }, ...children];
  });
}

function placeForDrop(root: HTMLDivElement, scene: Scene, pointerY: number): Place | null {
  const elements = new Map(
    [...root.querySelectorAll<HTMLElement>("[data-block-position]")].map((element) => [
      element.dataset.blockPosition,
      element,
    ]),
  );
  const listElements = new Map(
    [root, ...root.querySelectorAll<HTMLElement>("[data-scene-list-position]")].map((element) => [
      element.dataset.sceneListPosition,
      element,
    ]),
  );
  const emptyLists = sceneDropLists(scene.blocks, scene.line)
    .filter((list) => list.empty)
    .map((list) => ({ list, element: listElements.get(JSON.stringify(list.position)) ?? null }))
    .filter((item): item is { list: DropListTarget; element: HTMLElement } => item.element !== null);
  const emptyTarget = emptyLists
    .filter(({ element }) => {
      const rect = element.getBoundingClientRect();
      return pointerY >= rect.top && pointerY <= rect.bottom;
    })
    .toSorted((a, b) => b.list.depth - a.list.depth)[0];
  const visibleRows = sceneDropRows(scene.blocks, scene.line)
    .map(({ row, position }) => ({ row, element: elements.get(JSON.stringify(position)) }))
    .filter((item): item is { row: DropRow; element: HTMLElement } => item.element !== undefined);
  if (emptyTarget) return { at: "start", header: emptyTarget.list.header };
  return dropPlace(
    visibleRows.map(({ row }) => row),
    pointerY,
    visibleRows.map(({ element }) => element.getBoundingClientRect()),
  );
}

function BlockList({
  blocks,
  path,
  header,
  ctx,
  sceneLabel,
  positionPrefix = [],
}: {
  blocks: Block[];
  path: string;
  header: number;
  ctx: BlockContext;
  sceneLabel: string;
  positionPrefix?: BlockPosition;
}) {
  const diagnostics = useApp((state) => state.analysis.diagnostics);
  const characters = useApp((state) => state.analysis.outlines[path]?.characters ?? []);
  const selectedBlock = useApp((state) => state.selectedBlock);
  const selectBlock = useApp((state) => state.selectBlock);
  const edit = useApp((state) => state.edit);
  const names = Object.fromEntries(characters.map((character) => [character.id, character.name]));
  const listRef = useRef<HTMLDivElement>(null);
  const [dropY, setDropY] = useState<number | null>(null);
  const isRootList = positionPrefix.length === 0;
  const onDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    const blockDrag = event.dataTransfer.types.includes("application/x-kairos-block");
    const assetDrag = isRootList && event.dataTransfer.types.includes(ASSET_MIME);
    if (!blockDrag && !assetDrag) return;
    event.preventDefault();
    setDropY(event.clientY - (listRef.current?.getBoundingClientRect().top ?? 0));
  };
  const onDrop = (event: React.DragEvent<HTMLDivElement>) => {
    const blockDrag = event.dataTransfer.types.includes("application/x-kairos-block");
    const assetDrag = isRootList && event.dataTransfer.types.includes(ASSET_MIME);
    if (!blockDrag && !assetDrag) return;
    event.preventDefault();
    setDropY(null);
    const pointerY = event.clientY;
    const root = listRef.current;
    if (assetDrag) {
      let asset: AssetEntry;
      try {
        asset = JSON.parse(event.dataTransfer.getData(ASSET_MIME)) as AssetEntry;
      } catch {
        toast.error("Could not add this asset.");
        return;
      }
      const rect = root?.getBoundingClientRect();
      const block = assetToBlock(asset, rect ? (event.clientX - rect.left) / rect.width : 0.5);
      if (!block) return;
      void (async () => {
        await flushPendingEdits();
        const source = appStore().getState().buffers[path]?.source;
        const latestScene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
        if (!latestScene || !root) {
          toast.error("Could not add this asset because the scene changed.");
          return;
        }
        const place = placeForDrop(root, latestScene, pointerY) ?? { at: "start", header: latestScene.line };
        const focus = await edit([insertOp(path, place, block)]);
        if (focus) selectBlock(focus);
        else toast.error(appStore().getState().editError ?? "Could not add this asset.");
      })();
      return;
    }
    const positionData = event.dataTransfer.getData("application/x-kairos-block-position");
    let draggedPosition: BlockPosition;
    try {
      draggedPosition = JSON.parse(positionData) as BlockPosition;
    } catch {
      toast.error("Could not move this block.");
      return;
    }
    void (async () => {
      await flushPendingEdits();
      const source = appStore().getState().buffers[path]?.source;
      const latestScene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
      const dragged = latestScene ? resolveBlock(latestScene.blocks, draggedPosition) : null;
      if (!latestScene || !dragged || !root) {
        toast.error("Could not move this block because the scene changed.");
        return;
      }
      const place = placeForDrop(root, latestScene, pointerY);
      if (!place) return;
      const focus = await edit([{ op: "move", path, line: dragged.line, place }]);
      if (!focus) toast.error(appStore().getState().editError ?? "Could not move this block.");
    })();
  };
  const onDragEnd = () => setDropY(null);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    const isField = target.matches("input, textarea, select, [contenteditable=true]");
    const activeIndex = blocks.findIndex((block) => selectedBlock?.path === path && block.line === selectedBlock.line);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (isField) return;
      event.preventDefault();
      const next = Math.max(0, Math.min(blocks.length - 1, activeIndex + (event.key === "ArrowDown" ? 1 : -1)));
      if (blocks[next]) selectBlock({ path, line: blocks[next].line });
    } else if (event.key === "Enter" && !event.metaKey && !event.ctrlKey && !isField && blocks[activeIndex]) {
      event.preventDefault();
      listRef.current
        ?.querySelector<HTMLElement>(
          `[data-block-line="${blocks[activeIndex].line}"] input, [data-block-line="${blocks[activeIndex].line}"] textarea, [data-block-line="${blocks[activeIndex].line}"] select`,
        )
        ?.focus();
    } else if (event.key === "Escape" && isField) {
      target.blur();
    } else if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && !isField && blocks[activeIndex]) {
      event.preventDefault();
      const position = [...positionPrefix, { index: activeIndex }];
      void (async () => {
        await flushPendingEdits();
        const source = appStore().getState().buffers[path]?.source;
        const scene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
        const block = scene ? resolveBlock(scene.blocks, position) : null;
        if (!block) return;
        const character = block.kind === "dialogue" ? block.character : (ctx.characters[0] ?? null);
        const focus = await edit([
          insertOp(path, { at: "after", line: block.line }, { kind: "dialogue", character, text: "" }),
        ]);
        if (focus) selectBlock(focus);
      })();
    } else if ((event.key === "Delete" || event.key === "Backspace") && !isField && blocks[activeIndex]) {
      event.preventDefault();
      const position = [...positionPrefix, { index: activeIndex }];
      void (async () => {
        await flushPendingEdits();
        const source = appStore().getState().buffers[path]?.source;
        const scene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
        const block = scene ? resolveBlock(scene.blocks, position) : null;
        if (!block) return;
        const focus = await edit([{ op: "delete", path, line: block.line }]);
        if (focus) selectBlock(focus);
      })();
    }
  };

  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="Scene blocks"
      tabIndex={0}
      onKeyDown={(event) => {
        onKeyDown(event);
        event.stopPropagation();
      }}
      data-scene-list-position={JSON.stringify(positionPrefix)}
      className="relative flex flex-col outline-none"
      onDragOver={isRootList ? onDragOver : undefined}
      onDrop={isRootList ? onDrop : undefined}
      onDragEnd={isRootList ? onDragEnd : undefined}
    >
      <div className="flex items-center justify-center gap-2">
        {blocks.length === 0 && <span className="text-2xs text-muted-foreground">Add the first line</span>}
        <AddBlockMenu path={path} sceneLabel={sceneLabel} place={{ at: "start", header }} ctx={ctx} />
      </div>
      {blocks.map((block, index) => {
        const position = [...positionPrefix, { index }];
        const Icon = blockIcon(block);
        const issues = blockDiagnostics(block, diagnostics, path);
        const selected = selectedBlock?.path === path && selectedBlock.line === block.line;
        return (
          <div key={block.line} data-block-line={block.line} className="flex flex-col">
            <div className="group flex min-h-12 items-center border-b pr-2 hover:bg-accent/30">
              <button
                type="button"
                role="option"
                aria-selected={selected}
                data-testid="block"
                data-block-position={JSON.stringify(position)}
                draggable
                onDragStart={(event) => {
                  event.dataTransfer.setData("application/x-kairos-block", String(block.line));
                  event.dataTransfer.setData("application/x-kairos-block-position", JSON.stringify(position));
                  event.dataTransfer.effectAllowed = "move";
                }}
                onClick={() => selectBlock({ path, line: block.line })}
                className="flex min-h-12 min-w-0 flex-1 items-center gap-3 px-4 py-2 text-left outline-none focus-visible:bg-accent/50 aria-selected:bg-accent/70"
              >
                <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-medium text-foreground">{blockTitle(block, names)}</span>
                  <span className="block truncate text-2xs text-muted-foreground">{blockSummary(block)}</span>
                </span>
                {issues.length > 0 && (
                  <span
                    title={issues.map((issue) => `${issue.path}:${issue.line}: ${issue.message}`).join("\n")}
                    className="shrink-0 text-2xs font-medium text-destructive tabular"
                  >
                    {issues.length}
                  </span>
                )}
              </button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Block actions"
                    className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  >
                    <MoreHorizontal className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    onSelect={() =>
                      void (async () => {
                        await flushPendingEdits();
                        const source = appStore().getState().buffers[path]?.source;
                        const scene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
                        const current = scene ? resolveBlock(scene.blocks, position) : null;
                        if (!current) return;
                        const focus = await edit([
                          insertOp(path, { at: "after", line: current.line }, stripBlock(current)),
                        ]);
                        if (focus) selectBlock(focus);
                      })()
                    }
                  >
                    <Copy />
                    Duplicate
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="text-destructive"
                    onSelect={() =>
                      void (async () => {
                        await flushPendingEdits();
                        const source = appStore().getState().buffers[path]?.source;
                        const scene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
                        const current = scene ? resolveBlock(scene.blocks, position) : null;
                        if (!current) return;
                        const focus = await edit([{ op: "delete", path, line: current.line }]);
                        if (focus) selectBlock(focus);
                      })()
                    }
                  >
                    <Trash2 />
                    Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            {selected && (
              <BlockEditor
                block={block}
                path={path}
                ctx={ctx}
                sceneLabel={sceneLabel}
                position={position}
                renderBlocks={(nested, nestedHeader, nestedPosition) => (
                  <BlockList
                    blocks={nested}
                    path={path}
                    header={nestedHeader}
                    ctx={ctx}
                    sceneLabel={sceneLabel}
                    positionPrefix={nestedPosition}
                  />
                )}
              />
            )}
            <AddBlockMenu
              path={path}
              sceneLabel={sceneLabel}
              place={{ at: "after", line: block.line }}
              targetPosition={position}
              ctx={ctx}
            />
          </div>
        );
      })}
      {dropY !== null && isRootList && (
        <div
          aria-hidden="true"
          className="pointer-events-none absolute left-0 right-0 z-20 h-0.5 bg-primary"
          style={{ top: dropY }}
        />
      )}
    </div>
  );
}

export function SceneView({ label }: { label: string }) {
  const { path, scene } = useScene(label);
  const goToLabel = useApp((state) => state.goToLabel);
  const characters = useApp((state) => state.analysis.outlines[path]?.characters ?? []);
  const project = useApp((state) => state.project);
  const scenes = useApp((state) => state.analysis.graph.nodes.map((node) => node.label));
  const backgrounds =
    project?.assets.filter((asset) => asset.category === "background").map((asset) => asset.name.split(/\s+/)) ?? [];
  const music = project?.assets.filter((asset) => asset.category === "music").map((asset) => asset.name) ?? [];
  const ctx: BlockContext = { characters: characters.map((character) => character.id), backgrounds, music, scenes };
  const selectedBlock = useApp((state) => state.selectedBlock);
  const previewAt = useApp((state) => state.previewAt);

  useEffect(() => {
    if (
      !scene ||
      selectedBlock?.path !== path ||
      !sceneDropRows(scene.blocks, scene.line).some(({ row }) => row.line === selectedBlock.line)
    )
      return;
    const timer = setTimeout(() => previewAt(path, selectedBlock.line), 150);
    return () => clearTimeout(timer);
  }, [path, previewAt, scene, selectedBlock]);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <header className="flex min-h-14 shrink-0 items-center gap-3 border-b px-4 py-2">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-sm font-medium">{label}</h1>
          <p className="truncate text-2xs text-muted-foreground">
            {scene ? `${basename(path)}:${scene.line}` : path || "Scene not found"}
          </p>
        </div>
        <Button variant="secondary" size="sm" onClick={() => goToLabel(label)}>
          Open code
        </Button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {scene ? (
          <BlockList blocks={scene.blocks} path={path} header={scene.line} ctx={ctx} sceneLabel={label} />
        ) : (
          <div className="grid min-h-40 place-items-center p-6 text-center text-xs text-muted-foreground">
            Scene could not be opened
          </div>
        )}
      </div>
    </div>
  );
}

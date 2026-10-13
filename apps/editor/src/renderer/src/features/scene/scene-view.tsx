import type { Block } from "@shared/edit-types";
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
import { useRef } from "react";

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
import { flushPendingEdits, resolveBlock, type BlockPosition } from "./pending-edits";
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
      className="flex flex-col outline-none"
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

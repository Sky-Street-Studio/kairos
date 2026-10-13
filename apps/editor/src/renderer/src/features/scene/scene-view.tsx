import type { Block, ChoiceOption, IfBranch } from "@shared/edit-types";
import {
  AudioLines,
  Braces,
  CircleStop,
  CornerUpRight,
  Eye,
  EyeOff,
  GitBranch,
  Image,
  ListChecks,
  MessageSquareText,
  Music2,
  Pause,
  Repeat2,
  VolumeX,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { basename } from "@/lib/utils";
import { useApp } from "@/store";

import { blockDiagnostics, blockTitle, variableSummary } from "./block-labels";
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

function branchTitle(branch: ChoiceOption | IfBranch): string {
  if ("text" in branch) return branch.condition ? `If ${branch.condition}: “${branch.text}”` : `“${branch.text}”`;
  return branch.condition ? `If ${branch.condition}` : "Otherwise";
}

function BlockList({ blocks, path }: { blocks: Block[]; path: string }) {
  const diagnostics = useApp((state) => state.analysis.diagnostics);
  const characters = useApp((state) => state.analysis.outlines[path]?.characters ?? []);
  const selectedBlock = useApp((state) => state.selectedBlock);
  const selectBlock = useApp((state) => state.selectBlock);
  const names = Object.fromEntries(characters.map((character) => [character.id, character.name]));

  return (
    <div role="listbox" aria-label="Scene blocks" className="flex flex-col">
      {blocks.map((block) => {
        const Icon = blockIcon(block);
        const issues = blockDiagnostics(block, diagnostics, path);
        const selected = selectedBlock?.path === path && selectedBlock.line === block.line;
        const branches = block.kind === "choice" ? block.options : block.kind === "if" ? block.branches : null;

        return (
          <div key={block.line} className="flex flex-col">
            <button
              type="button"
              role="option"
              aria-selected={selected}
              data-testid="block"
              onClick={() => selectBlock({ path, line: block.line })}
              className="group flex min-h-12 items-center gap-3 border-b px-4 py-2 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:bg-accent/50 aria-selected:bg-accent/70"
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
            {branches?.map((branch) => (
              <div key={branch.line} className="border-b bg-muted/15 py-2 pl-11 pr-3">
                <p className="mb-1 text-2xs font-medium text-muted-foreground">{branchTitle(branch)}</p>
                {branch.blocks.length > 0 ? (
                  <div className="border-l pl-2">
                    <BlockList blocks={branch.blocks} path={path} />
                  </div>
                ) : (
                  <p className="py-1 text-2xs text-muted-foreground">No lines yet</p>
                )}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}

export function SceneView({ label }: { label: string }) {
  const { path, scene } = useScene(label);
  const goToLabel = useApp((state) => state.goToLabel);

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
        {scene?.blocks.length ? (
          <BlockList blocks={scene.blocks} path={path} />
        ) : (
          <div className="grid min-h-40 place-items-center p-6 text-center text-xs text-muted-foreground">
            {scene ? "Add the first line" : "Scene could not be opened"}
          </div>
        )}
      </div>
    </div>
  );
}

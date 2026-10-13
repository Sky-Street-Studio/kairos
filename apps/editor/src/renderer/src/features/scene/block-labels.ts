import type { Block } from "@shared/edit-types";
import type { FileDiagnostic } from "@shared/types";

export function blockTitle(block: Block, names: Record<string, string>): string {
  const who = (id: string | null | undefined) => (id ? (names[id] ?? id) : "Narrator");
  switch (block.kind) {
    case "dialogue":
      return block.character ? `${who(block.character)} says` : "Narrator";
    case "background":
      return "Change background";
    case "show":
      return `Show ${who(block.image[0])}`;
    case "hide":
      return `Hide ${who(block.character)}`;
    case "audio":
      return block.channel === "music" ? "Play music" : block.channel === "voice" ? "Play voice" : "Play sound";
    case "stopAudio":
      return `Stop ${block.channel}`;
    case "pause":
      return "Pause";
    case "choice":
      return "Choice";
    case "goTo":
      return `Go to ${block.scene}`;
    case "call":
      return `Visit ${block.scene} and come back`;
    case "setVariable":
      return `Change ${block.name}`;
    case "if":
      return "Only if…";
    case "end":
      return "End scene";
    case "code":
      return "Code";
  }
}

function children(block: Block): Block[] {
  if (block.kind === "choice") return block.options.flatMap((option) => option.blocks);
  if (block.kind === "if") return block.branches.flatMap((branch) => branch.blocks);
  return [];
}

/** Problems on this block's own lines; problems inside a nested block belong to that block. */
export function blockDiagnostics(block: Block, diagnostics: FileDiagnostic[], path: string): FileDiagnostic[] {
  const nested = children(block);
  return diagnostics.filter(
    (diagnostic) =>
      diagnostic.path === path &&
      diagnostic.line >= block.line &&
      diagnostic.line <= block.endLine &&
      !nested.some((child) => diagnostic.line >= child.line && diagnostic.line <= child.endLine),
  );
}

export function variableSummary(block: Extract<Block, { kind: "setVariable" }>): string {
  switch (block.op) {
    case "set":
      return `Set ${block.name} to ${block.value}`;
    case "add":
      return `Increase ${block.name} by ${block.value}`;
    case "subtract":
      return `Decrease ${block.name} by ${block.value}`;
    case "toggle":
      return `Flip ${block.name}`;
  }
}

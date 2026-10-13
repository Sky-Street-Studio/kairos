import type { Block, BlockKind, EditOp, Place } from "@shared/edit-types";

export interface BlockContext {
  characters: string[];
  backgrounds: string[][];
  music: string[];
  scenes: string[];
}

const strip = (block: Block): BlockKind => {
  const { line: _line, endLine: _end, ...kind } = block;
  return kind as BlockKind;
};

export const insertOp = (path: string, place: Place, block: BlockKind): EditOp => ({
  op: "insert",
  path,
  place,
  block,
});

export function fieldOp(path: string, block: Block, patch: Partial<BlockKind>): EditOp {
  return { op: "replace", path, line: block.line, block: { ...strip(block), ...patch } as BlockKind };
}

export function defaultBlock(kind: BlockKind["kind"], ctx: BlockContext): BlockKind {
  switch (kind) {
    case "dialogue":
      return { kind, character: ctx.characters[0] ?? null, text: "" };
    case "background":
      return { kind, image: ctx.backgrounds[0] ?? ["bg", "black"], transition: null };
    case "show":
      return { kind, image: [ctx.characters[0] ?? "someone"], position: "center", transition: null };
    case "hide":
      return { kind, character: ctx.characters[0] ?? "someone", transition: null };
    case "audio":
      return { kind, channel: "music", asset: ctx.music[0] ?? "", looped: true, fadeIn: null };
    case "stopAudio":
      return { kind, channel: "music", fadeOut: 1 };
    case "pause":
      return { kind, seconds: 1 };
    case "choice":
      return { kind, options: [{ line: 0, text: "Option", condition: null, blocks: [] }] };
    case "goTo":
    case "call":
      return { kind, scene: ctx.scenes[0] ?? "start" };
    case "setVariable":
      return { kind, name: "points", op: "add", value: "1" };
    case "if":
      return { kind, branches: [{ line: 0, condition: "points > 0", blocks: [] }] };
    case "end":
      return { kind };
    case "code":
      return { kind, text: "pass" };
  }
}

export const BLOCK_MENU: { kind: BlockKind["kind"]; label: string; keywords: string[] }[] = [
  { kind: "dialogue", label: "Character says…", keywords: ["line", "talk", "dialogue", "narration"] },
  { kind: "background", label: "Change background", keywords: ["scene", "bg", "location"] },
  { kind: "show", label: "Show character", keywords: ["sprite", "enter", "expression"] },
  { kind: "hide", label: "Hide character", keywords: ["leave", "exit"] },
  { kind: "audio", label: "Play music or sound", keywords: ["music", "sound", "voice", "audio"] },
  { kind: "stopAudio", label: "Stop audio", keywords: ["silence", "fade"] },
  { kind: "pause", label: "Pause", keywords: ["wait"] },
  { kind: "choice", label: "Choice", keywords: ["menu", "branch", "option"] },
  { kind: "goTo", label: "Go to scene", keywords: ["jump", "next"] },
  { kind: "call", label: "Visit scene and come back", keywords: ["call"] },
  { kind: "setVariable", label: "Change a variable", keywords: ["points", "affection", "flag"] },
  { kind: "if", label: "Only if…", keywords: ["condition", "if", "check"] },
  { kind: "end", label: "End scene", keywords: ["return", "stop"] },
  { kind: "code", label: "Code", keywords: ["script", "advanced"] },
];

export { strip as stripBlock };

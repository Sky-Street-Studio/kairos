import type { SourceFile } from "./types";

export type SetOp = "set" | "add" | "subtract" | "toggle";

export interface ChoiceOption {
  line: number;
  text: string;
  condition: string | null;
  blocks: Block[];
}

export interface IfBranch {
  line: number;
  condition: string | null;
  blocks: Block[];
}

export type BlockKind =
  | { kind: "dialogue"; character: string | null; text: string }
  | { kind: "background"; image: string[]; transition: string | null }
  | { kind: "show"; image: string[]; position: string | null; transition: string | null }
  | { kind: "hide"; character: string; transition: string | null }
  | { kind: "audio"; channel: string; asset: string; looped: boolean; fadeIn: number | null }
  | { kind: "stopAudio"; channel: string; fadeOut: number | null }
  | { kind: "pause"; seconds: number | null }
  | { kind: "choice"; options: ChoiceOption[] }
  | { kind: "goTo"; scene: string }
  | { kind: "call"; scene: string }
  | { kind: "setVariable"; name: string; op: SetOp; value: string }
  | { kind: "if"; branches: IfBranch[] }
  | { kind: "end" }
  | { kind: "code"; text: string };

export type Block = BlockKind & { line: number; endLine: number };

export interface Scene {
  label: string;
  line: number;
  blocks: Block[];
}

export type Place = { at: "after"; line: number } | { at: "start"; header: number };

export type EditOp =
  | { op: "insert"; path: string; place: Place; block: BlockKind }
  | { op: "replace"; path: string; line: number; block: BlockKind }
  | { op: "move"; path: string; line: number; place: Place }
  | { op: "delete"; path: string; line: number }
  | { op: "setChoiceText"; path: string; line: number; text: string }
  | { op: "setChoiceCondition"; path: string; line: number; condition: string | null }
  | { op: "addChoiceOption"; path: string; line: number; text: string }
  | { op: "setBranchCondition"; path: string; line: number; condition: string }
  | { op: "addBranch"; path: string; line: number; condition: string | null }
  | { op: "addScene"; path: string; name: string; after: string | null }
  | { op: "renameScene"; from: string; to: string }
  | { op: "deleteScene"; label: string; removeReferences: boolean }
  | { op: "duplicateScene"; label: string; name: string }
  | { op: "connect"; from: string; to: string; choice: string | null }
  | { op: "removeLink"; from: string; to: string; choice: string | null }
  | { op: "endScene"; label: string }
  | { op: "setCharacter"; path: string; id: string; name: string; color: string | null };

export interface Applied {
  files: SourceFile[];
  focus: { path: string; line: number } | null;
}

export interface Reference {
  path: string;
  line: number;
  kind: "jump" | "call";
  choice: string | null;
}

export interface EditorLayout {
  graph: Record<string, { x: number; y: number }>;
}

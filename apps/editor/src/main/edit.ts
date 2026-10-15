import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { Applied, EditOp, EditorLayout, Reference, Scene } from "@shared/edit-types";
import type { SourceFile } from "@shared/types";

import { napi } from "./analysis";
import { within } from "./paths";

export function sceneBlocks(source: string, label: string): Scene | null {
  return JSON.parse(napi().sceneBlocks(source, label)) as Scene | null;
}

export function applyEdits(files: SourceFile[], ops: EditOp[]): Applied {
  const result = JSON.parse(napi().applyEdits(JSON.stringify(files), JSON.stringify(ops))) as
    | { ok: Applied }
    | { error: string };
  if ("error" in result) throw new Error(result.error);
  return result.ok;
}

export function references(files: SourceFile[], label: string): Reference[] {
  return JSON.parse(napi().references(JSON.stringify(files), label)) as Reference[];
}

export function labelFromName(name: string, taken: string[]): string {
  return napi().labelFromName(name, JSON.stringify(taken));
}

export async function readLayout(root: string): Promise<EditorLayout> {
  const raw = await readFile(within(root, ".kairos/editor.json"), "utf8").catch(() => null);
  if (!raw) return { graph: {} };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return { graph: {} };
    const graph = (parsed as { graph?: unknown }).graph;
    return typeof graph === "object" && graph !== null && !Array.isArray(graph)
      ? ({ graph } as EditorLayout)
      : { graph: {} };
  } catch {
    return { graph: {} };
  }
}

export async function writeLayout(root: string, layout: EditorLayout): Promise<void> {
  const path = within(root, ".kairos/editor.json");
  await mkdir(join(root, ".kairos"), { recursive: true });
  await writeFile(path, `${JSON.stringify(layout, null, 2)}\n`, "utf8");
}

/** Edits the `start` line in place so the rest of `kairos.toml` keeps its formatting. */
export async function setStartLabel(root: string, label: string): Promise<void> {
  const path = within(root, "kairos.toml");
  const text = await readFile(path, "utf8");
  const line = `start = ${JSON.stringify(label)}`;
  const lineEnding = text.includes("\r\n") ? "\r\n" : "\n";
  const startLine = /^start\s*=.*(?=\r?$)/m;
  const nameLine = /^(name\s*=.*?)(\r?\n|$)/m;
  const next = startLine.test(text)
    ? text.replace(startLine, line)
    : nameLine.test(text)
      ? text.replace(
          nameLine,
          (_, name, ending: string) => `${name}${ending || lineEnding}${line}${ending || lineEnding}`,
        )
      : `${line}${lineEnding}${text}`;
  if (next === text) throw new Error("The start scene is already set to this label.");
  await writeFile(path, next, "utf8");
}

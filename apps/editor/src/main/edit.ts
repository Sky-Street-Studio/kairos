import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { Applied, EditOp, EditorLayout, Reference, Scene } from "@shared/edit-types";
import type { SourceFile } from "@shared/types";

import { napi } from "./analysis";

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
  const raw = await readFile(join(root, ".kairos/editor.json"), "utf8").catch(() => null);
  const parsed = raw ? (JSON.parse(raw) as Partial<EditorLayout>) : {};
  return { graph: parsed.graph ?? {} };
}

export async function writeLayout(root: string, layout: EditorLayout): Promise<void> {
  await mkdir(join(root, ".kairos"), { recursive: true });
  await writeFile(join(root, ".kairos/editor.json"), `${JSON.stringify(layout, null, 2)}\n`, "utf8");
}

/** Edits the `start` line in place so the rest of `kairos.toml` keeps its formatting. */
export async function setStartLabel(root: string, label: string): Promise<void> {
  const path = join(root, "kairos.toml");
  const text = await readFile(path, "utf8");
  const line = `start = ${JSON.stringify(label)}`;
  const next = /^start\s*=.*$/m.test(text)
    ? text.replace(/^start\s*=.*$/m, line)
    : text.replace(/^(name\s*=.*\n)/m, `$1${line}\n`);
  await writeFile(path, next, "utf8");
}

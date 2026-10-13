import { createRequire } from "node:module";
import { join } from "node:path";

import { app } from "electron";

import type { Analysis, AnalyzeInput } from "@shared/types";

interface Addon {
  analyze(input: AnalyzeInput): Analysis;
  exportGame(
    project: string,
    target: "web" | "desktop" | "windows-x64" | "linux-x64" | "macos",
    output: string,
    playerBinary?: string,
    playerPkg?: string,
  ): string;
  sceneBlocks(source: string, label: string): string;
  applyEdits(files: string, ops: string): string;
  references(files: string, label: string): string;
  labelFromName(name: string, taken: string): string;
}

let addon: Addon | undefined;

/** The packaged app ships the binding under Resources, outside node_modules. */
export function napi(): Addon {
  addon ??= createRequire(import.meta.url)(
    app.isPackaged ? join(process.resourcesPath, "kairos-napi", "index.js") : "@kairos/napi",
  ) as Addon;
  return addon;
}

export function analyze(input: AnalyzeInput): Analysis {
  return napi().analyze(input);
}

import { createRequire } from "node:module";
import { join } from "node:path";

import { app } from "electron";

import type { Analysis, Outline, SourceFile, StoryGraph } from "@shared/types";

interface Addon {
  checkProject(files: SourceFile[]): { diagnostics: Analysis["diagnostics"] };
  outline(source: string): Outline;
  storyGraph(files: SourceFile[]): StoryGraph;
  exportGame(
    project: string,
    target: "web" | "desktop",
    output: string,
    playerBinary?: string,
    playerPkg?: string,
  ): string;
}

let addon: Addon | undefined;

/** The packaged app ships the binding under Resources, outside node_modules. */
export function napi(): Addon {
  addon ??= createRequire(import.meta.url)(
    app.isPackaged ? join(process.resourcesPath, "kairos-napi", "index.js") : "@kairos/napi",
  ) as Addon;
  return addon;
}

export function analyze(files: SourceFile[]): Analysis {
  const k = napi();
  return {
    diagnostics: k.checkProject(files).diagnostics,
    outlines: Object.fromEntries(files.map((f) => [f.path, k.outline(f.source)])),
    graph: k.storyGraph(files),
  };
}

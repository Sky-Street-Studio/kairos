import { createRequire } from "node:module";

import type { Analysis, Outline, SourceFile, StoryGraph } from "@shared/types";

interface Addon {
  checkProject(files: SourceFile[]): { diagnostics: Analysis["diagnostics"] };
  outline(source: string): Outline;
  storyGraph(files: SourceFile[]): StoryGraph;
}

let addon: Addon | undefined;

function load(): Addon {
  addon ??= createRequire(import.meta.url)("@kairos/napi") as Addon;
  return addon;
}

export function analyze(files: SourceFile[]): Analysis {
  const k = load();
  return {
    diagnostics: k.checkProject(files).diagnostics,
    outlines: Object.fromEntries(files.map((f) => [f.path, k.outline(f.source)])),
    graph: k.storyGraph(files),
  };
}

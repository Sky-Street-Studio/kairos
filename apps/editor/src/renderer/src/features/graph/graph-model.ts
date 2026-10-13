import type { Block, ChoiceOption, EditorLayout } from "@shared/edit-types";
import type { StoryGraph } from "@shared/types";

import { layoutGraph } from "./layout";

export function positionsFor(graph: StoryGraph, saved: EditorLayout["graph"]) {
  const auto = layoutGraph(graph);
  return new Map(graph.nodes.map((n) => [n.label, saved[n.label] ?? auto.get(n.label) ?? { x: 0, y: 0 }]));
}

export function ghostTargets(graph: StoryGraph, edgesTo: { from: string; to: string }[]): string[] {
  const known = new Set(graph.nodes.map((n) => n.label));
  return [...new Set(edgesTo.map((e) => e.to).filter((to) => !known.has(to)))];
}

export function withSelection<T extends { id: string; selected?: boolean }>(next: T[], prev: T[]): T[] {
  const selected = new Set(prev.filter((n) => n.selected).map((n) => n.id));
  return next.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n));
}

function leadsTo(blocks: Block[], scene: string): boolean {
  return blocks.some((block) => {
    if (block.kind === "goTo") return block.scene === scene;
    if (block.kind === "if") return block.branches.some((branch) => leadsTo(branch.blocks, scene));
    return false;
  });
}

/** The choice option an arrow stands for: its text matches and its body goes to `to`, at any depth. */
export function findChoiceOption(blocks: Block[], text: string, to: string): ChoiceOption | null {
  for (const block of blocks) {
    if (block.kind === "choice") {
      const option = block.options.find((item) => item.text === text && leadsTo(item.blocks, to));
      if (option) return option;
    }
    const children =
      block.kind === "choice"
        ? block.options.map((option) => option.blocks)
        : block.kind === "if"
          ? block.branches.map((branch) => branch.blocks)
          : [];
    for (const child of children) {
      const found = findChoiceOption(child, text, to);
      if (found) return found;
    }
  }
  return null;
}

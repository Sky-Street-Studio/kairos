import type { EditorLayout } from "@shared/edit-types";
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

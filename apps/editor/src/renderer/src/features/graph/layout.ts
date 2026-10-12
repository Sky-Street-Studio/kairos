import dagre from "@dagrejs/dagre";

import type { GraphEdge, StoryGraph } from "@shared/types";

export const NODE_WIDTH = 188;
export const NODE_HEIGHT = 58;

export interface PositionedNode {
  id: string;
  x: number;
  y: number;
}

/** Top-to-bottom layered layout. Fallthrough edges get extra weight so linear chapters stack straight. */
export function layoutGraph(graph: StoryGraph): Map<string, PositionedNode> {
  const g = new dagre.graphlib.Graph({ multigraph: true });
  g.setGraph({ rankdir: "TB", nodesep: 72, ranksep: 84, marginx: 24, marginy: 24 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const n of graph.nodes) g.setNode(n.label, { width: NODE_WIDTH, height: NODE_HEIGHT });
  graph.edges.forEach((e, i) => {
    if (e.from !== e.to) g.setEdge(e.from, e.to, { weight: e.kind === "fallthrough" ? 3 : 1, minlen: 1 }, String(i));
  });
  dagre.layout(g);
  const out = new Map<string, PositionedNode>();
  for (const n of graph.nodes) {
    const p = g.node(n.label);
    out.set(n.label, { id: n.label, x: (p?.x ?? 0) - NODE_WIDTH / 2, y: (p?.y ?? 0) - NODE_HEIGHT / 2 });
  }
  return out;
}

export function edgeId(e: GraphEdge, index: number): string {
  return `${e.from}->${e.to}:${e.kind}:${index}`;
}

/** Collapses edges that share endpoints and kind; choice texts are joined so the label stays readable. */
export function mergeEdges(edges: GraphEdge[]): GraphEdge[] {
  const merged = new Map<string, GraphEdge>();
  for (const e of edges) {
    const key = `${e.from}\0${e.to}\0${e.kind}`;
    const prev = merged.get(key);
    if (!prev) merged.set(key, { ...e });
    else if (e.text && prev.text !== e.text) prev.text = prev.text ? `${prev.text} / ${e.text}` : e.text;
  }
  return [...merged.values()];
}

import { describe, expect, it } from "vitest";

import type { StoryGraph } from "@shared/types";

import { NODE_HEIGHT, layoutGraph, mergeEdges } from "./layout";

const graph: StoryGraph = {
  nodes: [
    { label: "start", path: "a.kai", line: 1 },
    { label: "about", path: "a.kai", line: 5 },
    { label: "end", path: "a.kai", line: 9 },
  ],
  edges: [
    { from: "start", to: "about", kind: "choice", text: "Tell me" },
    { from: "start", to: "about", kind: "choice", text: "More" },
    { from: "about", to: "end", kind: "jump" },
    { from: "end", to: "end", kind: "jump" },
  ],
};

describe("graph layout", () => {
  it("places every node in ranks that follow the edges", () => {
    const pos = layoutGraph(graph);
    expect([...pos.keys()]).toEqual(["start", "about", "end"]);
    expect(pos.get("about")!.y).toBeGreaterThanOrEqual(pos.get("start")!.y + NODE_HEIGHT);
    expect(pos.get("end")!.y).toBeGreaterThan(pos.get("about")!.y);
  });

  it("merges parallel edges and joins their choice text", () => {
    expect(mergeEdges(graph.edges)).toEqual([
      { from: "start", to: "about", kind: "choice", text: "Tell me / More" },
      { from: "about", to: "end", kind: "jump" },
      { from: "end", to: "end", kind: "jump" },
    ]);
  });
});

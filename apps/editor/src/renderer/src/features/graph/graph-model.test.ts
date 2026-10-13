import { describe, expect, it } from "vitest";

import { ghostTargets, positionsFor, withSelection } from "./graph-model";

const graph = {
  nodes: [
    { label: "start", path: "a.kai", line: 1 },
    { label: "end", path: "a.kai", line: 5 },
  ],
  edges: [{ from: "start", to: "end", kind: "jump" as const }],
  missing: [],
};

describe("graph positions", () => {
  it("keeps saved positions and lays out the rest", () => {
    const p = positionsFor(graph, { start: { x: 500, y: 40 } });
    expect(p.get("start")).toEqual({ x: 500, y: 40 });
    expect(p.get("end")).toBeDefined();
    expect(p.get("end")).not.toEqual({ x: 500, y: 40 });
  });
});

describe("ghost targets", () => {
  it("lists each missing label once", () => {
    const missing = [
      { from: "start", to: "nowhere" },
      { from: "end", to: "nowhere" },
      { from: "end", to: "lost" },
    ];
    expect(ghostTargets(graph, missing)).toEqual(["nowhere", "lost"]);
  });

  it("ignores targets that exist", () => {
    expect(ghostTargets(graph, [{ from: "start", to: "end" }])).toEqual([]);
  });
});

describe("selection carry-over", () => {
  it("keeps selected flags by id", () => {
    const prev = [{ id: "a", selected: true }, { id: "b" }];
    const next = [{ id: "a" }, { id: "b" }, { id: "c" }];
    expect(withSelection(next, prev)).toEqual([{ id: "a", selected: true }, { id: "b" }, { id: "c" }]);
  });
});

import { describe, expect, it } from "vitest";

import type { Block } from "@shared/edit-types";

import { findChoiceOption, ghostTargets, positionsFor, withSelection } from "./graph-model";

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

describe("choice lookup for renaming an arrow", () => {
  const goTo = (scene: string, line: number): Block => ({ kind: "goTo", scene, line, endLine: line });
  const blocks: Block[] = [
    {
      kind: "choice",
      line: 2,
      endLine: 5,
      options: [
        { line: 3, text: "Swim", condition: null, blocks: [goTo("sea", 4)] },
        { line: 5, text: "Swim", condition: null, blocks: [goTo("pool", 6)] },
      ],
    },
    {
      kind: "if",
      line: 7,
      endLine: 14,
      branches: [
        {
          line: 7,
          condition: "night",
          blocks: [
            {
              kind: "choice",
              line: 8,
              endLine: 14,
              options: [
                {
                  line: 9,
                  text: "Sleep",
                  condition: null,
                  blocks: [
                    {
                      kind: "if",
                      line: 10,
                      endLine: 11,
                      branches: [{ line: 10, condition: "tired", blocks: [goTo("bed", 11)] }],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
  ];

  it("finds the option with that text that leads to the target, however deeply nested", () => {
    expect(findChoiceOption(blocks, "Swim", "pool")?.line).toBe(5);
    expect(findChoiceOption(blocks, "Swim", "sea")?.line).toBe(3);
    expect(findChoiceOption(blocks, "Sleep", "bed")?.line).toBe(9);
    expect(findChoiceOption(blocks, "Sleep", "sea")).toBeNull();
  });
});

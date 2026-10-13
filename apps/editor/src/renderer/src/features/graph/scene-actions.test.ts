import { describe, expect, it } from "vitest";

import { deleteSummary, edgeOps, startAfterRename } from "./scene-actions";

describe("delete confirmation", () => {
  it("counts links in plain words", () => {
    expect(deleteSummary([])).toBe("Nothing else leads here.");
    expect(
      deleteSummary([
        { path: "a", line: 1, kind: "jump", choice: "Go" },
        { path: "a", line: 2, kind: "jump", choice: "Run" },
        { path: "a", line: 3, kind: "jump", choice: null },
        { path: "a", line: 4, kind: "call", choice: null },
      ]),
    ).toBe("2 choices, 1 jump and 1 call lead here.");
  });
});

describe("story arrow actions", () => {
  it("maps arrow actions to edits", () => {
    expect(edgeOps({ from: "a", to: "b", kind: "choice", text: "Go" }, "delete")).toEqual([
      { op: "removeLink", from: "a", to: "b", choice: "Go" },
    ]);
    expect(edgeOps({ from: "a", to: "b", kind: "jump", text: null }, "delete")).toEqual([
      { op: "removeLink", from: "a", to: "b", choice: null },
    ]);
    expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "makeExplicit")).toEqual([
      { op: "connect", from: "a", to: "b", choice: null },
    ]);
    expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "endHere")).toEqual([
      { op: "endScene", label: "a" },
    ]);
    expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "delete")).toEqual([
      { op: "endScene", label: "a" },
    ]);
  });
});

describe("start scene after a rename", () => {
  it("follows the renamed start scene and ignores everything else", () => {
    expect(startAfterRename("start", "start", "intro")).toBe("intro");
    expect(startAfterRename("start", "beach", "shore")).toBeNull();
    expect(startAfterRename("start", "start", "start")).toBeNull();
    expect(startAfterRename(undefined, "start", "intro")).toBeNull();
  });
});

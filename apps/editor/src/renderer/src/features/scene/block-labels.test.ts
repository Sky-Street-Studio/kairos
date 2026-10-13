import { describe, expect, it } from "vitest";

import { blockDiagnostics, blockTitle, variableSummary } from "./block-labels";

const names = { e: "Eileen" };

describe("block titles", () => {
  it("uses plain words", () => {
    expect(blockTitle({ kind: "dialogue", character: "e", text: "Hi", line: 2, endLine: 2 }, names)).toBe(
      "Eileen says",
    );
    expect(blockTitle({ kind: "dialogue", character: null, text: "Hi", line: 2, endLine: 2 }, names)).toBe("Narrator");
    expect(
      blockTitle({ kind: "background", image: ["bg", "room"], transition: null, line: 2, endLine: 2 }, names),
    ).toBe("Change background");
    expect(
      blockTitle(
        { kind: "show", image: ["e", "happy"], position: "left", transition: null, line: 2, endLine: 2 },
        names,
      ),
    ).toBe("Show Eileen");
    expect(blockTitle({ kind: "setVariable", name: "n", op: "add", value: "1", line: 2, endLine: 2 }, names)).toBe(
      "Change n",
    );
    expect(blockTitle({ kind: "code", text: "with fade", line: 2, endLine: 2 }, names)).toBe("Code");
  });

  it("attaches diagnostics to the block whose lines contain them", () => {
    const block = { kind: "code" as const, text: "x", line: 4, endLine: 6 };
    const diags = [
      { path: "a.kai", line: 5, message: "bad" },
      { path: "a.kai", line: 7, message: "other" },
      { path: "b.kai", line: 5, message: "elsewhere" },
    ];
    expect(blockDiagnostics(block, diags, "a.kai").map((d) => d.message)).toEqual(["bad"]);
  });
});

describe("nested diagnostics", () => {
  it("counts a problem once, on the innermost block that holds it", () => {
    const child = { kind: "dialogue" as const, character: null, text: "x", line: 4, endLine: 4 };
    const choice = {
      kind: "choice" as const,
      line: 2,
      endLine: 4,
      options: [{ line: 3, text: "Go", condition: "bad +", blocks: [child] }],
    };
    const diags = [
      { path: "a.kai", line: 3, message: "bad condition" },
      { path: "a.kai", line: 4, message: "bad line" },
    ];
    expect(blockDiagnostics(choice, diags, "a.kai").map((d) => d.message)).toEqual(["bad condition"]);
    expect(blockDiagnostics(child, diags, "a.kai").map((d) => d.message)).toEqual(["bad line"]);
  });
});

describe("variable summaries", () => {
  it("reads as plain words", () => {
    const base = { kind: "setVariable" as const, line: 2, endLine: 2, name: "points" };
    expect(variableSummary({ ...base, op: "set", value: "3" })).toBe("Set points to 3");
    expect(variableSummary({ ...base, op: "add", value: "1" })).toBe("Increase points by 1");
    expect(variableSummary({ ...base, op: "subtract", value: "2" })).toBe("Decrease points by 2");
    expect(variableSummary({ ...base, op: "toggle", value: "" })).toBe("Flip points");
  });
});

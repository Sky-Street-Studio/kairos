import { describe, expect, it } from "vitest";

import { dropPlace } from "./drop-target";

const rect = (top: number) => ({ top, bottom: top + 20, height: 20 }) as DOMRect;

const rows = [
  { line: 2, endLine: 2, depth: 0, parentHeader: 1 },
  { line: 3, endLine: 3, depth: 0, parentHeader: 1 },
];

describe("drop target", () => {
  it("drops before the first row at the start of the body", () => {
    expect(dropPlace(rows, 2, [rect(0), rect(20)])).toEqual({ at: "start", header: 1 });
  });

  it("drops after a row when below its middle", () => {
    expect(dropPlace(rows, 15, [rect(0), rect(20)])).toEqual({ at: "after", line: 2 });
    expect(dropPlace(rows, 25, [rect(0), rect(20)])).toEqual({ at: "after", line: 2 });
    expect(dropPlace(rows, 35, [rect(0), rect(20)])).toEqual({ at: "after", line: 3 });
  });

  it("uses the preceding sibling when entering later in a body", () => {
    expect(dropPlace(rows, 22, [rect(0), rect(20)])).toEqual({ at: "after", line: 2 });
  });

  it("uses the preceding row in the same body when nested rows intervene", () => {
    const nested = [
      { line: 2, endLine: 5, depth: 0, parentHeader: 1 },
      { line: 3, endLine: 3, depth: 1, parentHeader: 2 },
      { line: 6, endLine: 6, depth: 0, parentHeader: 1 },
    ];
    expect(dropPlace(nested, 42, [rect(0), rect(20), rect(40)])).toEqual({ at: "after", line: 2 });
  });

  it("drops after the last row below the list and returns null for an empty list", () => {
    expect(dropPlace(rows, 100, [rect(0), rect(20)])).toEqual({ at: "after", line: 3 });
    expect(dropPlace([], 0, [])).toBeNull();
  });
});

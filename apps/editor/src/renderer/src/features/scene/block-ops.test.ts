import { describe, expect, it } from "vitest";

import type { BlockKind } from "@shared/edit-types";

import { BLOCK_MENU, defaultBlock, fieldOp, insertOp } from "./block-ops";

const ctx = { characters: ["e"], backgrounds: [["bg", "room"]], music: ["theme.ogg"], scenes: ["start", "end"] };

describe("block ops", () => {
  it("builds replace ops for leaf blocks with the patched fields", () => {
    const block = { kind: "dialogue" as const, character: "e", text: "Hi", line: 3, endLine: 3 };
    expect(fieldOp("a.kai", block, { text: "Hello" })).toEqual({
      op: "replace",
      path: "a.kai",
      line: 3,
      block: { kind: "dialogue", character: "e", text: "Hello" },
    });
  });

  it("inserts after a block or at the start of a body", () => {
    expect(insertOp("a.kai", { at: "after", line: 3 }, defaultBlock("dialogue", ctx))).toEqual({
      op: "insert",
      path: "a.kai",
      place: { at: "after", line: 3 },
      block: { kind: "dialogue", character: "e", text: "" },
    });
    expect(insertOp("a.kai", { at: "start", header: 2 }, defaultBlock("end", ctx))).toEqual({
      op: "insert",
      path: "a.kai",
      place: { at: "start", header: 2 },
      block: { kind: "end" },
    });
  });

  it("gives every block kind a valid default", () => {
    const kinds: BlockKind["kind"][] = [
      "dialogue",
      "background",
      "show",
      "hide",
      "audio",
      "stopAudio",
      "pause",
      "choice",
      "goTo",
      "call",
      "setVariable",
      "if",
      "end",
      "code",
    ];
    expect(kinds.map((kind) => defaultBlock(kind, ctx))).toEqual([
      { kind: "dialogue", character: "e", text: "" },
      { kind: "background", image: ["bg", "room"], transition: null },
      { kind: "show", image: ["e"], position: "center", transition: null },
      { kind: "hide", character: "e", transition: null },
      { kind: "audio", channel: "music", asset: "theme.ogg", looped: true, fadeIn: null },
      { kind: "stopAudio", channel: "music", fadeOut: 1 },
      { kind: "pause", seconds: 1 },
      { kind: "choice", options: [{ line: 0, text: "Option", condition: null, blocks: [] }] },
      { kind: "goTo", scene: "start" },
      { kind: "call", scene: "start" },
      { kind: "setVariable", name: "points", op: "add", value: "1" },
      { kind: "if", branches: [{ line: 0, condition: "points > 0", blocks: [] }] },
      { kind: "end" },
      { kind: "code", text: "pass" },
    ]);
  });

  it("provides a searchable menu entry for every block kind", () => {
    expect(BLOCK_MENU.map(({ kind }) => kind)).toEqual([
      "dialogue",
      "background",
      "show",
      "hide",
      "audio",
      "stopAudio",
      "pause",
      "choice",
      "goTo",
      "call",
      "setVariable",
      "if",
      "end",
      "code",
    ]);
    expect(BLOCK_MENU.every(({ label, keywords }) => label.length > 0 && keywords.length > 0)).toBe(true);
  });
});

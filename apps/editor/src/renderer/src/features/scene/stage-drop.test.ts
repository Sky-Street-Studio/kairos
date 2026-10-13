import { describe, expect, it } from "vitest";

import { assetToBlock, showBlockAt, showOpFor } from "./stage-drop";

const blocks = [
  { kind: "background" as const, image: ["bg", "room"], transition: null, line: 2, endLine: 2 },
  { kind: "show" as const, image: ["e", "happy"], position: "left", transition: null, line: 3, endLine: 3 },
  { kind: "dialogue" as const, character: "e", text: "Hi", line: 4, endLine: 4 },
];

describe("stage drops", () => {
  it("turns assets into blocks", () => {
    expect(
      assetToBlock(
        { path: "assets/images/bg/bg beach.png", kind: "image", name: "bg beach", category: "background" },
        0.5,
      ),
    ).toEqual({ kind: "background", image: ["bg", "beach"], transition: null });
    expect(
      assetToBlock(
        {
          path: "assets/images/sprites/e/e sad.png",
          kind: "image",
          name: "e sad",
          category: "sprite",
          characterId: "e",
        },
        0.9,
      ),
    ).toEqual({ kind: "show", image: ["e", "sad"], position: "right", transition: null });
    expect(
      assetToBlock(
        { path: "assets/audio/music/theme.ogg", kind: "audio", name: "music/theme.ogg", category: "music" },
        0,
      ),
    ).toEqual({ kind: "audio", channel: "music", asset: "music/theme.ogg", looped: true, fadeIn: null });
  });

  it("updates the character's latest show block instead of adding another", () => {
    expect(showOpFor(blocks, 4, "e", ["e", "sad"], "right", "a.kai")).toEqual({
      op: "replace",
      path: "a.kai",
      line: 3,
      block: { kind: "show", image: ["e", "sad"], position: "right", transition: null },
    });
    expect(showOpFor(blocks, 4, "k", ["k"], "center", "a.kai")).toEqual({
      op: "insert",
      path: "a.kai",
      place: { at: "after", line: 4 },
      block: { kind: "show", image: ["k"], position: "center", transition: null },
    });
  });

  it("finds the show block that placed a clicked character", () => {
    expect(showBlockAt(blocks, 4, "e")?.line).toBe(3);
    expect(showBlockAt(blocks, 2, "e")).toBeNull();
  });
});

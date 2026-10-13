import { describe, expect, it } from "vitest";

import { assetPathFromUrl, assetUrl, classifyAsset } from "./assets";
import {
  addCharacter,
  formatCharacter,
  insertJump,
  insertStatement,
  setCharacterLine,
  stagePosition,
  statementForAsset,
  validateCharacter,
} from "./edits";

const script = `character e "Eileen"

label start:
    e "Hi."
    menu:
        "Go":
            jump about

    # trailing comment
label about:
    "About."
`;

describe("insertJump", () => {
  it("appends to the end of the label body at body indentation", () => {
    const r = insertJump(script, "start", "about")!;
    expect(r.changed).toBe(true);
    expect(r.source.split("\n").slice(3, 9)).toEqual([
      '    e "Hi."',
      "    menu:",
      '        "Go":',
      "            jump about",
      "    jump about",
      "",
    ]);
  });

  it("appends after the last line of the final label", () => {
    expect(insertJump(script, "about", "start")!.source.endsWith('    "About."\n    jump start\n')).toBe(true);
  });

  it("does not duplicate an existing trailing jump", () => {
    const once = insertJump(script, "about", "start")!.source;
    expect(insertJump(once, "about", "start")).toEqual({ source: once, changed: false });
  });

  it("indents an empty label body one level", () => {
    expect(insertJump("label a:\nlabel b:\n    return\n", "a", "b")!.source).toBe(
      "label a:\n    jump b\nlabel b:\n    return\n",
    );
  });

  it("returns null for unknown labels and keeps CRLF", () => {
    expect(insertJump(script, "nope", "about")).toBeNull();
    expect(insertJump("label a:\r\n    pass\r\n", "a", "a")!.source).toBe("label a:\r\n    pass\r\n    jump a\r\n");
  });
});

describe("characters", () => {
  it("formats and escapes", () => {
    expect(formatCharacter({ id: "e", name: 'Ei "E"', color: "#c8ffc8" })).toBe(
      'character e "Ei \\"E\\"" color="#c8ffc8"',
    );
    expect(formatCharacter({ id: "k", name: "Kai" })).toBe('character k "Kai"');
  });

  it("rewrites a declaration in place and keeps comments", () => {
    const src = 'character e "Eileen"  # lead\nlabel a:\n    pass\n';
    expect(setCharacterLine(src, 1, { id: "e", name: "Eileen", color: "#ffffff" })).toBe(
      'character e "Eileen" color="#ffffff"  # lead\nlabel a:\n    pass\n',
    );
    expect(() => setCharacterLine(src, 2, { id: "e", name: "E" })).toThrow();
  });

  it("adds after the last declaration", () => {
    expect(addCharacter(script, { id: "k", name: "Kai" }).split("\n").slice(0, 2)).toEqual([
      'character e "Eileen"',
      'character k "Kai"',
    ]);
    expect(addCharacter("label a:\n    pass\n", { id: "k", name: "Kai" })).toBe(
      'character k "Kai"\nlabel a:\n    pass\n',
    );
  });

  it("validates fields", () => {
    expect(validateCharacter({ id: "1x", name: "A" })).toMatch(/id/);
    expect(validateCharacter({ id: "x", name: " " })).toMatch(/name/);
    expect(validateCharacter({ id: "x", name: "A", color: "red" })).toMatch(/#/);
    expect(validateCharacter({ id: "x", name: "A", color: "#aabbcc" })).toBeNull();
  });
});

describe("stage composer", () => {
  const image = (name: string) => classifyAsset(`assets/images/${name}.png`);

  it("turns bg images into scenes and others into shows by drop position", () => {
    expect(statementForAsset(image("bg room"), 0.9)).toBe("scene bg room");
    expect(statementForAsset(classifyAsset("assets/images/bg/room.png"), 0.9)).toBe("scene room");
    expect(statementForAsset(classifyAsset("assets/images/sprites/e/face/happy.png"), 0.1)).toBe(
      "show e happy at left",
    );
    expect(statementForAsset(image("e happy"), 0.1)).toBe("show e happy at left");
    expect(statementForAsset(image("e happy"), 0.5)).toBe("show e happy at center");
    expect(statementForAsset(image("e happy"), 0.8)).toBe("show e happy at right");
    expect(statementForAsset(classifyAsset("assets/audio/theme.ogg"), 0)).toBe('play music "theme.ogg"');
    expect(statementForAsset(classifyAsset("assets/fonts/a.ttf"), 0)).toBeNull();
    expect([0, 0.34, 0.67].map(stagePosition)).toEqual(["left", "center", "right"]);
  });

  it("inserts after the cursor line, one level deeper after a block header", () => {
    const src = "label a:\n    scene bg room\n";
    expect(insertStatement(src, 2, "show e at left")).toEqual({
      source: "label a:\n    scene bg room\n    show e at left\n",
      line: 3,
    });
    expect(insertStatement(src, 1, "scene bg x").source).toBe("label a:\n    scene bg x\n    scene bg room\n");
  });

  it("inserts on a blank line using the indentation above", () => {
    expect(insertStatement("label a:\n    pass\n\nlabel b:\n", 3, "scene bg x").source).toBe(
      "label a:\n    pass\n    scene bg x\n\nlabel b:\n",
    );
  });
});

describe("assets", () => {
  it("names assets by the player's lookup rule", () => {
    expect(classifyAsset("assets/images/bg room.webp")).toEqual({
      path: "assets/images/bg room.webp",
      kind: "image",
      category: "background",
      name: "bg room",
    });
    expect(classifyAsset("assets/audio/sfx/door.ogg").name).toBe("sfx/door.ogg");
    expect(classifyAsset("assets/images/readme.txt").kind).toBe("other");
  });

  it("round-trips protocol URLs and rejects traversal", () => {
    const url = assetUrl("assets/images/bg room.png");
    expect(url).toBe("kairos-asset://project/images/bg%20room.png");
    expect(assetPathFromUrl(url)).toBe("images/bg room.png");
    expect(assetPathFromUrl("kairos-asset://project/images/..%2F..%2Fsecret")).toBeNull();
    expect(assetPathFromUrl("kairos-asset://other/x.png")).toBeNull();
  });
});

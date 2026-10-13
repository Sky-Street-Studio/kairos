import { describe, expect, it } from "vitest";

import { classifyAsset } from "@shared/assets";

import { stagePosition, statementForAsset } from "./asset-drag";

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
});

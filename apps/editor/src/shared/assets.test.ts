import { describe, expect, it } from "vitest";

import { AUDIO_EXTENSIONS, assetPathFromUrl, assetUrl, classifyAsset } from "./assets";

describe("asset categories", () => {
  it("uses folder categories and declared character ids for fallback images", () => {
    expect(classifyAsset("assets/images/bg/room.webp").category).toBe("background");
    expect(classifyAsset("assets/images/ui/logo.png").category).toBe("ui");
    expect(classifyAsset("assets/images/e happy.png", ["e"]).category).toBe("sprite");
    expect(classifyAsset("assets/images/e happy.png", ["e"]).characterId).toBe("e");
    expect(classifyAsset("assets/images/texture.png", ["e"]).category).toBe("other");
    expect(classifyAsset("assets/audio/music/theme.ogg").category).toBe("music");
    expect(classifyAsset("assets/audio/sfx/click.wav").category).toBe("sound");
    expect(classifyAsset("assets/audio/voice/e/line.mp3").category).toBe("voice");
  });

  it("accepts only the shared audio formats", () => {
    expect(AUDIO_EXTENSIONS).toEqual(["ogg", "mp3", "wav", "flac"]);
    expect(classifyAsset("assets/audio/theme.opus").kind).toBe("other");
    expect(classifyAsset("assets/audio/theme.m4a").kind).toBe("other");
  });

  it("names an image by its file stem, and a layer part by character and attribute", () => {
    expect(classifyAsset("assets/images/sprites/e/e happy.png").name).toBe("e happy");
    expect(classifyAsset("assets/images/bg/room.png").name).toBe("room");
    expect(classifyAsset("assets/images/sprites/e/face/happy.png").name).toBe("e happy");
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

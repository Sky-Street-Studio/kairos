import { describe, expect, it } from "vitest";

import { AUDIO_EXTENSIONS, classifyAsset } from "./assets";

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

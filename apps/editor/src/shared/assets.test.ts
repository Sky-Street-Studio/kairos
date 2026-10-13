import { describe, expect, it } from "vitest";

import { AUDIO_EXTENSIONS, classifyAsset, fileKind } from "./assets";

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

describe("fileKind", () => {
  it("opens scripts, text files anywhere, and images or audio under assets", () => {
    expect(fileKind("script/script.kai")).toBe("script");
    expect(fileKind("kairos.toml")).toBe("text");
    expect(fileKind("ui/theme.toml")).toBe("text");
    expect(fileKind("assets/audio/audio.toml")).toBe("text");
    expect(fileKind("assets/images/sprites/e/layers.toml")).toBe("text");
    expect(fileKind("script/notes.md")).toBe("text");
    expect(fileKind("assets/images/bg garden.png")).toBe("image");
    expect(fileKind("assets/images/sprites/e/face/happy.WEBP")).toBe("image");
    expect(fileKind("assets/audio/theme.ogg")).toBe("audio");
  });

  it("refuses files it has no view for or cannot serve", () => {
    expect(fileKind("ui/logo.png")).toBeNull();
    expect(fileKind("assets/audio/theme.m4a")).toBeNull();
    expect(fileKind("assets/fonts/body.ttf")).toBeNull();
  });
});

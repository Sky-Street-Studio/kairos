import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { electronApp } = vi.hoisted(() => ({
  electronApp: { getAppPath: vi.fn(() => "/repo/apps/editor"), isPackaged: false },
}));

vi.mock("electron", () => ({ app: electronApp }));

import { playerPkgDir, templateDir } from "./paths";

describe("playerPkgDir", () => {
  beforeEach(() => {
    electronApp.isPackaged = false;
    delete process.env.KAIROS_PLAYER_PKG;
    Reflect.deleteProperty(process, "resourcesPath");
  });

  afterEach(() => {
    Reflect.deleteProperty(process, "resourcesPath");
  });

  it("uses the built player in the repository while developing", () => {
    expect(playerPkgDir()).toBe("/repo/crates/kairos-player/pkg");
  });

  it("uses the bundled player resource in a packaged app", () => {
    electronApp.isPackaged = true;
    Object.defineProperty(process, "resourcesPath", { configurable: true, value: "/app/Contents/Resources" });

    expect(playerPkgDir()).toBe("/app/Contents/Resources/kairos-player");
  });

  it("supports a player package override during development", () => {
    process.env.KAIROS_PLAYER_PKG = "/tmp/player";

    expect(playerPkgDir()).toBe("/tmp/player");
  });
});

describe("templateDir", () => {
  afterEach(() => {
    electronApp.isPackaged = false;
    Reflect.deleteProperty(process, "resourcesPath");
  });

  it("uses the bundled template in a packaged app", () => {
    electronApp.isPackaged = true;
    Object.defineProperty(process, "resourcesPath", { configurable: true, value: "/app/Contents/Resources" });

    expect(templateDir()).toBe("/app/Contents/Resources/template");
  });
});

import { mkdtemp, mkdir, realpath, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { electronApp } = vi.hoisted(() => ({
  electronApp: { getAppPath: vi.fn(() => "/repo/apps/editor"), isPackaged: false },
}));

vi.mock("electron", () => ({ app: electronApp }));

import { playerPkgDir, templateDir, within } from "./paths";

let temp: string | undefined;

afterEach(async () => {
  if (temp) await import("node:fs/promises").then(({ rm }) => rm(temp!, { recursive: true, force: true }));
  temp = undefined;
});

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

describe("within", () => {
  it("keeps paths under a symlinked root and rejects symlinks that escape it", async () => {
    temp = await mkdtemp(join(tmpdir(), "kairos-paths-"));
    const realRoot = join(temp, "project");
    const linkedRoot = join(temp, "linked-project");
    const outside = join(temp, "outside");
    await mkdir(realRoot);
    await mkdir(outside);
    await mkdir(join(realRoot, "actual"));
    await writeFile(join(outside, "secret"), "secret");
    await writeFile(join(realRoot, "actual/file"), "inside");
    await symlink(realRoot, linkedRoot);
    await symlink(join(realRoot, "actual"), join(realRoot, "alias"));
    await symlink(outside, join(realRoot, "assets"));
    await symlink(join(outside, "future"), join(realRoot, "dangling"));

    expect(within(linkedRoot, "kairos.toml")).toBe(join(await realpath(realRoot), "kairos.toml"));
    expect(within(realRoot, "alias/file")).toBe(await realpath(join(realRoot, "actual/file")));
    expect(() => within(realRoot, "assets/secret")).toThrow("outside the project");
    expect(() => within(realRoot, "dangling")).toThrow("outside the project");
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

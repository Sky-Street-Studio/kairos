import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { electronApp } = vi.hoisted(() => ({ electronApp: { getPath: vi.fn(() => "") } }));

vi.mock("electron", () => ({ app: electronApp }));

let work: string;

beforeEach(async () => {
  vi.resetModules();
  work = await mkdtemp(join(tmpdir(), "kairos-settings-"));
  process.env.KAIROS_USER_DATA = join(work, "user-data");
  electronApp.getPath.mockImplementation(() => join(work, "user-data"));
});

afterEach(async () => {
  delete process.env.KAIROS_USER_DATA;
  await rm(work, { recursive: true, force: true });
});

async function project(name: string): Promise<string> {
  const root = join(work, name);
  await mkdir(root, { recursive: true });
  await writeFile(join(root, "kairos.toml"), `name = "${name}"\n`);
  return root;
}

async function saveRecent(recentProjects: string[]): Promise<void> {
  await mkdir(join(work, "user-data"), { recursive: true });
  await writeFile(join(work, "user-data", "settings.json"), JSON.stringify({ recentProjects }));
}

describe("recent projects", () => {
  it("drops projects whose folder no longer exists when settings load", async () => {
    const kept = await project("kept");
    await saveRecent([join(work, "deleted"), kept]);
    const { getSettings } = await import("./settings");

    expect((await getSettings()).recentProjects).toEqual([kept]);
  });

  it("writes the pruned list back so the stale entries stay gone", async () => {
    const kept = await project("kept");
    await saveRecent([join(work, "deleted"), kept]);
    const { getSettings } = await import("./settings");

    await getSettings();
    const saved = JSON.parse(await readFile(join(work, "user-data", "settings.json"), "utf8"));
    expect(saved.recentProjects).toEqual([kept]);
  });

  it("drops folders that are no longer Kairos projects", async () => {
    const kept = await project("kept");
    const emptied = join(work, "emptied");
    await mkdir(emptied);
    await saveRecent([emptied, kept]);
    const { getSettings } = await import("./settings");

    expect((await getSettings()).recentProjects).toEqual([kept]);
  });
});

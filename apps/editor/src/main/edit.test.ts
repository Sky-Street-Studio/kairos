import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { readLayout, setStartLabel, writeLayout } from "./edit";

describe("editor files", () => {
  it("round-trips the graph layout and tolerates a missing file", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-layout-"));
    expect(await readLayout(root)).toEqual({ graph: {} });
    await writeLayout(root, { graph: { start: { x: 10, y: 20 } } });
    expect(await readLayout(root)).toEqual({ graph: { start: { x: 10, y: 20 } } });
  });

  it("refuses to write layout through a symlink outside the project", async () => {
    const base = await mkdtemp(join(tmpdir(), "kairos-layout-symlink-"));
    const root = join(base, "project");
    const outside = join(base, "outside");
    await mkdir(root);
    await mkdir(outside);
    await writeFile(join(outside, "editor.json"), "safe");
    await symlink(outside, join(root, ".kairos"));

    try {
      await expect(readLayout(root)).rejects.toThrow("outside the project");
      await expect(writeLayout(root, { graph: { start: { x: 10, y: 20 } } })).rejects.toThrow("outside the project");
      expect(await readFile(join(outside, "editor.json"), "utf8")).toBe("safe");
    } finally {
      await rm(base, { recursive: true, force: true });
    }
  });

  it("falls back to an empty layout for malformed or invalid JSON", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-layout-invalid-"));
    await mkdir(join(root, ".kairos"));
    await writeFile(join(root, ".kairos/editor.json"), "not json");
    expect(await readLayout(root)).toEqual({ graph: {} });
    await writeFile(join(root, ".kairos/editor.json"), '{"graph":null}');
    expect(await readLayout(root)).toEqual({ graph: {} });
    await writeFile(join(root, ".kairos/editor.json"), '{"graph":[]}');
    expect(await readLayout(root)).toEqual({ graph: {} });
  });

  it("changes only the start key in kairos.toml", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-start-"));
    await writeFile(join(root, "kairos.toml"), 'name = "P"\nstart = "start"\n\n[window]\nwidth = 1280\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe(
      'name = "P"\nstart = "beach"\n\n[window]\nwidth = 1280\n',
    );
    await writeFile(join(root, "kairos.toml"), 'name = "P"\r\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('name = "P"\r\nstart = "beach"\r\n');
    await writeFile(join(root, "kairos.toml"), "[window]\nwidth = 1280\n");
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('start = "beach"\n[window]\nwidth = 1280\n');
    await expect(setStartLabel(root, "beach")).rejects.toThrow("already set");
    await writeFile(join(root, "kairos.toml"), 'name = "P"');
    await setStartLabel(root, "start");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('name = "P"\nstart = "start"\n');
  });
});

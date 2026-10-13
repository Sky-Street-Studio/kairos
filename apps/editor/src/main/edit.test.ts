import { mkdtemp, readFile, writeFile } from "node:fs/promises";
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

  it("changes only the start key in kairos.toml", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-start-"));
    await writeFile(join(root, "kairos.toml"), 'name = "P"\nstart = "start"\n\n[window]\nwidth = 1280\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe(
      'name = "P"\nstart = "beach"\n\n[window]\nwidth = 1280\n',
    );
    await writeFile(join(root, "kairos.toml"), 'name = "P"\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('name = "P"\nstart = "beach"\n');
  });
});

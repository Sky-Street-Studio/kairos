import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { loadProject } from "./project";

describe("loadProject translations", () => {
  it("includes only top-level TOML translations in the player snapshot", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-project-"));
    try {
      await writeFile(join(root, "kairos.toml"), 'name = "Test"');
      await mkdir(join(root, "tl/nested"), { recursive: true });
      await writeFile(join(root, "tl/ja.toml"), '"id" = "Hola"');
      await writeFile(join(root, "tl/notes.txt"), "ignored");
      await writeFile(join(root, "tl/nested/fr.toml"), '"id" = "ignored"');

      const snapshot = await loadProject(root);
      expect(snapshot.translations).toEqual([{ lang: "ja", source: '"id" = "Hola"' }]);
      expect(snapshot.tree.some((node) => node.path === "tl")).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

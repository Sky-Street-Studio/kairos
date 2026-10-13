import { describe, expect, it, vi } from "vitest";

import type { Scene } from "@shared/edit-types";

import { createPendingEditRegistry, resolveBlock, resolveBranch, resolveOption } from "./pending-edits";

const scene: Scene = {
  label: "start",
  line: 1,
  blocks: [
    {
      kind: "choice",
      line: 3,
      endLine: 7,
      options: [
        {
          line: 4,
          text: "Stay",
          condition: null,
          blocks: [{ kind: "dialogue", line: 5, endLine: 5, character: null, text: "Here" }],
        },
      ],
    },
    {
      kind: "if",
      line: 9,
      endLine: 12,
      branches: [{ line: 10, condition: "ready", blocks: [] }],
    },
  ],
};

describe("pending edit helpers", () => {
  it("resolves current block, option, and branch lines from their tree positions", () => {
    expect(resolveBlock(scene.blocks, [{ index: 0, child: { kind: "choice", index: 0 } }, { index: 0 }])?.line).toBe(5);
    expect(resolveOption(scene, [{ index: 0 }], 0)?.line).toBe(4);
    expect(resolveBranch(scene, [{ index: 1 }], 0)?.line).toBe(10);
    expect(resolveBlock(scene.blocks, [{ index: 3 }])).toBeNull();
  });

  it("flushes every registered edit before the caller continues", async () => {
    const order: string[] = [];
    const registry = createPendingEditRegistry();
    const unregister = registry.register(async () => {
      await Promise.resolve();
      order.push("field");
    });
    const structuralEdit = vi.fn(() => order.push("insert"));

    await registry.flush();
    structuralEdit();
    unregister();

    expect(order).toEqual(["field", "insert"]);
    await registry.flush();
    expect(order).toHaveLength(2);
  });
});

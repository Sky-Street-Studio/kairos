import type { Block, Scene } from "@shared/edit-types";

export type BlockPosition = { index: number; child?: { kind: "choice" | "if"; index: number } }[];

export function resolveBlock(blocks: Block[], position: BlockPosition): Block | null {
  let current = blocks;
  let block: Block | undefined;
  for (const [depth, segment] of position.entries()) {
    block = current[segment.index];
    if (!block) return null;
    if (depth < position.length - 1) {
      const child = segment.child;
      if (!child) return null;
      current =
        block.kind === "choice" && child.kind === "choice"
          ? (block.options[child.index]?.blocks ?? [])
          : block.kind === "if" && child.kind === "if"
            ? (block.branches[child.index]?.blocks ?? [])
            : [];
    }
  }
  return block ?? null;
}

export function resolveOption(scene: Scene, position: BlockPosition, index: number) {
  const block = resolveBlock(scene.blocks, position);
  return block?.kind === "choice" ? (block.options[index] ?? null) : null;
}

export function resolveBranch(scene: Scene, position: BlockPosition, index: number) {
  const block = resolveBlock(scene.blocks, position);
  return block?.kind === "if" ? (block.branches[index] ?? null) : null;
}

export function createPendingEditRegistry() {
  const flushers = new Set<() => Promise<void>>();
  return {
    register(flush: () => Promise<void>) {
      flushers.add(flush);
      return () => flushers.delete(flush);
    },
    async flush() {
      await Promise.all([...flushers].map((flush) => flush()));
    },
  };
}

export const pendingEdits = createPendingEditRegistry();
export const flushPendingEdits = () => pendingEdits.flush();

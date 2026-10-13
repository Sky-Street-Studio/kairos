import type { Block, BlockKind, EditOp } from "@shared/edit-types";
import type { AssetEntry } from "@shared/types";

const stagePosition = (x: number) => (x < 1 / 3 ? "left" : x < 2 / 3 ? "center" : "right");

export function assetToBlock(asset: AssetEntry, xFraction: number): BlockKind | null {
  if (asset.kind === "image") {
    const image = asset.name.split(/\s+/);
    return asset.category === "background" || image[0] === "bg"
      ? { kind: "background", image, transition: null }
      : { kind: "show", image, position: stagePosition(xFraction), transition: null };
  }
  if (asset.kind === "audio") {
    const channel = asset.category === "sound" ? "sound" : asset.category === "voice" ? "voice" : "music";
    return { kind: "audio", channel, asset: asset.name, looped: channel === "music", fadeIn: null };
  }
  return null;
}

export function showBlockAt(blocks: Block[], selectedLine: number, character: string): Block | null {
  return (
    blocks.findLast((block) => block.line <= selectedLine && block.kind === "show" && block.image[0] === character) ??
    null
  );
}

export function showOpFor(
  blocks: Block[],
  selectedLine: number,
  character: string,
  image: string[],
  position: string,
  path: string,
): EditOp {
  const block: BlockKind = { kind: "show", image, position, transition: null };
  const existing = showBlockAt(blocks, selectedLine, character);
  return existing
    ? { op: "replace", path, line: existing.line, block }
    : { op: "insert", path, place: { at: "after", line: selectedLine }, block };
}

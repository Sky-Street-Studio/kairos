import type { DragEvent } from "react";

import type { AssetEntry } from "@shared/types";

export const ASSET_MIME = "application/x-kairos-asset";

export type StagePosition = "left" | "center" | "right";

export function stagePosition(xFraction: number): StagePosition {
  if (xFraction < 1 / 3) return "left";
  if (xFraction < 2 / 3) return "center";
  return "right";
}

const quote = (s: string) => `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;

/** The statement a dropped asset turns into, or `null` for assets scripts cannot reference. */
export function statementForAsset(asset: AssetEntry, xFraction: number): string | null {
  if (asset.kind === "image") {
    return asset.category === "background" || asset.name.split(/\s+/)[0] === "bg"
      ? `scene ${asset.name}`
      : `show ${asset.name} at ${stagePosition(xFraction)}`;
  }
  if (asset.kind === "audio") return `play music ${quote(asset.name)}`;
  return null;
}

export function startAssetDrag(e: DragEvent, asset: AssetEntry) {
  e.dataTransfer.setData(ASSET_MIME, JSON.stringify(asset));
  e.dataTransfer.setData("text/plain", asset.name);
  e.dataTransfer.effectAllowed = "copy";
}

export const isAssetDrag = (e: DragEvent) => e.dataTransfer.types.includes(ASSET_MIME);

/** The statement to insert for an asset dropped at `e`, positioned across `surface`. */
export function droppedStatement(e: DragEvent, surface: HTMLElement): string | null {
  const raw = e.dataTransfer.getData(ASSET_MIME);
  if (!raw) return null;
  const rect = surface.getBoundingClientRect();
  return statementForAsset(JSON.parse(raw) as AssetEntry, (e.clientX - rect.left) / rect.width);
}

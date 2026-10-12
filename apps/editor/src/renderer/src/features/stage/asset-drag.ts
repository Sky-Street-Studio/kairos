import type { DragEvent } from "react";

import { statementForAsset } from "@shared/edits";
import type { AssetEntry } from "@shared/types";

export const ASSET_MIME = "application/x-kairos-asset";

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

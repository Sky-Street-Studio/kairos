import { ASSET_SCHEME, type AssetEntry, type AssetKind } from "./types";

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"];
export const AUDIO_EXTENSIONS = ["ogg", "mp3", "wav", "flac", "opus", "m4a"];

const extension = (path: string) => path.slice(path.lastIndexOf(".") + 1).toLowerCase();

/**
 * Mirrors the player's lookup rule: `scene bg room` loads `assets/images/bg room.<ext>`, and
 * `play music "theme.ogg"` loads `assets/audio/theme.ogg`.
 */
export function classifyAsset(path: string): AssetEntry {
  const ext = extension(path);
  if (path.startsWith("assets/images/") && IMAGE_EXTENSIONS.includes(ext)) {
    const file = path.slice(path.lastIndexOf("/") + 1);
    return { path, kind: "image", name: file.slice(0, file.length - ext.length - 1) };
  }
  if (path.startsWith("assets/audio/") && AUDIO_EXTENSIONS.includes(ext)) {
    return { path, kind: "audio", name: path.slice("assets/audio/".length) };
  }
  return { path, kind: "other" as AssetKind, name: path.slice("assets/".length) };
}

export function assetUrl(path: string): string {
  const rel = path.replace(/^assets\//, "");
  return `${ASSET_SCHEME}://project/${rel.split("/").map(encodeURIComponent).join("/")}`;
}

/** Inverse of {@link assetUrl}; returns `null` for anything that escapes `assets/`. */
export function assetPathFromUrl(url: string): string | null {
  const parsed = new URL(url);
  if (parsed.host !== "project") return null;
  const segments = parsed.pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (segments.length === 0 || segments.some((s) => s === ".." || s === "." || s.includes("/") || s.includes("\\"))) {
    return null;
  }
  return segments.join("/");
}

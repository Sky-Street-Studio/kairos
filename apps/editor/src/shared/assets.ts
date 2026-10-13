import { ASSET_SCHEME, type AssetEntry, type AssetKind } from "./types";

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"];
export const AUDIO_EXTENSIONS = ["ogg", "mp3", "wav", "flac"];

const extension = (path: string) => path.slice(path.lastIndexOf(".") + 1).toLowerCase();
const stem = (path: string) => path.slice(path.lastIndexOf("/") + 1).replace(/\.[^.]+$/, "");
export const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function categoryFor(
  path: string,
  kind: AssetKind,
  characterIds: string[],
): Pick<AssetEntry, "category" | "characterId"> {
  const rel = path.replace(/^assets\//, "");
  if (kind === "audio") {
    if (rel.startsWith("audio/music/")) return { category: "music" };
    if (rel.startsWith("audio/sfx/")) return { category: "sound" };
    if (rel.startsWith("audio/voice/")) return { category: "voice" };
    return { category: "other" };
  }
  if (kind !== "image") return { category: "other" };
  if (rel.startsWith("images/bg/")) return { category: "background" };
  if (rel.startsWith("images/ui/")) return { category: "ui" };
  const spriteMatch = rel.match(/^images\/sprites\/([^/]+)\//);
  if (spriteMatch?.[1]) return { category: "sprite", characterId: spriteMatch[1] };
  if (stem(path).startsWith("bg ")) return { category: "background" };
  const firstWord = stem(path).split(/\s+/)[0] ?? "";
  if (characterIds.includes(firstWord)) return { category: "sprite", characterId: firstWord };
  return { category: "other" };
}

/** Mirrors the player's lookup rule while retaining the editor's folder category. */
export function classifyAsset(path: string, characterIds: string[] = []): AssetEntry {
  const ext = extension(path);
  const isImage = path.startsWith("assets/images/") && IMAGE_EXTENSIONS.includes(ext);
  const isAudio = path.startsWith("assets/audio/") && AUDIO_EXTENSIONS.includes(ext);
  const kind: AssetKind = isImage ? "image" : isAudio ? "audio" : "other";
  const layer = path.match(/^assets\/images\/sprites\/([^/]+)\/[^/]+\/[^/]+$/);
  const name = isImage
    ? layer
      ? `${layer[1]} ${stem(path)}`
      : stem(path)
    : isAudio
      ? path.slice("assets/audio/".length)
      : path.slice("assets/".length);
  return { path, kind, name, ...categoryFor(path, kind, characterIds) };
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

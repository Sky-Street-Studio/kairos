import { ASSET_SCHEME, type AssetEntry, type AssetKind, type SourceFile, type SpriteLayerConfig } from "./types";

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"];
export const AUDIO_EXTENSIONS = ["ogg", "mp3", "wav", "flac"];

const extension = (path: string) => path.slice(path.lastIndexOf(".") + 1).toLowerCase();
const stem = (path: string) => path.slice(path.lastIndexOf("/") + 1).replace(/\.[^.]+$/, "");
const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

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

export function assetUsageCount(
  asset: AssetEntry,
  scripts: SourceFile[],
  spriteLayers: Record<string, SpriteLayerConfig> = {},
): number {
  if (asset.kind === "audio") {
    const file = asset.path.slice(asset.path.lastIndexOf("/") + 1);
    const names = [asset.name, file, stem(asset.path)].map(escapeRegex).join("|");
    const reference = new RegExp(`\\b(?:play|queue)\\s+(?:music|sound|voice)\\s+"(?:${names})"`, "g");
    return scripts.reduce((total, script) => total + [...script.source.matchAll(reference)].length, 0);
  }
  if (asset.kind !== "image") return 0;
  if (asset.category === "background") {
    const value = stem(asset.path);
    const reference = new RegExp(`\\bscene\\s+${escapeRegex(value)}(?=\\s|$)`, "gm");
    return scripts.reduce((total, script) => total + [...script.source.matchAll(reference)].length, 0);
  }
  if (asset.category !== "sprite") return 0;

  const layered = asset.path.match(/^assets\/images\/sprites\/([^/]+)\/([^/]+)\/([^/]+)\.[^.]+$/);
  const config = layered ? spriteLayers[layered[1]!] : undefined;
  if (!layered || !config) {
    const reference = new RegExp(`^\\s*show\\s+${escapeRegex(stem(asset.path))}(?=\\s|$)`, "gm");
    return scripts.reduce((total, script) => total + [...script.source.matchAll(reference)].length, 0);
  }
  const [, characterId, group, attribute] = layered;
  const groupAttributes = config.groups[group!] ?? [];
  const defaultAttribute = config.defaults[group!];
  const prefix = new RegExp(`^\\s*show\\s+${escapeRegex(characterId!)}(?=\\s|$)`);
  return scripts.reduce((total, script) => {
    const count = script.source.split("\n").filter((line) => {
      if (!prefix.test(line)) return false;
      const rest = line.replace(prefix, "").split(/\s+(?:at|with)\s+/)[0] ?? "";
      const selected = rest.split(/\s+/).find((value) => groupAttributes.includes(value));
      return selected
        ? selected === attribute
        : (defaultAttribute ?? (groupAttributes.length === 1 ? groupAttributes[0] : undefined)) === attribute;
    }).length;
    return total + count;
  }, 0);
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

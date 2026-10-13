import { copyFile, cp, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, dirname, extname, join } from "node:path";

import sharp from "sharp";

import { parse as parseToml, stringify as stringifyToml } from "smol-toml";

import { classifyAsset, escapeRegex } from "@shared/assets";
import type { AssetEntry, Manifest, ProjectSnapshot, SourceFile, TreeNode, TranslationFile } from "@shared/types";

import { templateDir, within } from "./paths";

const TOP_LEVEL = ["kairos.toml", "script", "assets", "ui"];

let currentRoot: string | null = null;

export function projectRoot(): string | null {
  return currentRoot;
}

function requireRoot(): string {
  if (!currentRoot) throw new Error("No project is open");
  return currentRoot;
}

async function exists(path: string): Promise<boolean> {
  return stat(path).then(
    () => true,
    () => false,
  );
}

export async function readManifest(root: string): Promise<Manifest> {
  const path = join(root, "kairos.toml");
  if (!(await exists(path))) throw new Error(`${root} has no kairos.toml, so it is not a Kairos project`);
  const raw = parseToml(await readFile(path, "utf8")) as Partial<Manifest>;
  const window = raw.window;
  return {
    name: String(raw.name ?? basename(root)),
    start: String(raw.start ?? "start"),
    window: { width: Number(window?.width ?? 1280), height: Number(window?.height ?? 720) },
  };
}

async function walk(root: string, rel: string): Promise<TreeNode[]> {
  const entries = await readdir(join(root, rel), { withFileTypes: true }).catch(() => []);
  const nodes = await Promise.all(
    entries
      .filter((e) => !e.name.startsWith("."))
      .map(async (e): Promise<TreeNode> => {
        const path = rel ? `${rel}/${e.name}` : e.name;
        return e.isDirectory()
          ? { name: e.name, path, kind: "dir", children: await walk(root, path) }
          : { name: e.name, path, kind: "file" };
      }),
  );
  return nodes.toSorted((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1));
}

function flatten(nodes: TreeNode[]): string[] {
  return nodes.flatMap((n) => (n.kind === "dir" ? flatten(n.children ?? []) : [n.path]));
}

async function readAudioMetadata(root: string): Promise<ProjectSnapshot["audioMetadata"]> {
  const parsed = parseToml(await readFile(join(root, "assets/audio/audio.toml"), "utf8").catch(() => "")) as Record<
    string,
    unknown
  >;
  return Object.fromEntries(
    Object.entries(parsed).map(([path, value]) => {
      const meta = value as { loop_start?: number; loop_end?: number };
      return [path, { loopStart: meta.loop_start, loopEnd: meta.loop_end }];
    }),
  );
}

async function readSpriteLayers(root: string, tree: TreeNode[]): Promise<ProjectSnapshot["spriteLayers"]> {
  const sprites =
    tree
      .find((node) => node.path === "assets")
      ?.children?.find((node) => node.path === "assets/images")
      ?.children?.find((node) => node.path === "assets/images/sprites")?.children ?? [];
  const layers = await Promise.all(
    sprites
      .filter((node) => node.kind === "dir")
      .map(async (sprite) => {
        const path = join(root, sprite.path, "layers.toml");
        if (!(await exists(path))) return null;
        const parsed = parseToml(await readFile(path, "utf8")) as {
          order?: string[];
          default?: Record<string, string>;
        };
        const groups = Object.fromEntries(
          await Promise.all(
            (sprite.children ?? [])
              .filter((node) => node.kind === "dir")
              .map(async (group) => [
                group.name,
                (group.children ?? [])
                  .filter((node) => node.kind === "file")
                  .map((node) => node.name.replace(/\.[^.]+$/, "")),
              ]),
          ),
        );
        return [sprite.name, { order: parsed.order ?? Object.keys(groups), defaults: parsed.default ?? {}, groups }];
      }),
  );
  return Object.fromEntries(layers.filter((entry): entry is NonNullable<typeof entry> => entry !== null));
}

export async function loadProject(root: string): Promise<ProjectSnapshot> {
  const manifest = await readManifest(root);
  const tree: TreeNode[] = [];
  for (const name of TOP_LEVEL) {
    const abs = join(root, name);
    if (!(await exists(abs))) continue;
    const isDir = (await stat(abs)).isDirectory();
    tree.push(
      isDir ? { name, path: name, kind: "dir", children: await walk(root, name) } : { name, path: name, kind: "file" },
    );
  }
  const scriptDir = tree.find((n) => n.path === "script");
  const scriptPaths = flatten(scriptDir?.children ?? [])
    .filter((path) => path.endsWith(".kai"))
    .toSorted();
  const scripts: SourceFile[] = await Promise.all(
    scriptPaths.map(async (path) => ({ path, source: await readFile(join(root, path), "utf8") })),
  );
  const assetDir = tree.find((n) => n.path === "assets");
  const assets: AssetEntry[] = flatten(assetDir?.children ?? []).map((path) => classifyAsset(path));
  const translations: TranslationFile[] = await Promise.all(
    (await readdir(join(root, "tl"), { withFileTypes: true }).catch(() => []))
      .filter((entry) => entry.isFile() && entry.name.endsWith(".toml"))
      .map(async ({ name }) => ({ lang: name.slice(0, -5), source: await readFile(join(root, "tl", name), "utf8") })),
  );
  const manifestSource = await readFile(join(root, "kairos.toml"), "utf8");
  const uiTheme = await readFile(join(root, "ui/theme.toml"), "utf8").catch(() => null);
  const audioMetadata = await readAudioMetadata(root);
  const spriteLayers = await readSpriteLayers(root, tree);
  const uiTitle = await readFile(join(root, "ui/title.toml"), "utf8").catch(() => null);
  currentRoot = root;
  return {
    root,
    manifest,
    manifestSource,
    uiTheme,
    uiTitle,
    scripts,
    translations,
    tree,
    assets,
    audioMetadata,
    spriteLayers,
  };
}

const MAX_PREVIEW_BYTES = 1024 * 1024;

export async function readProjectText(rel: string): Promise<string> {
  const abs = within(requireRoot(), rel);
  if ((await stat(abs)).size > MAX_PREVIEW_BYTES) throw new Error(`${rel} is too large to preview`);
  return readFile(abs, "utf8");
}

export async function writeProjectFile(rel: string, content: string): Promise<void> {
  const abs = within(requireRoot(), rel);
  await mkdir(dirname(abs), { recursive: true });
  await writeFile(abs, content, "utf8");
}

export async function importProjectAsset(
  sourcePath: string,
  folder: string,
  convertWebp: boolean,
  approveLarge: boolean,
): Promise<{ path?: string; warning?: { width: number; height: number; maxWidth: number; maxHeight: number } }> {
  const root = requireRoot();
  if (!folder.startsWith("assets/") || folder.split("/").some((part) => part === ".." || part === ".")) {
    throw new Error("Choose a folder inside assets");
  }
  const sourceExtension = extname(sourcePath).toLowerCase();
  const isImage = [".png", ".jpg", ".jpeg", ".webp"].includes(sourceExtension);
  const manifest = await readManifest(root);
  const info = isImage ? await sharp(sourcePath).metadata() : { width: 0, height: 0 };
  const maxWidth = (manifest.window?.width ?? 1280) * 2;
  const maxHeight = (manifest.window?.height ?? 720) * 2;
  const width = info.width ?? 0;
  const height = info.height ?? 0;
  if (!approveLarge && (width > maxWidth || height > maxHeight))
    return { warning: { width, height, maxWidth, maxHeight } };
  const outputExt = convertWebp && isImage ? ".webp" : sourceExtension;
  const rawName = basename(sourcePath, sourceExtension);
  let name = `${rawName}${outputExt}`;
  let target = within(root, `${folder}/${name}`);
  for (let suffix = 2; await exists(target); suffix++) {
    name = `${rawName} ${suffix}${outputExt}`;
    target = within(root, `${folder}/${name}`);
  }
  await mkdir(dirname(target), { recursive: true });
  if (!isImage) await copyFile(sourcePath, target);
  else {
    let image = sharp(sourcePath);
    if (sourceExtension === ".png" && folder.startsWith("assets/images/sprites/") && width && height) {
      const { data, info: raw } = await image.ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      let left = raw.width;
      let top = raw.height;
      let right = -1;
      let bottom = -1;
      for (let y = 0; y < raw.height; y++) {
        for (let x = 0; x < raw.width; x++) {
          if (data[(y * raw.width + x) * raw.channels + 3] !== 0) {
            left = Math.min(left, x);
            top = Math.min(top, y);
            right = Math.max(right, x);
            bottom = Math.max(bottom, y);
          }
        }
      }
      if (right >= left && bottom >= top && (left || top || right < raw.width - 1 || bottom < raw.height - 1)) {
        image = sharp(sourcePath).extract({ left, top, width: right - left + 1, height: bottom - top + 1 });
      }
    }
    if (convertWebp) await image.webp().toFile(target);
    else if (sourceExtension === ".jpg" || sourceExtension === ".jpeg") await image.jpeg().toFile(target);
    else if (sourceExtension === ".webp") await image.webp().toFile(target);
    else await image.png().toFile(target);
  }
  return { path: `${folder}/${name}` };
}

export async function renameProjectAsset(rel: string, newName: string): Promise<void> {
  const root = requireRoot();
  const source = within(root, rel);
  const safeName = newName.trim();
  if (!safeName || /[\\/]/.test(safeName)) throw new Error("Enter a file name without a folder path");
  const extension = extname(rel);
  const asset = classifyAsset(rel);
  const layerPart = /^assets\/images\/sprites\/[^/]+\/[^/]+\/[^/]+$/.test(rel);
  const tag = asset.category === "sprite" && !layerPart ? asset.characterId : undefined;
  const tagged = tag && !safeName.startsWith(`${tag} `) ? `${tag} ${safeName}` : safeName;
  const targetRel = `${dirname(rel)}/${tagged.toLowerCase().endsWith(extension.toLowerCase()) ? tagged : `${tagged}${extension}`}`;
  const target = within(root, targetRel);
  if (target === source) return;
  if (await exists(target)) throw new Error(`${basename(target)} already exists`);
  const oldName = basename(rel, extension);
  const nextName = basename(targetRel, extension);
  const scriptPaths = flatten(await walk(root, "script")).filter((path) => path.endsWith(".kai"));
  for (const scriptPathRel of scriptPaths) {
    const scriptPath = join(root, scriptPathRel);
    const sourceText = await readFile(scriptPath, "utf8");
    const nextText = sourceText
      .split("\n")
      .map((line) => {
        if (asset.kind === "audio") {
          if (!/\bplay\s+(?:music|sound|voice)\s+/.test(line)) return line;
          const oldRef = asset.name;
          return line.replace(
            /(\bplay\s+(?:music|sound|voice)\s+["'])([^"']+)(["'])/g,
            (_match, before, value, after) => {
              const leaf = basename(value);
              return value === oldRef || leaf === `${oldName}${extension}` || leaf === oldName
                ? `${before}${value.replace(leaf, `${nextName}${extension}`)}${after}`
                : `${before}${value}${after}`;
            },
          );
        }
        if (asset.kind !== "image" || !/^\s*(?:scene|show)\s+/.test(line)) return line;
        const characterId = asset.characterId ?? (oldName.includes(" ") ? oldName.split(/\s+/)[0] : undefined);
        if (
          characterId &&
          /^\s*show\s+/.test(line) &&
          !new RegExp(`^\\s*show\\s+${escapeRegex(characterId)}(?=\\s|$)`).test(line)
        )
          return line;
        if (asset.category === "background" && !/^\s*scene\s+/.test(line)) return line;
        return line.replace(new RegExp(`(?<=\\s)${escapeRegex(oldName)}(?=\\s|$)`, "g"), nextName);
      })
      .join("\n");
    if (nextText !== sourceText) await writeFile(scriptPath, nextText, "utf8");
  }
  if (asset.kind === "image" && rel.startsWith("assets/images/sprites/")) {
    const parts = rel.split("/");
    const group = parts[4];
    const metadataPath = join(root, "assets/images/sprites", parts[3]!, "layers.toml");
    if (group && (await exists(metadataPath))) {
      const metadata = parseToml(await readFile(metadataPath, "utf8")) as { default?: Record<string, string> };
      if (metadata.default?.[group] === oldName) {
        metadata.default[group] = nextName;
        await writeFile(metadataPath, `${stringifyToml(metadata)}\n`, "utf8");
      }
    }
  }
  if (asset.kind === "image") {
    const titlePath = join(root, "ui/title.toml");
    const title = parseToml(await readFile(titlePath, "utf8").catch(() => "")) as Record<string, unknown>;
    const fields = (["background", "logo"] as const).filter((field) => title[field] === oldName);
    if (fields.length) {
      for (const field of fields) title[field] = nextName;
      await writeFile(titlePath, `${stringifyToml(title)}\n`, "utf8");
    }
  }
  if (asset.kind === "audio") {
    const metadataPath = join(root, "assets/audio/audio.toml");
    const metadata = parseToml(await readFile(metadataPath, "utf8").catch(() => "")) as Record<string, unknown>;
    const oldKey = rel.slice("assets/audio/".length);
    const newKey = targetRel.slice("assets/audio/".length);
    if (oldKey in metadata) {
      metadata[newKey] = metadata[oldKey];
      delete metadata[oldKey];
      await writeFile(metadataPath, `${stringifyToml(metadata)}\n`, "utf8");
    }
  }
  await (await import("node:fs/promises")).rename(source, target);
}

const TEMPLATE_SKIP = new Set(["inputs.txt", "expected.txt"]);

/** Copies the starter project into `dest` and names it after the folder. */
export async function createFromTemplate(dest: string): Promise<string> {
  if (await exists(dest)) throw new Error(`${dest} already exists`);
  const src = templateDir();
  await cp(src, dest, { recursive: true, filter: (p) => !TEMPLATE_SKIP.has(basename(p)) });
  for (const dir of ["assets/images", "assets/audio", "ui"]) await mkdir(join(dest, dir), { recursive: true });
  const manifestPath = join(dest, "kairos.toml");
  const manifest = parseToml(await readFile(manifestPath, "utf8"));
  manifest.name = basename(dest);
  await writeFile(manifestPath, stringifyToml(manifest) + "\n", "utf8");
  return dest;
}

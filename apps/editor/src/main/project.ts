import { cp, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

import { parse as parseToml, stringify as stringifyToml } from "smol-toml";

import { classifyAsset } from "@shared/assets";
import { addCharacter, insertJump, setCharacterLine } from "@shared/edits";
import type {
  AssetEntry,
  EditResult,
  Manifest,
  ProjectEdit,
  ProjectSnapshot,
  SourceFile,
  TreeNode,
  TranslationFile,
} from "@shared/types";

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
  return { name: String(raw.name ?? basename(root)), start: String(raw.start ?? "start") };
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
  const scriptPaths = (scriptDir?.children ?? [])
    .filter((n) => n.kind === "file" && n.name.endsWith(".kai"))
    .map((n) => n.path)
    .toSorted();
  const scripts: SourceFile[] = await Promise.all(
    scriptPaths.map(async (path) => ({ path, source: await readFile(join(root, path), "utf8") })),
  );
  const assetDir = tree.find((n) => n.path === "assets");
  const assets: AssetEntry[] = flatten(assetDir?.children ?? []).map(classifyAsset);
  const translations: TranslationFile[] = await Promise.all(
    (await readdir(join(root, "tl"), { withFileTypes: true }).catch(() => []))
      .filter((entry) => entry.isFile() && entry.name.endsWith(".toml"))
      .map(async ({ name }) => ({ lang: name.slice(0, -5), source: await readFile(join(root, "tl", name), "utf8") })),
  );
  const manifestSource = await readFile(join(root, "kairos.toml"), "utf8");
  const uiTheme = await readFile(join(root, "ui/theme.toml"), "utf8").catch(() => null);
  currentRoot = root;
  return { root, manifest, manifestSource, uiTheme, scripts, translations, tree, assets };
}

export async function writeProjectFile(rel: string, content: string): Promise<void> {
  const abs = within(requireRoot(), rel);
  await mkdir(dirname(abs), { recursive: true });
  await writeFile(abs, content, "utf8");
}

export async function applyEdit(edit: ProjectEdit): Promise<EditResult> {
  const abs = within(requireRoot(), edit.path);
  const source = edit.source ?? (await readFile(abs, "utf8"));
  let next: string;
  if (edit.kind === "connect") {
    const result = insertJump(source, edit.from, edit.to);
    if (!result) throw new Error(`label ${edit.from} is not in ${edit.path}`);
    next = result.source;
  } else {
    next =
      edit.line === undefined
        ? addCharacter(source, edit.character)
        : setCharacterLine(source, edit.line, edit.character);
  }
  // A buffer sent from the renderer may differ from disk, so it is saved even if the edit was a no-op.
  if (next !== source || edit.source !== undefined) await writeFile(abs, next, "utf8");
  return { path: edit.path, source: next, changed: next !== source };
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

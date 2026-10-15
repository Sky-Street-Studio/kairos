import { lstatSync, readlinkSync, realpathSync } from "node:fs";
import { basename, dirname, isAbsolute, relative, resolve, sep } from "node:path";

import { app } from "electron";

/** The repository root while running from source. */
export function repoRoot(): string {
  return resolve(app.getAppPath(), "../..");
}

export function templateDir(): string {
  if (app.isPackaged) return resolve(process.resourcesPath, "template");
  return process.env.KAIROS_TEMPLATE ?? resolve(repoRoot(), "examples/hello-kairos");
}

export function playerPkgDir(): string {
  if (app.isPackaged) return resolve(process.resourcesPath, "kairos-player");
  return process.env.KAIROS_PLAYER_PKG ?? resolve(repoRoot(), "crates/kairos-player/pkg");
}

function canonicalPath(path: string): string {
  try {
    return realpathSync(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    try {
      if (lstatSync(path).isSymbolicLink()) {
        return canonicalPath(resolve(dirname(path), readlinkSync(path)));
      }
    } catch (pathError) {
      if ((pathError as NodeJS.ErrnoException).code !== "ENOENT") throw pathError;
    }
    const parent = dirname(path);
    if (parent === path) throw error;
    return resolve(canonicalPath(parent), basename(path));
  }
}

/** Resolves `rel` under `root`, refusing anything that escapes it. */
export function within(root: string, rel: string): string {
  const realRoot = realpathSync(root);
  const abs = resolve(root, rel);
  const target = canonicalPath(abs);
  const back = relative(realRoot, target);
  if (back === "" || back === ".." || back.startsWith(`..${sep}`) || isAbsolute(back)) {
    throw new Error(`${rel} is outside the project`);
  }
  return target;
}

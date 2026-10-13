import { isAbsolute, relative, resolve } from "node:path";

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

/** Resolves `rel` under `root`, refusing anything that escapes it. */
export function within(root: string, rel: string): string {
  const abs = resolve(root, rel);
  const back = relative(root, abs);
  if (back === "" || back.startsWith("..") || isAbsolute(back)) {
    throw new Error(`${rel} is outside the project`);
  }
  return abs;
}

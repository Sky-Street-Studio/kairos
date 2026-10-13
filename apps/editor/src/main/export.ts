import { existsSync } from "node:fs";
import { join } from "node:path";

import { app, BrowserWindow, dialog } from "electron";

import { napi } from "./analysis";
import { playerPkgDir, repoRoot } from "./paths";
import { projectRoot } from "./project";

export async function pickExportFolder(parent: Electron.BrowserWindow): Promise<string | null> {
  const result = await dialog.showOpenDialog(parent, {
    title: "Choose export folder",
    buttonLabel: "Choose folder",
    properties: ["openDirectory", "createDirectory"],
  });
  return result.canceled ? null : (result.filePaths[0] ?? null);
}

export function exportProject(target: "web" | "desktop", output: string): string {
  const root = projectRoot();
  if (!root) throw new Error("No project is open");
  const playerBinary =
    process.env.KAIROS_PLAYER_BINARY ??
    (app.isPackaged
      ? join(process.resourcesPath, "kairos")
      : ["release", "debug"].map((profile) => join(repoRoot(), "target", profile, "kairos")).find(existsSync));
  if (target === "desktop" && !playerBinary) {
    throw new Error("Kairos player binary not found; build it with `cargo build -p kairos-cli --release`.");
  }
  return napi().exportGame(root, target, output, playerBinary, playerPkgDir());
}

export async function chooseExportFolder(sender: Electron.WebContents): Promise<string | null> {
  const parent = BrowserWindow.fromWebContents(sender);
  if (!parent) return null;
  return pickExportFolder(parent);
}

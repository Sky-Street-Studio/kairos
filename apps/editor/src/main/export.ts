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

export type ExportTarget = "web" | "desktop" | "windows-x64" | "linux-x64" | "macos";

export function exportProject(target: ExportTarget, output: string): string {
  const root = projectRoot();
  if (!root) throw new Error("No project is open");
  const hostPlayerBinary =
    process.env.KAIROS_PLAYER_BINARY ??
    (app.isPackaged
      ? join(process.resourcesPath, "kairos")
      : ["release", "debug"].map((profile) => join(repoRoot(), "target", profile, "kairos")).find(existsSync));
  const platformBinary =
    target === "windows-x64"
      ? process.env.KAIROS_PLAYER_BINARY_WINDOWS_X64 ?? (process.platform === "win32" ? hostPlayerBinary : undefined)
      : target === "linux-x64"
        ? process.env.KAIROS_PLAYER_BINARY_LINUX_X64 ?? (process.platform === "linux" ? hostPlayerBinary : undefined)
        : target === "macos"
          ? process.env.KAIROS_PLAYER_BINARY_MACOS ?? (process.platform === "darwin" ? hostPlayerBinary : undefined)
          : undefined;
  const playerBinary = target === "desktop" ? hostPlayerBinary : platformBinary;
  if (target !== "web" && !playerBinary) {
    const setting = target === "desktop" ? "KAIROS_PLAYER_BINARY" : `KAIROS_PLAYER_BINARY_${target.toUpperCase().replace("-", "_")}`;
    throw new Error(`Kairos ${target} player binary not found; set ${setting}.`);
  }
  return napi().exportGame(root, target, output, playerBinary, playerPkgDir());
}

export async function chooseExportFolder(sender: Electron.WebContents): Promise<string | null> {
  const parent = BrowserWindow.fromWebContents(sender);
  if (!parent) return null;
  return pickExportFolder(parent);
}

import { readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

import { BrowserWindow, Menu, app, dialog, ipcMain, shell, type MenuItemConstructorOptions } from "electron";

import type { EditOp, EditorLayout } from "@shared/edit-types";
import type { AnalyzeInput, Settings, SourceFile, TerminalSpawn } from "@shared/types";

import { analyze } from "./analysis";
import { applyEdits, labelFromName, readLayout, references, sceneBlocks, setStartLabel, writeLayout } from "./edit";
import { chooseExportFolder, exportProject } from "./export";
import {
  createFromTemplate,
  importProjectAsset,
  loadProject,
  projectRoot,
  renameProjectAsset,
  writeProjectFile,
} from "./project";
import { handleProtocols, playerStatus, registerSchemes } from "./protocols";
import { getSettings, rememberProject, updateSettings } from "./settings";
import { agents, killAllTerminals, killTerminal, resizeTerminal, spawnTerminal, writeTerminal } from "./terminal";

registerSchemes();
app.setName("Kairos");
app.setPath("userData", process.env.KAIROS_USER_DATA ?? join(app.getPath("appData"), "Kairos"));

function initialProjectArg(): string | null {
  const flag = process.argv.find((a) => a.startsWith("--project="));
  const dir = flag?.slice("--project=".length) ?? process.env.KAIROS_PROJECT;
  return dir ? resolve(dir) : null;
}

function createWindow(): BrowserWindow {
  const win = new BrowserWindow({
    width: 1480,
    height: 920,
    minWidth: 960,
    minHeight: 600,
    show: false,
    title: "Kairos",
    backgroundColor: "#0b0c11",
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    trafficLightPosition: { x: 14, y: 13 },
    webPreferences: {
      preload: join(import.meta.dirname, "../preload/index.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  win.once("ready-to-show", () => win.show());
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("https:")) void shell.openExternal(url);
    return { action: "deny" };
  });
  if (process.env.ELECTRON_RENDERER_URL) {
    void win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void win.loadFile(join(import.meta.dirname, "../renderer/index.html"));
  }
  return win;
}

const requireOpenRoot = (): string =>
  projectRoot() ??
  (() => {
    throw new Error("Open a project first");
  })();

function registerIpc(): void {
  ipcMain.handle("project:initial", async () => initialProjectArg() ?? (await getSettings()).recentProjects[0] ?? null);
  ipcMain.handle("project:pick", async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender)!;
    const r = await dialog.showOpenDialog(win, {
      title: "Open Kairos project",
      buttonLabel: "Open project",
      properties: ["openDirectory"],
    });
    return r.canceled ? null : (r.filePaths[0] ?? null);
  });
  ipcMain.handle("project:open", async (_e, root: string) => {
    const snapshot = await loadProject(root);
    await rememberProject(root);
    return snapshot;
  });
  ipcMain.handle("project:reload", async () => {
    const root = projectRoot();
    return root ? loadProject(root) : null;
  });
  ipcMain.handle("project:write", (_e, path: string, content: string) => writeProjectFile(path, content));
  ipcMain.handle("project:asset-import", async (e, sourcePath: string | null, folder: string, convertWebp: boolean) => {
    let source = sourcePath;
    if (!source) {
      const audio = folder.startsWith("assets/audio/");
      const result = await dialog.showOpenDialog(BrowserWindow.fromWebContents(e.sender)!, {
        title: "Import asset",
        properties: ["openFile"],
        filters:
          folder === "assets"
            ? undefined
            : [
                {
                  name: audio ? "Audio" : "Images",
                  extensions: audio ? ["ogg", "mp3", "wav", "flac"] : ["png", "jpg", "jpeg", "webp"],
                },
              ],
      });
      if (result.canceled || result.filePaths.length === 0) return null;
      const imported: string[] = [];
      for (const filePath of result.filePaths) {
        const attempt = await importProjectAsset(filePath, folder, convertWebp, false);
        if (attempt.warning) {
          const answer = await dialog.showMessageBox(BrowserWindow.fromWebContents(e.sender)!, {
            type: "warning",
            buttons: ["Import", "Cancel"],
            defaultId: 0,
            cancelId: 1,
            message: `${basename(filePath)} is ${attempt.warning.width} × ${attempt.warning.height}px.`,
            detail: `This exceeds twice the manifest window size (${attempt.warning.maxWidth} × ${attempt.warning.maxHeight}px). Import anyway?`,
          });
          if (answer.response !== 0) continue;
          const approved = await importProjectAsset(filePath, folder, convertWebp, true);
          if (approved.path) imported.push(approved.path);
        } else if (attempt.path) imported.push(attempt.path);
      }
      return imported.length ? imported[0] : null;
    }
    const attempt = await importProjectAsset(source, folder, convertWebp, false);
    if (attempt.warning) {
      const answer = await dialog.showMessageBox(BrowserWindow.fromWebContents(e.sender)!, {
        type: "warning",
        buttons: ["Import", "Cancel"],
        defaultId: 0,
        cancelId: 1,
        message: `${basename(source)} is ${attempt.warning.width} × ${attempt.warning.height}px.`,
        detail: `This exceeds twice the manifest window size (${attempt.warning.maxWidth} × ${attempt.warning.maxHeight}px). Import anyway?`,
      });
      if (answer.response !== 0) return null;
      const approved = await importProjectAsset(source, folder, convertWebp, true);
      return approved.path ?? null;
    }
    return attempt.path ?? null;
  });
  ipcMain.handle("project:asset-rename", (_e, path: string, name: string) => renameProjectAsset(path, name));
  ipcMain.handle("project:create", async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender)!;
    const r = await dialog.showSaveDialog(win, {
      title: "New Kairos project",
      buttonLabel: "Create project",
      nameFieldLabel: "Project folder",
      defaultPath: "my-story",
    });
    return r.canceled || !r.filePath ? null : createFromTemplate(r.filePath);
  });

  ipcMain.handle("analysis:analyze", (_e, input: AnalyzeInput) => analyze(input));
  ipcMain.handle("edit:blocks", (_e, source: string, label: string) => sceneBlocks(source, label));
  ipcMain.handle("edit:apply", (_e, files: SourceFile[], ops: EditOp[]) => applyEdits(files, ops));
  ipcMain.handle("edit:references", (_e, files: SourceFile[], label: string) => references(files, label));
  ipcMain.handle("edit:label-from-name", (_e, name: string, taken: string[]) => labelFromName(name, taken));
  ipcMain.handle("edit:read-layout", () => readLayout(requireOpenRoot()));
  ipcMain.handle("edit:write-layout", (_e, layout: EditorLayout) => writeLayout(requireOpenRoot(), layout));
  ipcMain.handle("edit:set-start", (_e, label: string) => setStartLabel(requireOpenRoot(), label));
  ipcMain.handle("player:status", () => playerStatus());
  ipcMain.handle("export:pick-folder", (e) => chooseExportFolder(e.sender));
  ipcMain.handle("project:export", (_e, target: "web" | "desktop", output: string) => exportProject(target, output));

  ipcMain.handle("terminal:agents", () => agents());
  ipcMain.handle("terminal:spawn", (e, opts: TerminalSpawn) => spawnTerminal(e.sender, opts));
  ipcMain.on("terminal:write", (_e, id: number, data: string) => writeTerminal(id, data));
  ipcMain.on("terminal:resize", (_e, id: number, cols: number, rows: number) => resizeTerminal(id, cols, rows));
  ipcMain.on("terminal:kill", (_e, id: number) => killTerminal(id));

  ipcMain.handle("settings:get", () => getSettings());
  ipcMain.handle("settings:update", (_e, patch: Partial<Settings>) => updateSettings(patch));
  ipcMain.handle("settings:exportTheme", async (e, json: string) => {
    const win = BrowserWindow.fromWebContents(e.sender)!;
    const r = await dialog.showSaveDialog(win, {
      title: "Export theme",
      defaultPath: "kairos-theme.json",
      filters: [{ name: "Theme", extensions: ["json"] }],
    });
    if (r.canceled || !r.filePath) return false;
    await writeFile(r.filePath, json, "utf8");
    return true;
  });
  ipcMain.handle("settings:importTheme", async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender)!;
    const r = await dialog.showOpenDialog(win, {
      title: "Import theme",
      filters: [{ name: "Theme", extensions: ["json"] }],
      properties: ["openFile"],
    });
    const path = r.filePaths[0];
    return r.canceled || !path ? null : readFile(path, "utf8");
  });
}

function buildMenu(): void {
  const template: MenuItemConstructorOptions[] = [
    ...(process.platform === "darwin" ? [{ role: "appMenu" as const }] : []),
    { role: "editMenu" },
    {
      label: "View",
      submenu: [
        { role: "reload" },
        { role: "toggleDevTools" },
        { type: "separator" },
        { role: "resetZoom" },
        { role: "zoomIn" },
        { role: "zoomOut" },
        { type: "separator" },
        { role: "togglefullscreen" },
      ],
    },
    { role: "windowMenu" },
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

app.whenReady().then(() => {
  if (!app.isPackaged) app.dock?.setIcon(join(app.getAppPath(), "build/icon.png"));
  handleProtocols();
  registerIpc();
  buildMenu();
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  killAllTerminals();
  if (process.platform !== "darwin") app.quit();
});

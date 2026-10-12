import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

import { BrowserWindow, Menu, app, dialog, ipcMain, shell, type MenuItemConstructorOptions } from "electron";

import type { ProjectEdit, Settings, SourceFile, TerminalSpawn } from "@shared/types";

import { analyze } from "./analysis";
import { applyEdit, createFromTemplate, loadProject, projectRoot, writeProjectFile } from "./project";
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
  ipcMain.handle("project:edit", (_e, edit: ProjectEdit) => applyEdit(edit));
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

  ipcMain.handle("analysis:analyze", (_e, files: SourceFile[]) => analyze(files));
  ipcMain.handle("player:status", () => playerStatus());

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

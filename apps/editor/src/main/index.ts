import { readFile, realpath, stat, writeFile } from "node:fs/promises";
import { basename, isAbsolute, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

import { BrowserWindow, Menu, app, dialog, ipcMain, session, shell, type MenuItemConstructorOptions } from "electron";

import type { EditOp, EditorLayout } from "@shared/edit-types";
import type { AnalyzeInput, ExportTarget, MenuCommand, Settings, SourceFile, TerminalSpawn } from "@shared/types";

import { analyze } from "./analysis";
import { applyEdits, labelFromName, readLayout, references, sceneBlocks, setStartLabel, writeLayout } from "./edit";
import { chooseExportFolder, exportProject } from "./export";
import {
  createFromTemplate,
  importProjectAsset,
  loadProject,
  projectRoot,
  renameProjectAsset,
  readProjectText,
  writeProjectFile,
} from "./project";
import { handleProtocols, playerStatus, registerSchemes } from "./protocols";
import { getSettings, rememberProject, updateSettings, updateSettingsFromRenderer } from "./settings";
import { agents, killAllTerminals, killTerminal, resizeTerminal, spawnTerminal, writeTerminal } from "./terminal";

registerSchemes();
app.setName("Kairos");

const approvedPaths = new Set<string>();

async function approvePath(path: string): Promise<string> {
  approvedPaths.add(await realpath(path));
  return resolve(path);
}

async function requireApprovedPath(path: string): Promise<string> {
  const approved = await realpath(path);
  if (!approvedPaths.has(approved)) throw new Error("Choose this path from a Kairos dialog first");
  return resolve(path);
}

function trustedRendererUrl(url: string): boolean {
  try {
    const actual = new URL(url);
    if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) {
      return actual.origin === new URL(process.env.ELECTRON_RENDERER_URL).origin;
    }
    if (actual.protocol === "file:") {
      return actual.pathname === pathToFileURL(join(import.meta.dirname, "../renderer/index.html")).pathname;
    }
    return actual.protocol === "app:" && actual.host === "kairos";
  } catch {
    return false;
  }
}

function handle<Args extends unknown[]>(
  channel: string,
  listener: (event: Electron.IpcMainInvokeEvent, ...args: Args) => unknown,
): void {
  ipcMain.handle(channel, (event, ...args: Args) => {
    if (!trustedRendererUrl(event.senderFrame?.url ?? "")) throw new Error("Untrusted renderer");
    return listener(event, ...args);
  });
}
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
  win.webContents.on("will-navigate", (event, url) => {
    if (!trustedRendererUrl(url)) event.preventDefault();
  });
  win.webContents.on("will-redirect", (details) => {
    if (!details.isMainFrame || !trustedRendererUrl(details.url)) details.preventDefault();
  });
  win.webContents.on("will-frame-navigate", (details) => {
    if (!details.isMainFrame || !trustedRendererUrl(details.url)) details.preventDefault();
  });
  if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) {
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
  handle("project:initial", async () => {
    const settings = await getSettings();
    for (const root of settings.recentProjects) await approvePath(root).catch(() => undefined);
    const root = initialProjectArg() ?? (settings.reopenLastProject ? settings.recentProjects[0] : undefined) ?? null;
    return root ? approvePath(root).catch(() => root) : null;
  });
  handle("shell:reveal", async (_e, path: string) => {
    const root = await realpath(requireOpenRoot());
    const target = await realpath(resolve(root, path));
    const relativePath = relative(root, target);
    if (relativePath === ".." || relativePath.startsWith(`..${sep}`) || isAbsolute(relativePath)) {
      throw new Error("Path is outside the project");
    }
    shell.showItemInFolder(target);
  });
  handle("project:pick", async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender)!;
    const r = await dialog.showOpenDialog(win, {
      title: "Open Kairos project",
      buttonLabel: "Open project",
      properties: ["openDirectory"],
    });
    return r.canceled || !r.filePaths[0] ? null : approvePath(r.filePaths[0]);
  });
  handle("project:open", async (_e, root: string) => {
    const approvedRoot = await requireApprovedPath(root);
    const snapshot = await loadProject(approvedRoot);
    await rememberProject(snapshot.root);
    return snapshot;
  });
  handle("project:reload", async () => {
    const root = projectRoot();
    return root ? loadProject(root) : null;
  });
  handle("project:read-text", (_e, path: string) => readProjectText(path));
  handle("project:write", (_e, path: string, content: string) => writeProjectFile(path, content));
  handle("project:asset-import", async (e, sourcePath: string | null, folder: string, convertWebp: boolean) => {
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
        const approvedSource = await approvePath(filePath);
        const attempt = await importProjectAsset(approvedSource, folder, convertWebp, false);
        if (attempt.warning) {
          const answer = await dialog.showMessageBox(BrowserWindow.fromWebContents(e.sender)!, {
            type: "warning",
            buttons: ["Import", "Cancel"],
            defaultId: 0,
            cancelId: 1,
            message: `${basename(approvedSource)} is ${attempt.warning.width} × ${attempt.warning.height}px.`,
            detail: `This exceeds twice the manifest window size (${attempt.warning.maxWidth} × ${attempt.warning.maxHeight}px). Import anyway?`,
          });
          if (answer.response !== 0) continue;
          const approved = await importProjectAsset(approvedSource, folder, convertWebp, true);
          if (approved.path) imported.push(approved.path);
        } else if (attempt.path) imported.push(attempt.path);
      }
      return imported.length ? imported[0] : null;
    }
    const approvedSource = await requireApprovedPath(source);
    const attempt = await importProjectAsset(approvedSource, folder, convertWebp, false);
    if (attempt.warning) {
      const answer = await dialog.showMessageBox(BrowserWindow.fromWebContents(e.sender)!, {
        type: "warning",
        buttons: ["Import", "Cancel"],
        defaultId: 0,
        cancelId: 1,
        message: `${basename(approvedSource)} is ${attempt.warning.width} × ${attempt.warning.height}px.`,
        detail: `This exceeds twice the manifest window size (${attempt.warning.maxWidth} × ${attempt.warning.maxHeight}px). Import anyway?`,
      });
      if (answer.response !== 0) return null;
      const approved = await importProjectAsset(approvedSource, folder, convertWebp, true);
      return approved.path ?? null;
    }
    return attempt.path ?? null;
  });
  handle("project:asset-rename", (_e, path: string, name: string) => renameProjectAsset(path, name));
  handle("project:pick-location", async (e) => {
    const r = await dialog.showOpenDialog(BrowserWindow.fromWebContents(e.sender)!, {
      title: "Choose project location",
      buttonLabel: "Use this folder",
      properties: ["openDirectory", "createDirectory"],
    });
    return r.canceled || !r.filePaths[0] ? null : approvePath(r.filePaths[0]);
  });
  handle("project:create", async (_e, template: string, name: string, parent: string) => {
    const approvedParent = await realpath(parent);
    if (!approvedPaths.has(approvedParent) && !(await stat(approvedParent)).isDirectory()) {
      throw new Error("Choose an existing project location");
    }
    const root = await createFromTemplate(approvedParent, name, template as Parameters<typeof createFromTemplate>[2]);
    await approvePath(root);
    const { hiddenCodeProjects, beginnerProjects } = await getSettings();
    await updateSettings({
      hiddenCodeProjects: [...new Set([...hiddenCodeProjects, root])],
      beginnerProjects: [...new Set([...beginnerProjects, root])],
    });
    return root;
  });

  handle("analysis:analyze", (_e, input: AnalyzeInput) => analyze(input));
  handle("edit:blocks", (_e, source: string, label: string) => sceneBlocks(source, label));
  handle("edit:apply", (_e, files: SourceFile[], ops: EditOp[]) => applyEdits(files, ops));
  handle("edit:references", (_e, files: SourceFile[], label: string) => references(files, label));
  handle("edit:label-from-name", (_e, name: string, taken: string[]) => labelFromName(name, taken));
  handle("edit:read-layout", () => readLayout(requireOpenRoot()));
  handle("edit:write-layout", (_e, layout: EditorLayout) => writeLayout(requireOpenRoot(), layout));
  handle("edit:set-start", (_e, label: string) => setStartLabel(requireOpenRoot(), label));
  handle("player:status", () => playerStatus());
  handle("export:pick-folder", async (e) => {
    const path = await chooseExportFolder(e.sender);
    return path ? approvePath(path) : null;
  });
  handle("project:export", async (_e, target: ExportTarget, output: string, packAssets: boolean) =>
    exportProject(target, await requireApprovedPath(output), packAssets),
  );

  handle("terminal:agents", () => agents());
  handle("terminal:spawn", (e, opts: TerminalSpawn) => spawnTerminal(e.sender, opts));
  ipcMain.on("terminal:write", (e, id: number, data: string) => {
    if (trustedRendererUrl(e.senderFrame?.url ?? "")) writeTerminal(e.sender, id, data);
  });
  ipcMain.on("terminal:resize", (e, id: number, cols: number, rows: number) => {
    if (trustedRendererUrl(e.senderFrame?.url ?? "")) resizeTerminal(e.sender, id, cols, rows);
  });
  ipcMain.on("terminal:kill", (e, id: number) => {
    if (trustedRendererUrl(e.senderFrame?.url ?? "")) killTerminal(e.sender, id);
  });

  handle("settings:get", () => getSettings());
  handle("settings:update", (_e, patch: Partial<Settings>) => updateSettingsFromRenderer(patch));
  handle("settings:exportTheme", async (e, json: string) => {
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
  handle("settings:importTheme", async (e) => {
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

function menuCommand(label: string, command: MenuCommand, accelerator: string): MenuItemConstructorOptions {
  return {
    label,
    accelerator,
    // The renderer owns these shortcuts so they also work in tests and with focus in the editor; the menu only lists them.
    registerAccelerator: false,
    click: (_item, win) => {
      if (win instanceof BrowserWindow) win.webContents.send("menu:command", command);
    },
  };
}

function buildMenu(): void {
  const settings = menuCommand("Settings…", "settings", "CmdOrCtrl+,");
  const template: MenuItemConstructorOptions[] = [
    ...(process.platform === "darwin"
      ? [
          {
            label: app.name,
            submenu: [
              { role: "about" },
              { type: "separator" },
              settings,
              { type: "separator" },
              { role: "services" },
              { type: "separator" },
              { role: "hide" },
              { role: "hideOthers" },
              { role: "unhide" },
              { type: "separator" },
              { role: "quit" },
            ] satisfies MenuItemConstructorOptions[],
          },
        ]
      : [
          {
            label: "File",
            submenu: [settings, { type: "separator" }, { role: "quit" }],
          } satisfies MenuItemConstructorOptions,
        ]),
    { role: "editMenu" },
    {
      label: "View",
      submenu: [
        menuCommand("Toggle Sidebar", "toggle-sidebar", "CmdOrCtrl+B"),
        menuCommand("Toggle Terminal", "toggle-terminal", "CmdOrCtrl+J"),
        menuCommand("Toggle Preview", "toggle-preview", "CmdOrCtrl+\\"),
        { type: "separator" },
        { role: "reload" },
        ...(!app.isPackaged ? [{ role: "toggleDevTools" } satisfies MenuItemConstructorOptions] : []),
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
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  session.defaultSession.setPermissionCheckHandler(() => false);
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

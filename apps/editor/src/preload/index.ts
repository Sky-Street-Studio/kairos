import type { KairosApi } from "@shared/types";
import { contextBridge, type IpcRendererEvent, ipcRenderer } from "electron";

function invokeEdit<T>(channel: string, ...args: unknown[]): Promise<T> {
  return ipcRenderer.invoke(channel, ...args).catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(message.replace(/^Error invoking remote method '[^']+': (?:Error: )?/, ""));
  });
}

function subscribe<A extends unknown[]>(channel: string, listener: (...args: A) => void): () => void {
  const wrapped = (_e: IpcRendererEvent, ...args: unknown[]) => listener(...(args as A));
  ipcRenderer.on(channel, wrapped);
  return () => ipcRenderer.off(channel, wrapped);
}

const api: KairosApi = {
  platform: process.platform,
  project: {
    initial: () => ipcRenderer.invoke("project:initial"),
    pick: () => ipcRenderer.invoke("project:pick"),
    open: (root) => ipcRenderer.invoke("project:open", root),
    reload: () => ipcRenderer.invoke("project:reload"),
    readText: (path) => ipcRenderer.invoke("project:read-text", path),
    writeFile: (path, content) => ipcRenderer.invoke("project:write", path, content),
    importAsset: (sourcePath, folder, convertWebp) =>
      ipcRenderer.invoke("project:asset-import", sourcePath, folder, convertWebp ?? false),
    renameAsset: (path, name) => ipcRenderer.invoke("project:asset-rename", path, name),
    pickProjectLocation: () => ipcRenderer.invoke("project:pick-location"),
    createFromTemplate: (template, name, parent) => ipcRenderer.invoke("project:create", template, name, parent),
    pickExportFolder: () => ipcRenderer.invoke("export:pick-folder"),
    export: (target, output) => ipcRenderer.invoke("project:export", target, output),
  },
  analysis: {
    analyze: (input) => ipcRenderer.invoke("analysis:analyze", input),
  },
  edit: {
    blocks: (source, label) => invokeEdit("edit:blocks", source, label),
    apply: (files, ops) => invokeEdit("edit:apply", files, ops),
    references: (files, label) => invokeEdit("edit:references", files, label),
    labelFromName: (name, taken) => invokeEdit("edit:label-from-name", name, taken),
    readLayout: () => invokeEdit("edit:read-layout"),
    writeLayout: (layout) => invokeEdit("edit:write-layout", layout),
    setStart: (label) => invokeEdit("edit:set-start", label),
  },
  player: {
    status: () => ipcRenderer.invoke("player:status"),
  },
  terminal: {
    agents: () => ipcRenderer.invoke("terminal:agents"),
    spawn: (opts) => ipcRenderer.invoke("terminal:spawn", opts),
    write: (id, data) => ipcRenderer.send("terminal:write", id, data),
    resize: (id, cols, rows) => ipcRenderer.send("terminal:resize", id, cols, rows),
    kill: (id) => ipcRenderer.send("terminal:kill", id),
    onData: (listener) => subscribe("terminal:data", listener),
    onExit: (listener) => subscribe("terminal:exit", listener),
  },
  shell: {
    reveal: (path) => invokeEdit("shell:reveal", path),
  },
  settings: {
    get: () => ipcRenderer.invoke("settings:get"),
    update: (patch) => ipcRenderer.invoke("settings:update", patch),
    exportTheme: (json) => ipcRenderer.invoke("settings:exportTheme", json),
    importTheme: () => ipcRenderer.invoke("settings:importTheme"),
  },
};

contextBridge.exposeInMainWorld("kairos", api);

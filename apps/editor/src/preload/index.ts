import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";

import type { KairosApi } from "@shared/types";

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
    writeFile: (path, content) => ipcRenderer.invoke("project:write", path, content),
    importAsset: (sourcePath, folder, convertWebp) =>
      ipcRenderer.invoke("project:asset-import", sourcePath, folder, convertWebp ?? false),
    renameAsset: (path, name) => ipcRenderer.invoke("project:asset-rename", path, name),
    applyEdit: (edit) => ipcRenderer.invoke("project:edit", edit),
    createFromTemplate: () => ipcRenderer.invoke("project:create"),
  },
  analysis: {
    analyze: (files) => ipcRenderer.invoke("analysis:analyze", files),
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
  settings: {
    get: () => ipcRenderer.invoke("settings:get"),
    update: (patch) => ipcRenderer.invoke("settings:update", patch),
    exportTheme: (json) => ipcRenderer.invoke("settings:exportTheme", json),
    importTheme: () => ipcRenderer.invoke("settings:importTheme"),
  },
};

contextBridge.exposeInMainWorld("kairos", api);

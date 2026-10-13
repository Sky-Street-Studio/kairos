import type { ProjectSnapshot } from "@shared/types";

import type { FileBuffer, ProjectSlice, SliceCreator } from "./types";

const withoutScripts = ({ scripts: _scripts, ...rest }: ProjectSnapshot) => rest;

export const isDirty = (b: FileBuffer | undefined) => !!b && b.source !== b.saved;

export const createProjectSlice: SliceCreator<ProjectSlice> = (set, get, api) => ({
  project: null,
  buffers: {},
  loading: false,

  async openProject(root) {
    set({ loading: true });
    try {
      const snap = await api.project.open(root);
      const buffers = Object.fromEntries(snap.scripts.map((f) => [f.path, { ...f, saved: f.source }]));
      const first = snap.scripts[0]?.path;
      set({
        project: withoutScripts(snap),
        buffers,
        tabs: [{ id: "graph", kind: "graph" }, ...(first ? [{ id: first, kind: "file" as const, path: first }] : [])],
        activeId: first ?? "graph",
        cursor: null,
        reveal: null,
        playRequest: null,
      });
      get().clearHistory();
      await get().loadGraphLayout();
      await get().analyze();
    } finally {
      set({ loading: false });
    }
  },

  async refreshTree() {
    const snap = await api.project.reload();
    if (!snap) return;
    set((s) => {
      const buffers = { ...s.buffers };
      for (const f of snap.scripts) {
        if (!buffers[f.path] || !isDirty(buffers[f.path])) buffers[f.path] = { ...f, saved: f.source };
      }
      return { project: withoutScripts(snap), buffers };
    });
    await get().analyze();
  },

  setSource(path, source) {
    const current = get().buffers[path];
    if (!current || current.source === source) return;
    set((s) => ({ buffers: { ...s.buffers, [path]: { ...current, source } } }));
    get().record(
      {
        files: { [path]: { before: current.source, after: source } },
        kind: "code",
        at: Date.now(),
      },
      true,
    );
    get().scheduleAnalyze();
  },

  async save(path) {
    const buffer = get().buffers[path];
    if (!buffer) return;
    await api.project.writeFile(path, buffer.source);
    set((s) => ({
      buffers: { ...s.buffers, [path]: { ...buffer, saved: buffer.source } },
      previewVersion: s.previewVersion + 1,
    }));
    await get().analyze();
  },

  async saveAll() {
    const dirty = Object.values(get().buffers).filter(isDirty);
    for (const b of dirty) await get().save(b.path);
  },

  insertAtCursor(statement, line) {
    const { cursor, activeId, tabs, buffers } = get();
    const active = tabs.find((t) => t.id === activeId);
    const path = active?.kind === "file" ? active.path : cursor?.path;
    const buffer = path ? buffers[path] : undefined;
    if (!path || !buffer) return false;
    const lines = buffer.source.split("\n");
    const at = Math.min(line ?? (cursor?.path === path ? cursor.line : lines.length), lines.length);
    const ref = lines[at - 1] ?? "";
    const indent = (ref.match(/^\s*/)?.[0] ?? "") + (ref.trimEnd().endsWith(":") ? "    " : "");
    lines.splice(at, 0, indent + statement);
    get().setSource(path, lines.join("\n"));
    get().openFile(path, at + 1);
    return true;
  },
});

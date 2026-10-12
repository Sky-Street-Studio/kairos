import { insertStatement } from "@shared/edits";
import type { EditResult } from "@shared/types";

import type { FileBuffer, ProjectSlice, SliceCreator } from "./types";

export const isDirty = (b: FileBuffer | undefined) => !!b && b.source !== b.saved;

export const createProjectSlice: SliceCreator<ProjectSlice> = (set, get, api) => {
  const applied = async (result: EditResult) => {
    set((s) => ({
      buffers: { ...s.buffers, [result.path]: { path: result.path, source: result.source, saved: result.source } },
    }));
    set((s) => ({ previewVersion: s.previewVersion + 1 }));
    await get().analyze();
  };

  return {
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
          project: { root: snap.root, manifest: snap.manifest, tree: snap.tree, assets: snap.assets },
          buffers,
          tabs: [{ id: "graph", kind: "graph" }, ...(first ? [{ id: first, kind: "file" as const, path: first }] : [])],
          activeId: first ?? "graph",
          cursor: null,
          reveal: null,
          playRequest: null,
        });
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
        for (const f of snap.scripts) buffers[f.path] ??= { ...f, saved: f.source };
        return { project: { root: snap.root, manifest: snap.manifest, tree: snap.tree, assets: snap.assets }, buffers };
      });
      await get().analyze();
    },

    setSource(path, source) {
      const current = get().buffers[path];
      if (!current || current.source === source) return;
      set((s) => ({ buffers: { ...s.buffers, [path]: { ...current, source } } }));
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

    async connectLabels(from, to) {
      const node = get().analysis.graph.nodes.find((n) => n.label === from);
      if (!node || from === to) return false;
      const buffer = get().buffers[node.path];
      const result = await api.project.applyEdit({
        kind: "connect",
        path: node.path,
        source: isDirty(buffer) ? buffer?.source : undefined,
        from,
        to,
      });
      await applied(result);
      return result.changed;
    },

    async saveCharacter(character, at) {
      const path = at?.path ?? Object.keys(get().buffers).toSorted()[0];
      if (!path) throw new Error("Add a script file before declaring characters.");
      const buffer = get().buffers[path];
      await applied(
        await api.project.applyEdit({
          kind: "character",
          path,
          source: isDirty(buffer) ? buffer?.source : undefined,
          line: at?.line,
          character,
        }),
      );
    },

    insertAtCursor(statement, line) {
      const { cursor, activeId, tabs, buffers } = get();
      const active = tabs.find((t) => t.id === activeId);
      const path = active?.kind === "file" ? active.path : cursor?.path;
      const buffer = path ? buffers[path] : undefined;
      if (!path || !buffer) return false;
      const at = line ?? (cursor?.path === path ? cursor.line : buffer.source.split("\n").length);
      const next = insertStatement(buffer.source, at, statement);
      get().setSource(path, next.source);
      get().openFile(path, next.line);
      return true;
    },
  };
};

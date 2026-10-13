import type { Applied, EditorLayout } from "@shared/edit-types";

import type { EditSlice, SliceCreator } from "./types";

const EMPTY_LAYOUT: EditorLayout = { graph: {} };

export const createEditSlice: SliceCreator<EditSlice> = (set, get, api) => ({
  editError: null,
  graphLayout: EMPTY_LAYOUT,

  async edit(ops) {
    const sent = Object.values(get().buffers).map(({ path, source }) => ({ path, source }));
    let applied: Applied;
    try {
      applied = await api.edit.apply(sent, ops);
    } catch (e) {
      set({ editError: e instanceof Error ? e.message : String(e) });
      return null;
    }
    const buffers = get().buffers;
    if (sent.some((f) => buffers[f.path]?.source !== f.source)) return null;
    const files: Record<string, { before: string; after: string }> = {};
    set((s) => {
      const next = { ...s.buffers };
      for (const file of applied.files) {
        const buffer = next[file.path];
        if (!buffer) continue;
        files[file.path] = { before: buffer.source, after: file.source };
        next[file.path] = { ...buffer, source: file.source };
      }
      return { buffers: next, editError: null };
    });
    if (Object.keys(files).length > 0) get().record({ files, kind: "visual", at: Date.now() });
    get().scheduleAnalyze();
    return applied.focus;
  },

  async loadGraphLayout() {
    set({ graphLayout: await api.edit.readLayout().catch(() => EMPTY_LAYOUT) });
  },

  setNodePosition(label, x, y) {
    const graphLayout = { graph: { ...get().graphLayout.graph, [label]: { x, y } } };
    set({ graphLayout });
    void api.edit.writeLayout(graphLayout);
  },

  resetGraphLayout() {
    set({ graphLayout: EMPTY_LAYOUT });
    void api.edit.writeLayout(EMPTY_LAYOUT);
  },
});

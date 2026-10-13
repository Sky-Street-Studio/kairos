import type { SliceCreator, TabsSlice } from "./types";

let nonce = 0;

export const createTabsSlice: SliceCreator<TabsSlice> = (set, get) => ({
  tabs: [],
  activeId: null,
  reveal: null,
  assetReveal: null,
  cursor: null,

  openFile(path, line) {
    set((s) => ({
      tabs: s.tabs.some((t) => t.id === path) ? s.tabs : [...s.tabs, { id: path, kind: "file", path }],
      activeId: path,
      reveal: line === undefined ? s.reveal : { path, line, nonce: ++nonce },
    }));
  },

  openView(kind) {
    set((s) => ({
      tabs: s.tabs.some((t) => t.id === kind) ? s.tabs : [...s.tabs, { id: kind, kind }],
      activeId: kind,
    }));
  },

  showAsset(path) {
    get().openView("assets");
    set({ assetReveal: path });
  },

  closeTab(id) {
    set((s) => {
      const index = s.tabs.findIndex((t) => t.id === id);
      if (index < 0) return s;
      const tabs = s.tabs.filter((t) => t.id !== id);
      const activeId = s.activeId === id ? (tabs[Math.min(index, tabs.length - 1)]?.id ?? null) : s.activeId;
      return { tabs, activeId };
    });
  },

  setActive(id) {
    set({ activeId: id });
  },

  setCursor(path, line) {
    const c = get().cursor;
    if (c?.path !== path || c.line !== line) set({ cursor: { path, line } });
  },

  goToLabel(name) {
    const node = get().analysis.graph.nodes.find((n) => n.label === name);
    if (!node) return false;
    get().openFile(node.path, node.line);
    return true;
  },
});

import { DEFAULT_THEME } from "@kairos/theme";
import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";

import type { KairosApi, LayoutPrefs } from "@shared/types";

import { createEditSlice } from "./edit-slice";
import { createHistorySlice } from "./history-slice";
import { createProjectSlice } from "./project-slice";
import { createTabsSlice } from "./tabs-slice";
import type { AnalysisSlice, AppState, PreviewSlice, SliceCreator, UiSlice } from "./types";

export type { AppState, FileBuffer, Tab, ViewKind } from "./types";
export { isDirty } from "./project-slice";

export const EMPTY_ANALYSIS = {
  diagnostics: [],
  outlines: {},
  graph: { nodes: [], edges: [], missing: [] },
  assetUsage: {},
};
export const ANALYZE_DELAY_MS = 250;

export const DEFAULT_LAYOUT: LayoutPrefs = {
  sidebar: true,
  preview: true,
  bottom: false,
  sidebarWidth: 260,
  previewWidth: 400,
  bottomHeight: 240,
};

const createAnalysisSlice: SliceCreator<AnalysisSlice> = (set, get, api) => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let generation = 0;
  return {
    analysis: EMPTY_ANALYSIS,
    async analyze() {
      clearTimeout(timer);
      const { buffers, project } = get();
      const files = Object.values(buffers).map(({ path, source }) => ({ path, source }));
      const assets = project?.assets.map((asset) => asset.path) ?? [];
      const spriteLayers = Object.fromEntries(
        Object.entries(project?.spriteLayers ?? {}).map(([tag, { order, defaults }]) => [tag, { order, defaults }]),
      );
      const mine = ++generation;
      const analysis = await api.analysis.analyze({ files, assets, spriteLayers });
      if (mine === generation) set({ analysis });
    },
    scheduleAnalyze() {
      clearTimeout(timer);
      timer = setTimeout(() => void get().analyze(), ANALYZE_DELAY_MS);
    },
  };
};

const createUiSlice: SliceCreator<UiSlice> = (set, get, api) => {
  const persistLayout = () => void api.settings.update({ layout: get().layout });
  return {
    layout: DEFAULT_LAYOUT,
    paletteOpen: false,
    setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
    togglePanel(panel, open) {
      set((s) => ({ layout: { ...s.layout, [panel]: open ?? !s.layout[panel] } }));
      persistLayout();
    },
    resizePanel(key, size) {
      set((s) => ({ layout: { ...s.layout, [key]: Math.round(size) } }));
      persistLayout();
    },
    theme: DEFAULT_THEME,
    setTheme(theme) {
      set({ theme });
      void api.settings.update({ theme });
    },
  };
};

let playNonce = 0;

const createPreviewSlice: SliceCreator<PreviewSlice> = (set) => ({
  previewVersion: 0,
  playRequest: null,
  debugState: null,
  debugControl: null,
  setDebugState(debugState) {
    set({ debugState });
  },
  setDebugControl(debugControl) {
    set({ debugControl });
  },
  playFromLine(path, line, overrides) {
    set((s) => ({
      playRequest: { path, line, overrides, nonce: ++playNonce, mode: "play" },
      layout: { ...s.layout, preview: true },
    }));
  },
  previewAt(path, line) {
    set((s) => ({
      playRequest: { path, line, nonce: ++playNonce, mode: "seek" },
      layout: { ...s.layout, preview: true },
    }));
  },
});

export function createAppStore(api: KairosApi): StoreApi<AppState> {
  return createStore<AppState>()((set, get) => ({
    ...createProjectSlice(set, get, api),
    ...createAnalysisSlice(set, get, api),
    ...createTabsSlice(set, get, api),
    ...createUiSlice(set, get, api),
    ...createPreviewSlice(set, get, api),
    ...createHistorySlice(set, get, api),
    ...createEditSlice(set, get, api),
  }));
}

let store: StoreApi<AppState> | undefined;

export function appStore(): StoreApi<AppState> {
  store ??= createAppStore(window.kairos);
  return store;
}

export function useApp<T>(selector: (s: AppState) => T): T {
  return useStore(appStore(), selector);
}

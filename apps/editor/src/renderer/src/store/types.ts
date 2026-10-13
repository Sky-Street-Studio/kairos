import type { ThemeConfig } from "@kairos/theme";
import type { StoreApi } from "zustand";

import type { Analysis, CharacterFields, KairosApi, LayoutPrefs, ProjectSnapshot } from "@shared/types";

export interface FileBuffer {
  path: string;
  source: string;
  saved: string;
}

export type ViewKind = "graph" | "characters" | "settings" | "assets";

export type Tab = { id: string; kind: "file"; path: string } | { id: ViewKind; kind: ViewKind };

export interface Reveal {
  path: string;
  line: number;
  nonce: number;
}

export interface ProjectSlice {
  project: Omit<ProjectSnapshot, "scripts"> | null;
  buffers: Record<string, FileBuffer>;
  loading: boolean;
  openProject(root: string): Promise<void>;
  refreshTree(): Promise<void>;
  setSource(path: string, source: string): void;
  save(path: string): Promise<void>;
  saveAll(): Promise<void>;
  connectLabels(from: string, to: string): Promise<boolean>;
  saveCharacter(character: CharacterFields, at?: { path: string; line?: number }): Promise<void>;
  insertAtCursor(statement: string, line?: number): boolean;
}

export interface AnalysisSlice {
  analysis: Analysis;
  analyze(): Promise<void>;
  scheduleAnalyze(): void;
}

export interface TabsSlice {
  tabs: Tab[];
  activeId: string | null;
  reveal: Reveal | null;
  /** An asset the asset manager should scroll to; it clears this once shown. */
  assetReveal: string | null;
  cursor: { path: string; line: number } | null;
  openFile(path: string, line?: number): void;
  openView(kind: ViewKind): void;
  showAsset(path: string): void;
  closeTab(id: string): void;
  setActive(id: string): void;
  setCursor(path: string, line: number): void;
  goToLabel(name: string): boolean;
}

export type PanelKey = "sidebar" | "preview" | "bottom";

export interface UiSlice {
  layout: LayoutPrefs;
  paletteOpen: boolean;
  setPaletteOpen(open: boolean): void;
  togglePanel(panel: PanelKey, open?: boolean): void;
  resizePanel(key: "sidebarWidth" | "previewWidth" | "bottomHeight", size: number): void;
  theme: ThemeConfig;
  setTheme(theme: ThemeConfig): void;
}

export interface DebugLocation {
  label: string;
  path: string;
  line: number;
}

export interface DebugState extends DebugLocation {
  variables: Record<string, unknown>;
  callStack: DebugLocation[];
  history: number;
  position: number;
}

export interface DebugControl {
  setVariable(name: string, json: string): void;
  rollbackTo(position: number): void;
}

export interface PreviewSlice {
  /** Bumped after every save so the preview reloads the project. */
  previewVersion: number;
  playRequest: (Reveal & { overrides?: Record<string, unknown> }) | null;
  debugState: DebugState | null;
  debugControl: DebugControl | null;
  setDebugState(state: DebugState | null): void;
  setDebugControl(control: DebugControl | null): void;
  playFromLine(path: string, line: number, overrides?: Record<string, unknown>): void;
}

export type AppState = ProjectSlice & AnalysisSlice & TabsSlice & UiSlice & PreviewSlice;

export type SliceCreator<T> = (
  set: StoreApi<AppState>["setState"],
  get: StoreApi<AppState>["getState"],
  api: KairosApi,
) => T;

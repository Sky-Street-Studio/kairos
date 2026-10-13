import type { ThemeConfig } from "@kairos/theme";

export interface SourceFile {
  path: string;
  source: string;
}

export interface FileDiagnostic {
  path: string;
  line: number;
  message: string;
}

export interface OutlineLabel {
  name: string;
  line: number;
}

export interface OutlineCharacter {
  id: string;
  name: string;
  color?: string;
  line: number;
}

export interface Outline {
  labels: OutlineLabel[];
  characters: OutlineCharacter[];
}

export type EdgeKind = "jump" | "call" | "choice" | "fallthrough";

export interface GraphNode {
  label: string;
  path: string;
  line: number;
}

export interface GraphEdge {
  from: string;
  to: string;
  kind: EdgeKind;
  text?: string;
}

export interface StoryGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface Manifest {
  name: string;
  start: string;
  window?: { width: number; height: number };
}

export type AssetKind = "image" | "audio" | "other";
export type AssetCategory = "background" | "sprite" | "ui" | "music" | "sound" | "voice" | "other";

export interface AssetEntry {
  /** Project-relative, always starting with `assets/`. */
  path: string;
  kind: AssetKind;
  category: AssetCategory;
  characterId?: string;
  /** What a script writes to use the asset: `bg room` for images, `theme.ogg` for audio. */
  name: string;
}

export interface TreeNode {
  name: string;
  path: string;
  kind: "dir" | "file";
  children?: TreeNode[];
}

export interface SpriteLayerConfig {
  order: string[];
  defaults: Record<string, string>;
  groups: Record<string, string[]>;
}

export interface TranslationFile {
  lang: string;
  source: string;
}

export type ExportTarget = "web" | "desktop";

export interface ProjectSnapshot {
  root: string;
  manifest: Manifest;
  /** Raw `kairos.toml`, which the player parses itself. */
  manifestSource: string;
  /** Raw `ui/theme.toml`, if present. */
  uiTheme: string | null;
  /** Raw `ui/title.toml`, if present. */
  uiTitle: string | null;
  scripts: SourceFile[];
  translations: TranslationFile[];
  tree: TreeNode[];
  assets: AssetEntry[];
  audioMetadata?: Record<string, { loopStart?: number; loopEnd?: number }>;
  spriteLayers?: Record<string, SpriteLayerConfig>;
}

export interface CharacterFields {
  id: string;
  name: string;
  color?: string;
}

export type ProjectEdit =
  | { kind: "connect"; path: string; source?: string; from: string; to: string }
  | { kind: "character"; path: string; source?: string; line?: number; character: CharacterFields };

export interface EditResult {
  path: string;
  source: string;
  changed: boolean;
}

export interface AnalyzeInput {
  files: SourceFile[];
  /** Project-relative asset paths, e.g. `assets/images/bg room.png`. */
  assets: string[];
  spriteLayers: Record<string, Pick<SpriteLayerConfig, "order" | "defaults">>;
}

export interface Analysis {
  diagnostics: FileDiagnostic[];
  outlines: Record<string, Outline>;
  graph: StoryGraph;
  /** References per asset path, counting only the file the player would load. */
  assetUsage: Record<string, number>;
}

export interface PlayerStatus {
  available: boolean;
  moduleUrl: string;
  buildHint: string;
}

export interface AgentInfo {
  id: "claude" | "codex";
  name: string;
  available: boolean;
}

export interface TerminalSpawn {
  cols: number;
  rows: number;
  agent?: AgentInfo["id"];
}

export interface LayoutPrefs {
  sidebar: boolean;
  preview: boolean;
  bottom: boolean;
  sidebarWidth: number;
  previewWidth: number;
  bottomHeight: number;
}

export interface Settings {
  theme: ThemeConfig;
  recentProjects: string[];
  layout: LayoutPrefs;
}

export interface KairosApi {
  platform: string;
  project: {
    /** The project named by `--project=<dir>` or `KAIROS_PROJECT`, else the most recent one. */
    initial(): Promise<string | null>;
    pick(): Promise<string | null>;
    open(root: string): Promise<ProjectSnapshot>;
    reload(): Promise<ProjectSnapshot | null>;
    /** Reads a UTF-8 file inside the project for read-only preview; refuses files over 1 MB. */
    readText(path: string): Promise<string>;
    writeFile(path: string, content: string): Promise<void>;
    importAsset(sourcePath: string | null, folder: string, convertWebp?: boolean): Promise<string | null>;
    renameAsset(path: string, name: string): Promise<void>;
    applyEdit(edit: ProjectEdit): Promise<EditResult>;
    createFromTemplate(): Promise<string | null>;
    pickExportFolder(): Promise<string | null>;
    export(target: ExportTarget, output: string): Promise<string>;
  };
  analysis: {
    analyze(input: AnalyzeInput): Promise<Analysis>;
  };
  player: {
    status(): Promise<PlayerStatus>;
  };
  terminal: {
    agents(): Promise<AgentInfo[]>;
    spawn(opts: TerminalSpawn): Promise<number>;
    write(id: number, data: string): void;
    resize(id: number, cols: number, rows: number): void;
    kill(id: number): void;
    onData(listener: (id: number, data: string) => void): () => void;
    onExit(listener: (id: number, code: number) => void): () => void;
  };
  settings: {
    get(): Promise<Settings>;
    update(patch: Partial<Settings>): Promise<Settings>;
    exportTheme(json: string): Promise<boolean>;
    importTheme(): Promise<string | null>;
  };
}

export const ASSET_SCHEME = "kairos-asset";
export const PLAYER_SCHEME = "kairos-player";

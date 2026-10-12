import { ASSET_SCHEME, type SourceFile } from "@shared/types";

/** The surface `crates/kairos-player` exports when built with `wasm-bindgen --target web`. */
export interface WebPlayer {
  ready(): Promise<void>;
  load(projectJson: string): void;
  start(): void;
  playFromLine(path: string, line: number): void;
  resize(width: number, height: number): void;
  destroy(): void;
}

interface PlayerModule {
  default(): Promise<unknown>;
  WebPlayer: new (canvas: HTMLCanvasElement) => WebPlayer;
}

let modulePromise: Promise<PlayerModule> | undefined;

export function loadPlayerModule(url: string): Promise<PlayerModule> {
  modulePromise ??= import(/* @vite-ignore */ url).then(async (m: PlayerModule) => {
    await m.default();
    return m;
  });
  modulePromise.catch(() => (modulePromise = undefined));
  return modulePromise;
}

export function projectJson(
  manifestSource: string,
  scripts: SourceFile[],
  uiTheme: string | null,
  uiTitle: string | null,
): string {
  return JSON.stringify({
    manifest: manifestSource,
    scripts,
    assetBaseUrl: `${ASSET_SCHEME}://project/`,
    ...(uiTheme === null ? {} : { theme: uiTheme }),
    ...(uiTitle === null ? {} : { title: uiTitle }),
  });
}

export interface PlayerError {
  message: string;
  path?: string;
  line?: number;
}

/** The player throws strings shaped like `script/a.kai:12: message`. */
export function parsePlayerError(e: unknown): PlayerError {
  const text = e instanceof Error ? e.message : String(e);
  const m = /^(.+?\.kai):(\d+): (.*)$/s.exec(text);
  return m ? { path: m[1], line: Number(m[2]), message: m[3]! } : { message: text };
}

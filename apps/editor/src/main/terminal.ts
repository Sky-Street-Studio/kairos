import { execFileSync } from "node:child_process";
import { accessSync, chmodSync, constants } from "node:fs";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";

import type { WebContents } from "electron";
import type * as Pty from "node-pty";

import type { AgentInfo, TerminalSpawn } from "@shared/types";

import { projectRoot } from "./project";

const require = createRequire(import.meta.url);
const pty = require("node-pty") as typeof Pty;

/** Bun's installer drops the exec bit on node-pty's prebuilt spawn-helper, and spawning then fails silently. */
function ensureSpawnHelperExecutable(): void {
  if (process.platform === "win32") return;
  const helper = join(
    dirname(require.resolve("node-pty/package.json")),
    "prebuilds",
    `${process.platform}-${process.arch}`,
    "spawn-helper",
  );
  try {
    accessSync(helper, constants.X_OK);
  } catch {
    try {
      chmodSync(helper, 0o755);
    } catch {
      // No prebuilt helper: node-pty was compiled from source and has its own.
    }
  }
}

const AGENTS: { id: AgentInfo["id"]; name: string }[] = [
  { id: "claude", name: "Claude Code" },
  { id: "codex", name: "Codex" },
];

const shell = () => process.env.SHELL ?? (process.platform === "win32" ? "powershell.exe" : "/bin/zsh");

let cachedPath: string | undefined;

/** Apps launched from the Dock get a bare PATH, so ask the login shell for the user's real one. */
function userPath(): string {
  if (cachedPath !== undefined) return cachedPath;
  const fallback = [process.env.PATH, join(homedir(), ".local/bin"), "/opt/homebrew/bin", "/usr/local/bin"]
    .filter(Boolean)
    .join(delimiter);
  if (process.platform === "win32") return (cachedPath = fallback);
  try {
    const out = execFileSync(shell(), ["-ilc", 'printf "__PATH__%s" "$PATH"'], { encoding: "utf8", timeout: 4000 });
    cachedPath = [out.split("__PATH__").pop()?.trim(), fallback].filter(Boolean).join(delimiter);
  } catch {
    cachedPath = fallback;
  }
  return cachedPath;
}

function onPath(command: string): boolean {
  const exts = process.platform === "win32" ? [".exe", ".cmd", ""] : [""];
  return userPath()
    .split(delimiter)
    .some((dir) =>
      exts.some((ext) => {
        try {
          accessSync(join(dir, command + ext), constants.X_OK);
          return true;
        } catch {
          return false;
        }
      }),
    );
}

export function agents(): AgentInfo[] {
  return AGENTS.map((a) => ({ ...a, available: onPath(a.id) }));
}

const sessions = new Map<number, Map<number, Pty.IPty>>();
const attachedContents = new WeakSet<WebContents>();
let nextId = 1;

let helperChecked = false;

export function spawnTerminal(contents: WebContents, opts: TerminalSpawn): number {
  if (opts.agent && !AGENTS.some((agent) => agent.id === opts.agent)) throw new Error("Unknown terminal agent");
  if (!helperChecked) {
    ensureSpawnHelperExecutable();
    helperChecked = true;
  }
  const owned = sessions.get(contents.id) ?? new Map<number, Pty.IPty>();
  sessions.set(contents.id, owned);
  const id = nextId++;
  const proc = pty.spawn(shell(), [], {
    name: "xterm-256color",
    cols: Math.max(2, opts.cols),
    rows: Math.max(2, opts.rows),
    cwd: projectRoot() ?? homedir(),
    env: { ...process.env, PATH: userPath(), TERM_PROGRAM: "Kairos" } as Record<string, string>,
  });
  owned.set(id, proc);
  if (!attachedContents.has(contents)) {
    attachedContents.add(contents);
    const close = () => {
      for (const sessionId of sessions.get(contents.id)?.keys() ?? []) killTerminal(contents, sessionId);
    };
    contents.once("destroyed", close);
    contents.on("did-start-navigation", (_event, _url, _isInPlace, isMainFrame) => {
      if (isMainFrame) close();
    });
    contents.on("render-process-gone", close);
  }
  proc.onData((data) => {
    if (!contents.isDestroyed()) contents.send("terminal:data", id, data);
  });
  proc.onExit(({ exitCode }) => {
    owned.delete(id);
    if (owned.size === 0) sessions.delete(contents.id);
    if (!contents.isDestroyed()) contents.send("terminal:exit", id, exitCode);
  });
  if (opts.agent) proc.write(`${opts.agent}\r`);
  return id;
}

export function writeTerminal(contents: WebContents, id: number, data: string): void {
  sessions.get(contents.id)?.get(id)?.write(data);
}

export function resizeTerminal(contents: WebContents, id: number, cols: number, rows: number): void {
  sessions.get(contents.id)?.get(id)?.resize(Math.max(2, cols), Math.max(2, rows));
}

export function killTerminal(contents: WebContents, id: number): void {
  const owned = sessions.get(contents.id);
  owned?.get(id)?.kill();
  owned?.delete(id);
  if (owned?.size === 0) sessions.delete(contents.id);
}

export function killAllTerminals(): void {
  for (const [contentsId, owned] of sessions) {
    for (const proc of owned.values()) proc.kill();
    sessions.delete(contentsId);
  }
}

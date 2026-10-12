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

const sessions = new Map<number, Pty.IPty>();
let nextId = 1;

let helperChecked = false;

export function spawnTerminal(contents: WebContents, opts: TerminalSpawn): number {
  if (!helperChecked) {
    ensureSpawnHelperExecutable();
    helperChecked = true;
  }
  const id = nextId++;
  const proc = pty.spawn(shell(), [], {
    name: "xterm-256color",
    cols: Math.max(2, opts.cols),
    rows: Math.max(2, opts.rows),
    cwd: projectRoot() ?? homedir(),
    env: { ...process.env, PATH: userPath(), TERM_PROGRAM: "Kairos" } as Record<string, string>,
  });
  sessions.set(id, proc);
  proc.onData((data) => {
    if (!contents.isDestroyed()) contents.send("terminal:data", id, data);
  });
  proc.onExit(({ exitCode }) => {
    sessions.delete(id);
    if (!contents.isDestroyed()) contents.send("terminal:exit", id, exitCode);
  });
  if (opts.agent) proc.write(`${opts.agent}\r`);
  return id;
}

export function writeTerminal(id: number, data: string): void {
  sessions.get(id)?.write(data);
}

export function resizeTerminal(id: number, cols: number, rows: number): void {
  sessions.get(id)?.resize(Math.max(2, cols), Math.max(2, rows));
}

export function killTerminal(id: number): void {
  sessions.get(id)?.kill();
  sessions.delete(id);
}

export function killAllTerminals(): void {
  for (const id of sessions.keys()) killTerminal(id);
}

import { EventEmitter } from "node:events";

import { beforeEach, describe, expect, it, vi } from "vitest";

const { pty, fakeProc } = vi.hoisted(() => {
  const createdProc = {
    onData: vi.fn(),
    onExit: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
    kill: vi.fn(),
  };
  return { fakeProc: createdProc, pty: { spawn: vi.fn(() => createdProc) } };
});

vi.mock("node:module", () => ({
  createRequire: () => Object.assign(() => pty, { resolve: () => "/missing/node-pty/package.json" }),
}));
vi.mock("./project", () => ({ projectRoot: () => "/project" }));

import { killTerminal, spawnTerminal, writeTerminal } from "./terminal";

function contents(id: number) {
  const emitter = new EventEmitter() as EventEmitter & {
    id: number;
    isDestroyed: () => boolean;
    send: (...args: unknown[]) => void;
  };
  emitter.id = id;
  emitter.isDestroyed = () => false;
  emitter.send = vi.fn();
  return emitter;
}

describe("terminal ownership", () => {
  beforeEach(() => {
    pty.spawn.mockClear();
    fakeProc.onData.mockClear();
    fakeProc.onExit.mockClear();
    fakeProc.write.mockClear();
    fakeProc.resize.mockClear();
    fakeProc.kill.mockClear();
  });

  it("accepts only known agents and confines sessions to their webContents", () => {
    const owner = contents(11);
    const stranger = contents(12);

    expect(() => spawnTerminal(owner as never, { cols: 80, rows: 24, agent: "unknown" as never })).toThrow(
      "Unknown terminal agent",
    );
    expect(pty.spawn).not.toHaveBeenCalled();

    const id = spawnTerminal(owner as never, { cols: 80, rows: 24 });
    writeTerminal(stranger as never, id, "echo foreign");
    expect(fakeProc.write).not.toHaveBeenCalled();
    writeTerminal(owner as never, id, "echo owner");
    expect(fakeProc.write).toHaveBeenCalledWith("echo owner");
    killTerminal(stranger as never, id);
    expect(fakeProc.kill).not.toHaveBeenCalled();
    killTerminal(owner as never, id);
    expect(fakeProc.kill).toHaveBeenCalledTimes(1);
  });

  it("kills owned sessions on main-frame navigation", () => {
    const owner = contents(21);
    spawnTerminal(owner as never, { cols: 80, rows: 24 });

    owner.emit("did-start-navigation", {}, "file:///other.html", false, true);

    expect(fakeProc.kill).toHaveBeenCalledTimes(1);
  });
});

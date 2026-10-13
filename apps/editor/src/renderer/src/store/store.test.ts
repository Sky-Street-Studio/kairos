import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditOp } from "@shared/edit-types";
import type { Analysis, AnalyzeInput, KairosApi, ProjectSnapshot, SourceFile } from "@shared/types";

import { ANALYZE_DELAY_MS, createAppStore, isDirty } from "./index";

const SCRIPT = 'character e "Eileen"\nlabel start:\n    e "Hi."\nlabel end:\n    return\n';

const insertJump = (source: string, from: string, to: string) =>
  source.replace(new RegExp(`(label ${from}:\\n(?:    .*\\n)*)`), `$1    jump ${to}\n`);

function fakeApi() {
  const disk = new Map<string, string>([
    ["script/a.kai", SCRIPT],
    ["script/b.kai", "label other:\n    return\n"],
  ]);
  const analyzed: AnalyzeInput[] = [];
  const snapshot = (): ProjectSnapshot => ({
    root: "/p",
    manifest: { name: "P", start: "start" },
    manifestSource: 'name = "P"\n',
    uiTheme: null,
    uiTitle: null,
    scripts: [...disk].map(([path, source]) => ({ path, source })),
    translations: [],
    tree: [],
    assets: [{ path: "assets/images/bg room.png", kind: "image", name: "bg room", category: "background" }],
    spriteLayers: { e: { order: ["face"], defaults: { face: "happy" }, groups: { face: ["happy"] } } },
  });
  const api = {
    platform: "darwin",
    project: {
      initial: vi.fn(async () => null),
      pick: vi.fn(async () => null),
      open: vi.fn(async () => snapshot()),
      reload: vi.fn(async () => snapshot()),
      writeFile: vi.fn(async (path: string, content: string) => void disk.set(path, content)),
      importAsset: vi.fn(async () => null),
      renameAsset: vi.fn(async () => undefined),
      createFromTemplate: vi.fn(async () => null),
      pickExportFolder: vi.fn(async () => null),
      export: vi.fn(async () => "/p/export"),
    },
    analysis: {
      analyze: vi.fn(async (input: AnalyzeInput): Promise<Analysis> => {
        analyzed.push(input);
        const { files } = input;
        return {
          diagnostics: files.flatMap((f) =>
            f.source.includes("oops") ? [{ path: f.path, line: 1, message: "bad" }] : [],
          ),
          outlines: {},
          graph: {
            nodes: [
              { label: "start", path: "script/a.kai", line: 2 },
              { label: "end", path: "script/a.kai", line: 4 },
            ],
            edges: [],
          },
          assetUsage: {},
        };
      }),
    },
    edit: {
      blocks: vi.fn(async () => null),
      apply: vi.fn(async (files: SourceFile[], ops: EditOp[]) => {
        const op = ops[0]!;
        if (op.op === "endScene" && op.label === "missing") throw new Error("there is no scene `missing`");
        const file = files.find((f) => f.path === "script/a.kai")!;
        if (op.op === "connect")
          return {
            files: [{ path: file.path, source: insertJump(file.source, op.from, op.to) }],
            focus: { path: file.path, line: 3 },
          };
        return {
          files: [{ path: file.path, source: `${file.source}# edited\n` }],
          focus: { path: file.path, line: 1 },
        };
      }),
      references: vi.fn(async () => []),
      labelFromName: vi.fn(async (name: string) => name.toLowerCase().replace(/\W+/g, "_")),
      readLayout: vi.fn(async () => ({ graph: { start: { x: 5, y: 6 } } })),
      writeLayout: vi.fn(async () => undefined),
      setStart: vi.fn(async () => undefined),
    },
    player: { status: vi.fn() },
    terminal: {} as KairosApi["terminal"],
    settings: {
      get: vi.fn(),
      update: vi.fn(async () => ({}) as never),
      exportTheme: vi.fn(),
      importTheme: vi.fn(),
    },
  } satisfies KairosApi;
  return { api, disk, analyzed };
}

describe("app store", () => {
  let env: ReturnType<typeof fakeApi>;
  let store: ReturnType<typeof createAppStore>;

  beforeEach(async () => {
    env = fakeApi();
    store = createAppStore(env.api);
    await store.getState().openProject("/p");
  });

  afterEach(() => vi.useRealTimers());

  it("opens a project with the graph and first script as tabs", () => {
    const s = store.getState();
    expect(s.project?.manifest.name).toBe("P");
    expect(s.tabs.map((t) => t.id)).toEqual(["graph", "script/a.kai"]);
    expect(s.activeId).toBe("script/a.kai");
    expect(s.analysis.graph.nodes).toHaveLength(2);
  });

  it("re-analyzes unsaved buffers after a pause in typing", async () => {
    vi.useFakeTimers();
    store.getState().setSource("script/a.kai", "oops");
    expect(isDirty(store.getState().buffers["script/a.kai"])).toBe(true);
    expect(env.analyzed).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(ANALYZE_DELAY_MS);
    expect(env.analyzed[1]?.files.find((f) => f.path === "script/a.kai")?.source).toBe("oops");
    expect(store.getState().analysis.diagnostics).toEqual([{ path: "script/a.kai", line: 1, message: "bad" }]);
  });

  it("analyzes scripts against the project's assets and sprite layers", () => {
    const [input] = env.analyzed;
    expect(input?.assets).toEqual(["assets/images/bg room.png"]);
    expect(input?.spriteLayers).toEqual({ e: { order: ["face"], defaults: { face: "happy" } } });
  });

  it("saves to disk, clears dirty and bumps the preview", async () => {
    store.getState().setSource("script/a.kai", SCRIPT + "# more\n");
    await store.getState().save("script/a.kai");
    expect(env.disk.get("script/a.kai")).toBe(SCRIPT + "# more\n");
    expect(isDirty(store.getState().buffers["script/a.kai"])).toBe(false);
    expect(store.getState().previewVersion).toBe(1);
  });

  it("connects labels through a visual edit", async () => {
    await store.getState().edit([{ op: "connect", from: "start", to: "end", choice: null }]);
    expect(store.getState().buffers["script/a.kai"]!.source).toContain("jump end");
  });

  it("applies visual edits to buffers and undoes them with code edits in one history", async () => {
    await store.getState().edit([{ op: "endScene", label: "start" }]);
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
    store.getState().setSource("script/a.kai", `${SCRIPT}# edited\n# typed\n`);
    expect(store.getState().canUndo).toBe(true);
    store.getState().undo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
    store.getState().undo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
    store.getState().redo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
  });

  it("coalesces typing into one history entry per pause", () => {
    vi.useFakeTimers();
    store.getState().setSource("script/a.kai", `${SCRIPT}a`);
    store.getState().setSource("script/a.kai", `${SCRIPT}ab`);
    vi.advanceTimersByTime(1500);
    store.getState().setSource("script/a.kai", `${SCRIPT}abc`);
    store.getState().undo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}ab`);
    store.getState().undo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
  });

  it("reports rejected edits without touching buffers", async () => {
    expect(await store.getState().edit([{ op: "endScene", label: "missing" }])).toBeNull();
    expect(store.getState().editError).toBe("there is no scene `missing`");
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
  });

  it("drops edits whose buffers changed while they were in flight", async () => {
    const pending = store.getState().edit([{ op: "endScene", label: "start" }]);
    store.getState().setSource("script/a.kai", `${SCRIPT}# typed meanwhile\n`);
    expect(await pending).toBeNull();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# typed meanwhile\n`);
  });

  it("loads and persists graph positions", async () => {
    expect(store.getState().graphLayout.graph.start).toEqual({ x: 5, y: 6 });
    store.getState().setNodePosition("end", 1, 2);
    expect(env.api.edit.writeLayout).toHaveBeenCalledWith({ graph: { start: { x: 5, y: 6 }, end: { x: 1, y: 2 } } });
  });

  it("resets history when a project opens", async () => {
    await store.getState().edit([{ op: "endScene", label: "start" }]);
    await store.getState().openProject("/p");
    expect(store.getState().canUndo).toBe(false);
  });

  it("opens, reveals and closes tabs", () => {
    const s = store.getState();
    s.openFile("script/b.kai", 3);
    expect(store.getState().reveal).toMatchObject({ path: "script/b.kai", line: 3 });
    s.openView("characters");
    expect(store.getState().activeId).toBe("characters");
    s.closeTab("characters");
    expect(store.getState().activeId).toBe("script/b.kai");
    s.closeTab("graph");
    expect(store.getState().tabs.map((t) => t.id)).toEqual(["script/a.kai", "script/b.kai"]);
    expect(s.goToLabel("end")).toBe(true);
    expect(store.getState().reveal).toMatchObject({ path: "script/a.kai", line: 4 });
    expect(s.goToLabel("missing")).toBe(false);
  });

  it("inserts stage statements at the cursor in the active file", () => {
    const s = store.getState();
    s.setCursor("script/a.kai", 3);
    expect(s.insertAtCursor("scene bg room")).toBe(true);
    expect(store.getState().buffers["script/a.kai"]!.source.split("\n")[3]).toBe("    scene bg room");
    expect(store.getState().reveal).toMatchObject({ path: "script/a.kai", line: 4 });
  });

  it("inserts at the end of a file with no cursor, keeping the final newline", () => {
    expect(store.getState().insertAtCursor("scene bg room")).toBe(true);
    expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}    scene bg room\n`);
    expect(store.getState().reveal).toMatchObject({ path: "script/a.kai", line: 6 });
  });

  it("inserts on a blank line using the indentation above", () => {
    const s = store.getState();
    s.setSource("script/b.kai", "label a:\n    pass\n\nlabel b:\n");
    s.openFile("script/b.kai");
    expect(s.insertAtCursor("scene bg x", 3)).toBe(true);
    expect(store.getState().buffers["script/b.kai"]!.source).toBe("label a:\n    pass\n    scene bg x\n\nlabel b:\n");
    expect(store.getState().reveal).toMatchObject({ path: "script/b.kai", line: 3 });
  });

  it("keeps CRLF line endings when inserting", () => {
    const s = store.getState();
    s.setSource("script/b.kai", "label a:\r\n    pass\r\n");
    s.openFile("script/b.kai");
    expect(s.insertAtCursor("scene bg x", 2)).toBe(true);
    expect(store.getState().buffers["script/b.kai"]!.source).toBe("label a:\r\n    pass\r\n    scene bg x\r\n");
  });

  it("indents under a block header that has a trailing comment", () => {
    const s = store.getState();
    s.setSource("script/b.kai", "label a:  # note\n    pass\n");
    s.openFile("script/b.kai");
    expect(s.insertAtCursor("scene bg x", 1)).toBe(true);
    expect(store.getState().buffers["script/b.kai"]!.source).toBe("label a:  # note\n    scene bg x\n    pass\n");
  });

  it("clears an earlier rejection when a later edit goes stale", async () => {
    await store.getState().edit([{ op: "endScene", label: "missing" }]);
    const pending = store.getState().edit([{ op: "endScene", label: "start" }]);
    store.getState().setSource("script/a.kai", `${SCRIPT}# typed meanwhile\n`);
    expect(await pending).toBeNull();
    expect(store.getState().editError).toBeNull();
  });

  it("clears history instead of undoing over a file changed on disk", async () => {
    await store.getState().edit([{ op: "endScene", label: "start" }]);
    await store.getState().save("script/a.kai");
    env.disk.set("script/a.kai", "label start:\n    return\n");
    await store.getState().refreshTree();
    store.getState().undo();
    expect(store.getState().buffers["script/a.kai"]!.source).toBe("label start:\n    return\n");
    expect(store.getState().editError).toBe("The script changed outside the editor, so undo history was cleared.");
    expect(store.getState().canUndo).toBe(false);
    expect(store.getState().canRedo).toBe(false);
  });

  it("toggles panels, persists layout and opens the preview on play", () => {
    const s = store.getState();
    s.togglePanel("bottom");
    expect(store.getState().layout.bottom).toBe(true);
    expect(env.api.settings.update).toHaveBeenLastCalledWith({ layout: store.getState().layout });
    s.togglePanel("preview", false);
    s.playFromLine("script/a.kai", 3);
    expect(store.getState().layout.preview).toBe(true);
    expect(store.getState().playRequest).toMatchObject({ path: "script/a.kai", line: 3 });
  });
});

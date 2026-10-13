import { ListTree, Play } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/input";
import { Tooltip } from "@/components/ui/tooltip";
import { useResolvedAppearance } from "@/hooks/use-theme";
import { cn, modKey } from "@/lib/utils";
import { appStore, useApp } from "@/store";

import { droppedStatement, isAssetDrag } from "../stage/asset-drag";
import { defineTheme, ensureModel, modelUri, monaco, pathOfUri, registerKairosLanguage } from "./monaco";

registerKairosLanguage();

/** Model edits made to mirror the store must not echo back into it. */
let mirroring = false;

function syncModelsFromStore() {
  const { buffers } = appStore().getState();
  for (const buffer of Object.values(buffers)) {
    const model = monaco.editor.getModel(modelUri(buffer.path));
    if (!model || model.getValue() === buffer.source) continue;
    mirroring = true;
    try {
      model.pushEditOperations([], [{ range: model.getFullModelRange(), text: buffer.source }], () => null);
    } finally {
      mirroring = false;
    }
  }
}

function syncMarkers() {
  const { diagnostics } = appStore().getState().analysis;
  for (const model of monaco.editor.getModels()) {
    const path = pathOfUri(model.uri);
    monaco.editor.setModelMarkers(
      model,
      "kairos",
      diagnostics
        .filter((d) => d.path === path)
        .map((d) => {
          const line = Math.min(Math.max(d.line, 1), model.getLineCount());
          return {
            severity: monaco.MarkerSeverity.Error,
            message: d.message,
            startLineNumber: line,
            startColumn: model.getLineFirstNonWhitespaceColumn(line) || 1,
            endLineNumber: line,
            endColumn: model.getLineMaxColumn(line),
            source: "kairos",
          };
        }),
    );
  }
}

function Outline({ path, line }: { path: string; line: number }) {
  const outline = useApp((s) => s.analysis.outlines[path]);
  const openFile = useApp((s) => s.openFile);
  const labels = outline?.labels ?? [];
  const current = labels.toReversed().find((l) => l.line <= line)?.name;
  return (
    <nav aria-label="Outline" className="w-48 shrink-0 overflow-y-auto border-l bg-background/20 py-2">
      <h2 className="px-3 pb-1 text-2xs font-semibold tracking-wide text-muted-foreground uppercase">Outline</h2>
      {labels.length === 0 && <p className="px-3 text-xs text-muted-foreground">No labels in this file.</p>}
      {labels.map((l) => (
        <button
          key={`${l.name}:${l.line}`}
          type="button"
          onClick={() => openFile(path, l.line)}
          className={cn(
            "flex h-6 w-full items-center gap-2 px-3 text-left text-xs hover:bg-accent",
            current === l.name ? "text-foreground" : "text-muted-foreground",
          )}
        >
          <span className={cn("size-1.5 shrink-0 rounded-full", current === l.name ? "bg-primary" : "bg-border")} />
          <span className="truncate font-mono text-[11.5px]">{l.name}</span>
          <span className="ml-auto text-2xs tabular opacity-70">{l.line}</span>
        </button>
      ))}
      {outline && outline.characters.length > 0 && (
        <>
          <h2 className="px-3 pt-3 pb-1 text-2xs font-semibold tracking-wide text-muted-foreground uppercase">
            Characters
          </h2>
          {outline.characters.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => openFile(path, c.line)}
              className="flex h-6 w-full items-center gap-2 px-3 text-left text-xs text-muted-foreground hover:bg-accent"
            >
              <span
                className="size-2 shrink-0 rounded-full"
                style={{ background: c.color ?? "var(--muted-foreground)" }}
              />
              <span className="truncate">{c.name}</span>
            </button>
          ))}
        </>
      )}
    </nav>
  );
}

export default function EditorView({ path }: { path: string | null }) {
  const host = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const pathRef = useRef<string | null>(path);
  const viewStates = useRef(new Map<string, monaco.editor.ICodeEditorViewState | null>());
  const [showOutline, setShowOutline] = useState(true);
  const [dropLine, setDropLine] = useState<number | null>(null);
  const cursor = useApp((s) => s.cursor);
  const theme = useApp((s) => s.theme);
  const appearance = useResolvedAppearance();
  const playFromLine = useApp((s) => s.playFromLine);
  useLayoutEffect(() => {
    pathRef.current = path;
  }, [path]);

  useEffect(() => {
    const editor = monaco.editor.create(host.current!, {
      language: "kairos",
      automaticLayout: true,
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 21,
      fontLigatures: true,
      padding: { top: 10, bottom: 10 },
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      smoothScrolling: true,
      cursorBlinking: "smooth",
      cursorSmoothCaretAnimation: "on",
      renderLineHighlight: "all",
      lineNumbersMinChars: 3,
      glyphMargin: false,
      folding: true,
      stickyScroll: { enabled: true, maxLineCount: 2 },
      dropIntoEditor: { enabled: false },
      fixedOverflowWidgets: true,
      tabSize: 4,
      insertSpaces: true,
      detectIndentation: false,
      guides: { indentation: true },
      scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10, useShadows: false },
      overviewRulerLanes: 2,
      hideCursorInOverviewRuler: true,
      wordWrap: "on",
      wrappingIndent: "indent",
    });
    editorRef.current = editor;
    void document.fonts.ready.then(() => monaco.editor.remeasureFonts());

    editor.addAction({
      id: "kairos.playFromLine",
      label: "Play from this line",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.Enter],
      contextMenuGroupId: "navigation",
      contextMenuOrder: 0,
      run: (ed) => {
        const p = pathRef.current;
        const line = ed.getPosition()?.lineNumber;
        if (p && line) appStore().getState().playFromLine(p, line);
      },
    });
    editor.addAction({
      id: "kairos.save",
      label: "Save",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS],
      run: () => {
        const p = pathRef.current;
        if (p)
          void appStore()
            .getState()
            .save(p)
            .catch((e: Error) => toast.error("Save failed", { description: e.message }));
      },
    });
    editor.addAction({
      id: "kairos.palette",
      label: "Command palette",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyK],
      run: () => appStore().getState().setPaletteOpen(true),
    });
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyZ, () => appStore().getState().undo());
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyZ, () =>
      appStore().getState().redo(),
    );

    const subs = [
      editor.onDidChangeModelContent(() => {
        const model = editor.getModel();
        if (!mirroring && model) appStore().getState().setSource(pathOfUri(model.uri), model.getValue());
      }),
      editor.onDidChangeCursorPosition((e) => {
        const p = pathRef.current;
        if (p) appStore().getState().setCursor(p, e.position.lineNumber);
      }),
    ];

    const unsubscribe = appStore().subscribe((s, prev) => {
      if (s.buffers !== prev.buffers) syncModelsFromStore();
      if (s.analysis !== prev.analysis) syncMarkers();
      if (s.reveal && s.reveal !== prev.reveal && s.reveal.path === pathRef.current) {
        const { line } = s.reveal;
        queueMicrotask(() => {
          editor.setPosition({ lineNumber: line, column: 1 });
          editor.revealLineInCenterIfOutsideViewport(line);
          editor.focus();
        });
      }
    });

    return () => {
      subs.forEach((d) => d.dispose());
      unsubscribe();
      editor.dispose();
      editorRef.current = null;
    };
  }, []);

  useEffect(() => {
    monaco.editor.setTheme(defineTheme(theme, appearance));
  }, [theme, appearance]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !path) return;
    const state = appStore().getState();
    const buffer = state.buffers[path];
    if (!buffer) return;
    for (const b of Object.values(state.buffers)) ensureModel(b.path, b.source);
    const current = editor.getModel();
    if (current && pathOfUri(current.uri) !== path)
      viewStates.current.set(pathOfUri(current.uri), editor.saveViewState());
    const model = ensureModel(path, buffer.source);
    if (current !== model) {
      editor.setModel(model);
      const saved = viewStates.current.get(path);
      if (saved) editor.restoreViewState(saved);
    }
    syncModelsFromStore();
    syncMarkers();
    const reveal = state.reveal;
    if (reveal?.path === path) {
      editor.setPosition({ lineNumber: reveal.line, column: 1 });
      editor.revealLineInCenterIfOutsideViewport(reveal.line);
    }
    editor.focus();
  }, [path]);

  const cursorLine = cursor?.path === path ? cursor.line : 1;
  const mod = useMemo(() => modKey(), []);

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-1 border-b px-3 text-xs text-muted-foreground">
        <span className="truncate font-mono text-2xs">{path ?? ""}</span>
        <div className="ml-auto flex items-center gap-0.5">
          <Tooltip
            content={
              <>
                Play from line {cursorLine} <Kbd>{mod}⇧↵</Kbd>
              </>
            }
          >
            <Button variant="ghost" size="sm" onClick={() => path && playFromLine(path, cursorLine)}>
              <Play />
              Play from here
            </Button>
          </Tooltip>
          <Tooltip content="Toggle outline">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Toggle outline"
              aria-pressed={showOutline}
              onClick={() => setShowOutline(!showOutline)}
              className={cn(showOutline && "text-foreground")}
            >
              <ListTree />
            </Button>
          </Tooltip>
        </div>
      </div>
      <div className="flex min-h-0 flex-1">
        <div
          className="relative min-w-0 flex-1"
          onDragOverCapture={(e) => {
            if (!isAssetDrag(e)) return;
            e.preventDefault();
            e.stopPropagation();
            e.dataTransfer.dropEffect = "copy";
            setDropLine(editorRef.current?.getTargetAtClientPoint(e.clientX, e.clientY)?.position?.lineNumber ?? null);
          }}
          onDragLeave={() => setDropLine(null)}
          onDropCapture={(e) => {
            if (!isAssetDrag(e)) return;
            e.preventDefault();
            e.stopPropagation();
            setDropLine(null);
            const statement = droppedStatement(e, e.currentTarget);
            const line = editorRef.current?.getTargetAtClientPoint(e.clientX, e.clientY)?.position?.lineNumber;
            if (statement && appStore().getState().insertAtCursor(statement, line)) {
              toast(`Inserted ${statement}`);
            }
          }}
        >
          <div ref={host} className="absolute inset-0" data-testid="monaco-host" />
          {dropLine !== null && (
            <div className="pointer-events-none absolute inset-0 grid grid-cols-3 border-2 border-dashed border-primary/60 bg-primary/5 text-2xs text-primary">
              {(["left", "center", "right"] as const).map((p) => (
                <span key={p} className="flex items-end justify-center border-r border-primary/20 pb-3 last:border-r-0">
                  {p}
                </span>
              ))}
              <span className="absolute top-2 left-1/2 -translate-x-1/2 rounded-md bg-popover px-2 py-1 text-foreground shadow">
                Insert after line {dropLine}
              </span>
            </div>
          )}
        </div>
        {showOutline && path && <Outline path={path} line={cursorLine} />}
      </div>
    </div>
  );
}

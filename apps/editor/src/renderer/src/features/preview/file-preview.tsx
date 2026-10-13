import { Boxes, Music } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { useResolvedAppearance } from "@/hooks/use-theme";
import { basename } from "@/lib/utils";
import { useApp } from "@/store";
import { assetUrl, fileKind, SNAPSHOT_TEXT_FIELDS } from "@shared/assets";

import { defineTheme, monaco } from "../editor/monaco";

function Footer({ path, detail }: { path: string; detail?: string }) {
  const openView = useApp((s) => s.openView);
  return (
    <footer className="flex h-9 shrink-0 items-center gap-3 border-t px-3 text-xs text-muted-foreground">
      <span className="truncate font-mono">{path}</span>
      {detail && <span className="tabular">{detail}</span>}
      <Button variant="ghost" size="sm" className="ml-auto" onClick={() => openView("assets")}>
        <Boxes />
        Show in Assets
      </Button>
    </footer>
  );
}

function ImagePreview({ path }: { path: string }) {
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="grid min-h-0 flex-1 place-items-center overflow-auto bg-[conic-gradient(var(--muted)_25%,transparent_0_50%,var(--muted)_0_75%,transparent_0)] bg-[length:16px_16px] p-6">
        <img
          key={path}
          src={assetUrl(path)}
          alt={basename(path)}
          data-testid="image-preview"
          onLoad={(e) => setSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
          className="max-h-full max-w-full object-contain shadow-lg"
        />
      </div>
      <Footer path={path} detail={size ? `${size.w} × ${size.h}` : undefined} />
    </div>
  );
}

function AudioPreview({ path }: { path: string }) {
  const appearance = useResolvedAppearance();
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="grid flex-1 place-items-center p-6">
        <div className="flex w-full max-w-md flex-col items-center gap-4">
          <div className="grid size-20 place-items-center rounded-2xl border bg-card/60">
            <Music className="size-8 text-muted-foreground" />
          </div>
          <p className="truncate text-sm font-medium">{basename(path)}</p>
          <audio
            key={path}
            controls
            src={assetUrl(path)}
            data-testid="audio-preview"
            style={{ colorScheme: appearance }}
            className="w-full"
          />
        </div>
      </div>
      <Footer path={path} />
    </div>
  );
}

function TextPreview({ path }: { path: string }) {
  const host = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const source = useApp((s) => s.project?.[SNAPSHOT_TEXT_FIELDS[path as keyof typeof SNAPSHOT_TEXT_FIELDS]] ?? "");
  const theme = useApp((s) => s.theme);
  const appearance = useResolvedAppearance();
  const initialSource = useRef(source);

  useEffect(() => {
    const model = monaco.editor.createModel(initialSource.current, "ini");
    const editor = monaco.editor.create(host.current!, {
      model,
      readOnly: true,
      domReadOnly: true,
      automaticLayout: true,
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 21,
      padding: { top: 10, bottom: 10 },
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      renderLineHighlight: "none",
      lineNumbersMinChars: 3,
      glyphMargin: false,
      scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10, useShadows: false },
      wordWrap: "on",
    });
    editorRef.current = editor;
    return () => {
      editor.dispose();
      model.dispose();
      editorRef.current = null;
    };
  }, []);

  useEffect(() => {
    const model = editorRef.current?.getModel();
    if (model && model.getValue() !== source) model.setValue(source);
  }, [source]);

  useEffect(() => {
    monaco.editor.setTheme(defineTheme(theme, appearance));
  }, [theme, appearance]);

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <header className="flex h-8 shrink-0 items-center gap-2 border-b px-3 text-xs text-muted-foreground">
        <span className="truncate font-mono">{path}</span>
        <span className="ml-auto rounded-sm border px-1.5 py-0.5 text-2xs">Read-only</span>
      </header>
      <div className="relative min-h-0 flex-1">
        <div ref={host} className="absolute inset-0" data-testid="text-preview" />
      </div>
    </div>
  );
}

export default function FilePreview({ path }: { path: string }) {
  const kind = fileKind(path);
  if (kind === "image") return <ImagePreview path={path} />;
  if (kind === "audio") return <AudioPreview path={path} />;
  return <TextPreview key={path} path={path} />;
}

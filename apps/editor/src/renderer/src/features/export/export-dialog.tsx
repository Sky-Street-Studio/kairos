import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/store";

export function ExportDialog({ open, onClose }: { open: boolean; onClose(): void }) {
  const saveAll = useApp((state) => state.saveAll);
  const [target, setTarget] = useState<"web" | "desktop">("web");
  const [output, setOutput] = useState<string | null>(null);
  const [result, setResult] = useState<{ path?: string; error?: string } | null>(null);
  const [busy, setBusy] = useState(false);

  if (!open) return null;

  const chooseFolder = async () => {
    const path = await window.kairos.project.pickExportFolder();
    if (path) {
      setOutput(path);
      setResult(null);
    }
  };

  const runExport = async () => {
    if (!output) return;
    setBusy(true);
    setResult(null);
    try {
      await saveAll();
      setResult({ path: await window.kairos.project.export(target, output) });
    } catch (error) {
      setResult({ error: error instanceof Error ? error.message : String(error) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-5"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="export-title"
        className="w-full max-w-md rounded-xl border bg-popover p-5 text-popover-foreground shadow-[0_24px_64px_-12px_rgb(0_0_0/0.55)]"
      >
        <h2 id="export-title" className="text-sm font-semibold">
          Export game
        </h2>
        <p className="mt-1 text-xs text-muted-foreground">Choose a build target and output folder.</p>
        <label className="mt-5 block text-xs font-medium" htmlFor="export-target">
          Target
        </label>
        <select
          id="export-target"
          value={target}
          onChange={(event) => {
            setTarget(event.target.value as "web" | "desktop");
            setResult(null);
          }}
          className="mt-1 h-8 w-full rounded-md border bg-background px-2 text-xs"
        >
          <option value="web">Web</option>
          <option value="desktop">Desktop for this computer</option>
        </select>
        <div className="mt-4 flex items-center gap-2">
          <div className="min-w-0 flex-1 truncate rounded-md border bg-background/60 px-2 py-1.5 text-xs text-muted-foreground">
            {output ?? "No output folder selected"}
          </div>
          <Button onClick={() => void chooseFolder()}>Choose folder</Button>
        </div>
        {result?.path && (
          <p className="mt-4 break-all text-xs text-emerald-500" role="status">
            Exported to {result.path}
          </p>
        )}
        {result?.error && (
          <pre
            className="mt-4 max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-destructive/10 p-2 text-xs text-destructive"
            role="alert"
          >
            {result.error}
          </pre>
        )}
        <footer className="mt-5 flex justify-end gap-2">
          <Button onClick={onClose}>Close</Button>
          <Button variant="primary" disabled={!output || busy} onClick={() => void runExport()}>
            {busy ? "Exporting…" : "Export"}
          </Button>
        </footer>
      </section>
    </div>
  );
}

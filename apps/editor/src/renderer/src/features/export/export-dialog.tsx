import type { ExportTarget } from "@shared/types";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import { useApp } from "@/store";

type PickableTarget = Exclude<ExportTarget, "desktop">;

export function ExportDialog({ open, onClose }: { open: boolean; onClose(): void }) {
  const saveAll = useApp((state) => state.saveAll);
  const [target, setTarget] = useState<PickableTarget>("web");
  const [output, setOutput] = useState<string | null>(null);
  const [result, setResult] = useState<{ path?: string; error?: string } | null>(null);
  const [busy, setBusy] = useState(false);

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
    <Dialog
      open={open}
      onOpenChange={(value) => !value && onClose()}
      title="Export game"
      description="Choose a build target and output folder."
    >
      <div>
        <label className="block text-xs font-medium" htmlFor="export-target">
          Target
        </label>
        <select
          id="export-target"
          value={target}
          onChange={(event) => {
            setTarget(event.target.value as PickableTarget);
            setResult(null);
          }}
          className="mt-1 h-8 w-full rounded-md border bg-background px-2 text-xs"
        >
          <option value="web">Web</option>
          <option value="windows-x64">Windows x64</option>
          <option value="linux-x64">Linux x64</option>
          <option value="macos">macOS</option>
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
        <DialogFooter>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" disabled={!output || busy} onClick={() => void runExport()}>
            {busy ? "Exporting…" : "Export"}
          </Button>
        </DialogFooter>
      </div>
    </Dialog>
  );
}

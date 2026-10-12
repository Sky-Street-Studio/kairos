import { CircleAlert, CircleCheck, GitBranch } from "lucide-react";

import { isDirty, useApp } from "@/store";

export function StatusBar() {
  const project = useApp((s) => s.project);
  const diagnostics = useApp((s) => s.analysis.diagnostics);
  const nodes = useApp((s) => s.analysis.graph.nodes.length);
  const cursor = useApp((s) => s.cursor);
  const activeId = useApp((s) => s.activeId);
  const unsaved = useApp((s) => Object.values(s.buffers).filter(isDirty).length);
  const openFile = useApp((s) => s.openFile);

  if (!project) return <footer className="h-6 shrink-0" />;
  const first = diagnostics[0];

  return (
    <footer className="flex h-6 shrink-0 items-center gap-3 px-3 text-2xs text-muted-foreground tabular">
      <button
        type="button"
        disabled={!first}
        onClick={() => first && openFile(first.path, first.line)}
        className={
          diagnostics.length
            ? "flex items-center gap-1 text-destructive hover:underline"
            : "flex items-center gap-1 text-success"
        }
        data-testid="diagnostic-count"
      >
        {diagnostics.length ? <CircleAlert className="size-3" /> : <CircleCheck className="size-3" />}
        {diagnostics.length
          ? `${diagnostics.length} ${diagnostics.length === 1 ? "problem" : "problems"}`
          : "No problems"}
      </button>
      <span className="flex items-center gap-1">
        <GitBranch className="size-3" />
        {nodes} labels
      </span>
      {unsaved > 0 && <span className="text-warning">{unsaved} unsaved</span>}
      <span className="ml-auto">{cursor && activeId === cursor.path ? `${cursor.path}  Ln ${cursor.line}` : ""}</span>
      <span>start: {project.manifest.start}</span>
    </footer>
  );
}

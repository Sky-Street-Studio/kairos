import { Files, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useApp } from "@/store";
import type { TreeNode } from "@shared/types";

import { TreeRows } from "./sidebar";

function filterTree(nodes: TreeNode[], query: string, showCode: boolean): TreeNode[] {
  const normalized = query.toLowerCase();
  return nodes.flatMap((node) => {
    if (node.kind === "file") {
      if (!showCode && node.path.endsWith(".kai")) return [];
      return !query || node.name.toLowerCase().includes(normalized) ? [node] : [];
    }
    const children = filterTree(node.children ?? [], query, showCode);
    return children.length || (query && node.name.toLowerCase().includes(normalized)) ? [{ ...node, children }] : [];
  });
}

export function FilesPanel() {
  const tree = useApp((s) => s.project?.tree ?? []);
  const refreshTree = useApp((s) => s.refreshTree);
  const [query, setQuery] = useState("");
  const showCode = useApp((s) => s.showCode);
  const visible = useMemo(() => filterTree(tree, query.trim(), showCode), [tree, query, showCode]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-1 border-b px-2">
        <Files className="size-3.5 text-muted-foreground" />
        <span className="text-xs font-medium">Files</span>
        <Button
          variant="ghost"
          size="icon"
          className="ml-auto"
          aria-label="Refresh files"
          onClick={() => void refreshTree()}
        >
          <RefreshCw />
        </Button>
      </div>
      <label
        htmlFor="project-file-filter"
        className="mx-2 my-2 flex h-7 shrink-0 items-center gap-1.5 rounded-md border px-2 text-muted-foreground focus-within:border-ring"
      >
        <Search className="size-3" />
        <Input
          id="project-file-filter"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Find files"
          aria-label="Find files"
          className="h-full border-0 p-0 text-xs shadow-none focus-visible:ring-0"
        />
      </label>
      <div className="min-h-0 flex-1 overflow-auto pb-2">
        {visible.length ? (
          <TreeRows nodes={visible} depth={0} filter={query.trim()} />
        ) : (
          <p className="px-3 py-2 text-xs text-muted-foreground">No files match “{query}”.</p>
        )}
      </div>
    </div>
  );
}

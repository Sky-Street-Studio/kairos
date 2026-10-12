import { FolderOpen, Plus } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/input";
import { modKey } from "@/lib/utils";
import { useApp } from "@/store";

import { newProjectFromTemplate, openProjectDialog, openRecent } from "./actions";
import { KairosMark } from "./kairos-mark";

export function Welcome() {
  const [recent, setRecent] = useState<string[]>([]);
  const loading = useApp((s) => s.loading);
  useEffect(() => {
    void window.kairos.settings.get().then((s) => setRecent(s.recentProjects));
  }, []);

  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="w-full max-w-md rounded-xl border bg-panel p-8 shadow-[0_24px_60px_-24px_rgb(0_0_0/0.5)]">
        <KairosMark className="mb-5 size-9" />
        <h1 className="text-xl font-semibold tracking-[-0.01em] text-balance">Write your visual novel</h1>
        <p className="mt-1.5 text-sm text-muted-foreground">
          Open a folder that has a <code className="font-mono text-xs text-foreground">kairos.toml</code>, or start from
          the Hello Kairos template.
        </p>
        <div className="mt-6 flex gap-2">
          <Button variant="primary" size="lg" onClick={() => void openProjectDialog()} disabled={loading}>
            <FolderOpen />
            Open project
          </Button>
          <Button size="lg" onClick={() => void newProjectFromTemplate()} disabled={loading}>
            <Plus />
            New from template
          </Button>
        </div>
        {recent.length > 0 && (
          <div className="mt-7">
            <h2 className="mb-1.5 text-xs font-medium text-muted-foreground">Recent</h2>
            <ul className="-mx-2">
              {recent.slice(0, 5).map((r) => (
                <li key={r}>
                  <button
                    type="button"
                    onClick={() => void openRecent(r)}
                    className="flex h-7 w-full items-center gap-2 truncate rounded-md px-2 text-left text-xs hover:bg-accent"
                  >
                    <span className="font-medium">{r.split("/").pop()}</span>
                    <span className="truncate text-muted-foreground">{r.replace(/^\/Users\/[^/]+/, "~")}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
        <p className="mt-7 flex items-center gap-1.5 text-2xs text-muted-foreground">
          <Kbd>{modKey()}K</Kbd> opens the command palette anywhere.
        </p>
      </div>
    </div>
  );
}

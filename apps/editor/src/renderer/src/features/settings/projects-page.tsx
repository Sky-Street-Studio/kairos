import { FolderOpen, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { useApp } from "@/store";

import { openProjectDialog, openRecent } from "../shell/actions";
import { SettingsCard } from "./primitives";

const tildify = (path: string) => path.replace(/^\/Users\/[^/]+/, "~");

export function ProjectsPage() {
  const [recent, setRecent] = useState<string[] | null>(null);
  const current = useApp((s) => s.project?.root);
  useEffect(() => {
    void window.kairos.settings.get().then((s) => setRecent(s.recentProjects));
  }, []);

  const remove = async (root: string) => {
    const next = (recent ?? []).filter((r) => r !== root);
    setRecent(next);
    await window.kairos.settings.update({ recentProjects: next });
  };

  return (
    <SettingsCard
      title="Recent projects"
      description="The ten most recently opened. Kairos reopens the first one on launch."
      actions={
        <Button size="sm" onClick={() => void openProjectDialog()}>
          <FolderOpen />
          Open project
        </Button>
      }
    >
      {recent?.length === 0 && <p className="py-3.5 text-xs text-muted-foreground">No recent projects yet.</p>}
      <ul className="-mx-2 py-1.5">
        {recent?.map((root) => (
          <li key={root} className="group flex items-center gap-2 rounded-md pr-1 hover:bg-accent/60">
            <button
              type="button"
              onClick={() => void openRecent(root)}
              disabled={root === current}
              className="flex h-9 min-w-0 flex-1 items-center gap-2 px-2 text-left text-[13px] disabled:cursor-default"
            >
              <span className="shrink-0 font-medium">{root.split("/").pop()}</span>
              <span className="truncate text-xs text-muted-foreground">{tildify(root)}</span>
              {root === current && (
                <span className="ml-auto shrink-0 rounded-sm border px-1.5 py-0.5 text-2xs text-muted-foreground">
                  Open now
                </span>
              )}
            </button>
            <Tooltip content="Remove from list">
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Remove ${root} from recent projects`}
                onClick={() => void remove(root)}
                className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
              >
                <X />
              </Button>
            </Tooltip>
          </li>
        ))}
      </ul>
    </SettingsCard>
  );
}

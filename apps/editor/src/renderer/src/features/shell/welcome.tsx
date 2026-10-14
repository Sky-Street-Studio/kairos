import { FolderOpen, Plus } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/input";
import { modKey } from "@/lib/utils";
import { useApp } from "@/store";

import { createBeginnerProject, openProjectDialog, openRecent } from "./actions";
import { KairosMark } from "./kairos-mark";

export function Welcome() {
  const [recent, setRecent] = useState<string[]>([]);
  const [createOpen, setCreateOpen] = useState(false);
  const [template, setTemplate] = useState("blank-story");
  const [name, setName] = useState("my-story");
  const [location, setLocation] = useState("");
  const loading = useApp((s) => s.loading);
  useEffect(() => {
    void window.kairos.settings.get().then((s) => setRecent(s.recentProjects));
    const open = () => setCreateOpen(true);
    window.addEventListener("kairos:new-project", open);
    return () => window.removeEventListener("kairos:new-project", open);
  }, []);

  return (
    <div className="relative flex flex-1 items-center justify-center p-8">
      <div className="drag-region absolute inset-x-0 top-0 h-9" />
      <div className="w-full max-w-md rounded-xl border bg-panel p-8 shadow-[0_24px_60px_-24px_rgb(0_0_0/0.5)]">
        <KairosMark className="mb-5 size-9" />
        <h1 className="text-xl font-semibold tracking-[-0.01em] text-balance">Write your visual novel</h1>
        <p className="mt-1.5 text-sm text-muted-foreground">
          Open a project folder, or choose a starter and make your first story.
        </p>
        <div className="mt-6 flex gap-2">
          <Button variant="primary" size="lg" onClick={() => void openProjectDialog()} disabled={loading}>
            <FolderOpen />
            Open project
          </Button>
          <Button size="lg" onClick={() => setCreateOpen(true)} disabled={loading}>
            <Plus />
            New project
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
      <Dialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        title="New project"
        description="Choose a starting point, then name your project and choose where to save it."
      >
        <div className="space-y-2" role="radiogroup" aria-label="Starter template">
          {[
            ["blank-story", "Blank story", "Start with an empty scene."],
            ["kinetic-novel", "Kinetic novel", "A short linear story."],
            ["branching-romance", "Branching romance", "Choices, affection and two endings."],
          ].map(([id, title, summary]) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={template === id}
              onClick={() => setTemplate(id!)}
              className={`w-full rounded-md border p-3 text-left ${template === id ? "border-primary bg-accent/50" : "hover:bg-accent/40"}`}
            >
              <span className="block text-xs font-medium">{title}</span>
              <span className="mt-1 block text-xs text-muted-foreground">{summary}</span>
            </button>
          ))}
        </div>
        <label className="mt-4 block text-xs font-medium" htmlFor="new-project-name">
          Project name
        </label>
        <Input id="new-project-name" className="mt-1" value={name} onChange={(event) => setName(event.target.value)} />
        <label className="mt-3 block text-xs font-medium" htmlFor="new-project-location">
          Location
        </label>
        <div className="mt-1 flex gap-2">
          <Input
            id="new-project-location"
            aria-label="Project location"
            placeholder="Choose a folder"
            value={location}
            onChange={(event) => setLocation(event.target.value)}
          />
          <Button
            onClick={() => void window.kairos.project.pickProjectLocation().then((path) => path && setLocation(path))}
          >
            Browse
          </Button>
        </div>
        <DialogFooter>
          <Button onClick={() => setCreateOpen(false)}>Cancel</Button>
          <Button
            variant="primary"
            disabled={!name.trim() || !location.trim() || loading}
            onClick={() =>
              void createBeginnerProject(template, name, location).then((created) => created && setCreateOpen(false))
            }
          >
            Create project
          </Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}

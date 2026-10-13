import { Check, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/store";

export function GuidedChecklist() {
  const beginnerPath = useApp((state) => state.beginnerPath);
  const dismissed = useApp((state) => state.checklistDismissed);
  const project = useApp((state) => state.project);
  const buffers = useApp((state) => state.buffers);
  const outlines = useApp((state) => state.analysis.outlines);
  const edges = useApp((state) => state.analysis.graph.edges);
  const hasPlayed = useApp((state) => state.hasPlayed);
  const openView = useApp((state) => state.openView);
  const openScene = useApp((state) => state.openScene);
  const playFromLine = useApp((state) => state.playFromLine);
  const dismiss = useApp((state) => state.dismissChecklist);
  if (!beginnerPath || dismissed) return null;

  const files = Object.values(buffers);
  const hasCharacter = Object.values(outlines).some((outline) => outline.characters.length > 0);
  const hasLine = files.some((file) => /^\s*(?:[A-Za-z_]\w*\s+)?["']\S/m.test(file.source));
  const hasChoice = edges.some((edge) => edge.kind === "choice");
  const firstScene = project?.manifest.start;
  const firstFile = files[0];
  const items = [
    { label: "Add a character", done: hasCharacter, action: () => openView("characters") },
    { label: "Write a line", done: hasLine, action: () => firstScene && openScene(firstScene) },
    { label: "Add a choice", done: hasChoice, action: () => firstScene && openScene(firstScene) },
    {
      label: "Play the story",
      done: hasPlayed,
      action: () => firstFile && playFromLine(firstFile.path, 1),
    },
  ];

  return (
    <section
      aria-label="First project checklist"
      data-testid="beginner-checklist"
      className="absolute top-3 right-3 z-20 w-64 rounded-lg border bg-popover p-3 text-popover-foreground shadow-lg"
    >
      <header className="flex items-start justify-between gap-2">
        <div>
          <h2 className="text-xs font-semibold">Your first story</h2>
          <p className="mt-1 text-2xs text-muted-foreground">Try these steps in any order.</p>
        </div>
        <Button variant="ghost" size="icon" aria-label="Dismiss checklist" onClick={dismiss}>
          <X />
        </Button>
      </header>
      <ol className="mt-2 space-y-1">
        {items.map((item) => (
          <li key={item.label}>
            <button
              type="button"
              onClick={item.action}
              className="flex min-h-8 w-full items-center gap-2 rounded-md px-1.5 text-left text-xs hover:bg-accent"
              aria-pressed={item.done}
            >
              <span className="grid size-4 shrink-0 place-items-center rounded-full border text-primary">
                {item.done && <Check className="size-3" />}
              </span>
              <span className={item.done ? "text-muted-foreground line-through" : ""}>{item.label}</span>
            </button>
          </li>
        ))}
      </ol>
    </section>
  );
}

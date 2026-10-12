import { Check, MessageSquareText, Plus, RotateCcw } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useApp } from "@/store";
import { validateCharacter } from "@shared/edits";
import type { CharacterFields } from "@shared/types";

interface Row extends CharacterFields {
  path: string;
  line: number;
}

function lineCounts(buffers: Record<string, { source: string }>): Map<string, number> {
  const counts = new Map<string, number>();
  for (const b of Object.values(buffers)) {
    for (const m of b.source.matchAll(/^\s*([A-Za-z_]\w*)\s+"/gm)) counts.set(m[1]!, (counts.get(m[1]!) ?? 0) + 1);
  }
  return counts;
}

function CharacterRow({
  row,
  lines,
  onSave,
}: {
  row?: Row;
  lines?: number;
  onSave(c: CharacterFields): Promise<void>;
}) {
  const initial = { id: row?.id ?? "", name: row?.name ?? "", color: row?.color ?? "" };
  const [draft, setDraft] = useState(initial);
  const [busy, setBusy] = useState(false);
  const changed = draft.id !== initial.id || draft.name !== initial.name || draft.color !== initial.color;
  const fields: CharacterFields = { id: draft.id.trim(), name: draft.name, color: draft.color || undefined };
  const error = changed ? validateCharacter(fields) : null;
  const openFile = useApp((s) => s.openFile);

  const submit = async () => {
    if (!changed || error) return;
    setBusy(true);
    try {
      await onSave(fields);
      if (!row) setDraft({ id: "", name: "", color: "" });
    } catch (e) {
      toast.error("Could not save character", { description: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      className={cn(
        "grid grid-cols-[28px_minmax(80px,120px)_minmax(120px,1fr)_96px_minmax(140px,auto)] items-center gap-2 px-4 py-2",
        !row && "border-t border-dashed",
      )}
    >
      <label className="relative grid size-7 place-items-center" title="Name color">
        <input
          type="color"
          aria-label="Name color"
          value={draft.color || "#9a9cae"}
          onChange={(e) => setDraft({ ...draft, color: e.target.value })}
          className="size-6 cursor-pointer"
        />
      </label>
      <Input
        aria-label="Id"
        placeholder="id"
        value={draft.id}
        onChange={(e) => setDraft({ ...draft, id: e.target.value })}
        className="font-mono"
        aria-invalid={!!error && /id/.test(error)}
      />
      <Input
        aria-label="Display name"
        placeholder="Display name"
        value={draft.name}
        onChange={(e) => setDraft({ ...draft, name: e.target.value })}
        aria-invalid={!!error && /name/.test(error)}
      />
      <Input
        aria-label="Color hex"
        placeholder="#rrggbb"
        value={draft.color}
        onChange={(e) => setDraft({ ...draft, color: e.target.value })}
        className="font-mono"
        aria-invalid={!!error && /#/.test(error)}
      />
      <div className="flex items-center gap-1">
        {row ? (
          <>
            <button
              type="button"
              onClick={() => openFile(row.path, row.line)}
              className="mr-auto flex items-center gap-1 truncate text-2xs text-muted-foreground tabular hover:text-foreground"
            >
              <MessageSquareText className="size-3" />
              {lines ?? 0} lines · {row.path.replace(/^script\//, "")}:{row.line}
            </button>
            {changed && (
              <Button type="button" variant="ghost" size="icon" aria-label="Revert" onClick={() => setDraft(initial)}>
                <RotateCcw />
              </Button>
            )}
            <Button
              type="submit"
              size="sm"
              variant={changed ? "primary" : "ghost"}
              disabled={!changed || !!error || busy}
            >
              <Check />
              Save
            </Button>
          </>
        ) : (
          <Button type="submit" size="sm" variant="primary" disabled={!changed || !!error || busy} className="ml-auto">
            <Plus />
            Add character
          </Button>
        )}
      </div>
      {error && <p className="col-span-full col-start-2 text-2xs text-destructive">{error}</p>}
    </form>
  );
}

export function CharacterManager() {
  const outlines = useApp((s) => s.analysis.outlines);
  const buffers = useApp((s) => s.buffers);
  const saveCharacter = useApp((s) => s.saveCharacter);
  const rows = useMemo<Row[]>(
    () => Object.entries(outlines).flatMap(([path, o]) => o.characters.map((c) => ({ ...c, path }))),
    [outlines],
  );
  const counts = useMemo(() => lineCounts(buffers), [buffers]);

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-4xl py-6">
        <header className="px-4 pb-4">
          <h1 className="text-base font-semibold">Characters</h1>
          <p className="mt-0.5 text-xs text-muted-foreground">
            Edits rewrite the <code className="font-mono text-foreground">character</code> line in place and save the
            file.
          </p>
        </header>
        <div className="overflow-hidden rounded-lg border bg-card/60">
          <div className="grid grid-cols-[28px_minmax(80px,120px)_minmax(120px,1fr)_96px_minmax(140px,auto)] gap-2 border-b px-4 py-1.5 text-2xs font-medium text-muted-foreground">
            <span />
            <span>Id</span>
            <span>Display name</span>
            <span>Color</span>
            <span />
          </div>
          {rows.map((r) => (
            <CharacterRow
              key={`${r.path}:${r.line}:${r.id}:${r.name}:${r.color ?? ""}`}
              row={r}
              lines={counts.get(r.id)}
              onSave={(c) =>
                saveCharacter(c, { path: r.path, line: r.line }).then(() => void toast.success(`Saved ${c.name}`))
              }
            />
          ))}
          <CharacterRow
            onSave={async (c) => {
              if (rows.some((r) => r.id === c.id)) throw new Error(`A character with id ${c.id} already exists.`);
              await saveCharacter(c, rows[0] && { path: rows[0].path });
              toast.success(`Added ${c.name}`);
            }}
          />
        </div>
      </div>
    </div>
  );
}

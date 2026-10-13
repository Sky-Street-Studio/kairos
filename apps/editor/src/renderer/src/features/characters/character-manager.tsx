import { Check, MessageSquareText, Plus, RotateCcw } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { appStore, useApp } from "@/store";
import type { CharacterFields } from "@shared/types";

export const CHARACTER_ID = /^[A-Za-z_]\w*$/;

export function validateCharacter(c: CharacterFields): string | null {
  if (!CHARACTER_ID.test(c.id)) return "Use letters, digits and underscores for the id, starting with a letter.";
  if (c.name.trim() === "") return "Give the character a display name.";
  if (c.color && !/^#[0-9a-fA-F]{6}$/.test(c.color)) return "Colors look like #c8ffc8.";
  return null;
}

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
  const [failure, setFailure] = useState<string | null>(null);
  const changed = draft.id !== initial.id || draft.name !== initial.name || draft.color !== initial.color;
  const fields: CharacterFields = { id: draft.id.trim(), name: draft.name, color: draft.color || undefined };
  const error = changed ? validateCharacter(fields) : null;
  const openFile = useApp((s) => s.openFile);

  const submit = async () => {
    if (!changed || error) return;
    setBusy(true);
    setFailure(null);
    try {
      await onSave(fields);
      if (!row) setDraft({ id: "", name: "", color: "" });
    } catch (e) {
      setFailure((e as Error).message);
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
        readOnly={!!row}
        title={row ? "Rename an id in the script, where every line that uses it can change too" : undefined}
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
      {(error ?? failure) && <p className="col-span-full col-start-2 text-2xs text-destructive">{error ?? failure}</p>}
    </form>
  );
}

export function CharacterManager() {
  const outlines = useApp((s) => s.analysis.outlines);
  const buffers = useApp((s) => s.buffers);
  const edit = useApp((s) => s.edit);
  const rows = useMemo<Row[]>(
    () => Object.entries(outlines).flatMap(([path, o]) => o.characters.map((c) => ({ ...c, path }))),
    [outlines],
  );
  const counts = useMemo(() => lineCounts(buffers), [buffers]);

  const save = async ({ id, name, color }: CharacterFields, path: string | undefined) => {
    if (!path) throw new Error("Add a script file before declaring characters.");
    const focus = await edit([{ op: "setCharacter", path, id, name, color: color ?? null }]);
    if (!focus) throw new Error(appStore().getState().editError ?? "The script changed while saving. Try again.");
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-4xl py-6">
        <header className="px-4 pb-4">
          <h1 className="text-base font-semibold">Characters</h1>
          <p className="mt-0.5 text-xs text-muted-foreground">
            Edits rewrite the <code className="font-mono text-foreground">character</code> line in place.
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
              onSave={(c) => save(c, r.path).then(() => void toast.success(`Saved ${c.name}`))}
            />
          ))}
          <CharacterRow
            onSave={async (c) => {
              if (rows.some((r) => r.id === c.id)) throw new Error(`A character with id ${c.id} already exists.`);
              await save(c, Object.keys(buffers).toSorted()[0]);
              toast.success(`Added ${c.name}`);
            }}
          />
        </div>
      </div>
    </div>
  );
}

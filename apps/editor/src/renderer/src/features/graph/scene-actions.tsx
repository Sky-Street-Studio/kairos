import type { EditOp, Reference } from "@shared/edit-types";
import { useRef, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { appStore } from "@/store";

import { Dialog } from "../../components/ui/dialog";

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** The new start label when renaming `from` to `to` moves the start scene, else `null`. */
export function startAfterRename(start: string | undefined, from: string, to: string): string | null {
  return start === from && from !== to ? to : null;
}

export function deleteSummary(refs: Reference[]): string {
  if (refs.length === 0) return "Nothing else leads here.";
  const choices = refs.filter((r) => r.choice !== null).length;
  const jumps = refs.filter((r) => r.choice === null && r.kind === "jump").length;
  const calls = refs.filter((r) => r.choice === null && r.kind === "call").length;
  const parts = [
    choices && plural(choices, "choice"),
    jumps && plural(jumps, "jump"),
    calls && plural(calls, "call"),
  ].filter(Boolean) as string[];
  const list = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}` : parts[0];
  return `${list} lead${refs.length === 1 ? "s" : ""} here.`;
}

type NamePrompt = { title: string; initial: string };
type DeletePrompt = {
  labels: { label: string; summary: string }[];
  start: string | undefined;
};
type Dialogs = {
  name: NamePrompt | null;
  delete: DeletePrompt | null;
  nameValue: string;
  setNameValue(value: string): void;
  submitName(): void;
  cancelName(): void;
  submitDelete(removeReferences: boolean): void;
  cancelDelete(): void;
};

export function SceneDialogs({
  name,
  delete: deletion,
  nameValue,
  setNameValue,
  submitName,
  cancelName,
  submitDelete,
  cancelDelete,
}: Dialogs) {
  return (
    <>
      <Dialog open={name !== null} onOpenChange={(open) => !open && cancelName()} title={name?.title ?? "Scene name"}>
        <form
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            submitName();
          }}
        >
          <input
            autoFocus
            aria-label="Scene name"
            value={nameValue}
            onChange={(event) => setNameValue(event.target.value)}
            className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={cancelName}>
              Cancel
            </Button>
            <Button type="submit" disabled={!nameValue.trim()}>
              Create
            </Button>
          </div>
        </form>
      </Dialog>
      <Dialog open={deletion !== null} onOpenChange={(open) => !open && cancelDelete()} title="Delete scenes">
        <div className="space-y-3 text-sm">
          <ul className="max-h-48 space-y-2 overflow-y-auto">
            {deletion?.labels.map(({ label, summary }) => (
              <li key={label}>
                <span className="font-mono">{label}</span>
                <p className="text-xs text-muted-foreground">{summary}</p>
              </li>
            ))}
          </ul>
          {deletion?.labels.some(({ label }) => label === deletion.start) && (
            <p className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              This is the starting scene. Deleting it leaves the project without a starting scene.
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={cancelDelete}>
              Cancel
            </Button>
            <Button type="button" variant="secondary" onClick={() => submitDelete(false)}>
              Delete only
            </Button>
            <Button type="button" variant="destructive" onClick={() => submitDelete(true)}>
              Delete and remove links
            </Button>
          </div>
        </div>
      </Dialog>
    </>
  );
}

export function useSceneActions() {
  const [name, setName] = useState<NamePrompt | null>(null);
  const [nameValue, setNameValue] = useState("");
  const [deletion, setDeletion] = useState<DeletePrompt | null>(null);
  const pendingName = useRef<((value: string | null) => void) | null>(null);
  const pendingDelete = useRef<((removeReferences: boolean) => void) | null>(null);

  const askName = (title: string, initial: string) =>
    new Promise<string | null>((resolve) => {
      pendingName.current = resolve;
      setNameValue(initial);
      setName({ title, initial });
    });

  const finishName = (value: string | null) => {
    setName(null);
    pendingName.current?.(value);
    pendingName.current = null;
  };

  const askDelete = (labels: DeletePrompt["labels"], start: string | undefined) =>
    new Promise<boolean>((resolve) => {
      pendingDelete.current = resolve;
      setDeletion({ labels, start });
    });

  const finishDelete = (removeReferences: boolean) => {
    setDeletion(null);
    pendingDelete.current?.(removeReferences);
    pendingDelete.current = null;
  };

  const withEdit = async (ops: EditOp[]) => {
    const state = appStore().getState();
    const focus = await state.edit(ops);
    if (!focus) {
      const error = appStore().getState().editError;
      if (error) toast.error("Could not update the scenes", { description: error });
      return false;
    }
    return true;
  };

  const add = async (after?: string, from?: string): Promise<string | null> => {
    const state = appStore().getState();
    const labels = state.analysis.graph.nodes.map((node) => node.label);
    const inputName = await askName("Add scene", "");
    if (!inputName?.trim()) return null;
    try {
      const label = await window.kairos.edit.labelFromName(inputName, labels);
      const path =
        state.analysis.graph.nodes.find((node) => node.label === after)?.path ??
        Object.keys(state.buffers).toSorted()[0];
      if (!path) {
        toast.error("Could not add the scene", {
          description: "Open a project with a script file first.",
        });
        return null;
      }
      const ops = [
        { op: "addScene" as const, path, name: label, after: after ?? null },
        ...(from ? [{ op: "connect" as const, from, to: label, choice: null }] : []),
      ];
      return (await withEdit(ops)) ? label : null;
    } catch (error) {
      toast.error("Could not add the scene", {
        description: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  };

  const rename = async (label: string) => {
    const state = appStore().getState();
    const inputName = await askName("Rename scene", label);
    if (!inputName?.trim()) return null;
    try {
      const labels = state.analysis.graph.nodes.map((node) => node.label).filter((item) => item !== label);
      const next = await window.kairos.edit.labelFromName(inputName, labels);
      if (next === label) return label;
      if (!(await withEdit([{ op: "renameScene", from: label, to: next }]))) return null;
      const position = state.graphLayout.graph[label];
      if (position) state.updateNodePositions({ [label]: null, [next]: position });
      const start = startAfterRename(state.project?.manifest.start, label, next);
      if (start) {
        await window.kairos.edit.setStart(start);
        await state.refreshTree();
      }
      return next;
    } catch (error) {
      toast.error("Could not rename the scene", {
        description: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  };

  const duplicate = async (label: string) => {
    const state = appStore().getState();
    try {
      const labels = state.analysis.graph.nodes.map((node) => node.label);
      const copyLabel = await window.kairos.edit.labelFromName(`${label} copy`, labels);
      if (!(await withEdit([{ op: "duplicateScene", label, name: copyLabel }]))) return null;
      const position = state.graphLayout.graph[label];
      if (position) state.updateNodePositions({ [copyLabel]: position });
      return copyLabel;
    } catch (error) {
      toast.error("Could not duplicate the scene", {
        description: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  };

  const remove = async (labels: string[]) => {
    const state = appStore().getState();
    const unique = [...new Set(labels)].filter((label) =>
      state.analysis.graph.nodes.some((node) => node.label === label),
    );
    if (unique.length === 0) return;
    try {
      const summaries = await Promise.all(
        unique.map(async (label) => ({
          label,
          summary: deleteSummary(
            await window.kairos.edit.references(
              Object.values(state.buffers).map(({ path, source }) => ({
                path,
                source,
              })),
              label,
            ),
          ),
        })),
      );
      const removeReferences = await askDelete(summaries, state.project?.manifest.start);
      if (
        !(await withEdit(
          unique.map((label) => ({
            op: "deleteScene" as const,
            label,
            removeReferences,
          })),
        ))
      )
        return;
      state.updateNodePositions(Object.fromEntries(unique.map((label) => [label, null])));
    } catch (error) {
      toast.error("Could not delete the scene", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const createGhost = async (label: string) => {
    const state = appStore().getState();
    const path = Object.keys(state.buffers).toSorted()[0];
    if (!path) return;
    if (await withEdit([{ op: "addScene", path, name: label, after: null }])) return label;
    return null;
  };

  const setStart = async (label: string) => {
    if (appStore().getState().project?.manifest.start === label) return;
    try {
      await window.kairos.edit.setStart(label);
      await appStore().getState().refreshTree();
    } catch (error) {
      toast.error("Could not set the starting scene", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const dialogs: Dialogs = {
    name,
    delete: deletion,
    nameValue,
    setNameValue,
    submitName: () => finishName(nameValue),
    cancelName: () => finishName(null),
    submitDelete: finishDelete,
    cancelDelete: () => finishDelete(false),
  };

  return { add, rename, duplicate, remove, setStart, createGhost, dialogs };
}

import { Command } from "cmdk";
import { Plus } from "lucide-react";
import { useState } from "react";

import type { BlockKind, Place } from "@shared/edit-types";
import { Button } from "@/components/ui/button";
import { appStore, useApp } from "@/store";

import { BLOCK_MENU, defaultBlock, insertOp, type BlockContext } from "./block-ops";
import { flushPendingEdits, resolveBlock, type BlockPosition } from "./pending-edits";

export function AddBlockMenu({
  path,
  sceneLabel,
  place,
  targetPosition,
  ctx,
}: {
  path: string;
  sceneLabel: string;
  place: Place;
  targetPosition?: BlockPosition;
  ctx: BlockContext;
}) {
  const [open, setOpen] = useState(false);
  const edit = useApp((state) => state.edit);
  const selectBlock = useApp((state) => state.selectBlock);

  const add = async (kind: BlockKind["kind"]) => {
    await flushPendingEdits();
    const source = appStore().getState().buffers[path]?.source;
    const scene = source ? await window.kairos.edit.blocks(source, sceneLabel) : null;
    if (!scene) return;
    const currentPlace =
      place.at === "after" && targetPosition
        ? (() => {
            const block = resolveBlock(scene.blocks, targetPosition);
            return block ? { at: "after" as const, line: block.line } : null;
          })()
        : place.at === "start"
          ? { at: "start" as const, header: scene.line }
          : place;
    if (!currentPlace) return;
    const focus = await edit([insertOp(path, currentPlace, defaultBlock(kind, ctx))]);
    if (focus) selectBlock(focus);
    setOpen(false);
  };

  return (
    <div className="relative flex h-6 items-center justify-center">
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label="Add block"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="size-5 rounded-full text-muted-foreground hover:text-foreground"
      >
        <Plus className="size-3" aria-hidden />
      </Button>
      {open && (
        <div className="absolute left-1/2 top-full z-30 w-64 -translate-x-1/2 overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-lg">
          <Command
            label="Add block"
            className="text-xs"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.stopPropagation();
                setOpen(false);
              }
            }}
          >
            <Command.Input
              placeholder="Search blocks"
              className="h-9 w-full border-b bg-transparent px-3 text-xs outline-none placeholder:text-muted-foreground"
            />
            <Command.List className="max-h-64 overflow-y-auto p-1">
              <Command.Empty className="px-3 py-4 text-center text-muted-foreground">No blocks match.</Command.Empty>
              {BLOCK_MENU.map(({ kind, label, keywords }) => (
                <Command.Item
                  key={kind}
                  value={`${label} ${keywords.join(" ")}`}
                  onSelect={() => void add(kind)}
                  className="flex h-8 cursor-default items-center rounded px-2 data-[selected=true]:bg-accent"
                >
                  {label}
                </Command.Item>
              ))}
            </Command.List>
          </Command>
        </div>
      )}
    </div>
  );
}

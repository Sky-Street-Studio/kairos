import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import type { Block, BlockKind, EditOp, Scene } from "@shared/edit-types";
import { assetUrl } from "@shared/assets";
import type { AssetEntry } from "@shared/types";

import { Button } from "@/components/ui/button";
import { appStore, useApp } from "@/store";

import { fieldOp, type BlockContext } from "./block-ops";
import {
  flushPendingEdits,
  pendingEdits,
  resolveBlock,
  resolveBranch,
  resolveOption,
  type BlockPosition,
} from "./pending-edits";

const control =
  "h-8 w-full rounded-md border bg-background px-2 text-xs outline-none focus-visible:ring-1 focus-visible:ring-ring";
const labelClass = "grid gap-1 text-2xs text-muted-foreground";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={labelClass}>
      <span>{label}</span>
      {children}
    </div>
  );
}

function Select({
  value,
  options,
  onChange,
  label,
  ariaLabel,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange(value: string): void;
  label?: string;
  ariaLabel?: string;
}) {
  return (
    <Field label={label ?? ariaLabel ?? "Choose"}>
      <select
        aria-label={ariaLabel ?? label}
        className={control}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </Field>
  );
}

function Text({
  label,
  value,
  onChange,
  ariaLabel,
  placeholder,
}: {
  label: string;
  value: string;
  onChange(value: string): void;
  ariaLabel?: string;
  placeholder?: string;
}) {
  const [local, setLocal] = useState({ value, draft: value });
  const draft = local.value === value ? local.draft : value;
  return (
    <Field label={label}>
      <input
        aria-label={ariaLabel ?? label}
        className={control}
        value={draft}
        placeholder={placeholder}
        onChange={(event) => {
          setLocal({ value, draft: event.target.value });
          onChange(event.target.value);
        }}
      />
    </Field>
  );
}

function DialogueText({ value, names, onChange }: { value: string; names: string[]; onChange(value: string): void }) {
  const [local, setLocal] = useState({ value, draft: value });
  const [cursor, setCursor] = useState(value.length);
  const input = useRef<HTMLTextAreaElement>(null);
  const draft = local.value === value ? local.draft : value;
  const beforeCursor = draft.slice(0, cursor);
  const match = beforeCursor.match(/\[([\w]*)$/);
  const query = match?.[1] ?? "";
  const suggestions = match ? names.filter((name) => name.toLowerCase().startsWith(query.toLowerCase())) : [];
  const choose = (name: string) => {
    const start = beforeCursor.lastIndexOf("[");
    const text = `${draft.slice(0, start)}[${name}]${draft.slice(cursor)}`;
    const nextCursor = start + name.length + 2;
    setLocal({ value, draft: text });
    setCursor(nextCursor);
    onChange(text);
    requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.setSelectionRange(nextCursor, nextCursor);
    });
  };
  return (
    <div className="relative">
      <textarea
        ref={input}
        aria-label="Dialogue"
        className={`${control} min-h-16 resize-y py-2`}
        value={draft}
        onChange={(event) => {
          const text = event.target.value;
          setLocal({ value, draft: text });
          setCursor(event.target.selectionStart);
          onChange(text);
        }}
        onClick={(event) => setCursor(event.currentTarget.selectionStart)}
        onKeyUp={(event) => setCursor(event.currentTarget.selectionStart)}
      />
      {suggestions.length > 0 && (
        <div className="absolute left-0 top-full z-20 mt-1 max-h-36 w-full overflow-y-auto rounded-md border bg-popover p-1 shadow-lg">
          {suggestions.map((name) => (
            <button
              key={name}
              type="button"
              className="block w-full rounded px-2 py-1 text-left text-xs hover:bg-accent"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(name)}
            >
              {name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function VariableInput({
  value,
  values,
  listId,
  onChange,
}: {
  value: string;
  values: string[];
  listId: string;
  onChange(value: string): void;
}) {
  const [local, setLocal] = useState({ value, draft: value });
  const draft = local.value === value ? local.draft : value;
  return (
    <>
      <input
        aria-label="Variable"
        className={control}
        list={listId}
        value={draft}
        onChange={(event) => {
          setLocal({ value, draft: event.target.value });
          onChange(event.target.value);
        }}
      />
      <datalist id={listId}>
        {values.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
    </>
  );
}

function NumberField({ label, value, onChange }: { label: string; value: number; onChange(value: number): void }) {
  return (
    <Field label={label}>
      <input
        aria-label={label}
        type="number"
        min="0"
        step="0.1"
        className={control}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </Field>
  );
}

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange(checked: boolean): void }) {
  return (
    <label className="flex h-8 items-center gap-2 text-xs text-muted-foreground">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      {label}
    </label>
  );
}

function imageOptions(assets: AssetEntry[], category: AssetEntry["category"]) {
  return assets.filter((asset) => asset.kind === "image" && asset.category === category);
}

export function BlockEditor({
  block,
  path,
  ctx,
  sceneLabel,
  position,
  renderBlocks,
}: {
  block: Block;
  path: string;
  ctx: BlockContext;
  sceneLabel: string;
  position: BlockPosition;
  renderBlocks(blocks: Block[], header: number, position: BlockPosition): ReactNode;
}) {
  const edit = useApp((state) => state.edit);
  const selectBlock = useApp((state) => state.selectBlock);
  const project = useApp((state) => state.project);
  const buffers = useApp((state) => state.buffers);
  const characters = useApp((state) => state.analysis.outlines[path]?.characters ?? []);
  const [localCode, setLocalCode] = useState({
    line: block.line,
    baseText: block.kind === "code" ? block.text : "",
    text: block.kind === "code" ? block.text : "",
  });
  const codeText =
    block.kind === "code" && localCode.line === block.line && localCode.baseText === block.text
      ? localCode.text
      : block.kind === "code"
        ? block.text
        : "";
  const [codeError, setCodeError] = useState("");
  const [fieldError, setFieldError] = useState("");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingOps = useRef<((scene: Scene) => EditOp | null)[]>([]);
  const pendingPatch = useRef<Record<string, unknown>>({});
  const flushPromise = useRef<Promise<void> | null>(null);
  const flushRef = useRef<() => Promise<void>>(async () => {});
  const assets = project?.assets ?? [];
  const names = [
    ...new Set(
      Object.values(buffers).flatMap(({ source }) =>
        [...source.matchAll(/^(?:default|define)\s+(\w+)/gm)].flatMap((match) => (match[1] ? [match[1]] : [])),
      ),
    ),
  ].toSorted();

  const runOps = useCallback(
    async (ops: EditOp[]) => {
      const focus = await edit(ops);
      if (focus) selectBlock(focus);
      else {
        const message = appStore().getState().editError ?? "Could not update this field.";
        if (block.kind === "code") setCodeError(message);
        else setFieldError(message);
      }
      return focus;
    },
    [edit, selectBlock, block.kind],
  );
  const latestScene = useCallback(async () => {
    const source = appStore().getState().buffers[path]?.source;
    return source ? window.kairos.edit.blocks(source, sceneLabel) : null;
  }, [path, sceneLabel]);
  const flush = useCallback(async () => {
    if (flushPromise.current) await flushPromise.current;
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const builders = pendingOps.current;
    const patch = pendingPatch.current;
    pendingOps.current = [];
    pendingPatch.current = {};
    if (Object.keys(patch).length > 0) {
      builders.push((scene) => {
        const current = resolveBlock(scene.blocks, position);
        return current ? fieldOp(path, current, patch as Partial<BlockKind>) : null;
      });
    }
    if (builders.length === 0) return;
    flushPromise.current = (async () => {
      const scene = await latestScene();
      if (!scene) return;
      const ops = builders.map((build) => build(scene)).filter((op): op is EditOp => op !== null);
      if (ops.length > 0) await runOps(ops);
    })();
    try {
      await flushPromise.current;
    } finally {
      flushPromise.current = null;
    }
  }, [latestScene, position, path, runOps]);
  useEffect(() => {
    flushRef.current = flush;
  }, [flush]);
  useEffect(() => {
    const unregister = pendingEdits.register(() => flushRef.current());
    return () => {
      void flushRef.current();
      unregister();
    };
  }, []);
  const scheduleCommit = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => void flushRef.current(), 300);
  };
  const commitOperation = (build: (scene: Scene) => EditOp | null) => {
    setFieldError("");
    pendingOps.current.push(build);
    scheduleCommit();
  };
  const commit = (patch: Partial<BlockKind>) => {
    setFieldError("");
    pendingPatch.current = { ...pendingPatch.current, ...patch };
    scheduleCommit();
  };
  const runStructural = async (build: (scene: Scene) => EditOp | null) => {
    await flushPendingEdits();
    const scene = await latestScene();
    const op = scene ? build(scene) : null;
    if (op) await runOps([op]);
  };

  const characterOptions = [
    { value: "", label: "Narrator" },
    ...characters.map((character) => ({ value: character.id, label: character.name })),
  ];
  const transitionOptions = [
    { value: "", label: "No transition" },
    { value: "fade", label: "Fade" },
    { value: "dissolve", label: "Dissolve" },
  ];
  const fields: ReactNode[] = [];

  switch (block.kind) {
    case "dialogue":
      fields.push(
        <Select
          key="character"
          label="Speaker"
          value={block.character ?? ""}
          options={characterOptions}
          onChange={(character) => commit({ character: character || null })}
        />,
      );
      fields.push(
        <Field key="text" label="Dialogue">
          <DialogueText value={block.text} names={names} onChange={(text) => commit({ text })} />
        </Field>,
      );
      break;
    case "background": {
      const images = imageOptions(assets, "background");
      const current = block.image.join(" ");
      const preview = images.find((asset) => asset.name === current);
      fields.push(
        <Field key="image" label="Background">
          <select
            aria-label="Background"
            className={control}
            value={current}
            onChange={(event) => commit({ image: event.target.value.split(/\s+/) })}
          >
            {[...new Set([current, ...images.map((asset) => asset.name)])].map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
          {preview && (
            <img
              alt={`${current} background preview`}
              className="mt-2 h-14 w-full rounded object-cover"
              src={assetUrl(preview.path)}
            />
          )}
        </Field>,
      );
      fields.push(
        <Select
          key="transition"
          label="Transition"
          value={block.transition ?? ""}
          options={transitionOptions}
          onChange={(transition) => commit({ transition: transition || null })}
        />,
      );
      break;
    }
    case "show": {
      const tag = block.image[0] ?? "";
      const spriteAssets = assets.filter(
        (asset) => asset.kind === "image" && asset.category === "sprite" && asset.characterId === tag,
      );
      const expressions = [
        ...new Set([
          ...(project?.spriteLayers?.[tag]?.groups ? Object.keys(project.spriteLayers[tag].groups) : []),
          ...spriteAssets.map((asset) => asset.name.slice(tag.length).trim()).filter(Boolean),
        ]),
      ];
      fields.push(
        <Select
          key="character"
          label="Character"
          value={tag}
          options={[...new Set([tag, ...characters.map((character) => character.id)])].map((id) => ({
            value: id,
            label: characters.find((character) => character.id === id)?.name ?? id,
          }))}
          onChange={(character) => commit({ image: [character, ...block.image.slice(1)] })}
        />,
      );
      fields.push(
        <Select
          key="expression"
          label="Expression"
          value={block.image.slice(1).join(" ")}
          options={[
            { value: "", label: "Default" },
            ...expressions.map((expression) => ({ value: expression, label: expression })),
          ]}
          onChange={(expression) => commit({ image: expression ? [tag, ...expression.split(/\s+/)] : [tag] })}
        />,
      );
      fields.push(
        <Select
          key="position"
          label="Position"
          value={block.position ?? "center"}
          options={["left", "center", "right"].map((value) => ({ value, label: value }))}
          onChange={(blockPosition) => commit({ position: blockPosition })}
        />,
      );
      fields.push(
        <Select
          key="transition"
          label="Transition"
          value={block.transition ?? ""}
          options={transitionOptions}
          onChange={(transition) => commit({ transition: transition || null })}
        />,
      );
      break;
    }
    case "hide":
      fields.push(
        <Select
          key="character"
          label="Character"
          value={block.character}
          options={characters.map((character) => ({ value: character.id, label: character.name }))}
          onChange={(character) => commit({ character })}
        />,
      );
      fields.push(
        <Select
          key="transition"
          label="Transition"
          value={block.transition ?? ""}
          options={transitionOptions}
          onChange={(transition) => commit({ transition: transition || null })}
        />,
      );
      break;
    case "audio": {
      const audio = assets.filter(
        (asset) => asset.kind === "audio" && [block.channel, "other"].includes(asset.category),
      );
      fields.push(
        <Select
          key="channel"
          label="Channel"
          value={block.channel}
          options={["music", "sound", "voice"].map((value) => ({ value, label: value }))}
          onChange={(channel) => commit({ channel, looped: channel === "music" ? block.looped : false })}
        />,
      );
      fields.push(
        <Select
          key="asset"
          label="Audio"
          value={block.asset}
          options={[...new Set([block.asset, ...audio.map((asset) => asset.name)])].map((value) => ({
            value,
            label: value || "Choose audio",
          }))}
          onChange={(asset) => commit({ asset })}
        />,
      );
      if (block.channel === "music")
        fields.push(
          <Toggle key="looped" label="Loop music" checked={block.looped} onChange={(looped) => commit({ looped })} />,
        );
      fields.push(
        <NumberField
          key="fade"
          label="Fade in seconds"
          value={block.fadeIn ?? 0}
          onChange={(fadeIn) => commit({ fadeIn: fadeIn || null })}
        />,
      );
      break;
    }
    case "stopAudio":
      fields.push(
        <Select
          key="channel"
          label="Channel"
          value={block.channel}
          options={["music", "sound", "voice"].map((value) => ({ value, label: value }))}
          onChange={(channel) => commit({ channel })}
        />,
      );
      fields.push(
        <NumberField
          key="fade"
          label="Fade out seconds"
          value={block.fadeOut ?? 0}
          onChange={(fadeOut) => commit({ fadeOut: fadeOut || null })}
        />,
      );
      break;
    case "pause":
      fields.push(
        <Toggle
          key="until-click"
          label="Wait until click"
          checked={block.seconds === null}
          onChange={(wait) => commit({ seconds: wait ? null : 1 })}
        />,
      );
      if (block.seconds !== null)
        fields.push(
          <NumberField
            key="seconds"
            label="Seconds"
            value={block.seconds}
            onChange={(seconds) => commit({ seconds })}
          />,
        );
      break;
    case "goTo":
    case "call":
      fields.push(
        <Select
          key="scene"
          ariaLabel="Scene"
          value={block.scene}
          options={ctx.scenes.map((scene) => ({ value: scene, label: scene }))}
          onChange={(scene) => commit({ scene })}
        />,
      );
      break;
    case "setVariable":
      fields.push(
        <Field key="name" label="Variable">
          <VariableInput
            listId={`variables-${path}-${block.line}`}
            value={block.name}
            values={[...new Set([block.name, ...names])]}
            onChange={(name) => commit({ name })}
          />
        </Field>,
      );
      fields.push(
        <Select
          key="operation"
          label="Operation"
          value={block.op}
          options={["set", "add", "subtract", "toggle"].map((value) => ({ value, label: value }))}
          onChange={(op) => commit({ op: op as typeof block.op })}
        />,
      );
      if (block.op !== "toggle")
        fields.push(<Text key="value" label="Value" value={block.value} onChange={(value) => commit({ value })} />);
      break;
    case "choice":
      fields.push(
        <div key="options" className="grid gap-3">
          {block.options.map((option) => (
            <div key={option.line} className="grid gap-2 rounded-md border p-2">
              <Text
                label="Option"
                ariaLabel="Option text"
                value={option.text}
                onChange={(text) =>
                  commitOperation((scene) => {
                    const current = resolveOption(scene, position, block.options.indexOf(option));
                    return current ? { op: "setChoiceText", path, line: current.line, text } : null;
                  })
                }
              />
              <Text
                label="Only if"
                value={option.condition ?? ""}
                placeholder="Always available"
                onChange={(condition) =>
                  commitOperation((scene) => {
                    const current = resolveOption(scene, position, block.options.indexOf(option));
                    return current
                      ? { op: "setChoiceCondition", path, line: current.line, condition: condition || null }
                      : null;
                  })
                }
              />
              {renderBlocks(option.blocks, option.line, [
                ...position.slice(0, -1),
                { index: position.at(-1)?.index ?? 0, child: { kind: "choice", index: block.options.indexOf(option) } },
              ])}
            </div>
          ))}
        </div>,
      );
      fields.push(
        <Button
          key="add-option"
          variant="secondary"
          size="sm"
          aria-label="Add block"
          onClick={() =>
            void runStructural((scene) => {
              const current = resolveBlock(scene.blocks, position);
              return current?.kind === "choice"
                ? { op: "addChoiceOption", path, line: current.line, text: "Option" }
                : null;
            })
          }
        >
          + Add option
        </Button>,
      );
      break;
    case "if":
      fields.push(
        <div key="branches" className="grid gap-3">
          {block.branches.map((branch, index) => (
            <div key={branch.line} className="grid gap-2 rounded-md border p-2">
              {branch.condition !== null ? (
                <Text
                  label="Condition"
                  value={branch.condition}
                  onChange={(condition) =>
                    commitOperation((scene) => {
                      const current = resolveBranch(scene, position, block.branches.indexOf(branch));
                      return current ? { op: "setBranchCondition", path, line: current.line, condition } : null;
                    })
                  }
                />
              ) : (
                <p className="text-2xs text-muted-foreground">Otherwise</p>
              )}
              {renderBlocks(branch.blocks, branch.line, [
                ...position.slice(0, -1),
                { index: position.at(-1)?.index ?? 0, child: { kind: "if", index: block.branches.indexOf(branch) } },
              ])}
              {index === block.branches.length - 1 && branch.condition !== null && (
                <div className="flex gap-2">
                  <Button
                    variant="secondary"
                    size="sm"
                    aria-label="Add block"
                    onClick={() =>
                      void runStructural((scene) => {
                        const current = resolveBlock(scene.blocks, position);
                        return current?.kind === "if"
                          ? { op: "addBranch", path, line: current.line, condition: "points > 0" }
                          : null;
                      })
                    }
                  >
                    + Add else-if
                  </Button>
                  <Button
                    variant="secondary"
                    size="sm"
                    aria-label="Add block"
                    onClick={() =>
                      void runStructural((scene) => {
                        const current = resolveBlock(scene.blocks, position);
                        return current?.kind === "if"
                          ? { op: "addBranch", path, line: current.line, condition: null }
                          : null;
                      })
                    }
                  >
                    + Add otherwise
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>,
      );
      break;
    case "code":
      fields.push(
        <Field key="code" label="Code">
          <textarea
            aria-label="Code"
            className={`${control} min-h-24 resize-y py-2 font-mono`}
            value={codeText}
            onChange={(event) => {
              const text = event.target.value;
              setLocalCode({ line: block.line, baseText: block.text, text });
              setCodeError("");
              commitOperation((scene) => {
                const current = resolveBlock(scene.blocks, position);
                return current?.kind === "code"
                  ? { op: "replace", path, line: current.line, block: { kind: "code", text } }
                  : null;
              });
            }}
          />
        </Field>,
      );
      if (codeError)
        fields.push(
          <p key="error" className="text-2xs text-destructive">
            {codeError}
          </p>,
        );
      break;
  }

  if (fieldError)
    fields.push(
      <p key="field-error" role="alert" className="text-2xs text-destructive">
        {fieldError}
      </p>,
    );
  return <div className="grid gap-3 border-b bg-background/70 px-4 py-3">{fields}</div>;
}

import { PRESETS } from "@kairos/theme";
import { Command } from "cmdk";
import {
  FileCode2,
  FolderOpen,
  Hash,
  Network,
  PackageOpen,
  Palette,
  PanelBottom,
  PanelLeft,
  PanelRight,
  Plus,
  Redo2,
  Save,
  Settings2,
  Undo2,
  UsersRound,
  Wand2,
} from "lucide-react";
import { type ReactNode, useState } from "react";

import { Kbd } from "@/components/ui/input";
import { modKey } from "@/lib/utils";
import { appStore, useApp } from "@/store";

import { ExportDialog } from "../export/export-dialog";
import { SceneDialogs, useSceneActions } from "../graph/scene-actions";
import { newProjectFromTemplate, openProjectDialog } from "../shell/actions";

function Item({
  value,
  icon,
  children,
  hint,
  onSelect,
}: {
  value: string;
  icon: ReactNode;
  children: ReactNode;
  hint?: ReactNode;
  onSelect(): void;
}) {
  return (
    <Command.Item
      value={value}
      onSelect={() => {
        appStore().getState().setPaletteOpen(false);
        onSelect();
      }}
      className="flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 text-sm data-[selected=true]:bg-accent [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-muted-foreground"
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {hint && <span className="shrink-0 text-2xs text-muted-foreground">{hint}</span>}
    </Command.Item>
  );
}

const groupClass =
  "[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:pt-2 [&_[cmdk-group-heading]]:pb-1 [&_[cmdk-group-heading]]:text-2xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground";

export function CommandPalette() {
  const [exportOpen, setExportOpen] = useState(false);
  const sceneActions = useSceneActions();
  const open = useApp((s) => s.paletteOpen);
  const setOpen = useApp((s) => s.setPaletteOpen);
  const project = useApp((s) => s.project);
  const buffers = useApp((s) => s.buffers);
  const nodes = useApp((s) => s.analysis.graph.nodes);
  const theme = useApp((s) => s.theme);
  const s = appStore().getState();
  const mod = modKey();

  return [
    <Command.Dialog
      key="palette"
      open={open}
      onOpenChange={setOpen}
      label="Command palette"
      loop
      overlayClassName="fixed inset-0 z-40 bg-black/30"
      contentClassName="fixed top-[14%] left-1/2 z-50 w-[min(600px,calc(100vw-48px))] -translate-x-1/2 overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-[0_24px_64px_-12px_rgb(0_0_0/0.55)] data-[state=open]:animate-[palette-in_180ms_var(--ease-out-expo)]"
    >
      <Command.Input
        placeholder="Type a file, label or command"
        className="h-11 w-full border-b bg-transparent px-4 text-sm outline-none placeholder:text-muted-foreground"
      />
      <Command.List className="max-h-[min(420px,60vh)] overflow-y-auto p-1.5">
        <Command.Empty className="px-3 py-6 text-center text-xs text-muted-foreground">Nothing matches.</Command.Empty>
        {project && (
          <Command.Group heading="Files" className={groupClass}>
            {Object.keys(buffers)
              .toSorted()
              .map((path) => (
                <Item key={path} value={`file ${path}`} icon={<FileCode2 />} onSelect={() => s.openFile(path)}>
                  {path}
                </Item>
              ))}
          </Command.Group>
        )}
        {nodes.length > 0 && (
          <Command.Group heading="Go to label" className={groupClass}>
            {nodes.map((n) => (
              <Item
                key={n.label}
                value={`label ${n.label}`}
                icon={<Hash />}
                hint={`${n.path}:${n.line}`}
                onSelect={() => s.goToLabel(n.label)}
              >
                <span className="font-mono text-[12.5px]">{n.label}</span>
              </Item>
            ))}
          </Command.Group>
        )}
        <Command.Group heading="Commands" className={groupClass}>
          {project && (
            <>
              <Item value="story flow graph" icon={<Network />} onSelect={() => s.openView("graph")}>
                Show story flow
              </Item>
              <Item value="add scene" icon={<Plus />} onSelect={() => void sceneActions.add()}>
                Add scene
              </Item>
              <Item
                value="tidy up story flow"
                icon={<Wand2 />}
                onSelect={() => {
                  s.openView("graph");
                  s.resetGraphLayout();
                }}
              >
                Tidy up story flow
              </Item>
              <Item value="undo" icon={<Undo2 />} hint={<Kbd>{mod}Z</Kbd>} onSelect={() => s.undo()}>
                Undo
              </Item>
              <Item value="redo" icon={<Redo2 />} hint={<Kbd>{mod}⇧Z</Kbd>} onSelect={() => s.redo()}>
                Redo
              </Item>
              <Item value="characters manager" icon={<UsersRound />} onSelect={() => s.openView("characters")}>
                Manage characters
              </Item>
              <Item value="export game" icon={<PackageOpen />} onSelect={() => setExportOpen(true)}>
                Export game…
              </Item>
              <Item value="save all files" icon={<Save />} hint={<Kbd>{mod}⌥S</Kbd>} onSelect={() => void s.saveAll()}>
                Save all
              </Item>
              <Item
                value="toggle sidebar"
                icon={<PanelLeft />}
                hint={<Kbd>{mod}B</Kbd>}
                onSelect={() => s.togglePanel("sidebar")}
              >
                Toggle sidebar
              </Item>
              <Item
                value="toggle preview"
                icon={<PanelRight />}
                hint={<Kbd>{mod}\</Kbd>}
                onSelect={() => s.togglePanel("preview")}
              >
                Toggle preview
              </Item>
              <Item
                value="toggle terminal panel"
                icon={<PanelBottom />}
                hint={<Kbd>{mod}J</Kbd>}
                onSelect={() => s.togglePanel("bottom")}
              >
                Toggle terminal
              </Item>
            </>
          )}
          <Item value="open project folder" icon={<FolderOpen />} onSelect={() => void openProjectDialog()}>
            Open project…
          </Item>
          <Item value="new project from template" icon={<Plus />} onSelect={() => void newProjectFromTemplate()}>
            New project from template…
          </Item>
          <Item
            value="settings theme editor"
            icon={<Settings2 />}
            hint={<Kbd>{mod},</Kbd>}
            onSelect={() => s.openView("settings")}
          >
            Open settings
          </Item>
        </Command.Group>
        <Command.Group heading="Switch theme" className={groupClass}>
          {PRESETS.map((p) => (
            <Item
              key={p.id}
              value={`theme ${p.name}`}
              icon={<Palette />}
              hint={theme.id === p.id ? "current" : p.appearance}
              onSelect={() => s.setTheme(structuredClone(p))}
            >
              <span className="flex items-center gap-2">
                <span className="h-3 w-6 rounded-sm border" style={{ background: p.accent }} />
                {p.name}
              </span>
            </Item>
          ))}
        </Command.Group>
      </Command.List>
    </Command.Dialog>,
    <ExportDialog key="export" open={exportOpen} onClose={() => setExportOpen(false)} />,
    <SceneDialogs key="scene-dialogs" {...sceneActions.dialogs} />,
  ];
}

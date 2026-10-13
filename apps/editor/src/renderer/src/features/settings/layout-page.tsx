import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { modKey } from "@/lib/utils";
import { DEFAULT_LAYOUT, useApp } from "@/store";

import { SettingRow, SettingsCard } from "./primitives";

const PANELS = [
  { key: "sidebar", keys: "B", title: "Sidebar", description: "Project files, characters and assets." },
  { key: "bottom", keys: "J", title: "Terminal", description: "Shells and coding agents, under the editor." },
  {
    key: "preview",
    keys: "\\",
    title: "Preview",
    description: "The running game beside the editor. Playing from a line opens it again.",
  },
] as const;

export function LayoutPage() {
  const layout = useApp((s) => s.layout);
  const togglePanel = useApp((s) => s.togglePanel);
  const resizePanel = useApp((s) => s.resizePanel);
  const resized =
    layout.sidebarWidth !== DEFAULT_LAYOUT.sidebarWidth ||
    layout.previewWidth !== DEFAULT_LAYOUT.previewWidth ||
    layout.bottomHeight !== DEFAULT_LAYOUT.bottomHeight;

  return (
    <>
      <SettingsCard title="Panels" description="Each panel also toggles from the title bar.">
        {PANELS.map((p) => (
          <SettingRow key={p.key} title={p.title} description={p.description}>
            <Kbd className="mr-3">
              {modKey()}
              {p.keys}
            </Kbd>
            <Switch
              aria-label={`Show ${p.title.toLowerCase()}`}
              checked={layout[p.key]}
              onCheckedChange={(open) => togglePanel(p.key, open)}
            />
          </SettingRow>
        ))}
      </SettingsCard>
      <SettingsCard title="Sizes">
        <SettingRow
          title="Panel sizes"
          description={`Sidebar ${layout.sidebarWidth}px, preview ${layout.previewWidth}px, terminal ${layout.bottomHeight}px. Drag panel edges to resize.`}
        >
          <Button
            size="sm"
            disabled={!resized}
            onClick={() => {
              resizePanel("sidebarWidth", DEFAULT_LAYOUT.sidebarWidth);
              resizePanel("previewWidth", DEFAULT_LAYOUT.previewWidth);
              resizePanel("bottomHeight", DEFAULT_LAYOUT.bottomHeight);
            }}
          >
            <RotateCcw />
            Reset sizes
          </Button>
        </SettingRow>
      </SettingsCard>
    </>
  );
}

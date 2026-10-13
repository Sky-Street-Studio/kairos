import {
  MAX_STOPS,
  MIN_STOPS,
  PRESETS,
  parseThemeJson,
  themeVariables,
  type Appearance,
  type Gradient,
  type ThemeConfig,
} from "@kairos/theme";
import { Download, Laptop, Moon, Plus, RotateCcw, Sun, Trash2, Upload } from "lucide-react";
import type { ReactNode } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Segmented } from "@/components/ui/segmented";
import { useResolvedAppearance } from "@/hooks/use-theme";
import { cn } from "@/lib/utils";
import { useApp } from "@/store";

import { Range, SettingRow, SettingsCard } from "./primitives";

function GradientEditor({ label, value, onChange }: { label: string; value: Gradient; onChange(g: Gradient): void }) {
  const setStop = (i: number, patch: Partial<Gradient["stops"][number]>) =>
    onChange({ ...value, stops: value.stops.map((s, j) => (j === i ? { ...s, ...patch } : s)) });
  const sorted = (stops: Gradient["stops"]) => stops.toSorted((a, b) => a.position - b.position);
  return (
    <div className="space-y-3">
      <div
        className="h-9 rounded-md border"
        style={{
          background: `linear-gradient(${value.angle}deg, ${value.stops.map((s) => `${s.color} ${s.position}%`).join(", ")})`,
        }}
      />
      <Range
        label={`${label} angle`}
        value={value.angle}
        min={0}
        max={360}
        unit="°"
        onChange={(angle) => onChange({ ...value, angle })}
      />
      <div className="space-y-1.5">
        {value.stops.map((stop, i) => (
          <div key={i} className="flex items-center gap-2">
            <input
              type="color"
              aria-label={`${label} stop ${i + 1} color`}
              value={stop.color}
              onChange={(e) => setStop(i, { color: e.target.value })}
              className="size-7 shrink-0 cursor-pointer"
            />
            <Input
              aria-label={`${label} stop ${i + 1} hex`}
              value={stop.color}
              onChange={(e) =>
                /^#[0-9a-fA-F]{6}$/.test(e.target.value) && setStop(i, { color: e.target.value.toLowerCase() })
              }
              className="w-24 font-mono"
            />
            <input
              type="range"
              aria-label={`${label} stop ${i + 1} position`}
              min={0}
              max={100}
              value={stop.position}
              onChange={(e) => setStop(i, { position: Number(e.target.value) })}
              onPointerUp={() => onChange({ ...value, stops: sorted(value.stops) })}
              className="h-1 flex-1 accent-[var(--primary)]"
            />
            <span className="w-9 text-right text-xs text-muted-foreground tabular">{stop.position}%</span>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Remove stop ${i + 1}`}
              disabled={value.stops.length <= MIN_STOPS}
              onClick={() => onChange({ ...value, stops: value.stops.filter((_, j) => j !== i) })}
            >
              <Trash2 />
            </Button>
          </div>
        ))}
      </div>
      <Button
        size="sm"
        variant="ghost"
        disabled={value.stops.length >= MAX_STOPS}
        onClick={() => {
          const last = value.stops.at(-1)!;
          onChange({ ...value, stops: sorted([...value.stops, { color: last.color, position: 100 }]) });
        }}
      >
        <Plus />
        Add stop
      </Button>
    </div>
  );
}

function PresetCard({ preset, active, onPick }: { preset: ThemeConfig; active: boolean; onPick(): void }) {
  const v = themeVariables(preset, preset.appearance === "light" ? "light" : "dark");
  return (
    <button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      className={cn(
        "group overflow-hidden rounded-lg border text-left transition-[box-shadow,border-color] duration-150",
        active ? "border-primary shadow-[0_0_0_3px_var(--selection)]" : "hover:border-foreground/30",
      )}
    >
      <div className="relative h-16 p-2" style={{ background: `${v["app-gradient"]}, ${v.background}` }}>
        <div className="flex h-full gap-1">
          <div className="w-1/4 rounded-sm" style={{ background: v.sidebar }} />
          <div className="flex-1 rounded-sm p-1.5" style={{ background: v.card }}>
            <div className="h-1 w-3/5 rounded-full" style={{ background: v.foreground, opacity: 0.6 }} />
            <div className="mt-1 h-1 w-2/5 rounded-full" style={{ background: v["muted-foreground"], opacity: 0.6 }} />
            <div className="mt-1.5 h-1.5 w-8 rounded-full" style={{ background: v["accent-gradient"] }} />
          </div>
        </div>
      </div>
      <div className="flex items-center justify-between px-2.5 py-1.5 text-xs">
        <span className="font-medium">{preset.name}</span>
        <span className="text-2xs text-muted-foreground capitalize">{preset.appearance}</span>
      </div>
    </button>
  );
}

const APPEARANCES: { value: Appearance; label: string; icon: ReactNode }[] = [
  { value: "light", label: "Light", icon: <Sun /> },
  { value: "dark", label: "Dark", icon: <Moon /> },
  { value: "system", label: "System", icon: <Laptop /> },
];

export function AppearancePage() {
  const theme = useApp((s) => s.theme);
  const setTheme = useApp((s) => s.setTheme);
  const resolved = useResolvedAppearance();
  const patch = (p: Partial<ThemeConfig>) =>
    setTheme({ ...theme, ...p, id: theme.id.endsWith("-custom") ? theme.id : `${theme.id}-custom` });
  const basePreset = PRESETS.find((p) => theme.id.replace(/-custom$/, "") === p.id);

  const importTheme = async () => {
    const json = await window.kairos.settings.importTheme();
    if (json === null) return;
    const r = parseThemeJson(json);
    if (r.ok) {
      setTheme(r.theme);
      toast.success(`Imported ${r.theme.name}`);
    } else {
      toast.error("That file is not a Kairos theme", { description: r.errors.slice(0, 3).join("\n") });
    }
  };

  const exportTheme = async () => {
    if (await window.kairos.settings.exportTheme(JSON.stringify(theme, null, 2))) toast.success("Theme exported");
  };

  return (
    <>
      <SettingsCard title="Theme" description="Changes apply as you make them and are saved for next time.">
        <SettingRow
          title="Mode"
          description={theme.appearance === "system" ? `Following the system: ${resolved}.` : undefined}
        >
          <Segmented
            label="Mode"
            value={theme.appearance}
            options={APPEARANCES}
            onChange={(appearance) => setTheme({ ...theme, appearance })}
          />
        </SettingRow>
        <SettingRow title="Preset" stacked>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-2.5">
            {PRESETS.map((p) => (
              <PresetCard
                key={p.id}
                preset={p}
                active={theme.id === p.id}
                onPick={() =>
                  setTheme(
                    structuredClone({ ...p, appearance: theme.appearance === "system" ? "system" : p.appearance }),
                  )
                }
              />
            ))}
          </div>
        </SettingRow>
      </SettingsCard>

      <SettingsCard
        title="Customize"
        description="Editing any value forks the preset into your own theme."
        actions={
          <>
            {basePreset && theme.id !== basePreset.id && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setTheme(structuredClone({ ...basePreset, appearance: theme.appearance }))}
              >
                <RotateCcw />
                Reset to {basePreset.name}
              </Button>
            )}
            <Button size="sm" onClick={() => void importTheme()}>
              <Upload />
              Import JSON
            </Button>
            <Button size="sm" onClick={() => void exportTheme()}>
              <Download />
              Export JSON
            </Button>
          </>
        }
      >
        <SettingRow title="Theme name">
          <Input
            aria-label="Theme name"
            value={theme.name}
            onChange={(e) => patch({ name: e.target.value || "Untitled" })}
            className="w-60"
          />
        </SettingRow>
        <SettingRow
          title="Background"
          description="Stops set the hue. Lightness is fitted to light or dark mode so text stays readable."
          stacked
        >
          <GradientEditor
            label="Background"
            value={theme.background}
            onChange={(background) => patch({ background })}
          />
        </SettingRow>
        <SettingRow title="Background strength" stacked>
          <Range
            label="Background strength"
            value={theme.backgroundIntensity}
            min={0}
            max={100}
            unit="%"
            onChange={(backgroundIntensity) => patch({ backgroundIntensity })}
          />
        </SettingRow>
        <SettingRow title="Accent" description="Buttons, the active tab and story-flow choices." stacked>
          <GradientEditor label="Accent" value={theme.accent} onChange={(accent) => patch({ accent })} />
        </SettingRow>
        <SettingRow title="Surface tint" description="Shifts panels toward a color." stacked>
          <div className="flex items-center gap-3">
            <input
              type="color"
              aria-label="Surface tint color"
              value={theme.surfaceTint.color}
              onChange={(e) => patch({ surfaceTint: { ...theme.surfaceTint, color: e.target.value } })}
              className="size-7 shrink-0 cursor-pointer"
            />
            <div className="flex-1">
              <Range
                label="Surface tint amount"
                value={theme.surfaceTint.amount}
                min={0}
                max={40}
                unit="%"
                onChange={(amount) => patch({ surfaceTint: { ...theme.surfaceTint, amount } })}
              />
            </div>
          </div>
        </SettingRow>
      </SettingsCard>
    </>
  );
}

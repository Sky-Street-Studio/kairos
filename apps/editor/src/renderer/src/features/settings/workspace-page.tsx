import { SettingsCard, SettingRow } from "./primitives";
import { useApp } from "@/store";

export function WorkspacePage() {
  const showCode = useApp((state) => state.showCode);
  const setShowCode = useApp((state) => state.setShowCode);
  return (
    <SettingsCard title="Code" description="Choose whether script files appear in the editor.">
      <SettingRow title="Show code" description="Show .kai files, script tabs and the code editor.">
        <button
          type="button"
          role="switch"
          aria-checked={showCode}
          aria-label="Show code"
          onClick={() => setShowCode(!showCode)}
          className={`relative h-5 w-9 rounded-full border transition-colors ${showCode ? "border-primary bg-primary" : "bg-muted"}`}
        >
          <span
            className={`absolute top-0.5 size-3.5 rounded-full bg-background transition-transform ${showCode ? "translate-x-4" : "translate-x-0.5"}`}
          />
        </button>
      </SettingRow>
    </SettingsCard>
  );
}

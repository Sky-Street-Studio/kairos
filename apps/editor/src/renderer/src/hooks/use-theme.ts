import { applyTheme, resolveAppearance, type ResolvedAppearance } from "@kairos/theme";
import { useEffect, useSyncExternalStore } from "react";

import { useApp } from "@/store";

const query = () => window.matchMedia("(prefers-color-scheme: dark)");

function usePrefersDark(): boolean {
  return useSyncExternalStore(
    (notify) => {
      const q = query();
      q.addEventListener("change", notify);
      return () => q.removeEventListener("change", notify);
    },
    () => query().matches,
  );
}

export function useResolvedAppearance(): ResolvedAppearance {
  const appearance = useApp((s) => s.theme.appearance);
  const prefersDark = usePrefersDark();
  return resolveAppearance(appearance, prefersDark);
}

/** Applies the stored theme to `:root` and returns the appearance it resolved to. */
export function useAppliedTheme(): ResolvedAppearance {
  const theme = useApp((s) => s.theme);
  const prefersDark = usePrefersDark();
  const appearance = resolveAppearance(theme.appearance, prefersDark);
  useEffect(() => {
    applyTheme(theme, document.documentElement, prefersDark);
  }, [theme, prefersDark]);
  return appearance;
}

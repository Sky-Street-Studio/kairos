import { clampLightness, contrast, mix, withAlpha } from "./color";
import type { Appearance, ThemeConfig } from "./schema";

export type ResolvedAppearance = Exclude<Appearance, "system">;

interface Neutrals {
  surface: string;
  raised: string;
  foreground: string;
  muted: string;
  destructive: string;
  warning: string;
  success: string;
}

export const NEUTRALS: Record<ResolvedAppearance, Neutrals> = {
  dark: {
    surface: "#13141b",
    raised: "#1b1c25",
    foreground: "#e9e9f0",
    muted: "#9a9cae",
    destructive: "#ff6b6b",
    warning: "#f2b84b",
    success: "#4cd497",
  },
  light: {
    surface: "#ffffff",
    raised: "#f4f4f7",
    foreground: "#17181e",
    muted: "#5b5e6e",
    destructive: "#cc2929",
    warning: "#9a5b00",
    success: "#16794a",
  },
};

/** Lightness and chroma windows that keep theme colors legible in each appearance. */
const BACKGROUND_FIT = { dark: [0.15, 0.4, 0.15], light: [0.86, 0.95, 0.075] } as const;
const ACCENT_FIT = { dark: [0.7, 0.86, 0.2], light: [0.42, 0.54, 0.2] } as const;

export const THEME_VARIABLES = [
  "background",
  "foreground",
  "card",
  "card-foreground",
  "popover",
  "popover-foreground",
  "sidebar",
  "sidebar-foreground",
  "sidebar-border",
  "panel",
  "primary",
  "primary-foreground",
  "secondary",
  "secondary-foreground",
  "muted",
  "muted-foreground",
  "accent",
  "accent-foreground",
  "destructive",
  "warning",
  "success",
  "border",
  "input",
  "ring",
  "selection",
] as const;

export type ThemeVariable = (typeof THEME_VARIABLES)[number];
export type ThemeVariables = Record<ThemeVariable, string>;

export function resolveAppearance(appearance: Appearance, prefersDark: boolean): ResolvedAppearance {
  if (appearance === "system") return prefersDark ? "dark" : "light";
  return appearance;
}

function fit(color: string, [min, max, chroma]: readonly [number, number, number]): string {
  return clampLightness(color, min, max, chroma);
}

export function themeVariables(theme: ThemeConfig, appearance: ResolvedAppearance): ThemeVariables {
  const n = NEUTRALS[appearance];
  const tint = (c: string) => mix(c, theme.surfaceTint.color, theme.surfaceTint.amount / 100);
  const surface = tint(n.surface);
  const raised = tint(n.raised);
  const background = fit(theme.background, BACKGROUND_FIT[appearance]);
  const primary = fit(theme.accent, ACCENT_FIT[appearance]);
  const darkInk = "#111218";
  const lightInk = "#ffffff";
  const primaryForeground = contrast(primary, darkInk) >= contrast(primary, lightInk) ? darkInk : lightInk;
  const border = mix(surface, n.foreground, appearance === "dark" ? 0.11 : 0.13);
  const glass = appearance === "dark" ? 0.74 : 0.7;

  return {
    background,
    foreground: n.foreground,
    card: surface,
    "card-foreground": n.foreground,
    popover: raised,
    "popover-foreground": n.foreground,
    sidebar: withAlpha(surface, glass),
    "sidebar-foreground": n.foreground,
    "sidebar-border": withAlpha(border, 0.8),
    panel: withAlpha(surface, glass + 0.12),
    primary,
    "primary-foreground": primaryForeground,
    secondary: raised,
    "secondary-foreground": n.foreground,
    muted: raised,
    "muted-foreground": n.muted,
    accent: mix(surface, n.foreground, 0.07),
    "accent-foreground": n.foreground,
    destructive: n.destructive,
    warning: n.warning,
    success: n.success,
    border,
    input: border,
    ring: primary,
    selection: withAlpha(primary, 0.3),
  };
}

export interface ThemeTarget {
  style: { setProperty(name: string, value: string): void };
  dataset: Record<string, string | undefined>;
  classList: { toggle(token: string, force?: boolean): boolean };
}

/** Writes the theme to CSS custom properties on `target` (usually `document.documentElement`). */
export function applyTheme(theme: ThemeConfig, target: ThemeTarget, prefersDark: boolean): ResolvedAppearance {
  const appearance = resolveAppearance(theme.appearance, prefersDark);
  const vars = themeVariables(theme, appearance);
  for (const name of THEME_VARIABLES) target.style.setProperty(`--${name}`, vars[name]);
  target.dataset.appearance = appearance;
  target.classList.toggle("dark", appearance === "dark");
  return appearance;
}

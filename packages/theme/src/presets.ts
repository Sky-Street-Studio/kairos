import type { ThemeConfig } from "./schema";

const preset = (p: Omit<ThemeConfig, "version">): ThemeConfig => ({ version: 2, ...p });

export const PRESETS: readonly ThemeConfig[] = [
  preset({
    id: "graphite",
    name: "Graphite",
    appearance: "dark",
    background: "#17181e",
    accent: "#e9e9e9",
    surfaceTint: { color: "#e9e9e9", amount: 0 },
  }),
  preset({
    id: "slate",
    name: "Slate",
    appearance: "dark",
    background: "#171b23",
    accent: "#4c8dff",
    surfaceTint: { color: "#4c8dff", amount: 4 },
  }),
  preset({
    id: "pine",
    name: "Pine",
    appearance: "dark",
    background: "#141c19",
    accent: "#3fb27f",
    surfaceTint: { color: "#3fb27f", amount: 4 },
  }),
  preset({
    id: "paper",
    name: "Paper",
    appearance: "light",
    background: "#f3f3f6",
    accent: "#17181e",
    surfaceTint: { color: "#c9c9cf", amount: 2 },
  }),
  preset({
    id: "sand",
    name: "Sand",
    appearance: "light",
    background: "#f5f0e6",
    accent: "#a76d12",
    surfaceTint: { color: "#c9a96e", amount: 4 },
  }),
];

export const DEFAULT_THEME: ThemeConfig = PRESETS[0]!;

export function findPreset(id: string): ThemeConfig | undefined {
  return PRESETS.find((p) => p.id === id);
}

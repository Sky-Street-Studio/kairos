import type { ThemeConfig } from "./schema";

const preset = (p: Omit<ThemeConfig, "version">): ThemeConfig => ({ version: 1, ...p });

export const PRESETS: readonly ThemeConfig[] = [
  preset({
    id: "dusk",
    name: "Dusk",
    appearance: "dark",
    background: {
      angle: 145,
      stops: [
        { color: "#4b2fb8", position: 0 },
        { color: "#8a2d8f", position: 48 },
        { color: "#14507a", position: 100 },
      ],
    },
    backgroundIntensity: 62,
    accent: {
      angle: 120,
      stops: [
        { color: "#ff7a8a", position: 0 },
        { color: "#ffb36b", position: 100 },
      ],
    },
    surfaceTint: { color: "#6b55e0", amount: 7 },
  }),
  preset({
    id: "aurora",
    name: "Aurora",
    appearance: "dark",
    background: {
      angle: 200,
      stops: [
        { color: "#0f8a6c", position: 0 },
        { color: "#126f9c", position: 55 },
        { color: "#4a2d9a", position: 100 },
      ],
    },
    backgroundIntensity: 58,
    accent: {
      angle: 110,
      stops: [
        { color: "#3ee6b0", position: 0 },
        { color: "#58b4ff", position: 100 },
      ],
    },
    surfaceTint: { color: "#1fb58f", amount: 6 },
  }),
  preset({
    id: "ember",
    name: "Ember",
    appearance: "dark",
    background: {
      angle: 160,
      stops: [
        { color: "#a3341c", position: 0 },
        { color: "#7a2650", position: 60 },
        { color: "#2a1f6b", position: 100 },
      ],
    },
    backgroundIntensity: 55,
    accent: {
      angle: 100,
      stops: [
        { color: "#ff8a3d", position: 0 },
        { color: "#ffcf4a", position: 100 },
      ],
    },
    surfaceTint: { color: "#e0603a", amount: 6 },
  }),
  preset({
    id: "graphite",
    name: "Graphite",
    appearance: "dark",
    background: {
      angle: 180,
      stops: [
        { color: "#30323d", position: 0 },
        { color: "#30323d", position: 100 },
      ],
    },
    backgroundIntensity: 0,
    accent: {
      angle: 90,
      stops: [
        { color: "#7c9cff", position: 0 },
        { color: "#9a86ff", position: 100 },
      ],
    },
    surfaceTint: { color: "#7c9cff", amount: 0 },
  }),
  preset({
    id: "dawn",
    name: "Dawn",
    appearance: "light",
    background: {
      angle: 160,
      stops: [
        { color: "#ff9f7a", position: 0 },
        { color: "#e08bff", position: 50 },
        { color: "#7fa8ff", position: 100 },
      ],
    },
    backgroundIntensity: 70,
    accent: {
      angle: 110,
      stops: [
        { color: "#e8590c", position: 0 },
        { color: "#d6336c", position: 100 },
      ],
    },
    surfaceTint: { color: "#ff922b", amount: 3 },
  }),
  preset({
    id: "paper",
    name: "Paper",
    appearance: "light",
    background: {
      angle: 180,
      stops: [
        { color: "#e9e6df", position: 0 },
        { color: "#e9e6df", position: 100 },
      ],
    },
    backgroundIntensity: 0,
    accent: {
      angle: 110,
      stops: [
        { color: "#3b5bdb", position: 0 },
        { color: "#6741d9", position: 100 },
      ],
    },
    surfaceTint: { color: "#c9a96e", amount: 4 },
  }),
];

export const DEFAULT_THEME: ThemeConfig = PRESETS[0]!;

export function findPreset(id: string): ThemeConfig | undefined {
  return PRESETS.find((p) => p.id === id);
}

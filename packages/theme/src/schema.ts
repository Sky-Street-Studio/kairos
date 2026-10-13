import { isHex } from "./color";

export type Appearance = "light" | "dark" | "system";

export interface ThemeConfig {
  version: 2;
  id: string;
  name: string;
  appearance: Appearance;
  background: string;
  accent: string;
  surfaceTint: { color: string; amount: number };
}

export type ValidationResult = { ok: true; theme: ThemeConfig } | { ok: false; errors: string[] };

const REMOVED_PRESET_IDS = new Set(["dusk", "aurora", "ember", "dawn"]);

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const inRange = (v: unknown, min: number, max: number): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= min && v <= max;

export function validateTheme(input: unknown): ValidationResult {
  const errors: string[] = [];
  if (!isObject(input)) return { ok: false, errors: ["theme must be a JSON object"] };
  if (input.version !== 2) errors.push("version must be 2");
  if (typeof input.id !== "string" || input.id.trim() === "") errors.push("id must be a non-empty string");
  if (typeof input.name !== "string" || input.name.trim() === "") errors.push("name must be a non-empty string");
  if (input.appearance !== "light" && input.appearance !== "dark" && input.appearance !== "system") {
    errors.push('appearance must be "light", "dark" or "system"');
  }
  if (!isHex(input.background)) errors.push("background must be a #rrggbb hex color");
  if (!isHex(input.accent)) errors.push("accent must be a #rrggbb hex color");
  const tint = input.surfaceTint;
  if (!isObject(tint)) {
    errors.push("surfaceTint must be an object with color and amount");
  } else {
    if (!isHex(tint.color)) errors.push("surfaceTint.color must be a #rrggbb hex color");
    if (!inRange(tint.amount, 0, 40)) errors.push("surfaceTint.amount must be a number from 0 to 40");
  }
  return errors.length ? { ok: false, errors } : { ok: true, theme: input as unknown as ThemeConfig };
}

function migrateV1(input: Record<string, unknown>): Record<string, unknown> {
  const firstStop = (value: unknown) => (isObject(value) && Array.isArray(value.stops) ? value.stops[0] : undefined);
  const background = firstStop(input.background);
  const accent = firstStop(input.accent);
  if (typeof input.id === "string" && REMOVED_PRESET_IDS.has(input.id)) {
    return {
      version: 2,
      id: "graphite",
      name: "Graphite",
      appearance: "dark",
      background: "#17181e",
      accent: "#e9e9e9",
      surfaceTint: { color: "#e9e9e9", amount: 0 },
    };
  }
  return {
    version: 2,
    id: input.id,
    name: input.name,
    appearance: input.appearance,
    background: isObject(background) ? background.color : undefined,
    accent: isObject(accent) ? accent.color : undefined,
    surfaceTint: input.surfaceTint,
  };
}

export function parseThemeJson(json: string): ValidationResult {
  try {
    const input: unknown = JSON.parse(json);
    return isObject(input) && input.version === 1 ? validateTheme(migrateV1(input)) : validateTheme(input);
  } catch (e) {
    return { ok: false, errors: [`not valid JSON: ${(e as Error).message}`] };
  }
}

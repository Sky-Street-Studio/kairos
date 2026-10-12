import { isHex } from "./color";

export type Appearance = "light" | "dark" | "system";

export interface GradientStop {
  /** `#rrggbb`. Stops describe hue; lightness is fitted to the resolved appearance. */
  color: string;
  /** 0–100, non-decreasing across the gradient. */
  position: number;
}

export interface Gradient {
  /** Degrees, 0–360, CSS `linear-gradient` convention. */
  angle: number;
  stops: GradientStop[];
}

export interface ThemeConfig {
  version: 1;
  id: string;
  name: string;
  appearance: Appearance;
  background: Gradient;
  /** 0–100: how strongly the background gradient shows over the neutral base. */
  backgroundIntensity: number;
  accent: Gradient;
  /** Panels are tinted toward `color` by `amount` percent (0–40). */
  surfaceTint: { color: string; amount: number };
}

export type ValidationResult = { ok: true; theme: ThemeConfig } | { ok: false; errors: string[] };

export const MIN_STOPS = 2;
export const MAX_STOPS = 6;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const inRange = (v: unknown, min: number, max: number): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= min && v <= max;

function checkGradient(value: unknown, field: string, errors: string[]): void {
  if (!isObject(value)) {
    errors.push(`${field} must be an object with angle and stops`);
    return;
  }
  if (!inRange(value.angle, 0, 360)) errors.push(`${field}.angle must be a number from 0 to 360`);
  const stops = value.stops;
  if (!Array.isArray(stops) || stops.length < MIN_STOPS || stops.length > MAX_STOPS) {
    errors.push(`${field}.stops must have ${MIN_STOPS} to ${MAX_STOPS} entries`);
    return;
  }
  let last = -1;
  stops.forEach((stop: unknown, i) => {
    if (!isObject(stop)) {
      errors.push(`${field}.stops[${i}] must be an object`);
      return;
    }
    if (!isHex(stop.color)) errors.push(`${field}.stops[${i}].color must be a #rrggbb hex color`);
    if (!inRange(stop.position, 0, 100)) {
      errors.push(`${field}.stops[${i}].position must be a number from 0 to 100`);
    } else if (stop.position < last) {
      errors.push(`${field}.stops[${i}].position must not be lower than the previous stop`);
    } else {
      last = stop.position;
    }
  });
}

export function validateTheme(input: unknown): ValidationResult {
  const errors: string[] = [];
  if (!isObject(input)) return { ok: false, errors: ["theme must be a JSON object"] };
  if (input.version !== 1) errors.push("version must be 1");
  if (typeof input.id !== "string" || input.id.trim() === "") errors.push("id must be a non-empty string");
  if (typeof input.name !== "string" || input.name.trim() === "") errors.push("name must be a non-empty string");
  if (input.appearance !== "light" && input.appearance !== "dark" && input.appearance !== "system") {
    errors.push('appearance must be "light", "dark" or "system"');
  }
  checkGradient(input.background, "background", errors);
  checkGradient(input.accent, "accent", errors);
  if (!inRange(input.backgroundIntensity, 0, 100)) errors.push("backgroundIntensity must be a number from 0 to 100");
  const tint = input.surfaceTint;
  if (!isObject(tint)) {
    errors.push("surfaceTint must be an object with color and amount");
  } else {
    if (!isHex(tint.color)) errors.push("surfaceTint.color must be a #rrggbb hex color");
    if (!inRange(tint.amount, 0, 40)) errors.push("surfaceTint.amount must be a number from 0 to 40");
  }
  return errors.length ? { ok: false, errors } : { ok: true, theme: input as unknown as ThemeConfig };
}

export function parseThemeJson(json: string): ValidationResult {
  try {
    return validateTheme(JSON.parse(json));
  } catch (e) {
    return { ok: false, errors: [`not valid JSON: ${(e as Error).message}`] };
  }
}

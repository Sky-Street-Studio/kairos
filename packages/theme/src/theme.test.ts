import { describe, expect, it } from "vitest";

import {
  PRESETS,
  THEME_VARIABLES,
  applyTheme,
  contrast,
  mix,
  parseThemeJson,
  resolveAppearance,
  themeVariables,
  validateTheme,
  type ThemeConfig,
} from "./index";
import { hexToOklch, oklchToHex } from "./color";

const clone = (t: ThemeConfig): ThemeConfig => structuredClone(t);

describe("color", () => {
  it("round-trips hex through OKLCH", () => {
    for (const hex of ["#000000", "#ffffff", "#ff7a8a", "#14507a"]) {
      expect(oklchToHex(hexToOklch(hex))).toBe(hex);
    }
  });

  it("mixes in OKLab with endpoints intact", () => {
    expect(mix("#000000", "#ffffff", 0)).toBe("#000000");
    expect(mix("#000000", "#ffffff", 1)).toBe("#ffffff");
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 5);
  });
});

describe("presets", () => {
  it("has 4 to 6 valid presets with unique ids, both light and dark", () => {
    expect(PRESETS.length).toBeGreaterThanOrEqual(4);
    expect(PRESETS.length).toBeLessThanOrEqual(6);
    expect(new Set(PRESETS.map((p) => p.id)).size).toBe(PRESETS.length);
    expect(PRESETS.some((p) => p.appearance === "light")).toBe(true);
    expect(PRESETS.some((p) => p.appearance === "dark")).toBe(true);
    for (const p of PRESETS) expect(validateTheme(p)).toEqual({ ok: true, theme: p });
  });

  it.each(PRESETS.flatMap((p) => (["dark", "light"] as const).map((a) => [p.id, a, p] as const)))(
    "%s in %s keeps text readable",
    (_id, appearance, preset) => {
      const v = themeVariables(preset, appearance);
      expect(contrast(v.foreground, v.background)).toBeGreaterThanOrEqual(7);
      expect(contrast(v.foreground, v.card)).toBeGreaterThanOrEqual(7);
      expect(contrast(v["muted-foreground"], v.card)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(v["primary-foreground"], v.primary)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(v.primary, v.card)).toBeGreaterThanOrEqual(3);
    },
  );
});

describe("validation", () => {
  it("rejects bad colors, angles, stop order and stop counts", () => {
    const t = clone(PRESETS[0]!) as unknown as Record<string, any>;
    t.background.angle = 400;
    t.background.stops[1].position = -1;
    t.accent.stops = [{ color: "red", position: 0 }];
    t.surfaceTint.amount = 90;
    const r = validateTheme(t);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.errors).toEqual([
        "background.angle must be a number from 0 to 360",
        "background.stops[1].position must be a number from 0 to 100",
        "accent.stops must have 2 to 6 entries",
        "surfaceTint.amount must be a number from 0 to 40",
      ]);
    }
  });

  it("rejects decreasing stop positions", () => {
    const t = clone(PRESETS[0]!);
    t.background.stops[2]!.position = 10;
    const r = validateTheme(t);
    expect(r.ok ? [] : r.errors).toEqual(["background.stops[2].position must not be lower than the previous stop"]);
  });

  it("parses exported JSON and reports invalid JSON", () => {
    expect(parseThemeJson(JSON.stringify(PRESETS[1]))).toEqual({ ok: true, theme: PRESETS[1] });
    const bad = parseThemeJson("{");
    expect(bad.ok).toBe(false);
  });
});

describe("apply", () => {
  it("resolves system appearance from the OS preference", () => {
    expect(resolveAppearance("system", true)).toBe("dark");
    expect(resolveAppearance("system", false)).toBe("light");
    expect(resolveAppearance("light", true)).toBe("light");
  });

  it("writes every variable and the appearance marker", () => {
    const props = new Map<string, string>();
    const classes = new Set<string>();
    const target = {
      style: { setProperty: (k: string, v: string) => void props.set(k, v) },
      dataset: {} as Record<string, string | undefined>,
      classList: { toggle: (c: string, on?: boolean) => (on ? classes.add(c) : classes.delete(c), !!on) },
    };
    const theme = { ...clone(PRESETS[0]!), appearance: "system" as const };
    expect(applyTheme(theme, target, true)).toBe("dark");
    expect([...props.keys()]).toEqual(THEME_VARIABLES.map((n) => `--${n}`));
    expect(props.get("--app-gradient")).toMatch(/^linear-gradient\(145deg, #[0-9a-f]{6} 0%, /);
    expect(target.dataset.appearance).toBe("dark");
    expect(classes.has("dark")).toBe(true);
    applyTheme(theme, target, false);
    expect(classes.has("dark")).toBe(false);
  });

  it("flat presets with zero intensity draw a solid backdrop", () => {
    const graphite = PRESETS.find((p) => p.id === "graphite")!;
    const v = themeVariables(graphite, "dark");
    expect(v["app-gradient"]).toBe(`linear-gradient(180deg, ${v.background} 0%, ${v.background} 100%)`);
  });
});

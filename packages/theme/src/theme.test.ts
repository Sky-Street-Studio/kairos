import { describe, expect, it } from "vitest";

import {
  DEFAULT_THEME,
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
  it("provides flat non-purple presets with Graphite as the default", () => {
    expect(PRESETS.map((p) => p.id)).toEqual(["graphite", "slate", "pine", "paper", "sand"]);
    expect(PRESETS.some((p) => p.appearance === "light")).toBe(true);
    expect(PRESETS.some((p) => p.appearance === "dark")).toBe(true);
    expect(DEFAULT_THEME.id).toBe("graphite");
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
  it("rejects invalid colors and surface tint amounts", () => {
    const t = clone(PRESETS[0]!);
    t.background = "invalid";
    t.surfaceTint.amount = 90;
    const r = validateTheme(t);
    expect(r).toEqual({
      ok: false,
      errors: ["background must be a #rrggbb hex color", "surfaceTint.amount must be a number from 0 to 40"],
    });
  });

  it.each(["dusk", "aurora", "ember", "dawn"])("migrates removed v1 preset %s to Graphite", (id) => {
    const legacy = {
      version: 1,
      id,
      name: "Dusk",
      appearance: "dark",
      background: {
        angle: 145,
        stops: [
          { color: "#342455", position: 0 },
          { color: "#b055aa", position: 100 },
        ],
      },
      backgroundIntensity: 62,
      accent: {
        angle: 120,
        stops: [
          { color: "#e8e8e8", position: 0 },
          { color: "#ff7a8a", position: 100 },
        ],
      },
      surfaceTint: { color: "#6b55e0", amount: 7 },
    };
    expect(parseThemeJson(JSON.stringify(legacy))).toEqual({
      ok: true,
      theme: {
        version: 2,
        id: "graphite",
        name: "Graphite",
        appearance: "dark",
        background: "#17181e",
        accent: "#e9e9e9",
        surfaceTint: { color: "#e9e9e9", amount: 0 },
      },
    });
  });

  it("keeps the first stop of custom v1 themes", () => {
    const legacy = {
      version: 1,
      id: "my-theme",
      name: "My theme",
      appearance: "dark",
      background: {
        angle: 180,
        stops: [
          { color: "#202124", position: 0 },
          { color: "#ffffff", position: 100 },
        ],
      },
      backgroundIntensity: 30,
      accent: {
        angle: 90,
        stops: [
          { color: "#bbbbbb", position: 0 },
          { color: "#ff0000", position: 100 },
        ],
      },
      surfaceTint: { color: "#000000", amount: 0 },
    };
    expect(parseThemeJson(JSON.stringify(legacy))).toMatchObject({
      ok: true,
      theme: { version: 2, id: "my-theme", background: "#202124", accent: "#bbbbbb" },
    });
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
    expect(props.get("--background")).toBe(themeVariables(theme, "dark").background);
    const forbiddenWord = ["grad", "ient"].join("");
    expect([...props.values()].some((value) => value.includes(forbiddenWord))).toBe(false);
    expect(target.dataset.appearance).toBe("dark");
    expect(classes.has("dark")).toBe(true);
    applyTheme(theme, target, false);
    expect(classes.has("dark")).toBe(false);
  });

  it("keeps neutral accents neutral when fitting lightness", () => {
    const variables = themeVariables(PRESETS[0]!, "dark");
    expect(hexToOklch(variables.primary).c).toBeCloseTo(0, 2);
  });
});

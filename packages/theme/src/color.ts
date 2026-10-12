export type Rgb = [number, number, number];
export type Oklch = { l: number; c: number; h: number };

const HEX = /^#[0-9a-f]{6}$/i;

export function isHex(value: unknown): value is string {
  return typeof value === "string" && HEX.test(value);
}

export function hexToRgb(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

const byte = (v: number) =>
  Math.round(Math.min(1, Math.max(0, v)) * 255)
    .toString(16)
    .padStart(2, "0");

export function rgbToHex([r, g, b]: Rgb): string {
  return `#${byte(r)}${byte(g)}${byte(b)}`;
}

const toLinear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const fromLinear = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);

function rgbToOklab(rgb: Rgb): Rgb {
  const [r, g, b] = rgb.map(toLinear) as Rgb;
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function oklabToLinear([L, a, b]: Rgb): Rgb {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

export function hexToOklch(hex: string): Oklch {
  const [l, a, b] = rgbToOklab(hexToRgb(hex));
  return { l, c: Math.hypot(a, b), h: Math.atan2(b, a) };
}

/** Converts back to sRGB, lowering chroma until the color fits the gamut. */
export function oklchToHex({ l, c, h }: Oklch): string {
  for (let chroma = c; chroma >= 0; chroma -= 0.005) {
    const lin = oklabToLinear([l, chroma * Math.cos(h), chroma * Math.sin(h)]);
    if (lin.every((v) => v >= -1e-4 && v <= 1 + 1e-4)) return rgbToHex(lin.map(fromLinear) as Rgb);
  }
  return rgbToHex(oklabToLinear([l, 0, 0]).map(fromLinear) as Rgb);
}

/** Mixes `a` toward `b` by `amount` (0–1) in OKLab, which keeps perceived lightness even. */
export function mix(a: string, b: string, amount: number): string {
  const la = rgbToOklab(hexToRgb(a));
  const lb = rgbToOklab(hexToRgb(b));
  const t = Math.min(1, Math.max(0, amount));
  const lab = la.map((v, i) => v + (lb[i]! - v) * t) as Rgb;
  return rgbToHex(oklabToLinear(lab).map(fromLinear) as Rgb);
}

export function withAlpha(hex: string, alpha: number): string {
  return `${hex}${byte(alpha)}`;
}

function luminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex).map(toLinear) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: string, b: string): number {
  const [la, lb] = [luminance(a), luminance(b)];
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/** Clamps a color's OKLCH lightness into `[min, max]`, keeping hue and as much chroma as fits. */
export function clampLightness(hex: string, min: number, max: number, maxChroma = Infinity): string {
  const c = hexToOklch(hex);
  return oklchToHex({ l: Math.min(max, Math.max(min, c.l)), c: Math.min(c.c, maxChroma), h: c.h });
}

import { THEMES, type ThemeName } from "./theme";

export type Stop = { l: number; c: number; h: number };

/**
 * Chart ramp, warm to cool: amber → gold → straw → chartreuse → lime → mint.
 * Lightness climbs with hue so every stop sits on the vivid edge of sRGB.
 * Sibling `i` takes stop `i`, cycling, so neighbouring slices read as a gradient.
 */
export const STOPS: Stop[] = [
  { l: 0.74, c: 0.14, h: 48 },
  { l: 0.83, c: 0.13, h: 72 },
  { l: 0.91, c: 0.13, h: 96 },
  { l: 0.95, c: 0.16, h: 118 },
  { l: 0.91, c: 0.17, h: 140 },
  { l: 0.85, c: 0.14, h: 162 },
];

export const stopAt = (i: number) => STOPS[i % STOPS.length];

/** Per-theme nudge: lightness offset and chroma multiplier applied to every stop. */
const THEME_CHART: Record<ThemeName, { lift: number; chroma: number }> = {
  instrument: { lift: 0, chroma: 1 },
  glass: { lift: 0, chroma: 0.95 },
  light: { lift: -0.04, chroma: 1.1 },
};

/** Deeper rings fade: a little lighter, noticeably softer. */
const DEPTH = [
  { lift: 0, chroma: 1 },
  { lift: 0.03, chroma: 0.8 },
  { lift: 0.06, chroma: 0.62 },
];

const MAX_L = 0.97;

/** Segment colour for a ramp stop at a given ring depth (1 = outermost grouping). */
export function shade(theme: ThemeName, stop: Stop, depth: number): string {
  const t = THEME_CHART[theme];
  const d = DEPTH[Math.min(depth, DEPTH.length) - 1];
  const L = Math.min(MAX_L, stop.l + t.lift + d.lift);
  const C = stop.c * t.chroma * d.chroma;
  return `oklch(${L.toFixed(3)} ${C.toFixed(3)} ${stop.h.toFixed(1)})`;
}

export const tone = (theme: ThemeName, i: number, depth = 1) => shade(theme, stopAt(i), depth);

export const neutral = (theme: ThemeName) => (THEMES[theme].dark ? "oklch(0.42 0.008 70)" : "oklch(0.80 0.008 70)");

/** The ramp as CSS custom properties: `--chart-1`…`--chart-N` and `--chart-ramp` (a left-to-right gradient). */
export function chartVars(theme: ThemeName): Record<string, string> {
  const stops = STOPS.map((_, i) => tone(theme, i));
  const vars = Object.fromEntries(stops.map((s, i) => [`--chart-${i + 1}`, s]));
  vars["--chart-ramp"] = `linear-gradient(90deg, ${stops.join(", ")})`;
  return vars;
}

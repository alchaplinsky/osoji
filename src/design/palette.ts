import { THEMES, type ThemeName } from "./theme";

export type Stop = { l: number; c: number; h: number };

/**
 * Chart ramp by size rank, largest to smallest: coral → amber → yellow → lime → green.
 * Chroma is held low so the hues sit together as translucent glass on a dark ground.
 */
export const STOPS: Stop[] = [
  { l: 0.72, c: 0.12, h: 30 },
  { l: 0.78, c: 0.11, h: 57 },
  { l: 0.84, c: 0.1, h: 89 },
  { l: 0.86, c: 0.11, h: 109 },
  { l: 0.82, c: 0.11, h: 128 },
];

/** The ramp at `t` in [0, 1], blended between neighbouring stops. */
export function stopAt(t: number): Stop {
  const f = Math.min(1, Math.max(0, t)) * (STOPS.length - 1);
  const i = Math.min(STOPS.length - 2, Math.floor(f));
  const u = f - i;
  const mix = (k: keyof Stop) => STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * u;
  return { l: mix("l"), c: mix("c"), h: mix("h") };
}

/** Where sibling `rank` of `count` sits on the ramp: the largest at 0, the smallest at 1. */
export const rankAt = (rank: number, count: number) => (count > 1 ? Math.min(rank, count - 1) / (count - 1) : 0);

/** Per-theme nudge: lightness offset and chroma multiplier applied to every stop. */
const THEME_CHART: Record<ThemeName, { lift: number; chroma: number }> = {
  instrument: { lift: 0, chroma: 1 },
  glass: { lift: 0, chroma: 0.95 },
  light: { lift: -0.06, chroma: 1.15 },
};

/** Each ring deeper is a little darker and softer. */
const DEPTH_LIFT = -0.04;
const DEPTH_CHROMA = -0.01;

/** Segment colour for a ramp stop at a given ring depth (1 = outermost grouping). */
export function shade(theme: ThemeName, stop: Stop, depth = 1): string {
  const t = THEME_CHART[theme];
  const L = Math.min(0.9, Math.max(0.45, stop.l + t.lift + (depth - 1) * DEPTH_LIFT));
  const C = Math.max(0.06, stop.c * t.chroma + (depth - 1) * DEPTH_CHROMA);
  return `oklch(${L.toFixed(3)} ${C.toFixed(3)} ${stop.h.toFixed(1)})`;
}

/** Colour of sibling `rank` of `count` at a ring depth. */
export const tone = (theme: ThemeName, rank: number, count: number, depth = 1) => shade(theme, stopAt(rankAt(rank, count)), depth);

export const neutral = (theme: ThemeName) => (THEMES[theme].dark ? "oklch(0.42 0.008 70)" : "oklch(0.80 0.008 70)");

/** The ramp as CSS custom properties: `--chart-1`…`--chart-N` and `--chart-ramp` (a left-to-right gradient). */
export function chartVars(theme: ThemeName): Record<string, string> {
  const stops = STOPS.map((s) => shade(theme, s));
  const vars = Object.fromEntries(stops.map((s, i) => [`--chart-${i + 1}`, s]));
  vars["--chart-ramp"] = `linear-gradient(90deg, ${stops.join(", ")})`;
  return vars;
}

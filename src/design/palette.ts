import { THEMES, type ThemeName } from "./theme";

/** Hue wheel for chart segments: top-level item i gets hueAt(i). */
export const HUES = [28, 52, 78, 108, 142, 176, 208, 246, 290, 330];

export const hueAt = (i: number) => HUES[i % HUES.length];

const CHROMA_BY_DEPTH = [1, 0.88, 0.74];

/**
 * Segment colour for a hue at a given ring depth (1 = outermost grouping).
 * `j` is a sibling index used for subtle alternation inside a ring.
 */
export function shade(theme: ThemeName, hue: number, depth: number, j = 0): string {
  const { l, c } = THEMES[theme].chart;
  const L = Math.min(0.93, l + (depth - 1) * 0.05 + (j % 2) * 0.025);
  const C = c * CHROMA_BY_DEPTH[Math.min(depth, 3) - 1];
  const h = hue + ((j % 3) - 1) * 5;
  return `oklch(${L.toFixed(3)} ${C.toFixed(3)} ${h.toFixed(1)})`;
}

/** Colour of the i-th top-level item. */
export const tone = (theme: ThemeName, i: number, depth = 1) => shade(theme, hueAt(i), depth);

/** Neutral used for the merged "smaller items" slice. */
export const neutral = (theme: ThemeName) => (THEMES[theme].dark ? "oklch(0.42 0.008 70)" : "oklch(0.80 0.008 70)");

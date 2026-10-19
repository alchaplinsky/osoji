import { useId, useState, type CSSProperties } from "react";
import styles from "./glass.module.css";

export { styles as glass };

/** Inline style that tints a glass piece with its chart colour. */
export const tint = (color: string, style?: CSSProperties) => ({ "--seg": color, ...style }) as CSSProperties;

type Paint = "gloss" | "lens" | "binned" | "hidden" | "glow" | "halo";

/** Ids for one SVG chart's GlassDefs, unique on the page; `paint(name)` is the matching `url(#…)`. */
export function usePaints() {
  const id = useId().replace(/:/g, "");
  return { id, paint: (name: Paint) => `url(#${id}-${name})` };
}

/** The last few hovered keys, newest first: their glow layers stay drawn after hover moves on, so they can fade out. */
export function useLingering<K>(hovered: K | null, keep = 3): K[] {
  const [recent, setRecent] = useState<K[]>([]);
  if (hovered !== null && recent[0] !== hovered) setRecent([hovered, ...recent.filter((k) => k !== hovered)].slice(0, keep));
  return recent;
}

const stripes = (id: string, color: string) => (
  <pattern id={id} width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
    <rect width="2" height="7" style={{ fill: color }} />
  </pattern>
);

/** A blur of the shape at `strength` opacity; with `merge` the shape itself is laid back on top. */
const glow = (id: string, blur: number, strength: number, merge: boolean) => (
  <filter id={id} filterUnits="userSpaceOnUse" x="-10%" y="-10%" width="120%" height="120%" colorInterpolationFilters="sRGB">
    <feGaussianBlur in="SourceGraphic" stdDeviation={blur} result="blur" />
    <feComponentTransfer in="blur" result="halo">
      <feFuncA type="linear" slope={strength} />
    </feComponentTransfer>
    {merge && (
      <feMerge>
        <feMergeNode in="halo" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    )}
  </filter>
);

/** Gradients, patterns and filters the SVG charts paint with: the top gloss, the lens behind a dial, two hatches and two glows. */
export function GlassDefs({ id, neutral }: { id: string; neutral: string }) {
  return (
    <defs>
      <linearGradient id={`${id}-gloss`} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".16" />
        <stop offset=".5" stopColor="#fff" stopOpacity="0" />
        <stop offset="1" stopColor="#000" stopOpacity=".14" />
      </linearGradient>
      <radialGradient id={`${id}-lens`}>
        <stop offset="0" style={{ stopColor: "var(--fg)", stopOpacity: 0.05 }} />
        <stop offset="1" style={{ stopColor: "var(--fg)", stopOpacity: 0 }} />
      </radialGradient>
      {stripes(`${id}-binned`, "var(--hatch-line)")}
      {stripes(`${id}-hidden`, neutral)}
      {/* The hover glow, drawn on a layer of its own; and a soft resting halo applied to the shape itself. */}
      {glow(`${id}-glow`, 7, 1, false)}
      {glow(`${id}-halo`, 4, 0.35, true)}
    </defs>
  );
}

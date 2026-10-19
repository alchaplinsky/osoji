import { useId, type CSSProperties } from "react";
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

const stripes = (id: string, color: string) => (
  <pattern id={id} width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
    <rect width="2" height="7" style={{ fill: color }} />
  </pattern>
);

/** The glow filter for an SVG piece: a bright one when hovered, else a soft halo where `halo` asks for it. */
export const glowOf = (paint: (name: Paint) => string, hovered: boolean, halo: boolean) =>
  hovered ? paint("glow") : halo ? paint("halo") : undefined;

/** A glow in the shape's own colour: its blur, at `strength` opacity, laid under the shape. */
const glow = (id: string, blur: number, strength: number) => (
  <filter id={id} filterUnits="userSpaceOnUse" x="-10%" y="-10%" width="120%" height="120%" colorInterpolationFilters="sRGB">
    <feGaussianBlur in="SourceGraphic" stdDeviation={blur} result="blur" />
    <feComponentTransfer in="blur" result="halo">
      <feFuncA type="linear" slope={strength} />
    </feComponentTransfer>
    <feMerge>
      <feMergeNode in="halo" />
      <feMergeNode in="SourceGraphic" />
    </feMerge>
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
      {glow(`${id}-glow`, 6, 0.9)}
      {glow(`${id}-halo`, 4, 0.35)}
    </defs>
  );
}

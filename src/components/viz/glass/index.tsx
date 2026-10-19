import { useId, type CSSProperties } from "react";
import styles from "./glass.module.css";

export { styles as glass };

/** Inline style that tints a glass piece with its chart colour. */
export const tint = (color: string, style?: CSSProperties) => ({ "--seg": color, ...style }) as CSSProperties;

type Paint = "gloss" | "lens" | "binned" | "hidden";

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

/** Gradients and patterns the SVG charts paint with: the top gloss, the lens behind a dial, and two hatches. */
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
    </defs>
  );
}

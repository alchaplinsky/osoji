import { MAX_DEPTH, TAU, type Arc } from "./layout";

/** `ring` is the pitch from one ring to the next; `gap` is the empty part of it, also left around the hole. */
export type Geometry = { cx: number; cy: number; r0: number; ring: number; gap: number };

/** The prototype's proportions: the hole is 32% of the radius, and each ring's pitch is 7 parts ring to 1 part gap. */
const HOLE = 0.32;
const GAP_SHARE = 1 / 8;

export function geometry(width: number, height: number): Geometry {
  const R = Math.max(0, Math.min(width, height) / 2 - 6);
  const r0 = R * HOLE;
  // The outermost ring ends at R: MAX_DEPTH pitches minus the gap after the last one.
  const ring = (R - r0) / (MAX_DEPTH - GAP_SHARE);
  return { cx: width / 2, cy: height / 2, r0, ring, gap: ring * GAP_SHARE };
}

/** Gap between neighbouring arcs, in px along the ring's mid-line. */
const ARC_GAP = 2.5;
const CORNER = 5;
const MIN_SWEEP = 0.003;

/**
 * SVG path for an arc's ring segment, inset by the gaps and with rounded corners; null when too thin to draw.
 * Depth may be fractional while zooming, which can pull a ring into the hole.
 */
export function arcPath(g: Geometry, arc: Arc): string | null {
  const r0 = Math.max(0, g.r0 + (arc.depth - 1) * g.ring);
  const r1 = g.r0 + arc.depth * g.ring - g.gap;
  if (r1 - r0 < 1) return null;
  if (arc.a1 - arc.a0 >= TAU - 1e-9) return annulus(g, r0, r1);
  const gap = ARC_GAP / ((r0 + r1) / 2);
  const a0 = arc.a0 + gap / 2 - Math.PI / 2;
  const a1 = arc.a1 - gap / 2 - Math.PI / 2;
  if (a1 - a0 <= MIN_SWEEP) return null;
  return sector(g, r0, r1, a0, a1);
}

function point(g: Geometry, r: number, a: number) {
  return `${(g.cx + r * Math.cos(a)).toFixed(2)} ${(g.cy + r * Math.sin(a)).toFixed(2)}`;
}

const large = (sweep: number) => (sweep > Math.PI ? 1 : 0);

function sector(g: Geometry, r0: number, r1: number, a0: number, a1: number): string {
  const P = (r: number, a: number) => point(g, r, a);
  const sweep = a1 - a0;
  const rc = Math.min(CORNER, (r1 - r0) / 2 - 0.5, (sweep * r0) / 2 - 0.3);
  if (rc < 0.8) {
    return `M${P(r1, a0)}A${r1} ${r1} 0 ${large(sweep)} 1 ${P(r1, a1)}L${P(r0, a1)}A${r0} ${r0} 0 ${large(sweep)} 0 ${P(r0, a0)}Z`;
  }
  // Corner arcs are approximated by stepping rc along each edge, as an angle on the outer and inner rings.
  const dO = rc / r1;
  const dI = rc / r0;
  return [
    `M${P(r1 - rc, a0)}`,
    `A${rc} ${rc} 0 0 1 ${P(r1, a0 + dO)}`,
    `A${r1} ${r1} 0 ${large(sweep - 2 * dO)} 1 ${P(r1, a1 - dO)}`,
    `A${rc} ${rc} 0 0 1 ${P(r1 - rc, a1)}`,
    `L${P(r0 + rc, a1)}`,
    `A${rc} ${rc} 0 0 1 ${P(r0, a1 - dI)}`,
    `A${r0} ${r0} 0 ${large(sweep - 2 * dI)} 0 ${P(r0, a0 + dI)}`,
    `A${rc} ${rc} 0 0 1 ${P(r0 + rc, a0)}Z`,
  ].join("");
}

/** A full ring, drawn as two circles; paint it with `fill-rule: evenodd`. */
function annulus(g: Geometry, r0: number, r1: number): string {
  const circle = (r: number) => `M${g.cx - r} ${g.cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0Z`;
  return circle(r1) + circle(r0);
}

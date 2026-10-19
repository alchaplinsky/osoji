import { MAX_DEPTH, TAU, type Arc } from "./layout";

const EPS = 1e-9;

/** Both views' arcs in the outer view's angles: the inner view's arcs replace what the outer one drew inside `pivot`. */
export type World = { arcs: Arc[]; pivot: Arc };

export function zoomWorld(outer: Arc[], inner: Arc[], innerId: number): World | null {
  const pivot = outer.find((a) => a.key === innerId);
  if (!pivot) return null;
  const inside = (a: Arc) => a.depth > pivot.depth && a.a0 >= pivot.a0 - EPS && a.a1 <= pivot.a1 + EPS;
  const scale = (pivot.a1 - pivot.a0) / TAU;
  const nested = inner.map((a) => ({ ...a, depth: a.depth + pivot.depth, a0: pivot.a0 + a.a0 * scale, a1: pivot.a0 + a.a1 * scale }));
  return { arcs: outer.filter((a) => !inside(a)).concat(nested), pivot };
}

export type Shown = { arc: Arc; fade: number };

/**
 * The world through a camera `p` of the way from the outer view to the pivot: angles stretch so the pivot
 * spans the circle and rings shift inward by its depth. Rings sliding past the hole or the rim fade out.
 */
export function frame({ arcs, pivot }: World, p: number): Shown[] {
  const c0 = pivot.a0 * p;
  const c1 = TAU + (pivot.a1 - TAU) * p;
  const shift = pivot.depth * p;
  const shown: Shown[] = [];
  for (const arc of arcs) {
    const ring = arc.depth - shift;
    if (ring <= 0.02 || ring >= MAX_DEPTH + 0.98) continue;
    const a0 = Math.max(0, ((arc.a0 - c0) / (c1 - c0)) * TAU);
    const a1 = Math.min(TAU, ((arc.a1 - c0) / (c1 - c0)) * TAU);
    if (a1 <= a0) continue;
    const fade = ring < 1 ? ring : ring > MAX_DEPTH ? MAX_DEPTH + 1 - ring : 1;
    shown.push({ arc: { ...arc, depth: ring, a0, a1 }, fade });
  }
  return shown;
}

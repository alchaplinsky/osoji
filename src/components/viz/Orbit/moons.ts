import type { Slice } from "../../../screens/Breakdown/slices";

export type Moon = { slice: Slice; x: number; y: number; r: number };

export type OrbitGeometry = { cx: number; cy: number; R: number };

export const RINGS = [0.5, 0.68, 0.84];
const ORBIT_OF = [0.68, 0.5, 0.84];
export const CORE = 0.25;

export function orbitGeometry(w: number, h: number): OrbitGeometry {
  return { cx: w / 2, cy: h / 2, R: Math.max(0, Math.min(w, h) / 2 - 6) };
}

export function placeMoons(slices: Slice[], total: number, { cx, cy, R }: OrbitGeometry): Moon[] {
  let cum = 0;
  return slices.map((slice, i) => {
    const share = total > 0 ? slice.size / total : 0;
    const a = (cum + share / 2) * Math.PI * 2 - Math.PI / 2;
    cum += share;
    const r = Math.max(7, Math.min(R * 0.16, 6 + R * 0.24 * Math.sqrt(share)));
    const orbit = R * ORBIT_OF[i % ORBIT_OF.length];
    return { slice, x: cx + orbit * Math.cos(a), y: cy + orbit * Math.sin(a), r };
  });
}

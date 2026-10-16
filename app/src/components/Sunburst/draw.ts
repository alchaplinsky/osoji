import type { Arc } from "./layout";

export type Geometry = { cx: number; cy: number; r0: number; ring: number };

export function geometry(width: number, height: number, maxDepth: number): Geometry {
  const R = Math.min(width, height) / 2 - 12;
  const r0 = R * 0.22;
  return { cx: width / 2, cy: height / 2, r0, ring: (R - r0) / Math.max(maxDepth, 1) };
}

export function arcColor(arc: Arc, dark: boolean, highlighted: boolean): string {
  if (arc.hue < 0) return dark ? "hsl(0 0% 35%)" : "hsl(0 0% 78%)";
  const base = dark ? 42 : 58;
  const l = base + (arc.depth - 1) * (dark ? 6 : 5) + (highlighted ? 10 : 0);
  const s = arc.node?.dir ? 62 : 38;
  return `hsl(${arc.hue} ${s}% ${Math.min(l, 85)}%)`;
}

export function draw(
  ctx: CanvasRenderingContext2D,
  arcs: Arc[],
  g: Geometry,
  dark: boolean,
  highlightId: number | null,
  holeColor: string,
) {
  const gap = 0.004;
  for (const arc of arcs) {
    const inner = g.r0 + (arc.depth - 1) * g.ring;
    const outer = inner + g.ring - 1.5;
    if (arc.a1 - arc.a0 < gap * 2) continue; // sub-pixel slivers
    const a0 = arc.a0 + gap - Math.PI / 2;
    const a1 = arc.a1 - gap - Math.PI / 2;
    ctx.beginPath();
    ctx.arc(g.cx, g.cy, outer, a0, a1);
    ctx.arc(g.cx, g.cy, inner, a1, a0, true);
    ctx.closePath();
    ctx.fillStyle = arcColor(arc, dark, arc.node?.id === highlightId);
    ctx.fill();
  }
  ctx.beginPath();
  ctx.arc(g.cx, g.cy, g.r0 - 2, 0, Math.PI * 2);
  ctx.fillStyle = holeColor;
  ctx.fill();
}

export function hitTest(arcs: Arc[], g: Geometry, x: number, y: number): Arc | "center" | null {
  const dx = x - g.cx;
  const dy = y - g.cy;
  const r = Math.hypot(dx, dy);
  if (r < g.r0) return "center";
  const depth = Math.floor((r - g.r0) / g.ring) + 1;
  let theta = Math.atan2(dy, dx) + Math.PI / 2;
  if (theta < 0) theta += Math.PI * 2;
  for (const arc of arcs) {
    if (arc.depth === depth && theta >= arc.a0 && theta < arc.a1) return arc;
  }
  return null;
}

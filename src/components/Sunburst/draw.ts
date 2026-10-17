import type { ThemeName } from "../../design/theme";
import { neutral, tone } from "../../design/palette";
import { MAX_DEPTH, relatedOpacity, type Arc, type SliceKey } from "./layout";

export type Geometry = { cx: number; cy: number; r0: number; ring: number };

export function geometry(width: number, height: number): Geometry {
  const R = Math.max(0, Math.min(width, height) / 2 - 6);
  const r0 = Math.round(R * 0.28);
  return { cx: width / 2, cy: height / 2, r0, ring: (R - r0) / MAX_DEPTH };
}

export type Colors = { bg: string; card: string; line: string };

export function readColors(el: Element): Colors {
  const css = getComputedStyle(el);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return { bg: v("--bg"), card: v("--card"), line: v("--line") };
}

export const arcFill = (theme: ThemeName, arc: Arc) => (arc.node ? tone(theme, arc.index, arc.depth) : neutral(theme));

const RING_GAP = 3;

export function draw(ctx: CanvasRenderingContext2D, arcs: Arc[], g: Geometry, theme: ThemeName, related: Set<SliceKey> | null, colors: Colors) {
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = colors.bg;
  for (const arc of arcs) {
    const inner = g.r0 + (arc.depth - 1) * g.ring;
    const outer = g.r0 + arc.depth * g.ring - RING_GAP;
    const a0 = arc.a0 - Math.PI / 2;
    const a1 = arc.a1 - Math.PI / 2;
    ctx.beginPath();
    ctx.arc(g.cx, g.cy, outer, a0, a1);
    ctx.arc(g.cx, g.cy, inner, a1, a0, true);
    ctx.closePath();
    ctx.globalAlpha = relatedOpacity(related, arc.key);
    ctx.fillStyle = arcFill(theme, arc);
    ctx.fill();
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
  ctx.beginPath();
  ctx.arc(g.cx, g.cy, g.r0, 0, Math.PI * 2);
  ctx.fillStyle = colors.card;
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = colors.line;
  ctx.stroke();
}

export function hitTest(arcs: Arc[], g: Geometry, x: number, y: number): Arc | "center" | null {
  const dx = x - g.cx;
  const dy = y - g.cy;
  const r = Math.hypot(dx, dy);
  if (r < g.r0) return "center";
  const depth = Math.floor((r - g.r0) / g.ring) + 1;
  if (depth > MAX_DEPTH) return null;
  let theta = Math.atan2(dy, dx) + Math.PI / 2;
  if (theta < 0) theta += Math.PI * 2;
  return arcs.find((a) => a.depth === depth && theta >= a.a0 && theta < a.a1) ?? null;
}

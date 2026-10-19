import { useEffect, useState } from "react";
import type { ViewNode } from "../../lib/api";
import { pathTo } from "../../lib/tree";

const DURATION = 470;

/** A move between two views where one holds the other: `path` runs from `outer` down to `inner`. */
export type Transition<M> = { outer: ViewNode; inner: ViewNode; path: ViewNode[]; outerMeta: M };

/** Where the camera is: `p` runs from 0 (the outer view fills the chart) to 1 (the inner one does). */
export type Zoom<M> = Transition<M> & { p: number };

const reducedMotion = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

function relate<M>(from: { view: ViewNode; meta: M }, to: { view: ViewNode; meta: M }): Transition<M> & { dir: "in" | "out" } | null {
  if (from.view.id === to.view.id || reducedMotion()) return null;
  const down = pathTo(from.view, to.view.id);
  if (down.length > 1) return { outer: from.view, inner: to.view, path: down, outerMeta: from.meta, dir: "in" };
  const up = pathTo(to.view, from.view.id);
  if (up.length > 1) return { outer: to.view, inner: from.view, path: up, outerMeta: to.meta, dir: "out" };
  return null;
}

const easeOut = (t: number) => 1 - (1 - t) ** 4;

/**
 * Tracks drilling in and out of `view`. While a move between a view and one inside it plays, returns the
 * pair and the camera's position between them; null otherwise (and for unrelated jumps or reduced motion).
 * `meta` rides along with each view, e.g. the disk a root view spans.
 */
export function useZoom<M>(view: ViewNode, meta: M): Zoom<M> | null {
  const [shown, setShown] = useState({ view, meta });
  const [move, setMove] = useState<ReturnType<typeof relate<M>>>(null);
  const [t, setT] = useState(1);

  if (shown.view !== view) {
    const next = relate(shown, { view, meta });
    setShown({ view, meta });
    setMove(next);
    setT(next ? 0 : 1);
  }

  useEffect(() => {
    if (!move) return;
    let frame = 0;
    const start = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / DURATION);
      setT(k);
      if (k < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [move]);

  if (!move || t >= 1) return null;
  const e = easeOut(t);
  return { outer: move.outer, inner: move.inner, path: move.path, outerMeta: move.outerMeta, p: move.dir === "in" ? e : 1 - e };
}

export type Box = { x: number; y: number; w: number; h: number };

const lerp = (a: number, b: number, p: number) => a + (b - a) * p;

/** `box`, laid out to fill `whole`, scaled down to sit inside `focus` instead. */
export function nestBox(box: Box, whole: Box, focus: Box): Box {
  const sx = focus.w / whole.w;
  const sy = focus.h / whole.h;
  return { x: focus.x + (box.x - whole.x) * sx, y: focus.y + (box.y - whole.y) * sy, w: box.w * sx, h: box.h * sy };
}

/** `box` as seen through a camera that has moved `p` of the way from framing `whole` to framing `focus`. */
export function cameraBox(box: Box, whole: Box, focus: Box, p: number): Box {
  const cam = { x: lerp(whole.x, focus.x, p), y: lerp(whole.y, focus.y, p), w: lerp(whole.w, focus.w, p), h: lerp(whole.h, focus.h, p) };
  const sx = whole.w / cam.w;
  const sy = whole.h / cam.h;
  return { x: whole.x + (box.x - cam.x) * sx, y: whole.y + (box.y - cam.y) * sy, w: box.w * sx, h: box.h * sy };
}

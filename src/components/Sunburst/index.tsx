import { useEffect, useMemo, useRef, useState } from "react";
import type { ViewNode } from "../../lib/api";
import { layoutArcs, type Arc } from "./layout";
import { draw, geometry, hitTest } from "./draw";

type Props = {
  view: ViewNode;
  highlightId: number | null;
  onHover: (node: ViewNode | null, parent: ViewNode | null) => void;
  onDrill: (id: number) => void;
  onUp: () => void;
};

export default function Sunburst({ view, highlightId, onHover, onDrill, onUp }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const { arcs, maxDepth } = useMemo(() => layoutArcs(view), [view]);
  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;

  useEffect(() => {
    const el = canvasRef.current?.parentElement;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setSize({ w: e.contentRect.width, h: e.contentRect.height }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = size.w * dpr;
    canvas.height = size.h * dpr;
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, size.w, size.h);
    const hole = getComputedStyle(document.documentElement).getPropertyValue("--hole").trim();
    draw(ctx, arcs, geometry(size.w, size.h, maxDepth), dark, highlightId, hole);
  }, [arcs, maxDepth, size, dark, highlightId]);

  function locate(e: React.MouseEvent): Arc | "center" | null {
    const rect = canvasRef.current!.getBoundingClientRect();
    return hitTest(arcs, geometry(size.w, size.h, maxDepth), e.clientX - rect.left, e.clientY - rect.top);
  }

  return (
    <div className="sunburst">
      <canvas
        ref={canvasRef}
        onMouseMove={(e) => {
          const hit = locate(e);
          if (hit && hit !== "center") onHover(hit.node, hit.parent);
          else onHover(null, null);
        }}
        onMouseLeave={() => onHover(null, null)}
        onClick={(e) => {
          const hit = locate(e);
          if (hit === "center") onUp();
          else if (hit && hit.node?.dir) onDrill(hit.node.id);
        }}
      />
    </div>
  );
}

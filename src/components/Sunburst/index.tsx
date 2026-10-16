import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import type { ThemeName } from "../../design/theme";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { useElementSize } from "../../hooks/useElementSize";
import { cx } from "../ui";
import CenterLabel from "../viz/CenterLabel";
import { layoutArcs, relatedKeys, type Arc } from "./layout";
import { draw, geometry, hitTest, readColors } from "./draw";
import styles from "./Sunburst.module.css";

type SunburstProps = {
  view: ViewNode;
  theme: ThemeName;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onDrill: (id: number) => void;
  onUp: () => void;
  /** Clicking a file arc; directory arcs drill instead. */
  onSelectFile?: (id: number) => void;
  /** Font size of the centre size label in px. */
  centerSize?: number;
  className?: string;
};

export default function Sunburst({ view, theme, hoverId, onHover, onDrill, onUp, onSelectFile, centerSize = 20, className }: SunburstProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [pointer, setPointer] = useState(false);
  const arcs = useMemo(() => layoutArcs(view), [view]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.w === 0 || size.h === 0) return;
    // Deferred so theme variables applied by ancestor effects are in place before colours are read.
    const frame = requestAnimationFrame(() => {
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(size.w * dpr);
      const h = Math.round(size.h * dpr);
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size.w, size.h);
      draw(ctx, arcs, geometry(size.w, size.h), theme, related, readColors(canvas));
    });
    return () => cancelAnimationFrame(frame);
  }, [arcs, related, size, theme]);

  function locate(e: MouseEvent): Arc | "center" | null {
    const rect = e.currentTarget.getBoundingClientRect();
    return hitTest(arcs, geometry(size.w, size.h), e.clientX - rect.left, e.clientY - rect.top);
  }

  return (
    <div ref={rootRef} className={cx(styles.root, className)}>
      <canvas
        ref={canvasRef}
        className={cx(styles.canvas, pointer && styles.pointer)}
        onMouseMove={(e) => {
          const hit = locate(e);
          setPointer(hit !== null);
          const id = hit && hit !== "center" ? hit.hover : null;
          if (id !== hoverId) onHover(id);
        }}
        onMouseLeave={() => {
          setPointer(false);
          if (hoverId !== null) onHover(null);
        }}
        onClick={(e) => {
          const hit = locate(e);
          if (hit === "center") onUp();
          else if (hit?.node?.dir) onDrill(hit.node.id);
          else if (hit?.node) onSelectFile?.(hit.node.id);
        }}
      />
      <CenterLabel view={view} size={centerSize} />
    </div>
  );
}

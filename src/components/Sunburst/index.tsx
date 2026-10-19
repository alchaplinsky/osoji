import { useMemo } from "react";
import { neutral } from "../../design/palette";
import { THEMES } from "../../design/theme";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { humanBytes, humanPercent } from "../../lib/format";
import { useElementSize } from "../../hooks/useElementSize";
import { nodeFill, nodeName } from "../../screens/Breakdown/slices";
import { cx } from "../ui";
import CenterLabel from "../viz/CenterLabel";
import { GlassDefs, glass, tint, useLingering, usePaints } from "../viz/glass";
import { pointerProps } from "../viz/interact";
import type { Disk, VizProps } from "../viz/types";
import { useZoom } from "../viz/zoom";
import { arcPath, geometry } from "./geometry";
import { MAX_DEPTH, TAU, binnedKeys, layoutArcs, otherKey, relatedKeys, relatedOpacity, type Arc, type SliceKey } from "./layout";
import { frame, zoomWorld, type Shown } from "./zoom";
import styles from "./Sunburst.module.css";

/** The arc a hover id points at: "other" means the view's own tail. */
const hoveredKey = (view: ViewNode, hoverId: HoverId): SliceKey | null => (hoverId === "other" ? otherKey(view.id) : hoverId);

/** The centre readout: the hovered arc and its share of the circle, else the circle's whole. */
function centerOf(view: ViewNode, disk: Disk | undefined, hovered: Arc | undefined) {
  if (hovered) return { name: hovered.name, size: hovered.size, note: humanPercent(((hovered.a1 - hovered.a0) / TAU) * 100) };
  if (!disk) return { name: nodeName(view), size: view.size };
  const used = disk.total - disk.free;
  return { name: disk.name, size: used, note: `${humanPercent((used / disk.total) * 100)} of ${humanBytes(disk.total)}` };
}

export default function Sunburst({ view, theme, hoverId, onHover, onDrill, onUp, onSelectFile, onContextMenu, binIds, disk, centerSize = 20, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const { id, paint } = usePaints();
  const arcs = useMemo(() => layoutArcs(view, disk), [view, disk]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const zoom = useZoom(view, disk);
  const { outer, inner, outerMeta } = zoom ?? {};
  const binned = useMemo(() => binnedKeys(outer ?? view, binIds), [outer, view, binIds]);
  // Built once per move; each frame only re-projects it.
  const world = useMemo(() => (outer && inner ? zoomWorld(layoutArcs(outer, outerMeta), layoutArcs(inner), inner.id) : null), [outer, inner, outerMeta]);
  const shown: Shown[] = zoom && world ? frame(world, zoom.p) : arcs.map((arc) => ({ arc, fade: 1 }));
  const g = geometry(size.w, size.h);
  const hoverKey = hoveredKey(view, hoverId);
  // Dark themes keep a soft halo on the inner ring; Light glows only on hover.
  const dark = THEMES[theme].dark;
  const glowing = useLingering(hoverKey);
  const actions = { onHover, onDrill, onSelectFile, onContextMenu };

  return (
    <div ref={rootRef} className={cx(styles.root, zoom && styles.zooming, className)}>
      {size.w > 0 && size.h > 0 && (
        <svg className={styles.svg} width={size.w} height={size.h}>
          <GlassDefs id={id} neutral={neutral(theme)} />
          {/* One gap past the outer ring, as in the prototype. */}
          <circle className={styles.lens} cx={g.cx} cy={g.cy} r={g.r0 + MAX_DEPTH * g.ring} style={{ fill: paint("lens") }} />
          {shown.map(({ arc, fade }) => {
            const d = arcPath(g, arc);
            if (!d) return null;
            const inBin = binned.has(arc.key);
            const hovered = arc.key === hoverKey;
            const extra = arc.key === "hidden" || arc.key === "free";
            const color = arc.node ? nodeFill(arc.node, theme, arc.index, arc.count, arc.depth) : neutral(theme);
            return (
              <g
                key={arc.key}
                className={cx(styles.arc, arc.node && styles.node, hovered && cx(styles.lifted, glass.hovered), inBin && glass.binned)}
                style={{ opacity: fade * relatedOpacity(related, arc.key) }}
                {...pointerProps(arc.node, arc.hover, actions)}
              >
                {extra ? (
                  <path d={d} className={styles[arc.key as "hidden" | "free"]} style={arc.key === "hidden" ? { fill: paint("hidden") } : undefined} />
                ) : (
                  <>
                    {!inBin && glowing.includes(arc.key) && <path d={d} className={glass.halo} filter={paint("glow")} style={tint(color)} />}
                    <path
                      d={d}
                      className={glass.shape}
                      filter={!inBin && dark && arc.depth === 1 ? paint("halo") : undefined}
                      style={tint(color, inBin ? { fill: paint("binned") } : undefined)}
                    />
                    {!inBin && <path d={d} className={glass.gloss} style={{ fill: paint("gloss") }} />}
                  </>
                )}
              </g>
            );
          })}
          <circle className={styles.hole} cx={g.cx} cy={g.cy} r={g.r0 - g.gap} onClick={onUp} />
        </svg>
      )}
      <CenterLabel {...centerOf(view, disk, arcs.find((a) => a.key === hoverKey))} fontSize={centerSize} />
    </div>
  );
}

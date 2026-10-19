import { useId, useMemo } from "react";
import { neutral } from "../../design/palette";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { humanBytes, humanPercent } from "../../lib/format";
import { useElementSize } from "../../hooks/useElementSize";
import { nodeFill, nodeName } from "../../screens/Breakdown/slices";
import { cx } from "../ui";
import CenterLabel from "../viz/CenterLabel";
import { pointerProps } from "../viz/interact";
import type { Disk, VizProps } from "../viz/types";
import { arcPath, geometry } from "./geometry";
import { TAU, layoutArcs, otherKey, relatedKeys, relatedOpacity, type Arc } from "./layout";
import styles from "./Sunburst.module.css";

/** The centre readout: the hovered arc and its share of the circle, else the circle's whole. */
function centerOf(view: ViewNode, disk: Disk | undefined, arcs: Arc[], hoverId: HoverId) {
  const key = hoverId === "other" ? otherKey(view.id) : hoverId;
  const hovered = arcs.find((a) => a.key === key);
  if (hovered) return { name: hovered.name, size: hovered.size, note: humanPercent(((hovered.a1 - hovered.a0) / TAU) * 100) };
  if (!disk) return { name: nodeName(view), size: view.size };
  const used = disk.total - disk.free;
  return { name: disk.name, size: used, note: `${humanPercent((used / disk.total) * 100)} of ${humanBytes(disk.total)}` };
}

export default function Sunburst({ view, theme, hoverId, onHover, onDrill, onUp, onSelectFile, onContextMenu, disk, centerSize = 20, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  // useId() contains colons, which break url(#…) references in WebKit.
  const hatchId = useId().replace(/:/g, "");
  const arcs = useMemo(() => layoutArcs(view, disk), [view, disk]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const g = geometry(size.w, size.h);
  const actions = { onHover, onDrill, onSelectFile, onContextMenu };

  const fill = (arc: Arc) => {
    if (arc.node) return nodeFill(arc.node, theme, arc.index, arc.depth);
    if (arc.key === "hidden") return `url(#${hatchId})`;
    return arc.key === "free" ? undefined : neutral(theme);
  };

  return (
    <div ref={rootRef} className={cx(styles.root, className)}>
      {size.w > 0 && size.h > 0 && (
        <svg className={styles.svg} width={size.w} height={size.h}>
          <defs>
            <pattern id={hatchId} width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
              <rect width="2" height="6" fill={neutral(theme)} />
            </pattern>
          </defs>
          {arcs.map((arc) => {
            const d = arcPath(g, arc);
            if (!d) return null;
            return (
              <path
                key={arc.key}
                d={d}
                className={cx(styles.arc, arc.node && styles.node, arc.key === "free" && styles.free)}
                style={{ fill: fill(arc), opacity: relatedOpacity(related, arc.key) }}
                {...pointerProps(arc.node, arc.hover, actions)}
              />
            );
          })}
          <circle className={styles.hole} cx={g.cx} cy={g.cy} r={g.r0 - g.gap} onClick={onUp} />
        </svg>
      )}
      <CenterLabel {...centerOf(view, disk, arcs, hoverId)} fontSize={centerSize} />
    </div>
  );
}

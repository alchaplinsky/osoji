import { useMemo } from "react";
import type { ViewNode } from "../../../lib/api";
import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { pathTo } from "../../../lib/tree";
import { visibleSlices, type Slice } from "../../../screens/Breakdown/slices";
import { relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { Mono, cx } from "../../ui";
import { dragProps, pointerProps } from "../interact";
import type { VizProps } from "../types";
import InsideBar from "./InsideBar";
import Legend from "./Legend";
import styles from "./Shelf.module.css";

const LABEL_SHARE = 10;

/** The folder shown under the bar: the hovered child folder (or the one holding the hovered node), else the first child folder. */
function focusOf(view: ViewNode, slices: Slice[], hoverId: HoverId): ViewNode | null {
  if (typeof hoverId === "number") {
    const top = pathTo(view, hoverId)[1];
    if (top?.dir) return top;
  }
  return slices.find((s) => s.node?.dir)?.node ?? null;
}

export default function Shelf({ view, theme, hoverId, onHover, onDrill, onSelectFile, onDragStart, onContextMenu, className }: VizProps) {
  const slices = useMemo(() => visibleSlices(view, theme), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const focus = focusOf(view, slices, hoverId);
  const actions = { onHover, onDrill, onSelectFile, onContextMenu };

  return (
    <div className={cx(styles.root, className)}>
      <div>
        <div className={styles.bar}>
          {slices.map((slice) => {
            const pct = view.size > 0 ? (slice.size / view.size) * 100 : 0;
            return (
              <div
                key={slice.key}
                className={cx(styles.segment, !slice.node && styles.rest)}
                style={{ width: `${pct}%`, background: slice.fill, opacity: relatedOpacity(related, slice.key) }}
                {...pointerProps(slice.node, slice.hover, actions)}
                {...dragProps(slice.node, onDragStart)}
              >
                {pct >= LABEL_SHARE && (
                  <div className={styles.segmentLabel}>
                    <div className={styles.segmentName}>{slice.name}</div>
                    <Mono className={styles.segmentSize}>{humanBytes(slice.size)}</Mono>
                  </div>
                )}
              </div>
            );
          })}
        </div>
        <Legend slices={slices} actions={actions} />
      </div>
      {focus && <InsideBar focus={focus} theme={theme} related={related} actions={actions} />}
    </div>
  );
}

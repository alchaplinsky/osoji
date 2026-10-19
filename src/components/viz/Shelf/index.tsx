import { useMemo } from "react";
import type { ViewNode } from "../../../lib/api";
import type { HoverId } from "../../../lib/types";
import { isTrash } from "../../../lib/bin";
import { humanBytes } from "../../../lib/format";
import { pathTo } from "../../../lib/tree";
import { visibleSlices, type Slice } from "../../../screens/Breakdown/slices";
import { binnedKeys, relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { Mono, cx } from "../../ui";
import { glass, tint } from "../glass";
import { dragProps, pointerProps } from "../interact";
import type { VizProps } from "../types";
import InsideBar from "./InsideBar";
import Legend from "./Legend";
import { MIN_SHARE } from "./share";
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

export default function Shelf({ view, theme, hoverId, onHover, onDrill, onSelectFile, onDragStart, onContextMenu, binIds, className }: VizProps) {
  const slices = useMemo(() => visibleSlices(view, theme, 14, MIN_SHARE), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const binned = useMemo(() => binnedKeys(view, binIds), [view, binIds]);
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
                className={cx(
                  styles.segment,
                  glass.piece,
                  !slice.node && styles.rest,
                (!slice.node || isTrash(slice.node)) && glass.muted,
                  hoverId !== null && hoverId === slice.hover && cx(styles.lifted, glass.hovered),
                  binned.has(slice.key) && glass.binned,
                )}
                style={tint(slice.fill, { width: `${pct}%`, opacity: relatedOpacity(related, slice.key) })}
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
      {focus && <InsideBar focus={focus} theme={theme} hoverId={hoverId} related={related} binned={binned} actions={actions} />}
    </div>
  );
}

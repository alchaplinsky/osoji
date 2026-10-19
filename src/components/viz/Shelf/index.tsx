import { useMemo } from "react";
import type { ViewNode } from "../../../lib/api";
import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { pathTo } from "../../../lib/tree";
import { visibleSlices, type Slice } from "../../../screens/Breakdown/slices";
import { binnedKeys, relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { Mono, cx } from "../../ui";
import { glass, tint } from "../glass";
import { dragProps, pointerProps } from "../interact";
import type { VizProps } from "../types";
import { cameraBox, nestBox, useZoom, type Box } from "../zoom";
import InsideBar from "./InsideBar";
import Legend from "./Legend";
import styles from "./Shelf.module.css";

const LABEL_SHARE = 10;
/** Space between segments, in px. */
const GAP = 4;
/** The bar as a box in percent: segments are spans along x. */
const WHOLE: Box = { x: 0, y: 0, w: 100, h: 1 };
const EDGE = 0.01;

type Span = Box & { slice: Slice };

/** Each slice as a span of the bar, in percent. */
function spansOf(view: ViewNode, slices: Slice[]): Span[] {
  let x = 0;
  return slices.map((slice) => {
    const w = view.size > 0 ? (slice.size / view.size) * 100 : 0;
    const span = { slice, x, y: 0, w, h: 1 };
    x += w;
    return span;
  });
}

/** The folder shown under the bar: the hovered child folder (or the one holding the hovered node), else the first child folder. */
function focusOf(view: ViewNode, slices: Slice[], hoverId: HoverId): ViewNode | null {
  if (typeof hoverId === "number") {
    const top = pathTo(view, hoverId)[1];
    if (top?.dir) return top;
  }
  return slices.find((s) => s.node?.dir)?.node ?? null;
}

export default function Shelf({ view, theme, hoverId, onHover, onDrill, onSelectFile, onDragStart, onContextMenu, binIds, className }: VizProps) {
  const slices = useMemo(() => visibleSlices(view, theme), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const zoom = useZoom(view, null);
  const { outer, inner, path } = zoom ?? {};
  const binned = useMemo(() => binnedKeys(outer ?? view, binIds), [outer, view, binIds]);
  const focus = focusOf(view, slices, hoverId);
  const actions = { onHover, onDrill, onSelectFile, onContextMenu };

  // Both views laid out once per move: the outer one along the bar, the inner one inside the segment it zooms from.
  const world = useMemo(() => {
    if (!outer || !inner || !path) return null;
    const outerSpans = spansOf(outer, visibleSlices(outer, theme));
    const pivot = outerSpans.find((s) => s.slice.key === path[1].id);
    if (!pivot) return null;
    const innerSpans = spansOf(inner, visibleSlices(inner, theme)).map((s) => ({ ...s, ...nestBox(s, WHOLE, pivot) }));
    return { outerSpans, innerSpans, pivot };
  }, [outer, inner, path, theme]);

  const bar =
    zoom && world
      ? [
          ...world.outerSpans.map((s) => ({ ...s, ...cameraBox(s, WHOLE, world.pivot, zoom.p), fade: 1 - zoom.p })),
          ...world.innerSpans.map((s) => ({ ...s, ...cameraBox(s, WHOLE, world.pivot, zoom.p), fade: zoom.p })),
        ]
      : spansOf(view, slices).map((s) => ({ ...s, fade: 1 }));

  return (
    <div className={cx(styles.root, className)}>
      <div>
        <div className={cx(styles.bar, zoom && styles.zooming)}>
          {bar.map(({ slice, x, w, fade }) => (
            <div
              key={slice.key}
              className={cx(
                styles.segment,
                glass.piece,
                !slice.node && styles.rest,
                x <= EDGE && styles.first,
                x + w >= 100 - EDGE && styles.last,
                hoverId !== null && hoverId === slice.hover && cx(styles.lifted, glass.hovered),
                binned.has(slice.key) && glass.binned,
              )}
              style={tint(slice.fill, { left: `calc(${x}% + ${GAP / 2}px)`, width: `calc(${w}% - ${GAP}px)`, opacity: fade * relatedOpacity(related, slice.key) })}
              {...pointerProps(slice.node, slice.hover, actions)}
              {...dragProps(slice.node, onDragStart)}
            >
              {w >= LABEL_SHARE && (
                <div className={styles.segmentLabel}>
                  <div className={styles.segmentName}>{slice.name}</div>
                  <Mono className={styles.segmentSize}>{humanBytes(slice.size)}</Mono>
                </div>
              )}
            </div>
          ))}
        </div>
        <Legend slices={slices} actions={actions} />
      </div>
      {focus && <InsideBar focus={focus} theme={theme} hoverId={hoverId} related={related} binned={binned} actions={actions} />}
    </div>
  );
}

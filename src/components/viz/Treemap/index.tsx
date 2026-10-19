import { useMemo } from "react";
import type { ThemeName } from "../../../design/theme";
import type { ViewNode } from "../../../lib/api";
import { useElementSize } from "../../../hooks/useElementSize";
import { visibleSlices } from "../../../screens/Breakdown/slices";
import { binnedKeys, relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { cx } from "../../ui";
import { dragProps, pointerProps } from "../interact";
import type { VizProps } from "../types";
import { cameraBox, nestBox, useZoom, type Box } from "../zoom";
import { squarify } from "./squarify";
import Tile from "./Tile";
import styles from "./Treemap.module.css";

const cellsOf = (view: ViewNode, theme: ThemeName, box: Box) =>
  squarify(visibleSlices(view, theme).map((slice) => ({ slice, value: slice.size })), box.x, box.y, box.w, box.h);

export default function Treemap({ view, theme, hoverId, onHover, onDrill, onSelectFile, onDragStart, onContextMenu, binIds, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const whole = useMemo(() => ({ x: 0, y: 0, w: size.w, h: size.h }), [size]);
  const cells = useMemo(() => cellsOf(view, theme, whole), [view, theme, whole]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const zoom = useZoom(view, null);
  const { outer, inner, path } = zoom ?? {};
  const binned = useMemo(() => binnedKeys(outer ?? view, binIds), [outer, view, binIds]);

  // Both views laid out once per move: the outer one in the chart, the inner one inside the tile it zooms from.
  const world = useMemo(() => {
    if (!outer || !inner || !path) return null;
    const outerCells = cellsOf(outer, theme, whole);
    const focus = outerCells.find((c) => c.slice.key === path[1].id);
    if (!focus) return null;
    const innerCells = cellsOf(inner, theme, whole).map((c) => ({ ...c, ...nestBox(c, whole, focus) }));
    return { outerCells, innerCells, focus };
  }, [outer, inner, path, theme, whole]);

  const tiles =
    zoom && world
      ? [
          ...world.outerCells.map((c) => ({ ...c, ...cameraBox(c, whole, world.focus, zoom.p), fade: 1 - zoom.p })),
          ...world.innerCells.map((c) => ({ ...c, ...cameraBox(c, whole, world.focus, zoom.p), fade: zoom.p })),
        ]
      : cells.map((c) => ({ ...c, fade: 1 }));

  return (
    <div ref={rootRef} className={cx(styles.root, zoom && styles.zooming, className)}>
      {tiles.map(({ slice, fade, ...cell }) => (
        <Tile
          key={slice.key}
          slice={slice}
          cell={cell}
          hovered={hoverId !== null && hoverId === slice.hover}
          binned={binned.has(slice.key)}
          opacity={fade * relatedOpacity(related, slice.key)}
          {...pointerProps(slice.node, slice.hover, { onHover, onDrill, onSelectFile, onContextMenu })}
          {...dragProps(slice.node, onDragStart)}
        />
      ))}
    </div>
  );
}

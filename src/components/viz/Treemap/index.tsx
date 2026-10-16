import { useMemo } from "react";
import { useElementSize } from "../../../hooks/useElementSize";
import { visibleSlices } from "../../../screens/Breakdown/slices";
import { relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { cx } from "../../ui";
import { dragProps, pointerProps } from "../interact";
import type { VizProps } from "../types";
import { squarify } from "./squarify";
import Tile from "./Tile";
import styles from "./Treemap.module.css";

export default function Treemap({ view, theme, hoverId, onHover, onDrill, onSelectFile, onDragStart, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const slices = useMemo(() => visibleSlices(view, theme), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const cells = useMemo(() => squarify(slices.map((slice) => ({ slice, value: slice.size })), 0, 0, size.w, size.h), [slices, size]);

  return (
    <div ref={rootRef} className={cx(styles.root, className)}>
      {cells.map(({ slice, ...cell }) => (
        <Tile
          key={slice.key}
          slice={slice}
          cell={cell}
          theme={theme}
          hovered={hoverId !== null && hoverId === slice.hover}
          opacity={relatedOpacity(related, slice.key)}
          {...pointerProps(slice.node, slice.hover, { onHover, onDrill, onSelectFile })}
          {...dragProps(slice.node, onDragStart)}
        />
      ))}
    </div>
  );
}

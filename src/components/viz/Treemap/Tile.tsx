import type { DragEvent, MouseEventHandler } from "react";
import type { ThemeName } from "../../../design/theme";
import { hueAt, shade } from "../../../design/palette";
import { humanBytes } from "../../../lib/format";
import type { Slice } from "../../../screens/Breakdown/slices";
import { Mono, cx } from "../../ui";
import { squarify, type Rect } from "./squarify";
import styles from "./Treemap.module.css";

type TileProps = {
  slice: Slice;
  /** Outer cell; the tile is inset by 2px on each side. */
  cell: Rect;
  theme: ThemeName;
  hovered: boolean;
  opacity: number;
  onMouseEnter: MouseEventHandler;
  onMouseLeave: MouseEventHandler;
  onClick: MouseEventHandler;
  draggable?: boolean;
  onDragStart?: (e: DragEvent) => void;
};

const KIDS = 8;

export default function Tile({ slice, cell, theme, hovered, opacity, ...handlers }: TileProps) {
  const w = cell.w - 4;
  const h = cell.h - 4;
  const node = slice.node;
  const kids =
    node?.dir && w >= 140 && h >= 96
      ? squarify(
          node.children.slice(0, KIDS).map((child, j) => ({ child, j, value: child.size })),
          0,
          0,
          w - 16,
          h - 46,
        )
      : [];

  return (
    <div
      className={cx(styles.tile, node && (node.dir ? styles.dir : styles.file), hovered && styles.hovered)}
      style={{ left: cell.x + 2, top: cell.y + 2, width: w, height: h, background: slice.fill, opacity }}
      {...handlers}
    >
      {w >= 66 && h >= 36 && (
        <>
          <div className={styles.name}>{slice.name}</div>
          <Mono className={styles.size}>{humanBytes(slice.size)}</Mono>
        </>
      )}
      {kids.map((k) => (
        <div
          key={k.child.id}
          className={styles.kid}
          style={{ left: k.x + 8, top: k.y + 40, width: k.w - 3, height: k.h - 3, background: shade(theme, hueAt(k.j), 2) }}
        >
          {k.w >= 56 && k.h >= 20 && k.child.name}
        </div>
      ))}
    </div>
  );
}

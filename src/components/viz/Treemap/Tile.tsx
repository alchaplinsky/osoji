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
/** Room above the children grid for the name and size lines (9px padding + 16px + 2px + 14px, plus a 9px gap). */
const HEAD = 50;
/** Inset of the children grid from the tile's sides and bottom. */
const INSET = 10;

export default function Tile({ slice, cell, theme, hovered, opacity, ...handlers }: TileProps) {
  const w = cell.w - 4;
  const h = cell.h - 4;
  const node = slice.node;
  const kids =
    node?.dir && w >= 140 && h >= HEAD + 60
      ? squarify(
          node.children.slice(0, KIDS).map((child, j) => ({ child, j, value: child.size })),
          0,
          0,
          w - INSET * 2,
          h - HEAD - INSET,
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
          style={{ left: k.x + INSET, top: k.y + HEAD, width: k.w - 3, height: k.h - 3, background: shade(theme, hueAt(k.j), 2) }}
        >
          {k.w >= 56 && k.h >= 20 && k.child.name}
        </div>
      ))}
    </div>
  );
}

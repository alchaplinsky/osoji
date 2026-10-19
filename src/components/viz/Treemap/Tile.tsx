import type { DragEvent, MouseEventHandler } from "react";
import { humanBytes } from "../../../lib/format";
import type { Slice } from "../../../screens/Breakdown/slices";
import { Mono, cx } from "../../ui";
import { glass, tint } from "../glass";
import { squarify, type Rect } from "./squarify";
import styles from "./Treemap.module.css";

type TileProps = {
  slice: Slice;
  cell: Rect;
  hovered: boolean;
  binned: boolean;
  opacity: number;
  onMouseEnter: MouseEventHandler;
  onMouseLeave: MouseEventHandler;
  onClick: MouseEventHandler;
  onContextMenu?: MouseEventHandler;
  draggable?: boolean;
  onDragStart?: (e: DragEvent) => void;
};

const KIDS = 8;
/** Room above the children grid for the name and size lines (1px border + 8px padding + 16px + 2px + 14px, plus a 9px gap). */
const HEAD = 50;
/** Inset of the children grid from the tile's sides and bottom. */
const INSET = 10;
/** Space between neighbouring tiles. */
const GAP = 6;

export default function Tile({ slice, cell, hovered, binned, opacity, ...handlers }: TileProps) {
  const w = cell.w - GAP;
  const h = cell.h - GAP;
  const node = slice.node;
  const kids =
    node?.dir && w >= 140 && h >= HEAD + 60
      ? squarify(
          node.children.slice(0, KIDS).map((child) => ({ child, value: child.size })),
          0,
          0,
          w - INSET * 2,
          h - HEAD - INSET,
        )
      : [];

  return (
    <div
      className={cx(styles.tile, glass.piece, node && (node.dir ? styles.dir : styles.file), hovered && cx(styles.lifted, glass.hovered), binned && glass.binned)}
      style={tint(slice.fill, { left: cell.x + GAP / 2, top: cell.y + GAP / 2, width: w, height: h, opacity })}
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
          style={{ left: k.x + INSET, top: k.y + HEAD, width: k.w - 3, height: k.h - 3 }}
        >
          {k.w >= 56 && k.h >= 20 && k.child.name}
        </div>
      ))}
    </div>
  );
}

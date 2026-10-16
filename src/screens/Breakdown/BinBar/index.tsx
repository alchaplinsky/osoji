import { useState, type DragEvent } from "react";
import type { BinItem } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { Button, Icon, Mono, cx } from "../../../components/ui";
import BinChip from "./BinChip";
import { readBinDrop } from "./drag";
import styles from "./BinBar.module.css";

export { startBinDrag } from "./drag";

type BinBarProps = {
  items: BinItem[];
  onRemove: (id: number) => void;
  onDrop: (id: number) => void;
  onReview: () => void;
  /** The list panel is hidden, so the bar may grow wider. */
  wide: boolean;
  /** Overlays the bottom of a full-screen list (Largest files, Suggestions) instead of the viz column. */
  floating?: boolean;
};

const VISIBLE_CHIPS = 3;

export default function BinBar({ items, onRemove, onDrop, onReview, wide, floating }: BinBarProps) {
  const [over, setOver] = useState(false);
  const empty = items.length === 0;
  const total = items.reduce((sum, item) => sum + item.size, 0);
  const chips = items.slice(0, VISIBLE_CHIPS);
  const more = items.length - chips.length;
  const width = empty ? 480 : Math.min(wide ? 860 : 560, 300 + chips.length * 150 + (more > 0 ? 50 : 0));

  const dragOver = (e: DragEvent) => {
    e.preventDefault();
    if (!over) setOver(true);
  };
  const dragLeave = (e: DragEvent) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false);
  };
  const drop = (e: DragEvent) => {
    e.preventDefault();
    setOver(false);
    const id = readBinDrop(e);
    if (id !== null) onDrop(id);
  };

  return (
    <div
      className={cx(styles.bar, floating && styles.floating, !empty && styles.filled, over && styles.over)}
      style={{ width }}
      onDragOver={dragOver}
      onDragLeave={dragLeave}
      onDrop={drop}
    >
      <Icon name="trash" size={18} className={styles.icon} />
      <div className={styles.content}>
        {empty && <div className={styles.empty}>Drop items here, or tick them. Nothing goes until you review.</div>}
        {chips.map((item) => (
          <BinChip key={item.id} item={item} onRemove={() => onRemove(item.id)} />
        ))}
        {more > 0 && <div className={styles.more}>+{more}</div>}
      </div>
      <Button variant="primary" size="lg" className={styles.review} disabled={empty} onClick={onReview}>
        Free up {!empty && <Mono>{humanBytes(total)}</Mono>}
      </Button>
    </div>
  );
}

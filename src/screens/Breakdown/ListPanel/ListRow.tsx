import type { DragEvent } from "react";
import { Checkbox, Icon, Mono, cx } from "../../../components/ui";
import styles from "./ListPanel.module.css";

type ListRowProps = {
  name: string;
  sizeLabel: string;
  color: string;
  /** Bar fill, 0–100. */
  pct: number;
  dir: boolean;
  hovered: boolean;
  inBin: boolean;
  onEnter: () => void;
  onLeave: () => void;
  onOpen?: () => void;
  /** Omitted for rows that cannot go in the bin (the merged "smaller items" row). */
  onToggleBin?: () => void;
  onDragStart?: (e: DragEvent<HTMLDivElement>) => void;
};

export default function ListRow({ name, sizeLabel, color, pct, dir, hovered, inBin, onEnter, onLeave, onOpen, onToggleBin, onDragStart }: ListRowProps) {
  const binnable = !!onToggleBin;
  return (
    <div
      className={cx(styles.row, binnable && styles.rowDraggable, hovered && styles.rowHovered)}
      draggable={binnable}
      onDragStart={onDragStart}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      {onToggleBin ? <Checkbox checked={inBin} onChange={onToggleBin} aria-label={inBin ? `Remove ${name} from the bin` : `Add ${name} to the bin`} /> : <span />}
      <span className={styles.swatch} style={{ background: color }} />
      <div className={styles.body}>
        <div className={cx(styles.title, dir && styles.titleDir)} onClick={dir ? onOpen : undefined}>
          <span className={styles.name}>{name}</span>
          {dir && <Icon name="chevronRightSm" className={styles.chevron} />}
        </div>
        <div className={styles.track}>
          <div className={styles.fill} style={{ width: `${pct}%`, background: color }} />
        </div>
      </div>
      <Mono size={12} className={styles.size}>
        {sizeLabel}
      </Mono>
    </div>
  );
}

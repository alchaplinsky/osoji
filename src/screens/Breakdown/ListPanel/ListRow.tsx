import type { DragEvent, KeyboardEvent, MouseEvent } from "react";
import { Checkbox, Icon, Mono, cx } from "../../../components/ui";
import styles from "./ListPanel.module.css";

type ListRowProps = {
  name: string;
  sizeLabel: string;
  color: string;
  pct: number;
  dir: boolean;
  hovered: boolean;
  inBin: boolean;
  onEnter: () => void;
  onLeave: () => void;
  onOpen?: () => void;
  onToggleBin?: () => void;
  onDragStart?: (e: DragEvent<HTMLDivElement>) => void;
  onReveal?: () => void;
  onContextMenu?: (e: MouseEvent<HTMLDivElement>) => void;
};

export default function ListRow({ name, sizeLabel, color, pct, dir, hovered, inBin, onEnter, onLeave, onOpen, onToggleBin, onDragStart, onReveal, onContextMenu }: ListRowProps) {
  const binnable = !!onToggleBin;
  const open = dir ? onOpen : undefined;
  const onKeyDown = open
    ? (e: KeyboardEvent<HTMLDivElement>) => {
        if (e.target !== e.currentTarget) return;
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
      }
    : undefined;
  return (
    <div
      className={cx(styles.row, binnable && styles.rowDraggable, open && styles.rowOpenable, hovered && styles.rowHovered)}
      role={open ? "button" : undefined}
      tabIndex={open ? 0 : undefined}
      aria-label={open ? `Open ${name}` : undefined}
      draggable={binnable}
      onDragStart={onDragStart}
      onClick={open}
      // A folder row opens on the first click, so only file rows reveal on double-click.
      onDoubleClick={open ? undefined : onReveal}
      onContextMenu={onContextMenu}
      onKeyDown={onKeyDown}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      <span className={styles.mark}>
        <span className={styles.swatch} style={{ background: color }} />
        {onToggleBin && (
          <Checkbox checked={inBin} onChange={onToggleBin} className={styles.check} aria-label={inBin ? `Remove ${name} from the bin` : `Add ${name} to the bin`} />
        )}
      </span>
      <div className={styles.body}>
        <div className={styles.title}>
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

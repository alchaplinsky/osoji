import type { DragEvent, KeyboardEvent } from "react";
import { Checkbox, Icon, Mono, cx } from "../../../components/ui";
import GuardIcon from "../../../components/GuardIcon";
import type { Guard } from "../../../lib/types";
import styles from "./ListPanel.module.css";

type ListRowProps = {
  name: string;
  sizeLabel: string;
  color: string;
  pct: number;
  dir: boolean;
  hovered: boolean;
  inBin: boolean;
  guard: Guard | null;
  onEnter: () => void;
  onLeave: () => void;
  onOpen?: () => void;
  onToggleBin?: () => void;
  onDragStart?: (e: DragEvent<HTMLDivElement>) => void;
};

export default function ListRow({ name, sizeLabel, color, pct, dir, hovered, inBin, guard, onEnter, onLeave, onOpen, onToggleBin, onDragStart }: ListRowProps) {
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
          <GuardIcon guard={guard} />
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

import { Icon, Mono, cx } from "../../../components/ui";
import styles from "./ListPanel.module.css";

type ListRowProps = {
  name: string;
  sizeLabel: string;
  color: string;
  /** Bar fill, 0–100. */
  pct: number;
  dir: boolean;
  hovered: boolean;
  onEnter: () => void;
  onLeave: () => void;
  onOpen?: () => void;
};

export default function ListRow({ name, sizeLabel, color, pct, dir, hovered, onEnter, onLeave, onOpen }: ListRowProps) {
  return (
    // PR 3 adds a 16px checkbox column in front of the swatch.
    <div className={cx(styles.row, hovered && styles.rowHovered)} onMouseEnter={onEnter} onMouseLeave={onLeave}>
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

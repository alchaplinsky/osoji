import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { Mono, cx } from "../../../components/ui";
import type { Slice } from "../slices";
import { hatch, type NotScanned } from "./capacity";
import styles from "./CapacityBar.module.css";

type LegendProps = {
  items: Slice[];
  dimmed: (s: Slice) => boolean;
  onHover: (id: HoverId) => void;
  onOpen: (s: Slice) => void;
  notScanned?: NotScanned | null;
};

export default function Legend({ items, dimmed, onHover, onOpen, notScanned }: LegendProps) {
  return (
    <div className={styles.legend}>
      {items.map((s) => (
        <div
          key={s.key}
          className={cx(styles.legendItem, dimmed(s) && styles.legendDimmed)}
          onMouseEnter={() => onHover(s.hover)}
          onMouseLeave={() => onHover(null)}
          onClick={() => onOpen(s)}
        >
          <span className={styles.swatch} style={{ background: s.fill }} />
          <span>{s.name}</span>
          <Mono className={styles.legendSize}>{humanBytes(s.size)}</Mono>
        </div>
      ))}
      {notScanned && (
        <div className={cx(styles.legendItem, styles.legendStatic, notScanned.dimmed && styles.legendDimmed)} title={notScanned.title}>
          <span className={cx(styles.swatch, styles.hatched)} style={hatch(notScanned.color)} />
          <span>Not scanned</span>
          <Mono className={styles.legendSize}>{humanBytes(notScanned.size)}</Mono>
        </div>
      )}
    </div>
  );
}

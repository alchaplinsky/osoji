import type { ViewNode } from "../../../lib/api";
import type { HoverId, Volume } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { useTheme } from "../../../hooks/useTheme";
import { Mono, Panel, cx } from "../../../components/ui";
import { isRelated, relatedKeys } from "../../../components/Sunburst/layout";
import { slicesOf, type Slice } from "../slices";
import Legend from "./Legend";
import styles from "./CapacityBar.module.css";

type CapacityBarProps = {
  volume: Volume;
  /** The root view (id 0); its children are the segments. */
  root: ViewNode;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onOpen: (id: number) => void;
  /** Bytes queued in the bin. */
  pending: number;
};

export default function CapacityBar({ volume, root, hoverId, onHover, onOpen, pending }: CapacityBarProps) {
  const used = volume.total - volume.free;
  const { theme } = useTheme();
  const related = relatedKeys(root, hoverId);
  const segments = slicesOf(root, theme);
  const dimmed = (s: Slice) => !isRelated(related, s.key);
  const open = (s: Slice) => {
    if (s.node?.dir) onOpen(s.node.id);
  };

  return (
    <Panel className={styles.panel}>
      <div className={styles.summary}>
        <Mono numeral size={28}>
          {humanBytes(used)}
        </Mono>
        <span className={styles.usedLabel}>used</span>
        <span className={styles.spacer} />
        {pending > 0 && <Mono className={styles.pending}>−{humanBytes(pending)} after sweep</Mono>}
        <Mono numeral size={16} className={styles.free}>
          {humanBytes(volume.free)}
        </Mono>
        <span className={styles.freeLabel}>free of {humanBytes(volume.total)}</span>
      </div>
      <div className={styles.bar}>
        {segments.map((s) => (
          <div
            key={s.key}
            className={cx(styles.segment, dimmed(s) && styles.segmentDimmed)}
            style={{ width: `${(s.size / volume.total) * 100}%`, background: s.fill }}
            onMouseEnter={() => onHover(s.hover)}
            onMouseLeave={() => onHover(null)}
            onClick={() => open(s)}
          />
        ))}
        {pending > 0 && (
          <div
            className={styles.pendingStripe}
            style={{ left: `${((used - pending) / volume.total) * 100}%`, width: `${(pending / volume.total) * 100}%` }}
          />
        )}
      </div>
      <Legend items={segments.filter((s) => s.node).slice(0, 6)} dimmed={dimmed} onHover={onHover} onOpen={open} />
    </Panel>
  );
}

import type { ViewNode } from "../../../lib/api";
import type { HoverId, Volume } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { useTheme } from "../../../hooks/useTheme";
import { Mono, Panel, cx } from "../../../components/ui";
import { isRelated, relatedKeys } from "../../../components/Sunburst/layout";
import { slicesOf, type Slice } from "../slices";
import { neutral } from "../../../design/palette";
import { hatch, hiddenBytes, hiddenNote, type NotScanned } from "./capacity";
import Legend from "./Legend";
import styles from "./CapacityBar.module.css";

type CapacityBarProps = {
  volume: Volume;
  root: ViewNode;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onOpen: (id: number) => void;
  pending: number;
  /** Folders the scan could not read, mentioned in the "Not scanned" tooltip. */
  unreadable?: number;
};

export default function CapacityBar({ volume, root, hoverId, onHover, onOpen, pending, unreadable }: CapacityBarProps) {
  const used = volume.total - volume.free;
  const { theme } = useTheme();
  const related = relatedKeys(root, hoverId);
  const segments = slicesOf(root, theme);
  const dimmed = (s: Slice) => !isRelated(related, s.key);
  const hidden = hiddenBytes(volume, root);
  const notScanned: NotScanned | null = hidden > 0 ? { size: hidden, title: hiddenNote(unreadable), color: neutral(theme), dimmed: hoverId !== null && hoverId !== "hidden" } : null;
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
        {notScanned && (
          <div
            className={cx(styles.notScanned, styles.hatched, notScanned.dimmed && styles.segmentDimmed)}
            style={{ width: `${(notScanned.size / volume.total) * 100}%`, ...hatch(notScanned.color) }}
            title={notScanned.title}
          />
        )}
        {/* Absolute, over the tail of the scanned items the sweep will remove, not over the unscanned space. */}
        {pending > 0 && (
          <div
            className={styles.pendingStripe}
            style={{ left: `${((used - hidden - pending) / volume.total) * 100}%`, width: `${(pending / volume.total) * 100}%` }}
          />
        )}
      </div>
      <Legend items={segments.filter((s) => s.node).slice(0, 6)} dimmed={dimmed} onHover={onHover} onOpen={open} notScanned={notScanned} />
    </Panel>
  );
}

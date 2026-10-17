import type { CSSProperties } from "react";
import { cx } from "../cx";
import styles from "./SegmentBar.module.css";

export type Segment = { key: string | number; size: number; fill: string; title?: string };

type Props = {
  segments: Segment[];
  /** Size that fills the whole track. */
  total: number;
  height?: number;
  className?: string;
};

/** A track split into coloured segments sized by share of `total`, as in the capacity bar. */
export default function SegmentBar({ segments, total, height = 8, className }: Props) {
  const shown = total > 0 ? segments.filter((s) => s.size > 0) : [];
  return (
    <div className={cx(styles.track, className)} style={{ "--h": `${height}px` } as CSSProperties}>
      {shown.map((s) => (
        <div key={s.key} className={styles.segment} title={s.title} style={{ width: `${Math.min(100, (s.size / total) * 100)}%`, background: s.fill }} />
      ))}
    </div>
  );
}

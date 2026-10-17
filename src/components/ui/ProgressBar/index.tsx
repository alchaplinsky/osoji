import type { CSSProperties } from "react";
import { cx } from "../cx";
import styles from "./ProgressBar.module.css";

type Props = {
  value: number;
  height?: number;
  color?: string;
  className?: string;
  label?: string;
};

export default function ProgressBar({ value, height = 4, color = "var(--ac)", className, label }: Props) {
  const pct = Math.max(0, Math.min(100, value));
  const vars = { "--h": `${height}px`, "--w": `${pct}%`, "--fill": color } as CSSProperties;
  return (
    <div className={cx(styles.track, className)} style={vars} role="progressbar" aria-label={label} aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
      <div className={styles.fill} />
    </div>
  );
}

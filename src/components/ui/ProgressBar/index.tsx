import type { CSSProperties } from "react";
import { cx } from "../cx";
import styles from "./ProgressBar.module.css";

type Props = {
  /** 0–100, or `null` when the total isn't known yet: the bar sweeps instead of sitting at a bogus 0%. */
  value: number | null;
  height?: number;
  color?: string;
  className?: string;
  label?: string;
};

/** `color` is any CSS background; the default is the chart ramp gradient set by the theme provider. */
export default function ProgressBar({ value, height = 4, color = "var(--chart-ramp)", className, label }: Props) {
  const indeterminate = value === null;
  const pct = indeterminate ? 0 : Math.max(0, Math.min(100, value));
  const vars = { "--h": `${height}px`, "--w": `${pct}%`, "--fill": color } as CSSProperties;
  return (
    <div
      className={cx(styles.track, indeterminate && styles.indeterminate, className)}
      style={vars}
      role="progressbar"
      aria-label={label}
      aria-valuenow={indeterminate ? undefined : pct}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className={styles.fill} />
    </div>
  );
}

import type { HTMLAttributes } from "react";
import { cx } from "../cx";
import styles from "./Text.module.css";

/** Small uppercase section label. */
export function Eyebrow({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cx(styles.eyebrow, className)} {...rest} />;
}

type MonoProps = HTMLAttributes<HTMLSpanElement> & {
  /** Font size in px. */
  size?: number;
  /** Large-number treatment: medium weight, tight tracking, line-height 1. */
  numeral?: boolean;
};

/** Monospace text with tabular numerals, for sizes, paths and counts. */
export function Mono({ size, numeral, className, style, ...rest }: MonoProps) {
  return <span className={cx(styles.mono, numeral && styles.numeral, className)} style={size ? { fontSize: size, ...style } : style} {...rest} />;
}

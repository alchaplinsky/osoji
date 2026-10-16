import type { HTMLAttributes } from "react";
import { cx } from "../cx";
import styles from "./Text.module.css";

export function Eyebrow({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cx(styles.eyebrow, className)} {...rest} />;
}

type MonoProps = HTMLAttributes<HTMLSpanElement> & {
  size?: number;
  numeral?: boolean;
};

export function Mono({ size, numeral, className, style, ...rest }: MonoProps) {
  return <span className={cx(styles.mono, numeral && styles.numeral, className)} style={size ? { fontSize: size, ...style } : style} {...rest} />;
}

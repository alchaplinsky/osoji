import type { HTMLAttributes } from "react";
import { cx } from "../cx";
import styles from "./Badge.module.css";

export type SafetyTone = "safe" | "review" | "keep";

type Props = HTMLAttributes<HTMLSpanElement> & {
  variant?: "tint" | "outline" | "count";
  tone?: SafetyTone;
  color?: string;
};

export const SAFETY_LABEL: Record<SafetyTone, string> = { safe: "Safe", review: "Worth a look", keep: "Keep" };

export default function Badge({ variant = "tint", tone, color, className, style, ...rest }: Props) {
  return (
    <span
      className={cx(styles.badge, styles[variant], tone && styles[tone], className)}
      style={color ? ({ "--c": color, ...style } as React.CSSProperties) : style}
      {...rest}
    />
  );
}

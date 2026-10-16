import type { ButtonHTMLAttributes } from "react";
import { cx } from "../ui";
import styles from "./ReviewSheet.module.css";

type ModeOptionProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  hint: string;
  selected?: boolean;
};

export default function ModeOption({ label, hint, selected, className, type = "button", ...rest }: ModeOptionProps) {
  return (
    <button type={type} aria-pressed={selected} className={cx(styles.mode, selected && styles.selected, className)} {...rest}>
      <div className={styles.modeLabel}>{label}</div>
      <div className={styles.modeHint}>{hint}</div>
    </button>
  );
}

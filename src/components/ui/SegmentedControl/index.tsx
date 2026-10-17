import type { ReactNode } from "react";
import { cx } from "../cx";
import styles from "./SegmentedControl.module.css";

/** With `icon` set the option renders as a square glyph and `label` becomes its tooltip and accessible name. */
export type SegmentOption<T extends string> = { value: T; label: string; icon?: ReactNode; disabled?: boolean };

type Props<T extends string> = {
  options: readonly SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
};

export default function SegmentedControl<T extends string>({ options, value, onChange, size = "sm", className }: Props<T>) {
  return (
    <div role="radiogroup" className={cx(styles.root, styles[size], className)}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          aria-label={o.icon ? o.label : undefined}
          title={o.icon ? o.label : undefined}
          disabled={o.disabled}
          className={cx(styles.opt, !!o.icon && styles.iconOpt, o.value === value && styles.active)}
          onClick={() => onChange(o.value)}
        >
          {o.icon ?? o.label}
        </button>
      ))}
    </div>
  );
}

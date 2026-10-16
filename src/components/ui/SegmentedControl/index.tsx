import { cx } from "../cx";
import styles from "./SegmentedControl.module.css";

export type SegmentOption<T extends string> = { value: T; label: string; disabled?: boolean };

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
          disabled={o.disabled}
          className={cx(styles.opt, o.value === value && styles.active)}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

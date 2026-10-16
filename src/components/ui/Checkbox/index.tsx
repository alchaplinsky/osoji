import type { MouseEvent } from "react";
import Icon from "../Icon";
import { cx } from "../cx";
import styles from "./Checkbox.module.css";

type Props = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  className?: string;
  "aria-label"?: string;
};

export default function Checkbox({ checked, onChange, className, ...aria }: Props) {
  const toggle = (e: MouseEvent) => {
    e.stopPropagation();
    onChange(!checked);
  };
  return (
    <button type="button" role="checkbox" aria-checked={checked} className={cx(styles.box, checked && styles.checked, className)} onClick={toggle} {...aria}>
      {checked && <Icon name="check" strokeWidth={1.8} />}
    </button>
  );
}

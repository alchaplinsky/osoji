import { cx } from "../cx";
import styles from "./Toggle.module.css";

type Props = { checked: boolean; onChange: (checked: boolean) => void; className?: string; "aria-label"?: string };

export default function Toggle({ checked, onChange, className, ...aria }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={cx(styles.track, checked && styles.on, className)}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      {...aria}
    >
      <span className={styles.knob} />
    </button>
  );
}

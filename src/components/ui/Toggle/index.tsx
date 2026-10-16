import { cx } from "../cx";
import styles from "./Toggle.module.css";

type Props = { checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean; title?: string; className?: string; "aria-label"?: string };

export default function Toggle({ checked, onChange, disabled, title, className, ...aria }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      title={title}
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

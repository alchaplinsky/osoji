import type { ReactNode } from "react";
import { Card, Eyebrow, Toggle, cx } from "../../components/ui";
import styles from "./SettingGroup.module.css";

export default function SettingGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <Eyebrow className={styles.heading}>{title}</Eyebrow>
      <Card className={styles.group}>{children}</Card>
    </div>
  );
}

type SettingRowProps = {
  label: string;
  desc: string;
  checked: boolean;
  onChange?: (checked: boolean) => void;
  title?: string;
};

const noop = () => {};

export function SettingRow({ label, desc, checked, onChange, title }: SettingRowProps) {
  return (
    <div className={cx(styles.row, !onChange && styles.locked)} title={title} onClick={() => onChange?.(!checked)}>
      <div className={styles.text}>
        <div className={styles.label}>{label}</div>
        <div className={styles.desc}>{desc}</div>
      </div>
      <Toggle checked={checked} onChange={onChange ?? noop} disabled={!onChange} title={title} aria-label={label} />
    </div>
  );
}

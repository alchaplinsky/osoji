import type { ReactNode } from "react";
import { cx, Icon, type IconName } from "../ui";
import styles from "./NavItem.module.css";

type NavItemProps = {
  icon: IconName;
  label: ReactNode;
  active?: boolean;
  /** Dimmed, but still clickable so the owner can redirect (e.g. to the Scan screen). */
  disabled?: boolean;
  badge?: ReactNode;
  title?: string;
  onClick: () => void;
};

export default function NavItem({ icon, label, active, disabled, badge, title, onClick }: NavItemProps) {
  return (
    <button
      type="button"
      className={cx(styles.item, active && styles.active, disabled && styles.disabled)}
      aria-current={active ? "page" : undefined}
      aria-disabled={disabled || undefined}
      title={title}
      onClick={onClick}
    >
      <Icon name={icon} size={16} className={styles.icon} />
      <span className={styles.label}>{label}</span>
      {badge != null && badge !== "" && <span className={styles.badge}>{badge}</span>}
    </button>
  );
}

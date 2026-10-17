import type { ReactNode } from "react";
import { X } from "lucide-react";
import { cx } from "../cx";
import { Panel } from "../Surface";
import Button from "../Button";
import styles from "./Banner.module.css";

type BannerProps = {
  icon?: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
  onDismiss?: () => void;
  className?: string;
};

/** A full-width notice strip: optional leading icon, message, trailing actions and a dismiss button. */
export default function Banner({ icon, children, actions, onDismiss, className }: BannerProps) {
  return (
    <Panel role="status" className={cx(styles.banner, className)}>
      {icon && <span className={styles.icon}>{icon}</span>}
      <div className={styles.message}>{children}</div>
      {actions}
      {onDismiss && (
        <Button variant="ghost" size="sm" muted className={styles.dismiss} aria-label="Dismiss" title="Dismiss" onClick={onDismiss}>
          <X size={14} />
        </Button>
      )}
    </Panel>
  );
}

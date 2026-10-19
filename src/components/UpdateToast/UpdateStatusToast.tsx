import { Check, RefreshCw, X, type LucideIcon } from "lucide-react";
import type { UpdateCheckStatus } from "../../hooks/useAutoUpdate";
import { cx } from "../ui/cx";
import styles from "./UpdateToast.module.css";

const STATUS_COPY: Record<UpdateCheckStatus, { icon: LucideIcon; title: string; detail: string }> = {
  checking: { icon: RefreshCw, title: "Checking for updates…", detail: "Contacting GitHub Releases." },
  uptodate: { icon: Check, title: "You're up to date", detail: "Osoji is running the latest version." },
  error: { icon: X, title: "Update check failed", detail: "Couldn't reach GitHub Releases." },
};

/** Transient feedback for a manual check — auto-dismissed by the hook. */
export default function UpdateStatusToast({ status }: { status: UpdateCheckStatus }) {
  const { icon: StatusIcon, title, detail } = STATUS_COPY[status];
  return (
    <div className={styles.toast} role="status">
      <StatusIcon size={16} className={cx(styles.icon, status === "checking" && styles.spin)} />
      <div className={styles.body}>
        <div className={styles.text}>
          <strong>{title}</strong>
          <span>{detail}</span>
        </div>
      </div>
    </div>
  );
}

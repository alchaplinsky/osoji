import { Download } from "lucide-react";
import { ProgressBar } from "../ui";
import type { UpdateProgress } from "../../lib/autoUpdate";
import { humanBytes } from "../../lib/format";
import { cx } from "../ui/cx";
import styles from "./UpdateToast.module.css";

/** Whole-percent progress, or `null` when the total is unknown (no
 *  Content-Length yet), which is the bar's cue to sweep instead of showing 0%. */
export function downloadPercent(received: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.max(0, Math.round((received / total) * 100)));
}

/** A found update being fetched and staged — the slow part of a manual check,
 *  so it gets a byte count and a bar rather than a "checking" that never ends. */
export default function UpdateDownloadToast({ progress }: { progress: UpdateProgress }) {
  const { version, phase, received, total } = progress;
  const installing = phase === "installing";
  const pct = installing ? null : downloadPercent(received, total);

  let detail = `Version ${version}`;
  if (installing) detail += " · almost done";
  else if (pct !== null && total) detail += ` · ${pct}% · ${humanBytes(received)} of ${humanBytes(total)}`;
  else if (received > 0) detail += ` · ${humanBytes(received)}`;

  return (
    <div className={cx(styles.toast, styles.downloading)} role="status">
      <Download size={16} className={styles.icon} />
      <div className={styles.body}>
        <div className={styles.text}>
          <strong>{installing ? "Installing update…" : "Downloading update…"}</strong>
          <span>{detail}</span>
        </div>
        <ProgressBar value={pct} color="var(--ac)" label="Update download" />
      </div>
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import { Button, ProgressBar, Sheet } from "../ui";
import { humanBytes } from "../../lib/format";
import type { TrashProgress } from "../../lib/api";
import type { BinItem } from "../../lib/types";
import ModeOption from "./ModeOption";
import ReviewRow from "./ReviewRow";
import styles from "./ReviewSheet.module.css";

type ReviewSheetProps = {
  open: boolean;
  items: BinItem[];
  volumeName: string;
  busy: boolean;
  /** Sweep progress; the bar stays indeterminate until the first update arrives. */
  progress?: TrashProgress | null;
  error: string | null;
  warning?: string;
  onCancel: () => void;
  onConfirm: () => void;
};

export default function ReviewSheet({ open, items, volumeName, busy, progress, error, warning, onCancel, onConfirm }: ReviewSheetProps) {
  const total = humanBytes(items.reduce((sum, b) => sum + b.size, 0));
  const close = useCallback(() => {
    if (!busy) onCancel();
  }, [busy, onCancel]);

  // With a warning, the first click only arms the button; the second one sweeps.
  const [armed, setArmed] = useState(false);
  useEffect(() => setArmed(false), [open, items]);
  const needsArming = Boolean(warning) && !busy && !armed;
  const confirm = () => (needsArming ? setArmed(true) : onConfirm());
  const cancel = () => {
    setArmed(false);
    onCancel();
  };

  return (
    <Sheet
      open={open}
      onClose={close}
      title="Review before sweeping"
      sub={`${items.length} item${items.length === 1 ? "" : "s"} · ${total} will be moved to the Trash on ${volumeName}`}
      footer={
        <>
          <div className={styles.modes}>
            <ModeOption label="Move to Trash" hint="You can put items back later" selected />
            <ModeOption label="Delete permanently" hint="Not available yet" disabled title="Unshift never deletes permanently" />
          </div>
          {busy && progress && (
            <div>
              <ProgressBar value={progress.total > 0 ? (progress.done / progress.total) * 100 : 100} label="Moving to the Trash" />
              <div className={styles.progressLabel}>
                Moving {progress.current} · {progress.done} of {progress.total}
              </div>
            </div>
          )}
          {busy && !progress && (
            <div className={styles.progress} role="progressbar" aria-label="Moving to the Trash">
              <div className={styles.progressFill} />
            </div>
          )}
          <div className={styles.actions}>
            {error ? <div className={styles.error}>{error}</div> : warning && <div className={styles.warning}>{warning}</div>}
            <Button size="lg" onClick={cancel} disabled={busy}>Cancel</Button>
            <Button variant="primary" size="lg" onClick={confirm} disabled={busy}>
              {busy ? "Sweeping…" : warning && armed ? `Yes, move ${total} to Trash` : `Move ${total} to Trash`}
            </Button>
          </div>
        </>
      }
    >
      {items.map((item) => <ReviewRow key={item.id} item={item} />)}
    </Sheet>
  );
}

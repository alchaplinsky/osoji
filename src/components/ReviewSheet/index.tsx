import { useCallback, type MouseEvent } from "react";
import { Button, Sheet } from "../ui";
import { humanBytes } from "../../lib/format";
import type { BinItem } from "../../lib/types";
import ModeOption from "./ModeOption";
import ReviewRow from "./ReviewRow";
import styles from "./ReviewSheet.module.css";

type ReviewSheetProps = {
  open: boolean;
  items: BinItem[];
  volumeName: string;
  busy: boolean;
  error: string | null;
  warning?: string;
  onCancel: () => void;
  onConfirm: () => void;
  onItemMenu?: (e: MouseEvent, item: BinItem) => void;
};

export default function ReviewSheet({ open, items, volumeName, busy, error, warning, onCancel, onConfirm, onItemMenu }: ReviewSheetProps) {
  const total = humanBytes(items.reduce((sum, b) => sum + b.size, 0));
  const close = useCallback(() => {
    if (!busy) onCancel();
  }, [busy, onCancel]);

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
          {busy && (
            <div className={styles.progress} role="progressbar" aria-label="Moving to the Trash">
              <div className={styles.progressFill} />
            </div>
          )}
          <div className={styles.actions}>
            {error ? <div className={styles.error}>{error}</div> : warning && <div className={styles.warning}>{warning}</div>}
            <Button size="lg" onClick={onCancel} disabled={busy}>Cancel</Button>
            <Button variant="primary" size="lg" onClick={onConfirm} disabled={busy}>
              {busy ? "Sweeping…" : `Move ${total} to Trash`}
            </Button>
          </div>
        </>
      }
    >
      {items.map((item) => (
        <ReviewRow key={item.id} item={item} onContextMenu={onItemMenu && ((e) => onItemMenu(e, item))} />
      ))}
    </Sheet>
  );
}

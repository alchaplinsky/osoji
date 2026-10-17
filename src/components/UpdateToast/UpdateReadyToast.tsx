import { Download } from "lucide-react";
import { useState } from "react";
import { Button } from "../ui";
import { restartForUpdate } from "../../lib/autoUpdate";
import styles from "./UpdateToast.module.css";

type Props = { version: string; notes: string | null; onDismiss: () => void };

/** Update downloaded + staged: offer "Restart now" or "Skip for now". */
export default function UpdateReadyToast({ version, notes, onDismiss }: Props) {
  const [restarting, setRestarting] = useState(false);

  const onRestart = async () => {
    setRestarting(true);
    try {
      await restartForUpdate();
    } catch (err) {
      // Relaunch shouldn't fail, but if it does, don't trap the user in a
      // disabled state — let them dismiss and restart manually later.
      console.warn("Relaunch for update failed:", err);
      setRestarting(false);
    }
  };

  return (
    <div className={styles.toast} role="alert">
      <Download size={16} className={styles.icon} />
      <div className={styles.body}>
        <div className={styles.text}>
          <strong>Update ready</strong>
          <span>Version {version} has been downloaded.</span>
        </div>
        {notes && <p className={styles.notes}>{notes}</p>}
        <div className={styles.actions}>
          <Button variant="ghost" size="sm" muted onClick={onDismiss} disabled={restarting}>
            Skip for now
          </Button>
          <Button variant="primary" size="sm" onClick={onRestart} disabled={restarting}>
            {restarting ? "Restarting…" : "Restart now"}
          </Button>
        </div>
      </div>
    </div>
  );
}

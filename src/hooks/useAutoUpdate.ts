import { useCallback, useEffect, useRef, useState } from "react";
import { setupAppMenu } from "../lib/appMenu";
import { checkForUpdate, runStartupUpdateCheck, type UpdateProgress } from "../lib/autoUpdate";

/** A downloaded + staged update, waiting for a restart. */
export type UpdateReady = { version: string; notes: string | null };

/** Transient status of a *manual* "Check for Updates…" run. A found update
 *  transitions to `ready` instead. */
export type UpdateCheckStatus = "checking" | "uptodate" | "error";

export type AutoUpdate = {
  ready: UpdateReady | null;
  /** Download progress of the update a manual check found, while it's being fetched. */
  download: UpdateProgress | null;
  status: UpdateCheckStatus | null;
  /** Run an on-demand check, driving `status` for feedback and staging any update found. */
  runCheck: () => Promise<void>;
  /** Dismiss the restart toast. The staged update still applies on next launch. */
  dismiss: () => void;
};

const CLEAR_AFTER_MS: Record<Exclude<UpdateCheckStatus, "checking">, number> = { uptodate: 4000, error: 6000 };

/**
 * Owns the update lifecycle: a silent check at launch (no-op in dev), the
 * "Check for Updates…" menu item, and the state the {@link UpdateToast} renders.
 */
export function useAutoUpdate(): AutoUpdate {
  const [ready, setReady] = useState<UpdateReady | null>(null);
  const [download, setDownload] = useState<UpdateProgress | null>(null);
  const [status, setStatus] = useState<UpdateCheckStatus | null>(null);
  // Mirrors `status` so repeat menu clicks and stale auto-dismiss timers can
  // read the latest value without being re-created on every change.
  const statusRef = useRef<UpdateCheckStatus | null>(null);
  const update = (next: UpdateCheckStatus | null) => {
    statusRef.current = next;
    setStatus(next);
  };

  const runCheck = useCallback(async () => {
    // Ignore repeat clicks while a check is already in flight.
    if (statusRef.current === "checking") return;
    update("checking");

    // Status stays "checking" through the download too, so the guard above
    // still holds; `download` is what tells the toast to show progress.
    const result = await checkForUpdate(setDownload);
    setDownload(null);
    if (result.kind === "staged") {
      // Hand off to the restart toast; the transient status is done.
      update(null);
      setReady({ version: result.version, notes: result.notes });
      return;
    }

    const next = result.kind === "uptodate" ? "uptodate" : "error";
    update(next);
    // Auto-dismiss the feedback, but only if nothing has changed since — a new
    // check (or a staged update) may have superseded this one.
    setTimeout(() => {
      if (statusRef.current === next) update(null);
    }, CLEAR_AFTER_MS[next]);
  }, []);

  useEffect(() => {
    // Fire-and-forget so neither ever delays first paint.
    void runStartupUpdateCheck((version, notes) => setReady({ version, notes }));
    void setupAppMenu(() => void runCheck());
  }, [runCheck]);

  const dismiss = useCallback(() => setReady(null), []);

  return { ready, download, status, runCheck, dismiss };
}

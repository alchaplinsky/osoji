import { useCallback, useEffect, useState } from "react";
import { fullDiskAccess, openPrivacySettings } from "../lib/api";
import { isMac } from "../lib/platform";

export type FullDiskAccess = {
  /** `null` until the first probe answers. */
  granted: boolean | null;
  /** Hidden for the rest of this session, not across launches. */
  dismissed: boolean;
  dismiss: () => void;
  openSettings: () => void;
};

/**
 * Probes Full Disk Access on mount and again whenever the window regains focus, e.g. back from
 * System Settings. Full Disk Access is a macOS concept: elsewhere it counts as granted, unprobed.
 */
export function useFullDiskAccess(): FullDiskAccess {
  const [granted, setGranted] = useState<boolean | null>(isMac() ? null : true);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    if (!isMac()) return;
    let active = true;
    const probe = () => {
      fullDiskAccess()
        .then((ok) => active && setGranted(ok))
        .catch(console.error);
    };
    probe();
    window.addEventListener("focus", probe);
    return () => {
      active = false;
      window.removeEventListener("focus", probe);
    };
  }, []);

  const dismiss = useCallback(() => setDismissed(true), []);
  const openSettings = useCallback(() => {
    openPrivacySettings().catch(console.error);
  }, []);

  return { granted, dismissed, dismiss, openSettings };
}

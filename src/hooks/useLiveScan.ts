import { useEffect, useRef } from "react";
import { onScanChanged } from "../lib/api";
import { coalesce } from "./coalesce";

// Refreshes on-screen data when the backend reports on-disk changes to the scanned tree.
// Refreshes are coalesced so a burst of change batches costs at most one extra round-trip.
export function useLiveScan(onChanged: () => Promise<void> | void, { enabled }: { enabled: boolean }) {
  const latest = useRef(onChanged);
  latest.current = onChanged;

  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    // A follow-up queued before disabling must not fire into whatever took over (e.g. a sweep).
    const trigger = coalesce(() => (disposed ? undefined : latest.current()));
    const subscription = onScanChanged(trigger);
    subscription.catch(console.error);
    return () => {
      disposed = true;
      // listen() resolves asynchronously; unsubscribe whenever it lands.
      subscription.then((unlisten) => unlisten()).catch(console.error);
    };
  }, [enabled]);
}

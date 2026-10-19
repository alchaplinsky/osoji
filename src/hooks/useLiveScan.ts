import { useEffect, useRef } from "react";
import { onScanChanged } from "../lib/api";
import { pausable } from "./coalesce";

// Refreshes on-screen data when the backend reports on-disk changes to the scanned tree.
// Refreshes are coalesced so a burst of change batches costs at most one extra round-trip.
// `enabled` subscribes at all; `paused` keeps listening but holds refreshes until it clears, so a
// change that lands mid-sweep (the Trash growing) is refreshed once afterwards instead of lost.
export function useLiveScan(onChanged: () => Promise<void> | void, { enabled, paused }: { enabled: boolean; paused: boolean }) {
  const latest = useRef(onChanged);
  latest.current = onChanged;
  const isPaused = useRef(paused);
  const gate = useRef<ReturnType<typeof pausable> | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    const g = pausable(() => (disposed ? undefined : latest.current()));
    g.setPaused(isPaused.current);
    gate.current = g;
    const subscription = onScanChanged(g.trigger);
    subscription.catch(console.error);
    return () => {
      disposed = true;
      gate.current = null;
      // listen() resolves asynchronously; unsubscribe whenever it lands.
      subscription.then((unlisten) => unlisten()).catch(console.error);
    };
  }, [enabled]);

  useEffect(() => {
    isPaused.current = paused;
    gate.current?.setPaused(paused);
  }, [paused]);
}

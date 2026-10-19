import { useEffect, useRef } from "react";
import { onScanChanged } from "../lib/api";
import { pausable } from "./coalesce";

// Refreshes on-screen data when the backend reports on-disk changes to the scanned tree.
// Refreshes are coalesced so a burst of change batches costs at most one extra round-trip.
// `enabled` says whether a change is acted on at all; `paused` holds refreshes until it clears,
// so a change that lands mid-sweep (the Trash growing) is refreshed once afterwards instead of lost.
// The subscription itself lasts for the app's lifetime: the backend applies what changed during a
// walk right after it returns the summary, and a subscription made on "done" could miss that.
export function useLiveScan(onChanged: () => Promise<void> | void, { enabled, paused }: { enabled: boolean; paused: boolean }) {
  const latest = useRef(onChanged);
  latest.current = onChanged;
  const isEnabled = useRef(enabled);
  isEnabled.current = enabled;
  const isPaused = useRef(paused);
  const gate = useRef<ReturnType<typeof pausable> | null>(null);

  useEffect(() => {
    let disposed = false;
    const g = pausable(() => (disposed || !isEnabled.current ? undefined : latest.current()));
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
  }, []);

  useEffect(() => {
    isPaused.current = paused;
    gate.current?.setPaused(paused);
  }, [paused]);
}

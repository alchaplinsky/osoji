// Serializes calls to `run`. A trigger that lands while a run is in flight is remembered and
// replayed once that run ends, so a burst collapses into a single follow-up: runs never
// overlap, and the last trigger is never dropped. A failing run is reported, not rethrown,
// so one error cannot wedge the gate shut.
export function coalesce(run: () => Promise<void> | void, onError: (e: unknown) => void = console.error): () => void {
  let running = false;
  let pending = false;

  const drain = async () => {
    running = true;
    do {
      pending = false;
      try {
        await run();
      } catch (e) {
        onError(e);
      }
    } while (pending);
    running = false;
  };

  return () => {
    if (running) pending = true;
    else void drain();
  };
}

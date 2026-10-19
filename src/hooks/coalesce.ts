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

// `coalesce`, plus a pause: while paused, triggers only leave a mark, and resuming replays them
// as one run. The check is made when a run would start, so a follow-up queued before the pause
// waits for the resume too.
export function pausable(
  run: () => Promise<void> | void,
  onError?: (e: unknown) => void,
): { trigger: () => void; setPaused: (paused: boolean) => void } {
  let paused = false;
  let missed = false;
  const trigger = coalesce(() => {
    if (!paused) return run();
    missed = true;
  }, onError);
  const setPaused = (next: boolean) => {
    paused = next;
    if (!paused && missed) {
      missed = false;
      trigger();
    }
  };
  return { trigger, setPaused };
}

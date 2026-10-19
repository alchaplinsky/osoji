import { describe, expect, test, vi } from "vitest";
import { coalesce, pausable } from "./coalesce";

function deferred() {
  let resolve!: () => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("coalesce", () => {
  test("runs immediately when idle", () => {
    const run = vi.fn();
    const trigger = coalesce(run);
    trigger();
    expect(run).toHaveBeenCalledTimes(1);
  });

  test("collapses triggers during a run into exactly one follow-up", async () => {
    const runs = [deferred(), deferred()];
    const run = vi.fn(() => runs[run.mock.calls.length - 1].promise);
    const trigger = coalesce(run);

    trigger();
    trigger();
    trigger();
    trigger();
    expect(run).toHaveBeenCalledTimes(1);

    runs[0].resolve();
    await flush();
    expect(run).toHaveBeenCalledTimes(2);

    runs[1].resolve();
    await flush();
    expect(run).toHaveBeenCalledTimes(2);
  });

  test("never overlaps runs", async () => {
    let active = 0;
    let maxActive = 0;
    const gate = deferred();
    const trigger = coalesce(async () => {
      maxActive = Math.max(maxActive, ++active);
      await gate.promise;
      active--;
    });

    trigger();
    trigger();
    gate.resolve();
    await flush();
    trigger();
    await flush();
    expect(maxActive).toBe(1);
  });

  test("a failing run does not wedge the gate", async () => {
    const onError = vi.fn();
    const first = deferred();
    const run = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(undefined);
    const trigger = coalesce(run, onError);

    trigger();
    trigger();
    first.reject(new Error("boom"));
    await flush();
    expect(onError).toHaveBeenCalledWith(new Error("boom"));
    expect(run).toHaveBeenCalledTimes(2);

    trigger();
    expect(run).toHaveBeenCalledTimes(3);
  });

  test("a synchronous throw is reported and the gate reopens", async () => {
    const onError = vi.fn();
    const trigger = coalesce(() => {
      throw new Error("sync");
    }, onError);

    trigger();
    await flush();
    trigger();
    await flush();
    expect(onError).toHaveBeenCalledTimes(2);
  });
});

describe("pausable", () => {
  test("holds a trigger while paused and runs it once on resume", async () => {
    const run = vi.fn();
    const gate = pausable(run);
    gate.setPaused(true);
    gate.trigger();
    await flush();
    expect(run).not.toHaveBeenCalled();

    gate.setPaused(false);
    await flush();
    expect(run).toHaveBeenCalledTimes(1);
    gate.setPaused(false);
    await flush();
    expect(run).toHaveBeenCalledTimes(1);
  });

  test("collapses triggers while paused into one run", async () => {
    const run = vi.fn();
    const gate = pausable(run);
    gate.setPaused(true);
    gate.trigger();
    await flush();
    gate.trigger();
    gate.setPaused(false);
    await flush();
    expect(run).toHaveBeenCalledTimes(1);
  });

  test("resuming with nothing missed runs nothing", async () => {
    const run = vi.fn();
    const gate = pausable(run);
    gate.setPaused(true);
    gate.setPaused(false);
    await flush();
    expect(run).not.toHaveBeenCalled();
    gate.trigger();
    expect(run).toHaveBeenCalledTimes(1);
  });

  test("a follow-up queued before the pause waits for the resume", async () => {
    const first = deferred();
    const run = vi.fn().mockReturnValueOnce(first.promise).mockReturnValue(undefined);
    const gate = pausable(run);
    gate.trigger();
    gate.trigger();
    gate.setPaused(true);
    first.resolve();
    await flush();
    expect(run).toHaveBeenCalledTimes(1);

    gate.setPaused(false);
    await flush();
    expect(run).toHaveBeenCalledTimes(2);
  });
});

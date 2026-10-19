import { describe, expect, test, vi } from "vitest";
import { coalesce } from "./coalesce";

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

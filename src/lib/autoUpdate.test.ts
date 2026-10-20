import { beforeEach, describe, expect, test, vi } from "vitest";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import type { UpdateProgress } from "./autoUpdate";

const check = vi.fn();
const updaterAvailable = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("./api", () => ({ updaterAvailable: () => updaterAvailable() }));

const { checkForUpdate, runStartupUpdateCheck } = await import("./autoUpdate");

/** An update whose download replays `events` through the progress callback. */
function updateEmitting(events: DownloadEvent[]) {
  return {
    version: "1.2.0",
    currentVersion: "1.1.0",
    body: " notes ",
    downloadAndInstall: async (onEvent: (e: DownloadEvent) => void) => {
      for (const e of events) onEvent(e);
    },
  };
}

describe("checkForUpdate", () => {
  beforeEach(() => {
    check.mockReset();
    vi.spyOn(console, "info").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  test("reports the download from found through installing", async () => {
    check.mockResolvedValue(
      updateEmitting([
        { event: "Started", data: { contentLength: 300 } },
        { event: "Progress", data: { chunkLength: 100 } },
        { event: "Progress", data: { chunkLength: 200 } },
        { event: "Finished" },
      ]),
    );
    const seen: UpdateProgress[] = [];

    const result = await checkForUpdate((p) => seen.push(p));

    expect(result).toEqual({ kind: "staged", version: "1.2.0", notes: "notes" });
    expect(seen[0]).toEqual({ version: "1.2.0", phase: "downloading", received: 0, total: null });
    expect(seen[1]).toMatchObject({ phase: "downloading", total: 300 });
    // The burst of chunks is throttled, but the final report carries every byte.
    expect(seen.at(-1)).toEqual({ version: "1.2.0", phase: "installing", received: 300, total: 300 });
    expect(seen.filter((p) => p.phase === "downloading" && p.received > 0).length).toBe(1);
  });

  test("reports nothing when already up to date", async () => {
    check.mockResolvedValue(null);
    const onProgress = vi.fn();

    expect(await checkForUpdate(onProgress)).toEqual({ kind: "uptodate" });
    expect(onProgress).not.toHaveBeenCalled();
  });

  test("never throws: a failed check comes back as an error result", async () => {
    check.mockRejectedValue(new Error("offline"));

    expect(await checkForUpdate()).toEqual({ kind: "error", message: "Error: offline" });
  });
});

describe("runStartupUpdateCheck", () => {
  beforeEach(() => {
    check.mockReset();
    updaterAvailable.mockReset();
    vi.spyOn(console, "warn").mockImplementation(() => {});
    // The startup check is a no-op in dev; the tests run as a release build would.
    vi.stubEnv("DEV", false);
  });

  test("does not ask an install that cannot update itself", async () => {
    updaterAvailable.mockResolvedValue(false);
    const onStaged = vi.fn();

    await runStartupUpdateCheck(onStaged);

    expect(check).not.toHaveBeenCalled();
    expect(onStaged).not.toHaveBeenCalled();
  });

  test("checks when the install can update itself", async () => {
    updaterAvailable.mockResolvedValue(true);
    check.mockResolvedValue(null);

    await runStartupUpdateCheck(vi.fn());

    expect(check).toHaveBeenCalledTimes(1);
  });
});

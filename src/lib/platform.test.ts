import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/api/core")>()),
  invoke: (...args: unknown[]) => invoke(...args),
}));

/** A fresh copy of the module, so each test starts before the platform is loaded. */
const fresh = () => import("./platform");

describe("platform", () => {
  beforeEach(() => {
    invoke.mockReset();
    vi.resetModules();
  });
  afterEach(() => vi.unstubAllGlobals());

  test("reads macOS from the backend", async () => {
    invoke.mockResolvedValue("macos");
    const { loadPlatform, isMac, fileManager, modKey, modHint } = await fresh();

    await loadPlatform();

    expect(invoke).toHaveBeenCalledWith("platform");
    expect(isMac()).toBe(true);
    expect(fileManager()).toBe("Finder");
    expect(modKey()).toBe("⌘");
    expect(modHint("R")).toBe("⌘R");
    expect(modHint("Enter")).toBe("⌘⏎");
    expect(modHint("Shift+.")).toBe("⌘⇧.");
  });

  test("reads Linux from the backend", async () => {
    invoke.mockResolvedValue("linux");
    const { loadPlatform, isMac, fileManager, modKey, modHint } = await fresh();

    await loadPlatform();

    expect(isMac()).toBe(false);
    expect(fileManager()).toBe("file manager");
    expect(modKey()).toBe("Ctrl");
    expect(modHint("R")).toBe("Ctrl+R");
    expect(modHint("Enter")).toBe("Ctrl+Enter");
    expect(modHint("Shift+.")).toBe("Ctrl+Shift+.");
  });

  test("asks the backend only once", async () => {
    invoke.mockResolvedValue("linux");
    const { loadPlatform } = await fresh();

    await loadPlatform();
    await loadPlatform();

    expect(invoke).toHaveBeenCalledTimes(1);
  });

  test("falls back to the user agent when invoke is unavailable", async () => {
    invoke.mockRejectedValue(new TypeError("window.__TAURI_INTERNALS__ is undefined"));
    vi.stubGlobal("navigator", { platform: "MacIntel", userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)" });
    const { loadPlatform, isMac } = await fresh();

    await expect(loadPlatform()).resolves.toBeUndefined();

    expect(isMac()).toBe(true);
  });

  test("guesses Linux from a Linux user agent", async () => {
    invoke.mockRejectedValue(new TypeError("window.__TAURI_INTERNALS__ is undefined"));
    vi.stubGlobal("navigator", { platform: "Linux x86_64", userAgent: "Mozilla/5.0 (X11; Linux x86_64)" });
    const { loadPlatform, isMac } = await fresh();

    await loadPlatform();

    expect(isMac()).toBe(false);
  });

  test("guesses before the backend has answered", async () => {
    vi.stubGlobal("navigator", { platform: "Linux x86_64", userAgent: "Mozilla/5.0 (X11; Linux x86_64)" });
    const { isMac } = await fresh();

    expect(isMac()).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });
});

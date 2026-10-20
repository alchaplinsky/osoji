import { platform } from "./api";

// The OS name as the backend reports it ("macos", "linux"); null until `loadPlatform` answers.
let current: string | null = null;

/** Without a Tauri backend (the dev gallery in a plain browser) the user agent has to do. */
function fromUserAgent(): string {
  if (typeof navigator === "undefined") return "linux";
  return /mac/i.test(`${navigator.platform ?? ""} ${navigator.userAgent ?? ""}`) ? "macos" : "linux";
}

/**
 * Asks the backend which OS this is, once. Call it before the first render so `isMac()` and the
 * helpers below answer synchronously everywhere. Never rejects: when `invoke` is unavailable
 * the answer is guessed from the user agent.
 */
export async function loadPlatform(): Promise<void> {
  if (current === null) current = await platform().catch(fromUserAgent);
}

/** True on macOS. Before `loadPlatform` has answered this is the user-agent guess. */
export const isMac = () => (current ?? fromUserAgent()) === "macos";

/** What the OS calls its file browser, for copy like `Reveal in ${fileManager()}`. */
export const fileManager = () => (isMac() ? "Finder" : "file manager");

/** The primary modifier as shortcut hints show it. */
export const modKey = () => (isMac() ? "⌘" : "Ctrl");

const MAC_GLYPHS: Record<string, string> = { Enter: "⏎", Shift: "⇧" };

/** A hint for a primary-modifier shortcut: `modHint("Shift+.")` is "⌘⇧." on macOS and "Ctrl+Shift+." elsewhere. */
export function modHint(keys: string): string {
  const parts = keys.split("+");
  return isMac() ? modKey() + parts.map((p) => MAC_GLYPHS[p] ?? p).join("") : [modKey(), ...parts].join("+");
}

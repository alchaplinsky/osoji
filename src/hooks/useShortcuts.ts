import { useEffect, useRef } from "react";

// Keys are combos like "Meta+r" or "Meta+Enter": modifiers in Ctrl, Alt, Shift, Meta order, then the key.
export type Shortcuts = Record<string, () => void>;

export function comboOf(e: KeyboardEvent): string {
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Meta"].filter(Boolean);
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  return [...mods, key].join("+");
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT";
}

// One window keydown listener for the whole app. Matching combos are preventDefault-ed so the
// webview does not act on them (⌘R would otherwise reload it).
export function useShortcuts(shortcuts: Shortcuts, enabled = true) {
  const latest = useRef(shortcuts);
  latest.current = shortcuts;

  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || isEditable(e.target)) return;
      const run = latest.current[comboOf(e)];
      if (!run) return;
      e.preventDefault();
      run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled]);
}

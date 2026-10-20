import { describe, expect, test, vi } from "vitest";
import { comboOf, modCombo } from "./useShortcuts";

const platform = vi.hoisted(() => ({ mac: true }));
vi.mock("../lib/platform", () => ({ isMac: () => platform.mac }));

const key = (k: string, mods: Partial<KeyboardEvent> = {}) =>
  ({ key: k, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods }) as KeyboardEvent;

describe("comboOf", () => {
  test("names command shortcuts", () => {
    expect(comboOf(key("r", { metaKey: true }))).toBe("Meta+r");
    expect(comboOf(key("Enter", { metaKey: true }))).toBe("Meta+Enter");
  });

  test("lowercases letters and keeps the modifiers apart", () => {
    expect(comboOf(key("R", { metaKey: true, shiftKey: true }))).toBe("Shift+Meta+r");
    expect(comboOf(key("r"))).toBe("r");
  });
});

describe("modCombo", () => {
  test("is the command key on macOS, matching what comboOf names", () => {
    platform.mac = true;
    expect(modCombo("r")).toBe("Meta+r");
    expect(modCombo("Enter")).toBe(comboOf(key("Enter", { metaKey: true })));
  });

  test("is the control key elsewhere, matching what comboOf names", () => {
    platform.mac = false;
    expect(modCombo("r")).toBe("Ctrl+r");
    expect(modCombo("Enter")).toBe(comboOf(key("Enter", { ctrlKey: true })));
  });
});

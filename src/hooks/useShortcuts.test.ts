import { describe, expect, test } from "vitest";
import { comboOf } from "./useShortcuts";

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

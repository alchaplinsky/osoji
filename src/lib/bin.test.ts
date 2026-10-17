import { describe, expect, test } from "vitest";
import { reviewWarning, toBinItem, withItem } from "./bin";
import type { BinItem, Guard } from "./types";

const item = (id: number, guard: Guard | null = null, ancestors: number[] = []): BinItem => ({
  id,
  name: `item${id}`,
  path: `/x/item${id}`,
  size: 1,
  dir: false,
  ancestors,
  guard,
});

const PROTECTED: Guard = { kind: "protected", reason: "Part of macOS." };
const RISKY: Guard = { kind: "risky", reason: "Your photo library." };
const TRASH: Guard = { kind: "trash", reason: "Already in the Trash." };

describe("withItem", () => {
  test("never queues protected items or the Trash", () => {
    const queued = [item(1)];
    expect(withItem(queued, item(2, PROTECTED))).toBe(queued);
    expect(withItem(queued, item(3, TRASH))).toBe(queued);
  });

  test("queues risky items and lets an ancestor replace its descendants", () => {
    expect(withItem([item(3, null, [2])], item(2, RISKY)).map((b) => b.id)).toEqual([2]);
  });
});

describe("toBinItem", () => {
  test("carries the guard", () => {
    expect(toBinItem({ id: 1, name: "a", path: "/a", size: 1, mtime: 0, dir: false, ancestors: [], guard: RISKY }).guard).toEqual(RISKY);
  });
});

describe("reviewWarning", () => {
  test("is empty when nothing needs a second look", () => {
    expect(reviewWarning([item(1)], 0)).toBeUndefined();
  });

  test("names the first risky reason and combines with the size note", () => {
    expect(reviewWarning([item(1, RISKY)], 0)).toBe("1 item is marked risky: Your photo library. Double-check before you continue.");
    expect(reviewWarning([item(1, RISKY), item(2, RISKY)], 1)).toBe(
      "2 items are marked risky: Your photo library. 1 item is over 1 GB. Double-check before you continue.",
    );
    expect(reviewWarning([item(1)], 2)).toBe("2 items are over 1 GB. Double-check before you continue.");
  });
});

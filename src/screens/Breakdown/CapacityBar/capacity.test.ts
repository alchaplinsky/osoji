import { describe, expect, test } from "vitest";
import type { ViewNode } from "../../../lib/api";
import { GB } from "../../../lib/format";
import type { Volume } from "../../../lib/types";
import { hiddenBytes, hiddenNote } from "./capacity";
const volume: Volume = { name: "Macintosh HD", mount: "/", total: 1000 * GB, free: 400 * GB };

const node = (name: string, size: number): ViewNode => ({
  id: 1,
  name,
  size,
  items: 1,
  dir: true,
  error: false,
  children: [],
  other: 0,
  other_count: 0,
});

describe("hiddenBytes", () => {
  test("is the used space a volume-root scan did not count", () => {
    expect(hiddenBytes(volume, node("/", 570 * GB))).toBe(30 * GB);
  });

  test("is 0 when the scan root is not the volume root", () => {
    expect(hiddenBytes(volume, node("/Users/alex", 120 * GB))).toBe(0);
  });

  test("is 0 when the scan counted more than the volume uses", () => {
    expect(hiddenBytes(volume, node("/", 640 * GB))).toBe(0);
  });
});

describe("hiddenNote", () => {
  test("mentions unreadable folders only when there are some", () => {
    expect(hiddenNote()).not.toContain("could not be read.");
    expect(hiddenNote(0)).not.toContain("could not be read.");
    expect(hiddenNote(871)).toMatch(/ 871 folders could not be read\.$/);
  });
});

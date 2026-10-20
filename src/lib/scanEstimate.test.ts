import { describe, expect, test } from "vitest";
import { GB } from "./format";
import { expectedBytes, scanFraction } from "./scanEstimate";
import type { Volume } from "./types";
const volume: Volume = { name: "Macintosh HD", mount: "/", total: 1000 * GB, free: 400 * GB };

describe("expectedBytes", () => {
  test("prefers the previous scan of the same root", () => {
    expect(expectedBytes(volume, "/", 540 * GB)).toBe(540 * GB);
    expect(expectedBytes(volume, "/Users/alex", 120 * GB)).toBe(120 * GB);
  });

  test("falls back to the volume's used space only for the volume root", () => {
    expect(expectedBytes(volume, "/", null)).toBe(600 * GB);
    expect(expectedBytes(volume, "/Users/alex", null)).toBeNull();
  });

  test("has nothing to estimate from without a volume", () => {
    expect(expectedBytes(null, "/", null)).toBeNull();
  });
});

describe("scanFraction", () => {
  test("is a clamped share of the expected bytes", () => {
    expect(scanFraction(300 * GB, 600 * GB)).toBeCloseTo(0.5);
    expect(scanFraction(0, 600 * GB)).toBe(0);
  });

  test("never reaches 100% while the scan is still running", () => {
    expect(scanFraction(900 * GB, 600 * GB)).toBe(0.99);
  });

  test("is indeterminate when the expected total is unknown", () => {
    expect(scanFraction(300 * GB, null)).toBeNull();
  });
});

import { describe, expect, test } from "vitest";
import { GB } from "./format";
import { expectedScan, scanFraction, type ScanExpectation } from "./scanEstimate";
import type { Volume } from "./types";

const volume: Volume = { name: "Macintosh HD", mount: "/", total: 1000 * GB, free: 400 * GB };
const found = (bytes: number, entries: number) => ({ bytes, files: entries - 1, dirs: 1 });

describe("expectedScan", () => {
  test("prefers the previous scan of the same root", () => {
    expect(expectedScan(volume, "/", { bytes: 540 * GB, entries: 5000 })).toEqual({ bytes: 540 * GB, entries: 5000 });
    expect(expectedScan(volume, "/Users/alex", { bytes: 120 * GB, entries: 800 })).toEqual({ bytes: 120 * GB, entries: 800 });
  });

  test("falls back to the volume's used space only for the volume root, without an entry count", () => {
    expect(expectedScan(volume, "/", null)).toEqual({ bytes: 600 * GB, entries: null });
    expect(expectedScan(volume, "/Users/alex", null)).toBeNull();
  });

  test("has nothing to estimate from without a volume", () => {
    expect(expectedScan(null, "/", null)).toBeNull();
  });
});

describe("scanFraction", () => {
  const bytesOnly: ScanExpectation = { bytes: 600 * GB, entries: null };
  const full: ScanExpectation = { bytes: 600 * GB, entries: 1000 };

  test("is a clamped share of the expected bytes when only those are known", () => {
    expect(scanFraction(found(300 * GB, 10), bytesOnly)).toBeCloseTo(0.5);
    expect(scanFraction(found(0, 0), bytesOnly)).toBe(0);
    expect(scanFraction(null, bytesOnly)).toBe(0);
  });

  test("follows the slower of bytes and entries", () => {
    // A few big files early: nearly all the bytes, a tenth of the entries.
    expect(scanFraction(found(594 * GB, 100), full)).toBeCloseTo(0.1);
    // Many small files first: half the entries, a tenth of the bytes.
    expect(scanFraction(found(60 * GB, 500), full)).toBeCloseTo(0.1);
    expect(scanFraction(found(300 * GB, 500), full)).toBeCloseTo(0.5);
  });

  test("never reaches 100% while the scan is still running", () => {
    expect(scanFraction(found(900 * GB, 10), bytesOnly)).toBe(0.99);
    expect(scanFraction(found(900 * GB, 2000), full)).toBe(0.99);
  });

  test("is indeterminate when the expected total is unknown", () => {
    expect(scanFraction(found(300 * GB, 10), null)).toBeNull();
  });
});

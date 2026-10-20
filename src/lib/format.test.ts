import { describe, expect, test } from "vitest";
import { GB, humanBytes } from "./format";

describe("humanBytes", () => {
  test("uses decimal units, as Finder does", () => {
    expect(humanBytes(994_662_584_320)).toBe("994.7 GB");
    expect(humanBytes(GB)).toBe("1.00 GB");
    expect(humanBytes(1_500_000)).toBe("1.50 MB");
    expect(humanBytes(999)).toBe("999 B");
    expect(humanBytes(1000)).toBe("1.00 KB");
  });

  test("shows two decimals below ten and one above", () => {
    expect(humanBytes(9_990_000_000)).toBe("9.99 GB");
    expect(humanBytes(12_300_000_000)).toBe("12.3 GB");
  });
});

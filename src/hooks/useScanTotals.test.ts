import { describe, expect, test } from "vitest";
import { parseTotal } from "./useScanTotals";

describe("parseTotal", () => {
  test("keeps a stored record", () => {
    expect(parseTotal({ bytes: 500, entries: 20 })).toEqual({ bytes: 500, entries: 20 });
  });

  test("keeps a byte-only total from before the entry count was stored", () => {
    expect(parseTotal(500)).toEqual({ bytes: 500, entries: null });
    expect(parseTotal({ bytes: 500 })).toEqual({ bytes: 500, entries: null });
  });

  test("drops what cannot pace a scan", () => {
    expect(parseTotal(0)).toBeNull();
    expect(parseTotal({ bytes: 0, entries: 20 })).toBeNull();
    expect(parseTotal({ bytes: 500, entries: 0 })).toEqual({ bytes: 500, entries: null });
    expect(parseTotal("500")).toBeNull();
    expect(parseTotal(null)).toBeNull();
  });
});

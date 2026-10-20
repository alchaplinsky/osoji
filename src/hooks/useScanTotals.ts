import { useCallback, useEffect, useState } from "react";
import type { ScanTotal } from "../lib/scanEstimate";

const STORAGE_KEY = "osoji.scanTotals";

/** What each root was found to hold by its last completed scan, kept across launches. */
export type ScanTotals = {
  get: (root: string) => ScanTotal | null;
  remember: (root: string, total: ScanTotal) => void;
};

/** A stored record, or a plain byte total from before the entry count was kept: its bytes still pace a scan. */
export function parseTotal(v: unknown): ScanTotal | null {
  if (typeof v === "number") return v > 0 ? { bytes: v, entries: null } : null;
  if (typeof v !== "object" || v === null) return null;
  const { bytes, entries } = v as Partial<ScanTotal>;
  if (typeof bytes !== "number" || bytes <= 0) return null;
  return { bytes, entries: typeof entries === "number" && entries > 0 ? entries : null };
}

function load(): Record<string, ScanTotal> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (!stored || typeof stored !== "object") return {};
    const totals: Record<string, ScanTotal> = {};
    for (const [root, v] of Object.entries(stored)) {
      const total = parseTotal(v);
      if (total) totals[root] = total;
    }
    return totals;
  } catch {
    return {};
  }
}

export function useScanTotals(): ScanTotals {
  const [totals, setTotals] = useState<Record<string, ScanTotal>>(load);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(totals));
  }, [totals]);

  const get = useCallback((root: string) => totals[root] ?? null, [totals]);
  const remember = useCallback((root: string, total: ScanTotal) => {
    if (total.bytes <= 0) return;
    setTotals((t) => (t[root]?.bytes === total.bytes && t[root]?.entries === total.entries ? t : { ...t, [root]: total }));
  }, []);

  return { get, remember };
}
